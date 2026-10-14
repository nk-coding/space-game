# ADR-0037: Ship ids decide nothing in a fight: simultaneous fire within a tick, shared think ticks, auto orbits by both ids

- Status: Accepted (provisional, lead)
- Date: 2026-10-14
- Decided by: lead session (a correctness fix it ordered; default for Niklas to review)
- Related: [ADR-0010](0010-cross-platform-determinism.md), [ADR-0036](0036-arena-crate.md), [GDR-0034](../gdr/0034-behavior-first-match-fire-at-will.md) (its id-staggered think cadence is replaced here), `docs/sim.md` "The tick", "Weapons", "Behaviors", `docs/arena.md` "The fight spec" (spawn order)

## Context

The arena (ADR-0036) found that in a mirror fight the side spawned first, whose ships get the lower ids, wins: 2000
`rim_skiff` mirrors gave 1173 / 378 (449 undecided), and spawning side 1 first turned it round (425 / 1141). The
arena alternated the spawn order by seed parity as a stopgap. Measured one cause at a time (each the same 2000 or 1000
seeds with either side spawned first; the full numbers are in `docs/arena.md`), three id-order effects in the sim:

- **Weapons**: the guns fired ship by ship in id order and each hit landed at once, so a lower-id ship's pulse could
  shoot an enemy gun (1 hit point) off before that gun fired in the same tick. Fixing this alone: skiff mirror
  863 / 392.
- **Think cadence**: behaviors thought every 3 ticks staggered by id (`(tick + id) % 3`, GDR-0034), so of two
  enemies one always reacted a tick sooner. With the weapons fixed and the stagger removed: 0 / 0 / 2000 (every
  skiff mirror a mutual disarm, both orders). The stagger removed alone: 1181 / 0 (the first strike decides).
- **Auto orbit**: `orbit(r)` without a direction went by the ship's own id parity, so two ships circling each other
  went opposite ways depending on who had the lower id. Hunter mirrors (`raider_basic` orbits): 286 / 81 before,
  353 / 49 with the first two fixes, 0 / 0 / 1000 with this one (side 1 first: 1 / 0 / 999).

## Options considered

- **Keep the arena's stopgap**: seeds in pairs cancel the bias on average, but every single fight still carries it,
  and the game's own fights (voyages, where the player's ship usually spawns first) keep it. Not chosen.
- **Weapons**: apply each hit at once but in a fair order (alternating or seeded by tick): still one ship first in
  every tick. **Trace every shot against the state at the start of the fire pass, then apply all hits** (chosen):
  fire is mutual, as the plume pass already did; a hit whose part an earlier hit destroyed goes on to the part
  behind.
- **Cadence**: keep a stagger but from a seeded hash of the id: fair over many fights, still unfair in each one.
  **Every ship thinks on the same ticks** (chosen): the think pass is cheap next to a tick (the 200-ship load test
  shows no difference beyond noise), and a fleet's thinks were never a cost spike worth spreading.
- **Auto orbit**: by the geometry (keep turning the way the ship already moves): fair, but changes what a fleet
  does. **By the parity of the ship's and its target's ids together** (chosen): half of a fleet still circles a
  target each way (the content's "Half of a fleet circles each way"), and two ships circling each other go the
  same way, whoever has the lower id.

## Decision

- Weapons: beams and pulses are traced (ray cast, shield bubbles along the path) against the state at the start of
  the fire pass; their hits are applied after the last gun has fired, in firing order (ship, piece, gun). A hit
  whose part an earlier hit of the pass destroyed is cast again from its muzzle to the part behind (no second shield
  pass). Projectiles in flight still move before the guns fire, and plumes burn after the hits, for every ship alike.
  RNG draws are unchanged (the weapons roll nothing).
- Behaviors think when `tick % 3 == 0` (and on their first tick), every ship on the same ticks.
- `orbit(r, auto)` circles counter-clockwise when the ship's and its target's ids add up to an even number.
- The arena spawns side 0 first again, and its test checks that mirror fights end the same in either spawn order.

## Consequences

- Every hash stream changed (scenarios, arena fights, voyages); native vs wasm32 is identical again
  (`docs/sim.md` "Determinism", `docs/arena.md` "Determinism").
- Mirror fights of equal ships now usually end in a mutual first strike: both lose their nose guns on the same tick.
  The skiff mirror is a draw at 0.500 / 0.500 in all 4000 seeds, and every mirror in the arena matrix scores 0.50.
  Balance moved a little: in the matrix the fights with the hunter most (the raider against it 22 → 60 %); the slice
  voyages, goal cleared and home of 120 seeds, the starter 55 / 36 / 20 / 3 % → 56 / 31 / 18 / 7 %, the Runner
  66 / 78 / 67 / 35 % → 70 / 69 / 74 / 36 % (mostly within noise; `halvex_drift` a little harder). Details in
  `docs/arena.md` and `docs/sim.md` "The headless voyage runner".
- No cost: `sim_perf` and the load test before and after are the same within noise (`docs/sim.md` "Performance"),
  the arena's cost per tick too.
- One order effect is left, in the physics: Rapier's sequential contact solver resolves a collision between two
  bodies a little differently depending on their order, so head-on rams can still tip a fight (`sample_mid` mirror:
  37 of 1000 seeds end differently with the spawn order swapped). Changing the solver is not worth it now; the arena
  numbers show it.
- Guns still start with cooldowns staggered by a hash of ship id and part (GDR-0020); a gun that is not firing
  reloads to ready, so this matters only for ships that spawn in range of each other.
