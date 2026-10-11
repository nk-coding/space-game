# ADR-0017: The base content is embedded at compile time behind a pure in-memory loader

- Status: Accepted (provisional, lead)
- Date: 2026-10-11
- Decided by: lead session (default for Niklas to review)
- Related: [ADR-0003](0003-crate-architecture.md), [ADR-0005](0005-web-as-full-end-user-platform.md), [ADR-0006](0006-web-as-development-platform.md), [ADR-0015](0015-ron-content-packs-and-mods.md), `docs/content-model.md` "Loading in the game"

## Context

`sim` and `data` must not need a filesystem (wasm32, Android), yet content must load identically everywhere, and
native tools need to read pack directories. The concept said data comes in "through the asset layer".

## Options considered

- **Load through the client's asset layer** (the concept's wording).
- **Embed the base pack in the binary and keep the loader pure** (chosen).
- The trade-offs between the two were not recorded.

## Decision

- `data` never touches the filesystem: `Pack::from_files` takes `(path, contents)` pairs, and `Content::load`
  merges packs and validates them.
- Feature `embed` (default) compiles `content/base` (without `wiki/`) into the binary: `data::base_content()`,
  `base_pack()`, `base_pack_files()`.
- Feature `fs` (native tools only) loads pack directories.
- Mods and hot reload feed the same function with `(path, contents)` pairs.

## Consequences

- The same bytes load on every platform, which helps determinism.
- Changing base content means rebuilding the game (hot reload covers development); the binary grows with the
  content, which doesn't matter on the web (ADR-0006).
