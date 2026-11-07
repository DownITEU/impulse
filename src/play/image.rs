//! A DLL that may enter a game: signed by WeMod, x64, and held open against replacement.

use std::{
    fs::{File, OpenOptions},
    io, mem,
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, GetFinalPathNameByHandleW, VOLUME_NAME_DOS};

use super::trust;
use crate::pe;

/// The only way to name a file the service or the injector will load.
pub struct Image {
    path: PathBuf,
    lock: Option<File>,
}

impl Image {
    pub fn open(path: &Path) -> Result<Self> {
        // Denies writes and deletes while this value lives: the file checked is the file loaded.
        let lock = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(path)
            .with_context(|| format!("cannot open {}", path.display()))?;
        // The opened file's own path: a junction or symlink swapped later cannot redirect a
        // loader that opens it by path, and it is the form the loader records for the module.
        let path = final_path(&lock)?;
        // `LoadLibraryW` appends ".dll" to a name without an extension and would load a sibling.
        let dll = path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("dll"));
        ensure!(dll, "{} is not a .dll", path.display());
        trust::check(&path)?;
        let x64 = pe::header(&path).is_some_and(|header| header.machine == pe::X64);
        ensure!(x64, "{} is not an x64 image", path.display());
        Ok(Self { path, lock: Some(lock) })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// For a failed load whose remote thread may still open the file: the lock then lasts until exit.
    pub fn keep_locked(&self) {
        if let Some(copy) = self.lock.as_ref().and_then(|lock| lock.try_clone().ok()) {
            mem::forget(copy);
        }
    }

    /// A path no file backs, for tests that never load it.
    #[cfg(test)]
    pub fn unchecked(path: &str) -> Self {
        Self { path: path.into(), lock: None }
    }
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
    fn only_wemod_signed_x64_dlls_open() {
        let kernel32 = Path::new(r"C:\Windows\System32\kernel32.dll");
        let err = Image::open(kernel32).err().unwrap();
        assert!(err.to_string().contains("not by WeMod LLC"), "{err}");
        assert!(Image::open(Path::new(r"C:\no\such.dll")).is_err());
    }

    #[test]
    fn a_file_without_the_dll_extension_is_refused_by_its_resolved_path() {
        let err = Image::open(Path::new("Cargo.toml")).err().unwrap();
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
