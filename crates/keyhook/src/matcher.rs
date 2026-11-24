//! Matches chords and keys without multi-press or tap-only modes. App scope is handled by the caller.

use model::Chord;

use crate::{Key, Modifiers};

pub struct Binding<T> {
    pub action: T,
    /// Hides the key that completes the trigger from every other app.
    pub block: bool,
}

/// What fires a binding: a chord, or a hotkey, matched as RegisterHotKey did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Trigger {
    Chord(Chord),
    Key(Key),
}

impl From<Chord> for Trigger {
    fn from(chord: Chord) -> Self {
        Self::Chord(chord)
    }
}

impl From<Key> for Trigger {
    fn from(key: Key) -> Self {
        Self::Key(key)
    }
}

/// Pure: fed key changes without repeats, it never reads the system.
pub struct Matcher<T> {
    bindings: Vec<(Trigger, Binding<T>)>,
    down: [u64; 4],
}

impl<T> Default for Matcher<T> {
    fn default() -> Self {
        Self { bindings: Vec::new(), down: [0; 4] }
    }
}

impl<T> Matcher<T> {
    /// A chord may be bound more than once: all of them fire.
    pub fn bind(&mut self, trigger: impl Into<Trigger>, binding: Binding<T>) {
        self.bindings.push((trigger.into(), binding));
    }

    /// Swaps the bindings; the held keys stay, so a rebind in the middle of a chord does not lose it.
    pub fn replace(&mut self, bindings: impl IntoIterator<Item = (Trigger, Binding<T>)>) {
        self.bindings = bindings.into_iter().collect();
    }

    /// The modifiers held now.
    pub fn modifiers(&self) -> Modifiers {
        Modifiers::held(|vk| self.is_held(vk))
    }

    fn is_held(&self, vk: u8) -> bool {
        self.down[usize::from(vk / 64)] & (1 << (vk % 64)) != 0
    }

    /// A down fires the longest active chords that hold the key and whose keys are all held: a held
    /// Shift does not stop `F1`, yet `CTRL+F1` beats `F1`, unless it is not `active`. A `Key` fires on
    /// the down of its own key with exactly its modifiers held, whatever the chords do. `active` is
    /// asked only of bindings that match. Returns whether to hide the down.
    pub fn key(&mut self, vk: u8, down: bool, active: impl Fn(&T) -> bool, mut fire: impl FnMut(&T)) -> bool {
        let (word, bit) = (usize::from(vk / 64), 1 << (vk % 64));
        if !down {
            self.down[word] &= !bit;
            return false;
        }
        self.down[word] |= bit;
        // The chord's length, none for a `Key`, which the longest rule leaves alone.
        let mut hits: Vec<(usize, Option<usize>)> = Vec::new();
        for (at, (trigger, binding)) in self.bindings.iter().enumerate() {
            let len = match *trigger {
                Trigger::Chord(chord) => {
                    (chord.contains(vk) && chord.vks().all(|key| self.is_held(key))).then(|| Some(chord.vks().count()))
                }
                Trigger::Key(key) => key.pressed(vk, |key| self.is_held(key)).then_some(None),
            };
            if let Some(len) = len
                && active(&binding.action)
            {
                hits.push((at, len));
            }
        }
        let longest = hits.iter().filter_map(|&(_, len)| len).max();
        let mut block = false;
        for (at, len) in hits {
            if len.is_none() || len == longest {
                let binding = &self.bindings[at].1;
                fire(&binding.action);
                block |= binding.block;
            }
        }
        block
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::CTRL_SHIFT_O;

    const LCTRL: u8 = 0xA2;
    const RCTRL: u8 = 0xA3;
    const LSHIFT: u8 = 0xA0;
    const F1: u8 = 0x70;
    const O: u8 = b'O';

    fn matcher(bindings: &[(&[u16], char, bool)]) -> Matcher<char> {
        let mut matcher = Matcher::default();
        for &(vks, action, block) in bindings {
            matcher.bind(Chord::from_vks(vks).unwrap(), Binding { action, block });
        }
        matcher
    }

    fn feed(matcher: &mut Matcher<char>, keys: &[(u8, bool)]) -> (Vec<char>, Vec<bool>) {
        let mut fired = Vec::new();
        let blocks = keys.iter().map(|&(vk, down)| matcher.key(vk, down, |_| true, |&action| fired.push(action))).collect();
        (fired, blocks)
    }

    #[test]
    fn the_longest_held_chord_wins_and_extra_keys_do_not_stop_it() {
        let mut matcher = matcher(&[(&[0x70], 'a', false), (&[0x11, 0x70], 'b', false)]);
        assert_eq!(feed(&mut matcher, &[(LCTRL, true), (F1, true)]).0, ['b']);
        let (fired, _) = feed(&mut matcher, &[(F1, false), (LCTRL, false), (LSHIFT, true), (F1, true)]);
        assert_eq!(fired, ['a']);
    }

    #[test]
    fn a_right_modifier_is_not_the_left_one() {
        let mut matcher = matcher(&[(&[0x11, 0x70], 'b', false)]);
        assert_eq!(feed(&mut matcher, &[(RCTRL, true), (F1, true)]).0, []);
    }

    #[test]
    fn blocking_hides_only_the_down_that_completes_the_chord() {
        let mut matcher = matcher(&[(&[0x11, 0x70], 'b', true)]);
        let (fired, blocks) = feed(&mut matcher, &[(LCTRL, true), (F1, true), (LCTRL, false), (F1, false)]);
        assert_eq!(fired, ['b']);
        assert_eq!(blocks, [false, true, false, false]);
    }

    #[test]
    fn a_chord_bound_twice_fires_both() {
        let mut matcher = matcher(&[(&[0x70], 'a', false), (&[0x70], 'b', false)]);
        assert_eq!(feed(&mut matcher, &[(F1, true)]).0, ['a', 'b']);
    }

    #[test]
    fn a_key_fires_with_either_sides_modifiers_and_no_extra_one() {
        let mut matcher = Matcher::default();
        matcher.bind(CTRL_SHIFT_O, Binding { action: 'k', block: true });
        let (fired, blocks) = feed(&mut matcher, &[(RCTRL, true), (LSHIFT, true), (O, true), (O, false)]);
        assert_eq!(fired, ['k']);
        assert_eq!(blocks, [false, false, true, false]);
        let extra = feed(&mut matcher, &[(0xA4, true), (O, true), (O, false), (0xA4, false)]).0;
        assert_eq!(extra, []);
    }

    #[test]
    fn a_key_does_not_fire_when_its_modifiers_come_after_it() {
        let mut matcher = Matcher::default();
        matcher.bind(CTRL_SHIFT_O, Binding { action: 'k', block: false });
        assert_eq!(feed(&mut matcher, &[(O, true), (LCTRL, true), (LSHIFT, true)]).0, []);
    }

    #[test]
    fn an_inactive_longer_chord_does_not_shadow_a_shorter_active_one() {
        let mut matcher = matcher(&[(&[0x70], 'a', false), (&[0x11, 0x70], 'b', false)]);
        let mut fired = Vec::new();
        matcher.key(LCTRL, true, |_| true, |&action| fired.push(action));
        matcher.key(F1, true, |&action| action == 'a', |&action| fired.push(action));
        assert_eq!(fired, ['a']);
    }

    #[test]
    fn a_replace_keeps_the_held_keys() {
        let mut matcher = matcher(&[]);
        feed(&mut matcher, &[(LCTRL, true)]);
        let chord = Chord::from_vks(&[0x11, 0x70]).unwrap();
        matcher.replace([(chord.into(), Binding { action: 'z', block: false })]);
        assert!(matcher.modifiers().control);
        assert_eq!(feed(&mut matcher, &[(F1, true)]).0, ['z']);
    }
}
