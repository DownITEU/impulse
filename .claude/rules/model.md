---
paths:
  - "crates/model/**"
---

# model

What the UI, a run and the sources share, in no source's terms: data and traits, with only the logic every source must share.

## Surface
- `Source` (`id`, `texts`: its catalogs as `Files`, owned; `name`: what the user calls it, its id by default) and the capabilities over it: `Accounts`, `Games`, `Trainers` (by `Offer`, the installed build, `None` when unknown or not installed, and `Locale`), `Maps` (by `MapOffer` and `Locale`; `refresh` marks a copy stale), `Titles` (discovery: `titles`, a title's `offers`), `Downloads` (the bytes behind a URL the source gave), `Folders`, `Caches`, `Overlays` (`enter`: its overlay loaded into a running game, blocking); a source implements only what it has. `Snapshot`, `SignIn` (`Failed` carries the `Failure`: why there is no session), `Account`, `Title`, `Folder`, `Store`, `Clear` are their answers.
- `SourceId`, `Ref`: a source's name and an id it minted, printed `source:id`; only `Ref` splits one. `SourceId::APP` is the app's own.
- `Text`: a message of a source's catalog (its domain is its `SourceId`), or plain content. `Locale`: the language content is asked in, a BCP 47 tag. `Failure`: a `FailureKind`, the source's own `message`, the English `detail`.
- `Game`, `Offer`, `Standalone`, `Locator`: a game a source lists, with its store ids, `exe`, the title's other names (`terms`), art and offers. `Game.id`, `Offer.id` (the trainer) and `MapOffer.id` are the source's `Ref`s.
- `StoreId`: a launcher's id for a game, `<store>:<id>`, the one place that splits one (`store`, `id`, `steam_app`). `Installation`: a game on this machine (`Origin`: a launcher's `StoreId` or the picked executable, location, build).
- `Chord`, `Held`, `mouse`: a hotkey in Wand's key codes, its names (`Display`), the keys being pressed.
- `Process`, `Rule`: the game's process and how a trainer says which one to look for.
- `Trainer`, `Cheat`, `Kind`, `Args`, `Plan`, `Launch`: the cheat list, its `notes` (markdown, read before playing) and what running exactly this list takes; `Launch` says who starts the game: the user, the run, or the run only (`Required`: a game already running is refused).
- `Runner`, `Attach`, `Live`, `Session`, `Event`: a run. `prepare`, then `attach(Process, build)` (the install's `build`, the game's version the trainer is told), then set values and stop. `Attach::before_start(origin, location)` readies the install for a start; dropping its value undoes it without blocking the dropping thread.
- `Canvas`, `Layers`, `CanvasEvent`, `Rect`, `Texture`, `Cursor`: the overlay inside one game. `Layers` calls queue and return at once (they come from inside a draw); an `interact` layer takes the game's input over its rect, `block` all of it; `events` says `Ready`, then `Resized` (the game's frame, also its known size right after `Ready`), `Input` (a Win32 message, its point relative to the layer), and ends when it can no longer draw; dropping the `Canvas` closes its layers, the overlay stays in the game. A `Rect` is the game's client pixels; a `Texture` a shared handle with the adapter it lives on.
- `Installed`, `Approval`, `Setting`, `SettingKind`, `Value`: an installed plugin as the UI shows and drives it (`plugins` makes them; a setting's texts are its catalog's messages).
- `Map`, `MapOffer`, `TileSet`, `Tiles`, `Frame`, `Pin`, `Category`, `Group`, `Icon`, `Glyph`, `Region`, `Label`, `Picture`, `Level`, `PIN_LINK`: a game's map.

## Rules
- A snapshot stream yields `Cached(Option)` first, then `Fresh(Result)` only when the copy was missing or stale. An `Err` keeps the cached value: a consumer never clears on it.
- `Rule::matches` is case-insensitive. `image` is a whole-name path suffix (`*` inside one name); without it the install folder is a prefix at a `\` boundary. The glob stays iterative and bounded: the pattern is remote data.
- `Process` is pid and creation time together. `Attach::attach` refuses a process that is no longer the one given.
- `Runner::prepare` runs before the game is looked for and refuses what cannot run.
- `Plan::runner` runs exactly the cheat list of the `Trainer` that holds it. An implementer builds both from one value.
- A source answers in the closest locale it has; its copy in another locale is stale. Ids stay across locales.
- `Cheat::id` is unique in its trainer. `category` is its section's id; `category_name`, when the source has words for it, is what shows. `key` may be shared, or `None`: it then sets nothing.
- `Kind::slots` is the hotkey slot count (toggle and button 1, number, slider and selection 2: decrease then increase, `Unknown` 0). `Cheat::hotkeys` is indexed by slot with `None` for unbound, and may be shorter than the slots: read a missing slot as unbound.
- `Args::clamp` never panics: trainers send `min > max`. Do not replace it with `f64::clamp`.
- Dropping a `Session` ends the run. `Live::events` ends with the session. `position` and `teleport` use the game's own coordinates and are valid only when the trainer says so.
- `Group::id`, `Pin::id` and `Region::id` are stable across revisions: the user's state refers to them. A `revision` changes whenever its content does: staleness rests on it.
- A pin description links to another pin with `PIN_LINK`. A source rewrites or drops its own link forms.
- A `Frame` is valid: `axes` a permutation of 0, 1, 2, `scale` never zero.
- `Map.tile_sets` is never empty; `Map::tile_set` falls back to the first. `Tiles`: `min_zoom <= max_zoom <= Tiles::MAX_ZOOM`; `bounds` excludes its right and bottom edges. `Region.outlines` holds outer rings only.
- `set_folder` calls are ordered: an older one never saves over a newer.
- A crate marks an error at its origin with `.context(Failure::new(kind))`; `From<anyhow::Error>` finds the mark at the contract, else `Internal`. `detail` goes to the log only. A source names no UI place: the kind's text does.
- No I/O here, and no source name or UI text except the chord names.
- `model` names no crate of ours.

