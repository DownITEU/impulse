//! A hotkey as Windows registers it: modifiers and one key.

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN};

// The modifiers' virtual keys, either side, and the left one a chord names them by.
const MODIFIERS: [(HOT_KEY_MODIFIERS, [u8; 2]); 4] =
    [(MOD_CONTROL, [0xA2, 0xA3]), (MOD_SHIFT, [0xA0, 0xA1]), (MOD_ALT, [0xA4, 0xA5]), (MOD_WIN, [0x5B, 0x5C])];
// Side-less Ctrl, Shift and Alt.
const GENERIC: [(u8, HOT_KEY_MODIFIERS); 3] = [(0x11, MOD_CONTROL), (0x10, MOD_SHIFT), (0x12, MOD_ALT)];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Key {
    pub(crate) modifiers: HOT_KEY_MODIFIERS,
    pub(crate) vk: u8,
}

impl Key {
    /// Ctrl+Shift+O.
    pub const DEFAULT: Self = Self { modifiers: MOD_CONTROL | MOD_SHIFT, vk: b'O' };
    /// Ctrl+Shift+M.
    pub const MINIMAP: Self = Self { modifiers: MOD_CONTROL | MOD_SHIFT, vk: b'M' };

    /// For a thread message; never 0, which is no key.
    pub(crate) fn pack(self) -> usize {
        (self.modifiers as usize) << 8 | usize::from(self.vk)
    }

    pub(crate) fn unpack(packed: usize) -> Option<Self> {
        (packed != 0).then_some(Self { modifiers: (packed >> 8) as HOT_KEY_MODIFIERS, vk: packed as u8 })
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
                // The buttons are 1, 2 and 4 to 6, the notches 10 and 11.
                None if matches!(vk, 1..=6 | 10 | 11) || key.replace(vk).is_some() => return None,
                None => {}
            }
        }
        Some(Self { modifiers, vk: key? }).filter(|key| key.modifiers != 0)
    }

    /// Whether pressing `vks` fires it: Windows takes a modifier of either side.
    pub fn fired_by(self, vks: impl IntoIterator<Item = u8>) -> bool {
        Self::from_vks(vks) == Some(self)
    }

    /// The modifiers by their left keys, then the key.
    pub fn vks(self) -> impl Iterator<Item = u8> {
        let modifiers = MODIFIERS.into_iter().filter(move |(modifier, _)| self.modifiers & modifier != 0).map(|(_, [left, _])| left);
        modifiers.chain([self.vk])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_and_one_key() {
        assert_eq!(Key::from_vks([0xA2, 0xA1, b'O']), Some(Key::DEFAULT));
        assert_eq!(Key::from_vks([0x11, 0x10, b'O']), Some(Key::DEFAULT));
        assert_eq!(Key::DEFAULT.vks().collect::<Vec<_>>(), [0xA2, 0xA0, b'O']);
        assert_eq!(Key::from_vks([0x5B, 0x24]), Some(Key { modifiers: MOD_WIN, vk: 0x24 }));
        assert_eq!(Key::from_vks([0x24]), None);
        assert_eq!(Key::from_vks([0xA2, 0x0A]), None);
        assert_eq!(Key::from_vks([0xA2, 0xA0]), None);
        assert_eq!(Key::from_vks([0xA2, b'O', b'P']), None);
        assert_eq!(Key::from_vks([0xA2, 4]), None);
        assert!(Key::DEFAULT.fired_by([0xA3, 0xA1, b'O']));
        assert!(!Key::DEFAULT.fired_by([0xA3, 0xA1, 0xA4, b'O']));
    }

    #[test]
    fn packed_for_a_thread_message() {
        assert_eq!(Key::unpack(Key::MINIMAP.pack()), Some(Key::MINIMAP));
        assert_eq!(Key::unpack(Key { modifiers: MOD_WIN | MOD_ALT, vk: 0xFE }.pack()), Some(Key { modifiers: MOD_WIN | MOD_ALT, vk: 0xFE }));
        assert_eq!(Key::unpack(0), None);
    }
}
