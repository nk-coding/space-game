# GDR-0021: Shields are one bubble per powered emitter; bubbles stack and collapse when emptied

- Status: Accepted
- Date: 2026-10-10 (bubbles); 2026-10-11 (v1 rules confirmed)
- Decided by: Niklas
- Related: [GDR-0014](0014-power-tiers-and-weights.md), [GDR-0018](0018-weapon-families.md), [GDR-0019](0019-damage-model.md), [GDR-0020](0020-damage-v1-defaults.md), `docs/sim.md` "Weapons"

## Context

The 10 Oct concept: a bubble per emitter over nearby blocks, overlapping bubbles stack, they need power; energy is
absorbed well, kinetics partly pass, missiles detonate on the bubble. Weapons v1 needed concrete rules; the lead
proposed them on 11 Oct and Niklas confirmed them the same day. (`docs/sim.md` still calls them lead defaults.)

## Options considered

Not recorded beyond the concept and the confirmed v1 rules.

## Decision

- Each powered emitter projects its own bubble with its own recharging charge.
- A bubble absorbs energy fully, kinetic damage 50% and missiles fully.
- An emptied bubble collapses and stays down until it has recharged 30%.
- Overlapping bubbles stack along the shot: each takes its share of what reaches it.
- Shields are drawn as dotted rings (GDR-0007).

## Consequences

- Kinetic weapons are the answer to shields, energy weapons to armor (GDR-0018).
- Shield emitters compete for power (GDR-0014); per-family shares live in content.

## Notes

- 2026-10-12: shields are no longer drawn as dotted rings but as glowing energy bubbles,
  [GDR-0043](0043-shields-as-energy-bubbles.md) (Niklas). The rules above are unchanged.
