# GDR-0009: Controls: stick + lock with an always-on flight computer; letting go brakes

- Status: Accepted; PC/WASD part superseded by [GDR-0040](0040-keyboard-flight-and-smart-lock-aim.md) (2026-10-12); soft top speed superseded by [GDR-0041](0041-engine-rated-speed.md) (2026-10-12: top speed from each engine's rated speed); "letting go brakes" superseded by [GDR-0086](0086-letting-go-coasts.md) (2026-10-14: letting go coasts)
- Date: 2026-10-10
- Decided by: Niklas
- Related: [GDR-0010](0010-turning-sources.md), [GDR-0011](0011-locked-flight-rear-engines-trade-off.md), [GDR-0015](0015-layered-behavior.md), [GDR-0016](0016-combat-pace-and-aiming.md), [GDR-0023](0023-shot-leading.md), `docs/sim.md` "Flight", `docs/spike-plan.md` "Flight model"

## Context

Android first, with heavy physical ships and fixed guns aimed by turning. A flight computer turns the player's
intent into thrust (GDR-0015). The braking, drift and top-speed questions were answered on 10 Oct before the
spike.

## Options considered

- **Twin sticks**: familiar from mobile shooters, but turning a heavy physical ship with a thumb feels sluggish,
  and fixed guns make aiming hard.
- **Drag to steer** (drag = go here, tap = target): least effort and fits the flight computer, but feels more
  like commanding than piloting.
- **Stick + lock** (chosen): stick moves, tap an enemy to lock it; the flight computer keeps the nose on the
  target while you strafe. Catch: needs good target cycling in busy fights.

## Decision

- Without a lock the stick means "fly that way"; how far you push sets the speed.
- Tap or click a ship to lock it: the flight computer keeps the nose on it and the stick strafes.
- Letting go brakes to a stop with the ship's own thrusters by default. A drift toggle (player setting) turns
  braking off.
- Soft top speed from thrust and mass (implemented as `k × √(thrust / mass)`, a placeholder). (Superseded on 12 Oct
  2026 by GDR-0041: the top speed comes from each engine's rated speed.)
- PC: WASD acts as the stick; the mouse locks and fires. (Superseded on 12 Oct 2026 by GDR-0040: ship-relative
  keys, smart lock aim.)

## Consequences

- Target cycling matters (PC: Tab for the next target).
- Locked flight needs thrusters that push sideways and forwards (GDR-0011); turning needs enough torque
  (GDR-0010).
- Open: the feel of "fly that way" for forward-only ships, and ship speed (`docs/gdr/README.md` "Open").
