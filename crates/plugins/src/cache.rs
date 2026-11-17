//! A plugin's answers kept on disk (M15), as the guest gave them: imported again when read, asked again when stale.

use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

use crate::settings::Values;

// The disk one plugin's answers may take: the least recently written go first.
const MOST: u64 = 64 << 20;

/// What an answer was asked with: another stamp makes it stale.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Stamp {
    pub api: u32,
    pub version: String,
    /// The locale it was served in.
    pub locale: String,
    pub settings: Values,
    pub revision: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Kept<T> {
    stamp: Stamp,
    /// Seconds since 1970.
    at: u64,
    answer: T,
}

pub struct Cache {
    dir: PathBuf,
    /// How long an answer without a revision stays fresh.
    refresh: Duration,
}

impl Cache {
    pub fn new(dir: PathBuf, refresh: Duration) -> Self {
        Self { dir, refresh }
    }

    /// The answer kept for `key` in `wanted`'s wire, and whether it is fresh for `wanted` (blocking).
    pub fn read<T: DeserializeOwned>(&self, key: &str, wanted: &Stamp) -> Option<(T, bool)> {
        let kept: Kept<T> = serde_json::from_slice(&crate::read(&self.file(key), MOST).ok()?).ok()?;
        if kept.stamp.api != wanted.api {
            return None;
        }
        let young = Duration::from_secs(now().saturating_sub(kept.at)) < self.refresh;
        let fresh = kept.stamp == *wanted && (wanted.revision.is_some() || young);
        Some((kept.answer, fresh))
    }

    /// An answer as it is kept: encoded before the import takes it apart, kept with `put` once it passed.
    pub fn encode<T: Serialize>(stamp: Stamp, answer: &T) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(&Kept { stamp, at: now(), answer })
    }

    /// Blocking; a failed write is logged and the answer is asked again next time.
    pub fn put(&self, key: &str, kept: &[u8]) {
        if let Err(e) = disk::replace(&self.file(key), kept) {
            log::warn!("plugin answer not kept: {e:#}");
        }
        self.bound();
    }

    fn bound(&self) {
        let mut files: Vec<_> = fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            // Kept answers only: a `disk::replace` in flight is its writer's.
            .filter(|entry| entry.path().extension().is_some_and(|extension| extension == "json"))
            .filter_map(|entry| {
                let meta = entry.metadata().ok()?;
                Some((meta.modified().ok()?, meta.len(), entry.path()))
            })
            .collect();
        let mut total: u64 = files.iter().map(|(_, len, _)| len).sum();
        files.sort();
        for (_, len, file) in files {
            if total <= MOST {
                break;
            }
            if fs::remove_file(&file).is_ok() {
                total -= len;
            }
        }
    }

    // A key is the guest's id, of any length or letters: hashed into a file name.
    fn file(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{:x}.json", Sha256::digest(key)))
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_is_stale_for_another_stamp_and_once_old_without_a_revision_and_hidden_for_another_wire() {
        let dir = std::env::temp_dir().join(format!("impulse-plugin-cache-{}", std::process::id()));
        let stamp = Stamp { api: 1, version: "1.0".into(), locale: "fr".into(), settings: Values::new(), revision: Some("r1".into()) };
        let cache = Cache::new(dir.clone(), Duration::from_secs(3600));
        cache.put("map:a", &Cache::encode(stamp.clone(), &7).unwrap());
        assert_eq!(cache.read::<u32>("map:a", &stamp), Some((7, true)));
        let stale = [
            Stamp { revision: Some("r2".into()), ..stamp.clone() },
            Stamp { locale: "en".into(), ..stamp.clone() },
            Stamp { version: "1.1".into(), ..stamp.clone() },
            Stamp { settings: Values::from_iter([("level".to_owned(), 3.into())]), ..stamp.clone() },
            Stamp { revision: None, ..stamp.clone() },
        ];
        for wanted in stale {
            assert_eq!(cache.read::<u32>("map:a", &wanted), Some((7, false)), "{wanted:?}");
        }
        assert_eq!(cache.read::<u32>("map:a", &Stamp { api: 2, ..stamp.clone() }), None, "another wire is not shown");
        let unrevised = Stamp { revision: None, ..stamp };
        cache.put("map:u", &Cache::encode(unrevised.clone(), &8).unwrap());
        assert_eq!(cache.read::<u32>("map:u", &unrevised), Some((8, true)), "young enough without a revision");
        assert_eq!(Cache::new(dir.clone(), Duration::ZERO).read::<u32>("map:u", &unrevised), Some((8, false)));
        assert_eq!(cache.read::<u32>("map:b", &unrevised), None);
        let _ = fs::remove_dir_all(dir);
    }
}
