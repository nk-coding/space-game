# Content model

How game content is organised so that the game, the tools and a future wiki read the same files.
Written 11 Oct 2026. Code: `crates/data` (schema, loader, validation, stat lines, geometry) and
`crates/content-tool` (CLI). Content: `content/base/`.

## Principles

- **One source.** Every name, description, stat and ship layout lives in `content/` as RON. The game, the
  drydock UI, the sim and the wiki all read it through the `data` crate (or its JSON export).
- **The base game is a mod.** `content/base` is a pack like any other and loads through the same code.
- **Stable ids.** Every entry has a snake_case id, unique per kind. Ids never change once shipped (saves,
  blueprints and mods refer to them); rename the `name` instead.
- **Text in data, keyed.** Inline English text with derived keys (`part.fission_reactor.name`), so a locale
  layer can replace it later without touching the data.
- **No numbers in prose.** Summaries and descriptions describe what a part does; the numbers come from the
  stat lines, so text never goes stale when balance changes.
- **Placeholder numbers are marked.** Every file with balance numbers says so in a comment at the top.

## Pack layout

```
content/base/
  pack.ron                 manifest
  stats.ron                stat descriptors (label, unit, precision, better-direction); order = display order
  categories.ron           part categories; order = palette/wiki order
  factions/*.ron           ship_mind, automatons
  materials/armor.ron      armor materials
  parts/<category>.ron     parts, one file per category (+ automatons.ron for enemy parts)
  ships/<id>.ron           one blueprint per file
  wiki/<kind>/<id>.md      optional long-form prose, e.g. wiki/part/fission_reactor.md
  wiki/page/<id>.md        free-standing wiki articles (first "# " line = title)
```

File rules:

- A kind's data lives in `<dir>.ron` or anywhere under `<dir>/` (any depth), with `<dir>` one of `stats`,
  `categories`, `factions`, `materials`, `parts`, `ships`. File names are free; group however reads best.
- A file holds **one entry** `( ... )` or **a list** `[ ( ... ), ... ]`. Comments (`//`, `/* */`) are fine.
- `Some(...)` may be omitted (`power: (output: 10)`); RON's `implicit_some` is always on.
- Unknown fields are errors (typos fail loudly). Unknown `.ron` files are errors; other files (READMEs) are
  ignored.
- Files are read in sorted path order, entries in file order. That is the **load order**, which is also the
  display order for stats, categories and the drydock palette.
- `wiki/` is for tools only. The game build never embeds it.

## Common header

Every entry of every kind starts with:

| Field | Type | Notes |
|---|---|---|
| `id` | string | snake_case (`a-z`, `0-9`, `_`, starts with a letter), unique per kind |
| `name` | string | display name; for stats it is the label |
| `summary` | string | one sentence: in-game tooltip and wiki lead |
| `description` | optional string | longer in-game text |
| `lore` | optional string | flavour text |
| `tags` | list of strings | snake_case; used by bonus selectors and wiki filters |

## Kinds

### `pack.ron`

```ron
(id: "base", name: "Space Game", version: "0.1.0", schema_version: 1,
 description: "...", authors: ["Niklas"], depends: [])
```

`schema_version` is the content schema the files are written for (`data::SCHEMA_VERSION`, now 1). A
pack with another version is rejected; when the schema changes, the loader gains migrations and the number
goes up. `depends` lists packs that must load earlier.

### Stats (`stats.ron`)

Display information for every number the UI and wiki show: `name` (label), `summary`, `unit` (translatable,
may be empty), `decimals` (maximum; trailing zeros are trimmed), `better` (`Higher`, `Lower`, `Neutral`, for
green/red diffs in the drydock). Which part field feeds which stat id is fixed in code (`data::stats`), so
the game requires these ids: `build_points mass hp power_output power_draw heat_output heat_limit heat_vent
damage range shot_rate shot_speed penetration recoil thrust gimbal rcs_thrust torque shield_radius
shield_capacity shield_regen cargo_capacity rule_slots mass_per_cell hp_per_cell build_point_factor`. `fire_rate` is a bonus-only stat. Units (`t`, `kN`, `kN·m`, `MW`, `°`) are placeholders
except that the sim reads mass in t, thrust in kN and torque in kN·m (so kN / t = m/s²); 1 cell = 1 m.

### Categories (`categories.ron`)

`hull`, `power`, `weapons`, `defense`, `heat`, `propulsion`, `logic`, `cargo`. Optional `color` = system
colour key for the icons of the category's parts (see Art keys).

### Factions (`factions/*.ron`)

`glow`: hull glow key, `"player"` (teal) or `"enemy"` (coral). Ships name their faction.

### Materials (`materials/*.ron`)

Materials with an optional component block. For now only armor:

```ron
(id: "composite", name: "Composite", summary: "...", icon: "armor-composite",
 armor: (tone: "composite", mass_per_cell: 1.5, hp_per_cell: 1.4, build_point_factor: 1.5))
```

A hull part placed in a material gets `mass = mass_per_cell × area`, `hp = hp_per_cell × area` and
`build_points × build_point_factor`; slopes have half the area of their footprint. Fabrication resources
(He-3, chitin, ...) will be materials without an `armor` block, which is why the kind is not called "armor".

### Parts (`parts/*.ron`)

```ron
(
    id: "fission_reactor", name: "Fission reactor",
    summary: "A compact reactor that feeds its piece's shared power pool.",
    description: "Each venting [[part:radiator]] beside it raises its output.",
    tags: ["reactor"], category: "power", icon: "fission-reactor",
    size: (1, 1), build_points: 4, mass: 4, hp: 1,
    power: (output: 10),
    heat: (output: 3, limit: 6),
)
```

| Field | Notes |
|---|---|
| `category` | category id |
| `icon` | icon key = file stem in `art/icons` (`fission-reactor` → `fission-reactor.svg`, `.icon-only.svg`). Hull parts have none: they show their material |
| `color` | optional system colour key overriding the category's (automaton parts use `"enemy"`) |
| `size` | footprint `(w, h)` in cells before rotation |
| `shape` | `Rect` (default) or `Slope` |
| `build_points` | cost against the drydock budget (no economy; balancing and enemy scaling) |
| `hull` | `true`: takes an armor material per placement; then `mass`/`hp` must be absent |
| `mass`, `hp` | required for non-hull parts |

Optional component blocks (a part has any combination):

| Block | Fields | Meaning |
|---|---|---|
| `power` | `output`, `draw` | adds to / draws from the piece's shared pool at full load |
| `heat` | `output`, `limit` | heat at full load; shutdown level (none = never) |
| `radiator` | `vent` | heat dumped per straight side open to space |
| `weapon` | `family` (`Energy Kinetic Missile Disruption Drone Utility`), `damage` (per second), `range` (m), `delivery` (see below, default `Beam`) | fixed gun, fires along the part's forward side, needs a clear line of fire |
| `engine` | `thrust`, `gimbal` (degrees, default 0, at most 45) | pushes along the part's forward side, needs open cells right behind it; can swing its thrust up to `gimbal` degrees to either side (same point of action), which adds torque |
| `gyro` | `torque` (kN·m) | pure reaction torque about the centre of mass, either direction, no thrust. On the gyro part, and as the small built-in turning help of logic cores |
| `rcs` | `thrust` (kN per nozzle) | reaction-control thrusters: one nozzle on every straight side open to space (see `open_sides` below); each pushes the ship away from its side, at the side's centre |
| `shield` | `radius` (m), `capacity` (damage), `regen` (per second while powered), `absorb` (share of damage per family, e.g. `{Energy: 1.0, Kinetic: 0.5}`; missing = passes through) | bubble around the emitter that soaks shots entering it |
| `cargo` | `capacity` | salvage units |
| `logic` | `rule_slots` | hosts a mind, runs behavior rules |
| `bonuses` | list, see below | adjacency bonuses this part grants |

**Weapon delivery** (`weapon.delivery`, added 11 Oct 2026):

| Delivery | Fields | Meaning |
|---|---|---|
| `Beam` | — | continuous hit-scan: `damage` per second while firing, burns the first block it touches |
| `Pulse(rate: r)` | `rate` (shots/s) | hit-scan pulses, each dealing `damage / rate`, first block only |
| `Projectile(rate: r, speed: v, penetration: n, recoil: j)` | shots/s, m/s, blocks, kN·s | a flying shot (inherits the ship's velocity), lives `range / speed` s, goes through up to `n` blocks, pushes the shooter back with `j` per shot; kinetic shots ricochet off slopes hit at a shallow angle |

Stat lines: `range`, `shot_rate`, `shot_speed`, `penetration`, `recoil`, `shield_capacity`, `shield_regen`
(added 11 Oct 2026). The capacitor's `fire_rate` bonus multiplies the shot rate (beams: their damage).

**Adjacency bonuses** are declared on the part that grants them and apply to each **linked** neighbour
(touching through straight sides) that matches `target`:

```ron
// capacitor: lasers beside it fire 15% faster, each laser counts at most 3 capacitors
bonuses: [(target: Tag("laser"), stat: "fire_rate", effect: Percent(15), max_stacks: 3)],
// radiator: +2 power to each reactor beside it, while the radiator vents
bonuses: [(target: Tag("reactor"), stat: "power_output", effect: Add(2), when: [SourceVenting])],
```

`target` is `Part(id)`, `Tag(tag)` or `Category(id)`; `effect` is `Add(x)` (in the stat's unit) or
`Percent(x)`; `max_stacks` limits how many sources count per target; `when` holds conditions
(`SourceVenting`: the granting part has a straight side open to space). Declaring bonuses on the source lets a
mod add a new capacitor or a new laser without editing existing parts. Note: the concept-board drydock gave
the capacitor bonus to every gun; the content gives it to parts tagged `laser`, as the part text says.

**Turning** (decided 11 Oct 2026) comes from three sources the flight computer allocates across: the `gyro`
torque of logic cores and gyro parts, RCS pods, and engines (differential thrust plus `gimbal`). RCS is its own
block rather than an engine mode on purpose: code that handles `engine` (rear exhaust, clear-exhaust rule) must
never mistake an RCS pod for a rear engine, and the wiki can describe it on its own. Power draw of a gyro or RCS
pod is "at full use"; how draw and heat scale with use is up to the sim. The gyro sits in the `propulsion`
category (it moves the ship, draws power like the engines and shares their brown-out tier and colour), not
`logic`.

**Slopes** must be one cell high (`size: (w, 1)`) and hull parts. Steep variants come from rotation.

### Ships (`ships/<id>.ron`)

```ron
(
    id: "halvex_runner", name: "Halvex Runner", summary: "...", faction: "ship_mind",
    size: (12, 7),
    parts: [
        (part: "slope_1x2", x: 3, y: 0, flip: true, material: "composite"),
        (part: "beam_laser", x: 5, y: 0),
        ...
    ],
)
```

`size` is the drydock frame; every placement must fit inside. A placement is `part`, `x`, `y`, `rot`
(default 0), `flip` (default false), `material` (required for hull parts, forbidden otherwise). The index of a
placement in `parts` is the part index used by the geometry API.

Base ships: `halvex_runner`, `automaton_raider` (from `art/ships/automaton-raider.svg`), `sample_early` and
`sample_mid` (from the concept-board drafts).

The Halvex Runner started as an exact conversion of `art/ships/halvex-runner.layout.json`. On 11 Oct 2026 Niklas
decided that its layout must work under the drydock rules. The **content blueprint is now the source of truth**
and `art/ships/halvex-runner.*` stays the original concept art. The adjusted layout keeps the occupied cells, the
spine (beams, fusion reactor, torch drive, thrusters), the slopes and the left/right symmetry. The wing-tip pulse
lasers moved up into the wing shoulders, replacing the two 1×1 slopes that blocked their line of fire, so the
outline differs from the art's `outline_path` only there. The radiators now vent (wing tips and tail row), the
logic cores and shield emitters moved away from the fusion reactor's heat, and two gyros were added. The tests
check exactly that: same cells, outline = art outline except the two shoulder vertices, unchanged spine, mirror
symmetry, and no rule warnings. Every base ship validates without warnings.

## Coordinates and geometry

All in the **blueprint frame**, 1 cell = 1 m:

- `x` grows right, `y` grows **down**. The ship's **forward is up (−y)**.
- Cell `(x, y)` covers `[x, x+1] × [y, y+1]`. Polygon corners are integer points.
- A footprint is `w × h` before rotation. `(x, y)` is the top-left cell of the **rotated** footprint.
- `flip` mirrors left-right **before** rotating; `rot` = quarter turns **clockwise** on screen.
- **Slopes:** a right triangle over `w × 1`. Unrotated, the right angle is bottom-left, the straight sides are
  left and bottom, the face runs from the top-left to the bottom-right corner and faces up-right.
- **Linked cells:** cells of one part always; cells of different parts when both have a straight side on the
  shared edge. Slopes therefore link only through their two straight sides. Linked cells form **pieces**
  (each has its own power pool; cut-off pieces become salvage). The **main piece** is the one with a logic
  core, else the one with most parts.
- **Hull edge** (for the glow): slope faces are always edge; a straight side is edge unless a neighbour in the
  same piece covers it with a straight side.
- **Guns** need every cell straight ahead of their facing side empty, to the edge of the grid. **Engines** need
  the cells directly behind their back side empty. "Facing" is local forward turned by `rot`/`flip`.
- **Open to space**, two rules:
  - `ShipGrid::exposure(cell)` (the drydock's venting rule, used for radiators and heat loss): straight sides
    not linked to a neighbour, plus one for a slope face. A straight side facing a slope's face counts.
  - `ShipGrid::open_sides(part)` (stricter, used for RCS nozzles): straight sides whose neighbouring cell is
    empty. Each entry `(cell, dir)` is one nozzle at `dir.side_center(cell)`; its force points along
    `-dir.normal()`, pushing the ship away from that side.
- Polygons and hull segments wind so the solid is on the right when walking them on screen (positive
  shoelace area with y down).
- **For a y-up renderer (Bevy):** `world = (x, −y)`; rotation is `−rot × 90°` about z; the flip is a mirror in
  local x applied before the rotation. Draw an icon centred on `PlacedShape::center()` at the unrotated
  footprint size, mirrored if `flip`, then turned.

These rules are a port of `docs/concept/reference/drydock-model.js` and `drawHullGlow` in
`concept-board-script.js`; the tests check the raider's hull edge against its SVG outline, the Halvex Runner's
against the concept art's `outline_path` (except the two adjusted wing shoulders), and the drydock-model cases
(slope links, mirroring, rotation).

## References and links

- Fields that name another entry hold its id (`category: "power"`, `faction: "automatons"`,
  `material: "steel"`, `target: Part("capacitor")`); validation checks each target exists.
- Any text field and wiki file may link: `[[kind:id]]` or `[[kind:id|label]]`, kind one of `stat category
  faction material part ship page`. The game renders links as plain names (`Content::render_plain`), the wiki
  as hyperlinks. Broken links are errors.

## Text and localisation

- Keys: `<kind>.<id>.<field>` for `name`, `summary`, `description`, `lore`, and `stat.<id>.unit`.
- `Content::texts()` lists every key with its English text (the template for a translation).
- `Content::apply_texts(&map)` replaces texts by key and returns unknown keys. A later locale file
  (e.g. `locale/de.ron` in a pack, a map key → text) only needs a loader hook calling this.
- English only at launch (decided). Wiki prose is English-only for now.

## Mods

- A mod is a pack directory (or zip) with its own `pack.ron`. Packs load in a given order, base first.
- An entry with an id that already exists **replaces** it completely and keeps its load-order position
  (a mod can rebalance `pulse_laser` by redefining it). New ids are appended.
- Ids defined twice inside one pack are an error.
- Base ships pick up overridden parts automatically, because they reference ids.
- Not yet: partial patches (change one field), removing entries, locale files. Add when a mod needs them.
- Co-op needs matching mod lists (hash of pack ids + versions, later).

## Loading in the game

- The `data` crate never touches the filesystem: `Pack::from_files(iter of (path, contents))`, then
  `Content::load(&[packs...])` (merge + validate). This works on wasm32 and Android.
- Feature `embed` (default) compiles `content/base` (without `wiki/`) into the binary:
  `data::base_content()`, `data::base_pack()` (to combine with mods), `data::base_pack_files()`.
- Web mods come from a picked zip or a URL: unpack into `(path, contents)` and call the same function.
- Feature `fs` (native tools): `data::fs::load_pack_dir(path)`.
- Hot reload in dev: re-read the files, rebuild the packs, swap the `Content`.

## Validation

`Content::load` fails on errors; `data::validate` returns errors and warnings. Each problem names pack, file,
line/column (parse errors), kind, id and field path (e.g. `parts[3].material`).

- Errors: RON syntax and unknown fields, unknown file locations, malformed wiki paths, missing `pack.ron`,
  unsupported `schema_version`, dependency order, packs loaded twice, duplicate ids within a pack, bad id/tag
  format, empty name/summary, dangling references (category, faction, material, part, bonus targets, tags,
  stats), stat ids the code needs but `stats.ron` lacks, unknown colour/tone/glow keys, malformed icon keys
  (and missing icon files when the tool is given `art/icons`), non-finite or out-of-range numbers, hull parts
  with `mass`/`hp`, non-hull parts without them, illegal slopes (not `w × 1`, or not hull), hull placements
  without an armor material, materials on non-hull parts, `rot` outside 0–3, placements outside the frame,
  overlapping placements, wiki pages for missing entries, broken or malformed links.
- Warnings (valid but probably unintended): a ship that is several pieces, a gun without a clear line of fire,
  an engine with a blocked exhaust, an RCS part without any side open to space, a radiator that cannot vent.
- Turning parts: `engine.gimbal` between 0 and 45 degrees, `gyro.torque` and `rcs.thrust` above 0, no `rcs` on
  hull parts.

## The wiki

A future generator reads either the `data` crate directly (Rust) or the JSON export:

```
cargo run -p content-tool -- validate                 # all packs ok? icons exist?
cargo run -p content-tool -- export-json content.json  # resolved content for any external tool
cargo run -p content-tool -- list part
```

`export-json` (`format_version` 1) holds every kind in load order with all fields plus, per entry,
`text_keys`, `links` (targets mentioned in its texts) and `wiki` (markdown or null). Parts add `color_key`,
`stat_lines` (identical to the drydock), `stat_lines_by_material` for hull parts, `geometry` (area, polygon,
connectable sides) and `used_by_ships`. Ships add per-placement `cells`, `polygon`, `center` and `piece`,
plus `bounds`, `pieces`, `main_piece`, `hull_loops` and summed `totals`. Categories list their parts,
factions their ships. `pages` holds free articles; `texts` the whole key → text table.

## Art keys

Content never holds colours. It names keys that `art/tokens.json` resolves:

- system colours (`ship.system_colors`): `power weapons shields heat propulsion logic cargo enemy`
- armor tones (`ship.armor`): `steel composite reactive enemy`
- hull glow: `player` (`ship.hull_glow.color`), `enemy` (`ship.hull_glow.enemy_color`)
- icons: file stems in `art/icons`

The lists live in `data::art_keys`; a new key needs an entry there and in `tokens.json`.

## API quick reference (`data` crate)

```rust
let content = data::base_content();                      // or Content::load(&[base_pack()?, mod_pack])
let ship = content.ships.get("halvex_runner").unwrap();  // Table<T>: get, iter (load order), position
let grid = content.ship_grid(ship)?;                     // ShipGrid; part index = index in ship.parts
grid.shape(i).cells() / .polygon() / .center() / .connectable_sides() / .slope_normal() / .facing()
grid.occupant((x, y)); grid.linked((x, y), Dir::Up); grid.neighbors(i); grid.exposure((x, y));
let pieces = grid.pieces();                              // Pieces { parts, piece_of_part }
let main = grid.main_piece(&pieces, |i| part_of(i).logic.is_some());
let edges = grid.piece_hull_edges(&pieces, main.unwrap()); // Vec<HullSegment>, as drawHullGlow
let loops = data::geometry::hull_loops(&edges);          // closed outlines, collinear points merged
grid.bounds(); grid.line_of_fire_clear(i); grid.exhaust_clear(i); grid.remove(i); grid.shape(i).centroid();
grid.open_sides(i);                                      // Vec<((x, y), Dir)>: RCS nozzles of part i
dir.normal(); dir.side_center((x, y)); dir.side_segment((x, y));  // nozzle point/direction, side corners
content.part_stats(part, material) / content.material_stats(m);  // Vec<StatLine>
content.part_color(part); content.placement_material(&p); content.placement_mass(&p); content.placement_hp(&p);
content.render_plain(text); content.texts(); content.apply_texts(&locale_map);
```
