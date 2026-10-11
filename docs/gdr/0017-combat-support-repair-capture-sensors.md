# GDR-0017: Repair drones mid-fight, capture by killing all power, moderate sensors, a minimal HUD

- Status: Accepted
- Date: 2026-10-10
- Decided by: Niklas
- Related: [GDR-0002](0002-awakened-ship-mind-light-story.md), [GDR-0008](0008-ui-ship-mind-interface.md), [GDR-0013](0013-vessels-and-piloting.md), [GDR-0014](0014-power-tiers-and-weights.md), [GDR-0015](0015-layered-behavior.md)

## Context

The combat concept of 10 Oct also settled what happens around the shooting: repairs, taking ships over, what a
ship can see, and how much the HUD shows.

## Options considered

Not recorded.

## Decision

- **Repairs mid-fight:** repair drones fly out and patch blocks with repair material from the hold; they can be
  shot down.
- **Capture:** kill all of a ship's power and its logic core goes dark. Copy yourself into it and the ship is
  yours.
- **Sensors:** moderate. They set targeting range and scan detail; nearby ships are always visible. Behavior
  conditions only read what the sensors detect.
- **HUD:** minimal. Damage shows on the ships themselves, plus a hull map of your own ship.

## Consequences

- Capture relies on logic cores needing power (GDR-0014); a captured ship is a vessel like any other (GDR-0013).
- Repair material is a voyage supply (GDR-0028).
