use gpui_kit::{
    AnyView, App, Context, Div, FontWeight, IntoElement, Render, SharedString, Window,
    component::{h_flex, v_flex},
    div,
    prelude::*,
    px, rgb,
};
use i18n::{Arg, Key};

use super::links::{EMAIL, Link, VERSION};
use crate::{
    settings::label,
    ui::{
        assets::Icon,
        key,
        text::{t, tf},
        theme::*,
        widgets::{button, icon, mono, square, tracked},
    },
};

/// The ABOUT tab: who made it and how to reach them. Each row is one way to help, the support ones lit in the accent.
/// The update rows come in from their slice, so this one never names it.
pub struct AboutView {
    updates: AnyView,
}

impl AboutView {
    pub fn new(updates: AnyView) -> Self {
        Self { updates }
    }
}

impl Render for AboutView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        page(&self.updates, cx)
    }
}

fn page(updates: &AnyView, cx: &App) -> impl IntoElement {
    let accent = rgb(accent(cx));
    let version = tf(cx, key::ABOUT_VERSION, &[("version", Arg::Str(VERSION))]);
    let intro = v_flex()
        .gap(px(8.))
        .py(px(16.))
        .border_b_1()
        .border_color(line(0.07))
        .child(
            h_flex()
                .gap(px(10.))
                .child(square(10., accent))
                .child(tracked("IMPULSE", 20., 0.22).font_weight(FontWeight::BOLD))
                .child(mono(version, 11.).text_color(rgb(FAINT))),
        )
        .child(mono(t(cx, key::ABOUT_TAGLINE), 11.).text_color(accent))
        .child(div().text_color(rgb(MUTED)).child(t(cx, key::ABOUT_INTRO)));
    let contact = tf(cx, key::ABOUT_CONTACT_HINT, &[("address", Arg::Str(EMAIL))]);
    v_flex()
        .px(px(18.))
        .py(px(6.))
        .child(intro)
        .child(updates.clone())
        .child(entry(cx, Icon::Github, Link::Repo, [key::ABOUT_GITHUB, key::ABOUT_STAR], t(cx, key::ABOUT_GITHUB_HINT), true))
        .child(entry(cx, Icon::Discord, Link::Discord, [key::ABOUT_DISCORD, key::ABOUT_JOIN], t(cx, key::ABOUT_DISCORD_HINT), false))
        .child(entry(cx, Icon::Bug, Link::Bug, [key::ABOUT_BUG, key::ABOUT_REPORT], t(cx, key::ABOUT_BUG_HINT), false))
        .child(entry(cx, Icon::Coffee, Link::Support, [key::ABOUT_SUPPORT, key::ABOUT_COFFEE], t(cx, key::ABOUT_SUPPORT_HINT), true))
        .child(entry(cx, Icon::Mail, Link::Email, [key::ABOUT_CONTACT, key::ABOUT_EMAIL], contact, false).border_b_0())
}

fn frame(cx: &App, glyph: Icon, lit: bool) -> Div {
    h_flex().min_h(px(64.)).gap(px(16.)).border_b_1().border_color(line(0.07)).child(icon(
        glyph,
        20.,
        rgb(if lit { accent(cx) } else { DIM }),
    ))
}

/// A row of the tab: a glyph (`lit` draws it in the accent), a title with its hint, and room for what the row offers.
pub fn tab_row(cx: &App, glyph: Icon, lit: bool, title: impl Into<SharedString>, hint: impl Into<SharedString>) -> Div {
    frame(cx, glyph, lit).child(label(title, hint).flex_1().min_w_0())
}

/// A `tab_row` with its controls under the text, so a long hint keeps the whole width.
pub fn stacked_row(
    cx: &App,
    glyph: Icon,
    lit: bool,
    title: impl Into<SharedString>,
    hint: impl Into<SharedString>,
    controls: impl IntoElement,
) -> Div {
    frame(cx, glyph, lit)
        .items_start()
        .py(px(14.))
        .child(v_flex().flex_1().min_w_0().gap(px(10.)).child(label(title, hint)).child(controls))
}

/// A link's row and its button.
fn entry(cx: &App, glyph: Icon, link: Link, [title, action]: [Key; 2], hint: SharedString, lit: bool) -> Div {
    let go = button(&t(cx, action)).id(title.0).on_click(move |_, _, cx| link.open(cx)).child(icon(Icon::External, 12., rgb(DIM)));
    let go = if lit { go.border_color(rgb(accent(cx))).text_color(rgb(accent(cx))) } else { go };
    tab_row(cx, glyph, lit, t(cx, title), hint).child(go)
}
