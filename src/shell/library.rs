//! Temporary text report of the catalog and installed games; the real page replaces it.

use std::{collections::BTreeMap, path::Path};

use anyhow::Result;

use crate::{
    catalog::{self, Freshness},
    library,
};

pub fn scan(data_dir: &Path) -> Result<String> {
    let (catalog, freshness) = catalog::load(data_dir)?;
    let installed = library::scan(&catalog);
    let mut per_store = BTreeMap::<&str, usize>::new();
    for game in &installed {
        let store = game.store_id.split(':').next().unwrap_or_default();
        *per_store.entry(store).or_default() += 1;
        let title = &catalog.titles[&catalog.games[&game.game_id].title_id].name;
        let version = game.version.map_or("?".to_owned(), |v| v.to_string());
        let state = match game.supported {
            Some(true) => "supported",
            Some(false) => "UPDATE PENDING",
            None => "-",
        };
        eprintln!(
            "  {title} [{}] v{version} {state} @ {}",
            game.store_id,
            game.location.display()
        );
    }
    let stores: Vec<String> = per_store.iter().map(|(s, n)| format!("{s} {n}")).collect();
    let catalog_state = match freshness {
        Freshness::Downloaded => "catalog downloaded".to_owned(),
        Freshness::NotModified => "catalog up to date".to_owned(),
        Freshness::Offline(why) => format!("catalog offline ({why})"),
    };
    Ok(format!(
        "{} installed: {} ({catalog_state})",
        installed.len(),
        stores.join(", ")
    ))
}
