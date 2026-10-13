# Career

The player's progression in the first playable loop (GDR-0062, GDR-0063, ADR-0033): what a new game has, what
research unlocks, how a haul becomes research data, how voyages are launched, seeded and ended, when a fleet may
launch, and the save that holds it all. Written 13 Oct 2026 (track 7C).

This is its own document rather than a section of `content-model.md` because the career is not content: it is the
player's state and its rules, and the save is a storage format. The content it reads (families, research nodes,
starts, salvage blocks, `locked`) is described in `content-model.md`.

## Where the pieces live

| Piece | Where | Doc |
|---|---|---|
| The model: the save's facts, the rules, unloading, voyage seeds, readiness | `data::career` (`crates/data/src/career.rs`), tests `crates/data/tests/career.rs` | below |
| The content it reads | `Content.families`, `.research`, `.starts`, `Part::salvage`, `Material::salvage`, `locked` | `content-model.md` "Salvage", "Research families", "Research nodes", "Starts" |
| Cargo and collecting (the items a haul is made of) | `sim::salvage` (track 7B) | `sim.md` "Salvage" |
| Voyages as content, rolling and running them | `Content.voyages`, `.encounters`; `sim::voyage` (track 7E) | `content-model.md` "Voyages", "Encounters"; `sim.md` "Voyages" |
| The save document, new game, the switches, gating the drydock, FLEET | `game::career` (`crates/game/src/career.rs`), `saves/<slot>` in `game::storage`; `drydock/`, `fleet.rs` (track 7F) | [below](#where-the-game-keeps-the-save), `client.md` "Career", "FLEET", `drydock.md` |

The model is pure, like `data::edit` for the drydock (ADR-0030): no Bevy, no I/O, wasm-safe, ordered containers
only. Every question takes the `Content` it should be answered against.

## The save (format 1)

`Career` holds **facts only**. Everything that can be derived from content is derived on demand and never stored,
so a content rebalance (a node's cost, a part's unlock) applies to old saves without a migration. The game stores
`Career::to_ron()` as a document in `saves/` (ADR-0031 adds the header line) and reads it back with
`Career::from_ron(text)` followed by `Career::reconcile(&content)`. `from_ron` takes a blueprint's input limits
(at most `limits::MAX_DOCUMENT_BYTES`, a leading byte order mark skipped).

```ron
// space-game: saves, format 1, saved 1760351234
(
    packs: [
        ("base", "0.1.0"),
    ],
    start: "rim_awakening",
    seed: 4207,
    generator: 2,
    researched: [
        "automaton_cannon",
        "tamed_core",
    ],
    data: {
        "hull": 8.0,
        "logic": 12.0,
    },
    materials: {
        "programmable_matter": 6,
    },
    escorts: [
        "drone_wing",
    ],
    seen: [
        "automaton_cannon",
    ],
    voyages: {
        "yard_approach": (
            launched: 2,
            cleared: true,
            best_ticks: 12000,
        ),
    },
    log: (
        voyages: 2,
        recalled: 2,
        lost: 0,
        salvaged: 21,
    ),
    last_haul: (
        voyage: "yard_approach",
        outcome: Cleared,
        ticks: 12000,
        report: (
            parts: {
                "automaton_core": 5,
            },
            data: {
                "logic": 20.0,
            },
            materials: {
                "programmable_matter": 5,
            },
        ),
    ),
)
```

| Field | Meaning |
|---|---|
| `packs` | the packs `(id, version)` it was last played with; informational (a note when one is missing at load) |
| `start` | the content start it began from |
| `seed` | the game seed (ADR-0024); voyage seeds come from it |
| `generator` | the voyage roller's version (`career::GENERATOR`, 2); see [Voyage roller versions](#voyage-roller-versions) |
| `researched` | research node ids, in the order researched |
| `data` | research data in store per family (f32, only amounts above 0) |
| `materials` | materials in store (whole numbers, only amounts above 0) |
| `escorts` | saved design ids (`designs/`) flown as escorts; the flagship is the drydock's working design |
| `seen` | parts research unlocked that the player has since picked or selected in the drydock; left out when empty (added 13 Oct 2026, track 7F: no new format, it has a default) |
| `voyages` | per voyage ever launched: `launched` (count), `cleared` (the goal was cleared on a voyage that came home), `best_ticks` (the quickest such voyage, sim ticks at 30 per second) |
| `underway` | the voyage launched and not ended; left out when none |
| `log` | voyages launched, recalled, lost; items salvaged (parts and fragments unloaded) |
| `last_haul` | the debrief of the last voyage: `voyage`, `outcome` (`Recalled`, `Cleared` or `Lost`), `ticks`, `report` (`parts` and `fragments` by part id, `data`, `materials`) |

The text is written with the pack loader's options (`Some(..)` left out, four-space indent) and is deterministic;
unknown fields are errors, fields with defaults may be left out. `career::FORMAT` (1) is the format the game's
document header carries: raise it when a field is added or its meaning changes, and migrate older formats when
reading. Unknown fields are errors, so a field added without a new format would make an older build call a newer
save damaged instead of newer; with the format raised, its storage reports `Newer`. The last haul's report is
history (what was credited), not derived. A save that cannot be read (newer, damaged, or from a newer voyage roller,
below) is never written over: the game keeps it as it is ("How the game uses it" below).

## Derived values

- **Unlocks** (`Career::unlocks(&content) -> Unlocks`): every part, material and frame that is not `locked` in the
  content, plus every `Part`, `Material` and `Frame` unlock of the researched nodes. `Unlocks::all(&content)` is
  everything (the development switch `?unlock=all`).
- **Mind capacity** (`mind_capacity`): the start's `mind_capacity` plus every `MindCapacity(n)` of the researched
  nodes (2, then 3 and 4 with the two mind expansions of the base pack).
- **Open voyages** (`open_voyages`, `is_open`), in content order (the voyage map's): the start's `voyages`, and
  every voyage with `opens_after` whose voyages have all been cleared (`VoyageRecord::cleared`: a goal cleared on
  a voyage that came home). A voyage without `opens_after` opens only through a start. Loading drops records of
  voyages the content no longer has.
- **Fresh parts** (`fresh_parts(&content)`): `locked` parts a researched node unlocked that are not in `seen` (the
  drydock's NEW tags). `mark_seen(&content, part)` adds a fresh part to `seen` and says whether it was fresh (the
  game writes the save then); any other part changes nothing. `reconcile` drops seen ids the content no longer has,
  without a note (it is what the drydock showed, not progress).

## Research

`can_pay(&content, node)` answers whether a node can be researched now; `research(&content, node)` does it and
returns the node. Refusals (`career::Refusal`), checked in this order: `UnknownNode`, `AlreadyResearched`,
`Requires(missing)` (the node's `requires` not yet researched, in its order), `CannotPay(shortfalls)` (every data
cost, then every material cost, that the stores can't meet, with what is held and what it costs). A refused call
changes nothing.

Paying takes each cost from its store at once and unlocks immediately (GDR-0063). Research data within
`DATA_EPSILON` (0.001) of a cost counts as enough, so a sum of tenths that lands a hair below its cost still pays; a
store paid down to that little is removed. Stores hold only amounts above 0.

## Unloading a haul

A `Haul` is the items a voyage brought home with counts (`BTreeMap<HaulItem, u32>`). `HaulItem` mirrors the sim's
cargo item (`Part { part, material }`, `Fragment { part }`); `data` does not depend on `sim`, so the game converts.
`career::unload(&content, &haul) -> HaulReport` is pure and gives:

| Item | Gives |
|---|---|
| a whole non-hull part | its `salvage` block's `data` for its family and its `materials`, per item |
| a whole hull part | its armour material's `salvage.data` × the part's area (slopes count half a cell) for that family, per item |
| a fragment | the `fragment` materials of the part that left it; no data |
| an unknown part, a hull part without (or with an unknown or non-armour) material, a part without a block | nothing (whole parts are still listed) |

A non-hull part item that carries a material gives its own block; the material is ignored. Counts saturate; sums
are bounded by `MAX_DATA` and `MAX_MATERIALS`. The report lists `parts` and `fragments` by part id, `data` per
family and `materials`; `items()` counts both kinds.

## Voyages

- **Launch** (`voyage_launched(&content, id) -> seed`): refused while another voyage is underway (`Underway`) and
  for a voyage that is not open (`NotOpen`). It returns the seed the sim rolls this launch from, adds one to the
  voyage's `launched` and the log, and marks the voyage underway. The game writes the save now, so closing the game
  mid-voyage counts as losing it (GDR-0062; see loading below).
- **Seed** (`voyage_seed(id)`, the next launch's): `splitmix64(seed ^ splitmix64(fnv1a64(id) ^ launched))`, with
  64-bit FNV-1a over the id's UTF-8 bytes (never Rust's `DefaultHasher`) and one step of SplitMix64 (it adds the
  generator's increment, then mixes), both defined once in `data::hash` for the career and the sim's RNG streams and
  pinned by tests. The same save rolls the same voyage; each launch rolls a new one. It is part of the save format:
  changing it needs a new `GENERATOR`. Fixed values are tested (`tests/career.rs` `voyage_seeds_are_fixed`):
  seed 4207, `yard_approach`, no launches → 9993844787240758234; after one launch → 8810720374438629928.
- **End** (`voyage_ended(&content, id, end)`, refused with `NotUnderway` for any voyage but the one underway):
  - `VoyageEnd::Recalled { haul, cleared, ticks }`: the haul is unloaded into the stores; `cleared` marks the voyage
    cleared and keeps the quickest `ticks` as `best_ticks`; outcome `Cleared` or `Recalled`; the log counts a
    recall and the items.
  - `VoyageEnd::Lost { ticks }`: the haul stays behind (there is none to pass), the goal does not count; outcome
    `Lost`.
  - Either way the debrief becomes `last_haul`, which is returned. Cleared voyages stay launchable for more haul.

## Voyage-readiness

`career::voyage_ready(&content, &unlocks, mind_capacity, &flagship, &escorts) -> Vec<Readiness>`; empty means
ready. Entries, in this order: per ship (flagship first, then escorts by index, `FleetSlot`)

1. `LockedFrame { ship, size, frame }`: the blueprint's `size` (a drydock design's frame) fits no unlocked frame;
   `frame` names the first frame in content order it would fit (a locked one), if any. Same rule as a start's ship.
2. `LockedPart { ship, part }`, then `LockedMaterial { ship, material }`: each locked (or unknown) part and material
   it uses, once, in placement order.

then for the fleet

3. `TooManyCores { cores, capacity }`: its logic cores (all ships) exceed the mind capacity.
4. `NoCargo`: no ship has a hold (`Content::ship_hold`: cargo on its main piece, as the sim counts it; cargo on a
   piece that falls off at spawn holds nothing), so nothing salvaged can come home. A warning only:
   `Readiness::blocks()` is false for it alone.

The drydock and the map show the first entry. Flight-readiness (power, thrust: the design report) is the sim's
question and is checked by the game next to this list.

## Loading a save against changed content

Content may change between versions (a rebalance, a removed mod). `Career::reconcile(&content) -> Vec<Problem>`
runs after every load and never fails or panics; each change comes back as a warning (with kind, id and a field
path such as `researched[1]` or `data.lasers`) for the status line:

- a pack of `packs` that is not loaded now is noted; `packs` becomes the current list;
- an unknown `start` is replaced by the content's first start;
- a `generator` below `GENERATOR` is migrated to it with a note (0, which no build writes, too);
- researched nodes the content doesn't have, or listed twice, are dropped (their unlocks go with them);
- data of unknown families and materials the content doesn't have are dropped, in the stores and in the last haul's
  report; data that is not finite or below 0 is dropped; amounts above `MAX_DATA` or `MAX_MATERIALS` are lowered to
  it; zero amounts are dropped silently;
- records of voyages the content doesn't know are dropped, and so is a last haul from one;
- a voyage still `underway` was interrupted: it ends as `Lost` (ticks 0) when its launch is on record, else the
  mark is dropped.

The queries themselves cope with content that was not reconciled (an unknown start gives mind capacity 0 plus
research and opens only the voyages whose `opens_after` are all cleared; unknown nodes unlock nothing; research data
that is not finite counts as none, so it never pays).

### Voyage roller versions

`generator` names the voyage roller a career's voyages roll with (ADR-0024): the voyage seed and the sim's
`VoyagePlan::roll` (`docs/sim.md` "Rolling a plan").

| Version | What changed |
|---|---|
| 1 | 13 Oct 2026 (track 7E): the plan drew on a generator seeded like the sim's encounter stream; wrecks rolled their missing parts on that stream when they spawned, so a wreck depended on the order the sites were reached |
| 2 | 13 Oct 2026 (review): the plan's generator is salted (`sim::voyage::PLAN_STREAM`), and each wreck carries its own damage seed in the plan |

An older save is migrated, not replayed: nothing a save stores refers to a rolled voyage (no mid-voyage saves,
GDR-0062; a voyage underway at load counts as lost; every launch rolls anew), so a version 1 career simply rolls its
next launches with version 2, with a note. Its voyage seeds are the same in both versions. A save from a newer build
(`generator` above `GENERATOR`) is refused by `Career::from_ron` (an error on `generator`), never reset: this build
would roll its voyages differently. The game keeps it as it is and never writes over it.

## Bounds and defaults (placeholders, 13 Oct 2026, track 7C)

| Constant | Value | What |
|---|---|---|
| `career::FORMAT` | 1 | the save's format version |
| `career::GENERATOR` | 2 | the voyage roller's version, voyage seeds included |
| `career::MAX_DATA` | 1,000,000 | research data per family a store holds (100 × the largest content cost) |
| `career::MAX_MATERIALS` | 1,000,000 | of one material a store holds |
| `career::DATA_EPSILON` | 0.001 | research data that close to a cost pays it |

Not in the model: kills (the game would have to count them; the plan's log example had them), the game seed of a
new game (the game picks it, the model has no clock), whether escort designs still exist (they live in the game's
storage).

## Where the game keeps the save

Track 7F, 13 Oct 2026: `game::career` (`crates/game/src/career.rs`; tests there, `cargo test -p game career`).

- **The document**: `saves/<slot>` in the player storage (ADR-0031: the browser's local storage on the web, key
  `space-game/saves/career`; `<data dir>/saves/career.ron` on desktop; memory where the platform has none, with a
  note). The text is `Career::to_ron()` after the storage's header line `// space-game: saves, format 1, saved
  <unix seconds>` (`CareerDoc`, format `career::FORMAT`). The slot is `career`; `?save=<slot>` plays another one
  (a storage name: `a-z 0-9 _ -`; another name falls back to `career` with a note).
- **Loading** (`career::load`, in the plugin's build before the first frame): read → `Career::from_ron` →
  `reconcile` (the first change and how many more become a note on home). No save, or `?newgame=1`: a new game from
  the content's first start, seeded with the clock (Unix seconds), written at once; the drydock then docks a copy
  of the start's ship (`rim_skiff`), saving the last design's unsaved changes first so BLUEPRINTS keeps them.
- **A save that doesn't read** (broken RON, unknown fields, a header from a newer build, a newer voyage roller) is
  **kept as it is**: the
  game runs a new career in memory as a stand-in, never writes it, keeps the drydock's design and says so on home
  ("THE SAVE IS KEPT AS IT IS"). `?newgame=1` starts over in its place. Content without any start gets the same
  treatment (a stand-in career that unlocks what the content doesn't lock, mind capacity 0).
- **Writes**: at a new game, after every FLEET change, when the drydock first shows a fresh part (`seen`); the
  research screen (7G) writes after UNLOCK and the voyage (7H) at launch and end. Never during a voyage. A failed
  write leaves the change in the running game and says why (home's notes, FLEET's status line).
- **`?unlock=all`** (free mode): the drydock offers every part, material and frame (`Unlocks::all`), shows nothing
  locked and no NEW tags, and checks voyage-readiness against everything. No unlock is ever written: the save keeps
  the career's own progress (FLEET still saves its escorts).
- **Escorts**: their saved designs are read at start and whenever home or the drydock opens (`EscortDesigns`); an
  escort whose design is gone or doesn't read leaves the fleet, the save is written and home notes it.

## How the game uses it

- **Start**: read `saves/career` → `Career::from_ron` → `reconcile` (show the notes); none: `Career::new(&content,
  start, seed)` with the content's first start.
- **Drydock**: `unlocks` gates the palette, armour and frames (locked ones shown dimmed); `fresh_parts` gives the
  NEW tags and `mark_seen` ends them; `voyage_ready` gives the status line's voyage-ready line.
- **FLEET**: `escorts` and `set_escorts`; `mind_capacity` limits the picks; `voyage_ready` for the fleet.
- **Research screen**: `data()` and `materials()` on top, `can_pay` per node (the refusal says why not),
  `research` on UNLOCK, then write the save.
- **Map**: `open_voyages`, `voyage(id)` for cleared and best time, `voyage_ready` for SET COURSE.
- **Launch**: `voyage_launched` → write the save → roll the voyage with the returned seed and `mind_capacity`.
- **End**: convert the surviving ships' cargo to a `Haul`, `voyage_ended`, write the save, show `last_haul`.
