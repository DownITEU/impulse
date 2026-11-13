---
paths:
  - "crates/wand/**"
---

# Wand source

- `app` registers `Wand` through the capability traits in `model`. Keep source-specific behavior in this crate.
- Never bundle or commit third-party binaries. Check hashes and publisher signatures before loading them; keep verified images locked until the run ends.
- Keep credentials inside the session code and protect them with user-scoped DPAPI. Never log tokens, auth codes, headers or response bodies.
- Treat remote ids and paths as untrusted. Preserve listener scope and limits. Read the user's installation and settings without modifying them.
- Never prefetch trainers.
