//! Trainer hotkeys: the live run's keyboard hook, rebinding a slot by pressing its keys, and the
//! saved choices. Every hook is global: a capture's runs first and hides the keys from every app.

mod caps;
mod store;

use std::{path::Path, sync::Arc};

use futures::{StreamExt, channel::mpsc};
use gpui_kit::{Context, Div, Task, Window, prelude::*, rgb};
use hotkeys::{Binding, Capture, Chord, Fired, Hook, Matcher, Outcome, Step};
use model::Cheat;

use super::{Load, Page, run::Play, theme::*, widgets::mono};
use store::{Choice, Store};

pub(super) struct Bindings {
    store: Store,
    // Dropping it stops the hotkeys.
    armed: Option<(Hook, Task<()>)>,
    capture: Option<Capturing>,
}

struct Capturing {
    game: Arc<str>,
    // An id, not an index: a newer cheat list may arrive meanwhile.
    cheat: String,
    slot: usize,
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

    pub fn game_only(&self) -> bool {
        self.store.game_only()
    }

    pub fn disarm(&mut self) {
        self.armed = None;
    }

    /// What the slot being bound holds so far.
    fn held(&self, game: &str, cheat: &str, slot: usize) -> Option<Option<Chord>> {
        let capture = self.capture.as_ref()?;
        (*capture.game == *game && capture.cheat == cheat && capture.slot == slot).then_some(capture.held)
    }
}

impl Page {
    /// Hooks the live run's hotkeys, or stops them when no run is live.
    pub(super) fn arm_hotkeys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.bindings.armed = None;
        let Play::Live { game, pid, .. } = &self.play else { return };
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
        let (tx, mut presses) = mpsc::unbounded();
        let game_only = self.bindings.game_only();
        let hook = Hook::start(move |vk, down| {
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

    pub(super) fn toggle_game_only(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let on = !self.bindings.game_only();
        self.bindings.store.set_game_only(on);
        self.arm_hotkeys(window, cx);
        cx.notify();
    }

    /// Starts binding the open trainer's slot; a second click on it ends that.
    fn capture(&mut self, index: usize, slot: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(game) = self.selected.clone() else { return };
        let Some(cheat) = self.trainer(Some(&game)).map(|loaded| loaded.cheats[index].id.clone()) else { return };
        let again = self.bindings.held(&game, &cheat, slot).is_some();
        self.cancel_capture(cx);
        // Only the active window's user may be typing for it.
        if again || !window.is_window_active() {
            return;
        }
        let (tx, mut steps) = mpsc::unbounded();
        let mut capture = Capture::default();
        let hook = Hook::start(move |vk, down| {
            if let Some(step) = capture.key(vk, down) {
                let _ = tx.unbounded_send(step);
            }
            down
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
        self.bindings.capture = Some(Capturing { game, cheat, slot, held: None, _hook: hook, _steps: task });
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
                if let Some(capture) = self.bindings.capture.take() {
                    let Capturing { game, cheat, slot, .. } = capture;
                    self.rebind(game, &cheat, slot, outcome, window, cx);
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
        let Some(Load::Ready(loaded)) = self.trainers.get_mut(&game) else { return };
        let Some(index) = loaded.cheats.iter().position(|found| found.id == cheat) else { return };
        // As in Wand, a cheat never holds one chord twice.
        let taken = |chord: Chord| loaded.hotkeys[index].iter().enumerate().any(|(at, bound)| at != slot && *bound == Some(chord));
        if let Choice::Bound(chord) = choice
            && taken(chord)
        {
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
        if self.bindings.capture.is_some() {
            return mono("PRESS KEYS · ESC CANCEL · BKSP DEFAULT · DEL UNBIND", 11.).text_color(rgb(self.accent));
        }
        match self.bindings.armed.is_some() {
            true => mono("HOTKEYS ON", 11.),
            false => mono("HOTKEYS OFF", 11.).text_color(rgb(GHOST)),
        }
    }
}
