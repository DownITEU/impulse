---
paths:
  - "crates/remote/**"
---

# remote

The remote panel's server: the built panel, `POST /pair`, covers and the WebSocket at `/ws`, on this machine's LAN address. Transport only: pairing, tokens, commands and covers are asked of the app through `client::api::ServerSide`; the views it sends are the app's, already serialized.

## Surface
- `Server::bind(port)` (the LAN address, the port or one of the next free), `address`, `serve(side)`: one future owns the listener and every connection; dropping it stops them all. Run on GPUI's background executor, no runtime of its own.

## Rules
- Private-range peers only; `Host` must be the address served, `Origin` too for `/pair` and `/ws`. A cover and `/ws` need the device cookie; the app says whose it is.
- Every read, write and ask of the app is bounded in size and time (`DEADLINE`); a socket that stops reading, stays silent for two pings or waits on a stalled app is dropped. Connections are capped. A duplicate `Host` or `Origin`, or any `Transfer-Encoding`, drops the request.
- Every answer closes its connection and is `no-store`. The cookie is `HttpOnly; SameSite=Strict`. A token or a code is never logged.
- The panel is `web/panel/dist`, embedded by `rust-embed` (`allow_missing`, `debug-embed`: no file is read while serving): a build without it answers 503.
- The WebSocket handshake is `async-tungstenite`'s, over the bytes the head read already took (`Replay`).

## Traps
- The app publishes into `Views`, the server reads `Feed`: a socket sends the newest of each kind, in order, and its device's notes. A revoked device's sockets close at their next look.
- A token is the app's to check: the server passes the cookie on, whatever its shape.
