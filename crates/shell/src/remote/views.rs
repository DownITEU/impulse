//! The panel's views, made from the owners' state once per burst of changes; each goes out only when it changed.

use std::{
    mem,
    ops::BitOr,
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use client::{
    Load,
    api::{self, CheatRow, Collection, Control, Game, RunState, Section, View},
    trainer::{PINNED, Sheet},
};
use gpui_kit::{App, Context, Entity};

use super::{Handles, Remote, State};
use crate::{
    cheats::category,
    library::Library,
    run::{Runs, State as Play},
    settings::Settings,
    trainer::Trainers,
    ui::{
        key,
        text::{failed, t, why},
        theme::accent,
    },
};

/// Which views may have changed.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct Dirty(u8);

impl Dirty {
    pub const HELLO: Self = Self(1);
    pub const LIBRARY: Self = Self(2);
    pub const TRAINER: Self = Self(4);
    pub const RUN: Self = Self(8);
    pub const VALUES: Self = Self(16);
    pub const ALL: Self = Self(31);

    fn has(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl BitOr for Dirty {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// The last of each view sent.
#[derive(Default)]
pub struct Shown {
    hello: Option<View>,
    library: Option<View>,
    trainer: Option<View>,
    values: Option<View>,
    run: Option<View>,
    /// The trainer view's generation: cheat commands name it.
    pub sheet: u32,
    /// When the live run started, in Unix seconds: made once, so the panel's clock never jitters.
    since: Option<u64>,
}

impl Remote {
    /// Marks views to make again; a burst of changes makes them once, after the current update.
    pub(super) fn touch(&mut self, dirty: Dirty, cx: &mut Context<Self>) {
        let State::On(on) = &mut self.state else { return };
        let idle = on.dirty == Dirty::default();
        on.dirty = on.dirty | dirty;
        if idle {
            let remote = cx.weak_entity();
            cx.defer(move |cx| drop(remote.update(cx, |remote, cx| remote.publish(cx))));
        }
    }

    fn publish(&mut self, cx: &mut Context<Self>) {
        let Self { state: State::On(on), handles, settings, library, trainers, runs, .. } = self else { return };
        let (dirty, views, shown) = (mem::take(&mut on.dirty), &on.views, &mut on.shown);
        let send = |kept: &mut Option<View>, view: View| {
            if kept.as_ref() != Some(&view) {
                views.publish(&view);
                *kept = Some(view);
            }
        };
        if dirty.has(Dirty::HELLO) {
            send(&mut shown.hello, hello(settings, cx));
        }
        if dirty.has(Dirty::LIBRARY) {
            send(&mut shown.library, games(handles, library, trainers, runs, cx));
        }
        if dirty.has(Dirty::TRAINER) {
            let mut view = trainer(shown.sheet, handles, library, trainers, cx);
            // An equal view keeps its generation: a command against it still means the rows the device shows.
            if shown.trainer.as_ref() != Some(&view) {
                shown.sheet += 1;
                if let View::Trainer { sheet, .. } = &mut view {
                    *sheet = shown.sheet;
                }
                views.publish(&view);
                shown.trainer = Some(view);
            }
        }
        if dirty.has(Dirty::TRAINER | Dirty::VALUES) {
            let open = listed_open(library.read(cx)).map(|game| &game.id);
            let values = open.and_then(|game| trainers.read(cx).sheet(game)).map(|sheet| sheet.values.clone()).unwrap_or_default();
            send(&mut shown.values, View::Values { sheet: shown.sheet, values });
        }
        if dirty.has(Dirty::RUN) {
            let view = run(handles, runs, &mut shown.since, cx);
            send(&mut shown.run, view);
        }
    }
}

fn hello(settings: &Entity<Settings>, cx: &App) -> View {
    let language = settings.read(cx).prefs().language().tag.to_owned();
    View::Hello { version: api::VERSION, language, accent: format!("#{:06x}", accent(cx)) }
}

fn games(handles: &mut Handles, library: &Entity<Library>, trainers: &Entity<Trainers>, runs: &Entity<Runs>, cx: &App) -> View {
    let (library, trainers, runs) = (library.read(cx), trainers.read(cx), runs.read(cx));
    let games = library.games().iter().map(|game| Game {
        game: handles.of(&game.id),
        name: game.name.to_string(),
        favorite: trainers.game_profile(&game.id).favorite,
        open: library.open_id() == Some(&game.id),
        live: runs.is_live(&game.id),
        startable: runs.can_start(game, cx),
        art: game.thumbnail.is_some(),
    });
    View::Library { games: games.collect() }
}

/// The open game's trainer as the panel shows it.
fn trainer(generation: u32, handles: &mut Handles, library: &Entity<Library>, trainers: &Entity<Trainers>, cx: &App) -> View {
    let Some(game) = listed_open(library.read(cx)) else {
        return View::Trainer { sheet: generation, game: None, trainer: api::Sheet::None };
    };
    let trainers = trainers.read(cx);
    let sheet = match trainers.get(&game.id) {
        Some(Load::Ready(sheet)) => ready(sheet, trainers.trainer_profile(&game.id).collections.iter(), cx),
        Some(Load::Failed(failure)) => api::Sheet::Failed { why: failed(cx, key::CHEATS_FAILED_BODY, failure).to_string() },
        Some(Load::Pending) => api::Sheet::Pending,
        None if trainers.is_loading(&game.id) => api::Sheet::Pending,
        None => api::Sheet::None,
    };
    View::Trainer { sheet: generation, game: Some(handles.of(&game.id)), trainer: sheet }
}

/// The open game while it is listed: a catalog game the desktop browses is not the panel's.
pub(super) fn listed_open(library: &Library) -> Option<&Arc<client::library::Game>> {
    library.open_id().and_then(|id| library.game(id))
}

fn ready<'a>(sheet: &Sheet, collections: impl Iterator<Item = &'a client::profile::Collection>, cx: &App) -> api::Sheet {
    let sections = sheet.sections().into_iter().filter(|(_, cheats)| !cheats.is_empty());
    let sections = sections.map(|(id, cheats)| Section {
        name: match id == PINNED {
            true => t(cx, key::CHEATS_PINNED).to_string(),
            false => category(&sheet.cheats[cheats[0]], cx).to_string(),
        },
        pinned: id == PINNED,
        cheats: cheats.into_iter().map(|index| index as u32).collect(),
    });
    let cheats = sheet.cheats.iter().enumerate().map(|(index, cheat)| CheatRow {
        name: cheat.name.clone(),
        description: cheat.description.clone(),
        instructions: cheat.instructions.clone(),
        pinned: sheet.pinned.contains(&index),
        control: Control::of(cheat),
    });
    let collections = collections.map(|kept| Collection { name: kept.name.clone(), icon: kept.icon.clone() });
    api::Sheet::Ready { sections: sections.collect(), cheats: cheats.collect(), collections: collections.collect() }
}

fn run(handles: &mut Handles, runs: &Entity<Runs>, since: &mut Option<u64>, cx: &App) -> View {
    let runs = runs.read(cx);
    let game = runs.game();
    let (state, reason) = match game.map(|game| runs.state(game)) {
        Some(Play::Starting(_)) => (RunState::Starting, None),
        Some(Play::Live) => (RunState::Live, None),
        Some(Play::Stopping) => (RunState::Stopping, None),
        Some(Play::Failed(error)) => (RunState::Failed, Some(why(cx, error).to_string())),
        _ => (RunState::Idle, None),
    };
    *since = runs.since().map(|live| since.unwrap_or_else(|| unix(live)));
    View::Run { state, game: game.map(|game| handles.of(game)), since: *since, why: reason }
}

fn unix(instant: Instant) -> u64 {
    let then = SystemTime::now().checked_sub(instant.elapsed()).unwrap_or(UNIX_EPOCH);
    then.duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs())
}
