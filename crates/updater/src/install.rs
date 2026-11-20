//! Getting the new executable in place of the running one: download beside it, swap by renames, start it.

use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{self, Command},
};

use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};

use crate::feed::{Release, secure};

/// The flag a restarted Impulse starts with, followed by the pid of the one it replaces.
const RESTARTED: &str = "--restarted";

/// `exe`'s sibling named like it plus `suffix`.
fn beside(exe: &Path, suffix: &str) -> PathBuf {
    let mut name: OsString = exe.file_name().unwrap_or_default().to_owned();
    name.push(suffix);
    exe.with_file_name(name)
}

/// Downloads the release to `<exe>.new`. A body that is not the digest's is never written.
pub fn download(release: &Release, exe: &Path) -> Result<()> {
    ensure!(secure(&release.url), "the download is not https");
    let response = fetch::get(&release.url, None)?;
    ensure!(response.status == 200, "the download answered {}", response.status);
    verify(&response.body, &release.sha256)?;
    disk::replace(&beside(exe, ".new"), &response.body)
}

fn verify(body: &[u8], sha256: &str) -> Result<()> {
    ensure!(format!("{:x}", Sha256::digest(body)) == sha256, "the download does not match its digest");
    Ok(())
}

/// Puts what `download` fetched where the running `exe` is. Windows renames a running executable but never
/// overwrites it, so it goes to `<exe>.old` first; if the second step fails it comes back, or Impulse would not start.
pub fn swap(exe: &Path) -> Result<()> {
    let (old, new) = (beside(exe, ".old"), beside(exe, ".new"));
    fs::remove_file(&old).ok();
    fs::rename(exe, &old).context("move the running executable aside")?;
    if let Err(e) = fs::rename(&new, exe) {
        if let Err(back) = fs::rename(&old, exe) {
            log::error!("the executable is not back in place: {back}");
        }
        return Err(e).context("put the new executable in place");
    }
    Ok(())
}

/// Removes what a swap left: `<exe>.old` once its process is gone, and a download nothing swapped in.
pub fn cleanup(exe: &Path) {
    for suffix in [".old", ".new"] {
        let leftover = beside(exe, suffix);
        if leftover.exists()
            && let Err(e) = fs::remove_file(&leftover)
        {
            log::warn!("{} not removed: {e}", leftover.display());
        }
    }
}

/// Starts `exe`, which waits for this process to end before it claims the one instance.
pub fn restart(exe: &Path) -> Result<()> {
    let child = Command::new(exe).arg(RESTARTED).arg(process::id().to_string()).spawn().context("start the new executable")?;
    // Not waited for: it outlives this process.
    drop(child);
    Ok(())
}

/// The pid of the Impulse this one replaces, when it was started by `restart`.
pub fn restarted() -> Option<u32> {
    after_flag(env::args().skip(1))
}

fn after_flag(mut args: impl Iterator<Item = String>) -> Option<u32> {
    args.find(|arg| arg == RESTARTED)?;
    args.next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("impulse-updater-{name}-{}", process::id()));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_body_is_taken_only_for_its_digest() {
        let digest = format!("{:x}", Sha256::digest(b"exe"));
        assert!(verify(b"exe", &digest).is_ok());
        assert!(verify(b"exe!", &digest).is_err());
    }

    #[test]
    fn the_new_executable_takes_the_old_ones_place_and_the_old_one_stays_beside_it() {
        let dir = dir("swap");
        let (exe, new) = (dir.join("Impulse.exe"), dir.join("Impulse.exe.new"));
        fs::write(&exe, "old").unwrap();
        fs::write(&new, "new").unwrap();
        swap(&exe).unwrap();
        assert_eq!(fs::read_to_string(&exe).unwrap(), "new");
        assert_eq!(fs::read_to_string(dir.join("Impulse.exe.old")).unwrap(), "old");
        assert!(!new.exists());
        cleanup(&exe);
        assert!(!dir.join("Impulse.exe.old").exists());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_swap_without_the_download_leaves_the_executable_where_it_was() {
        let dir = dir("missing");
        let exe = dir.join("Impulse.exe");
        fs::write(&exe, "old").unwrap();
        assert!(swap(&exe).is_err());
        assert_eq!(fs::read_to_string(&exe).unwrap(), "old");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_restart_names_the_pid_it_waits_for() {
        let args = |list: &[&str]| after_flag(list.iter().map(|arg| (*arg).to_owned()));
        assert_eq!(args(&["--restarted", "4242"]), Some(4242));
        assert_eq!(args(&["x", "--restarted", "7"]), Some(7));
        assert_eq!((args(&[]), args(&["--restarted"]), args(&["--restarted", "soon"])), (None, None, None));
    }
}
