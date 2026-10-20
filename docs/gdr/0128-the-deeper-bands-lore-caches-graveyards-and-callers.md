# GDR-0128: The deeper bands' lore, caches and graveyards, the Swarm's scout and the Grown's sentinel as callers, and a mining sample ship

- Status: Accepted (provisional, lead), 20 Oct 2026, for Niklas to review: texts, counts and rewards are lead defaults.
- Date: 2026-10-20
- Decided by: lead session (the content tasks of 20 Oct 2026; every text and number a default)
- Related: [GDR-0080](0080-exploration-searching-and-story.md) (lore trails, the memory log), [GDR-0098](0098-slice-in-the-galaxy-and-rim-rewards.md) (the rim's caches, memories and graveyards), [GDR-0002](0002-awakened-ship-mind-light-story.md) (the story stays light), [GDR-0027](0027-races.md), [GDR-0099](0099-the-grown-and-the-outer-reach.md), [GDR-0117](0117-drones-and-the-swarm.md), [GDR-0126](0126-the-lattice-prism-relays-and-refractive-crystal.md), [GDR-0127](0127-the-phase-walkers-blink-drives-and-piercing-lances.md) (one race per inner cluster, `ends_in`), [GDR-0111](0111-an-enemy-scan-calls-its-factions-sites.md) and [GDR-0113](0113-a-call-brings-reinforcements.md) (callers), [GDR-0114](0114-mining-and-ore-refining.md) (its open "mining fit as a content sample ship"), [GDR-0100](0100-fight-balance-targets.md), `docs/results/deeper-bands-2026-10-20.md`

## Context

Only the rim had lore (three fragments), caches and graveyards; the outer, middle and inner bands had their gates and
bosses only. Only the automaton picket called (GDR-0113: "other callers need only content"). GDR-0114 left a mining
fit as a content sample ship open. The deeper bands are small: over 200 seeds the outer band has 13–23 clusters, the
middle 8–17, the inner 6–12, three of each band's being gates, and no two trail rewards share a cluster. The inner
band's three bosses (two resonators, a colossus) leave it 0–6 clusters; a lore trail with no cluster left made the
whole galaxy fail.

## Options considered

- **When a small band runs out of clusters**: any trail but a gate's is left out of that galaxy (chosen: already the
  rule for `ends_in` trails, GDR-0127; no galaxy can fail); let rewards share a cluster (changes GDR-0098's rule and
  the map's one-reward-per-cluster); fewer bosses in the inner band (not this record's to change).
- **What a small band keeps**: gates, then bosses, then lore, then the extra caches and graveyards (chosen: a band
  trail's `spare: true` places it after the band's lore); lore last (the inner caches crowd out the fragments: 67 %
  for a third); lore before the bosses (the colossus or a resonator would go instead).
- **The inner band's fragments**: two (chosen: the second in 88 % of galaxies, the spares 22–56 %); three (the third
  in 67 %, the spares 7–26 %).
- **Callers**: the Swarm's scout and the Grown's sentinel flying the picket's own mind (chosen); a station's alarm (a
  picket docked at an outpost; left for later, one caller per race first).
- **The mining sample in the arena report's pool**: no (chosen: the pool is ships that fly one against one under
  their own behaviors, and `miner` fights only its leader's lock); yes.

## Decision

- **Lore** (`lore/outer.ron`, `middle.ron`, `inner.ron`): three, three and two fragments in the rim's voice, the
  precursors' records and the ship-mind's own half-memories, never why: *Older than the yard*, *Facing inwards*, *A
  hollow core*; *The second chart*, *Not the cargo*, *Only loud*; *Forty names*, *Logged twice*. Each at the end of a
  `memory` trail of its band, as the rim's.
- **Caches** (`Cache` trails, length 2–3, into store; encounters kept by the band's race, wrecks of precursor hulls):
  the **overgrown cache** (outer: a grove guardian and a rammer, wary; 10 logic, 6 propulsion, 8 Grown data, 5
  programmable matter, 3 spores), the **resin-sealed cache** (middle: 3–4 spitters; 12 logic, 8 shields, 10 Swarm
  data, 6 programmable matter, 4 resin), the **crystal-grown cache** (inner, Lattice clusters only: a guard and a
  needle; 14 logic, 10 energy, 12 Lattice data, 8 programmable matter, 6 crystal). The rim's gives 16 data and 4 matter.
- **Graveyards** (`Salvage` trails, length 1–2, rich salvage only): the **overgrown fleet** (outer: precursor hulls and
  Grown dead, a rammer and a thornback), the **picked graveyard** (middle: precursor hulls, automatons and spitters, 2–3
  spitters picking), the **crystal wreck field** (inner, Lattice clusters: hulls held in crystal, two needles) and the
  **rift graveyard** (inner, phase rifts: walker wrecks and cut hulls, 1–2 skirmishers).
- **Counts**: one cache and one graveyard per band, all `spare`; the inner band's three (cache, rift graveyard, wreck
  field) `ends_in` their race's cluster types. **Generator**: per band gates, band trails, lore, spare trails; a trail
  without a free cluster for its end is left out (was: generation failed). Old content generates the same galaxies.
- **Callers**: `watched_grove` (the unarmed 3×3 **Grown sentinel**, a rammer and a thornback; 0–1 per grove reach)
  calls `grove_call`, one rammer; `scout_patrol` (the unarmed 3×3 **Swarm scout** and 2–3 spitters; 0–1 per swarm
  reach) calls `swarm_call`, one spitter. Both scouts fly `picket`. The call's banner names the caller's encounter
  (`PICKET POST CALLED · REINFORCEMENTS INBOUND`, `SCOUT PATROL CALLED …`).
- **`sample_miner`**: `sample_early`'s hull with two mining lasers in the nose, its pulse lasers on the shoulders, a
  cargo hold for its rear armour and armour for its shields (20 MW for exactly its draw; 28 units of hold), flying
  `miner`; tagged `sample`, not in the arena report's pool.

## Consequences

- Measured (`docs/results/deeper-bands-2026-10-20.md`): each cache is about as hard as its band's ordinary sites and
  easier than its boss or gate (outer 61–73 % after a grove, middle 84–89 % after a patrol, inner 41–48 % after a lance
  watch), each graveyard the easier rich salvage the rim's is (74–100 %). A call costs 8–12 points (the grove) and
  12–15 (the scout patrol) against the scout that never scans, inside GDR-0113's 5–15 (the first cuts, a rammer and a
  thornback or 2–3 spitters, cost 22–28).
- Placement over 500 seeds: every outer and middle trail in every galaxy (the middle's spares 99 %); in the inner band
  the fragments 99 % and 88 %, the crystal cache 56 %, the rift graveyard and the wreck field 22–23 % each.
- Every new galaxy hashes differently (seed 1: `af364f2879722a2d`, was `256b011b9688cc79`); saves keep theirs. The
  rules' fingerprint changed (new ships and encounters): saved lab runs restore stale.
- The sample miner flown as the flagship mines its hold full (median 24 items) and loses most fights (9 % cleared);
  the voyage runner's escorts fly `escort` (GDR-0123), so it does not measure the miner as an escort.
- Open: a station's alarm as a caller; whether the inner band's left-out fragment should go to another band instead.
