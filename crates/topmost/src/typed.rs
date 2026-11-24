//! What making a keystroke of a key typed into the game takes: the game's keyboard layout and the keys held.

use std::ptr;

use windows_sys::Win32::{
    Foundation::HWND,
    UI::{
        Input::KeyboardAndMouse::{GetKeyboardLayout, VK_CAPITAL, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_MENU, VK_SHIFT},
        WindowsAndMessaging::GetWindowThreadProcessId,
    },
};

/// The modifiers down while the key was pressed, and whether Caps Lock was on.
#[derive(Clone, Copy, Default)]
pub struct Held {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub caps: bool,
}

/// The keyboard layout (`HKL`) of `game`'s thread; a gone window gives this thread's.
pub fn layout(game: isize) -> isize {
    // SAFETY: plain Win32 calls; a gone window gives thread 0.
    unsafe { GetKeyboardLayout(GetWindowThreadProcessId(game as HWND, ptr::null_mut())) as isize }
}

/// `held` as `GetKeyboardState` gives it: the side-less key and the left one, Caps Lock's toggle in the low bit.
pub fn key_state(held: Held) -> [u8; 256] {
    let mut state = [0; 256];
    let keys = [(held.control, [VK_CONTROL, VK_LCONTROL]), (held.alt, [VK_MENU, VK_LMENU]), (held.shift, [VK_SHIFT, VK_LSHIFT])];
    for key in keys.into_iter().filter(|(down, _)| *down).flat_map(|(_, keys)| keys) {
        state[usize::from(key)] = 0x80;
    }
    state[usize::from(VK_CAPITAL)] = u8::from(held.caps);
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_held_modifiers_and_caps_lock_are_in_the_state() {
        let state = key_state(Held { control: true, alt: false, shift: true, caps: true });
        let mut down = vec![VK_SHIFT, VK_CONTROL, VK_LSHIFT, VK_LCONTROL];
        down.sort_unstable();
        assert_eq!((0..256u16).filter(|&vk| state[usize::from(vk)] == 0x80).collect::<Vec<_>>(), down);
        assert_eq!(state[usize::from(VK_CAPITAL)], 1);
    }
}
