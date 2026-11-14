---
paths:
  - "crates/{hotkeys,overlay,tray,video,journal,disk,fetch,pe}/**"
---

# Platform leaves

Small Windows and I/O crates with no dependency on our other crates but `model` (`hotkeys`). Each is data in, data out.

## hotkeys
- `Matcher` over `model::Chord` (matches as Wand does: longest chord wins, a chord bound twice fires both), `Capture`, `Hook`, `foreground_pid`.
- `Hook` runs on its own thread, never the tao/WebView2 thread: Windows removes a hook that stalls. Its handler returns at once (forward to a channel); true on a down hides it from every app.
- The mouse hook runs on every move: start it only when a chord `uses_mouse()`. A later hook sees keys first.

## overlay
- `Key` (modifiers and one key), `Watch` (one thread: `RegisterHotKey`, a foreground hook, the minimap key), and raw-HWND calls (`foreground`, `raise`, `lower`, `cloak`, `place`, `pass_through`, ...).
- `Watch`'s sink runs on the watch thread: forward to a channel. The calls take `isize` handles and are made outside GPUI updates.

## tray
- `Tray` lives on GPUI's main thread (its hidden window needs that loop); dropping it removes the icon. `set_menu` feeds the next right click.
- `claim()` is the single-instance mutex; a second start wakes the first and gets `false`. `hide` and `show` take the app window's HWND.
- Menu labels are literals in `pick`.

## video
- `Playback::start(file, device)`: MP4 (H.264) decoded on its own thread to NV12 textures on the window's D3D11 device, which must be multithread protected. `frame(elapsed)` never blocks.
- A `Frame`'s texture returns to the pool when dropped: hold none longer than needed. Dropping the `Playback` ends the thread.

## journal
- `init` first in `main`, before anything logs. `since`, `subscribe`, `closing` (call when the last window closes), `dir`.
- `OURS` lists the workspace crates (Debug up; others Warn up). A test fails when a crate is missing: add a new crate there.
- Messages are flattened to one line and the profile path is scrubbed.

## disk
- `replace(path, data)`: how every app file is written (temp file, rename). `size(path)`: a file's or folder's bytes, blocking.

## fetch
- `get(url, etag)`: blocking GET for public CDNs only (they send no CORS headers). The WeMod API goes through `wand`'s bridge. Body capped at 64 MiB, 120 s timeout. Run it off the UI thread.

## pe
- `header` (machine, timestamp) and `export` (RVA, forwarders skipped). `header` reads headers only: executables can be gigabytes.
