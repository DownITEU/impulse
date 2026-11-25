//! A device's commands: checked, then applied through the owners as the window applies them. A refusal is a note to
//! that device.

use client::{
    api::Command,
    library::GameId,
    profile::Collection,
    trainer::{Sheet, settle},
};
use gpui_kit::{App, Context};
use i18n::Key;
use model::{Cheat, Kind};

use super::{Remote, State, views::listed_open};
use crate::{
    cheats::DEFAULT_ICON,
    run::State as Play,
    ui::{
        key::{self, REMOTE_NOTE_CHANGED as CHANGED},
        text::t,
    },
};

impl Remote {
    pub(super) fn command(&mut self, device: u32, command: Command, cx: &mut Context<Self>) {
        // A revoked device's socket may still have sent one.
        if !self.devices.has(device) {
            return;
        }
        if let Err(note) = self.apply(command, cx)
            && let State::On(on) = &self.state
        {
            on.views.note(device, &t(cx, note));
        }
    }

    fn apply(&mut self, command: Command, cx: &mut Context<Self>) -> Result<(), Key> {
        match command {
            Command::Open { game } => {
                let game = self.game(game, cx)?;
                self.library.update(cx, |library, cx| library.open(game, cx));
            }
            Command::Favorite { game } => {
                let game = self.game(game, cx)?;
                self.trainers.update(cx, |trainers, cx| trainers.toggle_favorite(&game, cx));
            }
            Command::Start { game } => {
                let game = self.game(game, cx)?;
                if !self.library.read(cx).game(&game).is_some_and(|listed| self.runs.read(cx).can_start(listed, cx)) {
                    return Err(key::REMOTE_NOTE_BUSY);
                }
                // The window follows the panel.
                if self.library.read(cx).open_id() != Some(&game) {
                    self.library.update(cx, |library, cx| library.open(game.clone(), cx));
                }
                self.runs.update(cx, |runs, cx| runs.start(game, cx));
            }
            Command::Stop { game } => {
                let game = self.game(game, cx)?;
                if !matches!(self.runs.read(cx).state(&game), Play::Starting(_) | Play::Live) {
                    return Err(key::REMOTE_NOTE_NOT_LIVE);
                }
                self.runs.update(cx, |runs, cx| runs.stop(cx));
            }
            Command::Set { sheet, cheat, value } => {
                let (game, index, cheat) = self.cheat(sheet, cheat, cx)?;
                let value = settle(cheat, value).ok_or(CHANGED)?;
                self.live(&game, cx)?;
                self.runs.update(cx, |runs, cx| runs.set(index, value, cx));
            }
            Command::Press { sheet, cheat } => {
                let (game, index, cheat) = self.cheat(sheet, cheat, cx)?;
                if cheat.kind != Kind::Button {
                    return Err(CHANGED);
                }
                self.live(&game, cx)?;
                self.runs.update(cx, |runs, cx| runs.press(index, cx));
            }
            Command::Pin { sheet, cheat } => {
                let (game, index, _) = self.cheat(sheet, cheat, cx)?;
                self.trainers.update(cx, |trainers, cx| trainers.toggle_pin(&game, index, cx));
            }
            Command::Reset { sheet } => {
                let (game, _) = self.sheet(sheet, cx)?;
                self.runs.update(cx, |runs, cx| runs.reset(&game, cx));
            }
            Command::Collect { sheet, name } => {
                let (game, sheet) = self.sheet(sheet, cx)?;
                let mods = sheet.live_values();
                if mods.is_empty() {
                    return Err(key::REMOTE_NOTE_NOTHING_ON);
                }
                let collection = Collection::new(&name, DEFAULT_ICON, mods).ok_or(key::REMOTE_NOTE_NAME)?;
                self.trainers.update(cx, |trainers, cx| trainers.keep_collection(&game, None, collection, cx));
            }
            Command::Apply { sheet, collection } => {
                let game = self.collection(sheet, collection, cx)?;
                self.live(&game, cx)?;
                self.runs.update(cx, |runs, cx| runs.toggle_collection(&game, collection as usize, cx));
            }
            Command::Forget { sheet, collection } => {
                let game = self.collection(sheet, collection, cx)?;
                self.trainers.update(cx, |trainers, cx| trainers.delete_collection(&game, collection as usize, cx));
            }
        }
        Ok(())
    }

    fn game(&self, handle: u32, cx: &App) -> Result<GameId, Key> {
        let game = self.handles.game(handle).filter(|game| self.library.read(cx).game(game).is_some());
        game.cloned().ok_or(CHANGED)
    }

    /// The open game and its sheet, when `sheet` is the trainer view the panel was last sent.
    fn sheet<'a>(&self, sheet: u32, cx: &'a App) -> Result<(GameId, &'a Sheet), Key> {
        let State::On(on) = &self.state else { return Err(CHANGED) };
        let game = listed_open(self.library.read(cx)).map(|game| &game.id).filter(|_| on.shown.sheet == sheet).ok_or(CHANGED)?;
        let sheet = self.trainers.read(cx).sheet(game).ok_or(CHANGED)?;
        Ok((game.clone(), sheet))
    }

    fn cheat<'a>(&self, sheet: u32, cheat: u32, cx: &'a App) -> Result<(GameId, usize, &'a Cheat), Key> {
        let (game, sheet) = self.sheet(sheet, cx)?;
        let index = cheat as usize;
        Ok((game, index, sheet.cheats.get(index).ok_or(CHANGED)?))
    }

    /// The open game, when it still has the collection.
    fn collection(&self, sheet: u32, collection: u32, cx: &App) -> Result<GameId, Key> {
        let (game, _) = self.sheet(sheet, cx)?;
        let kept = self.trainers.read(cx).trainer_profile(&game).collections.len();
        ((collection as usize) < kept).then_some(game).ok_or(CHANGED)
    }

    fn live(&self, game: &GameId, cx: &App) -> Result<(), Key> {
        self.runs.read(cx).is_live(game).then_some(()).ok_or(key::REMOTE_NOTE_NOT_LIVE)
    }
}
