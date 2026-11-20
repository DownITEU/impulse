//! A panic on the UI thread ends Impulse: GPUI does not catch it. Before that, a live trainer is stopped as
//! STOP stops it, so its mods do not stay on in the game; the session runs on threads of its own meanwhile.

use std::{
    panic,
    sync::{Arc, Mutex, OnceLock, Weak, mpsc},
    thread::{self, ThreadId},
    time::Duration,
};

use gpui_kit::{App, Entity};
use model::Session;

use crate::run::{RunEvent, Runs};

// The process ends after this at most.
const WAIT: Duration = Duration::from_secs(2);

static UI: OnceLock<ThreadId> = OnceLock::new();
static LIVE: Mutex<Option<Weak<dyn Session>>> = Mutex::new(None);

/// On the UI thread, after the journal's hook, which logs the panic first.
pub fn install() {
    if UI.set(thread::current().id()).is_err() {
        return;
    }
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        previous(info);
        // A panic elsewhere may be caught, and Impulse goes on.
        if UI.get() == Some(&thread::current().id()) {
            stop();
        }
    }));
}

pub fn watch(runs: &Entity<Runs>, cx: &mut App) {
    cx.subscribe(runs, |runs, event: &RunEvent, cx| {
        if let (RunEvent::Live, Some((_, run))) = (event, runs.read(cx).live_run()) {
            follow(&run.session);
        }
    })
    .detach();
}

/// The run that went live; an ended one's session has nothing left to stop.
fn follow(session: &Arc<dyn Session>) {
    if let Ok(mut live) = LIVE.lock() {
        *live = Some(Arc::downgrade(session));
    }
}

fn stop() {
    // Not `lock`: the panic may have come while it was held.
    let Some(session) = LIVE.try_lock().ok().and_then(|live| live.as_ref()?.upgrade()) else { return };
    log::warn!("stopping the trainer before Impulse ends");
    let (done, stopped) = mpsc::channel();
    // A thread of its own: a lock the panicking code holds may keep it waiting.
    let spawned = thread::Builder::new().spawn(move || done.send(futures::executor::block_on(session.stop())));
    match spawned.map(|_| stopped.recv_timeout(WAIT)) {
        Ok(Ok(Ok(()))) => log::info!("trainer stopped"),
        Ok(Ok(Err(e))) => log::warn!("stopping the trainer: {e:#}"),
        _ => log::warn!("the trainer did not stop in time"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use model::Failure;

    type Result<T> = std::result::Result<T, Failure>;
    use futures::{FutureExt, future::BoxFuture};

    use super::*;

    #[derive(Default)]
    struct Fake {
        stopped: AtomicBool,
    }

    impl Session for Fake {
        fn set<'a>(&'a self, _: &'a str, _: f64) -> BoxFuture<'a, Result<()>> {
            async { Ok(()) }.boxed()
        }

        fn stop(&self) -> BoxFuture<'_, Result<()>> {
            self.stopped.store(true, Ordering::SeqCst);
            async { Ok(()) }.boxed()
        }

        fn position(&self) -> BoxFuture<'_, Result<[f64; 3]>> {
            async { Ok([0.; 3]) }.boxed()
        }

        fn teleport(&self, _: [f64; 3]) -> BoxFuture<'_, Result<()>> {
            async { Ok(()) }.boxed()
        }
    }

    #[test]
    fn a_ui_panic_stops_the_live_trainer_and_another_thread_does_not() {
        install();
        let fake = Arc::new(Fake::default());
        let session: Arc<dyn Session> = fake.clone();
        follow(&session);
        thread::spawn(|| panic::catch_unwind(|| panic!("caught elsewhere"))).join().unwrap().unwrap_err();
        assert!(!fake.stopped.load(Ordering::SeqCst));
        panic::catch_unwind(|| panic!("on the UI thread")).unwrap_err();
        assert!(fake.stopped.load(Ordering::SeqCst));
    }
}
