//! Cue sounds: a short tone (or the user's WAV) when a live cheat's control changes, for the cues the user
//! left on. `Cues` is the only caller of `sound`; `SoundsView` is the SOUNDS tab.

mod view;

use std::path::PathBuf;

use blocking::unblock;
use client::library::GameId;
use futures::{
    StreamExt,
    channel::mpsc::{self, UnboundedSender},
};
use gpui_kit::{App, Context, Entity, PathPromptOptions, Task};
use model::Kind;
use sound::Cue;

pub use view::SoundsView;

use crate::{
    run::{RunEvent, Runs},
    settings::Settings,
    trainer::{TrainerEvent, Trainers},
    ui::{key, text::t},
};

pub struct Cues {
    trainers: Entity<Trainers>,
    settings: Entity<Settings>,
    /// The cue whose last picked file was no clip.
    refused: Option<Cue>,
    /// The pick in progress: a newer one or a reset drops it, so a late answer changes nothing.
    choice: Task<()>,
    /// The cues to play, in order: `sound::play` opens a custom file, which may sit on a stalled share.
    plays: UnboundedSender<(Cue, Option<PathBuf>)>,
    _player: Task<()>,
}

impl Cues {
    pub fn new(runs: &Entity<Runs>, trainers: &Entity<Trainers>, settings: &Entity<Settings>, cx: &mut Context<Self>) -> Self {
        cx.subscribe(trainers, |cues, _, event: &TrainerEvent, cx| {
            if let TrainerEvent::Set { game, index, value } = event {
                cues.cue(game, *index, *value, cx);
            }
        })
        .detach();
        // A button keeps no value: its press is the change.
        cx.subscribe(runs, |cues, runs, event: &RunEvent, cx| {
            if let RunEvent::Pressed(index) = event
                && let Some(game) = runs.read(cx).live().cloned()
            {
                cues.cue(&game, *index, 1., cx);
            }
        })
        .detach();
        let (plays, mut queue) = mpsc::unbounded::<(Cue, Option<PathBuf>)>();
        let player = cx.spawn(async move |_, _| {
            while let Some(mut play) = queue.next().await {
                // Only the newest waiting cue is heard: it would replace the others at once.
                while let Ok(newer) = queue.try_recv() {
                    play = newer;
                }
                unblock(move || sound::play(play.0, play.1.as_deref())).await;
            }
        });
        Self { trainers: trainers.clone(), settings: settings.clone(), refused: None, choice: Task::ready(()), plays, _player: player }
    }

    pub fn refused(&self) -> Option<Cue> {
        self.refused
    }

    /// Plays `cue` as the user set it up, whether it is on or not: LISTEN.
    pub fn play(&self, cue: Cue, cx: &App) {
        let file = self.settings.read(cx).prefs().cues.file(cue).map(PathBuf::from);
        self.plays.unbounded_send((cue, file)).ok();
    }

    /// Asks for a WAV to play in place of `cue`'s tone and plays it once taken.
    pub fn choose(&mut self, cue: Cue, cx: &mut Context<Self>) {
        let prompt = Some(t(cx, key::SOUNDS_PROMPT));
        let picked = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt });
        self.choice = cx.spawn(async move |cues, cx| {
            let Ok(Ok(Some(mut paths))) = picked.await else { return };
            let Some(file) = paths.pop() else { return };
            let clip = unblock({
                let file = file.clone();
                move || sound::is_clip(&file)
            })
            .await;
            cues.update(cx, |cues, cx| {
                cues.refused = (!clip).then_some(cue);
                if clip {
                    cues.settings.update(cx, |settings, cx| settings.edit(|prefs| prefs.cues.set_file(cue, Some(file)), cx));
                    cues.play(cue, cx);
                }
                cx.notify();
            })
            .ok();
        });
    }

    /// Back to `cue`'s own tone.
    pub fn clear(&mut self, cue: Cue, cx: &mut Context<Self>) {
        self.refused = None;
        self.choice = Task::ready(());
        self.settings.update(cx, |settings, cx| settings.edit(|prefs| prefs.cues.set_file(cue, None), cx));
        cx.notify();
    }

    fn cue(&self, game: &GameId, index: usize, value: f64, cx: &App) {
        let Some(cheat) = self.trainers.read(cx).sheet(game).and_then(|sheet| sheet.cheats.get(index)) else { return };
        let cue = match cheat.kind {
            Kind::Toggle if value == 0. => Cue::ToggleOff,
            Kind::Toggle => Cue::ToggleOn,
            Kind::Slider => Cue::Slider,
            // Its SET is a press.
            Kind::Number if cheat.args.button => Cue::Button,
            Kind::Number => Cue::Number,
            Kind::Button => Cue::Button,
            Kind::Selection => Cue::Selection,
            Kind::Unknown => return,
        };
        if self.settings.read(cx).prefs().cues.plays(cue) {
            self.play(cue, cx);
        }
    }
}
