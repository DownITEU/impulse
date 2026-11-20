# Changelog

This file is the source of truth for release notes: the release workflow publishes the newest section and the app shows it.
The newest entry must match the version in `Cargo.toml`. A `-pre` build reads the section of its version.

## [0.1.1] - 2026-11-20

### Features

- Binding a hotkey shows its keys (cancel, default, unbind) above the cap. A chord another cheat already holds asks to KEEP BOTH or REBIND; one the overlay or the same cheat holds cannot be kept. Esc or a click aside drops the new chord.
- Hotkeys that two cheats of a game share are outlined in orange.

### Fixes

- The update check no longer fails with a gzip error from the second check on.

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
