mod library;

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

const LABELS: [&str; 3] = ["Session", "API check", "Library"];
const SESSION: usize = 0;
const API_CHECK: usize = 1;
const LIBRARY: usize = 2;

enum Status {
    Pending,
    Running,
    Ok(String),
    Failed(String),
}

pub struct Shell {
    transport: Result<Transport, String>,
    data_dir: PathBuf,
    steps: [Status; 3],
}

impl Shell {
    pub fn new(transport: Result<Transport>, data_dir: PathBuf, cx: &mut Context<Self>) -> Self {
        let mut shell = Self {
            transport: transport.map_err(|e| format!("{e:#}")),
            data_dir,
            steps: [Status::Pending, Status::Pending, Status::Pending],
        };
        shell.run(cx);
        shell
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        self.steps = [Status::Pending, Status::Pending, Status::Running];
        let dir = self.data_dir.clone();
        cx.spawn(async move |this, cx| {
            let scanned = blocking::unblock(move || library::scan(&dir)).await;
            report(&this, cx, LIBRARY, scanned, Clone::clone);
        })
        .detach();
        match self.transport.clone() {
            Ok(t) => {
                self.set(SESSION, Status::Running, cx);
                let vault = self.data_dir.join("session.bin");
                cx.spawn(async move |this, cx| drive(this, cx, t, vault).await)
                    .detach();
            }
            Err(e) => self.set(SESSION, Status::Failed(e), cx),
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
    let Some((session, _)) = report(&this, cx, SESSION, connected, |(s, source)| {
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
    let _ = this.update(cx, |shell, cx| shell.set(API_CHECK, Status::Running, cx));
    let trainers = session.check(&t).await;
    report(&this, cx, API_CHECK, trainers, |n| {
        format!("Enshrouded trainers: {n}")
    });
}

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
    let _ = this.update(cx, |shell, cx| shell.set(step, status, cx));
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
