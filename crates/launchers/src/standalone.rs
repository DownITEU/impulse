use std::{collections::HashSet, env, path::PathBuf};

use model::{Game, Locator};

use super::{
    App,
    registry::{CURRENT_USER, LOCAL_MACHINE, View, value},
};

const UNINSTALL: &str = r"HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";

/// Games outside any store; the game says where to look.
pub fn apps(games: &[Game]) -> Vec<App> {
    // Editions can share one.
    let mut seen = HashSet::new();
    games
        .iter()
        .flat_map(|game| &game.standalone)
        .filter(|app| seen.insert(app.store_id.to_ascii_lowercase()))
        .filter_map(|app| {
            let location = app.locators.iter().filter_map(locate).find(|dir| dir.join(&app.launcher).is_file())?;
            Some(App { store_id: app.store_id.clone(), name: None, location })
        })
        .collect()
}

fn locate(locator: &Locator) -> Option<PathBuf> {
    let raw = match locator {
        Locator::Directory(dir) => dir.clone(),
        Locator::RegistryValue(value) => registry_value(value)?,
        Locator::UninstallKey(key) => registry_value(&format!(r"{UNINSTALL}\{key}\InstallLocation"))?,
    };
    // Values can be quoted or carry an icon index: "C:\Game\game.exe",0
    let raw = raw.split(',').next()?.replace('"', "");
    Some(expand(&raw).replace('/', "\\").into())
}

// `HIVE\path\to\key\ValueName`, tried in the 64-bit view, then the 32-bit one.
fn registry_value(full: &str) -> Option<String> {
    let (key, name) = full.rsplit_once('\\')?;
    let (hive, path) = key.split_once('\\')?;
    let root = match hive {
        "HKEY_LOCAL_MACHINE" => LOCAL_MACHINE,
        "HKEY_CURRENT_USER" => CURRENT_USER,
        _ => return None,
    };
    value(root, path, name, View::Wow64).or_else(|| value(root, path, name, View::Wow32))
}

/// Expands `%VAR%`; unknown variables stay as written.
fn expand(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('%') else {
            break;
        };
        out.push_str(&rest[..start]);
        match env::var(&after[..end]) {
            Ok(v) => out.push_str(&v),
            Err(_) => out.push_str(&rest[start..start + end + 2]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_known_variables_only() {
        let windir = env::var("windir").unwrap();
        assert_eq!(expand(r"%WINDIR%\x"), format!(r"{windir}\x"));
        assert_eq!(expand("%NO_SUCH_VAR_IMPULSE%/a"), "%NO_SUCH_VAR_IMPULSE%/a");
        assert_eq!(expand("50% off"), "50% off");
    }
}
