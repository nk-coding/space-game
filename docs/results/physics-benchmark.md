# Physics engine benchmark (spike step 1)

**Decision (11 Oct 2026): Niklas chose Rapier2D.**

Run on 11 Oct 2026 in the VM. Harness: [`crates/bench/`](../../crates/bench/README.md). Raw results:
`crates/bench/results/2026-10-11/` (JSON lines per run, logs, `env.txt`, generated `summary.md`).
Niklas decides; this page recommends.

## Summary

- **Recommendation: Rapier2D 0.36 with one compound collider per ship piece** (`SharedShape::compound` of the
  greedy-merged rectangles plus slope triangles), `enhanced-determinism`. It was the fastest or tied for fastest
  natively in every realistic scenario. It splits a ship in ~0.4 ms on average (0.04 ms with merged
  rectangles). Its hashes are identical natively, in wasm (Node and browser) and multi-threaded. And it is a
  plain library that fits the engine-free `sim` crate and its `PhysicsBackend` seam.
- **Integration hint:** give each piece one compound collider rather than one collider per convex polygon (the
  plan in `docs/sim.md`). With merged rectangles that was ~5× faster for the ships (0.06 vs 0.29 ms) and ~25 %
  faster in `combined-merged` (2.8 vs 3.8 ms).
- **Collider layout matters more than the engine.** One collider per 1×1 cell costs 5–100× more than one
  compound shape per body, in every engine. Box2D has no compound shape, so it needs merged rectangles. Rapier
  and Avian are fast with a compound shape even with one part per cell, and fastest with merged rectangles
  inside the compound.
- **Determinism held everywhere.** Every engine gives the same state hash on every native run, in a separate
  process and with all engines linked into one binary (all 54 engine × scenario combinations). Native equals
  wasm under Node in all 45 wasm combinations. Multi-threaded Rapier, Avian and Box2D C give the same hash as
  single-threaded. In headless Chromium (full 930-tick runs of 5 scenarios × 5 engines) all 25 hashes matched
  native and Node. box2d-rust is bit-identical to the Box2D C library in all 9 scenarios.
- **wasm** (Node, V8) is 1.2–1.9× slower than native on the realistic scenes, similar for all engines. The
  ranking stays the same.
- **Runner-up: Avian2D with compound colliders.** It is the fastest when a compound has one part per cell.
  It loses on debris and in the combined scene, ties the simulation to Bevy's ECS, needs Bevy 0.19 (the game is
  on 0.20, and Avian has no 0.20 release yet), and its idiomatic one-entity-per-collider layout is the slowest
  setup measured (a ship split costs ~27 ms).
- **box2d-rust** is a faithful, deterministic, tiny port. It runs single-threaded only and has no compound
  shape. With merged rectangles it was 1.8× slower than Rapier-compound in the combined step (5.0 vs 2.8 ms),
  2.1× on debris and ~9× on the 20 ships alone (0.56 vs 0.06 ms). Box2D C with 4 worker threads scaled best of
  all engines, but C is reference-only.

## Setup

| | |
|---|---|
| Machine | VM (KVM), Intel Core Ultra 7 155H (P/E/LP-E hybrid laptop CPU), 15 vCPUs, 11 GB RAM, Linux 6.8 |
| Toolchain | rustc 1.99.0, release builds (`opt-level 3`, `codegen-units 1`, thin LTO; wasm: fat LTO), Node 24.11.1 |
| Load | Other agents compiled Bevy at the same time. 1-min load average: native one-process-per-run phase 3.6–12.3 (median 7.3), **lockstep phase 1.3–8.3 (median 2.0)**, multi-thread phase 4.7–13.3 (median 9.4), Node phase 2.7–10.1 (median 6.4), sleep-off phase ~8–13, native/Node pairs 3.4–20 |
| Engines | rapier2d 0.36.1 (`enhanced-determinism`; `parallel` for multi-thread), box2d-rust 1.4.0, avian2d 0.7.0 on Bevy 0.19.1 (`enhanced-determinism`; `parallel` for multi-thread), Box2D C at commit 56edae7 (3.2-dev, the commit box2d-rust ports) via boxdd-sys 0.6.0, gcc `-O2`, SSE2 |
| Variants | `rapier` = one collider per shape on the body; `rapier-compound` = one `SharedShape::compound` per body; `avian` = one child collider entity per shape; `avian-compound` = one `Collider::compound` per body; Box2D always has one shape per polygon (no compound type) |
| Settings | zero gravity, dt = 1/30 s, 4 substeps everywhere (Avian's default is 6), density 1, friction 0.5, restitution 0, each engine's default sleeping and continuous collision. 30 warm-up ticks + 900 measured ticks (30 s), 3 repeats |

**Scenarios** (one seeded generator, identical input for every engine; details in the bench README):

| Scenario | Content |
|---|---|
| ships | 20 ships of 200–2048 cells (22 592 shapes in total: one 1×1 m box per cell, plus slope triangles on convex corners). They fly in 10 pairs on collision courses (contact after 3–15 s) and all spin. |
| ships-merged | The same ships built from greedy-merged rectangles plus triangles (1 276 shapes) |
| debris | 2000 chunks of 1–4 cells in 8 drifting, colliding clouds |
| raycasts | ships + debris + 5000 closest-hit rays per tick (random, 200 m long; ~1 900 hits per tick) |
| breakage / breakage-merged | Ships with one cut per simulated second (30 events). Each event removes a row of cells (~26 on average). The parent keeps the largest part, with its collider rebuilt, and each other part becomes a new body that inherits the parent's velocity at its own centre of mass. |
| asteroids | 300 convex polygons (5–8 vertices, 3–14 m); a third are static |
| combined / combined-merged | Everything at once: 2 320 bodies, 28 292 shapes (6 976 when merged), 5000 rays per tick, 30 splits |

All engines produce the same macro outcome: pairs collide, split pieces drift off with their parent's motion,
and nothing explodes. I checked this visually from final-state SVGs (`bench-native --svg`).

## Results: native, one thread

**Lockstep** (primary table): all engines ran in one process and took turns, one tick each, with the order
rotated every tick. That way every engine sees the same machine noise at the same moment. These ran during
the quietest period (median load 2.0). Step time per tick in ms, **median / p95**, median over 3 repeats:

| Scenario | rapier | rapier-compound | box2d-rust | avian | avian-compound | box2d-c (ref) |
|---|---:|---:|---:|---:|---:|---:|
| ships (per cell) | 7.25 / 12.2 | 1.12 / 2.03 | 16.7 / 25.5 | 20.3 / 31.3 | **0.55 / 1.43** | 16.8 / 25.6 |
| ships-merged | 0.29 / 0.54 | **0.06 / 0.18** | 0.56 / 0.86 | 0.83 / 1.47 | **0.06 / 0.24** | 0.55 / 0.82 |
| debris | 1.50 / 4.22 | **1.26 / 3.92** | 2.69 / 5.13 | 4.40 / 8.31 | 2.06 / 4.95 | 2.38 / 3.48 |
| raycasts (step only) | 12.0 / 21.7 | **4.12 / 8.76** | 22.6 / 37.9 | 27.8 / 39.4 | 4.20 / 8.80 | 23.2 / 37.2 |
| breakage (per cell) | 8.20 / 16.0 | 1.20 / 2.98 | 15.5 / 24.8 | 19.3 / 32.4 | **0.49 / 1.45** | 15.6 / 26.5 |
| breakage-merged | 0.28 / 0.62 | **0.07 / 0.17** | 0.51 / 0.75 | 0.79 / 1.35 | **0.07 / 0.19** | 0.50 / 0.72 |
| asteroids | 0.06 / 0.11 | 0.06 / 0.11 | 0.06 / 0.11 | 0.19 / 0.30 | 0.11 / 0.19 | 0.06 / 0.10 |
| combined (per cell) | 17.3 / 40.9 | **4.70 / 12.6** | 22.7 / 39.4 | 31.4 / 61.7 | 5.32 / 13.1 | 23.6 / 53.3 |
| **combined-merged** | 3.79 / 8.58 | **2.79 / 6.60** | 5.01 / 8.30 | 7.27 / 13.4 | 3.44 / 7.11 | 3.92 / 6.90 |

The whole tick (split + step + 5000 rays) in lockstep, ms median / p95 / max:

| Scenario | rapier | rapier-compound | box2d-rust | avian | avian-compound | box2d-c |
|---|---:|---:|---:|---:|---:|---:|
| raycasts | 14.1 / 24.2 / 53.7 | 6.71 / 12.2 / 56.9 | 25.4 / 41.9 / 110 | 30.9 / 44.2 / 69.2 | 7.17 / 13.4 / 65.2 | 26.1 / 40.8 / 104 |
| combined | 21.1 / 49.6 / 153 | 9.08 / 20.9 / 56.7 | 26.8 / 50.3 / 199 | 36.3 / 78.2 / 327 | 10.5 / 23.5 / 124 | 28.4 / 66.0 / 260 |
| combined-merged | 6.91 / 12.4 / 43.3 | 6.14 / 11.3 / 42.8 | 8.59 / 12.7 / 28.1 | 10.8 / 17.7 / 46.6 | 7.14 / 11.8 / 20.7 | 8.02 / 12.3 / 32.7 |

**Raycasts**: 5000 closest-hit rays cost about the same everywhere: 2.1–3.0 ms per batch in the `raycasts` scene
and 3.0–3.8 ms in `combined-merged` (lockstep medians), i.e. 0.4–0.8 µs per ray. Rapier is slightly ahead and
Avian-compound slightly behind. Hit counts agree across engines to within 1.5 %.

**Breakage**: cost of one split event (rebuild the parent's colliders, create the piece bodies, set their
velocities), ms mean / max over the 30 events, lockstep:

| Scenario | rapier | rapier-compound | box2d-rust | avian | avian-compound | box2d-c |
|---|---:|---:|---:|---:|---:|---:|
| breakage (per cell) | 0.45 / 1.08 | 0.44 / 1.41 | 1.89 / 5.92 | 27.0 / 82.0 | 0.86 / 3.49 | 2.67 / 6.09 |
| breakage-merged | 0.03 / 0.10 | 0.04 / 0.14 | 0.05 / 0.15 | 0.47 / 1.14 | 0.14 / 0.58 | 0.06 / 0.17 |
| combined-merged | 0.10 / 0.17 | 0.11 / 0.36 | 0.22 / 0.69 | 1.51 / 3.77 | 0.86 / 5.42 | 0.27 / 0.79 |

**Setup** (creating all bodies and colliders): every variant takes at most ~45 ms for the 20 per-cell ships, except
the Avian entity variant, which takes 0.45–0.8 s for 22 600 collider entities.

**Separate processes** (one process per engine and repeat, the classic way): the same ranking, with much more
noise because each engine ran at a different moment. For example, Box2D C `combined` took 22 / 61 / 48 ms in
its three repeats. The tables are in `results/2026-10-11/summary.md`.

**Sleeping disabled** (lockstep, but run during a busy period, load 8–13): all engines got 2.1–2.8× slower
than in the sleep-on lockstep run, i.e. about uniformly, which is the load difference. Sleeping does not
change the ranking (nothing in these scenes sits still for long). Ships: avian-compound 1.52, rapier-compound
2.91, rapier 18.2, box2d-c 34.9 ms. combined-merged: rapier-compound 8.09, avian-compound 9.67, box2d-c 10.1,
rapier 10.6, box2d-rust 11.1, avian 19.3 ms (medians).

## Results: native, multi-thread (4 worker threads)

Separate processes, during the busiest period (median load 9.4 with 15 vCPUs, so 4 extra threads competed with
other agents' compilers). Step ms median / p95. Single-thread medians from the same kind of run (one process
per run) are in brackets.

| Scenario | rapier | rapier-compound | avian | avian-compound | box2d-c |
|---|---:|---:|---:|---:|---:|
| ships | 8.86 / 28.9 (8.85) | 1.11 / 4.01 (1.06) | 27.4 / 74.2 (28.6) | 0.60 / 3.94 (0.04) | 11.8 / 31.6 (26.3) |
| debris | 2.69 / 4.20 (1.21) | 2.33 / 6.41 (0.80) | 9.80 / 22.9 (5.63) | 5.63 / 19.2 (2.09) | 2.24 / 6.76 (3.20) |
| combined | 11.8 / 30.1 (22.6) | 1.98 / 6.51 (3.89) | 22.6 / 53.6 (43.6) | 4.06 / 14.5 (2.27) | 9.84 / 21.5 (48.1) |
| combined-merged | 2.49 / 13.2 (4.32) | 1.37 / 5.53 (2.64) | 6.56 / 13.6 (12.3) | 2.82 / 11.8 (4.11) | 1.84 / 5.44 (5.47) |

- Box2D C scales clearly with its own scheduler (1.4× on debris, 2–5× on the ship scenes). The Rust port is
  serial: Box2D's task system was not ported.
- Rapier `parallel` (rayon) helps on the big combined scenes (about 2×) and hurts on small ones. Its solver
  uses spin barriers, which suffer on a loaded VM.
- Avian `parallel` (Bevy task pools) is mixed: it helps the entity-per-collider variant on the big scenes
  (combined 43.6 → 22.6 ms) and mostly adds overhead to the compound variant.
- All of them keep their single-thread hash.
- On the web everything is single-threaded anyway (Bevy's web build), so this matters for desktop and later
  Android only. Measure again on a quiet machine or the phone before relying on these numbers.

## Results: wasm

**Node** (V8 = Chrome's engine), single thread, separate processes (median load 6.4), step ms median / p95:

| Scenario | rapier | rapier-compound | box2d-rust | avian | avian-compound |
|---|---:|---:|---:|---:|---:|
| ships | 8.11 / 19.4 | 0.99 / 2.39 | 14.0 / 27.6 | 21.8 / 33.9 | 0.07 / 0.23 |
| ships-merged | 0.36 / 0.60 | 0.07 / 0.11 | 0.64 / 0.84 | 1.16 / 1.98 | 0.04 / 0.10 |
| debris | 1.45 / 2.77 | 1.03 / 2.17 | 2.38 / 3.86 | 6.06 / 13.7 | 2.01 / 4.13 |
| raycasts | 10.5 / 33.0 | 2.76 / 7.47 | 33.0 / 80.6 | 42.6 / 74.6 | 3.06 / 10.9 |
| breakage | 8.14 / 19.4 | 1.11 / 2.63 | 12.5 / 22.2 | 24.9 / 44.1 | 0.13 / 0.37 |
| breakage-merged | 0.48 / 0.94 | 0.12 / 0.26 | 0.80 / 1.43 | 1.48 / 2.69 | 0.08 / 0.20 |
| asteroids | 0.10 / 0.20 | 0.10 / 0.20 | 0.09 / 0.17 | 0.40 / 0.82 | 0.23 / 0.45 |
| combined | 13.8 / 30.6 | 2.95 / 7.70 | 26.6 / 52.1 | 36.3 / 75.7 | 5.83 / 14.9 |
| combined-merged | 4.71 / 12.1 | 3.03 / 8.92 | 5.51 / 15.9 | 11.4 / 21.3 | 4.23 / 11.2 |

Several of these are *lower* than the separate-process native numbers. That is an artefact of the busier native
phase, not wasm being faster. Back-to-back pairs (native run, then the same Node run right after) give the real
ratio:

Step median, native / Node (ms), and the Node ÷ native ratio. Median over 3 repeats, each a native run and then
the same Node run right after (load 3.4–20, so absolute values are high; the ratios are what matter here). All
60 pairs had identical hashes.

| Scenario | rapier | rapier-compound | box2d-rust | avian | avian-compound |
|---|---:|---:|---:|---:|---:|
| ships-merged | 0.66 / 0.97 (×1.58) | 0.10 / 0.26 (×2.77) | 1.00 / 1.36 (×1.36) | 1.51 / 2.81 (×1.87) | 0.06 / 0.16 (×3.23) |
| debris | 2.10 / 3.42 (×1.63) | 1.43 / 2.37 (×1.94) | 4.08 / 5.47 (×1.34) | 8.81 / 15.0 (×1.59) | 3.29 / 7.82 (×1.65) |
| combined | 34.1 / 35.6 (×1.00) | 4.02 / 6.45 (×1.60) | 66.2 / 81.7 (×1.35) | 58.4 / 60.6 (×1.34) | 4.58 / 6.15 (×1.39) |
| combined-merged | 3.74 / 6.93 (×1.75) | 3.39 / 4.64 (×1.44) | 5.61 / 6.17 (×1.16) | 14.7 / 23.7 (×1.61) | 4.47 / 6.78 (×1.55) |

wasm in Node costs about 1.2–1.9× native on the realistic scenes (more for the sub-0.3 ms cases). box2d-rust
loses least, at ×1.2–1.4, probably because it has no SIMD-style code to lose. Rapier-compound stays the
fastest or near-fastest in wasm as well (combined-merged 4.6 ms in Node).

**Browser**: the page (`crates/bench/web/`) runs the same wasm builds in a Web Worker and shows step times and the
final hash next to the native and Node hashes. Headless Chromium (CPU only) ran it with full-length runs of 5 scenarios × 5 engines, and all
25 final hashes matched native and Node (`results/2026-10-11/browser-headless-hashes.txt`). Headless timings are not meaningful; Niklas can measure in his
host browser (command below).

**wasm size** (only in passing; the web build is a dev platform): after wasm-bindgen, without the name section,
no wasm-opt. box2d-rust 320 KB (126 KB gzip), Rapier 1.4 MB (559 KB gzip), Avian including the Bevy ECS core
8.2 MB (1.85 MB gzip). The game ships Bevy anyway.

## Determinism

The hash is FNV-1a over the raw f32 bits of position, rotation (cos, sin), linear and angular velocity of
every body, in a stable body order, every tick. The result is a running hash over all 930 ticks, plus
checkpoints. Ray results (hit body + hit distance bits) are hashed separately.

| Engine | Same hash on all repeats / processes | Native == Node wasm | Multi-thread == single | All-engines binary == per-engine binary | Browser (headless Chromium) |
|---|---|---|---|---|---|
| rapier, rapier-compound | yes, 9/9 scenarios | yes, 9/9 (incl. ray hashes) | yes, 9/9 (`parallel` + `enhanced-determinism`) | yes | yes, 5/5 full-length runs checked (ships-merged, debris, breakage-merged, asteroids, combined-merged) |
| box2d-rust | yes, 9/9 | yes, 9/9 (incl. ray hashes) | – (serial) | yes | yes, 5/5 full-length runs checked (ships-merged, debris, breakage-merged, asteroids, combined-merged) |
| avian, avian-compound | yes, 9/9 | yes, 9/9 (incl. ray hashes) | yes, 9/9 | yes | yes, 5/5 full-length runs checked (ships-merged, debris, breakage-merged, asteroids, combined-merged) |
| box2d-c (reference) | yes, 9/9 | – | yes, 9/9 (4 workers) | yes | – |

- **box2d-rust == Box2D C bit for bit** in all 9 scenarios, including the ray results. The port keeps Box2D's
  own deterministic `atan2`/`cos_sin` and avoids SIMD intrinsics and FMA.
- What each engine needs for cross-platform determinism (as configured here):
  - **Rapier**: the `enhanced-determinism` feature. It switches glam/simba to libm, uses deterministic hash
    maps and ordering, and can't be combined with `simd8`. `parallel` is fine.
  - **box2d-rust**: nothing; it is deterministic by construction.
  - **Avian**: the `enhanced-determinism` feature (libm in glam/bevy_math, parry's deterministic mode), which
    can't be combined with `simd`.
  - **All engines**: no FMA or fast-math flags, and the game's own code must keep the hard rules from
    `CLAUDE.md`.
- Not covered yet: aarch64 (Android). Everything is software math without FMA contraction, so it should hold,
  but it has to be measured with the phone.

## API notes per engine (what it took to implement the scenarios)

### Rapier2D 0.36.1 (`crates/bench/rapier/src/lib.rs`, ~200 lines)

- **Compound bodies:** two options, both measured. (a) One collider per cell attached to the body; every collider is its own leaf in the broad-phase BVH and can be removed individually. (b) One collider holding a `SharedShape::compound(Vec<(Pose, SharedShape)>)`; parry builds a BVH over the parts inside the shape, the broad phase sees one leaf per body. (b) is immutable: any change means building a new compound, which is cheap (a whole split event, rebuild included, averages ~0.4 ms natively with per-cell compounds and ~0.04 ms with merged rectangles).
- **Adding/removing colliders:** `colliders.insert_with_parent(builder, body, &mut bodies)` and `colliders.remove(handle, &mut islands, &mut bodies, &mut soft_bodies, wake_up)`. Insert updates mass properties immediately, removal only at the next step; call `recompute_mass_properties_from_colliders` if you need the new centre of mass right away. Rapier does **not** correct the linear velocity when the centre of mass moves; the adapter adds `w × Δcom` itself.
- **Splitting:** new `RigidBodyBuilder::dynamic().pose(parent_pose)`, attach the piece's colliders (same local coordinates as in the parent), read `center_of_mass()`, set `linvel = v + w × (com_piece − com_parent)`, `angvel = w`. No surprises.
- **Raycasts:** `broad_phase.as_query_pipeline(dispatcher, &bodies, &colliders, filter)` then `cast_ray(&ray, max_toi, solid)`; no separate query structure to keep up to date. `user_data: u128` on colliders/bodies maps hits back to game ids.
- **Determinism:** `enhanced-determinism` = libm math in glam/simba, deterministic hash maps and iteration order; documented to cover wasm. Can't be combined with `simd8`. `parallel` (rayon) works together with it.
- **Pain:** low. Explicit sets and handles, a `PhysicsWorld` convenience struct, plain Rust, no framework. 0.36 switched its math types to glam (`Vector = Vec2`, `Rotation = Rot2`, `Pose`); the prelude still re-exports nalgebra's `vector!`/`point!` macros, which produce the wrong types (pitfall).

### box2d-rust 1.4.0 (`crates/bench/box2d/src/lib.rs`, ~170 lines)

- A line-by-line port of the Box2D v3 C API: free functions taking `&mut World` (`create_body`, `create_polygon_shape`, `world_step(&mut world, dt, substeps)`), definition structs from `default_*_def()`. Familiar if you know Box2D v3. It says "v3.1" but ports C commit `56edae7` (3.2-dev; `get_version()` returns 3.2.0).
- **Compound bodies:** no compound shape type. Every polygon (max 8 vertices) is its own shape and broad-phase proxy on the body. One box per cell is therefore expensive; merged rectangles are the way to go.
- **Adding/removing shapes:** `create_polygon_shape(.., &shape_def, &polygon)` / `destroy_shape(world, id, update_body_mass)`. With the default `update_body_mass = true` every add recomputes the mass (O(n²) for a 2000-cell ship), so set it false and call `body_apply_mass_from_shapes` once. Box2D corrects the linear velocity for the centre-of-mass shift itself.
- **Splitting:** same pattern as Rapier.
- **Raycasts:** `world_cast_ray_closest(&mut world, origin, translation, filter)` returns the hit fraction and shape id; shape `user_data: u64` maps to game ids. Needs `&mut World` (recording hook), so queries can't run on a shared reference.
- **Determinism:** Box2D's own deterministic `atan2`/`cos_sin` approximations, no SIMD intrinsics (4-lane arrays the compiler auto-vectorises), no FMA. In this benchmark it is bit-identical to the Box2D C library in every scenario.
- **Threads:** serial by design; Box2D's task system was not ported (README: "the port is serial").
- **Pain:** low for a Box2D user, but C-style.

### Avian2D 0.7.0 on Bevy 0.19.1 (`crates/bench/avian/src/lib.rs`, ~280 lines)

- **ECS-driven:** bodies and colliders are entities with components; stepping means running a Bevy schedule. Headless setup that works: `App` + `TaskPoolPlugin` + `PhysicsPlugins::new(MySchedule)` with interpolation disabled, a manually advanced `Time` resource, `app.finish(); app.cleanup()` (or systems panic on missing resources), then `world.run_schedule(MySchedule)` per tick. Each of these steps fails in a non-obvious way when you get it wrong, and it took noticeably longer to get right than the other engines.
- **Compound bodies:** (a) one child entity per cell (`ChildOf(body)` + `Transform` offset), Avian's documented way, measured as `avian`; (b) `Collider::compound(...)` on the body entity, measured as `avian-compound`. (a) is very slow at this scale (tens of thousands of entities; a split that despawns and respawns ~1000 child entities costs ~27 ms).
- **Adding/removing colliders:** insert a new `Collider` component or despawn child entities; mass properties update at the next step's prepare stage, or immediately via `run_system_once(|mut h: MassPropertyHelper| h.update_mass_properties(e))` (only exact for colliders on the body entity). No velocity correction for the centre-of-mass shift.
- **Raycasts:** `SpatialQuery` system parameter; outside systems through a cached `SystemState<SpatialQuery>`. The hit entity is the collider entity. Child-collider poses are only updated at the start of the next step, so queries right after a step see them one step stale.
- **Determinism:** `enhanced-determinism` (libm in glam/bevy_math, parry's deterministic mode); can't be combined with `simd`.
- **Architecture:** Avian lives inside Bevy's ECS. The project's plan is an engine-free `sim` crate without Bevy (fixed 30 Hz, deterministic, also used by headless arena runs); with Avian the sim would have to become a Bevy app. Avian 0.7 needs Bevy 0.19, while the game is on Bevy 0.20 (released 8 Oct 2026); there is no Avian release for 0.20 yet.
- **Pain:** medium to high (see above), plus 8 MB of wasm (Bevy ECS is in it; the game ships Bevy anyway).

### Box2D C v3.2-dev via boxdd-sys 0.6.0 (reference only)

- Built from the vendored C sources with gcc (`-O2 -ffp-contract=off`, SSE2), pre-generated bindings, no libclang needed. Same commit (`56edae7`) that box2d-rust ports. Used through the raw FFI (`b2CreateBody`, `b2World_Step`, ...). Multithreading via Box2D's built-in scheduler (`workerCount`).

## Caveats

- **VM noise is large.** The same engine and run varied up to 3× between repeats in separate processes, from
  other agents' builds and from the host scheduling vCPUs across P-, E- and LP-E-cores. That is why lockstep
  is the primary table, why I report medians over 3 repeats, and why the load average is recorded with every
  run. Absolute numbers on a quiet desktop will be lower. Lead: `run-all.sh` repeats everything in a quiet
  period.
- **Lockstep vs warm cache.** In lockstep every engine steps right after the others, so its data is partly
  evicted from the caches. That is closer to a game frame (rendering and game logic run in between) than a
  tight benchmark loop. It mostly costs Avian, whose ECS data is spread out: avian-compound `ships` took
  0.04 ms in separate processes but 0.55 ms in lockstep. Rankings among the realistic variants are the same
  in both.
- **p95 and max** are dominated by load spikes in the VM more than by the engines (e.g. 100–300 ms max values).
- **Rapier per-cell compound cost**: Rapier's step with per-cell compounds (1.1 ms for `ships`) grows with
  the number of parts even when ships barely touch. From reading the code (not profiled): Rapier keeps a
  persistent contact manifold per pair of overlapping parts and rebuilds its solver contact graph for
  composite pairs every step. Avian computes composite contacts with a fresh workspace and drops empty
  manifolds. Merging rectangles inside the compound removes the gap (0.06 ms for both).
- **Fairness choices**:
  - The same substeps (4) everywhere; Avian's default of 6 would make it slower.
  - Engine defaults for sleeping, contact softness, continuous collision and contact recycling.
  - Avian's step includes its Position → Transform writeback, which it always does. The other engines need a
    similar sync step in a game.
  - Box2D C is compiled at `-O2` with SSE2 (no AVX2).
  - Splits rebuild the parent's whole collider set in every engine (no incremental removal), and velocities
    are handed over explicitly in every adapter.
- **Node vs browser**: Node and Chrome share V8, but a browser adds worker scheduling, a coarser timer
  (100 µs unless cross-origin isolated; the bench page is isolated, 5 µs) and power management. The browser
  page is there so Niklas can see real host-browser numbers.
- **Not measured**: the phone (deferred), wasm threads, joints, sensors, projectiles as bodies. Raycasts here
  are closest-hit only.

## How to reproduce / open the browser page

```sh
cd ~/workspace/space-game/crates/bench
./run-all.sh                 # everything (~3–4 h): builds, native per-engine, lockstep, sleep-off, multi-thread, Node, pairs
STAMP=quiet REPEATS=3 ./run-all.sh   # e.g. in a quiet period; results in results/quiet/
./build-wasm.sh && python3 web/serve.py 8090   # browser page at http://10.77.6.2:8090/
```
