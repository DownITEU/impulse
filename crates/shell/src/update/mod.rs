//! Updates: looking for a newer Impulse, fetching it beside the running executable and swapping it in when
//! Impulse quits. `Updates` is the state; `UpdatePanel` draws the ABOUT tab's rows and `chip` the status bar's.

mod chip;
mod notes;
mod panel;

use std::{cell::RefCell, env, path::PathBuf, rc::Rc, time::Duration};

use anyhow::Result;
use blocking::unblock;
use client::storage::{Paths, Stored};
use gpui_kit::{App, Context, Entity, EventEmitter, SharedString, Task};
use serde::{Deserialize, Serialize};
use updater::{Check, Release, Section, Version};

pub use chip::chip;
pub use panel::UpdatePanel;

use crate::{about::FEED, run::Runs, settings::Settings};

/// The first look is a moment after start, so it never competes with the window opening.
const START_DELAY: Duration = Duration::from_secs(10);
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// The exe carries its own notes: offline, and always those of this build.
const CHANGELOG: &str = include_str!("../../../../CHANGELOG.md");

/// `update.json`: the last feed answer, kept to show a known update offline and to ask with its ETag.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    /// The feed `etag` and `releases` answered.
    feed: String,
    etag: Option<String>,
    releases: Vec<Release>,
    /// The newest version whose offer was shown.
    offered: Option<Version>,
    /// The build whose notes were shown.
    shown: Option<Version>,
    /// SKIP THIS VERSION: it and every older one are not offered; CHECK NOW lifts it.
    skipped: Option<Version>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    /// The notes of every release newer than this build.
    Available,
    /// The notes of this build, after an update.
    New,
    Changelog,
}

/// Something asks for a dialog; the page closes what is in the way and `UpdateNotice` shows it.
pub struct Present(pub Mode);

#[derive(Clone, PartialEq)]
pub enum Status {
    Idle,
    Checking,
    Current,
    /// The newest release is one the user skipped.
    Skipped(Version),
    Available(Version),
    Downloading(Version),
    Ready(Version),
    CheckFailed,
    InstallFailed,
}

#[derive(Clone, Copy, PartialEq)]
enum Busy {
    Checking,
    Downloading,
}

#[derive(Clone, Copy, PartialEq)]
enum Failed {
    Check,
    Install,
}

/// What the swap at quit needs. The quit hook holds it, not the entity: a window that closes may drop the entity
/// before the app quits.
#[derive(Default)]
struct Plan {
    /// Where an update goes; `None` when Windows would not say.
    exe: Option<PathBuf>,
    /// The version fetched beside the executable and still on offer.
    ready: Option<Version>,
    /// RESTART NOW was pressed: the swap starts the new executable.
    restarting: bool,
}

pub struct Updates {
    settings: Entity<Settings>,
    runs: Entity<Runs>,
    saved: Stored<Saved>,
    current: Version,
    sections: Vec<Section>,
    plan: Rc<RefCell<Plan>>,
    /// The releases of the chosen channel newer than this build, newest first.
    offer: Vec<Release>,
    /// The feed and channel `offer` was made for.
    selected: (String, updater::Channel),
    busy: Option<Busy>,
    failed: Option<Failed>,
    checked: bool,
    /// CHECK NOW was pressed since start, whatever came of it.
    asked: bool,
    /// The auto-check is on (`!no_update_check`); kept to see it switched.
    watching: bool,
    /// The release in `<exe>.new`, waiting for the swap.
    staged: Option<Release>,
    task: Task<()>,
    _timer: Task<()>,
}

impl EventEmitter<Present> for Updates {}

impl Updates {
    pub fn new(settings: Entity<Settings>, runs: Entity<Runs>, paths: &Paths, cx: &mut Context<Self>) -> Self {
        cx.observe(&settings, |updates, _, cx| updates.settings_changed(cx)).detach();
        let exe = env::current_exe().inspect_err(|e| log::warn!("no path for the executable: {e}")).ok();
        let plan = Rc::new(RefCell::new(Plan { exe, ..Plan::default() }));
        // The swap waits for the quit: the running executable is only renamed, and nothing else is in the way.
        let hook = plan.clone();
        App::on_app_quit(cx, move |_| {
            apply(&hook.borrow());
            async {}
        })
        .detach();
        let current = updater::current();
        let mut saved: Stored<Saved> = Stored::load(paths.update());
        // A first launch has nothing new to show; an older build's user has not seen these notes.
        let fresh = !settings.read(cx).prefs().disclaimer_seen;
        saved.edit(|saved| match &saved.shown {
            None if fresh => saved.shown = Some(current.clone()),
            Some(shown) if *shown > current => saved.shown = Some(current.clone()),
            _ => {}
        });
        let watching = !settings.read(cx).prefs().no_update_check;
        let mut updates = Self {
            settings,
            runs,
            saved,
            sections: updater::sections(CHANGELOG),
            current,
            plan,
            offer: Vec::new(),
            selected: Default::default(),
            busy: None,
            failed: None,
            checked: false,
            asked: false,
            watching,
            staged: None,
            task: Task::ready(()),
            _timer: Task::ready(()),
        };
        updates.selected = updates.selection(cx);
        updates.recount();
        updates._timer = cx.spawn(async move |updates, cx| {
            cx.background_executor().timer(START_DELAY).await;
            loop {
                if updates.update(cx, |updates, cx| updates.auto(cx)).is_err() {
                    return;
                }
                cx.background_executor().timer(EVERY).await;
            }
        });
        updates
    }

    pub fn version(&self) -> &Version {
        &self.current
    }

    pub fn latest(&self) -> Option<&Release> {
        self.offer.first()
    }

    pub fn status(&self) -> Status {
        let newest = self.latest().map(|release| release.version.clone());
        let skipped = self.saved.skipped.clone().filter(|version| version.cmp_precedence(&self.current).is_gt());
        match (self.busy, newest, skipped) {
            (Some(Busy::Checking), ..) => Status::Checking,
            (Some(Busy::Downloading), Some(version), _) => Status::Downloading(version),
            (_, Some(version), _) if self.ready().is_some() => Status::Ready(version),
            (_, Some(_), _) if self.failed == Some(Failed::Install) => Status::InstallFailed,
            (_, Some(version), _) => Status::Available(version),
            (_, None, Some(version)) => Status::Skipped(version),
            (_, None, None) if self.failed == Some(Failed::Check) => Status::CheckFailed,
            (_, None, None) if self.checked => Status::Current,
            (_, None, None) => Status::Idle,
        }
    }

    /// Whether an offer shows unasked (a dialog, the status bar's chip): with the auto-check off Impulse stays quiet
    /// until the user asks with CHECK NOW.
    pub fn announces(&self) -> bool {
        self.asked || self.watching
    }

    /// The markdown a dialog shows; empty when there is nothing to say.
    pub fn notes(&self, mode: Mode) -> SharedString {
        match mode {
            Mode::Available => notes::available(&self.offer),
            Mode::New => notes::of(&self.sections, &self.current),
            Mode::Changelog => notes::all(&self.sections),
        }
        .into()
    }

    /// What to show unasked: the notes of the build the user just got, else an offer they have not seen.
    pub fn due(&self) -> Option<Mode> {
        // By version: the release after its pre-release has the same notes.
        let unseen = self.saved.shown.as_ref().map(updater::base) != Some(updater::base(&self.current));
        if unseen && !notes::of(&self.sections, &self.current).is_empty() {
            return Some(Mode::New);
        }
        let newest = self.latest().filter(|_| self.announces())?;
        (self.saved.offered.as_ref() != Some(&newest.version)).then_some(Mode::Available)
    }

    /// The dialog is up: it does not come again unasked.
    pub fn acknowledge(&mut self, mode: Mode) {
        let (current, newest) = (self.current.clone(), self.latest().map(|release| release.version.clone()));
        match mode {
            Mode::Available => self.saved.edit(|saved| saved.offered = newest),
            Mode::New => self.saved.edit(|saved| saved.shown = Some(current)),
            Mode::Changelog => {}
        }
    }

    pub fn present(&mut self, mode: Mode, cx: &mut Context<Self>) {
        cx.emit(Present(mode));
    }

    /// A running trainer must not be cut off by a restart: it waits for the quit.
    pub fn can_restart(&self, cx: &App) -> bool {
        self.ready().is_some() && !self.runs.read(cx).is_active()
    }

    pub fn restart(&mut self, cx: &mut Context<Self>) {
        if self.can_restart(cx) {
            self.plan.borrow_mut().restarting = true;
            cx.quit();
        }
    }

    /// SKIP THIS VERSION: nothing more is said of the newest release; a newer one is offered as ever.
    pub fn skip(&mut self, cx: &mut Context<Self>) {
        if let Some(version) = self.latest().map(|release| release.version.clone()) {
            self.set_skipped(Some(version), cx);
        }
    }

    fn set_skipped(&mut self, version: Option<Version>, cx: &mut Context<Self>) {
        self.saved.edit(|saved| saved.skipped = version);
        self.recount();
        cx.notify();
    }

    /// CHECK NOW: asks whatever the auto-check says, offers a skipped version again, and fetches a release on offer.
    pub fn check(&mut self, cx: &mut Context<Self>) {
        if self.busy.is_some() {
            return;
        }
        self.asked = true;
        if self.saved.skipped.is_some() {
            self.set_skipped(None, cx);
        }
        self.ask(cx);
    }

    fn ask(&mut self, cx: &mut Context<Self>) {
        if self.busy.is_some() {
            return;
        }
        let feed = self.selected.0.clone();
        let etag = self.saved.etag.clone().filter(|_| self.saved.feed == feed);
        self.busy = Some(Busy::Checking);
        self.failed = None;
        self.task = cx.spawn(async move |updates, cx| {
            let asked = feed.clone();
            let answer = unblock(move || updater::check(&asked, etag.as_deref())).await;
            updates.update(cx, |updates, cx| updates.checked_in(feed, answer, cx)).ok();
        });
        cx.notify();
    }

    fn auto(&mut self, cx: &mut Context<Self>) {
        if self.watching {
            self.ask(cx);
        }
    }

    fn checked_in(&mut self, feed: String, answer: Result<Check>, cx: &mut Context<Self>) {
        self.busy = None;
        let reached = answer.is_ok();
        match answer {
            Ok(Check::Same) => self.checked = true,
            Ok(Check::New(fetched)) => {
                self.saved.edit(|saved| {
                    saved.feed = feed;
                    saved.etag = fetched.etag;
                    saved.releases = fetched.releases;
                });
                self.checked = true;
            }
            Err(e) => {
                log::warn!("update check: {e:#}");
                self.failed = Some(Failed::Check);
            }
        }
        self.recount();
        // A known offer is not fetched on a feed that did not answer: it may be gone.
        if reached {
            self.fetch(cx);
        }
        cx.notify();
    }

    /// Fetches the newest offer beside the executable unless it is there, on its way or refused (a check asks again).
    fn fetch(&mut self, cx: &mut Context<Self>) {
        let Some(release) = self.latest().cloned() else { return };
        if self.busy.is_some() || self.ready().is_some() || self.failed == Some(Failed::Install) {
            return;
        }
        let Some(exe) = self.plan.borrow().exe.clone() else {
            self.failed = Some(Failed::Install);
            return;
        };
        // The download is written over the file of the one before.
        self.staged = None;
        self.publish();
        self.busy = Some(Busy::Downloading);
        self.task = cx.spawn(async move |updates, cx| {
            let wanted = release.clone();
            let done = unblock(move || updater::download(&wanted, &exe)).await;
            updates.update(cx, |updates, cx| updates.fetched(release, done, cx)).ok();
        });
    }

    fn fetched(&mut self, release: Release, done: Result<()>, cx: &mut Context<Self>) {
        self.busy = None;
        match done {
            Ok(()) => self.staged = Some(release),
            Err(e) => {
                log::warn!("update download: {e:#}");
                // A download the offer has left behind (a skip, a channel) says nothing of the one now on offer.
                if self.is_newest(&release) {
                    self.failed = Some(Failed::Install);
                }
            }
        }
        self.publish();
        // The channel may have changed meanwhile.
        self.fetch(cx);
        cx.notify();
    }

    /// The feed or the channel changed: a known answer of another feed means nothing, and what was fetched may no longer be offered.
    fn settings_changed(&mut self, cx: &mut Context<Self>) {
        let watching = !self.settings.read(cx).prefs().no_update_check;
        if watching != self.watching {
            self.watching = watching;
            self.auto(cx);
            cx.notify();
        }
        let selected = self.selection(cx);
        if selected == self.selected {
            return;
        }
        let feed_changed = selected.0 != self.selected.0;
        self.selected = selected;
        // A failure was of the offer before.
        self.failed = None;
        if feed_changed {
            self.task = Task::ready(());
            self.busy = None;
            self.checked = false;
            self.recount();
            self.auto(cx);
        } else {
            self.recount();
            if self.announces() {
                self.fetch(cx);
            }
        }
        cx.notify();
    }

    fn selection(&self, cx: &App) -> (String, updater::Channel) {
        let prefs = self.settings.read(cx).prefs();
        let feed = prefs.update_feed.as_deref().map(str::trim).filter(|feed| !feed.is_empty()).unwrap_or(FEED);
        (feed.to_owned(), prefs.update_channel)
    }

    fn recount(&mut self) {
        let (feed, channel) = &self.selected;
        self.offer = match self.saved.feed == *feed {
            true => updater::newer(&self.saved.releases, &self.current, *channel),
            false => Vec::new(),
        };
        if let Some(skipped) = &self.saved.skipped {
            // By precedence, as `newer` is: a rebuild of the skipped version (`1.0.0+b2`) stays skipped.
            self.offer.retain(|release| release.version.cmp_precedence(skipped).is_gt());
        }
        self.publish();
    }

    /// Whether `release` is the one on offer (a republished asset is another release).
    fn is_newest(&self, release: &Release) -> bool {
        self.latest().is_some_and(|newest| newest.version == release.version && newest.sha256 == release.sha256)
    }

    /// The fetched release, if it is still the one on offer.
    fn ready(&self) -> Option<&Release> {
        self.staged.as_ref().filter(|staged| self.is_newest(staged))
    }

    /// Tells the quit hook what to swap in.
    fn publish(&self) {
        self.plan.borrow_mut().ready = self.ready().map(|release| release.version.clone());
    }
}

/// On quit: the fetched release takes the executable's place, and a restart starts it.
fn apply(plan: &Plan) {
    let (Some(exe), Some(version)) = (plan.exe.as_deref(), &plan.ready) else { return };
    if let Err(e) = updater::swap(exe) {
        return log::warn!("update not installed: {e:#}");
    }
    log::info!("updated to {version}");
    if plan.restarting
        && let Err(e) = updater::restart(exe)
    {
        log::warn!("{e:#}");
    }
}
