# GDR-0052: Drydock stats show the ship at full use and full power; up to ~120 % heat reads "runs hot"

- Status: Accepted
- Date: 2026-10-12
- Decided by: Niklas (interactively)
- Related: [GDR-0051](0051-drydock-v1.md), [GDR-0022](0022-heat-per-part.md) (heat per part), [GDR-0014](0014-power-tiers-and-weights.md) (power tiers), [GDR-0012](0012-halvex-runner-follows-rules.md), [ADR-0030](../adr/0030-drydock-architecture.md), `docs/sim.md` "Design report (drydock)"

## Context

The drydock's numbers come from `sim::report`, a static solve with the sim's rules. Two assumptions shape them:

- **Heat.** The steady state depends on the load. With everything flat out at once (every gun firing, engines at
  full thrust, shields recharging), the Halvex Runner peaks at 109 %: both beam lasers and both shield emitters
  pass their shutdown limit. In normal fights it does not overheat (GDR-0012 asked for no overheating at normal
  load), and the mockup's status line says "HEAT RUNNING WARM".
- **Power.** When the reactors can't feed every part at full use, the capability numbers (thrust, turning,
  firepower) could show either what the parts can do when powered, or what the brown-out order would leave.

## Options considered

Heat:
- **Full use, softer wording** (chosen): the honest worst case; up to about 120 % reads as running hot (gold),
  only above that is it an overheat alarm (coral). The heat overlay still shows which parts would shut down.
- Full use, strict: anything over 100 % warns; the Runner would need changes.
- A typical combat duty cycle (placeholder shares per part kind): friendlier numbers, the worst case hidden.
- Both values.

Power:
- **Full power plus a deficit warning** (chosen): thrust, turning and firepower as if every part were powered;
  a shortage shows on its own (the power bar over 100 %, a warning, each part's power level in the inspector).
  This matches how the flight computer plans with the actuators that can produce.
- Scale the numbers by the power the brown-out order would deliver at full use (the concept-board drydock).

## Decision

- The design report assumes **full use**: all consumers at full draw, engines at full thrust, guns firing.
- **Peak heat** up to about 120 % of a part's limit is shown as "runs hot at full load" in gold; above that it is
  an overheat warning in coral. Parts over 100 % are still marked in the heat overlay. (120 % is a placeholder.)
- **Thrust, turning and firepower** assume full power to every working part; idle parts (over the control
  capacity) are left out. A power deficit at full use is a separate warning.

## Consequences

- The Halvex Runner stays as it is; its status reads as running hot, not as overheating.
- The heat colour threshold is a placeholder and lives with the client's status wording.
