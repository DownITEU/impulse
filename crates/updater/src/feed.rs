//! The release feed: a GitHub-shaped JSON array (Gitea, Forgejo and a static copy answer alike), reduced to the
//! releases that can be installed.

use anyhow::{Result, bail, ensure};
use semver::Version;
use serde::{Deserialize, Serialize};

/// The asset every release carries: the portable executable.
const ASSET: &str = "Impulse.exe";
/// A feed is read for its newest entries and a bounded text of each: the answer is kept on disk and parsed on a thread
/// that must not stall.
const MAX_RELEASES: usize = 30;
const MAX_NOTES: usize = 16 << 10;

/// Which releases are offered. A pre-release is a tag with a semver suffix (`v1.30.1-pre`) or one the host marks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    #[default]
    Stable,
    /// Pre-releases and releases, whichever is newer.
    Pre,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    pub version: Version,
    pub pre: bool,
    /// `YYYY-MM-DD`.
    pub date: String,
    /// Markdown, the release page's text.
    pub notes: String,
    pub url: String,
    /// Lowercase hex.
    pub sha256: String,
}

#[derive(Deserialize)]
struct Raw {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    body: Option<String>,
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
}

impl Raw {
    /// `None` for a draft, a tag that is not semver, a release without its executable and a SHA-256 digest, and an
    /// address that is not secure: nothing unverified is ever offered.
    fn release(self) -> Option<Release> {
        let version = Version::parse(self.tag_name.trim_start_matches('v')).ok()?;
        let asset = self.assets.into_iter().find(|asset| asset.name == ASSET)?;
        let sha256 = asset.digest?.strip_prefix("sha256:")?.to_ascii_lowercase();
        let digest = sha256.len() == 64 && sha256.bytes().all(|byte| byte.is_ascii_hexdigit());
        if self.draft || !digest || !secure(&asset.browser_download_url) {
            return None;
        }
        let date = self.published_at.unwrap_or_default().chars().take(10).collect();
        let notes = self.body.unwrap_or_default();
        let notes = match notes.char_indices().find(|(at, _)| *at >= MAX_NOTES) {
            Some((at, _)) => notes[..at].to_owned(),
            None => notes,
        };
        Some(Release { pre: self.prerelease || !version.pre.is_empty(), version, date, notes, url: asset.browser_download_url, sha256 })
    }
}

/// The installable releases of a feed's answer; an entry that does not read is skipped, as a remote file's bad field is.
pub fn parse(json: &[u8]) -> Result<Vec<Release>> {
    let entries: Vec<serde_json::Value> = serde_json::from_slice(json)?;
    Ok(entries.into_iter().take(MAX_RELEASES).filter_map(|entry| serde_json::from_value::<Raw>(entry).ok()?.release()).collect())
}

/// The releases of `channel` newer than `current` and than the `skipped` one, newest first. Never a downgrade, whatever
/// the dates say; build metadata (`1.0.0+mirror`) does not make a version newer, so a rebuild of a skipped one stays skipped.
pub fn newer(releases: &[Release], current: &Version, channel: Channel, skipped: Option<&Version>) -> Vec<Release> {
    let above = |release: &Release, floor: &Version| release.version.cmp_precedence(floor).is_gt();
    let newer = |release: &&Release| {
        above(release, current) && skipped.is_none_or(|skipped| above(release, skipped)) && (channel == Channel::Pre || !release.pre)
    };
    let mut found: Vec<Release> = releases.iter().filter(newer).cloned().collect();
    found.sort_by(|a, b| b.version.cmp_precedence(&a.version));
    found
}

pub struct Fetched {
    pub etag: Option<String>,
    pub releases: Vec<Release>,
}

pub enum Check {
    /// The `etag` still names the feed's answer.
    Same,
    New(Fetched),
}

/// https, or plain http to this machine (a feed under test): the feed and its files are the publisher, so they
/// are never read over a wire that others can change.
pub(crate) fn secure(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://") else { return url.starts_with("https://") };
    // `http://localhost@elsewhere/` names `elsewhere`.
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.contains('@') && matches!(authority.split(':').next(), Some("127.0.0.1" | "localhost"))
}

/// Asks the feed; with the `etag` of the last answer a 304 costs nothing against the host's limit.
pub fn check(feed: &str, etag: Option<&str>) -> Result<Check> {
    ensure!(secure(feed), "the release feed is not https");
    let response = fetch::get(feed, etag)?;
    match response.status {
        304 if etag.is_some() => Ok(Check::Same),
        200 => Ok(Check::New(Fetched { etag: response.etag, releases: parse(&response.body)? })),
        status => bail!("the release feed answered {status}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789ABCDEF";

    fn entry(tag: &str, extra: &str) -> String {
        format!(
            r#"{{"tag_name":"{tag}","body":"notes","published_at":"2026-11-20T10:00:00Z","assets":[{{"name":"Impulse.exe","browser_download_url":"https://x/{tag}","digest":"{DIGEST}"}}]{extra}}}"#
        )
    }

    fn feed(entries: &[String]) -> Vec<u8> {
        format!("[{}]", entries.join(",")).into_bytes()
    }

    fn releases(tags: &[&str]) -> Vec<Release> {
        parse(&feed(&tags.iter().map(|tag| entry(tag, "")).collect::<Vec<_>>())).unwrap()
    }

    fn versions(found: &[Release]) -> Vec<String> {
        found.iter().map(|release| release.version.to_string()).collect()
    }

    #[test]
    fn a_release_reads_its_version_date_notes_and_digest() {
        let release = &releases(&["v1.30.1"])[0];
        assert_eq!(release.version, Version::new(1, 30, 1));
        assert_eq!((release.date.as_str(), release.notes.as_str(), release.url.as_str()), ("2026-11-20", "notes", "https://x/v1.30.1"));
        assert_eq!(release.sha256, DIGEST.strip_prefix("sha256:").unwrap().to_ascii_lowercase());
        assert!(!release.pre);
    }

    #[test]
    fn a_tag_suffix_or_the_hosts_flag_makes_a_pre_release() {
        assert!(releases(&["v1.30.1-pre"])[0].pre);
        let flagged = parse(&feed(&[entry("v1.30.1", r#","prerelease":true"#)])).unwrap();
        assert!(flagged[0].pre);
    }

    #[test]
    fn drafts_odd_tags_and_releases_without_a_verified_executable_are_skipped() {
        let no_digest = r#"{"tag_name":"v1.0.0","assets":[{"name":"Impulse.exe","browser_download_url":"u"}]}"#;
        let other_asset = r#"{"tag_name":"v1.0.1","assets":[{"name":"x.zip","browser_download_url":"u","digest":"sha256:00"}]}"#;
        let short_digest = r#"{"tag_name":"v1.0.2","assets":[{"name":"Impulse.exe","browser_download_url":"u","digest":"sha256:00"}]}"#;
        let json = feed(&[
            entry("v1.1.0", r#","draft":true"#),
            entry("nightly", ""),
            no_digest.into(),
            other_asset.into(),
            short_digest.into(),
            "7".into(),
            entry("v1.2.0", ""),
        ]);
        assert_eq!(versions(&parse(&json).unwrap()), ["1.2.0"]);
        assert!(parse(b"not json").is_err());
    }

    #[test]
    fn stable_ignores_pre_releases_and_pre_takes_both_newest_first() {
        let all = releases(&["v1.30.0-pre", "v1.29.2", "v1.30.1-pre", "v1.29.1"]);
        let current = Version::new(1, 29, 1);
        assert_eq!(versions(&newer(&all, &current, Channel::Stable, None)), ["1.29.2"]);
        assert_eq!(versions(&newer(&all, &current, Channel::Pre, None)), ["1.30.1-pre", "1.30.0-pre", "1.29.2"]);
    }

    #[test]
    fn a_release_beats_its_own_pre_release_and_a_downgrade_is_never_offered() {
        let all = releases(&["v1.30.1", "v1.30.1-pre", "v1.28.0"]);
        assert_eq!(versions(&newer(&all, &Version::parse("1.30.1-pre").unwrap(), Channel::Stable, None)), ["1.30.1"]);
        assert!(newer(&all, &Version::new(1, 30, 1), Channel::Pre, None).is_empty());
        assert!(newer(&all, &Version::parse("2.0.0-pre").unwrap(), Channel::Stable, None).is_empty());
    }

    #[test]
    fn a_skip_drops_its_version_its_rebuilds_and_every_older_one_but_never_lowers_the_floor() {
        let all = releases(&["v1.3.0", "v1.2.0+b2", "v1.2.0", "v1.1.0"]);
        let current = Version::new(1, 0, 0);
        assert_eq!(versions(&newer(&all, &current, Channel::Stable, Some(&Version::new(1, 2, 0)))), ["1.3.0"]);
        assert_eq!(versions(&newer(&all, &Version::new(1, 2, 0), Channel::Stable, Some(&Version::new(0, 9, 0)))), ["1.3.0"]);
    }

    #[test]
    fn only_https_or_this_machine_is_trusted_with_a_feed() {
        assert!(secure("https://api.github.com/repos/o/r/releases"));
        assert!(secure("http://127.0.0.1:8000/releases.json") && secure("http://localhost/x"));
        assert!(!secure("http://api.github.com/x") && !secure("http://127.0.0.1.evil.com/x") && !secure("ftp://x") && !secure(""));
        assert!(!secure("http://localhost:80@evil.com/x") && !secure("http://127.0.0.1@evil.com") && !secure("http://evil.com#@localhost"));
    }

    #[test]
    fn build_metadata_is_not_newer_and_an_insecure_asset_is_not_offered() {
        let all = releases(&["v1.0.0+mirror", "v1.1.0"]);
        assert_eq!(versions(&newer(&all, &Version::new(1, 0, 0), Channel::Stable, None)), ["1.1.0"]);
        let digest = DIGEST.to_ascii_lowercase();
        let insecure = format!(
            r#"{{"tag_name":"v9.0.0","assets":[{{"name":"Impulse.exe","browser_download_url":"http://evil.com/x","digest":"{digest}"}}]}}"#
        );
        assert_eq!(versions(&parse(&feed(&[insecure, entry("v1.2.0", "")])).unwrap()), ["1.2.0"]);
    }

    #[test]
    fn a_feed_is_read_to_its_limits() {
        let long = "x".repeat(MAX_NOTES + 500);
        let big = entry("v1.0.0", "").replace("\"notes\"", &format!("\"{long}\""));
        assert_eq!(parse(&feed(&[big])).unwrap()[0].notes.len(), MAX_NOTES);
        let many: Vec<String> = (0..MAX_RELEASES + 5).map(|minor| entry(&format!("v1.{minor}.0"), "")).collect();
        assert_eq!(parse(&feed(&many)).unwrap().len(), MAX_RELEASES);
    }

    #[test]
    fn the_channel_is_saved_by_name() {
        assert_eq!(serde_json::to_string(&Channel::Pre).unwrap(), r#""pre""#);
        assert_eq!(serde_json::from_str::<Channel>(r#""stable""#).unwrap(), Channel::Stable);
    }
}
