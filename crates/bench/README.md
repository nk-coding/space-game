# Physics engine benchmark

Spike step 1: compares 2D physics engines on game-like scenarios so Niklas can pick one. Results and the
recommendation are in [`docs/results/physics-benchmark.md`](../../docs/results/physics-benchmark.md).

This is its **own cargo workspace** (`[workspace]` in `Cargo.toml`, excluded from the game workspace), with its
own `Cargo.lock` and `target/`. Box2D C is compiled here as a reference number only; it never goes near the game.

## Layout

| Path | What |
|---|---|
| `core/` | Engine-independent: seeded scenario generator, `PhysicsEngine` trait, timing harness, state hashing. No platform APIs (time comes in through a `Clock`), so it runs natively, in wasm and later on Android. |
| `rapier/` | Rapier2D 0.36.1 adapter (`enhanced-determinism`; `parallel` behind a feature). Engines `rapier` (one collider per shape) and `rapier-compound` (one `SharedShape::compound` per body). |
| `box2d/` | box2d-rust 1.4.0 adapter (pure Rust port of Box2D v3.1/3.2-dev; serial). Engine `box2d-rust`. |
| `avian/` | Avian2D 0.7.0 adapter on Bevy 0.19.1, headless (`enhanced-determinism`; `parallel` behind a feature). Engines `avian` (one child collider entity per shape) and `avian-compound` (one `Collider::compound` per body). |
| `boxdd/` | Box2D C via `boxdd-sys` 0.6.0 raw FFI (vendored C compiled with gcc, pre-generated bindings, no libclang). Engine `box2d-c`. Native only, reference only. |
| `native/` | Native runner `bench-native` (one engine per binary via cargo features). |
| `wasm/` | wasm-bindgen entry points; one wasm binary per engine (`--features rapier|box2d|avian`). |
| `node/run-node.mjs` | Runs the wasm builds under Node (V8). |
| `node/report.mjs` | Turns JSON-lines results into markdown tables, writes `web/reference-hashes.json`. |
| `web/` | Browser page (`index.html` + module worker) that runs the same wasm builds; `serve.py` serves it. |
| `results/<stamp>/` | Raw results of a full run (`env.txt`, `*.jsonl`, `*.log`, `summary.md`). |

## Scenarios

All share one seeded world generator (`core/src/scenario.rs`), zero gravity, fixed dt = 1/30 s, 4 substeps,
density 1, friction 0.5, restitution 0. Default run: 30 warm-up ticks + 900 measured ticks (30 s sim time).

| Scenario | Content |
|---|---|
| `ships` | 20 ships, 200–2048 cells each (22 592 shapes): one 1×1 m box per cell, slope triangles on convex corners. 10 pairs on collision courses (meet after 3–15 s), all spinning. |
| `ships-merged` | Same ships, greedy-merged rectangles + slope triangles (1 276 shapes). |
| `debris` | 2000 chunks (1–4 cell polyominoes, one box per cell) in 8 clouds, random drift and spin, colliding. |
| `raycasts` | `ships` + `debris` + 5000 closest-hit raycasts per tick (random origins, 200 m). Hits are counted and hashed. |
| `breakage` | `ships` + once per simulated second a ship is cut across one row; the largest part keeps the body (colliders rebuilt), the rest become new bodies with the parent's velocity at their own centre of mass. Split cost is timed separately. |
| `breakage-merged` | Same with merged rectangles (rebuild re-merges). |
| `asteroids` | 300 convex polygons (5–8 vertices, 3–14 m), a third static, the rest drifting. |
| `combined` | Everything at once (per-cell ships). |
| `combined-merged` | Everything at once with merged ships. |

Per tick the harness applies due breakage events (timed as *split*), steps (timed as *step*), casts rays (timed
as *rays*), then hashes every body (untimed): position, rotation (cos, sin), linear and angular velocity, raw
f32 bits, in stable body order. The running hash over all ticks is the determinism fingerprint.

## How to run

Rust via `source ~/.cargo/env`. Builds use `-j 6` (`JOBS=6`).

```sh
cd crates/bench

# Everything, ~3 h: builds all binaries + wasm, then
#   native-st        1 thread, one process per engine x scenario x repeat
#   lockstep         1 thread, all engines in one process ticking in turn (the robust comparison)
#   lockstep-nosleep same with sleeping disabled (NOSLEEP_SCENARIOS)
#   native-mt        engines' own multithreading, MT_THREADS worker threads
#   node             wasm under Node
#   pairs            native and Node runs back to back (native/wasm ratio; PAIR_SCENARIOS)
./run-all.sh
# Knobs: REPEATS=3 TICKS=900 WARMUP=30 MT_THREADS=4 ENGINES=rapier,box2d-rust SCENARIOS=ships,combined
#        MT_SCENARIOS / NOSLEEP_SCENARIOS / PAIR_SCENARIOS=...  SKIP_BUILD / SKIP_ST / SKIP_LOCKSTEP /
#        SKIP_NOSLEEP / SKIP_MT / SKIP_NODE / SKIP_PAIRS=1  STAMP=name
# Output: results/<stamp>/{env.txt,*.jsonl,*.log,summary.md}

# Single native runs (binary per engine: rapier, box2d, avian, boxdd):
cargo build --release -p bench-native --no-default-features --features rapier
target/release/bench-native --engines rapier-compound --scenarios ships,combined --repeats 1 --ticks 300
#   --threads 4 (needs --features rapier,parallel or avian,parallel; box2d-c threads without a feature)
#   --svg DIR   writes the final state of each run as SVG (visual sanity check)
#   --tick-hashes  stores every per-tick hash in the JSON (to find the first diverging tick)
#   --no-sleep  disables body sleeping in every engine
# All engines in one binary (default features) + lockstep:
cargo build --release -p bench-native && target/release/bench-native --lockstep --scenarios combined-merged

# wasm builds (web/pkg/<engine>/, also prints raw and gzip sizes):
./build-wasm.sh
node node/run-node.mjs --scenarios ships-merged --repeats 1 --ticks 300

# Tables from any result files:
node node/report.mjs results/<stamp>/*.jsonl
```

### Browser page

```sh
cd crates/bench
./build-wasm.sh            # once (or after code changes)
python3 web/serve.py 8090  # serves web/ on 0.0.0.0:8090 with COOP/COEP headers
```

Open `http://10.77.6.2:8090/` in the host browser, pick engines/scenarios, press **Run**. The page runs the
same scenarios in a Web Worker and shows step times and the final state hash, compared with the native and
Node hashes from the last `report.mjs` run (`web/reference-hashes.json`). "Copy results (JSON)" copies the raw
results. The plain per-cell `avian` engine is unticked by default (slow). URL parameters for scripted runs:
`?autorun=1&ticks=60&repeats=1&engines=rapier&scenarios=ships`.

## Fairness notes

- The VM is noisy (other agents' builds, host scheduling of vCPUs across P/E cores): the same run can vary 3×
  between processes. **Lockstep** mode (`--lockstep`) runs all engines of one binary in one process, one tick
  each in turn with the order rotated every tick, so every engine sees the same noise; it is the primary table
  in the results. Side effect: each engine starts its step with caches partly evicted by the others (closer to a
  game frame than a tight loop). Its hashes are identical to the per-engine binaries.

- Same scenario data, same dt, 4 substeps everywhere (Rapier: `num_solver_iterations` = substeps in its
  TGS-soft solver; Box2D: `subStepCount`; Avian: `SubstepCount`, whose default is 6). Sleeping and each engine's
  default continuous collision are left on (default in all three); nothing sleeps in practice (everything drifts).
- Each engine is built alone (one binary per engine, natively and in wasm), so cargo feature unification can't
  change one engine's code because of another.
- Avian's timed step is its whole physics schedule (prepare, collision, substeps, writeback incl. the
  Position→Transform sync it always does). Interpolation is disabled. The first steps (schedule build) fall in
  the warm-up.
- Splits rebuild the parent's whole collider set (not just the delta) in every engine and set velocities
  explicitly from the pre-split rigid motion.
