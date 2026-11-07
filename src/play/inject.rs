//! Puts `tophat_service` into a running process and starts it, as Wand's auxiliary service does.
//! x64 targets only: an x86 game needs the 32-bit service and its own `LoadLibraryW`.

use std::{ffi::c_void, io, mem::transmute, os::windows::ffi::OsStrExt, ptr};

use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, HMODULE, WAIT_OBJECT_0},
    System::{
        Diagnostics::Debug::WriteProcessMemory,
        LibraryLoader::{GetModuleHandleW, GetProcAddress},
        Memory::{MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE, VirtualAllocEx, VirtualFreeEx},
        ProcessStatus::{EnumProcessModulesEx, GetModuleFileNameExW, LIST_MODULES_ALL},
        Threading::{
            CreateRemoteThread, IsWow64Process, OpenProcess, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION,
            PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE, WaitForSingleObject,
        },
    },
};

use super::image::Image;
use crate::pe;

const START_EXPORT: &str = "StartTophatServiceThread";
const LOAD_WITHIN_MS: u32 = 30_000;

type ThreadStart = unsafe extern "system" fn(*mut c_void) -> u32;

/// What the service reads to find us: it dials `wand_port` and registers as `client_id`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Startup<'a> {
    pub wand_port: u16,
    pub client_id: &'a str,
    pub trainer_session_id: &'a str,
}

/// Loads `service` into `pid` and starts its thread. Returns once the thread is running; the
/// service registers with the server on its own. The DLL stays mapped for the game's lifetime.
pub fn tophat(pid: u32, service: &Image, startup: &Startup) -> Result<()> {
    let start = pe::export(service.path(), START_EXPORT).with_context(|| format!("the service has no {START_EXPORT}"))?;
    let process = open(pid)?;
    let mut wow = 0;
    // SAFETY: `process` is a live handle and `wow` outlives the call.
    ensure!(unsafe { IsWow64Process(process.0, &mut wow) } != 0, "cannot query process {pid}: {}", io::Error::last_os_error());
    ensure!(wow == 0, "process {pid} is 32-bit; only x64 games are supported");

    let wide: Vec<u8> = service.path().as_os_str().encode_wide().chain([0]).flat_map(u16::to_le_bytes).collect();
    let path = write(&process, &wide)?;
    let loader = spawn(&process, load_library()?, path.ptr)?;
    if let Err(err) = wait(&loader) {
        // The loader may yet read the path and open the file, so both must outlive this call.
        path.leak();
        service.keep_locked();
        return Err(err);
    }
    drop(path);
    // The thread's exit code is only the low half of the module handle, so the module list decides.
    let base = module_base(&process, pid, service)?;

    let mut json = serde_json::to_vec(startup)?;
    json.push(0);
    let arg = write(&process, &json)?;
    spawn(&process, start_address(base, start), arg.ptr)?;
    // The thread reads this after we return.
    arg.leak();
    Ok(())
}

/// A Win32 handle closed on drop.
struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle is owned, valid and closed once.
        unsafe { CloseHandle(self.0) };
    }
}

/// Memory allocated in the target, released on drop unless `leak`ed.
struct Remote<'a> {
    process: &'a Handle,
    ptr: *mut c_void,
}

impl Remote<'_> {
    fn leak(self) {
        std::mem::forget(self);
    }
}

impl Drop for Remote<'_> {
    fn drop(&mut self) {
        // SAFETY: `ptr` came from `VirtualAllocEx` on this process and is released once.
        unsafe { VirtualFreeEx(self.process.0, self.ptr, 0, MEM_RELEASE) };
    }
}

fn open(pid: u32) -> Result<Handle> {
    let access = PROCESS_CREATE_THREAD | PROCESS_QUERY_INFORMATION | PROCESS_VM_OPERATION | PROCESS_VM_WRITE | PROCESS_VM_READ;
    // SAFETY: plain Win32 call; the handle is owned by the returned guard.
    let handle = unsafe { OpenProcess(access, 0, pid) };
    ensure!(!handle.is_null(), "cannot open process {pid}: {}", io::Error::last_os_error());
    Ok(Handle(handle))
}

/// Copies `data` into fresh memory of the process.
fn write<'a>(process: &'a Handle, data: &[u8]) -> Result<Remote<'a>> {
    // SAFETY: plain Win32 call; a null result is checked below.
    let ptr = unsafe { VirtualAllocEx(process.0, ptr::null(), data.len(), MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) };
    ensure!(!ptr.is_null(), "cannot allocate in the process: {}", io::Error::last_os_error());
    let remote = Remote { process, ptr };
    let mut written = 0;
    // SAFETY: the buffer is valid for `data.len()` bytes and the target block was sized for it.
    let copied = unsafe { WriteProcessMemory(process.0, ptr, data.as_ptr().cast(), data.len(), &mut written) } != 0;
    ensure!(copied && written == data.len(), "cannot write to the process: {}", io::Error::last_os_error());
    Ok(remote)
}

/// Starts a thread at `start(arg)` in the process; it runs on if the handle is dropped.
fn spawn(process: &Handle, start: ThreadStart, arg: *mut c_void) -> Result<Handle> {
    // SAFETY: `start` is a function of the target (the same address as ours for kernel32, base plus
    // RVA for the service) taking one pointer, and `arg` points into the target.
    let thread = unsafe { CreateRemoteThread(process.0, ptr::null(), 0, Some(start), arg, 0, ptr::null_mut()) };
    ensure!(!thread.is_null(), "cannot start a thread in the process: {}", io::Error::last_os_error());
    Ok(Handle(thread))
}

fn wait(thread: &Handle) -> Result<()> {
    // SAFETY: the handle is a live thread handle.
    let done = unsafe { WaitForSingleObject(thread.0, LOAD_WITHIN_MS) } == WAIT_OBJECT_0;
    ensure!(done, "the remote thread did not finish in time");
    Ok(())
}

/// kernel32 maps at one address in every process of a boot, so ours is the target's.
fn load_library() -> Result<ThreadStart> {
    let kernel32: Vec<u16> = "kernel32.dll\0".encode_utf16().collect();
    // SAFETY: kernel32 is always loaded; both strings are NUL-terminated.
    let address = unsafe { GetProcAddress(GetModuleHandleW(kernel32.as_ptr()), c"LoadLibraryW".as_ptr().cast()) };
    let address = address.context("LoadLibraryW not found")?;
    // SAFETY: LoadLibraryW takes one pointer, which is a thread routine's shape.
    Ok(unsafe { transmute::<unsafe extern "system" fn() -> isize, ThreadStart>(address) })
}

fn start_address(base: usize, rva: u32) -> ThreadStart {
    // SAFETY: `base + rva` is the service's exported start routine, a thread routine.
    unsafe { transmute::<usize, ThreadStart>(base + rva as usize) }
}

/// The load address of the module loaded from exactly the image's path: another copy of the
/// same file name from elsewhere is a different module. Not Toolhelp: its snapshot fails
/// outright when any module of the target has a path beyond MAX_PATH.
fn module_base(process: &Handle, pid: u32, image: &Image) -> Result<usize> {
    let wanted = image.path().to_string_lossy();
    let mut modules: Vec<HMODULE> = vec![ptr::null_mut(); 1024];
    loop {
        let mut needed = 0;
        // SAFETY: `modules` is valid for the byte length passed and `needed` outlives the call.
        let listed = unsafe {
            EnumProcessModulesEx(process.0, modules.as_mut_ptr(), (modules.len() * size_of::<HMODULE>()) as u32, &mut needed, LIST_MODULES_ALL)
        };
        ensure!(listed != 0, "cannot list the modules of process {pid}: {}", io::Error::last_os_error());
        let count = needed as usize / size_of::<HMODULE>();
        if count <= modules.len() {
            modules.truncate(count);
            break;
        }
        // More modules loaded since: the room needed, with some to spare.
        modules.resize(count + 16, ptr::null_mut());
    }
    let mut name = vec![0u16; 32_768];
    for module in modules {
        // SAFETY: `name` is valid for the length passed.
        let len = unsafe { GetModuleFileNameExW(process.0, module, name.as_mut_ptr(), name.len() as u32) } as usize;
        if len != 0 && String::from_utf16_lossy(&name[..len]).eq_ignore_ascii_case(&wanted) {
            return Ok(module as usize);
        }
    }
    bail!("{} did not load in process {pid}: a dependency may be missing", image.path().display())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        future::Future,
        path::PathBuf,
        process::{Command, Stdio},
        time::Duration,
    };

    use futures::{StreamExt, executor::block_on};
    use tokio::runtime::Handle as Runtime;

    use super::{
        super::tophat::{Server, Session},
        *,
    };

    #[test]
    fn startup_is_the_json_the_service_reads() {
        let startup = Startup { wand_port: 5000, client_id: "c", trainer_session_id: "s" };
        let json = serde_json::to_string(&startup).unwrap();
        assert_eq!(json, r#"{"wandPort":5000,"clientId":"c","trainerSessionId":"s"}"#);
    }

    #[test]
    fn an_image_without_the_start_routine_is_refused_before_a_process_is_opened() {
        let image = Image::unchecked(r"C:\Windows\System32\kernel32.dll");
        let startup = Startup { wand_port: 1, client_id: "c", trainer_session_id: "s" };
        // kernel32 exports no start routine, and the pid is bogus: only the first check can be what fails.
        assert!(tophat(u32::MAX, &image, &startup).unwrap_err().to_string().contains(START_EXPORT));
    }

    fn installed_service() -> Option<PathBuf> {
        let wand = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Wand");
        let mut apps: Vec<_> = fs::read_dir(wand).ok()?.flatten().filter(|entry| entry.file_name().to_string_lossy().starts_with("app-")).collect();
        apps.sort_by_key(|entry| entry.file_name());
        let service = apps.pop()?.path().join(r"resources\app.asar.unpacked\static\unpacked\trainerlib\tophat_service_x64.dll");
        service.is_file().then_some(service)
    }

    /// Waits for `work` from this thread, with the timer on the runtime like the app's callers.
    fn bounded<T: Send + 'static>(runtime: &Runtime, secs: u64, work: impl Future<Output = T> + Send + 'static) -> Result<T> {
        let timed = runtime.spawn(async move { tokio::time::timeout(Duration::from_secs(secs), work).await });
        block_on(timed)?.context("timed out")
    }

    /// Needs a Wand install. The target is a throwaway `ping` we start, never a game.
    #[test]
    #[ignore = "injects the installed tophat_service into a throwaway process"]
    fn the_injected_service_dials_in_and_the_session_ends_cleanly() {
        let service = Image::open(&installed_service().expect("a Wand install with tophat_service_x64.dll")).unwrap();
        let server = Server::start().unwrap();
        let client_id = uuid::Uuid::new_v4().to_string();
        let waiting = server.expect(&client_id);
        let mut target = Command::new(r"C:\Windows\System32\PING.EXE")
            .args(["-n", "120", "127.0.0.1"])
            .stdout(Stdio::null())
            .spawn()
            .unwrap();

        let outcome = (|| -> Result<()> {
            let startup = Startup { wand_port: server.port, client_id: &client_id, trainer_session_id: &uuid::Uuid::new_v4().to_string() };
            tophat(target.id(), &service, &startup)?;
            let channel = bounded(&server.runtime, 20, waiting)?.context("the server dropped the waiter")?;
            let (session, mut events) = Session::open(server.runtime.clone(), channel, &client_id);
            // No trainer is running, so the service may refuse the stop; the session must still end.
            let stopped = block_on(session.stop());
            println!("stop: {stopped:?}");
            ensure!(bounded(&server.runtime, 10, async move { events.next().await })?.is_none());
            Ok(())
        })();

        target.kill().unwrap();
        target.wait().unwrap();
        outcome.unwrap();
    }
}
