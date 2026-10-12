# GDR-0049: Engine plume v1: damage and heat scale with thrust, blocked plumes keep thrust, enemy bubbles absorb plumes

- Status: Accepted (provisional, lead)
- Date: 2026-10-12
- Decided by: lead session (defaults for Niklas to review; numbers and the per-lane details by the plume agent)
- Related: [GDR-0039](0039-engine-exhaust-hurts-parts.md) (answers its open questions), [GDR-0046](0046-exhaust-hits-own-parts-and-enemies.md) (who is hit), [GDR-0041](0041-engine-rated-speed.md) (thrust fades at the rated speed), [GDR-0021](0021-shields.md) and [GDR-0045](0045-shields-absorb-rams-no-friendly-ram-damage.md) (bubbles), [GDR-0022](0022-heat-per-part.md) (heat), `docs/sim.md` "Engine plumes", `content/base/parts/propulsion.ron`

## Context

GDR-0039 lets engines sit anywhere and makes their plume damage and heat the parts in its path; GDR-0046 says
whose parts: the engine's own vessel, enemies and salvage, never other vessels of the same owner. Left open for
the implementation: the plume numbers per engine, whether damage and heat scale with the throttle, whether a
blocked plume still gives full thrust, and whether shields stop it. The lead session set defaults for Niklas to
review when it handed out the implementation.

## Options considered

How hard the plume burns:

- **With the engine's actual thrust output this tick** (chosen): throttle × power level × rated-speed falloff.
  An idle engine burns nothing, an engine holding its rated speed (no thrust left, a calm cruise flame only)
  next to nothing, a browned-out one less.
- **With the throttle only**: a ship cruising at its top speed would burn at full while giving no thrust. Not
  chosen.
- **Constant while the engine works**: not chosen.

A blocked plume:

- **Full thrust** (chosen): the cost of a buried engine is the burning, not lost thrust.
- **Less thrust the more is in the way**: punishes twice and needs more state. Not chosen.
- **No thrust** (the old clear-exhaust rule): replaced by GDR-0039.

Shields:

- **Bubbles of non-friendly ships covering the plume's hit point absorb it like energy damage** (chosen), and
  take the heat in proportion; a ship's own bubbles never stop its own exhaust (else a shield would make a
  buried engine free), and friendly vessels are not hit at all (GDR-0046).
- **Shields ignore plumes**: not chosen.
- **Every bubble, the own ones included**: not chosen (see above).

How far it goes:

- **It stops at the first part it hits** (chosen): that part takes the damage and heat; once it is destroyed
  the plume reaches the next one from the next tick on, so a plume burns its way through.
- **It carries on through parts with falling damage**: more casts and rules, nothing gained yet. Not chosen.

## Decision

- Each engine's plume is content data (`engine.plume`: `length` m, `damage` and `heat` per second at full
  thrust). It runs straight back from the engine's back side, one lane per back cell, the damage and heat shared
  evenly over the lanes (agent's choice for the plume's width: a 2×2 torch drive has two lanes).
- Damage and heat scale with the engine's thrust output this tick (lead default).
- A blocked plume gives full thrust (lead default).
- Bubbles of non-friendly ships covering the hit point absorb the plume like energy damage (their energy share),
  the heat going with the damage; own bubbles never stop the own exhaust (lead default).
- Each lane stops at the first part it hits, on whichever ship, and burns that part (lead default).
- Base numbers (placeholders): thruster 3 m, 1 damage/s, 1.5 heat/s; torch drive 6 m, 3 damage/s, 4 heat/s.
  Measured (tests in `crates/sim/tests/plume.rs`): a Halvex Runner's torch drive held 0.5 m from a raider's nose
  for one second at full thrust deals 2.8 hp and wrecks 2 parts (the Runner pulls 10 m away meanwhile); a
  thruster buried against a composite armour block (1.4 hp) burns through it in 1.4 s at full thrust and through
  the next one 1.4 s later.

## Consequences

- Burying an engine is legal but costs the parts behind it within seconds; the base ships' engines all exhaust
  into open space, so they never burn themselves.
- A plume is a short-range weapon: a raider on a Runner's tail gets burned, and a pilot can turn the torch drive
  on an enemy or on debris. No momentum is transferred (the plume does not push).
- The client gets each engine's reach (`Thruster::plume_length`) and every burn as `SimEvent::PlumeHit` (where
  a plume is blocked); the drawn flame is still the client's own length.
- Open: the numbers; whether a plume should push what it hits; whether the drawn flame should match the sim's
  reach; whether a plume should continue past light parts.
