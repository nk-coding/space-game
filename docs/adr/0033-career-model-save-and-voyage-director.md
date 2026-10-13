# ADR-0033: The career: progression rules in `data::career`, salvage and the voyage director in `sim`, one career save

- Status: Accepted (provisional, lead)
- Date: 2026-10-13
- Decided by: lead session (default for Niklas to review)
- Related: [ADR-0010](0010-cross-platform-determinism.md), [ADR-0022](0022-one-simulation-space-per-cluster.md), [ADR-0024](0024-saves-seed-and-deltas.md), [ADR-0030](0030-drydock-architecture.md), [ADR-0031](0031-player-storage-and-design-sharing.md), [GDR-0062](../gdr/0062-first-playable-loop.md), [GDR-0063](../gdr/0063-salvage-and-research-v1.md), `docs/plans/first-playable-loop.md` sections 5.2–5.6

## Context

The first playable loop (GDR-0062, GDR-0063) adds progression (unlocks, research, mind capacity), salvage and cargo,
voyages with seeded sites, and a save. Today's sandbox spawns raider waves in the client (`game::scene`) with a client
RNG, outside the sim's seeded streams and state hash. Voyages must be reproducible from the save, run headless for
balance runs, and later run on a co-op host and in the arena.

## Options considered

- Encounter logic **in `sim`, as an optional voyage director using the sim's seeded streams** (chosen), or in the
  client's scene like today's waves (quick, but outside the hash; the arena and co-op couldn't use it).
- Progression rules **as a pure model in `data` (`data::career`), like `data::edit` for the drydock** (chosen), or
  Bevy systems in the client (not testable on their own, not usable by tools).
- The save **stores facts only (research done, data and materials in store, escorts, voyage results, seed) and
  derives the rest from content** (chosen), or stores derived state too (a content rebalance would need migrations).
- **One career slot** (`saves/career`, `?save=<slot>` for testing) (chosen), or a slot picker.

## Decision

- `data::career`: no Bevy, no I/O, wasm-safe; the save's facts as serde types; answers unlocks, mind capacity, open
  voyages, payment, unloading a haul and voyage-readiness.
- `sim`: salvage and cargo (`sim::salvage`), wrecks spawned as wrecks, core fragments, and the voyage director
  (`sim::voyage`: lazy site spawning, waking, clearing, the recall check) on a new append-only RNG stream; all in
  the state hash. A `Voyage` determinism scenario joins the native-vs-wasm check; a headless voyage runner is the
  balance tool.
- `game`: a career plugin holds the save in `game::storage` under `saves/` and feeds `Unlocks` to the drydock, the
  research screen and the map.
- Voyage seeds come from the game seed, the voyage id (FNV-1a, never Rust's `DefaultHasher`) and the launch count.

## Consequences

- The sandbox keeps its client-side waves for now; voyages never copy that path.
- Mid-voyage saves are not needed (GDR-0062); world deltas (ADR-0024) come with the galaxy.
- The galaxy and co-op plans build on the same director and save (`docs/plans/galaxy-voyages.md`,
  `docs/plans/coop-arena.md`).
