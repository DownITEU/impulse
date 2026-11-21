//! A game run as administrator denies our process: the host executable, started again elevated through UAC
//! (`runas`), does that one load.

use std::{
    env,
    ffi::{OsStr, OsString},
    io, mem,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    ptr,
    sync::OnceLock,
};

use anyhow::{Result, anyhow, bail, ensure};
use model::{Failure, FailureKind, Process};
use windows_sys::Win32::{
    Foundation::{ERROR_CANCELLED, WAIT_OBJECT_0},
    Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation},
    System::Threading::{GetCurrentProcess, GetExitCodeProcess, OpenProcessToken, WaitForSingleObject},
    UI::{
        Shell::{SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW},
        WindowsAndMessaging::SW_HIDE,
    },
};

use crate::{handle::Handle, image::Image, inject};

const FLAG: &str = "--agent-load";
// UAC's prompt is answered before the copy starts: this covers its load alone.
const WITHIN_MS: u32 = 60_000;

// The elevated copy's exit codes.
const LOADED: u32 = 0;
/// Something may have run in the game: the image stays locked.
const FAILED: u32 = 1;
const DENIED: u32 = 2;
const ARCHITECTURE: u32 = 3;
/// Nothing ran: a bad request, an image or a process that does not check out.
const REFUSED: u32 = 4;

/// The executable a denied load starts elevated: set only by `serve_elevated`.
static HOST: OnceLock<PathBuf> = OnceLock::new();

/// First thing in the executable's `main`. When the command line asks for an elevated load, does it and returns its
/// exit code; otherwise makes this executable the one a denied `load` starts elevated, and returns `None`.
/// `publishers`: the signers a DLL may carry in that load; the request never chooses them.
pub fn serve_elevated(publishers: &[&str]) -> Option<i32> {
    let mut args = env::args_os().skip(1);
    if args.next().as_deref() != Some(OsStr::new(FLAG)) {
        if let Ok(exe) = env::current_exe() {
            let _ = HOST.set(exe);
        }
        return None;
    }
    let code = match args.next().as_deref().and_then(Request::decode) {
        Some(request) => request.load(publishers),
        _ => REFUSED,
    };
    Some(code as i32)
}

/// A host executable serves elevated loads and this process is not elevated yet.
pub(crate) fn can_elevate() -> bool {
    HOST.get().is_some() && !elevated()
}

/// Loads through an elevated copy of the host executable, after the user accepts UAC's prompt.
pub(crate) fn load(target: &Process, image: &Image, export: &str, arg: &[u8]) -> Result<()> {
    let Some(exe) = HOST.get() else { bail!("no executable serves elevated loads") };
    let request = Request { target: target.clone(), image: image.path().to_owned(), export: export.to_owned(), arg: arg.to_vec() };
    let pid = target.pid;
    match run(exe, &format!("{FLAG} {}", request.encode()), image)? {
        LOADED => Ok(()),
        DENIED => Err(anyhow!("process {pid} cannot be opened even as administrator").context(Failure::new(FailureKind::Protected))),
        ARCHITECTURE => {
            Err(anyhow!("process {pid} and {} differ in architecture", image.path().display())
                .context(Failure::new(FailureKind::Architecture)))
        }
        REFUSED => bail!("the elevated load refused its request"),
        code => {
            // Its loader may still open the file after the copy exits.
            image.keep_locked();
            bail!("the elevated load failed with exit code {code}")
        }
    }
}

/// Starts `exe` elevated and waits for its exit code. Declining UAC's prompt is `Elevation`.
fn run(exe: &Path, parameters: &str, image: &Image) -> Result<u32> {
    let (verb, file, parameters) = (wide(OsStr::new("runas")), wide(exe.as_os_str()), wide(OsStr::new(parameters)));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpVerb: verb.as_ptr(),
        lpFile: file.as_ptr(),
        lpParameters: parameters.as_ptr(),
        nShow: SW_HIDE,
        ..Default::default()
    };
    // SAFETY: the strings are NUL-terminated and outlive the call; `info` carries its own size.
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        let error = io::Error::last_os_error();
        let declined = error.raw_os_error() == Some(ERROR_CANCELLED as i32);
        let error = anyhow!("cannot start the elevated load: {error}");
        return Err(if declined { error.context(Failure::new(FailureKind::Elevation)) } else { error });
    }
    ensure!(!info.hProcess.is_null(), "the elevated load has no process");
    let process = Handle(info.hProcess);
    // SAFETY: a live process handle.
    if unsafe { WaitForSingleObject(process.0, WITHIN_MS) } != WAIT_OBJECT_0 {
        image.keep_locked();
        bail!("the elevated load did not finish in time");
    }
    let mut code = 0;
    // SAFETY: a live process handle; `code` outlives the call.
    ensure!(unsafe { GetExitCodeProcess(process.0, &mut code) } != 0, "no exit code: {}", io::Error::last_os_error());
    Ok(code)
}

/// This process runs as administrator: it loads into a game run as administrator itself.
pub fn elevated() -> bool {
    let mut token = ptr::null_mut();
    // SAFETY: the pseudo handle of this process needs no closing; the token is closed by its guard.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return false;
    }
    let token = Handle(token);
    let (mut elevation, mut size) = (TOKEN_ELEVATION::default(), 0);
    let wanted = mem::size_of::<TOKEN_ELEVATION>() as u32;
    // SAFETY: `elevation` is `wanted` bytes; both outputs outlive the call.
    let read = unsafe { GetTokenInformation(token.0, TokenElevation, (&raw mut elevation).cast(), wanted, &mut size) } != 0;
    read && elevation.TokenIsElevated != 0
}

/// What the elevated copy loads; it checks the image and the process again, as any load does.
#[derive(Debug, PartialEq)]
struct Request {
    target: Process,
    image: PathBuf,
    export: String,
    arg: Vec<u8>,
}

impl Request {
    /// Fields in hex joined by `.`: nothing on the command line needs quoting.
    fn encode(&self) -> String {
        let fields = [
            self.target.pid.to_string(),
            self.target.created.to_string(),
            hex(&utf16(self.target.path.as_os_str())),
            hex(&utf16(self.image.as_os_str())),
            hex(self.export.as_bytes()),
            hex(&self.arg),
        ];
        fields.join(".")
    }

    fn decode(text: &OsStr) -> Option<Self> {
        let mut fields = text.to_str()?.split('.');
        let mut next = || fields.next();
        let (pid, created) = (next()?.parse().ok()?, next()?.parse().ok()?);
        let (path, image) = (path(&unhex(next()?)?)?, path(&unhex(next()?)?)?);
        let export = String::from_utf8(unhex(next()?)?).ok()?;
        let arg = unhex(next()?)?;
        next().is_none().then_some(Self { target: Process { pid, path, created }, image, export, arg })
    }

    fn load(self, publishers: &[&str]) -> u32 {
        let Ok(image) = Image::open(&self.image, publishers) else { return REFUSED };
        match inject::load(&self.target, &image, &self.export, &self.arg) {
            Ok(()) => LOADED,
            Err(e) if inject::marked(&e, FailureKind::Protected) => DENIED,
            Err(e) if inject::marked(&e, FailureKind::Architecture) => ARCHITECTURE,
            Err(_) => FAILED,
        }
    }
}

fn wide(text: &OsStr) -> Vec<u16> {
    text.encode_wide().chain([0]).collect()
}

fn utf16(text: &OsStr) -> Vec<u8> {
    text.encode_wide().flat_map(u16::to_le_bytes).collect()
}

fn path(bytes: &[u8]) -> Option<PathBuf> {
    let units: Vec<u16> = bytes.chunks(2).map(|pair| Some(u16::from_le_bytes(pair.try_into().ok()?))).collect::<Option<_>>()?;
    Some(OsString::from_wide(&units).into())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    let digits = text.as_bytes();
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    digits.chunks(2).map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> Request {
        Request {
            target: Process { pid: 19860, path: r"D:\Games\Resident Evil Requiem\re9.exe".into(), created: 134_000_000_000_000_000 },
            image: r"C:\Users\Игрок\AppData\Local\Some App 1.2\service.dll".into(),
            export: "StartService".into(),
            arg: br#"{"port":5000}"#.iter().copied().chain([0]).collect(),
        }
    }

    #[test]
    fn a_request_crosses_the_command_line_whole() {
        let encoded = request().encode();
        assert!(encoded.bytes().all(|byte| byte.is_ascii_hexdigit() || byte == b'.'), "{encoded}");
        assert_eq!(Request::decode(OsStr::new(&encoded)), Some(request()));
    }

    #[test]
    fn a_damaged_request_is_refused() {
        let encoded = request().encode();
        for damaged in [format!("{encoded}.00"), encoded[..encoded.len() - 1].to_owned(), encoded.replacen('.', "x.", 1), String::new()] {
            assert_eq!(Request::decode(OsStr::new(&damaged)), None, "{damaged}");
        }
    }
}
