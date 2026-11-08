//! Short key names, as the hotkey caps show them.

use std::{borrow::Cow, fmt};

use crate::Chord;

/// `CTRL+SHIFT+F6`: the modifiers first, then the keys in press order.
impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let modifiers = self.modifiers().map(|modifier| Cow::Borrowed(modifier.name));
        for (at, part) in modifiers.chain(self.keys().map(name)).enumerate() {
            if at > 0 {
                f.write_str("+")?;
            }
            f.write_str(&part)?;
        }
        Ok(())
    }
}

fn name(vk: u8) -> Cow<'static, str> {
    let fixed = match vk {
        0x08 => "BKSP",
        0x09 => "TAB",
        0x0D => "ENTER",
        0x14 => "CAPS",
        0x1B => "ESC",
        0x20 => "SPACE",
        0x21 => "PGUP",
        0x22 => "PGDN",
        0x23 => "END",
        0x24 => "HOME",
        0x25 => "LEFT",
        0x26 => "UP",
        0x27 => "RIGHT",
        0x28 => "DOWN",
        0x2D => "INS",
        0x2E => "DEL",
        0x6A => "NUM *",
        0x6B => "NUM +",
        0x6D => "NUM -",
        0x6E => "NUM .",
        0x6F => "NUM /",
        0xBA => ";",
        0xBB => "=",
        0xBC => ",",
        0xBD => "-",
        0xBE => ".",
        0xBF => "/",
        0xC0 => "`",
        0xDB => "[",
        0xDC => "\\",
        0xDD => "]",
        0xDE => "'",
        // Digits and letters are their own ASCII codes.
        0x30..=0x39 | 0x41..=0x5A => return char::from(vk).to_string().into(),
        0x60..=0x69 => return format!("NUM {}", vk - 0x60).into(),
        0x70..=0x87 => return format!("F{}", vk - 0x6F).into(),
        _ => return format!("VK{vk}").into(),
    };
    fixed.into()
}

#[cfg(test)]
mod tests {
    use crate::Chord;

    fn named(vks: &[u16]) -> String {
        Chord::from_vks(vks).unwrap().to_string()
    }

    #[test]
    fn names_chords() {
        assert_eq!(named(&[17, 16, 117]), "CTRL+SHIFT+F6");
        assert_eq!(named(&[18, 98]), "ALT+NUM 2");
        assert_eq!(named(&[0x41]), "A");
        assert_eq!(named(&[7]), "VK7");
    }

    #[test]
    fn right_modifiers_carry_an_r_and_modifiers_keep_one_order() {
        assert_eq!(named(&[0xA3, 0xA1, 0x41]), "RCTRL+RSHIFT+A");
        assert_eq!(named(&[18, 17, 0x41]), "CTRL+ALT+A");
    }
}
