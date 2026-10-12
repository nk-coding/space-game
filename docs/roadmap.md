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

## In progress (12 Oct 2026, second session)

Niklas's play-test feedback, decided interactively (GDR-0041 to GDR-0047, `docs/concept-notes.md`):
- Done: engine rated speed instead of the soft top speed, a calm cruise flame, "Agile" ships (GDR-0041,
  GDR-0042; `raider_basic` retuned to stand off at 85% and orbit at 67% of its reach).
- Done: shields as glowing energy bubbles, one batched procedural mesh (GDR-0043, ADR-0029); see them hit with
  `?scene=showcase&fire=1&zoom=70`. The content embed in `data/build.rs` now tracks every file and directory.
- Done: the vessel model in `sim` (GDR-0013, 0038, 0044, 0045; `docs/sim.md` "Vessels"): owners, split-off
  vessels inheriting slot-limited behavior, focus as a sim command with automatic fallback, control cost and
  capacity with idle-by-priority, mind capacity with newest-dormant and wake/swap, friendly and shielded rams;
  ramming v2 (GDR-0047). Placeholders: logic core N 16, automaton core N 4, player mind capacity 24.

- Done: engine plumes burn parts in their path (GDR-0039, 0046, 0049), no friendly fire (GDR-0048), the flight
  computer plans only with working actuators.

Running now:
- Client: fleet bar, camera glide on focus switches, focus draw order, `?damage=` for the player only,
  browser-vs-native hash, screenshot checks (the atlas tier switch is already faster: 521c479).

Open questions for Niklas (ask interactively):
- Mind capacity numbers and N per core type: lead placeholders until he sets them.
- Vessel model lead defaults to review (`docs/sim.md` "Vessels", marked *lead default*): focus fallback order;
  a dark focus keeps the focus; dormant vessels are fully dark but owned; reactors idle after every consumer.
  Known limits: the flight computer doesn't know idle actuators; shots have no friendly rule (a split-off vessel
  can hit its parent).
- Shield collapses are rare in play (regen 1.5/s vs ~0.4 per cannon hit, kinetic passes half to the hull): a
  balance question once fights are tuned. All bubbles draw on one layer above every ship.
- Plume defaults and numbers (GDR-0049), whether plumes push what they hit, drawing the plume's full reach
  (`docs/sim.md` questions 16–18).
- Lead defaults to review: diagonal top speed can beat a rating (per-axis falloff); differential thrust nudges a
  locked ship sideways at speed; A/D spins the Runner at ~4.6 rad/s (`docs/sim.md` questions 14, 15).

Client leftovers:
- Browser-vs-native hash (`?hashlog=1&load=20` vs `determinism --scenario load`) and browser load-test numbers (`?load=1|5|20`) not run yet.
- Mipmap generation after an atlas tier switch is one frame (178 ms hitch at tier 256); spread over frames / async on native.
- Not verified: tuning panel visuals, touch input, `?fleet=N` wingmen and V switching; focus vessel draw order after a switch; `?damage=` scales enemy weapons too.

## Queued

Now unblocked (Rapier merged):
- Client side of the vessel model: fleet bar (wake/swap dormant vessels, `ShipCommand::Wake`), camera glide on
  focus switches; V / Shift+V and click-to-focus already go through `ShipCommand::Focus`.


## Phase 4: measurement

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
