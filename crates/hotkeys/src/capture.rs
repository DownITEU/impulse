//! Recording a chord for rebinding, as Easymic's `HotkeyCapture`.

use crate::{Chord, chord::Held};

const ESC: u8 = 0x1B;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    Preview(Chord),
    /// The chord at the first release; `None` when `ESC` alone cleared it.
    Done(Option<Chord>),
}

/// Fed key changes without repeats; its hook should hide every down, so neither the app nor the game acts on it.
#[derive(Default)]
pub struct Capture {
    held: Held,
    last: Option<Chord>,
    done: bool,
}

impl Capture {
    /// `None` for a release before any press and for everything after `Done`.
    pub fn key(&mut self, vk: u8, down: bool) -> Option<Step> {
        if self.done {
            return None;
        }
        if down {
            self.held.press(vk);
            self.last = self.held.chord();
            return self.last.map(Step::Preview);
        }
        let last = self.last?;
        self.done = true;
        Some(Step::Done((Chord::from_vks(&[ESC.into()]) != Some(last)).then_some(last)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(keys: &[(u8, bool)]) -> Vec<Option<Step>> {
        let mut capture = Capture::default();
        keys.iter().map(|&(vk, down)| capture.key(vk, down)).collect()
    }

    fn chord(vks: &[u16]) -> Chord {
        Chord::from_vks(vks).unwrap()
    }

    #[test]
    fn previews_each_press_and_ends_at_the_first_release() {
        let steps = feed(&[(0xA2, true), (0x75, true), (0xA2, false)]);
        let done = Step::Done(Some(chord(&[0x11, 0x75])));
        assert_eq!(steps, [Some(Step::Preview(chord(&[0x11]))), Some(Step::Preview(chord(&[0x11, 0x75]))), Some(done)]);
    }

    #[test]
    fn esc_alone_clears_and_esc_in_a_chord_binds() {
        assert_eq!(feed(&[(ESC, true), (ESC, false)])[1], Some(Step::Done(None)));
        assert_eq!(feed(&[(0xA2, true), (ESC, true), (ESC, false)])[2], Some(Step::Done(Some(chord(&[0x11, 0x1B])))));
        assert_eq!(feed(&[(ESC, true), (0x41, true), (ESC, false)])[2], Some(Step::Done(Some(chord(&[0x1B, 0x41])))));
    }

    #[test]
    fn nothing_follows_done() {
        assert_eq!(feed(&[(0x41, true), (0x41, false), (0x42, true), (0x42, false)])[2..], [None, None]);
    }

    #[test]
    fn a_release_before_any_press_is_ignored() {
        assert_eq!(feed(&[(0x41, false)]), [None]);
    }
}
