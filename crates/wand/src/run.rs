//! `model::Runner` for a WeMod trainer: its DLL from the CDN, Wand's service and plugin in the game.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
};

use anyhow::{Context, Result, anyhow};
use blocking::unblock;
use futures::{FutureExt, StreamExt, TryFutureExt, future::BoxFuture};
use model::{Attach, Failure, FailureKind, Live, Origin, Process};

use crate::{
    attach::{Attached, Game, attach},
    binary,
    folder::Setting,
    install::PUBLISHERS,
    proxy::{self, Proxy},
    texts::{ACTIVATE_UNSUPPORTED, FOLDER_UNSET, PROXY_FAILED, PROXY_MISSING, PROXY_RESTART, text},
    tophat::{Event, Outcome, Server},
    trainer::{Activate, Binary, Plan},
};

type Answer<T> = std::result::Result<T, Failure>;

/// Runs WeMod trainers on one Tophat server, started with the first run and kept for the app's life.
#[derive(Clone)]
pub struct Host {
    data_dir: PathBuf,
    folder: Arc<Setting>,
    server: Arc<Mutex<Option<Arc<Server>>>>,
}

impl Host {
    pub fn new(data_dir: PathBuf, folder: Arc<Setting>) -> Self {
        Self { data_dir, folder, server: Arc::default() }
    }

    pub fn runner(&self, plan: &Plan) -> Arc<dyn model::Runner> {
        Arc::new(Runner {
            host: self.clone(),
            binary: plan.binary.clone(),
            variables: plan.variables.clone(),
            proxy: plan.proxy.clone(),
            activate: plan.activate,
        })
    }

    fn server(&self) -> Result<Arc<Server>> {
        let mut server = self.server.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(server) = &*server {
            return Ok(server.clone());
        }
        let started = Arc::new(Server::start()?);
        *server = Some(started.clone());
        Ok(started)
    }
}

struct Runner {
    host: Host,
    binary: Result<Binary, String>,
    variables: Vec<String>,
    proxy: Result<Option<Proxy>, String>,
    activate: Activate,
}

impl model::Runner for Runner {
    fn prepare(&self) -> BoxFuture<'_, Answer<Box<dyn Attach>>> {
        async move {
            let unsupported = |why: &String| anyhow!("{why}").context(Failure::new(FailureKind::Unsupported));
            let Binary { url, hash } = self.binary.clone().map_err(|why| unsupported(&why))?;
            let proxy = self.proxy.clone().map_err(|why| unsupported(&why))?;
            let folder = self.host.folder.clone();
            let install = unblock(move || folder.install()).await;
            let unset = || Failure::new(FailureKind::SetupNeeded).saying(text(FOLDER_UNSET));
            let install = install.context("Wand's folder is not set").context(unset())?;
            let proxy = proxy.map(|proxy| (proxy, install.game_launcher()));
            let server = self.host.server()?;
            let data_dir = self.host.data_dir.clone();
            let trainer = {
                let hash = hash.clone();
                unblock(move || binary::ensure(&data_dir, &crate::install::trainer_caches(), &url, &hash)).await?
            };
            let trainerlib = install.trainerlib();
            let variables = self.variables.clone();
            let prepared = Prepared { server, trainerlib, trainer, hash, variables, proxy, activate: self.activate };
            Ok(Box::new(prepared) as Box<dyn Attach>)
        }
        .boxed()
    }
}

struct Prepared {
    server: Arc<Server>,
    trainerlib: PathBuf,
    trainer: PathBuf,
    hash: String,
    variables: Vec<String>,
    /// With the stand-in for the game's launcher.
    proxy: Option<(Proxy, PathBuf)>,
    activate: Activate,
}

impl Attach for Prepared {
    /// The stand-in for a store's start only: a picked executable starts as it is.
    fn before_start(&self, origin: &Origin, install: &Path) -> BoxFuture<'static, Answer<Option<Box<dyn Send>>>> {
        let Some((proxy, stand_in)) = self.proxy.clone().filter(|_| matches!(origin, Origin::Launcher(_))) else {
            return async { Ok(None) }.boxed();
        };
        let install = install.to_owned();
        async move {
            let mounted = unblock(move || {
                if !stand_in.is_file() {
                    let missing = Failure::new(FailureKind::SetupNeeded).saying(text(PROXY_MISSING));
                    return Err(Failure { detail: format!("{} is missing", stand_in.display()), ..missing });
                }
                let mounted = proxy::mount(&stand_in, &install, &proxy, |copy| agent::signed(copy, PUBLISHERS));
                // Guarded already here: a stop before this future is polled again drops it.
                mounted.map(|mounted| OffThread(Some(mounted))).map_err(|e| Failure::from(e).saying(text(PROXY_FAILED)))
            })
            .await?;
            Ok(Some(Box::new(mounted) as Box<dyn Send>))
        }
        .boxed()
    }

    fn attach(self: Box<Self>, process: Process, build: Option<u32>) -> BoxFuture<'static, Answer<Live>> {
        async move {
            let (trainer, trainer_hash, variables) = (&self.trainer, &self.hash, &self.variables);
            let activate = self.activate;
            let game = Game { process: &process, version: build.unwrap_or(0), trainer, trainer_hash, variables, activate };
            let (attached, events) = attach(&self.server, &self.trainerlib, game).await.map_err(|e| {
                let failure = Failure::from(e);
                match failure.kind {
                    // Started elsewhere, the game runs the anti-cheat our start avoids.
                    FailureKind::Protected if self.proxy.is_some() => failure.saying(text(PROXY_RESTART)),
                    // The classic loader takes this trainer, which may be why it failed.
                    FailureKind::Internal | FailureKind::LoadFailed | FailureKind::TrainerFailed if activate.classic() => {
                        let detail = format!("{} (activate.flags {})", failure.detail, activate.0);
                        Failure { detail, ..failure.saying(text(ACTIVATE_UNSUPPORTED)) }
                    }
                    _ => failure,
                }
            })?;
            let events = events.filter_map(|event| async move {
                match event {
                    // The trainer logs every position asked for, a line per variable.
                    Event::Log(text) if text.contains("wm_coord_") => None,
                    Event::Log(text) => Some(model::Event::Log(text)),
                    Event::Status(text) => Some(model::Event::Status(text)),
                    _ => None,
                }
            });
            Ok(Live { session: Box::new(attached), events: events.boxed() })
        }
        .boxed()
    }
}

/// Puts the launcher back off the dropping thread, which may be the UI's.
struct OffThread(Option<proxy::Mounted>);

impl Drop for OffThread {
    fn drop(&mut self) {
        if let Some(mounted) = self.0.take() {
            unblock(move || drop(mounted)).detach();
        }
    }
}

impl model::Session for Attached {
    fn set<'a>(&'a self, key: &'a str, value: f64) -> BoxFuture<'a, Answer<()>> {
        async move {
            match self.session.set_value(key, value).await? {
                Outcome::Failed => Err(Failure { detail: "the trainer refused it".into(), ..Failure::new(FailureKind::Refused) }),
                Outcome::Set | Outcome::Changed => Ok(()),
            }
        }
        .boxed()
    }

    fn stop(&self) -> BoxFuture<'_, Answer<()>> {
        self.session.stop().map_err(Failure::from).boxed()
    }

    fn position(&self) -> BoxFuture<'_, Answer<[f64; 3]>> {
        self.session.position().map_err(Failure::from).boxed()
    }

    fn teleport(&self, to: [f64; 3]) -> BoxFuture<'_, Answer<()>> {
        self.session.teleport(to).map_err(Failure::from).boxed()
    }
}
