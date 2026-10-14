# Plan: salvage, research and fabrication

Written 13 Oct 2026 by a planning agent for the lead session. Status: **proposal**. Nothing here is decided; the
decisions it needs are listed as gaps (section 3) and as questions for Niklas (section 8), and the records it
would produce are proposed in section 5.7. Every number marked PLACEHOLDER is for feel only (CLAUDE.md).

## 1. Summary

This phase closes the loop the concept board promises: **fight → wreck → salvage → research → unlock → build**.
In flight, your vessels strip parts off enemy wrecks into their cargo hold and bring the haul home by docking.
At home, salvaged parts are dismantled into research data (per tech tree) and materials, or kept and installed as
they are. Research unlocks tech-tree nodes (new parts, variants, size classes, refinements; no Mk levels), and
unlocked parts are fabricated from materials, consumed once (bonded matter). Designing and test flights stay free;
a real run needs every part built. Death stays mild: the ship re-forms at home, part of the haul stays behind. No
credits, no trading.

## 2. Already decided vs only sketched

### Decided (records)

| Topic | What is settled | Record |
|---|---|---|
| Research source | Dismantling salvaged parts gives research data for that part's family; alien nodes also need an artifact or lore; scans give data too; data counts only once home | [GDR-0025](../gdr/0025-research-from-salvage.md) |
| Salvage in flight | Loose debris by flying through it or a tractor beam; intact parts on big wrecks are cut free; cargo bays are parts (1×1 = 4 units, 2×2 = 20, placeholders) | [GDR-0025](../gdr/0025-research-from-salvage.md) |
| Tech trees | One tree per family (10 families), ~200 nodes; node kinds new part, variant, size class, refinement (I–V), alien; cross-links; all data | [GDR-0024](../gdr/0024-tech-trees-no-mk-levels.md) |
| Bonded matter, death | A new part consumes its materials once; a destroyed ship re-forms at home for free; the unbonded haul partly stays behind as a wreck; refit returns a removed part's matter; salvaged parts install as-is (small programmable-matter cost) or are dismantled | [GDR-0029](../gdr/0029-mild-death-bonded-matter.md) |
| No economy | No credits, no trading; build points stay as balance numbers, not prices | [GDR-0030](../gdr/0030-no-economy.md) |
| Materials, fabrication | No generic scrap; 30+ materials from places (harvest, salvage, races, precursor sites, missions); recipe chains resolved by the fabricator; hub upgrades; every logic core needs programmable matter (automaton cores are the first source); harvest tools are parts; unlimited home storage, the hold is the limit; fabrication only at home | [GDR-0031](../gdr/0031-materials-and-fabrication.md) |
| What is salvage | A piece without a working logic core is salvage; capture by killing all power | [GDR-0013](../gdr/0013-vessels-and-piloting.md), [GDR-0017](../gdr/0017-combat-support-repair-capture-sensors.md) |
| Home | Research lab, fabricator (parts and escorts), test range (simulated fights); the first hour salvages automaton wrecks | [GDR-0002](../gdr/0002-awakened-ship-mind-light-story.md), [GDR-0028](../gdr/0028-clusters-and-voyages.md) |
| Drydock today | Parts unlimited in v1; build points, device and control load are information; upgrade materials, variants and spare counts wait for this phase | [GDR-0051](../gdr/0051-drydock-v1.md), [GDR-0052](../gdr/0052-drydock-stats-full-use.md), [GDR-0053](../gdr/0053-drydock-v1-lead-defaults.md) |
| Locked entries | Parts and materials the player hasn't unlocked are hidden; which ones is content data (`locked`); content ships with locked parts still fly; salvage and research unlock later | [GDR-0054](../gdr/0054-drydock-locked-parts-tools-saving-sharing.md) |
| Content | RON packs, base loads like a mod, strict validation, schema version with migrations; stable ids, keyed text, no numbers in prose, wiki export | [ADR-0015](../adr/0015-ron-content-packs-and-mods.md), [ADR-0016](../adr/0016-wiki-ready-content-model.md) |
| Saves | Seed + generator version + deltas, versioned with migrations; stable content ids and RNG stream ids are part of the format | [ADR-0024](../adr/0024-saves-seed-and-deltas.md) |
| Player storage | Named, versioned RON documents in namespaces (`saves/` planned); migrations in `Document::from_ron` | [ADR-0031](../adr/0031-player-storage-and-design-sharing.md) |
| Determinism | Fixed step, seeded streams per system (append only), ordered containers, `sim::math` for trig, state hash | [ADR-0010](../adr/0010-cross-platform-determinism.md) |

### Only sketched (concept board, mockups, data draft)

- **Materials** (`docs/concept/boards/concept-board/09-materials.png`, `MATL` in `concept-board.html`): 46
  materials (11 harvested, 10 race, 4 precursor, 21 refined), each refined one with a facility ("via") and inputs.
- **Recipes**: 23 example parts in the recipe explorer (`RECIPES`), e.g. fusion reactor 2×2 = 2 fusion pellets,
  4 magnet coils, 2 superconductor, 4 steel plate. Numbers are placeholders on the board itself.
- **Trees** (`07-tech-trees.png`, `TREE`): one worked example (energy weapons, 17 nodes in five columns Start →
  Endgame, with "needs Thermal / Power / Lattice / Phase walkers" cross-links) and a one-line sketch with a node
  count (~15–25) for each of the 10 families.
- **Mockups**: the desktop drydock (`ui-mockups/desktop-drydock.png`) shows palette rows with `2 SPARE`, `CRAFT`
  and greyed `LOCKED · INNER BAND`, a variant picker (STANDARD / COMPACT TORUS / HOT-BURN) with a stat diff,
  UPGRADE MATERIALS "bonded once" with stored counts, `UPGRADE · 1 MISSING` and TRACK ON MAP. The home ring menu
  (`phone-home-ring-menu.png`) shows `RESEARCHING beam laser · focused 62 %`, `RESEARCH 3 READY`,
  `FABRICATOR 2 IN QUEUE`, `HAUL UNLOADED 14 parts · 9 materials`.
- `docs/concept/reference/data-draft/` has parts and ships only: no recipes, materials beyond armour, or tech.

### What the code has today

- `locked: bool` on `data::model::Part` and `Material` (`crates/core/data/src/content/model.rs`); set on `automaton_core` and
  `automaton_cannon` (`content/base/parts/automatons.ron`) and `automaton_plate` (`content/base/materials/armor.ron`).
  The drydock filters on it in `palette_parts` and `armor_materials` (`crates/client/game/src/drydock/state.rs`).
  Validation makes a locked paint block or bevel slope, or every armour material locked, an error.
- Salvage exists as a state, not as a resource: `Piece::salvage()` (= not the controlling piece,
  `crates/core/sim/src/ship/mod.rs`), `Ship::control: Option<PieceId>` (`None` = a wreck for good), `Ship::team()` is `None`
  for salvage while `Ship::owner()` is kept "for later salvage rights" (`crates/core/sim/src/ship/vessel.rs`, `docs/sim.md`
  "Owners"). Events: `PieceSnapshot::salvage`, `SimEvent::ControlLost`, `VesselLost`, `PartDestroyed { volatile }`
  (`crates/core/sim/src/events.rs`).
- `Stream::Debris = 2` ("debris and salvage generation", `crates/core/sim/src/rng.rs`) exists and is unused so far.
- Cargo capacity is compiled per part (`crates/core/sim/src/ship/design.rs`, `PartSpec` cargo) and shown by the drydock
  (`DesignReport::cargo`, `crates/core/sim/src/report.rs`). Nothing fills it.
- Wrecks are cleaned up by the client, not the sim: `crates/client/world/src/scene.rs` removes dark ships more than 320 m
  from the player or dark for 75 s; C removes raider vessels but their wrecks stay.
- Player data: `crates/client/world/src/storage.rs` (`Storage`, `Backend`, `Document { NAMESPACE, FORMAT, to_ron,
  from_ron(text, format) }`, header line `// space-game: <ns>, format N, saved T`). Namespaces in use:
  `designs/` (`DesignDoc`), `drydock/` (`WorkingDoc`, `WorkingBehavior`), `behaviors/` (`BehaviorDoc`). No save
  game, no inventory, no unlock state.
- `AppMode { Flight, Drydock, Behavior }` (`crates/client/world/src/mode.rs`); no home screen yet.
- Content kinds (`crates/core/data/src/content/kind.rs`): stat, category, faction, material, part, frame, ship, behavior_node,
  behavior, page. No tech, recipe or facility kind. `data::SCHEMA_VERSION` is 1.

### Records that look wrong or need care

- **The automaton core can't be salvaged as built.** GDR-0031 makes automaton cores the first source of
  programmable matter. But a raider has one core, which is also its only power source (`automaton_core`:
  `power: (output: 6)`), so a raider only becomes salvage when its core is destroyed (`ControlLost`). An intact
  core can only come from capture (GDR-0017: its core would have to overheat or be cut off from power, which a
  self-powered core never is). Not a wrong decision, but unreachable today: gap G3 / question Q3.
- **GDR-0029 "part of the haul stays behind"** doesn't say how much, nor what happens with several vessels: gap
  G4 / Q4.
- **The drydock mockup shows locked parts greyed** ("LOCKED · INNER BAND"); GDR-0054 hides them. GDR-0054 is the
  later decision and stays; the research screen is where upcoming parts are visible.
- **GDR-0051 "parts are unlimited"** was explicitly for v1. Real runs with costs need a new GDR that supersedes it
  for runs (designing and test flights can stay unlimited, Q6).
- The CLAUDE.md glossary says bonded matter is "programmable matter that a part consumes once"; GDR-0029 says a
  part consumes its *materials* once, bonding its matter. This plan follows the record.
- `docs/concept-notes.md` lists Progression as "Leaning" while GDR-0025 is `Accepted` (with "other sources"
  open). Harmless, but the table could say "Settled; other sources open".

## 3. Gaps

### For Niklas (asked as questions in section 8)

**G1. What salvage unlocks.** GDR-0029 lets salvaged parts be installed as they are; GDR-0054 says salvage or
research unlocks locked parts. Open: does salvage unlock only the copies you hold, or the part itself?
- *Copies usable, research for fabrication*: salvage pays at once (count-limited), research stays the way to
  make more. Two states to show in the palette.
- *First copy unlocks the part fully*: fastest; research loses its role for every part you can find.
- Recommendation: copies usable, research for fabrication (Q1).

**G2. How parts get into the hold.**
- *Strip wrecks part by part when close*, outermost first, automatically; big wrecks (over a part limit) must be
  shot apart, later cut with a tool. Works with a partly full hold; each part removed is one breakage update.
- *Absorb whole small pieces on touch*: simplest; a piece that doesn't fit whole is wasted.
- *Tractor beam only*: deliberate salvage builds, but nothing works until a new part exists.
- Recommendation: strip part by part (Q2). A tractor beam and a salvage cutter come later as utility parts that
  extend it (GDR-0025 names both).

**G3. Fragments of destroyed parts.** Most of a ship is destroyed, not salvaged, and the automaton core is
unreachable (above).
- *Destroyed parts sometimes leave a fragment of one of their materials* (a small loose pickup, flown through to
  collect); cores always leave programmable matter. Fights pay even when ships are shredded; matches "loose
  debris" in GDR-0025; one new cheap sim object. Not generic scrap: a fragment is the part's own material.
- *Only intact parts*: simplest; programmable matter needs another source (capture, precursor sites).
- *Only cores leave a fragment*: the minimal fix.
- Recommendation: fragments (Q3), built after the first slice (track SR-H).

**G4. Death and the haul.** The ship re-forms free (GDR-0029); open is how much haul stays behind.
- *A share stays as a recoverable wreck* (PLACEHOLDER half by hold units).
- *The haul of each lost vessel stays; vessels that get away keep theirs*: physical, but a lone ship loses all.
- *All lost for good*: harsh for mild death.
- Recommendation: a share as a recoverable wreck (Q4). In the first slice "recoverable" means only within the
  same run, since the sandbox has no persistent world.

**G5. Spending research.** GDR-0025 fixes data per family. Open: how it turns into nodes.
- *Pick any node whose needs are met; it unlocks at once.* The mockup's "62 %" becomes progress to a chosen
  target.
- *A target fills automatically* as hauls arrive.
- *Real-time timers*: idle-game pacing; wall-clock state in saves.
- Recommendation: pick and unlock at once (Q5).

**G6. When building costs apply.**
- *Designing and test flights free; launching a run needs every part built* (spares, salvaged copies or
  fabricated). Keeps the drydock playful; costs bite at launch.
- *Every edit takes and returns parts*: literal, but designing becomes bookkeeping and test flights cost.
- *Parts stay unlimited; materials only pay for unlocks*: drops GDR-0029's "consumes its materials once".
- Recommendation: costs at launch (Q6). The test range is simulated (GDR-0002), so test flights stay free.

**G7. Fabrication over time.** The mockup shows "2 IN QUEUE".
- *Instant at home; orders without materials wait and finish when a haul brings them.* No timers.
- *Each part takes time*: pacing, wall-clock state.
- *Instant only, no waiting orders*.
- Recommendation: instant with waiting orders (Q7). Waiting orders come with recipe chains (SR-J); the first
  slice fabricates instantly or refuses with what is missing.

**G8. Where the first loop lives.** There is no galaxy, voyage or home screen yet.
- *In the sandbox*: LAUNCH from the drydock starts a salvage run against raider waves; a dock ring at the start
  point brings the haul home.
- *Wait for the galaxy and home ring*: right place, nothing to play for a long time.
- *Sandbox, a key ends the run anywhere*: loses "bring it home".
- Recommendation: sandbox with a dock ring (Q8).

### Lead defaults (smaller; listed for review, no question needed)

- **G9. Own wrecks aren't salvage.** Parts that fall off your own ships are bonded matter: they re-form at home,
  so collecting them would count twice. Salvage = pieces of ships whose `owner()` differs from the collector's.
- **G10. Refit returns parts, not materials.** A part removed from a built ship goes to spares (the mockup's
  `N SPARE`); a spare can be broken down into its full materials (GDR-0029: refit returns the matter). Own parts
  never give research, so building and dismantling can't farm data.
- **G11. Hold units.** A salvaged part takes its cell area in hold units, rounded up (PLACEHOLDER). A fragment
  takes 1.
- **G12. Bonding salvaged parts.** Installing a salvaged copy costs programmable matter: 1 unit bonds 4 parts,
  the rest kept as credit (PLACEHOLDER; GDR-0029 says "small").
- **G13. Armour materials are resources.** The `steel` armour material is also the steel you stock: a hull part
  in steel consumes `ceil(area × per_cell)` steel. No separate "steel plate" entry. (The board has both; one
  entry is simpler for content and wiki.)
- **G14. Recipes live on the entries** (`fabricate` on parts, `make` on refined materials), not in a recipe
  kind: one place to look, as stats are. A recipe kind can come if alternate recipes are ever needed.
- **G15. Variants and size classes are their own parts** with `variant_of` (every variant gets its own
  silhouette, GDR-0024). Switching to a variant is a refit (old part to spares, new one fabricated); the
  inspector's "upgrade materials" is that refit's shopping list.
- **G16. Refinements are owner-wide stat steps** on a tag (all your lasers +3 % range per level), never items:
  there is one beam laser part, not "beam laser II". This is how "no Mk levels" and "I–V chains" fit together.
- **G17. Reverse-engineering enemy parts** needs at least one dismantled copy (`needs: [Dismantled(...)]`), the
  same shape as alien nodes needing an artifact.
- **G18. Start of a new game** comes from content (`start.ron`): the built ship, the known nodes, a stock. The
  first slice starts with the Halvex Runner built; the story start (a drone in a dead shipyard, GDR-0002) waits
  for the home phase.
- **G19. Tree size** (open in GDR-0024): the first slice has ~20 nodes over 9 trees, only for parts that exist.
  Growing the trees is content work after the loop proves fun.
- **G20. Code home.** Progression rules (unlocks, research, dismantling, launch plans, recipes) are pure and go
  into `data::progress`, next to `data::edit` (ADR-0030's precedent); the player's state is one document
  `saves/main` on ADR-0031's layer. A separate crate can come when galaxy state arrives (section 5.1).

## 4. Design proposal

### 4.1 The loop

1. **Launch.** In the drydock, LAUNCH turns the working design into your built ship. Parts already built stay;
   removed parts go to spares; missing parts come from spares, then salvaged copies (bonded with programmable
   matter), then the fabricator (if researched and the materials are there). If anything is missing, LAUNCH says
   what ("NEEDS 1 BEAM LASER (NOT RESEARCHED) · 2 CIRCUITS SHORT") and nothing changes. TEST-FLY stays free.
2. **Fight.** A salvage run is the sandbox: raider waves, your ship and its vessels. Aim points (`core`,
   `engines`, `weapons` in the target line) matter more now: shooting the core first leaves the cannon intact.
3. **Wreck.** A raider whose core dies is salvage (GDR-0013). Its remaining parts drift.
4. **Salvage.** Fly close: your vessel strips the wreck part by part into its hold, outermost first, a gold
   streak per part. Too-big wrecks are marked; shoot them apart. With fragments (Q3), destroyed parts also leave
   small loose pickups you fly through.
5. **Bring it home.** Fly back into the dock ring at the start point and press H (only with no hostile close).
   The haul is unloaded. Losing every vessel, or abandoning the run with T, follows the death rule (Q4): the ship
   re-forms at home for free, part of the haul stays behind.
6. **Sort.** The SALVAGE BAY lists what came home. Per part: DISMANTLE (research data for its tree plus some
   materials) or KEEP (a salvaged copy you can install). Hull parts are always melted into their armour
   material. "DISMANTLE ALL" for speed.
7. **Research.** The RESEARCH screen shows the trees, the data per tree and each node's state. A node whose
   needs are met unlocks at once (Q5).
8. **Unlock.** Unlocked parts and materials show in the drydock palette; parts you only hold as spares or
   salvaged copies show with their count.
9. **Build.** Design freely, test-fly freely, LAUNCH again: the fabricator makes what's missing from materials.

### 4.2 Rules

**Salvage in flight** (sim):
- A **collector** is a running vessel (not dark, not dormant) of an owner that collects (the player) with free
  hold units. AI owners don't collect.
- A **strippable piece** is a salvage piece of another owner's ship with at most the big-wreck limit of parts,
  whose hull comes within reach of the collector's hull (bounding circles).
- Every strip interval the collector takes one part from the nearest strippable piece: the part with the fewest
  linked alive neighbours (outermost), then the nearest, then the lowest index. If it doesn't fit, the smallest
  part that fits; if none fits, nothing.
- A removed part leaves through the breakage path (the piece may split) but is not "destroyed": no chain
  reaction, no `PartDestroyed`, a `PartSalvaged` event instead.
- The **hold** belongs to the vessel; its capacity is the cargo of its alive cargo parts. When capacity drops
  below the contents (a cargo bay shot off), the last collected items spill: lost in the first slice, loose
  pickups with SR-H.
- Fragments (if Q3): a destroyed non-hull part leaves a fragment with a chance; cores always leave programmable
  matter. Fragments drift, are collected within a pickup radius, and are removed by the client like wrecks.

**Home** (pure rules, `data::progress`):
- **Dismantling** a salvaged part gives research data = its build points to its tree, and half of its
  `fabricate` materials, each rounded up (PLACEHOLDERS; a part's `salvage` block can override both). It records
  the part as dismantled (for reverse-engineering).
- **Usable** part: researched, or held as a spare or salvaged copy. Usable armour material: researched (armour is
  stock, so holding it without research doesn't make it usable; G13).
- **Fabricable** part: researched, its facility built (later), its materials in stock.
- **Research** of a node: all `requires` done, all `needs` met (consumed items are taken), enough data in its
  tree; then the data is spent and the node's unlocks apply. Refinement levels cost `level × cost`.
- **Launch plan**: the multiset of `(part, material)` in the design minus the built ship's; additions from bonded
  spares, then salvaged copies (bond), then fabrication; removals to spares. All or nothing.
- **Death or abandon**: the death rule picks what of the hauls stays (Q4); the built ship is unchanged (it
  re-forms).

### 4.3 First-slice content (all PLACEHOLDER)

Materials (7): `steel`, `composite`, `reactive` (armour and stock), `automaton_plate` (armour; salvage only),
`circuits`, `precursor_alloy`, `programmable_matter`. Sources in the slice: the start stock, dismantling salvage
(raider plate → automaton plate; cannon and thruster → steel and circuits; core → circuits, precursor alloy,
programmable matter), breaking down own spares. Composite and reactive come only from the start stock until
harvesting exists; that is accepted for the slice and flagged by the reachability check (section 5.2).

Trees and nodes (~20 over 9 trees): known from the start, the parts and materials the Halvex Runner and a basic
ship need (armour block, slopes, steel, composite, pulse laser, fission reactor, capacitor, radiator, thruster,
gyro, shield emitter, logic core, cargo bay). Researchable: beam laser, fusion reactor, torch drive, RCS pod,
cargo hold, reactive armour (Early, 6–12 data), and the three automaton entries (automaton plate, automaton
cannon, automaton core; each needs one dismantled copy). The parts the Runner uses but you haven't researched are
usable as long as you hold them (built or spare). Which base parts start locked is a content choice for the lead
and Niklas.

### 4.4 Placeholder numbers

| What | Value (PLACEHOLDER) | Notes |
|---|---|---|
| Hold units of a salvaged part | cell area, rounded up (1×1 = 1, slope 1×1 = 1, 2×2 = 4) | cargo bay 4, cargo hold 20 (content) |
| Reach for stripping | 3 m between hulls (bounding circles) | a tractor beam part raises it later |
| Strip rate | one part every 10 ticks (3 per second) per collector | |
| Big wreck | a piece of more than 12 parts | shoot it apart; a cutter later |
| Research data from dismantling | the part's build points, rounded | raider: core 5, cannon 3, thruster 3, plate block 1 |
| Materials from dismantling | half of `fabricate`, each rounded up | own spares: all of it |
| Bonding a salvaged copy | 1 programmable matter per 4 parts | rest kept as credit |
| Node cost by era | Start 0, Early 6–12, Mid 20–40, Late 60–100, Endgame 150+ | refinement level n: n × cost |
| Refinement step | +3 % of one stat per level | |
| Death (if Q4 as recommended) | half the haul by hold units stays | which half: the last collected |
| Fragments (if Q3) | 50 % per destroyed non-hull part; cores 100 % | pickup radius 4 m, 1 hold unit each |
| Dock ring | 25 m around the start point, no hostile vessel within 60 m | |
| Start stock | Runner built; steel 40, composite 20, reactive 10, circuits 10, precursor alloy 4, programmable matter 2 | |

## 5. Technical plan

### 5.1 Crates and where things go

| Piece | Crate / file | Notes |
|---|---|---|
| New content kinds and fields, validation, export | `data` (`model.rs`, `kind.rs`, `load.rs`, `validate.rs`, `stats.rs`), `content-tool` | additive; schema 2 |
| Progression rules and the profile type | `data::progress` (new module, pure) | no I/O, integer amounts, `BTreeMap`/`Vec` only; used by `game`, later `net` and `arena` |
| Holds, stripping, fragments | `sim::salvage` (new module), `world.rs`, `events.rs`, `config.rs`, `hash` | deterministic, hashed |
| Owner-wide refinements | `sim::design` (`ShipDesign::compile` gains modifiers), `sim::report` | later track |
| Runs, HUD, FX, screens, the save document | `game` (`scene.rs`, `flight_hud.rs`, `fx.rs`, new `shipyard/` screens, `storage.rs` namespace `saves`) | |

A new crate (`campaign`) was considered for the rules. It would keep `data` about content, but today the rules
are small, pure and content-shaped, like `data::edit`. Proposed ADR: start in `data::progress`, move to a crate
when galaxy state (seed, deltas, ADR-0024) joins the profile.

### 5.2 Content model additions

New kinds (dirs follow the existing rule `<dir>.ron` or `<dir>/**`): `tree` (`trees.ron`), `tech`
(`tech/*.ron`), `start` (`start.ron`), later `facility` (`facilities.ron`). Text keys `tree.<id>.name`,
`tech.<id>.summary`, ...; links `[[tech:beam_laser]]`, `[[tree:energy_weapons]]`; wiki prose `wiki/tech/<id>.md`.

Materials become resources (content-model.md already says fabrication resources will be materials without an
`armor` block):

```ron
// content/base/materials/resources.ron. PLACEHOLDER NUMBERS.
[
    (
        id: "circuits", name: "Circuits", summary: "Etched logic for guns, shields and cores.",
        icon: "material-circuits", tags: ["refined"],
        // later (SR-J): how the fabricator makes it from other materials, and where
        make: (from: [("wiring", 2), ("silicates", 1)], at: "electronics_bench"),
    ),
    (
        id: "programmable_matter", name: "Programmable matter", tags: ["precursor"],
        summary: "Precursor matter that can host a copy of the ship-mind.", icon: "material-programmable-matter",
    ),
]
// content/base/materials/armor.ron: an armour material is also a stock resource
(
    id: "steel", name: "Steel", summary: "Cheap, heavy plating.", icon: "armor-steel",
    armor: (tone: "steel", mass_per_cell: 2, hp_per_cell: 1, build_point_factor: 1,
            per_cell: 1),   // new, default 1: a hull part in steel uses ceil(area × per_cell) steel
),
```

Parts gain `fabricate`, `salvage` and `variant_of`:

```ron
(
    id: "beam_laser", /* ... as today ... */
    fabricate: (from: [("steel", 2), ("circuits", 2), ("precursor_alloy", 1)]),  // consumed once (bonded)
    // optional; defaults: research = build points, materials = half of `fabricate` rounded up, no drops
    salvage: (research: 7),
),
(
    id: "automaton_core", /* ... */ locked: true,
    fabricate: (from: [("circuits", 2), ("programmable_matter", 1)]),
    salvage: (drops: [("programmable_matter", 1)]),   // fragment of a destroyed core (SR-H, Q3)
),
(
    id: "pulse_laser_rapid", /* ... */ locked: true,
    variant_of: "pulse_laser",                        // the inspector's variant picker
),
```

Hull parts have no `fabricate`: their cost comes from the armour material. `fabricate.at` (a facility) comes with
SR-J.

Trees and nodes:

```ron
// content/base/trees.ron
[
    (id: "energy_weapons", name: "Energy weapons", summary: "Lasers, beams and lances.", color: "weapons"),
    (id: "kinetic_weapons", name: "Kinetic weapons", summary: "Guns that throw mass.", color: "weapons"),
]
// content/base/tech/energy_weapons.ron. PLACEHOLDER COSTS.
[
    (id: "pulse_laser", name: "Pulse laser", summary: "The first fixed laser.", tree: "energy_weapons",
     kind: NewPart, era: Start, unlocks: [Part("pulse_laser")]),
    (id: "beam_laser", name: "Beam laser", summary: "A continuous beam: strong, hungry, hot.",
     tree: "energy_weapons", kind: NewPart, era: Early, requires: ["pulse_laser"], cost: 12,
     unlocks: [Part("beam_laser")]),
    (id: "focusing_optics", name: "Focusing optics", summary: "Lasers reach further.", tree: "energy_weapons",
     kind: Refinement(levels: 5), era: Early, requires: ["pulse_laser"], cost: 6,
     effects: [(target: Tag("laser"), stat: "range", effect: Percent(3))]),          // per level (SR-I)
    (id: "cooling_jackets", name: "Cooling jackets", summary: "Lasers shed heat faster.", tree: "energy_weapons",
     kind: Refinement(levels: 3), era: Mid, requires: ["focusing_optics", "heat_sinks"], cost: 10,  // cross-link
     effects: [(target: Tag("laser"), stat: "heat_output", effect: Percent(-4))]),
    (id: "prism_array", name: "Prism array", summary: "Splits one beam into many.", tree: "energy_weapons",
     kind: Alien(race: "lattice"), era: Late, requires: ["focused_beam"], cost: 60,
     needs: [Material("resonant_crystal", 1)], unlocks: [Part("prism_array")]),
]
// content/base/tech/kinetic_weapons.ron
[
    (id: "automaton_cannon", name: "Automaton cannon", summary: "Rebuild the raiders' nose gun.",
     tree: "kinetic_weapons", kind: NewPart, era: Early, cost: 9,
     needs: [Dismantled("automaton_cannon")], unlocks: [Part("automaton_cannon")]),
]
```

| Node field | Notes |
|---|---|
| `tree` | tree id; a part's tree (for research data) is the tree of the node that unlocks it |
| `kind` | `NewPart`, `Variant`, `SizeClass`, `Refinement(levels)`, `Alien(race: <faction id>)`: the UI's border style, and levels |
| `era` | `Start`, `Early`, `Mid`, `Late`, `Endgame`: the column, a cost guide, and later the arena's enemy pools (GDR-0030 scaling) |
| `requires` | node ids, any tree (cross-links) |
| `needs` | `Dismantled(part)`, `Material(id, n)` (consumed), later `Lore(id)` |
| `cost` | research data of its own tree (per level for refinements) |
| `unlocks` | `Part(id)`, `Material(id)`; later `Frame(id)` (drydock expansions, GDR-0006), `Facility(id)`, `MindCapacity(n)` (GDR-0013) |
| `effects` | refinements only: the existing bonus shape (`Selector`, stat id, `Effect`), applied to every ship of the owner |
| `row` | optional layout hint for the tree view |

Start:

```ron
// content/base/start.ron. PLACEHOLDER.
(
    id: "default", name: "New game", summary: "Wake at the rim with a Halvex Runner.",
    fleet: ["halvex_runner"],
    research: ["steel_plating", "composite_plating", "pulse_laser", "fission_reactor", "logic_core" /* ... */],
    materials: [("steel", 40), ("composite", 20), ("reactive", 10), ("circuits", 10),
                ("precursor_alloy", 4), ("programmable_matter", 2)],
)
```

`locked` keeps its meaning ("not available at the start") and GDR-0054's drydock rule; the tech node that unlocks
an entry is now named in content too.

Validation (errors unless noted): node references exist; `requires` has no cycles; `Refinement` levels 1–5 and
effects name bonus-capable stats; `Alien.race` is a faction; a part or material unlocked by two nodes; `make`
recipes form no cycle and name existing materials and facilities; amounts are positive integers; warnings for a
locked entry no node unlocks, a node that unlocks nothing, and a material with no source (no `make`, no part
yields or drops it, not in the start stock, not harvestable). The start entry's ship validates and its research
names root nodes.

Stats (labels in `stats.ron`, values from content): `research_data` (what dismantling gives), `hold_units`
(space a salvaged part takes). `export-json` adds trees, nodes (with resolved `unlocks`, `used_by`), recipes
with raw totals (the board's recipe explorer, for the wiki), and per material where it comes from.

Schema: the changes are additive, but ADR-0015 says a schema change raises `schema_version`; proposed: 2, with
packs at 1 still loading (nothing to migrate).

### 5.3 Simulation

New module `sim::salvage`, a new step phase `StepPhase::Salvage` after `Impacts` and before `Hash`:

```rust
/// Something a vessel can carry home. Small integer ids from `SalvageCatalog` (content ids sorted, built once,
/// as `BehaviorLibrary` does for behaviors).
pub enum HaulItem { Part { part: ItemId, material: Option<ItemId> }, Material(ItemId) }

/// A vessel's hold: entries in collection order (spills take the last first), used units.
pub struct Hold { entries: Vec<(HaulItem, u32)>, units: u32 }

impl Sim {
    pub fn set_salvage(&mut self, catalog: SalvageCatalog);       // like set_behaviors
    pub fn hold(&self, ship: ShipId) -> Option<&Hold>;
    pub fn take_hold(&mut self, ship: ShipId) -> Hold;             // docking (a TickInputs command, so replays see it)
    pub fn drop_haul(&mut self, ship: ShipId, keep: ...) -> ...;   // the death rule, as a command too
}

SimEvent::PartSalvaged { from: ShipId, piece: PieceId, part: PartIdx, by: ShipId, item: HaulItem }
SimEvent::HoldSpilled { ship: ShipId, item: HaulItem, count: u32 }
// SR-H: SimEvent::PickupSpawned / PickupCollected / PickupGone, `Sim::pickups()`
SimConfig::salvage: SalvageTuning { reach, strip_interval, big_wreck_parts, collects: [bool; MAX_OWNERS] }
```

- **Fits the existing code.** Holds live on `Ship` (they belong to the vessel; a split-off vessel starts empty;
  the kept vessel spills if its cargo shrank). Strippable pieces come from `Ship::pieces()` with
  `Piece::salvage()` or `Ship::control == None`, filtered by `Ship::owner()` (G9). Removing a part reuses the
  breakage resolution (`pending_destroyed` in `breakage.rs`) through a second list for salvaged parts, so splits,
  `PieceChanged` and collider updates stay one code path; salvaged parts never start chain reactions.
  `PartSpec` already carries `cargo`; it gains `hold_units` and the catalog item.
- **Determinism.** Collectors in ascending `ShipId`, candidates in ascending `(ShipId, PieceId)`, part choice by
  `(linked neighbours, distance², PartIdx)`; `Vec` and sorted ids only. Distances use plain IEEE ops (no trig).
  Fragment drops and scatter use `Stream::Debris` (unused today, so no other system's numbers move); a scatter
  direction comes from `sim::math` (libm) or rejection sampling in the unit square. Holds, the salvage tuning and
  (later) pickups go into `hash_state`. A new id counter for pickups changes every state hash once; run
  `scripts/determinism-check.sh` native vs wasm in the same change.
- **Performance budget** (to measure): stripping checks run every `strip_interval` ticks, bounding circles first;
  with nothing to strip they should stay around 1 µs per tick (the 20-ship load-test step is 0.011 ms median,
  `results/spike-summary.md`). A strip costs about what losing a part costs today (0.05 ms, 0.1 ms with a split,
  same file), at most 3 per second per collector. Fragments are points, not Rapier bodies: O(collectors ×
  pickups) per check, sort-and-sweep if counts grow past a few hundred.
- **Report.** `sim::report` keeps `cargo`; the drydock shows hold units next to it.
- **Refinements (SR-I).** `ShipDesign::compile(content, blueprint)` gains an owner's modifiers (the `effects` of
  researched refinement levels), applied after content stats and before adjacency bonuses; designs are cached
  per (blueprint, modifier set). `sim::report` takes the same modifiers, so the drydock shows real numbers. Co-op
  sends each player's modifiers with their designs.

### 5.4 Progression rules (`data::progress`)

```rust
pub struct Profile { /* the save's player state, serde (section 5.6) */ }
pub struct Unlocks<'a> { /* content + profile */ }
impl Unlocks<'_> {
    pub fn part_usable(&self, id: &str) -> bool;          // researched or held
    pub fn part_fabricable(&self, id: &str) -> Fab;        // Yes, NotResearched, Missing(Vec<(material, n)>)
    pub fn material_usable(&self, id: &str) -> bool;
    pub fn held(&self, part: &str, material: Option<&str>) -> Held; // { bonded, salvaged }
}
pub fn node_state(content: &Content, p: &Profile, node: &str) -> NodeState; // Done(level), Available, Blocked(Vec<Need>)
pub fn research(content: &Content, p: &mut Profile, node: &str) -> Result<(), Refusal>;
pub fn dismantle(content: &Content, p: &mut Profile, item: &ItemKey, n: u32) -> Result<Yield, Refusal>;
pub fn unload(p: &mut Profile, haul: &[(ItemKey, u32)]);                  // into the salvage bay
pub fn launch_plan(content: &Content, p: &Profile, design: &Blueprint) -> LaunchPlan; // spares, bond, fabricate, returns, missing
pub fn apply_launch(p: &mut Profile, plan: &LaunchPlan) -> Result<(), Refusal>;
pub fn fabricate_plan(content: &Content, p: &Profile, needs: &[(String, u32)]) -> FabPlan; // chains with SR-J
```

Pure, integer amounts, ordered containers, errors as values with words for the status line (as `data::edit`'s
`Refusal`). Tests: every rule on hand-made profiles; "N runs to unlock X" from measured raider yields (section 7).
`state::palette_parts` / `armor_materials` change from `!locked` to `Unlocks::part_usable` /
`material_usable`; a URL parameter `?unlock=all` keeps today's behaviour for development.

### 5.5 Client

Flight (`scene.rs`, `flight_hud.rs`, `fx.rs`):
- A **salvage run** scene next to the test flight: started by LAUNCH (bottom bar of the drydock), raider waves on,
  the dock ring at the start point (a dotted ring, `docs/style-guide.md`), `DOCK · H` when inside and safe.
- HUD: `HOLD 6/8` beside the hull map; strippable wreck pieces outlined gold, too-big ones coral `TOO BIG`;
  one short gold streak per `PartSalvaged` from the part to the collector's hull; fragments as small glowing
  diamonds in their material's colour (vector meshes, ADR-0021).
- End of run: H docks (`take_hold` for every own vessel), death or T abandons (the death rule); the haul goes
  into the profile's salvage bay; back to the drydock. The wreck cleanup keeps a wreck alive while a collector is
  within reach.

Home screens (until the ring menu exists, buttons in the drydock's bottom bar like BEHAVIOR; `AppMode` gains
`Research`, the others are overlays like BLUEPRINTS):
- **SALVAGE BAY**: "HAUL UNLOADED 14 PARTS · 9 MATERIALS"; per part DISMANTLE (shows the yield first) / KEEP;
  DISMANTLE ALL.
- **STORAGE**: materials with counts, spares and salvaged copies, BREAK DOWN for spares.
- **RESEARCH**: the tree list with data per tree, one tree as columns Start → Endgame like the concept board
  (node kinds as border styles, cross-links dashed), node detail with its unlocks' stat lines from content and
  what blocks it, RESEARCH.
- **Drydock**: palette rows `2 SPARE`, `1 SALVAGED`, `CRAFT` (researched, none held); locked stays hidden
  (GDR-0054); the status line adds the launch check; LAUNCH next to TEST-FLY. The variant picker and upgrade
  materials come with SR-K.

### 5.6 The player save

- One document **`saves/main`** (namespace `saves`, ADR-0031), `ProfileDoc` with `FORMAT = 1`, written after
  docking, after every research, dismantle, fabricate and launch, and when the desktop app closes. One document
  keeps the state consistent (the files backend writes atomically; local storage writes one key). A few KB.
- Shape (RON, ids not indices):

```ron
// space-game: saves, format 1, saved 1760351234
(
    packs: [("base", "0.1.0")],          // what it was played with, for messages
    runs: 12,                            // run counter; a run's sim seed derives from it (replays)
    research: (
        data: {"hull": 4, "kinetic_weapons": 7, "logic": 2},
        done: {"automaton_plate": 1, "beam_laser": 1, "focusing_optics": 2},  // node → level
        dismantled: ["automaton_cannon", "automaton_core"],
    ),
    materials: {"steel": 31, "circuits": 6, "programmable_matter": 2, "automaton_plate": 9},
    spares: [
        (part: "beam_laser", bonded: 1),
        (part: "automaton_cannon", salvaged: 2),
        (part: "armor_block", material: "steel", bonded: 4),
    ],
    bond_credit: 3,
    bay: [(part: "automaton_core", count: 1), (material: "programmable_matter", count: 1)],
    fleet: [(name: "Halvex Runner copy", design: (/* the content ship format, as designs/ */))],
)
```

- **Versioning** (ADR-0024, ADR-0031): new fields get serde defaults (no format bump); renames or changed meaning
  raise `FORMAT` and migrate in `ProfileDoc::from_ron`. A test reads a stored format-1 fixture in every later
  format. Content ids never change (ADR-0016); entries whose content is gone (a mod removed) are kept in the
  document, ignored, and named on the status line ("3 ITEMS NEED PACK lances"), so adding the mod back restores
  them.
- **Relation to ADR-0024**: this is the player part of a save. The galaxy seed, generator version and deltas
  join it with the galaxy phase (same document or `saves/main-galaxy`; decided then). Haul wrecks left in a
  cluster become galaxy deltas then.
- `designs/` stays as it is: designs are free drawings; the profile's `fleet` holds what is built.

### 5.7 Proposed records

- GDR (Proposed): **Salvage in flight**: stripping when close, hold units, big wrecks, own parts are not salvage,
  docking brings the haul home (G2, G8, G9, G11; Q2, Q8).
- GDR (Proposed): **Fragments of destroyed parts**, cores leave programmable matter (G3; Q3). Note that it
  answers GDR-0031's "automaton cores are the first source".
- GDR (Proposed): **What death costs of the haul** (G4; Q4), refining GDR-0029.
- GDR (Proposed): **Unlock and build rules**: salvaged copies usable, research for fabrication, designing and test
  flights free, launch needs built parts, refit returns spares, bonding cost (G1, G6, G10, G12; Q1, Q6).
  Supersedes GDR-0051's "unlimited parts" for runs.
- GDR (Proposed): **Research and fabrication over time** (G5, G7; Q5, Q7).
- GDR (lead default): **First-slice trees and numbers** (sections 4.3, 4.4), refinements as owner-wide steps (G16),
  variants as parts (G15), reverse-engineering (G17).
- ADR (Proposed): **Progression rules in `data::progress`; the profile document `saves/main`** (G20, 5.4, 5.6).
- ADR (Proposed): **Salvage is sim state**: holds and pickups hashed, `Stream::Debris`, `StepPhase::Salvage`,
  docking and haul drops as tick inputs (5.3).
- ADR (Proposed): **Content schema 2**: kinds `tree`, `tech`, `start` (later `facility`), resource materials,
  recipes on entries (G13, G14, 5.2).

## 6. Tracks

Track ids use the prefix SR (the lead picks the phase number). Each track updates its docs (`content-model.md`,
`sim.md`, `client.md`, `drydock.md`) and keeps determinism native vs wasm IDENTICAL.

**First slice** (fight → wreck → strip → dock → dismantle → research → unlock → launch with fabrication):

| Track | Goal | Depends on | Size | Parallel with |
|---|---|---|---|---|
| SR-A Content schema | Kinds `tree`, `tech`, `start`; `fabricate`, `salvage`, `variant_of` on parts; `per_cell` on armour; resource materials; validation (cycles, coverage, reachability); stats; export-json; schema 2. Types land first in a small commit | — | M | SR-C |
| SR-C `sim::salvage` | Holds, stripping, `PartSalvaged`, spill, `take_hold` and haul drop as inputs, tuning, hash, `StepPhase::Salvage`; tests (rules, splits, no chain reaction, replays), determinism check, timing numbers | — | M | SR-A, SR-B, SR-D |
| SR-B Slice content | The 7 materials, `fabricate` for the 20 parts, ~20 nodes in 9 trees, `start.ron`, which base parts start locked; all PLACEHOLDER | SR-A types | S | SR-C, SR-D, SR-E |
| SR-D `data::progress` | `Profile` (RON), `Unlocks`, node states, research, dismantle, unload, launch plan (spares, bonding, flat fabrication), death rule on hauls | SR-A types | M | SR-B, SR-C, SR-E |
| SR-E Salvage runs (flight) | Run scene, LAUNCH without costs at first, dock ring and H, HOLD in the HUD, wreck outlines, streak FX, death and abandon, haul hand-off, cleanup keeps wrecks being stripped | SR-C | M | SR-B, SR-D |
| SR-F1 Profile and drydock | `ProfileDoc` on `saves/main`, unlocks replace `!locked`, palette counts, launch check and apply, SALVAGE BAY and STORAGE overlays, `?unlock=all` | SR-D, SR-B, SR-E | M | SR-F2 |
| SR-F2 Research screen | `AppMode::Research`, tree view in the board's look, node states and detail, RESEARCH | SR-D, SR-B | M | SR-F1 |
| SR-G Slice play-test | The whole loop headless and in the browser; raider yields measured; placeholders tuned; `results/salvage-slice.md`; the records drafted for Niklas | SR-F1, SR-F2 | S | — |

Waves: 1) SR-A and SR-C. 2) SR-B, SR-D and SR-E. 3) SR-F1 and SR-F2. 4) SR-G.

**Later** (each after the slice; independent of each other unless noted):

| Track | Goal | Depends on | Size | Parallel with |
|---|---|---|---|---|
| SR-H Fragments and haul wrecks | Loose pickups from destroyed parts (`Stream::Debris`), fly-through pickup, spills and the death share as pickups (if Q3 yes) | SR-C, SR-E | M | SR-I, SR-J, SR-L |
| SR-I Refinements | Owner modifiers in `ShipDesign::compile` and `sim::report`, levels I–V on the research screen | SR-D, SR-F2 | M | SR-H, SR-J, SR-L |
| SR-J Chains and facilities | Raw materials, `make` recipes, the `facility` kind and its screen, the chain resolver, orders waiting for materials | SR-D, SR-F1 | M | SR-H, SR-I, SR-L |
| SR-K Variants and the inspector | Variant parts, the variant picker with stat diff, upgrade materials "bonded once", "missing → where to find it" (text until the galaxy map) | SR-F1, SR-J | M | SR-L |
| SR-L Salvage tools | Tractor beam (more reach, pulls pieces) and salvage cutter (strips big wrecks) as utility parts | SR-C, SR-E | M | SR-H, SR-I, SR-J, SR-K |
| SR-M Real trees | Grow trees with new parts and variants (content, icons, wiki prose) once the loop is fun | SR-G | L | any |

Waiting on other phases: capture as a salvage route (GDR-0017), scanning for research (sensors), harvesting and
hazard materials (galaxy), alien nodes and race materials (races), the home ring menu, "track on map" (galaxy
map), escorts built from the profile's `fleet`.

## 7. Risks, and what to prototype or measure first

1. **Raider yields** (measure first, before SR-B's numbers): a headless sim test that runs ~50 Runner-vs-raider
   fights (fixed seeds) and counts, per wreck, which parts survive. Raiders are 3×3 with the core in the middle,
   so a dead raider may leave only plate and a thruster; the cannon sits in the nose and often dies first. This
   decides Q3's weight and the data costs.
2. **A paper run of the economy** (in `data::progress` tests): feed measured yields into the rules and print after
   how many runs each node unlocks and whether a material runs dry. Target (PLACEHOLDER): the first enemy-part
   unlock within 2–3 runs.
3. **Strip cost**: time the per-tick check (budget ~1 µs with nothing to strip) and a strip (breakage + collider
   rebuild) on raider wrecks and on a 64×64 wreck (expected near today's 0.05–0.1 ms per lost part).
4. **Dead ends without harvesting**: composite and reactive have no source in the slice. The reachability warning
   makes it visible; accept for the slice, or swap the start recipes.
5. **Determinism**: new hashed state and a new id counter move every hash once; the native-vs-wasm check must
   stay IDENTICAL; docking and haul drops must be tick inputs, not client-side edits of sim state.
6. **Clarity of three things**: a design (free), the built ship (costs), spares. The status line wording and the
   LAUNCH check need a look in the browser early (SR-F1 screenshots).
7. **Grind and tedium**: data per node and the 2–3-run target are guesses; GDR-0024's "tree size that stays fun"
   is still open. Keep the slice small and tune after SR-G.
8. **Save churn**: one document, serde defaults, a format-1 fixture test from day one.
9. **Breakage reuse**: salvaged reactors must not explode once chain reactions exist; a test pins it.
10. **Scope**: 46 materials and 200 nodes are a sketch. Build the systems with ~7 materials and ~20 nodes
    (CLAUDE.md: minimal content first).

## 8. Questions for Niklas

Each is self-contained; the recommended option is first.

**Q1. You salvage a part you haven't researched (say an automaton cannon). What can you do with it?**
- Install the salvaged copies right away (count-limited); fabricating more needs research (Recommended). —
  Salvage pays at once and research stays the way to make more.
- The first salvaged copy unlocks the part fully, fabrication included. — Fastest progress, but research loses
  its role for every part you can find.

**Q2. How do your ships pick up salvage in flight?**
- Automatically strip wrecks part by part when close, outer parts first; wrecks over ~12 parts must be shot
  apart until a cutter part exists (Recommended). — Readable, no extra buttons, works with a half-full hold.
- Absorb whole small pieces on touch. — Simplest, but a piece that doesn't fit whole is wasted.
- Only with a tractor beam part you build. — Deliberate salvage builds, but nothing works until that part
  exists.

**Q3. Should destroyed parts leave small loose fragments of their material to fly through?**
- Yes: sometimes for any machine, always programmable matter from cores (Recommended). — Fights pay even when
  ships are shredded, and it is the only easy source of programmable matter: an intact automaton core is
  nearly impossible to salvage, because its core keeps a raider alive.
- No: only intact parts of wrecks are salvage. — Simplest; programmable matter then needs capture or precursor
  sites.
- Only cores leave a fragment. — Fixes programmable matter, no loose debris otherwise.

**Q4. Your ship is destroyed on a run. It re-forms at home for free (decided). What happens to the haul?**
- Half of it (placeholder) stays behind as a wreck you can recover (Recommended). — Tension without wiping the
  whole run; matches "part of the haul stays behind".
- The haul of each destroyed vessel stays; vessels that got away keep theirs. — Physical and rewards escorts,
  but a lone ship loses everything.
- All of it is lost for good. — Strongest tension; harsh for a mild-death game.

**Q5. How do you spend research data (one pool per tech tree, decided)?**
- Pick any node whose needs are met; it unlocks at once (Recommended). — No waiting, your choice; the home
  screen can still show progress to a chosen target.
- Pick a target; data flows into it as hauls arrive. — Less clicking, less control.
- Research takes real time. — Classic pacing, but feels like an idle game and needs clock time in saves.

**Q6. When do building costs apply?**
- Designing and test flights stay free; launching a real run needs every part built (spares, salvaged copies or
  fabricated) (Recommended). — The drydock stays playful; costs bite once, at launch.
- Every drydock edit takes and returns parts at once. — Closest to "building", but designing becomes
  bookkeeping and test flights cost.
- Parts stay unlimited; materials only pay for unlocks. — Simplest, but drops "a part consumes its materials
  once".

**Q7. How does the fabricator work over time?**
- Instant at home when the materials are there; an order without them waits and finishes when a haul brings
  them (Recommended). — No timers; the mockup's "2 IN QUEUE" are waiting orders.
- Each part takes time to make, in a queue. — Pacing, but waiting and clock state.
- Instant only, no waiting orders. — Simplest; you remember what is missing yourself.

**Q8. Where should the first playable salvage loop live?**
- In today's sandbox: LAUNCH from the drydock starts a salvage run against raider waves; flying back into a dock
  ring at the start point brings the haul home (Recommended). — Playable soon; no galaxy needed.
- Wait for the galaxy and the home ring menu, and build salvage into real voyages. — The final shape, but
  nothing to play for a long time.
- In the sandbox, but a key ends the run anywhere (no dock). — Simplest; loses the "bring it home" tension.
