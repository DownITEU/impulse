//! The REMOTE tab: the server on or off, its address and the pairing QR code, the paired devices, the warning.

use std::sync::Arc;

use gpui_kit::{
    Bounds, Context, Entity, IntoElement, Render, Window, canvas, component::v_flex, div, fill, point, prelude::*, px, rgb, size,
};
use i18n::Arg;
use qrcodegen::QrCode;

use super::Remote;
use crate::{
    settings::{Settings, row},
    ui::{
        key,
        text::{t, tf},
        theme::*,
        widgets::{button, mono, toggle},
    },
};

/// A QR module's side, and the light border around the code, in modules.
const MODULE: f32 = 4.;
const QUIET: i32 = 4;

pub struct RemoteTab {
    remote: Entity<Remote>,
    settings: Entity<Settings>,
}

impl RemoteTab {
    pub fn new(remote: Entity<Remote>, settings: Entity<Settings>, cx: &mut Context<Self>) -> Self {
        cx.observe(&remote, |_, _, cx| cx.notify()).detach();
        cx.observe(&settings, |_, _, cx| cx.notify()).detach();
        Self { remote, settings }
    }
}

impl Render for RemoteTab {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (remote, on) = (self.remote.read(cx), self.settings.read(cx).prefs().remote);
        let switch = div()
            .id("remote")
            .cursor_pointer()
            .on_click(
                cx.listener(move |tab, _, _, cx| tab.settings.update(cx, |settings, cx| settings.edit(|prefs| prefs.remote = !on, cx))),
            )
            .child(toggle(on, cx));
        let address = remote.address().map(|(address, qr)| {
            v_flex()
                .child(
                    row(t(cx, key::REMOTE_ADDRESS), t(cx, key::REMOTE_ADDRESS_HINT), 64.)
                        .child(mono(format!("http://{address}"), 12.).text_color(rgb(TEXT_BRIGHT))),
                )
                .when_some(qr, |column, qr| column.child(div().py(px(14.)).border_b_1().border_color(line(0.07)).child(code(qr))))
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
                    .on_click(cx.listener(move |tab, _, _, cx| tab.remote.update(cx, |remote, cx| remote.revoke(id, cx)))),
            )
        });
        v_flex()
            .px(px(18.))
            .py(px(10.))
            .child(row(t(cx, key::REMOTE), t(cx, key::REMOTE_HINT), 64.).child(switch))
            .children(address)
            .when(remote.failed(), |body| body.child(mono(t(cx, key::REMOTE_FAILED), 11.).py(px(10.)).text_color(rgb(WARN))))
            .child(row(t(cx, key::REMOTE_DEVICES), t(cx, key::REMOTE_DEVICES_HINT), 52.))
            .children(devices)
            .when(remote.devices().is_empty(), |body| {
                body.child(mono(t(cx, key::REMOTE_NO_DEVICES), 11.).py(px(10.)).text_color(rgb(FAINT)))
            })
            .child(mono(t(cx, key::REMOTE_TRUST), 11.).py(px(14.)).text_color(rgb(WARN)))
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
}
