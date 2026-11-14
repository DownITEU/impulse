---
paths:
  - "crates/client/**"
---

# client

The app's logic without GPUI: what `shell`'s entities hold and decide, and the files that keep it. Data in, data out; the async work and the UI stay in `shell`.

## Surface
- `storage`: `Paths` names every app file and folder; `Stored<T>`; `read`, `write` for the per-map files.
- `media`: `Media` (a cache folder per kind), the file per URL, `known_missing`, `steam_hero`; `limit` (`Kept`, `sweep`).
- `profile`: `Profiles` (`games.json`), `Profile`, `Collection`, `Values`.
- `bindings`: `Store` (`hotkeys.json`), `Choice`, `rebind` with the "no chord twice" rule.
- `trainer`: `Sheet`, a trainer as the app holds it, `step` (a hotkey's `Step`), `Span` (a slider), `initial`.
- `library`: `Game`, `Library`, `Support`, `Stamp`, `scan` (launchers joined with the catalog), `Picked` (`picked.json`), `sources` (the source menu).
- `run`: `request` (a `play::Request` from an install and a plan), `start`, `Stage`, and the bookkeeping `opened`, `closed`.
- `map`: `state` (the user's state of a map, a file per map) and `heading`.

## Rules
- No GPUI and no executor: a function here returns data; `scan` and `sweep` block and `start` is awaited, all by `shell`, off the UI thread.
- An app file is a `Stored<T>` opened from its `Paths` entry by its owner; a constructor takes `&Paths`, never the data folder. `edit` saves at once, blocking, in place and in order: tiny files only. Missing or damaged means the defaults, so a new field is `#[serde(default)]`.
- Map ids reach a file name only through `state::file`; a new `Saved` field is `#[serde(default)]`.
- Strings a frame prints are made once (`Game::name` is upper case, an `Arc<str>`); `shell` turns them into `SharedString` without a copy.
- A rebind is refused when the chord is held by another slot of the cheat (`Chord::same_keys`) or `reserved` says so: the overlay's test merges modifier sides.
- Stepping follows Wand's, except that `round_to` keeps the decimals of `min` too.
