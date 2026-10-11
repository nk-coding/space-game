# ADR-0011: 30 Hz fixed simulation, 60 fps rendering with interpolation

- Status: Accepted
- Date: 2026-10-10
- Decided by: Niklas
- Related: [ADR-0004](0004-target-platforms-and-performance-first.md), [ADR-0010](0010-cross-platform-determinism.md), `docs/sim.md` "The tick", `docs/client.md`

## Context

Many physical ships with debris on mid-range phones, a deterministic fixed step (ADR-0010), and a smooth picture
at 60 fps.

## Options considered

Not recorded. The concept pairs the numbers with the phone target ("60 fps render, 30 Hz fixed sim").

## Decision

- The simulation steps at a fixed 30 Hz (`DT = 1/30 s`).
- The client renders at 60 fps and interpolates between the previous and the current sim state
  (`Piece::render_pose(alpha)`; projectiles between `prev_pos` and `pos`).
- In Bevy the sim runs in `FixedUpdate` at 30 Hz; transforms are written from the interpolated pose every frame.

## Consequences

- The sim costs half of what a 60 Hz step would cost per second.
- What is on screen runs up to one tick (33 ms) behind the sim; input is applied per tick.
