# Plan: exploring the galaxy

Status: **proposal, reworked 14 Oct 2026** around [GDR-0075](../gdr/0075-voyages-feel-like-exploration.md) (the
direction), [GDR-0078](../gdr/0078-galaxy-exploration-v1.md) (Niklas's answers on discovery, hints, travel and
gates) and [GDR-0080](../gdr/0080-exploration-searching-and-story.md) / [ADR-0035](../adr/0035-saves-keep-the-galaxy-skeleton.md)
(searching, the scanner part, the story, the skeleton in the save). The first version (13 Oct, written before phase 7 was built) laid out lanes, routes and visible gates; it is
in git history (commit `e1f3d67`) for its technical detail. Every number marked *(placeholder)* is for feel, not a
spec. Niklas answered the questions of [section 9](#9-questions-for-niklas) the same day; the roadmap and the
record indexes are the lead's to update. **All tracks (E0–E8) built by 17 Oct 2026**; the last two in
[GDR-0098](../gdr/0098-slice-in-the-galaxy-and-rim-rewards.md).

## 1. Summary

The player is a ship-mind at the rim of a galaxy it does not remember. Most of the map is dark. Every voyage turns
something up: wrecks whose logs, decoded at home, point somewhere; beacons and odd readings that, once scanned,
mark an area on the map. These **signals** start wide and narrow as the player searches: first an area of the
galaxy, then, inside a cluster, a bearing from a scan, and the last stretch by flying. At the end of a trail lies
something worth the trip: a precursor cache, a lore fragment, a rich graveyard, or the gate to the next band and
its guardian. Between trails the player may always **jump blind** into unknown space nearby, for ordinary finds and
sometimes a new signal. Travel is by **jump range** from home and from any charted cluster (GDR-0093); research grows it. Nothing on
the map is a route.

## 2. Where we start

### Built (phase 7, 13–14 Oct 2026)

- **Home** is the shipyard, a long U dock with its buildings (GDR-0071); the gate in front of it opens the voyage
  map through a portal (14 Oct).
- **Voyages**: four small hand-made rim clusters (600–1000 m radius) in `content/base/voyages/rim.ron`, opened one
  after the other by clearing a goal site (`opens_after`, GDR-0062). The voyage director in `sim::voyage` rolls a
  plan from a seed (sites, ships, wrecks), spawns sites when the fleet comes near, wakes them, clears them when
  every enemy is destroyed or disarmed (GDR-0068), and checks the recall (ADR-0033, `docs/sim.md` "Voyages").
- **Salvage and research**: collecting by flying slowly over wrecks (GDR-0063, now up to 8 m/s, GDR-0079), cargo
  holds, core fragments, research data by family, unloading the haul at home, the research tree (GDR-0066,
  `docs/career.md`).
- **The career save** (`data::career`, format 1): unlocks, open and cleared voyages, voyage seeds from the game
  seed, the voyage id and the launch count, the haul. (Format 2, 15 Oct 2026: the voyage history; format 3, 16 Oct
  2026, E4: the galaxy.)
- **The voyage map** (`crates/client/game/src/map/`): a star map of the rim with the voyages, a panel with SET COURSE,
  zoom out to the whole galaxy (five bands drawn as placeholders).
- **Being changed now** (GDR-0078's slice step): the map no longer draws a route between voyages; voyages not yet
  opened stay unknown; clearing a goal reveals the next voyage as a new signal.

### Decided and still standing

| What | Record |
|---|---|
| A ship-mind wakes in an abandoned precursor shipyard at the rim; the story stays light: lore on derelicts, procedural finds | [GDR-0002](../gdr/0002-awakened-ship-mind-light-story.md) |
| One procedural galaxy per game seed, about 80 clusters, five bands rim → core, a precursor guardian at every band gate; set pieces placed in; content types derelicts, stations, outposts, bosses, megastructures, anomalies | [GDR-0026](../gdr/0026-procedural-galaxy-and-bands.md) (lanes superseded) |
| Races per band, for variety and tech | [GDR-0027](../gdr/0027-races.md) |
| A default voyage covers one cluster in 15–30 minutes; fights, salvage and scanning, light missions | [GDR-0028](../gdr/0028-clusters-and-voyages.md) (lanes superseded) |
| Death is mild: the ship re-forms at home, part of the haul stays behind | [GDR-0029](../gdr/0029-mild-death-bonded-matter.md) |
| Salvage and scans become research only at home | [GDR-0025](../gdr/0025-research-from-salvage.md) |
| Recall home in 3 s when no armed enemy is near; closing mid-voyage loses it; no mid-voyage saves in the slice | [GDR-0062](../gdr/0062-first-playable-loop.md), [GDR-0072](../gdr/0072-shipyard-parts-voyage-flight-defaults.md), [GDR-0074](../gdr/0074-test-range-waves-emergency-recall-cannon.md) |
| **Signals and blind jumps; signals narrow as you search; hints from derelicts and in-cluster signals; trails lead to gates, caches, lore and rich salvage; jump range instead of lanes; later gates hidden; sites revealed by scanning** | [GDR-0078](../gdr/0078-galaxy-exploration-v1.md) |
| One simulation space per cluster; only occupied clusters simulate | [ADR-0022](../adr/0022-one-simulation-space-per-cluster.md) |
| Saves store seed, generator version and deltas | [ADR-0024](../adr/0024-saves-seed-and-deltas.md) |
| Bit-exact determinism in `sim`; the career model, save and voyage director | [ADR-0010](../adr/0010-cross-platform-determinism.md), [ADR-0033](../adr/0033-career-model-save-and-voyage-director.md) |

## 3. The loop

1. **At home** the map shows what the player knows: home, charted clusters, the jump-range circles around home and
   every opened gate, and the active **signals** as soft areas with a short line of what found them ("a log from a
   hauler's wreck", "a beacon in Halvex Drift").
2. **Choosing a voyage**: a signal's area, a charted cluster (to finish it or for salvage), or a **blind jump** to
   any unknown spot within range. A signal never says exactly where; it says roughly where and what kind of thing
   (a gate, a cache, a graveyard, something old).
3. **The voyage**: the fleet arrives at the cluster's edge. Some sites show at once (the bright ones: a star, a big
   wreck field); the rest show after a **scan**. A scan also gives every active signal it can hear a bearing: a
   line from where it was taken. Two scans from different spots cross; the crossing is the place.
4. **Finding the place**: flying close reveals it (a site appears, sleeps or wakes like any other). Clearing or
   salvaging it completes the signal and pays its reward; often it holds the next signal of its trail (a log in a
   wreck, a beacon that starts up).
5. **Home**: the debrief says what was found ("SIGNAL FOUND", "SIGNAL NARROWED", "CHARTED: Corran Reach"); salvaged
   logs wait in the research screen to be decoded into signals; research may grow the jump range.

Wrong guesses are fine and cheap: a cluster inside a signal's area that is not the place still has its ordinary
finds, and a scan there gives a bearing out of it towards the place, which narrows the area on the map.

## 4. Design proposal

### 4.1 Signals

A signal is a pointer from a **source** to a **target**, stored in the save.

- **Target**: a hidden place: a site in a cluster (a cache, a lore wreck, a graveyard, a gate's guardian), fixed by
  the galaxy generator, so it is the same for every way of finding it.
- **Area**: a circle on the galaxy map that holds the target's cluster. A fresh signal's circle is wide *(placeholder:
  2–3 cluster spacings across, usually 3–6 clusters inside; 17 Oct 2026: 3–4 S across, about 5.6 clusters inside,
  [GDR-0098](../gdr/0098-slice-in-the-galaxy-and-rim-rewards.md))*. Its centre is offset from the target by a seeded
  amount, so the target is not simply in the middle.
- **Narrowing**:
  - *On the map*: a scan in a cluster inside the area but not the target's shrinks the area to the side the bearing
    points to *(placeholder: the circle halves, still holding the target; 17 Oct 2026: it keeps 60 % of its radius,
    since halving located 83 % of signals with the first scan, GDR-0098)*. A second source for the same target
    (another log of the same trail) shrinks it too.
  - *In the target's cluster*: a scan gives a bearing to the site; a second scan from elsewhere crosses it. At close
    range *(placeholder: 400 m)* the site is revealed.
- **States**: fresh → narrowed (any number of times) → located (the cluster is known) → found (the site was seen)
  → done (cleared or salvaged; built 16 Oct 2026: cleared, or seen with nothing to clear, [GDR-0097](../gdr/0097-galaxy-map-and-voyage-flow.md)). The map draws fresh and narrowed as areas, located as a mark on a cluster.
- **How many at once**: no limit in the rules; the generator places trails so that 2–4 are usually open
  *(placeholder)*. Old signals never expire.

### 4.2 Where signals come from (GDR-0078: derelicts and in-cluster signals)

- **Logs in wrecks**: some wrecks carry a **data core** (a salvage item, like a core fragment). Brought home, it
  is decoded in the research screen, free (GDR-0080), into a signal (built 16 Oct 2026 at the LOG instead, [GDR-0097](../gdr/0097-galaxy-map-and-voyage-flow.md)). The
  debrief lists it as "1 LOG TO DECODE".
- **Signals heard in a cluster**: a beacon, a distress call or an odd reading is a site with no enemies. Flying close
  and scanning it turns it into a signal at once, during the voyage.
- Trails tie them together: the place a signal leads to often holds the next source (a beacon wakes, a wreck has a
  log).

### 4.3 Trails and what they lead to

A **trail** is a chain of 1–4 signals *(placeholder)* the generator builds per galaxy from content templates. Its
last place holds the reward:

| Reward | What it is | Trail length *(placeholder)* |
|---|---|---|
| **Band gate** | The gate cluster of the next band with its guardian. Any one gate per boundary opens the band (GDR-0078). The rim's first gate is the Warden's (the slice; built 17 Oct 2026, its four voyages the gate's trail) | 3–4 |
| **Precursor cache** | A research node or a rare part found nowhere else | 2–3 |
| **Lore** | A fragment of what happened to the precursors and to the ship-mind, kept in a memory log | 1–2 |
| **Rich salvage** | A graveyard or a wreck field with far more to collect than usual, often guarded | 1–2 |

Gates come in 3 per boundary *(placeholder)*, each at the end of its own trail; the first signal of each gate trail
is reachable from the band's start without the others, so there is always more than one way on.

Built 17 Oct 2026 (E8, [GDR-0098](../gdr/0098-slice-in-the-galaxy-and-rim-rewards.md)): the last signal of a trail done
gives its reward once, kept on a lost voyage like every find: a gate's guardian beaten opens the gate (the rim's
generated gates keep a gate sentinel); a cache's trail gives research data and programmable matter into store
(`trails.ron`'s `gives`; a node or a part found nowhere else is later) beside a sealed cache's rich wrecks; a lore
trail's memory wreck, seen, puts its fragment into the memory log (the LOG's MEMORY rows); a graveyard trail ends
at a deep graveyard. Milestone cards (GDR-0080) wait.

### 4.4 Blind jumps

Any unknown spot within jump range can be a destination. The generator already knows what is there (the galaxy is
fixed per seed): an ordinary cluster of its band's type, with patrols, wreck fields and, now and then, a beacon or a
wreck with a log that starts a short trail *(placeholder: 1 in 3 clusters holds a source)*. A blind jump charts
the cluster. Empty space between clusters is not a destination; a jump picks the cluster nearest the chosen spot
(built 16 Oct 2026, [GDR-0097](../gdr/0097-galaxy-map-and-voyage-flow.md): within 1.5 spacings, never home or a slice voyage's; a scan narrows a signal once per voyage).

### 4.5 Jump range and travel

- From home and from any charted cluster (GDR-0093, 16 Oct 2026; was: home and opened gates) the player can launch
  to anything within its **jump range**: 2.5 S at the start, growing to 4 S by two research nodes of +0.75 S in the
  propulsion family (Niklas, GDR-0093; S is the spacing unit, about one slice voyage to the next). The map draws the
  range as soft circles.
- A target outside every circle is a reason to research range or open a nearer gate.
- Warp time grows with distance (3 s + 2 s per S, GDR-0093); the voyage starts at the
  destination's edge. Built 16 Oct 2026 as a warp on the map before the launch ([GDR-0097](../gdr/0097-galaxy-map-and-voyage-flow.md)). Coming home stays the recall (GDR-0062).
- Built 16 Oct 2026 (E4, [GDR-0096](../gdr/0096-career-galaxy-format-3.md)): `Unlock::JumpRange` and Jump calibration I
  and II (propulsion 8, then 16 and 3 programmable matter); `Career::jump_range` and `jump_from` (the nearest of home
  and the charted clusters within range); arriving charts a cluster.
- No lanes: GDR-0078 supersedes GDR-0026's and GDR-0028's lanes.

### 4.6 Inside a cluster: scanning

- **Clusters grow** for searching to have room: 2–3 km radius (GDR-0080), against the slice's 0.6–1 km.
  Bigger needs the old plan's work on sensor range and freezing far sites (section 6.3). E0 measured it (16 Oct
  2026, `docs/results/2026-10-16-bigger-clusters.md`): `f32` holds with the cluster centred on the origin up to
  about 4 km (beyond it slow drift goes visibly wrong), and a whole 2–3 km cluster spawned and asleep costs
  0.15–0.5 ms per tick in wasm.
- **Visible on arrival**: the star, lane-less edge markers, large wreck fields and anything within sensor range.
- **A scan needs a scanner part** built into the ship (GDR-0080): a ship without one cannot scan, so scanning is
  a drydock trade-off (room, power, mass); better scanners come by research. A scan is an action with a short
  charge *(lead default)*, then a sweep that reveals sites within the scanner's range *(placeholder: 1.2 km)* and
  gives each active signal it can hear a bearing. It wakes sleeping enemies nearby *(placeholder: within 300 m)*,
  so where you scan matters. The starter or an early research node must give a scanner (E8). Built 16 Oct 2026
  (E3, [GDR-0095](../gdr/0095-sensors-hidden-sites-and-scans.md)): the starter carries one (GDR-0093); the charge is
  2 s on the scanner's power; until E4 every trail place (and the slice's goal, until E7) counts as a signal a scan
  hears.
- **Bearings** show in flight as a line from the scan spot and on the cluster map overlay; two crossing lines mark
  the place. (In flight since E3: dashed gold lines; since E6 also on the cluster map, N, [GDR-0097](../gdr/0097-galaxy-map-and-voyage-flow.md).)

### 4.7 The slice's four voyages

They stay the first steps (GDR-0078). After the step being built now (no route, unknown until opened, a signal on
clearing), the later move is to place them into the generated rim as set pieces: the Shipyard approach is home's
own cluster, Halvex Drift and Corran Reach are hand-made clusters inside the first signals' areas, the Warden's gate
is the rim's first gate trail. Until the generator exists they stay as they are. Until E7 (16 Oct 2026, E4,
[GDR-0096](../gdr/0096-career-galaxy-format-3.md)): the career pins each voyage to a rim cluster, the first nearest home
and each later one nearest the voyage it opens after; flown voyages chart their clusters, the open one is a located
signal, and that is how a slice save migrates. Built 17 Oct 2026 (E7, [GDR-0098](../gdr/0098-slice-in-the-galaxy-and-rim-rewards.md)):
`galaxy.ron`'s set pieces place them at Niklas's distances from home (the Shipyard approach 0.9 S out, not home's own
cluster, which the map draws as the shipyard), the Warden on the rim gate nearest the band's inner edge 2.8–3.5 S
out, its voyages the gate's trail: clearing it opens the outer reach. Their signals stay located when they open. A
format 3 save keeps E4's pins; its Warden's cluster becomes a gate at load.

### 4.8 Placeholder numbers

| What | Value |
|---|---|
| Clusters per band (rim, outer, middle, inner, core) | by the bands' areas at 0.9 S spacing: about 37, 18, 13, 8, 3 (77–85; GDR-0094, was 12, 18, 22, 24, 4) |
| Fresh signal area | 2–3 S across (17 Oct 2026: 3–4 S, GDR-0098) |
| Narrowing per scan or second source | the area halves (17 Oct 2026: to 60 % of its radius) |
| Free clusters within the starting range of home | 3 neighbours (17 Oct 2026: 4 without a set piece) |
| Site revealed within | 400 m |
| Trails open at once | usually 2–4 |
| Gate trails per boundary | 3, each 3–4 signals long |
| Clusters holding a signal source | 1 in 3 |
| Jump range | 2.5 → 4 S by research (Niklas, GDR-0093) |
| Warp time | 3 s + 2 s per S (GDR-0093) |
| Cluster radius for searching | 2–3 km |
| Scan radius; scan wakes enemies within | 1.2 km; 300 m (the scanner: 1×1, 3 BP, 3 power while it charges 2 s, 8 s cooldown; GDR-0093) |

## 5. Galaxy generation

All of it a pure function of (game seed, generator version, content): PCG32 streams, libm math, ordered
containers, ascending ids (ADR-0010). A generation hash is checked native vs wasm. **Built 16 Oct 2026** (track E2,
the `galaxy` crate, [ADR-0042](../adr/0042-galaxy-generator-crate.md); its numbers [GDR-0094](../gdr/0094-galaxy-generator-defaults.md)):
steps 1–4 and 6 as below, step 1 changed: the plan's counts per band could not connect at the slice's spacing.

1. **Clusters** fill the whole disc by Poisson-disc sampling at the galaxy's spacing (0.9 S), so every spot lies
   within about one spacing of a cluster; each belongs to the band whose ring holds it (counts follow the bands'
   areas); positions are integers in S/100. Each gets a type from its band's weighted list, a seed and a name
   (syllables from content, on their own stream). Every band's clusters must be reachable hop by hop within the
   starting jump range from its entrances, else the placement is re-rolled (bounded).
2. **Home** is the rim cluster nearest a random angle; at least 3 rim clusters *(placeholder)* lie within the
   starting jump range, otherwise the angle is re-rolled (bounded, deterministic). (17 Oct 2026: with the set pieces
   placed from it, and 4 rim clusters without one in range, GDR-0098.)
3. **Gates**: per boundary, 3 clusters of the outer band near its inner edge, spread around the disc.
4. **Trails**: for each gate, and for a number of caches, lore fragments and graveyards per band (content counts),
   pick the target's cluster, then walk outward towards home picking 0–3 earlier places for the trail's sources
   (each the previous one's target), and decide each source's form (a log in a wreck site, a beacon site). Checks:
   every gate trail's first source is reachable from the band's start; no two targets share a site. (Built: the
   walk prefers places nearer the band's entrances, 0.9–2.5 S per step; no two rewards share a cluster.)
5. **Hand-made set pieces** (the slice's voyages first) are pinned to clusters by content. (Built 17 Oct 2026, E7: with
   home, step 2, so its free neighbours are counted without them; generator version 2.)
6. **Interiors** are generated on demand at warp time from the cluster's seed and type, with the trail places the
   cluster holds put in as fixed sites.

The save stores this skeleton (about 80 clusters and the trails, a few KB; ADR-0035), so updates and mods never
reshuffle a running galaxy; interiors are still generated from each cluster's seed.

## 6. Technical plan

### 6.1 Where the logic lives

- **`data`**: new content kinds: `galaxy` (generator numbers, names), `bands` (counts, cluster types, gate
  guardians, trail counts), `cluster_types` (radius, site entries), `trails` (templates: reward kind, length, source
  forms), `signal_sources` (beacon and log site kinds), `lore` (fragments). Validation and JSON export as for every
  kind (`docs/content-model.md`).
- **`data::career`** (save format 2, with a migration from 1): charted clusters, signals (target, area, state,
  sources seen), decoded and undecoded logs, opened gates, the jump range's research, the lore log; the voyage seed
  rule stays. Built 16 Oct 2026 (E4) as the save's format 3 with a migration from 2 (format 2 had gone to the history),
  the lore log left for E8; the skeleton's types moved to `data::skeleton`.
- **Generation**: a new engine-free crate `galaxy` (depends on `data`; no Bevy, no I/O, wasm-safe), or a module in
  `data`. Recommendation: the crate, as the 13 Oct plan proposed; it holds the skeleton, trails and interiors.
  Built 16 Oct 2026 as the crate ([ADR-0042](../adr/0042-galaxy-generator-crate.md)); the content kinds above are
  in `data` (`docs/content-model.md` "The galaxy").
- **`sim::voyage`** grows from a hand-made plan to a generated one: the plan takes sites from an interior instead of
  `voyages/*.ron`, plus hidden sites (not shown until revealed) and beacon sites. Scanning is a sim command
  (`ShipCommand::Scan`) with a result event (sites revealed, bearings), so it is deterministic and in the hash.
- **`game`**: the map's signal areas, range circles and blind-jump picking; the scan control and bearing lines in
  flight; the debrief's finds; decoding logs on the research screen.

### 6.2 What carries over from the 13 Oct plan

Still valid, now needed for bigger clusters: sensor range for behaviors (target selection sees only what sensors
detect, GDR-0017), frozen far sites (record → awake → frozen with hysteresis), posts and the `patrol` / `guard`
nodes, an inert fast path for wrecks, measuring a cluster-sized space first (the V0 load test). Not needed any more:
lanes, routes, the gate network as instant links between gates (a gate is now a place to jump from), the ring menu.
E0 (16 Oct 2026) narrowed the freezing: sites stay records until the fleet comes near (the director's lazy spawning,
built) and spawned sites stay asleep; freezing them back to records with hysteresis, and the inert fast path, can wait
until a cluster passes about 1000 bodies or the client's per-ship cost asks for it. E3 (16 Oct 2026) built sensor
range (800 m on cores, GDR-0095) and kept the streaming as E0 recommends.

### 6.3 Determinism

Generation, the director, scans and signal narrowing are all deterministic and in the voyage hash or the
generation hash. The client only draws and sends commands.

## 7. Tracks

Sizes: **S** one agent session, **M** one or two, **L** several.

| Track | Goal | Depends on | Size |
|---|---|---|---|
| E0 Measure | A cluster of 2–3 km with 20–40 awake and 200 frozen ships: step cost, `f32` at 3 km, spawn hitches (the old V0). **Done 16 Oct 2026** (`docs/results/2026-10-16-bigger-clusters.md`, `scripts/cluster-bench.sh`): one `f32` space centred on the origin, radius ≤ 4 km; no despawning needed for cost (lazy spawning, sleeping sites); spawn per site | — | S |
| E1 Content kinds | `galaxy`, `bands`, `cluster_types`, `trails`, `signal_sources`, `lore`; the rim filled, the other bands stubs. **Done 16 Oct 2026**: the six kinds with validation, JSON export and wiki pages (a page per entry, `mechanics/galaxy`); S in `stats.ron`; the jump-range research nodes and their unlock kind wait for E4 | — | M |
| E2 Generator | The `galaxy` crate: clusters, home, gates, trails, interiors; generation hash native vs wasm. **Done 16 Oct 2026** (ADR-0042, GDR-0094): `galaxy::generate` and `interior`, `content-tool galaxy [--svg]`, `scripts/determinism-check.sh --galaxy N` (50 seeds identical native vs wasm) | E1 | M |
| E3 Sim: bigger clusters | Sensor range, frozen sites, hidden sites, `Scan` with bearings, beacon sites; determinism green. **Done 16 Oct 2026** ([GDR-0095](../gdr/0095-sensors-hidden-sites-and-scans.md)): the scanner part on the starter, SCAN (G) and the `scan` action; cores see 800 m; sites hidden until sensors (a signal's site: 400 m) or a sweep reach them, bearings to the far ones, wakes within 300 m; the edge turns ships back; `VoyagePlan::roll_interior` (beacons alone as quiet sites); no despawning (E0); native = wasm. Fly a generated cluster: `?scene=voyage&cluster=signal` | E0 | M |
| E4 Career format 2 | Charted, signals and narrowing, logs, gates, range; migration from format 1. **Done 16 Oct 2026** ([GDR-0096](../gdr/0096-career-galaxy-format-3.md); the save's format 3, as format 2 had gone to the history): the career keeps the skeleton (`data::skeleton`), charted clusters (on arrival), signals (fresh → narrowed → located: halved towards the place), logs from data cores (one cargo unit, lost with the haul), gates, `Unlock::JumpRange` with Jump calibration I and II (2.5 → 3.25 → 4 S), jumps from home or any charted cluster, cluster voyages (`cluster_launched`/`cluster_ended`) and their `Leads` for `roll_interior`; slice saves migrate at load (voyages pinned in a chain from home, flown ones charted, the open one a located signal) | E1 | M |
| E5 Map | Unknown space, charted clusters, signal areas, range circles, blind-jump picking; the panel says what a signal knows. **Done 16 Oct 2026** ([GDR-0097](../gdr/0097-galaxy-map-and-voyage-flow.md)): the map on the career's skeleton (120 px a S; the voyages' content `map` place gone), charted clusters, gates, located signals, signal areas (dotted gold edge, faint fill, a pulse; the old edge fades when one narrows) worded from the content, the jump range as soft teal circles; a click on empty space picks a blind jump (the nearest cluster within 1.5 spacings); the galaxy's backdrop a tenth as heavy (the 2560 × 1440 timeout) | E2, E4 | M |
| E6 Voyage flow | Launch into a generated cluster, scanning in flight (control, bearing lines, the cluster overlay), finding, the debrief's finds, decoding logs at home. **Done 16 Oct 2026** ([GDR-0097](../gdr/0097-galaxy-map-and-voyage-flow.md)): the warp on the map (3 s + 2 s per S, Esc ends it), recorded launches (`cluster_launched`), scans narrow signals once a voyage and hear beacons, signals' sites found and done, the finds kept on a lost voyage, the debrief's lines, decoding at the LOG, the cluster map in flight (N) | E3, E4, E5 | L |
| E7 Slice into the galaxy | The four voyages pinned as set pieces; the Warden's gate as the rim's first gate trail. **Done 17 Oct 2026** ([GDR-0098](../gdr/0098-slice-in-the-galaxy-and-rim-rewards.md)): `galaxy.ron`'s set pieces at Niklas's distances, home with 4 free clusters in range, the Warden on a rim gate whose trail is the slice, its clearing opens the outer reach; the slice's goal an ordinary site (E3's stopgap gone); save format 4 (a format 3 save keeps its pins, its Warden's cluster becomes a gate); signal areas 3–4 S narrowing to 60 % (2–3 voyages to find one, `content-tool galaxy --seeds`) | E2, E6 | S |
| E8 Content | Rim trails: one gate trail beyond the Warden's, two caches, three lore fragments, two graveyards. **Done 17 Oct 2026** (GDR-0098): two gate trails beyond the Warden's (three gates a band) to a gate sentinel, two caches (a sealed cache; data and programmable matter into store), three lore fragments (memory wrecks; the memory log at the LOG), two graveyards (a deep graveyard); rewards on a trail's last signal done, banners in flight, debrief lines | E1, E6 | M |

Order: E0, E1 first; then E2, E3, E4 side by side; then E5; then E6; E7 and E8 last.

## 8. Risks

1. **Searching can drag.** Scans that tell too little, or areas too wide, turn exploring into chores. Tune with the
   headless runner first (how many scans and voyages a trail takes), then in play.
2. **Bigger clusters cost.** 2–3 km clusters need the streaming the slice never needed; measure (E0) before E3 is
   final. Measured 16 Oct 2026: they don't, at the sizes planned (see E0 in section 7).
3. **Getting stuck.** A target out of every jump range with no research left to grow it; the generator checks
   every gate trail is reachable, and the map says why a signal is out of reach.
4. **The slice changes under the play-test.** The 7L play-test plays the slice; the generated galaxy should land
   after it, not during it.
5. **Content volume.** Bands past the rim need their races' mechanics; until then only the rim is real.

## 9. Questions for Niklas

Answered on 14 Oct 2026 ([GDR-0080](../gdr/0080-exploration-searching-and-story.md),
[ADR-0035](../adr/0035-saves-keep-the-galaxy-skeleton.md)): clusters of 2–3 km; scanning needs a **scanner part**
(Niklas's choice over a plain key); a scan wakes sleeping enemies nearby; about two steps locate a signal; decoding a
log is free; a blind jump finds an ordinary cluster, sometimes a trail's start; saves keep the skeleton; the story is
lore fragments in a memory log plus milestone cards.

Answered on 16 Oct 2026 ([GDR-0093](../gdr/0093-galaxy-exploration-decisions-16-oct.md)): the scanner part's numbers
and the first one on the starter skiff (its hold 8 → 4); the jump range (2.5 → 4 S, two propulsion nodes, from home or
any charted cluster); how a signal is drawn (a dotted, pulsing area) and worded ("SIGNAL · SOMETHING OLD" over its
source line; NARROWED, LOCATED); a SCAN key and a behavior action; data cores take a hold slot.
