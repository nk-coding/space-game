# GDR-0019: Damage: outer blocks first, penetration by weapon type, ricochets off slopes, chain reactions, ramming

- Status: Accepted
- Date: 2026-10-10 (model); 2026-10-11 (kinetic shots stop at the hull)
- Decided by: Niklas
- Related: [GDR-0005](0005-fully-physical-ships.md), [GDR-0018](0018-weapon-families.md), [GDR-0020](0020-damage-v1-defaults.md), [GDR-0021](0021-shields.md), [ADR-0014](../adr/0014-physics-engine-by-benchmark.md), `docs/sim.md` "Weapons", "Damage and breakage"

## Context

Ships are grids of parts that break apart (GDR-0005). Where shots land decides which parts die, how ships split
and how armor and slopes pay off.

## Options considered

- Over-penetration of kinetic shots into whatever lies behind: not wanted for now (11 Oct); to be revisited with
  railguns.
- Other models: not recorded.

## Decision

- Outer blocks take hits first.
- Penetration by type: lasers burn the first block, railguns pierce about three, missiles splash.
- Slopes deflect kinetic shots at shallow incidence (more than 60° from the face normal).
- Chain reactions: destroyed reactors, capacitors and drives explode into their neighbours.
- Ramming deals damage by mass and speed, so a reinforced prow is a build.
- Kinetic shots stop at the hull: no over-penetration into another ship (Niklas, 11 Oct; revisit with railguns).

## Consequences

- The v1 implementation details are lead defaults (GDR-0020).
- Ramming waits for collisions from the physics engine (ADR-0014).
