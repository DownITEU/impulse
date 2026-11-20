//! The REMOTE PANEL dialog: the server on or off, its address and pairing QR code, the paired devices, what to check.

use std::sync::Arc;

use gpui_kit::{
    App, Bounds, ClipboardItem, Context, Entity, FocusHandle, FontWeight, IntoElement, Render, Task, Window, canvas,
    component::{h_flex, scroll::ScrollableElement, v_flex},
    div, fill, point,
    prelude::*,
    px, rgb, size,
};
use i18n::{Arg, Key};
use qrcodegen::QrCode;

use super::Remote;
use crate::{
    settings::{Settings, label, row},
    ui::{
        assets::Icon,
        dialog::dialog,
        key,
        text::{t, tf},
        theme::*,
        widgets::{button, flash, mono, square, switch, tinted_icon_button, tip, tracked},
    },
};

/// A QR module's side, and the light border around the code, in modules.
const MODULE: f32 = 4.;
const QUIET: i32 = 4;

/// What keeps a phone out most often, in the order to check it.
const TIPS: [Key; 3] = [key::REMOTE_TIP_NETWORK, key::REMOTE_TIP_FIREWALL, key::REMOTE_TIP_ISOLATION];

pub struct RemoteView {
    remote: Entity<Remote>,
    settings: Entity<Settings>,
    open: bool,
    // The address was just copied; dropping it hides the confirmation.
    copied: Option<Task<()>>,
    focus: FocusHandle,
}

impl RemoteView {
    pub fn new(remote: Entity<Remote>, settings: Entity<Settings>, cx: &mut Context<Self>) -> Self {
        cx.observe(&remote, |_, _, cx| cx.notify()).detach();
        cx.observe(&settings, |_, _, cx| cx.notify()).detach();
        Self { remote, settings, open: false, copied: None, focus: cx.focus_handle() }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn serving(&self, cx: &App) -> bool {
        self.remote.read(cx).link().is_some()
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Escape closes it, and a focused filter behind it would keep taking keys.
        self.focus.focus(window, cx);
        self.open = true;
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        cx.notify();
    }
}

impl Render for RemoteView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (remote, on) = (self.remote.read(cx), self.settings.read(cx).prefs().remote);
        let switch = switch("remote-switch", on, cx).on_click(
            cx.listener(move |view, _, _, cx| view.settings.update(cx, |settings, cx| settings.edit(|prefs| prefs.remote = !on, cx))),
        );
        let copied = self.copied.is_some();
        let pairing = remote.link().map(|(link, qr)| {
            // The tooltip stays the same: GPUI keeps an open one until the pointer leaves, so only the glyph confirms.
            let (text, copy) = (t(cx, key::REMOTE_COPY), link.clone());
            let copier = tinted_icon_button(
                "remote-copy",
                if copied { Icon::Check } else { Icon::Copy },
                28.,
                14.,
                if copied { accent(cx) } else { DIM },
            )
            .border_1()
            .border_color(line(0.16))
            .tooltip(move |window, cx| tip(text.clone(), window, cx))
            .on_click(cx.listener(move |view, _, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(copy.to_string()));
                view.copied = Some(flash(cx, |view: &mut Self| view.copied = None));
            }));
            h_flex().gap(px(18.)).py(px(14.)).border_b_1().border_color(line(0.07)).children(qr.map(code)).child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(12.))
                    .child(label(t(cx, key::REMOTE_ADDRESS), t(cx, key::REMOTE_ADDRESS_HINT)))
                    .child(h_flex().gap(px(10.)).child(mono(link, 13.).min_w_0().text_color(rgb(TEXT_BRIGHT))).child(copier)),
            )
        });
        let devices = remote.devices().iter().map(|device| {
            let id = device.id;
            let name = match device.name.is_empty() {
                true => t(cx, key::REMOTE_UNKNOWN_DEVICE),
                false => device.name.to_uppercase().into(),
            };
            row(name, tf(cx, key::REMOTE_DEVICE, &[("number", Arg::Num(f64::from(id)))]), 52.).child(
                button(&t(cx, key::REMOTE_REVOKE))
                    .id(("revoke", id as usize))
                    .on_click(cx.listener(move |view, _, _, cx| view.remote.update(cx, |remote, cx| remote.revoke(id, cx)))),
            )
        });
        let tips = TIPS.map(|tip| {
            h_flex()
                .items_start()
                .gap(px(10.))
                .child(square(4., rgb(accent(cx))).mt(px(6.)))
                .child(mono(t(cx, tip), 11.).flex_1().min_w_0().font_weight(FontWeight::NORMAL).text_color(rgb(DIM)))
        });
        let body = v_flex()
            .px(px(18.))
            .py(px(10.))
            .child(row(t(cx, key::REMOTE_SWITCH), t(cx, key::REMOTE_HINT), 64.).child(switch))
            .children(pairing)
            .when(remote.failed(), |body| body.child(mono(t(cx, key::REMOTE_FAILED), 11.).py(px(10.)).text_color(rgb(WARN))))
            .when(on, |body| {
                body.child(
                    v_flex()
                        .gap(px(8.))
                        .py(px(14.))
                        .border_b_1()
                        .border_color(line(0.07))
                        .child(tracked(&t(cx, key::REMOTE_TIPS), 12., 0.1).font_weight(FontWeight::SEMIBOLD))
                        .children(tips),
                )
            })
            .child(row(t(cx, key::REMOTE_DEVICES), t(cx, key::REMOTE_DEVICES_HINT), 52.))
            .children(devices)
            .when(remote.devices().is_empty(), |body| {
                body.child(mono(t(cx, key::REMOTE_NO_DEVICES), 11.).py(px(10.)).text_color(rgb(FAINT)))
            })
            .child(mono(t(cx, key::REMOTE_TRUST), 11.).py(px(14.)).text_color(rgb(WARN)));
        let body = div().id("remote-page").overflow_y_scrollbar().child(body);
        dialog("remote", t(cx, key::REMOTE), 560., &self.focus, |view: &mut Self, _, cx| view.close(cx), body, cx)
    }
}

/// Dark modules on a light square, painted: one quad each.
fn code(qr: Arc<QrCode>) -> impl IntoElement {
    let side = (qr.size() + 2 * QUIET) as f32 * MODULE;
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            window.paint_quad(fill(bounds, rgb(QR_LIGHT)));
            for (x, y) in (0..qr.size()).flat_map(|y| (0..qr.size()).map(move |x| (x, y))).filter(|&(x, y)| qr.get_module(x, y)) {
                let at = point(bounds.left() + px((x + QUIET) as f32 * MODULE), bounds.top() + px((y + QUIET) as f32 * MODULE));
                window.paint_quad(fill(Bounds::new(at, size(px(MODULE), px(MODULE))), rgb(QR_DARK)));
            }
        },
    )
    .size(px(side))
    .flex_none()
}
