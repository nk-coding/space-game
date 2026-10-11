# GDR-0022: Heat is simulated per part, with shutdown hysteresis

- Status: Accepted (provisional, lead)
- Date: 2026-10-11
- Decided by: lead session (default for Niklas to review)
- Related: [GDR-0004](0004-grid-ship-building-no-wiring.md), [GDR-0012](0012-halvex-runner-follows-rules.md), `docs/sim.md` "Heat"

## Context

The concept-board drydock computes steady-state heat per cell. The sim needs dynamic heat that is cheap enough for
20 ships of up to 2000 parts.

## Options considered

- **Per cell** (the drydock sketch).
- **Per part, lumped** (chosen). The reason was not recorded; the recorded property is that its steady state
  equals the drydock model's.

## Decision

- Heat is one level per part (capacity = the part's area). It conducts over shared edges to linked neighbours and
  is lost to space through open sides; radiators vent the most.
- A part above its heat limit shuts down until it cools below 75% of the limit (hysteresis).
- All constants are placeholders.

## Consequences

- A big part heats as one unit; hot spots inside a 2×2 machine are not modelled.
- Overdrive (GDR-0004) is not built yet.
