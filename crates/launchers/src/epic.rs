use std::{env, fs, path::PathBuf};

use serde::Deserialize;

use super::{
    App,
    registry::{LOCAL_MACHINE, View, value},
    ubisoft,
};

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Item {
    install_location: String,
    catalog_namespace: String,
    catalog_item_id: String,
    app_name: String,
    #[serde(default)]
    display_name: String,
    app_categories: Vec<String>,
    #[serde(rename = "bIsIncompleteInstall", default)]
    incomplete: bool,
    #[serde(default)]
    launch_executable: String,
    #[serde(default)]
    launch_command: String,
}

pub fn apps() -> Vec<App> {
    let Some(dir) = data_dir() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(dir.join("Manifests")) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("item")))
        .filter_map(|e| serde_json::from_slice::<Item>(&fs::read(e.path()).ok()?).ok())
        .filter(|item| !item.incomplete && item.app_categories.iter().any(|c| c == "games"))
        .flat_map(|item| {
            let location = redirect(&item).unwrap_or_else(|| item.install_location.clone().into());
            // The catalog lists both the full id and the bare app name.
            [format!("epic:{}:{}:{}", item.catalog_namespace, item.catalog_item_id, item.app_name), format!("epic:{}", item.app_name)].map(
                |store_id| App {
                    store_id,
                    name: Some(item.display_name.clone()).filter(|name| !name.is_empty()),
                    location: location.clone(),
                },
            )
        })
        .collect()
}

fn data_dir() -> Option<PathBuf> {
    let registered = value(LOCAL_MACHINE, r"SOFTWARE\Epic Games\EpicGamesLauncher", "AppDataPath", View::Wow32).map(PathBuf::from);
    let default = env::var_os("ProgramData").map(|p| PathBuf::from(p).join(r"Epic\EpicGamesLauncher\Data"));
    [registered, default].into_iter().flatten().find(|p| p.is_dir())
}

// Ubisoft games sold on Epic install through Ubisoft Connect, which owns the real folder.
fn redirect(item: &Item) -> Option<PathBuf> {
    if !item.launch_executable.eq_ignore_ascii_case("uplaylaunch.exe") {
        return None;
    }
    let command = item.launch_command.to_ascii_lowercase();
    let id = command.split("-uplayid=").nth(1)?;
    let id: String = id.chars().take_while(char::is_ascii_digit).collect();
    ubisoft::location(&id)
}
