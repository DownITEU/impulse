//! The store over the real test guest: what install writes, what open trusts, what remove leaves.

use std::{fs, path::PathBuf};

use futures::{StreamExt, executor::block_on};
use model::{Caches, FailureKind, Locale, MapOffer, Maps, Ref, Snapshot, Source, SourceId, Text, Value};
use serde_json::json;

use super::*;
use crate::tests::plugin;

struct Fixture {
    root: PathBuf,
    store: Store,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("impulse-store-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let store = Store::new(root.join("plugins"), root.join("plugins.lock"), root.join("cache"));
        Self { root, store }
    }

    /// A copy of the test guest in `pick/<name>`, its manifest changed by `change`.
    fn pick(&self, name: &str, change: impl FnOnce(&mut serde_json::Value)) -> PathBuf {
        let dir = self.root.join("picks").join(name);
        for file in ["impulse_test_guest.wasm", "locales/en.ftl"] {
            fs::create_dir_all(dir.join(file).parent().unwrap()).unwrap();
            fs::copy(plugin().join(file), dir.join(file)).unwrap();
        }
        let mut manifest: serde_json::Value = serde_json::from_slice(&fs::read(plugin().join("manifest.json")).unwrap()).unwrap();
        change(&mut manifest);
        fs::write(dir.join("manifest.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
        dir
    }

    fn install(&self, pick: &Path) {
        let approval = self.store.inspect(pick).unwrap();
        self.store.install(pick, &approval).unwrap();
    }

    fn folders(&self) -> Vec<String> {
        let mut names: Vec<_> =
            fs::read_dir(self.root.join("plugins")).into_iter().flatten().flatten().map(|e| e.file_name().into_string().unwrap()).collect();
        names.sort();
        names
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn id() -> SourceId {
    SourceId::new("plugin-test-guest").unwrap()
}

fn answer(plugin: &Plugin) {
    let offer = MapOffer { id: Ref::new(&plugin.id(), "city"), name: String::new(), thumbnail: None, revision: Some("r1".into()) };
    let both = block_on(plugin.map(&offer, &Locale::new("en")).collect::<Vec<_>>());
    // The guest's call needs no network for this map; the answer is kept.
    assert!(matches!(&both[..], [_, Snapshot::Fresh(Ok(_))]));
}

#[test]
fn an_installed_plugin_opens_and_a_file_changed_after_install_makes_open_refuse() {
    let fixture = Fixture::new("open");
    fixture.install(&fixture.pick("a", |_| {}));
    let plugin = fixture.store.open(&id()).unwrap();
    assert_eq!(plugin.id(), id());
    let installed = &fixture.store.list()[0];
    assert_eq!((installed.name.as_str(), installed.version.as_str(), installed.enabled), ("Test guest", "1.0.0", true));
    assert_eq!(installed.hosts, ["*.tiles.example.com", "tiles.example.com"]);
    let folder = fixture.root.join("plugins").join(&fixture.store.state().lock.plugins["plugin-test-guest"].dir);
    for (what, change) in [("a changed file", 0), ("a file added", 1), ("a file gone", 2)] {
        let file = folder.join("locales/en.ftl");
        let extra = folder.join("locales/fr.ftl");
        let original = fs::read(&file).unwrap();
        match change {
            0 => fs::write(&file, "level-label = Other").unwrap(),
            1 => fs::write(&extra, "a = A").unwrap(),
            _ => fs::remove_file(&file).unwrap(),
        }
        let failure = fixture.store.open(&id()).err().unwrap_or_else(|| panic!("{what} was opened"));
        assert_eq!(failure.kind, FailureKind::Refused, "{what}");
        assert!(failure.detail.contains("files changed since install: install it again"), "{what}: {}", failure.detail);
        assert!(fixture.store.list()[0].failure.is_some(), "{what}: the open failure is shown");
        let _ = fs::remove_file(&extra);
        fs::write(&file, original).unwrap();
    }
    assert!(fixture.store.open(&id()).is_ok() && fixture.store.list()[0].failure.is_none());
    fixture.store.set_enabled(&id(), false);
    assert_eq!(fixture.store.open(&id()).err().map(|e| e.kind), Some(FailureKind::Refused));
    assert_eq!(fixture.store.open(&SourceId::new("plugin-nobody").unwrap()).err().map(|e| e.kind), Some(FailureKind::NotFound));
}

#[test]
fn a_pick_changed_between_inspect_and_install_is_refused() {
    let fixture = Fixture::new("changed");
    let pick = fixture.pick("a", |_| {});
    let approval = fixture.store.inspect(&pick).unwrap();
    fs::write(pick.join("locales/en.ftl"), "level-label = Changed").unwrap();
    assert_eq!(fixture.store.install(&pick, &approval).err().map(|e| e.kind), Some(FailureKind::Refused));
    assert!(fixture.store.list().is_empty() && fixture.folders().is_empty());
}

#[test]
fn an_update_keeps_the_values_removes_the_old_folder_and_reports_what_it_widens() {
    let fixture = Fixture::new("update");
    fixture.install(&fixture.pick("v1", |_| {}));
    fixture.store.set_value(&id(), "level", Value::Number(7.)).unwrap();
    let before = fixture.folders();
    let v2 = fixture.pick("v2", |manifest| {
        manifest["version"] = json!("2.0.0");
        manifest["hosts"] = json!(["tiles.example.com", "new.example.com"]);
        manifest["links"] = json!(["wiki.example.com", "docs.example.com"]);
    });
    let approval = fixture.store.inspect(&v2).unwrap();
    assert_eq!(approval.replaces.as_deref(), Some("1.0.0"));
    assert_eq!(approval.widened, ["new.example.com", "docs.example.com"]);
    fixture.store.install(&v2, &approval).unwrap();
    let after = fixture.folders();
    assert_eq!((before.len(), after.len()), (1, 1));
    assert_ne!(before, after, "the old folder went");
    let installed = &fixture.store.list()[0];
    assert_eq!((installed.version.as_str(), installed.values[0].clone()), ("2.0.0", Value::Number(7.)));
    assert_eq!(installed.hosts, ["tiles.example.com", "new.example.com"]);
    assert!(fixture.store.open(&id()).is_ok());
    let fresh = Fixture::new("fresh");
    let approval = fresh.store.inspect(&v2).unwrap();
    assert_eq!((approval.replaces, approval.widened), (None, Vec::<String>::new()));
}

#[test]
fn installing_the_same_bytes_again_works() {
    let fixture = Fixture::new("again");
    let pick = fixture.pick("a", |_| {});
    fixture.install(&pick);
    let folders = fixture.folders();
    fixture.install(&pick);
    assert_eq!(fixture.folders(), folders);
    assert!(fixture.store.open(&id()).is_ok());
    assert_eq!(fixture.store.list().len(), 1);
}

#[test]
fn installing_the_same_pick_again_repairs_a_damaged_folder() {
    let fixture = Fixture::new("repair");
    let pick = fixture.pick("a", |_| {});
    fixture.install(&pick);
    let folder = fixture.root.join("plugins").join(&fixture.store.state().lock.plugins["plugin-test-guest"].dir);
    fs::write(folder.join("locales/en.ftl"), "level-label = Damaged").unwrap();
    assert!(fixture.store.open(&id()).is_err());
    fixture.install(&pick);
    assert!(fixture.store.open(&id()).is_ok());
    assert_eq!(fixture.folders().len(), 1);
}

#[test]
fn a_module_that_does_not_compile_is_refused_before_anything_is_installed() {
    let fixture = Fixture::new("garbage");
    let pick = fixture.pick("a", |_| {});
    fs::write(pick.join("impulse_test_guest.wasm"), b"not a module").unwrap();
    assert_eq!(fixture.store.inspect(&pick).err().map(|e| e.kind), Some(FailureKind::Refused));
    assert!(fixture.store.list().is_empty() && fixture.folders().is_empty());
}

#[test]
fn a_module_without_the_abi_is_refused_before_anything_is_installed() {
    let fixture = Fixture::new("empty");
    let pick = fixture.pick("a", |_| {});
    fs::write(pick.join("impulse_test_guest.wasm"), wat::parse_str("(module)").unwrap()).unwrap();
    let failure = fixture.store.inspect(&pick).err().unwrap();
    assert_eq!(failure.kind, FailureKind::Refused);
    assert!(failure.detail.contains("does not export"), "{}", failure.detail);
    assert!(fixture.store.list().is_empty() && fixture.folders().is_empty());
}

#[test]
fn a_remove_whose_lock_cannot_be_saved_fails_and_deletes_nothing() {
    let fixture = Fixture::new("remove-unsaved");
    fixture.install(&fixture.pick("a", |_| {}));
    let plugin = fixture.store.open(&id()).unwrap();
    answer(&plugin);
    // A folder where the lock file goes: the save fails.
    fs::remove_file(fixture.root.join("plugins.lock")).unwrap();
    fs::create_dir(fixture.root.join("plugins.lock")).unwrap();
    assert!(fixture.store.remove(&id()).is_err());
    assert_eq!(fixture.folders().len(), 1);
    assert!(fixture.root.join("cache/plugin-test-guest").is_dir() && fixture.store.list().len() == 1);
    fs::remove_dir(fixture.root.join("plugins.lock")).unwrap();
    block_on(plugin.clear(&block_on(plugin.stores())[0].id));
    assert_eq!(fixture.store.list()[0].bytes, 0, "the retire flag stayed unset");
}

#[test]
fn a_file_where_the_folder_goes_is_replaced_and_a_matching_folder_is_kept_untouched() {
    let fixture = Fixture::new("target");
    let pick = fixture.pick("a", |_| {});
    fixture.install(&pick);
    let name = fixture.store.state().lock.plugins["plugin-test-guest"].dir.clone();
    let target = fixture.root.join("plugins").join(&name);
    fixture.store.set_enabled(&id(), false);
    let bundle = Bundle::read(&pick).unwrap();
    install::write(&fixture.root.join("plugins"), &name, &bundle).unwrap();
    assert_eq!(fixture.folders(), std::slice::from_ref(&name), "a matching folder needs no staging");
    fs::remove_dir_all(&target).unwrap();
    fs::write(&target, b"stray").unwrap();
    install::write(&fixture.root.join("plugins"), &name, &bundle).unwrap();
    assert!(target.is_dir() && Bundle::read(&target).is_ok());
}

#[test]
fn a_lock_that_cannot_be_saved_fails_the_install_and_leaves_no_new_folder() {
    let fixture = Fixture::new("unsaved");
    fs::create_dir_all(&fixture.root).unwrap();
    fs::write(fixture.root.join("plugins.lock"), r#"{"version":3}"#).unwrap();
    let store = Store::new(fixture.root.join("plugins"), fixture.root.join("plugins.lock"), fixture.root.join("cache"));
    let pick = fixture.pick("a", |_| {});
    let approval = store.inspect(&pick).unwrap();
    assert!(store.install(&pick, &approval).is_err());
    assert!(fixture.folders().is_empty() && store.list().is_empty());
}

#[test]
fn a_manifest_that_differs_from_the_lock_is_refused_by_list_and_set_value() {
    let fixture = Fixture::new("manifest");
    fixture.install(&fixture.pick("a", |_| {}));
    let folder = fixture.root.join("plugins").join(&fixture.store.state().lock.plugins["plugin-test-guest"].dir);
    let mut manifest: serde_json::Value = serde_json::from_slice(&fs::read(folder.join("manifest.json")).unwrap()).unwrap();
    manifest["name"] = json!("Other");
    fs::write(folder.join("manifest.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
    let listed = &fixture.store.list()[0];
    assert_eq!(listed.failure.as_ref().map(|failure| failure.kind), Some(FailureKind::Refused));
    assert_ne!(listed.name, "Other");
    assert_eq!(fixture.store.set_value(&id(), "level", Value::Number(7.)).err().map(|e| e.kind), Some(FailureKind::Refused));
}

#[test]
fn remove_leaves_no_folder_lock_entry_values_or_answers_and_a_call_in_flight_does_not_bring_them_back() {
    let fixture = Fixture::new("remove");
    fixture.install(&fixture.pick("a", |_| {}));
    fixture.store.set_value(&id(), "night", Value::Switch(true)).unwrap();
    let plugin = fixture.store.open(&id()).unwrap();
    answer(&plugin);
    let answers = fixture.root.join("cache/plugin-test-guest");
    assert!(answers.is_dir() && fixture.store.list()[0].bytes > 0);
    // The stream holds the plugin's inside, not the `Plugin`: the store cannot reach it through a `Weak`.
    let offer = MapOffer { id: Ref::new(&plugin.id(), "city"), name: String::new(), thumbnail: None, revision: Some("r9".into()) };
    let in_flight = plugin.map(&offer, &Locale::new("en"));
    drop(plugin);
    fixture.store.remove(&id()).unwrap();
    assert!(fixture.folders().is_empty() && !answers.exists() && fixture.store.list().is_empty());
    let lock = fs::read_to_string(fixture.root.join("plugins.lock")).unwrap();
    assert!(!lock.contains("night") && !lock.contains("test-guest"), "{lock}");
    let both = block_on(in_flight.collect::<Vec<_>>());
    assert!(matches!(&both[..], [_, Snapshot::Fresh(Ok(_))]), "the call ran");
    assert!(!answers.exists(), "and kept nothing");
    // A later install starts a fresh flag.
    fixture.install(&fixture.pick("b", |_| {}));
    answer(&fixture.store.open(&id()).unwrap());
    assert!(answers.is_dir());
}

#[test]
fn a_value_is_saved_only_for_a_known_setting_it_fits_and_reaches_the_list() {
    let fixture = Fixture::new("values");
    fixture.install(&fixture.pick("a", |_| {}));
    let refused = |setting: &str, value: Value| fixture.store.set_value(&id(), setting, value).err().map(|e| e.kind);
    assert_eq!(refused("nope", Value::Number(1.)), Some(FailureKind::Refused));
    assert_eq!(refused("level", Value::Number(10.)), Some(FailureKind::Refused));
    assert_eq!(refused("level", Value::Number(2.5)), Some(FailureKind::Refused));
    assert_eq!(refused("level", Value::Text("3".into())), Some(FailureKind::Refused));
    assert_eq!(refused("night", Value::Number(1.)), Some(FailureKind::Refused));
    assert_eq!(refused("level", Value::Number(f64::NAN)), Some(FailureKind::Refused));
    assert_eq!(fixture.store.list()[0].values, [Value::Number(5.), Value::Switch(false)]);
    assert_eq!(refused("level", Value::Number(5.)), None, "a whole number from the UI");
    assert_eq!(refused("level", Value::Number(8.)), None);
    assert_eq!(refused("night", Value::Switch(true)), None);
    let installed = &fixture.store.list()[0];
    assert_eq!(installed.values, [Value::Number(8.), Value::Switch(true)]);
    assert_eq!(installed.settings[0].label, Text::key(&id(), "level-label"));
    assert_eq!(installed.settings[0].hint, Some(Text::key(&id(), "level-hint")));
    let reopened = Store::new(fixture.root.join("plugins"), fixture.root.join("plugins.lock"), fixture.root.join("cache"));
    assert_eq!(reopened.list()[0].values, installed.values, "kept beside the lock entry");
}

#[test]
fn a_plugin_that_kept_answers_lists_their_size_and_clears_them() {
    let fixture = Fixture::new("stores");
    fixture.install(&fixture.pick("a", |_| {}));
    let plugin = fixture.store.open(&id()).unwrap();
    answer(&plugin);
    let [store] = &block_on(plugin.stores())[..] else { panic!("one store") };
    assert_eq!(store.bytes, fixture.store.list()[0].bytes);
    block_on(plugin.clear(&store.id));
    assert_eq!(fixture.store.list()[0].bytes, 0);
}
