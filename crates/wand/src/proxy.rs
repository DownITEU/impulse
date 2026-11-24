//! Wand's way past an anti-cheat's launcher (`launch.proxy`): for one start, Wand's `GameLauncher.exe` stands
//! in for the launcher the store runs and starts the game itself, then puts the launcher back.

use std::{
    ffi::OsString,
    fs, io,
    path::{Component, Path, PathBuf},
    sync::{Condvar, Mutex, PoisonError},
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use serde_json::Value;

// The names Wand's service and the stand-in use beside the launcher.
const BACKUP: &str = ".wand-backup";
const CONFIG: &str = ".wand-config";
const LEFT: &str = ".wand";

#[derive(Clone, Debug, PartialEq)]
pub struct Proxy {
    /// Relative to the install folder.
    launcher: String,
    config: Config,
}

/// What the stand-in reads: paths relative to the launcher's folder.
#[derive(Clone, Debug, PartialEq, Serialize)]
struct Config {
    exe: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cwd: Option<String>,
    flags: u32,
}

impl Proxy {
    /// `launch.proxy`; `Err` names a field that is missing or would leave the game's folder.
    pub fn parse(launch: &Value) -> Result<Option<Self>, String> {
        let Some(proxy) = launch.get("proxy").filter(|proxy| proxy.is_object()) else { return Ok(None) };
        let text = |name| proxy.get(name).and_then(Value::as_str).filter(|text| !text.is_empty());
        let cwd = text("gameCwd").map(|cwd| inside("gameCwd", cwd, false)).transpose()?;
        Ok(Some(Self {
            launcher: inside("launcher", text("launcher").unwrap_or_default(), true)?,
            config: Config {
                exe: inside("gameExe", text("gameExe").unwrap_or_default(), true)?,
                args: text("gameArgs").map(str::to_owned),
                cwd,
                flags: proxy.get("flags").and_then(Value::as_u64).and_then(|flags| u32::try_from(flags).ok()).unwrap_or(0),
            },
        }))
    }

    /// The game's executable relative to the install folder.
    pub fn game(&self) -> String {
        let folder = Path::new(&self.launcher).parent().unwrap_or(Path::new(""));
        folder.join(&self.config.exe).to_string_lossy().into_owned()
    }
}

/// A relative path that stays in its folder (no root, drive, `..`, `.` or stream), with single `\`s: the process
/// rule matches whole names.
fn inside(field: &str, path: &str, exe: bool) -> Result<String, String> {
    // Windows drops a name's trailing dots and spaces: the process would not match.
    let name = |part| match part {
        Component::Normal(name) => name.to_str().filter(|name| !name.ends_with(['.', ' '])),
        _ => None,
    };
    let parts: Option<Vec<_>> = Path::new(path).components().map(name).collect();
    let typed = |path: &String| !path.is_empty() && !path.contains(':') && (!exe || path.to_ascii_lowercase().ends_with(".exe"));
    let plain = parts.map(|parts| parts.join(r"\")).filter(typed);
    plain.ok_or_else(|| format!("launch.proxy.{field} '{path}' is not a file in the game's folder"))
}

/// The stand-in in place of the launcher; dropping it puts the launcher back. Blocks.
pub struct Mounted {
    launcher: PathBuf,
    _lease: Lease,
}

/// Launchers a mount holds until they are back: a retry waits for the restore of the run before it.
static HELD: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
static FREED: Condvar = Condvar::new();
const FREED_WITHIN: Duration = Duration::from_secs(5);

struct Lease(PathBuf);

impl Lease {
    fn take(launcher: &Path) -> Result<Self> {
        let busy = |held: &mut Vec<PathBuf>| held.iter().any(|path| path.as_os_str().eq_ignore_ascii_case(launcher));
        let held = HELD.lock().unwrap_or_else(PoisonError::into_inner);
        let (mut held, _) = FREED.wait_timeout_while(held, FREED_WITHIN, busy).unwrap_or_else(PoisonError::into_inner);
        ensure!(!busy(&mut held), "{} is still being put back", launcher.display());
        held.push(launcher.to_owned());
        Ok(Self(launcher.to_owned()))
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        HELD.lock().unwrap_or_else(PoisonError::into_inner).retain(|path| !path.as_os_str().eq_ignore_ascii_case(&self.0));
        FREED.notify_all();
    }
}

/// Puts `stand_in` in place of the proxy's launcher in `install`, the launcher kept beside it, and writes what the
/// stand-in starts. `check` vets the copy that will run. A stand-in left by a start that never came (ours, or
/// another Wand version's: it passes `check`) keeps its backup; any other leftover is refused, never overwritten.
pub fn mount(stand_in: &Path, install: &Path, proxy: &Proxy, check: impl Fn(&Path) -> Result<()>) -> Result<Mounted> {
    let launcher = install.join(&proxy.launcher);
    let backup = beside(&launcher, BACKUP);
    let lease = Lease::take(&launcher)?;
    // A link in the game's folder must not carry the writes out of it.
    let real = |path: &Path| fs::canonicalize(path).with_context(|| format!("cannot find {}", path.display()));
    ensure!(
        real(launcher.parent().unwrap_or(install))?.starts_with(real(install)?),
        "{} leads out of the game's folder",
        launcher.display()
    );
    let ours = fs::read(stand_in).with_context(|| format!("cannot read {}", stand_in.display()))?;
    let theirs = fs::read(&launcher).with_context(|| format!("cannot read {}", launcher.display()))?;
    let stood_in = theirs == ours || check(&launcher).is_ok();
    match (stood_in, backup.exists()) {
        (false, false) => fs::rename(&launcher, &backup).with_context(|| format!("cannot move {}", launcher.display()))?,
        (true, true) => {}
        (true, false) => bail!("{} is Wand's stand-in, without the launcher it stands for", launcher.display()),
        (false, true) => bail!("{} is left from an earlier start", backup.display()),
    }
    // From here a failure puts the launcher back.
    let mounted = Mounted { launcher, _lease: lease };
    if theirs != ours {
        fs::write(&mounted.launcher, &ours).with_context(|| format!("cannot write {}", mounted.launcher.display()))?;
    }
    check(&mounted.launcher)?;
    let config = serde_json::to_vec(&proxy.config).context("cannot write the proxy's config")?;
    fs::write(beside(&mounted.launcher, CONFIG), config).context("cannot write the proxy's config")?;
    Ok(mounted)
}

impl Drop for Mounted {
    fn drop(&mut self) {
        if let Err(e) = unmount(&self.launcher) {
            // The stand-in restores it itself at the next start.
            log::warn!("cannot put {} back: {e}", self.launcher.display());
        }
    }
}

/// The stand-in moves itself to `.wand` once the game starts; a running one cannot be removed yet.
fn unmount(launcher: &Path) -> io::Result<()> {
    let (backup, left) = (beside(launcher, BACKUP), beside(launcher, LEFT));
    remove(&left).ok();
    if backup.exists() {
        if launcher.exists() {
            fs::rename(launcher, &left)?;
        }
        if let Err(e) = fs::rename(&backup, launcher) {
            // The stand-in, its backup and config stay valid: the next start restores them.
            fs::rename(&left, launcher).ok();
            return Err(e);
        }
        remove(&left).ok();
    }
    remove(&beside(launcher, CONFIG))
}

fn beside(path: &Path, suffix: &str) -> PathBuf {
    let mut name = OsString::from(path);
    name.push(suffix);
    name.into()
}

fn remove(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::{env, process::Command};

    use serde_json::json;

    use super::*;

    const STAND_IN: &[u8] = b"stand-in";
    const ORIGINAL: &[u8] = b"anti-cheat launcher";

    /// The publisher's check: only the stand-in is signed.
    fn signed(path: &Path) -> Result<()> {
        ensure!(fs::read(path)? == STAND_IN, "{} is not signed", path.display());
        Ok(())
    }

    fn ace_combat() -> Proxy {
        let launch = json!({"proxy": {"gameExe": "Game\\Binaries\\Win64\\AceCombat8.exe", "launcher": "start_protected_game.exe"}});
        Proxy::parse(&launch).unwrap().unwrap()
    }

    /// An install with the launcher and a stand-in outside it.
    fn install(name: &str) -> (PathBuf, PathBuf) {
        let root = env::temp_dir().join(format!("impulse-proxy-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("game")).unwrap();
        fs::write(root.join("game").join("start_protected_game.exe"), ORIGINAL).unwrap();
        fs::write(root.join("GameLauncher.exe"), STAND_IN).unwrap();
        (root.join("GameLauncher.exe"), root.join("game"))
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(dir).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    #[test]
    fn reads_wands_proxies_and_refuses_paths_out_of_the_game() {
        let proxy = ace_combat();
        assert_eq!(proxy.game(), r"Game\Binaries\Win64\AceCombat8.exe");
        let elden =
            json!({"proxy": {"gameExe": "eldenring.exe", "launcher": "Game\\start_protected_game.exe", "flags": 10, "gameCwd": "Game"}});
        let elden = Proxy::parse(&elden).unwrap().unwrap();
        assert_eq!((elden.game().as_str(), elden.config.flags, elden.config.cwd.as_deref()), (r"Game\eldenring.exe", 10, Some("Game")));
        let doubled = Proxy::parse(&json!({"proxy": {"launcher": "start.exe", "gameExe": r"Game\\Win64//g.exe"}})).unwrap().unwrap();
        assert_eq!(doubled.game(), r"Game\Win64\g.exe");
        for launch in [json!(false), json!({"required": true}), json!({"proxy": null})] {
            assert_eq!(Proxy::parse(&launch), Ok(None), "{launch}");
        }
        for (launcher, game) in [
            (r"..\start.exe", "g.exe"),
            (r"C:\start.exe", "g.exe"),
            (r"\start.exe", "g.exe"),
            ("start.exe:stream", "g.exe"),
            ("start.bat", "g.exe"),
            ("start.exe", r"bin\..\..\g.exe"),
            ("start.exe", r".\g.exe"),
            ("start.exe", r"Game.\g.exe"),
            ("start.exe", ""),
        ] {
            let launch = json!({"proxy": {"launcher": launcher, "gameExe": game}});
            assert!(Proxy::parse(&launch).is_err(), "{launch}");
        }
        assert!(Proxy::parse(&json!({"proxy": {"launcher": "s.exe", "gameExe": "g.exe", "gameCwd": ".."}})).is_err());
    }

    #[test]
    fn a_mount_swaps_the_launcher_and_its_drop_puts_it_back_byte_for_byte() {
        let (stand_in, game) = install("swap");
        let launcher = game.join("start_protected_game.exe");
        let mounted = mount(&stand_in, &game, &ace_combat(), signed).unwrap();
        assert_eq!(fs::read(&launcher).unwrap(), STAND_IN);
        assert_eq!(fs::read(beside(&launcher, BACKUP)).unwrap(), ORIGINAL);
        let config: Value = serde_json::from_slice(&fs::read(beside(&launcher, CONFIG)).unwrap()).unwrap();
        assert_eq!(config, json!({"exe": "Game\\Binaries\\Win64\\AceCombat8.exe", "flags": 0}));
        drop(mounted);
        assert_eq!(fs::read(&launcher).unwrap(), ORIGINAL);
        assert_eq!(files(&game), ["start_protected_game.exe"]);
        fs::remove_dir_all(stand_in.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_stand_in_left_by_a_start_that_never_came_keeps_its_backup() {
        let (stand_in, game) = install("left");
        let launcher = game.join("start_protected_game.exe");
        // What an app that quit mid-start leaves.
        fs::rename(&launcher, beside(&launcher, BACKUP)).unwrap();
        fs::write(&launcher, STAND_IN).unwrap();
        let again = mount(&stand_in, &game, &ace_combat(), signed).unwrap();
        assert_eq!(fs::read(beside(&launcher, BACKUP)).unwrap(), ORIGINAL);
        drop(again);
        assert_eq!(fs::read(&launcher).unwrap(), ORIGINAL);
        fs::remove_dir_all(stand_in.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_retry_mounts_only_once_the_run_before_it_put_the_launcher_back() {
        let (stand_in, game) = install("retry");
        let launcher = game.join("start_protected_game.exe");
        let first = mount(&stand_in, &game, &ace_combat(), signed).unwrap();
        std::thread::scope(|scope| {
            let retry = scope.spawn(|| mount(&stand_in, &game, &ace_combat(), signed).unwrap());
            std::thread::sleep(Duration::from_millis(100));
            drop(first);
            let retry = retry.join().unwrap();
            assert_eq!(fs::read(&launcher).unwrap(), STAND_IN, "the first restore left the retry's stand-in alone");
            assert!(beside(&launcher, CONFIG).exists());
            drop(retry);
        });
        assert_eq!(files(&game), ["start_protected_game.exe"]);
        fs::remove_dir_all(stand_in.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_refused_stand_in_or_a_missing_launcher_leaves_the_game_as_it_was() {
        let (stand_in, game) = install("refused");
        let launcher = game.join("start_protected_game.exe");
        let err = mount(&stand_in, &game, &ace_combat(), |path| bail!("{} is not signed", path.display())).err().unwrap();
        assert!(err.to_string().contains("not signed"), "{err}");
        assert_eq!(files(&game), ["start_protected_game.exe"]);
        assert_eq!(fs::read(&launcher).unwrap(), ORIGINAL);
        fs::remove_file(&launcher).unwrap();
        assert!(mount(&stand_in, &game, &ace_combat(), signed).is_err());
        assert!(files(&game).is_empty());
        fs::remove_dir_all(stand_in.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_leftover_it_cannot_account_for_is_refused_untouched() {
        let (stand_in, game) = install("leftover");
        let launcher = game.join("start_protected_game.exe");
        // A backup older than the launcher, e.g. the game updated since.
        fs::write(beside(&launcher, BACKUP), b"older launcher").unwrap();
        assert!(mount(&stand_in, &game, &ace_combat(), |_| bail!("unsigned")).is_err());
        assert_eq!(fs::read(beside(&launcher, BACKUP)).unwrap(), b"older launcher");
        assert_eq!(fs::read(&launcher).unwrap(), ORIGINAL);
        // The stand-in with nothing to put back.
        fs::remove_file(beside(&launcher, BACKUP)).unwrap();
        fs::write(&launcher, STAND_IN).unwrap();
        assert!(mount(&stand_in, &game, &ace_combat(), signed).is_err());
        assert_eq!(files(&game), ["start_protected_game.exe"]);
        fs::remove_dir_all(stand_in.parent().unwrap()).unwrap();
    }

    #[test]
    fn the_drop_after_the_stand_in_restored_the_launcher_removes_what_it_left() {
        let (stand_in, game) = install("restored");
        let launcher = game.join("start_protected_game.exe");
        let mounted = mount(&stand_in, &game, &ace_combat(), signed).unwrap();
        // What the stand-in does once the game runs.
        fs::rename(&launcher, beside(&launcher, LEFT)).unwrap();
        fs::rename(beside(&launcher, BACKUP), &launcher).unwrap();
        fs::remove_file(beside(&launcher, CONFIG)).unwrap();
        drop(mounted);
        assert_eq!(files(&game), ["start_protected_game.exe"]);
        assert_eq!(fs::read(&launcher).unwrap(), ORIGINAL);
        fs::remove_dir_all(stand_in.parent().unwrap()).unwrap();
    }

    /// Needs Wand installed: its own `GameLauncher.exe`, in place of a launcher, starts this test program as the
    /// game (listing its tests) and puts the launcher back.
    #[test]
    #[ignore]
    fn wands_stand_in_starts_the_game_and_restores_the_launcher() {
        let install = crate::install::installs().into_iter().next().expect("Wand is installed");
        let (_, game) = self::install("real");
        fs::create_dir_all(game.join("Game")).unwrap();
        fs::copy(env::current_exe().unwrap(), game.join("Game").join("game.exe")).unwrap();
        let launch =
            json!({"proxy": {"launcher": "start_protected_game.exe", "gameExe": "Game\\game.exe", "gameArgs": "--list", "flags": 8}});
        let proxy = Proxy::parse(&launch).unwrap().unwrap();
        let publishers = crate::install::PUBLISHERS;
        let mounted = mount(&install.game_launcher(), &game, &proxy, |path| agent::signed(path, publishers)).unwrap();
        let launcher = game.join("start_protected_game.exe");
        let status = Command::new(&launcher).env("SteamAppId", "480").current_dir(&game).status().unwrap();
        assert!(status.success(), "{status}");
        assert_eq!(fs::read(&launcher).unwrap(), ORIGINAL, "the stand-in put the launcher back");
        drop(mounted);
        assert_eq!(files(&game), ["Game", "start_protected_game.exe"]);
        fs::remove_dir_all(game.parent().unwrap()).unwrap();
    }
}
