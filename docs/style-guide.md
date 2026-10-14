# Style guide: ship style 2a and the UI look

Decided 10 Oct 2026 after two rounds of options ([GDR-0007](gdr/0007-ship-style-2a.md)). Pictures: [`concept/boards/ship-style/chosen-2a.png`](concept/boards/ship-style/chosen-2a.png), `concept/boards/ui-mockups/*.png`, [`../art/overview.png`](../art/overview.png). Exact values: [`../art/tokens.json`](../art/tokens.json); the client embeds them through the `art` crate (typed view: `crates/game/src/style.rs`).

## Ships

**The idea:** flat vector tiles with a soft glow where the energy is. Every block stays readable, the drydock and flight show identical tiles, and the ship looks a little "fancy" without becoming a smooth blob.

- **Blocks**: flat fills, no bevels and no per-block gradients, separated by thin dark seams (`#03070a`, about 0.04 cells wide on each tile).
- **Armour materials** differ by tone:
  - steel `#1b2531` (blue-grey)
  - composite `#142a2e` (teal-grey)
  - reactive `#211d2c` (violet-grey)
  - enemy armour `#221f2a`
- ~~**Machine tiles** are one flat colour (`#0f1c25`) with their glowing icon placed directly on the tile.~~
  Superseded on the ship on 13 Oct 2026 by [GDR-0069](gdr/0069-machines-drawn-as-components.md) (Niklas): see
  **Machines** below. The dark tile with its icon stays for the part tiles in the UI (the drydock's palette and
  inspector); never put a dark inner square behind an icon.
- **Machines** (GDR-0069, 13 Oct 2026; sketch: [`plans/mockups/ship-part-looks.png`](plans/mockups/ship-part-looks.png),
  options C and D) are drawn as housings: the machine's own flat vector shape (`content/base/icons/<key>.ship.svg`)
  on a cell in the hull's armour tone, with seams like every tile.
  - **The cell's tone**: the armour tone that most of the hull touching the machine has (counted per touching cell
    side; a tie goes to the tone the design uses most); a machine with no hull next to it takes the design's most
    used tone (by hull area; a tie goes to the first tone in name order); a design without hull, steel.
  - **Housings** are dark blue-grey (`#1a2a36`, raised parts `#24384a`; the automatons' `#2a2431` / `#3a3242`),
    outlined and detailed in the system colour; flat fills, no bevels. Glow only where the energy is: a reactor's
    core, an emitter's centre (a nozzle's mouth is a crisp hot rim: its glow is the plume). A glow is a soft layer under the drawing that shows through a
    window in the housing, with a crisp hot core on top.
  - **Engines** (thrusters, torch drives) reach past the hull at the stern: the nozzle bell sticks out 0.35 cells
    (thruster) and 0.45 cells (torch drive), placeholders, over the hull edge, and the plume starts at its mouth.
    Nothing else does: radiators keep their fins inside the cell, guns keep today's barrels. Drawn protrusions never
    collide; collisions stay on the grid.
  - Every machine follows rotation and mirroring; the drydock, its ghost, flight and the home screen show the same
    drawings. Pictures: [`results/parts-drydock-after.png`](results/parts-drydock-after.png),
    [`results/parts-flight-after.png`](results/parts-flight-after.png) (before: `parts-*-before.png`).
- **Icons** use the system colours:
  - power `#e2b15c`
  - weapons `#5cc6d6`
  - shields `#8fb0f2`
  - heat/radiators `#ec8f7c`
  - propulsion `#8cefdb`
  - logic `#ae8cec`
  - cargo `#8d9db4`

  Each icon has a soft blurred copy under it (opacity 0.55).
- **Hull edge**: a faint teal glow along the outer edge of the ship's main piece. It has two layers:
  - a halo: about 0.15 cells wide, opacity 0.18, blurred;
  - a crisp line: about 0.045 cells wide, opacity 0.32.

  Enemies use coral `#ec8f7c` instead. Pieces that break off lose the glow, which shows what has become salvage.
- **Aura**: a very faint radial teal glow behind ships in flight (0.10 at the centre, 0.04 midway, fading to 0).
- **Engines**: a teal plume (white-teal at the nozzle, fading out) over a blurred glow, plus a dotted particle trail.
- **Guns**: beam lasers show a short metal barrel through the nose with a cyan muzzle glow when the line of fire is clear.
- **Shields**: glowing energy bubbles in `#8fb0f2` ([GDR-0043](gdr/0043-shields-as-energy-bubbles.md), 12 Oct 2026; they
  replace the dotted rings). Values: `ship.shield_bubble` in `art/tokens.json`.
  - Fill almost clear in the middle (opacity 0.02, so the tiles stay readable), rising to 0.2 at the rim.
  - A crisp rim line (about 0.05 cells, opacity 0.6, tinted towards the near-white `#e4ecff`) over a soft glow
    (opacity 0.3, falling off over about 0.38 cells outside, faster inside).
  - A slow shimmer drifts over the fill (brightness ±27 %, faint veins towards the rim).
  - A hit: a flash where the shot entered, a ripple ring spreading across the bubble, the rim widening and
    whitening around the impact (0.7 s).
  - Below 30 % charge: stutter, wobble and a crackling, broken rim.
  - Collapse: a pop, cracks across the membrane, fragments going dark, rim shards flying out (0.85 s).
  - Overlapping bubbles never add up: the brighter one shows. A bubble that is down is not drawn.

### Rejected, and why

- 3D bevels and per-block gradients: they looked like outdated presentation shapes.
- Smoothed hulls and skins that hide the blocks: the blocks must stay visible.
- Gapped or rounded tiles: they looked blobby.
- Pure neon outlines: too far from the block look.
- Mixed treatments between drydock and flight: the same tiles everywhere.

## UI

- **Backdrop**: a dark blue-black radial gradient (`#0a1820` → `#050e15` → `#010409`) with faint particles (`#6c9b95`) and dotted rings (`#414c52`).
  - Later (Niklas, 12 Oct 2026): a fancier background (planets, nebulas and the like). The current backdrop is
    sufficient for now.
- **Text**: Barlow. Labels are uppercase with wide letter-spacing (0.2–0.38em) in grey `#869099` or dim `#5d6a74`. Numbers are white `#ebf5ff` with a slight glow. Barlow has no arrows (`→`, `▸`) and no `★`, and bevy_ui has no font fallback, so such a character shows as a box: write "turned into" as `»` and breadcrumbs as `›`.
- **Accents**: teal `#8cefdb` (primary), gold `#eed595`, coral `#ec8f7c` (danger and enemies), violet `#ae8cec` (logic and conditions).
- **Controls**: pill buttons. The primary action is a glowing teal pill. Round glowing ring buttons for menus and tools.
  Labels name the action only, never its key (Niklas, 13 Oct 2026: no `SAVE CTRL S`, `PAINT B`).
- **Tooltips** (13 Oct 2026): the key goes into a tooltip. After the pointer rests 0.5 s on a control, a small dark
  plate (the backdrop's darkest `#010409` at 94 %, a 1 px border in label grey at 28 %, 4 px corners, no glow) shows
  near the pointer: the control's name as a tracked label in grey `#869099` (8.5 px), then its key in white `#ebf5ff`
  (Barlow medium 10 px), e.g. `SAVE  CTRL+S`. It never leaves the window. Places in the shipyard get the same.
- **Scrollbars** (14 Oct 2026): thin overlay bars, label grey `#869099` at 38 % (62 % under the pointer), 4 px wide
  (6 px under the pointer), fully rounded, over the right edge (below a sideways strip); shown while scrolling or
  hovering, gone 1 s later; never boxed. A glow at a list's edge fades out past it, never ends in a straight line.
- **Layout**: a floating HUD with no boxed panels. The home screen is the shipyard itself, a top-down scene of the
  station with small tracked labels over its places ([GDR-0071](gdr/0071-shipyard-home-long-u.md); it replaced the
  ring menu around the docked ship).
- **Fleet bar** ([GDR-0050](gdr/0050-fleet-bar.md), 12 Oct 2026): a row of chips at the top centre, no panel.
  Each chip is outlined only by four thin corner brackets (grey at 40 %, teal for the focus); inside, the vessel's
  number (white; teal for the focus, coral when dark, dim when dormant), small violet dots for its logic cores, a
  2 px bar of its control load (teal, gold when nearly full, coral while parts idle) and a tiny tracked state
  word (`PILOT`, `AUTO`, `DARK`, `DORMANT`). Above the chips: `MIND 8 / 24` with a thin violet bar.
- Landscape only, on phone and desktop. See the mockups for the drydock (paint blocks, drag big machines), flight HUD, behavior editor (rule rows: violet conditions → teal actions) and galaxy map (dotted route, voyage panel with supplies and autopilot toggles).

## Rendering in Bevy

The client follows [ADR-0020](adr/0020-style-2a-renderer.md); details in [`client.md`](client.md).


- ~~Rasterise the icons into a texture atlas at build time.~~ Superseded (ADR-0021, ADR-0028): icons are SVGs in `content/base/icons/`, rasterised in memory at runtime at the resolution the display needs, regenerated when zoom or DPI changes.
- Each sim piece is one atlas-textured mesh (a lit piece's tiles are a second, opaque one, so shield bubbles can sit between ships by depth): every tile is drawn as its outline in the seam colour with the fill inset by the seam width, then the hull edge, then the machines' drawings (GDR-0069; an engine's bell over the hull edge) (`ship_view.rs`).
- The hull edge is built from the boundary edges of the main piece (`data::design::geometry`, `hull_edges`): straight sides covered by a neighbour are skipped; slope faces are always boundary. `drawHullGlow` in `concept/reference/concept-board-script.js` shows the original rule.
- Glow: pre-blurred glow sprites by default. Full-screen bloom is a toggle (key `3`, `?bloom=1`) to compare; it is the expensive option on phones and WebGL2.
- Plumes, trails, beams and sparks are pooled atlas sprites written into a few dynamic meshes every frame (`fx.rs`).
- Shield bubbles are a procedural shader (no textures), all bubbles in one mesh and one draw call, blended with `max` (ADR-0029).
- Keep every visual on the client: the `sim` crate knows nothing about colours or icons.
