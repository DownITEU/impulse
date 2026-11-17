//! `client`'s view of the plugins' store: opening one gives the registry every capability the plugin has.

use std::path::Path;

use client::Capabilities;
use model::{Approval, Failure, Installed, SourceId, Value};

pub struct Host(pub plugins::Store);

impl client::plugins::Host for Host {
    fn list(&self) -> Vec<Installed> {
        self.0.list()
    }

    fn inspect(&self, pick: &Path) -> Result<Approval, Failure> {
        self.0.inspect(pick)
    }

    fn install(&self, pick: &Path, approval: &Approval) -> Result<(), Failure> {
        self.0.install(pick, approval)
    }

    fn open(&self, id: &SourceId) -> Result<Capabilities, Failure> {
        let plugin = self.0.open(id)?;
        Ok(Capabilities {
            maps: Some(plugin.clone()),
            titles: Some(plugin.clone()),
            downloads: Some(plugin.clone()),
            caches: Some(plugin),
            ..Capabilities::default()
        })
    }

    fn set_enabled(&self, id: &SourceId, on: bool) {
        self.0.set_enabled(id, on);
    }

    fn remove(&self, id: &SourceId) -> Result<(), Failure> {
        self.0.remove(id)
    }

    fn set_value(&self, id: &SourceId, setting: &str, value: Value) -> Result<(), Failure> {
        self.0.set_value(id, setting, value)
    }
}
