# ADR-0016: Content is wiki-ready: stable ids, keyed text, stat descriptors, prose next to the data

- Status: Accepted
- Date: 2026-10-11
- Decided by: Niklas (wiki later, content wiki-ready now); lead session (the model's details)
- Related: [ADR-0015](0015-ron-content-packs-and-mods.md), [ADR-0017](0017-embedded-content-in-memory-loader.md), [ADR-0018](0018-english-only-keyed-text.md), [ADR-0021](0021-no-fixed-raster-graphics.md), `docs/content-model.md`

## Context

A wiki will come later. Writing the same facts twice, once for the game and once for the wiki, would drift
apart.

## Options considered

Not recorded. The decision was the direction itself: the same files feed the game and the wiki.

## Decision

- One source: the game, the tools and the future wiki read the same `content/` files, through the `data` crate or
  its JSON export.
- Stable snake_case ids, unique per kind, never changed once shipped (rename the `name` instead).
- Text lives in data with derived keys (`<kind>.<id>.<field>`), ready for a locale layer (ADR-0018).
- No numbers in prose: numbers come from stat lines, described in `stats.ron` (label, unit, precision,
  better-direction), so text never goes stale when balance changes.
- Optional long-form wiki prose in `wiki/<kind>/<id>.md` next to the data; never embedded in the game.
- `[[kind:id]]` links in text, checked by validation.
- `content-tool export-json` (format version 1) gives any external generator the resolved content.
- Content never holds colours; it names art keys that `art/tokens.json` resolves. Placeholder numbers are marked
  in every file.

## Consequences

- A wiki generator can be built on `data` later without touching the game.
- Icons move into the content pack so the wiki can show the SVGs directly (ADR-0021).
