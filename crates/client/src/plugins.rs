//! The plugins' life in the registry: added at start and on install or enable, replaced when a setting changes,
//! removed when disabled or removed. `app` gives the `Host`; every call blocks.

use std::{path::Path, sync::Arc};

use model::{Approval, Failure, Installed, SourceId, Value};

use crate::{Capabilities, Registry};

/// The installed plugins' files, the lock and the values; `app` makes it over `plugins`.
pub trait Host: Send + Sync {
    fn list(&self) -> Vec<Installed>;
    /// What installing the folder or `.zip` at `pick` would grant.
    fn inspect(&self, pick: &Path) -> Result<Approval, Failure>;
    /// Installs and enables what `approval` showed, unless the pick changed since.
    fn install(&self, pick: &Path, approval: &Approval) -> Result<(), Failure>;
    fn open(&self, id: &SourceId) -> Result<Capabilities, Failure>;
    fn set_enabled(&self, id: &SourceId, on: bool);
    /// Its files, lock entry, values and answers go; when the lock cannot be saved nothing goes and it fails.
    fn remove(&self, id: &SourceId) -> Result<(), Failure>;
    /// Saves a value that fits its setting.
    fn set_value(&self, id: &SourceId, setting: &str, value: Value) -> Result<(), Failure>;
}

pub struct Plugins {
    host: Arc<dyn Host>,
    registry: Arc<Registry>,
}

impl Plugins {
    pub fn new(host: Arc<dyn Host>, registry: Arc<Registry>) -> Self {
        Self { host, registry }
    }

    /// Adds every enabled plugin that opens; one that does not keeps its failure.
    pub fn start(&self) {
        for plugin in self.host.list().into_iter().filter(|plugin| plugin.enabled) {
            if let Err(e) = self.add(&plugin.id) {
                log::warn!("plugin {}: {e}", plugin.id.as_str());
            }
        }
    }

    pub fn list(&self) -> Vec<Installed> {
        self.host.list()
    }

    pub fn inspect(&self, pick: &Path) -> Result<Approval, Failure> {
        self.host.inspect(pick)
    }

    pub fn install(&self, pick: &Path, approval: &Approval) -> Result<(), Failure> {
        self.host.install(pick, approval)?;
        self.add(&approval.id)
    }

    pub fn set_enabled(&self, id: &SourceId, on: bool) -> Result<(), Failure> {
        self.host.set_enabled(id, on);
        if on {
            self.add(id)
        } else {
            self.registry.remove(id);
            Ok(())
        }
    }

    /// Its calls end before its files go; if the host refuses, the plugin is added back.
    pub fn remove(&self, id: &SourceId) -> Result<(), Failure> {
        self.registry.remove(id);
        let refused = self.host.remove(id).err();
        let Some(failure) = refused else { return Ok(()) };
        // A disabled plugin does not open: it was not in the registry.
        let _ = self.add(id);
        Err(failure)
    }

    /// A running plugin opens again with the value: a replace, so its copies ask again.
    pub fn set_value(&self, id: &SourceId, setting: &str, value: Value) -> Result<(), Failure> {
        self.host.set_value(id, setting, value)?;
        if self.registry.has(id) { self.add(id) } else { Ok(()) }
    }

    fn add(&self, id: &SourceId) -> Result<(), Failure> {
        self.registry.add(self.host.open(id)?);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use model::{FailureKind, Maps, Source};

    use super::*;

    struct Source1(SourceId);

    impl Source for Source1 {
        fn id(&self) -> SourceId {
            self.0.clone()
        }
    }

    impl Maps for Source1 {
        fn map(&self, _: &model::MapOffer, _: &model::Locale) -> futures::stream::BoxStream<'static, model::Snapshot<model::Map>> {
            Box::pin(futures::stream::empty())
        }
    }

    /// Plugins by id, enabled or not; `broken` ones do not open.
    #[derive(Default)]
    struct Fake {
        plugins: Mutex<Vec<(SourceId, bool)>>,
        broken: Vec<SourceId>,
        /// `remove` fails, as when the lock cannot be saved.
        stuck: bool,
    }

    impl Host for Fake {
        fn list(&self) -> Vec<Installed> {
            let installed = |(id, enabled): &(SourceId, bool)| Installed {
                id: id.clone(),
                name: String::new(),
                version: String::new(),
                hosts: Vec::new(),
                links: Vec::new(),
                enabled: *enabled,
                settings: Vec::new(),
                values: Vec::new(),
                bytes: 0,
                failure: None,
            };
            self.plugins.lock().unwrap().iter().map(installed).collect()
        }
        fn inspect(&self, _: &Path) -> Result<Approval, Failure> {
            Err(Failure::new(FailureKind::Unsupported))
        }
        fn install(&self, _: &Path, approval: &Approval) -> Result<(), Failure> {
            self.plugins.lock().unwrap().push((approval.id.clone(), true));
            Ok(())
        }
        fn open(&self, id: &SourceId) -> Result<Capabilities, Failure> {
            if self.broken.contains(id) {
                return Err(Failure::new(FailureKind::Refused));
            }
            Ok(Capabilities { maps: Some(Arc::new(Source1(id.clone()))), ..Capabilities::default() })
        }
        fn set_enabled(&self, id: &SourceId, on: bool) {
            self.plugins.lock().unwrap().iter_mut().filter(|(known, _)| known == id).for_each(|(_, enabled)| *enabled = on);
        }
        fn remove(&self, id: &SourceId) -> Result<(), Failure> {
            if self.stuck {
                return Err(Failure::new(FailureKind::Internal));
            }
            self.plugins.lock().unwrap().retain(|(known, _)| known != id);
            Ok(())
        }
        fn set_value(&self, _: &SourceId, _: &str, _: Value) -> Result<(), Failure> {
            Ok(())
        }
    }

    fn id(name: &str) -> SourceId {
        SourceId::new(name).unwrap()
    }

    #[test]
    fn start_adds_the_enabled_plugins_that_open_and_disabling_or_removing_takes_them_out() {
        let fake = Fake {
            plugins: Mutex::new(vec![(id("plugin-a"), true), (id("plugin-b"), false), (id("plugin-c"), true)]),
            broken: vec![id("plugin-c")],
            ..Fake::default()
        };
        let registry = Arc::new(Registry::default());
        let plugins = Plugins::new(Arc::new(fake), registry.clone());
        plugins.start();
        let there = || ["plugin-a", "plugin-b", "plugin-c"].map(|name| registry.has(&id(name)));
        assert_eq!(there(), [true, false, false]);
        plugins.set_enabled(&id("plugin-b"), true).unwrap();
        assert_eq!(there(), [true, true, false]);
        assert!(plugins.set_enabled(&id("plugin-c"), true).is_err(), "a broken plugin stays out");
        plugins.set_enabled(&id("plugin-a"), false).unwrap();
        plugins.remove(&id("plugin-b")).unwrap();
        assert_eq!(there(), [false, false, false]);
        assert!(plugins.list().iter().all(|plugin| plugin.id != id("plugin-b")));
    }

    #[test]
    fn a_remove_the_host_refuses_puts_the_plugin_back() {
        let fake = Fake { plugins: Mutex::new(vec![(id("plugin-a"), true)]), stuck: true, ..Fake::default() };
        let registry = Arc::new(Registry::default());
        let plugins = Plugins::new(Arc::new(fake), registry.clone());
        plugins.start();
        assert_eq!(plugins.remove(&id("plugin-a")).err().map(|e| e.kind), Some(FailureKind::Internal));
        assert!(registry.has(&id("plugin-a")));
        assert_eq!(plugins.list().len(), 1);
    }

    #[test]
    fn a_value_replaces_a_running_plugin_and_leaves_a_stopped_one_out() {
        let fake = Fake { plugins: Mutex::new(vec![(id("plugin-a"), true), (id("plugin-b"), false)]), ..Fake::default() };
        let registry = Arc::new(Registry::default());
        let plugins = Plugins::new(Arc::new(fake), registry.clone());
        plugins.start();
        let changes = registry.changes();
        plugins.set_value(&id("plugin-a"), "level", Value::Number(2.)).unwrap();
        plugins.set_value(&id("plugin-b"), "level", Value::Number(2.)).unwrap();
        drop(registry.changes());
        let told: Vec<_> = futures::executor::block_on_stream(changes).take(2).collect();
        assert_eq!(told, [crate::Change::Removed(id("plugin-a")), crate::Change::Added(id("plugin-a"))]);
        assert!(!registry.has(&id("plugin-b")));
    }
}
