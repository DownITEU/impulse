//! The map's keys: Escape and Ctrl+F anywhere in it, zoom and pan while the map itself has focus.

use gpui_kit::{App, KeyBinding, actions};

actions!(map_view, [CloseMap, FindPlace, ZoomIn, ZoomOut, PanLeft, PanRight, PanUp, PanDown]);

pub const CONTEXT: &str = "MapView";
// Keys that would type into the search field act only while the map itself has focus.
pub const STAGE: &str = "MapStage";

pub fn bind_keys(cx: &mut App) {
    let stage = Some(STAGE);
    cx.bind_keys([
        KeyBinding::new("escape", CloseMap, Some(CONTEXT)),
        KeyBinding::new("ctrl-f", FindPlace, Some(CONTEXT)),
        KeyBinding::new("=", ZoomIn, stage),
        KeyBinding::new("+", ZoomIn, stage),
        KeyBinding::new("-", ZoomOut, stage),
        KeyBinding::new("left", PanLeft, stage),
        KeyBinding::new("right", PanRight, stage),
        KeyBinding::new("up", PanUp, stage),
        KeyBinding::new("down", PanDown, stage),
    ]);
}
