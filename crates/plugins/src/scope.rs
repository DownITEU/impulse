//! What a plugin may reach: the hosts it fetches from, the hosts its links open, and its media under its source.

use anyhow::{Result, bail};
use model::SourceId;
use url::Url;

/// Host patterns: `api.example.com`, or `*.example.com` for that domain and every host below it.
#[derive(Debug, Default)]
pub struct Hosts(Vec<Pattern>);

#[derive(Debug)]
enum Pattern {
    Exact(String),
    Below(String),
}

impl Hosts {
    pub fn parse(patterns: &[String]) -> Result<Self> {
        let plain = |host: &str| {
            host.split('.').count() >= 2
                && host.split('.').all(|label| {
                    !label.is_empty()
                        && label.len() <= 63
                        && label.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                })
        };
        let parsed = patterns.iter().map(|pattern| match pattern.strip_prefix("*.") {
            Some(domain) if plain(domain) => Ok(Pattern::Below(domain.to_owned())),
            None if plain(pattern) => Ok(Pattern::Exact(pattern.clone())),
            _ => bail!("not a host or *.domain: {pattern:?}"),
        });
        Ok(Self(parsed.collect::<Result<_>>()?))
    }

    /// `host` as a URL gives it: lower case.
    pub fn allows(&self, host: &str) -> bool {
        self.0.iter().any(|pattern| match pattern {
            Pattern::Exact(exact) => host == exact,
            Pattern::Below(domain) => host.strip_suffix(domain.as_str()).is_some_and(|rest| rest.is_empty() || rest.ends_with('.')),
        })
    }
}

pub struct Scope {
    pub source: SourceId,
    pub hosts: Hosts,
    pub links: Hosts,
}

impl Scope {
    /// An https URL on one of the plugin's hosts, rewritten so the app fetches it under this plugin's scope:
    /// `plugin-<id>:<url>`. `None` for any other URL.
    pub fn media(&self, url: &str) -> Option<String> {
        on(url, &["https"], &self.hosts).then(|| format!("{}:{url}", self.source.as_str()))
    }

    /// Whether a description's web link may open: http or https on one of its link hosts.
    pub fn opens(&self, url: &str) -> bool {
        on(url, &["http", "https"], &self.links)
    }
}

fn on(url: &str, schemes: &[&str], hosts: &Hosts) -> bool {
    Url::parse(url).is_ok_and(|url| schemes.contains(&url.scheme()) && url.host_str().is_some_and(|host| hosts.allows(host)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hosts(patterns: &[&str]) -> Hosts {
        Hosts::parse(&patterns.iter().map(|pattern| (*pattern).to_owned()).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn a_pattern_names_one_host_or_a_domain_and_below_it() {
        let hosts = hosts(&["api.example.com", "*.cdn.net"]);
        assert!(hosts.allows("api.example.com") && hosts.allows("cdn.net") && hosts.allows("a.b.cdn.net"));
        assert!(!hosts.allows("example.com") && !hosts.allows("evilcdn.net") && !hosts.allows("cdn.net.evil.org"));
        for bad in ["*", "*.com", "Example.com", "a..b", "host:443", "", "*.a.b/c"] {
            assert!(Hosts::parse(&[bad.to_owned()]).is_err(), "{bad}");
        }
    }

    #[test]
    fn media_must_be_https_on_a_host_and_links_on_a_link_host() {
        let scope = Scope { source: SourceId::new("plugin-maps").unwrap(), hosts: hosts(&["*.tiles.io"]), links: hosts(&["wiki.io"]) };
        assert_eq!(scope.media("https://a.tiles.io/{z}/{x}/{y}.jpg").as_deref(), Some("plugin-maps:https://a.tiles.io/{z}/{x}/{y}.jpg"));
        for refused in ["http://a.tiles.io/x.png", "https://other.io/x.png", "file:///c:/x.png", "https://a.tiles.io.evil/x"] {
            assert_eq!(scope.media(refused), None, "{refused}");
        }
        assert!(scope.opens("http://wiki.io/page") && !scope.opens("https://a.tiles.io/page") && !scope.opens("javascript:alert(1)"));
    }
}
