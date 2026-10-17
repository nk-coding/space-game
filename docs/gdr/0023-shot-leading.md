# GDR-0023: AI ships lead their shots; the player's lock aims at the lead point for projectile weapons

- Status: Accepted (provisional, lead).
  2026-10-17: Accepted (Niklas, 17 Oct 2026).
- Date: 2026-10-11
- Decided by: lead session (default for Niklas to review)
- Related: [GDR-0009](0009-stick-and-lock-controls.md), [GDR-0016](0016-combat-pace-and-aiming.md), [GDR-0020](0020-damage-v1-defaults.md), `docs/sim.md` "Aiming and leading"

## Context

Fixed guns are aimed by turning the ship (GDR-0016). Projectiles fly at finite speed (the automaton cannon at
90 m/s), so aiming at where a moving target is misses it.

## Options considered

- **Aim at the target**: misses moving targets with projectiles.
- **Aim at the lead point** (chosen) for projectile weapons; lasers are hitscan and aim at the target.

## Decision

- AI ships lead their projectile shots (first-order intercept; the shot inherits the shooter's velocity).
- The player's lock aims at the lead point for projectile weapons: aim mode Auto leads when the ship's strongest
  gun by damage per second fires projectiles, otherwise it aims at the target. Target and Lead modes exist too.

## Consequences

- A ship that mixes lasers and cannons aims for whichever does most of its damage; the other kind fires when it
  happens to bear.
- Player aiming gets easier with cannons; whether that feels right is part of Niklas's browser tests.
