# GDR-0132: The EMP burst (a ring of static; blinded drones fall apart) and overdrive (every part past its limits, at the risk of burning)

- Status: Accepted (provisional, lead). Niklas decided both features on 20 Oct 2026 (EMP: a triggered ring of static
  about 150 m round the ship, long cooldown, shields absorb it like the ion cannon's static, no engine blackout;
  overdrive: one ship-wide key and a behavior action, about +50 % for 5 s, heat climbs fast, overheated parts take
  damage, a reactor can chain-explode, then a cooldown); the numbers and the rules below are lead defaults.
- Date: 2026-10-20
- Decided by: Niklas (the features), lead (the rules and numbers, for him to review)
- Related: [GDR-0004](0004-grid-ship-building-no-wiring.md) (overdrive), [GDR-0018](0018-weapon-families.md) (ion
  and EMP), [GDR-0125](0125-disruption-ion-static-blinds-minds.md) (static and blinding), [GDR-0117](0117-drones-and-the-swarm.md)
  (drones), [GDR-0084](0084-chain-reactions-v1.md) / [GDR-0085](0085-chain-reactions-v2.md) (blasts), [GDR-0120](0120-flight-and-the-fight-viewer-on-a-phone.md)
  (the ability slots round FIRE), `crates/core/sim/src/combat/emp.rs`, `crates/core/sim/src/ship/overdrive.rs`

## Context

GDR-0125 built the ion cannon and left the EMP burst open; GDR-0004 named overdrive ("burst output at the risk of
damage or an explosion") and nothing built it.

## Decision

**The EMP burst** (content `emp: (radius, static, cooldown)`; the part `emp_emitter`, 1 × 1, 4 BP, 2 t, 1.5 HP, draws 4
(tier Low) only while it recharges, heat 1.5 / limit 4; `emp: (radius: 150, static: 3, cooldown: 25)`):

- Asked by the EMP key (U) or the behavior action `emp` (needs no target, steps aside like `flare`), the first
  recharged working emitter on the controlling piece releases its burst at once (`SimEvent::EmpBurst`).
- Every **enemy** vessel whose controlling piece's centre of mass lies within the radius takes `static` (no core
  bonus). **Never the own ship, the own fleet or its drones** (lead default: the burst knows its own). Salvage, rocks
  and missiles take nothing.
- **Shields**: the target's bubbles that cover its centre but not the burst's take the static first, point for point
  as they take a shot's damage (every emitter absorbs Disruption whole, GDR-0125); a base bubble of 2 holds most of
  a burst and collapses. What is left fills the cores (`Sim::add_static`, the ion cannon's rule): 2 per alive core,
  then blinded 3 s, immune 4 s. So a burst blinds a one-core ship whose bubble is down and puts 3 of 4 into a two-core
  ship (an ion cannon finishes it).
- **Drones**: a drone holds 2 static, so a burst it takes in full blinds it, and **a blinded drone falls apart**
  (removed with `DroneLost`, as when it loses its core; the carrier rebuilds it). This holds for any blinding,
  the ion cannon's too. Before this rule a blinded drone flew `escort` back to its carrier for 3 s and returned.
- **No engine blackout.** The emitter then recharges `cooldown` seconds at full power (less power slower, overdriven
  faster).
- The logic research's `emp_burst` (10 logic + 4 energy data, requires `ion_cannon`) unlocks it; recipe 3 copper,
  4 crystal, 1 steel; its salvage feeds the logic family. Category defense, icons `emp-emitter(.ship).svg`.

**Overdrive** (every vessel; no part, no research):

- Asked by the OVERDRIVE key (O) or the behavior action `overdrive` (needs no target, steps aside), a running vessel
  that is neither overdriven nor cooling down goes into overdrive for **5 s** (`SimEvent::Overdrive`).
- Meanwhile every piece works at **1.5 ×**: reactor supply, every consumer's and reactor's work (guns fire faster,
  beams burn harder, shields regenerate faster, scans, blinks and EMPs charge faster) and the force and torque of
  engines, RCS, gyros and cores (the flight computer plans with it; rated speeds stay: it accelerates and turns
  harder, it flies no faster).
- **Heat**: work's heat times a further 1.5 (2.25 × in all at full work). Nothing shuts down for heat while
  overdriven; a part past its heat limit **burns** 0.5 hit points a second times heat / limit. A part that burns away
  is destroyed like any other: a reactor, capacitor or drive explodes and the blast can chain (GDR-0084/0085).
- Then it ends (`SimEvent::OverdriveEnded`): parts past their limit shut down until they cool (the crash after the
  burst), and the ship cannot overdrive again for **30 s**. A split-off vessel keeps the parent's state.
- Hashed only while overdriven or cooling down; EMP emitters' recharge only on ships that carry one.

## Consequences

- Tests: `crates/core/sim/tests/emp_overdrive.rs` (a burst blinds an enemy in reach, not one beyond it nor a
  friend; it recharges; a two-core ship holds 3 static without going blind; overdrive fires faster and runs hotter
  for its first 2 s; it ends and cools down; an overdriven pulse laser past its limit burns).
- First numbers: a pulse laser overdriven with the heat factor 2 burned away inside the 5 s (fewer shots than without),
  so the factor is 1.5.
- Built in the client: the EMP (U) and OVERDRIVE (O) keys. **Open** (deadline cut, 20 Oct 2026): the expanding violet
  ring for a burst, the HUD's overdrive indicator (active / cooldown), the touch buttons (EMP beside FLARE, OVERDRIVE
  in GDR-0120's diagonal slot), the drydock's risk hint, the wiki pages, the arena measurements (an EMP ship against
  a Swarm carrier, overdrive in a duel and how often it backfires, `arena report`), the voyage runner, the
  determinism check native against wasm with a scenario that bursts and overdrives, screenshots, and every number.
