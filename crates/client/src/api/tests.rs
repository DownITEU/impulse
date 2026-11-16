use std::{env, fs, path::Path};

use model::{Args, Kind};
use ts_rs::{Config, TS};

use super::*;
use crate::trainer::tests::cheat;

fn typescript() -> String {
    let cfg = Config::new().with_large_int("number");
    let decls = [
        View::decl(&cfg),
        Game::decl(&cfg),
        Sheet::decl(&cfg),
        Section::decl(&cfg),
        CheatRow::decl(&cfg),
        Control::decl(&cfg),
        Collection::decl(&cfg),
        RunState::decl(&cfg),
        Command::decl(&cfg),
    ];
    let mut out = format!(
        "// Generated from crates/client/src/api.rs by its tests; `IMPULSE_BLESS=1 cargo test -p client api` rewrites it.\n\nexport const VERSION = {VERSION};\n"
    );
    for decl in decls {
        out += &format!("\nexport {decl}\n");
    }
    out
}

#[test]
fn the_panel_types_are_generated_from_these() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/panel/src/api.ts");
    let made = typescript();
    if env::var("IMPULSE_BLESS").is_ok_and(|bless| bless == "1") {
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, &made).unwrap();
    }
    let kept = fs::read_to_string(&file).unwrap_or_default().replace("\r\n", "\n");
    assert!(kept == made, "{} is stale: IMPULSE_BLESS=1 cargo test -p client api", file.display());
}

#[test]
fn views_and_commands_are_tagged_by_type() {
    let note = serde_json::to_value(View::Note { text: "no".into() }).unwrap();
    assert_eq!(note, serde_json::json!({ "type": "note", "text": "no" }));
    let none = serde_json::to_value(View::Trainer { sheet: 2, game: None, trainer: Sheet::None }).unwrap();
    assert_eq!(none, serde_json::json!({ "type": "trainer", "sheet": 2, "game": null, "trainer": { "state": "none" } }));
    let set: Command = serde_json::from_str(r#"{ "type": "set", "sheet": 2, "cheat": 5, "value": 1.5 }"#).unwrap();
    assert_eq!(set, Command::Set { sheet: 2, cheat: 5, value: 1.5 });
    assert!(serde_json::from_str::<Command>(r#"{ "type": "format_disk" }"#).is_err());
}

#[test]
fn a_device_gets_the_window_s_controls_and_bounds() {
    let slider = Control::of(&cheat("a", Kind::Slider, Args { min: Some(5.), max: Some(1.), ..Args::default() }));
    assert_eq!(slider, Control::Slider { min: 5., max: 105., step: 1. });
    let selection = cheat("c", Kind::Selection, Args { options: vec!["A".into(), "B".into()], ..Args::default() });
    assert_eq!([0.6, 2., -1.].map(|value| settle(&selection, value)), [Some(1.), None, None]);
    assert_eq!(settle(&cheat("d", Kind::Slider, Args::default()), 250.), Some(100.));
    assert_eq!(settle(&cheat("e", Kind::Toggle, Args::default()), f64::NAN), None);
    let number = Control::of(&cheat("b", Kind::Number, Args { button: true, ..Args::default() }));
    assert_eq!(number, Control::Number { min: None, max: None, step: 1., apply: true });
}
