# ADR-0013: Physics sits behind a `PhysicsBackend` trait, with `FreeFlight` as the stand-in

- Status: Superseded by [ADR-0025](0025-main-repo-holds-only-chosen-code.md)
- Date: 2026-10-11
- Decided by: lead session (default for Niklas to review)
- Related: [ADR-0003](0003-crate-architecture.md), [ADR-0010](0010-cross-platform-determinism.md), [ADR-0014](0014-physics-engine-by-benchmark.md), GDR-0019, `docs/sim.md` "The physics seam"

## Context

The physics engine is picked by benchmark (ADR-0014), and the benchmark was still running while the sim core,
the flight computer and weapons were built. That work should not wait, and the engine should not leak into the
rest of the simulation.

## Options considered

- **Wait for the benchmark** before building the sim: blocks phases 2 and 3.
- **Build directly on the likely winner** (Rapier was "leaning"): rework if another engine wins.
- **A narrow trait the sim drives, plus an engine-free stand-in** (chosen).

## Decision

- `PhysicsBackend`: create, update and remove bodies from explicit mass properties and convex colliders; ghost
  flags for pieces right after a split; apply wrenches; step; read bodies back; ray casts; contacts; optional
  `hash_state` for engine-internal state.
- `Sim<P = FreeFlight>` owns the backend and calls it in a fixed order every tick.
- The sim computes mass properties and colliders itself (greedy-merged rectangles plus slope triangles), not the
  engine.
- `FreeFlight`: semi-implicit Euler about the centre of mass, no collisions, ray casts against the convex
  colliders with bounding-circle culling.
- The client only uses the backend-independent API, so switching engines means changing one type alias.

## Consequences

- Collisions and ramming damage wait for the real engine (`Impact` events will become ramming damage).
- Adding the engine: implement the trait, construct `Sim::with_backend`, add its determinism scenario.
- Engine features outside the trait (joints, continuous collision, and so on) need a trait change first.

Superseded on 11 Oct 2026: Niklas chose Rapier2D (ADR-0014) and decided that the main repo holds only chosen code
(ADR-0025), so the generic trait and `FreeFlight` are removed once Rapier is integrated. Until then they stay.
