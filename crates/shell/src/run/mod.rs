//! Running the open game's trainer: LAUNCH + ATTACH, the cheat values going to it, STOP, and what a run keeps.

mod tracker;
mod view;

use std::{
    mem,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result};
use client::{
    profile::Values,
    run::{self, Request, Stage},
    trainer::Step,
};
use futures::{StreamExt, stream::BoxStream};
use gpui_kit::{App, AppContext as _, Context, Entity, EventEmitter, Task};
use model::{Event, Live, Process, Session};
pub use tracker::{Fix, Lease, Tracked, Tracker};
pub use view::{STOPPING, label, run_note};

use crate::{
    library::{Library, LibraryEvent},
    trainer::{Hold, Trainers},
};

pub enum RunEvent {
    /// A trainer went live.
    Live,
    /// STOP: the trainer is being taken down.
    Stopping,
    /// No run any more: it ended by itself, its stop returned, or its start was abandoned.
    Ended,
    /// A hotkey pressed a button cheat.
    Pressed(usize),
}

/// A live run as the map follows it.
#[derive(Clone)]
pub struct Run {
    pub session: Arc<dyn Session>,
    /// When the trainer can tell where the player is.
    pub tracker: Option<Entity<Tracker>>,
    pub teleport: bool,
}

/// A game's run, as its button and note show it.
pub enum State<'a> {
    Idle,
    Starting(Stage),
    Live,
    Stopping,
    Failed(&'a str),
}

enum Play {
    Idle,
    /// Dropping the task abandons the run.
    Busy {
        game: Arc<str>,
        stage: Stage,
        // The run's variables come from the shown cheat list.
        hold: Hold,
        _run: Task<()>,
    },
    Live {
        game: Arc<str>,
        /// The game's process, for hotkeys that fire only while it is in front.
        pid: u32,
        since: Instant,
        /// Its time and mods are kept.
        closed: bool,
        run: Run,
        _hold: Hold,
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

pub struct Runs {
    library: Entity<Library>,
    trainers: Entity<Trainers>,
    play: Play,
}

impl EventEmitter<RunEvent> for Runs {}

impl Runs {
    pub fn new(library: Entity<Library>, trainers: Entity<Trainers>, cx: &mut Context<Self>) -> Self {
        cx.subscribe(&library, |runs, library, event: &LibraryEvent, cx| {
            if matches!(event, LibraryEvent::Listed) && runs.game().is_some_and(|game| library.read(cx).game(game).is_none()) {
                runs.stop(cx);
            }
        })
        .detach();
        // A run's time and mods are kept when it ends, and so do closing the window and quitting.
        cx.on_app_quit(|runs, cx| {
            runs.close(cx);
            async {}
        })
        .detach();
        cx.on_release(|runs, cx| runs.close(cx)).detach();
        Self { library, trainers, play: Play::Idle }
    }

    pub fn live(&self) -> Option<&Arc<str>> {
        self.live_run().map(|(game, _)| game)
    }

    pub fn is_live(&self, game: &str) -> bool {
        self.live().is_some_and(|live| **live == *game)
    }

    pub fn live_run(&self) -> Option<(&Arc<str>, &Run)> {
        match &self.play {
            Play::Live { game, run, .. } => Some((game, run)),
            _ => None,
        }
    }

    /// The live game's process.
    pub fn pid(&self) -> Option<u32> {
        match self.play {
            Play::Live { pid, .. } => Some(pid),
            _ => None,
        }
    }

    /// When the live run began.
    pub fn since(&self) -> Option<Instant> {
        match self.play {
            Play::Live { since, .. } => Some(since),
            _ => None,
        }
    }

    /// A run is under way, in place or ending: another cannot start.
    pub fn is_active(&self) -> bool {
        matches!(self.play, Play::Busy { .. } | Play::Live { .. } | Play::Stopping { .. })
    }

    pub fn is_stopping(&self) -> bool {
        matches!(self.play, Play::Stopping { .. })
    }

    /// An active run of `game`: it keeps its install.
    pub fn holds(&self, game: &str) -> bool {
        self.is_active() && self.game().is_some_and(|at| **at == *game)
    }

    pub fn state(&self, game: &str) -> State<'_> {
        if self.game().is_none_or(|at| **at != *game) {
            return State::Idle;
        }
        match &self.play {
            Play::Busy { stage, .. } => State::Starting(*stage),
            Play::Live { .. } => State::Live,
            Play::Stopping { .. } => State::Stopping,
            Play::Failed { error, .. } => State::Failed(error),
            Play::Idle => State::Idle,
        }
    }

    pub fn start(&mut self, game: Arc<str>, cx: &mut Context<Self>) {
        if self.is_active() {
            return;
        }
        let request = match self.request(&game, cx) {
            Ok(request) => request,
            Err(e) => return self.fail(game, e, cx),
        };
        let hold = self.trainers.update(cx, |trainers, cx| trainers.hold(game.clone(), cx));
        let id = game.clone();
        let task = cx.spawn(async move |runs, cx| {
            let outcome = {
                let (progress, mut progress_cx) = (runs.clone(), cx.clone());
                run::start(request, move |stage| {
                    progress.update(&mut progress_cx, |runs, cx| runs.staged(stage, cx)).ok();
                })
                .await
            };
            runs.update(cx, |runs, cx| runs.attached(id, outcome, cx)).ok();
        });
        self.play = Play::Busy { game, stage: Stage::Preparing, hold, _run: task };
        cx.notify();
    }

    /// Ends the run: a live trainer is stopped the way Wand stops it, and the run stays until that is done; a
    /// starting one is abandoned, which closes its session as it is dropped.
    pub fn stop(&mut self, cx: &mut Context<Self>) {
        self.close(cx);
        match mem::replace(&mut self.play, Play::Idle) {
            Play::Live { game, run: Run { session, .. }, .. } => {
                self.trainers.update(cx, |trainers, cx| trainers.reset(&game, cx));
                let stop = cx.spawn(async move |runs, cx| {
                    if let Err(e) = session.stop().await {
                        log::warn!("stopping the trainer: {e:#}");
                    }
                    runs.update(cx, |runs, cx| runs.stopped(cx)).ok();
                });
                self.play = Play::Stopping { game, _stop: stop };
                cx.emit(RunEvent::Stopping);
            }
            // Already on its way out.
            stopping @ Play::Stopping { .. } => self.play = stopping,
            Play::Busy { game, .. } => {
                self.trainers.update(cx, |trainers, cx| trainers.reset(&game, cx));
                cx.emit(RunEvent::Ended);
            }
            Play::Failed { .. } | Play::Idle => {}
        }
        cx.notify();
    }

    /// Sets a live cheat and sends it.
    pub fn set(&mut self, index: usize, value: f64, cx: &mut Context<Self>) {
        let Some(game) = self.live().cloned() else { return };
        self.trainers.update(cx, |trainers, cx| trainers.set(&game, index, value, cx));
        self.send(index, value, cx);
    }

    /// A hotkey slot of a live cheat.
    pub fn step(&mut self, index: usize, slot: usize, cx: &mut Context<Self>) {
        let Some(game) = self.live() else { return };
        match self.trainers.read(cx).sheet(game).and_then(|sheet| sheet.step(index, slot)) {
            Some(Step::Press(pressed)) => {
                self.send(index, pressed, cx);
                cx.emit(RunEvent::Pressed(index));
            }
            Some(Step::Set(next)) => self.set(index, next, cx),
            None => {}
        }
    }

    /// A live button cheat's press: sent, kept nowhere.
    pub fn press(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(game) = self.live() else { return };
        if let Some(pressed) = self.trainers.read(cx).sheet(game).map(|sheet| sheet.cheats[index].args.pressed) {
            self.send(index, pressed, cx);
        }
    }

    /// Sets the live cheats to `values`, as moving each control would.
    pub fn apply(&mut self, values: &Values, cx: &mut Context<Self>) {
        let Some(game) = self.live() else { return };
        let Some(changes) = self.trainers.read(cx).sheet(game).map(|sheet| sheet.changes(values)) else { return };
        for (index, value) in changes {
            self.set(index, value, cx);
        }
    }

    /// The run's game, whatever its state.
    fn game(&self) -> Option<&Arc<str>> {
        match &self.play {
            Play::Idle => None,
            Play::Busy { game, .. } | Play::Live { game, .. } | Play::Stopping { game, .. } | Play::Failed { game, .. } => Some(game),
        }
    }

    fn request(&self, game: &str, cx: &App) -> Result<Request> {
        let listed = self.library.read(cx).game(game).context("the game is not in the library")?;
        let sheet = self.trainers.read(cx).sheet(game).context("the cheat list is not loaded")?;
        Ok(run::request(&listed.install, &sheet.plan))
    }

    fn staged(&mut self, stage: Stage, cx: &mut Context<Self>) {
        if let Play::Busy { stage: current, .. } = &mut self.play {
            *current = stage;
            cx.notify();
        }
    }

    fn attached(&mut self, game: Arc<str>, outcome: Result<(Process, Live)>, cx: &mut Context<Self>) {
        let Play::Busy { hold, .. } = mem::replace(&mut self.play, Play::Idle) else { return };
        let (process, Live { session, events }) = match outcome {
            Ok(attached) => attached,
            Err(e) => return self.fail(game, e, cx),
        };
        log::info!("trainer is live in {game}");
        let session: Arc<dyn Session> = Arc::from(session);
        let watcher = cx.spawn(async move |runs, cx| {
            watch(events).await;
            runs.update(cx, |runs, cx| runs.ended(cx)).ok();
        });
        let clock = cx.spawn(async move |runs, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if runs.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        let (location, teleport) = self.trainers.read(cx).sheet(&game).map_or((false, false), |sheet| (sheet.location, sheet.teleport));
        let tracker = location.then(|| cx.new(|_| Tracker::new(session.clone())));
        self.trainers.update(cx, |trainers, cx| trainers.reset(&game, cx));
        self.play = Play::Live {
            game: game.clone(),
            pid: process.pid,
            since: Instant::now(),
            closed: false,
            run: Run { session, tracker, teleport },
            _hold: hold,
            _events: watcher,
            _clock: clock,
        };
        // Counts the run and sets the mods kept for it.
        if let Some(kept) = self.trainers.update(cx, |trainers, cx| trainers.run_opened(&game, cx)) {
            self.apply(&kept, cx);
        }
        cx.emit(RunEvent::Live);
        cx.notify();
    }

    /// The session ended by itself: the game closed or the service left.
    fn ended(&mut self, cx: &mut Context<Self>) {
        let Some(game) = self.live().cloned() else { return };
        log::info!("trainer session ended");
        self.close(cx);
        self.play = Play::Idle;
        self.trainers.update(cx, |trainers, cx| trainers.reset(&game, cx));
        cx.emit(RunEvent::Ended);
        cx.notify();
    }

    fn fail(&mut self, game: Arc<str>, error: anyhow::Error, cx: &mut Context<Self>) {
        log::warn!("run failed: {error:#}");
        self.play = Play::Failed { game, error: format!("{error:#}") };
        cx.notify();
    }

    fn stopped(&mut self, cx: &mut Context<Self>) {
        if self.is_stopping() {
            self.play = Play::Idle;
            cx.emit(RunEvent::Ended);
            cx.notify();
        }
    }

    /// Keeps the live run's time and what it leaves on, once: a run ends in several ways, and the window closing
    /// or the app quitting may come after any of them.
    fn close(&mut self, cx: &mut App) {
        let Play::Live { game, since, closed, .. } = &mut self.play else { return };
        if mem::replace(closed, true) {
            return;
        }
        let (game, secs) = (game.clone(), since.elapsed().as_secs());
        self.trainers.update(cx, |trainers, cx| trainers.run_closed(&game, secs, cx));
    }

    fn send(&self, index: usize, value: f64, cx: &mut Context<Self>) {
        let Play::Live { game, run, .. } = &self.play else { return };
        let Some(key) = self.trainers.read(cx).sheet(game).and_then(|sheet| sheet.cheats[index].key.clone()) else { return };
        let session = run.session.clone();
        cx.spawn(async move |_, _| {
            if let Err(e) = session.set(&key, value).await {
                log::warn!("{key} = {value}: {e:#}");
            }
        })
        .detach();
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
