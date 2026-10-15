# ADR-0039: The wiki is a VitePress site in the game's look; its content pages and every number come from the game's data

- Status: Proposed
- Date: 2026-10-15
- Decided by: Niklas (a Vite-based, VitePress-like wiki with search, styled like the game, vector only, reusing the
  game data without duplication, complete and consistent); wiki agent (the stack and the details below, for review)
- Supersedes: ADR-0032
- Related: [ADR-0016](0016-wiki-ready-content-model.md), [ADR-0018](0018-english-only-keyed-text.md), [ADR-0021](0021-no-fixed-raster-graphics.md), [ADR-0028](0028-icons-in-the-content-pack.md), [ADR-0032](0032-wiki-generator.md), `docs/wiki.md`, `docs/content-model.md`

## Context

Niklas (15 Oct 2026): the wiki should start now, "vitepressy" with lots of Markdown documents but styled like the
game, with search, describing all the game's functionality (every part, the mechanics, the behavior system), vector
graphics only, built with Vite, and reusing the game data as ADR-0016 set up, with no duplication. Completeness and
consistency matter most. The existing generator (ADR-0032) wrote a plain static HTML site with no search and no room
for hand-written mechanics pages.

## Options considered

- **Stack**:
  - extend ADR-0032's Rust HTML generator with search and prose pages: all Rust, but search, navigation, the phone
    layout and Markdown authoring would all be hand-built;
  - plain Vite with a Markdown plugin and an own search index: everything to build that VitePress has;
  - **VitePress with its default theme fully restyled** (chosen): Vite, Markdown pages, a local search index
    (MiniSearch) built at build time, sidebar, outline, prev/next and the phone layout ready; Vue components for the
    data views; a small dependency (`vitepress` and `reka-ui`, both pinned). Node is dev tooling only (CLAUDE.md).
- **Where the data comes from**: the JSON export (`export-json`) read by JavaScript, which would re-implement stat
  formatting, salvage, unlocks and drawings; or **`content-tool wiki-data`, which keeps all description logic in
  Rust** (chosen): the formatting the drydock uses (`Content::stat_line`, `part_stats`), the sim's design report,
  the blueprint geometry and the behavior parser, as ADR-0032 did.
- **Generated pages**: VitePress dynamic routes (their content is not in the search index) or **real Markdown files
  written into `wiki/gen/`** (chosen), mapped to their URLs by VitePress rewrites.

## Decision

- `wiki/` is a VitePress site (`npm run dev | build | check | deploy`; `docs/wiki.md`). `npm run build` first runs
  `content-tool wiki-data .`, which writes the git-ignored `wiki/gen/`:
  - one page per content entry of every kind and one list per kind (`gen/parts/thruster.md` → `parts/thruster.html`),
    made of HTML blocks Vue leaves alone, with the headings the outline and search read;
  - `refs.json` (names, links, values and sim constants that hand-written pages reference, and the rendered rule
    examples), `ui.json` (what the components show), `sidebar.json`, `sprite.svg` (the pack's icons), `tokens.css`
    (`art/tokens.json` as CSS variables) and the game's font.
- **Hand-written pages** (`guide/`, `mechanics/`, `behavior/`, listed in `nav.json`) hold only prose. They name data
  instead of copying it: `[[part:thruster]]` (a link), `[[part:thruster.thrust]]` (a value as the game shows it),
  `[[sim:tick_hz]]` (a simulation constant read from `sim`'s own constants and default config), components such as
  `<StatTable stats="thrust,rated_speed" />`, and ` ```behavior ` / ` ```assists ` blocks that the game's own parser
  checks and draws. The content pack's prose (`wiki/<kind>/<id>.md`, ADR-0016) stays where it is and shows on the
  entry's page.
- **Vector only**: drawings are SVG generated from the data (parts as technical drawings: tile, cell grid, linking
  sides, thrust and plume, line of fire, nozzles, vents, blast cells, shield reach; ships from the blueprint; rule
  graphs as the BEHAVIOR screen draws them) or written inline in the pages with the theme's token classes; CSS and the
  Barlow font otherwise.
- **The look** is the game's (`docs/style-guide.md`): the dark radial backdrop with dotted rings and particles,
  Barlow, tracked uppercase labels, thin 1px outlines, pill buttons, every colour a token. Scrolling regions use Reka
  UI's ScrollArea with thin bars in that look; the page keeps native scrolling (anchors, search jumps, the outline) with
  the native bar hidden and the same bar drawn over it (Niklas, 15 Oct 2026).
- **Checks** (completeness and consistency): `content-tool wiki-data` fails, and the test
  `the_wiki_is_complete_and_consistent` in `cargo test -p content-tool` fails, when a hand-written page references an
  unknown entry, value or constant, a rule example does not parse or check, a component names missing content, a page
  lacks a title or description or is missing from `nav.json`, or a raster image appears anywhere in `wiki/`; other
  tests check that every kind has a section and every entry a page (and a sidebar and reference entry), that every link
  resolves and that output is deterministic. `npm run check` checks the build: every entry and page exists, every link
  and fragment resolves, nothing raster, every page in the search index.
- The ADR-0032 HTML generator (`content-tool wiki`) is removed; its page logic lives on in `wiki-data`.

## Consequences

- Names and numbers in the wiki cannot drift from the game: a renamed id or a removed stat fails the check, a changed
  value shows on the next build. A new content kind needs a section in `KINDS` (a test enforces it).
- The wiki is a developer and player wiki at once: locked content is shown and marked, as before.
- Sim constants a page needs must be added to `wiki::refs::sim_values`; constants that live only in the client
  crates cannot be referenced (the tool does not depend on Bevy), so pages describe them in words.
- Deploying: `npm run deploy` copies the build to `../serve/wiki` and links it under the game's server
  (`http://<host>:8080/wiki/`).
