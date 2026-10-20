# Roadmap

Living plan, kept up to date by the lead session. Decisions live in `adr/` and `gdr/` (overview: `concept-notes.md`).
Started 11 Oct 2026. Web first, Android later, minimal content, wiki-ready data.

The original spike plan (`spike-plan.md`) still defines what must be answered. The spike code is written as the real foundation of the game rather than as throwaway code. Phone measurements are deferred until Android work starts.

## Phase 1: foundations (parallel)

| Track | Goal | Status |
|---|---|---|
| 1A Physics benchmark | Rapier2D, box2d_rust and Avian 2D (Box2D C as reference if it builds) on the spike scenarios, natively and as wasm (Node + a page for the host browser). Determinism hashes native vs. wasm. Niklas picks the engine. | done 11 Oct: **Niklas chose Rapier2D** (`results/physics-benchmark.md`); the harness moves to a separate local repo |
| 1B Content + `data` crate | Content model for game + wiki, schema, pack loader, validation, blueprint geometry, minimal base pack, `content-tool` CLI (validate, export JSON). | done 11 Oct (see `content-model.md`) |
| 1C Web client | Bevy 0.20 wasm build with Trunk, style 2a ship rendering (atlas from SVGs, tiles, seams, icons + glow, hull edge glow, aura, plumes), backdrop, camera. | done 11 Oct (screenshots in `results/`) |

## Phase 2: core simulation

Status: 2a (everything engine-independent: determinism foundation, ship model, power, heat, bonuses, engines, damage/split events, native-vs-wasm hash check) done 11 Oct (`docs/sim.md`). 2b (Rapier2D integration: compound collider per piece, collisions, ramming v1, ghosting after splits; FreeFlight stand-in and generic trait removed) done and merged 12 Oct.

- Ship model in `sim` from `data` blueprints: occupancy grid, per-part HP, pieces (connectivity with slope rules), mass/CoM/inertia, power pool per piece, simple heat, thrust/torque, gun and engine clearance.
- Determinism infrastructure: fixed 30 Hz step, RNG streams, state hash per tick, native-vs-wasm hash test.
- Physics integration with the chosen engine: compound bodies per piece, splitting with inherited velocity and spin.

## Phase 3: flight and feel (web)

Status 11 Oct: flight computer, thrust/torque allocation (engines + gimbal, RCS, gyros, core assist), maneuvers and raider brain done in `sim`. Weapons v1 (lasers, projectiles, recoil, ricochet, shields) in progress in `sim`. Raiders lead their shots; shared load-test scenario exists. Client bridge (30 Hz sim + interpolation + input, weapons visuals, flight HUD, live tuning sliders, browser load test, browser hash check) in progress. Behavior system v1 (rule lists as content, text format, rule slots, Rhai conditions) done 12 Oct (`behavior-format.md`); follow-ups (fire at will per gun facing, RaiderBrain removed) in progress. Power tiers + weights with partial output (GDR-0014) and icons in the content pack (ADR-0028) done 12 Oct. Client driven by the sim, done 12 Oct (`client.md`): sandbox with raider waves, keyboard flight (GDR-0040), lock with smart aim, weapons FX, flight HUD, egui tuning panel, vessel focus (V), runtime SVG rasterisation (`crates/art`).

- Flight computer (stick = fly that way, braking, drift toggle, soft top speed), target lock, WASD + mouse.
- 30 Hz sim with 60 fps interpolation in the client.
- Automaton raiders with a trivial AI (approach, orbit, fire). Basic weapons and damage, breakage, debris.

## State at the end of 12 Oct 2026 (before the move to a more powerful machine)

Done this session (Niklas's play-test feedback, decided interactively; GDR-0041 to GDR-0050, `docs/concept-notes.md`):
- Engine rated speed instead of the soft top speed, a calm cruise flame, "Agile" ships (GDR-0041, GDR-0042).
- Shields as glowing energy bubbles (GDR-0043, ADR-0029; two batched meshes so the focus draws above others).
- The vessel model in `sim` (GDR-0013, 0038, 0044, 0045): owners, split-off vessels, focus, control and mind
  capacity, dormant/wake; ramming v2 (GDR-0047); no friendly fire (GDR-0048); engine plumes (GDR-0039, 0046,
  0049); the flight computer plans only with working actuators.
- Client: fleet bar (GDR-0050), camera glide, focus draw order, `?damage=` for the player only, `?mind=N`, faster
  atlas tier switches (no 178 ms mip hitch), F3 atlas line; browser-vs-native hash IDENTICAL.
- Checks at the end: 199 tests, content validation clean, determinism native vs wasm IDENTICAL (all scenarios).

New VM (re-checked 12 Oct 2026, evening): 16 cores, ~17 GB RAM, `10.77.9.2`; Rust 1.99, trunk, the headless
helper (rebuilt with Playwright) and the workspace guard hook are set up; 203 tests pass and the web build runs. The
physics bench repo was not copied (not needed). Details in CLAUDE.md "Development environment".

Open questions for Niklas (ask interactively):
- Mind capacity numbers and N per core type (placeholders: logic core 16, automaton core 4, player 24).
- Plume defaults and numbers (GDR-0049), whether plumes push what they hit (`docs/sim.md` questions 16–18).
- Fleet bar defaults (GDR-0050): placement, waking without focusing, no number hotkeys.
- Vessel model lead defaults (`docs/sim.md` "Vessels", *lead default*): focus fallback order, a dark focus keeps
  the focus, dormant vessels fully dark but owned, reactors idle last.
- Lead defaults: diagonal top speed can beat a rating; differential thrust nudges a locked ship sideways at
  speed; A/D spins the Runner at ~4.6 rad/s (`docs/sim.md` questions 14, 15).
- Balance: `raider_basic` stand-off/orbit sit on a knife edge around the beam range; the `skirmisher` wingmen
  win only 3–5 of 12 waves; shields rarely collapse (regen 1.5/s vs ~0.4 per cannon hit).

Client leftovers:
- Blur at high zoom: fixed 13 Oct (5I). Region steps at tier 512 still cost up to ~50 ms (large plumes).
- Two non-focus ships still show each other's shield bubbles over their hulls (ADR-0029 note): halved 13 Oct (the
  higher ship's bubble still showed over the lower one's hull), fixed 17 Oct (two passes: over space and over the
  own hull only; `?scene=showcase&twin=1`).
- Browser load-test numbers (`?scene=sandbox&load=1|5|20`) need Niklas's real browser (headless timings are meaningless).

## State at the end of the 12–13 Oct 2026 session (new VM)

Done: the new VM is set up (CLAUDE.md); Drydock v1 wave 1 is merged (5A–5D below): `?scene=drydock` or T opens the
drydock with the Halvex Runner in the mockup's layout; the palette, search, categories, armour and overlay pills,
the selected part and test flights (T, waves toggle C) work; nothing can be edited or saved yet. Phase 4's spike
summary is written. Checks at the end: 250 tests pass (1 ignored timing test), the web build runs headless with no
errors, determinism native vs wasm IDENTICAL on the new VM. Niklas asked to stop after wave 1.

Next session: wave 2 of the drydock (5E–5G), three agents in their own worktrees, each with its own target dir
(CLAUDE.md: never share a target dir between checkouts). Wave-2 hooks from the foundation: `DrydockAction` handlers
go in `drydock::DockSet::Apply`; `PlayerDesign { blueprint, frame, revision }` (bump `revision` after an edit);
`DrydockUi`, `DrydockStatus`, `DrydockStats`, `DrydockCursor` (`docs/client.md` "Drydock").

Open questions for Niklas (ask interactively):
- Browser numbers and the flight-feel verdict: steps in `results/spike-summary.md` (port 8080).
- Drydock defaults (GDR-0053): the frame sizes (hidden enemy parts, fill and bevel answered 13 Oct, GDR-0054).
- Still open from 12 Oct (below): mind capacity numbers, plume defaults, fleet bar defaults, vessel model lead
  defaults, flight lead defaults, fight balance.

## Phase 5: Drydock v1 (started 12 Oct 2026, GDR-0051, ADR-0030)

Niklas chose the drydock as the next phase: unlimited parts (stats as information only), test-fly into the sandbox
with a waves toggle (T toggles back), the look close to `ui-mockups/desktop-drydock.png`.

| Track | Goal | Status |
|---|---|---|
| 5A `sim::report` | Static design report with the sim's own rules: power, steady-state heat, thrust, top speed, turn, yaw, firepower, control, plumes, shields, device load, issues | done 12 Oct (`docs/sim.md` "Design report"; Runner 24 µs; hashes unchanged) |
| 5B `data` plumbing | `validate_ship`, blueprint RON writer/reader, frames as content (frigate/cruiser/capital, placeholders), ship totals, ship-level stat labels | done 12 Oct (`docs/content-model.md`) |
| 5C `data::edit` | Pure editing model: legality, paint, fill, erase, bevel, slope orientation, mirror, undo/redo | done 12 Oct (`docs/drydock.md`) |
| 5D Client foundation | `Drydock` app state, static ship view with the flight tiles, the mockup's layout and widgets, test-fly round trip with an in-memory blueprint | done 13 Oct (`client.md` "Drydock", `results/drydock-*.png`) |
| 5E Editing in the client | Tools, palette, selection, inspector with power overrides, undo/redo wired to `data::edit`; locked parts hidden (GDR-0054) | done 13 Oct (`drydock.md` "Client: editing") |
| 5F Stats and overlays | `sim::report` in the stats panel, inspector, status line and the heat/mass/bonus/shields/arcs overlays | done 13 Oct (`drydock.md` "Client: stats and overlays") |
| 5G Blueprints | Save/load (local storage / files), the blueprints list (own designs + content ships as templates), frames, export, share codes (GDR-0054) | done 13 Oct (`drydock.md` \"Storage and sharing\", ADR-0031) |
| 5H Turning feel (GDR-0055) | Keyboard rotation ~1.5 rad/s with a ramp; cores tiny, gyros modest, RCS strong (Runner 90° ≈ 1.5 s); differential thrust last resort; whole-speed fade | done 13 Oct (GDR-0058; determinism IDENTICAL; fight balance not re-measured) |
| 5I Sharp icons | Part icons look pixelated at high zoom in Niklas's browser although the atlas tier (256 px/cell) is above the need (160): find and fix the sampling/upload bug | done 13 Oct: the material never switched to a new tier (Bevy 0.20 `get_mut` only marks on write), so icons sampled the 64 px/cell start atlas all session; fixed, plus a −0.5 LOD bias (`client.md` "Atlas", `results/atlas-sharpness-*.png`) |

## Phase 6: Behavior editor (started 13 Oct 2026, GDR-0057)

Niklas chose the behavior editor after the drydock: its own screen from the drydock, graph and text views,
behaviors saved on their own and picked by designs, TEST IN RANGE by watching or flying alongside.

Status 13 Oct 2026: v1 done. 361 tests pass; the editor, saving, the BEHAVIORS list and both TEST IN RANGE modes
work headless. Still open: Niklas tries it; then his review of GDR-0053, 0056, 0058, 0059, 0060.

| Track | Goal | Status |
|---|---|---|
| 6A `data::behavior_edit` | Pure editing model of a rule program: rules, conditions, parameters, target line, slots, undo, text round trip | done 13 Oct (`behavior-format.md` "Editing model") |
| 6B Plumbing | Player behaviors in the storage layer, designs naming their behavior, share codes carrying it, the sim library, TEST IN RANGE modes (watch / fly alongside), the live active rule | done 13 Oct (`drydock.md` "Behaviors"; share codes `SGD2`) |
| 6C Screen foundation | `AppMode::Behavior`, the mockup's layout, read-only graph view, text view with live errors, palette and inspector shells, BEHAVIOR button in the drydock | done 13 Oct (`client.md` "Behavior screen") |
| 6D Wiring | Graph editing and the inspector through the model, saving, test buttons, live rule highlight | done 13 Oct (GDR-0060; `client.md` "Behavior screen") |

## Phase 7: the first playable loop (started 13 Oct 2026, GDR-0062, GDR-0063, ADR-0033)

Niklas chose the first playable loop as the next phase (plan: `plans/first-playable-loop.md`; the deeper plans
`../space-game-archive/plans/salvage-research.md`, `plans/galaxy-voyages.md`, `plans/coop-arena.md` hold what waits). Decided 13 Oct: start
at home with the early-game kit, unlocks only (unlimited once researched), fly over wrecks to salvage, the haul stays
in a lost ship's wreck, auto-dismantle into research data, four small hand-made rim clusters, recall in 3 s when no
awake enemy is near, closing mid-voyage loses that voyage, mind capacity 2 → 4, destroyed cores leave a
programmable-matter fragment.

| Track | Goal | Depends on | Status |
|---|---|---|---|
| 7A Content schema | Kinds `families`, `research`, `starts`, `voyages`, `encounters`; `salvage` block on parts and materials; `locked` on frames; loader, validation, `export-json` | – | done 13 Oct (GDR-0066) |
| 7B Sim salvage | `Stream::Encounter`, wrecks spawned as wrecks, cargo, collecting, core fragments, `SimConfig::salvage`, hash; a cargo count in the sandbox HUD | – | done 13 Oct (GDR-0066; determinism IDENTICAL) |
| 7C `data::career` | The pure progression model: unlocks, mind capacity, payment, unloading, voyage seeds, voyage-ready | 7A | done 13 Oct (`docs/career.md`) |
| 7D Home screen | `AppMode::Home` as the start, the ring menu with the docked ship, navigation, the drydock's and behavior screen's ring buttons, `?scene=home\|sandbox` | – | done 13 Oct (`client.md` "Home") |
| 7E Voyage director | `sim::voyage`: rolling, lazy spawning, waking, clearing, recall check; `ScenarioKind::Voyage`; the headless voyage runner | 7A, 7B | done 13 Oct (`sim.md` \"Voyages\"; the runner: no voyage cleared yet, see 7K) |
| 7F Save and gating | `saves/career`, new game, `?save= ?newgame= ?unlock=all`; the drydock from `Unlocks`; FLEET | 7C, 7D | done 13 Oct (GDR-0070; `client.md` \"Career\", \"FLEET\") |
| 7G Research screen | `AppMode::Research`, the debrief, unloading the haul | 7C, 7D, 7F | done 13 Oct (tree with lanes; `client.md` \"Research\") |
| 7H Voyage flight | `SceneKind::Voyage`, launch, the voyage HUD, recall, death → home, the save update | 7E, 7F | done 13 Oct (GDR-0072; `client.md` \"Voyages\") |
| 7I Voyage map | `AppMode::Map`, the chain of clusters, SET COURSE | 7C, 7D, 7F | done 13 Oct (star map; `client.md` \"Voyage map\") |
| 7D2 Shipyard home | The home screen as the long U dock (GDR-0071) | 7D | done 13 Oct |
| 7P Machine drawings | Machines drawn as components, engine nozzles past the hull (GDR-0069) | – | done 13 Oct |
| 7J Slice content | Starter ship, locks, nine nodes, salvage values, four voyages, five encounters, the hunter and the Warden | 7A | done 13 Oct (GDR-0073, GDR-0074) |
| 7K Balance pass | Headless voyages over seeds against the evening budget; tune encounters and node costs | 7E, 7J | done 13 Oct (`results/slice-balance-2026-10-13.md`) |
| 7L Play-test | Niklas plays an evening | all | next: Niklas plays an evening on 8080 |

Alongside (13 Oct): six code reviews, a UI review, a security review of untrusted input and a dependency audit
(reports in the lead's scratch space; fixes merged as they land), a docs audit (merged), the wiki generator and a
fight-balance re-measure (GDR-0061, proposed).

### State at the end of 13 Oct 2026 (night)

Phase 7 is built end to end: home is the shipyard (GDR-0071), machines are drawn as components (GDR-0069), and the
loop runs home → voyage map → voyage (salvage, recall, loss) → debrief → research → drydock. 638 tests pass;
native vs wasm IDENTICAL (voyage, salvage, core, behavior). Port 8080 has the build. Niklas stopped for the day.

Next session:
- 7L: Niklas plays an evening; tune from his feel (GDR-0073 is the baseline).
- Review the lead defaults of today: GDR-0066, GDR-0070, GDR-0072 (and the older GDR-0053, 0056, 0058, 0059, 0060).
- Note for Niklas: making the cannon an upgrade (GDR-0074) also made `raider_basic` attack 3 s of every 9 (was 5),
  so raiders hit less often; the Warden's gate with a Runner + escort went from about 40 % to 68 % clears (easier
  than GDR-0073 meant). Judge in play.
- Open: plume numbers, mind capacity beyond the slice, whether RCS pods also reach past the hull (GDR-0069 lead
  reading), the needrestart exemption (CLAUDE.md, root).

### 14 Oct 2026 (Niklas's answers, an hour of agent work)

- **Next phase after the 7L play-test: galaxy exploration** (Niklas). First rework `docs/plans/galaxy-voyages.md`
  around [GDR-0075](gdr/0075-voyages-feel-like-exploration.md) (explore uncharted space, follow hints, several
  ways on; not a fixed path), ask Niklas the hint questions, then the outer band with new enemies and parts.
- Judge in play, not now: the Warden at ~68 % clears; the starter turning faster than the Runner.
- Confirmed: RCS pods stay inside their cells (GDR-0072's reading); FLEET's four slots and the 30 m escort default.
- Changed: cargo a shrinking hold can't keep spills as collectable fragments (GDR-0076); abandoning a voyage asks
  in a panel instead of M twice (GDR-0077).
- Built today: the portal transition into the voyage map, zoom on the map out to the whole galaxy, Esc on the
  BEHAVIOR screen goes back the way it came, the shipyard shows faintly behind BEHAVIOR and FLEET.
- Open from today's map work: at the default zoom the map still draws the slice's voyages as a dotted chain with
  travel times (from content's `opens_after`), which reads as the fixed path GDR-0075 wants to avoid; the zoomed-out
  galaxy's inner bands are placeholders (their names crowd the centre). Both belong to the galaxy-exploration
  rework.
- Builds (ADR-0034): `scripts/web-build.sh` is now the 30-second everyday build; `--full` for performance numbers.
- Later on 14 Oct: galaxy exploration v1 decided ([GDR-0078](gdr/0078-galaxy-exploration-v1.md): signals that
  narrow as you search, blind jumps, jump range instead of lanes, hidden gates, scanning inside clusters);
  `docs/plans/galaxy-voyages.md` reworked around it (tracks E0–E8). Its eight follow-up questions answered the
  same day ([GDR-0080](gdr/0080-exploration-searching-and-story.md): 2–3 km clusters, a scanner part, scans wake
  enemies, free decoding, story by fragments and cards; [ADR-0035](adr/0035-saves-keep-the-galaxy-skeleton.md)). The
  slice's map step is built: no route, voyages unknown until opened, a pulsing signal until the first launch,
  SIGNAL FOUND in the debrief (`docs/results/map-signals-*.png`).
- Niklas reviewed the lead defaults ([GDR-0079](gdr/0079-review-of-lead-defaults-14-oct.md)): most confirmed;
  collecting up to 8 m/s, loose items last the voyage (the sandbox keeps 60 s), the flight camera widens up to 1.4×
  at top speed (built; determinism native vs wasm IDENTICAL, hashes changed).
- Home: the moorings hang off unbroken truss bridges and open up and down; the view centres the whole station at
  any window shape; BEHAVIOR, RESEARCH and FLEET share one veiled backdrop and let only 3 % of the station show
  through.
- Drydock rework decided (Niklas): editing ([GDR-0081](gdr/0081-drydock-editing-bevel-rotate-replace.md): bevel after
  the stroke, R / Shift+wheel turns the brush, painting and placing replace what is under, the shield radius while
  placing) and layout ([GDR-0082](gdr/0082-drydock-layout-v2-and-part-availability.md): every part in sections, armour
  swatches on the left, tools in the bottom bar, icons and a ⋯ menu; part availability tracked, unlimited for now).
  Both being built.
- Evolution lab decided ([GDR-0083](gdr/0083-evolution-lab.md), [ADR-0036](adr/0036-arena-crate.md),
  `docs/plans/evolution-lab.md` tracks L1–L5). L1 is built (`docs/arena.md`: fight specs, the score, the `arena`
  CLI; native vs wasm IDENTICAL; about 1200 Runner-vs-3-raiders fights/s and 2300 starter mirrors/s on 16 threads).
  L2 and L3 are built too (`docs/arena.md` "Evolution"): scoring by voyages (`sim::voyage_runner`), configurable runs
  (behavior / design / both / stages; arena pool or voyages; frame, build points, parts, mirror lock), a MAP-Elites
  archive, `arena evolve` (identical on 1 or 16 threads; 200–400 generations of 32 per minute). Balance findings:
  automaton parts dominate every design run that may use them (the automaton core: power 6, 2 rule slots and a gyro
  for 5 BP against 11 for reactor + logic core + gyro); the best 60-BP design won while the design report counted
  none of its guns as clear. Checked: blocked guns are silent as written (GDR-0004/0070), but the design puts its
  1-hp reactors and slopes in front of the cannons as shutters that the enemy shoots off, then fires (52 of 60
  wins against 33–35 with the guns clear; `docs/arena.md`). Niklas chose chain reactions (14 Oct) and a latent
  firepower axis for the archive (every gun as if clear and powered); both are built (below). Next: L4, the lab screen.
- **Chain reactions v1** ([GDR-0084](gdr/0084-chain-reactions-v1.md), provisional numbers for review): reactors,
  torch drives, capacitors and thrusters blast their own piece when destroyed (armour takes a `blast_factor` share);
  `SimEvent::Explosion` is emitted. The old shutter design falls from 52 to 12 of 60 wins; the new best design (59
  of 60) keeps every exploding part out of its lines of fire but stacks reserve guns behind guns. Hashes changed,
  native vs wasm IDENTICAL. Voyage clear rates: the starter a little harder, the Runner's Corran Reach 89 → 70 of
  120. Follow-ups: an explosion visual in the client, blast reach and warnings in the drydock (built 17 Oct: the
  BLAST overlay and the status line's warning), and the rule questions listed in GDR-0084.
- **Chain reactions v2** ([GDR-0085](gdr/0085-chain-reactions-v2.md)): blasts reach every piece and ship (other ships'
  shields absorb, friends are hit), intact armour on the line shields, reactors and drives reach two cells deep, the
  automaton core, shield emitter and RCS pod explode. Niklas confirmed the reach and friendly damage. The automaton
  core no longer dominates the design runs (2 of 5 best designs, from 4); voyages a little harder.
- **Letting go coasts** ([GDR-0086](gdr/0086-letting-go-coasts.md)): no braking and no flip-and-burn on release. Pilot
  assists (auto-brake and more) as configurable behavior-like profiles for the piloted ship, weapon groups and key
  bindings: to discuss with Niklas while the structure refactor runs. Decided the same day:
  [GDR-0088](gdr/0088-pilot-assists-weapon-groups-and-key-bindings.md) (assist rules per design, switchable in
  flight; weapon groups 1–3; an action map with a settings screen), built after the refactor in four steps.
- **Voyage transitions** ([GDR-0087](gdr/0087-voyage-transitions.md)): the recall warp-out (charge, stretch, flash;
  amber and flickering for the emergency recall), the arrival, the loss dim and a dip to dark between screens.
- **Chain-reaction blasts in the client** (`fx/blast.rs`): a flash, glow and a ring out to the blast radius; chains
  step 40 ms apart.
- **Smooth scrolling** (`docs/client.md` "Smooth scrolling"): wheel notches glide (100 px, 0.18 s, ease-out,
  accumulating), touchpads move 1:1; the row lists scroll by pixels; the debrief scrolls with fixed buttons; the
  wheel hints are gone.
- **Scrollbars and glow room** (`docs/client.md` UI kit): every scrolling list has an overlay scrollbar (shows while
  scrolling or hovered, draggable), and lists let glows fade past their edges instead of clipping them.
- **Sim fairness fixed** ([ADR-0037](adr/0037-simultaneous-fire-within-a-tick.md)): ship ids no longer decide a fight.
  Three causes: guns fired and hit in id order (now every shot is traced first, then all hits land), behaviors
  thought on id-staggered ticks (now shared ticks), `orbit(auto)` turned by its own id (now by both ships' ids).
  Mirror fights end the same in either spawn order (a test checks it exactly); a small physics effect remains in
  head-on rams (Rapier's body order; up to 37 of 1000 seeds). Hashes changed, native vs wasm IDENTICAL; voyage clear
  rates moved within noise (Halvex Drift a little harder).
- **Next after the running work merges: a structure refactor** (Niklas, 14 Oct 2026: "these rust crates seem to lack
  some structure"). Chosen: feature folders and one plugin per feature in every crate, and `game` (37k lines, 33 flat
  top-level modules) split into a few crates (shared render and UI infrastructure, flight, the screens) for clearer
  boundaries and faster rebuilds; `data` grouped into content / editing models / career and `sim` into sub-folders,
  big files split at their seams. An ADR with the target layout goes to Niklas first; the refactor changes no
  behaviour (tests and determinism hashes identical) and measures build times before and after. Done on a quiet
  tree, after the four agents of 14 Oct merge.
- **Structure refactor done** ([ADR-0038](adr/0038-workspace-structure-feature-folders-and-client-split.md), 14 Oct
  2026): `crates/core/{data,sim}`, `crates/client/{art,kit,world,game}`, `crates/tools/{arena,content-tool}`; feature
  folders in `data` and `sim`; the client's cycles broken (`world::player`, `ViewScale`, `ui_icons::IconShapes`) and
  split into `kit`, `world`, `game`; the 16 files over 1,100 lines split. Behaviour unchanged (724 tests, every
  determinism and fight hash, the screens). Build times: the baseline is in `results/`, the after column waits for
  the VM's memory.
- **Dead stuff removed** (14 Oct 2026; Niklas: "in general check for such dead stuff and get rid of what you can get
  rid of"): the concept bundle, the concept ship art, the screen mockups, two superseded plans and 54 uncited
  screenshots moved to the local repo `../space-game-archive` (comments citing them reworded, living docs point
  there); the tests pinning the base ships to the concept art, unused code (`apply_texts`, `palette_tags`,
  `is_present`, `side_segment`, `outward_normal`, `part_count`, `MAX_SHARE_CODE_BYTES`, the arena's job counts),
  unused style tokens, Bevy's `png` feature and stale comments gone.

### 15 Oct 2026 (Niklas's play-test)

- **Play-test fixes** ([GDR-0089](gdr/0089-play-test-15-oct-speed-salvage-waves-station.md)):
  - At the top speed the velocity follows the push: the fade is weighted by the push angle, and a governor stops the
    ship's own thrust from passing the top speed.
  - Wrecks and loose cargo settle to a slow drift.
  - Test-range waves spend their budget on the biggest enemies first.
  - Range rings stay visible zoomed out.
  - The station:
    - comes alive: beacons, the gate's ring and approach light, the mind, research's dishes, shuttles, drifting
      dust, twinkling stars;
    - the mind and research buildings have more detail;
    - its joints were fixed: bridges into the mind, pylons onto the gate, the lane's strut;
    - FLEET's hover covers all four moorings.
- **The action map and SETTINGS** ([GDR-0090](gdr/0090-action-map-settings-and-pause-menu.md), GDR-0088 step 1):
  - every key and mouse button outside the fixed ones (Esc, Enter, the pointer, the wheel) is an action with up to
    two inputs; the defaults are in `content/base/controls.ron` (a new content kind, in the wiki too);
  - SETTINGS (the gear ring at home or O, and the new flight menu) rebinds them and sets glow, bloom and the UI size,
    saved on this device;
  - Esc in flight unlocks, else opens the flight menu (it pauses the test range);
  - tooltips and the help line show the bound keys; the debug toggles moved to F5–F7.
- **Weapon groups** ([GDR-0088](gdr/0088-pilot-assists-weapon-groups-and-key-bindings.md) step 2):
  - every gun has a group 1–3 (`Placement::group`), set with the drydock's GROUP pills 1 / 2 / 3 (the twin of a
    mirrored gun follows);
  - the pilot fires by a per-group fire mask in the sim (`PilotInput::fire`, `FIRE_ALL`);
  - keys: 1, 2, 3 fire their group (the right mouse button is group 2 too, the held left button group 1), Space
    fires all, touch fires all; all of them rebindable in SETTINGS.
- **The LOG** ([GDR-0091](gdr/0091-log-building-and-voyage-history.md); Niklas: the summary line showed over SETTINGS
  and ran into research):
  - a records mast on the top arm by the gate replaces the one-line summary over home; it glows while a voyage's
    result is new, its tooltip is the last voyage;
  - L or a click opens the LOG, a layer with the last 50 voyages, newest first;
  - the save keeps them (`history`, format 2);
  - home's words and the debrief's panel step aside under every layer over home.
- **Pilot assists** ([GDR-0088](gdr/0088-pilot-assists-weapon-groups-and-key-bindings.md) step 3):
  - a PILOT ASSISTS tab next to GRAPH and TEXT on the BEHAVIOR screen edits the design's assist list (blueprint
    `assists`, saved with the design) in the same graph, inspector and palette;
  - assists `face_target`, `brake` and `auto_fire(group)`, each on one channel (turning, thrust, a group's trigger);
    the pilot's use of a channel wins it at once; every matching rule decides its own channel (in the sim, 10 Hz,
    hashed only for ships with assists: every determinism stream unchanged);
  - conditions `lock_held`, `idle(...)` (pilot), `speed`, `can_stop_straight`;
  - in flight F1–F4 (ASSIST TOGGLE 1–4) and Z (ALL ASSISTS) switch them for the mission, with a banner; the HUD lists
    them under the speed; the debug overlay moved to F8.
- **Hold station and collision avoidance** ([GDR-0088](gdr/0088-pilot-assists-weapon-groups-and-key-bindings.md) step 4,
  15 Oct 2026; GDR-0088 is now built in full):
  - assists `hold_station(point|ally)` (back to where the assist took thrust over, or to the place beside the nearest
    own vessel; the pilot's thrust moves the station) and `avoid_collisions` (sidesteps a piece of any ship on a
    collision course within 6 s, deciding thrust only meanwhile), both on the thrust channel; with brake they turn the ship (flip and burn) only while
    neither the pilot nor face target has turning (Niklas: no base ship has RCS);
    behaviors don't get them (every AI determinism stream byte-identical);
  - a new determinism scenario `assists` (the flight scenario with every assist): native and wasm32-wasip1 identical;
  - the BEHAVIOR screen's tabs are now BEHAVIOR | PILOT ASSISTS, each with GRAPH / TEXT (Niklas); the assists' TEXT
    view edits the list as the blueprint's rule text.
- **Assists on the lock only** ([GDR-0088](gdr/0088-pilot-assists-weapon-groups-and-key-bindings.md), dated line of
  15 Oct 2026; Niklas: no target the assists pick unseen, and both a lock key and an auto-lock assist):
  - an assist list has no target line; auto-fire acts on the pilot's lock (nothing without one); `face_target` is
    removed (a held lock already aims the guns); old designs load with both left out;
  - `auto_lock(pick)` on a new lock channel locks the ship its pick (nearest, weakest, threat, attacker) finds while
    nothing is locked, as a click would; a lock the pilot drops pauses it (PAUSED in the HUD) until they lock again or
    switch it;
  - LOCK TARGET (Tab, the old "lock next"): the nearest enemy, then the next by distance, ordered in the sim;
  - every AI determinism stream byte-identical; the `assists` stream changes (it exercises both).
- **The evolution lab, widened** ([GDR-0092](gdr/0092-evolution-lab-for-your-own-ships.md), 15 Oct 2026; Niklas: re-watch
  any fight, save evolved designs, an end-user feature like a building, start from his own ships). Track L4 of
  `docs/plans/evolution-lab.md` is now GDR-0092's plan, in nine steps:
  1. **Built 15 Oct 2026**: the arena library moved to `crates/core/arena`, the CLI to `crates/tools/arena-cli`
     ([ADR-0040](adr/0040-arena-library-in-core.md); same commands and output); fight codes (`SGF1:`, the spec, the packs,
     the rules fingerprint over the content and two 300-tick probes, the record) printed by `arena fight` and per
     generation by `arena evolve`; the fight viewer `?scene=fight&fight=<code>` (`docs/client.md` "Fight viewer":
     no pilot, pause, 0.25–8×, 1 s back and forward through keyframes, from the start, follow a clicked ship,
     `AS RECORDED` or the changed rules at the end); side looks by owner (side 0 teal, side 1 coral whatever the
     faction); `determinism-check.sh --fight <code>`; every determinism stream byte-identical.
  2. **Built 15 Oct 2026** (with step 3): every fight of a run kept: the genome table (each child a change against its
     parent), job records (22 bytes a fight packed) that become fight codes, run files (`SGL1:` payload) after every
     generation with resume byte-identical to an uninterrupted run (native = wasm, `determinism-check.sh --run`), the
     run's fingerprint and the stale mark, breeding budgeted child by child; `arena evolve --save/--resume/--until`,
     `arena replay`, `arena lineage`;
  3. **Built 15 Oct 2026**: results that can launch: `Drydock::open_placed` (genomes keep frame cells), locks (parts,
     cells, categories, rules, target, LOCK ORDER), the pre-filter (the genome's own rule slots with the repair,
     `max_cores`, `min_hold`, part counts), up to four starts (taken forward from step 6) and the start's build points
     as the default cap; `arena evolve --from/--lock`;
  4. **Built 16 Oct 2026** (behind `?lab=1`): the lab v1 in the game (`docs/client.md` "Evolution"): the EVOLUTION
     building above the mind (states, a ring that fills with the generations, its label on the run) and its layer:
     RUNS (pause, resume, delete asked in the row), SETUP (one to four starts from YOUR DESIGNS and the templates,
     locks in lists: categories, parts, rules, the target line, LOCK ORDER; build points, hold, voyage-ready, mirror,
     the career's parts; design / behavior / both; ENEMY SHIPS and the hall of fame; QUICK / STANDARD / DEEP; seed),
     the run's page (progress as measured, scores by generation, the archive as ship cards, the picked design's fights
     with WATCH and its code, lineage, compare, SAVE AS DESIGN); runs compute at home and on every screen within 4–10
     ms a frame and pause in flight (natively the fights on the compute threads), persist as `lab/<id>` after every
     generation and resume after a reload; designs format 2 with an `origin` (the EVOLVED tag in BLUEPRINTS), share
     codes SGD3; EVOLVE THIS DESIGN in the drydock (taken forward from step 6, like several starts and the locks in
     lists);
  5. **Built 16 Oct 2026**: trials and goals (`docs/arena.md` "Trials and goals"): the `TrialKind` abstraction and
     nine kinds (THE ENEMIES YOU MEET, ENEMY SHIPS, TEST-RANGE WAVES and gauntlets as reinforcements in the referee and
     the viewer, BEAT YOUR START, YOUR FLEET, HALL OF FAME, CHAMPIONS, VOYAGES; WITH YOUR ESCORTS), weights, the four
     goals (WIN, UNHURT, QUICK, CHEAP) kept per fight, `lab.ron`'s goals and trial presets with GDR-0092's default
     selection; the test range's waves moved to `sim::voyage::waves` (unchanged, pinned by a test); VALIDATE (new
     seeds and the recorded fights again), RE-SCORE (a stale run as a new run under today's rules), IMPORT RUN,
     DUPLICATE, RENAME; the SETUP's trials and goals with the estimate, the run's SCORES by trial and goal and its
     fights in fixed columns; the CLI's `--trial`, `--goal(s)`, `arena trials`, `validate`, `rescore`;
     `determinism-check.sh --run mixed_trials` (native = wasm); every older stream and run unchanged;
  6. **Built 16 Oct 2026**: everything mutates within MAY CHANGE's genes (layout, parts by category, armour, power,
     behavior numbers and rules, assist numbers and rules; `lab.ron`'s presets EVERYTHING, the default, DESIGN ONLY,
     BEHAVIOR ONLY, ASSISTS ONLY, NUMBERS ONLY),
     the stand-in pilot (`sim::flight::stand_in`, `lab.ron`'s CASUAL, HANDS OFF, BUSY) and FLOWN BY (BOTH by default),
     locked assists, the LOCKS grid (a part's lock, KEEP EMPTY cells), the CLI's `--flown-by` and `--lock assist:N`;
     `determinism-check.sh --run everything` (native = wasm); runs breed otherwise than before (new run hashes),
     fights and codes unchanged; the same day GENERATION n (a click on the chart lists a generation's children),
     REPLACE START (over a start that is one of YOUR DESIGNS), TIDY at SAVE (`Run::tidied`: an evolved behavior's
     rules that never decided go) and the BLUEPRINTS preview card's ORIGIN (a click opens the run at the design);
  7. **Built 16 Oct 2026**: co-evolution, the ARMS RACE (`lab.ron`'s preset, off by default): an enemy population
     evolves against yours in the same run (its own starts, palette, archive and stream; frozen best and past
     champions each way, your starts as its anchor, packs sized by the wave rule), the PAST CHAMPIONS check and its
     dots in the chart, YOURS | THE ENEMY on the run page, evolved enemies saved as designs and exported to content
     (`enemy_NN.ron` in the CLI); `determinism-check.sh --run arms_race` (native = wasm); runs without an enemy
     unchanged;
  8. **Built 16 Oct 2026**: the career and the wiki: the research node Simulation vault (logic, 3 logic data) opens
     the lab (`facilities.ron`, a new content kind, and `Unlock::Facility`); `?lab=1` is gone; a locked lab is dark,
     says LOCKED and leads to RESEARCH on its node; the EVOLVING chip at home and on FLEET, the LOG and SETTINGS;
     `lab.ron` complete (trial kinds, sizes, measures, spreads, the frame budget; text keys); the wiki's generated lab
     page, `[[lab:…]]` references and the pages mechanics/evolution-lab and mechanics/evolution-trials, with a
     completeness check over every trial kind, trial, goal, gene, pilot, size and preset;
  9. **Built 17 Oct 2026** (Niklas approved): web workers (ADR-0043: the second wasm binary `lab-worker`, jobs and
     results as RON messages, one executor path with the native threads; a QUICK generation in headless Chromium in
     3.5 s with 40 ms of it in the frames, was 48 s and 1.8 s), voyage jobs re-watched (voyage codes `SGV1:` in the
     fight viewer, the flagship followed, checked against the run's hash), THE LADDER (a trial kind and a rating on
     YOUR DESIGNS in the lab, kept in `ladder/<id>`, each rung's worst fight to WATCH, the rating on BLUEPRINTS'
     preview card).
- **Next** (Niklas, 15 Oct 2026):
  1. The evolution lab is complete through GDR-0092 step 9.

### 16 Oct 2026: galaxy exploration, track E0 (`plans/galaxy-voyages.md`)

- **E0 measured** (`results/2026-10-16-bigger-clusters.md`; the bench is sim's `cluster` example,
  `scripts/cluster-bench.sh --wasm`): a 2.5 km cluster with every site spawned and asleep (103 ships, 99 wrecks)
  adds 0.12–0.15 ms per tick natively and 0.16–0.21 ms as wasm to a fight of 20–40 ships; a 3 km one with 375
  bodies 0.28 / 0.48 ms. `f32` holds at 3 km (fights, rams and hold as at the origin, native and wasm IDENTICAL at
  0–30 km; only slow coasting shows the 0.24 mm grid); spawning a site costs under 0.1 ms, the whole cluster at once
  2–5 ms.
- For E3: one `f32` space per cluster centred on the origin, radius ≤ 4 km (GDR-0080 and ADR-0022 dated status);
  keep lazy spawning and sleeping sites, no despawning; spawn per site. Not measured: the client's cost of drawing
  ~250 ships (Niklas's browser, once E3 has a big cluster).
- Next: E1 (content kinds) with E0 done; then E2, E3, E4.

### 16 Oct 2026: galaxy tracks E1 and E2 (`plans/galaxy-voyages.md`)

- Niklas answered the plan's open questions ([GDR-0093](gdr/0093-galaxy-exploration-decisions-16-oct.md)): the S
  unit, the scanner and the starter's, jump range 2.5 → 4 S from any charted cluster, a signal's look and words, SCAN
  for pilots and behaviors, data cores in the hold.
- **E1 built**: six content kinds in `data` (`galaxy`, `bands`, `cluster_types`, `trails`, `signal_sources`, `lore`)
  with validation, JSON export, `content-tool list` and wiki pages (one per entry, the galaxy's with a generated
  example map, the hand-written `mechanics/galaxy`); the rim real, the other four bands stubs; S in `stats.ron`.
- **E2 built**: the `galaxy` crate ([ADR-0042](adr/0042-galaxy-generator-crate.md), numbers
  [GDR-0094](gdr/0094-galaxy-generator-defaults.md)): Poisson-disc clusters at 0.9 S in ring bands (77–85, about 37
  in the rim), home, 3 gates per band, reachability at 2.5 S checked, trails with their signals' areas, interiors from
  each cluster's seed. `content-tool galaxy --seed N --svg F` to inspect; `scripts/determinism-check.sh --galaxy 50`:
  IDENTICAL native vs wasm; seed 1's hash pinned (`e2055530fd51f960`).
- PCG32 moved to `data::rng` (the sim re-exports it). The E0 bench's `records`/`none` difference was the bench calling
  `set_config` after moving the fight, not the director (results note); fixed in the bench.
- Next: E3 (sim: sensor range, hidden sites, `Scan`, beacons) and E4 (career format 2 with the skeleton, signals,
  the jump-range nodes and their unlock kind), side by side; then E5.

### 16 Oct 2026: galaxy track E3 (`plans/galaxy-voyages.md`)

- **E3 built** ([GDR-0095](gdr/0095-sensors-hidden-sites-and-scans.md), lead defaults): the scanner part (GDR-0093's
  numbers) on the starter skiff in place of its right cargo bay (hold 8 → 4, 60.5 BP); SCAN (G), the HUD's SCAN pill
  and the `scan` behavior action (starts a scan, lets the next rule fly); sensors on cores (800 m) bound behaviors'
  targets; a voyage's sites hidden until sensors or a sweep find them, signals' sites (trail places, the slice's goal)
  only within 400 m, far ones get bearings (dashed gold lines in flight); a sweep wakes sleeping sites within 300 m; the
  cluster's edge (radius + 500 m, at most 4 km) turns ships back; `VoyagePlan::roll_interior` rolls a generated
  cluster. Fly one: `?scene=voyage&cluster=signal[&galaxy=<seed>]` (a practice run, never saved).
- The evolution's mirror lock keeps a start's parts without a twin in place (the starter's scanner); the 60 BP skiff
  run allows 62. Hashes changed (the starter, sensors, the new action): native and wasm IDENTICAL (`docs/sim.md`
  "Sensors and scans"); the rules' fingerprint changed, so saved lab runs restore stale.
- Next: E4 (career format 2), then E5 (map).

### 16 Oct 2026: galaxy track E4 (`plans/galaxy-voyages.md`)

- **E4 built** ([GDR-0096](gdr/0096-career-galaxy-format-3.md), lead defaults): the career's save format 3 (the plan's
  "format 2") keeps the galaxy: the skeleton (types moved to `data::skeleton`), charted clusters (home, and every
  cluster launched into), signals heard (fresh → narrowed → located: the area halves towards its place), logs from data
  cores, gates opened, the slice's voyages pinned to clusters until E7. `Unlock::JumpRange` with Jump calibration I and
  II in the propulsion lane (2.5 → 3.25 → 4 S, GDR-0093), jumps from home or any charted cluster; cluster voyages in the
  model (`cluster_launched`, `cluster_ended`, their seed) and `Leads` for `VoyagePlan::roll_interior` (signals' sites
  only of heard signals, a log's data core in its first wreck: one cargo unit, lost with the haul).
- Slice saves (format 2) migrate at load and are written at once: the voyages pinned in a chain from home, the flown
  ones charted, the open one a located signal. `?scene=voyage&cluster=` flies a cluster of the career's galaxy with what
  the career knows (`&galaxy=<seed>`: another galaxy with everything in place).
- The drydock's POWER leaves draws in bursts out of full use (`power.intermittent`, the scanner's "+3 WHILE
  SCANNING"): the starter skiff reads 26 / 28 instead of 29 / 28 in coral. The rules' fingerprint changed (the
  scanner's power block), so saved lab runs restore stale.
- Next: E5 (the map: unknown space, charted clusters, signal areas, range circles, blind-jump picking), then E6.

### 16 Oct 2026: galaxy tracks E5 and E6 (`plans/galaxy-voyages.md`)

- **E5 built** ([GDR-0097](gdr/0097-galaxy-map-and-voyage-flow.md), lead defaults): the voyage map is the career's
  galaxy (the skeleton at 120 px a S, home left of the centre, the slice's voyages on their pinned clusters; the
  voyages' content `map` place is gone): charted clusters, opened gates, located signals, signal areas as Niklas chose
  (dotted gold edge, faint fill, a pulse to the edge; a narrowed one's old edge fades) worded `SIGNAL · A GATE` over
  the source's line, the jump range as soft teal circles; a click on empty space picks a blind jump. The galaxy's
  backdrop blended about 12 times its disc (73 M px² a frame at the default view, the 2560 × 1440 screenshot timed
  out): now under 5 times a smaller disc, about a tenth.
- **E6 built**: SET COURSE warps on the map (3 s + 2 s per S, Esc ends it), then records the launch; in flight scans
  narrow signals (once a voyage), hear beacons, signals' sites are found and done, kept on a lost voyage; the debrief
  lists charts, signals and logs; the LOG decodes data cores; N lays a cluster map over the flight view. The rules'
  fingerprint changed (the voyages' `map` went), so saved lab runs restore stale.
- Next: E7 (the slice pinned by content, the Warden's gate the rim's first gate trail), E8 (rim content).

### 17 Oct 2026: galaxy tracks E7 and E8, the galaxy track done (`plans/galaxy-voyages.md`)

- **E7 built** ([GDR-0098](gdr/0098-slice-in-the-galaxy-and-rim-rewards.md), lead defaults): the slice's voyages are
  set pieces in `galaxy.ron`, placed by the generator (version 2) at Niklas's distances from home, the Warden on the
  rim gate nearest the inner edge: its four voyages are the rim's first gate trail and clearing it opens the outer
  reach (the map says `GATE · OPEN`, the debrief `GATE OPEN · THE OUTER REACH`). The slice's goal is an ordinary site
  again (E3's stopgap). Save format 4: a format 3 save keeps E4's pins, its Warden's cluster becomes a gate at load.
- **E8 built**: the rim's trails end at their own encounters (gate sentinel, sealed cache, memory wreck, deep
  graveyard) and pay their rewards on their last signal done, kept on a lost voyage: gates open, caches give research
  data and programmable matter, lore fragments join the memory log (the LOG's MEMORY rows), graveyards are rich
  salvage; banners in flight, debrief lines.
- **Tunings**: signals located too fast (one scan located 83 % of them): fresh areas 3–4 S, a scan keeps 60 % of the
  radius: 15 % located by the first scan, 2 or 3 voyages to find one 66 % of the time (`content-tool galaxy --seeds
  8000`). Home had 1–4 free clusters in range (median 2): now at least 4 in every seed. The generation hash changed
  (seed 1: `8a509f91bd8fd0b8`); native = wasm. The rules' fingerprint changed (new encounters): saved lab runs
  restore stale.
- **The map** (Niklas): labels cut at a wide window's stage edge, the jump range's arcs meeting with a notch, and its
  flicker while zooming fixed (the outline is the union's exact arcs now).
- The galaxy plan's tracks are done. Next candidates: caches with a node or part of their own, milestone cards
  (GDR-0080), the bands past the rim with their races and guardians.

### 17 Oct 2026: blast reach in the drydock, FLEET's preview and formation, bubbles over other hulls

- **Blast reach and warnings** (the follow-up of GDR-0084 and GDR-0085): the design report lists every part that
  explodes with the sim's own targets, damage and cover (`DesignReport::blasts`, tested against a world), and warns
  when a blast would take a logic core, a gun or another exploding part (`Issue::BlastTakes`). The drydock's BLAST
  pill draws each reach, the cores and guns a blast takes (coral), the rest it destroys, what survives (gold) and the
  armour that covers (teal); a reactor being placed or selected shows its blast. The starter and the Runner show the
  warning (their reactors sit beside cores and guns). `docs/drydock.md` "Overlays".
- **FLEET** (`docs/client.md` "FLEET"): a click or ←/→ previews a design in the picked slot (dimmed ship, `PREVIEW ·
  ENTER ADDS IT`), Enter or a second click adds it; the dotted formation ring round the flagship with a line to each
  escort (dashed to a preview); an escort's line says the behavior it flies (`1 CORE · FLIES ESCORT`). The research
  screen's arrow keys step between cards.
- **Shield bubbles** (ADR-0029 note): one mesh, two passes (over space under every ship's tiles; over the own
  piece's tiles at their exact depth), so no bubble shows over another ship's hull.

### 17 Oct 2026: Niklas's decisions (the Grown, balance targets, the lead defaults)

- **The Grown and the outer reach** ([GDR-0099](gdr/0099-the-grown-and-the-outer-reach.md)): heart-fed regrowth,
  rammers with thorn guns, calm until provoked, regrowing tissue armour and a ram prow first, hot clusters, a
  precursor guardian at each gate to the middle reach. Built in two phases on 17 and 18 Oct 2026 (below).
- **Fight balance targets** ([GDR-0100](gdr/0100-fight-balance-targets.md)): GDR-0073's curve (the Warden about 40 %
  for a Runner and an escort), shields that collapse, encounter counts first, turn rates and test-range waves kept;
  the re-measurement is in `results/slice-balance-2026-10-17.md`: the Warden's raiders 2–3 and Corran Reach's
  hunter packs ×2 bring the Warden to 44 % (was 62 %); open: the first voyages stay easy for the starter, the cannon
  starter clears the Warden alone 37 %, shields collapse in 0–9 % of waves.
- **A different first enemy and shield capacity 2** (Niklas, same day; second pass
  `results/slice-balance-2026-10-17b.md`): the automaton gunner ([GDR-0101](gdr/0101-the-automaton-gunner.md): twin
  cannons, `gunner_pack` aims at the engines) in three new encounters on the first two voyages, a gunner pair in
  Corran Reach and wreck drifts; shield capacity 3 → 2 (one collapse in six sustained waves). The starter 57 / 37 / 22
  / 15 %, the Runner 74 / 65 / 40 / 25 %, a Runner with an escort 42 % at the Warden; open: the cannon starter (37 %
  alone at the Warden).
- **Lead defaults accepted**: twenty records' defaults, and GDR-0089's except the speed governor (judge in play);
  `gdr/README.md` lists them.
- **The outer reach, phase 1 of 2: the Grown's mechanics and parts** (lead, same day;
  [GDR-0102](gdr/0102-grown-mechanics-and-parts-defaults.md), lead defaults): hearts re-grow lost parts (one every
  2.5 s at 35 %, from living neighbours, nearest first; killing the heart stops it), grown tissue armour and the ram
  prow heal on a vessel, the ram prow deals 2 × and takes 0.5 × the ram damage, the thorn gun (kinetic, 1.6/s at 40 m),
  wary sites (the Grown faction) wake only when provoked and every site wakes when its wreck is stripped, hot stars
  heat every open side by the inverse square (`scorched_deep`, a stub in the outer reach); a `grown` determinism
  scenario (identical native vs wasm), the parts locked behind `?unlock=all`, `?scene=voyage&cluster=hot`, wiki pages.
  Next, phase 2: the Grown's ships, behaviors, groves, cluster types, research family, gate guardians and bosses.

### 18 Oct 2026: the outer reach, phase 2 of 2: the Grown in play

- ([GDR-0103](gdr/0103-the-grown-in-play-defaults.md), lead defaults; `results/outer-balance-2026-10-18.md`) The
  Grown's rammer, thornback, grove guardian and the giant elder on a ganglion beside the heart; the `ram` behavior
  action; groves (wary, their wrecks as bait), rim stragglers in a border drift near the rim, groves beside the hot
  stars; the `automaton_bastion` at each outer gate, two elder groves at the ends of their own trails; the Grown
  research family (regrowing tissue → prows and thorns → the heart). A Runner with one cannon skiff clears a grove run
  55 %, an outer gate 52 %, an elder grove 44 %. Saved designs carrying `automaton_cannon` are refitted with the
  player's cannon on load. `?scene=voyage&cluster=grove_reach` flies the first grove reach.
- Open: careers made before keep their galaxy (their outer reach keeps the stand-ins but for the hot clusters);
  the Grown's materials; heat research.

### 17 Oct 2026: scenery between the sites

- ([GDR-0104](gdr/0104-scenery-rocks-filler-landmarks.md), lead defaults; `results/scenery-2026-10-17.md`) Asteroid
  rock as anchored bodies per cluster type and slice voyage (fields, belts, scattered; shots, plumes and rams stop on
  it, behaviors and the scripted pilot steer round it, `avoid_collisions` sidesteps it); content kinds `rock` and
  `landmark` and a `scenery` block; the client's glints, debris, derelicts, landmarks, the corona and distant fights (removed 17 Oct, GDR-0107)
  (`world::scenery`). The slice's curve holds with rock (120 seeds), up to 13 points harder.
- Open: mining (a laser part, rock yield, `RockHit` into ore), drifting or breakable rock, rock on the cluster map.

### 17 Oct 2026: a cluster's sky

- ([GDR-0105](gdr/0105-cluster-sky-planets-nebula-galaxy.md), style B by Niklas, numbers lead defaults) A planet,
  its moons, a nebula and a far galaxy behind every cluster, rolled from the cluster's seed (`world::scenery::sky`),
  painted per vertex so every edge is soft; content kind `planet` (13 planet and 6 moon kinds) and a `sky` block on
  cluster types and bands; the far stars slowed to 0.02. Screenshots `results/sky-*.png`.
- Open: the sky tinting the gradient and the stars, turning planets and drifting clouds, the sky at the station and
  on the map.

### 17 Oct 2026: depth layers after the play-test, the minimap

- ([GDR-0107](gdr/0107-depth-layers-zoom-and-draw-order.md), lead defaults) Every layer before or behind the play
  scales with the zoom by its depth (`camera::depth_scale`), draws by its depth (landmarks, the star's glow and
  corona behind the planet), the stars in three layers; the distant fights removed; rocks drawn finer (rounded,
  roughened outlines, shading, craters; colliders unchanged). Screenshots `results/depth-*.png`,
  `results/scenery-rock-close.png`.
- ([GDR-0106](gdr/0106-flight-minimap.md), lead defaults) The minimap: bottom right on a voyage, north up, 1 km /
  2.5 km / off on B, only what the fleet knows; the cluster map shares its painter (and shows rocks, sensor rings and
  contacts now). Screenshots `results/minimap-*.png`.
- Open: a tap on touch to step the minimap; the minimap in the test range.

### 18 Oct 2026: the materials economy, the backdrop tuned

- ([GDR-0108](gdr/0108-materials-economy-workshop.md), Niklas; numbers lead defaults) The WORKSHOP (the fabricator
  and the salvage bay merged): recipes for every non-hull part, three fabrication resources, the first workshop
  upgrades, the fleet fitted from the workshop's stock (launch blocked when parts are missing; FABRICATE MISSING),
  hauled intact parts kept or dismantled (SORT AS SUGGESTED), most stripped parts break (sim `intact_chance` 0.15),
  dismantling returns 40 % (broken 25 %); save format 5 with older saves stocked from their designs. Inflow per
  voyage: `results/materials-inflow-2026-10-18.md`. Screenshots `results/workshop-*.png`.
- ([GDR-0107](gdr/0107-depth-layers-zoom-and-draw-order.md) status line) The backdrop's layers by parallax (stars
  0.01–0.06, planet 0.09, junk 0.3 and 0.7), fewer far stars, thinner near junk, smaller planets, a soft spiral
  galaxy. Screenshots `results/backdrop-*.png`.
- Open: hull plating as stock, dismantling stock parts, installing salvage with a bonding cost, harvesting the ores,
  more workshop upgrades, the inflow tuned after play.

### 18 Oct 2026: legacy cleanup

- ([ADR-0044](adr/0044-no-save-compatibility-before-release.md), Niklas) No save compatibility before release: the
  save migrations (formats 2 to 4, the older voyage roller), the parts' `replaces` refit, the first assist lists'
  `target` and `face_target`, the evolution runs' old `fitness`, `start` and plain-number forms, and the flight
  parameters that started the sandbox without `scene` are gone. An older save starts a new game (a log line, a note).

### 19 Oct 2026: mining and ore refining

- ([GDR-0114](gdr/0114-mining-and-ore-refining.md), lead defaults; `results/mining-2026-10-19.md`) The mining
  laser (`weapon.cut`, a beam's ore a second on rock) cuts a rock's ore (its kind's `ore` per m²) into the hold as
  `Item::Ore`, a spent rock drawn darker; the smelter within the workshop (`refine` conversions in data) pays steel
  from nickel-iron and coolant from ice for recipes and research; ore home in GAINED and the store; the `mine`
  behavior action, the `miner` escort behavior; the voyage runner mines. Closes the mining open on 17 and 18 Oct.
- Open: drifting or breakable rock, the other harvests (comets, gas, star skimming), a tractor, mining in the lab's
  trials, a mining sample ship.

### 19 Oct 2026: missiles and flares

- ([GDR-0115](gdr/0115-missiles-and-point-defence.md), lead defaults; `results/missiles-2026-10-19.md`) The rocket
  pod and the torpedo launcher fire homing missiles that blast by the chain-reaction rule, are absorbed whole by
  shields and can be shot down; the point-defence turret aims itself (missiles first, then small vessels) and needs a
  line of sight; the missiles research family; the automaton lancer (one rocket pod) in the lancer pack; the
  condition `missiles_incoming`. Flares the same day (GDR-0018's other counter): the flare launcher (three releases a
  fight), the FLARE key, the `flare` action and the `auto_flare` assist. Against a lancer pack the laser starter wins
  17 of 60, 28 with flares, 47 with point defence. The drydock's ARCS shows launch cones and the turret's arc.
- Open: whether a warhead spares friendly vessels; missile, point-defence and flare numbers.

### 19 Oct 2026: drones and the Swarm

- ([GDR-0117](gdr/0117-drones-and-the-swarm.md), lead defaults; `results/swarm-balance-2026-10-19.md`) A hangar
  part launches drones (small vessels with a nerve node for a core) while its ship has a target, one every 0.3 s
  through an open mouth, at most 48 alive; they fight on a leash, take no mind capacity, are no part of the fleet and
  die with their hangar. The Swarm fill the middle band (no longer a stub): spitters, hive carriers and the anchored
  hive (the band's boss site), Swarm reaches, brood deeps and raided borders, a research family ending in the
  player's drone bay; the drydock draws each hangar's launch lane. A Keeper-capable fleet clears a carrier voyage
  43–49 %, a hive voyage 44–45 %; a capped flood of 48 drones costs 0.06 ms a tick. Play-test link for the deep
  bands: `?newgame=1&save=<slot>&bands=<band>` (a test career with the kits of `content/base/playtest.ron`).
- Open: repair drones, drones that dock again, a recall action, a drone's look beyond its parts.

### 19 Oct 2026: the Spire

- ([GDR-0116](gdr/0116-the-spire-guards-the-inner-bands-gates.md), lead defaults;
  `results/spire-balance-2026-10-19.md`) The inner band's gates get their own guardian (trail `inner_gate`, two gate
  shards; the Keeper keeps the middle band's): `automaton_spire`, a three-section spine on cores of its own with
  torpedoes and rocket pods under four prism wards that take energy fire whole, escorted by three lancers and two
  hunters. Alone, a strong late fleet clears it 42–45 %, an all-laser one 12 % (the Keeper 81 / 82 / 66 %).
- Open: the inner band's clusters are still the uncharted stub (re-measure when the Lattice exist).

### 19–20 Oct 2026: phones and small screens (`plans/android.md` 4.1–4.4)

- ([ADR-0045](adr/0045-compact-layout-class-and-touch-in-the-kit.md), Proposed) The kit's layout class follows the
  window (`Compact` under 500 dp high; phone layouts made for 844 × 390 and scaled by `compact_scale`), touch in the
  kit (finger hover, 48 dp hit slop, a finger's drag scrolls every list, a finger on the UI never reaches the world),
  `TouchScreen`, `SafeArea`, `CompactText`; tested in Chrome DevTools' device mode or `web-drive.mjs --dpr=3`.
- The audit's four work packages, all done: D (home and its layers, FLEET, WORKSHOP, LOG, SETTINGS, the debrief;
  research, the voyage map, evolution), B the drydock ([GDR-0118](gdr/0118-drydock-on-a-phone.md), Proposed: a
  tool strip, the stats or the selected part, a 120 ms hold before a tool runs, machines held 48 dp above the
  finger, two fingers pan and pinch), C BEHAVIOR ([GDR-0119](gdr/0119-behavior-editor-on-a-phone.md), Proposed:
  the graph pans sideways, a row held 350 ms drags its rule, TEXT read-only), A flight and the fight viewer
  ([GDR-0120](gdr/0120-flight-and-the-fight-viewer-on-a-phone.md), Proposed: a floating stick, FIRE and FLARE, the
  corner ring pauses, the viewer's timeline scrubs). The kit's tooltips by a long press and flings of lifted lists
  ([GDR-0121](gdr/0121-tooltips-and-flings-by-touch.md), Proposed); a sweep of every screen at 844 × 390 DPR 3.
  Screenshots `results/phone-*.png`.
- Open: the list in `client.md` "Phones" ("Still not built for phones": the soft keyboard, the pinch checked on a
  device, `SafeArea` from Android's insets, the packages' leftovers); the Android build itself.

### 20 Oct 2026: the balance check (coop-arena A3)

- ([GDR-0122](gdr/0122-the-balance-check.md), lead defaults) GDR-0032's "short run on every data change":
  `scripts/arena-check.sh` (`arena check`, `arena::check`; `docs/arena.md` "The balance check") runs seven fixed
  fights (the four of `crates/core/arena/fights/` plus `skiff_vs_lancer`, `mid_vs_warden`, `runner_vs_rammers`; 1696
  seeded runs, about 6 s on 4 cores) against `crates/core/arena/check/baseline.ron` and exits 1 when a fight's win
  rate moves more than 0.15, side 0's score more than 0.12 or its mean time more than 15 %; `--update` writes the
  baseline after an intended change. Run it after every content or sim change.

### 20 Oct 2026: workshop upgrades for missiles and the Swarm

- ([GDR-0108](gdr/0108-materials-economy-workshop.md)'s status, lead defaults) two upgrades from GDR-0031's list,
  facilities within the workshop: the **munitions bay** (the rocket pod's, torpedo launcher's and flare launcher's
  recipes; installed by the Rocket pod research or Flares) and the **hive press** (every Swarm part's recipe;
  installed by Stingers). No new nodes or costs. The automaton forge already had its node (a gate shard). A content
  test: every researched part can be fabricated once its node, or one it requires, is researched (the recorded
  exceptions: the automatons' parts wait for the forge, the torch drive for the fusion reactor's node).

### 20 Oct 2026: behavior words for salvage

- ([GDR-0123](gdr/0123-behavior-words-for-salvage.md), lead defaults; `plans/first-playable-loop.md` §7) the
  behavior action `salvage` (the nearest slow salvage within sensor range with something that fits the hold, flown
  over at range 0 so collecting strips it), the conditions `wreck_in_range(range)` and `cargo_full`, the content's
  `salvager` escort template (the escort's rules with salvage between fights); the salvage determinism scenario flies
  one. The voyage runner's escorts keep `escort`: with `salvager` they doubled the haul but cleared fewer goals
  (47 against 61 of 80 on `yard_approach`).
- Open: the tractor (GDR-0031), a leader-distance word, salvage in the lab's trials.

### 20 Oct 2026: ratings and part reports (coop-arena A4)

- ([GDR-0124](gdr/0124-ratings-and-part-reports.md), lead defaults) GDR-0032's ratings and reports: `arena report`
  (`arena::report`; `docs/arena.md` "Ratings and part reports") fights a pool one against one in a round robin and
  rates it by a Bradley–Terry fit on the Elo scale; per part its pick rate among the top half, win rate and score when
  used, and the flags DOMINANT, UNDERUSED, NEVER PICKED (placeholder thresholds). The pool: designs given, a run
  file's best elites (`--run`) or `crates/core/arena/report/pool.ron` (15 content ships, 3360 fights, about 5 s).
  The first report (`results/arena-report-2026-10-20.md`): no part DOMINANT, the ratings follow the factions (the
  automatons' parts UNDERUSED with their ships), the Swarm carrier first at 59 build points, 15 researched parts
  carried by no content ship. No balance change.
- Open: a report over evolved designs from one palette (the thresholds revisited then); TrueSkill for team fights.

## Phase 4: measurement

Status 12 Oct 2026 (new VM): `results/spike-summary.md` written. Determinism native vs wasm IDENTICAL on the new VM
(all scenarios, two seeds); native load test 20 ships 0.011 ms median / 0.028 ms p95 per sim step. Pending: Niklas's
browser numbers (load test, look cost, optional browser hash) and his verdict on the flight feel (steps in the
summary). Phone questions stay deferred to Android.

- Load test in the browser: 1, 5, 20 ships with debris and projectiles; style 2a vs. debug shapes; glow sprites vs. bloom.
- Cross-platform hash: native vs. wasm (phone later).
- `results/spike-summary.md` answering the spike questions that can be answered without the phone.

## Later

- **Fancier backdrop** (Niklas, 12 Oct 2026): planets, nebulas and so on. The current backdrop is sufficient for
  now.

- Drydock (web first), behavior trees + Rhai, salvage/research, galaxy, home ring menu.
- Android build + phone measurements (deferred spike questions 4–7 on the phone).
- Wiki generator on top of `data` + `content/`: done 15 Oct 2026, the VitePress wiki (`wiki/`, ADR-0039; it replaced
  the static generator of 13 Oct).
- Co-op (`net`), arena/evolution.
