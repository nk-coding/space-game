# The wiki

A VitePress site in the game's look (`wiki/`, [ADR-0039](adr/0039-vitepress-wiki-from-game-data.md), written 15 Oct
2026; it replaced the static HTML generator of ADR-0032). Two halves:

- **Generated** (`content-tool wiki-data`, code `crates/tools/content-tool/src/wiki/`): a page for every entry of
  every content kind (parts, materials, categories, ships, frames, factions, research, families, facilities, starts,
  voyages, encounters, behaviors, behavior nodes, stats, controls, articles), a list per kind, a reference overview
  and the evolution lab's page (`reference/lab.md`, all of `lab.ron`: a table per list, a row with an anchor per
  entry; GDR-0092 step 8, 16 Oct 2026), plus the data the site's components and Markdown plugin read. Written into `wiki/gen/` (git-ignored).
- **Hand-written** (`wiki/guide/`, `wiki/mechanics/`, `wiki/behavior/`, the home page `wiki/index.md`): the guide, the
  player-facing mechanics and the behavior system. Prose only: every name and number the game's data holds is a
  reference (below), so it can never drift.

It is a player and modder wiki for now: locked content is shown and marked.

## Commands

```
cd wiki
npm install              # once: vitepress and reka-ui (pinned), ~130 MB in node_modules
npm run dev              # content-tool wiki-data, then the dev server (http://localhost:5173/wiki/)
npm run build            # content-tool wiki-data, then the static site in wiki/.vitepress/dist (about 10 s)
npm run check            # checks the build (below)
npm run deploy           # build, check, copy to ../serve/wiki and link it under the game: http://<host>:8080/wiki/
npm run gen              # only content-tool wiki-data (also: cargo run -p content-tool -- wiki-data wiki)
```

- `wiki-data <wiki_dir> [pack_dir...]` loads and validates the packs like `validate` (default `content/base`;
  mods after it in load order), writes `<wiki_dir>/gen/` and checks the hand-written pages. Problems in those pages
  are errors (the command fails after writing, so the site still builds while you fix them). The output is
  deterministic; unchanged files keep their time stamps, so the dev server reloads only what changed. `gen/` holds a
  `.wiki-files` list; the next run removes what it no longer makes and refuses a non-empty directory without one.
- `WIKI_BASE=/other/ npm run build` builds for another base path (default `/wiki/`).
- The 8080 server serves `../serve/game`; `deploy` copies the site to `../serve/wiki` and puts a link
  `serve/game/wiki → ../wiki` there, so the wiki sits beside the game without restarting the server. A game deploy
  that empties `serve/game` drops the link; deploy the wiki again.

## Writing pages

A page is Markdown with front matter (`title`, `description`; quote values with `: `), listed in `wiki/nav.json`
(its section and order in the sidebar). Links between hand-written pages are Markdown links with absolute paths:
`[pilot assists](/behavior/pilot-assists#channels)`.

### References

| Write | Shows |
|---|---|
| `[[part:thruster]]`, `[[part:thruster\|thrusters]]` | a link to the entry's page with its name (or the label); any kind: `stat`, `category`, `faction`, `material`, `part`, `frame`, `ship`, `behavior_node`, `behavior`, `family`, `research`, `facility`, `start`, `voyage`, `encounter`, `control`, `page` |
| `[[part:thruster.thrust]]` | a part's stat as the drydock shows it (`200 kN`); a hull part in a material: `[[part:block_1x1@steel.hp]]` |
| `[[part:capacitor.bonus.fire_rate]]`, `.bonus.<stat>.max_stacks`, `.power_tier`, `.absorb.<family>`, `.salvage`, `.salvage_data`, `.salvage_materials`, `.fragment` | a part's bonus, power tier, shield absorption and salvage |
| `[[material:steel.hp_per_cell]]`, `.salvage`, `.salvage_data` | a material's stats and salvage |
| `[[ship:rim_skiff.top_speed]]` | a design-report stat (`build_points`, `mass`, `hp`, `power_use`, `peak_heat`, `acceleration`, `top_speed`, `turn_time`, `firepower`, `control_load`, `rule_slots`, `cargo_capacity`) |
| `[[control:recall.keys]]` | the default keys as key caps |
| `[[behavior_node:orbit.signature]]`, `[[behavior_node:orbit.default.radius]]` | how a node is written; a parameter's default |
| `[[research:tamed_core.cost]]`, `.requires`, `.unlocks`; `[[frame:frigate.grid]]`; `[[start:rim_awakening.mind_capacity]]`; `[[voyage:yard_approach.sites]]`, `.length`, `.band`, `.opens_after`; `[[encounter:the_warden.wake_radius]]`; `[[behavior:escort.rules]]`; `[[stat:mass.unit]]` | more values |
| `[[galaxy:galaxy.jump_range]]`, `.spacing`, `.home_neighbours`, `.warp`, `.signal_area`, `.trail_step`, `.bands`; `[[band:rim.edge]]`, `.gates`; `[[cluster_type:wreck_drift.radius]]`; `[[trail:rim_gate.length]]`, `.hint`; `[[signal_source:hauler_log.line]]` | the galaxy's values (16 Oct 2026) |
| `[[lab:trial.voyages]]`, `[[lab:size.standard.population]]` | an entry of `lab.ron` (`goal`, `goal_preset`, `kind`, `trial`, `gene`, `gene_preset`, `pilot`, `size`, `measure`), linked to its row on the lab's page with its name, and its values (`summary`, a trial's `kind`, `weight`, `seeds`, `time_limit`, `default`, `target`, `count`, `ships`, `waves`, the arms race's numbers; a pilot's `lock`, `close_to`, `stick`, `fire`; a size's `population`, `generations`; a preset's `weights` or `genes`; a measure's `unit`); `[[lab:spread.design]]`, `[[lab:budget.layer_open]]`, `[[lab:palette.automatons]]` (`wiki::pages::lab`) |
| `[[sim:tick_hz]]` | a simulation constant with its unit, read from the sim's own constants and `SimConfig::default()` (the list: `wiki::refs::sim_values`; add a constant there before using it) |

`wiki::refs` makes the table (`gen/refs.json`); the site's Markdown plugin (`wiki/.vitepress/refs.mts`) fills it in.
Inside inline code (`` `[[part:x]]` ``) nothing is resolved. Constants that live only in the client crates (the
recall warp time, the drydock's heat thresholds, the escort slots) cannot be referenced: the tool does not depend on
Bevy, so describe them in words.

### Rule examples

A fenced block with the language `behavior` or `assists` is parsed and checked by the game's own parser and
vocabulary (`data::behavior::check_text_as`) and shown as its graph (as the BEHAVIOR screen's GRAPH view draws it)
over its text with every node linked. An example that does not check fails `wiki-data`.

### Components

`<PartCard id="thruster" />`, `<PartGrid category="propulsion" />` (all parts without `category`),
`<ShipCard id="rim_skiff" />`, `<ShipGallery />`, `<ControlsTable section="flight" />` (sections as in
`controls.ron`), `<NodeTable role="condition" />` (`target`, `condition`, `action`; `pilot` for the pilot assists'
own nodes, `tag="…"` for a tag), `<StatTable stats="thrust,rated_speed,gimbal" />` (sortable), `<PartsTable />`
(sortable, filterable), `<KindGrid />`, `<ScrollBox x>` (a sideways-scrolling region). They read `gen/ui.json`;
`wiki-data` checks their attributes against the content.

### Diagrams

Inline `<svg class="diagram" viewBox="…">` in the page, no `width`/`height`, no blank lines inside, colours only
through the theme's classes (strokes `s-teal s-gold s-coral s-violet s-blue s-dim s-label s-number s-ring`, fills
`f-…` likewise plus `f-tile f-steel f-bg`, `.line`, `.dotted`, `.thick`, `.glow`; text `num`, `big`, `t-teal`, …),
arrowheads `marker-end="url(#arrow-teal)"` (`#arrow`, `-gold`, `-coral`, `-violet`) from the theme. No `<a>` inside an
SVG (VitePress's link prefetching cannot read one). No raster images anywhere: SVG, CSS and fonts only.

## What the generated pages show

The same as ADR-0032's pages, now as VitePress pages: every entry's texts (with its links and the pack's prose for
it), an infobox with its drawing, facts and stats, and sections that link both ways (a part's ships, bonuses given and
received, research that unlocks it, salvage; a material's parts, ships and salvage; a ship's parts, design report,
pilot assists (as graph and text, without a target line: the graph's TARGET reads YOUR LOCK) and encounters; a
behavior's rules as graph and text, its nodes, ships and factions; a node's
parameters, Rhai script and behaviors; research costs, requirements and unlocks; voyages, sites, encounters and what
opens what; a control's keys and its section; the galaxy's bands, cluster types, trails, signal sources and lore
fragments, linked both ways), "Mentioned on" back-links and the data details for modders (id, defining file, text
keys, prose file).

- **The galaxy's page** (16 Oct 2026) shows an example: the galaxy generated for seed 1 by the `galaxy` crate (ADR-0042)
  drawn as SVG (`wiki::map`, also `content-tool galaxy --svg`): band edges dotted, home and gates as diamonds, the
  starting jump range, each trail from its first source to its reward coloured by what it leads to, the rim's signal
  areas. A generator that fails on the content is a wiki warning, so the completeness test fails too. The hand-written
  page `mechanics/galaxy` explains bands, S, home, gates, trails, signals and lore from the data.

- **Parts are drawn as technical drawings** from the data: the tile as the drydock's palette shows it on its cell
  grid, the sides that link to neighbours, and what the part does: thrust out of the forward side with the gimbal's
  swing, the plume behind (length labelled), the line of fire, RCS nozzles and radiator vents on the open sides,
  torque, the cells its blast reaches (each cell whose nearest point is within the blast radius of the part's centre),
  the shield's reach.
- Ships are drawn from the blueprint geometry with the hull glow of their faction, as the flight view draws them
  (GDR-0069): armour tiles, and machines as their housings (the pack's `icons/<key>.ship.svg`, an engine's nozzle
  reaching past its stern) on a cell in the hull's tone, by the game's own rule (`data::design::tone`, 15 Oct 2026).
  Part pages and tables keep the palette's tiles with their icons. Barrels, muzzle glows and nav lights are not
  drawn.
- Colours come from `art/tokens.json` (`gen/tokens.css`), the icons and ship drawings from the pack (`gen/sprite.svg`,
  inlined once per page), the font is Barlow from `crates/client/kit/assets/fonts` (with its licence).

## The look

`wiki/.vitepress/theme/`: VitePress's default theme for the structure (sidebar, outline, prev/next, local search,
phone layout) restyled in `style.css` after `docs/style-guide.md`: the radial backdrop with dotted rings and faint
particles (drawn once per page), Barlow, tracked uppercase grey labels, white numbers with a slight glow, thin 1px
outlines and dotted rules, pill buttons, teal for what you act on. Forced dark. No rule beside the sidebar's entries;
the active one gets a teal bar.

**Scrollbars** (Niklas, 15 Oct 2026): every scrolling region is a `ScrollBox` (Reka UI's ScrollArea: thin bars over
the edge, a dim track, the thumb grey, teal under the pointer and while dragged): the sidebar, the outline (also the
phone's outline menu and nav screen), the search results, wide tables, code blocks, rule graphs and examples. Those
default-theme components are copies in `theme/overrides/` that wrap their scrolling part, swapped in by aliases in
`config.mts` (VitePress 1.6.4; check them when updating VitePress). The page itself keeps the browser's own
scrolling, because VitePress's anchor jumps, search jumps and active-heading tracking follow the window; its native
bar is hidden and `PageScrollbar.vue` draws the same thin bar over the right edge (draggable; a click on the track
pages). A global rule hides any other native bar.

## Checks

- `content-tool wiki-data` (and the test `the_wiki_is_complete_and_consistent` in `cargo test -p content-tool`, which
  runs it over the repository's `wiki/`): every reference resolves, every rule example checks, every component names
  existing content, every page has a title and a description and is in `nav.json` (and every `nav.json` entry has a
  page), no raster image (a raster file anywhere in `wiki/`, an `<img>`, a raster `url(...)`, `src` or data URI).
  Every entry of the evolution lab's documented lists (every trial kind, trial, goal, goal preset, gene, gene preset,
  stand-in pilot and size: `pages::LAB_DOCUMENTED`) must be named on some hand-written page (`[[lab:<list>.<id>]]` or
  one of its values); the lab's pages are `mechanics/evolution-lab.md` and `mechanics/evolution-trials.md`.
- The other tests of `crates/tools/content-tool/src/wiki/tests.rs`: every kind has a section (`KINDS` covers
  `Kind::ALL`), every entry a page, a sidebar entry and a reference; every link in the pages and data resolves; two
  runs are identical; generated pages are blocks Vue leaves alone; pages link both ways; locked content is marked;
  drawings are vector and use the pack's icons; a mod with markup in every text is escaped everywhere.
- `npm run check` on the build: every entry and hand-written page has its page (a lab entry its row's anchor),
  every internal link and fragment resolves, no raster file, `<img>` or raster `url()`, and every page is in the
  search index.

## Not yet

Translated wikis (prose is English only, ADR-0018), a tag index, pictures of research trees and the voyage map, and
screenshots of the game (they would be raster).
