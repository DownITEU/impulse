//! Trainer hotkeys: the live run's hook, binding a slot or someone else's hotkey by pressing its keys or buttons, and
//! the saved choices. Every hook is global: a capture's runs first and hides what it takes from every app.

mod caps;
mod settings;

use std::sync::Arc;

use client::{
    bindings::{Choice, Store},
    storage::Paths,
};
use futures::{
    StreamExt,
    channel::{mpsc, oneshot},
};
use gpui_kit::{App, Context, Entity, Task, Window};
use hotkeys::{Binding, Capture, Chord, Fired, Hook, Matcher, Outcome, Step};
use model::Cheat;

use crate::{
    library::{Library, LibraryEvent},
    run::{RunEvent, Runs},
    trainer::{TrainerEvent, Trainers},
};

pub const PRESS_KEYS: &str = "PRESS KEYS";

/// What a capture binds.
#[derive(Clone, PartialEq)]
pub enum Target {
    // An id, not an index: a newer cheat list may arrive meanwhile.
    Slot {
        game: Arc<str>,
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
    _hook: Hook,
    _steps: Task<()>,
}

/// Tells whether a chord is someone else's hotkey.
type Reserved = Box<dyn Fn(Chord, &App) -> bool>;

pub struct Hotkeys {
    store: Store,
    trainers: Entity<Trainers>,
    runs: Entity<Runs>,
    reserved: Reserved,
    // Dropping it stops the hotkeys.
    armed: Option<(Hook, Task<()>)>,
    capture: Option<Capturing>,
}

impl Hotkeys {
    pub fn new(
        paths: &Paths,
        library: &Entity<Library>,
        trainers: Entity<Trainers>,
        runs: Entity<Runs>,
        reserved: impl Fn(Chord, &App) -> bool + 'static,
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
            // The user's choices over the provider's.
            let slots = trainers.read(cx).sheet(game).map(|sheet| hotkeys.slots(game, &sheet.cheats));
            if let Some(slots) = slots {
                trainers.update(cx, |trainers, cx| trainers.bind(game, slots, cx));
            }
            // Its cap may be gone, and with it the click that ends the capture.
            if matches!(hotkeys.capturing(), Some(Target::Slot { game: bound, .. }) if bound == game) {
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
                hotkeys.cancel(cx);
            }
        })
        .detach();
        Self { store: Store::load(paths), trainers, runs, reserved: Box::new(reserved), armed: None, capture: None }
    }

    /// Each cheat's slots, with the user's choices laid over the provider's.
    pub fn slots(&self, game: &str, cheats: &[Cheat]) -> Vec<Vec<Option<Chord>>> {
        cheats.iter().map(|cheat| self.store.slots(game, cheat)).collect()
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
    pub fn held_slot(&self, game: &str, cheat: &str, slot: usize) -> Option<Option<Chord>> {
        let capture = self.capture.as_ref()?;
        let bound =
            matches!(&capture.target, Target::Slot { game: at, cheat: id, slot: on } if **at == *game && id == cheat && *on == slot);
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

    /// Starts binding `target`; its outcome goes to the asker. `None` when the window is not active, or when it
    /// was being bound: a second click ends that.
    pub fn capture(&mut self, target: Target, window: &Window, cx: &mut Context<Self>) -> Option<oneshot::Receiver<Outcome>> {
        let again = self.capturing() == Some(&target);
        self.cancel(cx);
        // Only the active window's user may be typing for it.
        if again || !window.is_window_active() {
            return None;
        }
        let (tx, mut steps) = mpsc::unbounded();
        let mut capture = Capture::default();
        let hook = Hook::start(true, move |vk, down| {
            let step = capture.key(vk, down);
            if let Some(step) = step {
                let _ = tx.unbounded_send(step);
            }
            // A click it passes on still reaches the window and ends the capture there.
            down && step.is_some()
        });
        let hook = match hook {
            Ok(hook) => hook,
            Err(e) => {
                log::warn!("cannot read the keys to bind: {e}");
                return None;
            }
        };
        let task = cx.spawn(async move |hotkeys, cx| {
            while let Some(step) = steps.next().await {
                if hotkeys.update(cx, |hotkeys, cx| hotkeys.step(step, cx)).is_err() {
                    break;
                }
            }
        });
        let (asker, outcome) = oneshot::channel();
        self.capture = Some(Capturing { target, held: None, asker, _hook: hook, _steps: task });
        cx.notify();
        Some(outcome)
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.capture.take().is_some() {
            cx.notify();
        }
    }

    /// Binds a cheat's slot as a capture ended; refused when another slot of it or someone else's hotkey holds the chord.
    pub fn rebind(&mut self, game: &Arc<str>, cheat: &str, slot: usize, outcome: Outcome, cx: &mut Context<Self>) {
        let choice = match outcome {
            Outcome::Cancelled => return,
            Outcome::Reset => Choice::Default,
            Outcome::Unbound => Choice::Unbound,
            Outcome::Bound(chord) => Choice::Bound(chord),
        };
        let app: &App = cx;
        let Some(sheet) = self.trainers.read(app).sheet(game) else { return };
        let Some(cheat) = sheet.cheats.iter().find(|found| found.id == cheat) else { return };
        if self.store.rebind(game, cheat, slot, choice, |chord| (self.reserved)(chord, app)).is_none() {
            return;
        }
        let slots = self.slots(game, &sheet.cheats);
        self.trainers.update(cx, |trainers, cx| trainers.bind(game, slots, cx));
        if self.runs.read(cx).live() == Some(game) {
            self.arm(cx);
        }
    }

    fn step(&mut self, step: Step, cx: &mut Context<Self>) {
        match step {
            Step::Preview(held) => {
                if let Some(capture) = &mut self.capture {
                    capture.held = held;
                }
            }
            Step::Done(outcome) => {
                if let Some(capture) = self.capture.take() {
                    let _ = capture.asker.send(outcome);
                }
            }
        }
        cx.notify();
    }

    /// Hooks the live run's hotkeys, or stops them when no run is live or they are off.
    fn arm(&mut self, cx: &mut Context<Self>) {
        self.armed = None;
        cx.notify();
        let runs = self.runs.read(cx);
        let (Some(game), Some(pid)) = (runs.live(), runs.pid()) else { return };
        let Some(sheet) = self.trainers.read(cx).sheet(game).filter(|_| self.store.enabled()) else { return };
        let mut matcher = Matcher::default();
        for (index, slots) in sheet.hotkeys.iter().enumerate() {
            for (slot, chord) in slots.iter().enumerate() {
                if let Some(chord) = chord {
                    matcher.bind(*chord, Binding { action: (index, slot), block: false });
                }
            }
        }
        let mouse = sheet.hotkeys.iter().flatten().flatten().any(|chord| chord.uses_mouse());
        // A newer hook sees keys first, so it must not get ahead of a capture.
        self.capture = None;
        let (tx, mut presses) = mpsc::unbounded();
        let game_only = self.store.game_only();
        let hook = Hook::start(mouse, move |vk, down| {
            matcher.key(vk, down, |fired| {
                if let Fired::Press(action) = fired
                    && (!game_only || hotkeys::foreground_pid() == pid)
                {
                    let _ = tx.unbounded_send(action);
                }
            })
        });
        let hook = match hook {
            Ok(hook) => hook,
            Err(e) => return log::warn!("hotkeys are off: {e}"),
        };
        let runs = self.runs.downgrade();
        let task = cx.spawn(async move |_, cx| {
            while let Some((index, slot)) = presses.next().await {
                if runs.update(cx, |runs, cx| runs.step(index, slot, cx)).is_err() {
                    break;
                }
            }
        });
        self.armed = Some((hook, task));
    }
}
