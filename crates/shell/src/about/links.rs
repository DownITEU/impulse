use gpui_kit::App;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const EMAIL: &str = "contact@kitbyte.dev";

/// The project's repository, named once: the page and the release feed derive from it.
macro_rules! slug {
    () => {
        "tweak-for-me/impulse"
    };
}

const REPO: &str = concat!("https://github.com/", slug!());
/// The release list `updater` reads; a host with the same JSON shape (Gitea, Forgejo) can take its place.
pub const FEED: &str = concat!("https://api.github.com/repos/", slug!(), "/releases?per_page=30");

/// Every address the app sends the user to; the one place to change when one moves.
#[derive(Clone, Copy)]
pub enum Link {
    Repo,
    Discord,
    Bug,
    Support,
    Email,
}

impl Link {
    pub fn open(self, cx: &mut App) {
        cx.open_url(&self.url());
    }

    fn url(self) -> String {
        match self {
            Link::Repo => REPO.into(),
            Link::Discord => "https://discord.gg/GgX29D7Tt".into(),
            // The form starts with the version and two headings: the first things a fix needs.
            Link::Bug => format!(
                "{REPO}/issues/new?body=Impulse%20{VERSION}%0A%0A%23%23%20What%20happened%0A%0A%0A%23%23%20What%20you%20expected%0A%0A"
            ),
            Link::Support => "https://buymeacoffee.com/kitbyte".into(),
            Link::Email => format!("mailto:{EMAIL}?subject=Impulse%20{VERSION}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bug_report_opens_with_the_version() {
        assert!(Link::Bug.url().starts_with(&format!("{REPO}/issues/new?body=Impulse%20")));
        assert!(Link::Bug.url().contains(VERSION));
    }

    #[test]
    fn a_link_is_a_web_page_or_a_mail_address() {
        for link in [Link::Repo, Link::Discord, Link::Bug, Link::Support, Link::Email] {
            let url = link.url();
            assert!(url.starts_with("https://") || url.starts_with(&format!("mailto:{EMAIL}")), "{url}");
            assert!(!url.contains(' '), "{url}");
        }
    }
}
