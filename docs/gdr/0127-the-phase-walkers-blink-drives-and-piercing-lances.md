# GDR-0127: The Phase walkers: blink drives jump a ship a short way, piercing lances run through a hull, a giant phase ship is their boss

- Status: Accepted (provisional, lead), 20 Oct 2026, for Niklas to review. The design points marked (Niklas) are his
  of 20 Oct 2026; the rules' details and every number are lead defaults.
- Date: 2026-10-20
- Decided by: Niklas (the design, 20 Oct 2026) and the lead session (agent WALKERS: the rules, numbers and content;
  defaults for Niklas to review)
- Related: [GDR-0027](0027-races.md) (the inner band's races), [GDR-0126](0126-the-lattice-prism-relays-and-refractive-crystal.md)
  (the Lattice, the band's other race), [GDR-0026](0026-procedural-galaxy-and-bands.md),
  [GDR-0031](0031-materials-and-fabrication.md) and [GDR-0108](0108-materials-economy-workshop.md) (Phase residue and
  rift silt, the phase chamber), [GDR-0099](0099-the-grown-and-the-outer-reach.md) and
  [GDR-0103](0103-the-grown-in-play-defaults.md) (wary races), [GDR-0117](0117-drones-and-the-swarm.md) (the last
  race built end to end), [GDR-0112](0112-the-keeper-guards-the-middle-bands-gates.md) and
  [GDR-0116](0116-the-spire-guards-the-inner-bands-gates.md) (sections on cores of their own),
  [GDR-0013](0013-vessels-and-piloting.md) and [GDR-0044](0044-control-cost-priority-idle-dormant-newest.md) (vessels,
  splitting), [GDR-0088](0088-pilot-assists-weapon-groups-and-key-bindings.md) and [GDR-0090](0090-action-map-settings-and-pause-menu.md) (the action map),
  [GDR-0115](0115-missiles-and-point-defence.md) (the flare: the pattern of an asked ability),
  [GDR-0125](0125-disruption-ion-static-blinds-minds.md), `docs/sim.md` "Blink drives (GDR-0127)" and "Lances
  (GDR-0127)", `docs/results/walkers-balance-2026-10-20.md`

## Context

GDR-0027 gave the inner band's second race to the Phase walkers: wary, blink strikes with armour-piercing lances, giant
phase ships as bosses. GDR-0031 named their materials (Phase residue, rift silt) and their race upgrade (the phase
chamber). On 20 Oct 2026 Niklas decided the shape, after the Lattice's (GDR-0126):

- **(Niklas)** A **blink drive** part jumps the ship up to about 150 m along a direction, then cools down for several
  seconds; a visible charge-up shimmer telegraphs it. A behavior action (`blink` behind the target or away) with the
  conditions it needs. The player can research it: a key binding in the action map for the flown ship, and escorts
  use it through behaviors.
- **(Niklas)** A **piercing lance**: a short-range hit-scan that damages every cell along its line through the ship,
  losing strength with each cell (thick armour still helps a little). Player-researchable.
- **(Niklas)** **Wary hit-and-run**: calm until provoked, like the Grown; then they blink in, strike with lances,
  blink out and come back when the drive is ready again; they avoid long fights.
- **(Niklas)** The boss: **a giant phase ship whose sections each have a blink drive**, each section on cores of its
  own (like the Keeper and the Spire), so the pieces blink about on their own once shot apart.
- **(Niklas)** In the inner band **one faction per cluster**, never walkers and Lattice together, mixed cluster by
  cluster, the walkers the rarer.
- **(Niklas)** A walkers' **research family** with the blink drive and the lance; the **phase chamber** workshop
  upgrade installed by the node that unlocks the first walker part (as the hive press); their materials from salvage.

The rest was left to the lead.

## Options considered

- **What a blink moves**: the whole ship (all its pieces) or **the controlling piece** (chosen: the vessel; salvage
  pieces stay, and a split-off section with a drive of its own is a ship of its own).
- **Heavy ships**: one drive jumps anything (a giant blinks as freely as a skirmisher), range by mass, or **a load per
  drive** (chosen: `blink.mass` t each; a blink takes ready drives until they cover the piece's mass and all of them
  cool down; so the colossus blinks whole only with all four drives and a player's heavy ship needs several).
- **What blocks a landing**: nothing (ships inside each other), the path (a blink through a wreck field fails), or
  **the landing spot only** (chosen: no other body or rock within the piece's bounding radius plus a margin; the jump
  tries shorter spots, then fizzles with half the cooldown).
- **Heading after the jump**: turned onto the target, or **kept** with the speed and spin (chosen: physical; the
  walkers turn after landing, which is the player's moment).
- **The lance's fall-off**: per part, per hit point, or **per cell crossed** with a share for machines and a smaller
  one for armour (chosen: Niklas's "weaker with each cell"; "thick armour still helps").
- **Lance and shields**: let it through (an anti-shield weapon), or **energy** like the lasers (chosen: bubbles take it
  whole, unless fired from inside one, which a blink in close does).
- **Their armour**: a material of their own (a research node more), or **composite plate** (chosen: minimal content).
- **Their cluster share**: a separate band list, or **the band's weighted list** the Lattice set up (chosen: weight 2
  beside the Lattice's 5, about one inner cluster in four).
- **The boss in a walker cluster**: trails end anywhere in the band (the colossus could land in a Lattice cluster), or
  **a trail's `ends_in`** (chosen: the cluster types its end may lie in; with none free in the band the trail is left
  out of that galaxy).

## Decision

- **Blink drive** (content `blink: (range, charge, cooldown, mass)`, `flight/blink.rs`): asked by the BLINK key
  (`ShipCommand::Blink(dir)`: along the touch stick, else the thrust and strafe keys held, else the nose) or a
  `blink(behind|away)` rule. The drives on the active controlling piece that are alive, not overheated, not idle and
  cool are taken in part order until their `mass` covers the piece's mass (else no blink). They charge together for
  the longest `charge` at full power (the lowest work of them counts), drawing their power only then
  (`ConsumerKind::Blink`, `power.intermittent`); a drive lost or a vessel stopped breaks the charge off. Then the
  piece's frame moves by the jump, keeping velocity, heading and spin: `Dir` the shortest `range` of the drives along
  the direction; `Behind` towards a spot 12 m past the target's tail beyond its radius, at most the range; `Away` the
  full range straight away. The spot must be clear of other bodies (ships, salvage, rocks) within the piece's radius +
  2 m and of spots other blinks took this tick; else 3/4, 1/2, 1/4 of the jump; none clear: it fizzles
  (`SimEvent::BlinkBlocked`) and the drives cool half their cooldown. A jump cools every drive it took
  (`SimEvent::Blinked`). The step runs after the weapons and salvage, before the physics step. A split-off vessel
  keeps its drives' cooldowns.
- **Behavior words**: the action `blink(behind|away)` (needs a target; like `flare` it never decides the maneuver:
  it starts a blink and the next rule flies the ship while the drives charge), the condition `blink_ready`.
- **Lance** (content `weapon.lance: (keep, armor)`, pulse guns only, `combat/lance.rs`): the shot strikes the part it
  hits, then every part on its line through that piece, each with what is left; after each cell of a machine part the
  strength keeps `keep`, after each hull cell `armor`; it stops where the line leaves the piece (empty cell or another
  piece) or below 2 % of the shot. Applied with the other gun hits (fire stays mutual). `SimEvent::Lanced`.
- **Content** (placeholders): the **phase core** (their mind and power: 8 MW, 4 rule slots, sensors 750 m; leaves rift
  silt; theirs only); the **blink drive** (1 × 1, 4 BP, 1.5 t, 1 HP, 6 MW while charging, High tier; 150 m, 0.6 s
  charge, 4 s cooldown, 22 t; leaves phase residue); the **phase lance** (1 × 1, 4 BP, 2 t, 1 HP, 5 MW; Energy,
  1.6 damage/s in 0.5 pulses/s, 3.2 a shot, 40 m; keeps 80 % after a machine cell, 50 % after an armour cell).
  Ships: the **walker skirmisher** (3 × 4: a lance, a core, a drive, a thruster), the **walker striker** (4 × 5: two
  lances, two cores, two drives that jump it together), the **phase colossus** (11 × 7: a head with three lances, two
  wings with a lance and a thruster, a hub with two cores and two thrusters, each section with its core and drive,
  joined by single bridge blocks; whole about 69 t, so a blink needs all four 22 t drives). Their faction is wary;
  behaviors `walker_skirmisher` (out when the drive is ready again and the target is within 70 m, in behind it within
  180 m, closes to 25 m within 100 m, else keeps 160 m), `walker_striker` (out only below 70 % hull), `walker_colossus`
  (in behind whenever ready and 60–200 m off, else closes in; its sections keep the rules).
- **The band**: the cluster type `phase_rift` (walker packs, a strike, their dead), weight 2 in the inner band's list
  beside the Lattice's 5; encounters `walker_pack` (2–3 skirmishers), `walker_strike` (a striker and 1–2 skirmishers),
  `walker_dead`, `colossus_site` (the colossus and two skirmishers) at the end of a `phase_colossus` trail (one per
  galaxy, `ends_in: ["phase_rift"]`; 3 phase residue and 4 rift silt into store).
- **The player's side**: the research family `walkers` in the walkers' colour (`phase`, #ee82d6): `phase_lances` (14
  walker + 4 energy data, 2 rift silt: the lance and the **phase chamber**) → `blink_drive` (26 walker + 4 propulsion
  data, 3 phase residue). Recipes in the phase chamber. The BLINK key is J (`controls.ron`). The drydock warns TOO
  HEAVY TO BLINK when the drives cannot carry the main piece (`report::Issue::BlinkTooHeavy`).
- **Looks**: the charge shimmers round the hull in the walkers' colour (a broken ring tightening and brightening as it
  fills); a blink flashes out with a spray and in with a ring, a faint streak between; a lance fires in the phase
  colour and leaves its line through the hull, bright at the exit. Icons `phase-core`, `blink-drive`, `phase-lance`
  (each with its `.ship.svg`), `phase-residue`, `rift-silt`. Screenshots (1280 × 720):
  `docs/results/walkers-drydock-2026-10-20.png`, `walker-blink-charge-2026-10-20.png`,
  `walker-blink-out-2026-10-20.png`, `walker-lance-2026-10-20.png`.

## Consequences

- Balance (`docs/results/walkers-balance-2026-10-20.md`, 60 seeds, the voyage runner): a Keeper-capable fleet clears
  two walker packs 60 %, a pack and a strike 50–53 %, the colossus 30–35 % (a Keeper's gate 48–60 %, two Lattice watches
  45–50 %, the resonator 25–33 %). Without blinks the packs fall 75–88 %; with lances that stop at the first part
  87–97 %. The first cut (7 s cooldown) let the walkers die before their drives cooled; the cooldown is 4 s.
- Determinism: the arena fight `walker_strike` is in `scripts/determinism-check.sh --fight` (native = wasm, seeds 42
  and 7). A ship's blink state is hashed only when it has drives (their cooldowns) and while a charge runs, so worlds
  without them keep their hashes; the balance check's seven fights are unchanged fight for fight.
- The trail field `ends_in` is open to every trail; the Lattice's resonator trail does not use it yet, so a resonator
  can still end in a phase rift (and take the only one a colossus could have used).
- Open: whether the blink should turn the ship onto its target; a touch button for BLINK (the key only so far); a
  pilot assist that blinks (none yet); a phase armour of their own; whether trails that cannot be placed should fall
  back to any cluster; every number above.
