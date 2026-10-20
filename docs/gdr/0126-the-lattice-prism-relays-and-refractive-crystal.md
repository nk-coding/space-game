# GDR-0126: The Lattice: prisms relay a laser's beam further and harder, refractive crystal turns lasers away, a resonator is their boss

- Status: Accepted (provisional, lead), 20 Oct 2026, for Niklas to review. The design points marked (Niklas) are his
  of 20 Oct 2026; the rules' details and every number are lead defaults.
- Date: 2026-10-20
- Decided by: Niklas (the design, 20 Oct 2026) and the lead session (agent LATTICE: the rules, numbers and content;
  defaults for Niklas to review)
- Related: [GDR-0027](0027-races.md) (the inner band's races), [GDR-0026](0026-procedural-galaxy-and-bands.md),
  [GDR-0116](0116-the-spire-guards-the-inner-bands-gates.md) (the Spire and its prism ward),
  [GDR-0031](0031-materials-and-fabrication.md) and [GDR-0108](0108-materials-economy-workshop.md) (materials, the
  workshop's race upgrades), [GDR-0099](0099-the-grown-and-the-outer-reach.md) and
  [GDR-0103](0103-the-grown-in-play-defaults.md) (wary races, how the Grown were built),
  [GDR-0117](0117-drones-and-the-swarm.md) (the Swarm, the last race built end to end),
  [GDR-0110](0110-anchored-stations.md) (anchored stations), [GDR-0018](0018-weapon-families.md) and
  [GDR-0021](0021-shields.md) (weapon families, shield shares), [GDR-0020](0020-damage-v1-defaults.md) and
  [GDR-0084](0084-chain-reactions-v1.md) (kinetic ricochet, armour against blasts), GDR-0127 (the Phase walkers, the band's other race),
  `docs/sim.md` "Prisms, refraction and territory (GDR-0126)", `docs/results/lattice-balance-2026-10-20.md`

## Context

GDR-0027 gave the inner band to the Lattice ("long-range prisms and refractive crystal armour", neutral, the most
likely allies) and the Phase walkers; the band was still the uncharted stub of rim automatons, with the Spire at its
gates (GDR-0116). On 20 Oct 2026 Niklas decided the shape: **(Niklas)** the Lattice are neutral, wary like the Grown
and also hostile when a ship comes too close to their crystal sites or strips their wrecks (a warning first if it is
cheap to show; no standing system yet); their signature is the **prism relay**, a prism part that takes the beam of a
neighbouring laser and focuses it, more reach and damage per prism in the chain, researchable by the player as a
ship-building puzzle; **refractive crystal armour** reflects energy shots at an angle (the mirror of kinetic
ricochet) and the reflections can hit other ships, the shooter's allies included; their boss is an anchored crystal
**resonator** with prism spokes that relay beams across the structure, breaking spokes cutting its range; in the
inner band each cluster belongs to one faction, mixed cluster by cluster, the Lattice the more common; a Lattice
research family with the **crystal annealer** installed by the node of the first Lattice part.

## Options considered

- **How a chain forms**: prisms only straight ahead of the laser (simple, no turning, little puzzle), adjacency with
  any facing (ambiguous where several prisms touch), or **the first part in the line of fire** (chosen): a prism that
  is the first part in a laser's (or the previous prism's) line, entered by any side but its front, sends the beam on
  from its front. It is one rule for the drydock and the sim, a prism can turn a beam, and when a prism is shot out
  the beam crosses the gap to the next part in line.
- **Merging**: several lasers into one prism sum into one beam (new shot kinds, pulse and beam mixed), or **each
  laser's shot goes on alone** (chosen): beams entering one prism leave together from its front, each with its own
  numbers; a resonator's three spokes fire three beams out of one mouth.
- **What counts as a laser**: a hard-coded family, or **the prism's `target` selector** (chosen, content: `Tag("laser")`:
  the pulse, burst and beam lasers and the Lattice's focus emitter; cannons, missiles and ion guns are blocked).
- **The refracted shot's owner**: the shooter's (it would pass the shooter's friends, against Niklas's design), the
  crystal ship's (it would pass the Lattice's own fleet), or **nobody's** (chosen): it passes only the ship it glanced
  off and hits anyone else, the shooter included: a flat face hit head on throws the beam back at the gun.
- **The proximity provocation**: the encounter's wake radius (it already means "wakes on sight" for hostile races), or
  **a territory on the faction** (chosen: `territory: (wake, warn)`), with a warning ring at `warn` and the wake
  closer in.

## Decision

- **Prism relay** (`prism: (target, range, damage)`, `data::ShipGrid::beam_path`, `sim::Ship::beam_path`): the first
  part in a gun's line of fire that is a prism whose `target` picks the gun, entered by any side but its front, sends
  the beam on from its front; at most **4** prisms (`MAX_PRISMS`); a prism facing back at the beam, met twice, or a
  fifth blocks; a gun wider than one cell relays only through one prism over its whole front. Each prism adds its
  `range` and `damage` shares of the gun's own (summed). The prism: **+25 % reach, +15 % damage**, 1×1, 3 BP, 0.8 hit
  points, no power. In the sim only alive parts of the gun's piece stand in the way (a prism on a split-off piece
  does not relay). The mount fires from the last prism's front.
- **Refraction** (an armour's `refract`): an energy hit-scan shot (beam tick or pulse) that strikes a hull part of
  refractive armour leaves `refract` of its damage along the mirrored line (the surface normal) for the rest of its
  reach as nobody's shot (shields, then the first part it meets); the part takes the rest; at most **2** glances
  (`MAX_REFRACTIONS`). Refractive crystal: **0.5**, 1.5 t and 0.9 hit points a cell, build points ×1.4, blast factor
  0.7 (weaker than steel against blasts and shells).
- **Territory** (a faction's `territory`; only wary factions): a sleeping site of its ships warns once when a player
  vessel comes within `warn` (`SimEvent::SiteWarned`: a ring of light from each of its ships) and wakes within
  `wake`; the Lattice **warn at 420 m, wake at 260 m**. Stripping their wrecks wakes them, as it wakes any wary site.
- **The Lattice** (faction `lattice`, wary, a pale ice-blue accent `#bfe9ff`, crystal hulls `#162638`): the resonant
  core (mind and power, 7 MW, 4 rules, sees 900 m; leaves resonant crystal), the focus emitter (their laser: a beam,
  **1.2/s at 36 m**, alone a short gun), the prism. Ships: the needle (3×6, two prisms: 54 m, 1.56/s), the lance (5×7,
  two spines of three: 63 m, 1.73/s each, two cores), the guard (5×5, a flat crystal front, one prism) and the
  resonator (13×8, anchored: three spokes of an emitter and three prisms each into one central prism that fires up an
  open lance mouth, four prisms each: 72 m, 1.92/s, three beams; four cores; short emitters on its arms and back).
  Tactics: needles and lances snipe from 85 % of their reach, back off inside half of it and flee below 30 % hull;
  guards close to half their reach, flat front on; the resonator turns onto what comes within 110 % of its reach.
- **The inner band**: cluster types `lattice_reach` (weight 3) and `crystal_deep` (2) in the band's weighted list,
  which the walkers' types join with less weight in all (the Lattice the more common); encounters `lance_watch` (two
  or three needles), `lance_pair`, `crystal_site` (a guard and a needle among needle wrecks), `lattice_dead`,
  `resonator_site` (the resonator and two needles) at the end of a `lattice_resonator` trail (two per galaxy; three
  resonant crystal and six Lattice crystal into store); crystal rock (mines Lattice crystal); a pale sky. The gates
  keep the Spire.
- **The player's side**: the Lattice research family (`crystal_plating`: refractive crystal, 10 Lattice data and a
  resonant crystal → `prism_relay`: the prism and the crystal annealer, 16 Lattice and 4 energy data →
  `crystal_ward`: the Spire's prism ward, 28 Lattice and 6 shields data, 3 resonant crystal). The prism ward is made in
  the annealer now (GDR-0116 left it enemy only). Destroyed prisms leave a shard of Lattice crystal.
- **Look**: a relayed beam runs visibly from the laser through each prism (a glint on each) to the exit; a glance
  flashes where it strikes and draws the turned-away shot in the Lattice's colour; crystal hulls glint now and then;
  the drydock's ARCS overlay draws the relay path with a diamond on each prism and the line of fire from the exit.

## Consequences

- Numbers are placeholders; the runs are in `docs/results/lattice-balance-2026-10-20.md` (60 seeds, the headless
  voyage runner): Keeper-capable fleets clear a Lattice watch voyage 45–50 %, a watch and a lance pair 37 %, a
  crystal site 72–77 %, the resonator 25–33 % (the Keeper's gate 48–60 %, a Swarm carrier voyage 33–38 % on the same
  runs). Reach decides the fight with the runner's pilot: the first cut (88 m needles) cleared nothing, so the Lattice
  are long only through their prisms, not beyond the starters' guns. Refraction costs the fleets a quarter to a third
  of their clears (t_site 97 % without it).
- A player's pulse laser behind four prisms reaches 120 m: the prism is strong in the drydock; its glass and the
  chain's length are the price. Watch it in the part reports (GDR-0124).
- Open: standing with the Lattice (GDR-0027 calls them the most likely allies), a resonator that rebuilds its prisms,
  refraction off shields, the walkers' share of the band's clusters.
