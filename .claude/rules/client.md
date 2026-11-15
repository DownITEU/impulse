---
paths:
  - "crates/client/**"
---

# client

The app's logic without GPUI: what `shell`'s entities hold and decide, and the files that keep it. Data in, data out; the async work and the UI stay in `shell`.

## Surface
- `Registry`: the sources by capability, built in `app`. A `Ref` goes to the source it names, an unknown one answers `Fresh(Err)` (a folder: an error status); `folders` and `stores` join every source's; `games` and `sign_in` are the first source's; `texts`, each source's catalogs once.
- `storage`: `Paths` names every app file and folder; `Stored<T>`; `read`, `write` for the per-map files; `migrate` (v1 files to v2, run by `app` before the window); `by_ref` (serde for a map keyed by `Ref`); `name` (an id as a file name).
- `media`: `Media` (a cache folder per kind), the file per URL, `known_missing`, `steam_hero`; `limit` (`Kept`, `sweep`).
- `profile`: `Profiles` (`games.json`): a `GameProfile` by `GameId`, a `TrainerProfile` by the trainer's `Ref`; `Collection`, `Values`.
- `bindings`: `Store` (`hotkeys.json`, by trainer `Ref`), `Choice`, `rebind` with the "no chord twice" rule.
- `Load`: something that arrives after the window opens, or the `Failure` why it did not.
- `trainer`: `Sheet`, a trainer as the app holds it, with its `Ref` and folds by section id (`PINNED` or a category), `step` (a hotkey's `Step`), `Span` (a slider), `initial`; `Shelf`, the sheets by game and the holds (a held game's newer copy waits).
- `library`: `Game`, `GameId` (the first source's game `Ref` as text), `Library`, `Support`, `Stamp`, `scan` (launchers joined with the catalog), `Picked` (`picked.json`), `sources` (the source menu).
- `run`: `request` (a `play::Request` from an install and a plan), `start`, `Stage`, and the bookkeeping `opened`, `closed`.
- `map`: `state` (the user's state of a map, a file per map) and `heading`.

## Rules
- No GPUI and no executor: a function here returns data; `scan` and `sweep` block and `start` is awaited, all by `shell`, off the UI thread.
- An app file is a `Stored<T>` opened from its `Paths` entry by its owner; a constructor takes `&Paths`, never the data folder. `edit` saves at once, blocking, in place and in order: tiny files only. Missing or damaged means the defaults, so a new field is `#[serde(default)]`. An object file carries `version: 2`; one without it, or that does not read, is never written (`Stored::load`).
- Keys are the source's ids: no code here splits a `Ref` or names a source. A change of a file's keys or shape is a new version with its step in `migrate`.
- An id reaches a file name only through `storage::name`; a new `Saved` field is `#[serde(default)]`.
- Strings a frame prints are made once (`Game::name` is upper case, an `Arc<str>`); `shell` turns them into `SharedString` without a copy.
- A rebind is refused when the chord is held by another slot of the cheat (`Chord::same_keys`) or `reserved` says so: the overlay's test merges modifier sides.
- Stepping follows Wand's, except that `round_to` keeps the decimals of `min` too.
