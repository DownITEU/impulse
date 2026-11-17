---
paths:
  - "crates/sdk/**"
---

# sdk

What a third-party plugin builds against: the v1 wire, the manifest's format and the guest's side of the ABI. The one boundary outside authors depend on: once v1 freezes (after the MapGenie plugin), a change keeps every v1 guest and host reading each other.

## Surface
- `API` (the wire version), `wire` (`Request`, `Call`, `Reply`, `Error`, `Title`, `MapOffer`, `Map` and its parts, `HttpRequest`, `PIN_LINK`), `manifest` (`Manifest`, `Field`, `FieldKind`), `guest` (`Provider`, `Context`, `export!`, `http_get`, `log`, `mercator`).

## Rules
- Names no crate of ours; `model` does not name it either: `plugins` converts the wire into `model`, so `model` changes freely while v1 stays.
- A new field is optional with a default; an unknown field is ignored; a new call kind or error kind reads as `Call::Unknown` (answered `unsupported`) or `ErrorKind::Internal`. Never rename or retype a field of v1.
- Ids are bare strings: the host makes every `Ref` (M17). A pin's group is its group's id.
- The ABI is in `guest`'s module doc: one dispatch export, imports that never call back into the guest, a body crossing raw and a host error as a `Reply`. A new host import is additive: an old guest never imports it.
- `guest` compiles on every target: outside wasm32 its imports answer `unsupported`, so a provider's logic runs in native tests.

## Traps
- An instance lives for one call: the guest frees nothing, by design.
- `sdk::wire::PIN_LINK` and `model::PIN_LINK` are the same text; a test in `plugins` holds them together.
