# GDR-0015: Behavior is layered: rule lists over built-in maneuvers over an always-on flight computer

- Status: Accepted
- Date: concept phase (exact day not recorded; settled by 2026-10-11)
- Decided by: Niklas
- Related: [ADR-0019](../adr/0019-rhai-scripting.md), [GDR-0008](0008-ui-ship-mind-interface.md), [GDR-0009](0009-stick-and-lock-controls.md), [GDR-0013](0013-vessels-and-piloting.md), GDR-0032, `docs/behavior-format.md`, `docs/sim.md` "Flight"

## Context

v1's lesson: scripting drove the ship at too low a level. It was tedious, and one bad value sent the ship
spinning. v2 needs a higher abstraction (a node or behavior-tree system like Gladiabots), with detailed
scripting still possible.

## Options considered

- **Low-level scripting** (v1): rejected for the reasons above.
- **Layers, with goals flowing down** (chosen).

## Decision

- Three layers; goals flow down and only the flight computer fires thrusters:
  1. **Behavior** (the player edits it): a priority list of rules, condition → action, first match runs.
     Presets (Escort, Hunter, Miner, Guard) are behaviors you can open and change.
  2. **Built-in maneuvers**, set by parameters (approach, orbit, keep range, strafe, flank, dock, hold formation
     slot, flee). Advanced players can script new ones.
  3. **The flight computer**, always on: turns intent into thruster output, enforces speed and spin limits,
     avoids collisions. Nothing above it can push the ship past those limits.
- Behaviors have a text format, one line per rule; the editor switches between graph and text at any time.
- Conditions can only read what the ship's sensors detect.
- Rule slots come from logic cores: a cheap drone runs a short list, a capital ship a long one.
- Custom nodes are written in Rhai (ADR-0019); a script only returns a goal and runs with an instruction budget.

## Consequences

- AI enemies run the same behaviors as player escorts, so the balance arena tests them too (GDR-0032).
- Behavior system v1 (11 Oct): rule lists as content, the text format (`docs/behavior-format.md`), rule slots,
  Rhai conditions.
