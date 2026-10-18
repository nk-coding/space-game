# GDR-0097: The galaxy map and the voyage flow: the map on the career's skeleton, signals as gold areas, soft range circles, blind jumps to the nearest cluster, the warp on the map, finds kept on a lost voyage, decoding at the LOG, a cluster map in flight

- Status: Accepted (provisional, lead). 2026-10-17: its consequences on a thin rim by home and on signals located
  by one scan addressed (4 free clusters within range of home; areas 3–4 S narrowing to 60 %); the cut labels at a
  wide window's left edge, the jump range's outline (one union, its arcs meeting at the crossings) and its flicker
  while zooming fixed; the Warden's voyage says `GATE · OPEN` once cleared (tracks E7, E8, [GDR-0098](0098-slice-in-the-galaxy-and-rim-rewards.md)).
  2026-10-17: Accepted (Niklas, 17 Oct 2026). The lost-voyage rule is kept: finds, charts, signals and trail rewards survive a lost
  voyage; only the haul is lost, data cores included ([GDR-0029](0029-mild-death-bonded-matter.md), [GDR-0096](0096-career-galaxy-format-3.md)).
  2026-10-17: the cluster map shares its painter with the minimap and so also shows rocks, sensor rings and contacts within the sensors ([GDR-0106](0106-flight-minimap.md)).
  2026-10-18 (Niklas: the gold pulses stuttered at well over 120 fps): the signals' pulses, the areas' waves, a
  narrowed edge's fade and the warp's light ran in steps (15–30 a second) so the meshes were rebuilt only per step;
  their phases are continuous now (real time), the meshes rebuilt every frame while one runs.
- Date: 2026-10-16
- Decided by: lead session (defaults for Niklas to review, galaxy tracks E5 and E6)
- Related: [GDR-0078](0078-galaxy-exploration-v1.md), [GDR-0080](0080-exploration-searching-and-story.md), [GDR-0091](0091-log-building-and-voyage-history.md), [GDR-0093](0093-galaxy-exploration-decisions-16-oct.md), [GDR-0095](0095-sensors-hidden-sites-and-scans.md), [GDR-0096](0096-career-galaxy-format-3.md), [GDR-0065](0065-phase-7-screens-and-escort-action.md), `docs/plans/galaxy-voyages.md` (E5, E6, 4.1–4.5), `docs/client.md` "Voyage map", "Voyages", "LOG"

## Context

E4 put the galaxy into the career (charted clusters, signals, logs, gates, the jump range, cluster voyages in the
model); nothing in the game used them yet. The voyage map still placed the slice's four voyages by the content's `map`
positions (0 to 1) in front of a placeholder galaxy. Niklas decided how a signal looks and reads and that jumps start
from home or any charted cluster, warping 3 s + 2 s per S (GDR-0093). Open were: how the skeleton lands on the map,
how unknown space is picked, what a scan in flight does to the career's signals, when finds count, where logs are
decoded, how a 2–3 km cluster is seen in flight, and why the map's screenshot at 2560 × 1440 timed out.

## Options considered

- **The map's coordinates**: the skeleton's, turned so home sits left of the galaxy's centre (chosen: one space for
  voyages, clusters, areas and range circles; the content's `map` place goes); the content's places for the voyages
  and the skeleton for the rest (two spaces that disagree: a range circle would not hold what it reaches).
- **Uncharted clusters**: not drawn (chosen, the plan's 3: most of the map is dark); drawn faintly (every blind jump's
  target known in advance).
- **A blind jump's target**: the cluster nearest the clicked spot that a jump reaches (chosen, the plan's 4.4), within
  1.5 spacings (farther is empty space, refused); the spot snapped to a cluster (gives positions away).
- **Narrowing from a scan**: once per signal per voyage, the first scan in a cluster whose area holds it (chosen: one
  cluster tells one thing; else five scans in one place would locate anything); per scan.
- **When finds count**: at the voyage's end, recalled or lost (chosen: knowledge is no haul, like charting, GDR-0096);
  only when recalled.
- **The warp**: on the map, a light running from the jump's start to its cluster with a countdown, Esc ends it before
  anything launches (chosen: nothing recorded until the warp is done); in flight before the arrival.
- **Decoding logs**: at the LOG (chosen: the records mast GDR-0091 built, glowing while a core waits); the research
  screen (the plan's first idea, a screen about spending).
- **A whole cluster in flight**: a cluster map laid over the view on a key (chosen); a camera zoomed out to 0.15 px/m
  (ships vanish, the world's drawings are sized for the flight view).

## Decision

*(All numbers placeholders.)*

- **The map** (E5): the skeleton at **120 px a S** at the default view (the starting jump range spans the map's
  height), turned so the galaxy's centre lies right of home, flipped so the slice's voyages lie above home, home's height
  chosen so home, the voyages and the charted clusters sit in the map's middle. The bands are the content's
  (`bands.ron` edges). A slice voyage sits on its pinned cluster.
- **What it shows**: home; the known voyages as before; charted clusters (a small teal ring, `CHARTED`), opened gates
  (coral), clusters a located or found signal leads to (gold, `SIGNAL · A GATE · LOCATED`); signals fresh or narrowed as
  **areas**: a dotted gold edge like the band edges, a faint gold fill, a ring running from the middle out to the edge
  every 3.2 s; when an area changes (narrowed, located) its old edge fades out over 1.6 s while the new one draws. The
  label, placed above the area (else below, right, left or inside, clear of the other labels), reads the trail's hint
  over the source's line (`SIGNAL · A GATE` over "a log from a hauler's wreck") and `NARROWED`. The **jump range** is
  the union of soft teal circles round home and every charted cluster: its edge only, a thin line over a glow fading
  inwards, no dots and no fill, so it never reads as a signal. The legend says the range (`JUMP RANGE · 2.5 S`).
- **Picking**: a click on a known place selects it; a click on empty space picks a **blind jump** to the cluster
  nearest the spot within 1.5 spacings that a jump reaches (not home, not a slice voyage's: they have their own
  signals); next to a known cluster it selects that cluster; a signal's label picks a blind jump to its middle. The panel
  says where the jump starts from and how far, the warp (`WARP ≈ 5 S`), the signals whose area holds the spot; empty
  space, a spot out of range and a cluster past an unopened gate (jumps reach only the bands a gate opened) are refused
  with their reason.
- **The warp** (E6): SET COURSE on a cluster, a spot or a signal runs the warp on the map (a light along the course,
  rings closing on the target, `WARP · 4 S · ESC ENDS IT`), 3 s + 2 s per S from where the jump starts (never from the
  target itself); then the launch is recorded (`Career::cluster_launched`, saved) and the fleet arrives. Slice voyages
  launch at once, as before.
- **In flight** (a recorded cluster voyage): a player's scan narrows every heard signal not located whose area holds
  the cluster (or that leads there: located), once per signal per voyage; a scan whose sweep reaches a beacon's place
  hears its signal at once; a signal's site seen is **found** (a new state between located and done), cleared or seen
  with nothing to clear it is **done**. Each has a gold banner (`SIGNAL NARROWED · A GATE`, `SIGNAL HEARD · …` over the
  source's line, `SIGNAL FOUND · …`). The finds go to the career at the end, recalled or lost.
- **The cluster map**: N (the `cluster_map` action) lays the cluster over the view's middle: its edge, the entry, what
  the voyage's scans swept, the bearings to the edge, the sites found, the fleet; the site markers at the screen's edge
  step aside meanwhile. The flight goes on.
- **The debrief** lists the cluster charted (on its first launch), each signal heard or come further (`SIGNAL HEARD /
  NARROWED / LOCATED / FOUND / FOLLOWED · A GATE`, gold) and `1 LOG TO DECODE · AT THE LOG`; VOYAGE MAP shows after
  anything that changed the map.
- **Decoding** at the LOG: its list starts with the data cores waiting (the source's name and summary, DECODE); a
  decoded one shows its signal's words and `ON THE MAP` until the LOG closes. The mast glows and home's label says `1 LOG
  TO DECODE` while one waits.
- **The 2560 × 1440 map**: the galaxy's backdrop mesh was 1,200 large soft glows (haze and fog) over a disc of 1,414
  px radius, about 12 times its disc's area blended per frame (about 73 million px² at the default view, three times
  that at the UI scale of a 2560 px window): SwiftShader could not draw a frame in 30 s, and a real GPU pays it every
  frame. Now 160 haze and 160 fog glows, larger in alpha, on the 660 px disc: under 5 times its area, about 6 million
  px², a tenth (a test keeps it under 6). The map at 2560 × 1440 screenshots within the usual wait.

## Consequences

- The content's voyages lose their `map` place; the rules' fingerprint (which reads the voyages) changes, so saved lab
  runs restore stale (GDR-0092).
- Niklas's career keeps its galaxy; its map is the skeleton's from now on, the slice's voyages on the clusters E4
  pinned them to. In a galaxy whose rim is thin by home, the free clusters near the slice are few: blind jumps there
  often say EMPTY SPACE or BEYOND THE GATE; a fresh area usually holds 2–5 clusters, so one scan in another cluster
  often locates it outright (E4's halving rule).
- E7 pins the slice by content and makes the Warden's gate the rim's first gate trail; E8 gives trails their rewards
  (a done signal pays nothing yet).
