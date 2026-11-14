//! A run of a game's trainer: what starting it takes, and what it leaves in the game's profile.

use model::{Installation, Plan, Rule};
pub use play::{Request, Stage, run as start};

use crate::profile::{GameProfile, TrainerProfile, Values};

/// Starting `plan` on the installed game.
pub fn request(install: &Installation, plan: &Plan) -> Request {
    Request {
        runner: plan.runner.clone(),
        origin: install.origin.clone(),
        rule: Rule { image: plan.image.clone(), location: install.location.clone(), minimum_age: plan.minimum_age },
        launch: plan.launch,
    }
}

/// Counts a run that went live; the mods it starts with, when the game keeps them.
pub fn opened(game: &mut GameProfile, trainer: &mut TrainerProfile) -> Option<Values> {
    game.sessions += 1;
    trainer.save_mods.then(|| trainer.values.clone())
}

/// Keeps a run's time, and the mods it `left` on when the game keeps them.
pub fn closed(game: &mut GameProfile, trainer: &mut TrainerProfile, secs: u64, left: Option<Values>) {
    game.played += secs;
    game.last = secs;
    if let Some(left) = left.filter(|_| trainer.save_mods) {
        trainer.values = left;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_keeps_its_mods_only_when_the_game_saves_them() {
        let left = || Some(Values::from([("god".to_owned(), 1.)]));
        let (mut game, mut trainer) = (GameProfile::default(), TrainerProfile::default());
        assert_eq!(opened(&mut game, &mut trainer), None);
        closed(&mut game, &mut trainer, 30, left());
        assert!(trainer.values.is_empty());
        trainer.save_mods = true;
        closed(&mut game, &mut trainer, 12, left());
        assert_eq!((game.sessions, game.played, game.last), (1, 42, 12));
        assert_eq!(opened(&mut game, &mut trainer), left());
    }
}
