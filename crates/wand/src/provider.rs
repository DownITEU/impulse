//! `model::Provider` for Wand: the user's WeMod account handed off from Wand, its catalog and trainers.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
};

use anyhow::anyhow;
use blocking::unblock;
use futures::{
    FutureExt, StreamExt,
    channel::mpsc::{self, UnboundedSender},
    future::{self, BoxFuture, Shared},
    stream::{self, BoxStream},
};
use model::{Account, Folder, Game, Installation, SignIn, Snapshot};

use crate::{
    binary, catalog,
    folder::Setting,
    run::Host,
    session::{self, Session, Source, account},
    trainer::{self, Stamp, Trainer},
    transport::Transport,
};

type Link = Result<(Arc<Session>, Transport), String>;

pub struct Wand {
    data_dir: PathBuf,
    transport: Result<Transport, String>,
    // Shared by every caller; a failed connection is tried again by the next `sign_in`.
    session: Mutex<Shared<BoxFuture<'static, Link>>>,
    folder: Arc<Setting>,
    host: Host,
}

impl Wand {
    /// Starts the browser bridge at once: it takes a moment to come up.
    pub fn new(data_dir: PathBuf) -> Self {
        let transport = Transport::spawn(data_dir.clone()).map_err(|e| {
            log::error!("Transport: {e:#}");
            format!("{e:#}")
        });
        let folder = Arc::new(Setting::load(&data_dir));
        Self {
            session: Mutex::new(connect(&transport, &data_dir)),
            host: Host::new(data_dir.clone(), folder.clone()),
            transport,
            folder,
            data_dir,
        }
    }

    fn session(&self) -> Shared<BoxFuture<'static, Link>> {
        self.session.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

impl model::Provider for Wand {
    fn sign_in(&self) -> BoxStream<'static, SignIn> {
        let session = {
            let mut session = self.session.lock().unwrap_or_else(PoisonError::into_inner);
            if matches!(session.peek(), Some(Err(_))) {
                *session = connect(&self.transport, &self.data_dir);
            }
            session.clone()
        };
        let dir = self.data_dir.clone();
        channel(|tx| async move {
            let cached = {
                let dir = dir.clone();
                unblock(move || account::cached(&dir)).await
            };
            let known = cached.as_ref().is_some_and(|(_, fresh)| *fresh);
            if let Some((account, _)) = cached {
                send(&tx, SignIn::Account(Account { name: account.username }));
            }
            let (session, transport) = match session.await {
                Ok(link) => link,
                Err(_) => return send(&tx, SignIn::Failed),
            };
            if known {
                return;
            }
            match account::fetch(&session, &transport, &dir).await {
                Ok(account) => send(&tx, SignIn::Account(Account { name: account.username })),
                Err(e) => log::warn!("account: {e:#}"),
            }
        })
    }

    fn games(&self) -> BoxStream<'static, Snapshot<Vec<Game>>> {
        let dir = self.data_dir.clone();
        channel(|tx| async move {
            let cached = {
                let dir = dir.clone();
                unblock(move || catalog::cached(&dir)).await
            };
            let have_copy = cached.is_some();
            send(&tx, Snapshot::Cached(cached));
            match unblock(move || catalog::refresh(&dir, have_copy)).await {
                Ok(Some(games)) => {
                    log::info!("catalog updated");
                    send(&tx, Snapshot::Fresh(Ok(games)));
                }
                Ok(None) => log::debug!("catalog is up to date"),
                Err(e) => {
                    log::warn!("catalog not refreshed: {e:#}");
                    send(&tx, Snapshot::Fresh(Err(e)));
                }
            }
        })
    }

    /// The API is asked only when the disk copy is missing or was fetched for another trainer or build.
    fn trainer(&self, game: &Game, install: &Installation) -> BoxStream<'static, Snapshot<model::Trainer>> {
        let stamp = Stamp {
            updated_at: game.trainer.as_ref().and_then(|offer| offer.revision.clone()),
            version: install.build,
        };
        let (dir, id, session, host) = (self.data_dir.clone(), game.id.clone(), self.session(), self.host.clone());
        channel(|tx| async move {
            let copy = {
                let (dir, id) = (dir.clone(), id.clone());
                unblock(move || trainer::cached(&dir, &id)).await
            };
            let fresh = copy.as_ref().is_some_and(|(_, found)| *found == stamp);
            send(&tx, Snapshot::Cached(copy.map(|(trainer, _)| snapshot(trainer, &host))));
            if fresh {
                return;
            }
            let fetched = async {
                let (session, transport) = session.await.map_err(|e| anyhow!(e))?;
                trainer::fetch(&session, &transport, &dir, &id, stamp).await
            }
            .await;
            if let Err(e) = &fetched {
                log::warn!("trainer {id}: {e:#}");
            }
            send(&tx, Snapshot::Fresh(fetched.map(|trainer| snapshot(trainer, &host))));
        })
    }

    fn folders(&self) -> BoxFuture<'static, Vec<Folder>> {
        let folder = self.folder.clone();
        unblock(move || vec![folder.folder()]).boxed()
    }

    // Wand has one folder.
    fn set_folder(&self, _id: &'static str, path: PathBuf) -> BoxFuture<'static, Folder> {
        let (folder, call) = (self.folder.clone(), self.folder.call());
        unblock(move || folder.set(call, path)).boxed()
    }

    fn forget_caches(&self) {
        let dir = &self.data_dir;
        catalog::forget(dir);
        trainer::forget(dir);
        binary::forget(dir);
        account::forget(dir);
    }
}

/// The cheat list with a runner for exactly its plan.
fn snapshot(trainer: Trainer, host: &Host) -> model::Trainer {
    let plan = trainer.plan();
    let runner = host.runner(&plan);
    trainer.into_model(model::Plan { image: plan.image, minimum_age: plan.minimum_age, launch: plan.launch, runner })
}

fn connect(transport: &Result<Transport, String>, data_dir: &Path) -> Shared<BoxFuture<'static, Link>> {
    let (transport, vault) = (transport.clone(), data_dir.join("session.bin"));
    async move {
        let transport = transport?;
        let (session, source) = session::connect(&transport, vault).await.map_err(|e| {
            log::error!("Session: {e:#}");
            format!("{e:#}")
        })?;
        let how = match source {
            Source::Restored => "restored",
            Source::Refreshed => "refreshed",
            Source::Wand => "new from Wand",
        };
        let days = (session.expires_in().as_secs() + 43_200) / 86_400;
        log::info!("Session: {how}, expires in {days} d");
        Ok((Arc::new(session), transport))
    }
    .boxed()
    .shared()
}

/// A stream of what `body` sends; polling the stream runs the body.
fn channel<T, F>(body: impl FnOnce(UnboundedSender<T>) -> F) -> BoxStream<'static, T>
where
    T: Send + 'static,
    F: Future<Output = ()> + Send + 'static,
{
    let (tx, rx) = mpsc::unbounded();
    stream::select(rx.map(Some), stream::once(body(tx)).map(|()| None)).filter_map(future::ready).boxed()
}

// The receiver is gone only when nobody listens any more.
fn send<T>(tx: &UnboundedSender<T>, item: T) {
    let _ = tx.unbounded_send(item);
}
