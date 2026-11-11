use anyhow::Result;
use windows::Win32::Media::MediaFoundation::{MF_VERSION, MFSTARTUP_LITE, MFShutdown, MFStartup};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

/// COM and Media Foundation alive on the current thread. Whatever uses MF
/// objects must be dropped before this is.
pub struct Runtime;

impl Runtime {
    pub fn start() -> Result<Runtime> {
        // S_FALSE (already initialised) is a success that still needs its CoUninitialize.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()?;
        // Local files only: no sockets.
        if let Err(e) = unsafe { MFStartup(MF_VERSION, MFSTARTUP_LITE) } {
            unsafe { CoUninitialize() };
            return Err(e.into());
        }
        Ok(Runtime)
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = unsafe { MFShutdown() };
        unsafe { CoUninitialize() };
    }
}
