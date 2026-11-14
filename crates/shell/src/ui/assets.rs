use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString, assets::Assets as KitAssets};

macro_rules! icons {
    ($($name:ident = $file:literal),* $(,)?) => {
        #[derive(Clone, Copy)]
        pub enum Icon { $($name),* }

        impl Icon {
            pub fn path(self) -> &'static str {
                match self { $(Self::$name => concat!("icons/", $file, ".svg")),* }
            }
        }

        fn bundled(path: &str) -> Option<&'static [u8]> {
            $(if path == concat!("icons/", $file, ".svg") {
                return Some(include_bytes!(concat!("../../assets/icons/", $file, ".svg")));
            })*
            None
        }
    };
}

icons! {
    Settings = "settings",
    Sort = "sort",
    Warn = "warn",
    Pin = "pin",
    Star = "star",
    StarFill = "star-fill",
    ChevronDown = "chevron-down",
    ChevronRight = "chevron-right",
    Info = "info",
    Expand = "expand",
    Play = "play",
    Check = "check",
    WinMin = "win-min",
    WinMax = "win-max",
    WinRestore = "win-restore",
    Close = "close",
    Folder = "folder",
    External = "external",
    More = "more",
    Player = "player",
    Heading = "heading",
    Plus = "plus",
    Minus = "minus",
    Layers = "layers",
    Sidebar = "sidebar",
    Mods = "mods",
    Map = "map",
    Clock = "clock",
    Swords = "swords",
    Shield = "shield",
    Heart = "heart",
    Zap = "zap",
    Skull = "skull",
    Crosshair = "crosshair",
    Flame = "flame",
    Gem = "gem",
    Coins = "coins",
    Rocket = "rocket",
    Ghost = "ghost",
    Crown = "crown",
}

/// Impulse's icons, then the kit's icons for stock components.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match bundled(path) {
            Some(bytes) => Ok(Some(Cow::Borrowed(bytes))),
            None => KitAssets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        KitAssets.list(path)
    }
}
