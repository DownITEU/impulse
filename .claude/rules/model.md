---
paths:
  - "crates/model/**"
---

# model

What the UI, a run and the providers share, in no provider's terms: data and traits, with only the logic every provider must share.

## Surface
- `Provider`: a source of games, trainers, maps, an account, folders and caches. `Snapshot`, `SignIn`, `Account`, `Folder`, `Store`, `Clear` are its answers.
- `Game`, `Offer`, `Standalone`, `Locator`: a game a provider lists, with its store ids, `exe`, art and offers.
- `StoreId`: a launcher's id for a game, `<store>:<id>`, the one place that splits one (`store`, `id`, `steam_app`). `Installation`: a game on this machine (`Origin`: a launcher's `StoreId` or the picked executable, location, build).
- `Chord`, `Held`, `mouse`: a hotkey in Wand's key codes, its names (`Display`), the keys being pressed.
- `Process`, `Rule`: the game's process and how a trainer says which one to look for.
- `Trainer`, `Cheat`, `Kind`, `Args`, `Plan`: the cheat list and what running exactly this list takes.
- `Runner`, `Attach`, `Live`, `Session`, `Event`: a run. `prepare`, then `attach(Process)`, then set values and stop.
- `Map`, `MapOffer`, `Tiles`, `Frame`, `Pin`, `Category`, `Group`, `Glyph`, `Region`, `Label`, `Picture`, `Level`, `PIN_LINK`: a game's map.

## Rules
- A snapshot stream yields `Cached(Option)` first, then `Fresh(Result)` only when the copy was missing or stale. An `Err` keeps the cached value: a consumer never clears on it.
- `Rule::matches` is case-insensitive. `image` is a whole-name path suffix (`*` inside one name); without it the install folder is a prefix at a `\` boundary. The glob stays iterative and bounded: the pattern is remote data.
- `Process` is pid and creation time together. `Attach::attach` refuses a process that is no longer the one given.
- `Runner::prepare` runs before the game is looked for and refuses what cannot run.
- `Plan::runner` runs exactly the cheat list of the `Trainer` that holds it. An implementer builds both from one value.
- `Cheat::id` is unique in its trainer. `key` may be shared, or `None`: it then sets nothing.
- `Kind::slots` is the hotkey slot count (toggle and button 1, number, slider and selection 2: decrease then increase, `Unknown` 0). `Cheat::hotkeys` is indexed by slot with `None` for unbound, and may be shorter than the slots: read a missing slot as unbound.
- `Args::clamp` never panics: trainers send `min > max`. Do not replace it with `f64::clamp`.
- Dropping a `Session` ends the run. `Live::events` ends with the session. `position` and `teleport` use the game's own coordinates and are valid only when the trainer says so.
- `Group::id` and `Pin::id` are stable across revisions: the user's state refers to them. A `revision` changes whenever its content does: staleness rests on it.
- A pin description links to another pin with `PIN_LINK`. A provider rewrites or drops its own link forms.
- A `Frame` is valid: `axes` a permutation of 0, 1, 2, `scale` never zero.
- `set_folder` calls are ordered: an older one never saves over a newer.
- No I/O here, and no provider name or UI text except the chord names.
- `model` names no crate of ours.

