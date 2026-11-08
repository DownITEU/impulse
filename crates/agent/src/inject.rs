//! Loads a DLL into a running process and starts one of its exports there.
//! x64 targets only: an x86 game needs a 32-bit image and its own `LoadLibraryW`.

use std::{ffi::c_void, io, mem::transmute, os::windows::ffi::OsStrExt, ptr};

use anyhow::{Context, Result, bail, ensure};
use model::Process;
use windows_sys::Win32::{
    Foundation::{HMODULE, WAIT_OBJECT_0},
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

use crate::{handle::Handle, image::Image, process};

const LOAD_WITHIN_MS: u32 = 30_000;

type ThreadStart = unsafe extern "system" fn(*mut c_void) -> u32;

/// Loads `image` into `target` and starts `export` with a pointer to a copy of `arg`, which stays
/// in the target: the thread reads it after this returns. Returns once the thread runs; the DLL
/// stays mapped for the process's lifetime. `target` is checked against the opened process: a
/// reused pid must not receive the DLL.
pub fn load(target: &Process, image: &Image, export: &str, arg: &[u8]) -> Result<()> {
    let pid = target.pid;
    let start = pe::export(image.path(), export).with_context(|| format!("{} has no {export}", image.path().display()))?;
    let process = open(pid)?;
    process::verify(&process, target)?;
    let mut wow = 0;
    // SAFETY: `process` is a live handle and `wow` outlives the call.
    ensure!(unsafe { IsWow64Process(process.0, &mut wow) } != 0, "cannot query process {pid}: {}", io::Error::last_os_error());
    ensure!(wow == 0, "process {pid} is 32-bit; only x64 games are supported");

    let wide: Vec<u8> = image.path().as_os_str().encode_wide().chain([0]).flat_map(u16::to_le_bytes).collect();
    let path = write(&process, &wide)?;
    let loader = spawn(&process, load_library()?, path.ptr)?;
    if let Err(err) = wait(&loader) {
        // The loader may yet read the path and open the file, so both must outlive this call.
        path.leak();
        image.keep_locked();
        return Err(err);
    }
    drop(path);
    // The thread's exit code is only the low half of the module handle, so the module list decides.
    let base = module_base(&process, pid, image)?;

    let arg = write(&process, arg)?;
    spawn(&process, start_address(base, start), arg.ptr)?;
    arg.leak();
    Ok(())
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
    // RVA for the loaded image) taking one pointer, and `arg` points into the target.
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
    // SAFETY: `base + rva` is the image's exported start routine, a thread routine.
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
    use std::path::PathBuf;

    use super::*;

    const KERNEL32: &str = r"C:\Windows\System32\kernel32.dll";

    #[test]
    fn an_image_without_the_export_is_refused_before_a_process_is_opened() {
        let nobody = Process { pid: u32::MAX, path: PathBuf::new(), created: 0 };
        // The pid is bogus: only the first check can be what fails.
        let err = load(&nobody, &Image::unchecked(KERNEL32), "NoSuchExport", &[]).unwrap_err();
        assert!(err.to_string().contains("NoSuchExport"), "{err}");
    }

    #[test]
    fn a_process_that_is_not_the_one_found_is_never_injected() {
        let image = Image::unchecked(KERNEL32);
        assert!(pe::export(image.path(), "Sleep").is_some(), "the export must pass the first check");
        let mut me = process::of(std::process::id()).unwrap();
        // A pid that was reused has another creation time: nothing may happen before this error.
        me.created += 1;
        let err = load(&me, &image, "Sleep", &[]).unwrap_err();
        assert!(err.to_string().contains("reused"), "{err}");
    }
}
