# Impulse

Native Rust + GPUI client for the user's own Wand (WeMod) account and install. Architecture: `docs/architecture.md`.

## Commands

- `cargo build` - debug build (shaders compile at runtime; release needs `fxc.exe` from the Windows SDK)
- `cargo test`, `cargo clippy --all-targets -- -D warnings`

## Invariants

- Never ship or commit WeMod binaries (TrainerLib, trainer DLLs, Tophat). They are read from the user's Wand install or the WeMod CDN at runtime.
- Use Wand's access token only to mint `/v3/auth/overlay_code`. Never use Wand's refresh token, and never call `/v3/auth/remote_code` (it signs the desktop out).
- All WeMod API traffic goes through the hidden WebView2 `file://` bridge: the API allows CORS only for the `null` origin, and Cloudflare challenges non-browser clients.
- Never log tokens, auth codes or auth response bodies.
- Keep provider research out of Git.
