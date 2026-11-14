---
paths:
  - "crates/shell/**"
---

# shell

The GPUI UI crate: window root, page, trainer list, run, hotkey capture, tray, crash stop. Map and overlay have their own rules.

## Surface
- `init`, `Assets`, `Root::new`: all `app` calls. Everything else is private.
- `Root`: the window root; draws `Page` with `cached`, the open map as a modal over it.
- `Page`: the combined page, one `impl Page` block per region file (`header`, `sidebar`, `hero`, `pane`, `cheats`, `controls`, `run`, `boot`, ...).
- `Library` (`library/`): the game list, the picked executables and the open game; emits `LibraryEvent` (`Listed`, `Opened`, `Switched`).
- `Trainers` (`trainer/`): the sheets by game, the `Hold`, `games.json`; emits `TrainerEvent` (`Installed`, `Rebound`, `Set`). `Printed` is the cheat list's projection of the open game's sheet.
- `Runs` (`run/`): the run state, the session, the `Tracker`; `start`, `stop`, `set`, `step`, `press`, `apply`; emits `RunEvent` (`Live`, `Stopping`, `Ended`, `Pressed`).
- `Hotkeys` (`bindings/`: a `hotkeys` module would shadow the crate): `hotkeys.json`, the live run's hook, the capture; `capture`, `cancel`, `rebind`, `set_enabled`, `set_game_only`.

## Rules
- Colors and fonts come only from `theme.rs`; a new token goes there. The user's accent is the `Look` global: read it with `accent(cx)`, never pass it along.
- Use the `widgets.rs` primitives (`button`, `icon_button`, `toggle`, `choice`, `notice`, `tip`, `keycap`, `mono`, `tracked`); never a raw `div` styled as one.
- Logic without GPUI goes to `client` (its rules: `client.md`): app files are its `Stored<T>`, opened from `Paths` by the slice that owns them.
- `Settings` (`settings/`) owns the prefs: read them with `settings.read(cx).prefs()`, change them with `Settings::edit`. It sets `Look` and GPUI's reduce motion; whoever reacts to a pref observes it.
- `Library` owns the open game: open one with `Library::open`, react to `LibraryEvent`. A view holds the open game as an `Arc<Game>` while it draws.
- `Trainers` owns the sheets and the profiles: read them through it, change them with its commands. It never knows the run: a run holds its game's copy with `hold`.
- Provider streams: the catalog in `Library`, trainers in `Trainers`, sign-in in `boot.rs`. `Cached(Some)` shows at once; `Fresh(Err)` never empties a cached copy (it marks it `stale`); a library error shows only with no copy.
- `Trainers` loads the open game's trainer only: nothing is downloaded for a game the user did not open (the API cuts a client off after about ten trainer requests).
- A starting or live run carries its game's `Hold`: a newer copy waits until it drops, then shows with the pins, folds and hotkeys set meanwhile. The release lands on the next executor turn.
- `Runs` owns the run: views read it and send its commands; whoever reacts to a run subscribes to `RunEvent` (`crash::watch`, `Layer`, the page's `run_changed`). It stops a run whose game left the list.
- `Printed` is made on `Opened` and `Installed` of the open game, re-capped on `Rebound`; a number field follows `Set`. The cheat-list helpers take `&Context<Page>`, so they read the sheet in place.
- One run at a time: `Runs::start` refuses while `is_active`. `Stopping` lasts until `Session::stop` returns.
- A cheat value goes one way: `Runs::set`, `step` or `apply` change the live sheet through `Trainers` and send it; they do nothing without a live run. A run's time and mods are kept once (`closed` flag), on end, STOP, quit and release.
- Every live session is followed by `crash::watch`: a UI-thread panic stops it before the process ends.
- Quit: `quit` drops the tray icon first (quitting skips drops), stops a live trainer, then quits on `RunEvent::Ended` or after 5 s. Closing the window follows `quit_on_close`: quit, else hide in the tray.
- `Hotkeys` arms the live run's hook on `RunEvent`s and lays the user's choices over each copy on `Installed`. It never names the overlay: the overlay's keys reach it as the `reserved` test it is built with.
- A capture's outcome goes to its asker (`capture` returns a receiver), who applies it: the cheat list with `Hotkeys::rebind`, the overlay tab with the layer's setters. A capture hides keys from every app; it ends on its outcome, a click outside the cap, the window going inactive, another game opening, a copy of its game's trainer showing, the overlay opening or the run's hook arming.
- A hook handler only forwards to a channel; no GPUI call, no lock.
- Every header control calls `occlude()`: GPUI picks the window control area by the first hitbox under the cursor.
- What repaints often is its own entity (`LogView`, `Resources`, the map parts); never `cx.notify()` the page from a per-frame or per-record path.

## Traps
- `Page` is drawn cached: it re-renders only when it notifies, so an entity it reads must be observed. The session clock notifies it once a second while a run is live. A changed global reaches cached views only through `cx.refresh_windows()`.
- `Menu::Cheat(i)` and `Flash::Cheat(i)` hold a cheat index: `drop_cheat_state` whenever the trainer or game changes. A capture targets a cheat by id, not index: a newer list may arrive meanwhile.
- `reload` (caches cleared) stops the run first: it keeps what the run leaves on, from trainers about to go.
- `tracked` lays out one element per `char`: short captions only. `keycap` and `fitted` cut text only inside a parent with `min_w_0`.

## New code
no new `impl Page` block or `Page` field. New state goes into its own entity with events; a view renders and sends commands only; a reaction across slices subscribes to an event, never a hand-called `sync_*` (those are being removed).
