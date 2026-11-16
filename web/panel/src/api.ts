// Generated from crates/client/src/api.rs by its tests; `IMPULSE_BLESS=1 cargo test -p client api` rewrites it.

export const VERSION = 1;

export type View = { "type": "hello", version: number, language: string, accent: string, } | { "type": "library", games: Array<Game>, } | { "type": "trainer", sheet: number, game: number | null, trainer: Sheet, } | { "type": "values", sheet: number, values: Array<number>, } | { "type": "run", state: RunState, game: number | null, since: number | null, why: string | null, } | { "type": "note", text: string, };

export type Game = { game: number, name: string, favorite: boolean, open: boolean, live: boolean, 
/**
 * A run can start: installed, with a trainer, and no other run.
 */
startable: boolean, 
/**
 * The game has a cover: `/art/<game>` serves it once the window has shown it, else answers 404.
 */
art: boolean, };

export type Sheet = { "state": "none" } | { "state": "pending" } | { "state": "failed", why: string, } | { "state": "ready", sections: Array<Section>, cheats: Array<CheatRow>, collections: Array<Collection>, };

export type Section = { name: string, pinned: boolean, cheats: Array<number>, };

export type CheatRow = { name: string, description: string | null, instructions: string | null, pinned: boolean, control: Control, };

export type Control = { "kind": "toggle" } | { "kind": "button" } | { "kind": "number", min: number | null, max: number | null, step: number, apply: boolean, } | { "kind": "slider", min: number, max: number, step: number, } | { "kind": "selection", options: Array<string>, } | { "kind": "none" };

export type Collection = { name: string, icon: string, };

export type RunState = "idle" | "starting" | "live" | "stopping" | "failed";

export type Command = { "type": "open", game: number, } | { "type": "favorite", game: number, } | { "type": "start", game: number, } | { "type": "stop", game: number, } | { "type": "set", sheet: number, cheat: number, value: number, } | { "type": "press", sheet: number, cheat: number, } | { "type": "pin", sheet: number, cheat: number, } | { "type": "reset", sheet: number, } | { "type": "collect", sheet: number, name: string, } | { "type": "apply", sheet: number, collection: number, } | { "type": "forget", sheet: number, collection: number, };
