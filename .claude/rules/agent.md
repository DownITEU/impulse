---
paths:
  - "crates/agent/**"
---

# agent

The game's process: find it, check what may enter it, load a DLL into it, start an export. Windows, x64 and x86 (WOW64) targets.

## Surface
- `settled(&Rule)`: the matching process its `Pick` names running now, waited for while younger than `minimum_age`; `None` once nothing matches. Ranked before the age is checked: a small old helper in the folder is not taken for a game still starting up. With `Target::arguments`, only a process whose path matches has its command line read, through a verified handle; one unreadable does not match. Age, working set and command lines never leave the crate.
- `wait(&Rule, within)`: polls once a second until that process is old enough, `None` past `within`; needs no runtime; dropping either stops it within one poll.
- `of(pid)`: the identity of one process now (blocking), `None` when it cannot be looked at.
- `x64(&Process)`: whether it is a 64-bit process, asked of the process (blocking), never of its exe: a GDK game's is unreadable.
- `signaled(name, within)`: waits for a named event a module in a game sets (blocking): `false` when none of that name is held, an error when it is not set in time.
- `exit(&[Process])` (blocking: opens each) → `Exit`, a future of the first to exit, on any executor: the system thread pool's waits (`RegisterWaitForSingleObject`), no thread of ours while it waits; the wake goes through the blocking pool (one inline in the callback could drop the `Exit` there and deadlock its unregister); dropping it unregisters them. One gone already or whose pid is another's has exited; a denied open is `Protected`. Its handles stay inside.
- `Image::open(path, publishers)`, `Image::path`, `Image::x64` (else x86): the only way to name a DLL that enters a game.
- `signed(path, publishers)`: the signature check alone (blocking), for a copy the caller made and holds. `Program::open(path, publishers)`, `Program::path`: an `.exe` that runs on its own and never enters a game, locked and resolved like an `Image` (a start appends `.exe` to a bare name too); start it by `path`. Its signature must be embedded: system executables are catalog-signed.
- `idle(&Process, within)`: `WaitForInputIdle` (blocking); `false` past `within` or on any `WAIT_FAILED` (no GUI), as Wand. `suspend`, `resume(&Process)`: `NtSuspendProcess` / `NtResumeProcess` (ntdll, raw-dylib), each a count: one resume per suspend. A denied open is `Protected`; they never elevate.
- `load(&Process, &Image, export, arg)`: runs `LoadLibraryW` in the target, then starts `export` with a copy of `arg`.
- `map(&Process, &Image)`: the same checks and `LoadLibraryW` run, no export started: for a DLL whose `DllMain` does the work (an overlay). Elevates as `load` does (Wand's service does too).
- `elevated()`: this process runs as administrator (`app` marks it in the title bar).
- `serve_elevated(publishers)`: first thing in the host's `main`. `Some(exit code)` when the command line asks for an elevated load; else makes this executable the one a denied `load` starts elevated.
- `Image::unchecked`: tests only.

## Rules
- A process is its `Process` (pid, image path, creation time), never a pid alone: a reused pid has another time. Every remote operation first opens a handle and calls `process::verify` on it. `load` does it before any remote step, then refuses an image of the other architecture; new operations do the same.
- A DLL enters a game only as an `Image`: a `.dll` (`LoadLibraryW` appends the extension and would load a sibling), Authenticode-signed with one of `publishers` as the signer's display name, x64 or x86, held open without write or delete sharing while the value lives. Its path is the opened file's final path on a local drive. No constructor from a bare path.
- Revocation is not checked: it needs the network and a trainer must start offline.
- After a loader timeout `load` leaves the remote path buffer and keeps the file lock until exit (`keep_locked`): the remote thread may still open the file. The argument copy stays in the target too: the thread reads it after `load` returns.
- No handle (`Handle`, `HANDLE`) crosses the interface: only data (`Process`, `Image`, bytes, strings), so it can later run as a helper in a Proton prefix.
- `Image::unchecked` exists only under `cfg(test)` or the `test-support` feature, which only dev-dependencies enable.
- Protected processes that cannot be opened are skipped by the search, not errors.
- A process that denies `load` or `map` (a game run as administrator) is loaded by the host executable started elevated through UAC (`runas`, the request in hex on its command line; an empty export is a map), only when a host is set and this process is not elevated. The copy checks the image against the host's publishers, never the request's choice, and the process, like any load.
- Marks: a declined prompt is `Elevation`, denied even elevated `Protected`, as is a denial once the handle is open (allocation, write, remote thread: the game's protection stripped its rights, elevation does not lift that), an image of the other architecture `Architecture` (`needs_x64`: the image is x64). A failure after the copy may have started its loader keeps the parent's image lock: the copy's ends with it.
- `of`, `Image::open` and `load` block: callers use `blocking::unblock`.

## Traps
- The module is found by its full path through `EnumProcessModulesEx`. A Toolhelp snapshot fails when any module path exceeds MAX_PATH. The remote thread's exit code is only half the module handle.
- `LoadLibraryW`'s address is ours in an x64 target: kernel32 maps at one address in every x64 process of a boot. A WOW64 target's is its 32-bit kernel32's base plus the RVA in the SysWOW64 file; that module is recorded under System32 (redirected), so it is found by file name.
- Image paths compare case-insensitively; creation time is in 100 ns ticks.
- `ShellExecuteExW` with `runas` returns once the prompt is answered; `ERROR_CANCELLED` is a declined one.

## New code
- Memory operations go in `agent::memory` (plan phase 5): data in, data out, identity-checked like `load`.
