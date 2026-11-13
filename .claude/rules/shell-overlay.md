---
paths:
  - "crates/shell/src/{layer/**,docks/**,glance.rs,hud.rs}"
---

# shell overlay

The overlay: `Layer` (window mode), `Glance` (`glance.rs`), `Docks`, the HUD view (`hud.rs`).

## Surface
- `Layer`: `toggle`, `leave`, `recheck`, `is_over`, `hotkey`, `minimap_hotkey`, setters. Emits `Playing` and `MinimapKey`.
- `Glance`: the HUD minimap window; `set_hidden` for capture exclusion.
- `Docks`, `Spec`: floating panels; `DOCKS` and `MINIMAP_DOCK` live in `hud.rs`.
- `Page::hud`: draws the overlay in place of the page while `over`.

## Rules
- Hotkey, foreground events, activation loss and UI asks are `Ask`s, run one at a time by the one `switch` task. Win32 calls that move, show, raise, lower or cloak Impulse's own window run there (or in `Glance`'s call task), never inside a GPUI update: they run GPUI's callbacks at once.
- The window stays cloaked until a frame of the new layout shows (`reveal`: two next-frame callbacks, 500 ms at most).
- `Playing` is emitted on change only. `Front` needs the foreground hook and is never set while over. Call `recheck` when a run starts or ends.
- The minimap hotkey is registered only while the live game is in front or under the overlay (`hold_minimap_key`). `set_hotkey` registers the new key before the old watch drops, so a refused key keeps the old one; the old watch takes the minimap key with it, so hold it again.
- Overlay and minimap keys are rebound through the capture in `bindings` (`Target::Overlay`, `Target::Minimap`), which keeps them apart from trainer chords.
- `Glance` is a `PopUp` that never takes input: `focus: false`, `show: false`, `pass_through` before the first show. It is placed over the MINIMAP dock's minimap on the game's monitor, hidden (tiles dropped) when the live map is not placed, closed on `Playing::Idle` and on release. It honours `hide_from_capture`.
- Entering the overlay closes what belongs to the window (menu, editor, settings, log, capture) and selects the live game. Escape closes the editor, then the menu, then leaves.
- The map in the MAP dock is the page's `MapView`: `sync_map_dock` opens and hides it, and the focus must not stay in a gone map.
- Dock layout is one for all games, in `docks.json`, saved blocking on toggle and release. A missing or damaged file means each `Spec`'s first bounds. `fit` keeps a dock on screen.

## Traps
- `MINIMAP_DOCK` is `DOCKS[3]`: keep the order. A new dock is a `Spec` in `DOCKS` and an arm in `Page::hud`.
- Windows refuses `SetForegroundWindow` to a process without the last input: `take_input` shares the front window's input with `AttachThreadInput` for the call. The registered hotkey's press (not a hook's) lets the process take the foreground.
- Raise only after GPUI's fullscreen switch, or the layer's rect is saved for the way back. Unminimize first: a minimized window has no rect. `lower` never goes behind a topmost window, or it turns topmost.
- Do not read the foreground from a foreground event: it may be late; `switch` reads `overlay::foreground()` itself.
- `give_back` restores a minimized game with `ShowWindowAsync`: a hung game must not hang the caller.
- Windows skips a process's keyboard hook while it is in front if the process registered raw input: tao's raw input stays off (`DeviceEventFilter::Always` in `wand`'s webview).
- Over the game the window is transparent, and the kit's `Root` paints a background too: `see_through` clears both.
