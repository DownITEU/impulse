use anyhow::Result;
use windows::Win32::Media::MediaFoundation::{MF_VERSION, MFSTARTUP_LITE, MFShutdown, MFStartup};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

/// COM and Media Foundation alive on the current thread. Whatever uses MF
/// objects must be dropped before this is.
pub struct Runtime;

impl Runtime {
    pub fn start() -> Result<Runtime> {
        // S_FALSE (already initialised) is a success that still needs its CoUninitialize.
        // SAFETY: no pointers; a failure leaves this thread's COM state as it was, a success is undone by CoUninitialize below or in drop.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()?;
        // Local files only: no sockets.
        // SAFETY: COM is initialised on this thread by the call above; a success is undone by MFShutdown in drop.
        if let Err(e) = unsafe { MFStartup(MF_VERSION, MFSTARTUP_LITE) } {
            // SAFETY: undoes the CoInitializeEx above, which succeeded on this thread.
            unsafe { CoUninitialize() };
            return Err(e.into());
        }
        Ok(Runtime)
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        // SAFETY: pairs with the MFStartup in start, and drop runs once; a Runtime lives and dies on its decoder thread.
        let _ = unsafe { MFShutdown() };
        // SAFETY: pairs with the CoInitializeEx in start, on the same thread, and drop runs once.
        unsafe { CoUninitialize() };
    }
}
