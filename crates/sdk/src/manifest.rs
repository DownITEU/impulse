//! `manifest.json`, beside the module: who the plugin is, what it may reach, what the user can set.
//! The format only: the host checks every field when it installs and when it opens a plugin.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Manifest {
    /// `[a-z0-9-]`, at most 25 bytes; the plugin's source is `plugin-<id>`.
    pub id: String,
    pub name: String,
    pub version: String,
    /// The wire version it speaks: `crate::API`.
    pub api: u32,
    pub kind: Kind,
    /// The module's file name, beside the manifest.
    pub module: String,
    /// Hosts it fetches from, over https only: `api.example.com`, or `*.example.com` for that domain and below it.
    #[serde(default)]
    pub hosts: Vec<String>,
    /// Hosts a web link in its texts may open, in the same forms.
    #[serde(default)]
    pub links: Vec<String>,
    /// The content locales it serves, BCP 47 tags; the first is asked when none is closer.
    pub locales: Vec<String>,
    /// Days an answer without a revision stays fresh.
    #[serde(default = "week")]
    pub refresh: u32,
    #[serde(default)]
    pub settings: Vec<Field>,
}

fn week() -> u32 {
    7
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Maps,
}

/// A setting the host draws on the plugin's page and hands to every call; the guest cannot write it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Field {
    pub id: String,
    #[serde(flatten)]
    pub kind: FieldKind,
    /// A value of the field's kind.
    pub default: Value,
    /// Message ids of the plugin's catalog.
    pub label: String,
    #[serde(default)]
    pub hint: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum FieldKind {
    /// A boolean.
    Switch,
    /// A string of at most `max_length` characters.
    Text { max_length: u32 },
    /// A number from `min` to `max`, a whole number of `step`s from `min`.
    Number { min: f64, max: f64, step: f64 },
    /// One of the options' ids.
    Choice { options: Vec<Choice> },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Choice {
    pub id: String,
    /// A message id of the plugin's catalog.
    pub label: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_setting_reads_its_kind_beside_its_other_fields() {
        let field: Field =
            serde_json::from_str(r#"{"id": "level", "kind": "number", "min": 1, "max": 9, "step": 1, "default": 5, "label": "level"}"#)
                .unwrap();
        assert_eq!((field.kind, field.default, field.hint), (FieldKind::Number { min: 1., max: 9., step: 1. }, Value::from(5), None));
    }
}
