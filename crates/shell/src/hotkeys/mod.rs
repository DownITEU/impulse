//! Trainer hotkeys: the live run's chords, binding a slot or someone else's hotkey by pressing its keys or buttons, and
//! the saved choices. Both go through the app's one hook (`Input`); a capture hides what it takes from every app.

mod caps;
mod clash;
mod popover;
mod scope;
mod settings;

use std::sync::Arc;

use client::{
    bindings::{Choice, Store},
    library::GameId,
    storage::Paths,
    trainer::Sheet,
};
use futures::{
    StreamExt,
    channel::{mpsc, oneshot},
};
use gpui_kit::{App, Context, Entity, FocusHandle, Task, Window};
use keyhook::{Outcome, Step, Trigger};
use model::Chord;

use crate::{
    input::{Action, Hud, Input, Owner, Scope},
    library::{Library, LibraryEvent},
    run::{RunEvent, Runs},
    trainer::{TrainerEvent, Trainers},
};

use clash::Conflict;

pub use caps::{cap, caps};
pub use popover::bind_keys;
pub use scope::hotkeys_scope;
pub use settings::HotkeySettings;

#[derive(Clone, PartialEq)]
pub enum Target {
    // An id, not an index: a newer cheat list may arrive meanwhile.
    Slot {
        game: GameId,
        cheat: String,
        slot: usize,
    },
    /// Someone else's hotkey; a `required` one cannot be unbound.
    Key {
        name: &'static str,
        required: bool,
    },
}

struct Capturing {
    target: Target,
    held: Option<Chord>,
    // Dropped unsent when the capture is cancelled.
    asker: oneshot::Sender<Outcome>,
    _steps: Task<()>,
}

pub struct Hotkeys {
    store: Store,
    trainers: Entity<Trainers>,
    runs: Entity<Runs>,
    input: Entity<Input>,
    // Dropping it drops the presses of chords bound before.
    armed: Option<Task<()>>,
    capture: Option<Capturing>,
    // A pressed chord that another hotkey holds, waiting for its answer.
    conflict: Option<Conflict>,
    focus: FocusHandle,
}

impl Hotkeys {
    pub fn new(
        paths: &Paths,
        library: &Entity<Library>,
        trainers: Entity<Trainers>,
        runs: Entity<Runs>,
        input: Entity<Input>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe(&runs, |hotkeys, _, event: &RunEvent, cx| {
            if !matches!(event, RunEvent::Pressed(_)) {
                hotkeys.arm(cx);
            }
        })
        .detach();
        cx.subscribe(&trainers, |hotkeys, trainers, event: &TrainerEvent, cx| {
            let TrainerEvent::Installed(game) = event else { return };
            let slots = trainers.read(cx).sheet(game).map(|sheet| hotkeys.slots(sheet));
            if let Some(slots) = slots {
                trainers.update(cx, |trainers, cx| trainers.bind(game, slots, cx));
            }
            // Its cap may be gone, and with it the click that ends the capture or the question.
            if hotkeys.binding_game() == Some(game) {
                hotkeys.cancel(cx);
            }
        })
        .detach();
        cx.subscribe(library, |hotkeys, _, event: &LibraryEvent, cx| {
            if matches!(event, LibraryEvent::Opened) {
                hotkeys.cancel(cx);
            }
        })
        .detach();
        // A capture hides keys from every app: it must not outlive the user's attention.
        cx.observe_window_activation(window, |hotkeys, window, cx| {
            if !window.is_window_active() {
                // A question holds the focus: it goes back while there is a window to give it in.
                hotkeys.dismiss(window, cx);
                hotkeys.cancel(cx);
            }
        })
        .detach();
        let focus = cx.focus_handle();
        Self { store: Store::load(paths), trainers, runs, input, armed: None, capture: None, conflict: None, focus }
    }

    /// Each cheat's slots, with the user's choices laid over the source's.
    pub fn slots(&self, sheet: &Sheet) -> Vec<Vec<Option<Chord>>> {
        sheet.cheats.iter().map(|cheat| self.store.slots(&sheet.trainer, cheat)).collect()
    }

    pub fn enabled(&self) -> bool {
        self.store.enabled()
    }

    pub fn game_only(&self) -> bool {
        self.store.game_only()
    }

    pub fn is_armed(&self) -> bool {
        self.armed.is_some()
    }

    pub fn capturing(&self) -> Option<&Target> {
        self.capture.as_ref().map(|capture| &capture.target)
    }

    /// What the slot being bound holds so far.
    pub fn held_slot(&self, game: &GameId, cheat: &str, slot: usize) -> Option<Option<Chord>> {
        let capture = self.capture.as_ref()?;
        let bound = matches!(&capture.target, Target::Slot { game: at, cheat: id, slot: on } if at == game && id == cheat && *on == slot);
        bound.then_some(capture.held)
    }

    /// What `target`, being bound, holds so far.
    pub fn held(&self, target: &Target) -> Option<Option<Chord>> {
        self.capture.as_ref().filter(|capture| capture.target == *target).map(|capture| capture.held)
    }

    pub fn set_enabled(&mut self, on: bool, cx: &mut Context<Self>) {
        self.store.set_enabled(on);
        self.arm(cx);
    }

    pub fn set_game_only(&mut self, on: bool, cx: &mut Context<Self>) {
        self.store.set_game_only(on);
        self.arm(cx);
    }

    /// Starts binding `target`; its outcome goes to the asker. `None` when the user is not typing for the window,
    /// or when it was being bound: a second click ends that.
    pub fn capture(&mut self, target: Target, window: &Window, cx: &mut Context<Self>) -> Option<oneshot::Receiver<Outcome>> {
        let again = self.capturing() == Some(&target);
        self.cancel(cx);
        if again || !self.attended(window, cx) {
            return None;
        }
        let mut steps = self.input.update(cx, |input, _| input.capture())?;
        let task = cx.spawn(async move |hotkeys, cx| {
            while let Some(step) = steps.next().await {
                if hotkeys.update(cx, |hotkeys, cx| hotkeys.step(step, cx)).is_err() {
                    break;
                }
            }
        });
        let (asker, outcome) = oneshot::channel();
        self.capture = Some(Capturing { target, held: None, asker, _steps: task });
        cx.notify();
        Some(outcome)
    }

    /// Whether the user's keys are for the window: it is active, or the HUD is inside the game, which keeps the focus.
    fn attended(&self, window: &Window, cx: &App) -> bool {
        window.is_window_active() || self.input.read(cx).hud() == Hud::Inside
    }

    /// Ends a capture and drops a question with no focus to give back: its window is not at hand.
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        let (capture, conflict) = (self.end_capture(cx), self.conflict.take());
        if capture.is_some() || conflict.is_some() {
            cx.notify();
        }
    }

    fn end_capture(&mut self, cx: &mut Context<Self>) -> Option<Capturing> {
        let capture = self.capture.take();
        if capture.is_some() {
            self.input.update(cx, |input, _| input.end_capture());
        }
        capture
    }

    /// Binds a cheat's slot as a capture ended; a chord another hotkey holds becomes a question over the slot's cap.
    pub fn rebind(&mut self, game: &GameId, cheat: &str, slot: usize, outcome: Outcome, window: &mut Window, cx: &mut Context<Self>) {
        let choice = match outcome {
            Outcome::Cancelled => return,
            Outcome::Reset => Choice::Default,
            Outcome::Unbound => Choice::Unbound,
            Outcome::Bound(chord) => Choice::Bound(chord),
        };
        self.bind(game, cheat, slot, choice, None, window, cx);
    }

    fn step(&mut self, step: Step, cx: &mut Context<Self>) {
        match step {
            Step::Preview(held) => {
                if let Some(capture) = &mut self.capture {
                    capture.held = held;
                }
            }
            Step::Done(outcome) => {
                if let Some(capture) = self.end_capture(cx) {
                    let _ = capture.asker.send(outcome);
                }
            }
        }
        cx.notify();
    }

    /// Binds the live run's hotkeys, or none when no run is live or they are off.
    fn arm(&mut self, cx: &mut Context<Self>) {
        self.armed = None;
        cx.notify();
        let (tx, mut presses) = mpsc::unbounded();
        let scope = if self.store.game_only() { Scope::Game } else { Scope::Anywhere };
        let live = self.runs.read(cx).live().filter(|_| self.store.enabled());
        let sheet = live.and_then(|game| self.trainers.read(cx).sheet(game));
        let mut bound = Vec::new();
        for (index, slots) in sheet.iter().flat_map(|sheet| sheet.hotkeys.iter().enumerate()) {
            for (slot, chord) in slots.iter().enumerate() {
                if let Some(chord) = *chord {
                    let tx = tx.clone();
                    let press: Action = Arc::new(move || {
                        let _ = tx.unbounded_send((index, slot));
                    });
                    bound.push((Trigger::Chord(chord), scope, press));
                }
            }
        }
        let armed = sheet.is_some();
        self.input.update(cx, |input, _| input.bind(Owner::Trainer, bound));
        if !armed {
            return;
        }
        let runs = self.runs.downgrade();
        self.armed = Some(cx.spawn(async move |_, cx| {
            while let Some((index, slot)) = presses.next().await {
                if runs.update(cx, |runs, cx| runs.step(index, slot, cx)).is_err() {
                    break;
                }
            }
        }));
    }
}
