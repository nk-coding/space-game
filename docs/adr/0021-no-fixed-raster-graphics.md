# ADR-0021: No fixed raster graphics: vector art, rasterised at runtime at display resolution

- Status: Accepted
- Date: 2026-10-11
- Decided by: Niklas
- Related: [ADR-0016](0016-wiki-ready-content-model.md), [ADR-0020](0020-style-2a-renderer.md), GDR-0007, `CLAUDE.md` "Hard rules"

## Context

The art sources are vector: SVG icons per part (`art/icons`), SVG ships, colour tokens in `art/tokens.json`. The
first client rasterised the icons into an atlas at build time and committed the PNG. Screens differ in DPI, and
the camera zooms.

## Options considered

- **A build-time atlas at a few fixed sizes or with mipmaps** (the style guide's suggestion and the first
  client).
- **Vector art only, rasterised at runtime at the resolution the display needs** (chosen). The reason was not
  recorded beyond the rule itself.

## Decision

- All game art is vector: SVG, meshes, procedural shapes, vector fonts.
- Raster textures (icon atlases, glow sprites) are generated in memory at runtime at the resolution the display
  needs (zoom × DPI) and regenerated when that changes.
- Never commit PNGs for the game to use; screenshots in `docs/` are fine.
- Icons move into the content pack (`content/base/icons/`), so mods can bring their own and the wiki can show the
  SVGs directly.

## Consequences

- The committed atlas (`crates/game/assets/atlas/atlas.png`) and the build-time `atlas-tool` step have to be
  replaced by rasterising in the client at runtime, on wasm too.
- Rasterising costs time at start-up and on zoom or DPI changes.
- Queued in the roadmap: icons in the content pack, served by `data`, included in the wiki export.
