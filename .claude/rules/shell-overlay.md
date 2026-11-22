---
paths:
  - "crates/shell/src/layer/**"
---

# shell overlay

The overlay slice, `layer/` (an `overlay` module would shadow the crate): `Layer` (window mode), `Hud` (`hud.rs`), `Glance` (`glance.rs`), `Docks`, the overlay settings tab.

## Surface
- `Layer`: `toggle`, `leave`, `is_over`, `exclusive`, `hotkey`, `key(Slot)` (the extra hotkeys: minimap, zoom in, zoom out), `takes` (the reserved test `Hotkeys` gets), setters. Emits `Playing` and `Pressed(Slot)`. It observes `Settings` to hide the main window from capture.
- `Glance`: the HUD minimap window; it observes `Settings` for capture exclusion.
- `Docks`, `Spec`: floating panels; `DOCKS` and `MINIMAP_DOCK` live in `hud.rs`.
- `Hud`: the overlay's view, drawn by the page in place of itself while `Layer::is_over`. It makes the `Docks`, the dock's `Minimap`, the `Glance` and the `Warning` (`warning.rs`), and emits `Entered` when the window goes over a game.

## Rules
- Hotkey, foreground events, activation loss and UI asks are `Ask`s, run one at a time by the one `switch` task. Win32 calls that move, show, raise, lower or cloak Impulse's own window run there (or in `Glance`'s call task), never inside a GPUI update: they run GPUI's callbacks at once.
- The window stays cloaked until a frame of the new layout shows (`reveal`: two next-frame callbacks, 500 ms at most).
- `Playing` is emitted on change only. `Front` needs the foreground hook and is never set while over. It reads the live pid from `Runs` and rechecks on every `RunEvent` but `Pressed`.
- The extra hotkeys (minimap, zoom in, zoom out: `Slot`) are registered only while the live game is in front or under the overlay (`hold_keys`). `set_hotkey` registers the new key before the old watch drops, so a refused key keeps the old one; the old watch takes the extras with it, so hold them again. The zoom keys step the live map by `ZOOM_STEP` and do nothing without one.
- The overlay's and the extras' keys are bound by asking `Hotkeys` for a capture (`Target::Key`) and applying its outcome here: refused when another of them or a cheat's chord holds it. `Layer::takes` keeps trainer chords off them.
- `exclusive` is read once per entry, before our window covers the game (`overlay::exclusive`: Windows answers for the whole session and a few seconds late, so only for the game in front). `Warning` asks on `Entered` unless `skip_exclusive_warning` (its DON'T SHOW AGAIN button sets it and closes; a switch in the OVERLAY tab turns the question back on), and the chip under the title banner stays while `exclusive` (the title and chip are taller than the top bar, so the group is top-aligned; the docks start below both); it hands the focus back to the `Hud` on `Closed`.
- `Glance` is a `PopUp` that never takes input: `focus: false`, `show: false`, `pass_through` before the first show. It is placed over the MINIMAP dock's minimap on the game's monitor, hidden (tiles dropped) when the live map is not placed, closed on `Playing::Idle` and on release. It honours `hide_from_capture`.
- Entering the overlay closes the cheat list's menu and editor and the capture, selects the live game and emits `Entered`; the page closes its own parts on it (sources menu, picker, settings, log). The `Hud` draws the picker that the NO TRAINER card opens and takes the focus back when it closes. Escape closes the editor, then the menu, then leaves.
- The map in the MAP dock is the page's `MapView`, shown while `Maps::mapped`; the MINIMAP dock needs a record's map. Over a game the `Hud` opens and hides it (`place_map`) on each `Docks` and `Maps` change, so it does not depend on who hides it first (`MapView` hides itself when another game opens); the focus must not stay in a gone map. Out of the overlay the open map is the page's modal. The overlay tells `Maps` whether the MINIMAP dock is shown (`set_minimap`), at start and on each `Docks` change.
- Dock layout is one for all games, in `docks.json`, saved blocking on toggle, raise and release; a drag's bounds stay in its `Grab` until the release. A missing or damaged file means each `Spec`'s first bounds. `fit` keeps a dock on screen.

## Traps
- `MINIMAP_DOCK` is `DOCKS[3]`: keep the order. A new dock is a `Spec` in `DOCKS` and an arm in `Hud::render`.
- Windows refuses `SetForegroundWindow` to a process without the last input: `take_input` shares the front window's input with `AttachThreadInput` for the call. The registered hotkey's press (not a hook's) lets the process take the foreground.
- Raise only after GPUI's fullscreen switch, or the layer's rect is saved for the way back. Restore first (`overlay::restore`): a minimized or hidden window has no rect, and GPUI's fullscreen switch skips a hidden one. The switch is set, never toggled blind: a toggle against a window that is not where Impulse thinks leaves it fullscreen with dead caption buttons. `lower` never goes behind a topmost window, or it turns topmost.
- Do not read the foreground from a foreground event: it may be late; `switch` reads `overlay::foreground()` itself.
- `give_back` restores a minimized game with `ShowWindowAsync`: a hung game must not hang the caller.
- Windows skips a process's keyboard hook while it is in front if the process registered raw input: tao's raw input stays off (`DeviceEventFilter::Always` in `wand`'s webview).
- Over the game the window is transparent, and the kit's `Root` paints a background too: `see_through` clears both.
