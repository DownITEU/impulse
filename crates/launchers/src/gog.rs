use super::{
    App,
    registry::{self, LOCAL_MACHINE, View},
};

// Galaxy and offline installers both register here, so the Galaxy client is not required.
pub fn apps() -> Vec<App> {
    let Some(games) = registry::open(LOCAL_MACHINE, r"SOFTWARE\GOG.com\Games", View::Wow32) else {
        return Vec::new();
    };
    registry::children(&games, View::Wow32)
        .into_iter()
        .filter_map(|(_, key)| {
            let id: u64 = registry::string(&key, "gameID")?.parse().ok()?;
            Some(App { store_id: format!("gog:{id}"), location: registry::string(&key, "path")?.into() })
        })
        .collect()
}
