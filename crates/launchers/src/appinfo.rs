//! Steam's binary `appcache\appinfo.vdf`: an app's launch options.

use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
};

use crate::vdf::Node;

// The two layouts; 29 keeps key names in a table at the file's end.
const V28: u32 = 0x0756_4428;
const V29: u32 = 0x0756_4429;
// An entry's state, times, token, hashes and change number come before its key values.
const ENTRY_HEAD: usize = 60;
const DEPTH: usize = 32;
// A damaged v29 entry could name one long key many times: the copies are bounded, far above a real entry's.
const KEY_BYTES: usize = 16 << 20;
// Words of an option that starts a game without its anti-cheat.
const OFF: [&str; 3] = ["no", "without", "disable"];
const CHEATS: [&str; 7] = ["battleye", "anticheat", "anti cheat", "easyanticheat", "easy anti cheat", "eac", "be"];

/// The launch type of `app`'s option that starts it without its anti-cheat (blocking): its `type`,
/// else `dialog` (Steam's chooser). `None` without one.
pub fn anti_cheat_off(steam: &Path, app: u32) -> Option<String> {
    let file = read(&steam.join(r"appcache\appinfo.vdf"), app)?;
    let info = file.get("appinfo").filter(|info| info.get("common").is_some())?;
    let mut options: Vec<&(String, Node)> = info.get("config")?.get("launch")?.entries().iter().collect();
    // In JS object order, the one the options are meant in: array index keys (canonical, below u32::MAX) first, ascending.
    let index = |key: &str| key.parse::<u32>().ok().filter(|n| *n < u32::MAX && n.to_string() == key);
    options.sort_by_cached_key(|(key, _)| index(key).map_or((1, 0), |n| (0, n)));
    let option = options.into_iter().map(|(_, option)| option).find(|option| option.str("description").is_some_and(disables))?;
    let kind = option.str("type").filter(|kind| !kind.is_empty() && kind.bytes().all(|b| b.is_ascii_alphanumeric()));
    Some(kind.unwrap_or("dialog").to_owned())
}

/// Whether the description is of an option without anti-cheat: lower case, only letters, digits and white space kept,
/// then an "off" word and a cheat's, or a cheat's and "disable", apart by white space.
fn disables(description: &str) -> bool {
    let text: String = description.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric() || js_space(*c)).collect();
    let followed = |first: &[&str], second: &[&str]| {
        first.iter().any(|first| {
            text.match_indices(first).any(|(at, _)| {
                let rest = &text[at + first.len()..];
                let after = rest.trim_start_matches(js_space);
                after.len() < rest.len() && second.iter().any(|second| after.starts_with(second))
            })
        })
    };
    followed(&OFF, &CHEATS) || followed(&CHEATS, &["disable"])
}

/// JavaScript's `\s`: Unicode white space without U+0085, with U+FEFF.
fn js_space(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}'
}

/// `app`'s key values, found by walking the entries.
fn read(path: &Path, app: u32) -> Option<Node> {
    let mut file = BufReader::new(File::open(path).ok()?);
    let [magic, _universe] = [u32::from_le_bytes(take(&mut file)?), u32::from_le_bytes(take(&mut file)?)];
    let strings = match magic {
        V28 => None,
        V29 => {
            let at = u64::try_from(i64::from_le_bytes(take(&mut file)?)).ok()?;
            let entries = file.stream_position().ok()?;
            file.seek(SeekFrom::Start(at)).ok()?;
            let mut table = Vec::new();
            file.read_to_end(&mut table).ok()?;
            file.seek(SeekFrom::Start(entries)).ok()?;
            Some(strings(&table)?)
        }
        _ => return None,
    };
    loop {
        let [id, size] = [u32::from_le_bytes(take(&mut file)?), u32::from_le_bytes(take(&mut file)?)];
        if id == 0 {
            return None;
        }
        if id != app {
            file.seek_relative(i64::from(size)).ok()?;
            continue;
        }
        // Grown as read: a damaged size asks for no more than the file holds.
        let mut entry = Vec::new();
        file.take(u64::from(size)).read_to_end(&mut entry).ok()?;
        if entry.len() != size as usize {
            return None;
        }
        let mut cursor = Cursor { data: entry.get(ENTRY_HEAD..)?, at: 0, strings: strings.as_deref(), key_bytes: KEY_BYTES };
        return map(&mut cursor, 0);
    }
}

fn take<const N: usize>(file: &mut impl Read) -> Option<[u8; N]> {
    let mut bytes = [0; N];
    file.read_exact(&mut bytes).ok().map(|()| bytes)
}

/// The v29 key names: their count, then each ending in NUL.
fn strings(table: &[u8]) -> Option<Vec<String>> {
    let count = u32::from_le_bytes(table.get(..4)?.try_into().ok()?);
    let mut cursor = Cursor { data: table, at: 4, strings: None, key_bytes: 0 };
    (0..count).map(|_| cursor.text()).collect()
}

struct Cursor<'a> {
    data: &'a [u8],
    at: usize,
    strings: Option<&'a [String]>,
    /// What is left of `KEY_BYTES`.
    key_bytes: usize,
}

impl Cursor<'_> {
    fn bytes<const N: usize>(&mut self) -> Option<[u8; N]> {
        let bytes = self.data.get(self.at..self.at + N)?.try_into().ok()?;
        self.at += N;
        Some(bytes)
    }

    fn text(&mut self) -> Option<String> {
        let len = self.data.get(self.at..)?.iter().position(|&b| b == 0)?;
        let text = String::from_utf8_lossy(&self.data[self.at..self.at + len]).into_owned();
        self.at += len + 1;
        Some(text)
    }
}

/// Key values up to their end mark; numbers as text, which is all a lookup here needs.
fn map(cursor: &mut Cursor, depth: usize) -> Option<Node> {
    let mut entries = Vec::new();
    loop {
        let [kind] = cursor.bytes()?;
        if kind == 8 {
            return Some(Node::Map(entries));
        }
        let key = match cursor.strings {
            Some(strings) => {
                let key = strings.get(u32::from_le_bytes(cursor.bytes()?) as usize)?;
                cursor.key_bytes = cursor.key_bytes.checked_sub(key.len())?;
                key.clone()
            }
            None => cursor.text()?,
        };
        let value = match kind {
            0 if depth < DEPTH => map(cursor, depth + 1)?,
            1 => Node::Str(cursor.text()?),
            2 | 4 | 6 => Node::Str(i32::from_le_bytes(cursor.bytes()?).to_string()),
            3 => Node::Str(f32::from_le_bytes(cursor.bytes()?).to_string()),
            7 => Node::Str(u64::from_le_bytes(cursor.bytes()?).to_string()),
            10 => Node::Str(i64::from_le_bytes(cursor.bytes()?).to_string()),
            _ => return None,
        };
        entries.push((key, value));
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::{env, fs, path::PathBuf};

    use super::*;

    #[test]
    fn the_words_for_an_option_without_anti_cheat() {
        let yes = ["Play without Anti-Cheat", "Halo MCC (Anti-Cheat Disabled)", "No  EAC", "Launch (BattlEye disabled)", "disable\teac"];
        for yes in yes.into_iter().chain(["No\u{feff}EAC"]) {
            assert!(disables(yes), "{yes}");
        }
        for no in ["Play", "Anti-Cheat Enabled", "noeac", "Launch with BattlEye", "No\u{85}EAC"] {
            assert!(!disables(no), "{no}");
        }
    }

    /// A Steam folder in the temp folder, `label` apart, whose app cache holds `app` with `launch` options.
    pub(crate) fn steam_folder(label: &str, app: u32, launch: &[(&str, &str, Option<&str>)]) -> PathBuf {
        let steam = env::temp_dir().join(format!("impulse-{label}-{}", std::process::id()));
        fs::create_dir_all(steam.join("appcache")).unwrap();
        fs::write(steam.join(r"appcache\appinfo.vdf"), appinfo(app, launch)).unwrap();
        steam
    }

    /// A v29 file of one app: key names by index into the table after the entries.
    fn appinfo(app: u32, launch: &[(&str, &str, Option<&str>)]) -> Vec<u8> {
        let names = ["appinfo", "common", "name", "config", "launch", "description", "type", "0", "1", "2", "01"];
        let index = |name: &str| (names.iter().position(|n| *n == name).unwrap() as u32).to_le_bytes();
        let mut kv = Vec::new();
        let text = |kv: &mut Vec<u8>, key: &str, value: &str| {
            kv.push(1);
            kv.extend(index(key));
            kv.extend(value.as_bytes());
            kv.push(0);
        };
        let open = |kv: &mut Vec<u8>, key: &str| {
            kv.push(0);
            kv.extend(index(key));
        };
        open(&mut kv, "appinfo");
        open(&mut kv, "common");
        text(&mut kv, "name", "Game");
        kv.push(8);
        open(&mut kv, "config");
        open(&mut kv, "launch");
        for (key, description, kind) in launch {
            open(&mut kv, key);
            text(&mut kv, "description", description);
            if let Some(kind) = kind {
                text(&mut kv, "type", kind);
            }
            kv.push(8);
        }
        kv.extend([8, 8, 8, 8]);
        let entry = [vec![0; ENTRY_HEAD], kv].concat();
        let mut file = [V29.to_le_bytes(), 1u32.to_le_bytes()].concat();
        let entries = [7u32.to_le_bytes(), 0u32.to_le_bytes(), app.to_le_bytes(), (entry.len() as u32).to_le_bytes()].concat();
        let table_at = 16 + entries.len() + entry.len() + 4;
        file.extend((table_at as i64).to_le_bytes());
        file.extend(entries);
        file.extend(entry);
        file.extend(0u32.to_le_bytes());
        file.extend((names.len() as u32).to_le_bytes());
        for name in names {
            file.extend(name.as_bytes());
            file.push(0);
        }
        file
    }

    #[test]
    fn the_option_without_anti_cheat_is_found_in_steams_app_cache() {
        let steam = |launch: &[(&str, &str, Option<&str>)]| steam_folder("appinfo", 976730, launch);

        let steam_dir = steam(&[("1", "Play (Anti-Cheat Disabled)", Some("option1")), ("0", "Play", Some("default"))]);
        assert_eq!(anti_cheat_off(&steam_dir, 976730).as_deref(), Some("option1"), "past an entry of another app");
        assert_eq!(anti_cheat_off(&steam_dir, 1), None, "an app not there");
        steam(&[("2", "Without BattlEye", Some("option2")), ("0", "No EAC", None)]);
        assert_eq!(anti_cheat_off(&steam_dir, 976730).as_deref(), Some("dialog"), "the lowest index first, without a type: the chooser");
        steam(&[("01", "No EAC", Some("option1")), ("2", "No BE", Some("option2"))]);
        assert_eq!(anti_cheat_off(&steam_dir, 976730).as_deref(), Some("option2"), "\"01\" is no index: it keeps its place after");
        steam(&[("0", "Play", Some("default"))]);
        assert_eq!(anti_cheat_off(&steam_dir, 976730), None);
        fs::remove_dir_all(&steam_dir).unwrap();
    }

    #[test]
    fn a_key_name_without_its_end_is_a_damaged_table() {
        assert_eq!(strings(&[2, 0, 0, 0, b'a', 0, b'b', 0]), Some(vec!["a".into(), "b".into()]));
        assert_eq!(strings(&[2, 0, 0, 0, b'a', 0, b'b']), None);
    }

    /// Needs Steam with an app cache: its first app reads.
    #[test]
    #[ignore]
    fn steams_own_file_reads() {
        let path = crate::steam::install_dir().expect("Steam").join(r"appcache\appinfo.vdf");
        let bytes = fs::read(&path).expect("Steam's app cache");
        let first = if bytes.get(..4) == Some(&V29.to_le_bytes()) { 16 } else { 8 };
        let app = u32::from_le_bytes(bytes.get(first..first + 4).expect("an entry").try_into().unwrap());
        let info = read(&path, app).expect("the first app reads");
        assert!(info.get("appinfo").is_some_and(|info| info.str("appid").is_some()), "app {app}");
    }
}
