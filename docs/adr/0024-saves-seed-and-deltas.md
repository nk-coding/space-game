# ADR-0024: Saves store seed, generator version and deltas, versioned with migrations

- Status: Accepted
- Date: 2026-10-10
- Decided by: Niklas
- Related: [ADR-0010](0010-cross-platform-determinism.md), [ADR-0015](0015-ron-content-packs-and-mods.md), [ADR-0016](0016-wiki-ready-content-model.md), GDR-0026

## Context

The galaxy is procedural and must stay expandable: cluster count, band sizes, races and set pieces are generator
parameters and data, and saves must tolerate clusters and bands added later (GDR-0026).

## Options considered

Not recorded.

## Decision

- A save stores the galaxy seed, the generator version and the changes (deltas), not the whole world.
- Saves are versioned, with migrations.
- Export and import by file or share code now; optional cloud sync later.
- (Browser storage via IndexedDB came with ADR-0005 and is no longer required, ADR-0006.)

## Consequences

- The generator must be versioned, and old generator versions must stay reproducible or migratable.
- Stable content ids (ADR-0016) and the RNG stream ids (append only, ADR-0010) are part of the save format.
