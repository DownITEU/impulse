//! The user's preferences in `settings.json`, kept here alone, and the SETTINGS dialog. The accent and
//! reduce motion apply to every window.

mod folders;
mod view;

use gpui_kit::{App, Context};
use serde::{Deserialize, Serialize};

use client::storage::{Paths, Stored};
pub(super) use folders::{Folders, Shown};
pub(super) use view::{Tab, label, row};

use super::theme::{ACCENTS, Look};

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
    /// The game the user opened last.
    pub game: Option<String>,
    /// Off, the default: the close button hides Impulse in the tray.
    pub quit_on_close: bool,
    /// Screenshots, recordings and streams show no Impulse window.
    pub hide_from_capture: bool,
}

impl Prefs {
    pub fn accent(&self) -> u32 {
        ACCENTS.iter().find(|(name, _)| *name == self.accent).unwrap_or(&ACCENTS[0]).1
    }

    pub fn set_accent(&mut self, color: u32) {
        self.accent = ACCENTS.iter().find(|(_, other)| *other == color).map_or_else(String::new, |(name, _)| (*name).to_owned());
    }
}

pub struct Settings {
    prefs: Stored<Prefs>,
}

impl Settings {
    pub fn new(paths: &Paths, cx: &mut Context<Self>) -> Self {
        let settings = Self { prefs: Stored::load(paths.settings()) };
        settings.apply(cx);
        settings
    }

    pub fn prefs(&self) -> &Prefs {
        &self.prefs
    }

    /// Changes the prefs and saves them.
    pub fn edit(&mut self, change: impl FnOnce(&mut Prefs), cx: &mut Context<Self>) {
        self.prefs.edit(change);
        self.apply(cx);
        cx.notify();
    }

    fn apply(&self, cx: &mut App) {
        let accent = self.prefs.accent();
        if cx.try_global::<Look>().is_none_or(|look| look.accent != accent) {
            cx.set_global(Look { accent });
            // Cached views draw again only when refreshed.
            cx.refresh_windows();
        }
        cx.set_reduce_motion(self.prefs.reduce_motion);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_and_unknown_accents_mean_the_defaults() {
        let prefs: Prefs = serde_json::from_str(r#"{"accent":"PINK","lead":"thumbnail"}"#).unwrap();
        assert!(prefs.accent() == ACCENTS[0].1 && prefs.lead == Lead::Thumbnail && prefs.panels == Panels::Top);
        let mut cyan: Prefs = serde_json::from_str(r#"{"accent":"CYAN"}"#).unwrap();
        assert_eq!(cyan.accent(), ACCENTS[2].1);
        cyan.set_accent(ACCENTS[1].1);
        assert_eq!(cyan.accent, "AMBER");
    }
}
