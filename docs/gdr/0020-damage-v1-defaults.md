# GDR-0020: Damage v1 defaults: hitscan lasers, kinetic projectiles with ricochet, chain reactions later

- Status: Accepted (provisional, lead)
- Date: 2026-10-11
- Decided by: lead session (default for Niklas to review)
- Related: [GDR-0018](0018-weapon-families.md), [GDR-0019](0019-damage-model.md), [GDR-0021](0021-shields.md), [GDR-0023](0023-shot-leading.md), `docs/sim.md` "Weapons"

## Context

Weapons v1 in the sim needed concrete rules for the damage model (GDR-0019) before Niklas could try fights in the
browser.

## Options considered

Not recorded; these are the first working defaults.

## Decision

- Lasers are hitscan and burn the first block they touch (beams continuously, pulses per shot).
- Kinetic shots are projectiles: they inherit the shooter's velocity, live `range / speed` seconds, carry excess
  damage through up to `penetration` blocks of the same piece, and push the shooter back (recoil).
- Ricochet: a kinetic shot that hits a slope face more than 60° from its normal glances off with 60% speed and
  50% damage (at most two ricochets); the slope takes nothing.
- Chain reactions come later as an `explosion` block in content; the sim has a hook for it.
- Guns start with staggered cooldowns, so a fleet never fires in lock-step salvos.

## Consequences

- All numbers are placeholders (`weapons::tuning` in the sim, weapon stats in content).
- Chain reactions are not in the game until the `explosion` block exists.
