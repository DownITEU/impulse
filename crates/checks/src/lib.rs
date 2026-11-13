//! The workspace's rules that a test can hold: the crate graph, where GPUI may go, file sizes, the agent rules.
#![cfg(test)]

mod graph;
mod rules;
mod size;

use std::{
    fs,
    path::{Path, PathBuf},
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every file of ours, relative to the root with `/`.
fn files() -> Vec<String> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = format!("{prefix}{name}");
            match entry.file_type().unwrap().is_dir() {
                true if prefix.is_empty() && [".git", "target", "vendor"].contains(&name.as_str()) => {}
                true => walk(&entry.path(), &format!("{path}/"), out),
                false => out.push(path),
            }
        }
    }
    let mut out = Vec::new();
    walk(&root(), "", &mut out);
    out
}

/// Fails with every complaint at once.
fn verdict(complaints: Vec<String>) {
    assert!(complaints.is_empty(), "\n{}\n", complaints.join("\n"));
}
