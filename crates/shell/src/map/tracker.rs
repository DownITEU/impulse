//! Where the player is and which way they head, asked of the live trainer one question at a time, only while
//! a lease is held. The heading lasts the run, so a map opened later shows the way the player last went.

use std::{
    rc::{Rc, Weak},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Result;
use gpui_kit::{Context, EventEmitter, Task};
use model::{Frame, Session};

use super::heading::{Bearing, Heading};

// Failed answers in a row before the player counts as lost; asking goes on, slower each time.
const GIVE_UP: u32 = 3;
const SLOWEST: Duration = Duration::from_secs(60);

/// The player's position in the game's own coordinates, and which way they head.
#[derive(Clone, Copy)]
pub struct Fix {
    pub position: [f64; 3],
    pub bearing: Option<Bearing>,
}

pub enum Tracked {
    Fix(Fix),
    /// The trainer stopped answering.
    Lost,
}

/// Wants a fix this often while held.
pub struct Lease {
    _every: Rc<Duration>,
}

/// One per live run whose trainer can tell where the player is.
pub struct Tracker {
    session: Arc<dyn Session>,
    // The plane the heading is worked out on.
    frame: Option<Frame>,
    heading: Heading,
    last: Option<[f64; 3]>,
    leases: Vec<Weak<Duration>>,
    asking: bool,
    failures: u32,
    _asks: Task<()>,
}

impl EventEmitter<Tracked> for Tracker {}

impl Tracker {
    pub fn new(session: Arc<dyn Session>) -> Self {
        Self { session, frame: None, heading: Heading::default(), last: None, leases: Vec::new(), asking: false, failures: 0, _asks: Task::ready(()) }
    }

    /// `frame`: the leaseholder's map; another one starts the heading over. A rate change applies after the
    /// wait under way.
    pub fn lease(&mut self, every: Duration, frame: &Frame, cx: &mut Context<Self>) -> Lease {
        if self.frame.as_ref() != Some(frame) {
            (self.frame, self.heading, self.last) = (Some(frame.clone()), Heading::default(), None);
        }
        let every = Rc::new(every);
        self.leases.push(Rc::downgrade(&every));
        if !self.asking {
            self.ask(cx);
        }
        Lease { _every: every }
    }

    /// The bearing as of now: it goes stale while no map asks.
    pub fn last(&self) -> Option<Fix> {
        let position = self.last.filter(|_| self.failures < GIVE_UP)?;
        Some(Fix { position, bearing: self.heading.bearing(Instant::now()) })
    }

    /// The shortest wait a held lease wants.
    fn every(&mut self) -> Option<Duration> {
        self.leases.retain(|lease| lease.strong_count() > 0);
        self.leases.iter().filter_map(Weak::upgrade).map(|every| *every).min()
    }

    fn ask(&mut self, cx: &mut Context<Self>) {
        self.asking = true;
        let session = self.session.clone();
        self._asks = cx.spawn(async move |tracker, cx| {
            while let Ok(Some(every)) = tracker.update(cx, |tracker, _| tracker.every()) {
                let answer = session.position().await;
                let Ok(wait) = tracker.update(cx, |tracker, cx| tracker.answered(answer, every, cx)) else { return };
                cx.background_executor().timer(wait).await;
            }
            tracker.update(cx, |tracker, _| tracker.asking = false).ok();
        });
    }

    /// How long to wait before asking again.
    fn answered(&mut self, answer: Result<[f64; 3]>, every: Duration, cx: &mut Context<Self>) -> Duration {
        match answer {
            Ok(position) => {
                self.failures = 0;
                let at = Instant::now();
                if let Some(frame) = &self.frame {
                    self.heading.track(frame.to_map(position), at);
                }
                self.last = Some(position);
                cx.emit(Tracked::Fix(Fix { position, bearing: self.heading.bearing(at) }));
                every
            }
            Err(e) => {
                log::debug!("player position: {e:#}");
                self.failures += 1;
                if self.failures == GIVE_UP {
                    log::warn!("the trainer does not tell where the player is");
                    self.heading = Heading::default();
                    cx.emit(Tracked::Lost);
                }
                (every * 2u32.saturating_pow(self.failures)).min(SLOWEST)
            }
        }
    }
}
