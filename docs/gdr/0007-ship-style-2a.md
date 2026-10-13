# GDR-0007: Ships use style 2a: flat tiles with seams, glowing icons, a soft teal hull glow

- Status: Accepted; the machine-tile rule is superseded by [GDR-0069](0069-machines-drawn-as-components.md) (machines drawn as components)
- Date: 2026-10-10
- Decided by: Niklas
- Related: [GDR-0008](0008-ui-ship-mind-interface.md), [ADR-0020](../adr/0020-style-2a-renderer.md), [ADR-0021](../adr/0021-no-fixed-raster-graphics.md), `docs/style-guide.md`, `docs/concept/boards/ship-style/chosen-2a.png`, `art/tokens.json`

## Context

Feedback on the first look: ships looked too blocky in flight, and the ship look and ship screens should be "a
bit" fancier. Niklas's reference: a dark blue-black, bioluminescent look with glowing teal line art, a gold
accent, white numbers, soft halos.

## Options considered

- **Smoothed silhouettes in flight** (A plated, B sculpted, C streamlined): dropped; the ship looked weird, and
  Niklas wants to see the blocks.
- **Round 1:** 1 metal, 2 luminous, 3 neon line, 4 hybrid. Verdict: not 1 or 3; between 2 and 4, with 2 the
  favourite. He disliked the 3D bevel look of the blocks ("outdated ppt shapes").
- **Round 2, all flat:** **2a** flat dark tiles, thin seams, teal edge glow (chosen); 2b one soft light across the
  hull; 2c small gaps and rounded corners (looked blobby); 2d flat mid-tone with a faint glow.
- Also rejected: per-block gradients, pure neon outlines (too far from the block look), different treatments in
  drydock and flight.

## Decision

- Flat blocks with thin dark seams; no bevels, no per-block gradients. Armor materials differ by tone.
- Machine tiles are one flat colour with their glowing icon directly on the tile (no dark inner square).
- A soft teal glow along the hull edge plus a faint aura; enemies use the same tiles with a coral glow.
- Particle engine trails, dotted shield rings. Each colour means one thing.
- The drydock and flight show identical tiles.

## Consequences

- Exact colours and sizes are in `docs/style-guide.md` and `art/tokens.json`; the renderer is ADR-0020.
- Tech shows through new parts, sizes and materials on the hull, not through skins.

## Notes

- 2026-10-12: the dotted shield rings are superseded by glowing energy bubbles,
  [GDR-0043](0043-shields-as-energy-bubbles.md) (Niklas). The rest of this record stands.
