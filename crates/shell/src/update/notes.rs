//! The markdown a dialog shows, made from release notes and `CHANGELOG.md`'s sections.

use std::fmt::Write;

use updater::{Release, Section, Version};

use crate::about::{issue, profile};

/// `#12` and `@login` as GitHub reads them, made links to the issue and the account; code spans keep them as text.
pub fn linked(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    for (at, part) in markdown.split('`').enumerate() {
        if at > 0 {
            out.push('`');
        }
        if at % 2 == 1 {
            out.push_str(part);
            continue;
        }
        let mut rest = part;
        while let Some(found) = rest.find(['#', '@']) {
            let (before, from) = rest.split_at(found);
            out.push_str(before);
            let name = &from[1..];
            let name = &name[..name.find(|c: char| !c.is_ascii_alphanumeric() && c != '-').unwrap_or(name.len())];
            // Not inside a word: a mail address, a URL's anchor, a heading's next mark.
            let free = out.chars().last().is_none_or(|c| c.is_whitespace() || c == '(');
            let url = match from.as_bytes()[0] {
                b'#' if free && !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()) => Some(issue(name)),
                b'@' if free && login(name) => Some(profile(name)),
                _ => None,
            };
            let taken = url.as_ref().map_or(1, |_| 1 + name.len());
            match url {
                Some(url) => _ = write!(out, "[{}]({url})", &from[..taken]),
                None => out.push_str(&from[..1]),
            }
            rest = &from[taken..];
        }
        out.push_str(rest);
    }
    out
}

/// GitHub's rule for a login: letters, digits and single inner hyphens, 39 at most.
fn login(name: &str) -> bool {
    (1..=39).contains(&name.len()) && !name.starts_with('-') && !name.ends_with('-') && !name.contains("--")
}

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

    #[test]
    fn an_issue_and_a_login_become_links_but_not_in_a_word_or_code() {
        assert_eq!(
            linked("- A fix. (#26; thanks @Ghost-Inq)"),
            format!("- A fix. ([#26]({}); thanks [@Ghost-Inq]({}))", issue("26"), profile("Ghost-Inq"))
        );
        let kept = "### Fixes\n\nmail a@b.dev, page x#12, #12a, @-x, `@types/node` and `#3`";
        assert_eq!(linked(kept), kept);
    }
}
