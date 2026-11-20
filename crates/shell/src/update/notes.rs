//! The markdown a dialog shows, made from release notes and `CHANGELOG.md`'s sections.

use updater::{Release, Section, Version};

/// Each release under a heading of its version and date, in the order given.
pub fn available(releases: &[Release]) -> String {
    join(releases.iter().map(|release| (&release.version, release.date.as_str(), release.notes.as_str())))
}

pub fn all(sections: &[Section]) -> String {
    join(sections.iter().map(|section| (&section.version, section.date.as_str(), section.notes.as_str())))
}

/// The notes of `version`'s section, without a heading: the dialog's title says which.
pub fn of(sections: &[Section], version: &Version) -> String {
    let base = updater::base(version);
    sections.iter().find(|section| section.version == base).map(|section| section.notes.clone()).unwrap_or_default()
}

fn join<'a>(entries: impl Iterator<Item = (&'a Version, &'a str, &'a str)>) -> String {
    let entry = |(version, date, notes): (&Version, &str, &str)| match date.is_empty() {
        true => format!("## {version}\n\n{notes}"),
        false => format!("## {version} ({date})\n\n{notes}"),
    };
    entries.map(entry).collect::<Vec<_>>().join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(version: &str, date: &str, notes: &str) -> Section {
        Section { version: Version::parse(version).unwrap(), date: date.into(), notes: notes.into() }
    }

    #[test]
    fn each_release_has_a_heading_of_its_version_and_date() {
        let sections = [section("0.2.0", "2026-12-01", "- B"), section("0.1.0", "", "- A")];
        assert_eq!(all(&sections), "## 0.2.0 (2026-12-01)\n\n- B\n\n## 0.1.0\n\n- A");
    }

    #[test]
    fn a_pre_release_shows_the_notes_of_its_version_and_none_without_a_section() {
        let sections = [section("0.2.0", "2026-12-01", "- B")];
        assert_eq!(of(&sections, &Version::parse("0.2.0-pre").unwrap()), "- B");
        assert_eq!(of(&sections, &Version::new(0, 3, 0)), "");
        assert_eq!(all(&[]), "");
    }
}
