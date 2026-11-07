//! Puts `tophat_service` into a running process and starts it, as Wand's auxiliary service does.
//! x64 targets only: an x86 game needs the 32-bit service and its own `LoadLibraryW`.

use std::{
    ffi::c_void,
    fs::OpenOptions,
    io,
    mem::transmute,
    os::windows::{ffi::OsStrExt, fs::OpenOptionsExt},
    path::{self, Path},
    ptr, thread,
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_BAD_LENGTH, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0},
    Storage::FileSystem::FILE_SHARE_READ,
    System::{
        Diagnostics::{
            Debug::WriteProcessMemory,
            ToolHelp::{
                CreateToolhelp32Snapshot, MODULEENTRY32W, Module32FirstW, Module32NextW, TH32CS_SNAPMODULE,
                TH32CS_SNAPMODULE32,
            },
        },
        LibraryLoader::{GetModuleHandleW, GetProcAddress},
        Memory::{MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE, VirtualAllocEx},
        Threading::{
            CreateRemoteThread, IsWow64Process, OpenProcess, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION,
            PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE, WaitForSingleObject,
        },
    },
};

use super::trust;
use crate::pe;

const START_EXPORT: &str = "StartTophatServiceThread";
const LOAD_WITHIN_MS: u32 = 30_000;
const SNAPSHOT_TRIES: u32 = 20;

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
pub fn tophat(pid: u32, service: &Path, startup: &Startup) -> Result<()> {
    // Absolute, so the target cannot resolve it against its own working directory.
    let service = path::absolute(service)?;
    // Held until the load is done: it denies writes and deletes, so the file checked is the file loaded.
    let _locked = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&service)
        .with_context(|| format!("cannot open {}", service.display()))?;
    trust::check(&service)?;
    let image = pe::header(&service).context("the service is not a PE image")?;
    ensure!(image.machine == pe::X64, "only the x64 service is supported");
    let start = pe::export(&service, START_EXPORT).with_context(|| format!("the service has no {START_EXPORT}"))?;

    let process = open(pid)?;
    let mut wow = 0;
    // SAFETY: `process` is a live handle and `wow` outlives the call.
    ensure!(unsafe { IsWow64Process(process.0, &mut wow) } != 0, "cannot query process {pid}: {}", io::Error::last_os_error());
    ensure!(wow == 0, "process {pid} is 32-bit; only x64 games are supported");

    let wide: Vec<u8> = service.as_os_str().encode_wide().chain([0]).flat_map(u16::to_le_bytes).collect();
    wait(&spawn(&process, load_library()?, write(&process, &wide)?)?)?;
    // The thread's exit code is only the low half of the module handle, so the module list decides.
    let base = module_base(pid, &service)?;

    let mut json = serde_json::to_vec(startup)?;
    json.push(0);
    // The thread reads this after we return, so the memory is never freed.
    spawn(&process, start_address(base, start), write(&process, &json)?)?;
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

fn open(pid: u32) -> Result<Handle> {
    let access = PROCESS_CREATE_THREAD | PROCESS_QUERY_INFORMATION | PROCESS_VM_OPERATION | PROCESS_VM_WRITE | PROCESS_VM_READ;
    // SAFETY: plain Win32 call; the handle is owned by the returned guard.
    let handle = unsafe { OpenProcess(access, 0, pid) };
    ensure!(!handle.is_null(), "cannot open process {pid}: {}", io::Error::last_os_error());
    Ok(Handle(handle))
}

/// Copies `data` into fresh memory of the process.
fn write(process: &Handle, data: &[u8]) -> Result<*mut c_void> {
    // SAFETY: allocates in the target; the buffer is valid for `data.len()` bytes during the write.
    unsafe {
        let remote = VirtualAllocEx(process.0, ptr::null(), data.len(), MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
        let mut written = 0;
        let copied = !remote.is_null()
            && WriteProcessMemory(process.0, remote, data.as_ptr().cast(), data.len(), &mut written) != 0;
        ensure!(copied && written == data.len(), "cannot write to the process: {}", io::Error::last_os_error());
        Ok(remote)
    }
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

/// The load address of the module loaded from exactly `path`: another copy of the same
/// file name from elsewhere is a different module.
fn module_base(pid: u32, path: &Path) -> Result<usize> {
    let snapshot = module_snapshot(pid)?;
    let wanted = path.to_string_lossy();
    let mut entry = MODULEENTRY32W {
        dwSize: size_of::<MODULEENTRY32W>() as u32,
        ..Default::default()
    };
    // SAFETY: `entry` is sized as the API requires and outlives the calls.
    let mut more = unsafe { Module32FirstW(snapshot.0, &mut entry) };
    while more != 0 {
        let len = entry.szExePath.iter().position(|unit| *unit == 0).unwrap_or(entry.szExePath.len());
        if String::from_utf16_lossy(&entry.szExePath[..len]).eq_ignore_ascii_case(&wanted) {
            return Ok(entry.modBaseAddr as usize);
        }
        // SAFETY: as above.
        more = unsafe { Module32NextW(snapshot.0, &mut entry) };
    }
    bail!("{} did not load in process {pid}: a dependency may be missing", path.display())
}

fn module_snapshot(pid: u32) -> Result<Handle> {
    for _ in 0..SNAPSHOT_TRIES {
        // SAFETY: plain Win32 call; the handle is owned by the returned guard.
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) };
        if snapshot != INVALID_HANDLE_VALUE {
            return Ok(Handle(snapshot));
        }
        let err = io::Error::last_os_error();
        // The target loaded or unloaded a module while the list was copied; asking again works.
        ensure!(err.raw_os_error() == Some(ERROR_BAD_LENGTH as i32), "cannot list the modules of process {pid}: {err}");
        thread::sleep(Duration::from_millis(10));
    }
    bail!("the modules of process {pid} kept changing")
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
    use tokio::runtime::Handle;

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
    fn an_unsigned_image_never_reaches_a_process() {
        let dll = PathBuf::from(r"C:\Windows\System32\kernel32.dll");
        // Microsoft's, not WeMod's. The pid is bogus: the gate must refuse before any process is opened.
        let err = tophat(u32::MAX, &dll, &Startup { wand_port: 1, client_id: "c", trainer_session_id: "s" }).unwrap_err();
        assert!(err.to_string().contains("not by WeMod LLC"), "{err}");
    }

    fn installed_service() -> Option<PathBuf> {
        let wand = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Wand");
        let mut apps: Vec<_> = fs::read_dir(wand).ok()?.flatten().filter(|entry| entry.file_name().to_string_lossy().starts_with("app-")).collect();
        apps.sort_by_key(|entry| entry.file_name());
        let service = apps.pop()?.path().join(r"resources\app.asar.unpacked\static\unpacked\trainerlib\tophat_service_x64.dll");
        service.is_file().then_some(service)
    }

    /// Waits for `work` from this thread, with the timer on the runtime like the app's callers.
    fn bounded<T: Send + 'static>(runtime: &Handle, secs: u64, work: impl Future<Output = T> + Send + 'static) -> Result<T> {
        let timed = runtime.spawn(async move { tokio::time::timeout(Duration::from_secs(secs), work).await });
        block_on(timed)?.context("timed out")
    }

    /// Needs a Wand install. The target is a throwaway `ping` we start, never a game.
    #[test]
    #[ignore = "injects the installed tophat_service into a throwaway process"]
    fn the_injected_service_dials_in_and_the_session_ends_cleanly() {
        let service = installed_service().expect("a Wand install with tophat_service_x64.dll");
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
