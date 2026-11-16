use super::*;

const APP: Files = &[
    ("en", "title = SETTINGS\nplayed = { $hours ->\n    [one] { $hours } HOUR\n   *[other] { $hours } HOURS\n}\n"),
    ("xx", "title = REGLAGES\n"),
];
const XX: Language = Language { tag: "xx", name: "XX" };

#[test]
fn a_message_falls_back_to_english_then_to_its_key() {
    let catalog = Catalog::new(XX, false, [("app", APP)]);
    assert_eq!(catalog.text("app", "title", &[]), "REGLAGES");
    assert_eq!(catalog.text("app", "played", &[("hours", 3u32.into())]), "3 HOURS");
    assert_eq!(catalog.text("app", "gone", &[]), "gone");
    assert_eq!(catalog.text("other", "title", &[]), "title");
}

#[test]
fn plurals_follow_the_number() {
    let catalog = Catalog::new(ENGLISH, false, [("app", APP)]);
    assert_eq!(catalog.text("app", "played", &[("hours", 1u32.into())]), "1 HOUR");
    assert_eq!(catalog.text("app", "played", &[("hours", 1.5.into())]), "1.5 HOURS");
}

#[test]
fn french_plurals_count_zero_as_one() {
    const MODS: Files = &[("fr", "mods = { $count ->\n    [one] { $count } MOD\n   *[other] { $count } MODS\n}\n")];
    let catalog = Catalog::new(FRENCH, false, [("app", MODS)]);
    assert_eq!(catalog.text("app", "mods", &[("count", 0u32.into())]), "0 MOD");
    assert_eq!(catalog.text("app", "mods", &[("count", 2u32.into())]), "2 MODS");
}

#[test]
fn the_pseudo_locale_stretches_the_message_but_not_its_arguments() {
    const HELLO: Files = &[("en", "hello = Hi { $name }!\n")];
    let catalog = Catalog::new(ENGLISH, true, [("app", HELLO)]);
    assert_eq!(catalog.text("app", "hello", &[("name", "Ann".into())]), "[Hî ~]Ann[!]");
}

#[test]
fn a_system_tag_matches_ours_by_language() {
    assert_eq!(closest("en-GB"), Some(ENGLISH));
    assert_eq!(closest("EN_us"), Some(ENGLISH));
    assert_eq!(closest("fr-CA"), Some(FRENCH));
    assert_eq!(closest("xx-YY"), None);
}
