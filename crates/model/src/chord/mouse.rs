//! The mouse in Wand's codes: the buttons' virtual keys, and 10 and 11 (reserved by Windows) for a
//! wheel notch.

pub const LBUTTON: u8 = 0x01;
pub const RBUTTON: u8 = 0x02;
pub const MBUTTON: u8 = 0x04;
pub const XBUTTON1: u8 = 0x05;
pub const XBUTTON2: u8 = 0x06;
pub const WHEEL_UP: u8 = 0x0A;
pub const WHEEL_DOWN: u8 = 0x0B;

pub fn is_mouse(vk: u8) -> bool {
    matches!(vk, LBUTTON | RBUTTON | MBUTTON | XBUTTON1 | XBUTTON2 | WHEEL_UP | WHEEL_DOWN)
}

pub(super) fn name(vk: u8) -> Option<&'static str> {
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
