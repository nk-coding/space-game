# GDR-0129: The behavior words `blinded`, `cargo` and `leader_near`, a BLINK touch button, and the railgun

- Status: Accepted (provisional, lead), 20 Oct 2026
- Date: 2026-10-20
- Decided by: lead session (open points of GDR-0123, GDR-0125 and GDR-0127; the railgun after GDR-0018 and Niklas's
  GDR-0019 rule; every number a lead default)
- Related: [GDR-0123](0123-behavior-words-for-salvage.md) (salvage words, the `salvager`),
  [GDR-0125](0125-disruption-ion-static-blinds-minds.md) (static blinds minds), [GDR-0127](0127-the-phase-walkers-blink-drives-and-piercing-lances.md)
  (blink drives), [GDR-0120](0120-flight-and-the-fight-viewer-on-a-phone.md) (touch buttons press actions),
  [GDR-0018](0018-weapon-families.md) (the kinetic family), [GDR-0019](0019-damage-model.md) (railguns pierce about
  three blocks, shots stop at the hull), [GDR-0024](0024-tech-trees-no-mk-levels.md) (sidegrades),
  `docs/results/railgun-2026-10-20.md`

## Context

Three records left small points open: GDR-0123 a `cargo` percentage and a word for the distance to the leader, so a
salvager does not fall behind; GDR-0125 a `blinded` condition; GDR-0127 a touch button for BLINK. The kinetic research
family had two nodes (the reverse-engineered cannon and the long-barrel cannon), and GDR-0018's railgun was not built,
though the sim has carried piercing projectiles with recoil since weapons v1.

## Options considered

- **The leader's distance**: a value `leader_distance` compared in metres (not chosen: `%` distances mean a share of
  the weapon reach, which says nothing about a leader); a flag `leader_near(range)` like `wreck_in_range` (chosen).
  Without a leader it is **true** (chosen: a flagship or a lone ship never "falls behind", so `not leader_near` never
  holds for it and a salvager on its own still strips); false (not chosen: a lone salvager would never salvage).
- **`cargo` without a hold**: 100 % (chosen, as `cargo_full` holds for it); no value (not chosen: `cargo >= 80%` and
  `cargo_full` would disagree).
- **The `salvager` template**: unchanged (not chosen); with `leader_near` only (not chosen: measured, no change); with
  `leader_near(80m)` and `enemies_near(300m) < 1` (chosen: the best of seven variants).
- **The voyage runner's escorts**: switch to the improved salvager (not chosen: it still costs goals, and Niklas
  confirmed `escort` on 20 Oct 2026); keep `escort` (chosen).
- **BLINK's place**: left of FLARE (not chosen: a third slot in the row runs towards the thumb's rest); above FIRE
  (chosen: the right thumb reaches it from FIRE, and it never collides with FLARE). Dimmed while the drives charge or
  cool (not chosen: FLARE shows no such state either; the hull's shimmer and the debug view say it).
- **The railgun's size**: 1×2 like the long cannon (not chosen: then it only competes for the same cells); 1×3, a big
  machine (chosen: its space and power are its cost).
- **Its numbers**: GDR-0019's three blocks as `penetration: 3`, recoil 9 kN·s (the long cannon's 3.5, the cannon's
  2). The first cut (130 m, 3 a slug, 0.4 shots/s) was far too strong (the starter cleared the hunter den 82 % against
  44 % with long cannons); 100 m or a slug under 2.4 fell behind everywhere. Chosen: **110 m (the long cannon's reach),
  a slug of 2.5 (0.875/s at 0.35 shots/s), 400 m/s**.
- **Over-penetration**: shots fly on into the next ship (not chosen: GDR-0019 keeps "no over-penetration into another
  ship", revisit with railguns); the slug stops where the struck piece's hull ends, like every projectile (chosen;
  nothing new in the sim).

## Decision

- **Behavior words** (`content/base/behavior_nodes/conditions.ron`, `sim::behavior`): the flag **`blinded`** (static
  blinds the mind, GDR-0125); the value **`cargo`** (the hold's used share; no hold reads 100 %); the flag
  **`leader_near(range)`** (default 300 m: the leader `escort` follows within the distance, centre of mass to centre
  of mass; true without a leader; blind or not).
- **The `salvager`** strips only while its leader is within 80 m and no enemy within 300 m:
  `when wreck_in_range(250m) and not cargo_full and leader_near(80m) and enemies_near(300m) < 1 -> salvage`.
  Measured (320 seeds, rim skiff with two rim skiff escorts): `yard_approach` 229 cleared against 216 with GDR-0123's
  rules and 232 with plain escorts, `halvex_drift` 137 against 131 and 162; haul median 8 and 7 items against 12 and 8
  (5 and 4 with plain escorts). The voyage runner's escorts keep `escort`.
- **BLINK touch button** (`world::controls`): a 44 dp ring above FIRE, 14 dp up and centred over it, for a flown
  ship with a blink drive; a tap is the BLINK action (along the stick while the other thumb pushes it, else ahead).
  The holders of the FLARE and BLINK rings now show again in every flight (they stayed hidden after a start outside
  the flight, so FLARE did not show in a flight entered from another screen).
- **The railgun** (`content/base/parts/weapons.ron`): 1×3, 8 build points, control cost 2, 5 t, 3 hit points, draws
  7, heat 1.8 (limit 4.5); kinetic, 0.875 damage/s in 0.35 shots/s (a 2.5 slug), 110 m, 400 m/s, penetration 3,
  recoil 9; salvage 6 kinetic data; recipe 3 automaton plate, 5 copper, 5 steel; locked. Ammo feeders do not feed it
  (tag `railgun`, not `cannon`). The kinetic research's node **`railgun`** (`research/kinetic.ron`, NewPart, 9 kinetic
  + 3 energy data, after `long_cannon`).
- **Looks** (`world::fx`, `world::sim_bridge`): a projectile with three or more blocks to pierce draws as a white-hot
  needle with a thin wake that fades in 0.45 s; a gun with at least 6 kN·s of recoil kicks when it fires (a bigger
  flash and sparks thrown back past it on both sides). Icons `railgun.svg` / `railgun.ship.svg` (weapons cyan, two
  rails and a capacitor bank).

## Consequences

- The railgun is a sidegrade (`docs/results/railgun-2026-10-20.md`): on the starter it clears the hunter den 58 %
  against the long cannon's 44 % but the Warden 13 % against 36 %, and loses the duel to it; on the Runner it is about
  level with the beams. `arena report` rates both rail designs beside their siblings, no flag.
- New vocabulary changes the content fingerprint (saved lab runs restore stale); behaviors without the new words hash
  as before.
- Open: a BLINK assist; the tractor and a part that makes the runner's escorts salvage (GDR-0123); a railgun for an
  enemy faction; whether any weapon should over-penetrate after all; every number above.
