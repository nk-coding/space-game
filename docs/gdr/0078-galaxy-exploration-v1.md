# GDR-0078: Galaxy exploration v1: signals that narrow as you search, blind jumps, jump range, hidden gates

- Status: Accepted. 2026-10-16: jumps start from home or any charted cluster, not only from home and opened gates
  ([GDR-0093](0093-galaxy-exploration-decisions-16-oct.md), which also sets the jump range: 2.5 S growing to 4 S).
  2026-10-16: signals, their narrowing, logs, gates, charting and the jump range are in the career (track E4, save
  format 3, [GDR-0096](0096-career-galaxy-format-3.md)); the slice's voyages pinned into the generated galaxy until E7.
  2026-10-16: the map draws charted clusters, signal areas and the jump range, blind jumps go to the cluster nearest
  the spot, scans narrow signals in flight (tracks E5, E6, [GDR-0097](0097-galaxy-map-and-voyage-flow.md)).
- Date: 2026-10-14
- Decided by: Niklas (eight questions, interactively, after GDR-0075)
- Supersedes: GDR-0026 and GDR-0028 in part (travel along lanes; see Decision)
- Related: [GDR-0075](0075-voyages-feel-like-exploration.md), [GDR-0026](0026-procedural-galaxy-and-bands.md), [GDR-0028](0028-clusters-and-voyages.md), [GDR-0062](0062-first-playable-loop.md), [GDR-0063](0063-salvage-and-research-v1.md), `docs/plans/galaxy-voyages.md`

## Context

GDR-0075 set the direction: voyages should feel like exploring a galaxy and following hints, not a fixed path. The
galaxy plan (`docs/plans/galaxy-voyages.md`, 13 Oct) still laid out lanes, routes and visible gates, and the slice's
voyage map drew its four voyages as a dotted chain. Before the plan could be reworked, the shape of exploration
needed Niklas's answers.

## Options considered

Each question had a recommended option; Niklas chose it in every case, and picked hint sources and rewards from
several.

- **Finding places**: signals plus blind jumps (chosen); signals only (every place from a hint, very guided); fog
  of war only (hints as flavour, nothing pulls you anywhere).
- **Hint precision**: narrows as you search (chosen); the exact place on the map (little searching); words only
  (puzzle-like, easy to get stuck).
- **Travel**: jump range from home and opened gates, growing by research (chosen); lanes found as you go
  (GDR-0026 as written); anywhere you know, distance only adding warp time (distance never limits).
- **Band gates**: hidden and found by hint trails (chosen); visible from the start; one visible per boundary and
  the others hidden.
- **Searching inside a cluster**: yes, scans reveal sites (chosen); no, every site known on arrival.
- **The slice's four voyages**: keep them, but as hints instead of a drawn chain (chosen); leave them until the
  rework; replace them now with a small open rim.

## Decision

- **Discovery**: things found on voyages put a **signal** on the galaxy map, pointing at somewhere worth a trip.
  The player may also **jump blind** into any unknown neighbouring space for ordinary finds. Hints lead to the
  interesting places; curiosity is always allowed.
- **Signals narrow as you search**: a fresh signal marks a wide area on the map; inside a cluster, scans give a
  bearing; the last stretch is found by flying. Searching is part of the voyage.
- **Hint sources** (v1): derelicts and wrecks (logs and data cores in salvage, decoded at home in research) and
  signals picked up in clusters (beacons, distress calls, odd readings that you scan). Bosses' wrecks and the
  ship-mind's own memories were not chosen for v1.
- **Hint trails lead to**: band gates, precursor caches (research nodes or rare parts found nowhere else), lore
  and story, and rich salvage.
- **Travel by jump range**: from home or an opened gate the player launches to anything known within the drive's
  range; research grows the range. No lanes are drawn on the map. This replaces GDR-0026's "clusters linked by
  lanes" as the way of travel; the rest of GDR-0026 (bands, guardians at band gates, races, set pieces, a new seed
  per game) stands.
- **Gates are hidden**: the Warden's gate stays known (the slice); later gates are found by a trail of several
  signals, and any one gate of a boundary opens the next band.
- **Clusters are searched**: on arrival only some sites are known; sensor sweeps reveal the rest and give a
  signal's bearing. This needs bigger clusters than the slice's 600–1000 m.
- **The slice now**: the four voyages stay as the first steps; the map no longer draws a route between them,
  voyages not yet opened stay unknown, and clearing a voyage's goal reveals the next as a new signal.

## Consequences

- `docs/plans/galaxy-voyages.md` is reworked around this (lanes, routes and visible gates go; signals, scans,
  jump range and blind jumps come in); its remaining open questions go to Niklas interactively.
- New systems: signals as save state (a source, an area that narrows, what it leads to), scanning inside a
  cluster, the jump range and its research nodes, a blind-jump destination generator, hint content (log
  fragments, beacons) in data.
- Salvage and research gain a second purpose: decoding logs and data cores into signals.
- Open: cluster size for searching, how a scan works (an action, a part, a timer), how wide a fresh signal is and
  how fast it narrows, what a blind jump finds, the jump range numbers.
