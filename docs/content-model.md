# Content model

How game content is organised so that the game, the tools and a future wiki read the same files.
Written 11 Oct 2026. Code: `crates/data` (schema, loader, validation, stat lines, geometry, the blueprint text
format, the drydock's and the behavior editor's editing models) and `crates/content-tool` (CLI). Content:
`content/base/`.

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
  frames.ron               drydock frames (the design grid sizes); order = drydock order
  ships/<id>.ron           one blueprint per file
  behavior_nodes/*.ron     the behavior vocabulary: conditions, actions, target selectors (+ Rhai conditions)
  behaviors/<id>.ron       behavior programs (a target line and rules in the text format)
  icons/<key>.svg          part, material and category icons as SVG (the glyph without its tile)
  wiki/<kind>/<id>.md      optional long-form prose, e.g. wiki/part/fission_reactor.md
  wiki/page/<id>.md        free-standing wiki articles (first "# " line = title)
```

File rules:

- A kind's data lives in `<dir>.ron` or anywhere under `<dir>/` (any depth), with `<dir>` one of `stats`,
  `categories`, `factions`, `materials`, `parts`, `frames`, `ships`, `behavior_nodes`, `behaviors`. File names
  are free; group however reads best.
- A file holds **one entry** `( ... )` or **a list** `[ ( ... ), ... ]`. Comments (`//`, `/* */`) are fine.
- `Some(...)` may be omitted (`power: (output: 10)`); RON's `implicit_some` is always on.
- Unknown fields are errors (typos fail loudly). Unknown `.ron` files are errors; `.svg` files belong in
  `icons/` as `<key>.svg` with a kebab-case key; other files (READMEs) are ignored.
- Files are read in sorted path order, entries in file order. That is the **load order**, which is also the
  display order for stats, categories, frames and the drydock palette.
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
the game requires these ids: `build_points mass hp power_output power_draw power_min_level heat_output heat_limit heat_vent
damage range shot_rate shot_speed penetration recoil thrust rated_speed gimbal plume_length plume_damage plume_heat
rcs_thrust rcs_rated_speed torque
shield_radius shield_capacity shield_regen cargo_capacity rule_slots control_cost control_capacity mass_per_cell
hp_per_cell build_point_factor`. `fire_rate` is a bonus-only stat. Units (`t`, `kN`, `kN·m`, `MW`, `°`) are
placeholders except that the sim reads mass in t, thrust in kN, torque in kN·m (so kN / t = m/s²) and rated
speeds in m/s; 1 cell = 1 m.

**Ship stats** (added 12 Oct 2026). The drydock's stats panel shows ship-level values that the sim's design
report computes; their labels and units live in `stats.ron` too, and the game requires these ids
(`data::stats::SHIP_STATS`, a separate list from the part stats `BUILTIN_STATS`):

| id | Label | Unit | Better | Shown as |
|---|---|---|---|---|
| `power_use` | Power | MW | Neutral | used / produced |
| `peak_heat` | Peak heat | % | Lower | hottest part against its limit |
| `acceleration` | Thrust/mass | m/s² | Higher | |
| `top_speed` | Top speed | m/s | Higher | |
| `turn_time` | Turn 90° | s | Lower | |
| `yaw` | Yaw | — | Neutral | |
| `firepower` | Firepower | /s | Higher | damage per second of all guns |
| `control_load` | Control | — | Neutral | load / capacity |
| `device_load` | Device load | — | Neutral | |

The panel shows build points, mass, hit points, cargo and rule slots under their part stat ids (`build_points`,
`mass`, `hp`, `cargo_capacity`, `rule_slots`), summed by `Content::ship_totals`. `Content::stat_line(id, value)`
formats any value of any stat the way part stat lines are formatted. Precision and the summaries' exact
definitions are placeholders until the design report settles them.

### Categories (`categories.ron`)

`hull`, `power`, `weapons`, `defense`, `heat`, `propulsion`, `logic`, `cargo`. Optional `color` = system
colour key for the icons of the category's parts (see Art keys).

### Factions (`factions/*.ron`)

`glow`: hull glow key, `"player"` (teal) or `"enemy"` (coral). Ships name their faction. `behavior` (optional):
the default behavior of the faction's ships (`automatons` → `raider_basic`, `ship_mind` → `skirmisher`).

### Materials (`materials/*.ron`)

Materials with an optional component block. For now only armor:

```ron
(id: "composite", name: "Composite", summary: "...", icon: "armor-composite",
 armor: (tone: "composite", mass_per_cell: 1.5, hp_per_cell: 1.4, build_point_factor: 1.5))
```

A hull part placed in a material gets `mass = mass_per_cell × area`, `hp = hp_per_cell × area` and
`build_points × build_point_factor`; slopes have half the area of their footprint. Fabrication resources
(He-3, chitin, ...) will be materials without an `armor` block, which is why the kind is not called "armor".

`locked` (optional, default `false`, added 13 Oct 2026), on materials and parts alike: not available to the
player until unlocked (salvage, research). The drydock hides locked entries (palette, armor pills, the 1–9 keys);
content ships that use them still load, open in the drydock and fly. The base pack locks the automaton core,
the automaton cannon and Automaton plate (Niklas, 13 Oct 2026). Nothing unlocks yet; that comes with salvage and
research.

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
| `icon` | icon key: `fission-reactor` → `icons/fission-reactor.svg` in any loaded pack (see [Icons](#icons)). Hull parts have none: they show their material |
| `color` | optional system colour key overriding the category's (automaton parts use `"enemy"`) |
| `size` | footprint `(w, h)` in cells before rotation |
| `shape` | `Rect` (default) or `Slope` |
| `build_points` | cost against the drydock budget (no economy; balancing and enemy scaling) |
| `control_cost` | optional: what the part costs against its vessel's control capacity (GDR-0044, see below); default by kind |
| `hull` | `true`: takes an armor material per placement; then `mass`/`hp` must be absent |
| `mass`, `hp` | required for non-hull parts |
| `locked` | optional, default `false`: not available to the player until unlocked (salvage, research); the drydock hides it. Content ships may still use it (the raider flies its locked automaton parts) |

Optional component blocks (a part has any combination):

| Block | Fields | Meaning |
|---|---|---|
| `power` | `output`, `draw`, `tier`, `weight`, `min_level` | adds to / draws from the piece's shared pool at full load; consumers also give their allocation defaults (see below) |
| `heat` | `output`, `limit` | heat at full load; shutdown level (none = never) |
| `radiator` | `vent` | heat dumped per straight side open to space |
| `weapon` | `family` (`Energy Kinetic Missile Disruption Drone Utility`), `damage` (per second), `range` (m), `delivery` (see below, default `Beam`) | fixed gun, fires along the part's forward side, needs a clear line of fire |
| `engine` | `thrust`, `rated_speed` (m/s, required), `gimbal` (degrees, default 0, at most 45), `plume: (length, damage, heat)` (required, see below) | pushes along the part's forward side and may sit anywhere (GDR-0039); can swing its thrust up to `gimbal` degrees to either side (same point of action), which adds torque. While it pushes with the ship's motion, its thrust fades out as the ship's whole speed approaches `rated_speed` (full up to 80%, none at 100%; braking and sideways it keeps full thrust), so the fastest engines set the ship's top speed (GDR-0041, GDR-0055). Its exhaust plume burns the first part behind it |
| `gyro` | `torque` (kN·m) | pure reaction torque about the centre of mass, either direction, no thrust. On the gyro part, and as the small built-in turning help of logic cores |
| `rcs` | `thrust` (kN per nozzle), `rated_speed` (m/s, required) | reaction-control thrusters: one nozzle on every straight side open to space (see `open_sides` below); each pushes the ship away from its side, at the side's centre, and fades out towards `rated_speed` like an engine; two nozzles of opposite sides fire as a turning couple that pushes nothing and keeps full thrust at any speed (GDR-0055) |
| `shield` | `radius` (m), `capacity` (damage), `regen` (per second while powered), `absorb` (share of damage per family, e.g. `{Energy: 1.0, Kinetic: 0.5}`; missing = passes through) | bubble around the emitter that soaks shots entering it |
| `cargo` | `capacity` | salvage units |
| `logic` | `rule_slots`, `control_capacity` | hosts a mind, runs behavior rules and up to `control_capacity` of control cost (GDR-0038 "N") |
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

**Power allocation** (GDR-0014, fields added 12 Oct 2026). A part that draws power declares how it is fed
when its piece runs short:

```ron
power: (draw: 9, tier: Low, min_level: 0.5),   // beam laser: fed after everything else; off below 50%
```

| Field | Default | Meaning |
|---|---|---|
| `tier` | `Normal` | `Critical`, `High`, `Normal`, `Low` or `Off`: tiers are served in this order, `Off` never draws |
| `weight` | 1 | share inside the one tier that cannot be fully served (weight 2 runs at twice the level of weight 1); above 0, at most 100 |
| `min_level` | 0 | below this share of its draw the part switches off instead of running at partial output (0..1); shown as the stat line `power_min_level` |

`tier` and `weight` are defaults: a blueprint placement may override them (see Ships), and the sim can change
them at runtime. `min_level` belongs to the part type. The exact rule is in `docs/sim.md` "Power". Base content
(lead defaults, placeholders): logic core `Critical`; shield emitter `High`, minimum 25%; propulsion `Normal`;
guns `Low`, beam laser minimum 50%.

**Control** (GDR-0038, GDR-0044; fields added 12 Oct 2026). Every logic core runs up to its
`logic.control_capacity` of control cost; a vessel's capacity is the sum over its running cores, its load the
`control_cost` of its parts. Parts beyond the capacity go idle in the power brown-out order (the sim's rule:
`docs/sim.md` "Vessels").

```ron
control_cost: 2,                                 // a big machine (fusion reactor, torch drive, beam laser)
logic: (rule_slots: 6, control_capacity: 16),    // the logic core
```

| Field | Default | Meaning |
|---|---|---|
| `control_cost` (part) | 0 for hull parts, logic cores and parts without a power role (radiators, capacitors, cargo); 1 for parts that draw or make power | control cost against the vessel's capacity, at most 100; shown as the stat line `control_cost` (only when above 0) |
| `logic.control_capacity` | required | control cost one core of this type runs; stat line `control_capacity` |

Passive parts cost 0 because idling has nothing to switch off on them (validation warns about a cost on a part
without a power role). Base content (lead placeholders, 12 Oct 2026): logic core 16, automaton core 4; fusion
reactor, torch drive and beam laser cost 2; every base ship runs all its parts (Halvex Runner 18 of 32, raider
2 of 4, sample early 8 of 32, sample mid 16 of 32). `Content::ship_control(ship)` gives `(load, capacity)` of an
intact ship's main piece.

**Turning** (decided 11 Oct 2026) comes from three sources the flight computer allocates across: the `gyro`
torque of logic cores and gyro parts, RCS pods, and engines (differential thrust plus `gimbal`). RCS is its own
block rather than an engine mode on purpose: code that handles `engine` (rear exhaust, plume) must never mistake an
RCS pod for a rear engine, and the wiki can describe it on its own. Power draw of a gyro or RCS
pod is "at full use"; how draw and heat scale with use is up to the sim. The gyro sits in the `propulsion`
category (it moves the ship, draws power like the engines and shares their power tier and colour), not
`logic`. Since 13 Oct 2026 (GDR-0055) a logic core's torque is tiny, gyros are moderate and RCS pods are the
strong turners (two nozzles of opposite sides fire as a couple that turns without pushing); differential thrust
and gimbals are the flight computer's last resort. Numbers: `docs/sim.md` "Turning and speed of the base ships".

**Rated speed** (decided 12 Oct 2026, GDR-0041): every `engine` and `rcs` block has a `rated_speed`; there is no
global speed cap and no drag. The stat lines are `rated_speed` (engines) and `rcs_rated_speed` (RCS nozzles), both
labelled "Rated speed", so bonuses can raise them per part type (e.g. `(target: Tag("engine"), stat:
"rated_speed", effect: Percent(10))`) and the wiki shows them. Base values (placeholders): thruster 20 m/s, torch
drive 22 m/s, RCS pod 14 m/s. A thruster pushing with the ship's motion fades by the ship's whole speed
(GDR-0055), so the fastest engines set the top speed. Engine thrust was set for the agility target of GDR-0042,
torque and RCS thrust for GDR-0055.

**Plume** (GDR-0039, GDR-0046, GDR-0049; added 12 Oct 2026). Engines may sit anywhere; there is no open-space
rule. Every `engine` block has a required `plume`:

```ron
engine: (thrust: 700, rated_speed: 22, gimbal: 3, plume: (length: 6, damage: 3, heat: 4)),   // torch drive
```

| Field | Meaning |
|---|---|
| `length` | how far the exhaust reaches behind the engine's back side, m (0 = no plume; at most 50, `data::validate::MAX_PLUME_LENGTH`) |
| `damage` | damage per second at full thrust to the first part in its path |
| `heat` | heat per second at full thrust into that part (same units as `heat.output`) |

The plume runs straight back from the engine, one lane per cell of its back side (the numbers are shared over
the lanes), and burns the first part in each lane: the ship's own, an enemy's or a wreck's, never another vessel
of the same owner. Damage and heat scale with the thrust the engine actually gives (none when idle, next to none
at its rated speed); enemy shield bubbles absorb it like energy damage. The sim's rule is in `docs/sim.md` "Engine
plumes". Stat lines `plume_length` (m), `plume_damage` (/s), `plume_heat` (/s), all labelled "Plume ..." and
`Neutral` (a longer or hotter plume is not simply better). Base values (placeholders, lead, 12 Oct 2026): thruster
3 m, 1 /s, 1.5 /s; torch drive 6 m, 3 /s, 4 /s. RCS pods have no plume.

**Slopes** must be one cell high (`size: (w, 1)`) and hull parts. Steep variants come from rotation.

### Frames (`frames.ron`, added 12 Oct 2026)

Drydock frames set the grid size a design is built on (GDR-0006: frigate, cruiser and capital frames, bigger ones
with drydock expansions). The common header plus `size`:

```ron
(id: "frigate", name: "Frigate", summary: "The first drydock frame: room for scouts, escorts and raiders.", size: (24, 24))
```

| Field | Notes |
|---|---|
| `size` | grid `(w, h)` in cells; each side 1 to 128 (`data::validate::MAX_FRAME_SIZE`, a placeholder) |

Load order is the drydock's display order (`Content::frames`, a `Table<Frame>`); mods override and add frames by
id like any other kind. Base frames (lead placeholders, 12 Oct 2026): `frigate` 24×24, `cruiser` 40×40,
`capital` 64×64. Text keys `frame.<id>.name` etc., links `[[frame:id]]`, wiki prose `wiki/frame/<id>.md`.
Blueprints keep their own `size` and name no frame (v1); every base ship fits the frigate frame.

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

`size` is the blueprint's own grid (its frame); every placement must fit inside. It does not refer to a `frames`
entry (v1). A placement is `part`, `x`, `y`, `rot`
(default 0), `flip` (default false), `material` (required for hull parts, forbidden otherwise) and `power`
(optional, parts that draw power only): the player's override of the part's power tier and weight, e.g.
`(part: "shield_emitter", x: 2, y: 6, power: (tier: Low))` or `power: (tier: High, weight: 2)`; either field
may be left out. The index of a placement in `parts` is the part index used by the geometry API. `behavior` (optional) names the behavior the
ship flies when nobody pilots it; default: its faction's (`Content::ship_behavior`).

Base ships: `halvex_runner`, `automaton_raider` (from `art/ships/automaton-raider.svg`), `sample_early` and
`sample_mid` (from the concept-board drafts).

**Blueprint text** (added 12 Oct 2026, for the drydock). Saved player designs and exported blueprints use exactly
the format of the content ships, so an export can go into a pack's `ships/` folder as is:

- `data::blueprint_to_ron(&blueprint)` writes one blueprint in the style of `content/base/ships/*.ron`: the header
  fields in the order `id`, `name`, `summary`, `description`, `lore`, `tags`, `faction`, `behavior`, then `size`
  and `parts: [` with one placement per line (`part`, `x`, `y`, `rot`, `flip`, `material`, `power`), four-space
  indent. Defaults are left out: `rot: 0`, `flip: false`, absent `material`/`power`/`description`/`lore`/
  `behavior`, empty `tags`; a `power` override writes only the fields it sets. `source` is never written.
  Strings escape `"`, `\` and control characters (`\n`, `\t`, `\u{..}`) and keep other unicode as is; floats
  are written as the shortest text that reads back to the same value. The output is deterministic; a test
  checks that it reproduces every base ship file byte for byte (after its leading comments).
- `data::parse_blueprint(text)` reads one blueprint with the pack loader's options (implicit `Some`, unknown
  fields are errors, `source` is not a readable field, comments are fine). It checks syntax and schema only;
  `data::validate_ship` checks the result against the content.
- Writing and reading back gives an equal blueprint (tested on every base ship, on power overrides, on every
  `rot`/`flip`, and on quotes, backslashes, line breaks, control characters and unicode in every string).

**Ship totals** (added 12 Oct 2026). `Content::ship_totals(&blueprint)` returns `ShipTotals`: the per-part stat
values that add up, summed over every placement (connected or not): `build_points` (hull parts times their
material's factor), `mass`, `hp`, `power_output`, `power_draw`, `thrust`, `cargo_capacity`, `rule_slots`,
`control_cost`, `control_capacity`, plus `logic_cores` (placements with a `logic` block). Bonuses and sim-level
numbers (net power, heat, thrust per direction) are not in it; the main piece's rule slots and control are
`ship_rule_slots` and `ship_control`. It works on invalid designs too (unknown parts are skipped). The export's
ship `totals` come from it.

The Halvex Runner started as an exact conversion of `art/ships/halvex-runner.layout.json`. On 11 Oct 2026 Niklas
decided that its layout must work under the drydock rules. The **content blueprint is now the source of truth**
and `art/ships/halvex-runner.*` stays the original concept art. The adjusted layout keeps the occupied cells, the
spine (beams, fusion reactor, torch drive, thrusters), the slopes and the left/right symmetry. The wing-tip pulse
lasers moved up into the wing shoulders, replacing the two 1×1 slopes that blocked their line of fire, so the
outline differs from the art's `outline_path` only there. The radiators now vent (wing tips and tail row), the
logic cores and shield emitters moved away from the fusion reactor's heat, and two gyros were added. The tests
check exactly that: same cells, outline = art outline except the two shoulder vertices, unchanged spine, mirror
symmetry, and no rule warnings. Every base ship validates without warnings.

### Behavior nodes (`behavior_nodes/*.ron`, added 11 Oct 2026)

The **vocabulary** of the behavior language: every condition, action and target selector a rule can use. The
behavior editor's palette and the wiki list exactly these entries. The sim implements every built-in id
(`sim::behavior::BUILTIN_NODES`); loading checks both directions (a content node without an implementation, an
implementation without a content node, or a mismatch in role, value type, parameter types or choice options is
an error), so text, wiki and code cannot drift apart. The grammar is in `docs/behavior-format.md`.

```ron
(
    id: "orbit", name: "Orbit", role: Action, tags: ["move"],
    summary: "Circle the target at the given radius.",
    params: [
        (id: "radius", name: "Radius", summary: "Metres, or a share of your weapon reach.", type: Distance,
         default: "60%", min: 1, max: 5000),
        (id: "dir", name: "Direction", type: Choice([
            (id: "auto", name: "Auto"), (id: "ccw", name: "Counter-clockwise"), (id: "cw", name: "Clockwise"),
        ]), default: "auto"),
    ],
)
```

| Field | Notes |
|---|---|
| `role` | `Condition` (before the arrow, violet), `Action` (after it, teal), `Target` (the `target` line) |
| `value` | conditions only: `Distance`, `Percent`, `Seconds` or `Count` for a value that rules compare (`hull < 30%`); absent = a flag used bare (`shield_down`) |
| `params` | positional parameters, at most 4: `id` (snake_case; also the Rhai variable name), `name`, optional `summary`, `type`, optional `default`, `min`, `max` |
| `params[].type` | `Distance` (metres `45m`/`45`, or `60%` = a share of the weapon reach), `Percent` (always `%`), `Seconds`, `Count`, or `Choice([(id, name, summary?), ...])` |
| `params[].default` | written as in a rule (`"67%"`, `"45m"`, `"cw"`); parameters without a default come first and are required |
| `params[].min`, `max` | range of the number as written in the parameter's unit (m, %, s, count); reach-relative distances are checked against 0–1000% |
| `script` | Rhai source: makes the node a custom (flag) condition, tagged "Rhai" in the editor; see `docs/behavior-format.md` |

Text keys: the header fields as usual, plus `behavior_node.<id>.params.<param>.name` / `.summary` and
`behavior_node.<id>.params.<param>.options.<option>.name` / `.summary`.

Base vocabulary (v1): conditions `hull`, `shield`, `shield_down`, `heat`, `power`, `target_in_range`,
`target_hull`, `target_on_me`, `under_attack`, `enemies_near`, `cycle`, `target_time`, and the Rhai condition
`closing_in`; actions `approach`, `orbit`, `kite`, `face`, `flee`, `hold`; target selectors `nearest`,
`weakest`, `threat`, `attacker`, `locked`, each with an aim point (`centre`, `engines`, `weapons`, `power`,
`core`).

### Behaviors (`behaviors/*.ron`, added 11 Oct 2026)

A behavior program. `target` and every entry of `rules` are lines of the text format, exactly as the editor's
text view shows them (the RON form *is* the text form: one string per rule, the target line without its
keyword), so there is no second representation to keep in sync and errors point at a rule and a column.

```ron
(
    id: "raider_basic", name: "Raider pack", tags: ["automaton", "attack"],
    summary: "Close in and shoot, then strafe around the target for a few seconds, and again.",
    target: "nearest",
    rules: [
        "when target_in_range(120%) and cycle(9s) >= 5s -> orbit(67%)",
        "always -> approach(85%)",
    ],
)
```

`target` is optional (default `nearest`); a rule may end in a `# note`. Rules are written in canonical form
(the printer's output; a test checks the base pack). Base behaviors: `raider_basic` (the automatons' default,
two rules for one automaton core; it replaced the hard-coded raider brain) and `skirmisher` (the ship-mind's
default for unpiloted ships, six rules, uses the Rhai condition).

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
- **Guns** need every cell straight ahead of their facing side empty, to the edge of the grid. **Engines** may sit
  anywhere (GDR-0039; until 12 Oct 2026 they needed the cells directly behind their back side empty): their plume
  leaves through the back side and burns what is there. "Facing" is local forward turned by `rot`/`flip`.
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
  faction material part frame ship behavior_node behavior page`. The game renders links as plain names (`Content::render_plain`), the wiki
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
- Icons work the same way by key: a mod's `icons/pulse-laser.svg` replaces the base icon, new keys are added.
- Not yet: partial patches (change one field), removing entries, locale files. Add when a mod needs them.
- Co-op needs matching mod lists (hash of pack ids + versions, later).

## Loading in the game

- The `data` crate never touches the filesystem: `Pack::from_files(iter of (path, contents))`, then
  `Content::load(&[packs...])` (merge + validate). This works on wasm32 and Android.
- Feature `embed` (default) compiles `content/base` (`.ron` and `icons/*.svg`, without `wiki/`) into the
  binary: `data::base_content()`, `data::base_pack()` (to combine with mods), `data::base_pack_files()`.
- Web mods come from a picked zip or a URL: unpack into `(path, contents)` and call the same function.
- Feature `fs` (native tools): `data::fs::load_pack_dir(path)` (reads `.ron`, `.md` and `.svg`).
- Hot reload in dev: re-read the files, rebuild the packs, swap the `Content`.

## Validation

`Content::load` fails on errors; `data::validate` returns errors and warnings. Each problem names pack, file,
line/column (parse errors), kind, id and field path (e.g. `parts[3].material`).

`data::validate_ship(&content, &blueprint)` (added 12 Oct 2026, for the drydock) runs exactly the checks
`validate` runs on a content ship (header and ship rules, the same code) on one blueprint that need not be in the
content, such as a design being edited or a file being imported. Its problems carry kind `ship`, the blueprint's
id and the field paths (`parts[k]`, `parts[k].material`, `parts[k].power.weight`, `size`, `faction`, ...), so the
editor can mark the placement; `source` is set only for a blueprint loaded from a pack. Placement errors (unknown
part, material, `rot`, power override, outside the frame, overlap) end the check of that ship in both functions:
the ship-wide rules after them (rule slots, control load, pieces, line of fire, RCS and radiator exposure) need a
clean grid and report nothing until the placements are fixed. Header errors do not hold them back.

- Errors: RON syntax and unknown fields, unknown file locations, malformed wiki paths, missing `pack.ron`,
  unsupported `schema_version`, dependency order, packs loaded twice, duplicate ids within a pack, bad id/tag
  format, empty name/summary, dangling references (category, faction, material, part, bonus targets, tags,
  stats), stat ids the code needs but `stats.ron` lacks, unknown colour/tone/glow keys, malformed icon keys,
  icon keys (parts, materials, categories) with no `icons/<key>.svg` in any loaded pack, `.svg` files outside
  `icons/`, icons that are not usable SVG (see [Icons](#icons)), non-finite or out-of-range numbers, hull parts
  with `mass`/`hp`, non-hull parts without them, illegal slopes (not `w × 1`, or not hull), hull placements
  without an armor material, materials on non-hull parts, `rot` outside 0–3, placements outside the frame,
  overlapping placements, frame sides outside 1–128 cells, wiki pages for missing entries, broken or malformed
  links, a locked paint block or bevel slope (the first 1×1 hull part of each shape in content order, which the
  drydock paints, fills and bevels with: `Content::unit_hull_part`), every armor material locked.
- Warnings (valid but probably unintended): a ship that is several pieces, a gun without a clear line of fire,
  an RCS part without any side open to space, a radiator that cannot vent, an icon nothing uses, an empty
  placement `power` override. (An engine with parts behind it is no warning since GDR-0039: its plume burns them.)
- Power: `power.weight` above 0 and at most 100 (`data::MAX_POWER_WEIGHT`), `power.min_level` 0 to 1, `tier`,
  `weight` and `min_level` only on parts that draw power; a placement `power` override only on such parts, with
  the same weight range.
- Turning parts: `engine.gimbal` between 0 and 45 degrees, `gyro.torque` and `rcs.thrust` above 0, no `rcs` on
  hull parts.
- Rated speed: `engine.rated_speed` and `rcs.rated_speed` are required (a missing one is a parse error) and
  above 0.
- Plume: `engine.plume` is required (a parse error when missing); `length`, `damage` and `heat` are finite and not
  negative, `length` at most 50 m.
- Control: `control_cost` at most 100 (`data::MAX_CONTROL_COST`). Warnings: a `control_cost` above 0 on a part
  that draws and makes no power (it can never idle), a logic core with `control_capacity` 0, a ship whose control
  load exceeds its cores' capacity (the lowest-priority parts start idle).
- Behavior nodes: ids and option ids not keywords (`when if always and or not target`), unique parameter and
  option ids, defaults that parse and fit their type and range, required parameters before optional ones,
  `value` only on conditions, `script` only on flag conditions.
- Behaviors: every rule parses (column-exact syntax errors), names a node of the right role (with a
  "did you mean" suggestion), compares value conditions and only those, passes values of the right type and
  range, and at least one rule. Unknown behavior references on factions and ships. A ship whose behavior has
  more rules than its logic cores give rule slots is an error (field `behavior`, or `faction` for a faction
  default).
- In the sim (`BehaviorLibrary::new`, `sim::behavior::check_vocabulary`): content nodes and the sim's
  implementations agree, Rhai scripts compile (syntax, unknown variables, no `**`), script parameters do not
  shadow the script API.

## The wiki

`content-tool wiki <out_dir> [pack_dir...]` generates the wiki (added 13 Oct 2026; `docs/wiki.md`, ADR-0032): a
static HTML site with a page per entry, stats labelled from `stats.ron`, the pack's icons, ships drawn from their
blueprints and the prose of `wiki/`. It reads the `data` crate directly; other tools can use the JSON export:

```
cargo run -p content-tool -- validate                 # all packs ok (icons included)?
cargo run -p content-tool -- wiki /tmp/space-wiki      # the static wiki (docs/wiki.md)
cargo run -p content-tool -- export-json content.json  # resolved content for any external tool
cargo run -p content-tool -- list part
```

`export-json` (`format_version` 1) holds every kind in load order with all fields plus, per entry,
`text_keys`, `links` (targets mentioned in its texts) and `wiki` (markdown or null). Parts add `color_key`,
`stat_lines` (identical to the drydock), `stat_lines_by_material` for hull parts, `geometry` (area, polygon,
connectable sides) and `used_by_ships`. Ships add per-placement `cells`, `polygon`, `center` and `piece`,
plus `bounds`, `pieces`, `main_piece`, `hull_loops`, summed `totals` (including `control_cost` and
`control_capacity`; from `Content::ship_totals`, a key present when some part has that stat line), `rule_slots`,
`control_load`, `control_capacity` (of the main piece) and `effective_behavior`.
`frames` (added 12 Oct 2026) hold the header fields, `size` and the usual `text_keys`, `links` and `wiki`.
`behavior_nodes` add `param_text_keys`, `builtin` (false for Rhai nodes) and `used_by_behaviors`;
`behaviors` add `text` (the canonical text form), `program` (the parsed rules), `rule_count`,
`default_of_factions` and `flown_by_ships`. Categories list their parts,
factions their ships. `icons` lists every icon by key with its `svg` text, `source` (pack and file) and
`used_by` (parts, materials, categories), so the wiki can inline the SVGs or write them out as files. Ship
placements carry their `power` override. `pages` holds free articles; `texts` the whole key → text table.

## Art keys

Content never holds colours. It names keys that `art/tokens.json` resolves:

- system colours (`ship.system_colors`): `power weapons shields heat propulsion logic cargo enemy`
- armor tones (`ship.armor`): `steel composite reactive enemy`
- hull glow: `player` (`ship.hull_glow.color`), `enemy` (`ship.hull_glow.enemy_color`)
- icons: keys of `icons/<key>.svg` in the content packs (not `tokens.json`)

The lists live in `data::art_keys`; a new key needs an entry there and in `tokens.json`.

## Icons

Icons live in the content pack (ADR-0021, ADR-0028): `content/base/icons/<key>.svg`, one SVG per key, the
glyph of a part without its tile (the game draws tiles itself and rasterises the glyph at runtime at the
resolution it needs; the wiki can show the SVG as is or composite it on a tile). Materials have a swatch icon
(`armor-steel`, ...). Parts, materials and categories name them by `icon` key.

- `Content::icon_svg(key)` returns the SVG text from the last loaded pack that has the key; `Content::icons`
  lists them all (`Icon { key, svg, source }`). Mods add or replace icons by key.
- Every icon must be usable SVG (`data::svg::check_svg`, a cheap structural check without an XML library):
  well-formed tags, a root `<svg>` in the SVG namespace with a `viewBox`, and no `<image>`, `<text>`,
  `<script>` or `<foreignObject>` (no raster graphics; the runtime rasteriser has no fonts; the wiki shows the
  files in a browser).
- Glyphs are drawn on a 20 × 20 `viewBox` (one cell) in their system colour, with their soft glow as a
  blurred group; see `docs/style-guide.md`.

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
grid.bounds(); grid.line_of_fire_clear(i); grid.remove(i); grid.shape(i).centroid();
grid.open_sides(i);                                      // Vec<((x, y), Dir)>: RCS nozzles of part i
dir.normal(); dir.side_center((x, y)); dir.side_segment((x, y));  // nozzle point/direction, side corners
content.part_stats(part, material) / content.material_stats(m);  // Vec<StatLine>
content.stat_line("top_speed", 21.5);                    // Option<StatLine>: any stat value, labelled and formatted
content.part_color(part); content.placement_material(&p); content.placement_mass(&p); content.placement_hp(&p);
part.build_points_in(armor); part.mass_in(armor); part.hp_in(armor);  // a part's numbers in an armour material
let t = content.ship_totals(ship);   // ShipTotals: sums over every placement (build points with material factor,
                                     // mass, hp, power output/draw, thrust, cargo, logic cores, rule slots, control)
content.icon_svg("pulse-laser");                         // Option<&str>: SVG text of icons/pulse-laser.svg
content.render_plain(text); content.texts(); content.apply_texts(&locale_map);
content.ship_behavior(ship); content.ship_rule_slots(ship);   // effective behavior, slots of the intact ship
content.ship_control(ship); part.control_cost();              // (load, capacity) of the intact ship; cost with default
data::validate_ship(&content, &design);                      // Vec<Problem>: the content checks on one blueprint
let text = data::blueprint_to_ron(&design);                  // the RON of a saved or exported design
let design = data::parse_blueprint(&text)?;                  // Result<Blueprint, Vec<Problem>>: syntax and schema
let program = data::behavior::Program::parse(text)?;         // text form (or behavior.program() for RON form)
program.to_text(); program.to_parts();                       // canonical print
data::behavior::check_program(&program, &content.behavior_nodes); // Vec<LineError { line, col, message }>
data::behavior::check_text(text, &content.behavior_nodes);    // Result<Program, Vec<TextError>>: parse + check, by text line
// Editing models: data::edit::Drydock (docs/drydock.md), data::behavior_edit::BehaviorEditor (docs/behavior-format.md)
```
