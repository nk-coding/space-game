# GDR-0130: The core band: the precursor defences, radiation on every part open to space, and the Archive at the galaxy's centre with its warden mind

- Status: Accepted (provisional, lead), 20 Oct 2026, for Niklas to review: who lives in the core, the Archive and its
  warden, the lore canon and radiation as heat are Niklas's (20 Oct 2026); every layout, rule detail, text and number
  is a lead default.
  20 Oct 2026 (later, Niklas): the core's lore direction (the warden reads your name off your hull; every name on the
  list of forty crossed out but the warden's, "keeping", and yours, "expected, or late"; the codex's hint at what you
  were made to carry) and the end-game reward (the archive key pays for the codex: one more core at once, mind
  capacity up to 5, and the sentinel core) are confirmed.
- Date: 2026-10-20
- Decided by: Niklas (the four points under Decision, 20 Oct 2026); lead session (the rest, defaults for him to review)
- Related: [GDR-0026](0026-procedural-galaxy-and-bands.md) (the core: the precursor archive, the final gate, the best
  tech, rising radiation), [GDR-0027](0027-races.md) (the precursors are gone: gates, guardians, megastructures, the
  archive and you), [GDR-0112](0112-the-keeper-guards-the-middle-bands-gates.md) and
  [GDR-0116](0116-the-spire-guards-the-inner-bands-gates.md) (multi-section guardians on cores of their own; the inner
  gate gives two shards), [GDR-0110](0110-anchored-stations.md), [GDR-0080](0080-exploration-searching-and-story.md),
  [GDR-0098](0098-slice-in-the-galaxy-and-rim-rewards.md), [GDR-0128](0128-the-deeper-bands-lore-caches-graveyards-and-callers.md)
  (spare trails, the inner band's lore), [GDR-0100](0100-fight-balance-targets.md),
  [GDR-0102](0102-grown-mechanics-and-parts-defaults.md) and [GDR-0103](0103-the-grown-in-play-defaults.md) (stellar heat),
  [GDR-0125](0125-disruption-ion-static-blinds-minds.md), `docs/sim.md` "Radiation", `docs/content-model.md`,
  `docs/results/core-band-2026-10-20.md`

## Context

The core band was a stub: its one cluster type held the rim's automaton raiders, and the Spire's `inner_gate` trail
led into it with nothing to find. GDR-0026 promised the precursor archive, the best tech and rising radiation there;
the lore (GDR-0128) left a thread open: forty minds signed the gate order ("every guardian faces inwards ... until we
are sure"), and a list of forty names ends with the ship-mind's. Niklas decided on 20 Oct 2026 who lives in the core,
what its final site is, how radiation works and that the warden is one of the forty.

## Options considered

Niklas's points were decided before this record; the lead's choices:

- **Radiation**: the hot star's rule without the distance, heat through every side open to space, the same everywhere
  in the cluster and more the nearer the cluster lies to the galaxy's centre (chosen: one addition per ship a tick on
  the star's path, no new heat rule; armour skins soak it and radiators shed it); heat into every part, open or not
  (interior parts would cook regardless of radiators, and radiators would matter less); a dose that builds up over
  time (a new rule); radiation stronger toward the cluster's own centre (the cluster's position in the galaxy carries
  the band's gradient already).
- **How the warden's sections change its tactics**: its behavior reads `hull`, which already counts the parts lost
  with sections shot off (chosen: data only, the rest of the warden switches the moment a section's bridge breaks,
  heavy damage counts too); a new behavior word ("sections lost": a sim and vocabulary change in files another agent
  was editing).
- **Where the Archive lies**: a band's `centre` (chosen: its cluster nearest the galaxy's centre takes a cluster type
  of its own and holds the end of a trail of its own); a cluster type rolled only near the centre (it might roll
  twice or never); an `ends_in` trail (the end would not be the centre).
- **A small core band**: a band with a centre holds at least three clusters, else the generator tries again (chosen:
  the Archive's trail and the band's two fragments always fit; a quarter of the seeds lay out once more); trails that
  start in the inner band (breaks "a trail stays in its band"); a wider core band (shrinks the already small inner
  band).
- **The end goal's reward**: the archive key, which the Archive's codex takes for one more core at once and the
  precursors' own sentinel core (chosen: the ship-mind grows, which is what every mind expansion pays, and a piece of
  precursor tech to build with); two more cores (a big jump at once); lore only (nothing to show for the hardest fight).

## Decision

Niklas's (20 Oct 2026):

- **Who lives in the core: the precursor defences**, automated guardians stronger and more orderly than the rim's
  feral automatons (`factions/precursors.ron`), with a small roster of their own in the precursor look.
- **The final site: the Archive** at the galaxy's centre, at the end of a trail; its boss **the Archive's warden
  mind**, a huge precursor construct run by a surviving mind, one of the forty, still following the old gate order: a
  multi-section guardian whose sections change their tactics as it breaks, the hardest fight in the game.
- **Lore canon**: forty minds signed the gate order; the ship-mind's name is the last on a list of forty; the warden
  is one of the forty.
- **Radiation is heat on every part** in core clusters, stronger toward the centre, reusing the stellar heat; radiators
  and heat sinks become core gear; shown in the HUD the way star heat is.

The lead's defaults:

- **Radiation** (cluster type `radiation: (edge, centre)`, `galaxy::Interior::radiation`, `Voyage::radiation`; a
  content voyage may name one for test voyages): every part takes `radiation × open sides` heat a second, added to a hot
  star's; **0.25 on the core's outer edge, 0.35 at the galaxy's centre**, linear between. A ship without radiators
  overheats after about three minutes of fighting on the edge and within half a minute at the centre; the sample
  designs with four to six radiators cope. The HUD's subtitle says `RADIATION 0.28 A SIDE`.
- **The roster** (parts/precursors.ron, ships/precursor_*.ron, plated in **precursor alloy**): the **sentinel core**
  (their mind and power: four rules, control 12, power 10, heat limit 5, leaves programmable matter); the **sentry**
  (two long cannons, a shield, point defence, four radiators), the **aegis** (two beam lasers, two point defence
  turrets under a prism ward), the anchored **sentry station** (long cannons fore and aft, a torpedo launcher, a prism
  ward), a **megastructure fragment** spawned only as a wreck, and the **Archive's warden**. Their tactics:
  `sentry_patrol` (the weakest enemy, all together; off beyond reach while a bubble recharges; circle at 80 % of reach).
- **The Archive's warden** (`archive_warden`, 15 × 12): four sections on sentinel cores of their own joined by one
  bridge block each: the fore section (two torpedo launchers, three long cannons), the heart (two beam lasers, two prism
  wards, point defence, the fusion reactor, the torch drive and thrusters at its back), two wings (a wide shield, point
  defence, an ion cannon in front, a rocket pod and a long cannon facing back). Its rules (`behaviors/precursors.ron`):
  whole, keep 95 m off (120 m while a ward is down); **below 88 % hull** (the fore section shot off, both wings, or
  heavy damage) circle at 35 m, where the beams and the ion cannons bite; **below 55 %** ram. A section shot off is a
  vessel of its own, whole by its own count, and starts over at range.
- **Sites and clusters** (`encounters/precursors.ron`, `cluster_types/core.ron`): `sentry_patrol` (two or three
  sentries and an aegis), `sentry_post` (a station), `megastructure_fragment` (a fragment and sentry wrecks, one or two
  sentries), `sealed_vault` (one sentry), `archive_site` (the warden, three sentries and an aegis, wake radius 400 m);
  `sentry_reach` (weight 3) and `fragment_deep` (weight 2), and `archive_heart` at the band's centre. The core's sky:
  amber and ember nebulae, planets backlit.
- **Trails** (`trails.ron`): the band's centre trail **`archive`** (2–3 signals, a `Cache` whose target is
  `archive_site`): beaten, it gives the **archive key**, 30 logic, 20 hull and 12 shields data and 16 programmable
  matter; the spare `sealed_vault` (2–3 signals, 16 logic, 12 hull, 10 thermal data, 10 programmable matter). Two lore
  fragments (`lore/core.ron`): `the_order_kept` (a station's log: the order stands, signed by forty, and it never asked
  which of them is awake) and `two_names_left` (the list again, every name crossed out but two: one marked keeping, and
  yours, marked expected, or late). The warden's own text: one of the forty names is its own, and it knows yours.
- **Best tech, kept to two nodes** (`research/precursors.ron`): **precursor alloy** (hull; 40 hull and 8 logic data, a
  gate shard, 8 programmable matter; after reactive plating): armour of 2.3 hit points and 1.7 t a cell, build-point
  factor 2.2, a blast factor of 0.3, the best there is; and **the Archive's codex** (logic; 30 logic data, the archive
  key, 10 programmable matter; after mind expansion I and the automaton forge): **one more core at once** and the
  **sentinel core** for the player's own ships. One archive key in a galaxy.
- **The generator**: a band's `centre: (trail, cluster)`: its cluster nearest the galaxy's centre takes the cluster
  type (rolled as before, so other clusters keep theirs), its trail is placed first in the band and no other trail
  passes through it; a band with a centre holds at least three clusters or the attempt starts again.

## Consequences

- Tuning (`docs/results/core-band-2026-10-20.md`, the voyage runner, a Runner with three skiffs and no research): a
  core patrol or sentry post clears 46–63 % (about a walker strike); **the Archive 8 % (mixed), 19 % (guns and
  rockets), 14 % (lasers)**, below the Spire's gate (30 / 30 / 15 %): the hardest site in the game, cleared now and
  then by a strong late fleet, more often with research; radiation halves the mixed fleet's clears there (16 → 8 %).
- The generation hash moved (`af364f2879722a2d` → `7eb322ae41afb209`); saves keep their skeletons (ADR-0035), so a
  running game keeps its old core until a new game. No generator version bump: old content generates as before.
- The `stub` tag is gone from every band; `uncharted_deep` is deleted. Ships tagged `core` join the scenarios' deep
  tags. The `archive` determinism scenario (radiation, the warden shot apart bridge by bridge) and the arena fight
  `archive_warden` are in the determinism check.
- The mind's capacity tops out at 5 (the start's 2, two expansions, the codex). The Archive is the galaxy's end for
  now; what comes after it (new game+, what the keeping mind wants) stays open with the working title.
- Open: a radiation shield part or armour that blocks it; the Archive's interior as a place of its own (a set piece
  rather than a site); whether the forty's other surviving mind should appear elsewhere.
