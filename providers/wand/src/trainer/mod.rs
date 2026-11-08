//! Trainer metadata: the cheat list of a game. Kept on disk, so a game that was
//! seen once works offline and costs no request until its trainer changes.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use blocking::unblock;
use hotkeys::Chord;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{session::Session, transport::Transport};

mod plan;

pub use plan::Plan;

const DIR: &str = "trainers";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trainer {
    blueprint: Blueprint,
    /// Cheat uuids the user pinned in Wand.
    #[serde(default)]
    pinned_mods: Vec<String>,
    // What running it takes: read by `plan`.
    loader: Option<String>,
    #[serde(default)]
    loader_args: LoaderArgs,
    #[serde(default)]
    game_events: Vec<Value>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoaderArgs {
    binary_url: Option<String>,
    binary_hash: Option<String>,
}

#[derive(Deserialize)]
struct Blueprint {
    cheats: Vec<Cheat>,
    // Free-form on purpose: a shape we do not read must not make the trainer unreadable.
    #[serde(default)]
    config: Value,
}

#[derive(Deserialize)]
struct Cheat {
    uuid: String,
    name: String,
    category: String,
    /// The variable the trainer exposes for this cheat.
    #[serde(default)]
    target: String,
    #[serde(rename = "type")]
    kind: Kind,
    /// Key chords as Windows virtual-key codes.
    #[serde(default)]
    hotkeys: Vec<Vec<u16>>,
    #[serde(default)]
    args: Args,
    instructions: Option<String>,
    description: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Toggle,
    Number,
    Slider,
    Scalar,
    Incremental,
    Button,
    Selection,
    #[serde(other)]
    Unknown,
}

#[derive(Default, Deserialize)]
struct Args {
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
    /// Each is a string, a number or `{label, value}`.
    #[serde(default)]
    options: Vec<Value>,
    /// A label or `true` when the value is applied with a button.
    button: Option<Value>,
    /// What a button cheat sends when pressed; 1 (a toggle on) without it.
    value: Option<Value>,
}

impl From<Trainer> for model::Trainer {
    fn from(trainer: Trainer) -> Self {
        Self { cheats: trainer.blueprint.cheats.into_iter().map(cheat).collect(), pinned: trainer.pinned_mods }
    }
}

fn cheat(cheat: Cheat) -> model::Cheat {
    let kind = match cheat.kind {
        Kind::Toggle => model::Kind::Toggle,
        Kind::Number => model::Kind::Number,
        Kind::Slider => model::Kind::Slider,
        Kind::Scalar => model::Kind::Scalar,
        Kind::Incremental => model::Kind::Incremental,
        Kind::Button => model::Kind::Button,
        Kind::Selection => model::Kind::Selection,
        Kind::Unknown => model::Kind::Unknown,
    };
    let args = cheat.args;
    let text = |value: &Value| match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    let options = args.options.iter().map(|option| match option.get("label").filter(|l| l.is_string()) {
        Some(label) => text(label),
        None => text(option.get("value").unwrap_or(option)),
    });
    model::Cheat {
        id: cheat.uuid,
        key: Some(cheat.target).filter(|target| !target.is_empty()),
        name: cheat.name,
        category: cheat.category,
        kind,
        args: model::Args {
            min: args.min,
            max: args.max,
            step: args.step,
            options: options.collect(),
            button: !matches!(args.button, None | Some(Value::Null | Value::Bool(false))),
            pressed: args.value.as_ref().and_then(Value::as_f64).unwrap_or(1.),
        },
        hotkeys: cheat.hotkeys.iter().map(|vks| Chord::from_vks(vks)).collect(),
        instructions: cheat.instructions,
        description: cheat.description,
    }
}

/// What a stored copy was fetched for. A copy with another stamp is stale.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Stamp {
    /// The catalog's `updatedAt` of the game's trainer.
    pub updated_at: Option<String>,
    /// The installed game version the server picked the trainer for.
    pub version: Option<u32>,
}

// The response is stored as received: the trainer host needs fields this struct skips.
#[derive(Serialize, Deserialize)]
struct Stored {
    stamp: Stamp,
    trainer: Value,
}

/// The disk copy and the stamp it was fetched for (blocking).
pub fn cached(data_dir: &Path, game_id: &str) -> Option<(Trainer, Stamp)> {
    let stored: Stored = serde_json::from_slice(&fs::read(file(data_dir, game_id)).ok()?).ok()?;
    Some((Trainer::deserialize(&stored.trainer).ok()?, stored.stamp))
}

pub fn forget(data_dir: &Path) {
    let _ = fs::remove_dir_all(data_dir.join(DIR));
}

/// Asks the API for the trainer that fits the installed version and stores it.
pub async fn fetch(
    session: &Session,
    t: &Transport,
    data_dir: &Path,
    game_id: &str,
    stamp: Stamp,
) -> Result<Trainer> {
    #[derive(Deserialize)]
    struct Envelope {
        trainer: Value,
    }
    let versions = stamp.version.map(|v| v.to_string()).unwrap_or_default();
    let path = format!(
        "/v3/games/{game_id}/trainer?gameVersions={versions}&locale=en-US&supportedLoaders=artifacts&v=3"
    );
    let Envelope { trainer: raw } = session.get(t, &path).await?;
    let trainer = Trainer::deserialize(&raw).context("unexpected trainer format")?;
    let file = file(data_dir, game_id);
    let stored = Stored {
        stamp,
        trainer: raw,
    };
    let saved = unblock(move || disk::replace(&file, &serde_json::to_vec(&stored)?)).await;
    if let Err(e) = saved {
        log::warn!("trainer {game_id} not cached: {e:#}");
    }
    Ok(trainer)
}

// Ids come from the catalog, a remote file: nothing but letters and digits reaches the path.
fn file(data_dir: &Path, game_id: &str) -> PathBuf {
    let name: String = game_id
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    data_dir.join(DIR).join(format!("{name}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{
        "blueprint": {"cheats": [
            {"uuid": "a", "name": "Health", "category": "player", "type": "toggle", "hotkeys": [[16, 112]]},
            {"uuid": "b", "name": "Crit", "category": "player", "type": "number",
             "hotkeys": [[18, 117], [16, 117]], "args": {"min": 0, "max": 1000, "step": 10, "button": "set"}},
            {"uuid": "c", "name": "Time", "category": "world", "type": "selection",
             "args": {"options": ["Noon", 2, {"label": "Night", "value": 3}, {"value": "Dawn"}]}},
            {"uuid": "d", "name": "New", "category": "world", "type": "from-the-future", "args": {"value": 10}}
        ]},
        "pinnedMods": ["b"],
        "loaderArgs": {"binaryHash": "kept"}
    }"#;

    #[test]
    fn parses_every_control_shape() {
        let trainer: model::Trainer = serde_json::from_str::<Trainer>(JSON).unwrap().into();
        let cheats = &trainer.cheats;
        assert!(cheats[0].kind == model::Kind::Toggle && !cheats[0].args.button && cheats[0].key.is_none());
        assert_eq!(cheats[1].hotkeys, [Chord::from_vks(&[18, 117]), Chord::from_vks(&[16, 117])]);
        assert!(cheats[1].args.button && cheats[1].args.step == Some(10.));
        assert_eq!(cheats[2].args.options, ["Noon", "2", "Night", "Dawn"]);
        assert_eq!((cheats[2].args.pressed, cheats[3].args.pressed), (1., 10.));
        assert!(cheats[3].kind == model::Kind::Unknown);
        assert_eq!(trainer.pinned, ["b"]);
    }

    #[test]
    fn disk_copy_keeps_the_whole_response() {
        let dir = std::env::temp_dir().join(format!("impulse-trainer-{}", std::process::id()));
        let stamp = Stamp {
            updated_at: Some("2026-01-01T00:00:00Z".to_owned()),
            version: Some(7),
        };
        let stored = Stored {
            stamp: stamp.clone(),
            trainer: serde_json::from_str(JSON).unwrap(),
        };
        let path = file(&dir, "../82102");
        assert!(path.ends_with("trainers/82102.json"));
        disk::replace(&path, &serde_json::to_vec(&stored).unwrap()).unwrap();
        let (trainer, found) = cached(&dir, "82102").unwrap();
        assert!(found == stamp && trainer.blueprint.cheats.len() == 4);
        assert!(fs::read_to_string(&path).unwrap().contains("binaryHash"));
        fs::remove_dir_all(&dir).unwrap();
        assert!(cached(&dir, "82102").is_none());
    }
}
