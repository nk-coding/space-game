# ADR-0032: Wiki generator: `content-tool wiki` writes a static HTML site straight from `data`

- Status: Superseded by [ADR-0039](0039-vitepress-wiki-from-game-data.md)
- Date: 2026-10-13
- Decided by: wiki agent (proposal; the defaults below are for Niklas to review)
- Related: [ADR-0015](0015-ron-content-packs-and-mods.md), [ADR-0016](0016-wiki-ready-content-model.md), [ADR-0018](0018-english-only-keyed-text.md), [ADR-0021](0021-no-fixed-raster-graphics.md), [ADR-0028](0028-icons-in-the-content-pack.md), [ADR-0030](0030-drydock-architecture.md), [GDR-0052](../gdr/0052-drydock-stats-full-use.md), [GDR-0054](../gdr/0054-drydock-locked-parts-tools-saving-sharing.md), `docs/wiki.md`

## Context

The content model was made wiki-ready (ADR-0016): stable ids, keyed text, stat descriptors, prose files next to the
data, icons in the pack, a JSON export. The wiki itself did not exist. It should show everything the packs hold
(base plus mods), stay all Rust, use only vector art, look like the game, be small, fast and deterministic, and
need no JavaScript.

## Options considered

- **Where the generator lives**:
  - a `wiki` module in `data`: `data` must stay lean for wasm and the game, has no filesystem, and cannot use the
    sim's design report (the drydock's ship numbers) because `sim` depends on `data`;
  - a new `wiki` crate: one more crate for one command;
  - **a module of `content-tool`** (chosen): the CLI already loads packs from directories, validates them
    (including the sim's vocabulary check) and has `serde_json`. The generator is a pure function (content in,
    files out), so moving it into its own crate later is cheap.
- **Input**: the JSON export (any language) or **the `data` types directly** (chosen): typed, no second schema,
  and the same formatting code as the drydock (`Content::stat_line`, `part_stats`, `geometry`).
- **Output**: a static site generator (mdBook, Zola: external tools, their own templates and look) or a JavaScript
  app (not needed) or **plain HTML, one CSS file, no JavaScript** (chosen).
- **Markdown**: a dependency (pulldown-cmark) or **a small hand-written subset** (chosen): the prose files are
  short, and escaping everything by hand keeps the output safe with untrusted mod text. No new dependencies.
- **Style tokens**: depend on the `art` crate (pulls in resvg) or **embed `art/tokens.json` at build time** like
  `art` does (chosen).
- **Icons**: link the SVG files with `<img>` or **inline them** (chosen): one `<symbol>` per icon a page shows,
  with ids prefixed per icon, drawn with `<use>` in tables and ship layouts.

## Decision

- `content-tool wiki <out_dir> [pack_dir...]` (code `crates/content-tool/src/wiki/`). Packs load and validate as
  for the other commands; the site is `index.html`, `style.css`, `fonts/`, one list per kind (`parts.html`, …,
  `pages.html`) and one page per entry at `<kind key>/<id>.html`, mirroring `[[kind:id]]` links.
- Everything shown comes from the content: texts (the keyed English tables), stat labels, units and precision
  (`stats.ron`), icons (the pack's SVGs), ship and part drawings (blueprint geometry, style 2a, colours from
  `art/tokens.json`). The page chrome (headings, fact labels, "Locked") is the tool's own English, like the game's
  UI strings.
- Output is deterministic (sorted paths, load order, no dates). The directory must be new, empty or an earlier wiki;
  a `.wiki-files` list lets the next run remove stale pages without touching anything else. The output is not
  committed.

Defaults for review (agent's choices):

- **Locked content is shown**, everywhere, with a coral "Locked" badge: this is a developer and modder wiki for
  now. A player wiki would filter it out.
- **Ship pages show the sim's design report** (`sim::design_report` with `SimConfig::default()`: full power,
  full use, GDR-0052), the numbers of the drydock's stats panel, labelled from `stats.ron`. Yaw and device load are
  left out because the drydock shows them as words, not numbers.
- **Frames list the ships whose grid fits them**; ship pages name the smallest such frame (blueprints name no frame
  in v1).
- **Hull parts** are drawn in every armor material on their page and in the first unlocked armor material in
  lists; in a ship's parts table, in the ship's own material.
- **Back-links**: every entry lists the entries whose texts or prose link to it ("Mentioned on").
- **Markdown subset**: headings, paragraphs, one-level lists, quotes, rules, code blocks; inline links, code, bold,
  italic (also in content texts). Raw HTML and Markdown links show as text.
- **Fonts**: Barlow is copied from `crates/game/assets/fonts` (with `OFL.txt`) so the site looks like the game
  offline.
- Generated pages are not part of the repo; screenshots are in `docs/results/wiki-*.png`.

## Consequences

- Content authors and modders can see every entry, its numbers and its links in a browser; broken links and
  prose problems surface as validation errors or generator warnings.
- New kinds or fields need a page section in the generator; new stats need nothing (labels come from
  `stats.ron`).
- Icons are inlined as they are (after `data::svg::check_svg`); a `<style>` element in a mod's icon would apply to
  the whole page.
- Not yet: search, a tag index, translated wikis, behavior graph pictures.
