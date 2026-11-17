//! Each text domain: a crate with `locales/<tag>.ftl` and its `Key("...")` consts, or the panel with its `t('...')`
//! calls; the ids used must match the English file.

use std::{collections::BTreeSet, fs};

use fluent_syntax::{ast::Entry, parser};

use crate::{files, root, verdict};

/// A Fluent file's message ids; a file that does not parse is a complaint.
fn messages(path: &str, source: &str, complaints: &mut Vec<String>) -> BTreeSet<String> {
    let resource = parser::parse(source).unwrap_or_else(|(resource, errors)| {
        complaints.push(format!("{path}: does not parse: {errors:?}"));
        resource
    });
    let id = |entry: Entry<&str>| if let Entry::Message(message) = entry { Some(message.id.name.to_owned()) } else { None };
    resource.body.into_iter().filter_map(id).collect()
}

/// The ids named by `Key("...")` in Rust source.
fn keys(source: &str) -> impl Iterator<Item = String> + '_ {
    source.split("Key(\"").skip(1).filter_map(|rest| rest.split_once('"').map(|(id, _)| id.to_owned()))
}

/// The ids named by `t('...')` in the panel's TypeScript; `split('` and `x.t('` are not it.
fn calls(source: &str) -> impl Iterator<Item = String> + '_ {
    let bare = |at: usize| !source[..at].ends_with(|c: char| c.is_alphanumeric() || matches!(c, '_' | '$' | '.'));
    source.match_indices("t(").filter(move |(at, _)| bare(*at)).filter_map(|(at, _)| {
        // A formatter may break a long call after its parenthesis.
        let rest = source[at + 2..].trim_start();
        let quote = rest.chars().next().filter(|c| matches!(c, '\'' | '"'))?;
        rest[1..].split_once(quote).map(|(id, _)| id.to_owned())
    })
}

/// The ids a source file uses.
fn used(path: &str, source: &str) -> Vec<String> {
    match path.rsplit_once('.').map(|(_, extension)| extension) {
        Some("rs") => keys(source).collect(),
        Some("ts" | "tsx") => calls(source).collect(),
        _ => Vec::new(),
    }
}

#[test]
fn every_key_has_its_message_in_every_language() {
    let files = files();
    let mut complaints = Vec::new();
    for english in files.iter().filter(|path| path.ends_with("/locales/en.ftl")) {
        let krate = english.trim_end_matches("locales/en.ftl");
        // A plugin's messages are named by its manifest, not by `Key`s.
        if root().join(krate).join("manifest.json").exists() {
            continue;
        }
        let read = |path: &str| fs::read_to_string(root().join(path)).unwrap().replace("\r\n", "\n");
        let defined = messages(english, &read(english), &mut complaints);
        let used: BTreeSet<String> =
            files.iter().filter(|path| path.starts_with(&format!("{krate}src/"))).flat_map(|path| used(path, &read(path))).collect();
        complaints.extend(used.difference(&defined).map(|id| format!("{english}: no message `{id}`, used in {krate}src")));
        complaints.extend(defined.difference(&used).map(|id| format!("{english}: `{id}` is used nowhere in {krate}src")));
        for other in files.iter().filter(|path| path.starts_with(&format!("{krate}locales/")) && *path != english) {
            let translated = messages(other, &read(other), &mut complaints);
            complaints.extend(defined.difference(&translated).map(|id| format!("{other}: lacks `{id}`")));
            complaints.extend(translated.difference(&defined).map(|id| format!("{other}: `{id}` is not in English")));
        }
    }
    verdict(complaints);
}

#[test]
fn keys_are_read_from_their_consts() {
    assert_eq!(keys(r#"const A: Key = Key("a"); t(cx, Key("b-c"))"#).collect::<Vec<_>>(), ["a", "b-c"]);
}

#[test]
fn panel_ids_are_read_from_bare_t_calls() {
    let source = "t('a'); t(\"b-c\", { n: 1 }); s.split('-'); x.t('no'); t(id); sort('z'); t(\n    'd',\n    { n },\n)";
    assert_eq!(calls(source).collect::<Vec<_>>(), ["a", "b-c", "d"]);
}
