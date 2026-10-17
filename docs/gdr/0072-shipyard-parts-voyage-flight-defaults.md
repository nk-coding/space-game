# GDR-0072: Defaults of the shipyard home, the machine drawings and voyage flight

- Status: Accepted (provisional, lead); RCS pods inside their cells confirmed by Niklas 2026-10-14; abandon superseded by [GDR-0077](0077-abandon-voyage-asks-in-a-panel.md); the committed recall and the machine cell tone confirmed in [GDR-0079](0079-review-of-lead-defaults-14-oct.md).
  2026-10-17: Accepted (Niklas, 17 Oct 2026); "V (or the gate) flies the first open voyage" is superseded by [GDR-0097](0097-galaxy-map-and-voyage-flow.md) (V and the gate open
  the voyage map, where SET COURSE picks the voyage).
- Date: 2026-10-13
- Decided by: lead session (defaults for Niklas to review); details by the agents that built GDR-0069, GDR-0071 and track 7H
- Related: [GDR-0062](0062-first-playable-loop.md), [GDR-0065](0065-phase-7-screens-and-escort-action.md), [GDR-0068](0068-voyage-sites-clear-when-enemies-are-disarmed.md), [GDR-0069](0069-machines-drawn-as-components.md), [GDR-0071](0071-shipyard-home-long-u.md), `docs/client.md`, `docs/style-guide.md`, `docs/career.md`

## Decision

Machine drawings (GDR-0069):
- Each machine's drawing is `icons/<icon key>.ship.svg` in the content pack (found through the part's icon key);
  only parts with an `engine` may reach past the stern (at most one cell). Thruster bells reach 0.35 cells, torch
  drives 0.45. The bell has a crisp hot rim; the plume adds the glow when it fires.
- The cell under a machine takes the armour tone that touches it on the most sides (ties: the tone the design uses
  more, then name order); a machine touching no hull takes the design's most-used tone, else steel.
- Part icons are kept in the atlas at half density (only the UI draws them).
- RCS pods and every other machine stay inside their cells (lead reading of GDR-0069, to confirm).

Shipyard home (GDR-0071):
- Research is lit once anything is researched or any node can be paid; the gate is awake while any voyage is open.
- Escorts fill the moorings near ones first; in the drydock the walls and the parked cranes stay visible.

Voyage flight (track 7H):
- V (or the gate) flies the first open voyage whose goal is not cleared, else the first open one (until the voyage
  map lands). A `?scene=voyage` launch the career would refuse runs as an unsaved practice run.
- M pressed twice within 3 s abandons a voyage (counts as lost): a ship gone dark next to an armed enemy can neither
  recall nor be lost otherwise.
- The warp-out can't be broken off, but losing every vessel during it still loses the voyage.
- Kills count enemy vessels lost (a split-off half counts on its own). The haul is the cargo of every player vessel,
  dark and dormant ones included. The debrief's "dismissed" state and kill counts are not saved.

## Consequences

- All easy to change after Niklas plays a voyage.
