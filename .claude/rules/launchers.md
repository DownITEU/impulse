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
- `launchers::start(&Origin, location, app, arguments)`: starts the game through its launcher, or a picked executable (blocking); `arguments` are the trainer's for the game.

## Rules
- launchers: each launcher module's `apps()` returns `App` (the store id it mints, location) from local records. `scan` maps them to games by lower-cased store id (editions share one) and keeps one launcher install per game, of the store id the game's record lists first (Wand's preference), its picked one after it. As Wand's installed gate, a launcher install whose version file gives no build (none in the record, missing, unreadable) is not one; a picked one always is.
- Every `Installation` is made by `installation`: the build is `version::build` of `location\exe` as Wand reads each kind (PE timestamp; `.p7x` RFC 3161 countersignature time; `.zip`/`.jar` first entry's DOS date as UTC), `None` when unreadable; for an Xbox package an unreadable file gives its last write time, a missing `.p7x` the manifest's; the location's final path (links resolved, without the `\\?\` prefix) joins its `alternates` when it differs, as Wand resolves it before a match. `xbox` adds each package's `InstalledLocation`, `MutableLink` and `MutableLocation` (`StateRepository\Cache\Package\Data`, 64-bit view). A `uwp:` origin's `app` is its game's `package_apps` entry.
- Only plain ids reach a URI in `page`: digits for Steam and GOG, letters, digits and `._-` for UWP. Ids come from local records.
- `store_link` takes links from a remote catalog: a store app's page route (`APP_PAGES`) only while Windows has a handler for its scheme (`HKCR\<scheme>` with `URL Protocol`), else an `https://` page on a store's host (`WEB_STORES`); never another scheme, route or host, nor one with a space, a quote, a backtick or any of `<>\^{|}`.
- Only `xbox`, `ea` and `standalone` read the games. The others work from local records alone.
- Launchers' files and registry are only read. Registry views: launchers write under `WOW6432Node` (`View::Wow32`), packaged apps are in the 64-bit view.
- A picked executable that cannot be opened is `GameNotFound`, a launcher that cannot be started `LaunchFailed`.
- `start` makes Wand's command per store (`start.rs`, its tests hold each); Steam first takes the launch option `appinfo.vdf` describes as without anti-cheat (`appinfo.rs`). A picked executable starts in its own folder. EA and standalone games are not started (a standalone launcher file is the catalog's: never run), nor is one whose launcher is missing or whose URI no program handles: `false`, a run waits for the user.
- Only plain ids reach a URI or an argument: digits for GOG and Ubisoft, also `_` for Rockstar and `._-` for a package family; Epic's is percent-encoded; a GOG folder with a quote is refused. A launcher runs by its absolute path from its registry key (where Wand looks), through `ShellExecuteW` with the arguments apart: nothing a shell parses.
- Only a number reaches a `steam://` URI: the id comes from a remote catalog. A bad id is an error, never a launch.
- The trainer's start arguments are remote text, as Wand passes them on: a control character in ones a command line gets is an error. Steam: the anti-cheat-off option wins and drops them; ones starting with `-` or `/` run `steam.exe -applaunch <id> <arguments>` in Steam's folder; any other must be a plain word (letters, digits), Steam's route `steam://launch/<id>/<word>`. Ubisoft: ` -upc_arguments <base64>` after the id, a byte per character as Wand's `btoa` (Latin-1 only, else an error). A picked executable gets them as its arguments. Epic, GOG, Rockstar and Xbox take none. Never through a shell, so nothing is escaped (Wand's cmd `^&`).

## Traps
- A start returns once the shell has it, not when the game runs: the wait after it is needed.
- `ShellExecuteW` returns a value above 32 on success.

## New code
- A new launcher is one module with `apps()`, added to the list in `scan` through `timed`: each launcher's time and the builds' reads go to the debug log, where a slow scan shows.
