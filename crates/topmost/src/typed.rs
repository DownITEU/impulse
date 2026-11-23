//! What a key typed into the in-game HUD is, in the layout of the game the user types in. It follows how GPUI's
//! Windows backend makes a keystroke from `WM_KEYDOWN`, so the HUD reads keys as its own windows do.

use std::ptr;

use windows_sys::Win32::{
    Foundation::HWND,
    UI::{
        Input::KeyboardAndMouse::{
            GetKeyboardLayout, HKL, MapVirtualKeyExW, ToUnicodeEx, VK_0, VK_1, VK_2, VK_3, VK_4, VK_5, VK_6, VK_7, VK_8, VK_9, VK_ABNT_C1,
            VK_APPS, VK_BACK, VK_BROWSER_BACK, VK_BROWSER_FORWARD, VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F1,
            VK_F24, VK_HOME, VK_INSERT, VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_NEXT, VK_OEM_1, VK_OEM_2, VK_OEM_3,
            VK_OEM_4, VK_OEM_5, VK_OEM_6, VK_OEM_7, VK_OEM_8, VK_OEM_102, VK_OEM_COMMA, VK_OEM_MINUS, VK_OEM_PERIOD, VK_OEM_PLUS,
            VK_PACKET, VK_PRIOR, VK_RCONTROL, VK_RETURN, VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP,
        },
        WindowsAndMessaging::GetWindowThreadProcessId,
    },
};

const MAPVK_VK_TO_CHAR: u32 = 2;
// Never changes the kernel keyboard state, so a dead key does not compose (a known limit).
const KEEP_STATE: u32 = 0x4;

/// The modifiers down while the key was pressed, and whether Caps Lock was on.
#[derive(Clone, Copy, Default)]
pub struct Held {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub caps: bool,
}

/// A key typed into the HUD.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Typed {
    /// The key's name: "enter", "f5", "a", or the shifted character when `shifted`.
    pub key: String,
    /// The text the key types with the modifiers held; none for a control character.
    pub text: Option<String>,
    /// `key` already carries shift, so the caller drops shift from the modifiers.
    pub shifted: bool,
}

/// How a virtual key becomes a name.
#[derive(Debug, PartialEq, Eq)]
enum Kind {
    /// A modifier, Caps Lock or `VK_PACKET`: no key.
    Ignored,
    Named(String),
    /// The layout names it; `shiftable` keys name their shifted character while shift is held.
    Char {
        shiftable: bool,
    },
}

/// The key `vk` (with its `scan` code) typed into `game`, or none when it is not a key to type (a dead one either).
pub fn typed(game: isize, vk: u8, scan: u16, held: Held) -> Option<Typed> {
    let kind = kind(vk);
    if kind == Kind::Ignored {
        return None;
    }
    // SAFETY: plain Win32 calls; a gone window gives thread 0, which is this thread's layout.
    let layout = unsafe { GetKeyboardLayout(GetWindowThreadProcessId(game as HWND, ptr::null_mut())) };
    let (key, shifted) = match kind {
        Kind::Ignored => return None,
        Kind::Named(name) => (name, false),
        Kind::Char { shiftable: true } if held.shift => (shifted_key(vk, scan, layout)?, true),
        Kind::Char { .. } => (plain_key(vk, layout)?, false),
    };
    let (count, buffer) = unicode(vk, scan, held, layout);
    // A dead key would type its accent at once: it never composes here.
    if count < 0 {
        return None;
    }
    Some(Typed { key, text: text(count, &buffer), shifted })
}

fn kind(vk: u8) -> Kind {
    let vk = u16::from(vk);
    if matches!(
        vk,
        VK_SHIFT
            | VK_LSHIFT
            | VK_RSHIFT
            | VK_CONTROL
            | VK_LCONTROL
            | VK_RCONTROL
            | VK_MENU
            | VK_LMENU
            | VK_RMENU
            | VK_LWIN
            | VK_RWIN
            | VK_CAPITAL
            | VK_PACKET
    ) {
        return Kind::Ignored;
    }
    let name = match vk {
        VK_SPACE => "space",
        VK_BACK => "backspace",
        VK_RETURN => "enter",
        VK_TAB => "tab",
        VK_UP => "up",
        VK_DOWN => "down",
        VK_RIGHT => "right",
        VK_LEFT => "left",
        VK_HOME => "home",
        VK_END => "end",
        VK_PRIOR => "pageup",
        VK_NEXT => "pagedown",
        VK_BROWSER_BACK => "back",
        VK_BROWSER_FORWARD => "forward",
        VK_ESCAPE => "escape",
        VK_INSERT => "insert",
        VK_DELETE => "delete",
        VK_APPS => "menu",
        VK_F1..=VK_F24 => return Kind::Named(format!("f{}", vk - VK_F1 + 1)),
        _ => return Kind::Char { shiftable: shiftable(vk) },
    };
    Kind::Named(name.into())
}

// The keys whose shifted character is the key's name, so shift is not a modifier of it.
fn shiftable(vk: u16) -> bool {
    matches!(
        vk,
        VK_OEM_3
            | VK_OEM_MINUS
            | VK_OEM_PLUS
            | VK_OEM_4
            | VK_OEM_5
            | VK_OEM_6
            | VK_OEM_1
            | VK_OEM_7
            | VK_OEM_COMMA
            | VK_OEM_PERIOD
            | VK_OEM_2
            | VK_OEM_102
            | VK_OEM_8
            | VK_ABNT_C1
            | VK_0
            | VK_1
            | VK_2
            | VK_3
            | VK_4
            | VK_5
            | VK_6
            | VK_7
            | VK_8
            | VK_9
    )
}

// What Windows reads of the modifiers: the side-less key and the left one.
fn state(held: Held) -> [u8; 256] {
    let mut state = [0; 256];
    let keys = [(held.control, [VK_CONTROL, VK_LCONTROL]), (held.alt, [VK_MENU, VK_LMENU]), (held.shift, [VK_SHIFT, VK_LSHIFT])];
    for key in keys.into_iter().filter(|(down, _)| *down).flat_map(|(_, keys)| keys) {
        state[usize::from(key)] = 0x80;
    }
    // The low bit is a toggle's state.
    state[usize::from(VK_CAPITAL)] = u8::from(held.caps);
    state
}

// ToUnicodeEx's count (negative for a dead key) and what it wrote.
fn unicode(vk: u8, scan: u16, held: Held, layout: HKL) -> (i32, [u16; 8]) {
    let (state, mut buffer) = (state(held), [0; 8]);
    // SAFETY: `state` has the 256 bytes it reads and `buffer` the 8 units it is told of; both outlive the call.
    let count = unsafe {
        ToUnicodeEx(u32::from(vk), u32::from(scan), state.as_ptr(), buffer.as_mut_ptr(), buffer.len() as i32, KEEP_STATE, layout)
    };
    (count, buffer)
}

fn text(count: i32, buffer: &[u16]) -> Option<String> {
    let text = String::from_utf16(buffer.get(..usize::try_from(count).ok()?)?).ok()?;
    text.chars().next().is_some_and(|first| !first.is_control()).then_some(text)
}

// The character of the key with shift alone, or its dead key's accent.
fn shifted_key(vk: u8, scan: u16, layout: HKL) -> Option<String> {
    let (count, buffer) = unicode(vk, scan & 0xFF, Held { shift: true, ..Held::default() }, layout);
    let units = buffer.get(..count.unsigned_abs() as usize)?;
    let key = String::from_utf16(units).ok()?;
    (count < 0 || key.chars().next().is_some_and(|first| !first.is_control())).then_some(key)
}

fn plain_key(vk: u8, layout: HKL) -> Option<String> {
    // SAFETY: plain Win32 call.
    let data = unsafe { MapVirtualKeyExW(u32::from(vk), MAPVK_VK_TO_CHAR, layout) };
    // The high word flags a dead key, the low one is the character.
    let key = char::from_u32(data & 0xFFFF).filter(|&key| key != '\0')?;
    Some(key.to_ascii_lowercase().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(vk: u16) -> Kind {
        kind(vk as u8)
    }

    #[test]
    fn named_keys_have_their_names() {
        let names = [
            (VK_SPACE, "space"),
            (VK_BACK, "backspace"),
            (VK_RETURN, "enter"),
            (VK_TAB, "tab"),
            (VK_UP, "up"),
            (VK_DOWN, "down"),
            (VK_LEFT, "left"),
            (VK_RIGHT, "right"),
            (VK_HOME, "home"),
            (VK_END, "end"),
            (VK_PRIOR, "pageup"),
            (VK_NEXT, "pagedown"),
            (VK_ESCAPE, "escape"),
            (VK_INSERT, "insert"),
            (VK_DELETE, "delete"),
            (VK_APPS, "menu"),
            (VK_BROWSER_BACK, "back"),
            (VK_BROWSER_FORWARD, "forward"),
            (VK_F1, "f1"),
            (VK_F1 + 11, "f12"),
            (VK_F24, "f24"),
        ];
        for (vk, name) in names {
            assert_eq!(named(vk), Kind::Named(name.into()), "{name}");
        }
    }

    #[test]
    fn modifiers_caps_lock_and_packets_are_no_key() {
        let keys = [
            VK_SHIFT,
            VK_LSHIFT,
            VK_RSHIFT,
            VK_CONTROL,
            VK_LCONTROL,
            VK_RCONTROL,
            VK_MENU,
            VK_LMENU,
            VK_RMENU,
            VK_LWIN,
            VK_RWIN,
            VK_CAPITAL,
            VK_PACKET,
        ];
        for vk in keys {
            assert_eq!(named(vk), Kind::Ignored, "{vk:#x}");
            assert_eq!(typed(0, vk as u8, 0, Held::default()), None, "{vk:#x}");
        }
    }

    #[test]
    fn digits_and_punctuation_follow_shift_but_letters_do_not() {
        assert_eq!(named(VK_1), Kind::Char { shiftable: true });
        assert_eq!(named(VK_OEM_PLUS), Kind::Char { shiftable: true });
        assert_eq!(named(u16::from(b'A')), Kind::Char { shiftable: false });
    }

    #[test]
    fn only_the_held_modifiers_and_caps_lock_are_in_the_state() {
        let state = state(Held { control: true, alt: false, shift: true, caps: true });
        let mut down = vec![VK_SHIFT, VK_CONTROL, VK_LSHIFT, VK_LCONTROL];
        down.sort_unstable();
        assert_eq!((0..256u16).filter(|&vk| state[usize::from(vk)] == 0x80).collect::<Vec<_>>(), down);
        assert_eq!(state[usize::from(VK_CAPITAL)], 1);
    }
}
