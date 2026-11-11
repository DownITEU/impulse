//! The map itself: tiles, regions, labels, pins and the player, moved and zoomed, with what floats over it.
//! A view of its own, so what moves each frame (a glide, a lifted marker, a drag) repaints the map alone.

mod camera;
mod input;
mod motion;
mod overlays;
mod paint;
mod pins;
mod tiles;

use std::{ops::RangeInclusive, path::PathBuf, rc::Rc, sync::Arc, time::Instant};

use gpui_kit::{
    AnyElement, App, Bounds, Context, Entity, FocusHandle, HitboxBehavior, IntoElement, Pixels, Render, RenderImage, SharedString, Task, Window,
    canvas, div, prelude::*, px, rgb,
};
use model::{Map, Point};
use rustc_hash::FxHashMap as HashMap;

use super::{
    board::{Board, BoardEvent},
    keys::STAGE,
    markers::Images,
};
use crate::{Load, widgets::notice};
use camera::{Camera, Glide, Pixel, extent};
use pins::Marker;
use tiles::Tiles;

struct Drag {
    last: gpui_kit::Point<Pixels>,
    moved: bool,
}

/// What was worked out from the shown map's pins or points at them: forgotten with the map.
#[derive(Default)]
struct Shown {
    // By the zoom step they were grouped for.
    markers: Option<(i32, Rc<Vec<Marker>>)>,
    // Made once, not per frame: each label's line, each region's extent (a frame skips those off the view), group counts.
    labels: Vec<SharedString>,
    regions: Vec<Option<(Point, Point)>>,
    counts: HashMap<usize, SharedString>,
    hovered: Option<usize>,
    // The hovered pin once the cursor has rested on it.
    chip: Option<usize>,
    hovered_region: Option<usize>,
    hovered_mark: Option<usize>,
    // Markers drawn larger or on their way back, with their size now.
    lifts: Vec<(usize, f64)>,
    // Where a right click opened its menu.
    target: Option<Point>,
}

impl Shown {
    fn of(map: &Map) -> Self {
        Self {
            labels: map.labels.iter().map(|label| label.text.replace('\n', " ").into()).collect(),
            regions: map.regions.iter().map(|region| extent(region.outline.iter().copied())).collect(),
            ..Self::default()
        }
    }
}

pub struct Stage {
    board: Entity<Board>,
    tile_dir: PathBuf,
    // Counts the tile sets shown: a tile loaded for an earlier one is dropped.
    epoch: usize,
    tiles: Tiles,
    images: Images,
    // Images to drop from the atlas when the map is next drawn.
    stale: Vec<Arc<RenderImage>>,
    // `None` until the map is first painted: the view's size decides it.
    camera: Option<Camera>,
    glide: Option<Glide>,
    // When the last frame that moved something was painted.
    clock: Option<Instant>,
    // As last painted: the map's place in the window and a tile's side in its pixels.
    bounds: Bounds<Pixels>,
    tile: f64,
    shown: Shown,
    drag: Option<Drag>,
    layers_open: bool,
    // The view moves with the player.
    follow: bool,
    focus: FocusHandle,
    _chip: Task<()>,
}

impl Stage {
    pub fn new(board: Entity<Board>, tile_dir: PathBuf, cx: &mut Context<Self>) -> Self {
        cx.observe(&board, |stage, board, cx| {
            stage.followed(board.read(cx));
            cx.notify();
        })
        .detach();
        cx.subscribe(&board, |stage, board, event: &BoardEvent, cx| match event {
            BoardEvent::Installed { first, retiled } => stage.installed(*first, *retiled, board.read(cx)),
            BoardEvent::Moved => {
                stage.followed(board.read(cx));
                cx.notify();
            }
            BoardEvent::Filtered => {
                stage.shown.markers = None;
                stage.hover(None, cx);
            }
            BoardEvent::Restyled => stage.stale.extend(stage.images.clear()),
            BoardEvent::Selected => (stage.shown.target, stage.shown.hovered_mark) = (None, None),
            BoardEvent::MarkAdded => {}
            BoardEvent::Bring { to, close } => {
                if let Load::Ready(map) = &board.read(cx).map {
                    stage.bring(*to, *close, map);
                }
            }
        })
        .detach();
        Self {
            board,
            tile_dir,
            epoch: 0,
            tiles: Tiles::default(),
            images: Images::default(),
            stale: Vec::new(),
            camera: None,
            glide: None,
            clock: None,
            bounds: Bounds::default(),
            tile: 256.,
            shown: Shown::default(),
            drag: None,
            layers_open: false,
            follow: false,
            focus: cx.focus_handle(),
            _chip: Task::ready(()),
        }
    }

    pub fn take_focus(&self, window: &mut Window, cx: &mut App) {
        self.focus.focus(window, cx);
    }

    /// The camera's center and span, to keep with the map's state.
    pub fn view(&self) -> Option<(f64, f64, f64)> {
        self.camera.map(|camera| (camera.center.x, camera.center.y, camera.span))
    }

    /// Lets go of what only an open map needs.
    pub fn hide(&mut self, window: &mut Window) {
        // Markers are grouped again on the next frame.
        (self.drag, self.layers_open, self.shown.markers) = (None, false, None);
        self.drop_tiles();
        self.stale.extend(self.images.clear());
        for image in self.stale.drain(..) {
            let _ = window.drop_image(image);
        }
    }

    /// Escape: the layers or the right click menu; false when neither was open.
    pub fn close_top(&mut self, cx: &mut Context<Self>) -> bool {
        let open = self.layers_open || self.shown.target.is_some();
        if self.layers_open {
            self.layers_open = false;
        } else {
            self.shown.target = None;
        }
        cx.notify();
        open
    }

    /// A fresher copy of the same map keeps its tiles; its icons may have changed.
    fn installed(&mut self, first: bool, retiled: bool, board: &Board) {
        let Load::Ready(map) = &board.map else { return };
        if retiled {
            self.drop_tiles();
        }
        self.stale.extend(self.images.clear());
        (self.shown, self.glide, self.clock) = (Shown::of(map), None, None);
        if first {
            self.camera = board.saved.view.map(|(x, y, span)| Camera { center: Point { x, y }, span });
        }
    }

    // Loads on their way report to an epoch that is gone.
    fn drop_tiles(&mut self) {
        self.epoch += 1;
        self.stale.extend(self.tiles.clear());
    }

    /// Centers on `to`; `close` also zooms in near enough to tell pins apart.
    fn bring(&mut self, to: Point, close: bool, map: &Map) {
        let Some(camera) = &mut self.camera else { return };
        camera.center = to;
        if close {
            // Not deeper: many maps declare levels their CDN does not have, which shows as blur.
            let level = self.tile * 2f64.powi(i32::from(map.tiles.max_zoom.saturating_sub(3).max(map.tiles.min_zoom)));
            camera.span = camera.span.max(level);
        }
        (self.glide, self.follow) = (None, false);
    }

    // A run that ended leaves nothing to follow.
    fn followed(&mut self, board: &Board) {
        self.follow &= board.player.is_some();
        if self.follow {
            self.center_on_player(board);
        }
    }

    fn center_on_player(&mut self, board: &Board) {
        let (Some(camera), Load::Ready(map), Some(position)) = (&mut self.camera, &board.map, board.player) else { return };
        if let Some(frame) = &map.frame {
            camera.center = frame.to_map(position);
        }
    }

    /// The shown map, once loaded.
    fn map(&self, cx: &App) -> Option<Rc<Map>> {
        match &self.board.read(cx).map {
            Load::Ready(map) => Some(map.clone()),
            _ => None,
        }
    }

    fn size(&self) -> Pixel {
        (self.bounds.size.width.into(), self.bounds.size.height.into())
    }

    fn pixel(&self, position: gpui_kit::Point<Pixels>) -> Pixel {
        ((position.x - self.bounds.origin.x).into(), (position.y - self.bounds.origin.y).into())
    }

    /// From half the smallest level to twice the largest.
    fn spans(&self, map: &Map) -> RangeInclusive<f64> {
        let level = |zoom: u8| self.tile * 2f64.powi(zoom.into());
        level(map.tiles.min_zoom) / 2.0..=level(map.tiles.max_zoom) * 2.
    }
}

impl Render for Stage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for image in self.tiles.begin_frame().into_iter().chain(self.stale.drain(..)) {
            let _ = window.drop_image(image);
        }
        let board = self.board.clone();
        let board = board.read(cx);
        // Keeps the focus in the map, so Escape still closes it.
        let waiting = |tag, title, text: &str| -> AnyElement { div().size_full().p(px(24.)).track_focus(&self.focus).child(notice(tag, title, text)).into_any_element() };
        let map = match &board.map {
            Load::Ready(map) => map.clone(),
            Load::Pending => return waiting("[ LOADING ]", "LOADING THE MAP", "Pins and tiles are kept on disk after the first time."),
            Load::Failed(why) => return waiting("[ NO MAP ]", "THE MAP IS UNAVAILABLE", &format!("It could not be loaded: {why}.")),
        };
        let view = cx.entity();
        let stage = div()
            .id("map-stage")
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(rgb(map.background))
            .track_focus(&self.focus)
            .key_context(STAGE);
        self.bind_keys(stage, cx)
            .child(
                canvas(
                    |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
                    move |bounds, hitbox, window, cx| view.update(cx, |stage, cx| stage.paint(bounds, hitbox, window, cx)),
                )
                .size_full(),
            )
            .children(self.chip(&map, board))
            .children(self.target_menu(&map, board, cx))
            .child(self.readout(board))
            .child(self.buttons(&map, board, cx))
            .into_any_element()
    }
}
