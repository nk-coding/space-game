# GDR-0095: Sensors, hidden sites and scans in a cluster: cores see 800 m, signals' sites show within 400 m or under a sweep, far ones get bearings, the cluster's edge turns ships back

- Status: Accepted (provisional, lead). 2026-10-16: a launch into a cluster makes signals' sites only of the signals
  the career has heard (track E4, [GDR-0096](0096-career-galaxy-format-3.md)), no longer of every trail place.
  2026-10-16: scans narrow the career's signals and hear beacons; a signal's site seen is found, the map shows a
  cluster in flight (N) (tracks E5, E6, [GDR-0097](0097-galaxy-map-and-voyage-flow.md)).
  2026-10-17: the slice's goal is an ordinary site again (sensors show it at 800 m): its voyage is the signal, the
  slice being in the galaxy (track E7, [GDR-0098](0098-slice-in-the-galaxy-and-rim-rewards.md)).
- Date: 2026-10-16
- Decided by: lead session (defaults for Niklas to review, galaxy track E3)
- Related: [GDR-0017](0017-combat-support-repair-capture-sensors.md), [GDR-0078](0078-galaxy-exploration-v1.md), [GDR-0080](0080-exploration-searching-and-story.md), [GDR-0093](0093-galaxy-exploration-decisions-16-oct.md), [GDR-0094](0094-galaxy-generator-defaults.md), [GDR-0092](0092-evolution-lab-for-your-own-ships.md), [ADR-0022](../adr/0022-one-simulation-space-per-cluster.md), `docs/plans/galaxy-voyages.md` (E3, 4.6, 6.2), `docs/results/2026-10-16-bigger-clusters.md`

## Context

Track E3 builds the sim for 2–3 km clusters: the scanner Niklas set in GDR-0093 (1×1, 3 BP, 3 power while it charges
2 s, 1.2 km, 8 s cooldown, the starter carries one in place of a cargo bay), sites hidden until found, scans with
bearings. The plan and GDR-0094 left open how far a ship sees on its own (sensor range for behaviors, GDR-0017's
"moderate sensors"), what a scan gives a bearing to before the career knows signals (E4), how far sites stream, and
what happens at a cluster's edge (E0: one `f32` space holds about 4 km).

## Options considered

- **Sensor range**: a `sensor` block on logic and automaton cores, 800 m (chosen: the lead default the plan named;
  a 2–3 km cluster stays mostly dark, a fight stays in sight); unlimited (every behavior sees the whole cluster and
  its cost grows with it); range on a separate sensor part (one more part to build before a ship can fight at all).
- **What sensors show**: every site within range (chosen for ordinary sites); nothing, scans only (searching every
  raider pair by scan is a chore).
- **What a bearing points at**: a signal's site (chosen, the plan's 4.1: until E4 every trail place in a generated
  cluster and the slice's goal); every hidden site (a fan of lines that says little).
- **The edge**: a push back beyond the radius plus a margin (chosen); leaving the cluster as the recall (a
  surprise exit mid-fight); nothing (ships drift where `f32` goes wrong, E0).

## Decision

*(All numbers placeholders, in content or `sim::VoyageTuning`.)*

- **Sensors**: logic cores and automaton cores carry `sensor: (range: 800)`. A vessel sees as far as its working
  sensors on its controlling piece reach (the longest), at least 150 m (GDR-0017: nearby ships are always visible).
  Behaviors and pilot assists pick targets only within it; the `locked` selector follows the leader's lock wherever
  it is. The scanner does not widen a ship's targets: it widens what the fleet knows of the cluster.
- **Hidden sites**: a voyage's site shows (on the HUD's markers, as a dotted ring in the world) once one of the
  player's vessels has it within its sensors, or a scan sweeps over it. A **signal's site** (a trail's place in a
  generated cluster; the slice's goal, until E7 pins the slice into the galaxy) shows only within 400 m (if the
  vessel's sensors reach that far) or under a sweep. Found is final.
- **A scan**: SCAN (G) for the flown ship, a SCAN pill on the HUD, and the `scan` behavior action, which starts a
  scan when its rule holds and the scanner is ready and lets the next rule fly the ship (it never decides the
  maneuver; a ship without a working scanner skips it). The scanner with the longest range on the controlling piece
  charges; it draws its power only then, and less power charges it slower. A scanner lost mid-charge ends the scan
  without a sweep. The sweep reveals every site within the scanner's range, gives every hidden signal's site beyond it
  a **bearing** (an exact line from where the scan was taken; two from different spots cross at the place), and wakes
  the sleeping sites within the scanner's `wake` radius (300 m, GDR-0080). Then the cooldown.
- **Streaming**: as E0 recommends: sites stay records until a player vessel comes within 600 m (the director's lazy
  spawning), spawned sites sleep, nothing despawns; a whole cluster is never spawned at once.
- **The edge**: beyond the cluster's radius plus 500 m (at most 4 km from the centre) every piece is pushed back
  towards the centre, up to 3 m/s² 100 m past the edge, and its outward speed is taken away within 2 s. Not the
  piece's own thrust, so the speed governor never cuts it.
- **Beacons**: a trail's place with no encounter is a beacon alone: a quiet site that spawns nothing, found like any
  signal's site.
- **The evolution's mirror lock** (GDR-0092): the starter's single scanner has no twin across the centre line, so a
  start's parts without a mirror image are kept where they are (locked) instead of the start being refused; the
  lab's MIRROR stays on by default for a start with at most one such pair. The 60 BP skiff run allows 62 (the skiff
  is 60.5 BP with the scanner).

## Consequences

- Behaviors give up a target that gets more than 800 m away; raiders chasing a fleeing ship stop there.
- In the slice the goal no longer shows from the entry: its marker appears within 400 m or after a scan, and a scan
  from farther away points at it. A design without a scanner finds it by flying close.
- The rules' fingerprint changed (new part, new action, sensors): lab runs saved before restore stale (GDR-0092).
- E4 replaces "every trail place is heard" with the career's active signals; E5/E6 draw bearings on the cluster map
  overlay and decide how a generated cluster is launched (for testing: `?scene=voyage&cluster=signal`).
