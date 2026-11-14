//! A pin's clip: downloaded once, decoded on a thread of its own onto the window's GPU, each frame drawn when due.
//! A view of its own, paced by the clip's frames: a frame repaints the clip, not the map.

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use blocking::unblock;
use gpui_kit::{Bounds, Context, IntoElement, Pixels, Render, Task, Window, canvas, div, prelude::*, px, size};
use video::{Frame, ID3D11Device, Playback};

use crate::ui::art;

// A stall is not caught up: the clip goes on from where it stopped.
const STEP: Duration = Duration::from_millis(100);
// The shortest wait between frames, and the wait while the decoder has none ready.
const POLL: Duration = Duration::from_millis(8);

pub struct ClipView {
    // The frame drawn, and the one taken to draw next. A frame dropped gives its texture back to the decoder.
    shown: Option<Frame>,
    next: Option<Frame>,
    _play: Task<()>,
}

impl ClipView {
    /// Plays `url`, kept in `dir`, decoded on `device`, the window's.
    pub fn new(dir: PathBuf, url: String, device: ID3D11Device, cx: &mut Context<Self>) -> Self {
        let play = cx.spawn(async move |clip, cx| {
            let mut playback = match unblock(move || Playback::start(&art::path(&dir, &url)?, &device)).await {
                Ok(playback) => playback,
                Err(e) => return log::warn!("pin clip not played: {e:#}"),
            };
            let (mut played, mut last) = (Duration::ZERO, Instant::now());
            loop {
                let now = Instant::now();
                let elapsed = (now - last).min(STEP);
                last = now;
                if clip.update(cx, |clip, cx| clip.advance(&mut playback, &mut played, elapsed, cx)).is_err() {
                    return;
                }
                let wait = playback.next_due().map_or(POLL, |due| due.saturating_sub(played).max(POLL));
                cx.background_executor().timer(wait).await;
            }
        });
        Self { shown: None, next: None, _play: play }
    }

    /// Plays on by `elapsed` and takes the frame due then, unless the last one taken is still to be drawn (the window is hidden).
    fn advance(&mut self, playback: &mut Playback, played: &mut Duration, elapsed: Duration, cx: &mut Context<Self>) {
        if self.next.is_some() {
            return;
        }
        *played += elapsed;
        if let Some(frame) = playback.frame(*played) {
            self.next = Some(frame);
            cx.notify();
        }
    }
}

/// A picture of `size` scaled to cover `bounds`, centred on it.
fn cover(bounds: Bounds<Pixels>, (width, height): (u32, u32)) -> Bounds<Pixels> {
    let (width, height) = (width as f32, height as f32);
    let scale = (f32::from(bounds.size.width) / width).max(f32::from(bounds.size.height) / height);
    Bounds::centered_at(bounds.center(), size(px(width * scale), px(height * scale)))
}

impl Render for ClipView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        if let Some(next) = self.next.take() {
            self.shown = Some(next);
        }
        let Some(frame) = &self.shown else { return div().into_any_element() };
        let (texture, frame_size) = (frame.texture.get().clone(), (frame.width, frame.height));
        // The box around clips it to its own bounds.
        canvas(|_, _, _| {}, move |bounds, _, window, _| window.paint_surface(cover(bounds, frame_size), texture))
            .size_full()
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::point;

    use super::*;

    #[test]
    fn a_frame_covers_its_box_centred() {
        let bounds = Bounds::new(point(px(10.), px(20.)), size(px(340.), px(190.)));
        let covered = cover(bounds, (1920, 1080));
        assert_eq!(covered.center(), bounds.center());
        assert_eq!(covered.size.width, px(340.));
        assert!(covered.size.height >= px(190.));
        let tall = cover(bounds, (100, 400));
        assert_eq!(tall.size.width, px(340.));
        assert_eq!(tall.size.height, px(1360.));
    }
}
