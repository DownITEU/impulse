# Impulse

A native Windows client for your own Wand (WeMod) account and install: your installed games, their trainers and maps in a fast native window, with an overlay over the game.

Impulse works with the Wand you already have: it signs in through Wand's session, takes the trainer runtime from Wand's folder and trainers from WeMod's CDN, and ships no WeMod binaries. It keeps what it downloads under `%LOCALAPPDATA%\Impulse`: what you opened once shows offline, and a trainer's files are downloaded on its first launch.

## Features

- **Library** - installed games from Steam, Epic, GOG, Ubisoft, Xbox, Rockstar, EA and standalone installs, matched to WeMod's catalog; a game can also be added by its executable.
- **Trainers** - a game's cheat list, kept on disk; launch and attach, every control kind, pins, folded sections, a filter.
- **Saved mods and collections** - a run can start with the mods the last one left on; named sets of values apply in one click.
- **Hotkeys** - Wand's bindings and your own, firing anywhere or only while the game is in front.
- **Maps** - interactive maps with pins, regions, search, found pins, your own pins, pictures and clips; the player's position and heading while a trainer runs, and teleport.
- **Overlay** - a hotkey turns the window into a layer over the game, with movable docks: mods, map, minimap, session.
- **Minimap** - shown over the game while it is in front, hidden from screen capture on request.
- **Tray** - closing hides Impulse in the tray; quitting stops a live trainer first.
- **Resources** - the size of every cache, clearing, a limit for media.
- **Log** - the app's log with levels, a filter and copy.

## Requirements

- Windows 10 or 11, x64; Wand (WeMod) installed and signed in.
- To build: Rust (edition 2024) and the Windows SDK (`rc.exe`; `fxc.exe` for release builds).

## Build

```
cargo build              # debug: target\debug\Impulse.exe
cargo build --release    # release: target\release\Impulse.exe
scripts\check.ps1        # formatting, clippy and every test
```
