# ADR-0028: Icons live in the content pack as SVG text, loaded and validated by `data`

- Status: Accepted (provisional, lead)
- Date: 2026-10-12
- Decided by: lead session (default for Niklas to review), following ADR-0021 (Niklas); file layout and checks by the power agent
- Related: [ADR-0021](0021-no-fixed-raster-graphics.md), [ADR-0015](0015-ron-content-packs-and-mods.md), [ADR-0016](0016-wiki-ready-content-model.md), [ADR-0017](0017-embedded-content-in-memory-loader.md), [ADR-0020](0020-style-2a-renderer.md), `docs/content-model.md` "Icons"

## Context

ADR-0021 decided that icons move into the content pack so mods can bring their own and the wiki can show the
SVGs. Until then the part icons lived in `art/icons` twice: `<key>.svg` (the glyph on a style-2a tile) and
`<key>.icon-only.svg` (the glyph alone, which the client rasterises). Content named icons by file stem, but
`data` never loaded them, and the validator checked them only when `content-tool` found `art/icons`.

## Options considered

- **Keep `art/icons` and embed the files in the client**: mods could not add icons, and the wiki would read a
  second place next to the content.
- **Icons as files of the content pack, loaded by `data` like any other content** (chosen).

## Decision

- A pack holds its icons as `icons/<key>.svg` (kebab-case key). `data` loads them as text: embedded with the
  base pack, from the directory with the `fs` feature, from in-memory files for mods.
  `Content::icon_svg(key) -> Option<&str>`; a later pack replaces an icon with the same key.
- An icon is the **glyph only** (the former `*.icon-only.svg`). The tile variants are deleted: the game draws
  tiles itself, and the wiki can composite a glyph on a tile. The four armor swatches (`armor-*.svg`, the
  materials' icons) move as they are (agent's call, for review).
- Validation: every icon key of a part, material or category exists in the loaded packs; every icon passes a
  cheap SVG check (well-formed tags, an `<svg>` root in the SVG namespace with a `viewBox`, no `<image>`,
  `<text>`, `<script>` or `<foreignObject>`); an icon nothing uses is a warning.
- `content-tool export-json` lists every icon with its SVG text, source file and users.

## Consequences

- The client rasterises `content.icon_svg(key)` at runtime (ADR-0021) and picks up mod icons without code
  changes; the wiki uses the same SVGs.
- `art/` keeps the concept art (`ships/`, `tokens.json`, `overview.png`); colours still come from
  `tokens.json` by key. A copy of the glyphs stays in `art/icons/*.icon-only.svg` only until the client's
  `crates/art` reads them from `data`; then it is deleted.
- The embedded base pack grows by about 50 KB of SVG text (wasm size does not matter, ADR-0006).
