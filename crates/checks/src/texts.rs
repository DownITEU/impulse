//! Each text domain: a crate with `locales/<tag>.ftl`, whose `Key("...")` consts must match its English file.

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

#[test]
fn every_key_has_its_message_in_every_language() {
    let files = files();
    let mut complaints = Vec::new();
    for english in files.iter().filter(|path| path.starts_with("crates/") && path.ends_with("/locales/en.ftl")) {
        let krate = english.trim_end_matches("locales/en.ftl");
        let read = |path: &str| fs::read_to_string(root().join(path)).unwrap().replace("\r\n", "\n");
        let defined = messages(english, &read(english), &mut complaints);
        let used: BTreeSet<String> = files
            .iter()
            .filter(|path| path.starts_with(&format!("{krate}src/")) && path.ends_with(".rs"))
            .flat_map(|path| keys(&read(path)).collect::<Vec<_>>())
            .collect();
        complaints.extend(used.difference(&defined).map(|id| format!("{english}: no message `{id}`, used in {krate}src")));
        complaints.extend(defined.difference(&used).map(|id| format!("{english}: `{id}` has no `Key` in {krate}src")));
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
