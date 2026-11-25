//! The live game's overlay in the IN-GAME mode: the canvas its source loads into the game, asked for when a run goes
//! live and dropped when it ends. The in-game HUD (`Layer`) and `Glance` draw their layers on it; whatever fails
//! leaves them over the game.

use std::{rc::Rc, sync::Arc};

use blocking::unblock;
use client::{Registry, library::GameId};
use futures::StreamExt;
use gpui_kit::{App, Context, CursorStyle, Entity, EventEmitter, MirrorFrame, Task};
use model::{Canvas, CanvasEvent, Cursor, Failure, Layers, Rect, Texture};

use crate::{
    run::{RunEvent, Runs},
    settings::Settings,
    trainer::{TrainerEvent, Trainers},
};

/// The HUD's layer, the whole game's frame with the input, and the minimap window's.
pub(super) const HUD_LAYER: u32 = 0;
pub(super) const GLANCE_LAYER: u32 = 1;

#[derive(Clone, Copy)]
pub(super) enum InGameEvent {
    /// What the current canvas says.
    Canvas(CanvasEvent),
    /// The canvas was dropped or ended.
    Gone,
}

pub struct InGame {
    registry: Arc<Registry>,
    runs: Entity<Runs>,
    settings: Entity<Settings>,
    trainers: Entity<Trainers>,
    // IN-GAME is the live game's MODE: its own, else the OVERLAY tab's.
    wanted: bool,
    // Bumped whenever the canvas is dropped or asked for again: an answer or an event of an older one is ignored.
    generation: u64,
    // One ask at a time: an older one landing later would take the game's canvas from the newer.
    asking: bool,
    // Frame callbacks hold a `Weak`, so dropping this closes the layers whatever they do.
    layers: Option<Rc<dyn Layers>>,
    ready: bool,
    size: Option<(u32, u32)>,
    reader: Option<Task<()>>,
}

impl EventEmitter<InGameEvent> for InGame {}

impl InGame {
    pub fn new(
        registry: Arc<Registry>,
        runs: &Entity<Runs>,
        settings: &Entity<Settings>,
        trainers: &Entity<Trainers>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(settings, |in_game, _, cx| in_game.rewant(cx)).detach();
        cx.subscribe(trainers, |in_game, _, event: &TrainerEvent, cx| {
            if matches!(event, TrainerEvent::Changed(game) if in_game.runs.read(cx).is_live(game)) {
                in_game.rewant(cx);
            }
        })
        .detach();
        cx.subscribe(runs, |in_game, _, event: &RunEvent, cx| match event {
            RunEvent::Live => {
                in_game.wanted = in_game.wants(cx);
                in_game.enter(cx);
            }
            RunEvent::Pressed(_) => {}
            RunEvent::Starting | RunEvent::Failed | RunEvent::Stopping | RunEvent::Ended => in_game.leave(cx),
        })
        .detach();
        let (runs, settings, trainers) = (runs.clone(), settings.clone(), trainers.clone());
        let wanted = settings.read(cx).prefs().overlay_in_game;
        let reader = None;
        Self { registry, runs, settings, trainers, wanted, generation: 0, asking: false, layers: None, ready: false, size: None, reader }
    }

    fn wants(&self, cx: &App) -> bool {
        let own = self.runs.read(cx).live().and_then(|game| self.trainers.read(cx).game_profile(game).overlay_in_game);
        own.unwrap_or(self.settings.read(cx).prefs().overlay_in_game)
    }

    fn rewant(&mut self, cx: &mut Context<Self>) {
        let on = self.wants(cx);
        if on != self.wanted {
            self.wanted = on;
            self.enter(cx);
        }
    }

    /// The overlay in the game answered: layers show from now on.
    pub(super) fn ready(&self) -> bool {
        self.ready
    }

    /// The game's frame as the overlay draws in it, once ready and known.
    pub(super) fn size(&self) -> Option<(u32, u32)> {
        self.size.filter(|_| self.ready)
    }

    pub(super) fn layers(&self) -> Option<&dyn Layers> {
        self.layers.as_deref()
    }

    /// A window's frame callback: each frame goes into the game as `layer` at `at` in its client area; `first` runs
    /// after the first one.
    pub(super) fn frames(&self, layer: u32, (x, y): (i32, i32), first: impl FnOnce() + 'static) -> Option<Box<dyn FnMut(MirrorFrame)>> {
        let layers = Rc::downgrade(self.layers.as_ref()?);
        let mut first = Some(first);
        Some(Box::new(move |MirrorFrame { handle, width, height, adapter }| {
            let Some(layers) = layers.upgrade() else { return };
            layers.show(layer, Rect { x, y, width, height }, Texture { handle, width, height, adapter });
            if let Some(first) = first.take() {
                first();
            }
        }))
    }

    /// A window's cursor callback: the cursor it wants shows over `layer` in the game.
    pub(super) fn cursors(&self, layer: u32) -> Option<Box<dyn FnMut(CursorStyle)>> {
        let layers = Rc::downgrade(self.layers.as_ref()?);
        Some(Box::new(move |style| {
            if let Some(layers) = layers.upgrade() {
                layers.cursor(layer, cursor(style));
            }
        }))
    }

    /// Asks the live run's source for a canvas, off the UI thread, in the IN-GAME mode.
    fn enter(&mut self, cx: &mut Context<Self>) {
        self.leave(cx);
        // The ask under way asks again when it lands.
        if !self.asking {
            self.ask(cx);
        }
    }

    fn ask(&mut self, cx: &mut Context<Self>) {
        if !self.wanted {
            return;
        }
        let runs = self.runs.read(cx);
        let (Some(game), Some(process)) = (runs.live().and_then(GameId::game), runs.process().cloned()) else { return };
        self.asking = true;
        let (generation, registry) = (self.generation, self.registry.clone());
        cx.spawn(async move |in_game, cx| {
            let answer = unblock(move || registry.enter(&game, &process)).await;
            in_game
                .update(cx, |in_game, cx| {
                    in_game.asking = false;
                    if generation == in_game.generation {
                        in_game.arrived(generation, answer, cx);
                    } else {
                        drop(answer);
                        in_game.ask(cx);
                    }
                })
                .ok();
        })
        .detach();
    }

    /// The run is going or gone, another one is live or the mode changed: the canvas goes, a late answer with it.
    fn leave(&mut self, cx: &mut Context<Self>) {
        self.reader = None;
        self.drop_canvas(cx);
    }

    fn drop_canvas(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        let had = self.layers.take().is_some();
        (self.ready, self.size) = (false, None);
        if had {
            cx.emit(InGameEvent::Gone);
        }
    }

    fn arrived(&mut self, generation: u64, answer: Result<Canvas, Failure>, cx: &mut Context<Self>) {
        let Canvas { layers, mut events } = match answer {
            Ok(canvas) => canvas,
            // Not a game its source draws in, or another host holds the overlay: the HUD stays over the game.
            Err(failure) => return log::debug!("no in-game overlay: {:?}", failure.kind),
        };
        self.layers = Some(layers.into());
        self.reader = Some(cx.spawn(async move |in_game, cx| {
            while let Some(event) = events.next().await {
                if in_game.update(cx, |in_game, cx| in_game.canvas_event(generation, event, cx)).is_err() {
                    return;
                }
            }
            in_game.update(cx, |in_game, cx| in_game.canvas_ended(generation, cx)).ok();
        }));
    }

    fn canvas_event(&mut self, generation: u64, event: CanvasEvent, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        match event {
            CanvasEvent::Ready => self.ready = true,
            CanvasEvent::Resized { width, height } => self.size = Some((width, height)),
            CanvasEvent::Input { .. } => {}
        }
        cx.emit(InGameEvent::Canvas(event));
    }

    /// The overlay can no longer draw: back over the game.
    fn canvas_ended(&mut self, generation: u64, cx: &mut Context<Self>) {
        if generation == self.generation {
            log::debug!("the in-game overlay ended");
            self.drop_canvas(cx);
        }
    }
}

/// As GPUI shows it on Windows.
fn cursor(style: CursorStyle) -> Cursor {
    match style {
        CursorStyle::IBeam | CursorStyle::IBeamCursorForVerticalLayout => Cursor::Text,
        CursorStyle::Crosshair => Cursor::Cross,
        CursorStyle::PointingHand | CursorStyle::DragLink => Cursor::Hand,
        CursorStyle::ResizeLeft | CursorStyle::ResizeRight | CursorStyle::ResizeLeftRight | CursorStyle::ResizeColumn => {
            Cursor::ResizeLeftRight
        }
        CursorStyle::ResizeUp | CursorStyle::ResizeDown | CursorStyle::ResizeUpDown | CursorStyle::ResizeRow => Cursor::ResizeUpDown,
        CursorStyle::ResizeUpLeftDownRight => Cursor::ResizeFalling,
        CursorStyle::ResizeUpRightDownLeft => Cursor::ResizeRising,
        CursorStyle::OperationNotAllowed => Cursor::NotAllowed,
        _ => Cursor::Arrow,
    }
}
