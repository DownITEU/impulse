//! Names of Windows virtual-key codes, for the hotkey caps.

/// `[17, 16, 117]` -> `CTRL+SHIFT+F6`.
pub fn chord(keys: &[u16]) -> String {
    let names: Vec<String> = keys.iter().map(|&key| name(key)).collect();
    names.join("+")
}

fn name(key: u16) -> String {
    let fixed = match key {
        0x08 => "BKSP",
        0x09 => "TAB",
        0x0D => "ENTER",
        0x10 | 0xA0 | 0xA1 => "SHIFT",
        0x11 | 0xA2 | 0xA3 => "CTRL",
        0x12 | 0xA4 | 0xA5 => "ALT",
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
        0x30..=0x39 | 0x41..=0x5A => return char::from(key as u8).to_string(),
        0x60..=0x69 => return format!("NUM {}", key - 0x60),
        0x70..=0x87 => return format!("F{}", key - 0x6F),
        _ => return format!("VK{key}"),
    };
    fixed.to_owned()
}

#[cfg(test)]
mod tests {
    use super::chord;

    #[test]
    fn names_chords() {
        assert_eq!(chord(&[17, 16, 117]), "CTRL+SHIFT+F6");
        assert_eq!(chord(&[18, 98]), "ALT+NUM 2");
        assert_eq!(chord(&[0x41]), "A");
        assert_eq!(chord(&[7]), "VK7");
    }
}
