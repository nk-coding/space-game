# ADR-0036: An engine-free `arena` crate for headless fights, evolution and benchmarks

- Status: Accepted. Amended 2026-10-15 by [ADR-0040](0040-arena-library-in-core.md): the library lives in
  `crates/core/arena`, the CLI is the package `arena-cli` in `crates/tools/`.
- Date: 2026-10-14
- Decided by: Niklas (GDR-0083's "engine-free crate, both")
- Related: [ADR-0003](0003-crate-architecture.md), [ADR-0010](0010-cross-platform-determinism.md), [ADR-0025](0025-main-repo-holds-only-chosen-code.md), [GDR-0032](../gdr/0032-balancing-arena-evolution.md), [GDR-0083](../gdr/0083-evolution-lab.md), `docs/plans/coop-arena.md` 5.7

## Context

ADR-0003 planned an `arena` crate (headless fights, evolution, balance reports). GDR-0083 needs it now, used by the
game (a lab screen, time-sliced, in the browser too) and by a native CLI on every core, with the same results.

## Decision

- `crates/arena`: depends on `data` and `sim`; no Bevy, no I/O, no threads in the library, builds for wasm32.
- It holds: the fight spec (seed, sides with designs and behaviors, distance, time limit) and running a fight to a
  result; the scores (arena pool, voyage encounters through `sim::voyage`); the evolution (genomes as blueprints and
  behavior text, mutation and crossover through `data::edit` and `data::behavior_edit`, the archive, the run
  configuration and state), steppable in small slices so a client can run a few fights per frame.
- Deterministic: its own seeded streams, ordered containers, libm math; a run's results depend only on its
  configuration, content and seed.
- A native CLI (`crates/arena/src/bin/` or an example) runs batches on every core with `std::thread::scope`.

## Consequences

- The game gets a lab screen on top of it (dev-only first); the test range later gets a benchmark rating.
- `docs/plans/coop-arena.md`'s fight spec and scoring are the starting point.
