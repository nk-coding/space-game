# ADR-0044: No save compatibility before release: the game reads only the current formats and starts a new career in place of an older save

- Status: Accepted (Niklas, 18 Oct 2026: "I am running the only instance, and I can also just restart")
- Date: 2026-10-18
- Decided by: Niklas (no backwards compatibility needed); the shape by the lead session's cleanup
- Related: [ADR-0024](0024-saves-seed-and-deltas.md), [ADR-0031](0031-player-storage-and-design-sharing.md),
  [ADR-0035](0035-saves-keep-the-galaxy-skeleton.md), [GDR-0096](../gdr/0096-career-galaxy-format-3.md),
  [GDR-0098](../gdr/0098-slice-in-the-galaxy-and-rim-rewards.md), [GDR-0103](../gdr/0103-the-grown-in-play-defaults.md),
  [GDR-0108](../gdr/0108-materials-economy-workshop.md), `docs/career.md` "The save", `docs/drydock.md` "Storage and sharing"

## Context

ADR-0024 made saves "versioned, with migrations". Before release there is one player, Niklas, with no progress
worth keeping, yet each new save format (2 to 5 in three days) brought a migration: a slice career charted into its
galaxy, a format 3 skeleton's pins kept and its Warden made a gate, a format 4 save's designs stocked into the
workshop, an older voyage roller moved on, saved designs refitted when a part took over another's place
(`replaces`). Each stayed in the code, its tests and fixtures, and the docs.

## Options considered

- **Keep migrating every format**: what ADR-0024 asked; it costs code, tests and fixtures for every change, for one
  player who can start over.
- **No compatibility before release** (chosen): only the current formats read; an older save is replaced by a new
  game.

## Decision

- Until release the career reads only the current save format (`data::career::FORMAT`) and voyage roller version
  (`GENERATOR`). The storage refuses a document older than its `Document::OLDEST` (`StorageError::Older`); for the
  save `OLDEST` is `FORMAT`, so an older save starts a new game in its place, written at once, with a log line and
  the note A NEW GAME. A save that doesn't read or comes from a newer build is still kept as it is and never
  written.
- Nothing is migrated: a format change raises the number and drops the old reader. Content no longer carries
  compatibility fields (`replaces` is gone).
- Formats that only add fields with defaults may still read as they are (`OLDEST` below `FORMAT`; designs read
  format 1 and 2).

## Consequences

- A format change costs one number and its docs; the player starts over.
- At release (ADR-0024's migrations come back then) the formats must be frozen and a migration written per change.
