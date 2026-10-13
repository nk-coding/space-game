# Plan: the galaxy, voyages and the home

Status: **proposal** (13 Oct 2026, a planning agent for the lead session). Nothing here is decided. Every number
marked *(placeholder)* is for feel, not a spec. Records this plan would need are proposed in
[5.8](#58-proposed-records); none of them is written yet. The roadmap and the record indexes are the lead's to
update.

**Owner direction since this was written (14 Oct 2026, [GDR-0075](../gdr/0075-voyages-feel-like-exploration.md)):**
voyages must feel like *exploring a galaxy and following hints*, not like following a fixed path. Read the
proposal below through that; where it lays out a fixed route, it needs rework before it becomes records.

Read with: [GDR-0002](../gdr/0002-awakened-ship-mind-light-story.md), [GDR-0026](../gdr/0026-procedural-galaxy-and-bands.md),
[GDR-0027](../gdr/0027-races.md), [GDR-0028](../gdr/0028-clusters-and-voyages.md),
[ADR-0022](../adr/0022-one-simulation-space-per-cluster.md), [ADR-0024](../adr/0024-saves-seed-and-deltas.md),
the mockups `docs/concept/boards/ui-mockups/phone-galaxy-map.png` and `phone-home-ring-menu.png`, and the
concept board's "World" page (`docs/concept/boards/concept-board/08-world.png`, generator sketch in
`concept-board.html`, `generate()` and `drawCluster()`).

## 1. Summary

The game gets a place to live and a reason to fly. The player starts at **home**, a ring menu around the docked
ship in the abandoned precursor shipyard. From there the **galaxy map** shows about 80 generated clusters in five
bands, linked by lanes, with a guardian at every band gate. The player picks a destination and launches a
**voyage**: a short warp, then 15–30 minutes inside one cluster, a real simulation space with patrols, wreck
fields and scan targets. The voyage ends with a recall home and a summary. Beating a guardian opens the next band.
A few short story texts and lore fragments tell the awakened ship-mind's story along the way.

## 2. Already decided vs only sketched

### Decided (records)

| What | Record |
|---|---|
| You are a precursor ship-mind that wakes in an abandoned precursor shipyard at the rim; the shipyard is the fixed home (drydock, research lab, fabricator, test range). First hour: tiny drone → salvage automaton wrecks → repair the drydock → first real ship → first rim voyage → first research. Story stays light: procedural missions and lore on derelicts; lore also unlocks reverse-engineering | [GDR-0002](../gdr/0002-awakened-ship-mind-light-story.md) |
| One seamless procedural galaxy of clusters linked by lanes, handcrafted set pieces placed in, a new seed per game; about 80 clusters for now and expandable (cluster count, band sizes, races, set pieces are generator parameters and data) | [GDR-0026](../gdr/0026-procedural-galaxy-and-bands.md) |
| Five bands inward: rim → outer → middle → inner → core (the precursor archive); about 50 h to the core; difficulty by distance, race territory and hazards (stellar heat outer, ion storms and nebulae middle, radiation rising inner → core) | [GDR-0026](../gdr/0026-procedural-galaxy-and-bands.md) |
| A precursor guardian at every band gate; beat it to add the gate to your network; guardians are huge breakable machines built from parts, solo-able, their behavior changes as sections break | [GDR-0026](../gdr/0026-procedural-galaxy-and-bands.md) |
| The world state is static; cleared encounters come back slowly. Content types: derelicts and graveyards, race stations, outposts, bosses, megastructures and anomalies | [GDR-0026](../gdr/0026-procedural-galaxy-and-bands.md) |
| Races: rogue automatons (rim), the Grown (outer, wary), the Swarm (middle, hostile, raids neighbours), the Lattice (inner, neutral, open to standing) and Phase walkers (inner, wary); mainly for variety and tech; allies only inside race missions | [GDR-0027](../gdr/0027-races.md) |
| Clusters are large, 10+ minutes to cross; a default voyage covers one cluster in 15–30 minutes; longer voyage types exist but are never forced. Activities: fights, salvage and scanning, light missions. Travel: cruise inside, warp along lanes, gates for long distances. Supplies: ammo, fuel, repair material, refilled at home, outposts refill and repair | [GDR-0028](../gdr/0028-clusters-and-voyages.md) |
| Death is mild: bonded matter re-forms the ship at home; part of the unbonded haul stays behind as a wreck | [GDR-0029](../gdr/0029-mild-death-bonded-matter.md) |
| Salvage and scans only become research at home; scans also give a persistent cluster map, lore and enemy intel | [GDR-0025](../gdr/0025-research-from-salvage.md) |
| Home is a ring menu around the docked ship (drydock, research, fabricator, test range, launch voyage, storage, galaxy map, behavior). Travel: autopilot for transit over lanes, manual in fights, automatic hand-over when a hostile locks on | [GDR-0008](../gdr/0008-ui-ship-mind-interface.md) |
| Sensors are moderate; behavior conditions only read what the sensors detect | [GDR-0017](../gdr/0017-combat-support-repair-capture-sensors.md) |
| Escorts are vessels within the mind capacity (GDR-0003's "command capacity" is superseded) | [GDR-0013](../gdr/0013-vessels-and-piloting.md) |
| Ships of ~60×60 and up, no fixed block cap; the drydock measures a design's cost per device | [GDR-0006](../gdr/0006-ship-scale-no-fixed-cap.md) |
| Enemy designs: handcrafted per race, plus evolved offline and curated, plus runtime variation (not full generation); build points scale enemies | [GDR-0032](../gdr/0032-balancing-arena-evolution.md), [GDR-0030](../gdr/0030-no-economy.md) |
| Co-op later, in the host's galaxy; players may split up, so the host simulates several clusters; every boss solo-able | [GDR-0033](../gdr/0033-multiplayer-coop-and-arena.md) |
| One simulation space per cluster with small local coordinates; a short warp sequence while the next one loads; only occupied clusters simulate, empty ones freeze and settle their respawn timers on arrival; a cheap galaxy-wide event layer for Swarm raids and roaming bosses | [ADR-0022](../adr/0022-one-simulation-space-per-cluster.md) |
| Saves store seed, generator version and deltas, versioned with migrations; export and import by file or share code | [ADR-0024](../adr/0024-saves-seed-and-deltas.md) |
| Bit-exact determinism in `sim` (fixed 30 Hz, PCG32 streams with append-only ids, ordered containers, libm trig) | [ADR-0010](../adr/0010-cross-platform-determinism.md) |
| Player storage: named, versioned RON documents per namespace; `saves/` is planned | [ADR-0031](../adr/0031-player-storage-and-design-sharing.md) |

### Only sketched (concept board and mockups, no record)

- **Generator** (`concept-board.html`, `generate()`): a disc; bands by radius; clusters by rejection sampling
  with a minimum distance; lanes as a minimum spanning tree plus short extra links (35 % chance to one of the 3
  nearest within a reach); home = the rim cluster nearest a random angle; 3 gates per band boundary, at the home
  angle and ±2.1 rad, on the inner edge of the outer band; set pieces counted per band (derelicts 4/3/2/1,
  stations outer 2 and inner 3, hives middle 2, giant ships outer 1 and inner 1, megastructures 1/1/1/2);
  stellar heat in 35 % of plain outer clusters, ion storms in 40 % of middle clusters; outposts next to the first
  gates; names from syllables plus a suffix (Drift, Shoal, Verge, ...). In the sketch the core holds only the
  archive.
- **Pacing per band** (board table): rim 0–5 h, outer 5–15 h, middle 15–30 h, inner 30–45 h, core ~50 h. "A
  default voyage: one cluster past the frontier gate."
- **Cluster close-up** (`drawCluster()`): the star at the centre, an asteroid belt, lane exits around the edge,
  set pieces placed on rings, a wreck field, two patrols on ellipses, two scan targets, "≈ 10–15 min across".
- **Galaxy map mockup**: header "82 CLUSTERS · 17 CHARTED · 6 CLEARED"; legend home / charted / contract /
  cleared / unknown / gate; band labels; a dotted route; filters LANES / MATERIALS / THREAT; a voyage panel
  with destination, chips (band, race, hazard), route legs (Shipyard DOCKED, Corran Reach WARP 0:40, Halvex
  Drift WARP 0:55), "VOYAGE ≈ 20 MIN · RETURN ANY TIME", fuel / ammo / repair bars, toggles AUTOPILOT IN
  TRANSIT and HAND OVER ON LOCK, SET COURSE.
- **Home mockup**: "THE SHIPYARD · RIM", the ship's name, the docked ship in the centre; ring buttons with
  badges (RESEARCH 3 READY, FABRICATOR 2 IN QUEUE, LAUNCH VOYAGE + destination); RESEARCHING with a progress
  bar; HAUL UNLOADED "14 parts · 9 materials".

### Where sketches and records disagree, or a record looks shaky

1. The galaxy-map mockup labels Halvex Drift "OUTER BAND · AUTOMATONS · ION STORMS". GDR-0026/0027 put the
   Grown and stellar heat in the outer reach and ion storms in the middle (the board's own close-up says "Halvex
   Drift · Middle reach · The Swarm"). Treat the mockup's labels as illustrative.
2. The concept generator lets lanes cross band boundaries anywhere, while the board says gates "only open once
   their guardians fall" and lists each band's "way in" as a guardian. If lanes cross freely, guardians gate
   nothing. Needs a decision (gap 2, question 2).
3. GDR-0028's "10+ minutes to cross" names no speed. At today's top speed (~22 m/s, GDR-0042) that is ~13 km.
   ADR-0022's "small, precise local coordinates" holds up to roughly 8 km from the cluster origin in `f32`
   (gap 1). A faster cruise mode would break that. Not wrong, but the two records should name a size together.
4. ADR-0024 stores the seed and generator version and regenerates the world. The generator's parameters are
   content (GDR-0026: "generator parameters and data"), so a balance update or a mod would reshuffle a running
   galaxy unless every old parameter set stays frozen. This plan proposes to also store the small galaxy
   skeleton (gap 14, question 6), which would refine ADR-0024 with a new record, not change it silently.
5. GDR-0017 says behaviors only read what sensors detect, but target selection has no range today
   (`crates/sim/src/behavior/eval.rs`, `select` loops over every ship with `is_enemy`). Invisible in the sandbox,
   fatal in a 13 km cluster (every awake patrol would fly at the player). An implementation gap, not a record
   problem (gap 8).
6. The sandbox's raider waves (`crates/game/src/scene.rs`, `scene_tick`) roll positions with the client RNG
   (`backdrop::Rng`) and std `cos` / `sin` / `atan2`, and call `Sim::spawn_ship` from the client. Fine for a
   sandbox, but voyages must spawn from a deterministic layer (ADR-0010), or co-op, replays and saves break.
7. Faction glow keys are only `player` and `enemy` (`data::art_keys`, `Faction::glow` in
   `crates/data/src/model.rs`). Wary and neutral races (GDR-0027) have no look yet (a style question, gap 20).

## 3. Gaps

What must be decided before building. Each: options, trade-offs, recommendation. Gaps that need Niklas link to
a question in [section 8](#8-questions-for-niklas); the rest are lead defaults to record.

**1. Cluster size, coordinates and cruise** (question 1).
- (a) A cluster is a disc of ~6.5 km radius *(placeholder)*, ~13 km across, origin at its star; cruise is normal
  flight at the ship's own top speed under autopilot (~10 min across at 22 m/s); plus an optional transit
  time-skip (the sim runs 2–4 ticks per frame while no hostile is within sensor range). Within 8 km of the
  origin `f32` resolves 0.5 mm or better (one ulp is 0.49 mm at 4–8 km, 0.98 mm at 8–16 km; Rapier's allowed
  contact error is about 1 mm at 1 m length units). No new flight mode, determinism untouched.
- (b) Same, without the time-skip. Simplest; 10 minutes of flying may drag on a phone.
- (c) A separate cruise drive (5–10× speed out of combat) and clusters of 60–130 km. Feels vast, but coordinates
  past 16 km lose precision, so it needs origin shifting (rebasing every body deterministically) and a new flight
  mode that ignores the rated speeds of GDR-0041.
- Recommendation: (a). Validation caps a cluster's radius at 8 km *(placeholder)*.

**2. Gating at band boundaries** (question 2).
- (a) Hard: lanes cross a band boundary only from gate clusters; each gate's lane opens when its guardian falls;
  any one gate of a boundary opens the next band; the others stay optional bosses and shortcuts.
- (b) Soft: lanes cross anywhere; difficulty and hazards gate; a guardian only adds its gate to the network.
- (c) Hard, and every guardian of a boundary must fall before the band opens.
- Recommendation: (a): clear goals, still choice. (b) makes guardians optional and lets players rush the core.

**3. What a voyage is** (lead default). The player picks a destination cluster on the map; the route uses the
gate network (home ↔ opened gates, instant) and then warps along lanes; warps chain through charted clusters
without stopping; the voyage begins at the lane entry of the destination. Reachable: any cluster in an open band
that is charted or next to a charted one (exploring one step into the unknown). Alternative: stop in every
cluster on the route and fly across it (rejected: 10+ min per hop). Warp time per hop: 20 s *(placeholder)*;
the mockup's 0:40 and 0:55 are long for a phone session.

**4. Returning home** (question 3).
- (a) Recall any time: the recall charges for 15 s *(placeholder)*, cannot start while a hostile aims at one of
  your vessels, and a new lock cancels it. All your vessels and the haul come home.
- (b) Fly back to a lane exit or a gate and warp home. More tension, longer sessions.
- (c) Instant return any time. Simplest; fights and death lose their bite.
- Recommendation: (a), matching the mockup's "RETURN ANY TIME" while keeping GDR-0029's risk.

**5. What fuel is** (question 4). GDR-0028 lists ammo, fuel and repair material. Engines burn nothing today
(rated speed, GDR-0041), no gun uses ammo, and repair drones do not exist yet.
- (a) Fuel pays for warp hops and the recall only; ammo comes with missiles and kinetic ammo racks, repair
  material with repair drones (GDR-0017).
- (b) All three now; engines burn fuel per thrust. Closest to the list, but adds a fuel model to every tick of
  flight and to the drydock stats.
- (c) No supplies until salvage and research exist.
- Recommendation: (a). The first slice can ship with (c) and add (a) as a small track.

**6. Where the deterministic world logic lives** (ADR, lead).
- (a) A new engine-free crate `galaxy` (depends on `data` and `sim`): galaxy and cluster generation, the
  cluster director that spawns, freezes and clears encounters around a `sim::Sim`, voyage state, save types (no
  I/O).
- (b) A `sim::space` module inside `sim`. One determinism toolset, but `sim` grows into world logic.
- (c) In the client, like today's waves. Not deterministic; rejected.
- Recommendation: (a). `galaxy` copies `crates/sim/clippy.toml`'s bans and uses `sim::math` and `sim::rng`.

**7. What is simulated when inside a cluster** (ADR, lead).
- (a) Everything spawned on arrival; Rapier sleeping does the rest. Cheapest to build, but patrols never sleep
  and every ship runs power, heat and behaviors every tick.
- (b) Three states per site: **record** (not spawned; just an anchor and a seed) → **awake** (spawned in the
  `Sim`) when a player vessel comes within the spawn radius → **frozen** (kept in the `Sim`, skipped by every
  phase, its Rapier body disabled) beyond a larger freeze radius, thawed again on approach. Hysteresis avoids
  flapping; spawns are spread over ticks.
- (c) Like (b) but despawn back to a record instead of freezing (no sim change, but damage and wrecks are lost
  or must be serialised into the record).
- Recommendation: (b). It needs `Sim::set_frozen` ([5.3](#53-sim-changes)). Measure first (section 7).

**8. Sensor range for behaviors** (GDR-0017 implementation, lead). Target selectors and `under_attack` today see
every ship in the world. Options: a fixed sensor range per faction *(placeholder 800 m)*; or a `sensor` block on
logic cores and sensor parts in content (range per part, the best on the vessel counts). Recommendation: the
content block, with the logic core's default range as the fallback, so sensor parts can come later without
another change. Frozen ships are never seen.

**9. Factions in the sim: relations and owners** (lead). The sim has `Owner(u8)` with `MAX_OWNERS = 8`
(`crates/sim/src/vessel.rs`) and treats every other owner as an enemy. Wary and neutral races need a relation
table: `Hostile` / `Neutral` / `Allied` per owner pair, part of the hashed state, changed by commands and by a
provocation rule (a wary owner hit by you turns hostile to you for a while, *placeholder* 120 s).
No-friendly-fire, rams and plumes stay per owner as now (GDR-0045/0046/0048). Owners per cluster: the player(s)
first, then one owner per faction present in content load order. Recommendation: build it in the second slice;
the rim needs only hostile automatons.

**10. Band difficulty** (lead default, placeholder numbers). Express difficulty as a **threat budget** in build
points (GDR-0030) per encounter: a range per band, scaled by depth inside the band (0 at its outer edge, 1 at its
inner edge) plus a small random spread; encounters are filled from pools of designs until the budget is met.
Alternative: hand-set difficulty per cluster type (more control, does not scale with ~80 clusters). Numbers in
[4.7](#47-placeholder-numbers).

**11. Guardians** (lead default). A guardian is one huge blueprint with several sections, each with its own
logic core(s). The existing vessel rules do the rest: a section cut off with a working core becomes its own
vessel and inherits the behavior (GDR-0013). "Behavior changes as sections break" uses new conditions
(`cores_left`, `parts_left`) in ordinary rules. The guardian is beaten when none of its vessels runs. Beaten
guardians stay beaten (the gate stays open). Alternative: scripted phases in code (more control, against
"content in data").

**12. The refill clock** (lead default). "Cleared encounters come back slowly" needs a clock that is not the
wall clock (ADR-0010). Options: count voyages (exploitable by aborting voyages), or **played voyage time** (the
sum of sim ticks spent on voyages, saved as `clock`). Recommendation: played voyage time, 90 min *(placeholder)*
per encounter; guardians and set pieces never refill.

**13. Wrecks and debris** (lead default). Wrecks are salvage, so the sandbox's cleanup (dark ships 320 m away or
dark for 75 s, `scene_tick`) is wrong in a cluster. Options: keep all wrecks for the voyage and drop them when it
ends, except the player's death wreck (saved as a delta); or persist every wreck (save bloat). Recommendation:
the first, plus a cap of 150 salvage pieces per cluster *(placeholder)*: the smallest, oldest pieces far from
players go first.

**14. The galaxy layout in the save** (question 6, refines ADR-0024).
- (a) Store the **skeleton** (clusters with id, name, band, map position, type, seed, tags; lanes; ~80 entries,
  a few KB) next to the seed; cluster interiors regenerate from per-cluster seeds. Updates and mods never
  reshuffle a running galaxy; new bands or clusters are added by a migration.
- (b) Regenerate everything from seed + generator version, as ADR-0024 reads. Smallest saves, but every
  generator parameter in content must stay frozen per version, or old saves change shape.
- Recommendation: (a).

**15. Saving mid-voyage** (question 5). Phones kill apps; a 20-minute voyage must survive that.
- (a) A light checkpoint every 60 s *(placeholder)* and on pause: player vessels (blueprint, per-part hp, pose),
  haul, site states so far; resume regenerates the cluster and places them back. Not bit-exact with the
  uninterrupted run (fine for single player; a replay ends at a resume).
- (b) Save only at home; quitting mid-voyage counts as a recall. Simplest, generous.
- (c) A full world snapshot (every ship, projectile, Rapier state). Bit-exact resume and a base for co-op join,
  but serialising `Sim` and Rapier deterministically is large work.
- Recommendation: (a), with (b) as the first slice's behaviour.

**16. Home: menu or place** (question 7).
- (a) Home is the ring menu (the docked ship, no flying). The first hour plays in a small home cluster around the
  shipyard, revisitable as a quiet cluster later. Voyages start at the destination.
- (b) Home is a flyable cluster; you undock into it before every voyage. Immersive, adds travel to every voyage.
- Recommendation: (a).

**17. How the story is told** (question 8).
- (a) Short text cards at milestones (first wake, first voyage, each gate opened, reaching a new band, the
  archive) plus a memory log of lore fragments found on derelicts and megastructures.
- (b) The same plus short inner-voice lines in the flight HUD during voyages.
- (c) Lore on derelicts only.
- Recommendation: (a): little writing, all in content, matches GDR-0002's "light".

**18. Who flies a voyage** (lead default). The flagship is the player's working design (`drydock::PlayerDesign`);
escorts are other saved designs the player adds in the launch panel, within the mind capacity. Ships are free
until fabrication exists (as in drydock v1, GDR-0051); with fabrication, only built ships launch.

**19. Hazards** (later, lead default). Hazards are zones in a cluster with effects in content: stellar heat adds
heat to parts with sides open to space near the star; ion storms drain shield charge and shorten sensor range;
radiation damages exposed parts slowly. Not needed for the rim (the board: "None; the tutorial band").

**20. The look of non-hostile races** (later, style). A third hull glow key (`neutral`, e.g. the shield blue
`#8fb0f2`) for wary and neutral factions, switching to coral when hostile. A visual question for Niklas when
the second race comes.

## 4. Design proposal

### 4.1 The player-facing flow

1. **Home** (the ring menu, `phone-home-ring-menu.png`). The docked flagship in the centre in style 2a; ring
   buttons DRYDOCK, BEHAVIOR, GALAXY MAP, LAUNCH VOYAGE (shows the chosen destination), TEST RANGE (today's
   sandbox test flight), and RESEARCH, FABRICATOR, STORAGE shown as locked until their phase. Top left "THE
   SHIPYARD · RIM" and the flagship's name; bottom left HAUL UNLOADED from the last voyage; top right the
   research slot (empty until research exists). A story card shows here when a milestone fires.
2. **Galaxy map** (`phone-galaxy-map.png`). Band rings (dotted, the next gate ring coral), clusters as charted /
   cleared / unknown / gate / home, lanes (closed gate lanes dashed coral), the gate network as faint amber
   arcs. Tap a cluster: the panel shows its name, chips (band, race, hazard), the route legs with times, the
   estimated voyage length, supplies (once they exist), AUTOPILOT IN TRANSIT and HAND OVER ON LOCK, SET COURSE.
   SET COURSE sets the destination and returns home (LAUNCH VOYAGE starts it); a long press or a second button
   launches at once.
3. **Launch**: the warp sequence (streaks, the route legs ticking off) while the cluster is generated and loaded.
   Placeholder: gate hop 3 s, each lane hop 20 s.
4. **Arrival** at the lane entry on the cluster's edge. The cluster map overlay (M, or a HUD button) shows the
   star, the lane exits, sites already charted or scanned, and contacts inside sensor range. Tap a site to fly
   there under autopilot; escorts follow the flagship.
5. **In the cluster**: patrols move between waypoints, guards hold their site, ambushes wake when the player comes
   close. Fights are flown by hand: autopilot hands over as soon as a hostile aims at a player vessel (the toggle
   can turn that off). Salvage and scans come with their own phase; until then sites show what they will hold.
   Hazards come with their bands.
6. **Leaving**: RECALL (charges 15 s *(placeholder)*, blocked while a hostile aims at you) brings every vessel
   and the haul home; or fly into a lane exit to warp on to a neighbouring cluster (a longer voyage, never
   needed). Death: the vessels re-form at home (GDR-0029); half the haul *(placeholder)* stays as a wreck at
   the spot, saved as a delta and recoverable on a later voyage.
7. **Return**: a summary card (time, fights won, sites cleared, haul, newly charted clusters, lore found), then
   home with HAUL UNLOADED.

Guardian voyages are the same flow with the gate cluster as destination: the guardian sits at the gate site;
beating it opens the gate lane and adds the gate to the network (instant travel home ↔ gate).

### 4.2 Generation rules

All generation is a pure function of (galaxy seed, generator version, content), with PCG32 streams and libm math
from `sim` (ADR-0010), ordered containers and ascending ids. A generation hash (skeleton plus three sample
cluster interiors) is checked native vs wasm.

**Galaxy skeleton** (once per new game; stored in the save if question 6 goes that way):

1. Clusters per band from content (`bands.ron`, counts *(placeholder)*). The core's centre cluster is the
   archive. Other clusters are placed in the band's ring of a disc by rejection sampling with a minimum distance
   (as in the sketch), in map units; positions are rounded to integers after placement so the stored skeleton
   is exact.
2. Lanes inside each band: a minimum spanning tree (Prim, ties by index) plus extra links to one of the three
   nearest clusters within 1.3× the nearest distance, with a 35 % chance *(placeholders from the sketch)*.
3. Gates: on each boundary, `gate.count` clusters of the outer band nearest its inner edge, spread around the disc
   (the home angle and ±120° *(placeholder)*). Each gets one **gate lane** to its nearest cluster in the next
   band inward. With hard gating (question 2) these are the only cross-band lanes, closed until the guardian
   falls.
4. Home: the rim cluster nearest a random angle; it is also the first gate of the network. Check that at least 4
   rim clusters *(placeholder)* lie within two hops of home; otherwise re-roll the home angle (bounded tries,
   deterministic).
5. Set pieces and hazards: per band, from content counts and chances (the sketch's numbers as placeholders);
   outposts next to gate clusters.
6. Names: syllables and suffixes from content, unique, rolled on their own stream so adding a name list does
   not move clusters.
7. Checks: the whole graph is connected when all gates are open; each band's clusters are connected inside the
   band.

**Cluster interior** (on demand, at warp time): seed = `splitmix64(galaxy seed ^ cluster id)`, the cluster type
from the skeleton.

1. The star at the origin; lane exits on the edge at the angles of the lanes on the map.
2. Sites from the cluster type: for each entry, a count in its range, placed on its ring with a minimum spacing
   (rejection sampling, ordered). A site's id is its index in generation order (stable while the type is
   unchanged).
3. Encounters at sites: designs from the encounter's list, counts in range, then fitted to the band's threat
   budget (4.4). Runtime variation per GDR-0032 later (part swaps within a role).
4. Scenery: rock belts and wreck fields as hull-only ships without a core (salvage), from content blueprints.
5. Deltas are applied on top: cleared sites whose refill time has not passed stay empty, the death wreck is
   placed back, scanned sites show as known.

Generator versions: the version is a number in content (`galaxy.ron`); any change that moves generated output
bumps it. With a stored skeleton only interiors depend on it; a changed interior drops the deltas of sites that
no longer match (a migration logs them).

### 4.3 Inside a cluster

- **Sites** are anchors (a point, a radius) with optional scenery, an optional encounter, a scan target or a lane
  exit.
- **Encounter movement**: `Guard(radius)` (stay near the anchor, return after a chase), `Patrol(radius, speed)`
  (loop around the anchor), `Ambush` (dark until a player vessel is within its trigger radius), `Roam` (moves
  between sites; later, for roaming bosses).
- **Streaming** (gap 7): every 15 ticks *(placeholder)* the director compares player vessel positions with
  site anchors: spawn within 1.5 km, freeze beyond 2.5 km if no member is engaged, thaw within 1.5 km
  *(placeholders)*. At most one encounter spawns per tick.
- **Cleared**: an encounter is cleared when none of its vessels runs (all salvage or gone). The site's delta
  records the clock; the encounter refills after 90 min of played voyage time *(placeholder)*.
- **Charted**: a cluster is charted once visited; sites are known once seen inside sensor range or scanned. The
  map's "cleared" means every guard encounter of the cluster is cleared (patrols refill anyway).

### 4.4 Band difficulty

Threat budget per encounter in build points, as a multiple of the Halvex Runner's build points (the starter's
reference), scaled by depth in the band *(all placeholders)*:

| Band | Encounter budget | Guardian | Hazard | Board pacing |
|---|---|---|---|---|
| Rim | 0.3× → 1.0× | ~3× | none | 0–5 h |
| Outer | 0.8× → 2× | ~6× | stellar heat | 5–15 h |
| Middle | 1.5× → 4× | ~12× | ion storms, nebulae | 15–30 h |
| Inner | 3× → 8× | ~20× | radiation, rising | 30–45 h |
| Core | 6× → 12× | ~30× (final) | radiation, peak | ~50 h |

Spread ±15 % per encounter *(placeholder)*. Validation warns when an encounter's maximum (all ships at their max
count) misses its band's range.

### 4.5 Guardians and gates

- One guardian per gate cluster, at the gate site, `Guard` movement, never refills.
- Size grows by band: rim ~48×48 cells, outer ~64×64, middle ~80×80, inner ~96×96, final ~128×128
  *(placeholders; cost measured first, section 7)*. Sections of 300–800 parts *(placeholder)*, each with its own
  logic cores, joined by thin bridges that break first.
- Beaten = no running vessel of the guardian's encounter. Then the gate lane opens (delta `gates`), the gate
  joins the network, and its parts are salvage (precursor tech, GDR-0026).
- The first guardian's design is handcrafted; later ones may be evolved offline (GDR-0032).

### 4.6 The light story

- **Milestone cards** (content `story/`): wake (new game), the drydock repaired, the first real ship, the first
  voyage home, the first research, each gate opened, each band reached, the archive. Each card: a title and two
  to four sentences, shown on the home screen or after the voyage summary.
- **Memory log** (content `lore/`): fragments found on derelict and megastructure sites (scan targets, once
  scanning exists). Fragments tell what happened to the precursors and why the ship-mind was left behind. Some
  unlock alien tech nodes later (GDR-0002).
- **The first hour** (GDR-0002) plays in the home cluster: wake as a tiny drone, salvage automaton wrecks
  nearby, repair the drydock, build the first ship, then the first rim voyage. It needs salvage and the drydock
  unlock, so it lands after the salvage phase; this plan builds the hooks (beats, unlock flags, the home
  cluster type).
- Missions ("contracts" on the map) come with race stations, later.

### 4.7 Placeholder numbers

Every number here is a placeholder for feel.

| What | Value |
|---|---|
| Clusters per band (rim, outer, middle, inner, core) | 12, 18, 22, 24, 4 (= 80) |
| Gates per boundary | 3 |
| Extra lanes | 35 % chance, within 1.3× the nearest distance |
| Cluster radius | 6.5 km (validation limit 8 km) |
| Sites per cluster | 8–14 (3–6 encounters, 1–3 wreck fields, 0–1 rock belt, 2–3 scan targets) |
| Site spacing | ≥ 900 m |
| Sensor range (logic core default) | 800 m |
| Spawn / freeze radius | 1.5 km / 2.5 km, checked every 15 ticks |
| Awake ships budget | 40 (frozen ≤ 400, sleeping scenery bodies ≤ 300) |
| Warp time | gate hop 3 s, lane hop 20 s |
| Recall charge | 15 s |
| Haul lost on death | 50 % |
| Encounter refill | 90 min of played voyage time |
| Salvage pieces kept per cluster | 150 |
| Checkpoint interval | 60 s |
| Patrol speed | 8 m/s |
| Provocation (wary turns hostile) | 120 s |

## 5. Technical plan

### 5.1 Crates and how it fits

| Crate | Gets |
|---|---|
| `data` | New content kinds (5.2), validation, JSON export, link kinds, text keys |
| `sim` | Sensor range, frozen ships, posts and new behavior nodes, relations, small queries (5.3) |
| `galaxy` (new) | Generation, the cluster director, voyage state and report, save types and migrations (5.4) |
| `game` | `AppMode::Home` and `AppMode::Galaxy`, the voyage scene, map overlay, warp screen, save wiring (5.6) |

`galaxy` depends on `data` and `sim`, has no Bevy, no threads, no I/O and builds for wasm32 (CLAUDE.md hard
rules). The future `net` host holds one `ClusterSpace` per occupied cluster (ADR-0022, GDR-0033).

How it fits the existing code:

- One `Sim` per cluster, built the way `crates/sim/src/scenario.rs` builds scenarios (`Sim::with_config`,
  `ShipDesign::compile` per blueprint, shared by `Arc`, `Sim::spawn_ship(design, owner, body)`,
  `ShipCommand::Behavior`), but by the director instead of a scenario.
- The client keeps `SimWorld { sim, .. }` (`crates/game/src/sim_bridge.rs`); a `Voyage` resource holds the
  director. The `FixedUpdate` chain becomes `scene::scene_tick → voyage::pre_step → sim_bridge::step_sim →
  voyage::post_step → sim_bridge::apply_events`. `Sim::step` keeps events from calls made since the last step,
  so ships spawned in `pre_step` show up in `apply_events` as today (`ShipSpawned`, `VesselFormed`).
- `scene.rs` gets `SceneKind::Voyage`; HUD, fleet bar, camera glide, controls and effects work unchanged. The
  sandbox (waves, `?fleet=`, test flights, TEST IN RANGE) stays as it is.
- `mode.rs` gets `AppMode::Home` and `AppMode::Galaxy` next to `Flight`, `Drydock` and `Behavior`;
  `FlightExit` defaults to `Home` for voyages and stays `Drydock` for test flights.

### 5.2 Content model additions

New kinds follow `docs/content-model.md`: the common header (`id`, `name`, `summary`, optional `description`,
`lore`, `tags`), stable snake_case ids, keyed text (`band.rim.name`), links (`[[band:rim]]`,
`[[encounter:automaton_pack]]`), optional wiki prose (`wiki/band/rim.md`), placeholder comments at the top of
files with numbers. Sketches (field names are proposals):

```ron
// content/base/galaxy.ron: generator parameters (numbers are placeholders)
(
    id: "base", name: "The galaxy",
    summary: "Star clusters linked by lanes, from the rim to the precursor archive at the core.",
    generator: 1,                                   // bump when generated output changes (ADR-0024)
    extra_lanes: (chance: 0.35, reach: 1.3),
    gate_spread: 120,                               // degrees between gates of one boundary
    names: (
        first: ["Ka", "Ve", "Or", "Th", "Ny", "Sel"],
        second: ["ssa", "rin", "lon", "dra", "mae"],
        suffix: ["Drift", "Shoal", "Verge", "Expanse", "Reach", "Hollow"],
    ),
)
```

```ron
// content/base/bands.ron: load order = rim to core (numbers are placeholders)
[
    (
        id: "rim", name: "Rim",
        summary: "The galaxy's edge, where the shipyard sleeps among rogue automatons and old wrecks.",
        clusters: 12,
        races: ["automatons"],
        hazards: [],
        threat: (min: 0.3, max: 1.0),               // × the reference ship's build points
        reference_ship: "halvex_runner",
        gate: (guardian: "rim_warden", count: 3),   // gates on the boundary inward; the core has none
        cluster_types: [(id: "rim_drift", weight: 6), (id: "rim_graveyard", weight: 2)],
        set_pieces: [(cluster_type: "precursor_relay", count: 1)],
    ),
]
```

```ron
// content/base/clusters/rim_drift.ron (numbers are placeholders)
(
    id: "rim_drift", name: "Drift",
    summary: "A quiet cluster of rock and old wrecks, patrolled by automaton packs.",
    radius: 6500,                                   // m; validation: at most 8000
    star: "pale",                                   // art key for the backdrop's star
    spacing: 900,                                   // m between site anchors
    sites: [
        (site: "automaton_patrol", count: (2, 4), ring: (1500, 6000)),
        (site: "wreck_field", count: (1, 2), ring: (800, 5500)),
        (site: "rock_belt", count: (0, 1), ring: (3000, 4500)),
        (site: "scan_beacon", count: (2, 3), ring: (500, 6000)),
    ],
)
```

```ron
// content/base/sites/rim.ron
[
    (
        id: "automaton_patrol", name: "Automaton patrol",
        summary: "A small pack that circles its stretch of the cluster.",
        tags: ["patrol"],
        encounter: "automaton_pack",
        movement: Patrol(radius: 600, speed: 8),
    ),
    (
        id: "wreck_field", name: "Wreck field",
        summary: "Old hulls drifting together, some still holding intact parts.",
        tags: ["salvage", "derelict"],
        scenery: [(ship: "wreck_hauler", count: (2, 5), spread: 120)],
        encounter: "automaton_scavengers",
        movement: Guard(radius: 300),
        lore: ["shipyard_log"],                     // lore pool; used once scanning exists
    ),
]
```

```ron
// content/base/encounters/automatons.ron (numbers are placeholders)
[
    (
        id: "automaton_pack", name: "Automaton pack",
        summary: "Two to four raiders flying together.",
        faction: "automatons",
        ships: [(ship: "automaton_raider", count: (2, 4))],
        formation: Wedge(spacing: 18),
        refill: After(5400),                        // s of played voyage time
    ),
    (
        id: "rim_warden", name: "Rim warden",
        summary: "A gate guardian: a slab of precursor machinery that sheds sections as it breaks.",
        faction: "precursor_guardians",
        ships: [(ship: "rim_warden", count: (1, 1))],
        behavior: "warden",
        boss: true,
        refill: Never,
    ),
]
```

```ron
// content/base/factions/automatons.ron: new fields on the existing kind
(
    id: "automatons", name: "Rogue automatons", /* ... as today ... */
    glow: "enemy",
    behavior: "raider_basic",
    attitude: Hostile,                              // Hostile | Wary | Neutral (GDR-0027)
    standing: false,                                // the Lattice: true
)
```

```ron
// content/base/story/milestones.ron
[
    (
        id: "first_voyage_home", name: "Home again",
        summary: "The first haul comes home.",
        text: "...",                                // two to four sentences
        when: VoyagesDone(1),                       // NewGame | VoyagesDone(n) | BandReached(id) | GateOpened(band) | LoreFound(n)
        unlocks: [],                                // ring items or story flags
    ),
]
```

```ron
// content/base/lore/shipyard.ron
[
    (
        id: "shipyard_log_1", name: "Shipyard log, fragment 1",
        summary: "The last order the shipyard gave before it went dark.",
        lore: "...",
        pools: ["shipyard_log"],
    ),
]
```

Later kinds: `hazards/` (zone and effect, e.g. `zone: AroundStar(reach: 2500), effect: HeatOnOpenSides(0.5)`),
`missions/`. A `sensor: (range: 800)` block on logic cores (and later sensor parts) for gap 8.

Validation (errors unless noted): references exist (bands → cluster types, guardians, races; cluster types →
sites; sites → encounters, scenery ships, lore pools; encounters → ships, factions, behaviors); counts with
min ≤ max; rings inside the cluster radius; radius at most 8 km; every band but the last has a gate; the sum of
band counts within a sane range; an encounter's behavior fits its ships' rule slots; warning when an encounter
misses its band's threat range or a pool is unused.

Behavior vocabulary additions (content + sim, checked both ways, GDR-0037): actions `patrol(radius)`,
`guard(radius)` (return to the post when no target), `escort(distance)` (keep station on the leader when no
target; needed for escorts on autopilot), conditions `cores_left` (Count) and `parts_left` (Percent) for
guardian sections, `from_post` (Distance).

### 5.3 Sim changes

All hashed, all covered by the determinism check (ADR-0010).

1. **Sensor range**: the vessel's sensor range (the best `sensor` block of its running parts, else a default).
   Selectors in `behavior/eval.rs` (`select`), `under_attack`, `enemies_near` and the hand-over query only see
   ships within it. Frozen ships are never seen.
2. **Frozen ships**: `Sim::set_frozen(ship, bool)`. A frozen ship is skipped by think, flight, systems, weapons,
   plumes and impacts; its Rapier bodies are disabled (`RigidBody::set_enabled`, to be checked in Rapier
   `=0.36.1`), so they leave the broad phase and ray casts. It keeps every part state and its velocity. Hash: the
   flag; disabled bodies hash their stored pose.
3. **Inert fast path**: ships with no power, no heat above ambient and no commands (wrecks, rocks) skip
   `update_systems` until something changes (a hit, a contact).
4. **Posts**: `ShipCommand::Post(Option<Vec2>)`: the anchor the new `patrol` / `guard` / `from_post` nodes use.
5. **Relations** (second slice): `Sim::set_relation(a, b, Relation)`; behaviors target hostile owners only;
   provocation per gap 9. `friendly()` (`vessel.rs`) keeps its per-owner meaning.
6. **Queries**: `Sim::ships_within(point, radius, &mut Vec<ShipId>)` (ascending id; a scan over the ships, or
   Rapier's AABB query when that pays off); `Sim::aimed_at(ship)` (hostile vessels whose behavior targets it,
   from `aims_at`) for the hand-over and the recall rule.
7. **Scenery bodies**: big rocks may be Rapier fixed bodies (no solver cost) or sleeping dynamic bodies; G0
   decides (section 7). Rocks are hull-only ships without a core: salvage, harvestable later.
8. **Spawning asleep**: `spawn_ship` with a flag to start a body asleep (scenery), so 300 rocks do not run the
   solver for their first two seconds.

Unchanged: one `Sim` per cluster, `Owner` per side (`MAX_OWNERS = 8` is enough for a player, three co-op
players and four factions), `SimConfig::mind_capacity` per owner, the sim RNG streams (`Stream::Damage` to
`Stream::Scenario`; the director has its own streams in `galaxy`, also append-only).

### 5.4 The `galaxy` crate

- `skeleton.rs`: `GalaxySkeleton { clusters: Vec<ClusterNode>, lanes: Vec<Lane> }`, `ClusterNode { id: u16,
  name, band, pos: (i32, i32), cluster_type, seed: u64, tags }`, `generate(seed, content) -> GalaxySkeleton`;
  routes (Dijkstra over open lanes plus the gate network, ties by id).
- `interior.rs`: `ClusterLayout { radius, sites: Vec<Site>, exits: Vec<LaneExit> }`,
  `layout(skeleton, cluster id, content) -> ClusterLayout`.
- `director.rs`: `ClusterDirector` (a layout, site states record / awake / frozen / cleared, member ship ids per
  site, the clock, its own RNG streams); `load(&mut Sim, &deltas)` spawns scenery and the player's fleet at the
  entry; `pre_step(&mut Sim, &mut TickInputs)` streams sites, sets posts and behaviors, counts the recall;
  `post_step(&Sim)` reads `VesselLost` / `FocusChanged` to clear sites and detect death; `report() ->
  VoyageReport` (time, fights, cleared sites, haul placeholder, charted, death wreck); `hash(&mut
  StateHasher)`.
- `ClusterSpace { sim: Sim, director: ClusterDirector }` with `step(&TickInputs)` and `state_hash()` (the sim's
  hash folded with the director's), for tests, tools and `net`.
- `save.rs`: the save types (5.7), serde + RON, `migrate(text) -> Save`; no I/O (the client's `storage` writes
  them).
- `examples/voyage`: runs a scripted voyage (autopilot through every site, fights by behaviors) and prints
  per-tick hashes; `scripts/determinism-check.sh` gains `--scenario voyage` (it builds this example instead of
  `sim`'s `determinism`), and a generation-hash mode.

### 5.5 Determinism

- The director runs between steps, in the fixed tick order above, from sim state plus its own state only. No
  wall clock, no client RNG, libm trig, `BTreeMap` / `Vec` only.
- Spawns, freezes and posts are calls in site order, then ship order. The state hash covers them through the
  sim (spawned ships, frozen flags) and the director's hash (site states, clock, RNG).
- Same seed + content + deltas + inputs = same voyage hash on native, wasm and (later) Android. Checked by the
  voyage scenario, seeds 42 and 7, like the existing four.
- A resumed checkpoint (question 5, option a) is a new start: deterministic from the checkpoint on, not equal to
  the uninterrupted run.

### 5.6 Client screens

- **Home** (`crates/game/src/home.rs`, `AppMode::Home`): the mockup's layout with `ui_kit` ring buttons, pills
  and labels (`crates/game/src/ui_kit.rs`); the flagship drawn like the drydock's static ship view
  (`ship_model::design_visual`); story cards; keys D, B, G, L, R (drydock, behavior, galaxy map, launch, test
  range) *(lead default)*. The drydock, the BEHAVIOR screen and the map get a HOME button and Esc back.
  `?scene=home` starts here; making it the default start is the lead's call once it works.
- **Galaxy map** (`crates/game/src/galaxy_map/`, `AppMode::Galaxy`): world-space 2D meshes (`meshbuf`) for rings,
  lanes, nodes and the dotted route (the dotted-ring technique of `backdrop.rs`), its own pan and zoom; the panel
  in `bevy_ui` with `ui_kit`; icons as vector glyphs (`ui_icons`, content icons). Filters LANES / THREAT first,
  MATERIALS with the materials phase.
- **Voyage scene** (`scene.rs` `SceneKind::Voyage`, plus `crates/game/src/voyage.rs`): the warp screen (amortises
  generation and scenery spawns over frames), the cluster map overlay (same renderer as the galaxy map, at
  cluster scale), autopilot (`Maneuver::Hold { at }` for the flagship, `escort` for the others), hand-over on
  `Sim::aimed_at`, the RECALL button with its charge ring, a voyage line in the HUD (cluster, elapsed time,
  recall), off-screen markers for sites and lane exits (`fx.rs`), the star in the backdrop. No wreck cleanup by
  distance (gap 13).
- **Summary card** on return, then home.
- URL parameters for development: `?scene=home|galaxy|voyage`, `&galaxy=<seed>`, `&cluster=<id>`.

### 5.7 Save format

One document per save slot in `saves/` (ADR-0031: header line with namespace, format and time; newer formats
refused, older migrated). A sketch with the skeleton stored (question 6, option a):

```ron
// space-game: saves, format 1, saved 1760351234
(
    galaxy: (seed: 4207, generator: 1, packs: [("base", "0.1.0")]),
    skeleton: (
        clusters: [
            (id: 0, name: "The Shipyard", band: "rim", pos: (120, -950), cluster_type: "home", seed: 9182736455, tags: ["home", "gate"]),
            // ... about 80
        ],
        lanes: [(0, 4), (4, 9), (9, 17)],
    ),
    clock: 1296000,                       // ticks of played voyage time (30 Hz)
    flagship: "runner_mk2",               // a name in designs/
    charted: [0, 4, 9],
    gates: [0, 17],                       // gate clusters in the network
    clusters: {
        9: (visits: 2, sites: {3: (cleared_at: 1100000), 5: (known: true)}, wreck: None),
        17: (visits: 1, sites: {0: (cleared_at: 1250000)}, wreck: Some((pos: (812.5, -2040.0), haul: []))),
    },
    story: (milestones: ["wake", "first_voyage_home"], lore: ["shipyard_log_1"]),
    stats: (voyages: 12, deaths: 1),
)
```

- Designs and behaviors stay in their own namespaces (`designs/`, `behaviors/`); the save names them.
- Research, storage and materials join the save with their phases.
- Size: a few KB for the skeleton plus ~20 bytes per touched site; well under 100 KB after 100 voyages
  *(estimate, to measure)*.
- The mid-voyage checkpoint (question 5) is a second document, `saves/<slot>-voyage`, deleted at voyage end.
- Export and import as a file and a share code (`SGS1:` + base64url of zlib RON, like design codes) later.

### 5.8 Proposed records

- **ADR (proposed)**: a `galaxy` crate for generation, the cluster director and save types; engine-free and
  deterministic (gap 6).
- **ADR (proposed)**: cluster scale and coordinates: radius at most 8 km, origin at the star, no origin
  shifting; transit time-skip as a client option (gap 1, question 1).
- **ADR (proposed)**: streaming inside a cluster: record → awake → frozen with hysteresis; sensor range in the
  sim (gaps 7, 8).
- **ADR (proposed, refines ADR-0024)**: saves keep the galaxy skeleton; deltas keyed by cluster and site id;
  mid-voyage checkpoints (gaps 14, 15, questions 5, 6).
- **GDR (proposed)**: galaxy generation v1 and band gating (gap 2, question 2).
- **GDR (proposed)**: the voyage flow v1: destinations, chained warps, recall, death wreck, fuel (gaps 3–5,
  questions 3, 4).
- **GDR (proposed)**: home is the ring menu; the home cluster for the first hour (gap 16, question 7).
- **GDR (proposed)**: story delivery (gap 17, question 8).
- **GDR (lead default)**: encounter refill by played voyage time; wrecks per voyage; guardians as sectioned
  vessels; threat budgets (gaps 10–13).

## 6. Tracks

Sizes: **S** = one agent session, **M** = one or two sessions, **L** = several sessions or two waves. Track ids
are this plan's; the lead renumbers them into a phase.

**Slice 1: one rim voyage** (the minimal loop: home → map → voyage into a generated rim cluster with automaton
encounters → recall → home, saved).

| Track | Goal | Depends on | Size | Can run in parallel with |
|---|---|---|---|---|
| V0 Measure first | A `cluster_perf` example in `sim` (kept as the cluster load test, like `load_test`), on today's API: 300–1000 sleeping rock ships and 100 inert wrecks next to 20–40 fighting ships, a fight 6 km and 30 km from the origin, spawn hitches, a 64×64 guardian under fire. Fixed and disabled Rapier bodies in a throwaway harness outside the repo (ADR-0025), re-measured in the example after V2. Numbers to `docs/results/` | — | S | V1, V2, V5 |
| V1 Content kinds | `data`: `galaxy`, `bands`, `clusters`, `sites`, `encounters`, faction `attitude`, `story`, `lore`; validation, JSON export, links; base content: five band entries (only the rim filled), one rim cluster type, three sites, two automaton encounters, a placeholder home cluster | — | M | V0, V2, V5 |
| V2 Sim support | Sensor range in selectors; `set_frozen` with disabled bodies; inert fast path; posts and the `patrol` / `guard` / `escort` / `from_post` nodes; `ships_within`, `aimed_at`; spawn asleep; determinism check green | — (V0 informs) | M | V0, V1, V5 |
| V3 Generation | `galaxy` crate: skeleton, cluster interiors, routes; generation hash native vs wasm; the proposed ADR | V1 (types) | M | V2, V5, V6 |
| V4 Cluster director | Load a cluster into a `Sim`; streaming; encounter spawn, posts and formations; cleared detection; recall rule; death; voyage report; `ClusterSpace`; voyage scenario in the determinism check | V2, V3 | L | V6, V8 |
| V5 Home ring menu | `AppMode::Home` and `AppMode::Galaxy` in `mode.rs`; the ring menu per mockup wired to drydock, behavior, test range; locked research / fabricator / storage; HOME and Esc on the other screens | — | M | V0–V3 |
| V6 Galaxy map | Map renderer, pan and zoom, cluster states, route and voyage panel, SET COURSE (on a fixed test skeleton until V3 lands) | V5 (mode), V3 | M | V4, V8 |
| V7 Voyage scene | `SceneKind::Voyage`: warp screen, arrival, cluster map overlay, autopilot with hand-over, recall, death to home, summary card | V4, V5, V6 | L | V8 |
| V8 Saves | `saves/` document: skeleton, clock, deltas; save at home and at voyage end, load at start; migration hook | V3, V4 (types) | M | V6, V7 |

Order for parallel agents: wave 1 V0, V1, V2, V5; wave 2 V3 (once V1's types land), V6 on a stub; wave 3 V4, V6,
V8; wave 4 V7, then V8 finishes the wiring. V5 owns the `AppMode` change so V6 and V7 do not collide in
`mode.rs`; V2 and V4 never touch `game`.

**Slice 2: bands, gates and the story** (after Niklas has flown slice 1).

| Track | Goal | Depends on | Size | Can run in parallel with |
|---|---|---|---|---|
| V9 Guardians and gates | The rim guardian (sectioned blueprint, `warden` behavior with `cores_left` / `parts_left`), gate site, gate lane opening, gate network on the map and in routes | V7, V8, V0 (guardian cost) | L | V10, V11, V12 |
| V10 Hazards | Hazard zones in the sim (stellar heat first), `hazards/` content, HUD warning, map chips | V4 | M | V9, V11, V12 |
| V11 Relations and races | Relation table, wary and neutral attitudes, provocation, the neutral glow (after Niklas picks a look); a second faction once its race mechanics exist | V2 | M | V9, V10, V12 |
| V12 Story and lore | Milestone cards, memory log, lore pools on sites (fragments shown once scanning exists), home cluster hooks for the first hour | V5, V7 | M | V9, V10, V11 |
| V13 Supplies | Fuel for warps and recall (question 4), refill at home, the panel's bars | V7 | S | V9–V12 |
| V14 Mid-voyage checkpoint | Question 5, option a | V8 | M | V9–V13 |
| V15 Refill and event layer | Refill by the clock on arrival; hooks for roaming bosses and Swarm raids (stubs until those races exist) | V4, V8 | S | V9–V14 |
| V16 Transit time-skip | Run 2–4 ticks per frame in autopilot with no hostile in sensor range (question 1) | V7 | S | V9–V15 |

Outside this plan: salvage, scanning, research and fabrication (their own phase; the voyage report carries an
empty haul until then), missions and race stations, co-op with several spaces, new game+.

## 7. Risks and what to prototype or measure first

1. **Sim cost of a cluster-sized space** (V0, before V2's design is final). Measure the step (median, p95, the
   Rapier part separately; native release; load average noted) for: 20 fighting ships alone; plus 300 and 1000
   sleeping rock bodies; plus 100 inert wrecks; then, in a throwaway Rapier harness (or after V2), the same rocks
   as fixed bodies and 400 frozen ships as disabled bodies. Questions: does the broad phase grow with sleeping bodies? Is a disabled body free? What does
   `update_systems` cost per inert ship? Today's baseline: 20 ships 0.011 ms median, 0.028 ms p95 per step
   (`docs/sim.md` "The load test", new VM). Budget *(placeholder)*: the whole cluster under 1 ms native, so the
   phone keeps its 4 ms sim budget (ADR-0009).
2. **`f32` far from the origin.** Run the load test shifted to (6 km, 6 km) and to (30 km, 30 km): contact
   stability, ram counts, jitter of resting contacts, native vs wasm hashes identical. Expect fine at 6 km and
   visible trouble at 30 km; that confirms the 8 km limit.
3. **Guardian cost.** A 64×64 guardian (~3000 parts, 6 sections) under fire from 6 ships: per-tick p95 during the
   first wave of hits, piece rebuild cost per destroyed part, section splits. The combat benchmark already shows
   slow ticks when ~40 parts of a 2000-part piece die in one tick (`docs/sim.md` "Performance"). If it is too
   slow, guardians need cheaper rebuilds (partial compound updates) before V9.
4. **Spawn hitches.** Spawning 4–8 ships plus scenery in one tick: `ShipDesign::compile` is cached per blueprint,
   but `Ship::new`, bodies and the mind pass run per ship. Measure; the director already spreads spawns.
5. **Behavior cost with many awake ships.** Target selection is a scan over all ships per think (every 3 ticks).
   With the sensor range and frozen ships skipped it stays small; measure 40 awake ships with 400 frozen.
6. **Generator stability.** Content changes reshuffling saves (gap 14). Mitigated by storing the skeleton;
   measure generation time (skeleton and one interior, native and wasm; budget 50 ms *(placeholder)*).
7. **Client randomness leaking into voyages.** The sandbox pattern (client RNG, std trig, direct spawns) must not
   be copied; a review point for V4 and V7.
8. **Scope.** Voyages touch salvage, research, fabrication, missions, hazards and races. Slice 1 has none of them;
   its voyage is "fly, fight, come home". Niklas should fly it before slice 2 is fixed.
9. **Content volume.** Bands past the rim need new race mechanics (regrowing Grown hulls, Swarm drones, Lattice
   prisms, phase blinks) and designs. Until those exist, only the rim is real; the other bands are stubs.
10. **Phone sessions.** App kills mid-voyage (question 5) and long warps (gap 3). Keep warps short and checkpoint.

## 8. Questions for Niklas

Asked interactively later; each stands on its own.

**Q1. How big is a cluster, and how do you cross it?**
- (Recommended) About 13 km across; you cruise at your ship's own top speed on autopilot (~10 min across), and
  the game may fast-forward transit while no enemy is near. Trade-off: physics and precision stay as they are;
  the fast-forward is single-player only (or host-agreed in co-op).
- The same size, never fast-forwarded. Trade-off: simplest, but long flights can drag on a phone.
- A separate cruise drive (5–10× speed outside fights) and clusters of 60–130 km. Trade-off: feels vast, but needs
  coordinate re-basing and a new flight mode beside the rated speeds.

**Q2. Do guardians block the way into the next band?**
- (Recommended) Yes: lanes cross a band boundary only at gate clusters; beating any one of that boundary's
  guardians opens the next band, the others stay optional bosses. Trade-off: clear goals, some freedom.
- No: lanes cross anywhere; difficulty and hazards gate; guardians only add fast gates. Trade-off: most freedom,
  but guardians become optional and players can rush inward.
- Yes, and every guardian of a boundary must fall first. Trade-off: longest path, the least choice.

**Q3. How do you get home from a voyage?**
- (Recommended) Recall any time: it charges for about 15 s and cannot start while an enemy is aiming at you.
  Trade-off: phone-friendly, fights still matter.
- Fly back to a lane exit or a gate and warp home. Trade-off: more tension, longer sessions.
- Instant return at any moment. Trade-off: simplest, but danger and death lose their bite.

**Q4. What is voyage fuel spent on?**
- (Recommended) Only on warp hops and the recall; ammo comes with missiles and ammo racks, repair material with
  repair drones. Trade-off: flying stays free; supplies arrive with the parts that need them.
- Engines burn fuel too, and all three supplies start now. Trade-off: closest to the original list, but adds a
  fuel model to every flight and to the drydock stats.
- No supplies until salvage and research exist. Trade-off: simplest, but voyages have no limit for now.

**Q5. What happens if the game is closed in the middle of a voyage?**
- (Recommended) A light checkpoint every minute: your ships, damage, haul and cleared sites are kept, and the
  cluster is rebuilt around them on resume. Trade-off: small saves; the resumed run is not an exact replay.
- Saving only at home; closing mid-voyage counts as a recall. Trade-off: simplest, generous to the player.
- A full snapshot of the whole space. Trade-off: exact resume and a base for joining co-op, but a large
  engineering job.

**Q6. Should a save hold the galaxy's layout, or rebuild it from the seed?**
- (Recommended) Store the small layout (about 80 clusters and their lanes, a few KB) next to the seed; cluster
  insides are rebuilt from the seed. Trade-off: game updates and mods never reshuffle a running galaxy; refines
  ADR-0024 with a new record.
- Rebuild everything from seed and generator version (ADR-0024 as written). Trade-off: the smallest saves, but
  every generator setting must stay frozen per version or old galaxies change shape.

**Q7. Is home a menu or a place you fly in?**
- (Recommended) Home is the ring menu around your docked ship; the first hour plays in a small home cluster
  around the shipyard that you can revisit. Trade-off: fast voyages, a real place for the story's start.
- Home is a flyable cluster you undock into before every voyage. Trade-off: more immersive, more travel each
  time.

**Q8. How is the light story told?**
- (Recommended) Short text cards at milestones (waking, the first voyage home, each gate, each new band, the
  archive) plus a memory log of lore fragments found on derelicts. Trade-off: little writing, all in content.
- The same plus short inner-voice lines in the flight HUD during voyages. Trade-off: more presence, more text
  and HUD space.
- Lore on derelicts only. Trade-off: the least work, but no sense of progress in the story.
