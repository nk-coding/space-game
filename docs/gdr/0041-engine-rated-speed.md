# GDR-0041: Top speed comes from each engine's rated speed; no drag; a calm cruise flame at the cap

- Status: Accepted; the fade of thrusters pushing with the motion is refined by [GDR-0055](0055-slower-crisper-turning.md) (whole speed, not the speed along the axis)
- Date: 2026-10-12
- Decided by: Niklas (the falloff shape, the partial-stick rule and the cruise-flame look are the implementing
  agent's choices, documented in `docs/sim.md`)
- Supersedes: GDR-0009 (only its "soft top speed" bullet)
- Related: [GDR-0009](0009-stick-and-lock-controls.md), [GDR-0010](0010-turning-sources.md), [GDR-0011](0011-locked-flight-rear-engines-trade-off.md), [GDR-0040](0040-keyboard-flight-and-smart-lock-aim.md), [GDR-0042](0042-agility-target.md), `docs/sim.md` "Top speed and the cruise flame", `docs/client.md` "Plumes", `docs/content-model.md` "Parts"

## Context

The soft top speed (`top_speed_k × √(thrust / mass)`, GDR-0009) capped every ship by a formula. Niklas played the
web build on 12 Oct 2026: the cap felt weird because the engines went completely dark once the ship reached it,
and a fixed cap is wrong because it cannot be upgraded per engine, so a fleeing enemy can never be caught by
better engines.

## Options considered

Top speed:

- **Engine rated speed** (chosen): every engine and RCS pod has a rated speed; its thrust fades out as the ship
  approaches it, so the top speed comes from the engines and improves with them.
- **Space drag**: a drag force grows with speed, the top speed is where thrust equals drag. Reason not recorded.
- **No cap**: ships accelerate for as long as they thrust. Reason not recorded.

At the cap:

- **Calm cruise flame** (chosen): while the pilot holds the stick at top speed the plume settles into a steadier,
  slightly shorter flame with a hotter core, so cruising looks different from accelerating.
- **Full burn stays**: the plume keeps burning as if accelerating. Reason not recorded.
- **Pulsing cruise**: the plume pulses while holding speed. Reason not recorded.

## Decision

- Each engine (`engine.rated_speed`) and each RCS pod (`rcs.rated_speed`, per nozzle) has a rated speed in m/s:
  content data, placeholder numbers, stats `rated_speed` and `rcs_rated_speed` with label and unit, so bonuses
  and later upgrades raise it per engine.
- A thruster's thrust fades out as the ship's velocity along that thruster's push direction approaches its
  rating: full thrust up to 80% of the rating, then linearly down to none at 100% (agent's choice: flat first
  keeps the acceleration constant for most of the way and the approach to the top speed short). When the
  velocity points against the push direction (braking) or across it (turning), the thruster keeps full thrust.
- A ship's top speed in a direction therefore comes from its fastest thrusters pushing that way.
- **No drag**: a coasting ship keeps its speed. Holding a speed costs no thrust, power or heat.
- **Cruise flame**: while the helm holds a velocity (stick, keys, a maneuver), every engine shows a calm flame
  scaled by how much of its axis's top speed the velocity goal asks along that engine: full stick at the top
  speed = full cruise flame, half stick = half (agent's choice for partial stick: holding a lower speed reads as
  a gentler cruise flame, not as dead engines). Released stick (braking) and drift (coasting) keep today's
  behaviour: brake, or coast with the engines off. RCS nozzles only puff when they push.
- The soft top speed (`top_speed_k`) is removed.

## Consequences

- Better engines catch fleeing ships; a ship with mixed engines loses the slower ones' thrust first near the
  top. Base content (placeholders): thruster 20 m/s, torch drive 22 m/s, RCS pod 14 m/s.
- The flight computer allocates the fallen-off thrust, so velocity control stays exact and nothing jitters at the
  cap; the sim exposes per thruster the falloff, the effective output and the cruise level for the plumes.
- The rule works per thrust axis, so the reachable velocities form a box in the ship's frame: moving
  diagonally to its axes a ship can be faster than either rating (up to the box corner). "Fly that way" turns the
  best axis onto the stick and flies at that axis's rating; a locked ship strafing diagonally, or a ship with
  strong side thrusters flying crabwise, can go faster. Open whether that is wanted (see `docs/sim.md`).
- `SimConfig::speed_scale` scales every rating for feel tests (tuning panel).
