---
paths:
  - "crates/launchers/**"
---

# launchers

The installed games, from each launcher's own records, and the start of a game.

## Surface
- `launchers::scan(&[Game], &[Custom])`: every install, then the picked executables (blocking).
- `launchers::owners`, `launchers::install`, `Custom`: the games a picked executable belongs to, and the install it gives a game (blocking).
- `launchers::page(&StoreId)`: the launcher's name and the URI of the game's page.
- `launchers::store_link(&[String])`: the page to open for a game the user does not have (blocking: the registry); `store_name`: a store id's prefix as the app prints it (XBOX, EA, UBISOFT, else upper case).
- `launchers::start(&Origin)`: starts a Steam app or a picked executable (blocking).

## Rules
- launchers: each launcher module's `apps()` returns `App` (the store id it mints, location) from local records. `scan` maps them to games by lower-cased store id (editions share one) and keeps one launcher install per game, of the store id the game's record lists first (Wand's preference), its picked one after it.
- Every `Installation` is made by `installation`: the build is the PE timestamp of `location\exe`, `None` when unreadable.
- Only plain ids reach a URI in `page`: digits for Steam and GOG, letters, digits and `._-` for UWP. Ids come from local records.
- `store_link` takes links from a remote catalog: a store app's page route (`APP_PAGES`) only while Windows has a handler for its scheme (`HKCR\<scheme>` with `URL Protocol`), else an `https://` page on a store's host (`WEB_STORES`); never another scheme, route or host, nor one with a space, a quote, a backtick or any of `<>\^{|}`.
- Only `xbox`, `ea` and `standalone` read the games. The others work from local records alone.
- Launchers' files and registry are only read. Registry views: launchers write under `WOW6432Node` (`View::Wow32`), packaged apps are in the 64-bit view.
- A picked executable that cannot be opened is `GameNotFound`.
- Only Steam games (`steam://rungameid/<appid>`) and picked executables (started in their own folder) are started. Any other store returns `false`: a run waits for the user.
- Only a number reaches a `steam://` URI: the id comes from a remote catalog. A bad id is an error, never a launch.

## Traps
- A Steam start returns once the URI is handed over, not when the game runs: the wait after it is needed.
- `ShellExecuteW` returns a value above 32 on success.

## New code
- A new launcher is one module with `apps()`, added to the list in `scan` through `timed`: each launcher's time and the builds' reads go to the debug log, where a slow scan shows.
