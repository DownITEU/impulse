use std::sync::Mutex;

use futures::executor::block_on;
use model::{Clear, Source, Text};

use super::*;

/// Answers with its own name; a map stream it was asked for stays open until the test sends on it.
struct Fake {
    id: SourceId,
    log: Mutex<Vec<String>>,
    open: Mutex<Vec<mpsc::UnboundedSender<Snapshot<Map>>>>,
}

impl Fake {
    fn new(name: &str) -> Arc<Self> {
        Arc::new(Self { id: SourceId::new(name).unwrap(), log: Mutex::default(), open: Mutex::default() })
    }

    fn folder(&self, path: Option<PathBuf>) -> Folder {
        let (label, hint, status) = (Text::Plain(String::new()), Text::Plain(String::new()), Ok(Text::Plain(self.id.as_str().to_owned())));
        Folder { id: Ref::new(&self.id, "f"), label, hint, path, status, choices: Vec::new(), selected: None }
    }

    fn logged(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}

impl Source for Fake {
    fn id(&self) -> SourceId {
        self.id.clone()
    }
}

impl Games for Fake {
    fn games(&self) -> BoxStream<'static, Snapshot<Vec<Game>>> {
        stream::empty().boxed()
    }
}

impl Maps for Fake {
    // "open" stays open; any other map answers with an error naming who answered.
    fn map(&self, offer: &MapOffer, _: &Locale) -> BoxStream<'static, Snapshot<Map>> {
        if offer.id.id() == "open" {
            let (tx, rx) = mpsc::unbounded();
            self.open.lock().unwrap().push(tx);
            return rx.boxed();
        }
        stream::iter([Snapshot::Fresh(Err(anyhow::anyhow!("{} has {}", self.id.as_str(), offer.id).into()))]).boxed()
    }

    fn refresh(&self, map: &Ref) {
        self.log.lock().unwrap().push(format!("refresh {map}"));
    }
}

impl Downloads for Fake {
    fn download(&self, url: &str) -> Result<Vec<u8>, Failure> {
        Ok(format!("{} got {url}", self.id.as_str()).into_bytes())
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
        let (name, hint) = (Text::Plain(String::new()), Text::Plain(String::new()));
        let store = Store { id: Ref::new(&self.id, "s"), name, hint, bytes: 1, clear: Clear::Now };
        future::ready(vec![store]).boxed()
    }

    fn clear(&self, id: &Ref) -> BoxFuture<'static, ()> {
        self.log.lock().unwrap().push(format!("clear {id}"));
        future::ready(()).boxed()
    }
}

fn every(fake: &Arc<Fake>) -> Capabilities {
    Capabilities {
        maps: Some(fake.clone()),
        downloads: Some(fake.clone()),
        folders: Some(fake.clone()),
        caches: Some(fake.clone()),
        ..Capabilities::default()
    }
}

fn registry() -> (Registry, Arc<Fake>, Arc<Fake>) {
    let (a, b) = (Fake::new("a"), Fake::new("b"));
    let registry = Registry::default();
    registry.add(every(&a));
    registry.add(every(&b));
    (registry, a, b)
}

fn ask(registry: &Registry, id: &str) -> BoxStream<'static, Snapshot<Map>> {
    let offer = MapOffer { id: Ref::parse(id).unwrap(), name: String::new(), thumbnail: None, revision: None };
    registry.map(&offer, &Locale::new("en"))
}

fn answer(registry: &Registry, id: &str) -> String {
    match block_on(ask(registry, id).next()) {
        Some(Snapshot::Fresh(Err(e))) => e.detail,
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
    registry.refresh(&Ref::parse("b:m").unwrap());
    assert!(a.logged().is_empty());
    assert_eq!(b.logged(), ["clear b:s", "refresh b:m"]);
    let set = block_on(registry.set_folder(&Ref::parse("b:f").unwrap(), "x".into()));
    assert!(matches!(set.status, Ok(Text::Plain(name)) if name == "b"));
    assert!(block_on(registry.set_folder(&Ref::parse("c:f").unwrap(), "x".into())).status.is_err());
}

#[test]
fn a_download_goes_to_the_source_its_url_names() {
    let (registry, ..) = registry();
    assert_eq!(registry.download("b://tiles.io/1.png").unwrap(), b"b got b://tiles.io/1.png");
    for unrouted in ["c://tiles.io/1.png", "https://tiles.io/1.png", "no scheme"] {
        assert_eq!(registry.download(unrouted).unwrap_err().kind, FailureKind::Unsupported, "{unrouted}");
    }
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

#[test]
fn a_removed_source_ends_its_streams_lets_nothing_late_through_and_answers_again_once_added() {
    let (registry, a, _) = registry();
    let changes = registry.changes();
    let mut open = ask(&registry, "a:open");
    registry.remove(&a.id);
    a.open.lock().unwrap()[0].unbounded_send(Snapshot::Cached(None)).unwrap();
    assert!(block_on(open.next()).is_none(), "the late copy is dropped and the stream ends");
    assert_eq!(answer(&registry, "a:x"), "no source for a:x");
    assert_eq!(registry.texts().len(), 1);
    registry.add(every(&a));
    assert_eq!(answer(&registry, "a:x"), "a has a:x");
    let mut again = ask(&registry, "a:open");
    a.open.lock().unwrap()[1].unbounded_send(Snapshot::Cached(None)).unwrap();
    assert!(matches!(block_on(again.next()), Some(Snapshot::Cached(None))), "the new registration answers");
    registry.add(every(&a));
    assert!(block_on(again.next()).is_none(), "added again: the older registration's streams end");
    drop(registry);
    let changes: Vec<_> = block_on(changes.collect());
    let (added, removed) = (Change::Added(a.id.clone()), Change::Removed(a.id.clone()));
    assert_eq!(changes, [removed.clone(), added.clone(), removed, added]);
}

#[test]
fn a_source_plays_only_with_games_trainers_or_accounts() {
    let (registry, a, b) = registry();
    registry.add(Capabilities { games: Some(a.clone()), ..Capabilities::default() });
    assert!(registry.plays(&a.id));
    assert!(!registry.plays(&b.id));
    assert!(!registry.plays(&SourceId::new("c").unwrap()));
}

#[test]
#[should_panic(expected = "a capability of another source")]
fn a_capability_of_another_source_is_refused() {
    let (a, b) = (Fake::new("a"), Fake::new("b"));
    Registry::default().add(Capabilities { maps: Some(a), caches: Some(b), ..Capabilities::default() });
}
