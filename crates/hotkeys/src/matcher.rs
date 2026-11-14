//! Which bindings a key fires, matched as Wand does: Easymic's `HotkeyService` without multi-press,
//! tap-only or per-app scopes.

use model::Chord;

pub struct Binding<T> {
    pub action: T,
    /// Hides the key that completes the chord from every other app.
    pub block: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Fired<T> {
    Press(T),
    /// One of the chord's keys went up.
    Release(T),
}

/// Pure: fed key changes without repeats, it never reads the system.
pub struct Matcher<T> {
    bindings: Vec<(Chord, Binding<T>)>,
    down: [u64; 4],
    // Fired bindings waiting for one of their keys to go up.
    pressed: Vec<usize>,
}

impl<T> Default for Matcher<T> {
    fn default() -> Self {
        Self { bindings: Vec::new(), down: [0; 4], pressed: Vec::new() }
    }
}

impl<T: Clone> Matcher<T> {
    /// A chord may be bound more than once: all of them fire.
    pub fn bind(&mut self, chord: Chord, binding: Binding<T>) {
        self.bindings.push((chord, binding));
    }

    /// A down fires the longest bound chords that hold the key and whose keys are all held: a held
    /// Shift does not stop `F1`, yet `CTRL+F1` beats `F1`. Returns whether to hide the down.
    pub fn key(&mut self, vk: u8, down: bool, mut fire: impl FnMut(Fired<T>)) -> bool {
        let (word, bit) = (usize::from(vk / 64), 1 << (vk % 64));
        if !down {
            self.down[word] &= !bit;
            let bindings = &self.bindings;
            self.pressed.retain(|&at| {
                let (chord, binding) = &bindings[at];
                let up = chord.contains(vk);
                if up {
                    fire(Fired::Release(binding.action.clone()));
                }
                !up
            });
            return false;
        }
        self.down[word] |= bit;
        let held = |key: u8| self.down[usize::from(key / 64)] & (1 << (key % 64)) != 0;
        let matches = |chord: &Chord| chord.contains(vk) && chord.vks().all(held);
        let longest = self.bindings.iter().filter(|(chord, _)| matches(chord)).map(|(chord, _)| chord.vks().count()).max();
        let mut block = false;
        for (at, (chord, binding)) in self.bindings.iter().enumerate() {
            if Some(chord.vks().count()) == longest && matches(chord) {
                fire(Fired::Press(binding.action.clone()));
                self.pressed.push(at);
                block |= binding.block;
            }
        }
        block
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LCTRL: u8 = 0xA2;
    const RCTRL: u8 = 0xA3;
    const LSHIFT: u8 = 0xA0;
    const F1: u8 = 0x70;

    fn matcher(bindings: &[(&[u16], char, bool)]) -> Matcher<char> {
        let mut matcher = Matcher::default();
        for &(vks, action, block) in bindings {
            matcher.bind(Chord::from_vks(vks).unwrap(), Binding { action, block });
        }
        matcher
    }

    fn feed(matcher: &mut Matcher<char>, keys: &[(u8, bool)]) -> (Vec<Fired<char>>, Vec<bool>) {
        let mut fired = Vec::new();
        let blocks = keys.iter().map(|&(vk, down)| matcher.key(vk, down, |event| fired.push(event))).collect();
        (fired, blocks)
    }

    #[test]
    fn the_longest_held_chord_wins_and_extra_keys_do_not_stop_it() {
        let mut matcher = matcher(&[(&[0x70], 'a', false), (&[0x11, 0x70], 'b', false)]);
        assert_eq!(feed(&mut matcher, &[(LCTRL, true), (F1, true)]).0, [Fired::Press('b')]);
        let (fired, _) = feed(&mut matcher, &[(F1, false), (LCTRL, false), (LSHIFT, true), (F1, true)]);
        assert_eq!(fired, [Fired::Release('b'), Fired::Press('a')]);
    }

    #[test]
    fn a_right_modifier_is_not_the_left_one() {
        let mut matcher = matcher(&[(&[0x11, 0x70], 'b', false)]);
        assert_eq!(feed(&mut matcher, &[(RCTRL, true), (F1, true)]).0, []);
    }

    #[test]
    fn the_first_key_up_releases_and_blocking_hides_only_the_down() {
        let mut matcher = matcher(&[(&[0x11, 0x70], 'b', true)]);
        let (fired, blocks) = feed(&mut matcher, &[(LCTRL, true), (F1, true), (LCTRL, false), (F1, false)]);
        assert_eq!(fired, [Fired::Press('b'), Fired::Release('b')]);
        assert_eq!(blocks, [false, true, false, false]);
    }

    #[test]
    fn a_chord_bound_twice_fires_both() {
        let mut matcher = matcher(&[(&[0x70], 'a', false), (&[0x70], 'b', false)]);
        assert_eq!(feed(&mut matcher, &[(F1, true)]).0, [Fired::Press('a'), Fired::Press('b')]);
    }
}
