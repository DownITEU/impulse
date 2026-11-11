//! What moves between frames: a zoom gliding to its end, markers growing and shrinking.

use std::time::Instant;

use gpui_kit::{Context, Window};
use model::Map;

use super::{
    Stage,
    camera::{Glide, Pixel},
};
use crate::map::markers::Look;

// Seconds a marker takes to cover about two thirds of its change in size.
const GROW: f64 = 0.05;

impl Stage {
    /// Zooms by `factor` about `pixel`; a zoom on its way goes on from where it would end.
    pub(super) fn zoom(&mut self, factor: f64, pixel: Pixel, cx: &mut Context<Self>) {
        let (Some(map), Some(camera)) = (self.map(cx), self.camera) else { return };
        let spans = self.spans(&map);
        let from = self.glide.map_or(camera.span, |glide| glide.span);
        self.glide = Some(Glide { span: (from * factor).clamp(*spans.start(), *spans.end()), pixel });
        self.hover(None, cx);
    }

    /// Advances what moves by the time since the last frame; asks for the next frame while anything does.
    pub(super) fn animate(&mut self, map: &Map, selected: Option<usize>, window: &mut Window) {
        let now = Instant::now();
        // Long gaps (the first frame, a stall) do not jump.
        let seconds = self.clock.map_or(0., |last| (now - last).as_secs_f64().min(0.1));
        let (size, spans) = (self.size(), self.spans(map));
        if let (Some(glide), Some(camera)) = (self.glide, &mut self.camera)
            && glide.step(camera, seconds, size, &spans)
        {
            self.glide = None;
        }
        let grown = self.grow(seconds, selected);
        self.clock = (self.glide.is_some() || !grown).then_some(now);
        if self.clock.is_some() {
            window.request_animation_frame();
        }
    }

    /// Eases each lifted marker toward its look's size; true once all are there.
    fn grow(&mut self, seconds: f64, selected: Option<usize>) -> bool {
        for pin in [self.shown.hovered, selected].into_iter().flatten() {
            if self.lift(pin).is_none() {
                self.shown.lifts.push((pin, 1.));
            }
        }
        let part = 1. - (-seconds / GROW).exp();
        let mut grown = true;
        for index in 0..self.shown.lifts.len() {
            let (pin, scale) = self.shown.lifts[index];
            let target = self.look(pin, selected).scale();
            let next = if (target - scale).abs() < 0.005 { target } else { scale + (target - scale) * part };
            grown &= next == target;
            self.shown.lifts[index].1 = next;
        }
        self.shown.lifts.retain(|&(_, scale)| scale != 1.);
        grown
    }

    /// The size a lifted marker is drawn at now.
    pub(super) fn lift(&self, pin: usize) -> Option<f64> {
        self.shown.lifts.iter().find(|(lifted, _)| *lifted == pin).map(|&(_, scale)| scale)
    }

    pub(super) fn look(&self, pin: usize, selected: Option<usize>) -> Look {
        match Some(pin) {
            pin if pin == selected => Look::Selected,
            pin if pin == self.shown.hovered => Look::Hovered,
            _ => Look::Plain,
        }
    }
}
