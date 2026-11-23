//! The HUD inside the game's frame (the IN-GAME mode): Impulse's window, cloaked and never in front, takes the size the
//! overlay draws in and is mirrored into the canvas as its input layer. The game keeps the focus; the input the overlay
//! takes over it is posted to the window.

use anyhow::Result;
use gpui_kit::{AsyncWindowContext, WeakEntity};
use model::CanvasEvent;
use topmost::Away;

use super::{
    Ask, Layer, Mode, fullscreen,
    in_game::{HUD_LAYER, InGameEvent},
    reveal,
};

impl Layer {
    pub(super) fn is_inside(&self) -> bool {
        matches!(self.mode, Mode::Over { inside: Some(_), .. })
    }

    /// While inside: the HUD layer's input goes to the window; a new size or the canvas's end is an ask.
    pub(super) fn in_game_event(&self, event: InGameEvent) {
        if !self.is_inside() {
            return;
        }
        let ask = match event {
            InGameEvent::Canvas(CanvasEvent::Input { layer: HUD_LAYER, message, wparam, lparam }) => {
                if let Some(hwnd) = self.hwnd {
                    topmost::post_input(hwnd, message, wparam, lparam);
                }
                return;
            }
            InGameEvent::Canvas(CanvasEvent::Resized { .. }) => Ask::Refit,
            InGameEvent::Gone => Ask::Lost,
            InGameEvent::Canvas(_) => return,
        };
        let _ = self.asks.unbounded_send(ask);
    }
}

pub(super) async fn enter(
    layer: &WeakEntity<Layer>,
    hwnd: isize,
    game: isize,
    size: (u32, u32),
    cx: &mut AsyncWindowContext,
) -> Result<()> {
    topmost::cloak(hwnd, true);
    // A hidden or minimized window neither draws nor turns fullscreen; it is put back on the way out.
    let away = topmost::restore(hwnd);
    layer.update_in(cx, |layer, window, cx| {
        layer.exclusive = false;
        layer.turn(Mode::Over { game, inside: Some(away) }, window, cx);
    })?;
    // Frameless first: GPUI's switch saves the window's rect for the way back.
    fullscreen(true, cx).await?;
    fit(hwnd, game, Some(size));
    let drawn = layer.update_in(cx, |layer, window, cx| {
        let in_game = layer.in_game.read(cx);
        let Some(layers) = in_game.layers() else { return false };
        // Before the first frame, so the layer is described with the input; the block waits for the layer.
        layers.interact(HUD_LAYER, true);
        layers.block(true);
        window.mirror_frames(in_game.frames(HUD_LAYER, (0, 0), || {}));
        window.mirror_cursor(in_game.cursors(HUD_LAYER));
        window.refresh();
        true
    })?;
    // The canvas went before the mode was set: nothing asks to leave.
    if !drawn {
        return leave(layer, hwnd, game, Some(away), cx).await;
    }
    Ok(())
}

/// Out of the game's frame: put back as it was, behind the game, or in front where the user asked for it (`away`: `None`).
pub(super) async fn leave(
    layer: &WeakEntity<Layer>,
    hwnd: isize,
    game: isize,
    away: Option<Away>,
    cx: &mut AsyncWindowContext,
) -> Result<()> {
    layer.update_in(cx, |layer, window, cx| {
        window.mirror_frames(None);
        window.mirror_cursor(None);
        if let Some(layers) = layer.in_game.read(cx).layers() {
            layers.hide(HUD_LAYER);
            layers.block(false);
        }
        layer.turn(Mode::Window { armed: None }, window, cx);
    })?;
    fullscreen(false, cx).await?;
    match away {
        Some(Away::Shown) => topmost::lower(hwnd, Some(game)),
        Some(away) => {
            topmost::put_away(hwnd, away);
            topmost::cloak(hwnd, false);
            return Ok(());
        }
        None => {}
    }
    reveal(hwnd, cx).await
}

/// The window at the game's client area with the size the overlay draws in, right behind the game.
pub(super) fn fit(hwnd: isize, game: isize, size: Option<(u32, u32)>) {
    let (Some((left, top, ..)), Some((width, height))) = (topmost::client_area(game), size) else { return };
    let rect = (left, top, width as i32, height as i32);
    topmost::lower(hwnd, Some(game));
    topmost::resize(hwnd, rect);
    // Moved onto a monitor of another scale, GPUI's fullscreen fills that monitor: once more there.
    if topmost::client_area(hwnd).is_some_and(|(.., w, h)| (w, h) != (rect.2, rect.3)) {
        topmost::resize(hwnd, rect);
    }
}
