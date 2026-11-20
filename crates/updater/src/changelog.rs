//! `CHANGELOG.md`'s sections: `## [x.y.z] - YYYY-MM-DD`, then markdown. The exe carries the file, so what a
//! build changed shows offline and always matches it.

use semver::Version;

pub struct Section {
    pub version: Version,
    pub date: String,
    pub notes: String,
}

/// The version without a pre-release suffix: one section covers a version's pre-releases and its release.
pub fn base(version: &Version) -> Version {
    Version::new(version.major, version.minor, version.patch)
}

/// The sections in file order; text before the first one is a preamble, and a heading that is no version is skipped.
pub fn sections(text: &str) -> Vec<Section> {
    let mut found: Vec<Section> = Vec::new();
    let mut open = false;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            let heading = rest.strip_prefix('[').and_then(|rest| rest.split_once("] - "));
            let version = heading.and_then(|(version, date)| Some((Version::parse(version).ok()?, date.trim())));
            open = version.is_some();
            if let Some((version, date)) = version {
                found.push(Section { version, date: date.to_owned(), notes: String::new() });
            }
        } else if open && let Some(section) = found.last_mut() {
            section.notes.push_str(line);
            section.notes.push('\n');
        }
    }
    for section in &mut found {
        section.notes = section.notes.trim().to_owned();
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "# Changelog\n\nSome preamble.\n\n## [0.2.0] - 2026-12-01\n\n### Fixes\n\n- A fix.\n\n## [0.1.0] - 2026-11-20\n\n### Features\n\n- A feature.\n\n## [oops] - x\n\nignored\n";

    #[test]
    fn sections_come_in_file_order_with_their_date_and_notes() {
        let found = sections(TEXT);
        let heads: Vec<_> = found.iter().map(|section| (section.version.to_string(), section.date.as_str())).collect();
        assert_eq!(heads, [("0.2.0".to_owned(), "2026-12-01"), ("0.1.0".to_owned(), "2026-11-20")]);
        assert_eq!(found[0].notes, "### Fixes\n\n- A fix.");
        assert_eq!(found[1].notes, "### Features\n\n- A feature.");
    }

    #[test]
    fn a_pre_release_reads_its_versions_section() {
        let pre = Version::parse("0.2.0-pre").unwrap();
        assert!(sections(TEXT).iter().any(|section| section.version == base(&pre)));
    }

    #[test]
    fn a_file_without_sections_has_none() {
        assert!(sections("").is_empty() && sections("# Changelog\n\ntext").is_empty());
    }
}
