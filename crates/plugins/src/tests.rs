//! The test guest (`tests/guest`), built for wasm32 and run as a plugin: the SDK's side of the ABI, the import of a
//! real answer and the cache. Needs `rustup target add wasm32-unknown-unknown`; no network.

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use futures::{StreamExt, executor::block_on};
use model::{Caches, Downloads, FailureKind, Locale, Map, MapOffer, Maps, Ref, Snapshot, Source, StoreId, Titles};
use sdk::wire;

use crate::{Bundle, Plugin, cache::Retired, runtime::Net, scope::Scope};

/// The guest's folder: its manifest and the module, built once per test run.
pub(crate) fn plugin() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let here = Path::new(env!("CARGO_MANIFEST_DIR"));
        let target = here.join("../../target/guest");
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let built = Command::new(cargo)
            .args(["build", "--release", "--offline", "--target", "wasm32-unknown-unknown", "--target-dir"])
            .arg(&target)
            .arg("--manifest-path")
            .arg(here.join("tests/guest/Cargo.toml"))
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "the test guest (needs the wasm32-unknown-unknown target): {}",
            String::from_utf8_lossy(&built.stderr)
        );
        let dir = std::env::temp_dir().join(format!("impulse-test-guest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(here.join("tests/guest/manifest.json"), dir.join("manifest.json")).unwrap();
        std::fs::create_dir_all(dir.join("locales")).unwrap();
        std::fs::copy(here.join("tests/guest/locales/en.ftl"), dir.join("locales/en.ftl")).unwrap();
        std::fs::copy(target.join("wasm32-unknown-unknown/release/impulse_test_guest.wasm"), dir.join("impulse_test_guest.wasm")).unwrap();
        dir
    })
}

fn fake(_: Arc<Scope>) -> Net {
    Arc::new(|url, _, _| match url {
        "https://tiles.example.com/data.json" => Ok(b"fetched".to_vec()),
        _ => Err(wire::Error::new(wire::ErrorKind::Refused, "outside")),
    })
}

fn refusing(_: Arc<Scope>) -> Net {
    Arc::new(|_, _, _| Err(wire::Error::new(wire::ErrorKind::Refused, "outside")))
}

fn open(cache: &Path, values: serde_json::Value, net: impl FnOnce(Arc<Scope>) -> Net) -> Plugin {
    with_flag(cache, values, net, Arc::default())
}

fn with_flag(cache: &Path, values: serde_json::Value, net: impl FnOnce(Arc<Scope>) -> Net, retired: Arc<Retired>) -> Plugin {
    Plugin::with(&Bundle::read(plugin()).unwrap(), cache, values.as_object().unwrap(), net, retired).unwrap_or_else(|e| panic!("{e}"))
}

/// A net that serves the guest's `net` map and counts every request: each guest call of it is one.
fn counting(calls: &Arc<AtomicUsize>) -> impl FnOnce(Arc<Scope>) -> Net {
    let calls = calls.clone();
    move |_| {
        Arc::new(move |_, _, _| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(b"fetched".to_vec())
        })
    }
}

/// What the stream gives: the cached copy, then a fresh one when the copy was missing or stale.
fn ask(plugin: &Plugin, map: &str, revision: &str, locale: &str) -> Vec<Snapshot<Map>> {
    let offer = MapOffer { id: Ref::new(&plugin.id(), map), name: String::new(), thumbnail: None, revision: Some(revision.into()) };
    block_on(plugin.map(&offer, &Locale::new(locale)).collect())
}

fn cache(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("impulse-guest-cache-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn a_real_guest_serves_a_map_that_passes_the_import() {
    let plugin = open(&cache("serve"), serde_json::json!({"level": 7, "night": "on"}), fake);
    assert_eq!(plugin.id().as_str(), "plugin-test-guest");
    let [Snapshot::Cached(None), Snapshot::Fresh(Ok(map))] = &ask(&plugin, "city", "r1", "fr-CA")[..] else {
        panic!("cached none, then fresh")
    };
    assert_eq!(map.tile_sets.iter().map(|set| set.id.as_str()).collect::<Vec<_>>(), ["sat", "drawn"]);
    assert_eq!(map.tile_sets[0].tiles.template, "plugin-test-guest://tiles.example.com/sat/{z}/{x}/{y}.jpg");
    let (p1, p2, p3) = (&map.pins[0], &map.pins[1], &map.pins[2]);
    assert!(
        p1.description.contains("[the gate](pin:p2)") && p1.description.contains("(https://wiki.example.com/p1)"),
        "{}",
        p1.description
    );
    assert!(p1.description.ends_with("or elsewhere."), "{}", p1.description);
    assert_eq!((p2.image.is_none(), p2.video.as_deref()), (true, Some("plugin-test-guest://tiles.example.com/p2.mp4")));
    assert_eq!(p3.name, "fr 7", "the closest locale and the user's setting reach the guest");
    assert_eq!(map.regions[0].outlines.len(), 2);
    assert!((p1.at.x - (20. + 180.) / 360.).abs() < 1e-12, "the SDK's mercator");
}

#[test]
fn a_kept_answer_shows_first_and_is_asked_again_only_when_stale() {
    let dir = cache("keep");
    let plugin = open(&dir, serde_json::json!({}), fake);
    assert!(matches!(&ask(&plugin, "city", "r1", "fr")[..], [Snapshot::Cached(None), Snapshot::Fresh(Ok(_))]));
    assert!(matches!(&ask(&plugin, "city", "r1", "fr")[..], [Snapshot::Cached(Some(_))]), "fresh: no call");
    for (what, revision, locale) in [("another revision", "r2", "fr"), ("another locale", "r1", "en")] {
        assert!(matches!(&ask(&plugin, "city", revision, locale)[..], [Snapshot::Cached(Some(_)), Snapshot::Fresh(Ok(_))]), "{what}");
    }
    let changed = open(&dir, serde_json::json!({"level": 3}), fake);
    assert!(matches!(&ask(&changed, "city", "r1", "fr")[..], [Snapshot::Cached(Some(_)), Snapshot::Fresh(Ok(_))]), "another setting");
}

#[test]
fn a_request_goes_through_the_host_and_the_guests_errors_keep_their_kind() {
    let plugin = open(&cache("net"), serde_json::json!({}), fake);
    let [_, Snapshot::Fresh(Ok(map))] = &ask(&plugin, "net", "r1", "en")[..] else { panic!("the body reaches the guest") };
    assert_eq!(map.pins[0].name, "fetched");
    let refused = open(&cache("refused"), serde_json::json!({}), refusing);
    let [_, Snapshot::Fresh(Err(failure))] = &ask(&refused, "net", "r1", "en")[..] else { panic!("the host's error reaches the guest") };
    assert_eq!(failure.kind, FailureKind::Refused);
    let [_, Snapshot::Fresh(Err(failure))] = &ask(&plugin, "gone", "r1", "en")[..] else { panic!("a map the guest lacks") };
    assert_eq!(failure.kind, FailureKind::NotFound);
}

fn offer(plugin: &Plugin, map: &str) -> MapOffer {
    MapOffer { id: Ref::new(&plugin.id(), map), name: String::new(), thumbnail: None, revision: None }
}

#[test]
fn titles_and_offers_are_the_guests_under_its_own_source_and_kept() {
    let calls = Arc::new(AtomicUsize::new(0));
    let plugin = open(&cache("discover"), serde_json::json!({}), counting(&calls));
    let en = Locale::new("en");
    let [Snapshot::Cached(None), Snapshot::Fresh(Ok(titles))] = &block_on(plugin.titles(&en).collect::<Vec<_>>())[..] else {
        panic!("cached none, then fresh")
    };
    assert_eq!(titles.len(), 1);
    assert_eq!((titles[0].id.to_string().as_str(), titles[0].name.as_str()), ("plugin-test-guest:night", "Night City"));
    assert_eq!(titles[0].stores, [StoreId::parse("steam:1091500").unwrap()]);
    let [Snapshot::Cached(None), Snapshot::Fresh(Ok(offers))] = &block_on(plugin.offers(&titles[0].id, &en).collect::<Vec<_>>())[..] else {
        panic!("cached none, then fresh")
    };
    assert_eq!((offers[0].id.to_string().as_str(), offers[0].revision.as_deref()), ("plugin-test-guest:city", Some("r1")));
    assert!(matches!(&block_on(plugin.titles(&en).collect::<Vec<_>>())[..], [Snapshot::Cached(Some(_))]), "kept: no call");
    assert!(matches!(&block_on(plugin.offers(&titles[0].id, &en).collect::<Vec<_>>())[..], [Snapshot::Cached(Some(_))]));
    let gone = Ref::new(&plugin.id(), "gone");
    let [_, Snapshot::Fresh(Err(failure))] = &block_on(plugin.offers(&gone, &en).collect::<Vec<_>>())[..] else { panic!("no such title") };
    assert_eq!(failure.kind, FailureKind::NotFound);
}

#[test]
fn a_reader_content_with_the_cached_copy_makes_no_guest_call() {
    let calls = Arc::new(AtomicUsize::new(0));
    let plugin = open(&cache("lazy"), serde_json::json!({}), counting(&calls));
    let first = block_on(plugin.map(&offer(&plugin, "net"), &Locale::new("en")).next());
    assert!(matches!(first, Some(Snapshot::Cached(None))));
    // Let a task that was started eagerly finish before looking.
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert_eq!(calls.load(Ordering::SeqCst), 0, "the stream was dropped before the fresh call");
    let both = block_on(plugin.map(&offer(&plugin, "net"), &Locale::new("en")).collect::<Vec<_>>());
    assert_eq!((both.len(), calls.load(Ordering::SeqCst)), (2, 1));
}

#[test]
fn refresh_marks_a_fresh_copy_stale_until_the_next_answer() {
    let calls = Arc::new(AtomicUsize::new(0));
    let plugin = open(&cache("refresh"), serde_json::json!({}), counting(&calls));
    let ask = |plugin: &Plugin| block_on(plugin.map(&offer(plugin, "net"), &Locale::new("en")).collect::<Vec<_>>()).len();
    assert_eq!((ask(&plugin), calls.load(Ordering::SeqCst)), (2, 1));
    assert_eq!((ask(&plugin), calls.load(Ordering::SeqCst)), (1, 1), "fresh");
    plugin.refresh(&Ref::new(&plugin.id(), "net"));
    assert_eq!((ask(&plugin), calls.load(Ordering::SeqCst)), (2, 2), "marked stale: asked again");
    assert_eq!((ask(&plugin), calls.load(Ordering::SeqCst)), (1, 2), "the answer cleared the mark");
}

#[test]
fn a_download_fetches_the_https_url_behind_a_plugin_url_and_refuses_others() {
    let net = |_: Arc<Scope>| -> Net {
        Arc::new(|url, limit, left| {
            assert_eq!((limit, left.as_secs()), (64 << 20, 60), "{url}");
            match url {
                "https://tiles.example.com/a.png" => Ok(b"png".to_vec()),
                "https://tiles.example.com/busy" => Err(wire::Error::new(wire::ErrorKind::Offline, "HTTP 429")),
                _ => Err(wire::Error::new(wire::ErrorKind::NotFound, "HTTP 404")),
            }
        })
    };
    let plugin = open(&cache("download"), serde_json::json!({}), net);
    assert_eq!(plugin.download("plugin-test-guest://tiles.example.com/a.png").unwrap(), b"png");
    let kind = |url: &str| plugin.download(url).unwrap_err().kind;
    assert_eq!(kind("plugin-test-guest://tiles.example.com/missing.png"), FailureKind::NotFound);
    assert_eq!(kind("plugin-test-guest://tiles.example.com/busy"), FailureKind::Offline);
    for refused in [
        "plugin-other://tiles.example.com/a.png",
        "plugin-test-guest:https://tiles.example.com/a.png",
        "plugin-test-guest:tiles.example.com/a.png",
        "https://tiles.example.com/a.png",
        "plugin-test-guestx://tiles.example.com/a.png",
    ] {
        assert_eq!(kind(refused), FailureKind::Refused, "{refused}");
    }
}

#[test]
fn a_plugin_reads_its_catalogs_and_keeps_its_answers_in_a_folder_of_its_id() {
    let dir = cache("texts");
    let plugin = open(&dir, serde_json::json!({}), fake);
    let texts = plugin.texts();
    assert_eq!(texts.len(), 1);
    assert!(&*texts[0].0 == "en" && texts[0].1.contains("level-label = Level"));
    let _ = block_on(plugin.titles(&Locale::new("en")).collect::<Vec<_>>());
    assert!(dir.join("plugin-test-guest").is_dir(), "under the source id: a bare id such as `con` is a device name");
    assert_eq!(plugin.name(), model::Text::Plain("Test guest".into()));
}

#[test]
fn a_failed_call_is_the_plugins_failure_until_a_call_succeeds() {
    let plugin = open(&cache("failure"), serde_json::json!({}), fake);
    assert!(plugin.failure().is_none());
    let [_, Snapshot::Fresh(Err(_))] = &ask(&plugin, "gone", "r1", "en")[..] else { panic!("a map the guest lacks") };
    assert_eq!(plugin.failure().map(|failure| failure.kind), Some(FailureKind::NotFound));
    assert!(matches!(&ask(&plugin, "city", "r1", "en")[..], [_, Snapshot::Fresh(Ok(_))]));
    assert!(plugin.failure().is_none());
}

#[test]
fn a_retired_plugin_writes_no_answer_and_clearing_empties_its_folder() {
    let dir = cache("retire");
    let retired = Arc::new(Retired::default());
    let plugin = with_flag(&dir, serde_json::json!({}), fake, retired.clone());
    let folder = dir.join("plugin-test-guest");
    let [store] = &block_on(plugin.stores())[..] else { panic!("one store") };
    assert_eq!((store.id.to_string().as_str(), store.bytes), ("plugin-test-guest:answers", 0));
    assert_eq!(store.hint, model::Text::Plain("*.tiles.example.com, tiles.example.com".into()));
    assert!(matches!(&ask(&plugin, "city", "r1", "en")[..], [_, Snapshot::Fresh(Ok(_))]));
    assert!(block_on(plugin.stores())[0].bytes > 0);
    plugin.refresh(&Ref::new(&plugin.id(), "city"));
    block_on(plugin.clear(&store.id));
    assert!(!folder.exists());
    assert!(matches!(&ask(&plugin, "city", "r1", "en")[..], [Snapshot::Cached(None), Snapshot::Fresh(Ok(_))]), "cleared");
    std::fs::remove_dir_all(&folder).unwrap();
    retired.retire();
    assert!(matches!(&ask(&plugin, "city", "r1", "en")[..], [Snapshot::Cached(None), Snapshot::Fresh(Ok(_))]));
    assert!(!folder.exists(), "a retired plugin brings no folder back");
}
