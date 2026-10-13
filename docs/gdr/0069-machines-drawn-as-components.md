# GDR-0069: Machines are drawn as their own components; engine nozzles reach past the hull

- Status: Accepted
- Date: 2026-10-13
- Decided by: Niklas (from the proposals "Ship Part Looks", options C and D)
- Supersedes: the machine-tile rule of [GDR-0007](0007-ship-style-2a.md) ("one flat colour per machine with its glowing icon placed directly on the tile")
- Related: [GDR-0007](0007-ship-style-2a.md), [ADR-0020](../adr/0020-style-2a-renderer.md), [ADR-0021](../adr/0021-no-fixed-raster-graphics.md), [ADR-0028](../adr/0028-icons-in-the-content-pack.md), `docs/style-guide.md`

## Context

Niklas found the ships too blocky: every machine was a square dark tile with an icon, so an engine (which obviously
has a nozzle) read as a block. Four treatments were drawn on the Halvex Runner: today's tiles, detailed tiles,
housings (the machine drawn as its own shape on a hull-coloured cell) and housings with the working ends reaching
past the hull.

## Options considered

- A: today's tiles; B: detailed tiles with icons; C: housings; D: housings plus silhouette (nozzles, barrels and
  radiator fins past the hull). Niklas chose a mix.

## Decision

- Machines are drawn as housings (option C): the machine's own flat vector shape on a cell in the hull's armour
  tone, the system colour as its accent, glow only where the energy is. The drawing replaces the icon on the ship.
- Engines (thrusters, torch drives) use option D: their nozzle bell reaches past the hull at the stern (placeholder:
  up to about 0.35–0.45 cells). Radiators stay inside their cells (option C). Beam barrels stay as today.
- Everything else of style 2a stands: flat fills, seams, square armour tiles, hull edge glow, no bevels or gradients.
- Lead reading (to confirm): RCS pods and the other machines follow C; only engines reach past the hull.

## Consequences

- Each part needs a vector drawing for the ship (content, next to its icon; the palette and wiki may keep the icon).
- The cell under a machine takes the hull tone of its design (the neighbours' armour, else the design's main
  material).
- Drawn protrusions don't collide; collisions stay on the grid.
