//! A chord another hotkey holds: the question over the slot's cap and its answers.

use client::{
    bindings::{Choice, Clash, Resolution},
    library::GameId,
};
use gpui_kit::{App, Context, FocusHandle, Window};

use super::{Hotkeys, Target};
use crate::input::Owner;

/// The question asked after a chord was pressed for a slot; the slot keeps its old hotkey until it is answered.
pub struct Conflict {
    game: GameId,
    cheat: String,
    slot: usize,
    choice: Choice,
    pub clash: Clash,
    /// The chord asked for, and the cheats holding it, as printed.
    pub keys: String,
    pub names: String,
    /// What had the focus before the question took it.
    before: Option<FocusHandle>,
}

impl Hotkeys {
    /// The question over this slot, if it is the one being asked.
    pub fn conflict_at(&self, game: &GameId, cheat: &str, slot: usize) -> Option<&Conflict> {
        self.conflict.as_ref().filter(|asked| asked.game == *game && asked.cheat == cheat && asked.slot == slot)
    }

    /// The focus of the question: Escape reaches it while it holds it.
    pub fn focus(&self) -> &FocusHandle {
        &self.focus
    }

    /// The game whose slot is being bound or asked about.
    pub(super) fn binding_game(&self) -> Option<&GameId> {
        match self.capturing() {
            Some(Target::Slot { game, .. }) => Some(game),
            _ => self.conflict.as_ref().map(|asked| &asked.game),
        }
    }

    /// Drops the question and its press.
    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(asked) = self.conflict.take() {
            self.give_back(asked.before, window, cx);
            cx.notify();
        }
    }

    /// Binds the slot as asked, against the bindings as they are now: they may have changed since.
    pub fn answer(&mut self, resolution: Resolution, window: &mut Window, cx: &mut Context<Self>) {
        let Some(asked) = self.conflict.take() else { return };
        self.give_back(asked.before, window, cx);
        self.bind(&asked.game, &asked.cheat, asked.slot, asked.choice, Some(resolution), window, cx);
    }

    fn give_back(&self, before: Option<FocusHandle>, window: &mut Window, cx: &mut App) {
        if let Some(before) = before.filter(|_| self.focus.is_focused(window)) {
            before.focus(window, cx);
        }
    }

    /// Binds a cheat's slot, or asks about the chord when it is held. `resolution` is the answer to such a question.
    #[expect(clippy::too_many_arguments, reason = "a slot, its choice and its answer, over the window it is bound in")]
    pub(super) fn bind(
        &mut self,
        game: &GameId,
        cheat: &str,
        slot: usize,
        choice: Choice,
        resolution: Option<Resolution>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let app: &App = cx;
        let Some(sheet) = self.trainers.read(app).sheet(game) else { return };
        let Some(index) = sheet.cheats.iter().position(|found| found.id == cheat) else { return };
        let asked = self.store.rebind(&sheet.trainer, &sheet.cheats, (index, slot), choice, resolution, |chord| {
            self.input.read(app).taken(Owner::Trainer, chord)
        });
        let clash = match asked {
            Ok(()) => {
                let slots = self.slots(sheet);
                self.trainers.update(cx, |trainers, cx| trainers.bind(game, slots, cx));
                if self.runs.read(cx).is_live(game) {
                    self.arm(cx);
                }
                return;
            }
            // A press that ended as the window went inactive has no one to answer it.
            Err(_) if !self.attended(window, app) => return,
            Err(clash) => clash,
        };
        let keys = choice.chord(&sheet.cheats[index], slot).map(|chord| chord.to_string()).unwrap_or_default();
        let mut holders: Vec<usize> = match &clash {
            Clash::Slots { held, .. } => held.iter().map(|&(holder, _)| holder).collect(),
            Clash::Reserved => Vec::new(),
        };
        holders.dedup();
        let names = holders.iter().map(|&holder| sheet.cheats[holder].name.to_uppercase()).collect::<Vec<_>>().join(", ");
        // An answer that asks again keeps the focus it first took.
        let before = self.conflict.as_ref().map_or_else(|| window.focused(app), |asked| asked.before.clone());
        self.focus.focus(window, cx);
        self.conflict = Some(Conflict { game: game.clone(), cheat: cheat.to_owned(), slot, choice, clash, keys, names, before });
        cx.notify();
    }
}
