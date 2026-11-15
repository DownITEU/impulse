---
paths:
  - "crates/agent/**"
---

# agent

The game's process: find it, check what may enter it, load a DLL into it, start an export. Windows, x64 targets only.

## Surface
- `find(&Rule)`: the matching process that is old enough, the one with the biggest working set (blocking). Age and working set never leave the crate.
- `wait(&Rule, within)`: polls `find` once a second; needs no runtime; dropping it stops within one poll. Its timeout is marked `GameNotFound`.
- `of(pid)`: the identity of one process now (blocking), `None` when it cannot be looked at.
- `Image::open(path, publisher)`, `Image::path`: the only way to name a DLL that enters a game.
- `load(&Process, &Image, export, arg)`: runs `LoadLibraryW` in the target, then starts `export` with a copy of `arg`.
- `Image::unchecked`: tests only.

## Rules
- A process is its `Process` (pid, image path, creation time), never a pid alone: a reused pid has another time. Every remote operation first opens a handle and calls `process::verify` on it. `load` does it before any remote step; new operations do the same.
- A DLL enters a game only as an `Image`: a `.dll` (`LoadLibraryW` appends the extension and would load a sibling), Authenticode-signed with `publisher` as the signer's display name, x64, held open without write or delete sharing while the value lives. Its path is the opened file's final path on a local drive. No constructor from a bare path.
- Revocation is not checked: it needs the network and a trainer must start offline.
- After a loader timeout `load` leaves the remote path buffer and keeps the file lock until exit (`keep_locked`): the remote thread may still open the file. The argument copy stays in the target too: the thread reads it after `load` returns.
- No handle (`Handle`, `HANDLE`) crosses the interface: only data (`Process`, `Image`, bytes, strings), so it can later run as a helper in a Proton prefix.
- `Image::unchecked` exists only under `cfg(test)` or the `test-support` feature, which only dev-dependencies enable.
- Protected processes that cannot be opened are skipped, not errors.
- `find`, `of`, `Image::open` and `load` block: callers use `blocking::unblock`.

## Traps
- The module is found by its full path through `EnumProcessModulesEx`. A Toolhelp snapshot fails when any module path exceeds MAX_PATH. The remote thread's exit code is only half the module handle.
- `LoadLibraryW`'s address is ours: kernel32 maps at one address in every process of a boot, which holds for x64 targets only, so `load` refuses a 32-bit one.
- Image paths compare case-insensitively; creation time is in 100 ns ticks.

## New code
- Memory operations go in `agent::memory` (plan phase 5): data in, data out, identity-checked like `load`.
