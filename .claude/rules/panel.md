---
paths:
  - "web/panel/**"
---

# panel

The remote panel: a Preact page `remote` embeds from `web/panel/dist`. It shows what the app sends and sends commands; it keeps nothing.

## Surface
- `src/api.ts`: the protocol, generated from `client::api` (`IMPULSE_BLESS=1 cargo test -p client api`); never edited by hand.
- `src/session/`: the only code that talks to the server. `connect` pairs with the address's `#code`, then keeps one socket to `/ws`; `useSession` (the newest of each view), `send`; `state.ts` derives `valuesOf`, `isLive`, `isOutdated`. `demo.ts` feeds sample views in `pnpm dev` (`/?demo`, `/?demo=fr`).
- `src/i18n/`: `useT()`, captions from `locales/<tag>.ftl` in the language Hello names, English for a missing one; `useLanguage()`, the tag numbers follow.
- `src/session/` also names the server's paths the page loads directly: `coverOf` (`/art/<game>`).
- Feature folders: `app/` (the frame: top bar, dock, drawers, placeholders, notes), `library/` (the games, the open game's header with its run, favorite, start and stop), `trainer/` (sections, tiles, controls, collections), `shared/` (primitives). `app` composes the features; a feature reaches only `shared`, `session` and `i18n`.

## Rules
- Everything shown comes from the app: names, sections, options and failures arrive worded. The panel's own captions are `t('id')` with a literal id; `checks` matches them to `locales/en.ftl`, and every language to English.
- No browser storage. Favorites, pins, collections and RESET are commands; the app owns them.
- A command names the sheet of the trainer view it acts on; a refusal comes back as a note. Values show only for the trainer view they name. Controls act only while `isLive`.
- pnpm only. `scripts\panel.ps1` builds `dist`; `scripts\check.ps1` runs `pnpm verify` once the panel is installed. Source files stay under 500 lines (`checks`).

## Traps
- `dist` is embedded at compile time: a new build shows after the next cargo build (`remote`'s build script watches the folder).
- Plain HTTP on the LAN: secure-context APIs (`crypto.subtle`, the clipboard) are missing. The cookie is `HttpOnly`: the page never sees it, and a socket that never opens may mean not paired or not reachable.
