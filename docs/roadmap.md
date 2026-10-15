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
- Two non-focus ships still show each other's shield bubbles over their hulls (ADR-0029 note).
- Browser load-test numbers (`?load=1|5|20`) need Niklas's real browser (headless timings are meaningless).

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
  120. Follow-ups: an explosion visual in the client, blast reach and warnings in the drydock, and the rule
  questions listed in GDR-0084.
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
  - the save keeps them (`history`, format 2; an old save's `last_haul` starts it);
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
  4. the lab v1 in the game (the EVOLUTION building and layer, one start, threads natively and slices on the web,
     persistence, SAVE AS DESIGN), behind `?lab=1`;
  5. trials (the `TrialKind` abstraction, the test-range waves in shared code, goals, VALIDATE);
  6. everything mutates (behavior structure, assists with a stand-in pilot, the LOCKS UI);
  7. co-evolution (the ARMS RACE);
  8. the career and the wiki (the Simulation vault, `lab.ron`, the wiki page; `?lab=1` goes);
  9. optional: web workers, voyage replays, THE LADDER.
- **Next** (Niklas, 15 Oct 2026):
  1. The evolution lab, GDR-0092 step 4 (the lab v1 in the game, on `arena::Run`'s API, `docs/arena.md` "The API").

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
