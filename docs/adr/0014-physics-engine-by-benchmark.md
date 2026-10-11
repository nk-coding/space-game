# ADR-0014: Physics engine: Rapier2D with one compound collider per ship piece, chosen by benchmark

- Status: Accepted
- Date: 2026-10-11 (engine chosen); 2026-10-10 (benchmark decided as the method and as the spike's first step)
- Decided by: Niklas
- Related: [ADR-0002](0002-all-rust-stack-with-bevy.md), [ADR-0003](0003-crate-architecture.md), [ADR-0004](0004-target-platforms-and-performance-first.md), [ADR-0006](0006-web-as-development-platform.md), [ADR-0010](0010-cross-platform-determinism.md), [ADR-0013](0013-physics-backend-trait.md), [ADR-0025](0025-main-repo-holds-only-chosen-code.md), [GDR-0005](../gdr/0005-fully-physical-ships.md), `docs/results/physics-benchmark.md`, `docs/spike-plan.md` "Step 1"

## Context

Ships are compound bodies of hundreds to thousands of blocks that break apart; fights add debris and thousands of
ray casts per tick; the targets are mid-range phones. Performance is the top priority (ADR-0004) and the engine
must be cross-platform deterministic (ADR-0010). On 10 Oct Niklas made a benchmark on game-like scenarios the
first step of the spike (the concept notes had leaned towards Rapier before any numbers existed). The benchmark
ran on 11 Oct 2026: 20 compound ships of 200–2048 blocks, 2000 debris chunks, 5000 ray casts per tick, a split
every second, an asteroid field, and everything combined; native single- and multi-threaded, wasm under Node and
in headless Chromium.

## Options considered

- **Rapier2D 0.36** (chosen): fastest or tied for fastest natively in every realistic scenario with one compound
  shape per body (combined scene with merged rectangles 2.79 ms median step, 6.14 ms for the whole tick with
  5000 rays); a split costs well under 1 ms (~0.04 ms with merged rectangles); hashes identical natively, in wasm
  (Node and browser) and multi-threaded; a plain library that fits the engine-free `sim` crate.
- **Avian 2D 0.7** (rejected; runner-up): fastest with one compound part per cell, but slower on debris and in the
  combined scene (3.44 ms), ties the simulation to Bevy's ECS (conflicts with ADR-0003), needs Bevy 0.19 while the
  game is on 0.20, and its idiomatic one-entity-per-collider layout was the slowest setup measured (~27 ms per
  ship split).
- **box2d-rust 1.4** (rejected): a faithful, deterministic, tiny port (bit-identical to Box2D C), but
  single-threaded only and without a compound shape; 1.8× slower than Rapier-compound in the combined step
  (5.0 vs 2.8 ms), 2.1× on debris, ~9× on the 20 ships alone.
- **Box2D v3.1 in C**: reference number only, never a candidate (ADR-0002).

## Decision

- Rapier2D 0.36 (0.36.1 in the benchmark) with `enhanced-determinism`.
- One compound collider per ship piece (`SharedShape::compound`), built from the greedy-merged rectangles plus
  slope triangles the sim already computes.

## Consequences

- The benchmark's main finding drives the integration: collider layout matters more than the engine (one collider
  per cell costs 5–100× more than one compound shape per body, in every engine).
- Physics integration (bodies per piece, splits with inherited velocity and spin, collisions and ramming) can
  start; when it lands, the generic trait and the `FreeFlight` stand-in go away (ADR-0025, superseding ADR-0013).
- `enhanced-determinism` can't be combined with `simd8`. Multi-threading (`parallel`) keeps the hash but only
  helps on desktop and Android; the web build is single-threaded.
- Not measured yet: the phone (deferred), joints, sensors.
