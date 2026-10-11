# ADR-0018: English only at launch; every string is keyed in data

- Status: Accepted
- Date: 2026-10-10
- Decided by: Niklas
- Related: [ADR-0015](0015-ron-content-packs-and-mods.md), [ADR-0016](0016-wiki-ready-content-model.md), `docs/content-model.md` "Text and localisation"

## Context

A solo hobby project with lots of text (parts, races, lore, UI), and a possible store release later.

## Options considered

Not recorded.

## Decision

- English only at launch.
- Every string lives in data files under a key, so translations can be added later without code changes.
- Keys are `<kind>.<id>.<field>`; `Content::texts()` lists every key with its English text and
  `Content::apply_texts()` replaces texts by key, ready for a locale file per pack. Wiki prose is English only for
  now.

## Consequences

- No translation work now. Content text is already keyed; UI strings in the client will need keys too.
