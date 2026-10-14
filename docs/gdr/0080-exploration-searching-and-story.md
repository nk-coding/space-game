# GDR-0080: Exploration details: 2–3 km clusters, a scanner part, scans wake enemies, about two steps to locate, free decoding, story by fragments and cards

- Status: Accepted
- Date: 2026-10-14
- Decided by: Niklas (eight questions from `docs/plans/galaxy-voyages.md` section 9, interactively)
- Related: [GDR-0078](0078-galaxy-exploration-v1.md), [GDR-0075](0075-voyages-feel-like-exploration.md), [GDR-0002](0002-awakened-ship-mind-light-story.md), [GDR-0028](0028-clusters-and-voyages.md), [ADR-0035](../adr/0035-saves-keep-the-galaxy-skeleton.md), `docs/plans/galaxy-voyages.md`

## Context

GDR-0078 set how exploration works (signals that narrow, blind jumps, jump range, hidden gates, scanning inside
clusters). The reworked plan left eight questions on how searching feels and how the story is told.

## Options considered

The recommended option came first in each question; Niklas chose it in all but one.

- **Cluster size**: 2–3 km (chosen); keep ~1 km and search only on the map; 6–8 km with a cruise speed.
- **How a scan works**: a key with a short charge; a passive sensor; **a scanner part you build in** (chosen:
  ships without one cannot scan).
- **What a scan risks**: it wakes sleeping enemies nearby (chosen); nothing; it reveals you across the cluster.
- **How fast signals narrow**: about two steps (chosen); one step; three to four.
- **Decoding a log**: free (chosen); some research data; a timer.
- **A blind jump**: an ordinary cluster, sometimes the start of a trail (chosen); always something small;
  sometimes danger above the band.
- **The galaxy layout in the save**: keep the skeleton (chosen; ADR-0035); rebuild from the seed.
- **The story**: lore fragments plus milestone cards (chosen); fragments only; also an inner voice in flight.

## Decision

- **Clusters** for searching are about 2–3 km in radius *(placeholder)*.
- **Scanning needs a scanner part**: a part in content, built into a ship in the drydock; a ship without one
  cannot scan, so scanning is a design trade-off (room, power, mass). How a scan is triggered and its numbers are a
  lead default until play: an action (key or button) with a short charge, then a sweep that reveals sites within
  the scanner's range and gives active signals a bearing; better scanners (research) reach further.
- **A scan wakes sleeping enemies** within a few hundred metres *(placeholder: 300 m)*.
- **Narrowing**: two scans or one second source usually locate a signal's cluster; a scan or two inside find the
  place.
- **Decoding a log** brought home is free.
- **A blind jump** finds an ordinary cluster of its band; about 1 in 3 *(placeholder)* holds the start of a short
  trail.
- **The story**: lore fragments at the ends of lore trails, kept in a memory log, plus short cards at milestones
  (each gate opened, each new band); no inner voice in flight for now.

## Consequences

- New content: a scanner part (family, size, power, range) and its research nodes; the starter or an early node
  must give the player a scanner, or the first trail could never be followed (lead to settle in the content
  track).
- The drydock shows a scanner's range; the flight HUD shows the scan control only on ships with a working scanner.
- Bigger clusters need sensor range for behaviors and frozen far sites before they ship (plan track E0, E3).
- The memory log and milestone cards are content (`lore`, `story`), shown at home.
