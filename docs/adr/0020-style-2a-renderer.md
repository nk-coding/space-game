# ADR-0020: Style 2a renderer: one atlas mesh per ship piece, glow sprites, sRGB compositing on an HDR camera

- Status: Accepted (provisional, lead)
- Date: 2026-10-11
- Decided by: lead session (client work; default for Niklas to review)
- Related: [ADR-0011](0011-30hz-sim-60fps-interpolation.md), [ADR-0021](0021-no-fixed-raster-graphics.md), GDR-0007, `docs/style-guide.md` "Rendering in Bevy", `docs/client.md`, `docs/results/web-client-*.png`

## Context

Style 2a (GDR-0007) stacks many layers per ship: seams, tiles, slopes, icons with glow, hull halo and line, aura,
plumes, shield rings, for 20+ ships on WebGL2 and later on phones. The faint glows (halo opacity 0.18, line 0.32)
must look like the browser-rendered concept art. The style guide suggested an icon atlas, quads, a hull line mesh
and glow sprites first, with full-screen bloom as the expensive option to measure. The concept's art direction
lists "glow on everything, bloom as a default" under Avoid.

## Options considered

- **One sprite or entity per tile and icon**: simplest, but many entities per ship.
- **One atlas-textured mesh per ship (now per sim piece) holding every layer** (chosen).
- **Glow from full-screen bloom** or **from additive glow sprites** (sprites chosen as default; bloom kept as a
  toggle to measure).
- **Bevy's default linear-space blending** or **sRGB-space compositing** (chosen): the faint glows only match the
  concept art when blended like a browser.

## Decision

- Each sim piece is drawn as one atlas-textured body mesh (barrels, seams, tiles, slopes, icon glows, icons, hull
  halo and line, muzzle glows, nav lights). Variants (lit, dark, without glow sprites, debug shapes) are built
  lazily; the transform is `Piece::render_pose(alpha)`.
- Effects (plumes, trails, beams, tracers, sparks) are pooled atlas sprites, drawn in immediate mode.
- Glow sprites by default; bloom is a toggle.
- The camera composites in sRGB space (`CompositingSpace::Srgb`), which needs `Hdr` on WebGL2 in Bevy 0.20. UI
  colours go through a helper because bevy_ui does not encode for it.
- All visuals live in the client; `sim` knows nothing about colours or icons.

## Consequences

- The cost of glow sprites vs bloom, and of style 2a vs debug shapes, is still to be measured in the browser load
  test (roadmap phase 4).
- The icon atlas was first rasterised at build time (`atlas-tool`, PNG committed). ADR-0021 requires runtime
  rasterisation at display resolution, so that part has to change.
