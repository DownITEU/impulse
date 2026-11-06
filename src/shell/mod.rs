use anyhow::Result;
use gpui_kit::{
    AsyncApp, Context, IntoElement, ParentElement, Render, Styled, WeakEntity, Window,
    component::{ActiveTheme, Disableable, button::Button, h_flex, v_flex},
    div, px,
};

use std::path::PathBuf;

use crate::{
    session::{self, Source},
    transport::Transport,
};

const LABELS: [&str; 2] = ["Session", "API check"];

enum Status {
    Pending,
    Running,
    Ok(String),
    Failed(String),
}

pub struct Shell {
    transport: Result<Transport, String>,
    vault: PathBuf,
    steps: [Status; 2],
}

impl Shell {
    pub fn new(transport: Result<Transport>, vault: PathBuf, cx: &mut Context<Self>) -> Self {
        let mut shell = Self {
            transport: transport.map_err(|e| format!("{e:#}")),
            vault,
            steps: [Status::Pending, Status::Pending],
        };
        shell.run(cx);
        shell
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        self.steps = [Status::Pending, Status::Pending];
        match self.transport.clone() {
            Ok(t) => {
                self.set(0, Status::Running, cx);
                let vault = self.vault.clone();
                cx.spawn(async move |this, cx| drive(this, cx, t, vault).await)
                    .detach();
            }
            Err(e) => self.set(0, Status::Failed(e), cx),
        }
    }

    fn set(&mut self, step: usize, status: Status, cx: &mut Context<Self>) {
        if let Status::Ok(text) | Status::Failed(text) = &status {
            eprintln!("{}: {text}", LABELS[step]);
        }
        self.steps[step] = status;
        cx.notify();
    }
}

async fn drive(this: WeakEntity<Shell>, cx: &mut AsyncApp, t: Transport, vault: PathBuf) {
    let connected = session::connect(&t, vault).await;
    let Some((session, _)) = report(&this, cx, 0, connected, |(s, source)| {
        let how = match source {
            Source::Restored => "restored",
            Source::Refreshed => "refreshed",
            Source::Wand => "new from Wand",
        };
        let days = (s.expires_in().as_secs() + 43_200) / 86_400;
        format!("{how}, expires in {days} d")
    }) else {
        return;
    };
    let trainers = session.check(&t).await;
    report(&this, cx, 1, trainers, |n| {
        format!("Enshrouded trainers: {n}")
    });
}

// Records the step outcome and starts the next step on success.
fn report<T>(
    this: &WeakEntity<Shell>,
    cx: &mut AsyncApp,
    step: usize,
    result: Result<T>,
    detail: impl FnOnce(&T) -> String,
) -> Option<T> {
    let (status, value) = match result {
        Ok(v) => (Status::Ok(detail(&v)), Some(v)),
        Err(e) => (Status::Failed(format!("{e:#}")), None),
    };
    let next = value.is_some() && step + 1 < LABELS.len();
    let _ = this.update(cx, |shell, cx| {
        shell.set(step, status, cx);
        if next {
            shell.set(step + 1, Status::Running, cx);
        }
    });
    value
}

impl Render for Shell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (bg, fg, muted, ok, bad) = (
            theme.background,
            theme.foreground,
            theme.muted_foreground,
            theme.success,
            theme.danger,
        );
        let failed = self.steps.iter().any(|s| matches!(s, Status::Failed(_)));
        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .bg(bg)
            .text_color(fg)
            .children(LABELS.iter().zip(&self.steps).map(|(label, status)| {
                let (text, color) = match status {
                    Status::Pending => ("pending", muted),
                    Status::Running => ("running", fg),
                    Status::Ok(text) => (text.as_str(), ok),
                    Status::Failed(text) => (text.as_str(), bad),
                };
                h_flex()
                    .gap_3()
                    .items_start()
                    .child(div().w(px(150.)).flex_none().child(*label))
                    .child(div().flex_1().text_color(color).child(text.to_owned()))
            }))
            .child(
                div().child(
                    Button::new("retry")
                        .label("Retry")
                        .disabled(!failed)
                        .on_click(cx.listener(|shell, _, _, cx| shell.run(cx))),
                ),
            )
    }
}
