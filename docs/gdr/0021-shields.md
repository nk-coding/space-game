# GDR-0021: Shields are one bubble per powered emitter; bubbles stack and collapse when emptied

- Status: Accepted. 2026-10-16: the point-blank pass is the muzzle's only; a projectile in flight is taken by every
  bubble once, also by one whose ship moved onto it between ticks (a bug fix approved by Niklas; Notes)
  2026-10-17: the shield emitter's capacity 3 → 2 (Niklas, [GDR-0100](0100-fight-balance-targets.md) B2; regen 0.2
  stays): a bubble collapses in about one wave in six under sustained fire
  (`docs/results/slice-balance-2026-10-17b.md`). The rules above are unchanged.
  2026-10-19: missiles exist ([GDR-0115](0115-missiles-and-point-defence.md)): a bubble takes a missile whole up to its
  charge, as decided; a missile it does not stop goes off at the hull with what is left.
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
- 2026-10-16: flying at about 20 m/s at enemies, Niklas saw automaton cannon shells hit his hull whole with his
  shields up. Ships move after the weapons in a tick, so a ship closing on a shell can carry its bubble past the
  shell's last position between two ticks (0.67 m a tick at 20 m/s); the shell then started its next segment inside
  the bubble and passed it under the point-blank rule (a shot fired from inside a bubble passes it), which was meant
  for the muzzle only. Fixed the same day, Niklas approving: a shot passes only the bubbles its muzzle is inside when
  it is fired; a projectile in flight is taken once by every bubble it meets, also one it finds itself inside
  (`docs/sim.md` "Weapons"). The rules above are unchanged. Every fight with shields and shells changes, so the
  stream hashes and the rules' fingerprint are new ([GDR-0092](0092-evolution-lab-for-your-own-ships.md),
  `docs/sim.md` "Determinism").
