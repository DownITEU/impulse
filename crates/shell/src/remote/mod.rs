//! The remote panel's bridge: the server's life, the paired devices and the pairing code, the views it publishes from
//! the owners' state and the commands it applies through them.

mod command;
mod view;
mod views;

use std::{collections::HashMap, fs, io, iter, net::SocketAddr, path::PathBuf, sync::Arc, time::Instant};

use client::{
    api::{Ask, ServerSide, Views, coalesce, link},
    library::GameId,
    media::{self, Media},
    remote::{CODE_LIFE, Device, Devices, Pairing},
    storage::{Paths, Stored},
};
use futures::{FutureExt, StreamExt, future::BoxFuture};
use gpui_kit::{App, AppContext as _, Context, Entity, SharedString, Task};
use qrcodegen::{QrCode, QrCodeEcc};

use crate::{
    library::Library,
    run::{RunEvent, Runs},
    settings::{Prefs, Settings},
    trainer::{TrainerEvent, Trainers},
};
pub use view::RemoteView;
use views::{Dirty, Shown};

/// On, and the disclaimer accepted: a paired device must not start a trainer before it.
fn serves(prefs: &Prefs) -> bool {
    prefs.remote && prefs.disclaimer_seen
}

/// What `app` hands over to serve the panel: where it listens and the serving future, for the server's side of a
/// link. A build without the server has none.
pub type Serve = fn(ServerSide) -> io::Result<(SocketAddr, BoxFuture<'static, ()>)>;

pub struct Remote {
    serve: Serve,
    settings: Entity<Settings>,
    library: Entity<Library>,
    trainers: Entity<Trainers>,
    runs: Entity<Runs>,
    pictures: PathBuf,
    devices: Stored<Devices>,
    handles: Handles,
    state: State,
}

enum State {
    Off,
    Failed,
    On(Box<On>),
}

struct On {
    /// `http://<address>`: shown as it is, and with the pairing code in the QR code.
    link: SharedString,
    views: Views,
    pairing: Pairing,
    /// The pairing URL, renewed with its code.
    qr: Option<Arc<QrCode>>,
    shown: Shown,
    dirty: Dirty,
    _serving: Task<()>,
    _asks: Task<()>,
    _renew: Task<()>,
}

impl Remote {
    pub fn new(
        serve: Serve,
        settings: Entity<Settings>,
        (library, trainers, runs): (Entity<Library>, Entity<Trainers>, Entity<Runs>),
        paths: &Paths,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&settings, |remote, settings, cx| {
            match (serves(settings.read(cx).prefs()), matches!(remote.state, State::Off)) {
                (true, true) => remote.start(cx),
                (false, false) => remote.stop(cx),
                // The language words the views too.
                _ => remote.touch(Dirty::ALL, cx),
            }
        })
        .detach();
        cx.observe(&library, |remote, _, cx| remote.touch(Dirty::LIBRARY | Dirty::TRAINER, cx)).detach();
        cx.subscribe(&trainers, |remote, _, event: &TrainerEvent, cx| {
            let open = |game| remote.library.read(cx).open_id() == Some(game);
            let dirty = match event {
                TrainerEvent::Installed(game) | TrainerEvent::Changed(game) if open(game) => Dirty::LIBRARY | Dirty::TRAINER,
                TrainerEvent::Installed(_) | TrainerEvent::Changed(_) => Dirty::LIBRARY,
                TrainerEvent::Set { game, .. } if open(game) => Dirty::VALUES,
                _ => return,
            };
            remote.touch(dirty, cx);
        })
        .detach();
        // Not `observe`: the run clock notifies every second.
        cx.subscribe(&runs, |remote, _, event: &RunEvent, cx| {
            if !matches!(event, RunEvent::Pressed(_)) {
                remote.touch(Dirty::LIBRARY | Dirty::RUN, cx);
            }
        })
        .detach();
        let on = serves(settings.read(cx).prefs());
        let mut remote = Self {
            serve,
            settings,
            library,
            trainers,
            runs,
            pictures: Media::Pictures.dir(paths),
            devices: Stored::load(paths.remote_devices()),
            handles: Handles::default(),
            state: State::Off,
        };
        if on {
            remote.start(cx);
        }
        remote
    }

    pub fn failed(&self) -> bool {
        matches!(self.state, State::Failed)
    }

    /// The server's link and the pairing URL's QR code, while it listens.
    pub fn link(&self) -> Option<(SharedString, Option<Arc<QrCode>>)> {
        let State::On(on) = &self.state else { return None };
        Some((on.link.clone(), on.qr.clone()))
    }

    pub fn devices(&self) -> &[Device] {
        self.devices.list()
    }

    /// The device is signed out and its sockets close.
    pub fn revoke(&mut self, id: u32, cx: &mut Context<Self>) {
        self.devices.edit(|devices| devices.revoke(id));
        if let State::On(on) = &self.state {
            on.views.close(id);
        }
        cx.notify();
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        let (server, views, mut asks) = link();
        match (self.serve)(server) {
            Ok((address, serving)) => {
                let answers = cx.spawn(async move |remote, cx| {
                    while let Some(ask) = asks.next().await {
                        let mut burst = vec![ask];
                        burst.extend(iter::from_fn(|| asks.next().now_or_never().flatten()));
                        // One update each: the views a command changes go out before the next is checked.
                        for ask in coalesce(burst) {
                            if remote.update(cx, |remote, cx| remote.answer(ask, cx)).is_err() {
                                return;
                            }
                        }
                    }
                });
                log::info!("remote: serving on {address}");
                self.state = State::On(Box::new(On {
                    link: format!("http://{address}").into(),
                    views,
                    pairing: Pairing::default(),
                    qr: None,
                    shown: Shown::default(),
                    dirty: Dirty::default(),
                    _serving: cx.background_executor().spawn(serving),
                    _asks: answers,
                    _renew: Task::ready(()),
                }));
                self.renew(cx);
                self.touch(Dirty::ALL, cx);
            }
            Err(e) => {
                log::warn!("remote: no server: {e}");
                self.state = State::Failed;
            }
        }
        cx.notify();
    }

    /// Dropping the server closes every connection.
    fn stop(&mut self, cx: &mut Context<Self>) {
        self.state = State::Off;
        cx.notify();
    }

    /// A new code once the last one is used or old.
    fn renew(&mut self, cx: &mut Context<Self>) {
        let State::On(on) = &mut self.state else { return };
        // In the fragment, the code never reaches a request line: the page reads it and clears it.
        let url = format!("{}/#{}", on.link, on.pairing.code(Instant::now()));
        on.qr = QrCode::encode_text(&url, QrCodeEcc::Medium).ok().map(Arc::new);
        on._renew = cx.spawn(async move |remote, cx| {
            cx.background_executor().timer(CODE_LIFE).await;
            remote.update(cx, |remote, cx| remote.renew(cx)).ok();
        });
        cx.notify();
    }

    fn answer(&mut self, ask: Ask, cx: &mut Context<Self>) {
        match ask {
            Ask::Pair { code, user_agent, reply } => {
                let State::On(on) = &mut self.state else { return };
                let token = on.pairing.redeem(&code, Instant::now()).then(|| self.devices.edit(|devices| devices.add(&user_agent)));
                log::info!("remote: pairing {}", if token.is_some() { "accepted" } else { "refused" });
                if token.is_some() {
                    self.renew(cx);
                }
                drop(reply.send(token));
            }
            Ask::Device { token, reply } => drop(reply.send(self.devices.find(&token))),
            Ask::Command { device, command } => self.command(device, command, cx),
            Ask::Art { game, reply } => {
                let file = self.art(game, cx);
                // From the cache only: the panel shows no art rather than download it.
                let read = blocking::unblock(move || file.and_then(|file| fs::read(file).ok()).filter(|bytes| !bytes.is_empty()));
                cx.background_spawn(async move { drop(reply.send(read.await)) }).detach();
            }
        }
    }

    /// The cached file of the game's thumbnail.
    fn art(&self, handle: u32, cx: &App) -> Option<PathBuf> {
        let game = self.library.read(cx).game(self.handles.game(handle)?)?;
        game.thumbnail.as_deref().map(|url| media::file(&self.pictures, url))
    }
}

/// A game's handle is its place here: it stays while the app runs, and no source id leaves the app.
#[derive(Default)]
struct Handles {
    games: Vec<GameId>,
    by_game: HashMap<GameId, u32>,
}

impl Handles {
    fn of(&mut self, game: &GameId) -> u32 {
        *self.by_game.entry(game.clone()).or_insert_with(|| {
            self.games.push(game.clone());
            self.games.len() as u32 - 1
        })
    }

    fn game(&self, handle: u32) -> Option<&GameId> {
        self.games.get(handle as usize)
    }
}
