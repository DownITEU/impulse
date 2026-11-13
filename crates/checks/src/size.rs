use std::fs;

use crate::{files, root, verdict};

const MOST_LINES: usize = 500;

#[test]
fn no_source_file_is_over_500_lines() {
    let complaints = files()
        .into_iter()
        .filter(|path| path.starts_with("crates/") && path.ends_with(".rs"))
        .filter_map(|path| {
            let lines = fs::read_to_string(root().join(&path)).unwrap().lines().count();
            (lines > MOST_LINES).then(|| format!("{path}: {lines} lines, split it by what it does"))
        })
        .collect();
    verdict(complaints);
}
