use std::sync::Mutex;

use futures::executor::block_on;
use model::{Clear, Source};

use super::*;

struct Fake {
    id: SourceId,
    cleared: Mutex<Vec<String>>,
}

impl Fake {
    fn new(name: &str) -> Arc<Self> {
        Arc::new(Self { id: SourceId::new(name).unwrap(), cleared: Mutex::default() })
    }

    fn folder(&self, path: Option<PathBuf>) -> Folder {
        let status = Ok(self.id.as_str().to_owned());
        Folder { id: Ref::new(&self.id, "f"), label: "", hint: "", path, status, choices: Vec::new(), selected: None }
    }
}

impl Source for Fake {
    fn id(&self) -> SourceId {
        self.id.clone()
    }
}

impl Maps for Fake {
    // The error names who answered.
    fn map(&self, offer: &MapOffer) -> BoxStream<'static, Snapshot<Map>> {
        stream::iter([Snapshot::Fresh(Err(anyhow::anyhow!("{} has {}", self.id.as_str(), offer.id)))]).boxed()
    }
}

impl Folders for Fake {
    fn folders(&self) -> BoxFuture<'static, Vec<Folder>> {
        future::ready(vec![self.folder(None)]).boxed()
    }

    fn set_folder(&self, _: &Ref, path: PathBuf) -> BoxFuture<'static, Folder> {
        future::ready(self.folder(Some(path))).boxed()
    }
}

impl Caches for Fake {
    fn stores(&self) -> BoxFuture<'static, Vec<Store>> {
        let store = Store { id: Ref::new(&self.id, "s"), name: "", hint: "", bytes: 1, clear: Clear::Now };
        future::ready(vec![store]).boxed()
    }

    fn clear(&self, id: &Ref) -> BoxFuture<'static, ()> {
        self.cleared.lock().unwrap().push(id.to_string());
        future::ready(()).boxed()
    }
}

fn registry() -> (Registry, Arc<Fake>, Arc<Fake>) {
    let (a, b) = (Fake::new("a"), Fake::new("b"));
    let registry = Registry::default()
        .with_maps(a.clone())
        .with_maps(b.clone())
        .with_folders(a.clone())
        .with_folders(b.clone())
        .with_caches(a.clone())
        .with_caches(b.clone());
    (registry, a, b)
}

fn answer(registry: &Registry, id: &str) -> String {
    let offer = MapOffer { id: Ref::parse(id).unwrap(), name: String::new(), thumbnail: None, revision: None };
    match block_on(registry.map(&offer).next()) {
        Some(Snapshot::Fresh(Err(e))) => e.to_string(),
        _ => panic!("one fresh error"),
    }
}

#[test]
fn a_ref_reaches_its_own_source_and_an_unknown_one_fails() {
    let (registry, a, b) = registry();
    assert_eq!(answer(&registry, "b:x"), "b has b:x");
    assert_eq!(answer(&registry, "a:x"), "a has a:x");
    assert_eq!(answer(&registry, "c:x"), "no source for c:x");
    block_on(registry.clear(&Ref::parse("b:s").unwrap()));
    block_on(registry.clear(&Ref::parse("app:pictures").unwrap()));
    assert!(a.cleared.lock().unwrap().is_empty());
    assert_eq!(*b.cleared.lock().unwrap(), ["b:s"]);
    let set = block_on(registry.set_folder(&Ref::parse("b:f").unwrap(), "x".into()));
    assert_eq!(set.status.as_deref(), Ok("b"));
    assert!(block_on(registry.set_folder(&Ref::parse("c:f").unwrap(), "x".into())).status.is_err());
}

#[test]
fn folders_and_caches_list_every_source() {
    let (registry, ..) = registry();
    let folders: Vec<_> = block_on(registry.folders()).iter().map(|folder| folder.id.to_string()).collect();
    assert_eq!(folders, ["a:f", "b:f"]);
    let stores: Vec<_> = block_on(registry.stores()).iter().map(|store| store.id.to_string()).collect();
    assert_eq!(stores, ["a:s", "b:s"]);
    assert!(matches!(block_on(Registry::default().sign_in().next()), Some(SignIn::Failed)));
}
