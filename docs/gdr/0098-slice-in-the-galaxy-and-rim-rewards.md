# GDR-0098: The slice in the galaxy and the rim's rewards: set pieces by content, the Warden's gate the rim's first gate, wider signal areas that narrow to 60 %, four free clusters by home, caches, memories and graveyards

- Status: Accepted (provisional, lead).
  2026-10-17: Accepted (Niklas, 17 Oct 2026): the galaxy's shape and pace are kept (the set pieces' distances, areas 3–4 S narrowing
  to 60 %, 2–3 voyages to find a signal); a trail's reward still counts on a lost voyage ([GDR-0097](0097-galaxy-map-and-voyage-flow.md)).
- Date: 2026-10-17
- Decided by: lead session (defaults for Niklas to review, galaxy tracks E7 and E8, and two tunings of E4)
- Related: [GDR-0002](0002-awakened-ship-mind-light-story.md), [GDR-0026](0026-procedural-galaxy-and-bands.md), [GDR-0029](0029-mild-death-bonded-matter.md), [GDR-0075](0075-voyages-feel-like-exploration.md), [GDR-0078](0078-galaxy-exploration-v1.md), [GDR-0080](0080-exploration-searching-and-story.md), [GDR-0093](0093-galaxy-exploration-decisions-16-oct.md), [GDR-0094](0094-galaxy-generator-defaults.md), [GDR-0095](0095-sensors-hidden-sites-and-scans.md), [GDR-0096](0096-career-galaxy-format-3.md), [GDR-0097](0097-galaxy-map-and-voyage-flow.md), [GDR-0092](0092-evolution-lab-for-your-own-ships.md), `docs/plans/galaxy-voyages.md` (E7, E8, 4.1, 4.3, 4.7)

## Context

E4 pinned the slice's four voyages to rim clusters provisionally, in a chain from home, each taking the free cluster
nearest the one before; the Warden opened nothing; the slice's goal counted as a signal's site (E3's stopgap); trails
ended at the slice's encounters and paid nothing. Two things showed in E5 and E6 (GDR-0097's consequences): the
chain took the clusters nearest home, so many spots in the starting range said EMPTY SPACE (over 200 seeds of the
same content: home had 1 to 4 free clusters within 2.5 S, median 2, 3 or more in only 30 %), and a fresh area held
about 3 clusters, so one scan from another cluster located it outright in 83 % of searches (`content-tool galaxy
--seeds`, a player who jumps into the area at random and scans: 1.6 voyages to find a signal).

## Options considered

- **Where the slice goes**: set pieces in `galaxy.ron`, placed by the generator at Niklas's distances from home
  (chosen: content decides, the skeleton keeps them, the generator can promise free clusters around them); keep the
  career's chain (no content, takes home's neighbours); the plan's "the Shipyard approach is home's own cluster" (the
  map draws the shipyard there; Niklas's notes put the approach 0.9 S out, and that is kept).
- **The Warden's gate**: the generator puts the Warden on one of the rim's three gates (the one nearest the band's
  inner edge 2.8–3.5 S from home) and the slice's voyages are that gate's trail (chosen: clearing it opens the outer
  reach, the plan's 4.7); a fourth gate beside the three; a generated trail to the Warden as well (two ways to the
  same guardian).
- **The slice's signals**: they stay as they are, located when their voyage opens (chosen: the cleared texts name
  the next voyage; a search for Halvex Drift would hide the first loop); the plan's "Halvex and Corran inside the
  first signals' areas" (a search before the first real voyage).
- **Signals locate too fast**: wider fresh areas, 3–4 S across (was 2–3), and gentler narrowing, the radius to 60 %
  per scan (chosen, over 8000 seeds: 15 % located by the first scan, 2 or 3 voyages 66 % of the time, 1.98 on average, a lucky first
  jump into the place 32 %); 3.5–4.5 S and 60 % (fewer lucky hits, areas wider than the rim is deep); scans from
  two different clusters to narrow (one more rule to explain, and E6's once-a-voyage already keeps one cluster from
  telling more than one thing).
- **Home's free clusters**: the generator re-rolls home until 4 rim clusters without a set piece lie within the
  starting jump range (chosen: 4 in every one of 8000 seeds, with 64 spots tried for home and 32 placements, none
  failing; 5 failed most seeds); 3 (holds with the old tries, but only just more than the chain left).
- **Rewards** (the plan's 4.3): a gate opens when its guardian is beaten (a new encounter, the gate sentinel, for the
  rim's generated gates; the Warden for the slice's); a cache's trail `gives` research data and programmable matter
  straight into store, beside its sealed cache's rich wrecks (chosen: data on the trail template, no new mechanics); a
  research node or a part found nowhere else (needs nodes and parts that exist only for caches, later); a lore trail's
  fragment joins the memory log, shown at the LOG (chosen; GDR-0080's milestone cards wait); a graveyard is its own
  rich encounter, paid by salvage.
- **Old saves**: format 4 adds two fields with defaults; a format 3 save keeps E4's pins, and at load its Warden's
  cluster becomes a gate (chosen: a few lines in `reconcile`, tested on the format 2 fixture written as E4 would);
  regenerating the skeleton (ADR-0035 forbids it).

## Decision

*(All numbers placeholders, in content: `galaxy.ron`, `trails.ron`, `encounters/automatons.ron`, `lore/rim.ron`.)*

- **Set pieces** (`galaxy.ron`'s `set_pieces`, generator version 2): the Shipyard approach 0.8–1.3 S from home,
  Halvex Drift 1.7–2.3, Corran Reach 2.0–2.5, the Warden's gate 2.8–3.5 and a gate (Niklas's 0.9, 2.1, 2.3 and 3.2 S,
  GDR-0093: Halvex and Corran in the starting range, the Warden mostly with Jump calibration I). A gate's piece takes
  the band's cluster nearest its inner edge in its distance; the others the cluster nearest the way from the piece
  they open after to the gate's, a trail step (0.9–2.5 S) from it, and the gate's piece a trail step from Corran Reach
  (a first try without the step left a quarter of the slice's steps beyond 2.5 S, up to 5.75; now 0.9–2.5, median
  1.05, as the slice's own 0.9–1.3). Home is tried again until they all fit. The clusters take the voyages' names; no
  trail place lies on one; the band's gates turn so the Warden's is the first. The career's `pins` come from the
  skeleton (`skeleton.pieces`).
- **The Warden's gate is the rim's first gate trail**: clearing its goal opens the gate (`Career::sync_slice`), so
  jumps reach the outer reach. The map's Warden says `GATE · OPEN`, the debrief `GATE OPEN · THE OUTER REACH`
  (coral), and its cleared text says the gate wakes. The other two rim gates have generated trails to a **gate
  sentinel** (the Warden's ship with a hunter).
- **E3's stopgap goes**: a slice voyage's goal is an ordinary site (sensors show it at 800 m); its voyage is the
  signal.
- **Home**: at least 4 free rim clusters (no set piece) within the starting 2.5 S.
- **Signals**: fresh areas 3–4 S across (the place up to 60 % of the radius off the centre); a scan from another
  cluster keeps 60 % of the radius (`signal_area.narrow`), the place within 60 % of the new radius.
- **Rewards**: the last signal of a trail done gives the trail's reward once, also on a lost voyage (a find, like
  charting): a gate opens; a fragment joins the memory log (`Exploration::memories`, the LOG's MEMORY rows); a
  cache gives 8 logic and 8 propulsion data and 4 programmable matter. Encounters: **sealed cache** (a hunter and 1–2
  raiders round 2–3 precursor hulls), **memory wreck** (one precursor hull, unguarded: seen is done), **deep
  graveyard** (2–3 raiders over 7–10 raider and 1–2 hunter wrecks), **gate sentinel**. In flight a reward's banner
  (`CACHE OPENED` over what went into store, `A MEMORY · THE LAST LAUNCH`, `GATE OPEN · …`); the debrief lists it.
- **Lore**: the rim's three fragments are the rim's lore for now (the plan's three), final for the slice; light, as
  GDR-0002 asks.
- **Save format 4**: `skeleton.pieces`, `galaxy.memories`. A format 3 save keeps its pins; its Warden's cluster
  becomes a gate at load (a note), and its clearing opens it.

## Consequences

- Every new galaxy hashes differently (seed 1: `8a509f91bd8fd0b8`, was `e2055530fd51f960`); saves keep theirs.
- Niklas's career keeps its galaxy and its pins; at its next load the Warden's cluster is a gate, opened once the
  Warden is cleared (at load, if it is already); its skeleton keeps its 2–3 S areas (only new galaxies get the wider
  ones), and every signal narrows to 60 % from now on. Its home's free neighbours stay what E4 left.
- The rules' fingerprint changed (four encounters, a voyage's text, the slice's goal no signal): saved lab runs
  restore stale (GDR-0092).
- The galaxy plan's tracks are done; caches that give a node or a part, milestone cards, and bands past the rim
  with their own guardians are next.
