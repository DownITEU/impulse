# Architecture

## Layers

- `app` composes; `shell` is the UI; `wand` is a provider behind `model`; `play` runs a trainer on `model` alone; `agent` is the only way into a game's process; the leaves know no crate of ours.
- Dependencies point one way. The allowed edges are `GRAPH` in `crates/checks/src/graph.rs`; a change of an edge edits it in the same change.
- GPUI only in `shell` and `app`.
- Everything that talks to WeMod/Wand, names it or touches its files is in `wand`, and only `app` names `wand`. The UI and a run speak `model` types.

## Where new code goes

- A feature is a folder or a crate with one public surface; other code uses that surface, never its fields.
- One owner per piece of state; others hold projections of it.
- A reaction in another feature subscribes to an event; it is never a call someone must remember to make.
- A view renders and sends commands: it does not await a provider, write a file or call Win32.
- Operations on a game's process go in `agent`, with data in and data out; Win32 calls go in a leaf crate behind a data-only interface.
- Every file the app keeps is written through `disk::replace`; a missing or damaged file means the defaults. In `shell`: a `Stored<T>` at a path named in `Paths` (`storage.rs`).

## Standing goals

- Local first: the disk copy shows first, the network only refreshes it; nothing is downloaded ahead for a game the user did not open.
- Speed and memory first: no allocation or formatting per frame, no polling where an event exists, no thread without need. Measure in a release build, memory as the private working set (`scripts\ui.ps1` `mem:`).
