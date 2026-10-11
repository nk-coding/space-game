# GDR-0014: Power is allocated by tiers and weights with partial output; logic cores need power

- Status: Accepted
- Date: 2026-10-11
- Decided by: Niklas
- Related: [GDR-0004](0004-grid-ship-building-no-wiring.md), [GDR-0013](0013-vessels-and-piloting.md), GDR-0017, GDR-0021, `docs/sim.md` "Power"

## Context

Every connected piece shares one power pool (GDR-0004). When supply falls below demand, something has to give. The
lead session's provisional rule (11 Oct) was a fixed brown-out order (logic core → shields → engines → weapons →
other) in which at most one consumer is partly fed. Whether a logic core needs power was a lead default as well.

## Options considered

- **A fixed brown-out order, all or nothing per part** (the provisional rule).
- **Customisable per part, with partial output** (chosen).

## Decision

- Every consumer part has a **tier** (critical / high / normal / low / off) and a **weight** within its tier.
  Defaults come from the part type in content; the player overrides them per part in the drydock (for example
  front shields high, rear shields low); behaviors may change them in battle later.
- When supply < demand, tiers are served in order. Inside the tier that can't be fully served, the remaining
  power is shared by weight, so parts run at **partial output** (weapons fire slower, shields recharge slower,
  engines push less). A part type may define a minimum level below which it switches off.
- **Logic cores need power:** an unpowered piece goes dark until power returns. This supports "kill all power →
  capture" (GDR-0017).

## Consequences

- Queued in the roadmap: tiers and weights, per-part overrides in blueprints, minimum operating levels. The sim
  uses the fixed order until then.
- Power layout becomes a drydock decision as well as a fight-time one.
