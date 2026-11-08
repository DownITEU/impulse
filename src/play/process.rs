//! Finding the game's process the way Wand does: by executable name or install folder, once it
//! has run long enough to be past its start-up, taking the biggest if several match.

use std::{
    io, mem,
    path::{Path, PathBuf},
    ptr,
    time::Duration,
};

use anyhow::{Result, bail};
use blocking::unblock;
use tokio::runtime::Handle as Runtime;
use windows_sys::Win32::{
    Foundation::FILETIME,
    System::{
        ProcessStatus::{EnumProcesses, GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
        SystemInformation::GetSystemTimeAsFileTime,
        Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW},
    },
};

use super::handle::Handle;

const POLL: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, PartialEq)]
pub struct Process {
    pub pid: u32,
    pub path: PathBuf,
    pub age: Duration,
    pub working_set: usize,
}

#[derive(Clone)]
pub struct Rule {
    /// Path suffix of the executable, `*` matching inside one folder or file name. Without it
    /// the install folder decides.
    pub image: Option<String>,
    pub location: PathBuf,
    /// A younger process is still starting up.
    pub minimum_age: Duration,
}

impl Rule {
    fn matches(&self, process: &Process) -> bool {
        let path = process.path.to_string_lossy().to_lowercase();
        match &self.image {
            Some(image) => ends_with_image(&path, &image.to_lowercase()),
            None => {
                let folder = self.location.to_string_lossy().to_lowercase();
                path.strip_prefix(folder.trim_end_matches('\\')).is_some_and(|rest| rest.starts_with('\\'))
            }
        }
    }
}

/// The matching process that is old enough, the one with the most memory if there are several.
pub fn pick(processes: Vec<Process>, rule: &Rule) -> Option<Process> {
    processes
        .into_iter()
        .filter(|process| rule.matches(process) && process.age >= rule.minimum_age)
        .max_by_key(|process| process.working_set)
}

/// Polls until `pick` finds a process. The sleeps run on `runtime`: this future may be driven by
/// an executor that has no timer.
pub async fn wait(runtime: &Runtime, rule: &Rule, timeout: Duration) -> Result<Process> {
    let mut waited = Duration::ZERO;
    loop {
        let found = {
            let rule = rule.clone();
            unblock(move || running().map(|all| pick(all, &rule))).await?
        };
        if let Some(process) = found {
            return Ok(process);
        }
        if waited >= timeout {
            bail!("the game did not start in {} s", timeout.as_secs());
        }
        // Inside the task: a timer built on this thread would panic without a runtime here.
        let _ = runtime.spawn(async { tokio::time::sleep(POLL).await }).await;
        waited += POLL;
    }
}

/// Every process we may look at (blocking). Protected ones are skipped.
pub fn running() -> Result<Vec<Process>> {
    let mut pids = vec![0u32; 1024];
    let count = loop {
        let mut bytes = 0;
        // SAFETY: `pids` is valid for the byte length passed and `bytes` outlives the call.
        let listed = unsafe { EnumProcesses(pids.as_mut_ptr(), (pids.len() * size_of::<u32>()) as u32, &mut bytes) };
        if listed == 0 {
            bail!("cannot list processes: {}", io::Error::last_os_error());
        }
        let count = bytes as usize / size_of::<u32>();
        if count < pids.len() {
            break count;
        }
        pids.resize(pids.len() * 2, 0);
    };
    let now = now();
    Ok(pids[..count].iter().filter_map(|&pid| inspect(pid, now)).collect())
}

fn inspect(pid: u32, now: u64) -> Option<Process> {
    // SAFETY: plain Win32 call; the handle is owned by the guard.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let process = Handle(handle);

    let mut name = vec![0u16; 32_768];
    let mut len = name.len() as u32;
    // SAFETY: `name` is valid for `len` characters.
    if unsafe { QueryFullProcessImageNameW(process.0, 0, name.as_mut_ptr(), &mut len) } == 0 {
        return None;
    }
    let path = PathBuf::from(String::from_utf16_lossy(&name[..len as usize]));

    let mut times = [FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 }; 4];
    // SAFETY: the four outputs are distinct, valid FILETIMEs.
    let [created, exited, kernel, user] = &mut times;
    if unsafe { GetProcessTimes(process.0, created, exited, kernel, user) } == 0 {
        return None;
    }
    let age = Duration::from_nanos(now.saturating_sub(ticks(created)) * 100);

    // SAFETY: zeroed counters of the declared size.
    let mut memory: PROCESS_MEMORY_COUNTERS = unsafe { mem::zeroed() };
    memory.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    let known = unsafe { GetProcessMemoryInfo(process.0, &mut memory, memory.cb) } != 0;
    Some(Process { pid, path, age, working_set: if known { memory.WorkingSetSize } else { 0 } })
}

/// 100 ns ticks since 1601.
fn ticks(time: &FILETIME) -> u64 {
    (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
}

fn now() -> u64 {
    let mut time = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    // SAFETY: the output is a valid FILETIME.
    unsafe { GetSystemTimeAsFileTime(&mut time) };
    ticks(&time)
}

/// `path` ends with the folders and file of `image`, whole names only: `game.exe` is not `mygame.exe`.
fn ends_with_image(path: &str, image: &str) -> bool {
    let mut names = path.rsplit('\\');
    let patterns = image.rsplit(['\\', '/']);
    patterns.into_iter().all(|pattern| names.next().is_some_and(|name| glob(pattern, name)))
}

/// `*` matches any run of characters.
fn glob(pattern: &str, text: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == text,
        Some((head, tail)) => {
            let Some(rest) = text.strip_prefix(head) else { return false };
            (0..=rest.len()).filter(|&at| rest.is_char_boundary(at)).any(|at| glob(tail, &rest[at..]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(path: &str, secs: u64, working_set: usize) -> Process {
        Process { pid: 1, path: PathBuf::from(path), age: Duration::from_secs(secs), working_set }
    }

    fn rule(image: Option<&str>, location: &str) -> Rule {
        Rule { image: image.map(str::to_owned), location: PathBuf::from(location), minimum_age: Duration::from_secs(8) }
    }

    #[test]
    fn an_image_matches_whole_trailing_names() {
        let path = r"d:\steam\common\enshrouded\enshrouded.exe";
        assert!(ends_with_image(path, "enshrouded.exe"));
        assert!(ends_with_image(path, r"enshrouded\enshrouded.exe"));
        assert!(ends_with_image(path, "ens*ded.exe"));
        assert!(ends_with_image(path, "*.exe"));
        assert!(!ends_with_image(path, "shrouded.exe"), "a longer name is another program");
        assert!(!ends_with_image(path, r"*.exe\enshrouded.exe"));
        assert!(!ends_with_image(r"c:\a.exe", r"b\a.exe"), "more names than the path has");
        assert!(!ends_with_image(path, "enshrouded.ex"));
    }

    #[test]
    fn without_an_image_the_install_folder_decides() {
        let rule = rule(None, r"D:\Steam\common\Game\");
        let inside = process(r"D:\STEAM\common\game\bin\x64\g.exe", 60, 1);
        assert!(rule.matches(&inside));
        assert!(!rule.matches(&process(r"D:\Steam\common\Game2\g.exe", 60, 1)), "a sibling folder with the same prefix");
        assert!(!rule.matches(&process(r"D:\Steam\common\g.exe", 60, 1)));
    }

    #[test]
    fn the_biggest_old_enough_match_is_picked() {
        let rule = rule(Some("g.exe"), r"C:\g");
        let all = vec![
            process(r"C:\g\g.exe", 30, 100),
            process(r"C:\g\g.exe", 30, 900),
            process(r"C:\g\g.exe", 2, 5000),
            process(r"C:\other\o.exe", 30, 9000),
        ];
        assert_eq!(pick(all.clone(), &rule), Some(all[1].clone()));
        assert_eq!(pick(vec![all[2].clone()], &rule), None, "too young");
    }

    #[test]
    fn this_process_is_listed_with_a_sane_age_and_memory() {
        let me = std::env::current_exe().unwrap();
        let listed = running().unwrap();
        let found = listed.iter().find(|p| p.pid == std::process::id()).expect("our own process");
        assert!(found.path.to_string_lossy().eq_ignore_ascii_case(&me.to_string_lossy()));
        assert!(found.working_set > 0 && found.age < Duration::from_secs(3600), "{found:?}");
    }
}
