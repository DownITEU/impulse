//! Stores as the app names them, and a game's page to open in one the user does not have it from.

use crate::registry::{self, CLASSES_ROOT, View};

/// The store apps' page routes; any other app link is refused: the links come from a remote catalog.
const APP_PAGES: [&str; 5] =
    ["steam://store/", "com.epicgames.launcher://store/", "goggalaxy://openGameView/", "ms-windows-store://pdp/", "msxbox://game/"];

/// The stores' web hosts: a page elsewhere would show under a store's name.
const WEB_STORES: [&str; 13] = [
    "store.steampowered.com",
    "www.epicgames.com",
    "store.epicgames.com",
    "www.gog.com",
    "www.microsoft.com",
    "apps.microsoft.com",
    "www.xbox.com",
    "store.ubi.com",
    "store.ubisoft.com",
    "www.origin.com",
    "www.ea.com",
    "store.rockstargames.com",
    "shop.battle.net",
];

/// A store id's prefix as the app prints it.
pub fn store_name(store: &str) -> String {
    match store.to_ascii_lowercase().as_str() {
        "uwp" => "XBOX".into(),
        "origin" => "EA".into(),
        "uplay" => "UBISOFT".into(),
        _ => store.to_uppercase(),
    }
}

/// The one of `uris` to open: a store app's page Windows has the app for (blocking: the registry), else a web
/// page; none when neither is there.
pub fn store_link(uris: &[String]) -> Option<&str> {
    pick(uris, handled)
}

fn pick(uris: &[String], handled: impl Fn(&str) -> bool) -> Option<&str> {
    // A quote or a space would reach the app's command line.
    let plain = |uri: &&String| uri.bytes().all(|b| b.is_ascii_graphic() && !br#""<>\^`{|}"#.contains(&b));
    let app = |uri: &&String| {
        APP_PAGES.iter().any(|page| uri.starts_with(page)) && uri.split_once("://").is_some_and(|(scheme, _)| handled(scheme))
    };
    let web = |uri: &&String| {
        uri.strip_prefix("https://").is_some_and(|rest| {
            let host = rest.split('/').next().unwrap_or_default();
            WEB_STORES.iter().any(|store| store.eq_ignore_ascii_case(host))
        })
    };
    let mut shown = uris.iter().filter(plain);
    shown.clone().find(app).or_else(|| shown.find(web)).map(String::as_str)
}

/// Windows opens links of `scheme` with an app.
fn handled(scheme: &str) -> bool {
    registry::open(CLASSES_ROOT, scheme, View::Wow64).is_some_and(|key| key.get_string("URL Protocol").is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uris(all: &[&str]) -> Vec<String> {
        all.iter().map(|uri| uri.to_string()).collect()
    }

    #[test]
    fn the_store_app_opens_its_page_when_windows_has_it_else_the_web() {
        let steam = uris(&["steam://store/1", "https://store.steampowered.com/app/1"]);
        assert_eq!(pick(&steam, |scheme| scheme == "steam"), Some("steam://store/1"));
        assert_eq!(pick(&steam, |_| false), Some("https://store.steampowered.com/app/1"));
        assert_eq!(pick(&uris(&["https://Store.SteamPowered.com/app/1"]), |_| false), Some("https://Store.SteamPowered.com/app/1"));
        assert_eq!(pick(&uris(&["goggalaxy://openGameView/2"]), |_| false), None);
    }

    #[test]
    fn a_link_of_another_kind_or_with_a_quote_never_passes() {
        let odd = uris(&[
            "steam://run/1",
            "ms-msdt:/id",
            "file:///c:/x.exe",
            "https://store.steampowered.com/\"x",
            "http://store.steampowered.com/app/1",
            "https://store.steampowered.com.example/app/1",
            "https://elsewhere.example/app/1",
        ]);
        assert_eq!(pick(&odd, |_| true), None);
    }

    #[test]
    fn stores_print_by_the_name_users_know() {
        assert_eq!([store_name("uwp"), store_name("origin"), store_name("uplay"), store_name("gog")], ["XBOX", "EA", "UBISOFT", "GOG"]);
    }
}
