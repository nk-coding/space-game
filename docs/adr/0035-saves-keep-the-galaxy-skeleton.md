# ADR-0035: Saves keep the galaxy skeleton next to the seed

- Status: Accepted
- Date: 2026-10-14
- Decided by: Niklas (`docs/plans/galaxy-voyages.md` question 7)
- Related: [ADR-0024](0024-saves-seed-and-deltas.md) (refined), [ADR-0033](0033-career-model-save-and-voyage-director.md), [GDR-0078](../gdr/0078-galaxy-exploration-v1.md), [GDR-0080](../gdr/0080-exploration-searching-and-story.md)

## Context

ADR-0024 stores a game's seed, the generator version and deltas, and regenerates the world. The galaxy's
generator numbers are content (GDR-0026), so a balance update or a mod would reshuffle a running galaxy unless
every old parameter set stayed frozen forever. With exploration (GDR-0078) the save also holds signals that point
at generated places; a reshuffle would leave them pointing at nothing.

## Options considered

- **Keep the skeleton** (chosen): the clusters (id, name, band, map position, type, seed), the gates and the trails
  with their places, a few KB, stored with the seed. Cluster interiors are still generated from each cluster's seed.
- **Rebuild everything from the seed**: the smallest saves, but every generator setting must stay frozen per
  generator version.

## Decision

- A new game generates the skeleton once and the career save stores it. Loading never regenerates it.
- Cluster interiors are generated on demand from the cluster's seed, type and the generator version; trail places
  the skeleton fixes are put in as fixed sites.
- New bands, clusters or trails reach an old save only through a migration.

## Consequences

- `data::career` gets a format with the skeleton (and a migration from the slice's format).
- Generator changes may change interiors but never the map of a running game; a changed interior drops the deltas
  of sites that no longer match (logged by the migration).
- ADR-0024 stands otherwise (seed, generator version, deltas).
