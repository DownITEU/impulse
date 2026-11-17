//! `plugins.lock`: what the user approved for each installed plugin, and the user's settings beside it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::Values;

/// Keyed by source id (`plugin-<manifest id>`).
#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Lock {
    pub plugins: BTreeMap<String, Entry>,
}

#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    /// The folder under the plugins' folder.
    pub dir: String,
    pub version: String,
    pub enabled: bool,
    /// What the user approved.
    pub hosts: Vec<String>,
    pub links: Vec<String>,
    /// Sha256 by file name: `open` refuses a folder that differs.
    pub files: BTreeMap<String, String>,
    /// The user's settings by field id (P11).
    pub values: Values,
}
