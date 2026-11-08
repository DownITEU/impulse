//! The mouse in Wand's codes: the buttons' virtual keys, and 10 and 11 (reserved by Windows) for a
//! wheel notch.

use windows_sys::Win32::UI::WindowsAndMessaging::{
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_XBUTTONDOWN, WM_XBUTTONUP,
};

pub(crate) const LBUTTON: u8 = 0x01;
pub(crate) const RBUTTON: u8 = 0x02;
const MBUTTON: u8 = 0x04;
const XBUTTON1: u8 = 0x05;
const XBUTTON2: u8 = 0x06;
const WHEEL_UP: u8 = 0x0A;
const WHEEL_DOWN: u8 = 0x0B;

pub(crate) fn is_mouse(vk: u8) -> bool {
    matches!(vk, LBUTTON | RBUTTON | MBUTTON | XBUTTON1 | XBUTTON2 | WHEEL_UP | WHEEL_DOWN)
}

/// A notch has no release of its own: the hook adds one.
pub(crate) fn is_notch(vk: u8) -> bool {
    matches!(vk, WHEEL_UP | WHEEL_DOWN)
}

pub(crate) fn name(vk: u8) -> Option<&'static str> {
    Some(match vk {
        LBUTTON => "LMB",
        RBUTTON => "RMB",
        MBUTTON => "MMB",
        XBUTTON1 => "MB4",
        XBUTTON2 => "MB5",
        WHEEL_UP => "WHEEL UP",
        WHEEL_DOWN => "WHEEL DN",
        _ => return None,
    })
}

/// The code and whether it went down, from a `WH_MOUSE_LL` message and its `mouseData`; a notch is a down.
pub(crate) fn input(message: u32, data: u32) -> Option<(u8, bool)> {
    let high = (data >> 16) as u16;
    Some(match message {
        WM_LBUTTONDOWN | WM_LBUTTONUP => (LBUTTON, message == WM_LBUTTONDOWN),
        WM_RBUTTONDOWN | WM_RBUTTONUP => (RBUTTON, message == WM_RBUTTONDOWN),
        WM_MBUTTONDOWN | WM_MBUTTONUP => (MBUTTON, message == WM_MBUTTONDOWN),
        WM_XBUTTONDOWN | WM_XBUTTONUP => match high {
            1 => (XBUTTON1, message == WM_XBUTTONDOWN),
            2 => (XBUTTON2, message == WM_XBUTTONDOWN),
            _ => return None,
        },
        WM_MOUSEWHEEL => match high as i16 {
            0 => return None,
            delta => (if delta > 0 { WHEEL_UP } else { WHEEL_DOWN }, true),
        },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_buttons_and_notches_take_wands_codes() {
        assert_eq!(input(WM_XBUTTONDOWN, 2 << 16), Some((XBUTTON2, true)));
        assert_eq!(input(WM_XBUTTONUP, 1 << 16), Some((XBUTTON1, false)));
        assert_eq!(input(WM_MOUSEWHEEL, 120 << 16), Some((WHEEL_UP, true)));
        assert_eq!(input(WM_MOUSEWHEEL, u32::from(-120_i16 as u16) << 16), Some((WHEEL_DOWN, true)));
        assert_eq!(input(WM_MOUSEWHEEL, 0), None);
    }
}
