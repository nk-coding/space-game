# Plan: the first playable loop (a vertical slice)

Written 13 Oct 2026 by a planning agent, for the lead session and Niklas. Status: **proposal**. Nothing here is
decided; section 9 holds the questions, section 5.7 the records this would need.

This is the integrating plan. Other agents are writing deeper plans in parallel: `salvage-research.md`,
`galaxy-voyages.md`, `android.md` and `coop-arena.md` (not written yet when this was). This plan takes the
smallest piece of each that the slice needs and says what can wait. Where a deeper plan later says more about its
own topic, it wins on depth; this plan sets the scope of the slice.

Read for it: `CLAUDE.md`, `docs/concept-notes.md`, `docs/roadmap.md`, `docs/spike-plan.md`,
`docs/content-model.md`, `docs/drydock.md`, `docs/client.md`, `docs/sim.md` (parts), the GDR and ADR indexes and
GDR-0002, 0003, 0006, 0008, 0013, 0017, 0024–0032, 0038, 0044, 0050, 0051, 0053, 0054, 0057, ADR-0022, 0024, 0031,
every concept board page and every UI mockup, and the code named below.

## 1. Summary

You wake in the shipyard with one small steel ship. You pick a short voyage into a rim cluster, fight raider packs,
and fly over the wrecks to strip their parts into your cargo bays. You come home (or lose the ship and keep nothing
from that trip), the haul turns into research data, and you unlock a part: the raiders' cannon, beam lasers, a
fusion reactor, a bigger frame, or room in your mind for an escort. You rebuild in the drydock, maybe teach the
escort a behavior, and fly further. Four voyages end at the Warden, an automaton boss: about three hours of play.

## 2. What exists today and what is missing

| Area | Today | Missing for a loop |
|---|---|---|
| App start and modes | `AppMode::{Flight, Drydock, Behavior}` (`crates/client/world/src/mode.rs`); starts in the sandbox; `?scene=` picks another | A home screen, a research screen, a voyage picker; a start that isn't the sandbox |
| Home shipyard | Nothing. The drydock and behavior screens have a ring-menu button (`DrydockAction::Menu`, `BehaviorAction::Menu`) with no handler | The ring menu of `ui-mockups/phone-home-ring-menu.png` (GDR-0008) |
| Flight and combat | Complete for the slice: physics, breakage, weapons, shields, plumes, vessels, focus, behaviors (`crates/core/sim`) | Nothing new for combat itself; balance is open (see risks) |
| Enemies | `automaton_raider` (3×3, one core) and `raider_basic`; waves spawned by `scene.rs` with a client RNG (`backdrop::Rng`) | A few more enemy ships (a hunter, a boss), encounters placed in a world, spawning from the sim's seeded streams |
| Scenes | `SceneKind::{Sandbox, Showcase, Load}` and test flights / TEST IN RANGE (`scene.rs`, `test_range.rs`). Wrecks are removed after 75 s dark or 320 m away; a lost ship re-forms in place after 3 s | A voyage scene: sites, wrecks that stay, recall home, death sends you home |
| Wrecks and salvage | Pieces without a working core are salvage (`Piece::salvage()`, GDR-0013). `Stream::Debris` exists ("debris and salvage generation") but nothing uses salvage | Collecting parts from wrecks; wrecks spawned as wrecks (derelicts) |
| Cargo | Parts have `cargo: (capacity)`; the design report sums it (`DesignReport::cargo`, the drydock's CARGO cell). Nothing is ever carried | A hold on each ship, filled by salvage, kept by its wreck |
| Research and tech | GDR-0024/0025 decide the shape; no code, no content | A few research nodes in content, research data, a screen to unlock them |
| Unlocks | `locked: bool` on parts and materials (GDR-0054); the drydock hides locked entries (`drydock::state::palette_parts`, `armor_materials`). "Nothing unlocks yet" | A per-player unlock set, derived from research |
| Materials | Armour materials only (`materials/armor.ron`); the kind allows materials without an `armor` block | One or two non-armour materials (programmable matter) |
| Voyages and world | Decided (GDR-0026, 0028, ADR-0022); no code, no content | A handful of small rim clusters with seeded sites |
| Death | The sandbox re-forms the ship in place; a test flight returns to the drydock | Mild death per GDR-0029: recalled home, the haul stays with the wreck |
| Fleet and mind capacity | Vessels, focus, dormant vessels, fleet bar (`fleet_bar.rs`, GDR-0050); `SimConfig::mind_capacity` (player 24, `?mind=N`); wingmen via `?fleet=N` | Mind capacity as a progression number; escorts chosen at home |
| Storage | `game::storage` (ADR-0031): named, versioned RON documents; `designs/`, `drydock/`, `behaviors/` in use, `saves/` and `settings/` planned | The player's save (progress, unlocks, research data, fleet) |
| HUD | Hull, speed, target panel, warnings, fleet bar (`flight_hud/`) | Cargo, site markers, voyage title, recall |
| Determinism | Native vs wasm IDENTICAL on `core`, `flight`, `load`, `behavior` scenarios (`sim::scenario`) | A `voyage` scenario covering salvage and the encounter logic |
| Content | 20 parts, 4 materials, 3 frames, 4 ships, 2 behaviors (`content/base`) | The slice's content (section 3) |

## 3. The slice

All numbers in this section are **placeholders** for feel, to be tuned in track 7K.

### 3.1 The flow

1. **Start.** The app opens at the shipyard (the home ring). First run: a new game starts by itself. Two lines of
   intro text: you woke in a dead precursor shipyard; the drydock still works. The docked ship in the middle is your
   working design, at first a copy of the starter ship (steel, fission reactors, pulse lasers, thrusters: the
   concept board's "early game" ship, `10-art-direction.png`).
2. **Look around.** The ring offers DRYDOCK, BEHAVIOR, RESEARCH, VOYAGES, TEST RANGE and FLEET. TEST RANGE is
   today's sandbox with raider waves: simulated, no haul, no risk.
3. **Pick a voyage.** VOYAGES shows a small map: the shipyard and four rim clusters in a chain. Only the first is
   open. The panel names it, its threats ("RAIDERS · WRECKS"), the expected time ("≈ 8 MIN · RECALL WHEN CLEAR")
   and whether your fleet is voyage-ready. SET COURSE launches.
4. **Fly the cluster.** A short warp-in, then you are at the cluster's entry. Site markers show where things are
   (raiders, wrecks, the goal site in gold). Sites are 400–700 m apart; at 22 m/s that is 20–30 s of flying.
5. **Fight.** A raider pack holds still until a ship of yours comes within 250 m, then attacks under its normal
   behavior. Damage stays for the whole voyage: there are no repairs out here.
6. **Salvage.** Fly slowly over a wreck: your ship strips it, best parts first (an icon flies into the ship, the
   cargo count goes up: `SALVAGE 5 / 8 · CARGO HOLD`, as in `phone-flight-hud.png`). A full hold takes nothing
   more. Derelict sites hold old, pre-broken wrecks: precursor hulls with beam lasers and fusion reactors in them,
   and burned-out raiders whose cores survived.
7. **Decide.** Push on to the next site with a damaged ship and a part-full hold, or recall home. Recall works when
   no awake enemy is within 200 m; it takes a 3 s warp-out.
8. **Lose (mild death).** If your last vessel goes, "SHIP LOST · RECALLED HOME": you are home at once with your
   ship and escorts whole again (bonded matter, GDR-0029). The haul of this voyage stays in the wreck's cargo; it is
   lost when the voyage ends. While an escort lives, the focus moves to it and the voyage goes on; it can scoop the
   flagship's wreck.
9. **Unload.** At home, the haul is dismantled into research data per tech family and a little programmable
   matter. The home screen shows it ("HAUL UNLOADED · 14 PARTS → KINETIC +18 · PROPULSION +9 · LOGIC +4") and
   "RESEARCH · 1 READY".
10. **Research.** The research screen lists about nine nodes by family. A node you can pay for glows; UNLOCK pays
    and unlocks at once ("AUTOMATON CANNON · NOW IN THE DRYDOCK").
11. **Rebuild.** In the drydock the new part is in the palette with a NEW tag. Parts you have unlocked are unlimited;
    the frame and your mind capacity are the limits. Test-fly in the test range as today. A design that uses locked
    parts still opens and test-flies, but isn't voyage-ready.
12. **Go further.** Clearing a voyage's goal site once opens the next voyage. Mind expansions let you bring an
    escort (FLEET: pick a saved design; it flies the behavior its design names). The fourth voyage ends at the
    Warden, a 3-core automaton boss with escorts. Beating it is the end of the slice: "THE GATE TO THE OUTER REACH
    STAYS DARK · TO BE CONTINUED".

### 3.2 Starting kit and unlocks

At a new game the player has every part and material that is not `locked` in content, the frigate frame, mind
capacity 2 (one 2-core ship) and the first voyage. In content, `locked: true` moves to: composite and reactive
armour, the fusion reactor, beam laser and torch drive (plus the automaton core, cannon and plate as today), and the
cruiser and capital frames.

So the starting kit is: armour block and the three slopes in steel, fission reactor, capacitor, radiator, pulse
laser, shield emitter, thruster, gyro, RCS pod, logic core, cargo bay, cargo hold. The Halvex Runner (composite,
fusion, beams, torch drive) becomes the mid-game target, as the concept board's "mid game" ship.

Research nodes (9):

| Node | Kind (GDR-0024) | Unlocks | Cost | Needs |
|---|---|---|---|---|
| Reverse-engineered cannon | new part | `automaton_cannon` | kinetic 12 | – |
| Composite plating | new part (material) | `composite` | hull 20 | – |
| Beam laser | new part | `beam_laser` | energy 18 | – |
| Fusion reactor | new part | `fusion_reactor` | power 20 | – |
| Torch drive | new part | `torch_drive` | propulsion 20 | – |
| Tamed automaton core | new part | `automaton_core` (cheap 1×1 core: 2 rule slots, control 4: drones) | logic 8, programmable matter 1 | – |
| Mind expansion I | refinement | mind capacity +1 | logic 10, programmable matter 2 | Tamed automaton core |
| Mind expansion II | refinement | mind capacity +1 | logic 20, programmable matter 4 | Mind expansion I |
| Cruiser drydock | size class | `cruiser` frame | hull 30 | Composite plating |

Reactive armour, the automaton plate and the capital frame stay locked in the slice.

Research data per salvaged part (placeholders: about the part's build points): automaton cannon kinetic 3,
thruster propulsion 3, automaton core logic 4 + programmable matter 1, logic core logic 3 + programmable matter 1,
pulse laser energy 3, beam laser energy 7, fission reactor power 4, fusion reactor power 10, torch drive propulsion
9, shield emitter shields 3, radiator thermal 2, capacitor power 2, hull hull 1 per cell (composite 1.5). A part
takes cargo units equal to its footprint cells (a beam 2, a fusion reactor 4).

### 3.3 Voyages and encounters

| Voyage | Opens after | Sites (goal in bold) | Size | Length |
|---|---|---|---|---|
| Shipyard approach | start | raider pack (2) ×2, derelict field ×1, **raider pack (3)** | radius 600 m | 6–8 min |
| Halvex Drift | Shipyard approach | raider pack (3–4) ×2, precursor wrecks ×1, core graveyard ×1, **raider den** | radius 900 m | 10–15 min |
| Corran Reach | Halvex Drift | hunter pack ×2, raider pack ×1, precursor wrecks ×1, **hunter den** | radius 1000 m | 12–15 min |
| The Warden's gate | Corran Reach | raider pack ×2, core graveyard ×1, **the Warden** | radius 900 m | 10–20 min |

(The names Halvex Drift and Corran Reach come from `phone-galaxy-map.png`.)

Encounters (5 kinds, with a few variants by count):

| Encounter | Ships | Wrecks | What it gives |
|---|---|---|---|
| Raider pack / den | 2–5 `automaton_raider` | – | fights; raider wrecks: cannons, thrusters, plate |
| Hunter pack / den | 2–3 `automaton_hunter` (new: 5×5, two cannons, one core) + raiders | – | harder fights, kinetic data |
| Derelict field | 0–1 raider | pre-broken `sample_early` and raider hulks | steel parts, power, a first taste of salvage without a fight |
| Precursor wrecks | 0–2 raiders | pre-broken `sample_mid` hulls (composite, fusion, beams, torch drive) | energy, power, propulsion, hull data: the way to the mid-game parts |
| Core graveyard | 1–2 raiders | burned-out raider hulks with intact cores | logic data and programmable matter |
| The Warden | `automaton_warden` (new: about 9×9, three automaton cores, four cannons, shield emitters) + 2–4 raiders | – | the end of the slice; shooting it apart leaves several vessels (GDR-0013), so it fights on in pieces |

Why derelicts are needed: a raider becomes salvage only when its one core is destroyed, so combat never yields
automaton cores (see 5.7, "Records that look off"). Cores come from the graveyard hulks.

### 3.4 Rules of the slice

- **Voyage-ready**: the flagship is flight-ready (the drydock's design report), uses only unlocked parts, materials
  and frame, and the fleet's logic cores fit the mind capacity. No cargo is only a warning ("NO CARGO · YOU CAN'T
  BRING SALVAGE HOME").
- **The flagship** is the drydock's working design (what is on the grid); **escorts** are saved designs picked in
  FLEET. Escorts follow the flagship's lock (`Leader`, as the `?fleet=` wingmen do today).
- **Salvage**: a running vessel of the player strips a salvage piece within 5 m of its hull at 3 parts per second,
  highest research value per cargo unit first, until its hold is full. A stripped part leaves the wreck (the wreck may
  break up further). Enemies never collect.
- **Cargo** belongs to the ship. When the vessel is lost, its cargo stays in the wreck, and a wreck with cargo gives
  that cargo first when stripped. When the voyage ends, everything left behind is gone.
- **No repairs** during a voyage; ships come home whole (re-formed from bonded matter), every time.
- **Recall** when no awake enemy vessel is within 200 m; 3 s warp-out; home with every surviving vessel's haul.
- **Death**: losing the last vessel ends the voyage at once; the voyage's haul is lost; the goal is not cleared.
- **Quitting mid-voyage** (closing the app) counts as a loss of that voyage's haul: no mid-voyage saves.
- **Re-launching** a voyage rolls new site positions and counts (seeded); cleared voyages stay replayable for more
  haul.
- **Mind capacity** starts at 2 and grows to 4 in the slice.

### 3.5 An evening (placeholders)

| Step | Minutes |
|---|---|
| New game, look at home, first launch | 5 |
| Shipyard approach, two runs | 15 |
| First research (cannon), first rebuild, test range | 15 |
| Halvex Drift, two or three runs (one lost) | 35 |
| Research beams / fusion / composite, rebuild towards a Runner | 20 |
| Corran Reach, two runs | 30 |
| Core graveyard runs for programmable matter, mind expansion, an escort and its behavior | 25 |
| The Warden, two or three attempts | 40 |
| **Total** | **≈ 3 h** |

### 3.6 Where the slice is smaller than the records

These are deliberate cuts for the slice, not changes to the decisions; each needs a line in the slice's record:

- **GDR-0028**: clusters are "10+ minutes to cross" and a voyage 15–30 minutes. Slice clusters are 1–2 km across and
  voyages 6–20 minutes, because there is no cruise, warp or autopilot yet. The record stays the target.
- **GDR-0025**: "intact parts on big wrecks have to be cut free"; scans give research. The slice strips every wreck
  by flying over it and has no scans.
- **GDR-0029, GDR-0031**: a new part consumes its materials once (bonded matter); the fabricator resolves recipes.
  The slice has no part costs: unlocked parts are unlimited, as in Drydock v1 (GDR-0051).
- **GDR-0028**: supplies (ammo, fuel, repair material) and outposts are left out; "no repairs during a voyage" is
  the slice's only pressure to go home.
- **GDR-0026**: one procedural galaxy with a new seed per game. The slice has four hand-made clusters with seeded
  sites; the generator places such templates later.
- **GDR-0008**: the home ring shows six of its eight buttons (no fabricator, no storage; the galaxy map is the small
  voyage map).
- **GDR-0002**: the first hour (wake as a tiny drone, salvage wrecks, repair the drydock) is skipped; you start with
  a working ship.

## 4. Gaps: what must be decided first

### G1. What limits building

- **A. Unlocks only** (recommended): researched parts are unlimited; the frame (frigate 24×24) and mind capacity are
  the limits. Uses what exists; the bonded-matter costs of GDR-0029 come with fabrication. Risk: once a part is
  unlocked, nothing stops a ship full of it besides the frame and control.
- **B. Unlocks plus a build-point budget per frame**: GDR-0030 already says designs are built within a budget. One
  more number to tune; the drydock already shows build points.
- **C. Material costs now** (bonded matter): the real design, but it needs recipes, an inventory and spare counts in
  the drydock: the size of the whole salvage-research plan.

Recommendation: A. Control capacity (GDR-0038) already caps machines per core, and mind capacity caps cores.

### G2. The starting kit and what `locked` means

- **A. Early-game kit** (recommended): lock composite, reactive, fusion, beam, torch drive and the bigger frames in
  content; a new starter ship from the kit. Research then unlocks visible, ship-changing parts. Consequence: the
  free drydock and the sandbox no longer offer those parts unless `?unlock=all` (dev).
- **B. Today's kit**: everything but the automaton parts. Research only adds the automaton parts, frames and mind
  capacity: thin progression.
- **C. Bare minimum** (armour, one gun, one engine, a core): more to unlock, more content needed now.

Recommendation: A. `locked` keeps its GDR-0054 meaning ("not available until unlocked") and gains the one GDR-0054
already foresaw: "an unlock system will clear it per player later". Frames get the same flag. A starter ship is
needed because `sample_early` has no gyros or RCS pods and, with GDR-0055's tiny core torque, may turn badly: check it
with the design report and add a gyro pair or two RCS pods.

### G3. How salvage is collected

- **A. Fly over the wreck** (recommended): strips parts in reach, best first. Uses physics and flying skill; works on
  touch; the haul is visible.
- **B. Collected when the site is cleared**: no flying, no skill, but no choice about the hold either.
- **C. A tractor beam part**: GDR-0025 names it; a new part, aiming and behavior words. Better as a later unlock.

Recommendation: A, with "cutting parts free" from big wrecks waiting.

### G4. What happens to the haul when the last ship is lost

- **A. It stays in the wreck's cargo for the rest of the voyage** (recommended): the literal GDR-0029 ("the haul
  stays behind as a wreck"); an escort can scoop it; the voyage's haul is lost when you are recalled.
- **B. Half comes home**: gentler; less tension.
- **C. The wreck stays in that cluster for the next voyage there**: a corpse run; needs wrecks saved as world deltas
  (ADR-0024), which waits for the galaxy.

Recommendation: A.

### G5. How the haul becomes research

- **A. Dismantled automatically at home into data per family; nodes unlock at once when paid** (recommended).
  Simple, no inventory screen.
- **B. Per part: dismantle or keep to install as-is** (GDR-0029: "install as-is for a small bonding cost"). Needs
  spare counts in the drydock (the mockup's "2 SPARE" rows). Good later; doubles the UI.
- **C. Timed research** (the mockup's "Beam laser · focused 62 %"): a reason to come back, but a wall-clock timer in a
  hobby game mostly means waiting.

Recommendation: A. The mockup's "focused" progress can still show as data held / cost of the node you pin.

### G6. Research families

- **A. A small `families` content list that follows GDR-0024** (recommended): hull, power, energy, kinetic,
  missiles, shields, thermal, propulsion, logic, utility. Each salvageable part names its family.
- **B. Use the part categories** (hull, power, weapons, defense, heat, propulsion, logic, cargo): no new kind, but
  weapons don't split into energy and kinetic, and saves would need a migration when the families come.

Recommendation: A. If `salvage-research.md` defines the families first, use its list.

### G7. What a voyage is

- **A. Hand-made cluster templates with seeded sites** (recommended): four in a chain; the galaxy generator places
  templates later (GDR-0026's "set pieces placed in").
- **B. Endless escalating waves in one arena, cash out any time**: almost no new code, but no place, no travel, no
  "further".
- **C. Wait for the procedural galaxy** (`galaxy-voyages.md`): the right end state, but it puts the loop behind the
  largest system.

Recommendation: A. Also: recall only when no awake enemy is near (so recall can't skip a lost fight), and the
voyage world spawns a site's ships only when a player vessel comes within 600 m (keeps the world small).

### G8. Mind capacity numbers (open since GDR-0013)

- **A. Start 2 cores, +1 twice by research, 4 in the slice** (recommended): one ship at first, a drone or small
  escort later; the fleet bar and dormant vessels start to matter.
- **B. Start 4, research to 6**: escorts from the first voyage; less to earn.
- **C. Keep 24**: progression only through parts and frames; mind capacity stays a dev setting.

Recommendation: A. Note: the `skirmisher` escorts fight poorly today (won 3–5 of 12 waves, `docs/sim.md`), so the
escort reward needs a better default behavior or the behavior editor's help.

### G9. Where the encounter logic runs

- **A. In `sim`, as an optional voyage subsystem** (recommended): deterministic, in the state hash, uses the sim's
  seeded streams, runs headless for balance runs and later the co-op host and the arena.
- **B. In the client's scene, like today's waves**: quick, but outside the hash and the seeded streams, and the
  arena couldn't run it.

Recommendation: A. Today's wave spawning in `scene.rs` uses `backdrop::Rng`, a client RNG; fine for the sandbox,
not for voyages.

### G10. How the game starts and where the sandbox goes

- **A. Start at home; the sandbox becomes TEST RANGE and `?scene=sandbox`** (recommended).
- **B. GDR-0002's first hour as a tutorial voyage** (a tiny drone salvages wrecks to repair the drydock): better
  onboarding, more content work; can come after the slice.
- **C. Keep starting in the sandbox, home as a button**: least change for development, but it isn't a game start.

Recommendation: A.

### G11. One save or several

- **A. One career slot** (`saves/career`), `?save=<slot>` for testing, NEW GAME behind a confirm (recommended).
- **B. A slot picker screen**: more UI, little value for one player.

Recommendation: A (not asked; a lead default).

## 5. Technical plan

### 5.1 App modes and screens

| Mode / scene | Screen | Notes |
|---|---|---|
| `AppMode::Home` (new, the default start) | The ring menu around the docked ship (`phone-home-ring-menu.png`), laid out for 1280×720 like the drydock | The docked ship is the working design drawn as in the drydock (`drydock::view`'s mesh path). Buttons: DRYDOCK, BEHAVIOR, RESEARCH (`n READY`), VOYAGES, TEST RANGE, FLEET. Top left `THE SHIPYARD · RIM` and the design's name; bottom left `HAUL UNLOADED` and the last voyage's result. The ring button on the drydock and behavior screens (`DrydockAction::Menu`, `BehaviorAction::Menu`) opens it |
| `AppMode::Research` (new) | Families as columns, node cards (name, kind tag, cost chips gold when payable, needs), the data pools on top, the selected node's part preview (`Content::part_stats`, as the drydock inspector) and UNLOCK | Pure data from `data::career` (5.4) |
| `AppMode::Map` (new) | The voyage map: shipyard and four clusters (positions in content), legend (home, open, cleared, locked), the right panel of `phone-galaxy-map.png` without fuel/ammo/repair: destination, chips (`RIM`, `AUTOMATONS`), sites summary, `≈ 10 MIN · RECALL WHEN CLEAR`, the fleet and its voyage-ready check, SET COURSE | Launch sends a `StartVoyage` message, like `test_range::StartTest` |
| FLEET (a layer over Home, like BLUEPRINTS) | The flagship (working design) and escort slots: pick saved designs; `CORES 3 / 4` | Stored in the save |
| `AppMode::Flight`, `SceneKind::Voyage(id)` (new) | Flight with the voyage HUD: title `HALVEX DRIFT · RIM`, `SITES 2 / 5`, cargo `SALVAGE 5 / 8 · CARGO HOLD`, site markers with distance (raiders coral, wrecks gold, goal brighter, cleared dim), a RECALL pill and key (enabled when the sim allows), warp-out countdown | No re-form, no wreck clean-up, waves off. T does nothing (use recall). Losing everything → Home after 3 s |
| Debrief | A panel on Home after a voyage: name, time, sites, parts by kind, data per family, materials, `RESEARCH READY: n` | Also written into the save as `last_haul` |
| Drydock (exists) | Palette and frames from the unlock set; a NEW tag on freshly unlocked parts; locked parts in content order shown dimmed as `LOCKED · <node>` (the mockup's locked rows); the status line adds `NOT VOYAGE-READY · USES BEAM LASER` / `3 CORES, YOUR MIND RUNS 2` | `drydock::state::palette_parts(content)` becomes `palette_parts(content, unlocks)` |
| Sandbox (exists) | TEST RANGE from Home; `?scene=sandbox` | Unchanged; test flights and TEST IN RANGE unchanged |

URL parameters (for development): `scene=home|research|map|sandbox`, `scene=voyage&voyage=<id>` (launch at once),
`save=<slot>`, `newgame=1`, `unlock=all` (every part, frame and material in the drydock, the save untouched).

All new screens use `ui_kit` widgets and its hand-made hit testing, which already takes touch; the phone layouts
come with `android.md`.

### 5.2 The player save

A new document type in `game::storage` (ADR-0031), namespace `saves`, name `career` by default. It stores facts and
inventory, never what can be derived from content, so a content rebalance (a node's cost, a part's unlock) applies to
old saves without a migration.

```ron
// space-game: saves, format 1, saved 1760351234
(
    packs: [("base", "0.1.0")],      // the content it was played with (informational, like share codes)
    start: "rim_awakening",          // the content start it began from
    seed: 4207,                      // the game's seed (ADR-0024); voyages roll their sites from it
    generator: 1,                    // version of the voyage roller (ADR-0024: old versions stay reproducible)
    researched: ["automaton_cannon", "composite_plating"],   // node ids, in the order done
    data: {"kinetic": 4, "hull": 11, "logic": 2},            // research data in store
    materials: {"programmable_matter": 1},
    escorts: ["drone_wing"],         // saved design ids (`designs/`); the flagship is the drydock's working design
    voyages: {
        "yard_approach": (launched: 2, cleared: true, best_ticks: 13500),
        "halvex_drift": (launched: 1, cleared: false),
    },
    log: (voyages: 3, lost: 1, kills: 21, parts_salvaged: 34),
    last_haul: Some((voyage: "halvex_drift", lost: false, parts: {"automaton_cannon": 3, "thruster": 2},
                     data: {"kinetic": 9, "propulsion": 6})),
)
```

- **Derived, never stored**: the unlock set (content entries not `locked`, plus every unlock of `researched`), the
  mind capacity (the start's plus every refinement), the open voyages (the start's plus those opened by cleared
  ones), what is payable.
- **Versioning**: `FORMAT = 1`; older formats migrate in `Document::from_ron` (ADR-0031). Ids are content ids
  (stable by rule); a node or voyage id that no longer exists is dropped on load with a note in the status line.
  The other plans will add fields (materials beyond programmable matter, saved wrecks, galaxy deltas): add them with
  `#[serde(default)]` where possible, raise the format only when meaning changes.
- **Voyage seed**: `seed`, the voyage id and `launched` mixed with a fixed hash (FNV-1a of the id bytes, never Rust's
  `DefaultHasher`), so a voyage is reproducible from the save and different on each launch.
- **Writes**: when a voyage ends (recall, loss), after UNLOCK, after a FLEET change, at NEW GAME. Not during a voyage.
- **Designs stay global** in `designs/` (shared by every slot, the free drydock and imports). A deleted escort design
  empties its slot with a note.
- On the web the save lives in the browser's local storage per origin (`:8080` and `:8081` keep separate games);
  on Android it stays in memory until the files backend lands (`android.md`).

### 5.3 Content additions

The kinds follow `docs/content-model.md`: common header, stable ids, keyed text, placeholders marked at the top of
each file, `[[kind:id]]` links (new link kinds: `family`, `research`, `voyage`, `encounter`, `start`), JSON export
for the wiki. Every new kind gets loader, validation and `export-json` support.

**Locks** (existing flag; `locked` added to frames):

```ron
// materials/armor.ron
(id: "composite", ..., locked: true),
// parts/power.ron
(id: "fusion_reactor", ..., locked: true),
// frames.ron
(id: "cruiser", name: "Cruiser", summary: "...", size: (40, 40), locked: true),
```

**A non-armour material** (the kind already allows it):

```ron
// materials/precursor.ron. PLACEHOLDER.
(
    id: "programmable_matter", name: "Programmable matter",
    summary: "Precursor matter that can host a copy of the ship-mind.",
    description: "Every [[part:logic_core|logic core]] needs some. Rogue automaton cores are the first source.",
    icon: "programmable-matter",
)
```

**Research families** (`families.ron`, new kind; G6):

```ron
// The tech families of GDR-0024: one research tree each. Order = display order.
[
    (id: "hull", name: "Hull and armour", summary: "...", color: "steel"),
    (id: "power", name: "Power", summary: "...", color: "power"),
    (id: "energy", name: "Energy weapons", summary: "...", color: "weapons"),
    (id: "kinetic", name: "Kinetic weapons", summary: "...", color: "weapons"),
    // shields, thermal, propulsion, logic, utility; missiles when they exist
]
```

**What a part gives when salvaged** (a new optional block on parts and armour materials; a part without it is never
collected):

```ron
// parts/automatons.ron
(id: "automaton_cannon", ..., salvage: (family: "kinetic", data: 3)),
(id: "automaton_core", ..., salvage: (family: "logic", data: 4, materials: {"programmable_matter": 1})),
// materials/armor.ron: per cell of hull in this material
(id: "automaton_plate", ..., salvage: (family: "hull", data: 1)),
```

Cargo units are the part's footprint cells (code rule, documented in the content model).

**Research nodes** (`research/*.ron`, new kind):

```ron
// research/rim.ron: the slice's nodes. PLACEHOLDER NUMBERS.
[
    (
        id: "automaton_cannon",
        name: "Reverse-engineered cannon",
        summary: "Build the raiders' crude kinetic gun into your own ships.",
        family: "kinetic",
        kind: NewPart,                     // GDR-0024: NewPart, Variant, SizeClass, Refinement, Alien
        cost: (data: {"kinetic": 12}),
        unlocks: [Part("automaton_cannon")],
    ),
    (
        id: "mind_expansion_1",
        name: "Mind expansion I",
        summary: "Run one more logic core at once.",
        family: "logic",
        kind: Refinement,
        cost: (data: {"logic": 10}, materials: {"programmable_matter": 2}),
        requires: ["tamed_core"],
        unlocks: [MindCapacity(1)],
    ),
    (
        id: "cruiser_drydock",
        name: "Cruiser drydock",
        summary: "Expand the drydock for line ships with several big machines.",
        family: "hull",
        kind: SizeClass,
        cost: (data: {"hull": 30}),
        requires: ["composite_plating"],
        unlocks: [Frame("cruiser")],
    ),
]
```

Unlock targets: `Part(id)`, `Material(id)`, `Frame(id)`, `MindCapacity(n)`. Validation: references exist, unlock
targets are `locked` in content (unlocking an open entry is a warning), no cycles in `requires`, costs name known
families and materials.

**How a game starts** (`starts.ron`, new kind; one entry in the slice):

```ron
// How a new game begins. PLACEHOLDER NUMBERS.
(
    id: "rim_awakening",
    name: "Awakening at the rim",
    summary: "You wake in a dead shipyard with one small ship and a drydock that still works.",
    ship: "rim_skiff",              // the first working design (a copy)
    mind_capacity: 2,               // logic cores the mind runs at once (GDR-0013)
    voyages: ["yard_approach"],     // open at the start
)
```

**Voyages** (`voyages/*.ron`, new kind; the galaxy plan may rename it "clusters"):

```ron
// voyages/halvex_drift.ron. PLACEHOLDER NUMBERS.
(
    id: "halvex_drift",
    name: "Halvex Drift",
    summary: "A raider-haunted drift of wrecks one warp out from the shipyard.",
    band: "rim",                         // text only in the slice
    map: (0.62, 0.18),                   // place on the voyage map (0..1)
    opens_after: ["yard_approach"],      // voyages whose goal must be cleared once
    radius: 900,                         // m; sites lie inside (GDR-0028's large clusters come with travel)
    spacing: (400, 700),                 // m between sites
    sites: [
        (encounter: "raider_pack", count: 2),
        (encounter: "precursor_wrecks"),
        (encounter: "core_graveyard"),
        (encounter: "raider_den", goal: true),   // clearing it clears the voyage
    ],
)
```

**Encounters** (`encounters/*.ron`, new kind):

```ron
// encounters/automatons.ron. PLACEHOLDER NUMBERS.
[
    (
        id: "raider_pack",
        name: "Raider pack",
        summary: "A few raiders drifting among the rocks, waking when a ship comes near.",
        ships: [(ship: "automaton_raider", count: (2, 4))],   // they fly their faction's behavior once awake
        wake_radius: 250,
    ),
    (
        id: "precursor_wrecks",
        name: "Precursor wrecks",
        summary: "Old hulls full of parts the shipyard has forgotten how to make.",
        lore: "...",
        ships: [(ship: "automaton_raider", count: (0, 2))],
        wrecks: [(ship: "sample_mid", count: (1, 2), damage: 0.4)],   // share of parts destroyed at spawn
        wake_radius: 200,
    ),
]
```

**Ships**: `rim_skiff` (the starter, from the kit), `automaton_hunter`, `automaton_warden`. Enemy designs are
handmade for now (GDR-0032's evolved enemies wait for the arena). Texts stay short; numbers live in stat lines.

### 5.4 The progression model in `data` (`data::career`)

A pure model, like `data::edit` is for the drydock (ADR-0030): no Bevy, no I/O, builds for wasm32, tested on its own.
It holds the save's facts as serde types and answers every rule question:

- `Career::new(&content, start, seed)`, `unlocks(&content) -> Unlocks` (parts, materials, frames), `mind_capacity`,
  `open_voyages`, `can_pay(node)`, `research(node)`, `unload(haul) -> HaulReport`, `voyage_seed(id)`,
  `voyage_ended(id, outcome)`.
- `voyage_ready(&content, &unlocks, flagship, escorts) -> Vec<Readiness>`: locked parts, frame, cores over capacity,
  no cargo (a warning). The drydock and the map show its first entry.
- `Haul`: salvage items as `(part or material id, count)`; `unload` turns them into data and materials with the
  content's `salvage` blocks.

### 5.5 Sim changes (determinism kept)

- **A new RNG stream**, appended: `Stream::Encounter = 5` (site placement, counts, wreck damage). `Stream` is append
  only (`crates/core/sim/src/rng.rs`).
- **Wrecks from the start**: `Sim::spawn_wreck(design, owner, body, damage)`: the ship spawns as salvage (no
  controlled piece; a `burned_out` flag keeps `form_vessels` from making its cores vessels), then `damage` of its
  parts are destroyed (rolled on `Stream::Encounter`) and breakage splits it into pieces as usual.
- **Cargo and collecting** (`sim::salvage`): `Ship::cargo` (a `BTreeMap` of item → count and the units used);
  capacity is the sum of the alive cargo parts of the vessel's controlling piece (if it drops below the load, the
  lowest-value items go). Every 10 ticks each running vessel of an owner that collects (a config mask: the player)
  strips the nearest salvage piece in reach: one part per pass at most (3 per second), highest value per unit first,
  ties by part index; the part leaves through the destroy path without destruction effects (a `removed` flag on
  `PartDestroyed`, or a new `SimEvent::Collected { ship, from, piece, item, units }`). A wreck's own cargo comes
  first. A lost vessel's ship keeps its cargo. The cargo is in the state hash. Reach, interval and rate are
  `SimConfig::salvage` placeholders (in the hash, like the other tuning).
- **The voyage director** (`sim::voyage`, optional, G9): `VoyagePlan::roll(&content, &voyage, seed)` places sites
  (positions, counts, compiled designs); `Sim::start_voyage(plan, player, enemy)`. Each tick, after the vessels: a
  site's ships and wrecks spawn when a player vessel comes within the spawn radius (600 m); its ships hold
  (`Maneuver::Hold`) until a player vessel is within `wake_radius`, then get their faction behavior
  (`SimEvent::SiteWoke`); a site is cleared when it had ships and none of them is a running vessel any more
  (`SiteCleared { goal }`). Queries: `Sim::voyage()` (sites and their states, goal cleared), `can_recall(owner)` (no
  awake enemy vessel within 200 m). Its state is in the hash. It works on owners, so co-op can add a second player
  owner later.
- **A determinism scenario**: `ScenarioKind::Voyage` in `sim::scenario` (a voyage with random pilot input, salvage and
  waking), added to `scripts/determinism-check.sh`; native vs wasm must stay IDENTICAL.
- **A headless runner** (`cargo run --release -p sim --example voyage -- --voyage halvex_drift --design rim_skiff
  --seeds 1..20`): the flagship under `skirmisher` (or a scripted pilot), reporting goal cleared, time, hp lost, haul
  and data per family. This is the balance tool of track 7K and a seed of the arena (GDR-0032).
- **Cost**: the collect pass looks at salvage pieces near each collecting vessel every 10 ticks (a few vessels × at
  most a few dozen pieces); lazy site spawning keeps one or two sites' ships in the world. Rapier's sleeping keeps
  idle wrecks cheap.

Nothing changes in combat, flight or behaviors. No new behavior words are needed: sleeping ships hold by the
director's maneuver, awake ones fly their faction's behavior.

### 5.6 The client, and how the existing parts fit

- **`scene.rs`**: `SceneKind::Voyage(id)`; `Scene::begin_voyage(launch)` like `begin_test`; `build_world` builds the
  sim with the career's mind capacity (`SimConfig::mind_capacity[PLAYER]`), rolls the plan, starts the voyage,
  spawns the flagship and escorts at the entry (`wing_slot`, `Leader`), focuses the flagship. In voyages: no waves, no
  re-form, no wreck clean-up; on losing every vessel a banner and, after 3 s, `AppMode::Home` with the outcome; on
  recall a 3 s warp-out, then Home with the haul.
- **`sim_bridge::apply_events`**: `Collected` → a small effect (the part's tile flies into the ship, `fx.rs`);
  `SiteWoke` / `SiteCleared` → banners.
- **`flight_hud/`**: cargo, voyage title and site counts, site markers (the off-screen marker code exists), RECALL.
- **A `career` module** (Bevy side): `CareerPlugin` holds the save (`Career` resource), loads it at start (or starts a
  new game), writes it at the moments above, and provides `Unlocks` to the drydock, the map and the research screen.
- **The drydock**: palette, armour pills and frame pills read `Unlocks` instead of only the content flag; the status
  line adds the voyage-ready line; TEST-FLY stays a test-range flight. In free mode (`?unlock=all`) nothing is hidden.
- **The behavior editor**: unchanged; reachable from Home and the drydock. Escort designs carry their behaviors as
  today.
- **The fleet bar**: unchanged; it starts to matter once escorts come.
- **Test flights and TEST IN RANGE**: unchanged, never touch the save.
- **The tuning panel** (U) stays a dev tool and applies to voyages like to the sandbox.

### 5.7 Records

Proposed (the lead writes them; status `Proposed` until Niklas answers):

- **GDR: the first playable loop**: the slice's scope and flow (section 3), the cuts against GDR-0025, 0026, 0028,
  0029 and 0008 (3.6), the end at the Warden.
- **GDR: salvage v1**: fly-over stripping, best value first, cargo per ship, a lost ship's cargo stays in its wreck,
  no repairs on voyages (answers G3, G4).
- **GDR: research v1**: auto-dismantle at home, data per family, nodes as content, instant unlocks, the starting
  kit and the meaning of `locked` (answers G2, G5, G6); refines GDR-0025 and GDR-0054.
- **GDR: voyages v1**: hand-made cluster templates with seeded sites, waking sites, recall rule, death sends you home,
  re-launch re-rolls (G7); refines GDR-0028 for the slice.
- **GDR: mind capacity numbers** (G8): answers the open question of GDR-0013 / 0038 / 0044 for the slice.
- **GDR: the game starts at home** (G10).
- **GDR: career drydock**: researched parts are unlimited; the frame and mind capacity limit a design; partly
  supersedes GDR-0051's "every part is unlimited" for the career (G1).
- **ADR: the career save**: `saves/<slot>`, facts stored and the rest derived, the voyage seed (ADR-0024, ADR-0031).
- **ADR: progression model in `data::career`, salvage and the voyage director in `sim`** (G9).

Records that look off (for the lead to check, not changed here):

- `automaton_raider`'s description says its wrecks are "rich in salvage, above all their cores", and GDR-0031 says
  "automaton cores at the rim are the first source" of programmable matter. But a raider has one core, which is also
  its power; it becomes salvage only when that core is destroyed. So fights never yield cores; only derelicts (or
  capture, GDR-0017) can. The slice uses core graveyards; the text or the raider's design may need a change.
- The `skirmisher` loses most waves as an unpiloted ship (`docs/sim.md` "Sandbox fights at the new speeds"). Escorts
  are a reward in the slice; they need a better default.
- `scene.rs` spawns waves with a client RNG. Fine for the sandbox; voyages must use the sim's streams (G9).

## 6. Tracks

Numbered 7A–7L as a suggestion (the lead may renumber). Sizes: S a day or less for one agent, M a few days, L about a
week. Agents work in their own worktrees and target dirs (CLAUDE.md).

| Track | Goal | Depends on | Size | Parallel with |
|---|---|---|---|---|
| 7A Content schema | Kinds `families`, `research`, `starts`, `voyages`, `encounters`; `salvage` block on parts and materials; `locked` on frames; new link kinds; loader, validation, `export-json`; `content-model.md` | – | M | 7B, 7D |
| 7B Sim salvage | `Stream::Encounter`; `Sim::spawn_wreck`; cargo, collecting, `Collected`, cargo kept by wrecks, hash; `SimConfig::salvage`; a cargo count in today's sandbox HUD to feel it early; `sim.md` | – (a stub value rule until 7A's `salvage` block lands) | M | 7A, 7D |
| 7C `data::career` | The pure progression model (5.4): unlock set, mind capacity, payment, unloading, voyage opening and seed, voyage-ready; tests | 7A | M | 7B, 7D, 7E |
| 7D Home screen | `AppMode::Home` as the start, the ring menu with the docked ship, navigation to the drydock, behavior screen and test range; the drydock's and behavior screen's ring buttons; `?scene=home|sandbox` | – | M | 7A, 7B, 7C |
| 7E Voyage director | `sim::voyage`: plan rolling, lazy spawning, waking, clearing, recall check; `ScenarioKind::Voyage` and the determinism check; the headless `voyage` example | 7A, 7B | L | 7C, 7F, 7G |
| 7F Save and gating | The `saves/` document, load / new game, `?save= ?newgame= ?unlock=all`; the drydock's palette, armour and frames from `Unlocks`, NEW and LOCKED rows, the voyage-ready line; FLEET | 7C, 7D | M | 7E, 7G |
| 7G Research screen | `AppMode::Research`, the debrief on Home, unloading the haul into the save | 7C, 7D, 7F (save resource; can start on a stub) | M | 7E, 7F |
| 7H Voyage flight | `SceneKind::Voyage`, launch, the voyage HUD, collect effects, recall, death → Home, the save update | 7E, 7F | L | 7I, 7J |
| 7I Voyage map | `AppMode::Map`: the chain of clusters, the destination panel, SET COURSE | 7C, 7D, 7F | S | 7H, 7J |
| 7J Slice content | The starter ship, locks, nine nodes, salvage values, four voyages, five encounters, the hunter and the Warden, short texts | 7A (schema); numbers checked with 7E's example | M | 7H, 7I |
| 7K Balance pass | Headless voyages over seeds: clear rates, times, haul per run against the evening budget (3.5); tune encounters and node costs; report in `docs/results/` | 7E, 7J | M | 7H (late) |
| 7L Play-test | Niklas plays an evening; feedback into records | all | S | – |

Waves (3–5 agents each):

| Wave | Tracks | Notes |
|---|---|---|
| 0 | Questions (section 9), records (5.7) | The lead, with Niklas |
| 1 | 7A, 7B, 7D (+ 7C as soon as 7A's types are merged) | 7B's first deliverable is salvage in today's sandbox: the first feel check |
| 2 | 7C, 7E, 7F, 7G | 7J can start once 7A is merged |
| 3 | 7H, 7I, 7J | 7H is the integration point |
| 4 | 7K, 7L | |

## 7. What explicitly waits

| What | Why it waits | Whose plan |
|---|---|---|
| Fabrication, recipes, 30+ materials, part costs (bonded matter), spare counts, "install as-is" | A material economy needs harvesting and recipe chains; the loop works without it (G1, G5) | salvage-research |
| Cutting parts free from big wrecks, tractor beams, scans, lore unlocks, enemy intel | Fly-over stripping proves salvage first | salvage-research |
| The procedural galaxy, lanes, warp, gates, bands beyond the rim, hazards, autopilot, the full galaxy map | The slice needs four places, not a generator; templates fit the generator later | galaxy-voyages |
| Supplies (ammo, fuel, repair material), outposts, repair drones | "No repairs out here" gives enough pressure for a slice | galaxy-voyages / salvage-research |
| Saved wrecks and world deltas, encounters coming back slowly | Needs the galaxy's save model (ADR-0024); the slice re-rolls every launch | galaxy-voyages |
| Phone layouts and touch for the new screens, Android storage | Web is the dev platform; the screens use `ui_kit`, which already takes touch | android |
| Co-op voyages, a shared haul | Single player first; the director already works by owner | coop-arena |
| The arena and evolved enemies | Handmade enemies are enough for four voyages; the headless `voyage` runner is the first step | coop-arena |
| The first hour (tiny drone, repairing the drydock) | Onboarding content; the slice starts with a working ship (G10) | later |
| Capture (GDR-0017), alien nodes, variants and refinement chains | One kind of each node is enough to show the system | salvage-research |
| Timed research, research focus | Waiting isn't play (G5) | later |
| Mid-voyage saves, several save slots, new game+ | Voyages are short; one player | later |
| Behavior words for salvage (`wreck in range`, `cargo full`, `tractor`) | Escorts don't need to salvage in the slice (built 20 Oct 2026 without the tractor: `salvage`, `wreck_in_range`, `cargo_full`, [GDR-0123](../gdr/0123-behavior-words-for-salvage.md)) | salvage-research |
| The fabricator, storage and galaxy map buttons of the ring | Nothing behind them yet | later |

## 8. Risks and what to try first

| Risk | Why | Try first |
|---|---|---|
| Fights decide the curve, and balance is open | `raider_basic` sits on a knife edge around the beam range; the skirmisher loses most waves; nothing was re-measured after GDR-0055's slower turning | The headless `voyage` runner (7E) before any content numbers; measure the starter ship against packs of 2, 3, 4 |
| Salvage may feel fiddly or trivial | Wrecks fly off at 20 m/s after a kill; pieces split into fragments; 5 m reach may be too short or too long | 7B in today's sandbox first: count what survives a raider kill, fly over it, tune reach and rate before voyages exist |
| No cores from fights | A raider's only core dies with it (5.7) | Core graveyards in content; measure programmable matter per run in 7K |
| Empty travel | Sites 400–700 m apart with no autopilot | Try spacings in the runner and in play; shrink before adding travel systems |
| Save churn | Three parallel plans will each add to the save | Store facts and derive the rest; content ids only; migrations from format 1 (5.2) |
| The starter ship handles badly | `sample_early` has no gyros or RCS pods; core torque is tiny since GDR-0055 | Check its turn time in the design report; build `rim_skiff` with a gyro pair |
| Escorts aren't a reward | The default escort behavior fights poorly | Give escorts a better content behavior in 7J (an `escort` template), check it in the runner |
| Many wrecks in one world | Voyages keep every wreck | Lazy site spawning; measure tick time with a full voyage in the runner (median and p95) |
| Three new screens by hand in bevy_ui | The drydock took several waves | Keep them lists and a small map; reuse `ui_kit`, `ui_icons`, the drydock's view |
| Browser storage per origin | `:8080` and `:8081` have separate games | Note it in the docs; export of the save waits |

## 9. Questions for Niklas

Each is self-contained for the question tool. The recommended option is first.

**Q1. What limits ship building in the first playable loop?** Today every part is unlimited (Drydock v1). The game's
design says new parts cost bonded matter (materials, GDR-0029), but there is no fabrication yet.
- Unlocks only: researched parts are unlimited; the frame and the mind capacity limit a design. (Recommended) Uses
  what exists; part costs come later with fabrication.
- Unlocks plus a build-point budget per frame. One more limit to tune; the drydock already shows build points.
- Material costs now (bonded matter). The real design, but needs recipes and an inventory first: a much bigger
  slice.

**Q2. What does a new game start with?** The concept board's "early game" ship uses steel, fission reactors, pulse
lasers and thrusters; the "mid game" ship composite, fusion, beam lasers and a torch drive.
- The early-game kit; composite, fusion, beams and the torch drive come from research; a small starter ship.
  (Recommended) Research changes what ships look like; the Halvex Runner becomes the goal. The free drydock hides
  those parts unless a dev switch is on.
- Today's kit (everything but the automaton parts). Nothing changes for building; research only adds automaton
  parts, frames and mind capacity: thin progression.
- A bare minimum (armour, one gun, one engine, a core). More to unlock, but more content needed now.

**Q3. How is salvage collected from wrecks?**
- Fly slowly over a wreck: your ship strips its parts into the cargo bays, best first, until the hold is full.
  (Recommended) Uses flying and physics, works on touch; cargo bays become a design trade-off.
- Automatically when a site is cleared. No flying needed, but no skill and no choice.
- With a tractor beam part. Closer to the concept, but a new part, aiming and behavior words first.

**Q4. What happens to the haul when you lose your last ship?** The ship itself always re-forms at home for free.
- It stays in the wreck's cargo for the rest of that voyage (an escort can scoop it); you come home with nothing
  from that voyage. (Recommended) Literal "the haul stays behind as a wreck"; real tension, simple.
- Half of it comes home. Gentler, less tension.
- The wreck stays in that cluster for your next voyage there. A corpse run; needs saved wrecks, which wait for the
  galaxy.

**Q5. How does the haul become research?**
- At home it is dismantled automatically into research data per tech family; a node unlocks at once when you pay
  its data. (Recommended) No inventory screen; the loop stays short.
- You choose per part: dismantle it for data, or keep it to install as-is. Closer to the concept, needs spare counts
  in the drydock.
- Research takes time once paid (a timer per node). A reason to come back, but mostly waiting.

**Q6. What is a voyage in the first playable loop?**
- One small hand-made rim cluster per voyage (four in a chain), with a few seeded sites; recall home whenever no
  enemy is near. (Recommended) Small clusters (1–2 km) until travel exists; the procedural galaxy places such
  clusters later.
- Endless escalating waves in one arena; cash out any time. Almost no new code, but no places and no "further".
- Wait for the procedural galaxy and build the loop on it. The right end state, but the loop waits for the biggest
  system.

**Q7. Mind capacity (how many logic cores you run at once) in the slice?** Today it is a dev setting of 24.
- Start with 2 (one ship), research +1 twice: 4 by the end, enough for a drone or a small escort. (Recommended)
  The fleet becomes something you earn.
- Start with 4, research up to 6. Escorts from the first voyage; less to earn.
- Keep 24. Progression only through parts and frames.

**Q8. Where does the game start?**
- At the home shipyard with a working starter ship; today's sandbox becomes the TEST RANGE button (and
  `?scene=sandbox`). (Recommended) A real game start; your dev flow keeps a URL.
- With the first hour from the concept: a tiny drone salvages wrecks to repair the drydock. Better onboarding, more
  content work; can follow the slice.
- In the sandbox as today, with home as a button. Least change, but it doesn't feel like a game.
