# Forked crates

Copies of crates.io releases with Impulse's changes, used through `[patch.crates-io]` in the root `Cargo.toml`. Examples, tests and benches are left out.

- `gpui-pre` 0.3.12 - `paint_surface` and the window's D3D11 device on Windows.
- `gpui-pre-windows` 0.3.12 - `draw_surfaces`: NV12 textures drawn by a shader; the device is made with video support and multithread protection, so Media Foundation decodes on it. A fullscreen window ignores the minimum size, so it fills a monitor smaller than that, and keeps filling its monitor when the DPI changes. A `PopUp` window is `WS_POPUP`, with no frame or rounded corners.

To update one, copy the new release over it and apply the changes again: `git diff` against the commit that added the release shows them.
