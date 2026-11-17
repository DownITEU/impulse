//! The installed plugins: their files, the lock of what the user approved, their settings and their answers (P7, P8).
//! Ids at the surface are source ids (`plugin-<manifest id>`); the lock is keyed by their text.

mod install;
mod lock;
#[cfg(test)]
mod tests;
mod view;

use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, PoisonError, Weak},
};

use disk::Stored;
use model::{Approval, Failure, FailureKind, Installed, SourceId, Value};
use sdk::manifest::Manifest;

use self::lock::{Entry, Lock};
use crate::{Bundle, Plugin, bundle, cache, cache::Retired, manifest, runtime, scope::Scope, settings};

/// The installed plugins. Every call blocks; the lock file has no other writer.
pub struct Store {
    dir: PathBuf,
    caches: PathBuf,
    state: Mutex<State>,
}

struct State {
    lock: Stored<Lock>,
    /// The plugins `open` made, for their last failure.
    running: HashMap<String, Weak<Plugin>>,
    /// One flag per source id, shared by every cache opened for it: `remove` stops them all, in-flight calls included.
    retired: HashMap<String, Arc<Retired>>,
    /// Why the last `open` failed.
    failed: HashMap<String, Failure>,
}

impl State {
    fn failure(&self, key: &str) -> Option<Failure> {
        let running = self.running.get(key).and_then(Weak::upgrade).and_then(|plugin| plugin.failure());
        running.or_else(|| self.failed.get(key).cloned())
    }
}

fn fail(kind: FailureKind, detail: impl Into<String>) -> Failure {
    Failure { detail: detail.into(), ..Failure::new(kind) }
}

impl Store {
    /// `dir` holds the installed plugins, `lock` is `plugins.lock`, `caches` holds their answers.
    pub fn new(dir: PathBuf, lock: PathBuf, caches: PathBuf) -> Self {
        let state = State { lock: Stored::load(lock), running: HashMap::new(), retired: HashMap::new(), failed: HashMap::new() };
        Self { dir, caches, state: Mutex::new(state) }
    }

    /// Every installed plugin, in id order.
    pub fn list(&self) -> Vec<Installed> {
        let entries: Vec<_> = {
            let state = self.state();
            state.lock.plugins.iter().map(|(key, entry)| (key.clone(), entry.clone(), state.failure(key))).collect()
        };
        let installed = |(key, entry, failure): (String, Entry, Option<Failure>)| {
            let source = SourceId::new(&key)?;
            let bytes = cache::size(&cache::folder(&self.caches, &source));
            Some(match self.manifest(&entry) {
                Ok((manifest, _)) => view::installed(&source, &entry, &manifest, bytes, failure),
                Err(e) => view::unreadable(&source, &entry, bytes, Failure::from(e)),
            })
        };
        entries.into_iter().filter_map(installed).collect()
    }

    /// What installing the folder or `.zip` at `pick` would grant. A module that does not compile is `Refused`: it
    /// would replace a working install with one that never opens.
    pub fn inspect(&self, pick: &Path) -> Result<Approval, Failure> {
        let bundle = Bundle::read(pick)?;
        runtime::check(&bundle.module)?;
        let manifest = &bundle.manifest;
        let id = bundle.scope.source.clone();
        let old = self.state().lock.plugins.get(id.as_str()).cloned();
        let mut widened = Vec::new();
        if let Some(old) = &old {
            let new = manifest.hosts.iter().filter(|host| !old.hosts.contains(host));
            for host in new.chain(manifest.links.iter().filter(|link| !old.links.contains(link))) {
                if !widened.contains(host) {
                    widened.push(host.clone());
                }
            }
        }
        Ok(Approval {
            id,
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            hosts: manifest.hosts.clone(),
            links: manifest.links.clone(),
            replaces: old.map(|old| old.version),
            widened,
            digest: bundle.digest(),
        })
    }

    /// Installs and enables what `approval` showed. A pick that changed since is `Refused`. The previous version's
    /// folder goes only after the lock is saved with the new one: a crash or a failed save leaves one of the two usable.
    pub fn install(&self, pick: &Path, approval: &Approval) -> Result<(), Failure> {
        let bundle = Bundle::read(pick)?;
        let source = bundle.scope.source.clone();
        let digest = bundle.digest();
        if digest != approval.digest || source != approval.id {
            return Err(fail(FailureKind::Refused, "the pick changed since it was shown"));
        }
        let name = format!("{}.{}", source.as_str(), &digest[..12]);
        install::write(&self.dir, &name, &bundle).map_err(Failure::from)?;
        let key = source.as_str();
        let mut state = self.state();
        let saved = state.lock.try_edit(|lock| {
            let values = lock.plugins.get(key).map(|old| old.values.clone()).unwrap_or_default();
            let manifest = &bundle.manifest;
            let entry = Entry {
                dir: name.clone(),
                version: manifest.version.clone(),
                enabled: true,
                hosts: manifest.hosts.clone(),
                links: manifest.links.clone(),
                files: bundle.hashes().clone(),
                values,
            };
            lock.plugins.insert(key.to_owned(), entry)
        });
        let old = match saved {
            Ok(old) => old,
            Err(e) => {
                // The lock still names the old folder: only a folder it does not name is the new one's alone.
                if state.lock.plugins.get(key).is_none_or(|old| old.dir != name) {
                    self.remove_folder(&name);
                }
                return Err(Failure::from(e));
            }
        };
        state.failed.remove(key);
        if let Some(old) = old.filter(|old| old.dir != name) {
            self.remove_folder(&old.dir);
        }
        Ok(())
    }

    /// The enabled plugin, opened with the user's values. Files that differ from what was approved are `Refused`.
    pub fn open(&self, id: &SourceId) -> Result<Arc<Plugin>, Failure> {
        let key = id.as_str();
        let (entry, retired) = {
            let mut state = self.state();
            let entry = state.lock.plugins.get(key).cloned();
            let entry = entry.ok_or_else(|| fail(FailureKind::NotFound, format!("{key} is not installed")))?;
            if !entry.enabled {
                return Err(fail(FailureKind::Refused, format!("{key} is not enabled")));
            }
            (entry, state.retired.entry(key.to_owned()).or_default().clone())
        };
        let opened = self.open_entry(id, &entry, retired);
        let mut state = self.state();
        match &opened {
            Ok(plugin) => {
                state.running.insert(key.to_owned(), Arc::downgrade(plugin));
                state.failed.remove(key);
            }
            Err(e) => {
                state.failed.insert(key.to_owned(), e.clone());
            }
        }
        opened
    }

    pub fn set_enabled(&self, id: &SourceId, on: bool) {
        let mut state = self.state();
        state.lock.edit(|lock| {
            if let Some(entry) = lock.plugins.get_mut(id.as_str()) {
                entry.enabled = on;
            }
        });
        if !on {
            state.failed.remove(id.as_str());
        }
    }

    /// The lock entry with its values is saved away first; if that fails nothing else goes and the failure is returned.
    /// Then every copy of it, a call in flight's too, writes no more answers, and its folder and answers go. A later
    /// install starts a fresh flag.
    pub fn remove(&self, id: &SourceId) -> Result<(), Failure> {
        let key = id.as_str();
        let mut state = self.state();
        // The lock goes first: a folder left behind is harmless, a lock entry without its folder is not.
        let entry = state.lock.try_edit(|lock| lock.plugins.remove(key)).map_err(Failure::from)?;
        if let Some(retired) = state.retired.remove(key) {
            retired.retire();
        }
        state.running.remove(key);
        state.failed.remove(key);
        if let Some(entry) = entry {
            self.remove_folder(&entry.dir);
        }
        let answers = cache::folder(&self.caches, id);
        if let Err(e) = fs::remove_dir_all(&answers).or_else(|e| if e.kind() == io::ErrorKind::NotFound { Ok(()) } else { Err(e) }) {
            log::warn!("{}: answers not removed: {e}", key);
        }
        Ok(())
    }

    /// Saves `value` for the plugin's setting; an unknown setting or a value outside its field is `Refused`.
    pub fn set_value(&self, id: &SourceId, setting: &str, value: Value) -> Result<(), Failure> {
        let key = id.as_str();
        let entry = self.state().lock.plugins.get(key).cloned();
        let entry = entry.ok_or_else(|| fail(FailureKind::NotFound, format!("{key} is not installed")))?;
        let (manifest, _) = self.manifest(&entry).map_err(Failure::from)?;
        let field = manifest.settings.iter().find(|field| field.id == setting);
        let field = field.ok_or_else(|| fail(FailureKind::Refused, format!("{key} has no setting {setting:?}")))?;
        let json = view::to_json(&value).filter(|json| settings::fits(field, json));
        let json = json.ok_or_else(|| fail(FailureKind::Refused, format!("{key}: a value outside setting {setting:?}")))?;
        self.state().lock.edit(|lock| {
            if let Some(entry) = lock.plugins.get_mut(key) {
                entry.values.insert(setting.to_owned(), json);
            }
        });
        Ok(())
    }

    fn open_entry(&self, id: &SourceId, entry: &Entry, retired: Arc<Retired>) -> Result<Arc<Plugin>, Failure> {
        let changed = |why: &str| fail(FailureKind::Refused, format!("files changed since install: install it again ({why})"));
        let folder = self.folder(&entry.dir).map_err(Failure::from)?;
        let bundle = Bundle::read(&folder).map_err(|e| changed(&e.detail))?;
        if bundle.scope.source != *id || *bundle.hashes() != entry.files {
            return Err(changed("a file differs"));
        }
        Plugin::open_retirable(&bundle, &self.caches, &entry.values, retired).map(Arc::new)
    }

    /// The installed manifest, read alone and checked against the hash the lock pins.
    fn manifest(&self, entry: &Entry) -> anyhow::Result<(Manifest, Scope)> {
        let bytes = crate::read(&self.folder(&entry.dir)?.join(bundle::MANIFEST), manifest::MANIFEST)?;
        if entry.files.get(bundle::MANIFEST) != Some(&bundle::hash(&bytes)) {
            let changed = anyhow::anyhow!("files changed since install: install it again (manifest.json differs)");
            return Err(changed.context(Failure::new(FailureKind::Refused)));
        }
        manifest::parse(&bytes)
    }

    /// A folder of the plugins' folder; a name that is not one plain name is never joined to it.
    fn folder(&self, name: &str) -> anyhow::Result<PathBuf> {
        if name.is_empty() || name.starts_with('.') || name.contains(['/', '\\', ':']) {
            return Err(anyhow::anyhow!("the lock names a folder that is not a plain name").context(Failure::new(FailureKind::Refused)));
        }
        Ok(self.dir.join(name))
    }

    fn remove_folder(&self, name: &str) {
        let Ok(folder) = self.folder(name) else { return };
        if let Err(e) = fs::remove_dir_all(&folder) {
            log::warn!("{} not removed: {e}", folder.display());
        }
    }

    // A poisoned lock holds plain state that is still good.
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
