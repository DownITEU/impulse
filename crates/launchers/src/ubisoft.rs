use std::path::PathBuf;

use super::{
    App,
    registry::{self, Key, LOCAL_MACHINE, View},
};

const INSTALLS: &str = r"SOFTWARE\Ubisoft\Launcher\Installs";
pub const LAUNCHER: &str = "UbisoftGameLauncher.exe";

/// Ubisoft Connect's folder, when its game launcher is there.
pub fn launcher() -> Option<PathBuf> {
    let dir = PathBuf::from(registry::value(LOCAL_MACHINE, r"SOFTWARE\Ubisoft\Launcher", "InstallDir", View::Wow32)?);
    dir.join(LAUNCHER).is_file().then_some(dir)
}

pub fn apps() -> Vec<App> {
    let Some(installs) = registry::open(LOCAL_MACHINE, INSTALLS, View::Wow32) else {
        return Vec::new();
    };
    registry::children(&installs, View::Wow32)
        .into_iter()
        .filter_map(|(id, key)| {
            let location = install_dir(&id, &key)?;
            Some(App::new(format!("uplay:{id}"), location))
        })
        .collect()
}

pub fn location(id: &str) -> Option<PathBuf> {
    let key = registry::open(LOCAL_MACHINE, &format!(r"{INSTALLS}\{id}"), View::Wow32)?;
    install_dir(id, &key)
}

// Uninstalled games keep their key; `uplay_install.state` marks a real install.
fn install_dir(id: &str, key: &Key) -> Option<PathBuf> {
    id.parse::<u32>().ok()?;
    let dir = PathBuf::from(registry::string(key, "InstallDir")?.replace('/', "\\"));
    dir.join("uplay_install.state").is_file().then_some(dir)
}
