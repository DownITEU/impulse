//! An installed plugin as the app shows and drives it: `plugins` makes these, `client` hands them to the UI.

use crate::{Failure, SourceId, Text};

#[derive(Clone, Debug)]
pub struct Installed {
    pub id: SourceId,
    pub name: String,
    pub version: String,
    /// Hosts it fetches from, as approved.
    pub hosts: Vec<String>,
    /// Hosts its links open.
    pub links: Vec<String>,
    pub enabled: bool,
    pub settings: Vec<Setting>,
    /// The user's value of each setting, in `settings` order; a bad or missing one is the default.
    pub values: Vec<Value>,
    /// Its answers kept on disk.
    pub bytes: u64,
    /// Why it did not open, else its last call's failure.
    pub failure: Option<Failure>,
}

/// What installing a pick would grant, shown before it is installed.
#[derive(Clone, Debug, PartialEq)]
pub struct Approval {
    pub id: SourceId,
    pub name: String,
    pub version: String,
    pub hosts: Vec<String>,
    pub links: Vec<String>,
    /// The installed version it replaces.
    pub replaces: Option<String>,
    /// Hosts and links the installed version was not approved for.
    pub widened: Vec<String>,
    /// The pick's files: installing refuses a pick changed since.
    pub digest: String,
}

/// A setting the plugin declares; its texts are messages of the plugin's catalog.
#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    pub id: String,
    pub kind: SettingKind,
    pub label: Text,
    pub hint: Option<Text>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SettingKind {
    Switch,
    Text {
        max_length: u32,
    },
    Number {
        min: f64,
        max: f64,
        step: f64,
    },
    /// Option ids with their labels.
    Choice {
        options: Vec<(String, Text)>,
    },
}

/// A setting's value; a choice is the option's id.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Switch(bool),
    Text(String),
    Number(f64),
}
