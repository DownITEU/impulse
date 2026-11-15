//! The trainer DLL on disk: fetched once from the public CDN and named by its hash.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use model::{Failure, FailureKind};
use sha2::{Digest, Sha256};

// Not under `trainers`: that folder is the metadata's, and dropping it must not reach here.
const DIR: &str = "trainer-binaries";

/// Drops every downloaded DLL; the next run fetches again. A DLL loaded in a game stays.
pub fn size(data_dir: &Path) -> u64 {
    disk::size(&data_dir.join(DIR))
}

pub fn forget(data_dir: &Path) {
    let _ = fs::remove_dir_all(data_dir.join(DIR));
}

/// The DLL whose SHA-256 is `hash`, downloaded from `url` when it is not on disk (blocking).
/// Both come from a remote response: the hash becomes a file name and the URL is fetched.
pub fn ensure(data_dir: &Path, url: &str, hash: &str) -> Result<PathBuf> {
    ensure!(url.starts_with("https://"), "the trainer binary is not served over https");
    ensure!(hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()), "the trainer binary hash is malformed");
    let hash = hash.to_ascii_lowercase();
    let path = data_dir.join(DIR).join(format!("{hash}.dll"));
    // Hashed again on every use: cheap next to the injection, and a damaged copy is fetched anew.
    if hash_of(&path).is_ok_and(|found| found == hash) {
        return Ok(path);
    }
    let response = fetch::get(url, None).context(Failure::new(FailureKind::Offline))?;
    ensure!(response.status == 200, "GET {url}: status {}", response.status);
    ensure!(sha256(&response.body) == hash, "the downloaded trainer does not match its hash");
    disk::replace(&path, &response.body)?;
    Ok(path)
}

/// Lowercase hex SHA-256 of a file.
pub fn hash_of(path: &Path) -> io::Result<String> {
    fs::read(path).map(|bytes| sha256(&bytes))
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    fn dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("impulse-binary-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_hash_that_is_no_hash_never_reaches_a_path() {
        let dir = dir("hash");
        for hash in ["", "../../evil", &"g".repeat(64), &"a".repeat(63)] {
            let err = ensure(&dir, "https://x.invalid/t", hash).unwrap_err();
            assert!(err.to_string().contains("malformed"), "{hash}: {err}");
        }
        assert!(ensure(&dir, "http://x.invalid/t", &"a".repeat(64)).unwrap_err().to_string().contains("https"));
        assert!(!dir.exists());
    }

    #[test]
    fn a_good_copy_is_used_and_a_damaged_one_is_not() {
        let dir = dir("copy");
        let content = b"pretend dll";
        let hash = sha256(content).to_uppercase();
        let path = dir.join(DIR).join(format!("{}.dll", hash.to_lowercase()));
        disk::replace(&path, content).unwrap();
        // The URL cannot resolve: a download would fail.
        assert_eq!(ensure(&dir, "https://x.invalid/t", &hash).unwrap(), path);

        assert_eq!(hash_of(&path).unwrap(), hash.to_lowercase());
        fs::write(&path, b"tampered").unwrap();
        assert!(ensure(&dir, "https://x.invalid/t", &hash).is_err());
        assert_ne!(hash_of(&path).unwrap(), hash.to_lowercase());
        fs::remove_dir_all(&dir).unwrap();
    }
}
