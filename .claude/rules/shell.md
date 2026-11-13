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

## Rules
- Colors and fonts come only from `theme.rs`; a new token goes there. The user's accent is the `Look` global: read it with `accent(cx)`, never pass it along.
- Use the `widgets.rs` primitives (`button`, `icon_button`, `toggle`, `choice`, `notice`, `tip`, `keycap`, `mono`, `tracked`); never a raw `div` styled as one.
- An app file is a `Stored<T>` (`storage.rs`) opened from its `Paths` entry by the slice that owns it; a constructor takes `&Paths`, never the data folder. `edit` saves at once, blocking, in place and in order: tiny files only. Missing or damaged means the defaults, so a new field is `#[serde(default)]`.
- `Settings` (`settings/`) owns the prefs: read them with `settings.read(cx).prefs()`, change them with `Settings::edit`. It sets `Look` and GPUI's reduce motion; whoever reacts to a pref observes it.
- Provider streams are consumed in `boot.rs`. `Cached(Some)` shows at once; `Fresh(Err)` never empties a cached copy (it marks it `stale`); a library error shows only with no copy.
- `open_trainer` serves the open game only: nothing is downloaded for a game the user did not open (the API cuts a client off after about ten trainer requests).
- A newer trainer copy for a game with a run `Busy` or `Live` waits in `deferred` (`Play::holds`); every path that ends a run calls `install_deferred`.
- One run at a time: `start_run` refuses while `Play::is_active`. `Stopping` lasts until `Session::stop` returns.
- A cheat value reaches the trainer only through `push_value`. `close_session` keeps time and mods once per run (`closed` flag), on end, STOP, quit and page release.
- Every live session goes to `crash::follow`: a UI-thread panic stops it before the process ends.
- Quit: `quit` drops the tray icon first (quitting skips drops), stops a live trainer, then quits in `run_stopped` or after 5 s. Closing the window follows `quit_on_close`: quit, else hide in the tray.
- A hotkey capture hides keys from every app. It ends on its outcome, a click outside the cap, the window going inactive, another game opening or a run hook arming. `start_capture` calls `cancel_capture` first.
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
