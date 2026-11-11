//! `settings.json`: how the page looks, and the game it opens with.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{Page, store, theme::ACCENTS};

const FILE: &str = "settings.json";

/// The leading item of a sidebar row.
#[derive(Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lead {
    #[default]
    Number,
    Thumbnail,
}

/// Where the map and session panels sit.
#[derive(Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Panels {
    /// In a row over the cheat list.
    #[default]
    Top,
    /// In a column beside it.
    Side,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// A name of `ACCENTS`; an unknown one means the first.
    accent: String,
    pub lead: Lead,
    pub panels: Panels,
    pub reduce_motion: bool,
    /// The game open last.
    pub game: Option<String>,
}

impl Prefs {
    pub fn load(data_dir: &Path) -> Self {
        store::load(&data_dir.join(FILE)).unwrap_or_default()
    }

    pub fn accent(&self) -> u32 {
        ACCENTS.iter().find(|(name, _)| *name == self.accent).unwrap_or(&ACCENTS[0]).1
    }
}

impl Page {
    pub(super) fn save_prefs(&self) {
        let accent = ACCENTS.iter().find(|(_, color)| *color == self.accent).map_or_else(String::new, |(name, _)| (*name).to_owned());
        let prefs = Prefs {
            accent,
            lead: self.lead,
            panels: self.panels,
            reduce_motion: self.reduce_motion,
            game: self.selected.as_deref().map(str::to_owned),
        };
        store::save(&self.data_dir.join(FILE), &prefs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_and_unknown_accents_mean_the_defaults() {
        let prefs: Prefs = serde_json::from_str(r#"{"accent":"PINK","lead":"thumbnail"}"#).unwrap();
        assert!(prefs.accent() == ACCENTS[0].1 && prefs.lead == Lead::Thumbnail && prefs.panels == Panels::Top);
        let cyan: Prefs = serde_json::from_str(r#"{"accent":"CYAN"}"#).unwrap();
        assert_eq!(cyan.accent(), ACCENTS[2].1);
    }
}
