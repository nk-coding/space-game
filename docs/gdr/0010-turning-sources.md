# GDR-0010: Ships turn with core assist, gyros, RCS pods and their engines (gimbal and differential thrust)

- Status: Accepted
- Date: 2026-10-11
- Decided by: Niklas (the gimbal part is the lead session's interpretation)
- Related: [GDR-0005](0005-fully-physical-ships.md), [GDR-0009](0009-stick-and-lock-controls.md), [GDR-0011](0011-locked-flight-rear-engines-trade-off.md), [GDR-0012](0012-halvex-runner-follows-rules.md), [GDR-0014](0014-power-tiers-and-weights.md), `docs/content-model.md` "Turning", `docs/sim.md` "Actuators"

## Context

With thrusters only (turning through differential thrust), the Halvex Runner needed about 9 s per 90° turn and
the automaton raider couldn't turn at all. Stick + lock (GDR-0009) needs ships that turn briskly.

## Options considered

- **Thrusters only** (the drydock sketch's rule): far too slow.
- **Three turning sources, all data-driven** (chosen). Other options were not recorded.

## Decision

The flight computer allocates across three sources:

- **Core assist:** a bit of built-in turning help from logic cores (a small reaction torque).
- **Dedicated parts:** gyros (pure torque) and RCS thrusters (small, multi-directional; one nozzle on each side
  open to space).
- **Regular engines:** off-axis engines through differential thrust, and every engine through a small gimbal
  angle (lead's interpretation: a per-engine placeholder that can be 0).

## Consequences

- RCS is its own content block, not an engine mode, so engine rules (exhaust behind) never apply to it; the gyro
  sits in the propulsion category and draws power in the engine tier.
- With placeholder numbers the Halvex Runner turns 90° in about 3.3 s and the raider in about 1.5 s.
- Turning is a drydock trade-off: gyros and RCS pods cost space, mass and power.
