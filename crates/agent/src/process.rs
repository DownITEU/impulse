//! Finding the game's process by its `Rule`, taking the one its `Pick` names if several match.

use std::{
    io, mem,
    path::{Path, PathBuf},
    ptr, thread,
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail, ensure};
use blocking::unblock;
use model::{Failure, FailureKind, Pick, Process, Rule, Target};
use windows_sys::{
    Wdk::System::Threading::{NtQueryInformationProcess, ProcessCommandLineInformation},
    Win32::{
        Foundation::{ERROR_ACCESS_DENIED, FILETIME, UNICODE_STRING},
        System::{
            ProcessStatus::{EnumProcesses, GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            SystemInformation::GetSystemTimeAsFileTime,
            Threading::{GetProcessTimes, IsWow64Process, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW},
        },
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

/// The matching process the rule's `Pick` names, waited for while it is younger than `minimum_age`; `None` once
/// nothing matches. Dropping the future stops it within one poll.
pub async fn settled(rule: &Rule) -> Result<Option<Process>> {
    poll(rule, None).await
}

/// Polls until that process is old enough. Dropping the future stops it within one poll.
pub async fn wait(rule: &Rule, timeout: Duration) -> Result<Process> {
    let found = poll(rule, Some(timeout)).await?;
    found.ok_or_else(|| anyhow!("the game did not start in {} s", timeout.as_secs()).context(Failure::new(FailureKind::GameNotFound)))
}

/// `None`: past `within`; without one, as soon as nothing matches.
async fn poll(rule: &Rule, within: Option<Duration>) -> Result<Option<Process>> {
    let mut waited = Duration::ZERO;
    loop {
        let found = {
            let rule = rule.clone();
            unblock(move || running().map(|candidates| pick(candidates, &rule, |process| takes(process, &rule.target)))).await?
        };
        match found {
            Some(candidate) if candidate.age >= rule.target.minimum_age => return Ok(Some(candidate.process)),
            None if within.is_none() => return Ok(None),
            _ if within.is_some_and(|within| waited >= within) => return Ok(None),
            _ => {}
        }
        // A blocking-pool sleep: the caller's executor may have no timer.
        unblock(|| thread::sleep(POLL)).await;
        waited += POLL;
    }
}

// Ranked before the age is checked: a small old helper in the game's folder is not the game starting up.
// `takes` is asked only of a process whose path matches; ties keep the first.
fn pick(candidates: Vec<Candidate>, rule: &Rule, takes: impl Fn(&Process) -> bool) -> Option<Candidate> {
    // By creation time, not age: each age is read at its own moment of the scan.
    let better = |best: &Candidate, next: &Candidate| match rule.target.pick {
        Pick::Biggest => next.working_set > best.working_set,
        Pick::Oldest => next.process.created < best.process.created,
        Pick::Newest => next.process.created > best.process.created,
    };
    let matching = candidates.into_iter().filter(|candidate| rule.matches(&candidate.process.path) && takes(&candidate.process));
    matching.reduce(|best, next| if better(&best, &next) { next } else { best })
}

/// Whether `process` was started with `target`'s arguments; one whose command line cannot be read was not (blocking).
fn takes(process: &Process, target: &Target) -> bool {
    target.arguments.is_none() || command_line(process).is_ok_and(|line| target.takes(&line))
}

/// The command line `target` was started with, through a verified handle (blocking). It never leaves this crate.
fn command_line(target: &Process) -> Result<String> {
    let process = open(target, 0)?;
    let mut size = 0;
    // SAFETY: a size query: no buffer, `size` outlives the call.
    unsafe { NtQueryInformationProcess(process.0, ProcessCommandLineInformation, ptr::null_mut(), 0, &mut size) };
    ensure!(size as usize >= size_of::<UNICODE_STRING>(), "cannot size the command line of process {}", target.pid);
    // In u64s: the UNICODE_STRING at its head is pointer-aligned.
    let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
    // SAFETY: `buffer` holds at least `size` bytes, aligned for what the call writes.
    let status =
        unsafe { NtQueryInformationProcess(process.0, ProcessCommandLineInformation, buffer.as_mut_ptr().cast(), size, &mut size) };
    ensure!(status >= 0, "cannot read the command line of process {}: NTSTATUS {:#x}", target.pid, status as u32);
    // SAFETY: the call wrote a UNICODE_STRING at the head of `buffer`, which is aligned for it.
    let string = unsafe { buffer.as_ptr().cast::<UNICODE_STRING>().read() };
    // Its text follows it in `buffer`: read by offset, never through the pointer.
    let at = (string.Buffer as usize).checked_sub(buffer.as_ptr() as usize);
    let bytes: Vec<u8> = buffer.iter().flat_map(|word| word.to_ne_bytes()).collect();
    let text = at.and_then(|at| bytes.get(at..at + usize::from(string.Length))).context("odd command line")?;
    Ok(String::from_utf16_lossy(&text.as_chunks().0.iter().map(|pair| u16::from_ne_bytes(*pair)).collect::<Vec<_>>()))
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
    Ok(!wow64(&open(target, 0)?, target.pid)?)
}

/// `target` opened with `access` and enough to check it is that process.
pub(crate) fn open(target: &Process, access: u32) -> Result<Handle> {
    let process = open_pid(target.pid, PROCESS_QUERY_LIMITED_INFORMATION | access)?;
    verify(&process, target)?;
    Ok(process)
}

/// A denial is `Protected`: run as administrator, or kept by its protection. Not yet verified.
pub(crate) fn open_pid(pid: u32, access: u32) -> Result<Handle> {
    // SAFETY: plain Win32 call; the handle is owned by the guard.
    let handle = unsafe { OpenProcess(access, 0, pid) };
    if handle.is_null() {
        let error = io::Error::last_os_error();
        let denied = error.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32);
        let error = anyhow!("cannot open process {pid}: {error}");
        return Err(if denied { error.context(Failure::new(FailureKind::Protected)) } else { error });
    }
    Ok(Handle(handle))
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

    /// `secs` old: created that much before a common moment.
    fn candidate(path: &str, secs: u64, working_set: usize) -> Candidate {
        let process = Process { pid: 1, path: PathBuf::from(path), created: 1000 - secs };
        Candidate { process, age: Duration::from_secs(secs), working_set }
    }

    fn rule(pick: Pick) -> Rule {
        let target = Target { image: None, minimum_age: Duration::from_secs(8), pick, arguments: None };
        Rule { target, location: PathBuf::from(r"C:\g"), alternates: Vec::new() }
    }

    #[test]
    fn the_biggest_match_is_the_game_even_while_it_starts_up() {
        let all = vec![candidate(r"C:\g\helper.exe", 30, 100), candidate(r"C:\g\g.exe", 2, 5000), candidate(r"C:\other\o.exe", 30, 9000)];
        assert_eq!(pick(all, &rule(Pick::Biggest), |_| true).unwrap().process.path, PathBuf::from(r"C:\g\g.exe"), "not its old helper");
    }

    #[test]
    fn the_trainer_may_pick_by_age_and_by_arguments() {
        let all = || vec![candidate(r"C:\g\a.exe", 30, 100), candidate(r"C:\g\b.exe", 2, 100), candidate(r"C:\g\c.exe", 30, 100)];
        let picked = |by: Pick, takes: fn(&Process) -> bool| pick(all(), &rule(by), takes).unwrap().process.path.display().to_string();
        assert_eq!(picked(Pick::Oldest, |_| true), r"C:\g\a.exe", "ties keep the first");
        assert_eq!(picked(Pick::Newest, |_| true), r"C:\g\b.exe");
        assert_eq!(picked(Pick::Biggest, |_| true), r"C:\g\a.exe", "ties keep the first");
        assert_eq!(picked(Pick::Oldest, |process| process.path.ends_with("c.exe")), r"C:\g\c.exe", "only one takes the arguments");
        let other = vec![candidate(r"C:\other\o.exe", 30, 100)];
        assert!(pick(other, &rule(Pick::Biggest), |_| panic!("asked of a path that does not match")).is_none());
    }

    #[test]
    fn a_command_line_is_read_for_its_arguments() {
        let me = of(std::process::id()).unwrap();
        let line = command_line(&me).unwrap();
        assert!(line.to_lowercase().contains(&me.path.file_name().unwrap().to_string_lossy().to_lowercase()), "{line}");
        let target = Target { arguments: Some("--no-such-argument".into()), ..rule(Pick::Biggest).target };
        assert!(!takes(&me, &target));
        let reused = Process { created: me.created + 1, ..me };
        assert!(command_line(&reused).is_err(), "never another process's");
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
