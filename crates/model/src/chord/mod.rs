//! Hotkey chords use virtual-key codes and reserve two codes for mouse wheel input.

pub mod mouse;
mod names;

/// Modifier bits in byte 0, up to seven other keys above it, the newest in byte 1.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Chord(u64);

pub(crate) struct Modifier {
    vk: u8,
    bit: u8,
    pub name: &'static str,
}

/// Plain names are the left keys; the right ones carry an `R`.
const MODIFIERS: [Modifier; 6] = [
    Modifier { vk: 0xA2, bit: 0x04, name: "CTRL" },
    Modifier { vk: 0xA3, bit: 0x20, name: "RCTRL" },
    Modifier { vk: 0xA0, bit: 0x08, name: "SHIFT" },
    Modifier { vk: 0xA1, bit: 0x40, name: "RSHIFT" },
    Modifier { vk: 0xA4, bit: 0x10, name: "ALT" },
    Modifier { vk: 0xA5, bit: 0x80, name: "RALT" },
];

const MAX_KEYS: usize = 7;

impl Chord {
    /// Virtual-key codes in press order. The side-less `VK_SHIFT`, `VK_CONTROL` and `VK_MENU` are
    /// the left keys. `None` for no key, a code above 255, a key other than a modifier named twice or
    /// more than seven such keys.
    pub fn from_vks(vks: &[u16]) -> Option<Self> {
        let mut held = Held::default();
        for &vk in vks {
            let vk = match u8::try_from(vk).ok()? {
                0 => return None,
                0x10 => 0xA0,
                0x11 => 0xA2,
                0x12 => 0xA4,
                vk => vk,
            };
            let full = |chord: Chord| chord.keys().count() == MAX_KEYS || chord.contains(vk);
            // A key held twice takes two bytes, yet the keyboard reports it once: it could never match.
            if bit(vk).is_none() && held.chord().is_some_and(full) {
                return None;
            }
            held.press(vk);
        }
        held.chord()
    }

    /// A modifier is in it when its bit is set, any other key when it is among the keys.
    pub fn contains(self, vk: u8) -> bool {
        match bit(vk) {
            Some(bit) => self.0 & u64::from(bit) != 0,
            None => self.keys().any(|key| key == vk),
        }
    }

    /// The same keys in any press order: `Matcher` cannot tell such chords apart.
    pub fn same_keys(self, other: Chord) -> bool {
        self.vks().count() == other.vks().count() && self.vks().all(|vk| other.contains(vk))
    }

    /// It needs the mouse hook.
    pub fn uses_mouse(self) -> bool {
        self.keys().any(mouse::is_mouse)
    }

    /// Its virtual-key codes, modifiers first: `from_vks` reads them back to the same chord.
    pub fn vks(self) -> impl Iterator<Item = u8> {
        self.modifiers().map(|modifier| modifier.vk).chain(self.keys())
    }

    pub(crate) fn modifiers(self) -> impl Iterator<Item = &'static Modifier> {
        MODIFIERS.iter().filter(move |modifier| self.0 & u64::from(modifier.bit) != 0)
    }

    /// The keys besides modifiers, in press order.
    pub fn keys(self) -> impl Iterator<Item = u8> {
        let byte = move |at: usize| (self.0 >> (8 * at)) as u8;
        let count = (1..=MAX_KEYS).take_while(|&at| byte(at) != 0).count();
        (1..=count).rev().map(byte)
    }
}

fn bit(vk: u8) -> Option<u8> {
    MODIFIERS.iter().find(|modifier| modifier.vk == vk).map(|modifier| modifier.bit)
}

/// The keys held now, tracked as Easymic's `KeyChord` does.
#[derive(Default)]
pub struct Held(u64);

impl Held {
    pub fn press(&mut self, vk: u8) {
        match bit(vk) {
            Some(bit) => self.0 |= u64::from(bit),
            None => self.0 = ((self.0 & !0xFF) << 8) | (u64::from(vk) << 8) | (self.0 & 0xFF),
        }
    }

    pub fn release(&mut self, vk: u8) {
        let modifiers = self.0 & 0xFF;
        self.0 = match bit(vk) {
            Some(bit) => self.0 & !u64::from(bit),
            // Only the newest key's release keeps the keys below it; any other ends the combination.
            None if (self.0 >> 8) as u8 == vk => ((self.0 >> 16) << 8) | modifiers,
            None => modifiers,
        };
    }

    pub fn chord(&self) -> Option<Chord> {
        (self.0 != 0).then_some(Chord(self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_keep_press_order_and_modifiers_go_to_bits() {
        let chord = Chord::from_vks(&[0x11, 0x41, 0x42]).unwrap();
        assert_eq!(chord.keys().collect::<Vec<_>>(), [0x41, 0x42]);
        assert_eq!(chord.modifiers().map(|modifier| modifier.name).collect::<Vec<_>>(), ["CTRL"]);
    }

    #[test]
    fn side_less_modifiers_are_the_left_keys() {
        assert_eq!(Chord::from_vks(&[0x11, 0x10, 0x12, 0x70]), Chord::from_vks(&[0xA2, 0xA0, 0xA4, 0x70]));
        assert_ne!(Chord::from_vks(&[0x11, 0x70]), Chord::from_vks(&[0xA3, 0x70]));
    }

    #[test]
    fn a_chord_that_could_never_match_is_refused() {
        for vks in [&[][..], &[0], &[0x100], &[0x41, 0x41], &[0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48]] {
            assert_eq!(Chord::from_vks(vks), None, "{vks:?}");
        }
        assert!(Chord::from_vks(&[0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47]).is_some());
    }

    #[test]
    fn same_keys_ignore_press_order_but_not_sides() {
        let chord = |vks: &[u16]| Chord::from_vks(vks).unwrap();
        assert!(chord(&[0x70, 0x71]).same_keys(chord(&[0x71, 0x70])));
        assert!(!chord(&[0xA2, 0x70]).same_keys(chord(&[0xA3, 0x70])));
        assert!(!chord(&[0x70]).same_keys(chord(&[0x70, 0x71])));
    }

    #[test]
    fn releasing_the_newest_key_keeps_the_rest_and_any_other_ends_them() {
        let mut held = Held::default();
        for vk in [0xA2, 0x41, 0x42] {
            held.press(vk);
        }
        held.release(0x42);
        assert_eq!(held.chord(), Chord::from_vks(&[0xA2, 0x41]));
        held.press(0x42);
        held.release(0x41);
        assert_eq!(held.chord(), Chord::from_vks(&[0xA2]));
        held.release(0xA2);
        assert_eq!(held.chord(), None);
    }
}
