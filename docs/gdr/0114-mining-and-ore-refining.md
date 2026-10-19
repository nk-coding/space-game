# GDR-0114: Mining and ore refining: a mining laser cuts a rock's ore into the hold, the workshop's smelter refines it

- Status: Accepted (provisional, lead), 19 Oct 2026, for Niklas to review.
- Date: 2026-10-19
- Decided by: lead session (the content proposal "Mining laser and ore refining"; every number a lead default)
- Related: [GDR-0031](0031-materials-and-fabrication.md) (materials by place, harvesting with tool parts, the hub's
  smelter), [GDR-0104](0104-scenery-rocks-filler-landmarks.md) (rock kinds name their material),
  [GDR-0108](0108-materials-economy-workshop.md) (the workshop, recipes, upgrades within it),
  [GDR-0095](0095-sensors-hidden-sites-and-scans.md) (the `scan` action's pattern), `docs/results/mining-2026-10-19.md`

## Context

Rock kinds have named their material since GDR-0104 (nickel-iron, water ice), and the roadmap of 17 and 18 Oct 2026
left mining open: a laser part, a rock's yield, `RockHit` into ore, a use for the ores. GDR-0031 wants materials to
come from places and recipe chains resolved by the hub's upgrades (a smelter first) without factory logistics. Quiet
clusters had little to bring home but wrecks.

## Options considered

- **How a laser takes ore**: a beam's `cut` (ore units per second at full power) on the gun, added to the rock it
  rests on each tick, a whole unit into the hold at a time (chosen: no new delivery, no random draws, power and heat
  as any gun); damage to the rock with ore as its hit points (not chosen: rocks would need hit points every weapon
  touches, and a cannon would mine); a tractor or a scoop that collects drifting chunks (later, GDR-0031's tractor).
- **What the rock does**: its ore counts down and a spent rock stays as cover, drawn darker (chosen: the collider and
  the steering stay as they are); a rock that shrinks or breaks (not chosen: a new collider and new steering circles
  mid-voyage, the client's rock meshes rebuilt; breakable rock stays open from GDR-0104).
- **A full hold**: the ore drifts loose beside the rock and can be collected (chosen: the cargo rules of GDR-0076);
  the beam cuts nothing (not chosen: the cut would be lost silently).
- **How the smelter refines**: GDR-0031's chains, resolved when paying: a recipe or a research cost short of steel
  takes nickel-iron for it (chosen: no button, no queue, the store keeps the ore as it came home, so GAINED and the
  store show ore); refine everything as the haul is unloaded (not chosen: the ore would never be seen, and ore used
  as itself in costs would vanish); a REFINE button (not chosen: a new control for what the chain does anyway).
- **What the ores are for**: steel (every hull-and-frame recipe; the store's steel is what fights barely bring
  home), coolant for the hottest machines, and nickel-iron itself in the smelter's research (chosen: three uses,
  two of them gated by the smelter, so the first mining run pays for it).

## Decision

- **The mining laser** (`parts/mining.ron`): a 1×1 utility beam, 0.3 damage per second to ships, 40 m, 0.5 ore a
  second (`weapon.cut`, the stat Mining), draw 3, heat 1, recipe copper 2, crystal 1, steel 2; open from the start
  (no research). Several beams on one rock share its cut; whoever completes a unit gets it.
- **Rock yield**: a rock kind's `ore` is units per square metre of its circle (π r², rounded at the roll): iron rock
  0.02 (about 2 to 30 units), ice 0.03 (2 to 21), slag 0.015. A rock with none left is spent (`Rock::ore` 0): it
  stays, gives nothing and is drawn darker (`world.rock_spent`).
- **Ore in the hold**: `Item::Ore { material }`, a cargo unit each, worth 0.25 a unit (`combat::mining::ORE_VALUE`:
  below every part and fragment, so a shrinking hold sheds it first and collecting takes a wreck's parts first);
  home it is one unit of its material (`HaulItem::Ore`), in the voyage report's GAINED and the store. `SimEvent::Mined`
  for each unit (stowed or loose, and whether it was the rock's last).
- **The smelter** (`facilities.ron`, within the workshop, locked): `refine` conversions in data, nickel-iron 2 →
  steel 1 and water ice 2 → coolant 1. The workshop pays a material the store is short of by refining (whole batches,
  `Career::payment`), for recipes and research alike; the research screen's cost chips and the workshop's materials
  column count what refining adds (`Career::available`). Validation: known materials, whole amounts, one step only,
  one way into each material.
- **Research** (`research/mining.ron`): the Smelter, utility family, 3 utility data and 6 nickel-iron.
- **Ore in costs**: coolant 3 in the fusion reactor's and the torch drive's recipes, coolant 2 in Fin tuning III;
  steel everywhere it was.
- **The behavior action `mine`** ("Mine nearest rock", like `scan` needing no target ship): the nearest spawned rock
  with ore within sensor range, while the ship has a working, clear mining gun and a free hold unit; it approaches the
  point of the rock's circle nearest the ship to half its shortest mining reach (`MINE_STANDOFF`), nose on it, and
  only the mining guns fire. Otherwise it steps aside (no target). The content's `miner` behavior is the escort
  template with `mine` between fighting the leader's lock and keeping near the leader.
- **The voyage runner mines**: a ship with a mining laser and hold room mines the nearest rock with ore within 300 m
  when there is no fight or salvage, as `mine` flies it; escorts with one fly `miner`.
- **Balance** (`docs/results/mining-2026-10-19.md`, 120 seeds a row): on a quiet test voyage (wrecks, a raider pair,
  two ice fields and scattered iron) the skiff with a mining laser for one pulse laser and a second cargo bay for its
  scanner brings home 7.6 material units a voyage (4.3 of them nickel-iron) in 2.3 minutes against the rim skiff's
  4.6 in 1.9; with the ore counted at what the smelter makes of it (two for one) that is about 5.3, 2.3 a minute
  against the skiff's fight-and-salvage 2.4. Rare materials (programmable matter, crystal, automaton plate) fall
  from 2.4 to 1.5 a voyage. On the slice's voyages the one-gun miner loses most fights (it clears 0–6 %).

## Consequences

- Quiet clusters are worth a run with a mining fit: cargo room and a mining laser trade against guns, and the hold
  caps a run (8 units in 16 s of cutting for the skiff fit); a bigger hold mines more.
- Fusion reactors, torch drives and Fin tuning III now need coolant: ice mined and smelted, or the parts from intact
  salvage.
- Voyages without mining hash as before (rocks' ore and a behavior's rock enter the state hash only once a mining
  beam cuts or a `mine` rule decides); the content's fingerprint changes, so saved lab runs restore stale.
- Open: drifting or breakable rock, comets, gas and star skimming (GDR-0031's other harvests), a tractor, harvest
  drones, mining in the evolution lab's trials, a mining fit as a content sample ship, the ore value per material.
