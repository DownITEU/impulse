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
    /// the left keys. `None` for no key, a code above 255, a key named twice or more than seven keys.
    pub fn from_vks(vks: &[u16]) -> Option<Self> {
        let mut chord = Self(0);
        for &vk in vks {
            let vk = match u8::try_from(vk).ok()? {
                0 => return None,
                0x10 => 0xA0,
                0x11 => 0xA2,
                0x12 => 0xA4,
                vk => vk,
            };
            if let Some(modifier) = MODIFIERS.iter().find(|modifier| modifier.vk == vk) {
                chord.0 |= u64::from(modifier.bit);
            } else if chord.keys().count() == MAX_KEYS || chord.keys().any(|key| key == vk) {
                // A key held twice takes two bytes, yet the keyboard reports it once: it could never match.
                return None;
            } else {
                chord.0 = ((chord.0 & !0xFF) << 8) | (u64::from(vk) << 8) | (chord.0 & 0xFF);
            }
        }
        (chord.0 != 0).then_some(chord)
    }

    pub(crate) fn modifiers(self) -> impl Iterator<Item = &'static Modifier> {
        MODIFIERS.iter().filter(move |modifier| self.0 & u64::from(modifier.bit) != 0)
    }

    /// The keys besides modifiers, in press order.
    pub(crate) fn keys(self) -> impl Iterator<Item = u8> {
        let byte = move |at: usize| (self.0 >> (8 * at)) as u8;
        let count = (1..=MAX_KEYS).take_while(|&at| byte(at) != 0).count();
        (1..=count).rev().map(byte)
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
}
