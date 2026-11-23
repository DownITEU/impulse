//! Recording a chord for rebinding, after Easymic's `HotkeyCapture`.

use model::{
    Chord, Held,
    mouse::{LBUTTON, RBUTTON},
};

const ESC: u8 = 0x1B;
const BACKSPACE: u8 = 0x08;
const DELETE: u8 = 0x2E;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// The keys held so far; `None` once every modifier went up before a key.
    Preview(Option<Chord>),
    Done(Outcome),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    Bound(Chord),
    /// `ESC` alone.
    Cancelled,
    /// `BACKSPACE` alone: back to the default.
    Reset,
    /// `DELETE` alone.
    Unbound,
}

/// Fed key changes without repeats; its hook should hide every down it returns a step for, so neither
/// the app nor the game acts on it.
#[derive(Default)]
pub struct Capture {
    held: Held,
    done: bool,
}

impl Capture {
    /// Ends at the first release once a key besides modifiers is held, with the chord held just before
    /// it; modifiers alone come and go. `None` for a release it never saw pressed, a click with nothing
    /// held and after `Done`.
    pub fn key(&mut self, vk: u8, down: bool) -> Option<Step> {
        if self.done {
            return None;
        }
        if down {
            let alone = self.held.chord().is_none();
            // A click alone is the user's way out of binding, not a binding.
            if alone && matches!(vk, LBUTTON | RBUTTON) {
                return None;
            }
            if let Some(outcome) = alone.then(|| command(vk)).flatten() {
                self.done = true;
                return Some(Step::Done(outcome));
            }
            self.held.press(vk);
            return Some(Step::Preview(self.held.chord()));
        }
        let before = self.held.chord().filter(|chord| chord.contains(vk))?;
        self.held.release(vk);
        if before.keys().next().is_none() {
            return Some(Step::Preview(self.held.chord()));
        }
        self.done = true;
        Some(Step::Done(Outcome::Bound(before)))
    }
}

fn command(vk: u8) -> Option<Outcome> {
    match vk {
        ESC => Some(Outcome::Cancelled),
        BACKSPACE => Some(Outcome::Reset),
        DELETE => Some(Outcome::Unbound),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LCTRL: u8 = 0xA2;
    const LSHIFT: u8 = 0xA0;
    const F6: u8 = 0x75;

    fn feed(keys: &[(u8, bool)]) -> Vec<Option<Step>> {
        let mut capture = Capture::default();
        keys.iter().map(|&(vk, down)| capture.key(vk, down)).collect()
    }

    fn chord(vks: &[u16]) -> Option<Chord> {
        Chord::from_vks(vks)
    }

    #[test]
    fn previews_each_change_and_ends_at_the_first_release_after_a_key() {
        let steps = feed(&[(LCTRL, true), (LSHIFT, true), (LSHIFT, false), (F6, true), (LCTRL, false), (F6, false)]);
        let previews = [chord(&[0x11]), chord(&[0x11, 0x10]), chord(&[0x11]), chord(&[0x11, 0x75])];
        let mut expected: Vec<_> = previews.into_iter().map(|preview| Some(Step::Preview(preview))).collect();
        expected.extend([Some(Step::Done(Outcome::Bound(chord(&[0x11, 0x75]).unwrap()))), None]);
        assert_eq!(steps, expected);
    }

    #[test]
    fn modifiers_alone_never_bind() {
        assert_eq!(feed(&[(LCTRL, true), (LCTRL, false)])[1], Some(Step::Preview(None)));
    }

    #[test]
    fn commands_count_only_alone() {
        assert_eq!(feed(&[(ESC, true)]), [Some(Step::Done(Outcome::Cancelled))]);
        assert_eq!(feed(&[(BACKSPACE, true)]), [Some(Step::Done(Outcome::Reset))]);
        assert_eq!(feed(&[(DELETE, true)]), [Some(Step::Done(Outcome::Unbound))]);
        let steps = feed(&[(LCTRL, true), (DELETE, true), (DELETE, false)]);
        assert_eq!(steps[2], Some(Step::Done(Outcome::Bound(chord(&[0x11, 0x2E]).unwrap()))));
    }

    #[test]
    fn a_click_binds_only_with_something_held_and_a_notch_binds_at_once() {
        assert_eq!(feed(&[(LBUTTON, true), (LBUTTON, false)]), [None, None]);
        let ctrl_click = feed(&[(LCTRL, true), (RBUTTON, true), (RBUTTON, false)]);
        assert_eq!(ctrl_click[2], Some(Step::Done(Outcome::Bound(chord(&[0x11, 0x02]).unwrap()))));
        let notch = feed(&[(0x0A, true), (0x0A, false)]);
        assert_eq!(notch[1], Some(Step::Done(Outcome::Bound(chord(&[0x0A]).unwrap()))));
    }

    #[test]
    fn a_release_it_never_saw_pressed_is_ignored() {
        assert_eq!(feed(&[(0x41, false)]), [None]);
    }
}
