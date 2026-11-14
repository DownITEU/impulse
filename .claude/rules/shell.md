---
paths:
  - "crates/shell/**"
---

# shell

The GPUI UI crate: window root, page, trainer list, run, hotkey capture, tray, crash stop. Map and overlay have their own rules.

## Surface
- `init`, `Assets`, `Root::new`: all `app` calls. Everything else is private.
- `Root`: the window root; draws `Page` with `cached`, the open map as a modal over it.
- `Page` (`page/`): the combined page, layout only: one `impl Page` in `page/mod.rs` makes the entities and lays out their views; `header` and `status_bar` are components.
- `Hero` (`page/hero.rs`): the open game's art, title, run button and actions; owns the sources menu (`close`).
- `Sidebar`, `GamePicker` (`library/`): the game list with its filter and the add-game dialog. The page gives the sidebar its `Marks` (favorite, live) as a function and makes the sidebar observe what it reads.
- `Library` (`library/`): the game list, the picked executables and the open game; emits `LibraryEvent` (`Listed`, `Opened`, `Switched`).
- `Trainers` (`trainer/`): the sheets by game, the `Hold`, `games.json`; emits `TrainerEvent` (`Installed`, `Rebound`, `Set`). `title` draws a game's name and meta.
- `CheatList` (`cheats/`): the open game's cheat list, drawn by the page and the HUD; its `Printed` projection, filter, menus, flash and `CollectionEditor` (emits `EditorClosed`; whoever draws `editor()` takes the focus back).
- `Runs` (`run/`): the run state, the session, the `Tracker`; `start`, `stop`, `set`, `step`, `press`, `apply`, `can_start`, `is_live`; emits `RunEvent` (`Live`, `Stopping`, `Ended`, `Pressed`). Views: `run_button`, `session_panel` (the hotkeys' scope comes in as a slot: `run` cannot name `bindings`), `run_note`.
- `Hotkeys` (`bindings/`: a `hotkeys` module would shadow the crate): `hotkeys.json`, the live run's hook, the capture; `capture`, `cancel`, `rebind`, `set_enabled`, `set_game_only`. Views: `caps`, `hotkeys_scope`, `HotkeySettings`.
- `Account` (`account.rs`): the first source's session, read by the header; `reload` asks again.
- `Folders` (`settings/`): every source's folders and the user's edits checked by their source (`scan`, `set`); emits `Shown(at)` when a row's field must show the source's path. `SettingsView` holds the fields.
- `Resources` (`resources/`): sizes, clear, the media limit; clears the media (`app:` stores) itself and the rest through the registry; emits `Cleared`. `ResourcesView` draws it in the RESOURCES tab.

## Rules
- Colors and fonts come only from `ui/theme.rs`; a new token goes there. The user's accent is the `Look` global: read it with `accent(cx)`, never pass it along.
- A caption is a catalog message (`i18n.md`): `t(cx, key::X)`, never a literal. Captions move to the catalog slice by slice (`docs/refactor-plan.md` 7.1); new code writes none.
- Use the `ui/widgets.rs` primitives (and `ui/dialog.rs`, `ui/dropdown.rs`) (`button`, `icon_button`, `toggle`, `choice`, `notice`, `tip`, `keycap`, `mono`, `tracked`); never a raw `div` styled as one.
- Logic without GPUI goes to `client` (its rules: `client.md`): app files are its `Stored<T>`, opened from `Paths` by the slice that owns them.
- `Settings` (`settings/`) owns the prefs: read them with `settings.read(cx).prefs()`, change them with `Settings::edit`. It sets `Look`, `Texts` (the language) and GPUI's reduce motion; whoever reacts to a pref observes it.
- `Library` owns the open game: open one with `Library::open`, react to `LibraryEvent`. A view holds the open game as an `Arc<Game>` while it draws.
- `Trainers` owns the sheets and the profiles: read them through it, change them with its commands. It never knows the run: a run holds its game's copy with `hold`.
- Sources come only through the `Arc<Registry>` the entities hold. Its streams: the catalog in `Library`, trainers in `Trainers`, sign-in in `Account`. `Cached(Some)` shows at once; `Fresh(Err)` never empties a cached copy (it marks it `stale`); a library error shows only with no copy.
- `Trainers` loads the open game's trainer only: nothing is downloaded for a game the user did not open (the API cuts a client off after about ten trainer requests).
- A starting or live run carries its game's `Hold`: a newer copy waits until it drops, then shows with the pins, folds and hotkeys set meanwhile. The release lands on the next executor turn.
- `Runs` owns the run: views read it and send its commands; whoever reacts to a run subscribes to `RunEvent` (`crash::watch`, `Layer`, `Hotkeys`, `Maps`, `TrayIcon`). It stops a run whose game left the list.
- `Printed` is made on `Opened` and `Installed` of the open game, re-capped on `Rebound`; a number field follows `Set`. The cheat list reads the sheet in place through `Trainers`.
- One run at a time: `Runs::start` refuses while `is_active`. `Stopping` lasts until `Session::stop` returns.
- A cheat value goes one way: `Runs::set`, `step` or `apply` change the live sheet through `Trainers` and send it; they do nothing without a live run. A run's time and mods are kept once (`closed` flag), on end, STOP, quit and release.
- Every live session is followed by `crash::watch`: a UI-thread panic stops it before the process ends.
- `TrayIcon` (`systray.rs`) owns the icon, its menu (it observes `Runs`, `Library`, `Layer`), the close button and quitting. Quit: `quit` drops the tray icon first (quitting skips drops), stops a live trainer, then quits on `RunEvent::Ended` or after 5 s. Closing the window follows `quit_on_close`: quit, else hide in the tray.
- `Hotkeys` arms the live run's hook on `RunEvent`s and lays the user's choices over each copy on `Installed`. It never names the overlay: the overlay's keys reach it as the `reserved` test it is built with.
- A capture's outcome goes to its asker (`capture` returns a receiver), who applies it: the cheat list with `Hotkeys::rebind`, the overlay tab with the layer's setters. A capture hides keys from every app; it ends on its outcome, a click outside the cap, the window going inactive, another game opening, a copy of its game's trainer showing, the overlay opening or the run's hook arming.
- A hook handler only forwards to a channel; no GPUI call, no lock.
- Every header control calls `occlude()`: GPUI picks the window control area by the first hitbox under the cursor.
- What repaints often is its own entity (`LogView`, which also owns whether it is open; `ResourcesView`; the map parts); never `cx.notify()` the page from a per-frame or per-record path.

## Traps
- `Page` is drawn cached: it re-renders only when it notifies, so an entity it reads must be observed. The session clock notifies it once a second while a run is live. A changed global reaches cached views only through `cx.refresh_windows()`.
- The cheat list's menu, flash and editor hold an index: `CheatList::drop_state` whenever the trainer or game changes. A capture targets a cheat by id, not index: a newer list may arrive meanwhile.
- `reload` (caches cleared) stops the run first: it keeps what the run leaves on, from trainers about to go.
- `tracked` lays out one element per `char`: short captions only. `keycap` and `fitted` cut text only inside a parent with `min_w_0`.

## New code
`Page` only makes entities and lays out views. New state goes into its slice's entity with events; a view renders and sends commands only; a reaction across slices subscribes to an event, never a hand-called sync. An upward need comes as a slot or a function given at construction.
