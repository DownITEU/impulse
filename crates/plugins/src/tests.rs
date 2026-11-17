//! The test guest (`tests/guest`), built for wasm32 and run as a plugin: the SDK's side of the ABI, the import of a
//! real answer and the cache. Needs `rustup target add wasm32-unknown-unknown`; no network.

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, OnceLock},
};

use futures::{StreamExt, executor::block_on};
use model::{FailureKind, Locale, Map, MapOffer, Maps, Ref, Snapshot, Source};
use sdk::wire;

use crate::{Plugin, runtime::Net, scope::Scope};

/// The guest's folder: its manifest and the module, built once per test run.
fn plugin() -> &'static Path {
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
        std::fs::copy(target.join("wasm32-unknown-unknown/release/impulse_test_guest.wasm"), dir.join("impulse_test_guest.wasm")).unwrap();
        dir
    })
}

fn fake(_: Arc<Scope>) -> Net {
    Arc::new(|url, _| match url {
        "https://tiles.example.com/data.json" => Ok(b"fetched".to_vec()),
        _ => Err(wire::Error::new(wire::ErrorKind::Refused, "outside")),
    })
}

fn refusing(_: Arc<Scope>) -> Net {
    Arc::new(|_, _| Err(wire::Error::new(wire::ErrorKind::Refused, "outside")))
}

fn open(cache: &Path, values: serde_json::Value, net: fn(Arc<Scope>) -> Net) -> Plugin {
    Plugin::with(plugin(), cache.to_owned(), values.as_object().unwrap(), net).unwrap_or_else(|e| panic!("{e}"))
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
    assert_eq!(map.tile_sets[0].tiles.template, "plugin-test-guest:https://tiles.example.com/sat/{z}/{x}/{y}.jpg");
    let (p1, p2, p3) = (&map.pins[0], &map.pins[1], &map.pins[2]);
    assert!(
        p1.description.contains("[the gate](pin:p2)") && p1.description.contains("(https://wiki.example.com/p1)"),
        "{}",
        p1.description
    );
    assert!(p1.description.ends_with("or elsewhere."), "{}", p1.description);
    assert_eq!((p2.image.is_none(), p2.video.as_deref()), (true, Some("plugin-test-guest:https://tiles.example.com/p2.mp4")));
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
