# GDR-0110: Stations are anchored ships: every piece keeps its place and only turns; the first is the automaton outpost, a rim and an outer site

- Status: Accepted (provisional, lead), 18 Oct 2026, for Niklas to review
- Date: 2026-10-18
- Decided by: lead session (content batch of 18 Oct 2026, the precursor outpost; default for Niklas to review)
- Related: [GDR-0026](0026-procedural-galaxy-and-bands.md) (site kinds: race stations, outposts), [GDR-0013](0013-vessels-and-piloting.md), [GDR-0047](0047-ramming-crushes-inward-bumps-free.md), [GDR-0068](0068-voyage-sites-clear-when-enemies-are-disarmed.md), [GDR-0104](0104-scenery-rocks-filler-landmarks.md) (anchored rock), [GDR-0109](0109-a-ram-prow-and-an-engine-arm-a-ship.md), `docs/results/encounters-balance-2026-10-18.md`

## Context

GDR-0026 lists outposts and stations among the site kinds. The first, a precursor outpost the automatons keep, has
no engines and must not be shoved about by rams (a skiff at 15 m/s pushes a free 13×9 hull at 4.6 m/s), shots or
plumes; it should still turn its guns onto you, and its spokes, which carry cores of their own, should stay where
they were when shot off.

## Options considered

- **An `anchored` flag on the design** (chosen): explicit in the content, nothing else changes for ships without it.
  Its pieces' Rapier bodies lock their translation (they turn on their gyros), and a breakage gives an anchored
  ship's pieces no velocity.
- **Every design without an engine is anchored**: no new field, but engine-less player or evolved designs would
  silently become immovable turrets.
- **A fixed body, like the rock** (GDR-0104): it could not turn, so it could not aim.

## Decision

- A blueprint's `anchored: true` makes it a station: each of its pieces, split-off ones and its wrecks too, keeps
  its place and only turns. Ram damage works as against any hull (the impulse of hitting something that does not
  give way).
- The `automaton_outpost` (13×9, automatons): a gunless hub with two shield emitters, a fusion reactor, two cores,
  capacitors and radiators, and three spokes on one bridge block each, each with its own core: a scanner mast in
  front, a cannon on each side spoke. It flies the `outpost` behavior (turn the front onto the nearest enemy in
  reach, else hold). Sites (lead defaults): `precursor_outpost` (the outpost and a `sample_mid` wreck, 0–1 per wreck
  drift) and `outpost_garrison` (with a gunner, a hunter and 1–2 `sample_mid` wrecks, 0–1 per border drift).
- Tuning (seeds 1–120, the voyage runner): a rim test voyage ending at the outpost clears 64 % with a Runner and a
  skiff (a hunter den's 72 %, the Warden 42 %); an outer one ending at the garrison 53–54 % with a Runner and an
  escort (a grove run 52–55 %).

## Consequences

- A station's guns fire only along their facing, so it aims by turning; a broken-off spoke turns on its own core.
- More stations (race stations, the Swarm's hives) are designs with the flag.
- Open: a station's own salvage reward (a cache's "straight into store"); docked wrecks stand in for now.
