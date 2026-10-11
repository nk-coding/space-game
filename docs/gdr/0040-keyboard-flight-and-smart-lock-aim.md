# GDR-0040: PC keyboard flight controls: rotate + thrust + strafe, smart lock aim with A/D nudge

- Status: Accepted
- Date: 2026-10-12
- Decided by: Niklas
- Supersedes: GDR-0009 (only its PC/WASD part: "WASD acts as the stick")
- Related: [GDR-0009](0009-stick-and-lock-controls.md), [GDR-0010](0010-turning-sources.md), [GDR-0011](0011-locked-flight-rear-engines-trade-off.md), [GDR-0016](0016-combat-pace-and-aiming.md), [GDR-0023](0023-shot-leading.md), [GDR-0034](0034-behavior-first-match-fire-at-will.md), `docs/client.md` "Controls", `docs/sim.md` "Flight"

## Context

The first browser build mapped WASD onto the touch stick ("fly that way" in screen directions). Niklas tried it
on 12 Oct 2026 and found it very weird on a keyboard: the ship turns on its own to follow the keys. Keyboard
players expect to steer the ship itself. Fire at will is per gun since GDR-0034, so a lock that only points the
nose wastes side and rear guns.

## Options considered

- **WASD as the stick** (GDR-0009, the first build): one code path for touch and keyboard, but the ship turns by
  itself; felt wrong.
- **Ship-relative keys** (chosen): A/D rotate, W/S thrust along the nose, Q/E strafe; the flight computer still
  helps (cancels sideways drift, brakes on release), the drift toggle turns that off.

## Decision

- **Keyboard (PC):** A/D rotate the ship, W/S thrust forward and backward along the nose, Q/E strafe sideways
  with whatever can push that way (RCS, side thrusters, gimbal). The flight computer cancels sideways drift so
  the ship goes where it points, and brakes when the keys are released. The drift toggle turns both off (pure
  momentum). The soft top speed still applies.
- **Lock:** smart aim. The lock turns the ship so that its strongest working guns bear on the target (lead point
  for projectile guns), including side or rear guns when they are all that is left; destroyed, overheated or
  blocked guns don't count. Holding A/D adds an aim offset (the nudge) that springs back when released. W/S still
  thrust along the nose and Q/E strafe.
- **Touch:** unchanged: the stick means "fly that way" (push distance = speed), tap to lock.
- **Mouse:** click a ship to lock, click empty space to unlock, hold to fire, as before.

## Consequences

- `sim::PilotInput` has optional ship-relative `axes` (turn, thrust, strafe) next to the touch stick, and an
  `aim_offset` that the client integrates with `sim::flight::nudge_step` once per tick (deterministic input).
  `flight::smart_aim` groups working guns by facing and picks the group with the most damage per second.
- Turning by keys is a rate command (`HeadingGoal::Rate`), capped at a rate the ship can stop from within about a
  quarter turn.
- Placeholders: nudge cap 0.6 rad, growth 1.2 rad/s, spring-back 5 /s.
- Ships with engines only at the back still can't brake or strafe while locked (GDR-0011); S does nothing on
  them.
