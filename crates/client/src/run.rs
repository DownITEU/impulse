//! A run of a game's trainer: what starting it takes, and what it leaves in the game's profile.

use model::{Installation, Plan, Rule};
pub use play::{Request, Stage, run as start};

use crate::profile::{Profile, Values};

/// Starting `plan` on the installed game.
pub fn request(install: &Installation, plan: &Plan) -> Request {
    Request {
        runner: plan.runner.clone(),
        store_id: install.store_id.clone(),
        rule: Rule { image: plan.image.clone(), location: install.location.clone(), minimum_age: plan.minimum_age },
        launch: plan.launch,
    }
}

/// Counts a run that went live; the mods it starts with, when the game keeps them.
pub fn opened(profile: &mut Profile) -> Option<Values> {
    profile.sessions += 1;
    profile.save_mods.then(|| profile.values.clone())
}

/// Keeps a run's time, and the mods it `left` on when the game keeps them.
pub fn closed(profile: &mut Profile, secs: u64, left: Option<Values>) {
    profile.played += secs;
    profile.last = secs;
    if let Some(left) = left.filter(|_| profile.save_mods) {
        profile.values = left;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_keeps_its_mods_only_when_the_game_saves_them() {
        let left = || Some(Values::from([("god".to_owned(), 1.)]));
        let mut profile = Profile::default();
        assert_eq!(opened(&mut profile), None);
        closed(&mut profile, 30, left());
        assert!(profile.values.is_empty());
        profile.save_mods = true;
        closed(&mut profile, 12, left());
        assert_eq!((profile.sessions, profile.played, profile.last), (1, 42, 12));
        assert_eq!(opened(&mut profile), left());
    }
}
