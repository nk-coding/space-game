# Roadmap

Living plan, kept up to date by the lead session. Decisions themselves live in `concept-notes.md`.
Started 11 Oct 2026. Web first, Android later, minimal content, wiki-ready data.

The original spike plan (`spike-plan.md`) still defines what must be answered. The spike code is written as the real foundation of the game rather than as throwaway code. Phone measurements are deferred until Android work starts.

## Phase 1: foundations (parallel)

| Track | Goal | Status |
|---|---|---|
| 1A Physics benchmark | Rapier2D, box2d_rust and Avian 2D (Box2D C as reference if it builds) on the spike scenarios, natively and as wasm (Node + a page for the host browser). Determinism hashes native vs. wasm. Niklas picks the engine. | in progress |
| 1B Content + `data` crate | Content model for game + wiki, schema, pack loader, validation, blueprint geometry, minimal base pack, `content-tool` CLI (validate, export JSON). | in progress |
| 1C Web client | Bevy 0.20 wasm build with Trunk, style 2a ship rendering (atlas from SVGs, tiles, seams, icons + glow, hull edge glow, aura, plumes), backdrop, camera. | in progress |

## Phase 2: core simulation

- Ship model in `sim` from `data` blueprints: occupancy grid, per-part HP, pieces (connectivity with slope rules), mass/CoM/inertia, power pool per piece, simple heat, thrust/torque, gun and engine clearance.
- Determinism infrastructure: fixed 30 Hz step, RNG streams, state hash per tick, native-vs-wasm hash test.
- Physics integration with the chosen engine: compound bodies per piece, splitting with inherited velocity and spin.

## Phase 3: flight and feel (web)

- Flight computer (stick = fly that way, braking, drift toggle, soft top speed), target lock, WASD + mouse.
- 30 Hz sim with 60 fps interpolation in the client.
- Automaton raiders with a trivial AI (approach, orbit, fire). Basic weapons and damage, breakage, debris.

## Phase 4: measurement

- Load test in the browser: 1, 5, 20 ships with debris and projectiles; style 2a vs. debug shapes; glow sprites vs. bloom.
- Cross-platform hash: native vs. wasm (phone later).
- `results/spike-summary.md` answering the spike questions that can be answered without the phone.

## Later

- Drydock (web first), behavior trees + Rhai, salvage/research, galaxy, home ring menu.
- Android build + phone measurements (deferred spike questions 4–7 on the phone).
- Wiki generator on top of `data` + `content/`.
- Co-op (`net`), arena/evolution.
