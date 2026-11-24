//! Starting a game through its launcher, or the executable the user picked.

use std::{
    ffi::{OsStr, OsString},
    fs, io,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    ptr,
};

use anyhow::{Context, Result, bail, ensure};
use model::{Failure, FailureKind, Origin};
use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

use crate::{gog, rockstar, ubisoft};

// ShellExecuteW's answer when no program handles a URI's scheme: that launcher is not installed.
const SE_ERR_NOASSOC: usize = 31;

/// What the shell is handed: a file or URI, its arguments, the folder it runs in.
#[derive(Debug, PartialEq)]
struct Command {
    target: OsString,
    params: Option<String>,
    dir: Option<PathBuf>,
}

/// Starts the game through its launcher, or the executable the user picked in its own folder (blocking).
/// `false`: its launcher is not here or cannot start it (EA, a standalone game), so the user starts the game and the run waits.
pub fn start(origin: &Origin, location: &Path) -> Result<bool> {
    let Some(command) = command(origin, location, launcher)? else {
        return Ok(false);
    };
    let opened = open(&command);
    match origin {
        Origin::Picked(_) => opened.context(Failure::new(FailureKind::GameNotFound)),
        Origin::Launcher(_) => opened,
    }
}

fn launcher(store: &str) -> Option<PathBuf> {
    let dir = match store {
        "gog" => gog::launcher(),
        "uplay" => ubisoft::launcher(),
        "rockstar" => rockstar::launcher(),
        _ => None,
    };
    // A relative registry value would run whatever sits in the current folder.
    dir.filter(|dir| dir.is_absolute())
}

/// Ids come from local records, the folders from the launchers' registry: each is checked to be plain before it reaches
/// a URI or an argument. `launcher` is a store's launcher folder, when it is installed.
fn command(origin: &Origin, location: &Path, launcher: impl Fn(&str) -> Option<PathBuf>) -> Result<Option<Command>> {
    let id = match origin {
        Origin::Picked(exe) => return Ok(Some(Command { target: exe.into(), params: None, dir: exe.parent().map(Path::to_owned) })),
        Origin::Launcher(id) => id,
    };
    let uri = |uri: String| Some(Command { target: uri.into(), params: None, dir: None });
    let run = |dir: PathBuf, exe: &str, params: String| Command { target: dir.join(exe).into(), params: Some(params), dir: Some(dir) };
    let plain_id = |extra: &[u8]| {
        let value = id.id();
        ensure!(plain(value, extra), "unexpected {} id '{value}'", id.store());
        Ok::<_, anyhow::Error>(value)
    };
    let dir = launcher(id.store());
    Ok(match id.store() {
        // The id comes from a remote catalog: only a number reaches the URI.
        "steam" => uri(format!("steam://rungameid/{}", id.steam_app().with_context(|| format!("unexpected Steam app id '{}'", id.id()))?)),
        "epic" => uri(format!("com.epicgames.launcher://apps/{}?action=launch&silent=true", percent(id.id()))),
        "gog" => {
            let game = plain_id(b"")?;
            let folder = location.to_str().filter(|folder| !folder.contains(['"', '\0'])).context("unexpected GOG folder")?;
            // A backslash before the closing quote would escape it.
            let folder = folder.trim_end_matches('\\');
            dir.map(|dir| run(dir, gog::CLIENT, format!(r#"/command=runGame /gameId={game} /path="{folder}""#)))
        }
        "uplay" => {
            let game = plain_id(b"")?;
            launcher("uplay").map(|dir| run(dir, ubisoft::LAUNCHER, format!("-upc_uplay_id {game}")))
        }
        "rockstar" => {
            let game = plain_id(b"_")?;
            dir.map(|dir| run(dir, rockstar::LAUNCHER, format!("-minmodeApp={game}")))
        }
        "uwp" => {
            let family = plain(b"._-")?;
            let manifest = fs::read_to_string(location.join("AppxManifest.xml")).unwrap_or_default();
            uri(format!(r"shell:AppsFolder\{family}!{}", app_id(&manifest).unwrap_or("App")))
        }
        _ => None,
    })
}

/// The package's first application.
fn app_id(manifest: &str) -> Option<&str> {
    let at = manifest.match_indices("<Application").find(|(at, tag)| manifest[at + tag.len()..].starts_with(char::is_whitespace))?.0;
    let tag = &manifest[at..at + manifest[at..].find('>')?];
    let value = tag.match_indices("Id=\"").find(|(i, _)| tag[..*i].ends_with(char::is_whitespace))?.0 + "Id=\"".len();
    let id = tag[value..].split('"').next()?;
    (!id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.')).then_some(id)
}

fn percent(text: &str) -> String {
    text.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { char::from(b).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// `Ok(false)` when no program handles a URI's scheme.
fn open(command: &Command) -> Result<bool> {
    let wide = |text: &OsStr| text.encode_wide().chain([0]).collect::<Vec<u16>>();
    let (target, verb) = (wide(&command.target), wide(OsStr::new("open")));
    let params = command.params.as_deref().map(|params| wide(OsStr::new(params)));
    let dir = command.dir.as_ref().map(|dir| wide(dir.as_os_str()));
    let (params_w, dir_w) = (params.as_ref().map_or(ptr::null(), |p| p.as_ptr()), dir.as_ref().map_or(ptr::null(), |d| d.as_ptr()));
    // SAFETY: the strings are NUL-terminated and outlive the call.
    let result = unsafe { ShellExecuteW(ptr::null_mut(), verb.as_ptr(), target.as_ptr(), params_w, dir_w, SW_SHOWNORMAL) } as usize;
    match result {
        SE_ERR_NOASSOC => Ok(false),
        // Values up to 32 are error codes.
        0..=32 => bail!("cannot open {}: {}", command.target.display(), io::Error::last_os_error()),
        _ => Ok(true),
    }
}

#[cfg(test)]
mod tests {
    use model::StoreId;

    use super::*;

    fn command_of(id: &str, location: &str) -> Result<Option<Command>> {
        let installed = |store: &str| (store != "rockstar").then(|| PathBuf::from(format!(r"C:\{store}")));
        command(&Origin::Launcher(StoreId::parse(id).unwrap()), Path::new(location), installed)
    }

    fn params(id: &str, location: &str) -> Option<String> {
        command_of(id, location).unwrap().unwrap().params
    }

    #[test]
    fn each_store_starts_through_its_launcher() {
        let target = |id: &str| command_of(id, r"D:\Games\X").unwrap().unwrap().target;
        assert_eq!(target("steam:1203620"), "steam://rungameid/1203620");
        assert_eq!(target("epic:ns:item:App Name"), "com.epicgames.launcher://apps/ns%3Aitem%3AApp%20Name?action=launch&silent=true");
        assert_eq!(target("gog:1207"), r"C:\gog\GalaxyClient.exe");
        assert_eq!(params("gog:1207", r"D:\GOG Games\X\").unwrap(), r#"/command=runGame /gameId=1207 /path="D:\GOG Games\X""#);
        assert_eq!(params("uplay:635", "").unwrap(), "-upc_uplay_id 635");
        assert_eq!(target("uwp:Game.Name_8wekyb3d8bbwe"), r"shell:AppsFolder\Game.Name_8wekyb3d8bbwe!App", "no manifest: App");
        assert_eq!(command_of("rockstar:rdr2", "").unwrap(), None, "its launcher is not installed");
        assert_eq!(command_of("ea:Origin.OFR.1", "").unwrap(), None);
        assert_eq!(command_of("standalone:gta", "").unwrap(), None, "the catalog's own launcher file is not run");
    }

    #[test]
    fn odd_ids_and_folders_are_refused() {
        for id in ["steam:", "steam:12 34", "steam:1&calc", "gog:1 2", "uplay:6&x", "rockstar:a b", "uwp:a/b", "uwp:x!y"] {
            assert!(command_of(id, r"D:\Games").is_err(), "{id}");
        }
        assert!(command_of("gog:1", r#"D:\a" /x"#).is_err(), "a quote would end the path argument");
    }

    #[test]
    fn the_first_application_of_a_manifest_is_started() {
        let manifest =
            r#"<Package><Applications><Application Id="HaloMCC" Executable="mcc.exe"/><Application Id="Other"/></Applications></Package>"#;
        assert_eq!(app_id(manifest), Some("HaloMCC"));
        assert_eq!(app_id(r#"<Applications><Application Id="a&b"/>"#), None);
        assert_eq!(app_id("<Package/>"), None);
    }
}
