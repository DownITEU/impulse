//! A bundle written into the plugins' folder: staged under a name of its own, then renamed into place.

use std::{fs, path::Path};

use anyhow::{Context, Result};

use crate::Bundle;

/// `dir/<name>` holds the bundle's files when this returns. A crash leaves at most `<name>.new`, which the next
/// install of the same bytes removes first. A folder already there is kept only when its files are the bundle's
/// (the name has the digest in it, but the folder may have been damaged since); otherwise it is replaced.
pub fn write(dir: &Path, name: &str, bundle: &Bundle) -> Result<()> {
    let staging = dir.join(format!("{name}.new"));
    let target = dir.join(name);
    if target.exists() && Bundle::read(&target).is_ok_and(|held| held.hashes() == bundle.hashes()) {
        return Ok(());
    }
    if staging.exists() {
        fs::remove_dir_all(&staging).with_context(|| format!("remove {}", staging.display()))?;
    }
    for (file, bytes) in bundle.files() {
        // Names are the manifest's module, `manifest.json` and checked language tags: always inside the staging folder.
        disk::replace(&staging.join(file), bytes)?;
    }
    if target.exists() {
        // A stray regular file named like the folder cannot be removed as a folder.
        let removed = if target.is_dir() { fs::remove_dir_all(&target) } else { fs::remove_file(&target) };
        removed.with_context(|| format!("remove {}", target.display()))?;
    }
    fs::rename(&staging, &target).with_context(|| format!("move into {}", target.display()))
}
