# Changelog

This file is the source of truth for release notes: the release workflow publishes the newest section and the app shows it.
The newest entry must match the version in `Cargo.toml`. A `-pre` build reads the section of its version.

## [0.3.0] - 2026-11-22

### Features

- Games with an anti-cheat launcher start the way Wand starts them: through Wand's own launcher, so the trainer can attach. The game's launcher is put back right after. When the trainer must start the game itself and the game already runs, Impulse asks to close it first. (#9)
- A trainer's notes show above its cheats as READ BEFORE PLAYING, folded like a section. (#9)

### Fixes

- Wand sign-in no longer fails with "Failed to fetch". (#6, #11)
- Long dropdown lists scroll and stay inside the window, and a long option ends in "..." instead of running past the list. (#12)
- For games with an anti-cheat launcher, Impulse attaches to the game itself, never to the launcher. (#9)
- An attach the game's protection blocks says so, instead of a bare "Access is denied". (#9)
- A trainer that needs Wand's own way of loading says so when its attach fails, and the log names how. (#8)
- When Wand's service does not answer, Impulse asks to update Wand. (#5)
- Wand sign-in shows why it failed and gives feedback when you click Try Again. (#6)
- Sign-in errors in the log name the failed API request and distinguish a fresh sign-in from a session refresh. The log includes the HTTP status when WebView2 can read it. The connectivity check reports only whether the probe passed, not why the original request failed. (#6)

## [0.2.0] - 2026-11-21

### Features

- 32-bit games: their trainers load like the 64-bit ones. A trainer made for the game's other build says so.
- Games run as administrator: Impulse asks for administrator rights (UAC) to attach, as Wand does. Run as administrator itself, it needs no prompt and shows a shield in the title bar. A game whose protection keeps trainers out says so.
- From the second start, the game list shows at once from the last scan and refreshes in the background, instead of waiting for every launcher after a reboot.
- The log shows how long each launcher's scan takes.

### Fixes

- Older trainers signed with WeMod's earlier company name (Daring Development, such as Mad Max) load instead of being refused.
- A UAC prompt or the lock screen no longer leaves an error in the log.

## [0.1.1] - 2026-11-20

### Features

- Binding a hotkey shows its keys (cancel, default, unbind) above the cap. A chord another cheat already holds asks to KEEP BOTH or REBIND; one the overlay or the same cheat holds cannot be kept. Esc or a click aside drops the new chord.
- Hotkeys that two cheats of a game share are outlined in orange.

### Fixes

- The update check no longer fails with a gzip error from the second check on.
- A trainer whose cheats send an empty `args` array (Spyro Reignited Trilogy) loads instead of failing with "cannot download the cheat list".

## [0.1.0] - 2026-11-20

### Features

- Your installed games from Steam, Epic, GOG, Ubisoft, Xbox, Rockstar and EA, or any executable you add, matched to Wand's catalog.
- Trainers from your Wand account: launch, attach, every kind of control, pins, saved mods and named collections.
- Imported and custom hotkeys, firing anywhere or only while the game is in front.
- Interactive maps with pins, regions, search and found pins, and your position and heading while a trainer runs.
- An overlay over the game with movable docks (mods, map, minimap, session) and a minimap that screen capture can skip.
- A remote panel to control a trainer from your phone on the local network.
- Plugins (alpha) that add map and title sources.
- Ten languages, cue sounds, a tray icon and a log.
- Updates: Impulse checks for a new version on the stable or the pre-release channel and shows what changed. A version can be skipped, and the checks can be turned off.
