//! Drawing inside a game's own frame: a source's overlay loaded into the game, which shows the textures it is given.

use futures::stream::BoxStream;

use crate::{Failure, Process, Ref, Source};

pub trait Overlays: Source {
    /// Blocking. Loads the source's overlay into `process`, the running `game`, once per process, and links to it.
    /// `Unsupported`: not a game it draws in; `Refused`: another host holds the overlay (the source's own app runs).
    fn enter(&self, game: &Ref, process: &Process) -> Result<Canvas, Failure>;
}

/// The overlay inside one game. Dropping it closes what it shows and the link; the overlay stays in the game.
pub struct Canvas {
    pub layers: Box<dyn Layers>,
    /// Ends when the overlay can no longer draw: the link is lost, a texture it cannot open. A game that closes
    /// without a word may leave it open: its run's end drops the canvas.
    pub events: BoxStream<'static, CanvasEvent>,
}

/// Calls queue and return at once: they come from the UI thread, inside a draw.
pub trait Layers: Send {
    /// Shows `layer`, or moves it or gives it a new frame.
    fn show(&self, layer: u32, rect: Rect, frame: Texture);
    fn hide(&self, layer: u32);
    /// `layer` takes the mouse over its rect, and the keys once clicked; the game gets neither.
    fn interact(&self, layer: u32, on: bool);
    /// The overlay takes all of the game's input and shows the cursor.
    fn block(&self, on: bool);
    /// The pointer's shape over `layer`.
    fn cursor(&self, layer: u32, cursor: Cursor);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cursor {
    #[default]
    Arrow,
    Text,
    Cross,
    Hand,
    ResizeLeftRight,
    ResizeUpDown,
    /// Up-left and down-right.
    ResizeFalling,
    /// Up-right and down-left.
    ResizeRising,
    NotAllowed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasEvent {
    /// The overlay in the game answered: layers show from now on. Its size follows once known.
    Ready,
    /// The game's frame as the overlay draws in it, in the game's pixels: a layer placed by it moves.
    Resized { width: u32, height: u32 },
    /// A Win32 input message to an interactive layer as the game got it, its point relative to the layer's rect.
    Input { layer: u32, message: u32, wparam: usize, lparam: isize },
}

/// In the game's client area, physical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// A drawn frame in a GPU texture another process opens by its shared handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Texture {
    /// The legacy (KMT) shared handle, a number valid in any process.
    pub handle: usize,
    pub width: u32,
    pub height: u32,
    /// The PCI device id of the adapter it lives on: only a game on the same one can open it.
    pub adapter: u32,
}
