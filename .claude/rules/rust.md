---
paths:
  - "crates/**/*.rs"
---

# Rust

- Errors: `anyhow` with `context` inside a crate. A message names what failed in plain words; it never holds a token, a header, a body or script text.
- Remote and disk data never panic the app: an unreadable file means the defaults or the cached copy, a bad field in a remote file is skipped.
- Blocking work (network, waits, registry, folder scans, large files) runs through `blocking::unblock`, never on GPUI's main thread or the WebView2 thread. The UI's tiny JSON stores are the exception: saved in place and in order (`shell` rules).
- `unsafe` covers the one call and has a `// SAFETY:` comment saying why it holds. A Win32 handle is owned by a type that closes it on drop.
- Logs: the `log` macros, English, one line. Debug for our detail, Warn for what the user may notice.
- Comments say why, not what, in one or two lines. Doc comments on items other modules use. No commented-out code.
- Tests sit in the file under `#[cfg(test)] mod tests` (a sibling `tests.rs` when long). A test's name states the behaviour (`a_reused_pid_is_refused`). A hand-run end-to-end test is `#[ignore]` and says in its doc what it needs.
- Dependency versions only in the root `[workspace.dependencies]`; a crate names the features it uses. Prefer a crate already in `Cargo.lock`.
- Formatting is `cargo fmt`; `scripts\check.ps1` passes before work is called done.
