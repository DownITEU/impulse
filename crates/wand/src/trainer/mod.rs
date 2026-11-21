//! Trainer metadata: the cheat list of a game. Kept on disk, so a game that was
//! seen once works offline and costs no request until its trainer changes.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use blocking::unblock;
use model::Chord;
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{DeserializeOwned, Error as _},
};
use serde_json::Value;

use crate::{session::Session, texts, transport::Transport};

mod plan;

pub use plan::{Binary, Plan};

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
    #[serde(default, deserialize_with = "object_or_default")]
    loader_args: LoaderArgs,
    #[serde(default)]
    game_events: Vec<Value>,
    /// The answer's `i18n.strings`: the English text of a name, description or instruction to its translation.
    #[serde(skip)]
    strings: HashMap<String, String>,
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
    #[serde(default)]
    flags: u32,
    // Free-form on purpose: a shape we do not read must not make the trainer unreadable.
    #[serde(default)]
    config: Value,
    /// What to read before playing, markdown.
    #[serde(default)]
    notes: Value,
}

// Bits of `Blueprint::flags`.
const TELEPORTS: u32 = 4;
const LOCATES: u32 = 8;

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
    #[serde(default, deserialize_with = "object_or_default")]
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

/// Wand sends an empty object as `[]`: anything but an object is the defaults.
fn object_or_default<'de, D: Deserializer<'de>, T: DeserializeOwned + Default>(deserializer: D) -> Result<T, D::Error> {
    match Value::deserialize(deserializer)? {
        object @ Value::Object(_) => T::deserialize(object).map_err(D::Error::custom),
        _ => Ok(T::default()),
    }
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

impl Trainer {
    pub fn into_model(self, plan: model::Plan) -> model::Trainer {
        let flags = self.blueprint.flags;
        let strings = self.strings;
        model::Trainer {
            notes: notes(&self.blueprint.notes, &strings),
            cheats: self.blueprint.cheats.into_iter().map(|c| cheat(c, &strings)).collect(),
            pinned: self.pinned_mods,
            location: flags & LOCATES != 0,
            teleport: flags & TELEPORTS != 0,
            plan,
        }
    }
}

/// The trainer of an answer, with its translations; a bad `i18n` gives none.
fn read(Answer { trainer, i18n }: &Answer) -> Result<Trainer, serde_json::Error> {
    let mut trainer = Trainer::deserialize(trainer)?;
    if let Some(strings) = i18n.get("strings").and_then(Value::as_object) {
        let text = |(english, text): (&String, &Value)| Some((english.clone(), text.as_str()?.to_owned()));
        trainer.strings = strings.iter().filter_map(text).collect();
    }
    Ok(trainer)
}

/// The notes in the answer's language; blank ones are none.
fn notes(notes: &Value, strings: &HashMap<String, String>) -> Option<String> {
    let notes = notes.as_str()?;
    let notes = strings.get(notes).map_or(notes, String::as_str).trim();
    (!notes.is_empty()).then(|| notes.to_owned())
}

fn cheat(cheat: Cheat, strings: &HashMap<String, String>) -> model::Cheat {
    let translate = |text: String| strings.get(&text).cloned().unwrap_or(text);
    // A scalar is shown as a slider, an incremental as a number without a text field.
    let typed = matches!(cheat.kind, Kind::Number);
    let kind = match cheat.kind {
        Kind::Toggle => model::Kind::Toggle,
        Kind::Number | Kind::Incremental => model::Kind::Number,
        Kind::Slider | Kind::Scalar => model::Kind::Slider,
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
        name: translate(cheat.name),
        category_name: texts::category(&cheat.category),
        category: cheat.category,
        kind,
        args: model::Args {
            min: args.min,
            max: args.max,
            step: args.step,
            options: options.collect(),
            button: typed && !matches!(args.button, None | Some(Value::Null | Value::Bool(false))),
            pressed: args.value.as_ref().and_then(Value::as_f64).unwrap_or(1.),
        },
        hotkeys: cheat.hotkeys.iter().map(|vks| Chord::from_vks(vks)).collect(),
        instructions: cheat.instructions.map(translate),
        description: cheat.description.map(translate),
    }
}

/// What a stored copy was fetched for. A copy with another stamp is stale.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Stamp {
    /// The catalog's `updatedAt` of the game's trainer.
    pub updated_at: Option<String>,
    /// The installed game version the server picked the trainer for.
    pub version: Option<u32>,
    /// The content locale Wand was asked for (one it serves).
    #[serde(default = "crate::locale::english")]
    pub locale: String,
}

/// The API's answer, stored as received: the trainer host needs fields `Trainer` skips.
#[derive(Serialize, Deserialize)]
struct Answer {
    trainer: Value,
    #[serde(default)]
    i18n: Value,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    stamp: Stamp,
    #[serde(flatten)]
    answer: Answer,
}

/// The disk copy and the stamp it was fetched for (blocking).
pub fn cached(data_dir: &Path, game_id: &str) -> Option<(Trainer, Stamp)> {
    let stored: Stored = serde_json::from_slice(&fs::read(file(data_dir, game_id)).ok()?).ok()?;
    Some((read(&stored.answer).ok()?, stored.stamp))
}

pub fn size(data_dir: &Path) -> u64 {
    disk::size(&data_dir.join(DIR))
}

pub fn forget(data_dir: &Path) {
    let _ = fs::remove_dir_all(data_dir.join(DIR));
}

/// Asks the API for the trainer that fits the installed version and stores it.
pub async fn fetch(session: &Session, t: &Transport, data_dir: &Path, game_id: &str, stamp: Stamp) -> Result<Trainer> {
    let versions = stamp.version.map(|v| v.to_string()).unwrap_or_default();
    let path = format!("/v3/games/{game_id}/trainer?gameVersions={versions}&locale={}&supportedLoaders=artifacts&v=3", stamp.locale);
    let answer: Answer = session.get(t, &path).await?;
    let trainer = read(&answer).context("unexpected trainer format")?;
    let file = file(data_dir, game_id);
    let stored = Stored { stamp, answer };
    let saved = unblock(move || disk::replace(&file, &serde_json::to_vec(&stored)?)).await;
    if let Err(e) = saved {
        log::warn!("trainer {game_id} not cached: {e:#}");
    }
    Ok(trainer)
}

// Ids come from the catalog, a remote file: nothing but letters and digits reaches the path.
fn file(data_dir: &Path, game_id: &str) -> PathBuf {
    let name: String = game_id.chars().filter(char::is_ascii_alphanumeric).collect();
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
            {"uuid": "d", "name": "New", "category": "world", "type": "from-the-future", "args": {"value": 10}},
            {"uuid": "e", "name": "Gold", "category": "world", "type": "incremental", "args": {"button": "add"}},
            {"uuid": "f", "name": "Speed", "category": "world", "type": "scalar"}
        ]},
        "pinnedMods": ["b"],
        "loaderArgs": {"binaryHash": "kept"}
    }"#;

    #[test]
    fn parses_every_control_shape() {
        let trainer: Trainer = serde_json::from_str(JSON).unwrap();
        let pinned = trainer.pinned_mods;
        let cheats: Vec<model::Cheat> = trainer.blueprint.cheats.into_iter().map(|c| cheat(c, &HashMap::new())).collect();
        assert!(cheats[0].kind == model::Kind::Toggle && !cheats[0].args.button && cheats[0].key.is_none());
        assert_eq!(cheats[1].hotkeys, [Chord::from_vks(&[18, 117]), Chord::from_vks(&[16, 117])]);
        assert!(cheats[1].args.button && cheats[1].args.step == Some(10.));
        assert_eq!(cheats[2].args.options, ["Noon", "2", "Night", "Dawn"]);
        assert_eq!((cheats[2].args.pressed, cheats[3].args.pressed), (1., 10.));
        assert!(cheats[3].kind == model::Kind::Unknown);
        assert!(cheats[4].kind == model::Kind::Number && !cheats[4].args.button);
        assert!(cheats[5].kind == model::Kind::Slider);
        assert_eq!(pinned, ["b"]);
    }

    /// Wand sends an empty object as `[]` (Spyro Reignited Trilogy's trainer has cheats with `"args": []`).
    #[test]
    fn an_empty_array_where_args_are_expected_means_none() {
        let json = r#"{
            "blueprint": {"cheats": [{"uuid": "a", "name": "Health", "category": "player", "type": "toggle", "args": []}]},
            "loaderArgs": []
        }"#;
        let trainer: Trainer = serde_json::from_str(json).unwrap();
        let cheats = model_cheats(trainer);
        assert!(cheats.len() == 1 && cheats[0].args.min.is_none() && cheats[0].args.options.is_empty());
    }

    #[test]
    fn disk_copy_keeps_the_whole_response() {
        let dir = std::env::temp_dir().join(format!("impulse-trainer-{}", std::process::id()));
        let stamp = Stamp { updated_at: Some("2026-01-01T00:00:00Z".to_owned()), version: Some(7), locale: "fr-FR".to_owned() };
        let stored = Stored { stamp: stamp.clone(), answer: Answer { trainer: serde_json::from_str(JSON).unwrap(), i18n: Value::Null } };
        let path = file(&dir, "../82102");
        assert!(path.ends_with("trainers/82102.json"));
        disk::replace(&path, &serde_json::to_vec(&stored).unwrap()).unwrap();
        let (trainer, found) = cached(&dir, "82102").unwrap();
        assert!(found == stamp && trainer.blueprint.cheats.len() == 6);
        assert!(fs::read_to_string(&path).unwrap().contains("binaryHash"));
        fs::remove_dir_all(&dir).unwrap();
        assert!(cached(&dir, "82102").is_none());
    }

    fn stored_copy(name: &str, stored: &Value) -> (Trainer, Stamp) {
        let dir = std::env::temp_dir().join(format!("impulse-trainer-{name}-{}", std::process::id()));
        disk::replace(&file(&dir, "1"), &serde_json::to_vec(stored).unwrap()).unwrap();
        let copy = cached(&dir, "1").unwrap();
        fs::remove_dir_all(&dir).unwrap();
        copy
    }

    fn translated(i18n: Value) -> Value {
        serde_json::json!({
            "stamp": {"updated_at": null, "version": null, "locale": "fr-FR"},
            "trainer": {"blueprint": {"cheats": [
                {"uuid": "a", "name": "Health", "category": "player", "target": "hp", "type": "toggle",
                 "description": "Never die", "instructions": "Press once"},
                {"uuid": "b", "name": "Gold", "category": "odd", "type": "toggle", "description": "Kept"}]}},
            "i18n": i18n
        })
    }

    fn model_cheats(trainer: Trainer) -> Vec<model::Cheat> {
        let strings = trainer.strings;
        trainer.blueprint.cheats.into_iter().map(|c| cheat(c, &strings)).collect()
    }

    #[test]
    fn a_stored_translation_reaches_name_description_and_instructions() {
        let i18n = serde_json::json!({"locale": "fr-FR", "strings": {
            "Health": "Santé", "Never die": "Ne jamais mourir", "Press once": "Appuyez une fois", "bad": 3}});
        let (trainer, stamp) = stored_copy("i18n", &translated(i18n));
        assert_eq!(stamp.locale, "fr-FR");
        let [health, gold] = &model_cheats(trainer)[..] else { panic!("two cheats") };
        assert_eq!(
            (health.name.as_str(), health.description.as_deref(), health.instructions.as_deref()),
            ("Santé", Some("Ne jamais mourir"), Some("Appuyez une fois"))
        );
        assert_eq!((health.id.as_str(), health.category.as_str(), health.key.as_deref()), ("a", "player", Some("hp")));
        assert_eq!((gold.name.as_str(), gold.description.as_deref()), ("Gold", Some("Kept")));
        assert!(health.category_name == Some(texts::text(texts::CATEGORY_PLAYER)));
        assert!(gold.category_name.is_none());
    }

    #[test]
    fn notes_come_in_the_answers_language_and_blank_ones_are_none() {
        let strings = HashMap::from([("Restart after a mission".to_owned(), "Relancez après une mission".to_owned())]);
        assert_eq!(notes(&Value::from("Restart after a mission"), &strings).as_deref(), Some("Relancez après une mission"));
        assert_eq!(notes(&Value::from("  Untranslated\n"), &strings).as_deref(), Some("Untranslated"));
        for blank in [Value::from(" \n"), Value::Null, Value::from(3)] {
            assert_eq!(notes(&blank, &strings), None, "{blank}");
        }
    }

    #[test]
    fn a_missing_or_bad_i18n_keeps_the_english() {
        for i18n in [Value::Null, serde_json::json!("x"), serde_json::json!({"strings": [1]})] {
            let (trainer, _) = stored_copy("plain", &translated(i18n));
            assert!(trainer.strings.is_empty());
            assert_eq!(model_cheats(trainer)[0].name, "Health");
        }
    }

    #[test]
    fn a_stamp_stored_before_locales_reads_as_english() {
        let mut stored = translated(Value::Null);
        stored.as_object_mut().unwrap().remove("i18n");
        stored["stamp"].as_object_mut().unwrap().remove("locale");
        let (_, stamp) = stored_copy("old", &stored);
        assert_eq!(stamp.locale, "en-US");
    }
}
