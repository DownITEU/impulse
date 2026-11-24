# Forked crates

Copies of crates.io releases with Impulse's changes, used through `[patch.crates-io]` in the root `Cargo.toml`. Examples, tests and benches are left out.

- `gpui-pre` 0.3.12 - `paint_surface`, the window's D3D11 device and `mirror_frames` (each drawn frame copied into a KMT-shared texture, with its adapter's PCI device id) on Windows; `mirror_cursor` (the cursor the window wants on each change, hovered or not); `hold_active` (a window counted active while its keys come from elsewhere). `Anchored::attach_tracked`: a target read at prepaint, so a dropdown follows its trigger's bounds of the same frame.
- `gpui-pre-windows` 0.3.12 - `draw_surfaces`: NV12 textures drawn by a shader; the device is made with video support and multithread protection, so Media Foundation decodes on it. A fullscreen window ignores the minimum size, so it fills a monitor smaller than that, and keeps filling its monitor when the DPI changes. A `PopUp` window is `WS_POPUP`, with no frame or rounded corners. The vsync thread's redraw of a window destroyed but not yet unlisted logs nothing. The key code takes the keyboard layout as an argument (`ToUnicodeEx`, `MapVirtualKeyExW`; its own windows pass this thread's), and `keystroke_in` makes the keystroke of a key typed into another thread's window, in that thread's layout with a given key state.

To update one, copy the new release over it and apply the changes again: `git diff` against the commit that added the release shows them.
