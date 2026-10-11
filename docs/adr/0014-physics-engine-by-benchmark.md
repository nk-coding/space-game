# ADR-0014: Physics engine: picked by benchmark on game-like scenarios (choice pending)

- Status: Proposed
- Date: 2026-10-10 (the benchmark was decided as the method and as the spike's first step); the engine is not chosen yet
- Decided by: Niklas (method: performance decides, benchmark first); the engine choice is pending, Niklas decides from the numbers
- Related: [ADR-0002](0002-all-rust-stack-with-bevy.md), [ADR-0004](0004-target-platforms-and-performance-first.md), [ADR-0006](0006-web-as-development-platform.md), [ADR-0010](0010-cross-platform-determinism.md), [ADR-0013](0013-physics-backend-trait.md), `crates/bench/README.md`, `docs/spike-plan.md` "Step 1", `docs/results/physics-benchmark.md` (when written)

## Context

Ships are compound bodies of hundreds to thousands of blocks that break apart; fights add debris and thousands of
ray casts per tick; the targets are mid-range phones. Performance is the top priority (ADR-0004), and the engine
must run cross-platform deterministic (ADR-0010). The concept notes leaned towards Rapier2D before any numbers
existed.

## Options considered

- **Rapier2D** (0.36.1, `enhanced-determinism`): pure Rust, mature, standalone, has a cross-platform determinism
  mode whose docs list WASM. Watch for: the speed cost of that mode; it can't be combined with `simd8`.
- **box2d-rust** (1.4.0): pure Rust port of Box2D 3.1, bit-exact with C in Box2D's determinism test. Watch for:
  young; maintenance and multithreading.
- **Avian 2D** (0.7.0, on Bevy 0.19): pure Rust and Bevy-native, but it ties the core to Bevy's ECS, which
  conflicts with ADR-0003. If it wins, discuss with Niklas before building on it. Its determinism mode needs
  checking.
- **Box2D v3.1 in C**: a reference number only, never a candidate for shipping (ADR-0002).

## Decision (proposed)

Pick the engine with the best numbers on the benchmark's game-like scenarios, provided its per-tick state hash
matches natively and in wasm:

- scenarios: 20 compound ships of 200–2000 blocks, 2000 debris chunks, 5000 ray casts per tick, a breakage every
  second, an asteroid field, and all of them combined;
- metrics: mean and p95 step time at 30 Hz, single- and multi-threaded, natively (weighs most, ADR-0006) and as
  wasm; the phone later.

The harness exists in `crates/bench`; results and a recommendation go to `docs/results/physics-benchmark.md`.
Niklas decides.

## Consequences

- Physics integration (compound bodies per piece, splits with inherited velocity and spin, collisions) waits for
  this decision; until then `FreeFlight` stands in (ADR-0013).
- When Niklas picks, this record gets the chosen engine and the numbers and becomes `Accepted`.
