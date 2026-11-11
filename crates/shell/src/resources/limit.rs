//! The media limit: kept in `resources.json`, held by dropping the least recently used files.

use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};

const FILE: &str = "resources.json";
pub const DEFAULT: Option<u64> = Some(2 << 30);

#[derive(Serialize, Deserialize)]
struct Stored {
    /// Bytes; `None`: no limit.
    limit: Option<u64>,
}

/// Blocking; a missing or damaged file means the default.
pub fn load(data_dir: &Path) -> Option<u64> {
    crate::store::load::<Stored>(&data_dir.join(FILE)).map_or(DEFAULT, |stored| stored.limit)
}

/// Blocking.
pub fn store(data_dir: &Path, limit: Option<u64>) {
    crate::store::save(&data_dir.join(FILE), &Stored { limit });
}

/// Deletes the least recently used files in `dirs` until they hold at most `limit` bytes (blocking).
/// A file in use cannot be deleted on Windows and stays.
pub fn sweep(dirs: &[PathBuf], limit: u64) {
    let mut files: Vec<(SystemTime, u64, PathBuf)> = dirs
        .iter()
        .flat_map(|dir| fs::read_dir(dir).into_iter().flatten().flatten())
        .filter_map(|entry| {
            let meta = entry.metadata().ok().filter(|meta| meta.is_file())?;
            Some((meta.modified().ok()?, meta.len(), entry.path()))
        })
        .collect();
    let before: u64 = files.iter().map(|(_, len, _)| len).sum();
    if before <= limit {
        return;
    }
    files.sort();
    let mut total = before;
    for (_, len, path) in files {
        if total <= limit {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
    log::info!("media limit: {} MB freed", (before - total) >> 20);
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn the_oldest_files_go_first_and_the_limit_survives_a_restart() {
        let dir = std::env::temp_dir().join(format!("impulse-limit-{}", std::process::id()));
        let (pictures, videos) = (dir.join("art"), dir.join("videos"));
        let start = SystemTime::now() - Duration::from_secs(1000);
        for (index, (folder, name)) in [(&videos, "old"), (&pictures, "mid"), (&videos, "new")].into_iter().enumerate() {
            fs::create_dir_all(folder).unwrap();
            fs::write(folder.join(name), [0; 100]).unwrap();
            let file = fs::File::options().write(true).open(folder.join(name)).unwrap();
            file.set_modified(start + Duration::from_secs(index as u64 * 100)).unwrap();
        }
        sweep(&[pictures.clone(), videos.clone()], 150);
        assert!(!videos.join("old").exists() && !pictures.join("mid").exists() && videos.join("new").exists());

        assert_eq!(load(&dir), DEFAULT);
        store(&dir, None);
        assert_eq!(load(&dir), None);
        let _ = fs::remove_dir_all(&dir);
    }
}
