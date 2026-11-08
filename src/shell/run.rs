//! Running the open game's trainer: LAUNCH + ATTACH, the cheat values going to it, STOP.

use std::{mem, sync::Arc};

use anyhow::{Context as _, Result, anyhow};
use futures::{StreamExt, channel::mpsc::Receiver};
use gpui_kit::{Context, Div, Task, Window, prelude::*, rgb};

use super::{Page, theme::*, widgets::mono};
use crate::play::{
    process::Rule,
    run::{Request, Stage, run},
    tophat::{self, Event, Outcome, Server},
};

pub(super) enum Play {
    Idle,
    /// Dropping the task abandons the run.
    Busy {
        game: Arc<str>,
        stage: Stage,
        _run: Task<()>,
    },
    Live {
        game: Arc<str>,
        session: Arc<tophat::Session>,
        _events: Task<()>,
    },
    /// The trainer is being taken down: a new run waits until it is gone.
    Stopping {
        game: Arc<str>,
        _stop: Task<()>,
    },
    /// Stays until the next run, so the reason is still there when the user looks.
    Failed {
        game: Arc<str>,
        error: String,
    },
}

impl Play {
    pub fn game(&self) -> Option<&Arc<str>> {
        match self {
            Self::Idle => None,
            Self::Busy { game, .. } | Self::Live { game, .. } | Self::Stopping { game, .. } | Self::Failed { game, .. } => {
                Some(game)
            }
        }
    }

    pub fn live(&self) -> Option<&Arc<str>> {
        match self {
            Self::Live { game, .. } => Some(game),
            _ => None,
        }
    }

    /// The run's variables came from its cheat list, so that list must stay as it is.
    pub fn holds(&self, id: &Arc<str>) -> bool {
        matches!(self, Self::Busy { game, .. } | Self::Live { game, .. } if game == id)
    }

    /// A run is under way, in place or ending: another cannot start.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Busy { .. } | Self::Live { .. } | Self::Stopping { .. })
    }
}

pub(super) const STOPPING: &str = "STOPPING THE TRAINER";

pub(super) fn label(stage: Stage) -> &'static str {
    match stage {
        Stage::Downloading => "DOWNLOADING THE TRAINER",
        Stage::Launching => "STARTING THE GAME",
        Stage::WaitingForGame => "WAITING FOR THE GAME",
        Stage::Attaching => "LOADING THE TRAINER",
    }
}

impl Page {
    pub(super) fn start_run(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else { return };
        if self.play.is_active() {
            return;
        }
        let prepared = self.run_request(&id).and_then(|request| Ok((request, self.tophat_server()?)));
        let (request, server) = match prepared {
            Ok(prepared) => prepared,
            Err(e) => return self.fail_run(id, e, cx),
        };
        let game = id.clone();
        let task = cx.spawn_in(window, async move |page, cx| {
            let outcome = {
                let (progress, mut progress_cx) = (page.clone(), cx.clone());
                run(&server, request, move |stage| {
                    progress.update(&mut progress_cx, |page, cx| page.run_stage(stage, cx)).ok();
                })
                .await
            };
            page.update(cx, |page, cx| page.run_finished(game, outcome, cx)).ok();
        });
        self.play = Play::Busy { game: id, stage: Stage::Downloading, _run: task };
        cx.notify();
    }

    fn run_request(&self, id: &Arc<str>) -> Result<Request> {
        let game = self.games().iter().find(|game| game.id == *id).context("the game is not in the library")?;
        let plan = &self.trainer(Some(id)).context("the cheat list is not loaded")?.plan;
        let binary = plan.binary.as_ref().map_err(|why| anyhow!("{why}"))?;
        Ok(Request {
            data_dir: self.data_dir.clone(),
            trainerlib: self.wand_trainerlib().context("Wand's folder is not set: pick it in Settings, APP")?,
            store_id: game.store_id.clone(),
            rule: Rule { image: plan.image.clone(), location: game.location.clone(), minimum_age: plan.minimum_age },
            launch: plan.launch,
            binary_url: binary.url.clone(),
            binary_hash: binary.hash.clone(),
            variables: plan.variables.clone(),
        })
    }

    /// One server for the app's life, started with the first run.
    fn tophat_server(&mut self) -> Result<Arc<Server>> {
        if let Some(server) = &self.tophat {
            return Ok(server.clone());
        }
        let server = Arc::new(Server::start()?);
        self.tophat = Some(server.clone());
        Ok(server)
    }

    fn run_stage(&mut self, stage: Stage, cx: &mut Context<Self>) {
        if let Play::Busy { stage: current, .. } = &mut self.play {
            *current = stage;
            cx.notify();
        }
    }

    fn run_finished(&mut self, id: Arc<str>, outcome: Result<(tophat::Session, Receiver<Event>)>, cx: &mut Context<Self>) {
        let (session, events) = match outcome {
            Ok(attached) => attached,
            Err(e) => return self.fail_run(id, e, cx),
        };
        self.reset_values(&id);
        let watcher = cx.spawn(async move |page, cx| {
            watch(events).await;
            page.update(cx, |page, cx| page.run_ended(cx)).ok();
        });
        log::info!("trainer is live in {id}");
        self.play = Play::Live { game: id, session: Arc::new(session), _events: watcher };
        cx.notify();
    }

    /// The session ended by itself: the game closed or the service left.
    fn run_ended(&mut self, cx: &mut Context<Self>) {
        if let Play::Live { game, .. } = &self.play {
            log::info!("trainer session ended");
            let game = game.clone();
            self.play = Play::Idle;
            self.reset_values(&game);
            self.install_deferred();
            cx.notify();
        }
    }

    fn fail_run(&mut self, id: Arc<str>, error: anyhow::Error, cx: &mut Context<Self>) {
        log::warn!("run failed: {error:#}");
        self.play = Play::Failed { game: id, error: format!("{error:#}") };
        self.install_deferred();
        cx.notify();
    }

    /// Ends the run: a live trainer is stopped the way Wand stops it, and the page stays busy until
    /// that is done; a starting one is abandoned, which closes its session as it is dropped.
    pub(super) fn stop_run(&mut self, cx: &mut Context<Self>) {
        match mem::replace(&mut self.play, Play::Idle) {
            Play::Live { game, session, .. } => {
                self.reset_values(&game);
                let id = game.clone();
                let stop = cx.spawn(async move |page, cx| {
                    if let Err(e) = session.stop().await {
                        log::warn!("stopping the trainer: {e:#}");
                    }
                    page.update(cx, |page, cx| page.run_stopped(cx)).ok();
                });
                self.play = Play::Stopping { game: id, _stop: stop };
            }
            // Already on its way out.
            stopping @ Play::Stopping { .. } => self.play = stopping,
            other => {
                if let Some(game) = other.game() {
                    self.reset_values(&game.clone());
                }
            }
        }
        self.install_deferred();
        cx.notify();
    }

    fn run_stopped(&mut self, cx: &mut Context<Self>) {
        if matches!(self.play, Play::Stopping { .. }) {
            self.play = Play::Idle;
            cx.notify();
        }
    }

    fn reset_values(&mut self, id: &Arc<str>) {
        if let Some(super::Load::Ready(loaded)) = self.trainers.get_mut(id) {
            loaded.reset();
        }
    }

    /// Sends a cheat's value to the running trainer.
    pub(super) fn push_value(&self, index: usize, value: f64, cx: &mut Context<Self>) {
        let Play::Live { game, session, .. } = &self.play else { return };
        let Some(target) = self.trainer(Some(game)).map(|loaded| loaded.cheats[index].target.clone()).filter(|target| !target.is_empty()) else {
            return;
        };
        let session = session.clone();
        cx.spawn(async move |_, _| match session.set_value(&target, value).await {
            Ok(Outcome::Failed) => log::warn!("the trainer refused {target} = {value}"),
            Ok(_) => {}
            Err(e) => log::warn!("{target}: {e:#}"),
        })
        .detach();
    }

    /// The line above the cheat list while the trainer is not live.
    pub(super) fn run_note(&self, id: &Arc<str>) -> Div {
        match &self.play {
            Play::Failed { game, error } if game == id => mono(error.clone(), 11.).text_color(rgb(WARN)),
            Play::Busy { game, stage, .. } if game == id => mono(label(*stage), 11.).text_color(rgb(self.accent)),
            Play::Stopping { game, .. } if game == id => mono(STOPPING, 11.).text_color(rgb(self.accent)),
            _ => mono("TRAINER IDLE · CONTROLS UNLOCK AFTER LAUNCH", 11.).text_color(rgb(DIM)),
        }
    }
}

async fn watch(mut events: Receiver<Event>) {
    while let Some(event) = events.next().await {
        match event {
            Event::Log(text) => log::info!("trainer: {text}"),
            Event::Status(text) => log::debug!("trainer status: {text}"),
            _ => {}
        }
    }
}
