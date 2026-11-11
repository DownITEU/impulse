//! Trainer hotkeys: the live run's hook, rebinding a slot or the overlay hotkey by pressing its keys or
//! buttons, the saved choices and the Settings tab. Every hook is global: a capture's runs first and
//! hides what it takes from every app.

mod caps;
mod settings;
mod store;

use std::{path::Path, sync::Arc};

use futures::{StreamExt, channel::mpsc};
use gpui_kit::{Context, Div, Task, Window, prelude::*, rgb};
use hotkeys::{Binding, Capture, Chord, Fired, Hook, Matcher, Outcome, Step};
use model::Cheat;

use super::{Load, Page, run::Play, theme::*, widgets::mono};
pub(super) use store::file;
use store::{Choice, Store};

pub(super) const PRESS_KEYS: &str = "PRESS KEYS";

pub(super) struct Bindings {
    store: Store,
    // Dropping it stops the hotkeys.
    armed: Option<(Hook, Task<()>)>,
    capture: Option<Capturing>,
}

/// What a capture binds.
#[derive(PartialEq)]
pub(super) enum Target {
    // An id, not an index: a newer cheat list may arrive meanwhile.
    Slot { game: Arc<str>, cheat: String, slot: usize },
    Overlay,
}

struct Capturing {
    target: Target,
    held: Option<Chord>,
    _hook: Hook,
    _steps: Task<()>,
}

impl Bindings {
    pub fn load(data_dir: &Path) -> Self {
        Self { store: Store::load(data_dir), armed: None, capture: None }
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

    pub fn disarm(&mut self) {
        self.armed = None;
    }

    /// The caller repaints.
    pub fn cancel(&mut self) {
        self.capture = None;
    }

    /// What the slot being bound holds so far.
    fn held(&self, game: &str, cheat: &str, slot: usize) -> Option<Option<Chord>> {
        let capture = self.capture.as_ref()?;
        let bound = matches!(&capture.target, Target::Slot { game: at, cheat: id, slot: on } if **at == *game && id == cheat && *on == slot);
        bound.then_some(capture.held)
    }

    pub fn overlay_held(&self) -> Option<Option<Chord>> {
        self.capture.as_ref().filter(|capture| capture.target == Target::Overlay).map(|capture| capture.held)
    }
}

impl Page {
    /// Hooks the live run's hotkeys, or stops them when no run is live or they are off.
    pub(super) fn arm_hotkeys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.bindings.armed = None;
        let Play::Live { game, pid, .. } = &self.play else { return };
        if !self.bindings.enabled() {
            return cx.notify();
        }
        let (game, pid) = (game.clone(), *pid);
        // A newer hook sees keys first, so it must not get ahead of a capture.
        self.cancel_capture(cx);
        let Some(loaded) = self.trainer(Some(&game)) else { return };
        let mut matcher = Matcher::default();
        for (index, slots) in loaded.hotkeys.iter().enumerate() {
            for (slot, chord) in slots.iter().enumerate() {
                if let Some(chord) = chord {
                    matcher.bind(*chord, Binding { action: (index, slot), block: false });
                }
            }
        }
        let mouse = loaded.hotkeys.iter().flatten().flatten().any(|chord| chord.uses_mouse());
        let (tx, mut presses) = mpsc::unbounded();
        let game_only = self.bindings.game_only();
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
        let task = cx.spawn_in(window, async move |page, cx| {
            while let Some((index, slot)) = presses.next().await {
                if page.update_in(cx, |page, window, cx| page.hotkey(index, slot, window, cx)).is_err() {
                    break;
                }
            }
        });
        self.bindings.armed = Some((hook, task));
        cx.notify();
    }

    pub(super) fn set_hotkeys_enabled(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.bindings.store.set_enabled(on);
        self.arm_hotkeys(window, cx);
        cx.notify();
    }

    pub(super) fn set_game_only(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.bindings.store.set_game_only(on);
        self.arm_hotkeys(window, cx);
        cx.notify();
    }

    /// Starts binding the open trainer's slot.
    fn capture(&mut self, index: usize, slot: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = self.selected.clone() else { return };
        let Some(cheat) = self.trainer(Some(&game)).map(|loaded| loaded.cheats[index].id.clone()) else { return };
        self.start_capture(Target::Slot { game, cheat, slot }, window, cx);
    }

    /// A second click on what is being bound ends that.
    pub(super) fn start_capture(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        let again = self.bindings.capture.as_ref().is_some_and(|capture| capture.target == target);
        self.cancel_capture(cx);
        // Only the active window's user may be typing for it.
        if again || !window.is_window_active() {
            return;
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
            Err(e) => return log::warn!("cannot read the keys to bind: {e}"),
        };
        let task = cx.spawn_in(window, async move |page, cx| {
            while let Some(step) = steps.next().await {
                if page.update_in(cx, |page, window, cx| page.capture_step(step, window, cx)).is_err() {
                    break;
                }
            }
        });
        self.bindings.capture = Some(Capturing { target, held: None, _hook: hook, _steps: task });
        cx.notify();
    }

    pub(super) fn cancel_capture(&mut self, cx: &mut Context<Self>) {
        if self.bindings.capture.take().is_some() {
            cx.notify();
        }
    }

    fn capture_step(&mut self, step: Step, window: &mut Window, cx: &mut Context<Self>) {
        match step {
            Step::Preview(held) => {
                if let Some(capture) = &mut self.bindings.capture {
                    capture.held = held;
                }
            }
            Step::Done(outcome) => {
                match self.bindings.capture.take().map(|capture| capture.target) {
                    Some(Target::Slot { game, cheat, slot }) => self.rebind(game, &cheat, slot, outcome, window, cx),
                    Some(Target::Overlay) => self.rebind_overlay(outcome, cx),
                    None => {}
                }
            }
        }
        cx.notify();
    }

    fn rebind(&mut self, game: Arc<str>, cheat: &str, slot: usize, outcome: Outcome, window: &mut Window, cx: &mut Context<Self>) {
        let choice = match outcome {
            Outcome::Cancelled => return,
            Outcome::Reset => Choice::Default,
            Outcome::Unbound => Choice::Unbound,
            Outcome::Bound(chord) => Choice::Bound(chord),
        };
        let overlay = self.layer.read(cx).hotkey();
        let Some(Load::Ready(loaded)) = self.trainers.get_mut(&game) else { return };
        let Some(index) = loaded.cheats.iter().position(|found| found.id == cheat) else { return };
        let wanted = match choice {
            Choice::Bound(chord) => Some(chord),
            Choice::Unbound => None,
            Choice::Default => loaded.cheats[index].hotkeys.get(slot).copied().flatten(),
        };
        // As in Wand, a cheat never holds one chord twice, nor the overlay's.
        let taken = |chord: Chord| {
            let other = |(at, bound): (usize, &Option<Chord>)| at != slot && bound.is_some_and(|bound| bound.same_keys(chord));
            overlay.fired_by(chord.vks()) || loaded.hotkeys[index].iter().enumerate().any(other)
        };
        if wanted.is_some_and(taken) {
            return;
        }
        self.bindings.store.choose(&game, cheat, slot, choice);
        let slots = self.bindings.store.slots(&game, &loaded.cheats[index]);
        loaded.rebind(index, slots);
        if self.play.live() == Some(&game) {
            self.arm_hotkeys(window, cx);
        }
    }

    /// The status bar's hotkey note; while binding, the keys that end it.
    pub(super) fn hotkeys_status(&self) -> Div {
        if let Some(capture) = &self.bindings.capture {
            let keys = match capture.target {
                Target::Slot { .. } => "PRESS KEYS · ESC CANCEL · BKSP DEFAULT · DEL UNBIND",
                Target::Overlay => "PRESS KEYS · ESC CANCEL · BKSP DEFAULT",
            };
            return mono(keys, 11.).text_color(rgb(self.accent));
        }
        match self.bindings.armed.is_some() {
            true => mono("HOTKEYS ON", 11.),
            false => mono("HOTKEYS OFF", 11.).text_color(rgb(GHOST)),
        }
    }
}
