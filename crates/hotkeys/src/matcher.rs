//! Which bindings a key fires: Easymic's `HotkeyService` without multi-press, tap-only or per-app scopes.

use std::collections::{HashMap, hash_map::Entry};

use crate::{Chord, chord::Held};

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
    bindings: HashMap<Chord, Binding<T>>,
    held: Held,
    // Fired chords waiting for one of their keys to go up.
    pressed: Vec<Chord>,
}

impl<T> Default for Matcher<T> {
    fn default() -> Self {
        Self { bindings: HashMap::new(), held: Held::default(), pressed: Vec::new() }
    }
}

impl<T: Clone> Matcher<T> {
    /// False when the chord is taken.
    pub fn bind(&mut self, chord: Chord, binding: Binding<T>) -> bool {
        match self.bindings.entry(chord) {
            Entry::Vacant(entry) => {
                entry.insert(binding);
                true
            }
            Entry::Occupied(_) => false,
        }
    }

    /// On a down, both the whole held chord and the new key with the held modifiers fire: a held
    /// movement key does not stop `NUM 1`, yet `CTRL+F1` never fires `F1`. Returns whether to hide the down.
    pub fn key(&mut self, vk: u8, down: bool, mut fire: impl FnMut(Fired<T>)) -> bool {
        if !down {
            let bindings = &self.bindings;
            self.pressed.retain(|chord| {
                let up = chord.contains(vk);
                if up {
                    fire(Fired::Release(bindings[chord].action.clone()));
                }
                !up
            });
            self.held.release(vk);
            return false;
        }
        self.held.press(vk);
        let whole = self.held.chord();
        let alone = self.held.alone(vk).filter(|&alone| Some(alone) != whole);
        let mut block = false;
        for chord in whole.into_iter().chain(alone) {
            if let Some(binding) = self.bindings.get(&chord) {
                fire(Fired::Press(binding.action.clone()));
                self.pressed.push(chord);
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
    const F1: u8 = 0x70;
    const W: u8 = 0x57;

    fn matcher(bindings: &[(&[u16], char, bool)]) -> Matcher<char> {
        let mut matcher = Matcher::default();
        for &(vks, action, block) in bindings {
            assert!(matcher.bind(Chord::from_vks(vks).unwrap(), Binding { action, block }));
        }
        matcher
    }

    fn feed(matcher: &mut Matcher<char>, keys: &[(u8, bool)]) -> (Vec<Fired<char>>, Vec<bool>) {
        let mut fired = Vec::new();
        let blocks = keys.iter().map(|&(vk, down)| matcher.key(vk, down, |event| fired.push(event))).collect();
        (fired, blocks)
    }

    #[test]
    fn modifiers_must_match_exactly_and_by_side() {
        let mut matcher = matcher(&[(&[0x70], 'a', false), (&[0x11, 0x70], 'b', false)]);
        assert_eq!(feed(&mut matcher, &[(LCTRL, true), (F1, true)]).0, [Fired::Press('b')]);
        let (fired, _) = feed(&mut matcher, &[(F1, false), (LCTRL, false), (RCTRL, true), (F1, true)]);
        assert_eq!(fired, [Fired::Release('b')]);
    }

    #[test]
    fn a_held_key_does_not_stop_another_binding() {
        let mut matcher = matcher(&[(&[0x70], 'a', false)]);
        assert_eq!(feed(&mut matcher, &[(W, true), (F1, true)]).0, [Fired::Press('a')]);
    }

    #[test]
    fn the_first_key_up_releases_and_blocking_hides_only_the_down() {
        let mut matcher = matcher(&[(&[0x11, 0x70], 'b', true)]);
        let (fired, blocks) = feed(&mut matcher, &[(LCTRL, true), (F1, true), (LCTRL, false), (F1, false)]);
        assert_eq!(fired, [Fired::Press('b'), Fired::Release('b')]);
        assert_eq!(blocks, [false, true, false, false]);
    }

    #[test]
    fn a_chord_binds_once() {
        let mut matcher = matcher(&[(&[0x70], 'a', false)]);
        assert!(!matcher.bind(Chord::from_vks(&[0x70]).unwrap(), Binding { action: 'b', block: false }));
    }
}
