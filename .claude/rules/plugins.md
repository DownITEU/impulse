---
paths:
  - "crates/plugins/**"
---

# plugins

The WASM host: a plugin the user installs runs as a source, `plugin-<id>`, behind `model`'s traits.

## Surface
- `Bundle::read(pick)`: a plugin's files read once from a folder or a `.zip` (at its root or under one top folder; other entries ignored, nothing extracted to a path from it), each with its sha256; `digest`.
- `Plugin::open(bundle, caches, values)` (blocking): the manifest checked, the module compiled once (lazily), the user's settings checked against their fields, the catalogs taken from the bundle. Its answers are kept in `caches/<source id>` (a bare id may be a Windows device name). `Values`: settings by field id.
- `Store` (`app` wraps it as `client::plugins::Host`): `plugins/`, `plugins.lock` (a `disk::Stored`, its only writer: per source id the folder, version, enabled, approved hosts and links, file hashes and the user's values) and the answer caches. `list`, `inspect` (an `Approval`: what the pick grants, the version it replaces, the hosts and links not approved before, the digest), `install`, `open`, `set_enabled`, `remove`, `set_value`; every call blocks.
- `Plugin` is a `model::Source` (`name`: the manifest's) and:
  - `Maps`: `map` by offer; `refresh(map)` marks the copy stale in memory until a fresh answer for it arrives.
  - `Titles`: `titles` (cache key `titles`) and `offers` (key `maps:<title id>`), which ask `Call::Titles` and `Call::Maps`.
  - `Downloads`: `download(url)` (blocking) for a URL this plugin put in its data.
  - `Caches`: one store, its answers (`clear` empties them and the stale marks).
  - `failure()`: its last call's failure, until a call succeeds.
- `Source::texts()`: `locales/*.ftl` beside the manifest, from the bundle: at most 16 files of 256 KB, the tag is the file stem (1 to 35 bytes of ASCII letters, digits, `-`, the one `manifest::language_tag`, as the manifest's locales); a bad file is skipped with a debug log, a missing folder is none. A catalog whose name differs only by case from an earlier one is skipped too (they cannot coexist on Windows).
- `map`, `titles` and `offers` share one stream (`serve`): `Cached` first, then the guest's call only when the copy is missing or stale. The call starts when the stream is polled past `Cached`, never before: a reader that takes `Cached` and drops the stream makes none.

## Rules
- Install is approval: `install` takes only the bytes `inspect` showed (same digest), writes `plugins/<source id>.<12 hex of the digest>` through a `.new` folder (every file via `disk::replace`), saves the lock entry with `Stored::try_edit`, then removes the old folder: a crash or a failed save leaves one approved version usable (the failure is returned and the new folder removed unless it is the old entry's own). A folder already there is reused, before anything is staged, only when its files equal the bundle's hashes, else replaced (a file in its place too): installing again repairs. `inspect` refuses a module that does not validate in full or lacks the ABI exports `run` looks up (`runtime::check`, static, nothing of the guest runs), so a broken pick never replaces a working install.
- `open` refuses files that differ from the lock (`Refused`: install it again) and a folder the lock does not name is never opened. `list` and `set_value` read the manifest alone and check it against the lock's hash the same way.
- `set_value` keeps a value only when it fits its field. `Store` keeps one retire flag per source id, shared by every `Cache` it opens for it (instances a call still holds, or a settings change replaced, too): `remove` saves the lock without the entry first (`Stored::try_edit`): on failure nothing is deleted, the flag stays unset and the failure is returned; else it sets the flag, held across a write's check, then the folder and answers go. A retired cache neither writes nor clears. A later install or open starts a fresh flag.
- Only `app` names this crate. It reaches `model`, `sdk`, `disk` and `fetch`, never `client`.
- Every guest answer passes `import` before the app sees it, fresh or read from disk: counts and lengths bounded, ids set and unique per kind, groups found, zooms and bounds valid, points finite. A broken structure refuses the answer (`Refused`); media, tile templates and links off the manifest's hosts are left out. Titles: at most 10,000, offers 256 per title, stores 32 per title, ids 256 bytes, names 512; a store id that does not parse is dropped. Every `Ref` is made from the plugin's own source id.
- Kept media is `plugin-<id>://<host>/<rest>`: the https URL with its scheme replaced by the source id. GPUI's `img()` needs scheme and authority, and it still parses as a `model::Ref`. `download` takes only that form: another source, or no `//` after the prefix, is `Refused`; it fetches `https:<rest>` through the plugin's `Net` with 64 MiB and 60 s.
- `Net` is `(url, byte limit, time left)`. Statuses reach the guest and `download` as error kinds: 404 and 410 `NotFound`; 401 and 403 `Refused`; 429 and 5xx `Offline` (worth asking again); others `Internal`; a `fetch::Refused` marker `Refused`; any other error `Offline`. `runtime::guest` maps a wire error to a `Failure` kind for both.
- An instance per call (`runtime`), on a blocking thread, never the UI's: one memory of 64 MB, one table of 64 Ki elements, 2,000 M fuel, a 16 MB reply, 64 requests, one 60 s deadline for the call with each request given what is left. Past one: `Refused`, the plugin named in the detail. A body past 16 MB reaches the guest as `refused`.
- Files are read bounded: the manifest 64 KB, the module 16 MB, a catalog 256 KB (16 at most), a zip of 1,024 entries with sizes checked before and while inflating, a kept answer 64 MB. A guest's text reaches the log cut to 1 KiB.
- Imports trap until the guest's `impulse_api` answered `sdk::API`; `take` with nothing pending traps.
- A guest's GET is `fetch::guarded` with the manifest's `hosts`. No headers of the plugin's.
- Answers are kept as the guest gave them (`cache`), stamped with the wire, the plugin's version, the served locale, the settings and the revision; another stamp, or no revision and older than `refresh`, asks again; one of another wire is not shown. Titles and offers carry no revision: `refresh` days decide. 64 MB per plugin, the least recently written dropped first.
- Settings are the user's: the guest gets them with every call and cannot write them; a value that does not fit its field is the default.

## Traps
- wasmi 2.0.1's `Caller::get_export` reads a dangling instance (an access violation): imports reach the guest's memory through the `Memory` kept in `Host`.
- The tests build `tests/guest` (its own workspace) with `cargo build --target wasm32-unknown-unknown`: the gate needs that target (`rustup target add wasm32-unknown-unknown`). The fuel test runs a few seconds.
- `serde_json`'s `float_roundtrip` is on for the whole build once this crate is in it.
