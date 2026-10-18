# GDR-0068: A voyage site clears when its enemies are destroyed or disarmed; only armed enemies block recall

- Status: Accepted
  2026-10-18: a ram prow and a working engine arm a ship like a gun ([GDR-0109](0109-a-ram-prow-and-an-engine-arm-a-ship.md)).
- Date: 2026-10-13
- Decided by: Niklas (the clearing rule, interactively); lead session (the director's other defaults, for review)
- Related: [GDR-0062](0062-first-playable-loop.md), [GDR-0013](0013-vessels-and-piloting.md), [ADR-0033](../adr/0033-career-model-save-and-voyage-director.md), `docs/sim.md` "Voyages", `docs/plans/slice-content.md`

## Context

The voyage director (track 7E) cleared a site when none of its ships was a running vessel, and blocked recall while
an awake enemy vessel was near. The headless runner showed enemies whose guns were all shot off drifting on with a
working core: in 83 of 160 runs one of them blocked clearing (and, within 200 m, recall). The Warden's shielded
heart has no guns at all.

## Options considered

- **Destroyed or disarmed** (chosen): a site clears when every enemy ship is destroyed or has no working weapon;
  only armed awake enemies block recall; the Warden's heart gets a weapon so it must still be beaten.
- Every core destroyed (strict): disarmed enemies must be hunted down; recall stays blocked near them.
- Disarmed clears, but any awake enemy vessel still blocks recall.

## Decision

- Clearing and recall as chosen above. A disarmed enemy is salvage-to-be.
- Director defaults (lead): sites spawn when a player vessel comes within 600 m, recall needs no armed awake enemy
  within 200 m (`SimConfig::voyage`, placeholders); a sleeping site also wakes when one of its ships is hit or
  stops running; a site with only wrecks is quiet and never needs clearing; clearing is final; site layout keeps
  5 m gaps within 30 m of the site centre, places the goal last at the farthest of 8 candidate spots, and the
  entry at (0, −radius); each voyage has exactly one goal site with guaranteed ships.

## Consequences

- The sim's clearing and recall checks look at working weapons; the Warden's content gets a gun on its heart.
