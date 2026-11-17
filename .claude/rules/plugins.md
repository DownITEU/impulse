---
paths:
  - "crates/plugins/**"
---

# plugins

The WASM host: a plugin the user installs runs as a source, `plugin-<id>`, behind `model`'s traits.

## Surface
- `Plugin::open(dir, cache, values)` (blocking): the manifest checked, the module compiled once (lazily), the user's settings checked against their fields. `Plugin` is a `model::Source` and `Maps`. `Values`: settings by field id.

## Rules
- Only `app` names this crate (step 4 registers plugins). It reaches `model`, `sdk`, `disk` and `fetch`, never `client`.
- Every guest answer passes `import` before the app sees it, fresh or read from disk: counts and lengths bounded, ids set and unique per kind, groups found, zooms and bounds valid, points finite. A broken structure refuses the map (`Refused`); media, tile templates and links off the manifest's hosts are left out, and kept media is rewritten `plugin-<id>:<url>` (resolved by step 4's media routing).
- An instance per call (`runtime`), on a blocking thread, never the UI's: one memory of 64 MB, one table of 64 Ki elements, 2,000 M fuel, a 16 MB reply, 64 requests, one 60 s deadline for the call with each request given what is left. Past one: `Refused`, the plugin named in the detail. A body past 16 MB reaches the guest as `refused`.
- Files are read bounded: the manifest 64 KB, the module 16 MB, a kept answer 64 MB. A guest's text reaches the log cut to 1 KiB.
- Imports trap until the guest's `impulse_api` answered `sdk::API`; `take` with nothing pending traps.
- A guest's GET is `fetch::guarded` with the manifest's `hosts`; its status reaches the guest as an error kind. No headers of the plugin's.
- Answers are kept as the guest gave them (`cache`), stamped with the wire, the plugin's version, the served locale, the settings and the revision; another stamp, or no revision and older than `refresh`, asks again; one of another wire is not shown. 64 MB per plugin, the least recently written dropped first.
- Settings are the user's: the guest gets them with every call and cannot write them; a value that does not fit its field is the default.

## Traps
- wasmi 2.0.1's `Caller::get_export` reads a dangling instance (an access violation): imports reach the guest's memory through the `Memory` kept in `Host`.
- The tests build `tests/guest` (its own workspace) with `cargo build --target wasm32-unknown-unknown`: the gate needs that target (`rustup target add wasm32-unknown-unknown`). The fuel test runs a few seconds.
- `serde_json`'s `float_roundtrip` is on for the whole build once this crate is in it.
