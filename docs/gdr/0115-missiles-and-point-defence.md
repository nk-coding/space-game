# GDR-0115: Missiles home, blast by the chain-reaction rule and can be shot down; point defence aims itself

- Status: Accepted (provisional, lead)
  2026-10-19: point defence needs a line of sight (lead default, an open question of the first version): one ray
  cast from the target back to the turret's muzzle that meets no hull (its own ship's but the turret itself, an
  ally's, an enemy's, a wreck's) and no rock; it looks at the nearest three missiles, then three small vessels, and
  holds when it sees none (the laser starter with point defence against a lancer pack 47 of 60 before, 46 after).
  2026-10-19: flares (lead defaults, GDR-0018's other counter): the flare launcher (1 × 1, 2 BP; 3 releases a fight,
  6 s apart, of 3 decoys at 14 m/s that burn 2.5 s; enemy homing missiles within 12 m of one turn onto it and are
  spent on it, no blast), fired by the FLARE key (N), the behavior action `flare` (it steps aside like `scan`) or the
  assist `auto_flare`, both meant for `when missiles_incoming >= 1`; the missiles research's `flares` node after
  point defence. The laser starter with flares for its nose slopes wins 28 of 60 against a lancer pack (17 without,
  47 with point defence), 51 against a lone lancer.
  2026-10-19: the lancer carries one rocket pod (two before): the laser starter won 4 of 60 against a lancer pack,
  12 against the old lancer alone, 17 now; point defence 47, rockets 48, cannons 50; the rocket pod is unchanged.
  The Spire's encounter (GDR-0116) has two lancers and is a little easier for it.
- Date: 2026-10-19
- Decided by: lead session (default for Niklas to review)
- Related: [GDR-0018](0018-weapon-families.md), [GDR-0021](0021-shields.md), [GDR-0048](0048-no-friendly-fire.md),
  [GDR-0084](0084-chain-reactions-v1.md), [GDR-0085](0085-chain-reactions-v2.md), [GDR-0024](0024-tech-trees-no-mk-levels.md),
  `docs/sim.md` "Missiles and point defence (GDR-0115)", `docs/results/missiles-2026-10-19.md`

## Context

GDR-0018 named missiles (rocket swarms, torpedoes: strong against large, slow targets; countered by point defence
and flares) as the third weapon family, and GDR-0021 already let shields absorb missiles whole, but nothing fired
one. The content proposals of 18 Oct 2026 ("Missiles and point defence") asked for a homing delivery that can be shot
down, a rocket pod, a torpedo and a point-defence turret, reusing projectiles, the blast rule and shields.

## Options considered

- **Delivery**: a projectile with a seeker (chosen: the same flight, shield and no-friendly-fire rules, one list,
  one way to draw it) or missiles as bodies in the physics (collisions for free, but hundreds of tiny bodies cost
  far more and need their own ordering).
- **Guidance**: lock-on to the shooter's target (behaviors fire at will at a target state, not an id), or a seeker
  that picks the nearest hostile vessel ahead of it (chosen: fire and forget; works for pilots, behaviors and
  turrets alike; a launch picks the obvious target).
- **Warhead**: a direct hit like a shell, or a blast by the chain-reaction rule at the point of impact (chosen: the
  card's ask; one rule for every explosion, already shown by the BLAST overlay and drawn by the blast effect).
- **Friendly fire**: blasts spare nobody (GDR-0085's lead default for chain blasts), or a warhead spares its
  owner's vessels like every shot (chosen: a missile is aimed, GDR-0048).
- **Point defence**: a gun the trigger fires (one more thing to hold), or one that fires on its own (chosen).
  Hitting missiles: a ray against a missile's circle, or a hit on the missile it aims at (chosen: no colliders to
  test, a hit-scan pulse at a point is exact; the rate and the missile's hit points do the balancing).

## Decision

- **`Delivery::Missile(rate, salvo, speed, accel, turn, life, hp, radius)`**: each shot a salvo of homing missiles
  fanned a little (±0.14 rad, jitter ±0.03 rad from the `Weapons` stream), leaving at the muzzle's velocity plus 35 %
  of the top speed, accelerating to it. The seeker homes on the nearest hostile vessel within 60° of the missile's
  course and within what it can still fly, at launch and whenever its target is gone; the course turns towards
  the target's lead point by at most the turn rate. After `life` seconds it fizzles.
- **Warhead**: where it strikes a hull (or a rock) it deals `damage / rate / salvo` within `radius` m by GDR-0085's
  rule (every part with a cell in reach, armour takes its share and shields what is behind it), on everyone but the
  shooter's own vessels. Bubbles take the missile first (whole, GDR-0021), and the blast's share where it crosses
  another bubble.
- **Point defence** (`WeaponSpec::defence: (arc, ship_mass)` on a pulse gun): always on while its vessel runs; each
  shot at the nearest hostile missile within its range and arc (the missile loses the shot's damage, at 0 it is
  shot down), else at the nearest hostile vessel of at most `ship_mass` t; no target, it holds. Not part of
  firepower, reach, the smart lock aim or fire at will.
- **Fire at will** launches within 0.25 rad of the launcher's facing (guns: 0.12): the seeker does the rest.
- **Cap**: at most 96 live missiles in the world (lead default; a launcher waits while it is full).
- **Behaviors**: the condition `missiles_incoming` counts the missiles homing on the ship; the `skirmisher` no longer
  dodges by orbiting while missiles home on it (it cannot out-turn them).
- **Content** (placeholders, lead defaults):

| Part | Size | Damage/s | Salvo × rate | Per missile | Speed, accel, turn | Life | HP | Blast |
|---|---|---|---|---|---|---|---|---|
| Rocket pod | 1 × 1, 3 BP | 0.9 | 2 × 0.45/s | 1 | 45 m/s, 60 m/s², 100°/s | 2.2 s | 0.2 | 0.5 m |
| Torpedo launcher | 1 × 2, 6 BP | 0.6 | 1 × 0.15/s | 4 | 22 m/s, 12 m/s², 45°/s | 7 s | 1 | 1.5 m |
| Point-defence turret | 1 × 1, 3 BP | 0.9 | 6 pulses/s | 0.15 | arc ±120°, range 28 m | | | small ships ≤ 25 t |

  The launchers explode when destroyed (rocket pod 0.5 within 0.6 m, torpedo 1.2 within 1 m). A missiles research
  family: `rocket_pod` (4 missiles + 2 kinetic data), `point_defence` (2 missiles + 3 energy), `torpedo` (8 missiles,
  after the rocket pod). The automaton lancer (the gunner's frame with two rocket pods) runs with a raider as the
  `lancer_pack`, up to one per hunting reach.

## Consequences

- Balance (`docs/results/missiles-2026-10-19.md`; the starter refitted, arena 1 v 2, 60 seeds, wins): rockets 45 / 56 /
  37 against a gunner pair, a hunter pack and a lancer pack, between the pulse lasers' 21 / 23 / 4 and the cannons'
  43 / 51 / 52, strongest against the armoured hunters; point defence lifts the laser starter against the lancers from
  4 to 47 of 60 and leaves the others as they were. Voyage runner, Corran Reach (120 seeds, clear %): lasers 18,
  cannons 44, rockets 24, lasers + point defence 36; with its two hunter packs made lancer packs 14 / 42 / 41 / 45.
  The first numbers (1.2 damage/s in salvos of three, 140°/s, a 0.5 rad launch cone) made a lone lancer beat the
  starter 98 % and the rocket starter beat every fit: homing fire at will lands where fixed guns rarely bear.
- Every fight with a missile changes the state hash (`Projectile::missile`); fights without missiles keep theirs.
  Galaxy interiors of hunting reaches roll differently (a new site entry).
- Open: whether a warhead should spare friendly vessels, and missile, point-defence and flare numbers (flares and
  point defence's line of sight: status lines of 19 Oct 2026).
