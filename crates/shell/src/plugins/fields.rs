//! A plugin's settings as `ui` controls: a switch, choices, or a field kept on Enter or when it loses the focus.

use gpui_kit::{
    AnyElement, Context, Entity, Focusable, SharedString, Subscription, Window,
    component::{
        h_flex,
        input::{Input, InputEvent, InputState},
    },
    prelude::*,
    px, rgb,
};
use model::{Installed, SettingKind, SourceId, Value};

use super::PluginsView;
use crate::{
    settings::row,
    ui::{
        key,
        text::text,
        theme::*,
        widgets::{choice, input, segmented, switch},
    },
};

/// The fields of the enabled plugins' text and number settings.
#[derive(Default)]
pub struct Fields(Vec<Field>);

struct Field {
    plugin: SourceId,
    setting: String,
    // Part of the reuse key: a setting that changes kind gets a new input.
    number: bool,
    state: Entity<InputState>,
    _relabel: Subscription,
    _keep: Subscription,
}

impl Fields {
    /// A field per text or number setting of an enabled plugin, each showing the kept value unless it is being edited.
    pub fn sync(&mut self, list: &[Installed], window: &mut Window, cx: &mut Context<PluginsView>) {
        let typed = list.iter().filter(|plugin| plugin.enabled).flat_map(|plugin| {
            plugin
                .settings
                .iter()
                .zip(&plugin.values)
                .filter(|(setting, _)| typed(&setting.kind))
                .map(move |(setting, value)| (plugin, setting, value))
        });
        let mut kept = Vec::new();
        for (plugin, setting, value) in typed {
            let number = matches!(setting.kind, SettingKind::Number { .. });
            let at = self.0.iter().position(|field| field.plugin == plugin.id && field.setting == setting.id && field.number == number);
            let field = match at {
                Some(at) => self.0.swap_remove(at),
                None => field(plugin.id.clone(), setting.id.clone(), number, window, cx),
            };
            let shown = printed(value);
            if !field.state.focus_handle(cx).is_focused(window) && field.state.read(cx).value() != shown.as_str() {
                field.state.update(cx, |state, cx| state.set_value(shown, window, cx));
            }
            kept.push(field);
        }
        self.0 = kept;
    }

    fn get(&self, plugin: &SourceId, setting: &str) -> Option<&Entity<InputState>> {
        self.0.iter().find(|field| field.plugin == *plugin && field.setting == setting).map(|field| &field.state)
    }
}

fn typed(kind: &SettingKind) -> bool {
    matches!(kind, SettingKind::Text { .. } | SettingKind::Number { .. })
}

fn field(plugin: SourceId, setting: String, number: bool, window: &mut Window, cx: &mut Context<PluginsView>) -> Field {
    let (state, relabel) = input(key::PLUGINS_VALUE, window, cx);
    let (id, name) = (plugin.clone(), setting.clone());
    let keep = cx.subscribe(&state, move |view, state, event: &InputEvent, cx| {
        if !matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
            return;
        }
        let typed = state.read(cx).value().to_string();
        let plugins = view.plugins.read(cx);
        let kept = plugins.list().iter().find(|plugin| plugin.id == id).and_then(|plugin| {
            let at = plugin.settings.iter().position(|setting| setting.id == name)?;
            plugin.values.get(at).map(printed)
        });
        if kept.as_deref() == Some(typed.as_str()) {
            return;
        }
        // A number that does not parse goes as text: the plugin's field refuses it and says why.
        let value = match (number, typed.trim().replace(',', ".").parse::<f64>()) {
            (true, Ok(number)) => Value::Number(number),
            _ => Value::Text(typed),
        };
        let (id, name) = (id.clone(), name.clone());
        view.plugins.update(cx, |plugins, cx| plugins.set_value(id, name, value, cx));
    });
    Field { plugin, setting, number, state, _relabel: relabel, _keep: keep }
}

fn printed(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        Value::Number(number) if number.fract() == 0. && number.abs() < 1e15 => format!("{number:.0}"),
        Value::Number(number) => number.to_string(),
        Value::Switch(_) => String::new(),
    }
}

/// The setting rows of an enabled plugin; `next` numbers its controls.
pub fn rows(view: &PluginsView, plugin: &Installed, next: &mut usize, cx: &mut Context<PluginsView>) -> Vec<AnyElement> {
    let mut id = || {
        *next += 1;
        ("plugin-control", *next)
    };
    plugin
        .settings
        .iter()
        .zip(&plugin.values)
        .map(|(setting, value)| {
            let hint = setting.hint.as_ref().map_or_else(SharedString::default, |hint| text(cx, hint));
            let row = row(text(cx, &setting.label), hint, 56.).border_b_0();
            let set = |value: Value| {
                let (plugin, setting) = (plugin.id.clone(), setting.id.clone());
                cx.listener(move |view: &mut PluginsView, _, _, cx| {
                    let (plugin, setting, value) = (plugin.clone(), setting.clone(), value.clone());
                    view.plugins.update(cx, |plugins, cx| plugins.set_value(plugin, setting, value, cx));
                })
            };
            match (&setting.kind, value) {
                (SettingKind::Switch, Value::Switch(on)) => {
                    row.child(switch(id(), *on, cx).on_click(set(Value::Switch(!on)))).into_any_element()
                }
                (SettingKind::Choice { options }, Value::Text(current)) => row
                    .child(segmented().h(px(26.)).children(options.iter().map(|(option, label)| {
                        choice(id(), text(cx, label), option == current, cx).on_click(set(Value::Text(option.clone())))
                    })))
                    .into_any_element(),
                _ => match view.fields.get(&plugin.id, &setting.id) {
                    Some(state) => row
                        .child(
                            h_flex()
                                .w(px(200.))
                                .h(px(32.))
                                .px(px(10.))
                                .border_1()
                                .border_color(line(0.14))
                                .bg(rgb(SUNKEN))
                                .child(Input::new(state).appearance(false).px_0().font_family(MONO).text_size(px(TYPE_FIELD))),
                        )
                        .into_any_element(),
                    None => row.into_any_element(),
                },
            }
        })
        .collect()
}
