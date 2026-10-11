# Simulation core (`crates/sim`)

Written 11 Oct 2026. Status: the engine-independent core of spike step 2 / roadmap phase 2 is done. The physics
layer (engine still being chosen) plugs in on top of it, see [How the physics layer plugs in](#how-the-physics-layer-plugs-in).
All numbers in the heat and power model are **placeholders** for feel.

No Bevy, no graphics, no threads, no filesystem, no wall-clock. Builds for `wasm32-unknown-unknown` and
`wasm32-wasip1`. Dependencies: `data` (content) and `libm` (pinned `=0.2.16`, `arch` feature off).

## Module map

| Module | What it holds |
|---|---|
| `math` | `Vec2`, `Rot`, and the only allowed transcendental functions (`sin`, `cos`, `atan2`, `exp` via libm) |
| `rng` | `Pcg32` (reference PCG32), `splitmix64`, `Stream` (one per system), `RngStreams` |
| `hash` | `StateHasher` (FNV-1a 64 over 32-bit words), `StateHash` trait, `fnv1a64` |
| `ids` | `ShipId`, `PieceId` (monotonic `u32`, never reused), `IdAlloc` |
| `design` | `ShipDesign::compile(content, blueprint)`: the blueprint compiled once (part specs, links, bonus links, gun/engine mounts and their blockers), shared by `Arc` |
| `mass` | `MassProps`, `polygon_mass`, `combine`, `to_piece` (blueprint → piece frame) |
| `collider` | `ConvexPolygon`, greedy rectangle merge + slope triangles |
| `ship` | `Ship`, `Piece`, `PartState`, `Body`, `ShipControls`, systems (`update_systems`), `Thruster`, `GunMount`, `Wrench`, `ThrustAuthority`, `inherited_velocity`, `thruster_wrench`, heat `tuning` constants |
| `breakage` | `Ship::apply_damage`, `Ship::resolve_breakage`, `DamageTarget`, `DamageOutcome`, chain-reaction hook |
| `events` | `SimEvent`, `SplitEvent`, `PieceSnapshot` |
| `world` | `Sim` (tick, seed, RNG, ships, events, state hash), `TickInputs`, `DamageCommand` |
| `scenario` | the deterministic test scenario and the free-flight stand-in |

Examples: `determinism` (prints per-tick hashes; used by `scripts/determinism-check.sh`), `sim_perf` (load numbers).

## Frames and units

| Frame | Axes | Origin |
|---|---|---|
| Blueprint (`data`) | x right, **y down**, 1 cell = 1 m, forward = −y | blueprint frame corner |
| Piece (one rigid body) | x right, **y up**, metres; forward = **+y** | `Piece::origin`, a fixed blueprint point: the piece's centre of mass when the piece was created |
| World | x right, y up, metres; angles counter-clockwise in radians | world origin |

- Blueprint `(x, y)` → piece `(x − ox, oy − y)` (`mass::to_piece`). World = `body.pos + R(body.angle) · local`.
- With `angle = 0` a ship's nose points to world +y. This matches the client's mapping `world = (x, −y)`.
- `Body.pos` is the **frame origin** in the world, `Body.linvel` the velocity of the **centre of mass**
  (the Rapier and Box2D convention). `MassProps.com` is `(0, 0)` when a piece is created and moves when it
  loses parts without splitting; the frame itself never moves relative to the parts.
- Polygons are counter-clockwise in the piece frame.
- Units: mass in content "t", thrust in "kN", so kN / t = m/s². Torque kN·m, inertia t·m². Time step
  `DT = 1/30 s` (`TICK_HZ = 30`). Heat is a unitless level on the same scale as each part's `heat.limit`.

## The tick

`Sim::step(&TickInputs)`:

1. Clear events. Apply inputs in list order: `controls` replace a ship's `ShipControls` (per-engine-slot
   throttle 0..1, `fire`, `shields`); `damage` commands are applied.
2. `resolve_breakage`: every damaged piece is processed once (connectivity, rebuild, splits) → events.
3. `update_systems` per ship: power → heat → engine wrench and gun output per piece.
4. *(Physics layer, next: apply wrenches, step the engine, write bodies back, turn contacts and ray hits into
   damage, resolve breakage again.)*
5. `tick += 1`; debug builds store `state_hash()` (`last_tick_hash()`).

Events (`Sim::events()`) live until the next step starts.

## Determinism

The rules (also in `src/math.rs`, enforced where possible):

- Fixed step; no wall-clock anywhere. `Sim::time_seconds()` is display-only.
- **RNG**: in-crate PCG32 (checked against the reference vector of pcg-random.org), so a dependency update can
  never change the numbers. One stream per system (`Stream::Damage`, `Weapons`, `Debris`, `Ai`, `Scenario`),
  seeded with `splitmix64(world_seed ^ splitmix64(stream_id))` and using `stream_id` as the PCG sequence.
  Stream ids are part of the save/replay format: append only, never renumber.
- **Math**: `f32` everywhere. `+ − × ÷`, `sqrt`, `abs`, `floor`, `min/max`, comparisons and casts are exact
  IEEE operations. Transcendentals go through `sim::math` (libm, pure Rust). `crates/sim/clippy.toml` bans
  std's `sin/cos/tan/atan2/exp/ln/powf/powi/hypot/mul_add/...` in the crate (`cargo clippy -p sim` warns). Rust
  never fuses `a*b+c` by itself. `libm_values_are_pinned` pins exact bit patterns.
- **Containers**: `Vec` and `BTreeMap` only; every loop runs in index order. Ids are monotonic counters.
- **State hash**: `Sim::state_hash()` = FNV-1a 64 fed with 32-bit words: tick, seed, RNG states, id counters,
  and per ship the controls, every part's alive/overheated flags, hp, heat, piece, power share and work, and
  per piece its parts, frame origin, mass properties, body, power balance, wrench, collider vertices and gun
  output. Floats by exact bits; NaN canonicalised (and debug-asserted never to occur). Computed every tick in
  debug builds, on demand in release.

How it is checked:

- `cargo test -p sim` (`tests/determinism.rs`): same seed → identical hash sequence over 600 ticks; different
  seeds → different hashes and different runs; a cloned world continues identically; a 1e-6 change in one
  part's hp changes the hash; incrementally maintained caches equal a full recompute.
- `scripts/determinism-check.sh [--seed N] [--ticks N] [--scenario NAME]` builds the `determinism` example
  natively and for `wasm32-wasip1`, runs the wasm under Node's built-in WASI (`scripts/wasi-run.mjs`), and
  compares the per-tick hashes, printing the first divergent tick (exit 1). `--compare A.txt B.txt` compares
  any two hash streams (`<tick> <hash>` lines, `#` comments), e.g. a browser console dump later.
- **Result (11 Oct 2026)**: native x86_64 vs wasm32 (Node 24 / V8): **identical** for seeds 42 (900 ticks),
  1, 7 and 123456789 (3000 ticks each). Debug and release builds also agree.

The scenario (`sim::scenario`) spawns two copies of every blueprint in the content (no hard-coded ids), every
second ship flat out (heat builds up, parts overheat and recover), controls and hits rolled on
`Stream::Scenario`, a blast on a part and its neighbours every 75 ticks. Seed 42, 900 ticks: 181 parts
destroyed, 32 splits, 39 pieces spawned, 474 overheated part-ticks. Bodies move with a free-flight stand-in
(`scenario::free_flight`: wrench → velocity → pose, no collisions), so the hash also covers libm rotations and
split kinematics.

For the browser: the game build (`wasm32-unknown-unknown`) can call `sim::scenario::run(seed, ticks)` and log
the same `<tick> <hash>` lines to the console; capture them with `tools/headless/shot.mjs` and run
`--compare` against the native stream.

## The ship model

### Design (static, once per blueprint)

`ShipDesign::compile` resolves every placement from content: mass and hit points (`placement_mass/hp`), the
part's polygon centroid and inertia, base stats (`power_output/draw`, `heat_output/limit`, `heat_vent`,
`damage`, `fire_rate`, `thrust`, `shield_radius`), its consumer kind, and static topology:

- **links**: every pair of linked parts with the number of shared cell edges (heat conducts through each);
- **neighbours** per part (CSR lists);
- **bonus links**: for every part, the bonuses it can receive (source, effect, `max_stacks` group, venting
  condition), from the content `bonuses` blocks;
- **guns / engines**: slots, muzzle (centre of the facing side), thrust point (part centre), facing, and the
  parts that block the line of fire (to the grid edge) or the exhaust (row directly behind).

Key fact: whether two alive parts are linked depends only on those two parts, and destruction only removes
links. So "static lists filtered by alive" is always the current topology, and nothing static is recomputed.

### Runtime (per ship)

- `PartState`: `hp`, `heat`, `alive`, `overheated`, `piece`, `open_sides`, `clear`, `power` (share of its
  demand served this tick), `work` (0..1, how hard it worked).
- `grid`: a clone of the design's `data::ShipGrid`; destroyed parts are removed. All pieces share it (pieces
  are never linked to each other, so cell queries stay correct after splits; line of fire and exhaust only
  count blockers of the same piece).
- `pieces`: one per connected group = one rigid body. Each caches mass properties, colliders, core flag,
  power lists, thruster table, thrust authority and gun mounts, rebuilt only when it loses a part.
- Per-part caches (exposure, bonus-adjusted stats, heat constants) are updated only around a destroyed part
  (its linked neighbours, and their neighbours for bonuses). `Ship::verify_caches` checks this equals a full
  recompute (tested over 900 scenario ticks).

### Mass properties and colliders

- Each part's mass is spread uniformly over its polygon (rectangle or slope triangle); exact polygon integrals
  give centroid and inertia. A piece combines them in two passes (centre first, then parallel axis).
- Halvex Runner: 103 t, inertia 1050 t·m², centre of mass at x = 6.0 (on the mirror axis, exact), within 0.5%
  of brute-force sampling (test).
- Colliders: square cells of rectangular parts merged greedily (extend right, then down) into rectangles,
  slope triangles kept (each tagged with its part). Runner: 23 convex shapes (12 triangles) for 56 parts; a
  50×40 slab without slopes: 1 shape. `Ship::part_at_local(piece, point)` maps a hit point back to a part.

### Power

Per piece and tick: `supply` = output of alive, non-overheated reactors (with bonuses). Consumers ask for
`draw × activity` (engines: throttle; guns: `fire` and a clear line of fire; shields: `shields`; logic and
other consumers: 1). Supply is handed out in **brown-out priority order**: logic core → shields → engines →
weapons → other, ties by part index; at most one consumer is partly fed. Per-ship order:
`Ship::set_power_priority`. Output (thrust, gun dps) and heat scale with `work = activity × power`.
Salvage pieces (no logic core) run nothing.

### Heat (dynamic, lumped per part)

Per tick, with capacity `C = area`:

```
E    = C·T + heat_output × work × DT                      (generation)
E   -= K × shared_edges × (T − T_neighbour) × DT          (conduction over links, explicit)
T'   = E / (C + loss × DT)                                (loss to space, implicit → unconditionally stable)
loss = 0.02 × area + open_sides × (radiator ? vent : 0.3)
```

Reactors make `heat × (1/3 + 2/3 × load)` (none when nothing draws). The steady state equals the drydock
model's (`loss·T = gen − Σ K (T − T_n)`, K = 1). A part above its `heat.limit` shuts down (no work, no draw,
no output) from the next tick until it cools below **75%** of the limit (hysteresis). Constants in
`ship::tuning`, all placeholders. Runner flat out (all guns, full throttle): the first part (a shield emitter
boxed in between two reactors) overheats after 8.3 s; the other base ships never overheat.

### Adjacency bonuses

Evaluated from content `bonuses` on linked neighbours: `Add` and `Percent` per stat
(`effective = (base + Σadd) × (1 + Σpercent/100)`), `max_stacks` per (granting part type, bonus entry),
`SourceVenting` = the source has an open side. Simulated stats: power output/draw, heat output/limit/vent,
damage, fire rate, thrust, shield radius (others are reported in `ShipDesign::warnings`). Examples in the base
content: capacitor → +15% fire rate per laser neighbour (max 3); venting radiator → +2 power per reactor
neighbour (sample_mid: 58 instead of 50).

### Engines and the thruster table

Each engine pushes towards its facing side (the exhaust leaves the opposite side), along the line through
its centre, only while its exhaust is clear. `Piece::thrusters` (`Thruster`: slot, part, unit `dir`, `point`,
`arm = point − com`, `torque_per_force`, `max_force` with bonuses, `clear`) is the flight computer's input;
`thruster_wrench(thrusters, throttle)` / `Piece::max_wrench(&throttles)` give net force (piece frame) and
torque about the centre of mass; `Piece::authority` sums what the clear thrusters can do per direction and
turning sense. Each tick `Piece::wrench` holds the actual force/torque (after power and heat) for the physics
layer. Runner at full throttle: (0, 69) kN, torque exactly 0.

### Guns

`Piece::guns` (`GunMount`): muzzle (centre of the facing side), unit direction, `clear`, `fire_rate`,
`dps = damage × fire_rate`, and `output` (dps this tick). Firing, projectiles and ray casts come with physics.

## Damage and breakage

- `Sim::apply_damage(ship, DamageTarget::Part(i) | Cell((x, y)), amount)` → `Ignored`, `Damaged { hp_left }`
  or `Destroyed { excess }` (excess for penetration later). A destroyed part leaves the grid at once.
- `Sim::resolve_breakage()` (also step 2 of every tick) processes each damaged piece once: drop dead parts,
  flood-fill the rest; one component → `SimEvent::PieceChanged`; several → `SimEvent::Split`; none →
  `SimEvent::PieceDestroyed`. Every destroyed part emits `SimEvent::PartDestroyed { volatile }`.
- On a split the **main component** (has a logic core, then more parts, then lowest part index) keeps the
  piece id, frame and body. Every other component gets a new `PieceId`, a frame on its own centre of mass,
  the parent's angle and angular velocity, and `linvel = v + ω × (com_new − com_old)`
  (`ship::inherited_velocity`). The kept piece's `linvel` is corrected the same way (its centre moved).
  Tests check that no surviving part moves in the world and that linear momentum of the remaining mass is
  conserved.
- `PieceSnapshot` carries everything for a body: id, parts, origin, offset in the parent frame, mass
  properties, colliders, body, `salvage`.
- Each piece keeps only its own reactors and power pool. A piece without a logic core is **salvage**
  (`Piece::salvage()`, `PieceSnapshot::salvage`): its systems stop and the client drops its hull glow
  (`Ship::piece_hull_edges(piece)` gives the outline of any piece).
- Chain reactions: `Ship::chain_reaction(part)` is called for every destroyed part during resolution and does
  nothing yet. `PartSpec::volatile` marks reactors for now.

## How the physics layer plugs in

What it receives, per piece (`(ShipId, PieceId)` is the body key):

1. **Create** a dynamic body for every piece after `spawn_ship` (and for `SplitEvent::spawned`): pose and
   velocities from `piece.body`, colliders = `piece.colliders` (convex, CCW, piece frame), and mass properties
   **set explicitly** from `piece.mass` (mass, local centre `com`, inertia) instead of letting the engine
   derive them from collider density, so the sim stays the authority.
2. **Every tick**, before the engine step: apply `piece.wrench` (force rotated to world by the body angle,
   at the centre of mass, plus torque).
3. **After the engine step**: write pose and velocities back into `piece.body` (`Ship::pieces_mut`). `pos` is
   the body origin, `linvel` the centre-of-mass velocity.
4. **Hits**: contact points and ray hits → piece frame → `Ship::part_at_local` → `Sim::apply_damage`.
   Then `Sim::resolve_breakage()` and consume the events: `PieceChanged` → replace colliders and mass
   properties, set `linvel`; `Split` → `kept` like `PieceChanged`, create a body per `spawned`;
   `PieceDestroyed` → remove the body.
5. Integrate this into `Sim::step` behind a small trait (create/update/remove body, apply wrench, step, read
   body, ray cast), so the engine stays swappable and the determinism check covers it: add a scenario to the
   `determinism` example and run `scripts/determinism-check.sh --scenario <name>`.

Notes for the physics layer:

- New pieces are created exactly where their parts were, so they touch the kept piece along the cut
  (shared edges, no overlap). To avoid contact "explosions" (spike question 3), ignore collisions between
  pieces of the same ship for a few ticks after a split, or rely on the engine's speculative contacts; measure.
- Use the engine's deterministic mode (Rapier `enhanced-determinism`), single-threaded on the web.

## Performance

`cargo run --release -p sim --example sim_perf` (20 ships × 2000 parts = 40 000 parts, guns firing, engines
at 80%), native, VM load average 8–10 (noisy; medians of 3 runs):

| What | Mean | p95 |
|---|---|---|
| `Sim::step` (power, heat, wrench for all 20 ships) | 0.52 ms | 0.85 ms |
| `state_hash` (whole world) | 0.48 ms | 0.73 ms |
| lose one part of a 2000-part piece (resolve + rebuild) | 0.16 ms | 0.48 ms |
| cut a 50-part row and split a 2000-part piece | 0.16 ms | — |

The per-tick systems allocate nothing. The base scenario (8 ships) runs 900 ticks in about 45 ms (dev profile).

## Design questions for Niklas

1. **Ships can barely turn.** With the placeholder numbers the runner accelerates at 0.67 m/s² and turns at
   0.04 rad/s² at most (a 90° turn takes ~9 s); the raider has one centred thruster and cannot turn at all.
   Arcade-fast fights need far more. Options: (a) **a gyro part** (content: torque, power draw, heat) plus a
   small built-in torque per logic core so every ship can turn (recommended); (b) side thrusters only (pure
   physics, but every design needs them); (c) scale all thrust by a global factor. Probably also (c) for
   linear acceleration once the flight computer exists.
2. **Split-off pieces that still have a logic core.** Now: any piece with a core stays active (systems run,
   it follows the ship's controls), pieces without one are salvage. Options: (a) **only the main piece stays
   active; every other piece becomes salvage and its cores go dark** (matches the concept board, where the
   glow follows the main piece, and the capture idea "dark core → copy yourself in") (recommended); (b) every
   cored piece becomes an independent ship of the same faction with its own AI; (c) keep the current rule.
3. **Does a core need power?** Now a core counts as long as it is alive. Option (a) **a piece without power
   goes dark** (core unpowered → systems stop, capturable), which matches "kill all power → dark logic core"
   (recommended); (b) cores work without power.
4. **Brown-out order.** Now fixed: logic → shields → engines → weapons → other. Options: (a) **this default,
   reorderable per ship later** (drydock setting or behavior-tree node) (recommended); (b) every consumer gets
   the same share (the drydock model's rule); (c) priorities in content per category.
5. **Chain reactions.** Which parts explode, how much, how far? Recommend (a) **an `explosion` block in
   content** (damage, radius) on reactors, capacitors and drives, rolled on `Stream::Damage`; (b) hard-code
   "reactors explode".
6. **Content check.** The Halvex Runner's four radiators are all boxed in (none vents, so no radiator bonus)
   and its wing-tip pulse lasers have a slope right in front (never fire). Intended, or should the layout
   change? The heat model runs its boxed-in shield emitter into shutdown after 8 s at full load.
7. **Heat model granularity.** Lumped per part (fast, matches the drydock steady state) vs per cell (hot spots
   inside a 2×2 reactor). Recommend (a) **per part** until a design needs more.
