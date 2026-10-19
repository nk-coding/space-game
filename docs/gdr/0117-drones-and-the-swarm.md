# GDR-0117: Drones launch from hangars while their carrier has a target and die with their hangar; the Swarm fill the middle band

- Status: Accepted (provisional, lead)
  2026-10-19: spitters that matter (lead default): a spitter has no side thrust, so orbiting it never pointed its nose
  stingers at the target (four spitters dealt about 5 damage before they died); its behavior now turns them onto the target for
  five seconds of every eight (`face`) and closes in for the other three, and a Swarm raid has three or four spitters
  (two before). Patrol voyages clear 73–80 %, raid voyages 70–79 % (they cleared every time); the carrier site alone is
  unchanged (45–62 %), but a voyage with a patrol before it is harder: the carrier voyage 21–37 %, the hive 14–37 %
  (`docs/results/swarm-balance-2026-10-19.md`, follow-up).
  2026-10-19 (later): the voyages before a boss, retuned in the encounters (lead default; the spitter is unchanged):
  a spitter patrol has three to five spitters (three or four) and wakes at 220 m (300), so the fleet that creeps up
  on it opens fire first; the hive carrier comes without its spitter (0–1). The Keeper-capable fleets clear the
  patrol-then-carrier voyage 43–49 % (32–37 before), the patrol-then-hive voyage 44–45 % (35–37), the outer-reach
  fleet 29 and 23 % (21, 14); patrol voyages 68–79 %, raids 70–79 % (unchanged), the carrier site alone 55–69 %
  (240 seeds, `docs/results/swarm-balance-2026-10-19.md`, second follow-up).
- Date: 2026-10-19
- Decided by: lead session (default for Niklas to review)
- Related: [GDR-0018](0018-weapon-families.md) (the drones family, hangar blocks), [GDR-0027](0027-races.md) (the Swarm),
  [GDR-0026](0026-procedural-galaxy-and-bands.md) (bands, hives as bosses), [GDR-0031](0031-materials-and-fabrication.md),
  [GDR-0013](0013-vessels-and-piloting.md), [GDR-0044](0044-control-cost-priority-idle-dormant-newest.md),
  [GDR-0110](0110-anchored-stations.md), [GDR-0112](0112-the-keeper-guards-the-middle-bands-gates.md),
  `docs/sim.md` "Drones (GDR-0117)", `docs/results/swarm-balance-2026-10-19.md`

## Context

GDR-0018 named drones (fighters, repair drones: flanking and swarming; they need hangar blocks and rule slots) as a
weapon family, and GDR-0027 gave the middle band to the Swarm: "drone floods from hive carriers; hives are the
bosses". Nothing launched a drone, and the middle band was a stub of the rim's automatons. The content proposals of
18 Oct 2026 ("Swarm vanguard") asked for drones as spawned vessels, a hive carrier, the hive as the band's boss, the
Swarm's materials (chitin, resin, nerve cores), their research family and a hangar of the player's own.

## Options considered

- **What a drone is**: a projectile with a brain (cheap, but no hull, no parts to shoot, nothing the behavior system
  flies), or a small vessel of its own with its own core-equivalent, spawned like any ship (chosen: physics, damage,
  behaviors, shields and the client's drawing all work unchanged).
- **When a hangar launches**: on a trigger or a behavior action (one more thing to hold, one more rule slot), or
  whenever its vessel aims at something (chosen: the pilot's lock or the behavior's target; the same rule for the
  player, an escort, a carrier and a hive).
- **Whom drones fight**: the nearest enemy each picks (a flood scatters over every ship in reach, wakes sleeping
  sites), or what the carrier aims at (chosen: the `locked` selector with the carrier as leader; a player's drones
  attack the lock).
- **Range**: free (they chase anything across the cluster), or a leash from the carrier (chosen: the hangar's
  `leash`, the condition `off_leash` turns them back).
- **When the hangar dies**: its drones go dormant (they drift as salvage-like vessels, thousands of bodies over a
  long fight), or die (chosen: removed at once without a wreck, so a flood never leaves a field of debris and
  killing the hangar is the counter).
- **Mind capacity**: drones take a core each (a player's three drones would eat a quarter of the mind), or none
  (chosen: the hangar's capacity is the limit, and drones are no member of the fleet).
- **The stinger's family**: energy (shield bubbles took it whole: a spitter dealt 1–5 damage to a skiff in 90 s), or
  kinetic (chosen: bubbles take half).

## Decision

- **Hangar** (a part's `hangar: (drone, capacity, rebuild, leash)`): it holds `capacity` drones of the ship `drone`,
  out and in the bay together, and starts full. While it works (alive, not overheated, not idle, on the vessel's
  controlling piece) it rebuilds a lost drone in `rebuild` s at full power. While its vessel is running and aims at
  something it launches one drone every 0.3 s by its front side, if the cells in front of that side hold no part;
  the drone starts just beyond the mouth, facing out, at the carrier's velocity plus 8 m/s.
- **Drones** fly their design's behavior (`drone`: what the carrier aims at, circling close; back to the carrier
  when `off_leash`; near the carrier with nothing to fight), the carrier their leader. They take no mind capacity,
  never go dormant, are never the focus and are no part of the fleet (`Sim::vessels` and the fleet bar leave them
  out; no `VesselFormed`/`VesselLost`, so kills and losses count ships; they neither block nor allow a recall, nor
  spawn or reveal a voyage's sites). A voyage site's carrier takes its drones in.
- **Death**: a drone whose hangar is destroyed, leaves the controlling piece or whose carrier is lost, and a drone
  that loses its core, is removed at once without a wreck (`SimEvent::DroneLost`). A dark or dormant carrier keeps
  its drones.
- **Cap**: at most 48 drones alive in the world (`sim::ship::drones::tuning::MAX_DRONES`); a hangar waits while it is
  full. A full hive flood is 24. Deterministic: the pass runs once per tick after the impacts, in ship and hangar
  order, without randomness.
- **The Swarm** (faction `swarm`, hostile: not wary): the nerve node (their core and power, leaves a nerve core), the
  swarm jet (rated 28 m/s: faster than any engine), the stinger (a 55 m kinetic pulse gun), the brood hangar (6
  drones, 7 s, 450 m, 3.5 hit points; leaves resin); chitin armour. Ships: `swarm_drone` (3×3), `swarm_spitter` (4×4
  gunship), `swarm_carrier` (7×6, a brood hangar in each flank: 12 drones, closes in to 150 m and holds),
  `swarm_hive` (11×11, anchored, four pods round a hub, each pod with its own nerve node and brood hangar: 24 drones;
  the band's boss, without escorts).
- **The middle band** (no longer a stub): cluster types `swarm_reach`, `brood_deep` and, within 0.4 S of the outer
  reach, `raided_border` (the Swarm raid the Grown: GDR-0027's exception); encounters `swarm_patrol`, `hive_carrier`,
  `brood_nest`, `swarm_dead`, `swarm_raid`, `hive_site`; two `swarm_hive` trails per galaxy end at a hive (3 nerve
  cores and 6 resin into store). The gates keep the Keeper (GDR-0112).
- **The player's side**: the Swarm research family (`chitin_plating`: chitin, 10 Swarm data and 3 resin → `stingers`:
  the stinger → `drone_bay`: the drone bay, 28 Swarm data, 4 utility, 3 nerve cores). The drone bay holds 3 bay
  drones (stinger, nerve node, jet), 10 s rebuild, 300 m leash. The drydock's ARCS overlay draws each hangar's launch
  lane (a chevron per drone; coral where a part blocks the mouth), the hover card says what it launches.
- **Look**: Swarm parts in their own system colour, chartreuse (`ship.system_colors.swarm`, `#c3e45e`), chitin hulls
  in a dark brown tone (`ship.armor.chitin`); the hull glow stays the enemy's coral. A launch flashes and a lost
  drone pops in the Swarm's colour.

## Consequences

- Numbers are placeholders; the balance runs are in `docs/results/swarm-balance-2026-10-19.md` (120 seeds, the
  headless voyage runner): a Keeper-capable fleet (the Runner with two skiffs, 60 % at the Keeper's gate) clears a
  carrier voyage 51–67 %, a brood nest 55–62 % and a hive 41–57 %; an outer-reach fleet (the Runner and a cannon
  skiff, a grove run 52 %) 30, 51 and 28 %. Spitter patrols and raids clear every time (filler). A full hive flood
  costs 0.03 ms a tick, the cap of 48 drones 0.06 ms (1.2 µs a drone, two thirds of it the physics).
- The headless runner's pilot fights drones only while no other armed enemy is within 400 m (a player dives for the
  hangar; a drone's loss is no kill).
- Open: repair drones (GDR-0018's second kind), drones that dock again, a behavior action that recalls them, spitters
  that matter, a drone's look beyond its parts.
