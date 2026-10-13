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
`plans/salvage-research.md`, `plans/galaxy-voyages.md`, `plans/coop-arena.md` hold what waits). Decided 13 Oct: start
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
| 7E Voyage director | `sim::voyage`: rolling, lazy spawning, waking, clearing, recall check; `ScenarioKind::Voyage`; the headless voyage runner | 7A, 7B | in progress |
| 7F Save and gating | `saves/career`, new game, `?save= ?newgame= ?unlock=all`; the drydock from `Unlocks`; FLEET | 7C, 7D | wave 2 |
| 7G Research screen | `AppMode::Research`, the debrief, unloading the haul | 7C, 7D, 7F | wave 2 |
| 7H Voyage flight | `SceneKind::Voyage`, launch, the voyage HUD, recall, death → home, the save update | 7E, 7F | wave 3 |
| 7I Voyage map | `AppMode::Map`, the chain of clusters, SET COURSE | 7C, 7D, 7F | wave 3 |
| 7J Slice content | Starter ship, locks, nine nodes, salvage values, four voyages, five encounters, the hunter and the Warden | 7A | wave 3 |
| 7K Balance pass | Headless voyages over seeds against the evening budget; tune encounters and node costs | 7E, 7J | wave 4 |
| 7L Play-test | Niklas plays an evening | all | wave 4 |

Alongside (13 Oct): six code reviews, a UI review, a security review of untrusted input and a dependency audit
(reports in the lead's scratch space; fixes merged as they land), a docs audit (merged), the wiki generator and a
fight-balance re-measure (GDR-0061, proposed).

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
- Wiki generator on top of `data` + `content/`.
- Co-op (`net`), arena/evolution.
