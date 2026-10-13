# GDR-0065: Phase 7 screens: debrief panel, research tree, star map, recall under the cargo, fleet formation, locked parts shown dimmed; an `escort` action

- Status: Accepted
- Date: 2026-10-13
- Decided by: Niklas (interactively, from the mockups in `docs/plans/mockups/`)
- Supersedes: the hiding of locked parts in [GDR-0054](0054-drydock-locked-parts-tools-saving-sharing.md), for the career's drydock
- Related: [GDR-0008](0008-ui-ship-mind-interface.md), [GDR-0054](0054-drydock-locked-parts-tools-saving-sharing.md), [GDR-0062](0062-first-playable-loop.md), [GDR-0063](0063-salvage-and-research-v1.md), [GDR-0037](0037-behavior-vocabulary-as-content.md), `docs/plans/mockups/README.md`, `docs/plans/slice-content.md`

## Context

Phase 7 adds a home screen, a research screen, a voyage map, the voyage HUD, a fleet layer and an unlock-aware
drydock. A mockup agent drew each screen in the style of the existing UI, with a second variant where a real choice
existed. Escorts also need to stay near their leader between fights, which no behavior action does today.

## Options considered

- Debrief: **a full panel after a voyage until dismissed, then a one-line summary** (chosen, `home-b.png`), or
  always one line (`home.png`).
- Research: columns of cards per family (`research.png`), or **a tree with lanes and connectors** (chosen,
  `research-b.png`; it grows with the trees).
- Voyage map: **a star map with the chain inside the rim band** (chosen, `map-b.png`), or a plain chain diagram.
- RECALL: **under the cargo block** (chosen, `voyage-hud-b.png`), or bottom right.
- Fleet: **the formation drawn on the dock, a saved design per slot** (chosen, `fleet-b.png`), or a list layer.
- Locked parts in the career drydock: **shown dimmed as `LOCKED · <research node>`, NEW tags on fresh unlocks**
  (chosen, `drydock-locked.png`), or hidden with one hint row (GDR-0054's rule).
- Escorts between fights: **a new content action `escort(distance)`: keep within N m of the leader** (chosen), a
  built-in follow in the sim, or holding position.

## Decision

The six choices above, as drawn in the chosen mockups (placeholders for sizes and texts). `escort(distance)` is a
new behavior action in content (GDR-0037), used as the fallback of a new `escort` template behavior.

## Consequences

- The career's drydock shows what research gives; the free drydock (test flights, `?unlock=all`) shows everything.
- The research screen needs a layout for lanes and connectors; the map positions come from content.
