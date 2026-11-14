---
paths:
  - "crates/{launchers,play}/**"
---

# launchers and play

`launchers`: the installed games, from each launcher's own records. `play`: a whole run on `model` alone, from the prepared trainer to the attached session.

## Surface
- `launchers::scan(&[Game], &[Custom])`: every install, then the picked executables (blocking).
- `launchers::owners`, `launchers::install`, `Custom`: the games a picked executable belongs to, and the install it gives a game (blocking).
- `launchers::page(&StoreId)`: the launcher's name and the URI of the game's page.
- `launchers::start(&Origin)`: starts a Steam app or a picked executable (blocking).
- `play::run`, `Request`, `Stage`: one run; `Stage` is Preparing, Launching, WaitingForGame, Attaching.

## Rules
- launchers: each launcher module's `apps()` returns `App` (the store id it mints, location) from local records. `scan` maps them to games by lower-cased store id (editions share one) and keeps one launcher install per game, its picked one after it.
- Every `Installation` is made by `installation`: the build is the PE timestamp of `location\exe`, `None` when unreadable.
- Only plain ids reach a URI in `page`: digits for Steam and GOG, letters, digits and `._-` for UWP. Ids come from local records.
- Only `xbox`, `ea` and `standalone` read the games. The others work from local records alone.
- Launchers' files and registry are only read. Registry views: launchers write under `WOW6432Node` (`View::Wow32`), packaged apps are in the 64-bit view.
- play: `run` names no source. Order: `Runner::prepare` first (a trainer that cannot run is refused before any game starts), then `agent::find`, else start and `agent::wait`, then `attach` with the process exactly as found.
- The wait is 45 s after a launch of ours, 180 s otherwise (the user starts the game, or the store cannot be started from here).
- Only Steam games (`steam://rungameid/<appid>`) and picked executables (started in their own folder) are started. Any other store returns `false`: the run waits for the user.
- Only a number reaches a `steam://` URI: the id comes from a remote catalog. A bad id is an error, never a launch.
- Blocking calls (`agent::find`, `launchers::start`) go through `unblock`. Dropping the `run` future abandons the run; a trainer already inside ends with its session.

## Traps
- A Steam start returns once the URI is handed over, not when the game runs: the wait after it is needed.
- `ShellExecuteW` returns a value above 32 on success.

## New code
- A new launcher is one module with `apps()`, added to the list in `scan`.
