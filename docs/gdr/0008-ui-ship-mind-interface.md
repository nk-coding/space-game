# GDR-0008: The UI is a diegetic ship-mind interface with a floating HUD, landscape only

- Status: Accepted
- Date: 2026-10-10
- Decided by: Niklas
- Related: [GDR-0007](0007-ship-style-2a.md), [GDR-0009](0009-stick-and-lock-controls.md), [GDR-0015](0015-layered-behavior.md), GDR-0017, `docs/style-guide.md` "UI", `docs/concept/boards/ui-mockups/`

## Context

Niklas's style reference: floating HUD numbers with wide-tracked uppercase labels, no boxed panels, dotted rings,
particles, a dark blue-black backdrop. The mockups canvas has six boards (phone drydock, flight HUD, home ring
menu, behavior editor, galaxy map with autopilot, desktop drydock 1280×720) plus a ship parts style sheet.

## Options considered

- **A skeuomorphic cockpit**: not wanted ("no skeuomorphic cockpit"); reason not recorded.
- **Boxed panels**: replaced by the floating HUD of the reference look.
- **A diegetic ship-mind interface** (chosen).

## Decision

- **Look:** thin lines, corner brackets, mono labels; floating HUD without boxed panels; Barlow, uppercase labels
  with wide letter-spacing, white glowing numbers; pill buttons (primary = glowing teal), glowing ring buttons,
  dotted rings, particles; accents teal, gold, coral (danger, enemies), violet (logic).
- **Landscape only**, on phone and desktop.
- **Drydock:** paint blocks and armor (drag; material picked once), drag and drop for big machines; live stats
  (power, heat, thrust/mass, yaw, firepower, cargo, mind capacity, device load); test-fly from the drydock;
  overlays for heat, mass, bonuses, shields and arcs. Desktop adds search and category tabs, shortcuts, a part
  inspector with a variant picker (stat diff), and upgrade materials with "missing → where to find it" and
  "track on map".
- **Behavior editor:** a node graph on all platforms (a priority list of rules: violet conditions → teal
  actions), Rhai nodes tagged, limited rule slots per logic core, graph/text toggle, "test in range".
- **Flight HUD:** customisable (move or hide widgets). Default: hull map and supplies top left, target panel with
  priority chips top right, stick bottom left, fire and ability buttons bottom right, off-screen markers.
- **Home:** a ring menu around the docked ship (drydock, research, fabricator, test range, launch voyage,
  storage, galaxy map, behavior).
- **Travel:** autopilot for transit (route over lanes on the galaxy map, supplies check), manual flying in
  fights, automatic hand-over when a hostile locks on.

## Consequences

- The mockups in `docs/concept/boards/ui-mockups/` are the reference for every screen.
- UI colours and fonts come from `art/tokens.json` and vector fonts (ADR-0021).
