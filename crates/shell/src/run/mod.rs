//! Running the open game's trainer: LAUNCH + ATTACH, the cheat values going to it, STOP, and what a run keeps.

mod tracker;
mod view;

use crate::ui::{key, text::app_text};
use std::{
    mem,
    sync::Arc,
    time::{Duration, Instant},
};

use client::{
    library::{Game, GameId},
    profile::Values,
    run::{self, Request, Stage},
    trainer::Step,
};
use futures::{StreamExt, stream::BoxStream};
use gpui_kit::{App, AppContext as _, Context, Entity, EventEmitter, Task};
use model::{Event, Failure, FailureKind, Live, Process, Session};
pub use tracker::{Fix, Lease, Tracked, Tracker};
pub use view::{run_button, run_note, session_panel};

use crate::{
    library::{Library, LibraryEvent},
    trainer::{Hold, Trainers},
};

pub enum RunEvent {
    Starting,
    /// A start failed; `state` says why until the next one.
    Failed,
    Live,
    /// STOP: the trainer is being taken down.
    Stopping,
    /// No run any more: it ended by itself, its stop returned, its start was abandoned or its failure cleared.
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
    Failed(&'a Failure),
}

enum Play {
    Idle,
    /// Dropping the task abandons the run.
    Busy {
        game: GameId,
        stage: Stage,
        // The run's variables come from the shown cheat list.
        hold: Hold,
        _run: Task<()>,
    },
    Live {
        game: GameId,
        /// The game's process: its pid for hotkeys that fire only while it is in front, and what a source's overlay enters.
        process: Process,
        since: Instant,
        /// Its time and mods are kept.
        closed: bool,
        run: Run,
        _hold: Hold,
        _events: Task<()>,
        _clock: Task<()>,
    },
    /// The trainer is being taken down: a new run waits until it is gone.
    Stopping {
        game: GameId,
        _stop: Task<()>,
    },
    /// Stays until the next run, so the reason is still there when the user looks.
    Failed {
        game: GameId,
        error: Failure,
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

    pub fn live(&self) -> Option<&GameId> {
        self.live_run().map(|(game, _)| game)
    }

    pub fn is_live(&self, game: &GameId) -> bool {
        self.live() == Some(game)
    }

    pub fn live_run(&self) -> Option<(&GameId, &Run)> {
        match &self.play {
            Play::Live { game, run, .. } => Some((game, run)),
            _ => None,
        }
    }

    pub fn pid(&self) -> Option<u32> {
        self.process().map(|process| process.pid)
    }

    pub fn process(&self) -> Option<&Process> {
        match &self.play {
            Play::Live { process, .. } => Some(process),
            _ => None,
        }
    }

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
    pub fn holds(&self, game: &GameId) -> bool {
        self.is_active() && self.game() == Some(game)
    }

    /// Nothing runs before the cheat list is there or for a game not installed, and one run at a time.
    pub fn can_start(&self, game: &Game, cx: &App) -> bool {
        !self.is_active() && game.installed() && self.trainers.read(cx).sheet(&game.id).is_some()
    }

    pub fn state(&self, game: &GameId) -> State<'_> {
        if self.game() != Some(game) {
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

    pub fn start(&mut self, game: GameId, cx: &mut Context<Self>) {
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
        cx.emit(RunEvent::Starting);
        cx.notify();
    }

    /// A live run stays active until the session stops. Cancelling startup drops the session.
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
            Play::Failed { .. } => cx.emit(RunEvent::Ended),
            Play::Idle => {}
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
            cx.emit(RunEvent::Pressed(index));
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

    /// RESET: every live mod at rest; with no run, the kept ones are forgotten.
    pub fn reset(&mut self, game: &GameId, cx: &mut Context<Self>) {
        if !self.is_live(game) {
            return self.trainers.update(cx, |trainers, cx| trainers.forget_kept(game, cx));
        }
        if let Some(rest) = self.trainers.read(cx).sheet(game).map(|sheet| sheet.rest(&sheet.live_values())) {
            self.apply(&rest, cx);
        }
    }

    /// The live game's collection `at`: its mods on, or at rest when all of them are.
    pub fn toggle_collection(&mut self, game: &GameId, at: usize, cx: &mut Context<Self>) {
        if !self.is_live(game) {
            return;
        }
        let trainers = self.trainers.read(cx);
        let (Some(collection), Some(sheet)) = (trainers.trainer_profile(game).collections.get(at), trainers.sheet(game)) else { return };
        let values = if sheet.holds(&collection.mods) { sheet.rest(&collection.mods) } else { collection.mods.clone() };
        self.apply(&values, cx);
    }

    /// The run's game, whatever its state.
    pub fn game(&self) -> Option<&GameId> {
        match &self.play {
            Play::Idle => None,
            Play::Busy { game, .. } | Play::Live { game, .. } | Play::Stopping { game, .. } | Play::Failed { game, .. } => Some(game),
        }
    }

    fn request(&self, game: &GameId, cx: &App) -> Result<Request, Failure> {
        let listed = self.library.read(cx).game(game).ok_or(Failure::new(FailureKind::GameNotFound))?;
        let no_cheats = || Failure::new(FailureKind::NotFound).saying(app_text(key::RUN_NO_CHEATS));
        let sheet = self.trainers.read(cx).sheet(game).ok_or_else(no_cheats)?;
        let install = listed.install.as_ref().ok_or(Failure::new(FailureKind::GameNotFound))?;
        Ok(run::request(install, &sheet.plan))
    }

    fn staged(&mut self, stage: Stage, cx: &mut Context<Self>) {
        if let Play::Busy { stage: current, .. } = &mut self.play {
            *current = stage;
            cx.notify();
        }
    }

    fn attached(&mut self, game: GameId, outcome: Result<(Process, Live), Failure>, cx: &mut Context<Self>) {
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
            process,
            since: Instant::now(),
            closed: false,
            run: Run { session, tracker, teleport },
            _hold: hold,
            _events: watcher,
            _clock: clock,
        };
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

    fn fail(&mut self, game: GameId, error: Failure, cx: &mut Context<Self>) {
        log::warn!("run failed: {error}");
        self.play = Play::Failed { game, error };
        cx.emit(RunEvent::Failed);
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
