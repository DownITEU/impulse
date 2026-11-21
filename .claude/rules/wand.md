---
paths:
  - "crates/wand/**"
---

# Wand source

- `app` registers `Wand` through the capability traits in `model`. Keep source-specific behavior in this crate.
- Never bundle or commit third-party binaries. Check hashes and publisher signatures before loading them (`PUBLISHERS`: WeMod LLC, and its earlier name Daring Development Inc. on older trainers); keep verified images locked until the run ends. The service and plugin are those of the trainer's architecture (`binaries`), as Wand picks them.
- Keep credentials inside the session code and protect them with user-scoped DPAPI. Never log tokens, auth codes, headers or response bodies.
- Treat remote ids and paths as untrusted. Preserve listener scope and limits. Read the user's installation and settings without modifying them.
- Never prefetch trainers.
- A bridge request that fails names its method and address, never the query (a download's may be signed). A `TypeError` is probed with a `no-cors` request: no answer (network, firewall) and an answer without CORS headers (a proxy, a check page) are worded apart.
