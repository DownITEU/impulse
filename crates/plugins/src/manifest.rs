//! A plugin's manifest as the host trusts it: read and checked when the plugin opens.

use std::{collections::HashSet, path::Path};

use anyhow::{Context, Result, bail, ensure};
use model::{Failure, FailureKind, Locale, SourceId};
use sdk::manifest::{FieldKind, Manifest};

use crate::{
    scope::{Hosts, Scope},
    settings,
};

const ID: usize = 25;
const MOST_SETTINGS: usize = 32;
const MANIFEST: u64 = 64 << 10;

/// The manifest in `dir` and the scope it grants.
pub fn read(dir: &Path) -> Result<(Manifest, Scope)> {
    let file = dir.join("manifest.json");
    let bytes = crate::read(&file, MANIFEST)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("{} is not a manifest", file.display()))
        .context(Failure::new(FailureKind::Refused))?;
    if manifest.api != sdk::API {
        return Err(anyhow::anyhow!("speaks wire v{}, this app v{}", manifest.api, sdk::API))
            .context(Failure::new(FailureKind::Unsupported));
    }
    let scope = check(&manifest).with_context(|| format!("manifest of {:?}", manifest.id)).context(Failure::new(FailureKind::Refused))?;
    Ok((manifest, scope))
}

fn check(manifest: &Manifest) -> Result<Scope> {
    let plain = |text: &str, most: usize| {
        !text.is_empty() && text.len() <= most && text.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    };
    ensure!(plain(&manifest.id, ID), "id {:?}: [a-z0-9-], at most {ID} bytes", manifest.id);
    let source = SourceId::new(&format!("plugin-{}", manifest.id)).context("id")?;
    ensure!(!manifest.name.is_empty() && manifest.name.len() <= 64, "name");
    ensure!(!manifest.version.is_empty() && manifest.version.len() <= 32, "version");
    // A file beside the manifest, never a path out of the plugin's folder.
    let module = &manifest.module;
    ensure!(
        module.len() <= 64 && module.ends_with(".wasm") && !module.contains(['/', '\\', ':']) && !module.starts_with('.'),
        "module {module:?}"
    );
    ensure!(
        !manifest.locales.is_empty()
            && manifest
                .locales
                .iter()
                .all(|tag| tag.len() <= 35 && !tag.is_empty() && tag.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')),
        "locales"
    );
    ensure!(manifest.refresh >= 1, "refresh of 0 days");
    ensure!(manifest.settings.len() <= MOST_SETTINGS, "more than {MOST_SETTINGS} settings");
    let mut ids = HashSet::new();
    for field in &manifest.settings {
        ensure!(plain(&field.id, 64) && ids.insert(&field.id), "setting id {:?}", field.id);
        ensure!(!field.label.is_empty(), "setting {:?}: no label", field.id);
        match &field.kind {
            FieldKind::Switch => {}
            FieldKind::Text { max_length } => ensure!((1..=4096).contains(max_length), "setting {:?}: max_length", field.id),
            FieldKind::Number { min, max, step } => {
                ensure!(min.is_finite() && max.is_finite() && min <= max && *step > 0. && step.is_finite(), "setting {:?}: range", field.id)
            }
            FieldKind::Choice { options } => {
                let mut seen = HashSet::new();
                ensure!(
                    !options.is_empty() && options.iter().all(|option| plain(&option.id, 64) && seen.insert(&option.id)),
                    "setting {:?}: options",
                    field.id
                )
            }
        }
        if !settings::fits(field, &field.default) {
            bail!("setting {:?}: a default outside its field", field.id);
        }
    }
    Ok(Scope { source, hosts: Hosts::parse(&manifest.hosts).context("hosts")?, links: Hosts::parse(&manifest.links).context("links")? })
}

/// The locale of `served` closest to `asked`: the same tag, else the same language, else the first.
pub fn closest<'a>(served: &'a [String], asked: &Locale) -> &'a str {
    let language = |tag: &str| tag.split('-').next().unwrap_or_default().to_ascii_lowercase();
    let exact = served.iter().find(|tag| tag.eq_ignore_ascii_case(asked.as_str()));
    let near = || served.iter().find(|tag| language(tag) == language(asked.as_str()));
    exact.or_else(near).or(served.first()).map_or("", String::as_str)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn manifest(change: impl FnOnce(&mut serde_json::Value)) -> Result<Scope> {
        let mut json = json!({
            "id": "maps", "name": "Maps", "version": "1.0", "api": 1, "kind": "maps", "module": "maps.wasm",
            "hosts": ["*.tiles.io"], "locales": ["en", "pt-BR"],
            "settings": [{"id": "level", "kind": "number", "min": 1, "max": 9, "step": 1, "default": 5, "label": "level"}]
        });
        change(&mut json);
        check(&serde_json::from_value(json)?)
    }

    #[test]
    fn a_manifest_names_a_plain_id_a_module_beside_it_and_defaults_that_fit() {
        assert_eq!(manifest(|_| {}).unwrap().source.as_str(), "plugin-maps");
        type Breaks = (&'static str, fn(&mut serde_json::Value));
        let broken: [Breaks; 6] = [
            ("an id with capitals", |json| json["id"] = json!("Maps")),
            ("an id too long", |json| json["id"] = json!("a".repeat(26))),
            ("a module in another folder", |json| json["module"] = json!("../other/maps.wasm")),
            ("a default outside its field", |json| json["settings"][0]["default"] = json!(10)),
            ("a bad host", |json| json["hosts"] = json!(["*"])),
            ("no locale", |json| json["locales"] = json!([])),
        ];
        for (what, breaks) in broken {
            assert!(manifest(breaks).is_err(), "{what}");
        }
    }

    #[test]
    fn the_closest_locale_is_the_tag_then_the_language_then_the_first() {
        let served = ["en".to_owned(), "pt-BR".to_owned()];
        assert_eq!(closest(&served, &Locale::new("pt-br")), "pt-BR");
        assert_eq!(closest(&served, &Locale::new("pt-PT")), "pt-BR");
        assert_eq!(closest(&served, &Locale::new("fr")), "en");
    }
}
