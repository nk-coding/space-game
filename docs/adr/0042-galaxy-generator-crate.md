# ADR-0042: The galaxy generator is an engine-free `galaxy` crate in `core/`, deterministic with integer positions and a generation hash

- Status: Accepted (provisional, lead)
- Date: 2026-10-16
- Decided by: lead session (default for Niklas to review; the plan's recommendation, `docs/plans/galaxy-voyages.md` 6.1)
- Related: [ADR-0003](0003-crate-architecture.md), [ADR-0010](0010-cross-platform-determinism.md), [ADR-0024](0024-saves-seed-and-deltas.md), [ADR-0035](0035-saves-keep-the-galaxy-skeleton.md), [ADR-0038](0038-workspace-structure-feature-folders-and-client-split.md), [GDR-0093](../gdr/0093-galaxy-exploration-decisions-16-oct.md), [GDR-0094](../gdr/0094-galaxy-generator-defaults.md)

## Context

Galaxy track E2 generates a game's galaxy (clusters in bands, home, gates, trails of signals, cluster interiors)
from the game seed and the content's galaxy kinds (E1). The save keeps the result (ADR-0035), so the generator must
give the same skeleton on every platform for the same seed, generator version and content, and the game, the wiki
and tools all need it. The plan left open whether it is a crate or a module of `data`.

## Options considered

- **A `galaxy` crate in `crates/core/`** (chosen): depends on `data` only (no `sim`, no Bevy, no I/O), builds for
  wasm32; the game (map, voyages), `content-tool` (inspection, the wiki's example) and later the voyage director's
  interiors use it. Keeps `data` a content library.
- **A module of `data`**: one crate fewer, but `data` would grow generation logic beside the schema; reason enough
  to keep it out, as the plan recommended.
- **Inside `sim`**: generation needs no physics; it would drag Rapier into the wiki tool for nothing.

## Decision

- `crates/core/galaxy`: `generate(content, galaxy_id, seed) -> Skeleton` and `interior(content, skeleton, cluster)`;
  `GENERATOR_VERSION` (1) goes into every skeleton and is raised whenever the same inputs would generate a different
  skeleton or interior.
- **Determinism contract**: map positions are integers in hundredths of S, every distance test is on integers;
  roots and angles through `libm`; ordered containers only; every step draws from a PCG32 stream of its own with a
  fixed id (place, home, gates, kinds, seeds, names, trails, interior; append only), seeded by
  `splitmix64(seed ^ splitmix64(id + attempt · 2³²))`. PCG32 moved from `sim::rng` to `data::rng` so the sim and the
  generator share one implementation (`sim::rng::Pcg32` re-exports it).
- **Generation hash**: `Skeleton::hash` is FNV-1a 64 of the skeleton's RON. `scripts/determinism-check.sh --galaxy
  N` builds galaxy's `galaxy_hashes` example natively and as wasm32-wasip1 and compares, per seed, the skeleton's
  hash and the hash of every interior. A test pins seed 1's hash on the base content.
- Inspection: `content-tool galaxy [--seed N] [--svg out.svg]` prints the bands, home, gates and trails and draws
  the map; the galaxy's wiki page shows seed 1's map.

## Consequences

- Any change to the generator's draws or rules changes skeletons: bump `GENERATOR_VERSION` and the pinned hash.
  Content changes to the galaxy kinds change new games only (running ones keep their skeleton, ADR-0035).
- E3 (`sim::voyage` grows generated plans) takes interiors from this crate; E4 stores the skeleton in the career save.
- Measured 16 Oct 2026: 50 seeds identical native vs wasm (`--galaxy 50`).
