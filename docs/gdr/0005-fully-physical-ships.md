# GDR-0005: Ships are fully physical and break apart

- Status: Accepted
- Date: concept phase (exact day not recorded; settled by 2026-10-11)
- Decided by: Niklas
- Related: [GDR-0004](0004-grid-ship-building-no-wiring.md), [GDR-0006](0006-ship-scale-no-fixed-cap.md), [GDR-0013](0013-vessels-and-piloting.md), GDR-0019, [ADR-0013](../adr/0013-physics-backend-trait.md), [ADR-0014](../adr/0014-physics-engine-by-benchmark.md)

## Context

The brief asks for efficient native code and 2D physics. Layout should matter, and fights should be physical and
readable.

## Options considered

Not recorded.

## Decision

- Ships have real mass, centre of mass and thrust torque: engines push from where they sit, so off-centre thrust
  turns the ship; kinetic recoil pushes the shooter.
- Ships break apart. When parts are destroyed, connectivity is recomputed and every connected piece becomes its
  own body that keeps its velocity and spin and only its own reactors.
- Collisions deal ramming damage by mass and speed (GDR-0019).
- Pieces cut off from a ship become salvage; since 11 Oct, pieces that still have a working logic core become
  vessels instead (GDR-0013).
- The hull glow follows the outer edge of the main piece, so split-off pieces lose it and read as salvage.

## Consequences

- The physics engine's speed with compound bodies and splits is a top risk, hence the benchmark (ADR-0014).
- Layout is a design problem: mass distribution, engine placement and turning (GDR-0010).
