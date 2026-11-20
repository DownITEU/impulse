//! The release notes: `CHANGELOG.md` is what the release page and the app's popups show, so it leads with the
//! version the workspace builds.

use std::fs;

use crate::{root, verdict};

/// A section's `## [x.y.z] - date`: the version and the date.
fn heading(line: &str) -> Option<(&str, &str)> {
    line.strip_prefix("## [")?.split_once("] - ")
}

fn is_version(text: &str) -> bool {
    let parts: Vec<&str> = text.split('.').collect();
    parts.len() == 3 && parts.iter().all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn is_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10 && bytes.iter().enumerate().all(|(at, byte)| if matches!(at, 4 | 7) { *byte == b'-' } else { byte.is_ascii_digit() })
}

/// The workspace's version without a `-pre`: one section covers a version's pre-releases and its release.
fn workspace_version() -> &'static str {
    env!("CARGO_PKG_VERSION").split('-').next().unwrap()
}

fn changelog() -> String {
    fs::read_to_string(root().join("CHANGELOG.md")).expect("CHANGELOG.md at the root")
}

#[test]
fn the_changelog_leads_with_the_workspace_version() {
    let text = changelog();
    let newest = text.lines().find_map(heading).expect("a `## [x.y.z] - date` section").0;
    assert_eq!(newest, workspace_version(), "CHANGELOG.md's newest section is not the version the workspace builds");
}

#[test]
fn every_changelog_heading_is_a_version_and_a_date_newest_first() {
    let text = changelog();
    let mut complaints = Vec::new();
    let mut last: Option<Vec<u32>> = None;
    for line in text.lines().filter(|line| line.starts_with("## ")) {
        let Some((version, _)) = heading(line).filter(|(version, date)| is_version(version) && is_date(date)) else {
            complaints.push(format!("{line}: a section is `## [x.y.z] - YYYY-MM-DD`"));
            continue;
        };
        let numbers: Vec<u32> = version.split('.').map(|part| part.parse().unwrap_or(0)).collect();
        if last.as_ref().is_some_and(|last| *last <= numbers) {
            complaints.push(format!("{line}: sections go newest first, each version once"));
        }
        last = Some(numbers);
    }
    verdict(complaints);
}
