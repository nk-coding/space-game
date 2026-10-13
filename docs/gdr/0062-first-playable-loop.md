# GDR-0062: The first playable loop: start at home, short hand-made rim voyages, mind capacity 2 → 4

- Status: Accepted
- Date: 2026-10-13
- Decided by: Niklas (interactively)
- Related: [GDR-0002](0002-awakened-ship-mind-light-story.md), [GDR-0008](0008-ui-ship-mind-interface.md), [GDR-0013](0013-vessels-and-piloting.md), [GDR-0026](0026-procedural-galaxy-and-bands.md), [GDR-0028](0028-clusters-and-voyages.md), [GDR-0029](0029-mild-death-bonded-matter.md), [GDR-0063](0063-salvage-and-research-v1.md), [ADR-0033](../adr/0033-career-model-save-and-voyage-director.md), `docs/plans/first-playable-loop.md`

## Context

After the drydock and the behavior editor the game had a sandbox, an editor for ships and one for behaviors, but
nothing to achieve and no reason to come back. Niklas chose the first playable loop as the next phase (over Android,
co-op and the arena, and polishing what exists). The plan (`docs/plans/first-playable-loop.md`) proposed a small
slice: home → voyage → fights and wrecks → salvage → research → rebuild → further, ending at a boss after about
three hours. The deeper plans for the galaxy and for salvage and research came in at the same time; this record
sets the slice's scope.

## Options considered

- Next phase: **the first playable loop** (chosen), Android first, arena and co-op, polishing only.
- Start: **at the home shipyard with a working starter ship; the sandbox becomes TEST RANGE** (chosen), the
  concept's first hour as a drone, or the sandbox as today.
- A voyage: **one small hand-made rim cluster (1–2 km) with seeded sites, four in a chain** (chosen), endless waves
  in one arena, or waiting for the procedural galaxy.
- Mind capacity: **start 2, +1 twice by research (4 by the end of the slice)** (chosen), start 4 and grow to 6, or
  keep 24.
- Recall: **3 s warp-out whenever no awake enemy is within about 200 m** (chosen), a 15 s charge broken by fire, or
  flying back to the cluster's exit.
- Closing the game mid-voyage: **the voyage is lost, the ships are safe** (chosen), it counts as a recall, or a
  checkpoint every minute.

## Decision

- The game starts at home (the ring menu of GDR-0008) with a starter ship; today's sandbox stays as TEST RANGE
  (and `?scene=sandbox`).
- A voyage is a small hand-made rim cluster with seeded sites (raider packs, derelicts, a goal site); clearing the
  goal opens the next one. Four voyages; the last ends at the Warden, an automaton boss. Re-launching re-rolls the
  sites.
- Recall home takes 3 s and works when no awake enemy vessel is near (placeholder 200 m). Losing the last vessel
  sends you home (mild death, GDR-0029). Closing or reloading mid-voyage counts as losing that voyage; no mid-voyage
  saves in the slice.
- Mind capacity starts at 2 logic cores and grows by research to 4 in the slice (numbers are placeholders).
- All numbers of the plan stay placeholders until play-tested.

## Consequences

- The slice is deliberately smaller than GDR-0028 (clusters "10+ minutes across", supplies, outposts), GDR-0026
  (one procedural galaxy) and GDR-0002 (the first hour): those records stay the target; the slice's clusters become
  templates the galaxy generator places later.
- New screens: home, research, a small voyage map, the voyage HUD. A career save (ADR-0033).
- Reloading the page during development costs a voyage's haul; `?scene=sandbox` and the test range are unaffected.
