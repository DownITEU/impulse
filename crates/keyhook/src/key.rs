//! A hotkey as RegisterHotKey matched it: modifiers of either side, exactly, and one key.

use model::Chord;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN};

// The modifiers' virtual keys, either side, and the left one a chord names them by.
const MODIFIERS: [(HOT_KEY_MODIFIERS, [u8; 2]); 4] =
    [(MOD_CONTROL, [0xA2, 0xA3]), (MOD_SHIFT, [0xA0, 0xA1]), (MOD_ALT, [0xA4, 0xA5]), (MOD_WIN, [0x5B, 0x5C])];
// Side-less Ctrl, Shift and Alt.
const GENERIC: [(u8, HOT_KEY_MODIFIERS); 3] = [(0x11, MOD_CONTROL), (0x10, MOD_SHIFT), (0x12, MOD_ALT)];

/// Side-less Ctrl, Shift or Alt: the low-level hook reports the sided keys instead.
pub(crate) fn is_generic(vk: u8) -> bool {
    GENERIC.iter().any(|&(generic, _)| generic == vk)
}

/// Which modifiers, of either side.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Modifiers {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

impl Modifiers {
    const fn bits(self) -> HOT_KEY_MODIFIERS {
        const fn bit(on: bool, bit: HOT_KEY_MODIFIERS) -> HOT_KEY_MODIFIERS {
            if on { bit } else { 0 }
        }
        bit(self.control, MOD_CONTROL) | bit(self.alt, MOD_ALT) | bit(self.shift, MOD_SHIFT) | bit(self.win, MOD_WIN)
    }

    /// The modifiers among the keys `held`, a side-less one included.
    pub(crate) fn held(held: impl Fn(u8) -> bool) -> Self {
        let sided = MODIFIERS.iter().filter(|(_, sides)| sides.iter().any(|&side| held(side))).map(|(modifier, _)| *modifier);
        let generic = GENERIC.iter().filter(|(generic, _)| held(*generic)).map(|(_, modifier)| *modifier);
        let bits = sided.chain(generic).fold(0, |all, modifier| all | modifier);
        Self { control: bits & MOD_CONTROL != 0, alt: bits & MOD_ALT != 0, shift: bits & MOD_SHIFT != 0, win: bits & MOD_WIN != 0 }
    }
}

/// A hotkey as RegisterHotKey matched it: modifiers of either side, exactly, and one key.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Key {
    pub(crate) modifiers: HOT_KEY_MODIFIERS,
    pub(crate) vk: u8,
}

impl Key {
    pub const fn new(modifiers: Modifiers, vk: u8) -> Self {
        Self { modifiers: modifiers.bits(), vk }
    }

    /// `None` without a modifier and exactly one key, or with a mouse button or wheel notch: a lone key
    /// would be taken from every app.
    pub fn from_vks(vks: impl IntoIterator<Item = u8>) -> Option<Self> {
        let (mut modifiers, mut key) = (0, None);
        for vk in vks {
            let modifier = MODIFIERS.iter().find(|(_, sides)| sides.contains(&vk)).map(|(modifier, _)| *modifier);
            let generic = GENERIC.iter().find(|(generic, _)| *generic == vk).map(|(_, modifier)| *modifier);
            match modifier.or(generic) {
                Some(modifier) => modifiers |= modifier,
                None if model::mouse::is_mouse(vk) || key.replace(vk).is_some() => return None,
                None => {}
            }
        }
        Some(Self { modifiers, vk: key? }).filter(|key| key.modifiers != 0)
    }

    /// Whether pressing `chord`, its key last, fires it too: the chord holds its key and exactly its
    /// modifiers, of either side, whatever else it holds.
    pub fn clashes(self, chord: Chord) -> bool {
        chord.contains(self.vk) && self.pressed(self.vk, |vk| chord.contains(vk))
    }

    /// The modifiers by their left keys, then the key.
    pub fn vks(self) -> impl Iterator<Item = u8> {
        let modifiers = MODIFIERS.into_iter().filter(move |(modifier, _)| self.modifiers & modifier != 0).map(|(_, [left, _])| left);
        modifiers.chain([self.vk])
    }

    /// Whether the down of `vk` fires it, given which keys are `held`: the held modifiers are exactly its own.
    pub(crate) fn pressed(self, vk: u8, held: impl Fn(u8) -> bool) -> bool {
        vk == self.vk && Modifiers::held(held).bits() == self.modifiers
    }
}

#[cfg(test)]
pub(crate) const CTRL_SHIFT_O: Key = Key::new(Modifiers { control: true, shift: true, alt: false, win: false }, b'O');

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_and_one_key() {
        assert_eq!(Key::from_vks([0xA2, 0xA1, b'O']), Some(CTRL_SHIFT_O));
        assert_eq!(Key::from_vks([0x11, 0x10, b'O']), Some(CTRL_SHIFT_O));
        assert_eq!(CTRL_SHIFT_O.vks().collect::<Vec<_>>(), [0xA2, 0xA0, b'O']);
        assert_eq!(Key::from_vks([0x5B, 0x24]), Some(Key { modifiers: MOD_WIN, vk: 0x24 }));
        assert_eq!(Key::from_vks([0x24]), None);
        assert_eq!(Key::from_vks([0xA2, 0x0A]), None);
        assert_eq!(Key::from_vks([0xA2, 0xA0]), None);
        assert_eq!(Key::from_vks([0xA2, b'O', b'P']), None);
        assert_eq!(Key::from_vks([0xA2, 4]), None);
    }

    #[test]
    fn a_chord_clashes_when_pressing_it_fires_the_key() {
        let chord = |vks: &[u16]| Chord::from_vks(vks).unwrap();
        assert!(CTRL_SHIFT_O.clashes(chord(&[0xA3, 0xA1, 0x4F])));
        assert!(CTRL_SHIFT_O.clashes(chord(&[0x11, 0x10, 0x50, 0x4F])));
        assert!(!CTRL_SHIFT_O.clashes(chord(&[0xA3, 0xA1, 0xA4, 0x4F])));
        assert!(!CTRL_SHIFT_O.clashes(chord(&[0x11, 0x10, 0x50])));
    }

    #[test]
    fn pressed_wants_its_key_and_exactly_its_modifiers() {
        let with = |held: &'static [u8]| move |vk| held.contains(&vk);
        assert!(CTRL_SHIFT_O.pressed(b'O', with(&[0xA2, 0xA1])));
        assert!(CTRL_SHIFT_O.pressed(b'O', with(&[0x11, 0x10])));
        assert!(!CTRL_SHIFT_O.pressed(b'O', with(&[0xA2])));
        assert!(!CTRL_SHIFT_O.pressed(b'O', with(&[0xA2, 0xA0, 0xA4])));
        assert!(!CTRL_SHIFT_O.pressed(b'P', with(&[0xA2, 0xA0])));
    }

    #[test]
    fn held_modifiers_take_either_side() {
        let held = Modifiers::held(|vk| [0xA3, 0xA4, 0x10].contains(&vk));
        assert_eq!(held, Modifiers { control: true, alt: true, shift: true, win: false });
    }
}
