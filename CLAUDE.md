# Impulse

Native Rust + GPUI client for the user's own Wand (WeMod) account and install. Architecture: `docs/architecture.md`.

## Commands

- `cargo build` - debug build (shaders compile at runtime; release needs `fxc.exe` from the Windows SDK)
- `cargo test`, `cargo clippy --all-targets -- -D warnings`; `cargo test -p <crate>` for one crate (non-UI crates build without GPUI)
- `scripts\ui.ps1 -Steps 'click:1016,25;wait:800;shot:out.png'` - starts the debug exe, drives it with posted window messages (`wait`, `move`, `click`, `text`, `shot`; client pixels) and captures it with `PrintWindow`, then kills it. No real cursor, focus or screen capture; refuses to run while Impulse is open. Build first with `cargo build`.

## Invariants

- Never ship or commit WeMod binaries (TrainerLib, trainer DLLs, Tophat).
- WeMod/Wand code lives in `crates/wand`, and its rules are in `crates/wand/CLAUDE.md`. Read them before touching anything that reaches Wand, its API, its files or a game process.
- Never log tokens, auth codes or auth response bodies.
- Keep provider research out of Git.
