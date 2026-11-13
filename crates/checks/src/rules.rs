//! The agent rules in `.claude/rules/`: each `paths` glob must still name files, and `CLAUDE.md`'s crate map every crate.

use std::fs;

use crate::{files, root, verdict};

const RULES: &str = ".claude/rules/";

/// The globs of a rule's frontmatter, written as a `paths:` list; `None` without frontmatter (always loaded).
fn globs(text: &str) -> Option<Result<Vec<String>, String>> {
    let front = text.strip_prefix("---\n")?;
    let Some((front, _)) = front.split_once("\n---\n") else { return Some(Err("frontmatter has no closing ---".into())) };
    let mut lines = front.lines();
    if lines.next() != Some("paths:") {
        return Some(Err("frontmatter must be a `paths:` list".into()));
    }
    let globs = lines
        .map(|line| line.strip_prefix("  - \"").and_then(|glob| glob.strip_suffix('"')).map(str::to_owned).ok_or(line))
        .collect::<Result<Vec<_>, _>>();
    Some(match globs {
        Err(line) => Err(format!("`{line}` is not a `  - \"glob\"` line")),
        Ok(globs) if globs.is_empty() => Err("`paths:` lists no glob".into()),
        Ok(globs) => Ok(globs),
    })
}

/// `{a,b}` as Claude Code expands it: one level, no nesting.
fn expand(glob: &str) -> Vec<String> {
    match (glob.find('{'), glob.find('}')) {
        (Some(open), Some(close)) if open < close => glob[open + 1..close]
            .split(',')
            .flat_map(|choice| expand(&format!("{}{choice}{}", &glob[..open], &glob[close + 1..])))
            .collect(),
        _ => vec![glob.to_owned()],
    }
}

/// `**` spans folders; `*` and `?` stay within one name.
fn matches(glob: &[&str], path: &[&str]) -> bool {
    match (glob.split_first(), path.split_first()) {
        (None, None) => true,
        (Some((&"**", rest)), _) => matches(rest, path) || path.split_first().is_some_and(|(_, tail)| matches(glob, tail)),
        (Some((part, rest)), Some((name, tail))) => name_matches(part.as_bytes(), name.as_bytes()) && matches(rest, tail),
        _ => false,
    }
}

fn name_matches(glob: &[u8], name: &[u8]) -> bool {
    match (glob.split_first(), name.split_first()) {
        (None, None) => true,
        (Some((b'*', rest)), _) => name_matches(rest, name) || name.split_first().is_some_and(|(_, tail)| name_matches(glob, tail)),
        (Some((b'?', rest)), Some((_, tail))) => name_matches(rest, tail),
        (Some((wanted, rest)), Some((found, tail))) => wanted == found && name_matches(rest, tail),
        _ => false,
    }
}

#[test]
fn every_rule_glob_names_files() {
    let files = files();
    let split: Vec<Vec<&str>> = files.iter().map(|path| path.split('/').collect()).collect();
    let mut complaints = Vec::new();
    for rule in files.iter().filter(|path| path.starts_with(RULES) && path.ends_with(".md")) {
        let text = fs::read_to_string(root().join(rule)).unwrap().replace("\r\n", "\n");
        match globs(&text) {
            None => {}
            Some(Err(why)) => complaints.push(format!("{rule}: {why}")),
            Some(Ok(globs)) => {
                for glob in globs.iter().flat_map(|glob| expand(glob)) {
                    let parts: Vec<&str> = glob.split('/').collect();
                    // Claude Code matches a glob without `/` at any depth; ours are written from the root.
                    if parts.len() == 1 {
                        complaints.push(format!("{rule}: `{glob}` has no `/`: write it from the root, like `**/{glob}`"));
                    } else if !split.iter().any(|path| matches(&parts, path)) {
                        complaints.push(format!("{rule}: `{glob}` matches no file"));
                    }
                }
            }
        }
    }
    verdict(complaints);
}

#[test]
fn the_crate_map_names_every_crate() {
    let claude = fs::read_to_string(root().join("CLAUDE.md")).unwrap().replace("\r\n", "\n");
    let map = claude.split("\n## ").find(|section| section.starts_with("Crates\n")).unwrap_or_default();
    let complaints = fs::read_dir(root().join("crates"))
        .unwrap()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| !map.contains(&format!("`{name}`")))
        .map(|name| format!("CLAUDE.md: the crate map lacks `{name}`"))
        .collect();
    verdict(complaints);
}

#[test]
fn globs_match_as_claude_code_reads_them() {
    let hit = |glob: &str, path: &str| {
        expand(glob).iter().any(|glob| matches(&glob.split('/').collect::<Vec<_>>(), &path.split('/').collect::<Vec<_>>()))
    };
    assert!(hit("crates/agent/**", "crates/agent/src/lib.rs"));
    assert!(hit("crates/**/*.rs", "crates/shell/src/map/stage/paint.rs"));
    assert!(hit("crates/{tray,pe}/**", "crates/pe/src/lib.rs"));
    assert!(!hit("crates/{tray,pe}/**", "crates/play/src/lib.rs"));
    assert!(!hit("crates/*.rs", "crates/agent/src/lib.rs"));
    assert_eq!(globs("---\npaths:\n  - \"a/**\"\n---\nbody"), Some(Ok(vec!["a/**".to_owned()])));
    assert!(globs("# no frontmatter").is_none());
    assert!(matches!(globs("---\npaths: a/**\n---\n"), Some(Err(_))));
    assert!(matches!(globs("---\npaths:\n  - \"a/**\"\n  - b/**\n---\n"), Some(Err(_))));
}
