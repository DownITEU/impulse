//! The PLUGINS tab: INSTALL and the approval card, then each plugin's switch, scopes, size, failure, REMOVE and
//! settings.

use gpui_kit::{
    AnyElement, Context, Div, Entity, IntoElement, Render, SharedString, Window,
    component::{h_flex, v_flex},
    prelude::*,
    px, rgb,
};
use i18n::{Arg, Key};
use model::{Approval, Installed};

use super::{Plugins, fields};
use crate::{
    resources::sized,
    settings::{label, row},
    ui::{
        key,
        text::{t, tf, why},
        theme::*,
        widgets::{button, mono, switch, tracked},
    },
};

pub struct PluginsView {
    pub(super) plugins: Entity<Plugins>,
    pub(super) fields: fields::Fields,
}

impl PluginsView {
    pub fn new(plugins: Entity<Plugins>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe_in(&plugins, window, |view, plugins, window, cx| {
            let list = plugins.read(cx).list().to_vec();
            view.fields.sync(&list, window, cx);
            cx.notify();
        })
        .detach();
        Self { plugins, fields: fields::Fields::default() }
    }

    fn approval(&self, approval: &Approval, cx: &mut Context<Self>) -> Div {
        let digest = approval.digest.clone();
        let title = tf(cx, key::PLUGINS_APPROVE_TITLE, &[("name", Arg::Str(&approval.name)), ("version", Arg::Str(&approval.version))]);
        v_flex()
            .gap(px(10.))
            .my(px(12.))
            .p(px(14.))
            .border_1()
            .border_color(rgb(accent(cx)))
            .child(tracked(&title, 13., 0.1).text_color(rgb(TEXT_BRIGHT)))
            .children(
                approval
                    .replaces
                    .as_ref()
                    .map(|version| mono(tf(cx, key::PLUGINS_REPLACES, &[("version", Arg::Str(version))]), 11.).text_color(rgb(FAINT))),
            )
            .child(scope(key::PLUGINS_HOSTS, &approval.hosts, &approval.widened, cx))
            .child(scope(key::PLUGINS_LINKS, &approval.links, &approval.widened, cx))
            .child(mono(t(cx, key::PLUGINS_SANDBOX), 11.).text_color(rgb(WARN)))
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(
                        button(&t(cx, key::PLUGINS_APPROVE)).id("plugin-approve").on_click(
                            cx.listener(move |view, _, _, cx| view.plugins.update(cx, |plugins, cx| plugins.approve(&digest, cx))),
                        ),
                    )
                    .child(
                        button(&t(cx, key::PLUGINS_CANCEL))
                            .id("plugin-cancel")
                            .on_click(cx.listener(|view, _, _, cx| view.plugins.update(cx, |plugins, cx| plugins.cancel(cx)))),
                    ),
            )
    }

    fn plugin(&self, at: usize, plugin: &Installed, next: &mut usize, cx: &mut Context<Self>) -> Div {
        let size = sized(cx, plugin.bytes);
        let meta = tf(cx, key::PLUGINS_VERSION, &[("version", Arg::Str(&plugin.version)), ("size", Arg::Str(&size))]);
        let (id, on) = (plugin.id.clone(), plugin.enabled);
        let remove = plugin.id.clone();
        let head = h_flex()
            .gap(px(14.))
            .child(label(SharedString::from(plugin.name.to_uppercase()), meta).flex_1().min_w_0())
            .child(switch(("plugin-on", at), on, cx).on_click(cx.listener(move |view, _, _, cx| {
                let id = id.clone();
                view.plugins.update(cx, |plugins, cx| plugins.set_enabled(id, !on, cx));
            })))
            .child(button(&t(cx, key::PLUGINS_REMOVE)).id(("plugin-remove", at)).on_click(cx.listener(move |view, _, _, cx| {
                let id = remove.clone();
                view.plugins.update(cx, |plugins, cx| plugins.remove(id, cx));
            })));
        let settings = if plugin.enabled { fields::rows(self, plugin, next, cx) } else { Vec::new() };
        v_flex()
            .gap(px(8.))
            .py(px(12.))
            .border_b_1()
            .border_color(line(0.07))
            .child(head)
            .child(scope(key::PLUGINS_HOSTS, &plugin.hosts, &[], cx))
            .when(!plugin.links.is_empty(), |card| card.child(scope(key::PLUGINS_LINKS, &plugin.links, &[], cx)))
            .children(plugin.failure.as_ref().map(|failure| mono(why(cx, failure), 11.).text_color(rgb(DANGER))))
            .children(settings)
    }
}

/// A scope's caption and its hosts, those not approved before marked.
fn scope(caption: Key, hosts: &[String], widened: &[String], cx: &Context<PluginsView>) -> Div {
    let hosts: Vec<AnyElement> = match hosts.is_empty() {
        true => vec![mono(t(cx, key::PLUGINS_NO_HOSTS), 11.).text_color(rgb(FAINT)).into_any_element()],
        false => hosts
            .iter()
            .map(|host| {
                h_flex()
                    .gap(px(6.))
                    .child(mono(host.clone(), 11.).text_color(rgb(TEXT)))
                    .when(widened.contains(host), |line| line.child(mono(t(cx, key::PLUGINS_NEW), 10.).text_color(rgb(accent(cx)))))
                    .into_any_element()
            })
            .collect(),
    };
    h_flex()
        .items_start()
        .gap(px(12.))
        .child(mono(t(cx, caption), 10.5).w(px(120.)).flex_none().text_color(rgb(FAINT)))
        .child(v_flex().gap(px(2.)).children(hosts))
}

impl Render for PluginsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let plugins = self.plugins.read(cx);
        let (list, approval, failure) = (plugins.list().to_vec(), plugins.approval().cloned(), plugins.failure().cloned());
        let install = row(t(cx, key::PLUGINS_INSTALL), t(cx, key::PLUGINS_INSTALL_HINT), 64.).child(
            h_flex()
                .gap(px(8.))
                .child(
                    button(&t(cx, key::PLUGINS_FOLDER))
                        .id("plugin-folder")
                        .on_click(cx.listener(|view, _, _, cx| view.plugins.update(cx, |plugins, cx| plugins.pick(false, cx)))),
                )
                .child(
                    button(&t(cx, key::PLUGINS_ZIP))
                        .id("plugin-zip")
                        .on_click(cx.listener(|view, _, _, cx| view.plugins.update(cx, |plugins, cx| plugins.pick(true, cx)))),
                ),
        );
        let mut next = 0;
        let cards: Vec<Div> = list.iter().enumerate().map(|(at, plugin)| self.plugin(at, plugin, &mut next, cx)).collect();
        v_flex()
            .px(px(18.))
            .py(px(6.))
            .child(install)
            .children(failure.map(|failure| mono(why(cx, &failure), 11.).pt(px(10.)).text_color(rgb(DANGER))))
            .children(approval.map(|approval| self.approval(&approval, cx)))
            .when(list.is_empty(), |tab| tab.child(mono(t(cx, key::PLUGINS_NONE), 11.).py(px(14.)).text_color(rgb(FAINT))))
            .children(cards)
    }
}
