---
paths:
  - "crates/plugins/**"
---

# plugins

The WASM host: a plugin the user installs runs as a source, `plugin-<id>`, behind `model`'s traits.

## Surface
- `Plugin::open(dir, caches, values)` (blocking): the manifest checked, the module compiled once (lazily), the user's settings checked against their fields, the catalogs read. Its answers are kept in `caches/<manifest id>`. `Values`: settings by field id.
- `Plugin` is a `model::Source` and:
  - `Maps`: `map` by offer; `refresh(map)` marks the copy stale in memory until a fresh answer for it arrives.
  - `Titles`: `titles` (cache key `titles`) and `offers` (key `maps:<title id>`), which ask `Call::Titles` and `Call::Maps`.
  - `Downloads`: `download(url)` (blocking) for a URL this plugin put in its data.
- `Source::texts()`: `locales/*.ftl` beside the manifest, read once at open: at most 16 files of 256 KB, the tag is the file stem (1 to 35 bytes of ASCII letters, digits, `-`); a bad file is skipped with a debug log, a missing folder is none.
- `map`, `titles` and `offers` share one stream (`serve`): `Cached` first, then the guest's call only when the copy is missing or stale. The call starts when the stream is polled past `Cached`, never before: a reader that takes `Cached` and drops the stream makes none.

## Rules
- Only `app` names this crate. It reaches `model`, `sdk`, `disk` and `fetch`, never `client`.
- Every guest answer passes `import` before the app sees it, fresh or read from disk: counts and lengths bounded, ids set and unique per kind, groups found, zooms and bounds valid, points finite. A broken structure refuses the answer (`Refused`); media, tile templates and links off the manifest's hosts are left out. Titles: at most 10,000, offers 256 per title, stores 32 per title, ids 256 bytes, names 512; a store id that does not parse is dropped. Every `Ref` is made from the plugin's own source id.
- Kept media is `plugin-<id>://<host>/<rest>`: the https URL with its scheme replaced by the source id. GPUI's `img()` needs scheme and authority, and it still parses as a `model::Ref`. `download` takes only that form: another source, or no `//` after the prefix, is `Refused`; it fetches `https:<rest>` through the plugin's `Net` with 64 MiB and 60 s.
- `Net` is `(url, byte limit, time left)`. Statuses reach the guest and `download` as error kinds: 404 and 410 `NotFound`; 401 and 403 `Refused`; 429 and 5xx `Offline` (worth asking again); others `Internal`; a `fetch::Refused` marker `Refused`; any other error `Offline`. `runtime::guest` maps a wire error to a `Failure` kind for both.
- An instance per call (`runtime`), on a blocking thread, never the UI's: one memory of 64 MB, one table of 64 Ki elements, 2,000 M fuel, a 16 MB reply, 64 requests, one 60 s deadline for the call with each request given what is left. Past one: `Refused`, the plugin named in the detail. A body past 16 MB reaches the guest as `refused`.
- Files are read bounded: the manifest 64 KB, the module 16 MB, a kept answer 64 MB. A guest's text reaches the log cut to 1 KiB.
- Imports trap until the guest's `impulse_api` answered `sdk::API`; `take` with nothing pending traps.
- A guest's GET is `fetch::guarded` with the manifest's `hosts`. No headers of the plugin's.
- Answers are kept as the guest gave them (`cache`), stamped with the wire, the plugin's version, the served locale, the settings and the revision; another stamp, or no revision and older than `refresh`, asks again; one of another wire is not shown. Titles and offers carry no revision: `refresh` days decide. 64 MB per plugin, the least recently written dropped first.
- Settings are the user's: the guest gets them with every call and cannot write them; a value that does not fit its field is the default.

## Traps
- wasmi 2.0.1's `Caller::get_export` reads a dangling instance (an access violation): imports reach the guest's memory through the `Memory` kept in `Host`.
- The tests build `tests/guest` (its own workspace) with `cargo build --target wasm32-unknown-unknown`: the gate needs that target (`rustup target add wasm32-unknown-unknown`). The fuel test runs a few seconds.
- `serde_json`'s `float_roundtrip` is on for the whole build once this crate is in it.
