# GDR-0026: One procedural galaxy, five bands from rim to core, gated by guardians

- Status: Accepted; travel along lanes superseded by [GDR-0078](0078-galaxy-exploration-v1.md) (jump range, no lanes; 2026-10-14).
  2026-10-17: set pieces are placed into the galaxy by content, the slice's four first; the rim's first gate is the
  Warden's ([GDR-0098](0098-slice-in-the-galaxy-and-rim-rewards.md)).
- Date: 2026-10-10
- Decided by: Niklas (interactive Q&A)
- Related: [GDR-0002](0002-awakened-ship-mind-light-story.md), [GDR-0027](0027-races.md), [GDR-0028](0028-clusters-and-voyages.md), [ADR-0022](../adr/0022-one-simulation-space-per-cluster.md), [ADR-0024](../adr/0024-saves-seed-and-deltas.md)

## Context

The brief: a vast world with long playtime and lots of content.

## Options considered

- **Location-based play**: parked (light version only: your real location seeds your home region).
- Other world structures: not recorded.

## Decision

- One seamless procedural galaxy of star clusters linked by lanes, with handcrafted set pieces placed in; a new
  seed per game. About 80 clusters for now, and it must stay expandable: cluster count, band sizes, races and set
  pieces are generator parameters and data, and saves must tolerate added clusters and bands.
- The journey goes inward: rim → outer reach → middle reach → inner reach → core (the precursor archive: final
  gate and best tech). About 50 hours to the core; the loop (exploring, bigger ships, co-op and arena) continues
  afterwards.
- Gating: difficulty by distance, race territory and environmental hazards (stellar heat in the outer reach, ion
  storms and nebulae in the middle, radiation rising from the inner reach to the core). A precursor guardian sits
  at every band gate; beat it to add the gate to your network.
- Guardians are huge breakable precursor machines built from parts; their parts are precursor tech; each can be
  beaten solo; their behavior changes as sections break.
- The world state is static; cleared encounters come back slowly.
- Content types: derelicts and graveyards, race stations, outposts, bosses (guardians, swarm hives, giant ships),
  megastructures and anomalies; more over development, all as data.

## Consequences

- Saves store seed, generator version and deltas (ADR-0024); one simulation space per cluster (ADR-0022).
- Open: new game+.
