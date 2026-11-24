//! Finding the game's process by its `Rule`, taking the biggest if several match.

use std::{
    io, mem,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use anyhow::{Result, anyhow, bail, ensure};
use blocking::unblock;
use model::{Failure, FailureKind, Process, Rule};
use windows_sys::Win32::{
    Foundation::FILETIME,
    System::{
        ProcessStatus::{EnumProcesses, GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
        SystemInformation::GetSystemTimeAsFileTime,
        Threading::{GetProcessTimes, IsWow64Process, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW},
    },
};

use crate::handle::Handle;

const POLL: Duration = Duration::from_secs(1);

/// What ranks a process; only its identity leaves this module.
#[derive(Debug)]
struct Candidate {
    process: Process,
    age: Duration,
    working_set: usize,
}

/// The matching process with the most memory, once it is old enough (blocking).
fn find(rule: &Rule) -> Result<Option<Process>> {
    Ok(pick(running()?, rule))
}

// Ranked before the age, as Wand does: a small old helper in the game's folder is not the game still starting up.
fn pick(candidates: Vec<Candidate>, rule: &Rule) -> Option<Process> {
    biggest(candidates, rule).filter(|candidate| candidate.age >= rule.minimum_age).map(|candidate| candidate.process)
}

fn biggest(candidates: Vec<Candidate>, rule: &Rule) -> Option<Candidate> {
    candidates.into_iter().filter(|candidate| rule.matches(&candidate.process.path)).max_by_key(|candidate| candidate.working_set)
}

/// What `find` takes from the processes running now, waited for while it is younger than `minimum_age`; `None` once
/// nothing matches. Dropping the future stops it within one poll.
pub async fn settled(rule: &Rule) -> Result<Option<Process>> {
    loop {
        let found = {
            let rule = rule.clone();
            unblock(move || running().map(|candidates| biggest(candidates, &rule))).await?
        };
        match found {
            Some(candidate) if candidate.age < rule.minimum_age => unblock(|| thread::sleep(POLL)).await,
            found => return Ok(found.map(|candidate| candidate.process)),
        }
    }
}

/// Polls until `find` finds a process. Dropping the future stops it within one poll.
pub async fn wait(rule: &Rule, timeout: Duration) -> Result<Process> {
    let mut waited = Duration::ZERO;
    loop {
        let found = {
            let rule = rule.clone();
            unblock(move || find(&rule)).await?
        };
        if let Some(process) = found {
            return Ok(process);
        }
        if waited >= timeout {
            return Err(anyhow!("the game did not start in {} s", timeout.as_secs()).context(Failure::new(FailureKind::GameNotFound)));
        }
        // A blocking-pool sleep: the caller's executor may have no timer.
        unblock(|| thread::sleep(POLL)).await;
        waited += POLL;
    }
}

/// Every process we may look at (blocking). Protected ones are skipped.
fn running() -> Result<Vec<Candidate>> {
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
    Ok(pids[..count].iter().filter_map(|&pid| candidate(pid)).collect())
}

/// The process `pid` is now, if it can be looked at (blocking).
pub fn of(pid: u32) -> Option<Process> {
    candidate(pid).map(|candidate| candidate.process)
}

/// Whether `target` is a 64-bit process, asked of the process: its exe may be unreadable (a GDK game's).
pub fn x64(target: &Process) -> Result<bool> {
    // SAFETY: plain Win32 call; the handle is owned by the guard.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, target.pid) };
    ensure!(!handle.is_null(), "cannot open process {}: {}", target.pid, io::Error::last_os_error());
    let process = Handle(handle);
    verify(&process, target)?;
    Ok(!wow64(&process, target.pid)?)
}

/// A 32-bit process on 64-bit Windows.
pub(crate) fn wow64(process: &Handle, pid: u32) -> Result<bool> {
    let mut wow = 0;
    // SAFETY: `process` is a live handle and `wow` outlives the call.
    ensure!(unsafe { IsWow64Process(process.0, &mut wow) } != 0, "cannot query process {pid}: {}", io::Error::last_os_error());
    Ok(wow != 0)
}

fn candidate(pid: u32) -> Option<Candidate> {
    // SAFETY: plain Win32 call; the handle is owned by the guard.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let process = Handle(handle);
    let (path, created) = identity(&process)?;
    let age = Duration::from_nanos(now().saturating_sub(created) * 100);

    // SAFETY: zeroed counters of the declared size.
    let mut memory: PROCESS_MEMORY_COUNTERS = unsafe { mem::zeroed() };
    memory.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    // SAFETY: the guard holds a live process handle and `memory` is a valid output of size `cb`.
    let known = unsafe { GetProcessMemoryInfo(process.0, &mut memory, memory.cb) } != 0;
    let working_set = if known { memory.WorkingSetSize } else { 0 };
    Some(Candidate { process: Process { pid, path, created }, age, working_set })
}

/// Fails unless `handle` is the very process `expected` describes. A handle keeps its process
/// from being replaced, so this closes the gap between finding a process by pid and opening it
/// for injection: the pid may have been reused in between.
pub(crate) fn verify(handle: &Handle, expected: &Process) -> Result<()> {
    match identity(handle) {
        Some((path, created)) if created == expected.created && same_path(&path, &expected.path) => Ok(()),
        _ => bail!("process {} is not the one that was found: it exited and its id was reused", expected.pid),
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy())
}

/// Image path and creation time of the process a handle names.
fn identity(process: &Handle) -> Option<(PathBuf, u64)> {
    let mut name = vec![0u16; 32_768];
    let mut len = name.len() as u32;
    // SAFETY: `name` is valid for `len` characters.
    if unsafe { QueryFullProcessImageNameW(process.0, 0, name.as_mut_ptr(), &mut len) } == 0 {
        return None;
    }
    let path = PathBuf::from(String::from_utf16_lossy(&name[..len as usize]));

    let mut times = [FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 }; 4];
    let [created, exited, kernel, user] = &mut times;
    // SAFETY: the guard holds a live process handle and the four outputs are distinct, valid FILETIMEs.
    if unsafe { GetProcessTimes(process.0, created, exited, kernel, user) } == 0 {
        return None;
    }
    Some((path, ticks(created)))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(path: &str, secs: u64, working_set: usize) -> Candidate {
        let process = Process { pid: 1, path: PathBuf::from(path), created: 7 };
        Candidate { process, age: Duration::from_secs(secs), working_set }
    }

    #[test]
    fn the_biggest_match_is_picked_once_old_enough() {
        let rule = Rule { image: None, location: PathBuf::from(r"C:\g"), minimum_age: Duration::from_secs(8) };
        let all = |game_age| {
            vec![candidate(r"C:\g\helper.exe", 30, 100), candidate(r"C:\g\g.exe", game_age, 5000), candidate(r"C:\other\o.exe", 30, 9000)]
        };
        assert_eq!(pick(all(30), &rule), Some(all(30).swap_remove(1).process));
        assert_eq!(pick(all(2), &rule), None, "the game is still starting up: not its old helper");
    }

    #[test]
    fn a_handle_is_verified_against_what_was_found() {
        let found = of(std::process::id()).unwrap();
        // SAFETY: plain Win32 call; the handle is owned by the guard.
        let ours = Handle(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, found.pid) });
        assert!(verify(&ours, &found).is_ok());
        let reused = Process { created: found.created + 1, ..found.clone() };
        assert!(verify(&ours, &reused).unwrap_err().to_string().contains("reused"));
        let other = Process { path: PathBuf::from(r"C:\elsewhere\other.exe"), ..found };
        assert!(verify(&ours, &other).is_err());
    }

    #[test]
    fn this_process_is_told_64_bit() {
        assert!(x64(&of(std::process::id()).unwrap()).unwrap());
    }

    #[test]
    fn this_process_is_listed_with_a_sane_age_and_memory() {
        let me = std::env::current_exe().unwrap();
        let listed = running().unwrap();
        let found = listed.iter().find(|c| c.process.pid == std::process::id()).expect("our own process");
        assert!(found.process.path.to_string_lossy().eq_ignore_ascii_case(&me.to_string_lossy()));
        assert!(found.working_set > 0 && found.age < Duration::from_secs(3600), "{found:?}");
    }
}
