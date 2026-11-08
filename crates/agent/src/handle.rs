use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};

/// A Win32 handle closed on drop.
pub struct Handle(pub HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle is owned, valid and closed once.
        unsafe { CloseHandle(self.0) };
    }
}
