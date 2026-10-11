# Simulation core (`crates/sim`)

Written 11 Oct 2026, updated the same evening (phase 3a: physics seam, flight computer, maneuvers; then
weapons v1 and shields). All numbers in the heat, power, flight, weapon and ramming models are **placeholders**
for feel.

Behavior system v1 (behaviors, Rhai conditions) added late on 11 Oct 2026: see [Behaviors](#behaviors).

**Rapier2D integrated on 12 Oct 2026** (phase 2b, ADR-0014): bodies, collisions, ramming v1, ghosting after
splits and ray casts run on Rapier; the engine-free `FreeFlight` stand-in and the generic `PhysicsBackend` trait
are gone (ADR-0025). See [Physics](#physics-rapier2d).

No Bevy, no graphics, no threads, no filesystem, no wall-clock. Builds for `wasm32-unknown-unknown` and
`wasm32-wasip1`. Dependencies: `data` (content), `libm` (pinned `=0.2.16`, `arch` feature off), `rhai`
(pinned `=1.26.1`, sandboxed; see [Rhai conditions](#rhai-conditions)) and `rapier2d` (pinned `=0.36.1`,
`enhanced-determinism`, default features off, single thread).

## Module map

| Module | What it holds |
|---|---|
| `math` | `Vec2`, `Rot`, and the only allowed transcendental functions (`sin`, `cos`, `atan2`, `exp` via libm) |
| `rng` | `Pcg32` (reference PCG32), `splitmix64`, `Stream` (one per system), `RngStreams` |
| `hash` | `StateHasher` (FNV-1a 64 over 32-bit words), `StateHash` trait, `fnv1a64` |
| `ids` | `ShipId`, `PieceId` (monotonic `u32`, never reused), `IdAlloc` |
| `config` | `SimConfig` (thrust/torque scales, split ghost ticks, `FlightTuning`, `PhysicsTuning`) |
| `design` | `ShipDesign::compile(content, blueprint)`: the blueprint compiled once (part specs, links, bonus links, gun/engine mounts and their blockers), shared by `Arc` |
| `mass` | `MassProps`, `polygon_mass`, `combine`, `to_piece` (blueprint → piece frame) |
| `collider` | `ConvexPolygon`, greedy rectangle merge + slope triangles |
| `ship` | `Ship`, `Piece`, `PartState`, `Body`, `ShipControls`, systems (`update_systems`), `Thruster`, `GunMount`, `Wrench`, `ThrustAuthority`, `inherited_velocity`, heat `tuning` constants |
| `power` | power allocation by tier and weight (`allocate`), `PowerSetting`, `Consumer` rows, `Ship::set_power_setting` |
| `alloc` | control allocation: `AllocTable`, `Actuation`, `TorqueSource`, `allocate`, `wrench_of` |
| `flight` | flight computer: `PilotInput`, `Target`, `FlightGoal`, `HeadingGoal`, `HelmMode`, `Kin`, `FlightTelemetry` |
| `maneuver` | `Maneuver` (Brake, Hold, FaceTarget, Approach, Orbit, KeepRange, Flee) |
| `behavior` | `BehaviorLibrary`, `BehaviorId`, `BehaviorState` (debug view), `RuleStatus`, `Why`, `BUILTIN_NODES`, `check_vocabulary`; `compile.rs` (program → flat code), `eval.rs` (think pass, fly), `script.rs` (Rhai host) |
| `weapons` | beams, pulses, projectiles (`Projectile`, `ProjectileId`), shields (`Bubble`), `BeamTrace`, `ShotKind`, `GoneReason`, weapon `tuning` |
| `physics` | `Physics` (Rapier2D: one body per piece, compound colliders, ghost filter, ray casts, contacts), `BodyKey`, `RayHit`, `Contact`, `wrap_angle` |
| `breakage` | `Ship::apply_damage`, `Ship::resolve_breakage`, `DamageTarget`, `DamageOutcome`, chain-reaction hook |
| `events` | `SimEvent`, `SplitEvent`, `PieceSnapshot` |
| `world` | `Sim` (tick, config, RNG, ships, physics, events, impacts and ramming, state hash), `TickInputs`, `ShipCommand`, `DamageCommand`, `PartHit`, `StepPhase` |
| `scenario` | the deterministic `core`, `flight`, `load` and `behavior` scenarios |

Examples: `determinism` (prints per-tick hashes; used by `scripts/determinism-check.sh`), `sim_perf` (load
numbers, Rapier vs the rest), `load_test` (also Rapier vs the rest), `behavior_perf` (behavior cost).

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
  loses parts without splitting; the frame itself never moves relative to the parts. Angles stay in (−π, π].
- Polygons are counter-clockwise in the piece frame.
- Units: mass in content "t", thrust in "kN", so kN / t = m/s². Torque kN·m, inertia t·m². Time step
  `DT = 1/30 s` (`TICK_HZ = 30`). Heat is a unitless level on the same scale as each part's `heat.limit`.

## The tick

`Sim::step(&TickInputs)`:

1. Events of the previous step are dropped (events from calls made since, such as `spawn_ship` or a direct
   `resolve_breakage`, are kept).
2. Inputs in list order: raw `controls` (switch the ship to `HelmMode::Raw`), `commands` (pilot input,
   maneuver, goal, behavior, leader), scripted `damage`.
3. `resolve_breakage`: each damaged piece is processed once (connectivity, rebuild, splits) → events; bodies
   are updated, created and removed in the physics, and split pieces ghost each other for a few ticks.
4. **Behaviors think**: every ship with a behavior whose turn it is (every 3 ticks, staggered) picks its target
   and rule from one snapshot of the world; the decisions are applied afterwards (see [Behaviors](#behaviors)).
5. **Flight**: every ship is planned from the same snapshot of the world (behaviors, maneuvers, pilot input →
   `FlightGoal` and what the guns do), then the flight computer allocates the actuators of each controlling
   piece and fire at will picks the guns that fire (per gun facing).
6. **Systems** per ship: power → heat → wrench (engines, RCS, gimbals, gyros, cores), gun output, shield
   regeneration.
7. **Weapons**: shield bubbles are collected, live projectiles fly (ray-cast segment), then every gun fires
   (beams and pulses hit at once, projectiles spawn, recoil adds to the wrench); breakage is resolved again.
8. **Physics**: expired ghosts are cleared, this tick's wrenches replace the last ones (force rotated to world,
   at the centre of mass), Rapier steps `DT` (4 substeps).
9. **Impacts**: contacts of the step become `SimEvent::Impact` (closing speed from the velocities before the
   step), bodies are read back (the previous pose is kept for interpolation), impacts above the ramming
   threshold damage the touching part on both sides, and breakage is resolved once more.
10. `tick += 1`; debug builds store `state_hash()` (`last_tick_hash()`).

`Sim::step_profiled(&inputs, |phase| ...)` is the same step with a callback at the start of every `StepPhase`
(`Inputs`, `Flight`, `Systems`, `Weapons`, `Physics`, `Impacts`, `Hash`, `Done`), so tools can time the phases
with their own clock (`load_test` and `sim_perf` report the Rapier step separately).

## Determinism

The rules (also in `src/math.rs`, enforced where possible):

- Fixed step; no wall-clock anywhere. `Sim::time_seconds()` is display-only.
- **RNG**: in-crate PCG32 (checked against the reference vector of pcg-random.org), so a dependency update can
  never change the numbers. One stream per system (`Stream::Damage`, `Weapons`, `Debris`, `Ai`, `Scenario`),
  seeded with `splitmix64(world_seed ^ splitmix64(stream_id))` and using `stream_id` as the PCG sequence.
  Stream ids are part of the save/replay format: append only, never renumber. (Behaviors use no
  randomness.)
- **Math**: `f32` everywhere. `+ − × ÷`, `sqrt`, `abs`, `floor`, `min/max`, comparisons and casts are exact
  IEEE operations. Transcendentals go through `sim::math` (libm, pure Rust). `crates/sim/clippy.toml` bans
  std's `sin/cos/tan/atan2/exp/ln/powf/powi/hypot/mul_add/...` in the crate (`cargo clippy -p sim` warns). Rust
  never fuses `a*b+c` by itself. `libm_values_are_pinned` pins exact bit patterns.
- **Containers**: `Vec` and `BTreeMap` only; every loop runs in index order. Ids are monotonic counters.
  The flight plan reads one snapshot for all ships, so ship order never matters.
- **State hash**: `Sim::state_hash()` = FNV-1a 64 fed with 32-bit words: tick, seed, config, RNG states, id
  counters, and per ship the controls (including the per-gun fire mask), control piece, helm and behavior state,
  every part's flags, hp, heat, piece, power share and work, and per piece its parts, flags, ghost timer, frame
  origin, mass properties, body, power balance, consumer power settings (part, tier, weight, in allocation
  order), wrench, collider vertices, gun output and actuator commands; then Rapier's own state per body in key
  order (translation, rotation as cos/sin, linear and angular velocity, sleeping and ghost flags). Floats by
  exact bits; NaN canonicalised (and debug-asserted never to occur). Computed every tick in debug builds, on
  demand in release.
- **Physics**: Rapier2D with `enhanced-determinism` (libm in glam/simba, deterministic hash maps and iteration
  order; never combined with `simd8`), single-threaded. The sim makes every Rapier call in a fixed order (ships
  by id, pieces by id); contacts are sorted by body key before the sim reads them. Angles are read back with
  `sim::math::atan2` and written with `Rot::from_angle` (libm), never std trig.

How it is checked:

- `cargo test -p sim` (`tests/determinism.rs`): same seed → identical hash sequence (core 600 ticks, flight 900
  ticks); different seeds → different hashes and different runs; a cloned world continues identically; a 1e-6
  change in one part's hp changes the hash; incrementally maintained caches equal a full recompute.
- `scripts/determinism-check.sh [--seed N] [--ticks N] [--scenario core|flight|load|behavior]` builds the `determinism`
  example natively and for `wasm32-wasip1`, runs the wasm under Node's built-in WASI (`scripts/wasi-run.mjs`)
  and compares the per-tick hashes, printing the first divergent tick (exit 1). `--compare A.txt B.txt` compares
  any two hash streams (`<tick> <hash>` lines, `#` comments), e.g. a browser console dump.
- **Result (11 Oct 2026, with FreeFlight physics and the flight computer)**: native x86_64 vs wasm32 (Node 24 /
  V8) **identical** for core and flight, seed 42 (1800 ticks), flight seeds 1, 7, 123456789 and core seed 7
  (3000 ticks each). Dev and release builds also agree. **With weapons v1** (same evening): core and flight
  seed 42 (1800 ticks) and flight seed 7 (3000 ticks) identical. **With leading and the load test**: core,
  flight and load (20 ships) seed 42, 1800 ticks, identical. **With behaviors and Rhai** (11 Oct, late):
  behavior (1548 Rhai evaluations), core, flight and load, seed 42, 1800 ticks each: identical. **With fire at
  will per gun and the raider brain removed** (12 Oct): the same four, plus flight seed 7 (3000 ticks):
  identical. **With power tiers and weights** (12 Oct): core, flight, load and behavior, seed 42, 1800 ticks:
  identical. (The hash streams differ from the previous version from tick 1 on, since power settings are hashed
  and idle consumers now show their tier's level; the scenario counters are unchanged except one more shot in
  `core`, 567.) **With Rapier2D** (12 Oct 2026, release builds, on top of power tiers): core, flight, load (20
  ships) and behavior, seeds 42 and 7, 1800 ticks each: native x86_64 vs wasm32 under Node 24 **identical**
  (8 runs, 4–64 impacts and up to 24 rams per run). (The load scenario rolls no random input: both seeds give
  the same fight, different hashes.)

Scenarios (`sim::scenario`); `core` and `flight` spawn two copies of every blueprint in the content:

- `core`: raw throttles and fire flags (every second ship flat out), random hits, a blast every 75 ticks, plus
  one heat-stress ship (three beams on two reactors, always firing, never hit) so overheating stays covered.
  Seed 42, 1800 ticks (Rapier): 266 parts destroyed, 43 splits, 248 overheated part-ticks, 8 impacts.
- `flight`: player ships flown by random pilot input (stick, lock on an enemy, drift, fire), automaton ships
  by their faction's behavior (`raider_basic`; the raider brain until 12 Oct), fewer scripted hits; all
  weapons fire for real. Seed 42, 1800 ticks (brain, 11 Oct): 136 parts destroyed, 26 splits, 1002 pulses and
  projectiles fired, 78 hull hits, 13 shield hits; with `raider_basic` and fire at will per gun (12 Oct): 139
  parts destroyed, 22 splits, 844 shots, 105 hull hits, 25 shield hits; with Rapier (12 Oct): 138 parts
  destroyed, 26 splits, 999 shots, 82 hull hits, 17 shield hits, 11 impacts (1 ram).

- `load` (`ScenarioKind::Load(n)`, `--ships N`, default 20): the shared [load test](#the-load-test).
- `behavior`: two of every blueprint, every ship under a behavior: raiders `raider_basic`, the first ship-mind
  ship piloted by random input (the leader), the others `skirmisher` (with the Rhai condition `closing_in`) and
  three player programs typed as text (`weakest(engines)`, `threat(weapons)`, an escort on `locked(power)`
  following the leader), plus hits and blasts that knock out cores (slots drop). Seed 42, 1800 ticks (12 Oct, fire at will
  per gun): 114 parts destroyed, 19 splits, 400 shots, 111 hull hits, 1548 Rhai evaluations; with Rapier:
  112 parts destroyed, 18 splits, 399 shots, 95 hull hits, 1520 Rhai evaluations, 23 impacts (5 rams).

For the browser: the game build (`wasm32-unknown-unknown`) can call `sim::scenario::run_kind(kind, seed, ticks)`
and log the same `<tick> <hash>` lines to the console; capture them with `tools/headless/shot.mjs` and run
`--compare` against the native stream.

## The ship model

### Design (static, once per blueprint)

`ShipDesign::compile` resolves every placement from content: mass and hit points (`placement_mass/hp`), the
part's polygon centroid and inertia, base stats (`power_output/draw`, `heat_output/limit`, `heat_vent`,
`damage`, `fire_rate`, `thrust`, `gimbal`, `rcs_thrust`, `torque`, `shield_radius`), its consumer kind, and
static topology:

- **links**: every pair of linked parts with the number of shared cell edges (heat conducts through each);
- **neighbours** per part (CSR lists);
- **bonus links**: for every part, the bonuses it can receive (source, effect, `max_stacks` group, venting
  condition), from the content `bonuses` blocks;
- **guns / engines**: slots, muzzle (centre of the facing side), thrust point (part centre), facing, and the
  parts that block the line of fire (to the grid edge) or the exhaust (row directly behind).

Key fact: whether two alive parts are linked depends only on those two parts, and destruction only removes
links. So "static lists filtered by alive" is always the current topology, and nothing static is recomputed.

### Runtime (per ship)

- `PartState`: `hp`, `heat`, `alive`, `overheated`, `piece`, `open_sides`, `clear`, `power` (level: share of
  its demand served this tick, see [Power](#power)), `work` (0..1, how hard it worked).
- `power_settings`: tier and weight per consumer part (the blueprint's at spawn; `Ship::set_power_setting`).
- `grid`: a clone of the design's `data::ShipGrid`; destroyed parts are removed. All pieces share it (pieces
  are never linked to each other, so cell queries stay correct after splits; line of fire, exhaust and RCS
  nozzles only count parts of the same piece).
- `pieces`: one per connected group = one rigid body. Each caches mass properties, colliders, cores, power
  lists, the thruster table (engines + RCS nozzles), torque sources, allocation table, authority and gun
  mounts, rebuilt only when it loses a part.
- Per-part caches (exposure, bonus-adjusted stats, heat constants) are updated only around a destroyed part
  (its linked neighbours, and their neighbours for bonuses). `Ship::verify_caches` checks this equals a full
  recompute (tested over 900 scenario ticks).

### Control, salvage and dark pieces (provisional rules, lead session 11 Oct 2026)

- The **controlling piece** (`Ship::control_piece()`, `Piece::controlled`) is the blueprint's main piece if it
  has a logic core. On a split it stays with the part that keeps the parent's id (the component with a core,
  then the most parts). It holds control only while it has a core: if it loses its last core, or is
  destroyed, the ship is dark for good (`SimEvent::ControlLost`).
- **Every other piece is salvage** (`Piece::salvage()`), even with a core: its systems stay off and the client
  drops its hull glow.
- **A logic core needs power**: the controlling piece is `active` only while a core is alive, not overheated
  and the piece has supply > 0. An unpowered (or core-overheated) piece goes dark (no thrust, shields, guns
  or flight computer) and wakes up when power returns. Supports "kill all power → capture".
- To change any of these, see `Ship::new` (initial control), `Ship::check_control` (`breakage.rs`) and the
  `active` line at the top of `Ship::update_systems`.

### Mass properties and colliders

- Each part's mass is spread uniformly over its polygon (rectangle or slope triangle); exact polygon integrals
  give centroid and inertia. A piece combines them in two passes (centre first, then parallel axis).
- Halvex Runner (current layout): 106.5 t, inertia 1176 t·m², centre of mass on the mirror axis, within 0.5% of
  brute-force sampling (test).
- Colliders: square cells of rectangular parts merged greedily (extend right, then down) into rectangles, slope
  triangles kept (each tagged with its part). Runner: one triangle per slope (10) plus far fewer rectangles than
  parts; a 50×40 slab without slopes: 1 shape. `Ship::part_at_local(piece, point)` maps a hit point to a part.

### Power

Tiers and weights with partial output (GDR-0014, decided by Niklas; implemented 12 Oct 2026). Code:
`src/power.rs`.

Per piece and tick:

- **Supply** = output of alive, non-overheated reactors (with bonuses); 0 for a dark piece.
- **Demand** of each consumer = `draw × activity`. Activity: engines = throttle; RCS pods = sum of their
  nozzles' throttles (capped at 1); gyros = |torque used| / max; guns = the trigger and the per-gun fire mask
  (`ShipControls::gun_fires`) and a clear line of fire, so a gun that holds draws nothing; shields =
  `shields`; logic and other = 1. Overheated parts ask for nothing.
- **Allocation:**
  1. Tiers are served in order: `Critical`, `High`, `Normal`, `Low`. `Off` never draws.
  2. A tier whose demand fits into what is left is served in full.
  3. The first tier that does not fit shares what is left **by weight**: every running part gets the same
     multiple `k × weight` of its own demand (capped at 1; parts that reach 1 leave the rest to the others).
     Equal weights = every part at the same level; weight 2 runs at twice the level of weight 1.
  4. A part whose level would fall below its **minimum level** switches off instead: one at a time, lowest
     weight first (ties: the higher part index first), and its share goes back to the others of the tier by
     weight. A part that reaches its minimum once others have switched off keeps running.
  5. If parts of that tier run at partial output, the tiers below get nothing. If the tier ends up fully served
     because the parts that did not fit switched off, what is left goes on to the next tier.
- **Partial output** scales what a part does, through `work = activity × level`: gun damage per second (beams)
  and fire rate (pulse and projectile cooldowns run at `work`), shield recharge (`regen × work`; the bubble is
  up while its emitter gets any power, so a shield below its minimum drops its bubble but keeps its charge),
  thrust of engines and RCS nozzles (`max_force × level`, also the plume `Piece::output`), gyro and core torque
  (`max × level`). Heat scales with `work` as before. A logic core's reaction torque is free (its own draw
  covers it).

**Settings.** Every consumer has a tier and a weight (`PowerSetting`); the minimum level belongs to the part type.

| Where | What |
|---|---|
| content part `power: (draw, tier, weight, min_level)` | defaults (tier `Normal`, weight 1, min level 0) |
| blueprint placement `power: (tier: High, weight: 2)` | the player's drydock override for this part (both optional) |
| `Ship::set_power_setting(part, PowerSetting { tier, weight })`, `ShipCommand::Power { part, setting }` | runtime change (UI, behaviors later); the command goes through `TickInputs`, so replays and co-op see it |

`set_power_setting` refuses parts that draw no power and weights outside `(0, 100]` (`data::MAX_POWER_WEIGHT`).
Settings survive piece rebuilds and splits (per-part list on the ship) and are part of the state hash.

Base content defaults (lead defaults, placeholders like the numbers; the old fixed brown-out order as data):
logic core `Critical`; shield emitter `High` with minimum 25% (below it the bubble cannot hold); engines, RCS
pods and gyros `Normal` (so thrust and turning brown out together); guns `Low`, the beam laser with minimum
50%.

**Exposed for the client:** `PartState::power` is the part's level, 0..1: 1 = full, in between = partial
output (draw a dimmed icon), 0 = off or no power. A consumer that asks for nothing this tick shows the level
its tier and weight are served at (1 in a fully served tier, 0 in a tier that gets nothing, `Off`, or on a dark
piece). `Piece::power` (`supply`, `demand` without `Off` parts, `served`), `Piece::consumers()` (rows in
allocation order with `tier`, `weight`, `min_level`), `Ship::power_setting(part)`.

**Cost.** Rows are cached per piece in allocation order (tier, weight descending, part index) and re-sorted only
when a setting changes or the piece is rebuilt. A tick is one pass over the rows plus at most two more over the
one shared tier; only `+ − × ÷` and comparisons in a fixed order; no allocation. Measured 12 Oct 2026 (native
release, load average 4.5–9, five interleaved runs each, medians): `update_systems` (power + heat + wrench) for
20 ships of ~1930 parts and ~300 consumers each, 419 → 416 µs with power to spare and 394 → 415 µs in a
brown-out (+5%, about 1 µs per 2000-part ship per tick; heat dominates). The 20-ship load test: median step
0.024–0.034 → 0.025–0.027 ms, the same within noise.

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
`ship::tuning`, all placeholders. With the current Halvex Runner layout nothing overheats under combat load;
flat out with the gyros working all the time the shield emitters overheat after about 15 s.

### Adjacency bonuses

Evaluated from content `bonuses` on linked neighbours: `Add` and `Percent` per stat
(`effective = (base + Σadd) × (1 + Σpercent/100)`), `max_stacks` per (granting part type, bonus entry),
`SourceVenting` = the source has an open side. Simulated stats: power output/draw, heat output/limit/vent,
damage, fire rate, thrust, gimbal, RCS thrust, torque, shield radius (others are reported in
`ShipDesign::warnings`).

### Actuators: engines, RCS, gimbals, gyros, cores

- **Engines** push towards their facing side, along the line through their centre, only while the exhaust is
  clear. `gimbal` (degrees) lets an engine swing its thrust: modelled as a lateral force up to
  `throttle × thrust × sin(gimbal)` at the engine (axial thrust unchanged), so it costs nothing extra but adds
  a little side force with the torque.
- **RCS pods**: one nozzle (thruster row) per straight side whose neighbouring cell is empty or belongs to
  another piece, at the side's centre, pushing away from that side, `rcs_thrust` each. Nozzles appear and
  vanish as neighbours are shot away or pieces split.
- **Gyros** and **logic / automaton cores** (`gyro` block): pure torque up to `torque` in either direction.
- `Piece::thrusters` (`Thruster`: slot (engines only), part, unit `dir`, `point`, `arm`, `torque_per_force`,
  `max_force`, `clear`, `gimbal`, `lat`, `torque_per_lateral`, `rcs`), `Piece::torque_sources`, and
  `Piece::authority` (thrust per direction, differential torque, pure torque) are the flight computer's view.
- `SimConfig::thrust_scale` / `torque_scale` multiply all thrust / pure torque (1 = content values) for feel
  tests.

### Guns

`Piece::guns` (`GunMount`): muzzle (centre of the facing side), unit direction, `clear`, `fire_rate`
(bonus multiplier), `dps = damage × fire_rate`, `output` (dps this tick), `family`, `delivery`, `range`,
`rate` (shots/s with the bonus), `shot_damage` (`damage / base rate`). Firing: see [Weapons](#weapons).

## Flight

### The flight computer (`flight.rs`)

Every tick, for the controlling piece of every active ship:

1. **Goal** (`FlightGoal { heading, velocity }`) from the helm: `HelmMode::Pilot(PilotInput)`,
   `HelmMode::Maneuver`, `HelmMode::Goal` (tools) or a behavior. `HelmMode::Raw` skips the flight computer.
2. **Velocity**: dead-beat `F = m · R⁻¹ · (v_des − v) / DT` (piece frame); the allocation clamps it to what the
   thrusters can do, so nothing overshoots. `velocity: None` = coast.
3. **Heading**: `HeadingGoal::Angle(nose angle)`, `Thrust(world dir)` (point the best thrust axis there),
   `Auto` (keep the heading if the ship can push along the needed acceleration with at least half its best
   axis, else turn the best axis that way: flip-and-burn for ships with engines only at the back) or `Keep`.
   Turn rate `ω_des = sign(e) · min(ω_max, √(2 · margin · α · |e|), gain · |e|)` with `α` = (pure torque +
   differential torque) / inertia, then `τ = I · (ω_des − ω) / DT`.
4. **Flip, then burn**: while the ship turns its thrust towards the needed direction (`Thrust`/`Auto`), the
   translation force fades in from 25° to 8° of heading error, so slow-turning ships do not chase their own
   velocity vector.
5. **Allocation** (`alloc.rs`), fixed passes, no iteration: per axis group the requested force, with the free
   choice of torque inside the group (blend of the least-torque and most-torque fills; a symmetric ship asked
   for pure translation gets exactly symmetric throttles); then pure torque (gyros, cores, shared evenly);
   then gimbals of firing engines; then **differential thrust**, which adds unrequested force, only for real
   turns (heading error > 3°), never for holding a heading.

Pilot input (concept: touch controls):

- **No lock**: the stick is "fly that way". `v_des = stick × v_top`; the thrust points along the needed change
  of velocity while it is large and blends back to the stick direction as it shrinks, so the nose settles on
  the stick.
- **Soft top speed**: `v_top = top_speed_k × √(a_best)` (a_best = best axis thrust / mass). For the heading, an
  overspeed of up to 10% along the stick is ignored (no flip for it); the velocity goal stays exact, so the
  ship never thrusts while too fast. Sluggish ships get a lower top speed and take longer to reach it.
- **Released stick**: brakes to a stop (`Auto` heading: flip-and-burn if needed), unless `drift` is set
  (coast, keep the heading).
- **Lock** (`Target::Ship / Piece / Point`): the nose stays on the target, or on its lead point (see
  `PilotInput.aim` under [Aiming](#aiming-and-leading)); the stick strafes with whatever thrusters push that
  way (RCS); released = brake. A ship with engines only at the back cannot brake or strafe while locked.

`Ship::telemetry()` shows the top speed, goal, heading target, requested force/torque and the expected wrench.

### Tuning (`SimConfig::flight`, placeholders)

| Field | Default | Meaning |
|---|---|---|
| `top_speed_k` | 12 | `v_top = k·√a` (m/s per √(m/s²)) |
| `max_turn_rate` | 3 rad/s | cap on the commanded turn rate |
| `turn_margin` | 0.7 | share of `α` planned for braking a turn (no overshoot) |
| `turn_gain` | 8 /s | linear zone near the target heading |
| `stop_margin` | 0.6 | share of the deceleration planned for stopping (maneuvers) |
| `position_gain` | 1.2 /s | linear zone of position → velocity |
| `stick_deadzone` | 0.05 | shorter sticks count as released |
| `keep_heading_fraction` | 0.5 | `Auto` keeps the heading if this share of the best axis pushes the right way |

`SimConfig::thrust_scale` and `torque_scale` (default 1) scale the content's thrust and torque for feel tests;
`split_ghost_ticks` (10) is the collision grace after a split.

### Turning and speed of the base ships (content as of 11 Oct 2026, scales 1)

| Ship | Mass | Inertia | Pure torque | 90° turn | Max α | Thrust → accel | Top speed |
|---|---|---|---|---|---|---|---|
| Automaton raider | 15.8 t | 17 t·m² | 50 kN·m (core) | **1.47 s** | **2.96 rad/s²** | 12 kN → 0.76 m/s² | 10.5 m/s |
| Halvex Runner | 106.5 t | 1176 t·m² | 660 kN·m (2 cores, 2 gyros) | **3.27 s** | **0.60 rad/s²** | 69 kN → 0.65 m/s² | 9.7 m/s |
| Sample early | 43 t | 214 t·m² | 60 kN·m | 3.87 s | 0.42 rad/s² | 24 kN → 0.56 m/s² | 9.0 m/s |
| Sample mid | 88 t | 1010 t·m² | 60 kN·m | 8.47 s | 0.09 rad/s² | 69 kN → 0.78 m/s² | 10.6 m/s |

(90° turn from rest to within 1°, holding position; overshoot under 0.4° for all four.)

### Maneuvers (`maneuver.rs`)

`Maneuver::Brake`, `Hold { at }`, `FaceTarget { target, aim }`, `Approach { target, range, aim }` (close in at
the speed it can still stop from, then face the target or its lead point and keep pace), `Orbit { target,
radius, ccw }` (tangential speed
limited by the centripetal acceleration the ship can hold, radial error corrected). Each becomes a
`FlightGoal` every tick; an unresolvable target brakes.

`KeepRange { target, range, aim }` (back off with `HeadingGoal::Auto` when closer than `range`, so forward-only
ships turn away; otherwise like `Approach`) and `Flee { from }` (straight away at top speed) were added for
behaviors. `Maneuver::goal_for(kin, target_state, tuning)` takes an already resolved target (behaviors fly
relative to an aim point on the target ship).

The hard-coded `RaiderBrain` was deleted on 12 Oct 2026 (Niklas; ADR-0025, no dead code): the content behavior
`raider_basic` flies the same fight (see [Behaviors](#behaviors)).

## Behaviors

Code: `src/behavior/` (`mod.rs` library and vocabulary, `compile.rs`, `eval.rs`, `script.rs`). Format:
`docs/behavior-format.md`; content kinds: `docs/content-model.md`; records: GDR-0034 to GDR-0037, ADR-0026 and
ADR-0027 (all Proposed).

The top layer: **behavior → maneuver → flight computer**. A behavior picks a target and one maneuver; it never
touches thrusters.

### Library and vocabulary

`BehaviorLibrary::new(&content)` builds the vocabulary from the `behavior_nodes` content and compiles every
content behavior (`BehaviorId(0..)` in load order). The vocabulary check runs both ways against
`BUILTIN_NODES` (id, role, value type, parameter types, choice options): a content node without an
implementation, or an implementation without a content node, is an error. Rhai nodes are compiled here
(`check_vocabulary(&content)` runs the same checks for tools). Programs compile to flat data (`compile::Code`:
a target selector, rules as ranges into one condition array, actions as enums); names, defaults and units are
resolved once, reach-relative distances (`60%`) stay symbolic and resolve at each think. `add_text(id, text)`
adds a player's program (errors by text line and column); the newest program under an id wins `id()`, older
handles keep working. `default_for(&design)` = the blueprint's or faction's behavior.

### Evaluation

- **Cadence**: a ship thinks when `(tick + ship_id) % 3 == 0` (`tuning::THINK_PERIOD`, 10 Hz, staggered so a
  fleet spreads its thinks) and on the first tick after it gets a behavior. Dark ships do not think.
- **Two phases** (order-free, like the flight plan): every due ship decides from the same snapshot (the state
  after this tick's input breakage) into a per-ship scratch slot; then all decisions are applied. Per-pass
  caches: each ship's centre (one sin/cos per ship per pass instead of per candidate) and hull share (lazy).
- **A think**: `Kin` of the controlling piece; reach = longest clear gun (60 m without guns). The target line
  picks the target among active enemy ships (other faction), keeping the current one unless another's cost is
  below 0.8 × its own; the aim point is the nearest working part of the chosen kind on the target's controlling
  piece (or its centre). Then the rules in order, within the working slots: conditions short-circuit left to
  right; the first rule whose conditions hold and whose action has what it needs wins.
- **Fly** (every tick, in `flight::plan`): the stored maneuver's goal relative to the aim point's current
  position and velocity (`aim_state`). No matching rule: `Maneuver::Brake`.
- **Fire at will, per gun** (Niklas, 12 Oct 2026; GDR-0034): the plan carries the aim point's state
  (`Fire::AtWill`); when the flight computer runs, `guns_at_will` checks every clear gun of the controlling
  piece on its own: it fires when the target is within 0.933 × **its** reach from its muzzle and the aim point
  (for projectile guns its own lead point, from muzzle position and velocity) is within 0.12 rad of **its**
  facing. The result is the per-slot mask `ShipControls::guns` (weapon power demand reads
  `ShipControls::gun_fires(slot)`, so a gun that holds draws no power and does not fire). Broadside and rear
  guns fire whenever the target crosses their line, e.g. while orbiting. One `atan2` per clear gun per tick;
  no allocation once warmed up. Pilot and helm fire set `fire` with an empty mask (all guns).
- **Slots**: `Ship::rule_slots()` = `rule_slots_of(control piece)`: the sum of `rule_slots` of the piece's
  working (alive, not overheated) logic cores, 0 while the piece is dark. Rules beyond are `NoSlot`.
- **Conditions** (built-in): `hull` (Σ hp on the controlling piece / intact hp), `shield` (charge / capacity of
  working emitters; no emitters → `NoValue`), `shield_down`, `heat` (hottest part / its limit), `power`
  (served / demand), `target_in_range(d)`, `target_hull`, `target_on_me` (its behavior target, or its pilot
  lock, is me), `under_attack`, `enemies_near(r)`, `cycle(p)` and `target_time` (integer ticks since the
  target was picked). Target conditions without a target fail with `NoTarget`, regardless of `not`.
- **Actions** → maneuvers: `approach(r)` → `Approach`, `orbit(r, dir)` → `Orbit` (`auto` = by ship id
  parity), `kite(r)` → `KeepRange`, `face` → `FaceTarget`, `flee` → `Flee`, `hold` → `Brake`. All aim
  with `AimMode::Auto` (lead for projectile ships).
- **Targets**: `nearest` (distance), `weakest` (hull share, ties by distance), `threat` ((d + 50) / firepower,
  firepower × 2 if it aims at me), `attacker` (nearest enemy aiming at me), `locked` (the lock of
  `Ship::leader()`: its pilot lock or behavior target).
- **`raider_basic`** (the automatons' default; it replaced the raider brain): with a cannon raider (75 m reach)
  it approaches to 50 m (67%), and within 90 m (120%) orbits at 45 m (60%) for the last 4 s of every 9 s;
  its cannon fires at will within 70 m (93.3%).

### API

```rust
let content = data::base_content();
let mut world = sim::Sim::new(seed);
world.set_behaviors(sim::BehaviorLibrary::new(&content)?);           // before commanding behaviors
let raider = world.spawn_ship(raider_design.clone(), body);
world.command(raider, ShipCommand::Behavior(world.behaviors().default_for(&raider_design))); // raider_basic
let mine = world.behaviors_mut().add_text("my_escort", "target locked\nalways -> face")?; // player program
world.command(escort, ShipCommand::Behavior(Some(mine)));
world.command(escort, ShipCommand::Leader(Some(player_ship)));     // `locked` follows the player's lock
// debug view / "test in range":
let st = world.ship(escort).unwrap().behavior().unwrap();
st.active_rule(); st.rules();          // [RuleStatus]: Active, Skipped, NoSlot, NoTarget, Failed { cond, why }
st.describe(i);                        // "`hull < 30%` is false (hull is 82%)"
st.target(); st.aim_part(); st.maneuver(); st.slots(); st.last_think(); st.script_error();
st.program().program.to_text();       // the source, e.g. for a scan report
```

- `ShipCommand::Behavior(Some(id))`: a behavior overrides the helm. `Behavior(None)` hands the ship back to the
  helm. Unknown ids are ignored. `set_behaviors` drops every ship's behavior (handles refer to the old library).
- `BehaviorState::inherit()` copies a running behavior (shared program, same target and timers, thinks on its
  next tick): the hook for the vessel split (GDR-0013).
- State hash: each ship's behavior state (id, program fingerprint, target, aim part, acquisition tick, active
  rule, maneuver, slots, last think) and its leader. Per-rule statuses are debug data and not hashed.

### Rhai conditions

A content node with a `script` is a flag condition run by `script::ScriptHost` (one raw `rhai::Engine`, shared
by `Arc`, one reusable `Scope` per script in the sim's scratch). Sandbox and determinism (ADR-0027): no
packages, no `print`/time/modules/`eval`, `**` disabled (Rhai would call the platform's `powf`), f32 floats,
maths only through `sim::math` (libm), strict variables, 2000 operations per run (`tuning::SCRIPT_MAX_OPS`) plus
small limits on depth, strings, arrays and maps. A script sees `me`, `target` (`ShipView`, getters only) and its
parameters; they are plain scope variables overwritten before every run (constants would be folded in by
Rhai's optimizer). Out of budget, errors and non-bool results count as false (`Why::Script(ScriptFault)`), and
`BehaviorState::script_error()` holds the message (built only on failure).

### Cost

`cargo run --release -p sim --example behavior_perf`: 20 ships (10 Halvex Runners vs 10 Sample Early), every
ship under a 10-rule behavior (built-in nodes, `target nearest(engines)`), against the same fight under the
2-rule `raider_basic`. The think pass is timed as 10 back-to-back passes over all 20 ships (a normal tick thinks
for a third of them). 12 Oct 2026, with fire at will per gun, VM load average ~10 (other agents building), so
the numbers are noisy; the run with the median mean of 5 runs × 900 ticks, ranges over two invocations:

| What | Min | Median | Mean | p95 |
|---|---|---|---|---|
| Think pass, all 20 ships at once (built-in nodes) | 5.0–5.3 µs | 8.4–18.2 µs | 12.8–15.9 µs | 24–26 µs |
| Think pass, all 20 ships at once (+ one Rhai condition per think) | 8.5–11.3 µs | 33–34 µs | 31–34 µs | 47–51 µs |
| `Sim::step` with the 10-rule behavior | 10–12 µs | 17–23 µs | 20–25 µs | 28–49 µs |
| `Sim::step` with the 10-rule behavior + the Rhai condition | 11–19 µs | 22–27 µs | 24–31 µs | 36–49 µs |
| `Sim::step` with `raider_basic` (2 rules) | 8–19 µs | 20 µs | 21–22 µs | 22–24 µs |

One think costs well under a microsecond per ship for ten rules (0.25–0.9 µs by min and median); with the
3-tick cadence behaviors add a few µs per tick for 20 ships, and ten rules are within the noise of two. (11 Oct,
same load: think pass min 10–13 µs, median 16–17 µs.) A Rhai condition adds about 0.5–1 µs per evaluation. The
think pass allocates nothing once warmed up (`tests/behavior_alloc.rs`); a Rhai evaluation makes one small
allocation inside Rhai. Fire at will costs one `atan2` per clear gun per tick.

## Weapons

All hit tests are ray casts through Rapier's query pipeline (`Physics::raycast`, see [Physics](#physics-rapier2d));
a shooter never hits any piece of its own ship. A gun fires while `output > 0`: firing commanded
(`PilotInput.fire`, `ShipControls.fire` and its per-gun mask, or fire at will under a behavior), clear line of
fire, powered, not overheated, piece active. Guns are fixed: the pilot aims by turning the ship; under a
behavior each gun fires when the target crosses its own facing.

| Delivery (content) | Behaviour |
|---|---|
| `Beam` | continuous hit-scan; each tick `dps × work × DT` to the first block in range; `Sim::beams()` lists this tick's beams (`from`, `to`, `hit`) |
| `Pulse(rate)` | a cooldown per gun (`rate × fire_rate` shots/s, slower when browned out); each pulse is a hit-scan dealing `damage / rate` to the first block |
| `Projectile(rate, speed, penetration, recoil)` | same cooldown; a `Projectile` entity at the muzzle with the muzzle's velocity (`v + ω × r`) plus `speed`, living `range / speed` s |

- **Lasers** (beam, pulse) burn the first block only.
- **Projectiles**: each tick the segment they travel is ray-cast. On a hull hit the block takes the damage; if
  it breaks, the excess carries on along the shot through the next blocks of the same piece (grid walk) until
  `penetration` blocks are used up, the damage runs out or the shot leaves the hull (it does not fly on to
  another ship). The automaton cannon has penetration 1; the tests' railgun mod pierces 3.
- **Ricochet**: a kinetic shot that hits a slope face more than 60° from the face normal glances off: it is
  mirrored on the face and keeps 60% speed and 50% damage; the slope takes nothing; at most 2 ricochets.
- **Recoil**: `recoil` kN·s per shot pushes the shooter back at the muzzle for one tick (force and torque added
  to the piece's wrench).
- **Shields**: every working emitter of an active piece projects a bubble of `shield_radius` around itself
  with `capacity`, regenerating `regen × work` per second. A shot whose path enters a bubble of another ship
  loses `damage × absorb[family]` (content: energy 1.0, kinetic 0.5, missile 1.0) up to what the bubble has
  left. Bubbles along the path are crossed in entry order, so overlapping bubbles stack (each takes its share
  of what reaches it). A shot that starts inside a bubble passes it. A bubble that runs empty collapses and
  stays down until it has recharged 30% of its capacity. A beam that a bubble fully absorbs ends at the bubble.
- Tuning constants (`weapons::tuning`, placeholders): ricochet cosine 0.5, speed 0.6, damage 0.5, max 2
  ricochets, shield restart 0.3.

Events (in `Sim::events()`, for visuals and sound):

| Event | When |
|---|---|
| `ShotFired { ship, piece, gun, kind, from, to, projectile }` | every pulse (`to` = where it ended) and every projectile (`to` = end of its range); not for beams |
| `Hit { by, ship, piece, part, point, normal, damage, absorbed, family }` | damage reached a part (after shields); beams: once per tick on target |
| `ProjectileSpawned { id, owner, pos, vel }`, `ProjectileGone { id, pos, reason }` | `reason` = `Hit`, `Absorbed` or `Expired` |
| `Ricochet { id, point, normal, vel }` | a kinetic shot glanced off a slope |
| `ShieldHit { by, ship, piece, part, point, absorbed, left, depleted }` | a shot entered a bubble (`point` = entry); `depleted` = it collapsed |

### Aiming and leading

- `flight::intercept_time(r, v, speed)` / `lead_point(shooter, shooter_vel, target, target_vel, speed)`:
  first-order intercept (constant target velocity) with the shot inheriting the shooter's velocity, as the sim
  fires them; only `sqrt`, no transcendentals. A target that cannot be caught is aimed at directly.
- `AimMode` (`PilotInput.aim`, `Maneuver::FaceTarget/Approach { aim }`): `Target` (nose on the target),
  `Lead` (nose on the lead point of the ship's strongest projectile gun), `Auto` (default: lead if the ship's
  strongest gun by damage per second fires projectiles, else the target). A ship that mixes lasers and cannons
  therefore aims for whichever does most of its damage; the other kind still fires when it happens to bear.
- Guns start with **staggered cooldowns** (spread over one firing period, from a hash of ship id and part
  index), so a fleet's guns never fire in lock-step. A gun that is not firing reloads to ready, so a fresh
  trigger fires at once (a player's alpha strike).

Queries each frame: `Sim::projectiles()` (`pos`, `prev_pos`, `vel`; draw at `prev_pos + (pos − prev_pos) ×
alpha`), `Sim::beams()` (this tick's beams), `Sim::bubbles()` (`centre`, `radius`, `left`, `capacity` of every
bubble that is up), and per emitter `ship.part(i).shield` and `.shield_down`. `Sim::spawn_projectile` creates a
projectile directly (tools, tests).

## Physics (Rapier2D)

Rapier2D `=0.36.1` (ADR-0014) with `enhanced-determinism`, default features off (no `block-solver`: the
benchmarked configuration), single-threaded (`parallel` off; see [Performance](#performance) for whether it
would pay on native). `physics.rs` wraps it in the concrete type `Physics`; `Sim` owns one and makes every call
itself, in a fixed order. **No Rapier type appears in the public API**: the client sees `Body`, `BodyKey`,
`RayHit`, `Contact`, `PhysicsTuning` and `Sim::physics()` (read-only: `body`, `is_ghost`, `is_sleeping`,
`mass_props`, `raycast`, `len`).

The engine-free `FreeFlight` stand-in and the generic `PhysicsBackend` trait were removed on 12 Oct 2026
(ADR-0025: the main repo holds only chosen code). `Sim` has no backend type parameter; switching engines later
means rewriting `physics.rs` (about 580 lines with docs, plus tests).

### Bodies

- One dynamic body per piece, keyed `BodyKey { ship, piece }` (orders by ship, then piece; the body's and the
  collider's user data hold the key).
- One collider per body holding a `SharedShape::compound` of the piece's colliders (`collider.rs`: greedy-merged
  rectangles as cuboids, slope triangles as triangles); sub-shape `k` is `piece.colliders[k]`.
- **The sim is the authority on mass**: colliders carry zero mass properties and the body gets the piece's
  `MassProps` (mass, centre of mass, inertia) as explicit additional mass properties. Rapier adds them to zero,
  so its centre of mass equals the sim's bit for bit (test `rapier_mass_properties_are_the_pieces`).
- **Changes**: a piece that loses parts gets a new compound in place (`set_shape`, same handles; measured at
  ~0.04 ms per split in the benchmark) and new mass properties; a split creates bodies for the new pieces. Rapier
  keeps the frame origin when the centre of mass moves and does not correct the velocity, so the sim hands over
  `v + ω × (com_new − com_old)` (`ship::inherited_velocity`) for the kept piece and every new one, as the
  benchmark adapter did.
- Read-back after each step: `Body.pos` = Rapier's translation (the frame origin), `Body.angle` =
  `atan2(sin, cos)` of Rapier's rotation (libm), `Body.linvel` = velocity of the centre of mass.
- Wrenches: each tick's force (world frame, at the centre of mass) and torque replace the last tick's (Rapier
  keeps user forces until reset, so they are reset first). A non-zero wrench wakes a sleeping body.

### Settings (`SimConfig::physics`: `PhysicsTuning`, placeholders, part of the state hash)

| Field | Default | Meaning |
|---|---|---|
| `substeps` | 4 | solver substeps per tick at `DT = 1/30 s` (as benchmarked) |
| `friction` | 0.5 | hull friction (as benchmarked) |
| `restitution` | 0.2 | hull bounce; 0 would make rammed ships stick together |
| `sleep_speed` | 0.01 m/s | a body sleeps after its fastest point stayed below this for `sleep_time`; ≤ 0 = never |
| `sleep_time` | 2 s | |
| `ccd_prediction` | 4 m | soft CCD reach per tick (speculative contacts up to ~120 m/s) |
| `impact_min_impulse` | 1 kN·s | weaker contacts are not reported (resting contacts) |
| `ram_threshold` | 20 kN·s | impulse per tick above which a contact deals ramming damage |
| `ram_damage` | 0.02 hp per kN·s | ramming damage per kN·s above the threshold, to each side |

Gravity is always zero. `Sim::set_config` applies a new `PhysicsTuning` to the world and to every body at once.

### Sleeping

Rapier's island sleeping stays on, with a far lower threshold than Rapier's default (0.05 m/s for 0.5 s): nothing
slows down in space, so only a body that really stopped (a parked, dead or idle ship, wreckage that came to rest)
sleeps, and a sleeping body costs nothing in the solver. A sleeping body wakes when a wrench is set on it
(thrust, gyros, recoil), when an awake body touches it (contacts wake the island), when it loses a part (new
mass properties), or when it is teleported. Shots alone do not wake it: weapons v1 transfers no momentum.
Drifting wreckage never sleeps. Test: `sleeping_bodies_wake_on_thrust_and_on_hits`.

### Continuous collision

Projectiles are sim-side ray casts (they cannot tunnel), so CCD is only about bodies. Every body has Rapier's
**soft CCD** (`ccd_prediction`, 4 m per tick): its broad-phase box grows along its motion and the narrow phase
creates speculative contacts from the relative velocity, so pieces closing at up to ~120 m/s stop at the hull
instead of passing through 1 m parts; it costs only where bodies come within one tick of each other. Full
("bullet") CCD is off: no body is small and fast. Rapier's automatic CCD against fixed bodies stays on (there
are no fixed bodies yet).

### Ghosting after splits

Every piece of a split (the kept one and the new ones) is a ghost for `split_ghost_ticks` (10 ticks): a flag
in its collider's user data plus Rapier's contact-pair filter hook, which drops a pair when both colliders are
ghosts of the same ship. It is a pure function of the two colliders (deterministic), and other pairs never
run the hook. Ghosts still collide with other ships. So freshly separated pieces never push each other apart;
afterwards pieces of one ship collide like any others. Tests: `split_pieces_ghost_each_other_then_collide`,
`ghosts_of_one_ship_pass_through_each_other`.

### Ray casts

`Physics::raycast(origin, dir, max, exclude)` → `RayHit { key, distance, point, local_point, normal }`, through
Rapier's query pipeline over the broad-phase BVH, with a predicate on the collider's user data that skips every
piece of one ship (`exclude`, the shooter). Rays are solid: one that starts inside a body hits it at distance 0.
Bodies created or moved between steps (splits at the start of a tick, teleports) go into the BVH at once, so
guns see them in the same tick. Equal distances resolve in the BVH's traversal order (deterministic).
`Sim::raycast` maps the hit to a part (`part_at_local` just inside the surface).

### Collisions, impacts and ramming (v1)

All bodies collide (ships and their pieces; pieces of one ship once the ghost time is over). After each step,
every touching pair whose normal impulse in that step (all contact points, all substeps) reached
`impact_min_impulse` becomes a `Contact`, sorted by `(a, b)` with `a < b`, and is reported as
`SimEvent::Impact(contact)`. Rapier counts only the velocity solve in the impulse, so pushing overlapping bodies
apart (penetration recovery) is no impact and deals no damage.

| `Contact` field | Meaning |
|---|---|
| `a`, `b` | the two bodies |
| `point` | world contact point on `a`'s surface: the impulse-weighted centre of all contact points |
| `normal` | world normal from `a` to `b` |
| `impulse` | normal impulse of the step (kN·s = t·m/s) |
| `relative_speed` | closing speed along the normal at the point, from the velocities before the step (m/s, > 0 = approaching) |
| `part_a`, `part_b` | the part under the point on each side |
| `damage` | ramming damage dealt to `part_a` and to `part_b` each (0 below the threshold) |

**Ramming damage** (concept: "ramming damage by mass and speed; ram prows are a build"):

```
damage = max(0, impulse − ram_threshold) × ram_damage        to part_a and to part_b, each
```

The impulse already carries mass and speed: head-on, `impulse ≈ (1 + e) · μ · v` with the reduced mass
`μ = m_a·m_b / (m_a + m_b)` and the closing speed `v`. Both sides take the same damage (the same impulse acts on
both hulls), so armour and hit points decide who breaks: a ram prow is heavy, tough armour at the nose. Damage
is applied right after the step and the breakage is resolved in the same tick (`PartDestroyed`, `Split` follow
the `Impact`). Shields do not stop rams. With the defaults:

| Collision | Impulse | Damage to each side |
|---|---|---|
| two raiders (15.8 t) head-on at 8 m/s each (measured, test) | 157 kN·s | 2.7 hp (both nose cannons, 1 hp, destroyed) |
| raider rammed by a Halvex Runner (106.5 t), 10 m/s | ~165 kN·s | ~2.9 hp |
| two runners head-on, 10 m/s closing | ~640 kN·s | ~12 hp |
| two raiders touching at 1 m/s (test) | < 20 kN·s | none (impact only) |
| a runner pushing another at full thrust (69 kN) | ~2.3 kN·s per tick | none |

The part under a contact: the contact point moved 5 cm into the hull (against the normal) and looked up with
`Ship::part_at_local`; if that misses, the centre of the strongest manifold, then the strongest manifold's
sub-shape (a slope triangle names its part; a merged rectangle is looked up at the point clamped into it). The
centre of all points is used because two flat faces touch at the ends of their overlap, which usually sit on a
neighbouring slope's corner.

v1 limits: one part per side, per pair and tick (a long scrape spreads over ticks, not over parts); no excess
damage carried into the next part; no content stat for rams yet (a `ram` bonus on prow parts could scale the
damage later).

## Damage and breakage

- `Sim::apply_damage(ship, DamageTarget::Part(i) | Cell((x, y)), amount)` → `Ignored`, `Damaged { hp_left }`
  or `Destroyed { excess }` (excess for penetration later). A destroyed part leaves the grid at once.
- Damage comes from scripted commands, weapons and rams ([Collisions](#collisions-impacts-and-ramming-v1)).
- `Sim::resolve_breakage()` (steps 3, 7 and 9 of every tick) processes each damaged piece once: drop dead parts,
  flood-fill the rest; one component → `SimEvent::PieceChanged`; several → `SimEvent::Split`; none →
  `SimEvent::PieceDestroyed`. Every destroyed part emits `SimEvent::PartDestroyed { volatile }`.
- On a split the main component keeps the piece id, frame and body. Every other component gets a new
  `PieceId`, a frame on its own centre of mass, the parent's angle and angular velocity, and
  `linvel = v + ω × (com_new − com_old)` (`ship::inherited_velocity`). The kept piece's `linvel` is corrected
  the same way. Tests check that no surviving part moves and that linear momentum is conserved.
- `Sim::raycast(origin, dir, max, exclude)` → `PartHit { ship, piece, part, hit }`.
- Chain reactions: `Ship::chain_reaction(part)` is the hook (does nothing yet); the plan is an `explosion`
  block in content.

## Driving the sim from the client

```rust
let content = data::base_content();
let runner = Arc::new(sim::ShipDesign::compile(&content, content.ships.get("halvex_runner").unwrap())?);
let mut world = sim::Sim::new(seed);                               // or Sim::with_config(seed, config)
let me = world.spawn_ship(runner, sim::Body::at(Vec2::ZERO, 0.0));
// every FixedUpdate (30 Hz):
let input = sim::PilotInput { stick, lock, drift, fire };          // stick in world terms, |stick| ≤ 1
world.step(&sim::TickInputs { commands: vec![(me, sim::ShipCommand::Pilot(input))], ..Default::default() });
for e in world.events() { /* ShipSpawned, Split (spawn visuals for `spawned`), PieceDestroyed, ControlLost, ... */ }
// every frame (60 fps), alpha = Time<Fixed>::overstep_fraction():
for ship in world.ships() { for piece in ship.pieces() {
    let (pos, angle) = piece.render_pose(alpha);                   // world, y up, CCW
    // parts: blueprint (x, y) → piece frame (x − origin.x, origin.y − y); glow if piece.active
    // plumes: piece.thrusters[i] with piece.output[i] (0..1; RCS nozzles have rcs = true)
}}
```

- Weapons: draw `world.beams()` as lines, `world.projectiles()` as tracers (interpolate `prev_pos → pos`),
  `world.bubbles()` as dotted rings (alpha from `left / capacity`); flash on `ShieldHit`, sparks on `Hit` and
  `Ricochet`, muzzle flashes on `ShotFired`.
- Collisions: `SimEvent::Impact(contact)` at `contact.point` (scale sparks and sound by `impulse`; `damage > 0`
  is a ram). Ships now collide, so spawn them apart.
- Enemies: `world.set_behaviors(BehaviorLibrary::new(&content)?)` once, then
  `world.command(raider, ShipCommand::Behavior(world.behaviors().default_for(&design)))` (see
  [Behaviors](#behaviors)).
- Debug HUD: `ship.telemetry()` (top speed, goal, heading target), `piece.power`, `ship.parts()[i].heat`.
- Power: `ship.parts()[i].power` (0..1) dims a part's icon at partial power; the drydock writes the player's
  tiers and weights into the blueprint placements; in flight `ShipCommand::Power { part, setting }` changes
  them (see [Power](#power)).
- Keep stable ids: visuals keyed by `(ShipId, PieceId)`; a split keeps the parent's `PieceId` for the kept part.

## Performance

`cargo run --release -p sim --example sim_perf`: 20 ships × 2000 parts (40 000 parts; each ship one collider
(a 50×40 slab merges into one rectangle), 100 thruster rows of which 50 RCS nozzles, 62 torque sources, guns
firing), native. Step times are split with `Sim::step_profiled` into the Rapier step and the rest (the sim's own
systems). 12 Oct 2026 with Rapier (on `dev` `fa08197`: fire at will per gun, before power tiers), VM load
average ~7, medians of 3 runs, each run giving mean / median / p95 in ms; the FreeFlight build of the same `dev`
commit, run alternately, for reference:

| What | Step mean / median / p95 | Rapier step mean / median / p95 | FreeFlight step mean / p95 |
|---|---|---|---|
| raw throttles, 20 ships | 0.58 / 0.47 / 0.89 | 0.027 / 0.013 / 0.046 | 0.56 / 0.76 |
| pilot input with lock (+ flight computer and allocation) | 0.64 / 0.50 / 0.79 | 0.024 / 0.015 / 0.045 | 0.64 / 1.22 |
| combat: 20 gunboats firing (below) | 1.18 / 0.79 / 3.4 | 0.023 / 0.013 / 0.062 | 1.04 / 3.0 |

| What | Time |
|---|---|
| `allocate` alone (100 rows, 62 torque sources) | 0.8–1.0 µs |
| `state_hash` (whole world, now with Rapier's body state) | 0.44 ms mean (FreeFlight build 0.42 ms) |
| lose one part of a 2000-part piece (resolve + rebuild + body update), median of 20 | 0.08 ms, of which Rapier ≤ 0.001 ms |
| cut a 50-part row and split a 2000-part piece, median of 20 | 0.09–0.11 ms, of which Rapier 0.003–0.012 ms |

The Rapier step for 20 big bodies is ~0.013 ms (median); the sim's own systems (power, heat, weapons, breakage)
dominate at 0.45–0.8 ms, as with FreeFlight. Rebuilding a piece's compound and creating split bodies is
negligible next to the sim's own breakage work (the split's Rapier part is within the benchmark's ~0.04 ms per
split with merged rectangles). An earlier run at load ~10 (before the rebase) gave about twice these medians and
p95 values up to 5× higher for both builds: the VM's noise.

**Combat** (same tool): 20 gunboats (50×40 slabs, 17 pulse lasers + 17 cannons each, shield emitters inside) in
two lines 60 m apart, every gun firing for 150 ticks; about 90 live projectiles on average, 340 at most. The slow
ticks are the first wave of hits, about 40 destroyed parts per tick, each rebuilding a 2000-part piece
(`PERF_DETAIL=1` lists them). Huge slab ships are the worst case for piece rebuilds; see
[The load test](#the-load-test) for realistic ships.

The flight computer + allocation for 20 big ships costs well under 0.1 ms per tick. The sim's per-tick code
allocates nothing once warmed up (Rapier's step manages its own buffers).

**Compared with the benchmark** (`docs/results/physics-benchmark.md`, Rapier compound with merged rectangles,
lockstep at load ~2): `ships-merged` (20 ships of 200–2048 cells, 1276 shapes, colliding pairs, all spinning)
stepped in 0.06 ms median / 0.18 ms p95. Here the Rapier step is 0.013–0.015 ms median for 20 ships, lower because
the scenes are lighter: 20 slabs of one rectangle each, or 20 small ships of 7–20 shapes with a few collisions;
p95 is 0.025–0.06 ms at load ~7. Even with one body the step costs ~2 µs (pipeline overhead). No big difference
to explain.

**Threads**: `parallel` stays off. The Rapier step of these scenes is 0.01–0.02 ms; in the benchmark rayon helped
only on the big combined scenes (several ms) and hurt small ones. Revisit when a real scene's Rapier step passes
~1 ms on native (desktop/Android only; the web build is single-threaded anyway).

## The load test

`sim::scenario::load_test(n, seed) -> Sim` (and `load_test_with(&content, n, seed)`): `n` ships on the base
content, ⌈n/2⌉ Halvex Runners in a line facing ⌊n/2⌋ automaton raiders 80 m away, every ship under the content
behavior `raider_basic` (the raider brain until 12 Oct), so they engage at once and produce projectiles, hits
and (eventually) debris. With `n = 1` the lone runner orbits a point and fires into space. The browser client
calls the same function; the final state hash of a run matches the `determinism --scenario load --ships n`
output, so native and browser runs can be compared.

`scripts/load-test-native.sh [--ships 1,5,20] [--ticks 1800] [--seed 42]` runs the `load_test` example and
prints a table, with the Rapier step alone next to the whole step. Native release, 1800 ticks, seed 42, 12 Oct
2026 with Rapier (on `dev` `fa08197`: `raider_basic`, fire at will per gun, before power tiers), VM load ~7,
medians of 3 runs (ms; the FreeFlight build of the same `dev` commit, run alternately, for reference):

| Ships | Step mean / median / p95 | Rapier step mean / median / p95 | FreeFlight mean / median / p95 | Impacts / rams | Shots | Hits | Splits | Active at end |
|---|---|---|---|---|---|---|---|---|
| 1 | 0.003 / 0.003 / 0.004 | 0.002 / 0.002 / 0.002 | 0.001 / 0.001 / 0.001 | 0 / 0 | 552 | 0 | 0 | 1 |
| 5 | 0.010 / 0.007 / 0.013 | 0.006 / 0.005 / 0.007 | 0.003 / 0.003 / 0.004 | 20 / 6 | 72 | 164 | 1 | 3 |
| 20 | 0.029 / 0.025 / 0.047 | 0.017 / 0.014 / 0.025 | 0.012 / 0.011 / 0.019 | 64 / 24 | 318 | 787 | 1 | 11 |

Max values were 0.1–1.8 ms (load spikes). With 20 fighting ships the Rapier step is ~0.014 ms (median) and the
sim's own systems ~0.011 ms (as with FreeFlight): Rapier a bit more than doubles the cost of a tick at these ship
sizes (a runner has 56 parts), which is still tiny. The fight itself changed: ships now collide (64 impacts, 24 of
them rams), so the counts differ from the FreeFlight run (327 shots, 966 hits, 2 splits, 10 active at the end;
final hash `d8cfba40e2ae85ec`).

## Design questions for Niklas

Answered (provisionally, lead session 11 Oct 2026; easy to change, see the sections above): split-off cored
pieces are salvage; a core needs power; chain reactions later via content; heat per part. The fixed brown-out
order was replaced by Niklas's tiers and weights (GDR-0014, implemented 12 Oct 2026). Turning: decided by Niklas (cores, gyros, RCS, gimbals). Halvex Runner layout: fixed in content.
Weapons (lead session, 11 Oct 2026, defaults Niklas may change): shields keep per-family shares with a collapse
until 30% recharged; a penetrating shot stops at the hull; raiders lead their shots (done, plus `AimMode` for
the player's lock). Fight length: Niklas tunes speed and ranges in the browser with live sliders.

Open:

1. **Linear speed feels slow.** The base ships accelerate at 0.56–0.78 m/s² and top out around 9–11 m/s (soft
   top speed `12·√a`). For fast, arcade fights: (a) **raise thrust in content** (or try `thrust_scale` 5–10 in
   the browser first and then bake the factor into the content) (recommended); (b) only raise `top_speed_k`
   (ships reach a higher top speed, slowly).
2. **Braking and strafing under lock.** A ship with engines only at the back cannot brake or strafe while its
   nose is locked on a target. Options: (a) **accept it; RCS pods are the answer** (a design choice in the
   drydock) (recommended); (b) let the flight computer break the lock briefly to flip and brake.
3. **Feel of "fly that way".** Forward-only ships point their thrust along the needed change of velocity, so
   a hard turn swings the nose past the stick before it settles (a "drift turn"). Alternative: keep the nose on
   the stick and only use the thrust that happens to help (slower, sideways drift remains). Recommend (a)
   **current behaviour**; judge in the browser.
4. **`raider_basic` numbers** (engage 90 m, stand-off 50 m, orbit 45 m, fire range 70 m, cone 0.12 rad; the
   raider brain's until 12 Oct) are placeholders. A stationary Halvex Runner that just locks and fires kills
   three raiders about 15 s after contact and takes 3–4 damage: the raiders are slow (0.76 m/s²) and the
   runner's pulse lasers reach 60 m. Tune together with question 1 in the browser.
5. **Missiles, disruption, drones** are not built yet (`absorb` already lists missiles: bubbles stop them).

Behaviors (asked 11 Oct 2026; **answered by Niklas on 12 Oct**, GDR-0034 to GDR-0037 accepted):

6. Fire at will with side and rear guns: **per gun facing** (done).
7. `RaiderBrain`: **deleted** after the client and the load test switched to behaviors.
8. One target line per behavior: **kept**.
9. No `or`: **confirmed** (two rules with the same action).
10. Unpiloted ship-mind vessels default to `skirmisher`: **confirmed**. Rule slots (6 per logic core, 2 per
    automaton core, lowest rules drop first): **confirmed**.

Physics (Rapier integration, 12 Oct 2026; defaults in [Collisions](#collisions-impacts-and-ramming-v1)):

11. **Ramming numbers.** v1 deals `(impulse − 20 kN·s) × 0.02` hp to the touching part of each side: a raider
    ramming a runner at 10 m/s takes out about 3 hp per side, a gentle bump nothing. (a) **Keep the formula and
    tune the two numbers in the browser** (recommended); (b) add a content stat (`ram`) so prow parts deal more
    and take less; (c) spread the damage over every touching part instead of one per side.
12. **Do shields stop rams?** v1: no, bubbles only stop shots. (a) **No** (recommended: ramming answers
    shield-heavy ships); (b) bubbles absorb part of a ram.
13. **Friendly collisions.** Every body collides with every other; three raiders spawned 20 m apart and
    converging on one target rammed each other (seen in the old brain-vs-behavior test). (a) **Keep, and add a
    separation term to the flight computer later** (recommended); (b) no collisions within a faction (one more
    case in the contact filter).
