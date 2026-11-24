//! Starting a game through its launcher, or the executable the user picked.

use std::{
    ffi::{OsStr, OsString},
    fs, io,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    ptr,
};

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use model::{Failure, FailureKind, Origin};
use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

use crate::{appinfo, gog, rockstar, steam, ubisoft};

// ShellExecuteW's answer when no program handles a URI's scheme: that launcher is not installed.
const SE_ERR_NOASSOC: usize = 31;

/// What the shell is handed: a file or URI, its arguments, the folder it runs in.
#[derive(Debug, PartialEq)]
struct Command {
    target: OsString,
    params: Option<String>,
    dir: Option<PathBuf>,
}

/// Starts the game through its launcher, or the executable the user picked in its own folder (blocking). `app`: the
/// package application to open (`Installation::app`), else the manifest's first. `arguments`: the trainer's for the
/// game, remote text, for the launchers that pass any on.
/// `false`: its launcher is not here or cannot start it (EA, a standalone game), so the user starts the game and the run waits.
pub fn start(origin: &Origin, location: &Path, app: Option<&str>, arguments: Option<&str>) -> Result<bool> {
    let Some(command) = command(origin, location, app, arguments, launcher)? else {
        return Ok(false);
    };
    let opened = open(&command);
    let kind = match origin {
        Origin::Picked(_) => FailureKind::GameNotFound,
        Origin::Launcher(_) => FailureKind::LaunchFailed,
    };
    opened.context(Failure::new(kind))
}

fn launcher(store: &str) -> Option<PathBuf> {
    let dir = match store {
        "steam" => steam::install_dir(),
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
fn command(
    origin: &Origin,
    location: &Path,
    app: Option<&str>,
    arguments: Option<&str>,
    launcher: impl Fn(&str) -> Option<PathBuf>,
) -> Result<Option<Command>> {
    let id = match origin {
        Origin::Picked(exe) => {
            let params = arguments.map(passed).transpose()?.map(str::to_owned);
            return Ok(Some(Command { target: exe.into(), params, dir: exe.parent().map(Path::to_owned) }));
        }
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
        "steam" => {
            let app = id.steam_app().with_context(|| format!("unexpected Steam app id '{}'", id.id()))?;
            // The launch option without anti-cheat, when Steam lists one, wins over the trainer's arguments.
            match (dir.as_ref().and_then(|steam| appinfo::anti_cheat_off(steam, app)), arguments) {
                (Some(kind), _) => uri(format!("steam://launch/{app}/{kind}")),
                (None, None) => uri(format!("steam://rungameid/{app}")),
                (None, Some(arguments)) if arguments.starts_with(['-', '/']) => {
                    let arguments = passed(arguments)?;
                    dir.map(|dir| run(dir, steam::CLIENT, format!("-applaunch {app} {arguments}")))
                }
                // Else they go in Steam's route as a launch type: only a plain word does.
                (None, Some(kind)) => {
                    ensure!(plain(kind, b""), "unexpected Steam start arguments");
                    uri(format!("steam://launch/{app}/{kind}"))
                }
            }
        }
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
            let encoded = match arguments {
                // Like JS `btoa`: a byte per character, Latin-1 only.
                Some(arguments) => {
                    let bytes: Option<Vec<u8>> = arguments.chars().map(|c| u8::try_from(c).ok()).collect();
                    format!(" -upc_arguments {}", STANDARD.encode(bytes.context("unexpected Ubisoft start arguments")?))
                }
                None => String::new(),
            };
            dir.map(|dir| run(dir, ubisoft::LAUNCHER, format!("-upc_uplay_id {game}{encoded}")))
        }
        "rockstar" => {
            let game = plain_id(b"_")?;
            dir.map(|dir| run(dir, rockstar::LAUNCHER, format!("-minmodeApp={game}")))
        }
        "uwp" => {
            let family = plain_id(b"._-")?;
            let manifest = || fs::read_to_string(location.join("AppxManifest.xml")).unwrap_or_default();
            // The source's choice is remote data: only a plain one, else the manifest's.
            let app = app.filter(|app| plain(app, b".")).map(str::to_owned).or_else(|| app_id(&manifest()).map(str::to_owned));
            uri(format!(r"shell:AppsFolder\{family}!{}", app.as_deref().unwrap_or("App")))
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
    plain(id, b".").then_some(id)
}

/// The trainer's arguments as they reach a command line: remote text, so no control character.
fn passed(arguments: &str) -> Result<&str> {
    ensure!(!arguments.contains(char::is_control), "unexpected start arguments");
    Ok(arguments)
}

fn plain(value: &str, extra: &[u8]) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_alphanumeric() || extra.contains(&b))
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
    use std::env;

    use model::StoreId;

    use super::*;

    fn command_of(id: &str, location: &str) -> Result<Option<Command>> {
        command_with(id, location, None)
    }

    fn command_with(id: &str, location: &str, app: Option<&str>) -> Result<Option<Command>> {
        // Steam's folder would be read for its app cache.
        let installed = |store: &str| (!["rockstar", "steam"].contains(&store)).then(|| PathBuf::from(format!(r"C:\{store}")));
        command(&Origin::Launcher(StoreId::parse(id).unwrap()), Path::new(location), app, None, installed)
    }

    /// With the trainer's `arguments`; every launcher is in a folder of its name that does not exist (no Steam cache).
    fn passing(id: &str, arguments: &str) -> Result<Option<Command>> {
        let nowhere = |store: &str| Some(env::temp_dir().join(format!("impulse-nowhere-{store}")));
        command(&Origin::Launcher(StoreId::parse(id).unwrap()), Path::new(r"D:\G"), None, Some(arguments), nowhere)
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
        let package = |app| command_with("uwp:Halo_8w", r"D:\Games\X", Some(app)).unwrap().unwrap().target;
        assert_eq!(package("HaloMCCShippingNoEAC"), r"shell:AppsFolder\Halo_8w!HaloMCCShippingNoEAC", "the source's application");
        assert_eq!(package("a!b"), r"shell:AppsFolder\Halo_8w!App", "an odd one is not used");
        assert_eq!(command_of("rockstar:rdr2", "").unwrap(), None, "its launcher is not installed");
        assert_eq!(command_of("ea:Origin.OFR.1", "").unwrap(), None);
        assert_eq!(command_of("standalone:gta", "").unwrap(), None, "the catalog's own launcher file is not run");
    }

    #[test]
    fn a_steam_game_starts_through_its_option_without_anti_cheat() {
        let steam = crate::appinfo::tests::steam_folder("start", 976730, &[("0", "Play without EAC", Some("option1"))]);
        let origin = Origin::Launcher(StoreId::parse("steam:976730").unwrap());
        let target = command(&origin, Path::new(""), None, Some("-dx11"), |_| Some(steam.clone())).unwrap().unwrap().target;
        assert_eq!(target, "steam://launch/976730/option1", "over the trainer's arguments");
        fs::remove_dir_all(&steam).unwrap();
    }

    #[test]
    fn the_trainers_arguments_reach_the_launchers_that_pass_them_on() {
        let steam = passing("steam:7", "-dx11 -windowed").unwrap().unwrap();
        assert_eq!(steam.target, env::temp_dir().join(r"impulse-nowhere-steam\steam.exe").into_os_string());
        assert_eq!(steam.params.as_deref(), Some("-applaunch 7 -dx11 -windowed"));
        assert_eq!(passing("steam:7", "dialog").unwrap().unwrap().target, "steam://launch/7/dialog", "a word goes in Steam's route");
        assert!(passing("steam:7", "a/b").is_err(), "nothing else goes there");
        let uplay = passing("uplay:635", "-x 1").unwrap().unwrap().params;
        assert_eq!(uplay.as_deref(), Some("-upc_uplay_id 635 -upc_arguments LXggMQ=="), "base64");
        let latin = passing("uplay:635", "-é").unwrap().unwrap().params;
        assert_eq!(latin.as_deref(), Some("-upc_uplay_id 635 -upc_arguments Lek="), "a byte per character, as btoa");
        assert!(passing("uplay:635", "-ő").is_err(), "beyond what btoa takes");
        assert!(passing("steam:7", "-a\0b").is_err() && passing("steam:7", "-a\nb").is_err());
        assert_eq!(passing("epic:x", "-x\n").unwrap().unwrap().params, None, "Epic passes none on, and so checks none");
        let exe = PathBuf::from(r"D:\G\g.exe");
        let picked = command(&Origin::Picked(exe), Path::new(r"D:\G"), None, Some("-x"), |_| None).unwrap().unwrap();
        assert_eq!(picked.params.as_deref(), Some("-x"));
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
