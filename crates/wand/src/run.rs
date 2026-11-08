//! `model::Runner` for a WeMod trainer: its DLL from the CDN, Wand's service and plugin in the game.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};

use anyhow::{Context, Result, anyhow, bail};
use blocking::unblock;
use futures::{FutureExt, StreamExt, future::BoxFuture};
use model::{Attach, Live, Process};

use crate::{
    attach::{Game, attach},
    binary,
    folder::Setting,
    tophat::{Event, Outcome, Server, Session},
    trainer::{Binary, Plan},
};

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
        Arc::new(Runner { host: self.clone(), binary: plan.binary.clone(), variables: plan.variables.clone() })
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
    /// `Err` says why this trainer cannot run here.
    binary: Result<Binary, String>,
    variables: Vec<String>,
}

impl model::Runner for Runner {
    fn prepare(&self) -> BoxFuture<'_, Result<Box<dyn Attach>>> {
        async move {
            let Binary { url, hash } = self.binary.clone().map_err(|why| anyhow!("{why}"))?;
            let folder = self.host.folder.clone();
            let install = unblock(move || folder.install()).await;
            let trainerlib = install.context("Wand's folder is not set: pick it in Settings, APP")?.trainerlib();
            let server = self.host.server()?;
            let data_dir = self.host.data_dir.clone();
            let trainer = {
                let hash = hash.clone();
                unblock(move || binary::ensure(&data_dir, &url, &hash)).await?
            };
            let prepared = Prepared { server, trainerlib, trainer, hash, variables: self.variables.clone() };
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
}

impl Attach for Prepared {
    fn attach(self: Box<Self>, process: Process) -> BoxFuture<'static, Result<Live>> {
        async move {
            let game = Game { process: &process, trainer: &self.trainer, trainer_hash: &self.hash, variables: &self.variables };
            let (session, events) = attach(&self.server, &self.trainerlib, game).await?;
            let events = events.filter_map(|event| async move {
                match event {
                    Event::Log(text) => Some(model::Event::Log(text)),
                    Event::Status(text) => Some(model::Event::Status(text)),
                    _ => None,
                }
            });
            Ok(Live { session: Box::new(Running(session)), events: events.boxed() })
        }
        .boxed()
    }
}

struct Running(Session);

impl model::Session for Running {
    fn set<'a>(&'a self, key: &'a str, value: f64) -> BoxFuture<'a, Result<()>> {
        async move {
            match self.0.set_value(key, value).await? {
                Outcome::Failed => bail!("the trainer refused it"),
                Outcome::Set | Outcome::Changed => Ok(()),
            }
        }
        .boxed()
    }

    fn stop(&self) -> BoxFuture<'_, Result<()>> {
        self.0.stop().boxed()
    }
}
