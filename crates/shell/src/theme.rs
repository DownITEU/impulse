//! Design tokens (`docs/design.md`), the single source for colors and fonts.

use std::borrow::Cow;

use gpui_kit::{
    App, Rgba,
    component::{Theme, ThemeMode},
    px, rgb,
};

pub const UI: &str = "Chakra Petch";
pub const MONO: &str = "JetBrains Mono";

pub const BG: u32 = 0x0A0B0A;
pub const PANEL: u32 = 0x0C0E0B;
pub const SUNKEN: u32 = 0x060706;
pub const RAISED: u32 = 0x121411;
pub const RAISED_HOVER: u32 = 0x1C201B;
pub const ROW_HOVER: u32 = 0x121511;
pub const ROW_SELECTED: u32 = 0x171B16;
pub const STATUSBAR: u32 = 0x070807;
pub const AVATAR: u32 = 0x1A1E19;

pub const TEXT_BRIGHT: u32 = 0xF2F5EE;
pub const TEXT: u32 = 0xE4E8DF;
pub const TEXT_2: u32 = 0xC9CFC3;
pub const MUTED: u32 = 0xA9B0A3;
pub const DIM: u32 = 0x8F978A;
pub const FAINT: u32 = 0x7C8476;
pub const GHOST: u32 = 0x4A5046;
pub const WARN: u32 = 0xFFB547;
pub const DANGER: u32 = 0xFF6B5A;

pub const SEGMENT_OFF: u32 = 0x262A24;
pub const SEGMENT_IDLE: u32 = 0x6E7569;
pub const TICK: u32 = 0x1E221D;
pub const TICK_IDLE: u32 = 0x5A6155;
const SCROLL_THUMB: u32 = 0x2A2F28;

pub const ACCENTS: [(&str, u32); 4] = [("LIME", 0xC6FF3D), ("AMBER", 0xFFB020), ("CYAN", 0x4DE2FF), ("WHITE", 0xF2F2F2)];

pub fn alpha(hex: u32, a: f32) -> Rgba {
    Rgba { a, ..rgb(hex) }
}

/// Hairlines: the text color at a low alpha.
pub fn line(a: f32) -> Rgba {
    alpha(TEXT, a)
}

macro_rules! font {
    ($file:literal) => {
        Cow::Borrowed(include_bytes!(concat!("../assets/fonts/", $file, ".ttf")).as_slice())
    };
}

pub fn init(cx: &mut App) {
    cx.text_system()
        .add_fonts(vec![
            font!("ChakraPetch-Regular"),
            font!("ChakraPetch-Medium"),
            font!("ChakraPetch-SemiBold"),
            font!("ChakraPetch-Bold"),
            font!("JetBrainsMono-Regular"),
            font!("JetBrainsMono-Medium"),
            font!("JetBrainsMono-Bold"),
        ])
        .expect("bundled fonts load");
    // Stock components (inputs, scrollbars) read these.
    Theme::change(ThemeMode::Dark, None, cx);
    let theme = Theme::global_mut(cx);
    theme.font_family = UI.into();
    theme.mono_font_family = MONO.into();
    theme.radius = px(0.);
    theme.background = rgb(BG).into();
    theme.foreground = rgb(TEXT).into();
    theme.muted_foreground = rgb(DIM).into();
    theme.caret = rgb(TEXT).into();
    theme.popover = rgb(RAISED).into();
    theme.popover_foreground = rgb(TEXT).into();
    theme.border = line(0.2).into();
    theme.scrollbar = alpha(BG, 0.).into();
    theme.scrollbar_thumb = rgb(SCROLL_THUMB).into();
    theme.scrollbar_thumb_hover = rgb(GHOST).into();
}
