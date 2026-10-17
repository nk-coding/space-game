# GDR-0096: The galaxy in the career: save format 3 keeps the skeleton, charting on arrival, signals that halve towards their place, data cores as one-slot logs, two jump calibrations, the slice pinned in a chain from home

- Status: Accepted (provisional, lead). 2026-10-16: found comes between located and done, jumps reach only bands a
  gate opened and never start at their target, logs are decoded at the LOG (tracks E5, E6, [GDR-0097](0097-galaxy-map-and-voyage-flow.md)).
  2026-10-17: the slice's voyages lie where the skeleton's set pieces put them (the chain stays only in format 3
  saves, whose Warden's cluster becomes a gate at load); a scan keeps 60 % of an area's radius instead of halving it;
  `Lead::Voyage` signals stay; save format 4 adds the set pieces and the memory log (tracks E7, E8, [GDR-0098](0098-slice-in-the-galaxy-and-rim-rewards.md)).
  2026-10-17: Accepted (Niklas, 17 Oct 2026); charting kept on a lost voyage and a data core lost with the haul confirmed with the
  lost-voyage rule ([GDR-0097](0097-galaxy-map-and-voyage-flow.md)).
- Date: 2026-10-16
- Decided by: lead session (defaults for Niklas to review, galaxy track E4)
- Related: [GDR-0078](0078-galaxy-exploration-v1.md), [GDR-0080](0080-exploration-searching-and-story.md), [GDR-0093](0093-galaxy-exploration-decisions-16-oct.md), [GDR-0094](0094-galaxy-generator-defaults.md), [GDR-0095](0095-sensors-hidden-sites-and-scans.md), [GDR-0029](0029-mild-death-bonded-matter.md), [ADR-0033](../adr/0033-career-model-save-and-voyage-director.md), [ADR-0035](../adr/0035-saves-keep-the-galaxy-skeleton.md), [ADR-0042](../adr/0042-galaxy-generator-crate.md), `docs/plans/galaxy-voyages.md` (E4, 4.1, 4.2, 4.5, 4.7), `docs/career.md`

## Context

Track E4 puts the galaxy into the career: what the player has charted, the signals heard and how far they have
narrowed, the logs to decode, the gates opened and the jump range (GDR-0093: 2.5 S growing to 4 S by two propulsion
nodes of 0.75 S, jumps from home or any charted cluster; a data core takes a hold slot and is lost on death). Niklas's
running career is a slice save (four hand-made voyages); it must keep working and move into the galaxy until E7 pins
the slice's voyages into it by content. The plan's numbers for narrowing were placeholders, and the starter skiff's
drydock read 29 / 28 power in coral because its scanner was counted as always charging.

## Options considered

- **Where the skeleton lives**: in the save (chosen, ADR-0035; its types moved to `data::skeleton` so the career can
  hold it, the generator re-exports them); a seed only, regenerated at load (ADR-0035 ruled it out).
- **When a cluster is charted**: on arrival, kept when the voyage is lost (chosen: the chart is knowledge, not haul);
  only on a voyage that comes home (a lost voyage would leave a hole in the map the player has seen).
- **The jump range**: derived from content and research like mind capacity (chosen: a rebalance reaches old saves);
  stored (a migration per change).
- **Narrowing**: the area halves and its centre moves towards the place until the place lies within 60 % of the new
  radius (chosen: the skeleton's own offset rule, so a narrowed area looks like a fresh one); a fixed shrink around the
  old centre (the place could fall out).
- **The slice's voyages** until E7: pinned in a chain (chosen: the first to the rim cluster nearest home, each later
  one to the free rim cluster nearest the voyage it opens after; like the slice, a step of about one spacing each,
  0.9–4 S from home); by distance from home alone (scattered round home); not at all (a migrated save would have an
  empty map).

## Decision

*(All numbers placeholders.)*

- **Save format 3** (the plan's "format 2"; format 2 was taken by the history, GDR-0091): the career's `galaxy` holds
  the skeleton (the content's first galaxy, generated once from the game seed: about 65 KB of RON), the charted clusters
  (home from the start; launches per cluster for its seeds), the signals heard, the logs to decode, the gates opened, the
  slice's pins and the cluster underway. A format 2 save reads as it is and gets its galaxy at load (charted from its
  voyages, then written at once); content without a galaxy leaves a career without one.
- **Charting**: launching into a cluster charts it (lost or not). Jumps start from home or any charted cluster and
  reach the jump range; the nearest start is the one a jump leaves from.
- **Jump range**: the galaxy's `jump_range` (2.5 S) plus every `JumpRange(s)` research unlock, a new unlock kind like
  `MindCapacity`: **Jump calibration I** (propulsion 8 data) and **II** (propulsion 16, 3 programmable matter, after I),
  0.75 S each (2.5 → 3.25 → 4 S); refinements in the propulsion lane of the research screen.
- **A cluster voyage's seed**: `splitmix64(game seed ^ splitmix64(cluster seed ^ launches))`, like a voyage's.
- **Signals**: heard fresh with the skeleton's area (a beacon scanned, a log decoded; hearing one again changes
  nothing). A scan in another cluster narrows it (above); once no other cluster lies inside, or the scan is taken in
  its own cluster, it is **located** (its area becomes the cluster). States Fresh, Narrowed, Located, Done; found (the
  site seen) comes with E6. A launch into a cluster makes a signal's site there only of the signals heard and not done
  (replacing E3's "every trail place is heard", GDR-0095).
- **Data cores**: a log's place (a source of form `Log`) holds a data core in the first wreck of its site (loose at the
  site without one) until the career has it: one cargo unit, worth 1000 a unit, so a hold takes it first and a
  shrinking hold drops it last. Brought home it is a **log** to decode (free, GDR-0080; the decoding UI is E6); lost with
  the haul (GDR-0029) it stays in its wreck and the next launch rolls it again.
- **Gates**: the career records the gates opened (a guardian beaten, E6/E7); the slice's Warden opens none yet.
- **The slice** (provisional until E7): every content voyage is pinned in a chain as above; a launched voyage charts its
  cluster, an open one not cleared is a located signal (`Lead::Voyage`), a cleared one's signal is done. Slice voyages
  keep their own records, seeds and the voyage map; the jump range does not gate them.
- **Practice runs** `?scene=voyage&cluster=<id|signal>` fly a cluster of the career's galaxy with what the career knows;
  with `&galaxy=<seed>`, another galaxy with every trail place heard and every data core in place.
- **The drydock's power** (16 Oct 2026, the starter's 29 / 28): a part's `power.intermittent` words ("while scanning",
  the scanner's) leave its draw out of full use; POWER shows it under the bar ("+3 WHILE SCANNING"), so the skiff reads
  26 / 28. A scanner short of power charges slower, so the line stays in the label's tone.

## Consequences

- Niklas's career keeps its voyages, research and history; it gains a galaxy whose first clusters are its voyages,
  the cleared and flown ones charted, the next open one a located signal.
- E5 draws charted clusters, signal areas and range circles from the career; E6 decodes logs, narrows signals from scan
  bearings, marks them found and done and launches recorded cluster voyages (`Career::cluster_launched`); E7 replaces the
  pins with content and makes the Warden's gate the rim's first gate trail (a migration of `Lead::Voyage` signals).
- The rules' fingerprint changed (the scanner's power block): saved lab runs restore stale (GDR-0092).
