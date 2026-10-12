# Style guide: ship style 2a and the UI look

Decided 10 Oct 2026 after two rounds of options. Pictures: `boards/ship-style/chosen-2a.png`, `boards/ui-mockups/*.png`, `assets/overview.png`. Exact values: `assets/tokens.json`.

## Ships

**The idea:** flat vector tiles with a soft glow where the energy is. Every block stays readable, the drydock and flight show identical tiles, and the ship looks a little "fancy" without becoming a smooth blob.

- **Blocks**: flat fills, no bevels and no per-block gradients, separated by thin dark seams (`#03070a`, about 0.04 cells wide on each tile).
- **Armour materials** differ by tone:
  - steel `#1b2531` (blue-grey)
  - composite `#142a2e` (teal-grey)
  - reactive `#211d2c` (violet-grey)
  - enemy armour `#221f2a`
- **Machine tiles** are one flat colour (`#0f1c25`) with their glowing icon placed directly on the tile. Never put a dark inner square behind an icon.
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
- **Text**: Barlow. Labels are uppercase with wide letter-spacing (0.2–0.38em) in grey `#869099` or dim `#5d6a74`. Numbers are white `#ebf5ff` with a slight glow.
- **Accents**: teal `#8cefdb` (primary), gold `#eed595`, coral `#ec8f7c` (danger and enemies), violet `#ae8cec` (logic and conditions).
- **Controls**: pill buttons. The primary action is a glowing teal pill. Round glowing ring buttons for menus and tools.
- **Layout**: a floating HUD with no boxed panels. The ring menu sits around the docked ship on the home screen.
- Landscape only, on phone and desktop. See the mockups for the drydock (paint blocks, drag big machines), flight HUD, behavior editor (rule rows: violet conditions → teal actions) and galaxy map (dotted route, voyage panel with supplies and autopilot toggles).

## Rendering in Bevy (suggestion, to be measured in the spike)

- ~~Rasterise the icons into a texture atlas at build time.~~ Superseded (ADR-0021, ADR-0028): icons are SVGs in `content/base/icons/`, rasterised in memory at runtime at the resolution the display needs, regenerated when zoom or DPI changes.
- Draw tiles as coloured quads or sprites, with the seam baked into the tile texture or drawn as a second, slightly smaller quad. Draw icons on top.
- Build the hull edge as a line mesh from the boundary edges of the main piece. Skip straight sides covered by a neighbour; slope faces are always boundary. `drawHullGlow` in `reference/concept-board-script.js` shows the edge rule.
- Glow: try additive sprites or a pre-blurred line texture first. Full-screen bloom is the expensive option on phones and WebGL2, and the spike measures both.
- Shield bubbles are a procedural shader (no textures), all bubbles in one mesh and one draw call, blended with `max` (ADR-0029).
- Keep every visual on the client: the `sim` crate knows nothing about colours or icons.
