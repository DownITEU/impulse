//! An installed plugin as `model` shows it: settings, values and the rest of `Installed`.

use model::{Failure, Installed, Setting, SettingKind, SourceId, Text, Value};
use sdk::manifest::{Field, FieldKind, Manifest};

use super::lock::Entry;
use crate::settings;

pub fn installed(source: &SourceId, entry: &Entry, manifest: &Manifest, bytes: u64, failure: Option<Failure>) -> Installed {
    let resolved = settings::resolve(&manifest.settings, &entry.values);
    Installed {
        id: source.clone(),
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        hosts: entry.hosts.clone(),
        links: entry.links.clone(),
        enabled: entry.enabled,
        settings: manifest.settings.iter().map(|field| setting(source, field)).collect(),
        values: manifest.settings.iter().filter_map(|field| resolved.get(&field.id)).map(value).collect(),
        bytes,
        failure,
    }
}

/// A plugin whose manifest no longer reads: its id for a name, and why.
pub fn unreadable(source: &SourceId, entry: &Entry, bytes: u64, failure: Failure) -> Installed {
    Installed {
        id: source.clone(),
        name: source.as_str().to_owned(),
        version: entry.version.clone(),
        hosts: entry.hosts.clone(),
        links: entry.links.clone(),
        enabled: entry.enabled,
        settings: Vec::new(),
        values: Vec::new(),
        bytes,
        failure: Some(failure),
    }
}

fn setting(source: &SourceId, field: &Field) -> Setting {
    let kind = match &field.kind {
        FieldKind::Switch => SettingKind::Switch,
        FieldKind::Text { max_length } => SettingKind::Text { max_length: *max_length },
        FieldKind::Number { min, max, step } => SettingKind::Number { min: *min, max: *max, step: *step },
        FieldKind::Choice { options } => {
            SettingKind::Choice { options: options.iter().map(|option| (option.id.clone(), Text::key(source, &option.label))).collect() }
        }
    };
    Setting {
        id: field.id.clone(),
        kind,
        label: Text::key(source, &field.label),
        hint: field.hint.as_deref().map(|hint| Text::key(source, hint)),
    }
}

/// A value `fits` accepted, as the UI takes it.
fn value(json: &serde_json::Value) -> Value {
    match json {
        serde_json::Value::Bool(on) => Value::Switch(*on),
        serde_json::Value::Number(number) => Value::Number(number.as_f64().unwrap_or_default()),
        serde_json::Value::String(text) => Value::Text(text.clone()),
        _ => Value::Text(String::new()),
    }
}

/// The JSON `settings::fits` checks; `None` for a number that is not finite.
pub fn to_json(value: &Value) -> Option<serde_json::Value> {
    Some(match value {
        Value::Switch(on) => (*on).into(),
        Value::Text(text) => text.clone().into(),
        // A whole number stays whole: it must equal the default's when they match.
        Value::Number(number) if number.fract() == 0. && number.abs() < 9e15 => (*number as i64).into(),
        Value::Number(number) => serde_json::Number::from_f64(*number)?.into(),
    })
}
