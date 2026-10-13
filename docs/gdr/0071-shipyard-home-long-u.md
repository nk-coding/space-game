# GDR-0071: The home screen is the shipyard: a long U-shaped dock, the building area fills the U, the gate at the open side

- Status: Accepted
- Date: 2026-10-13
- Decided by: Niklas (three rounds of sketches, `docs/plans/mockups/shipyard-long-u.html`)
- Supersedes: the ring-menu layout of home in [GDR-0008](0008-ui-ship-mind-interface.md) and [GDR-0062](0062-first-playable-loop.md)
- Related: [GDR-0008](0008-ui-ship-mind-interface.md), [GDR-0062](0062-first-playable-loop.md), [GDR-0065](0065-phase-7-screens-and-escort-action.md), [GDR-0069](0069-machines-drawn-as-components.md), `docs/client.md` "Home"

## Context

The first home screen (track 7D) was a ring menu of buttons around the docked ship. Niklas wanted home to be the
shipyard itself: areas drawn as places, a click flying the camera there, the yard growing with progress. Round 1
(ring station, yard floor, wreck yard) didn't fit; he asked for a U-shaped station like the Star Trek drydocks with
the portal at the open side and the other buildings attached to the U. Round 2 offered three U variants; round 3
detailed the long U.

## Options considered

- Round 1: ring station, yard floor, wreck yard (rejected: too little detail, not the station he pictured).
- Round 2: the U with the mouth up; **the long U, mouth to the side (chosen)**; the deep U holding the portal.
- Round 3: the long U with more detail and the building area filling the whole U (chosen).

## Decision

- Home is a top-down world scene of the shipyard: a long U-shaped dock lying on its side, the gate (voyages) in
  front of the open side, an approach path of lights from the mouth to the gate.
- The building area is the whole inside of the U: the drydock frame. A bigger frame rebuilds the U bigger. The
  ship sits in it as designed (nose up is fine).
- Buildings attach to the U: research (with dishes) and later the fabricator on one long wall, the test range
  tower and lane and a salvage bay (later) on the other, the mind (behaviors) behind the base, fleet moorings on a
  truss bridge running off the screen edge.
- Detail: plate runs in varied tones, lattice trusses, conduits (power, data, the mind's violet), window rows lit
  when a building runs and dark when it doesn't, gantry cranes with carriages and hoist lines, docking clamps, work
  lights, drones, red and green navigation lights, a name plate.
- Clicking a place eases the camera onto it, then that screen's panels slide in; the drydock is the building area
  zoomed in. Keys stay (D, B, R, V, T, F).
- All of it is flat vector shapes with a few glows, built once and rebuilt only when the yard changes. Sizes and
  layout are placeholders.

## Consequences

- Home, the drydock and the voyage screens share one camera; the ring buttons of 7D go away.
- Each area needs states from content (dark, running, upgraded) and a slot in the yard layout.
