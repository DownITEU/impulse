use std::{
    env, fmt, fs,
    path::Path,
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use rusty_leveldb::{DB, Options};
use serde_json::Value;

// Chromium layout: `_<origin>\0` + script key; both keys and values carry an encoding byte.
const KEY: &[u8] = b"_file://\x00\x01infinity:globalStore";
// Wand 12.x keeps its Electron profile in `wemod`; `Wand` is a stale profile from an older build.
const PROFILES: [&str; 2] = ["wemod", "Wand"];

pub struct WandToken {
    access_token: String,
}

impl WandToken {
    pub(crate) fn expose(&self) -> &str {
        &self.access_token
    }
}

impl fmt::Debug for WandToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WandToken(<redacted>)")
    }
}

/// Reads Wand's access token from a copy of its localStorage leveldb (blocking).
pub fn read() -> Result<WandToken> {
    let root = env::var_os("APPDATA").context("APPDATA is not set")?;
    let mut found = Vec::new();
    let mut failed = None;
    for profile in PROFILES {
        match read_profile(
            &Path::new(&root)
                .join(profile)
                .join(r"Local Storage\leveldb"),
        ) {
            Ok(Some(candidate)) => found.push(candidate),
            Ok(None) => {}
            Err(e) => failed = Some(e),
        }
    }
    if found.is_empty()
        && let Some(e) = failed
    {
        return Err(e);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    newest(found, now)
}

fn newest(found: Vec<(f64, String)>, now: f64) -> Result<WandToken> {
    let (expires_at, access_token) = found
        .into_iter()
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .context("Wand session not found: install Wand and sign in")?;
    if expires_at <= now {
        bail!("Wand session expired: open Wand and sign in");
    }
    Ok(WandToken { access_token })
}

fn read_profile(src: &Path) -> Result<Option<(f64, String)>> {
    if !src.is_dir() {
        return Ok(None);
    }
    let dst = env::temp_dir().join(format!("impulse-wand-{}", process::id()));
    let _ = fs::remove_dir_all(&dst);
    let result = copy_dir(src, &dst).and_then(|()| read_db(&dst));
    let _ = fs::remove_dir_all(&dst);
    result
}

// The live dir is never opened: Wand holds LOCK, and opening would write to it.
fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).context("create temp dir")?;
    for entry in fs::read_dir(src).context("list Wand store")? {
        let name = entry.context("list Wand store")?.file_name();
        // Files can vanish mid-copy (leveldb compaction) or be locked; the probe skipped them too.
        if name != "LOCK" {
            let _ = fs::copy(src.join(&name), dst.join(&name));
        }
    }
    Ok(())
}

fn read_db(dir: &Path) -> Result<Option<(f64, String)>> {
    let opt = Options {
        create_if_missing: false,
        ..Options::default()
    };
    let mut db = DB::open(dir, opt).context("open Wand store copy")?;
    db.get(KEY)
        .map(|raw| parse(&raw))
        .transpose()
        .map(Option::flatten)
}

fn parse(raw: &[u8]) -> Result<Option<(f64, String)>> {
    let store: Value =
        serde_json::from_str(&decode(raw)?).map_err(|_| anyhow!("Wand store is not valid JSON"))?;
    let token = &store["token"];
    Ok(token["expiresAt"]
        .as_f64()
        .zip(token["accessToken"].as_str())
        .map(|(at, t)| (at, t.to_owned())))
}

// First byte: 0 = UTF-16LE, 1 = Latin-1.
fn decode(raw: &[u8]) -> Result<String> {
    match raw.split_first() {
        Some((0, rest)) => {
            let units: Vec<u16> = rest
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&c| u16::from_le_bytes(c))
                .collect();
            Ok(String::from_utf16_lossy(&units))
        }
        Some((1, rest)) => Ok(rest.iter().map(|&b| b as char).collect()),
        _ => bail!("unknown Wand store encoding"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{"token":{"accessToken":"tok-1","expiresAt":2000.5}}"#;

    fn utf16(s: &str) -> Vec<u8> {
        std::iter::once(0)
            .chain(s.encode_utf16().flat_map(u16::to_le_bytes))
            .collect()
    }

    fn latin1(s: &str) -> Vec<u8> {
        std::iter::once(1).chain(s.bytes()).collect()
    }

    #[test]
    fn parses_both_encodings() {
        for raw in [utf16(JSON), latin1(JSON)] {
            assert_eq!(parse(&raw).unwrap(), Some((2000.5, "tok-1".to_owned())));
        }
        assert!(decode(&[7, 1]).is_err());
        assert_eq!(parse(&latin1(r#"{"token":{}}"#)).unwrap(), None);
    }

    #[test]
    fn picks_newest_expiry() {
        let found = vec![
            (100.0, "old".to_owned()),
            (300.0, "new".to_owned()),
            (200.0, "mid".to_owned()),
        ];
        assert_eq!(newest(found, 50.0).unwrap().expose(), "new");
    }

    #[test]
    fn rejects_expired_and_missing() {
        let expired = newest(vec![(100.0, "old".to_owned())], 100.0).unwrap_err();
        assert_eq!(
            expired.to_string(),
            "Wand session expired: open Wand and sign in"
        );
        let missing = newest(vec![], 0.0).unwrap_err();
        assert_eq!(
            missing.to_string(),
            "Wand session not found: install Wand and sign in"
        );
    }

    #[test]
    fn debug_is_redacted() {
        let token = WandToken {
            access_token: "secret-value".to_owned(),
        };
        assert!(!format!("{token:?}").contains("secret-value"));
    }
}
