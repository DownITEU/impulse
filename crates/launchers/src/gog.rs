use std::{env, path::PathBuf};

use super::{
    App,
    registry::{self, LOCAL_MACHINE, View},
};

pub const CLIENT: &str = "GalaxyClient.exe";

/// GOG Galaxy's folder, when its client is there.
pub fn launcher() -> Option<PathBuf> {
    let registered = registry::value(LOCAL_MACHINE, r"SOFTWARE\GOG.com\GalaxyClient\paths", "client", View::Wow32).map(PathBuf::from);
    let default = env::var_os("ProgramFiles(x86)").map(|programs| PathBuf::from(programs).join("GOG Galaxy"));
    [registered, default].into_iter().flatten().find(|dir| dir.join(CLIENT).is_file())
}

// Galaxy and offline installers both register here, so the Galaxy client is not required.
pub fn apps() -> Vec<App> {
    let Some(games) = registry::open(LOCAL_MACHINE, r"SOFTWARE\GOG.com\Games", View::Wow32) else {
        return Vec::new();
    };
    registry::children(&games, View::Wow32)
        .into_iter()
        .filter_map(|(_, key)| {
            let id: u64 = registry::string(&key, "gameID")?.parse().ok()?;
            let app = App::new(format!("gog:{id}"), registry::string(&key, "path")?.into());
            Some(app.named(registry::string(&key, "gameName").as_deref()))
        })
        .collect()
}
