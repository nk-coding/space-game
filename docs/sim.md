# Simulation core (`crates/sim`)

Written 11 Oct 2026, updated the same evening (phase 3a: physics seam, flight computer, maneuvers). Status:
everything that does not depend on the physics engine is in place; the engine (still being chosen) plugs in
behind the [`PhysicsBackend`](#the-physics-seam) trait, and until then the engine-free `FreeFlight` backend
moves the ships. All numbers in the heat, power and flight models are **placeholders** for feel.

No Bevy, no graphics, no threads, no filesystem, no wall-clock. Builds for `wasm32-unknown-unknown` and
`wasm32-wasip1`. Dependencies: `data` (content) and `libm` (pinned `=0.2.16`, `arch` feature off).

## Module map

| Module | What it holds |
|---|---|
| `math` | `Vec2`, `Rot`, and the only allowed transcendental functions (`sin`, `cos`, `atan2`, `exp` via libm) |
| `rng` | `Pcg32` (reference PCG32), `splitmix64`, `Stream` (one per system), `RngStreams` |
| `hash` | `StateHasher` (FNV-1a 64 over 32-bit words), `StateHash` trait, `fnv1a64` |
| `ids` | `ShipId`, `PieceId` (monotonic `u32`, never reused), `IdAlloc` |
| `config` | `SimConfig` (thrust/torque scales, split ghost ticks, `FlightTuning`) |
| `design` | `ShipDesign::compile(content, blueprint)`: the blueprint compiled once (part specs, links, bonus links, gun/engine mounts and their blockers), shared by `Arc` |
| `mass` | `MassProps`, `polygon_mass`, `combine`, `to_piece` (blueprint → piece frame) |
| `collider` | `ConvexPolygon`, greedy rectangle merge + slope triangles |
| `ship` | `Ship`, `Piece`, `PartState`, `Body`, `ShipControls`, systems (`update_systems`), `Thruster`, `GunMount`, `Wrench`, `ThrustAuthority`, `inherited_velocity`, heat `tuning` constants |
| `alloc` | control allocation: `AllocTable`, `Actuation`, `TorqueSource`, `allocate`, `wrench_of` |
| `flight` | flight computer: `PilotInput`, `Target`, `FlightGoal`, `HeadingGoal`, `HelmMode`, `Kin`, `FlightTelemetry` |
| `maneuver` | `Maneuver` (Brake, Hold, FaceTarget, Approach, Orbit), `RaiderBrain` |
| `physics` | `PhysicsBackend` trait, `BodyKey`, `BodyDesc`, `RayHit`, `Contact`, the `FreeFlight` backend, `wrap_angle` |
| `breakage` | `Ship::apply_damage`, `Ship::resolve_breakage`, `DamageTarget`, `DamageOutcome`, chain-reaction hook |
| `events` | `SimEvent`, `SplitEvent`, `PieceSnapshot` |
| `world` | `Sim<P = FreeFlight>` (tick, config, RNG, ships, backend, events, state hash), `TickInputs`, `ShipCommand`, `DamageCommand`, `PartHit` |
| `scenario` | the deterministic `core` and `flight` scenarios |

Examples: `determinism` (prints per-tick hashes; used by `scripts/determinism-check.sh`), `sim_perf` (load
numbers).

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
   maneuver, goal, brain), scripted `damage`.
3. `resolve_breakage`: each damaged piece is processed once (connectivity, rebuild, splits) → events; bodies
   are updated, created and removed in the backend, and split pieces ghost each other for a few ticks.
4. **Flight**: every ship is planned from the same snapshot of the world (brains, maneuvers, pilot input →
   `FlightGoal`), then the flight computer allocates the actuators of each controlling piece.
5. **Systems** per ship: power → heat → wrench (engines, RCS, gimbals, gyros, cores) and gun output.
6. **Physics**: expired ghosts are cleared, wrenches applied (force rotated to world, at the centre of mass),
   the backend steps `DT`, bodies are read back (the previous pose is kept for interpolation), new contacts
   become `SimEvent::Impact`.
7. `tick += 1`; debug builds store `state_hash()` (`last_tick_hash()`).

## Determinism

The rules (also in `src/math.rs`, enforced where possible):

- Fixed step; no wall-clock anywhere. `Sim::time_seconds()` is display-only.
- **RNG**: in-crate PCG32 (checked against the reference vector of pcg-random.org), so a dependency update can
  never change the numbers. One stream per system (`Stream::Damage`, `Weapons`, `Debris`, `Ai`, `Scenario`),
  seeded with `splitmix64(world_seed ^ splitmix64(stream_id))` and using `stream_id` as the PCG sequence.
  Stream ids are part of the save/replay format: append only, never renumber. (The raider brain uses no
  randomness.)
- **Math**: `f32` everywhere. `+ − × ÷`, `sqrt`, `abs`, `floor`, `min/max`, comparisons and casts are exact
  IEEE operations. Transcendentals go through `sim::math` (libm, pure Rust). `crates/sim/clippy.toml` bans
  std's `sin/cos/tan/atan2/exp/ln/powf/powi/hypot/mul_add/...` in the crate (`cargo clippy -p sim` warns). Rust
  never fuses `a*b+c` by itself. `libm_values_are_pinned` pins exact bit patterns.
- **Containers**: `Vec` and `BTreeMap` only; every loop runs in index order. Ids are monotonic counters.
  The flight plan reads one snapshot for all ships, so ship order never matters.
- **State hash**: `Sim::state_hash()` = FNV-1a 64 fed with 32-bit words: tick, seed, config, RNG states, id
  counters, and per ship the controls, control piece, helm and brain state, every part's flags, hp, heat,
  piece, power share and work, and per piece its parts, flags, ghost timer, frame origin, mass properties,
  body, power balance, wrench, collider vertices, gun output and actuator commands. Floats by exact bits; NaN
  canonicalised (and debug-asserted never to occur). Computed every tick in debug builds, on demand in release.
  `PhysicsBackend::hash_state` can add engine-internal state.

How it is checked:

- `cargo test -p sim` (`tests/determinism.rs`): same seed → identical hash sequence (core 600 ticks, flight 900
  ticks); different seeds → different hashes and different runs; a cloned world continues identically; a 1e-6
  change in one part's hp changes the hash; incrementally maintained caches equal a full recompute.
- `scripts/determinism-check.sh [--seed N] [--ticks N] [--scenario core|flight]` builds the `determinism`
  example natively and for `wasm32-wasip1`, runs the wasm under Node's built-in WASI (`scripts/wasi-run.mjs`)
  and compares the per-tick hashes, printing the first divergent tick (exit 1). `--compare A.txt B.txt` compares
  any two hash streams (`<tick> <hash>` lines, `#` comments), e.g. a browser console dump.
- **Result (11 Oct 2026, with FreeFlight physics and the flight computer)**: native x86_64 vs wasm32 (Node 24 /
  V8) **identical** for core and flight, seed 42 (1800 ticks), flight seeds 1, 7, 123456789 and core seed 7
  (3000 ticks each). Dev and release builds also agree.

Scenarios (`sim::scenario`), both spawning two copies of every blueprint in the content:

- `core`: raw throttles and fire flags (every second ship flat out), random hits, a blast every 75 ticks, plus
  one heat-stress ship (three beams on two reactors, always firing, never hit) so overheating stays covered.
  Seed 42, 1800 ticks: 266 parts destroyed, 50 splits, 248 overheated part-ticks.
- `flight`: player ships flown by random pilot input (stick, lock on an enemy, drift, fire), automaton ships
  by the raider brain, fewer hits. Seed 42, 1800 ticks: 128 parts destroyed, 21 splits, 5651 firing ship-ticks.

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

- `PartState`: `hp`, `heat`, `alive`, `overheated`, `piece`, `open_sides`, `clear`, `power` (share of its
  demand served this tick), `work` (0..1, how hard it worked).
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

Per piece and tick: `supply` = output of alive, non-overheated reactors (with bonuses). Consumers ask for
`draw × activity` and are fed in **brown-out priority order**: logic core → shields → engines (engines, RCS
pods and gyros: everything that moves the ship, one tier) → weapons → other, ties by part index; at most one
consumer is partly fed. Per-ship order: `Ship::set_power_priority`. Activity: engines = throttle; RCS pods =
sum of their nozzles' throttles (capped at 1); gyros = |torque used| / max; guns = `fire` and a clear line of
fire; shields = `shields`; logic and other = 1. Output (thrust, torque, gun dps) and heat scale with
`work = activity × power`. A logic core's reaction torque is free (the core's own draw covers it).

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

`Piece::guns` (`GunMount`): muzzle (centre of the facing side), unit direction, `clear`, `fire_rate`,
`dps = damage × fire_rate`, and `output` (dps this tick). Projectiles and hits come with the physics engine;
`Sim::raycast` already maps a ray to the first part hit.

## Flight

### The flight computer (`flight.rs`)

Every tick, for the controlling piece of every active ship:

1. **Goal** (`FlightGoal { heading, velocity }`) from the helm: `HelmMode::Pilot(PilotInput)`,
   `HelmMode::Maneuver`, `HelmMode::Goal` (tools) or a brain. `HelmMode::Raw` skips the flight computer.
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
- **Lock** (`Target::Ship / Piece / Point`): the nose stays on the target; the stick strafes with whatever
  thrusters push that way (RCS); released = brake. A ship with engines only at the back cannot brake or strafe
  while locked.

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

### Maneuvers and the raider brain (`maneuver.rs`)

`Maneuver::Brake`, `Hold { at }`, `FaceTarget(target)`, `Approach { target, range }` (close in at the speed it
can still stop from, then face the target and keep pace), `Orbit { target, radius, ccw }` (tangential speed
limited by the centripetal acceleration the ship can hold, radial error corrected). Each becomes a
`FlightGoal` every tick; an unresolvable target brakes.

`RaiderBrain` (hard-coded until behavior trees): the nearest active ship of another faction is the target;
beyond 70 m it approaches to 30 m, then alternates attack (approach/face, 4 s) and orbit (28 m, 3 s, direction
by ship id); it fires when the target is within 55 m and 0.15 rad of the nose. No randomness.

## The physics seam

```rust
pub trait PhysicsBackend {
    fn create_body(&mut self, key: BodyKey, desc: &BodyDesc);   // pose, velocities, explicit mass, convex colliders
    fn update_body(&mut self, key: BodyKey, desc: &BodyDesc);   // new colliders + mass, new velocities, same pose
    fn remove_body(&mut self, key: BodyKey);
    fn set_ghost(&mut self, key: BodyKey, ghost: bool);         // ghost bodies of one ship ignore each other
    fn set_body(&mut self, key: BodyKey, body: Body);           // teleport
    fn apply_wrench(&mut self, key: BodyKey, force: Vec2, torque: f32); // world force at the CoM, cleared by step
    fn step(&mut self, dt: f32);
    fn body(&self, key: BodyKey) -> Option<Body>;
    fn raycast(&self, origin: Vec2, dir: Vec2, max: f32, exclude: Option<ShipId>) -> Option<RayHit>;
    fn contacts(&self) -> &[Contact];                           // started during the last step
    fn hash_state(&self, _h: &mut StateHasher) {}
}
```

`BodyKey { ship, piece }` orders by ship, then piece. `Sim<P>` owns the backend and makes every call itself, in a
fixed order: bodies on spawn and on breakage events, ghosts on splits (`split_ghost_ticks`), wrenches and the
step every tick, read-back afterwards. To add an engine:

1. Implement the trait (Rapier: one `RigidBody` per key with `set_additional_mass_properties`/explicit
   `MassProperties` and one convex collider per `ConvexPolygon`; ghosts via collision groups or a contact
   filter on the ship id; deterministic mode, single thread on the web).
2. `Sim::with_backend(seed, config, backend)`; the client uses `Sim<YourBackend>` instead of `Sim`.
3. Add its scenario to the `determinism` example and run `scripts/determinism-check.sh --scenario <name>`.
4. Ramming damage later: turn `SimEvent::Impact(Contact)` (impulse, closing speed) into damage via
   `Ship::part_at_local`.

`FreeFlight` (the stand-in): semi-implicit Euler about each body's centre of mass, no collisions (`contacts()`
is empty), ray casts against the convex colliders with bounding-circle culling.

## Damage and breakage

- `Sim::apply_damage(ship, DamageTarget::Part(i) | Cell((x, y)), amount)` → `Ignored`, `Damaged { hp_left }`
  or `Destroyed { excess }` (excess for penetration later). A destroyed part leaves the grid at once.
- `Sim::resolve_breakage()` (also step 3 of every tick) processes each damaged piece once: drop dead parts,
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

- Enemies: `world.command(raider, ShipCommand::Brain(Some(RaiderBrain::default())))`.
- Debug HUD: `ship.telemetry()` (top speed, goal, heading target), `piece.power`, `ship.parts()[i].heat`.
- Keep stable ids: visuals keyed by `(ShipId, PieceId)`; a split keeps the parent's `PieceId` for the kept part.

## Performance

`cargo run --release -p sim --example sim_perf`: 20 ships × 2000 parts (40 000 parts; each ship 100 thruster
rows of which 50 RCS nozzles, 62 torque sources, guns firing), native, VM load average ~11 (other agents
building Bevy; medians of 3 runs):

| What | Mean | p95 |
|---|---|---|
| `Sim::step`, raw throttles (systems + FreeFlight for 20 ships) | 0.81 ms | 1.86 ms |
| `Sim::step`, pilot input with lock (+ flight computer and allocation) | 0.88 ms | 1.93 ms |
| `allocate` alone (100 rows, 62 torque sources) | 1.5 µs | — |
| `state_hash` (whole world) | 0.63 ms | 1.27 ms |
| lose one part of a 2000-part piece (resolve + rebuild + body update) | 0.20 ms | 0.56 ms |
| cut a 50-part row and split a 2000-part piece | 0.18 ms | — |

The flight computer + allocation for 20 big ships costs well under 0.1 ms per tick (the step difference is
within the noise; 20 × 1.5 µs for allocation). The per-tick code allocates nothing once warmed up.

## Design questions for Niklas

Answered (provisionally, lead session 11 Oct 2026; easy to change, see the sections above): split-off cored
pieces are salvage; a core needs power; fixed brown-out order; chain reactions later via content; heat per
part. Turning: decided by Niklas (cores, gyros, RCS, gimbals). Halvex Runner layout: fixed in content.

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
4. **Raider brain numbers** (engage 70 m, stand-off 30 m, orbit 28 m, fire cone 0.15 rad) are placeholders;
   tune once weapons fire for real.
