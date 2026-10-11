# ADR-0015: Content lives in RON packs; the base game loads like a mod; mods from day one

- Status: Accepted
- Date: 2026-10-10
- Decided by: Niklas
- Related: [ADR-0016](0016-wiki-ready-content-model.md), [ADR-0017](0017-embedded-content-in-memory-loader.md), [ADR-0019](0019-rhai-scripting.md), [ADR-0023](0023-host-authoritative-netcode.md), GDR-0024, GDR-0032, `docs/content-model.md`

## Context

The game needs lots of content (races, about 200 tech nodes, materials, set pieces) and a balance arena that can
test changed numbers without a rebuild. Modding was wanted from the start.

## Options considered

- Other data formats: not recorded.
- **Mods added later** or **mods from day one, through the base game's own loader** (chosen). The reason is not
  recorded beyond "mods from day one".

## Decision

- All content lives in data: parts, materials, ships, later tech trees, races, set pieces, recipes, behaviors and
  strings, as hand-edited RON files. Names, descriptions, stats and layouts are never hard-coded in Rust.
- `content/base` is a pack with a `pack.ron` manifest and loads exactly like a mod. Packs load in order; an entry
  with an existing id replaces it.
- Mods from day one: a mods folder with RON data and Rhai scripts (ADR-0019), versioned and validated schemas, a
  deterministic script API; co-op requires a matching mod-list hash.
- Authoring: hand-edited RON; enemy and guardian ships come from the drydock's export; hot reload in development.

## Consequences

- Validation is strict (unknown fields are errors); `content-tool validate` checks packs.
- A schema change needs a migration and a new `schema_version`.
- Not yet built: partial patches, removing entries, locale files.
- Mods from a zip or URL in the browser are no longer required (ADR-0006).
