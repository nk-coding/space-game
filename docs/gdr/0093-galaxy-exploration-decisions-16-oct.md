# GDR-0093: Galaxy exploration decisions of 16 Oct: the S unit, a light scanner on the starter, jump range 2.5 → 4 S from any charted cluster, how signals look and read, SCAN for pilots and behaviors, data cores in the hold

- Status: Accepted. 2026-10-16: the scanner, the starter's (hold 8 → 4), SCAN (G) and the `scan` behavior action built
  (track E3); the open questions around them (sensor range, hidden sites, bearings, the cluster's edge) as lead
  defaults in [GDR-0095](0095-sensors-hidden-sites-and-scans.md).
- Date: 2026-10-16
- Decided by: Niklas (the open questions of `docs/plans/galaxy-voyages.md` section 9, interactively)
- Supersedes: GDR-0078 in part (jumps start from home and opened gates only; now from home or any charted cluster)
- Related: [GDR-0078](0078-galaxy-exploration-v1.md), [GDR-0080](0080-exploration-searching-and-story.md), [GDR-0094](0094-galaxy-generator-defaults.md), [ADR-0042](../adr/0042-galaxy-generator-crate.md), `docs/plans/galaxy-voyages.md`

## Context

GDR-0078 and GDR-0080 decided how exploring works; the plan left open the scanner's numbers and how the first one is
obtained, the jump range and its research, and how a signal is drawn and worded. Before tracks E1 and E2 (the
galaxy's content and generator), the distance unit the galaxy is measured in was needed too.

## Decision

- **S, the spacing unit**: the galaxy map measures distances in S, about the distance between neighbouring voyages
  of the slice (about 200 px of the voyage map's stage, 0.28 of its content units). From home: the Shipyard approach
  lies 0.9 S, Halvex Drift 2.1 S, Corran Reach 2.3 S, the Warden's gate 3.2 S.
- **Scanner** (track E3): a light 1×1 sweep part, 3 BP, drawing 3 power only while it charges (2 s), scan range
  1.2 km (the plan's 4.8), 8 s cooldown: about five scans cover a 2–3 km cluster *(placeholders)*.
- **The first scanner**: the starter Rim skiff carries one in place of a cargo bay (its hold goes from 8 to 4).
- **Jump range** (E4): 2.5 S at the start, growing to 4 S through two propulsion research nodes of +0.75 S each (a
  new unlock kind, like `MindCapacity`). Halvex and Corran are in reach at the start, the first node reaches the
  Warden; the slice fits unchanged. Warp time 3 s + 2 s per S (the plan's 4.5).
- **Jumps start from home or any charted cluster**, not only from home and opened gates.
- **A signal's look** (E5/E6): a dotted edge (like the band edges), a faint fill, a slow pulse running out to the
  edge; a narrowed area redraws smaller while the old edge fades. It must read differently from the soft range
  circles.
- **A signal's words**: what it leads to over what found it, e.g. "SIGNAL · SOMETHING OLD" over "a log from a
  hauler's wreck"; its states read NARROWED and LOCATED.
- **Scanning**: a SCAN key for the flown ship (`controls.ron`) and a behavior action node, so escorts and automated
  ships can scan too.
- **A data core** takes one hold slot and is lost on death like the rest of the haul.

## Consequences

- E1 puts S into the content (`stats.ron`'s `galaxy_distance` and `jump_range`, unit S), the base jump range and
  warp time into `galaxy.ron`, the hint and source line into `trails.ron` and `signal_sources.ron`; the generator (E2)
  places clusters in S (GDR-0094).
- E3 builds the scanner part, the SCAN control and action node; E4 the two propulsion nodes and their unlock kind,
  the data core in the hold; E5/E6 the signal's look and words.
- Reachability in the generator assumes jumps from any charted cluster: every cluster of a band is reachable hop by
  hop within the starting range.
