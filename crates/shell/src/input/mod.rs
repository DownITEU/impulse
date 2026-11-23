//! The app's one keyboard and mouse hook: every hotkey (the trainer's chords, the overlay's keys) judged by where the
//! user is as it is pressed, the capture of a new chord, and the keys typed into the HUD inside the game.

use std::{
    cell::Cell,
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicU32, Ordering::Relaxed},
        mpsc as sync,
    },
};

use futures::channel::mpsc;
use gpui_kit::{Context, Entity};
use keyhook::{Binding, Capture, Hook, Matcher, Modifiers, Outcome, Step, Stroke, Trigger};
use model::{Chord, mouse};
use topmost::Held;

use crate::run::{RunEvent, Runs};

/// Who binds here; each one's bindings are replaced as a whole.
#[derive(Clone, Copy, PartialEq)]
pub enum Owner {
    Trainer,
    Overlay,
}

/// Where a binding acts, judged as it is pressed.
#[derive(Clone, Copy, PartialEq)]
pub enum Scope {
    /// The live game in front, with no HUD up.
    Game,
    Anywhere,
    /// The live game in front or the HUD up; the key is hidden from every app when it acts.
    Hud,
}

/// The overlay as the keys see it.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum Hud {
    #[default]
    Off,
    Over,
    /// Inside the game's frame and ready: the game keeps the focus, so the HUD's keys come from here.
    Inside,
}

/// What a binding does; it runs on the hook thread, so it only forwards to a channel.
pub type Action = Arc<dyn Fn() + Send + Sync>;

/// A key typed into the HUD inside the game, with the modifiers held as it went down.
#[derive(Clone, Copy)]
pub struct HudKey {
    pub vk: u8,
    pub scan: u16,
    pub held: Held,
}

pub struct Input {
    shared: Arc<Shared>,
    bound: [Vec<(Trigger, Scope, Action)>; 2],
    capture: Option<mpsc::UnboundedSender<Step>>,
    // The hook thread sends to `typing`; the HUD takes `typed` once.
    typing: mpsc::UnboundedSender<HudKey>,
    typed: Option<mpsc::UnboundedReceiver<HudKey>>,
    hook: Option<Running>,
}

struct Running {
    hook: Hook,
    mouse: bool,
    configs: sync::Sender<Config>,
}

/// Read by the hook thread as keys come: a press is judged when it happens, a change restarts nothing.
#[derive(Default)]
struct Shared {
    // The live game's process; 0 for none.
    live: AtomicU32,
    hud: AtomicU8,
}

impl Shared {
    fn hud(&self) -> Hud {
        match self.hud.load(Relaxed) {
            1 => Hud::Over,
            2 => Hud::Inside,
            _ => Hud::Off,
        }
    }
}

/// What the hook thread acts on; it takes a new one at the next key and keeps the keys held.
struct Config {
    bound: Vec<(Trigger, Binding<(Scope, Action)>)>,
    capture: Option<mpsc::UnboundedSender<Step>>,
}

impl Input {
    pub fn new(runs: &Entity<Runs>, cx: &mut Context<Self>) -> Self {
        cx.subscribe(runs, |input, runs, event: &RunEvent, cx| {
            if !matches!(event, RunEvent::Pressed(_)) {
                input.shared.live.store(runs.read(cx).pid().unwrap_or(0), Relaxed);
                input.sync();
            }
        })
        .detach();
        let (typing, typed) = mpsc::unbounded();
        Self { shared: Arc::default(), bound: [Vec::new(), Vec::new()], capture: None, typing, typed: Some(typed), hook: None }
    }

    pub fn bind(&mut self, owner: Owner, bound: Vec<(Trigger, Scope, Action)>) {
        self.bound[owner as usize] = bound;
        self.sync();
    }

    /// Whether a binding of someone other than `owner` fires on `chord`.
    pub fn taken(&self, owner: Owner, chord: Chord) -> bool {
        let others = self.bound.iter().enumerate().filter(|&(at, _)| at != owner as usize).flat_map(|(_, bound)| bound);
        others.into_iter().any(|(trigger, ..)| match trigger {
            Trigger::Chord(bound) => *bound == chord,
            Trigger::Key(key) => key.clashes(chord),
        })
    }

    pub fn hud(&self) -> Hud {
        self.shared.hud()
    }

    pub fn set_hud(&mut self, hud: Hud) {
        let was = self.shared.hud();
        if hud != was {
            // Inside, the window never goes inactive, so a capture made there ends with the HUD.
            if was == Hud::Inside
                && let Some(steps) = self.capture.take()
            {
                let _ = steps.unbounded_send(Step::Done(Outcome::Cancelled));
            }
            self.shared.hud.store(hud as u8, Relaxed);
            self.sync();
        }
    }

    /// The keys typed into the HUD inside the game, given once: to the HUD.
    pub fn typed_keys(&mut self) -> Option<mpsc::UnboundedReceiver<HudKey>> {
        self.typed.take()
    }

    /// Every key and button goes to the capture until `end_capture`, and nothing fires; `None` without a hook.
    pub fn capture(&mut self) -> Option<mpsc::UnboundedReceiver<Step>> {
        let (steps, received) = mpsc::unbounded();
        self.capture = Some(steps);
        self.sync();
        if self.hook.is_none() {
            self.capture = None;
            return None;
        }
        Some(received)
    }

    pub fn end_capture(&mut self) {
        if self.capture.take().is_some() {
            self.sync();
        }
    }

    /// The hook runs while a run is live, the HUD is up (its key closes it) or a capture is on; the mouse's only
    /// while a chord or the capture needs it.
    fn sync(&mut self) {
        if self.shared.live.load(Relaxed) == 0 && self.shared.hud() == Hud::Off && self.capture.is_none() {
            self.hook = None;
            return;
        }
        let mouse = self.capture.is_some()
            || self.bound.iter().flatten().any(|(trigger, ..)| matches!(trigger, Trigger::Chord(chord) if chord.uses_mouse()));
        let bound = self.bound.iter().flatten();
        let config = Config {
            bound: bound
                .map(|(trigger, scope, action)| (*trigger, Binding { action: (*scope, action.clone()), block: *scope == Scope::Hud }))
                .collect(),
            capture: self.capture.clone(),
        };
        if let Some(running) = &mut self.hook {
            let _ = running.configs.send(config);
            if running.mouse != mouse {
                match running.hook.set_mouse(mouse) {
                    Ok(()) => running.mouse = mouse,
                    Err(e) => log::warn!("mouse hotkeys: {e}"),
                }
            }
            return;
        }
        let (configs, received) = sync::channel();
        let _ = configs.send(config);
        let mut judge = Judge {
            matcher: Matcher::default(),
            capture: None,
            typing: self.typing.clone(),
            shared: self.shared.clone(),
            configs: received,
        };
        match Hook::start(mouse, move |stroke| judge.stroke(stroke)) {
            Ok(hook) => self.hook = Some(Running { hook, mouse, configs }),
            Err(e) => log::warn!("hotkeys are off: {e}"),
        }
    }
}

/// The hook thread's side.
struct Judge {
    // Also the ledger of the keys held, fed every key.
    matcher: Matcher<(Scope, Action)>,
    capture: Option<(Capture, mpsc::UnboundedSender<Step>)>,
    typing: mpsc::UnboundedSender<HudKey>,
    shared: Arc<Shared>,
    configs: sync::Receiver<Config>,
}

impl Judge {
    /// Whether to hide the key from every app.
    fn stroke(&mut self, Stroke { vk, down, repeat, held, scan }: Stroke) -> bool {
        while let Ok(config) = self.configs.try_recv() {
            self.apply(config);
        }
        // A modifier held as the run goes live still counts for the keys pressed with it.
        if held {
            self.matcher.key(vk, true, |_| false, |_| {});
            return false;
        }
        let (hud, live) = (self.shared.hud(), self.shared.live.load(Relaxed));
        // Asked once per key, and only when something needs it.
        let front = Cell::new(None);
        let in_front = || {
            let known = front.get().unwrap_or_else(|| live != 0 && keyhook::foreground_pid() == live);
            front.set(Some(known));
            known
        };
        // Inside, the user binds in the game: keys typed in another app are not the capture's.
        if let Some((capture, steps)) = &mut self.capture
            && (hud != Hud::Inside || in_front())
        {
            if repeat {
                return false;
            }
            self.matcher.key(vk, down, |_| false, |_| {});
            let step = capture.key(vk, down);
            if let Some(step) = step {
                let _ = steps.unbounded_send(step);
            }
            // A click it passes on still reaches the window and ends the capture there.
            return down && step.is_some();
        }
        let active = |(scope, _): &(Scope, Action)| match scope {
            Scope::Anywhere => true,
            Scope::Game => hud == Hud::Off && in_front(),
            Scope::Hud => hud != Hud::Off || in_front(),
        };
        let hidden = !repeat && self.matcher.key(vk, down, active, |(_, action)| action());
        // What would reach a focused HUD over the game goes to the one inside it; never what is typed elsewhere.
        if down && !hidden && hud == Hud::Inside && !mouse::is_mouse(vk) && in_front() {
            let Modifiers { control, alt, shift, .. } = self.matcher.modifiers();
            let held = Held { control, alt, shift, caps: keyhook::caps_lock() };
            let _ = self.typing.unbounded_send(HudKey { vk, scan, held });
        }
        hidden
    }

    fn apply(&mut self, Config { bound, capture }: Config) {
        self.matcher.replace(bound);
        // The same capture keeps what it holds so far.
        let same = matches!((&self.capture, &capture), (Some((_, old)), Some(new)) if old.same_receiver(new));
        if !same {
            self.capture = capture.map(|steps| (Capture::default(), steps));
        }
    }
}
