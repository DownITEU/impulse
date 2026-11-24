//! A run of a game's trainer: the trainer prepared, the game's process found or started, the trainer inside it; and
//! what a run leaves in the game's profile.

use std::{sync::Arc, time::Duration};

use blocking::unblock;
use model::{Failure, FailureKind, Installation, Launch, Live, Origin, Plan, Process, Rule, Runner};

use crate::profile::{GameProfile, TrainerProfile, Values};

type Result<T> = std::result::Result<T, Failure>;

// Allow extra time when the user has to launch the game manually.
const LAUNCHED_WITHIN: Duration = Duration::from_secs(45);
const STARTED_BY_USER_WITHIN: Duration = Duration::from_secs(180);

pub struct Request {
    pub runner: Arc<dyn Runner>,
    pub origin: Origin,
    pub rule: Rule,
    pub launch: Launch,
    pub build: Option<u32>,
    pub app: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    Preparing,
    Launching,
    WaitingForGame,
    Attaching,
}

pub fn request(install: &Installation, plan: &Plan) -> Request {
    Request {
        runner: plan.runner.clone(),
        origin: install.origin.clone(),
        rule: Rule {
            image: plan.image.clone(),
            location: install.location.clone(),
            alternates: install.alternates.clone(),
            minimum_age: plan.minimum_age,
        },
        launch: plan.launch,
        build: install.build,
        app: install.app.clone(),
    }
}

/// The game's process and the live trainer. Dropping the future abandons the run; a trainer that is
/// already in the game ends with its session.
pub async fn start(request: Request, mut stage: impl FnMut(Stage)) -> Result<(Process, Live)> {
    stage(Stage::Preparing);
    let prepared = request.runner.prepare().await?;

    let required = request.launch == Launch::Required;
    // A game the run must start counts at any age: one started a moment ago runs all the same.
    let minimum_age = if required { Duration::ZERO } else { request.rule.minimum_age };
    // Still starting up, it is waited for, never started a second time.
    stage(Stage::WaitingForGame);
    let running = agent::settled(&Rule { minimum_age, ..request.rule.clone() }).await?;
    // Held until the run ends, whatever the outcome.
    let mut _ready = None;
    let process = match running {
        Some(_) if required => return Err(Failure::new(FailureKind::Restart)),
        Some(process) => process,
        None => {
            let mut started = false;
            if request.launch != Launch::User {
                _ready = prepared.before_start(&request.origin, &request.rule.location).await?;
                stage(Stage::Launching);
                let (origin, location, app) = (request.origin.clone(), request.rule.location.clone(), request.app.clone());
                started = unblock(move || launchers::start(&origin, &location, app.as_deref())).await?;
            }
            stage(Stage::WaitingForGame);
            let within = if started { LAUNCHED_WITHIN } else { STARTED_BY_USER_WITHIN };
            agent::wait(&request.rule, within).await?
        }
    };

    stage(Stage::Attaching);
    Ok((process.clone(), prepared.attach(process, request.build).await?))
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
    use std::{
        path::{Path, PathBuf},
        sync::atomic::{AtomicUsize, Ordering},
    };

    use futures::{FutureExt, executor::block_on, future::BoxFuture};
    use model::Attach;

    use super::*;

    /// Counts what the run asked of it; attaching always fails.
    #[derive(Default)]
    struct Probe {
        readied: AtomicUsize,
        attached: AtomicUsize,
    }

    struct Prepared(Arc<Probe>);
    struct Prepare(Arc<Probe>);

    impl Runner for Prepare {
        fn prepare(&self) -> BoxFuture<'_, Result<Box<dyn Attach>>> {
            let prepared: Box<dyn Attach> = Box::new(Prepared(self.0.clone()));
            async move { Ok(prepared) }.boxed()
        }
    }

    impl Attach for Prepared {
        fn before_start(&self, _: &Origin, _: &Path) -> BoxFuture<'static, Result<Option<Box<dyn Send>>>> {
            self.0.readied.fetch_add(1, Ordering::SeqCst);
            async { Ok(None) }.boxed()
        }

        fn attach(self: Box<Self>, _: Process, _: Option<u32>) -> BoxFuture<'static, Result<Live>> {
            self.0.attached.fetch_add(1, Ordering::SeqCst);
            async { Err(Failure::new(FailureKind::Refused)) }.boxed()
        }
    }

    #[test]
    fn a_running_game_is_refused_untouched_when_the_run_must_start_it() {
        let probe = Arc::new(Probe::default());
        let me = std::env::current_exe().unwrap();
        let image = me.file_name().unwrap().to_string_lossy().into_owned();
        let request = |launch, minimum_age| {
            let rule =
                Rule { image: Some(image.clone()), location: PathBuf::from(me.parent().unwrap()), alternates: Vec::new(), minimum_age };
            Request { runner: Arc::new(Prepare(probe.clone())), origin: Origin::Picked(me.clone()), rule, launch, build: None, app: None }
        };

        // Younger than the rule's age, and still refused.
        let refused = block_on(start(request(Launch::Required, Duration::from_secs(3600)), |_| {})).err().unwrap();
        assert_eq!(refused.kind, FailureKind::Restart);
        assert_eq!((probe.readied.load(Ordering::SeqCst), probe.attached.load(Ordering::SeqCst)), (0, 0));

        let attached = block_on(start(request(Launch::Start, Duration::ZERO), |_| {})).err().unwrap();
        assert_eq!(attached.kind, FailureKind::Refused, "a run that may attach does");
        assert_eq!((probe.readied.load(Ordering::SeqCst), probe.attached.load(Ordering::SeqCst)), (0, 1));
    }

    #[test]
    fn a_game_still_starting_up_is_waited_for_not_started_again() {
        let probe = Arc::new(Probe::default());
        let me = std::env::current_exe().unwrap();
        let image = Some(me.file_name().unwrap().to_string_lossy().into_owned());
        // Longer than the tests have run: a start would ready the install first. No launcher starts that store.
        let rule =
            Rule { image, location: PathBuf::from(me.parent().unwrap()), alternates: Vec::new(), minimum_age: Duration::from_secs(3) };
        let origin = Origin::Launcher(model::StoreId::parse("itch:0").unwrap());
        let request = Request { runner: Arc::new(Prepare(probe.clone())), origin, rule, launch: Launch::Start, build: None, app: None };

        let attached = block_on(start(request, |_| {})).err().unwrap();
        assert_eq!(attached.kind, FailureKind::Refused);
        assert_eq!((probe.readied.load(Ordering::SeqCst), probe.attached.load(Ordering::SeqCst)), (0, 1));
    }

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
