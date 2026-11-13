//! Docks: panels floating over the overlay, each moved by its header (some by their content too) and, if it may be, sized by its
//! corner, shown and hidden from a toolbar. One layout for all games, kept in `docks.json`.

mod frame;

use std::path::{Path, PathBuf};

use gpui_kit::{Bounds, Context, MouseButton, MouseMoveEvent, Pixels, Point, Size, point, px, size};
use serde::{Deserialize, Serialize};

use frame::HEADER;
pub(super) use frame::{below_header, frame, input, toolbar};

use super::{assets::Icon, store};

const FILE: &str = "docks.json";

pub(super) struct Spec {
    pub id: &'static str,
    pub title: &'static str,
    pub icon: Icon,
    /// Where it opens first, on a screen this size.
    pub first: fn(Size<Pixels>) -> Bounds<Pixels>,
    /// `None`: it is only moved, and keeps its first size.
    pub smallest: Option<Size<Pixels>>,
    /// Until the user toggles it.
    pub shown: bool,
    /// Its content moves it too, so its header may leave the screen's top and the content reach the corner.
    pub body_moves: bool,
}

#[derive(Serialize, Deserialize)]
struct Placed {
    id: String,
    shown: bool,
    /// x, y, width, height in logical pixels; missing until moved or sized.
    #[serde(default)]
    bounds: Option<[f32; 4]>,
}

#[derive(Clone, Copy)]
struct Grab {
    spec: &'static Spec,
    resize: bool,
    from: Point<Pixels>,
    start: Bounds<Pixels>,
}

pub(super) struct Docks {
    file: PathBuf,
    specs: &'static [Spec],
    /// Back to front; a dock without an entry is as its spec says, behind the placed ones.
    placed: Vec<Placed>,
    grab: Option<Grab>,
}

impl Docks {
    /// A missing or damaged file means the first layout. Blocking, like the saves: the file is tiny.
    pub fn load(data_dir: &Path, specs: &'static [Spec]) -> Self {
        let file = data_dir.join(FILE);
        Self { placed: store::load(&file).unwrap_or_default(), file, specs, grab: None }
    }

    pub fn is_shown(&self, spec: &Spec) -> bool {
        self.find(spec.id).map_or(spec.shown, |placed| placed.shown)
    }

    /// The shown docks, back to front.
    pub fn shown(&self) -> Vec<&'static Spec> {
        let mut shown: Vec<_> = self.specs.iter().filter(|spec| self.is_shown(spec)).collect();
        shown.sort_by_key(|spec| self.placed.iter().position(|placed| placed.id == spec.id));
        shown
    }

    pub fn toggle(&mut self, spec: &Spec, cx: &mut Context<Self>) {
        let shown = !self.is_shown(spec);
        self.entry(spec.id).shown = shown;
        if shown {
            self.raise(spec.id);
        }
        self.save();
        cx.notify();
    }

    pub fn hide(&mut self, spec: &Spec, cx: &mut Context<Self>) {
        if self.is_shown(spec) {
            self.toggle(spec, cx);
        }
    }

    pub fn bounds(&self, spec: &Spec, screen: Size<Pixels>) -> Bounds<Pixels> {
        let kept =
            self.find(spec.id).and_then(|placed| placed.bounds).map(|[x, y, w, h]| Bounds::new(point(px(x), px(y)), size(px(w), px(h))));
        let first = (spec.first)(screen);
        let mut bounds = kept.unwrap_or(first);
        if spec.smallest.is_none() {
            bounds.size = first.size;
        }
        fit(bounds, spec, screen)
    }

    /// A press over a dock brings it to the front.
    fn press(&mut self, at: Point<Pixels>, screen: Size<Pixels>, cx: &mut Context<Self>) {
        let shown = self.shown();
        let Some(spec) = shown.iter().rev().find(|spec| self.bounds(spec, screen).contains(&at)) else {
            return;
        };
        if shown.last().is_some_and(|top| top.id != spec.id) {
            self.raise(spec.id);
            cx.notify();
        }
    }

    fn grab(&mut self, spec: &'static Spec, resize: bool, at: Point<Pixels>, screen: Size<Pixels>, cx: &mut Context<Self>) {
        self.grab = Some(Grab { spec, resize, from: at, start: self.bounds(spec, screen) });
        cx.notify();
    }

    fn follow(&mut self, event: &MouseMoveEvent, screen: Size<Pixels>, cx: &mut Context<Self>) {
        let Some(Grab { spec, resize, from, start }) = self.grab else {
            return;
        };
        // Its release happened outside the window.
        if event.pressed_button != Some(MouseButton::Left) {
            return self.release(cx);
        }
        let shift = event.position - from;
        let bounds = match resize {
            true => {
                let room = size(screen.width - start.origin.x, screen.height - start.origin.y);
                Bounds::new(start.origin, size(start.size.width + shift.x, start.size.height + shift.y).min(&room))
            }
            false => Bounds::new(start.origin + shift, start.size),
        };
        let Bounds { origin, size } = fit(bounds, spec, screen);
        self.entry(spec.id).bounds = Some([origin.x.into(), origin.y.into(), size.width.into(), size.height.into()]);
        cx.notify();
    }

    fn release(&mut self, cx: &mut Context<Self>) {
        if self.grab.take().is_some() {
            self.save();
            cx.notify();
        }
    }

    fn find(&self, id: &str) -> Option<&Placed> {
        self.placed.iter().find(|placed| placed.id == id)
    }

    fn entry(&mut self, id: &str) -> &mut Placed {
        let index = match self.placed.iter().position(|placed| placed.id == id) {
            Some(index) => index,
            None => {
                self.placed.push(Placed { id: id.to_owned(), shown: true, bounds: None });
                self.placed.len() - 1
            }
        };
        &mut self.placed[index]
    }

    fn raise(&mut self, id: &str) {
        let index = self.placed.iter().position(|placed| placed.id == id);
        let placed = index.map(|index| self.placed.remove(index)).unwrap_or(Placed { id: id.to_owned(), shown: true, bounds: None });
        self.placed.push(placed);
    }

    fn save(&self) {
        store::save(&self.file, &self.placed);
    }
}

/// On the screen but for a header it may hide, and no smaller than its smallest or larger than the screen.
fn fit(bounds: Bounds<Pixels>, spec: &Spec, screen: Size<Pixels>) -> Bounds<Pixels> {
    let size = bounds.size.max(&spec.smallest.unwrap_or_default()).min(&screen);
    let top = px(if spec.body_moves { -HEADER } else { 0. });
    let x = bounds.origin.x.clamp(px(0.), screen.width - size.width);
    let y = bounds.origin.y.clamp(top, screen.height - size.height);
    Bounds::new(point(x, y), size)
}
