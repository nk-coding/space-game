# Content model

How game content is organised so that the game, the tools and a future wiki read the same files.
Written 11 Oct 2026. Code: `crates/core/data` (schema, loader, validation, stat lines, geometry, the blueprint text
format, the drydock's and the behavior editor's editing models) and `crates/tools/content-tool` (CLI). Content:
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
  materials/precursor.ron  materials without an armor block (programmable matter)
  parts/<category>.ron     parts, one file per category (+ automatons.ron for enemy parts)
  frames.ron               drydock frames (the design grid sizes); order = drydock order
  ships/<id>.ron           one blueprint per file
  families.ron             research families (one tech tree each); order = display order
  research/*.ron           research nodes (research/rim.ron: the first playable loop's nodes)
  starts.ron               how a new game begins
  voyages/rim.ron          voyages (the first playable loop's four rim clusters); order = voyage map order
  encounters/*.ron         what waits at a voyage site (ships, wrecks)
  controls.ron             the action map: every player action, its section, screens and default inputs (GDR-0090)
  facilities.ron           the home shipyard's buildings research opens (the evolution lab; GDR-0092)
  lab.ron                  the evolution lab's words and numbers: goals, trials, genes, pilots, sizes, measures (GDR-0092)
  behavior_nodes/*.ron     the behavior vocabulary: conditions, actions, target selectors (+ Rhai conditions)
  behaviors/<id>.ron       behavior programs (a target line and rules in the text format)
  icons/<key>.svg          part, material and category icons as SVG (the glyph without its tile)
  icons/<key>.ship.svg     a machine as drawn on a ship (its housing, GDR-0069; see Ship drawings)
  wiki/<kind>/<id>.md      optional long-form prose, e.g. wiki/part/fission_reactor.md
  wiki/page/<id>.md        free-standing wiki articles (first "# " line = title)
```

File rules:

- A kind's data lives in `<dir>.ron` or anywhere under `<dir>/` (any depth), with `<dir>` one of `stats`,
  `categories`, `factions`, `materials`, `parts`, `frames`, `ships`, `behavior_nodes`, `behaviors`, `families`,
  `research`, `facilities`, `starts`, `voyages`, `encounters`, `controls`. File names are free; group however reads
  best. `lab.ron` is one file of its own (one `( ... )`, not an entry kind).
- A file holds **one entry** `( ... )` or **a list** `[ ( ... ), ... ]`. Comments (`//`, `/* */`) are fine.
- `Some(...)` may be omitted (`power: (output: 10)`); RON's `implicit_some` is always on.
- Unknown fields are errors (typos fail loudly). Unknown `.ron` files are errors; `.svg` files belong in
  `icons/` as `<key>.svg` or `<key>.ship.svg` with a kebab-case key; other files (READMEs) are ignored.
- Files are read in sorted path order, entries in file order. That is the **load order**, which is also the
  display order for stats, categories, frames, families, voyages and the drydock palette.
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
green/red diffs in the drydock). Which part field feeds which stat id is fixed in code (`data::design::stats`), so
the game requires these ids: `build_points mass hp power_output power_draw power_min_level heat_output heat_limit heat_vent
damage range shot_rate shot_speed penetration recoil thrust rated_speed gimbal plume_length plume_damage plume_heat
rcs_thrust rcs_rated_speed torque
shield_radius shield_capacity shield_regen cargo_capacity rule_slots control_cost control_capacity mass_per_cell
hp_per_cell build_point_factor blast_damage blast_radius blast_factor` (the last three since 14 Oct 2026,
GDR-0084). `fire_rate` is a bonus-only stat. Units (`t`, `kN`, `kN·m`, `MW`, `°`) are
placeholders except that the sim reads mass in t, thrust in kN, torque in kN·m (so kN / t = m/s²) and rated
speeds in m/s; 1 cell = 1 m.

**Ship stats** (added 12 Oct 2026). The drydock's stats panel shows ship-level values that the sim's design
report computes; their labels and units live in `stats.ron` too, and the game requires these ids
(`data::design::stats::SHIP_STATS`, a separate list from the part stats `BUILTIN_STATS`):

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
 armor: (tone: "composite", mass_per_cell: 1.5, hp_per_cell: 1.4, build_point_factor: 1.5, blast_factor: 0.5))
```

A hull part placed in a material gets `mass = mass_per_cell × area`, `hp = hp_per_cell × area` and
`build_points × build_point_factor`; slopes have half the area of their footprint. `blast_factor` (optional, default
1, at least 0; added 14 Oct 2026, GDR-0084) is the share of a chain reaction's damage the hull part takes, and since
GDR-0085 also the share it lets through to the parts behind it (base: 0.5, reactive 0.25); stat line `blast_factor`
("Blast damage taken", ×). Fabrication resources
(He-3, chitin, ...) will be materials without an `armor` block, which is why the kind is not called "armor".
The first one is **programmable matter** (`materials/precursor.ron`, added 13 Oct 2026): research costs it, and
cores give it when salvaged (see [Salvage](#salvage-added-13-oct-2026)). A material without `armor` never shows in
the drydock and cannot be a placement's material.

An armour material may carry a `salvage` block (what one cell of hull area in it gives when salvaged; see
[Salvage](#salvage-added-13-oct-2026)):

```ron
(id: "composite", ..., salvage: (family: "hull", data: 1.5))
```

`locked` (optional, default `false`, added 13 Oct 2026), on materials, parts and frames alike: not available to
the player until unlocked (research: an unlock of a [research node](#research-nodes-added-13-oct-2026)
clears it per player, GDR-0063). The free drydock hides locked parts and materials (palette, armor pills, the 1–9
keys); the career's drydock shows them dimmed until researched (GDR-0065); content ships that use them still load,
open in the drydock and fly. The base pack locks the automatons'
core, cannon and plate (GDR-0054) and the mid-game kit (GDR-0063): composite and reactive armour, the fusion
reactor, the beam laser, the torch drive, and the cruiser and capital frames (Niklas, 13 Oct 2026). A new game
has everything else (the early-game kit: steel, fission reactor,
capacitor, radiator, pulse laser, shield emitter, thruster, gyro, RCS pod, logic core, cargo bay and hold, the
frigate frame). Until the career drydock reads the player's unlocks, the free drydock and the sandbox simply hide
what is locked; a development switch (`?unlock=all`) is planned.

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
| `barrel` | optional, guns only: `(width, length, muzzle)` in cells, the barrel the client draws through the gun's nose (`length` beyond its front edge) and the radius of the muzzle glow at its tip; a gun without one gets a small glow at its front edge (13 Oct 2026, from the reference SVGs) |
| `size` | footprint `(w, h)` in cells before rotation; each side 1 to 16 (`data::content::validate::MAX_PART_SIDE`, a placeholder) |
| `shape` | `Rect` (default) or `Slope` |
| `build_points` | how big and costly the part is; no budget limits a design (no economy): balancing, the test range's wave size, the arena's design limit |
| `control_cost` | optional: what the part costs against its vessel's control capacity (GDR-0044, see below); default by kind |
| `hull` | `true`: takes an armor material per placement; then `mass`/`hp` must be absent |
| `mass`, `hp` | required for non-hull parts |
| `locked` | optional, default `false`: not in a new game until research unlocks it; the free drydock hides it, the career's shows it dimmed if a research node unlocks it, else hides it (GDR-0065). Content ships may still use it (the raider flies its locked automaton parts) |
| `salvage` | optional, non-hull parts only: what the part gives when salvaged (see [Salvage](#salvage-added-13-oct-2026)). A part without it is never collected |

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
| `explosion` | `damage`, `radius` (m, at most 4, `data::content::validate::MAX_BLAST_RADIUS`) | a chain reaction (GDR-0019, GDR-0084, GDR-0085, added 14 Oct 2026): when damage destroys the part, every other part with a cell within `radius` m of its centre, on any piece of any ship, takes `damage` once (hull parts times their armour's `blast_factor`; intact armour on the line from the centre lets only its `blast_factor` through); parts it destroys explode in turn, in the same tick. Other ships' shields absorb it, the ship's own do not. Stat lines `blast_damage` (HP) and `blast_radius` (m). Base values (placeholders, lead, 14 Oct 2026, GDR-0085): fission reactor and automaton core 1.5 / 1.5, fusion reactor 2.5 / 2, torch drive 1.2 / 2, shield emitter 1 / 0.6, capacitor, thruster and RCS pod 0.5 / 0.6; the sim's rule is in `docs/sim.md` "Damage and breakage" |
| `bonuses` | list, see below | adjacency bonuses this part grants |

**Weapon delivery** (`weapon.delivery`, added 11 Oct 2026):

| Delivery | Fields | Meaning |
|---|---|---|
| `Beam` | — | continuous hit-scan: `damage` per second while firing, burns the first block it touches |
| `Pulse(rate: r)` | `rate` (shots/s, at most 60) | hit-scan pulses, each dealing `damage / rate`, first block only |
| `Projectile(rate: r, speed: v, penetration: n, recoil: j)` | shots/s, m/s, blocks, kN·s | a flying shot (inherits the ship's velocity), lives `range / speed` s, hits up to `n` blocks (the first included; what breaks one carries on into the next), pushes the shooter back with `j` per shot; kinetic shots ricochet off slopes hit at a shallow angle |

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

`target` is `Part(id)`, `Tag(tag)` or `Category(id)`; `stat` is one the sim changes per part
(`data::design::stats::BONUS_STATS`: `power_output power_draw heat_output heat_limit heat_vent damage fire_rate thrust
shield_radius torque rcs_thrust gimbal range shield_capacity shield_regen rated_speed rcs_rated_speed plume_length
plume_damage plume_heat`; any other is an error, not a silent no-op); `effect` is `Add(x)` (in the stat's unit,
at most ±1000000) or `Percent(x)` (above −100, at most 1000); `max_stacks` limits how many sources count per
target; `when` holds conditions
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
| `length` | how far the exhaust reaches behind the engine's back side, m (0 = no plume; at most 50, `data::content::validate::MAX_PLUME_LENGTH`) |
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
| `size` | grid `(w, h)` in cells; each side 1 to 128 (`data::content::validate::MAX_FRAME_SIZE`, a placeholder) |
| `locked` | optional, default `false` (added 13 Oct 2026): a new game cannot build in it until research unlocks it (`Unlock::Frame`). At least one frame must stay open |

Load order is the drydock's display order (`Content::frames`, a `Table<Frame>`); mods override and add frames by
id like any other kind. Base frames (lead placeholders, 12 Oct 2026): `frigate` 24×24, `cruiser` 40×40,
`capital` 64×64; cruiser and capital are locked (GDR-0063, 13 Oct 2026). Text keys `frame.<id>.name` etc., links `[[frame:id]]`, wiki prose `wiki/frame/<id>.md`.
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

`size` is the blueprint's own grid (its frame), each side 1 to 128 cells like a frame; every placement must fit
inside. It does not refer to a `frames` entry (v1). Placement coordinates are read only within ±65536
(`data::content::limits::MAX_COORD`). A placement is `part`, `x`, `y`, `rot`
(default 0), `flip` (default false), `material` (required for hull parts, forbidden otherwise) and `power`
(optional, parts that draw power only): the player's override of the part's power tier and weight, e.g.
`(part: "shield_emitter", x: 2, y: 6, power: (tier: Low))` or `power: (tier: High, weight: 2)`; either field
may be left out; and `group` (optional, weapons only, 1 to `MAX_WEAPON_GROUP` = 3; none is group 1, and the writer
leaves group 1 out): the gun's weapon group, which of the pilot's fire triggers fires it (GDR-0088, e.g.
`(part: "pulse_laser", x: 4, y: 0, group: 2)`; behaviors fire every gun at will). The index of a placement in `parts` is the part index used by the geometry API. `behavior` (optional) names the behavior the
ship flies when nobody pilots it; default: its faction's (`Content::ship_behavior`). `assists` (optional, GDR-0088
step 3, 15 Oct 2026; `AssistList`) holds the design's **pilot assists**: rules in the behavior format that help the
player fly it, `rules` (one line each), checked as an assist list (`data::behavior::check_assists`; errors at
`assists.rules[i]`), e.g. `assists: (rules: ["when idle(thrust) >= 1s -> brake(toggle2)", "always -> auto_lock(nearest,
toggle1)"])`. An assist list has no target: its assists act on the pilot's lock (GDR-0088, dated line of 15 Oct 2026).
Older lists (step 3, the same day) had a `target` field (a target line without its keyword) and could hold
`face_target` rules: reading leaves both out (`AssistList` reads through `AssistListText`, which still refuses any
other unknown field) and the writer never writes them, so those designs load and are saved back without them; the
blueprint format keeps its version, as the new text reads in older builds. None (the
default) is no assists: the ship coasts when let go. The drydock writes it with the design; the BEHAVIOR screen's
PILOT ASSISTS tab edits it (its graph, or its TEXT view in exactly this rule text) (`docs/behavior-format.md` "Pilot assists").

Base ships: `halvex_runner`, `automaton_raider` (from `../space-game-archive/ships/automaton-raider.svg`), `sample_early` and
`sample_mid` (from the concept-board drafts), and `rim_skiff` (13 Oct 2026, the starter ship of `rim_awakening`:
`sample_early` with a gyro pair in place of its two rear armour blocks and radiators on the shoulders in place of
two slopes, so it turns 90° in about 0.67 s by the design report instead of 1.6 s and its reactors feed the gyros;
early-game kit only; a lead placeholder), and the voyages' enemies `automaton_hunter` (5×4, two cannons on forward
prongs, one core) and `automaton_warden` (9×7, the boss of `wardens_gate`: a shielded heart and two armed fists,
three cores, so it fights on in pieces), both from `../space-game-archive/plans/slice-content.md` 2 (placeholders). Since GDR-0068
(a site clears when its enemies are destroyed or disarmed) the Warden's heart has a cannon in its nose in place of
its second shield emitter, so the heart must be beaten too and each section still runs on its own core's 6 MW.

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

The Halvex Runner started as an exact conversion of `../space-game-archive/ships/halvex-runner.layout.json`. On 11 Oct 2026 Niklas
decided that its layout must work under the drydock rules. The **content blueprint is now the source of truth**
and the original concept art is archived (`../space-game-archive/ships/`). The adjusted layout keeps the occupied cells, the
spine (beams, fusion reactor, torch drive, thrusters), the slopes and the left/right symmetry. The wing-tip pulse
lasers moved up into the wing shoulders, replacing the two 1×1 slopes that blocked their line of fire, so the
outline differs from the concept art only there. The radiators now vent (wing tips and tail row), the
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
| `pilot` | default false; true: the node belongs to pilot assists only (GDR-0088, 15 Oct 2026): an action marked so is an assist (`assists.ron`: `auto_lock`, `brake`, `hold_station`, `avoid_collisions`, `auto_fire`; an assist list takes no other action, and no target selector: it has no target line), a condition marked so is about the pilot's own input (`lock_held`, `idle`); behaviors refuse them. Not on target selectors or scripted conditions (validation); the sim's `BUILTIN_NODES` must agree |

The assists' actions end in two choices, `toggle` (`none`, `toggle1`..`toggle4`: the ASSIST TOGGLE that switches the
rule in flight) and `start` (`on`, `off`); `auto_fire` takes its weapon `group` (Count, 1–3) first, `hold_station`
(step 4, 15 Oct 2026) its station `at` (`point`, `ally`) first, `auto_lock` its `pick` first (a choice of the target
selectors that suit a pilot, `nearest`, `weakest`, `threat`, `attacker`, each with its name and summary in
`assists.ron`; the sim's `BUILTIN_NODES` pins the ids and runs them as the target selectors); `avoid_collisions` takes
only the two. `speed` (Percent)
and `can_stop_straight` (flag), added with them, are conditions of both lists.

Text keys: the header fields as usual, plus `behavior_node.<id>.params.<param>.name` / `.summary` and
`behavior_node.<id>.params.<param>.options.<option>.name` / `.summary`.

Base vocabulary (v1): conditions `hull`, `shield`, `shield_down`, `heat`, `power`, `target_in_range`,
`target_hull`, `target_on_me`, `under_attack`, `enemies_near`, `cycle`, `target_time`, and the Rhai condition
`closing_in`; actions `approach`, `orbit`, `kite`, `face`, `flee`, `hold`, `escort` (13 Oct 2026, GDR-0065: keep
near the leader, the one action without a target); target selectors `nearest`,
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
        "when target_in_range(120%) and cycle(9s) >= 3s -> orbit(58%)",
        "always -> approach(85%)",
    ],
)
```

`target` is optional (default `nearest`); a rule may end in a `# note`. Rules are written in canonical form
(the printer's output; a test checks the base pack). Base behaviors: `raider_basic` (the automatons' default,
two rules for one automaton core; it replaced the hard-coded raider brain), `skirmisher` (the ship-mind's
default for unpiloted ships, four rules since GDR-0061, uses the Rhai condition) and `escort` (three rules: fight
the leader's lock, keep near the leader between fights; GDR-0065).

### Salvage (added 13 Oct 2026)

What a salvaged part gives when the haul is dismantled at home (GDR-0063): research data for one
[family](#research-families-familiesron-added-13-oct-2026) and, optionally, materials. The career model turns a haul
into data and materials with it (`docs/career.md` "Unloading a haul"). The sim reads only two facts from it
(compiled into the ship's design): a part without the block (a hull part: its material without one) is never
collected, and only a part whose block has a non-empty `fragment` leaves a fragment when destroyed. What a
collector takes first and how much room an item needs are sim rules (`docs/sim.md` "Salvage").

```ron
// parts/automatons.ron
(id: "automaton_cannon", ..., salvage: (family: "kinetic", data: 3)),
(id: "automaton_core", ..., salvage: (family: "logic", data: 4,
    materials: {"programmable_matter": 1}, fragment: {"programmable_matter": 1})),
// materials/armor.ron: per cell of hull area in this material (slopes count half a cell)
(id: "steel", ..., salvage: (family: "hull", data: 1)),
```

| Field | Notes |
|---|---|
| `family` | research family id the data goes to |
| `data` | research data one intact part gives (on a material: per cell of hull area); finite, 0 to 10000 (`data::content::validate::MAX_RESEARCH_DATA`) |
| `materials` | parts only, optional: materials (id → amount) one intact part gives besides the data |
| `fragment` | parts only, optional: materials (id → amount) a **destroyed** part of this kind leaves as a fragment to scoop (GDR-0063: destroyed cores leave programmable matter); empty = it leaves nothing |

Material amounts are whole numbers, 1 to 10000 (`data::content::validate::MAX_MATERIAL_AMOUNT`), of known materials. Hull
parts have no block of their own (like `mass` and `hp`, it comes from their material); only armour materials have
one. Cargo units a salvaged part takes are a sim rule, not content.

Base values (lead placeholders, 13 Oct 2026, after `docs/plans/first-playable-loop.md` 3.2: data about the part's
build points): automaton cannon kinetic 3, thruster propulsion 3, gyro propulsion 4, RCS pod propulsion 2, torch
drive propulsion 9, pulse laser energy 3, beam laser energy 7, fission reactor power 4, fusion reactor power 10,
capacitor power 2, shield emitter shields 3, radiator thermal 2, cargo bay utility 1.5, cargo hold utility 5, logic
core logic 3, automaton core logic 4; both cores also give 1 programmable matter and leave a fragment of 1. Hull per
cell: steel 1, composite 1.5, reactive 2, Automaton plate 1.

### Research families (`families.ron`, added 13 Oct 2026)

One tech tree per system family (GDR-0024), and the pool its research data goes to. The common header plus an
optional `color`, a system colour key (see [Art keys](#art-keys)); without one the family shows in the neutral
colour, like the hull category. Load order is the display order.

```ron
(id: "energy", name: "Energy weapons", summary: "Lasers and beams.", color: "weapons")
```

Base families (lead placeholders, 13 Oct 2026): `hull` (no colour), `power`, `energy` and `kinetic` (`weapons`),
`shields`, `thermal` (`heat`), `propulsion`, `logic`, `utility` (`cargo`). Missiles come when missiles exist.

### Research nodes (added 13 Oct 2026)

`research/*.ron`. A node is paid at once in research data and materials and unlocks immediately (GDR-0063); which
nodes a player has researched lives in the save, never in content (`docs/career.md`).

```ron
(
    id: "mind_expansion_1",
    name: "Mind expansion I",
    summary: "Run one more logic core at once.",
    family: "logic",
    kind: Refinement,
    cost: (data: {"logic": 10}, materials: {"programmable_matter": 2}),
    requires: ["tamed_core"],
    unlocks: [MindCapacity(1)],
)
```

| Field | Notes |
|---|---|
| `family` | the family whose tree the node sits in |
| `kind` | `NewPart`, `Variant`, `SizeClass`, `Refinement`, `Alien` (GDR-0024) or `Facility` (16 Oct 2026, GDR-0092): how the research screen draws it |
| `cost` | required: `data` (family id → amount, each above 0, at most 10000) and `materials` (id → whole amount, 1 to 10000); both may be left out (`cost: ()`) |
| `requires` | optional: node ids (any family) that must be researched first |
| `unlocks` | required list of `Part(id)`, `Material(id)`, `Frame(id)`, `Facility(id)` (clears that entry's `locked` for the player) and `MindCapacity(n)` (n more logic cores the mind runs at once, 1 to 100) |

Validation: the family, every cost family and material, every `requires` node and every unlock target exist; no
node listed twice in `requires`; no cycle in `requires` (each cycle is reported once, with its path); unlocking an
entry that is not `locked` is a warning (a new game has it already), so is a node that unlocks nothing.

Base nodes (`research/rim.ron`, the first playable loop's nine, placeholders; costs about halved by the slice
tuning, GDR-0073): `automaton_cannon` (kinetic 3), `composite_plating` (hull 8, unlocks composite), `beam_laser`
(energy 10), `fusion_reactor` (power 10), `torch_drive` (propulsion 10), `tamed_core` (logic 3 + programmable
matter 1, unlocks the automaton core), `mind_expansion_1` (logic 10 + 3, needs `tamed_core`), `mind_expansion_2`
(logic 16 + 5, needs `mind_expansion_1`), `cruiser_drydock` (hull 20, needs `composite_plating`, unlocks the cruiser
frame), `simulation_vault` (logic 3, kind `Facility`, unlocks the facility `evolution`, GDR-0092 step 8, 16 Oct 2026).
Reactive armour, Automaton plate and the capital frame stay locked.

### Starts (`starts.ron`, added 13 Oct 2026)

How a new game begins. A new game has every part, material and frame that is not `locked` (a start names no unlocks
of its own). The content needs at least one start (an error otherwise).

```ron
(
    id: "rim_awakening",
    name: "Awakening at the rim",
    summary: "You wake in a dead shipyard with one small ship and a drydock that still works.",
    ship: "rim_skiff",              // the first working design (a copy)
    mind_capacity: 2,               // logic cores the mind runs at once (GDR-0013); research adds to it
    voyages: ["yard_approach"],     // voyages open at the start
)
```

| Field | Notes |
|---|---|
| `ship` | a ship id. It must be one a new game can take on a voyage: no locked part or material, a size that fits a frame that is not locked, a logic core, and no more logic cores than `mind_capacity`. A ship without a hold (cargo on its main piece) is a warning: nothing salvaged would come home |
| `mind_capacity` | 1 to 100 (`data::content::validate::MAX_MIND_CAPACITY`, a sanity bound) |
| `voyages` | optional: voyage ids, each known and listed once. Clearing a voyage opens those that name it in `opens_after` ([Voyages](#voyages-voyagesron-added-13-oct-2026)) |

Base start (placeholder, GDR-0062): `rim_awakening`, ship `rim_skiff`, mind capacity 2 (the two mind expansions
bring it to 4), voyage `yard_approach` open.

### Voyages (`voyages/*.ron`, added 13 Oct 2026)

A voyage is one small hand-made cluster with seeded sites (GDR-0062). Each launch rolls where its sites lie and what
each holds (`sim::voyage`, `docs/sim.md` "Voyages"); the career remembers launches and clears (`docs/career.md`).

```ron
// voyages/rim.ron (one of four). PLACEHOLDER NUMBERS.
(
    id: "halvex_drift",
    name: "Halvex Drift",
    summary: "A raider-haunted drift of wrecks one warp out from the shipyard.",
    band: "rim",                         // band of the galaxy (GDR-0026), a snake_case key
    map: (0.44, 0.30),                   // place on the voyage map, 0 to 1 (x right, y down)
    opens_after: ["yard_approach"],      // voyages whose goal must have been cleared once
    radius: 900,                         // m: the sites lie inside; the fleet enters at the edge
    spacing: (400, 700),                 // m between sites (see below)
    minutes: (6, 10),                    // expected length, for the map ("≈ 6–10 MIN")
    cleared: "The raiders' den is broken. Their hunters still hold the [[voyage:corran_reach]].",
    sites: [
        (encounter: "raider_pair"),
        (encounter: "precursor_wrecks"),
        (encounter: "core_graveyard"),
        (encounter: "raider_den", goal: true),   // clearing it clears the voyage
    ],
)
```

| Field | Notes |
|---|---|
| `band` | snake_case key; text only until bands are content |
| `map` | each coordinate 0 to 1 |
| `opens_after` | optional: voyage ids, each known and listed once, no cycles. Empty: only a start's `voyages` opens it (a voyage nothing opens is a warning, and so is one that opens only after voyages that can never open). The voyage map draws no route along it and doesn't show a voyage before it opens; one opened this way shows as a new signal until its first launch, and the debrief that opened it says `SIGNAL FOUND · <name>` (GDR-0078) |
| `radius` | m, above 0 and at most 20000 (`data::content::validate::MAX_VOYAGE_RADIUS`, a sanity bound) |
| `spacing` | `(min, max)` m, `0 < min ≤ max`, `min ≤ 2 × radius`: no two sites (and no site and the entry) closer than `min`; each site within `max` of an earlier one or the entry |
| `minutes` | `(min, max)`, `1 ≤ min ≤ max ≤ 600` |
| `cleared` | optional text (a text key `voyage.<id>.cleared`): the debrief's line the first time the goal falls |
| `sites` | at least one; `encounter` a known encounter, `count` (default 1) at least 1, at most 64 sites in all (`MAX_VOYAGE_SITES`). Exactly one entry is the goal (`goal: true`), with count 1 and an encounter whose ships' minimum counts are not all 0 (a goal must always have ships to clear), one of them armed (`Content::ship_armed`: a weapon and power on its main piece; GDR-0068) |

Base voyages (placeholders, `../space-game-archive/plans/slice-content.md` 3.3), in one file in chain order: `yard_approach` (open at
the start; 600 m, raider pair ×2, derelict field, **raider pack**), `halvex_drift` (900 m, raider pack ×2, precursor
wrecks, core graveyard, **raider den**), `corran_reach` (1000 m, hunter pack ×2, raider pack, precursor wrecks,
**hunter den**), `wardens_gate` (900 m, raider pack ×2, core graveyard, **the Warden**).

### Encounters (`encounters/*.ron`, added 13 Oct 2026)

What waits at a voyage site. Its ships hold where they spawned until a player vessel comes within `wake_radius`,
then fly their own or their faction's behavior; its wrecks spawn as salvage with a share of their parts already
gone (`Sim::spawn_wreck`). Variants by count (a pair, a pack, a den) are entries of their own.

```ron
// encounters/automatons.ron (one of nine). PLACEHOLDER NUMBERS.
(
    id: "precursor_wrecks",
    name: "Precursor wrecks",
    summary: "Old hulls full of parts the shipyard has forgotten how to make.",
    lore: "Their logs stop mid-sentence, all on the same day.",
    tags: ["wrecks"],
    ships: [(ship: "automaton_raider", count: (0, 2))],            // (min, max), both included, per launch
    wrecks: [(ship: "sample_mid", count: (1, 2), damage: 0.4)],     // share of parts gone, per wreck
    wake_radius: 200,                                                // m from the site
)
```

| Field | Notes |
|---|---|
| `ships` | optional: a known ship with a logic core (one without would spawn as salvage) and `count` `(min, max)`, `min ≤ max ≤ 32` (`MAX_ENCOUNTER_COUNT`). A ship without a behavior (its own or its faction's) is a warning: it would only hold |
| `wrecks` | optional: any known ship, `count` as above, `damage` 0 to below 1 (default 0) |
| `wake_radius` | m, above 0 and at most 20000 |

An encounter needs ships or wrecks. Base encounters (placeholders, `../space-game-archive/plans/slice-content.md` 3.2, counts tuned
by GDR-0073): `raider_pair` (2 raiders), `raider_pack` (3), `raider_den` (2–3, wakes at 300 m), `hunter_pack` (1
hunter, 1 raider), `hunter_den` (2–3 hunters), `derelict_field` (0–1 raider; a `sample_early` and 2–3 raider wrecks,
half their parts gone), `precursor_wrecks` (0–1 raider; 1–2 `sample_mid` wrecks), `core_graveyard` (0–1 raider; 4–6
raider wrecks, 30 % gone, so most of their cores are intact), `the_warden` (the `automaton_warden` and 1–2 raiders).
Packs wake at 250 m, wreck sites at 200 m, dens and the Warden at 300 m. The voyages (`voyages/rim.ron`): Shipyard
approach (raider pair ×2, derelict field, goal raider pair), Halvex Drift (raider pair, precursor wrecks, core
graveyard, goal raider den), Corran Reach (hunter pack, precursor wrecks, goal hunter den), the Warden's gate (raider
pack, core graveyard, goal the Warden).

### Controls (`controls.ron`, added 15 Oct 2026, GDR-0090)

The action map's actions (`data::input::Action`; every action needs exactly one entry once the content has any, and
an entry's `id` must be one of them: validation checks both ways). Order = the settings screen's order inside a
section.

| Field | Meaning |
|---|---|
| `section` | the settings screen's group: `flight`, `mission`, `camera`, `shipyard`, `drydock`, `map`, `debug` |
| `contexts` | the screens it acts on: `flight`, `home`, `drydock`, `behavior`, `research`, `map`, `any`. Two actions sharing one cannot share an input |
| `inputs` | up to two default inputs: a key (Bevy's `KeyCode` names, `data::input::KEYS`: `KeyW`, `Digit1`, `ArrowUp`, `F5`, `Space`, …) or a mouse button (`MouseRight`, `MouseMiddle`, `MouseBack`, `MouseForward`), optionally with `Ctrl+`, `Shift+`, `Alt+` (`Ctrl+Shift+KeyZ`) |

Fixed and never listed: Esc, Enter, text fields, the clipboard, the left mouse button as the pointer, middle-drag,
the wheel, touch. The player's changes are not content: the client saves them (`settings/controls`). A mod rebinds a
default by replacing the entry. The wiki lists the controls with their default inputs.

The fire actions (GDR-0088 step 2, 15 Oct 2026): `fire` (FIRE ALL, Space: every gun) and `fire_group_1` (Digit1),
`fire_group_2` (Digit2 and `MouseRight`, which `fire` held until then) and `fire_group_3` (Digit3), each firing the
guns of its weapon group (placement `group`). The held left button on empty space fires group 1 (the pointer is not
in the map).

The assist actions (GDR-0088 step 3, 15 Oct 2026): `assist_toggle_1` to `assist_toggle_4` (F1–F4) switch the pilot
assists on that toggle slot (which ones is set per design in their `toggle` value), `all_assists` (Z) all of them. The
debug overlay moved from F3 to F8 for them. `lock_next` (Tab) is LOCK TARGET (named "Lock target" since 15 Oct 2026,
the id kept for saved bindings): the nearest enemy, then the next by distance (`sim::flight::next_lock`).

### Facilities (`facilities.ron`, added 16 Oct 2026, GDR-0092)

The home shipyard's buildings that the player opens (GDR-0071: "each area needs states from content"). Common header
plus `locked` (default false): a locked facility is dark until a research node's `Facility(id)` opens it
(`Career::unlocks`'s `facilities`; `?unlock=all` opens every one). `name` is the label over the building at home. The
shipyard places and draws the building; only `evolution` (the evolution lab, locked, opened by `simulation_vault`) is
content so far; the fabricator and the salvage bay come later. The wiki gives each a page with the research that
opens it. Not in the rules' fingerprint.

### The evolution lab (`lab.ron`, GDR-0092; completed 16 Oct 2026, step 8)

One file per pack (a later pack's entry of the same id replaces an earlier one in its place; its `budget` replaces
the earlier one), not an entry kind: problems name fields as `lab.trials[<id>].seeds`, its texts are keyed
`lab.<list>.<id>.<field>` (`lab.goal.win.name`, in `Content::texts`). Not in the rules' fingerprint: a run keeps
everything it was made with, so a change here changes no run.

| List | Holds |
|---|---|
| `goals` | the arena's four goals (`win`, `unhurt`, `quick`, `cheap`): name, summary, weight in a new run (0 to 3) |
| `goal_presets` | the one-click goal weights (WIN, WIN UNHURT, ...) |
| `kinds` | the trial kinds' words, one per kind the arena fights (`data::TRIAL_KINDS`: `encounters`, `enemy_ships`, `waves`, `gauntlet`, `beat_your_start`, `fleet`, `hall_of_fame`, `champions`, `voyages`, `co_evolution`); a trial without a preset shows its kind's name |
| `trials` | the setup's trial presets: name, summary, `kind` with its data, weight, seeds, time limit, `default` (on in a new run) |
| `genes`, `gene_presets` | MAY CHANGE's genes (`data::GENES`) and the presets over them, the first a new run's |
| `pilots` | the stand-in pilots (`flies`: lock, close to, stick and fire duty), the first the default |
| `palettes` | parts an enemy faction may build with in an arms race besides its ships' |
| `sizes` | SIZE's presets: children per generation and generations, one `default: true` |
| `measures` | the archive's measures (`data::MEASURES`), named by a stat (`stat`) or by their own `name` and `unit` |
| `spreads` | the archive's grids, `design` (runs whose design may change; the arena's own default, a test keeps them equal) and `behavior`: one to three axes of `cells` bins of a measure from `min` to `max` |
| `budget` | the ms a frame gives the run that computes: `layer_open`, `elsewhere` |

Validation: goals, genes, kinds, measures and spreads are the arena's; presets name goals and genes; trials' numbers,
ships and factions are in range and exist, every trial's kind has its words; a measure has a stat or a name, not both;
a lab with trials needs sizes (at most one default), both spreads and a budget. The mutation operators' weights stay
the arena's (a run breeds by them and its file doesn't carry them). The wiki generates the lab's page from it
(`reference/lab.md`) and hand-written pages reference it as `[[lab:trial.voyages]]` and
`[[lab:size.standard.population]]` (`docs/wiki.md`).

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

These rules are a port of `../space-game-archive/concept/reference/drydock-model.js` and `drawHullGlow` in
`concept-board-script.js`; the tests check the drydock-model cases (slope links, mirroring, rotation). The checks
of the base ships against the concept art went with the art (14 Oct 2026, archived): the blueprints are the source
of truth.

## References and links

- Fields that name another entry hold its id (`category: "power"`, `faction: "automatons"`,
  `material: "steel"`, `target: Part("capacitor")`); validation checks each target exists.
- Any text field and wiki file may link: `[[kind:id]]` or `[[kind:id|label]]`, kind one of `stat category
  faction material part frame ship behavior_node behavior family research start voyage encounter control page`. The game renders links as plain names (`Content::render_plain`), the wiki
  as hyperlinks. Broken links are errors.

## Text and localisation

- Keys: `<kind>.<id>.<field>` for `name`, `summary`, `description`, `lore`, and `stat.<id>.unit`.
- `Content::texts()` lists every key with its English text (the template for a translation).
- A later locale file (e.g. `locale/de.ron` in a pack, a map key → text) replaces texts by these keys; the hook
  that applies it comes with the first locale (an unused one was removed on 14 Oct 2026).
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
  `icons/`, icons that are not usable SVG (see [Icons](#icons)), machines without their ship drawing and drawings
  with a wrong `viewBox` ([Ship drawings](#ship-drawings-added-13-oct-2026-gdr-0069)), non-finite or out-of-range numbers, hull parts
  with `mass`/`hp`, non-hull parts without them, illegal slopes (not `w × 1`, or not hull), hull placements
  without an armor material, materials on non-hull parts, `rot` outside 0–3, placements outside the frame,
  overlapping placements, frame sides outside 1–128 cells, wiki pages for missing entries, broken or malformed
  links, unknown fields anywhere (inside `weapon.delivery` too), a locked paint block or bevel slope (the first 1×1
  hull part of each shape in content order, which the drydock paints, fills and bevels with:
  `Content::unit_hull_part`), every armor material locked, every frame locked.
- Career content (added 13 Oct 2026): unknown families, materials, nodes, ships and unlock targets; amounts out
  of range; `salvage` on a hull part or a material without `armor`; a node listed twice in `requires`; cycles in
  `requires`; no start at all; a start whose ship uses locked parts or materials, fits no open frame, has no
  logic core or more logic cores than its mind capacity; unknown or repeated voyages. Details in [Salvage](#salvage-added-13-oct-2026),
  [Research nodes](#research-nodes-added-13-oct-2026) and [Starts](#starts-startsron-added-13-oct-2026).
- Voyages and encounters (added 13 Oct 2026): unknown encounters, ships and voyages; cycles in `opens_after`; no
  goal, two goals, a goal with a count above 1 or whose encounter may roll no ships, or no armed ship (a weapon and
  power on its main piece, GDR-0068: the site would clear the moment it wakes); a fighting ship without a
  logic core; an encounter with neither ships nor wrecks; numbers out of range (`MAX_VOYAGE_RADIUS`,
  `MAX_VOYAGE_SITES`, `MAX_ENCOUNTER_COUNT`, `MAX_VOYAGE_MINUTES`). Details in
  [Voyages](#voyages-voyagesron-added-13-oct-2026) and [Encounters](#encounters-encountersron-added-13-oct-2026).
- Warnings (valid but probably unintended): a ship that is several pieces, a gun without a clear line of fire,
  an RCS part without any side open to space, a radiator that cannot vent, an icon nothing uses, an empty
  placement `power` override, a research node that unlocks an entry that is not `locked` or unlocks nothing, an
  encounter ship without a behavior, a voyage that no start opens and that opens after nothing, a voyage that
  opens only after voyages that can never open, a start ship without a hold. (An engine with parts behind it is no
  warning since GDR-0039: its plume burns them.)
- Power: `power.weight` above 0 and at most 100 (`data::MAX_POWER_WEIGHT`), `power.min_level` 0 to 1, `tier`,
  `weight` and `min_level` only on parts that draw power; a placement `power` override only on such parts, with
  the same weight range.
- Turning parts: `engine.gimbal` between 0 and 45 degrees, `gyro.torque` and `rcs.thrust` above 0, no `rcs` on
  hull parts.
- Rated speed: `engine.rated_speed` and `rcs.rated_speed` are required (a missing one is a parse error) and
  above 0.
- Plume: `engine.plume` is required (a parse error when missing); `length`, `damage` and `heat` are finite and not
  negative, `length` at most 50 m.
- Upper limits (added 13 Oct 2026 after the untrusted-input review; placeholders that keep a mod from hanging the
  sim or exhausting memory, in `data::content::validate`): every number at most 1000000 (`MAX_NUMBER`); shot rates
  (`Pulse`, `Projectile`) at most 60 per second (`MAX_SHOT_RATE`); footprint sides at most 16 cells
  (`MAX_PART_SIDE`); frame sides and ship `size` sides at most 128 cells (`MAX_FRAME_SIZE`, also the drydock's
  limit); `logic.rule_slots` at most 64 (the most rules a behavior has); `penetration`, `control_capacity` and
  `max_stacks` at most 10000 (`MAX_COUNT`); bonus amounts as under Adjacency bonuses. `ship_rule_slots`,
  `ship_control` and `ship_totals` saturate instead of overflowing. RON files may start with a byte order mark;
  nesting deeper than 32 brackets is an error.
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

`content-tool wiki-data <wiki_dir> [pack_dir...]` feeds the wiki (15 Oct 2026; `docs/wiki.md`, ADR-0039, which
replaced the static HTML generator of ADR-0032): it writes `wiki/gen/`, a page per entry of every kind (stats
labelled from `stats.ron`, the pack's icons, parts and ships drawn from their geometry, the prose of `wiki/<kind>/`)
and the data the VitePress site reads (`refs.json`: every entry, its values as the game formats them and the sim
constants, which hand-written wiki pages reference as `[[part:thruster.thrust]]` or `[[sim:tick_hz]]` instead of
copying them; `ui.json`, `sidebar.json`, the icon sprite and `art/tokens.json` as CSS). It reads the `data` crate
directly and checks the wiki's hand-written pages against the content; other tools can use the JSON export:

```
cargo run -p content-tool -- validate                 # all packs ok (icons included)?
cargo run -p content-tool -- wiki-data wiki          # wiki/gen for the wiki site (docs/wiki.md)
cargo run -p content-tool -- export-json content.json  # resolved content for any external tool
cargo run -p content-tool -- list part
```

`export-json` (`format_version` 1) holds every kind in load order with all fields (numbers that are content f32s
written as their shortest text: `0.28`, not `0.2800000011920929`) plus, per entry,
`text_keys`, `links` (targets mentioned in its texts) and `wiki` (markdown or null). Parts add `color_key`,
`stat_lines` (identical to the drydock), `stat_lines_by_material` for hull parts, `geometry` (area, polygon,
connectable sides) and `used_by_ships`. Ships add per-placement `cells`, `polygon`, `center` and `piece`,
plus `bounds`, `pieces`, `main_piece`, `hull_loops`, summed `totals` (including `control_cost` and
`control_capacity`; from `Content::ship_totals`, a key present when some part has that stat line), `rule_slots`,
`control_load`, `control_capacity` (of the main piece) and `effective_behavior`.
`frames` (added 12 Oct 2026) hold the header fields, `size`, `locked` and the usual `text_keys`, `links` and `wiki`.
Parts, materials and frames add `unlocked_by` (the research nodes that unlock them); parts and armour materials
carry their `salvage` block. `families` (added 13 Oct 2026) add `salvaged_from_parts`, `salvaged_from_materials`
(what feeds them) and `research` (their nodes); `research` nodes add `required_by`; `starts` hold their fields.
`voyages` (added 13 Oct 2026) add `site_count` (sites in all), `opens` (the voyages that open after it) and
`opened_by_starts`; `encounters` add `used_by_voyages`; ships add `in_encounters`.
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

The lists live in `data::content::art_keys`; a new key needs an entry there and in `tokens.json`.

## Icons

Icons live in the content pack (ADR-0021, ADR-0028): `content/base/icons/<key>.svg`, one SVG per key, the
glyph of a part without its tile (the game draws tiles itself and rasterises the glyph at runtime at the
resolution it needs; the wiki can show the SVG as is or composite it on a tile). Materials have a swatch icon
(`armor-steel`, ...). Parts, materials and categories name them by `icon` key.

- `Content::icon_svg(key)` returns the SVG text from the last loaded pack that has the key; `Content::icons`
  lists them all (`Icon { key, svg, source }`). Mods add or replace icons by key.
- Every icon must be usable SVG (`data::content::svg::check_svg`, a cheap structural check without an XML library):
  well-formed tags and quoted attributes, a root `<svg>` in the SVG namespace with a `viewBox` (four numbers
  separated by spaces or commas, either quote style; width and height above 0 and at most 10000 units,
  `data::content::svg::MAX_VIEW_BOX`), and no `<image>`, `<feImage>`, `<text>`, `<script>` or `<foreignObject>` (in any
  case, with or without a namespace prefix: no raster graphics; the runtime rasteriser has no fonts; the wiki shows
  the files in a browser). Also refused (13 Oct 2026, untrusted-input review): event handler attributes (`on...`),
  references outside the file (an `href` or `url(...)` that does not start with `#`, `@import`) and declarations
  other than a plain `<!DOCTYPE ...>` (no entities).
- The rasteriser (`art`) reads the viewBox with the same code (`data::content::svg::view_box`), so every icon that validates
  lays out; one wider or taller than a row of the atlas layout is stored at a lower density. It never loads images
  or files (its usvg image resolver returns nothing). An icon the XML parser still refuses (an undeclared
  namespace prefix, say) stays blank and is named in `art::Raster::failed`; the rest of the atlas is fine.
- Glyphs are drawn on a 20 × 20 `viewBox` (one cell) in their system colour, with their soft glow as a
  blurred group; see `docs/style-guide.md`.

### Ship drawings (added 13 Oct 2026, GDR-0069)

On a ship a machine is drawn as its housing, not as its icon. Every machine (a part without `hull: true`) has a
second SVG next to its icon: `icons/<icon key>.ship.svg` (the thruster: `icons/thruster.ship.svg`). The icon stays
for the drydock's palette and inspector and for the wiki's part drawings; the wiki's ship drawings use the housings, on
the hull-toned cells of `data::design::tone`, as the game does.

- Found by the part's `icon` key, so a part needs no new field and parts sharing an icon share the drawing.
  `Content::ship_svg(key)` returns it (last loaded pack wins, like icons); `Content::ship_art` lists them all.
- **Geometry**: the part unrotated, nose (its facing) up, 20 units per cell. The `viewBox` is `0 0 W H` with `W` =
  the footprint's width × 20 and `H` its height × 20 plus an optional **overhang** at the stern (+y), at most one
  cell: what reaches past the hull behind the part. Only engines (parts with `engine`) may overhang (GDR-0069:
  the nozzle bell). An engine's viewBox ends just past the nozzle's mouth: the game starts the plume there. Base,
  placeholders: the thruster's bell reaches 0.35 cells past the stern, the torch drive's 0.45 (viewBoxes 0.375 and
  0.5 cells past). Keep the sizes in steps of 1.25 units (1/16 cell): they map to whole texels at every atlas tier.
  The game rotates and mirrors the drawing with the part; the overhang never collides.
- **Look**: a housing in `#1a2a36` / `#24384a` with system-colour details (style guide "Machines"). The cell under
  it is drawn by the game in the hull's armour tone, so the drawing has no background. Glows are
  `<g filter="url(#gl)">` groups as in icons; the game draws them as a soft layer *under* the drawing, so a glowing
  centre needs a window in its housing (an `evenodd` ring) to show.
- **Validation** (errors): a machine without an icon or without its ship drawing; a drawing that fails
  `check_svg`; a `viewBox` that does not start at `0 0`, is not as wide as the footprint, is shorter than it or
  overhangs by more than a cell; an overhang on a part without `engine`. A drawing no part uses is a warning.
- The `.ship.svg` suffix is reserved: any other dotted name under `icons/` is a load error, as before.

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
let loops = data::design::geometry::hull_loops(&edges);          // closed outlines, collinear points merged
grid.bounds(); grid.line_of_fire_clear(i); grid.remove(i); grid.shape(i).centroid();
grid.open_sides(i);                                      // Vec<((x, y), Dir)>: RCS nozzles of part i
dir.normal(); dir.side_center((x, y));  // nozzle point/direction
content.part_stats(part, material) / content.material_stats(m);  // Vec<StatLine>
content.stat_line("top_speed", 21.5);                    // Option<StatLine>: any stat value, labelled and formatted
content.part_color(part); content.placement_material(&p); content.placement_mass(&p); content.placement_hp(&p);
part.build_points_in(armor); part.mass_in(armor); part.hp_in(armor);  // a part's numbers in an armour material
let t = content.ship_totals(ship);   // ShipTotals: sums over every placement (build points with material factor,
                                     // mass, hp, power output/draw, thrust, cargo, logic cores, rule slots, control)
content.icon_svg("pulse-laser");                         // Option<&str>: SVG text of icons/pulse-laser.svg
content.render_plain(text); content.texts();
content.families / content.research / content.starts;      // Table<Family>, Table<ResearchNode>, Table<Start>
content.voyages / content.encounters;                      // Table<Voyage> (map order), Table<Encounter>
voyage.goal(); voyage.site_count(); encounter.always_has_ships();
part.salvage / material.salvage;                             // Option<SalvageSpec> / Option<HullSalvageSpec>
content.ship_behavior(ship); content.ship_rule_slots(ship);   // effective behavior, slots of the intact ship
content.ship_control(ship); part.control_cost();              // (load, capacity) of the intact ship; cost with default
data::validate_ship(&content, &design);                      // Vec<Problem>: the content checks on one blueprint
let text = data::blueprint_to_ron(&design);                  // the RON of a saved or exported design
let design = data::parse_blueprint(&text)?;                  // Result<Blueprint, Vec<Problem>>: syntax and schema
let program = data::behavior::Program::parse(text)?;         // text form (or behavior.program() for RON form)
program.to_text(); program.to_parts();                       // canonical print
data::behavior::check_program(&program, &content.behavior_nodes); // Vec<LineError { line, col, message }>
data::behavior::check_text(text, &content.behavior_nodes);    // Result<Program, Vec<TextError>>: parse + check, by text line
data::behavior::check_assists(&program, &content.behavior_nodes); // a design's pilot assists (ListKind::Assists)
// Editing models: data::design::edit::Drydock (docs/drydock.md), data::behavior::edit::BehaviorEditor (docs/behavior-format.md)
// The career (docs/career.md): the save's facts and the progression rules
let mut career = data::Career::new(&content, "rim_awakening", seed)?; // or Career::from_ron(text)? + .reconcile(&content)
career.unlocks(&content); career.mind_capacity(&content); career.open_voyages(&content);  // derived, never stored
career.can_pay(&content, "tamed_core"); career.research(&content, "tamed_core")?;        // Refusal says why not
let seed = career.voyage_launched(&content, "yard_approach")?;                             // the sim rolls the voyage from it
career.voyage_ended(&content, "yard_approach", VoyageEnd::Recalled { haul, cleared, ticks })?; // unloads the haul
data::career::unload(&content, &haul); data::career::voyage_ready(&content, &unlocks, capacity, &flagship, &escorts);
```
