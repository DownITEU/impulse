//! A DLL that may enter a game: signed by an expected publisher, x64 or x86, and held open against replacement.

use std::{
    fs::{File, OpenOptions},
    io, mem,
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, GetFinalPathNameByHandleW, VOLUME_NAME_DOS};

use crate::trust;

/// The only way to name a file the injector or a loaded service will load.
pub struct Image {
    path: PathBuf,
    x64: bool,
    lock: Option<File>,
}

/// A program that runs on its own, never in a game, signed and held open against replacement like an `Image`.
pub struct Program {
    path: PathBuf,
    _lock: File,
}

impl Program {
    /// `publishers`: the signers' display names the file may carry.
    pub fn open(path: &Path, publishers: &[&str]) -> Result<Self> {
        let (lock, path) = locked(path)?;
        // A start appends ".exe" to a name without an extension and would run a sibling.
        ensure!(has_extension(&path, "exe"), "{} is not an .exe", path.display());
        trust::check(&path, publishers)?;
        Ok(Self { path, _lock: lock })
    }

    /// The file checked, every link resolved: the one to start.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Image {
    /// `publishers`: the signers' display names the file may carry.
    pub fn open(path: &Path, publishers: &[&str]) -> Result<Self> {
        let (lock, path) = locked(path)?;
        // `LoadLibraryW` appends ".dll" to a name without an extension and would load a sibling.
        ensure!(has_extension(&path, "dll"), "{} is not a .dll", path.display());
        trust::check(&path, publishers)?;
        let x64 = match pe::header(&path).map(|header| header.machine) {
            Some(pe::X64) => true,
            Some(pe::X86) => false,
            _ => bail!("{} is neither an x64 nor an x86 image", path.display()),
        };
        Ok(Self { path, x64, lock: Some(lock) })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// x64, else x86: a process takes only an image of its own architecture.
    pub fn x64(&self) -> bool {
        self.x64
    }

    /// For a failed load whose remote thread may still open the file: the lock then lasts until exit.
    pub(crate) fn keep_locked(&self) {
        if let Some(copy) = self.lock.as_ref().and_then(|lock| lock.try_clone().ok()) {
            mem::forget(copy);
        }
    }

    /// An x64 path no file backs, for tests that never load it. Never in a normal build.
    #[cfg(any(test, feature = "test-support"))]
    pub fn unchecked(path: &str) -> Self {
        Self { path: path.into(), x64: true, lock: None }
    }

    #[cfg(test)]
    pub(crate) fn unchecked_x86(path: &str) -> Self {
        Self { path: path.into(), x64: false, lock: None }
    }
}

/// The file opened without write or delete sharing, so the file checked is the file used, and its own path: a junction
/// or symlink swapped later cannot redirect what opens it by that path (the loader records a module so too).
fn locked(path: &Path) -> Result<(File, PathBuf)> {
    let lock =
        OpenOptions::new().read(true).share_mode(FILE_SHARE_READ).open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let path = final_path(&lock)?;
    Ok((lock, path))
}

fn has_extension(path: &Path, wanted: &str) -> bool {
    path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case(wanted))
}

/// A local drive path with every link resolved.
fn final_path(file: &File) -> Result<PathBuf> {
    let mut buf = vec![0u16; 512];
    loop {
        // SAFETY: the handle is live and `buf` is valid for the length passed.
        let len = unsafe { GetFinalPathNameByHandleW(file.as_raw_handle(), buf.as_mut_ptr(), buf.len() as u32, VOLUME_NAME_DOS) } as usize;
        ensure!(len != 0, "cannot resolve the file's path: {}", io::Error::last_os_error());
        if len < buf.len() {
            buf.truncate(len);
            break;
        }
        // Too small: `len` is the size needed, with the terminator.
        buf.resize(len + 1, 0);
    }
    let text = String::from_utf16(&buf).context("the file's path is not valid Unicode")?;
    match text.strip_prefix(r"\\?\") {
        Some(local) if !local.starts_with(r"UNC\") => Ok(local.into()),
        _ => bail!("{text} is not on a local drive"),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, process::Command};

    use super::*;

    #[test]
    fn only_a_dll_signed_by_an_expected_publisher_opens() {
        let kernel32 = Path::new(r"C:\Windows\System32\kernel32.dll");
        assert!(Image::open(kernel32, &["Someone Else", "Microsoft Windows"]).is_ok());
        let err = Image::open(kernel32, &["Someone Else", "Another Publisher"]).err().unwrap();
        assert!(err.to_string().contains("not by Someone Else or Another Publisher"), "{err}");
        assert!(Image::open(Path::new(r"C:\no\such.dll"), &["Microsoft Windows"]).is_err());
    }

    /// System executables are catalog-signed, not embedded: a signed one opens in the trainer host test (`wand`).
    #[test]
    fn a_program_is_a_signed_exe() {
        let dll = Program::open(Path::new(r"C:\Windows\System32\kernel32.dll"), &["Microsoft Windows"]).err().unwrap();
        assert!(dll.to_string().contains("is not an .exe"), "{dll}");
        let unsigned = Program::open(&std::env::current_exe().unwrap(), &["Microsoft Windows"]).err().unwrap();
        assert!(unsigned.to_string().contains("no valid signature"), "{unsigned}");
    }

    #[test]
    fn a_file_without_the_dll_extension_is_refused_by_its_resolved_path() {
        let err = Image::open(Path::new("Cargo.toml"), &["Microsoft Windows"]).err().unwrap();
        let text = err.to_string();
        assert!(text.contains("is not a .dll") && text.contains(":\\"), "{text}");
    }

    #[test]
    fn links_resolve_to_the_file_that_was_opened() {
        let root = std::env::temp_dir().join(format!("impulse-image-{}", std::process::id()));
        let (real, link) = (root.join("real"), root.join("link"));
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join("a.dll"), b"x").unwrap();
        let junction = Command::new("cmd").args(["/C", "mklink", "/J"]).arg(&link).arg(&real).output().unwrap();
        assert!(junction.status.success(), "mklink failed");

        let through_link = File::open(link.join("a.dll")).unwrap();
        let resolved = final_path(&through_link).unwrap();
        let expected = fs::canonicalize(real.join("a.dll")).unwrap();
        assert_eq!(resolved.to_string_lossy(), expected.to_string_lossy().trim_start_matches(r"\\?\"));
        fs::remove_dir_all(&root).unwrap();
    }
}
