//! Running the open game's trainer: LAUNCH + ATTACH, the cheat values going to it, STOP.

use std::{
    mem,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result};
use client::run::{self, Request, Stage};
use futures::{StreamExt, stream::BoxStream};
use gpui_kit::{App, Context, Div, Entity, Task, Window, prelude::*, rgb};
use model::{Event, Live, Process, Session};

use super::{Page, map::Tracker, theme::*, widgets::mono};

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
        /// The game's process, for hotkeys that fire only while it is in front.
        pid: u32,
        since: Instant,
        /// Its time and mods are kept.
        closed: bool,
        session: Arc<dyn Session>,
        /// When the trainer can tell where the player is.
        tracker: Option<Entity<Tracker>>,
        _events: Task<()>,
        /// Repaints the session clock.
        _clock: Task<()>,
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
            Self::Busy { game, .. } | Self::Live { game, .. } | Self::Stopping { game, .. } | Self::Failed { game, .. } => Some(game),
        }
    }

    pub fn live(&self) -> Option<&Arc<str>> {
        match self {
            Self::Live { game, .. } => Some(game),
            _ => None,
        }
    }

    /// The live game's process.
    pub fn pid(&self) -> Option<u32> {
        match self {
            Self::Live { pid, .. } => Some(*pid),
            _ => None,
        }
    }

    /// When the live run began.
    pub fn since(&self) -> Option<Instant> {
        match self {
            Self::Live { since, .. } => Some(*since),
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
        Stage::Preparing => "PREPARING THE TRAINER",
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
        let request = match self.run_request(&id) {
            Ok(request) => request,
            Err(e) => return self.fail_run(id, e, cx),
        };
        let game = id.clone();
        let task = cx.spawn_in(window, async move |page, cx| {
            let outcome = {
                let (progress, mut progress_cx) = (page.clone(), cx.clone());
                run::start(request, move |stage| {
                    progress.update(&mut progress_cx, |page, cx| page.run_stage(stage, cx)).ok();
                })
                .await
            };
            page.update_in(cx, |page, window, cx| page.run_finished(game, outcome, window, cx)).ok();
        });
        self.play = Play::Busy { game: id, stage: Stage::Preparing, _run: task };
        cx.notify();
    }

    fn run_request(&self, id: &Arc<str>) -> Result<Request> {
        let game = self.games().iter().find(|game| game.id == *id).context("the game is not in the library")?;
        let loaded = self.trainer(Some(id)).context("the cheat list is not loaded")?;
        Ok(run::request(&game.install, &loaded.sheet.plan))
    }

    fn run_stage(&mut self, stage: Stage, cx: &mut Context<Self>) {
        if let Play::Busy { stage: current, .. } = &mut self.play {
            *current = stage;
            cx.notify();
        }
    }

    fn run_finished(&mut self, id: Arc<str>, outcome: Result<(Process, Live)>, window: &mut Window, cx: &mut Context<Self>) {
        let (process, Live { session, events }) = match outcome {
            Ok(attached) => attached,
            Err(e) => return self.fail_run(id, e, cx),
        };
        self.reset_values(&id);
        let watcher = cx.spawn(async move |page, cx| {
            watch(events).await;
            page.update(cx, |page, cx| page.run_ended(cx)).ok();
        });
        let clock = cx.spawn(async move |page, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if page.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        log::info!("trainer is live in {id}");
        let session: Arc<dyn Session> = Arc::from(session);
        super::crash::follow(&session);
        let located = self.trainer(Some(&id)).is_some_and(|loaded| loaded.sheet.location);
        let tracker = located.then(|| cx.new(|_| Tracker::new(session.clone())));
        self.play = Play::Live {
            game: id,
            pid: process.pid,
            since: Instant::now(),
            closed: false,
            session,
            tracker,
            _events: watcher,
            _clock: clock,
        };
        self.open_session(window, cx);
        self.sync_map(cx);
        self.layer.read(cx).recheck();
        self.arm_hotkeys(window, cx);
        cx.notify();
    }

    /// The session ended by itself: the game closed or the service left.
    fn run_ended(&mut self, cx: &mut Context<Self>) {
        if let Play::Live { game, .. } = &self.play {
            log::info!("trainer session ended");
            let game = game.clone();
            self.close_session();
            self.play = Play::Idle;
            self.sync_map(cx);
            self.layer.read(cx).recheck();
            self.bindings.disarm();
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
        self.bindings.disarm();
        self.close_session();
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
        self.sync_map(cx);
        self.layer.read(cx).recheck();
        self.install_deferred();
        cx.notify();
    }

    fn run_stopped(&mut self, cx: &mut Context<Self>) {
        if self.quitting {
            return cx.quit();
        }
        if matches!(self.play, Play::Stopping { .. }) {
            self.play = Play::Idle;
            cx.notify();
        }
    }

    fn reset_values(&mut self, id: &Arc<str>) {
        if let Some(super::Load::Ready(loaded)) = self.trainers.get_mut(id) {
            loaded.sheet.reset();
        }
    }

    /// Sends a cheat's value to the running trainer.
    pub(super) fn push_value(&self, index: usize, value: f64, cx: &mut Context<Self>) {
        let Play::Live { game, session, .. } = &self.play else { return };
        let Some(key) = self.trainer(Some(game)).and_then(|loaded| loaded.sheet.cheats[index].key.clone()) else {
            return;
        };
        let session = session.clone();
        cx.spawn(async move |_, _| {
            if let Err(e) = session.set(&key, value).await {
                log::warn!("{key} = {value}: {e:#}");
            }
        })
        .detach();
    }

    /// The line above the cheat list while the trainer is not live.
    pub(super) fn run_note(&self, id: &Arc<str>, cx: &App) -> Div {
        match &self.play {
            Play::Failed { game, error } if game == id => mono(error.clone(), 11.).text_color(rgb(WARN)),
            Play::Busy { game, stage, .. } if game == id => mono(label(*stage), 11.).text_color(rgb(accent(cx))),
            Play::Stopping { game, .. } if game == id => mono(STOPPING, 11.).text_color(rgb(accent(cx))),
            _ => mono("TRAINER IDLE · CONTROLS UNLOCK AFTER LAUNCH", 11.).text_color(rgb(DIM)),
        }
    }
}

async fn watch(mut events: BoxStream<'static, Event>) {
    while let Some(event) = events.next().await {
        match event {
            Event::Log(text) => log::info!("trainer: {text}"),
            Event::Status(text) => log::debug!("trainer status: {text}"),
        }
    }
}
