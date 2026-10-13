# Simulation core (`crates/sim`)

Written 11 Oct 2026, updated the same evening (phase 3a: physics seam, flight computer, maneuvers; then
weapons v1 and shields). All numbers in the heat, power, flight, weapon and ramming models are **placeholders**
for feel.

Behavior system v1 (behaviors, Rhai conditions) added late on 11 Oct 2026: see [Behaviors](#behaviors).

**The vessel model on 12 Oct 2026** (GDR-0013, GDR-0038, GDR-0044, GDR-0045): owners, split-off vessels, focus,
control capacity and idle parts, mind capacity and dormant vessels, friendly and shielded rams. See
[Vessels](#vessels).

**Rapier2D integrated on 12 Oct 2026** (phase 2b, ADR-0014): bodies, collisions, ramming v1, ghosting after
splits and ray casts run on Rapier; the engine-free `FreeFlight` stand-in and the generic `PhysicsBackend` trait
are gone (ADR-0025). See [Physics](#physics-rapier2d).

**Rated speed and agility on 12 Oct 2026** (GDR-0041, GDR-0042): the top speed comes from each engine's rated
speed (no drag, a cruise flame at the cap) instead of the soft top speed, and the base ships are about four times
as agile (turning was slowed again on 13 Oct, below). See [Top speed and the cruise flame](#top-speed-and-the-cruise-flame).

**Slower, crisper turning on 13 Oct 2026** (GDR-0055, numbers in GDR-0058): a tiny core torque, moderate gyros, RCS
pods as the strong turners (couples), gimbals and differential thrust only as the last resort, A/D ramping up to
1.5 rad/s, and the top speed capped by the whole speed. See [The last resort](#the-last-resort),
[Keyboard rotation](#keyboard-rotation) and
[Turning and speed of the base ships](#turning-and-speed-of-the-base-ships-content-of-13-oct-2026-gdr-0055-scales-1).

**Engine plumes, no friendly fire and planning with working actuators on 12 Oct 2026** (GDR-0039, GDR-0046,
GDR-0048, GDR-0049): engines may sit anywhere and their exhaust burns the first part in its path (the clear-exhaust
rule is gone); shots pass through the shooter's own vessels; the flight computer plans only with actuators that
can produce. See [Engine plumes](#engine-plumes), [Weapons](#weapons) and
[Planning with what works](#planning-with-what-works).

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
| `config` | `SimConfig` (thrust/torque/speed scales, split ghost ticks, `FlightTuning`, `PhysicsTuning`, mind capacity per owner) |
| `control` | `ControlState`, the per-piece control capacity and idle pass (GDR-0044) |
| `design` | `ShipDesign::compile(content, blueprint)`: the blueprint compiled once (part specs, links, bonus links, gun mounts and their blockers, engine mounts with their plume lanes and paths), shared by `Arc` |
| `mass` | `MassProps`, `polygon_mass`, `combine`, `to_piece` (blueprint → piece frame) |
| `collider` | `ConvexPolygon`, greedy rectangle merge + slope triangles |
| `ship` | `Ship`, `Piece`, `PartState`, `Body`, `ShipControls`, systems (`update_systems`), `Thruster`, `GunMount`, `Wrench`, `ThrustAuthority` (what the actuators that can produce do now), `producing`, `rated_falloff`, `inherited_velocity`, heat and falloff `tuning` constants |
| `power` | power allocation by tier and weight (`allocate`), `PowerSetting`, `Consumer` rows, `Ship::set_power_setting` |
| `report` | the static design report for the drydock (`design_report`, `DesignReport`, `Issue`), see [Design report (drydock)](#design-report-drydock) |
| `alloc` | control allocation: `AllocTable`, `Actuation`, `TorqueSource`, `allocate` (first line), `couples` (RCS couples), `assist` (last resort), `wrench_of` |
| `flight` | flight computer: `PilotInput`, `Target`, `FlightGoal`, `HeadingGoal`, `HelmMode`, `Kin`, `FlightTelemetry`, `TurnAssist`, `nudge_step`, `turn_ramp_step` |
| `maneuver` | `Maneuver` (Brake, Hold, FaceTarget, Approach, Orbit, KeepRange, Flee) |
| `behavior` | `BehaviorLibrary`, `BehaviorId`, `BehaviorState` (debug view), `RuleStatus`, `Why`, `BUILTIN_NODES`, `check_vocabulary`; `compile.rs` (program → flat code), `eval.rs` (think pass, fly), `script.rs` (Rhai host) |
| `weapons` | beams, pulses, projectiles (`Projectile`, `ProjectileId`), shields (`Bubble`), no friendly fire, `BeamTrace`, `ShotKind`, `GoneReason`, weapon `tuning` |
| `physics` | `Physics` (Rapier2D: one body per piece, compound colliders, ghost filter, ray casts, contacts), `BodyKey`, `RayHit`, `Contact`, `wrap_angle` |
| `plume` | engine exhaust plumes (crate-private): lanes, the own-hull path, ray casts against other bodies, shields, `SimEvent::PlumeHit` |
| `breakage` | `Ship::apply_damage`, `Ship::resolve_breakage`, `DamageTarget`, `DamageOutcome`, chain-reaction hook |
| `events` | `SimEvent`, `SplitEvent`, `PieceSnapshot` |
| `vessel` | `Owner`, `friendly`, `VesselState`, `VesselInfo`; the vessel split, focus and mind pass (`Sim::focus`, `vessels`, `mind_load`) |
| `world` | `Sim` (tick, config, RNG, ships, physics, events, impacts and ramming, state hash), `TickInputs`, `ShipCommand`, `DamageCommand`, `PartHit`, `StepPhase` |
| `scenario` | the deterministic `core`, `flight`, `load` and `behavior` scenarios |

Examples: `determinism` (prints per-tick hashes; used by `scripts/determinism-check.sh`), `sim_perf` (load
numbers, Rapier vs the rest; `--report` times the design report, `--calibrate` measures its device-load model),
`load_test` (also Rapier vs the rest), `behavior_perf` (behavior cost).

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
   maneuver, goal, behavior, leader, power setting, focus, wake), scripted `damage`. No NaN or infinity from
   outside reaches the state (13 Oct 2026 review fix): a pilot input, maneuver or goal with one is ignored, a raw
   throttle counts it as 0, damage ignores it; so do `spawn_ship`, `teleport` and `spawn_projectile` (as 0),
   and `with_config`, `set_config` and `design_report` replace it with the default
   (`SimConfig::finite_or_default`). Test `non_finite_inputs_never_reach_the_state`.
3. `resolve_breakage`: each damaged piece is processed once (connectivity, rebuild, splits) → events; bodies
   are updated, created and removed in the physics, and split pieces ghost each other for a few ticks. Cored
   pieces of a split become vessels of their own, and the mind pass runs if anything changed (see
   [Vessels](#vessels)).
4. **Behaviors think**: every ship with a behavior whose turn it is (every 3 ticks, staggered) picks its target
   and rule from one snapshot of the world; the decisions are applied afterwards (see [Behaviors](#behaviors)).
5. **Flight**: every ship is planned from the same snapshot of the world (behaviors, maneuvers, pilot input →
   `FlightGoal` and what the guns do), then the flight computer allocates the actuators of each controlling
   piece and fire at will picks the guns that fire (per gun facing).
6. **Systems** per ship: per piece the control pass (which parts idle), power, shield regeneration, the wrench
   (engines, RCS, gimbals, gyros, cores) and gun output; then heat (ship-wide); then each piece's authority (what
   can push and turn, for the next plan).
7. **Weapons**: shield bubbles are collected, live projectiles fly (ray-cast segment), then every gun fires
   (beams and pulses hit at once, projectiles spawn, recoil adds to the wrench), then every firing engine's plume
   burns the first part in its path ([Engine plumes](#engine-plumes)); breakage is resolved again.
8. **Physics**: expired ghosts are cleared, this tick's wrenches replace the last ones (force rotated to world,
   at the centre of mass), Rapier steps `DT` (4 substeps).
9. **Impacts**: contacts of the step become `SimEvent::Impact` (closing speed from the velocities before the
   step), bodies are read back (the previous pose is kept for interpolation), impacts above the ramming
   threshold crush inward from the touching part on both sides (after shields; none within one owner), and
   breakage is resolved once more.
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
  counters (ships, pieces, projectiles), the minds (focus per owner, claim counter), and per ship the controls
  (including the per-gun fire mask), control piece, gun cooldowns, helm, behavior state, leader, owner, dormant
  flag, claim, formed tick and parked behavior, every part's flags (alive, overheated, idle), hp, heat, piece, power
  share, work and shield charge, and per piece its parts, flags, control load, ghost timer, frame
  origin, mass properties, body, power balance, consumer power settings (part, tier, weight, in allocation
  order), wrench, collider vertices, gun output, actuator commands (throttle, gimbal, RCS couple, torque) and
  the last-resort turning state (`Piece::assist`); then every projectile; then Rapier's own state per body in key
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
  the same fight, different hashes.) **With rated speed and the agile content** (GDR-0041/0042, 12 Oct 2026,
  release builds): core, flight, load (20 ships) and behavior with seed 42, flight and load with seed 7, 1800
  ticks each: native x86_64 vs wasm32 under Node **identical**. The falloff and the cruise level use only
  `+ − × ÷` and comparisons. **With engine plumes, no friendly fire and planning with working actuators**
  (12 Oct 2026, release builds, on top of the vessel model): core, flight, load (20 ships) and behavior, seeds 42
  and 7, 1800 ticks each: native x86_64 vs wasm32 under Node 24 **identical** (8 runs). The plume pass uses only
  `+ − × ÷`, `sqrt`, comparisons and Rapier ray casts, in ship, piece, row and lane order; projectiles hash the
  owner they were fired for. The hash streams differ from the version before from tick 1 on (the core scenario
  spawns the burner and rolls its inputs too), and the fights changed (no friendly fire, plumes). Counters
  with seed 42 (`burns` = `SimEvent::PlumeHit`). The slower turning of 13 Oct 2026 (GDR-0055) changes the hash
  streams and the fights again; the counters below are from 12 Oct, and native vs wasm has not been re-checked
  since:

| Scenario | Parts destroyed | Splits | Shots | Hits | Shield hits | Impacts / rams | Burns | Overheated part-ticks |
|---|---|---|---|---|---|---|---|---|
| core | 271 | 48 | 599 | 35 | 0 | 147 / 1 | 220 | 124 |
| flight | 133 | 21 | 919 | 47 | 13 | 172 / 1 | 8 | 0 |
| load (20) | 83 | 3 | 202 | 839 | 11 | 175 / 1 | 4 | 0 |
| behavior | 119 | 21 | 465 | 89 | 4 | 251 / 0 | 25 | 0 |

Seed 7: core 214 burns (the burner and the plume runs), flight 6, load 4, behavior 1. Most of the core
scenario's 147 impacts are pieces of one ship pushing against each other after splits (the random inputs changed
with the burner). The flight scenario's hull hits fell from 166 to 47, mostly because ship-mind ships no
longer hit each other.

Scenarios (`sim::scenario`); `core` and `flight` spawn two copies of every blueprint in the content:

- `core`: raw throttles and fire flags (every second ship flat out), random hits, a blast every 75 ticks, a ram
  run every 600 ticks (a ship-mind vessel and an automaton or wreck launched head-on at 20 m/s each, away from the
  rest), plus one heat-stress ship (three beams on two reactors, always firing, never hit) so overheating stays covered, and
  (since 12 Oct 2026) a burner (two thrusters in one column facing each other, their thrust cancelling, reactive
  blocks between them: the own-hull plume) and a plume run every 600 ticks (an automaton or a wreck put right
  behind a ship-mind vessel's engine at full throttle: a plume on another owner's hull).
  Seed 42, 1800 ticks (Rapier): 266 parts destroyed, 43 splits, 248 overheated part-ticks, 8 impacts; with rated
  speed (12 Oct): 266 parts destroyed, 50 splits, 248 overheated part-ticks, 781 shots, no impacts.
- `flight`: player ships flown by random pilot input (stick, lock on an enemy, drift, fire), automaton ships
  by their faction's behavior (`raider_basic`; the raider brain until 12 Oct), fewer scripted hits; all
  weapons fire for real. Seed 42, 1800 ticks (brain, 11 Oct): 136 parts destroyed, 26 splits, 1002 pulses and
  projectiles fired, 78 hull hits, 13 shield hits; with `raider_basic` and fire at will per gun (12 Oct): 139
  parts destroyed, 22 splits, 844 shots, 105 hull hits, 25 shield hits; with Rapier (12 Oct): 138 parts
  destroyed, 26 splits, 999 shots, 82 hull hits, 17 shield hits, 11 impacts (1 ram); with rated speed and the
  agile content (12 Oct): 139 parts destroyed, 23 splits, 897 shots, 108 hull hits, 28 shield hits, 83 impacts
  (15 rams).

- `load` (`ScenarioKind::Load(n)`, `--ships N`, default 20): the shared [load test](#the-load-test). Seed 42,
  1800 ticks, with rated speed and the agile content (12 Oct): 205 parts destroyed, 6 splits, 168 shots, 845
  hits, 113 impacts of which 72 rams (Rapier version: 64 impacts, 24 rams): the faster ships collide far more
  often and harder.
- `behavior`: two of every blueprint, every ship under a behavior: raiders `raider_basic`, the first ship-mind
  ship piloted by random input (the leader), the others `skirmisher` (with the Rhai condition `closing_in`) and
  three player programs typed as text (`weakest(engines)`, `threat(weapons)`, an escort on `locked(power)`
  following the leader), plus hits and blasts that knock out cores (slots drop). Seed 42, 1800 ticks (12 Oct, fire at will
  per gun): 114 parts destroyed, 19 splits, 400 shots, 111 hull hits, 1548 Rhai evaluations; with Rapier:
  112 parts destroyed, 18 splits, 399 shots, 95 hull hits, 1520 Rhai evaluations, 23 impacts (5 rams); with
  rated speed and the agile content (12 Oct): 122 parts destroyed, 21 splits, 284 shots, 110 hull hits, 1338
  Rhai evaluations, 74 impacts (11 rams). Since the vessel model (12 Oct) the leader is the player's focus and
  the player's mind capacity is 8 cores, so some ship-mind vessels start dormant; counters in
  [Vessels](#determinism-and-scenarios).

For the browser: the game build (`wasm32-unknown-unknown`) can call `sim::scenario::run_kind(kind, seed, ticks)`
and log the same `<tick> <hash>` lines to the console; capture them with `tools/headless/shot.mjs` and run
`--compare` against the native stream.

## The ship model

### Design (static, once per blueprint)

`ShipDesign::compile` resolves every placement from content: mass and hit points (`placement_mass/hp`), the
part's polygon centroid and inertia, base stats (`power_output/draw`, `heat_output/limit`, `heat_vent`,
`damage`, `fire_rate`, `thrust`, `gimbal`, `rcs_thrust`, `torque`, `shield_radius`, `rated_speed`,
`rcs_rated_speed`), its consumer kind, and
static topology:

- **links**: every pair of linked parts with the number of shared cell edges (heat conducts through each);
- **neighbours** per part (CSR lists);
- **bonus links**: for every part, the bonuses it can receive (source, effect, `max_stacks` group, venting
  condition), from the content `bonuses` blocks;
- **guns / engines**: slots, muzzle (centre of the facing side), thrust point (part centre), facing, the parts
  that block a gun's line of fire (to the grid edge), and per engine its **plume lanes** (one per cell of its back
  side, from the centre of that cell's back face) with the parts in each lane's path to the grid edge, in order,
  with the distance to each (`ShipDesign::plume_lanes`, `plume_path`).

Key fact: whether two alive parts are linked depends only on those two parts, and destruction only removes
links. So "static lists filtered by alive" is always the current topology, and nothing static is recomputed.

### Runtime (per ship)

- `PartState`: `hp`, `heat`, `alive`, `overheated`, `piece`, `open_sides`, `clear`, `power` (level: share of
  its demand served this tick, see [Power](#power)), `work` (0..1, how hard it worked), `shield` and `shield_down` (shield emitters: bubble charge;
  collapsed until it recharges), `idle` (over the control capacity, GDR-0044).
- `power_settings`: tier and weight per consumer part (the blueprint's at spawn; `Ship::set_power_setting`).
- `grid`: a clone of the design's `data::ShipGrid`; destroyed parts are removed. All pieces share it (pieces
  are never linked to each other, so cell queries stay correct after splits; line of fire, plume paths and RCS
  nozzles only count parts of the same piece).
- `pieces`: one per connected group = one rigid body. Each caches mass properties, colliders, its bounding
  radius, cores, power lists, the thruster table (engines + RCS nozzles), torque sources, allocation table, gun
  mounts and its plume reach, rebuilt only when it loses a part; its authority is refreshed every tick.
- Per-part caches (exposure, bonus-adjusted stats, heat constants) are updated only around a destroyed part
  (its linked neighbours, and their neighbours for bonuses). `Ship::verify_caches` checks this equals a full
  recompute (tested over 900 scenario ticks).

### Control, salvage and dark pieces

- The **controlling piece** (`Ship::control_piece()`, `Piece::controlled`) is the blueprint's main piece if it
  has a logic core: the ship's vessel. On a split it stays with the part that keeps the parent's id (the
  component with a core, then the most parts). It holds control only while it has a core: if it loses its last
  core, or is destroyed, the ship is salvage for good (`SimEvent::ControlLost`, `VesselLost`).
- **Every other cored piece of a split becomes a vessel of its own** (a new ship; GDR-0013, see
  [Vessels](#vessels)); the pieces left in a ship besides its controlling one have no core: salvage
  (`Piece::salvage()`), systems off, no hull glow. (Until 12 Oct 2026 every other piece was salvage, even with
  a core: the lead's provisional rule.)
- **A logic core needs power** (GDR-0014): the controlling piece is `active` only while its vessel is not
  dormant, a core is alive and not overheated, and the piece has supply > 0. An unpowered (or core-overheated)
  piece goes dark (no thrust, shields, guns or flight computer) and wakes up when power returns. Supports "kill
  all power → capture".
- See `Ship::new` (initial control), `Ship::check_control` (`breakage.rs`), the `active` line at the top of
  `Ship::update_systems` and `Sim::form_vessels` (`vessel.rs`).

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

- **Supply** = output of alive, non-overheated, non-idle reactors (with bonuses); 0 for a dark piece.
- **Demand** of each consumer = `draw × activity`. Activity: engines = throttle × rated-speed falloff; RCS pods =
  sum of their nozzles' throttle × falloff plus their couple share (capped at 1), so a thruster held at its
  rating idles; gyros =
  |torque used| / max; guns = the trigger and the per-gun fire mask
  (`ShipControls::gun_fires`) and a clear line of fire, so a gun that holds draws nothing; shields =
  `shields`; logic and other = 1. Overheated and idle parts ask for nothing.
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
  thrust of engines and RCS nozzles (`max_force × level × (throttle × falloff + couple)`, also the plume
  `Piece::output`), gyro and core torque (`max × level`). Heat scales with `work` as before. A logic core's
  reaction torque is free (its own draw covers it).

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
damage, fire rate, thrust, gimbal, RCS thrust, torque, shield radius, range, shield capacity/regen, rated speed,
RCS rated speed and plume length, damage and heat (others are reported in `ShipDesign::warnings`).

### Actuators: engines, RCS, gimbals, gyros, cores

- **Engines** push towards their facing side, along the line through their centre. They may sit anywhere
  (GDR-0039): a part behind one takes its plume ([Engine plumes](#engine-plumes)), and the engine keeps full
  thrust. `gimbal` (degrees) lets an engine swing its thrust: modelled as a lateral force up to
  `throttle × thrust × sin(gimbal)` at the engine (axial thrust unchanged), so it costs nothing extra but adds
  a little side force with the torque.
- **RCS pods**: one nozzle (thruster row) per straight side whose neighbouring cell is empty or belongs to
  another piece, at the side's centre, pushing away from that side, `rcs_thrust` each. Nozzles appear and
  vanish as neighbours are shot away or pieces split. Two nozzles of opposite sides whose torques turn the same
  way (the front nozzle of a pod right of the centre of mass and the back nozzle of one left of it, say) fire
  together as a **couple**: equal force, so they turn the ship without pushing it (GDR-0055). A pod far from
  the centre of mass with its front and back sides open is the strongest turner per part.
- **Gyros** and **logic / automaton cores** (`gyro` block): pure torque up to `torque` in either direction (a
  logic core's is tiny since GDR-0055: it keeps a ship controllable, nothing more).
- **Rated speed** (engines and RCS nozzles, GDR-0041, GDR-0055): a row pushing with the motion fades with the
  piece's whole speed, see [Top speed and the cruise flame](#top-speed-and-the-cruise-flame).
- `Piece::thrusters` (`Thruster`: slot (engines only), part, unit `dir`, `point`, `arm`, `torque_per_force`,
  `max_force`, `rated`, `plume_length`, `plume_damage`, `plume_heat` (engines only), `gimbal`, `lat`,
  `torque_per_lateral`, `rcs`), `Piece::torque_sources`, and `Piece::authority` (thrust per direction,
  differential torque, pure torque, RCS couple torque, top speed per direction, of the actuators that can
  produce now, see [Planning with what works](#planning-with-what-works)) are the flight computer's view.
- `SimConfig::thrust_scale` / `torque_scale` / `speed_scale` multiply all thrust / pure torque / rated speeds (1 =
  content values) for feel tests.

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
   thrusters can do at this tick's velocity (each row's rated-speed falloff), so nothing overshoots, not even at
   the top speed. `velocity: None` = coast.
3. **Heading**: `HeadingGoal::Angle(nose angle)`, `Thrust(world dir)` (point the best thrust axis there),
   `Auto` (keep the heading if the ship can push along the needed acceleration with at least half its best
   axis, else turn the best axis that way: flip-and-burn for ships with engines only at the back) or `Keep`.
   Turn rate `ω_des = sign(e) · min(ω_max, √(2 · margin · α · |e|), gain · |e|)` with `α` = (pure torque + the
   RCS couples that lasting translation leaves free, plus differential torque while the last resort is on) /
   inertia, then `τ = I · (ω_des − ω) / DT`. "Lasting" translation is a thrust group pushing flat out (W held,
   a full strafe): its nozzles cannot also turn the ship in a couple, so the turn is planned without them (the
   torque of translation's own fill that turns that way counts instead); a dead-beat correction that a group
   meets within the tick frees its nozzles again and is not counted (`alloc::lasting`, `flight::free_couples`;
   13 Oct 2026 review fix: a Runner with two pods, locked 90° to the left with W held, overshot the lock by 2.1°,
   now 0.45°). `Rate(r)` (keyboard) asks `ω_des = r`, but changes the turn rate only within `margin` × what the
   first line gives this tick in that sense, and never opens the last resort
   ([Keyboard rotation](#keyboard-rotation)).
4. **Flip, then burn**: while the ship turns its thrust towards the needed direction (`Thrust`/`Auto`), the
   translation force fades in from 25° to 8° of heading error, so slow-turning ships do not chase their own
   velocity vector.
5. **Allocation** (`alloc.rs`), fixed passes, no iteration, over the actuators that can produce this tick
   ([Planning with what works](#planning-with-what-works)). The **first line** (`alloc::allocate`) adds no
   force nobody asked for: per axis group the requested force, with the free choice of torque inside the group
   (blend of the least-torque and most-torque fills; a symmetric ship asked for pure translation gets exactly
   symmetric throttles); then pure torque (gyros, cores, shared evenly); then **RCS couples** (`alloc::couples`:
   per pair of opposite groups, forward/backward and right/left, equal force from the RCS rows whose torque
   turns the needed way, strongest levers first, all scaled alike when they could do more than needed). It
   returns the torque still missing. The **last resort** (`alloc::assist`): gimbals of firing engines, then
   **differential thrust**; both add unrequested force (see [The last resort](#the-last-resort)).

Pilot input (concept: touch controls):

- **No lock**: the stick is "fly that way". `v_des = stick × v_top`, with `v_top` the top speed of the best
  thrust axis (`Kin::top_speed`: the rating of its fastest thrusters), because the ship turns that axis onto
  the stick; the thrust points along the needed change of velocity while it is large and blends back to the
  stick direction as it shrinks, so the nose settles on the stick. For the heading, an overspeed of up to 10%
  along the stick (after a push) is ignored (no flip for it); the velocity goal stays exact.
- **Released stick**: brakes to a stop (`Auto` heading: flip-and-burn if needed), unless `drift` is set
  (coast, keep the heading).
- **Lock** (`Target::Ship / Piece / Point`): by default (`AimMode::Auto`, GDR-0040, `flight::smart_aim`) the ship
  turns its strongest working guns onto the target (the side with the most damage per second, front first on
  ties; their lead point if that side's strongest gun fires projectiles); `AimMode::Target` / `Lead` point the
  nose at the target or its lead point (see [Aiming](#aiming-and-leading)); A/D add the nudge (`aim_offset`); the
  stick strafes with whatever thrusters push that
  way (RCS), the full stick being the top speed reachable along the stick from this heading
  (`Kin::top_along`); released = brake. A ship with engines only at the back cannot brake or strafe while locked.
- **Keyboard** (GDR-0040): a full W, S, Q or E is the top speed of the thrusters pushing that way; W with Q or E
  asks for both (with the whole-speed fade the side push stops once the speed passes the side thrusters'
  rating). A/D: [Keyboard rotation](#keyboard-rotation).

`Ship::telemetry()` shows the top speed (best axis), goal, heading target, requested force/torque, the expected
wrench and whether the last resort was on (`assist`).

### The last resort

Decided by Niklas on 13 Oct 2026 ([GDR-0055](gdr/0055-slower-crisper-turning.md)): gimbals and differential thrust
(off-axis engines) push the ship sideways, so they turn it only after gyros, cores and RCS couples have been at
their limit for a few ticks. The numbers and details (6 ticks; differential thrust only while steering to a
heading, never for the keys, never to stop a spin) are lead defaults ([GDR-0058](gdr/0058-turning-v2-defaults.md)).
Code: `flight::TurnAssist`, `Piece::assist`, the end of `flight::execute`.

- **Gate** (`TurnAssist { ticks, on }`, per piece, hashed): every tick the first line reports the torque it could
  not give. `assist_after` (6) ticks in a row with torque missing turn the last resort **on**; it stays on while
  torque is still missing or the ship is still turning (heading error above 3°), so a turn planned with it can
  brake with it; then it is off and the count starts over. Pieces the flight computer does not fly (not
  controlling, dark, raw throttles, no goal) are reset, and so is a piece under keyboard rotation (a `Rate`
  goal): the keys close a gate that a lock, `Auto` or behavior turn left open, and never open it (13 Oct 2026
  review fix: a lock turn's open gate raised the key rate of the sample mid to about 1.15 rad/s with
  differential thrust).
- **What it adds**: gimbals of firing engines whenever it is on (they also hold a heading against uneven thrust),
  and differential thrust only while the flight computer steers to a heading: never to stop a spin without one
  (`Keep`, a settled `Auto`), which kept a released Runner from pushing itself into a flip-and-burn loop in
  testing.
- **Planning**: the turn plan counts differential thrust only while the gate is on (planning with what works): a
  turn starts on the first line's `α` (`Kin::alpha`), and once the gate opens the plan speeds up with the extra
  torque and keeps it to the end of the turn. The design report's turn estimate runs the same gate in one
  dimension.
- In practice: the Halvex Runner's gyros saturate in the first 0.2 s of a big turn, so differential thrust from
  its side thrusters joins in for the rest of it (90° in 1.50 s; 2.21 rad/s² on the first line, up to
  2.8 rad/s² measured); keyboard rotation, small corrections and holding a heading never use it. A ship without
  gyros or RCS (the sample ships) turns mostly on the last resort.

### Keyboard rotation

Decided by Niklas on 13 Oct 2026 (GDR-0055; the ramp numbers and the ramp in the client: lead defaults, GDR-0058):
A/D turned the Runner at about 4.6 rad/s (a 0.1 s tap turned it
~26°), too fast for fine aim. Now:

- **Ramp** (client side, deterministic input like the lock's nudge): the client integrates
  `flight::turn_ramp_step(level, key)` once per sim tick and sends the result as `PilotAxes::turn`. A press starts
  at `KEY_TURN_START` (0.2) of the key turn rate and grows by `KEY_TURN_GROWTH` (1.0) per second to 1 (0.8 s); a
  release drops to 0; the other key starts over. An analog input (a future gamepad stick) can send its deflection
  directly. Kept in the client because the input already carries a rate share (`turn × rate`), as with
  `nudge_step` (GDR-0040), and the sim needs no extra hashed state for it.
- **Rate**: `turn × min(key_turn_rate, √(margin · α · π))`: `FlightTuning::key_turn_rate` (1.5 rad/s), or slower
  if the ship could not stop from it within about a quarter turn.
- **Never above what the ship can do**: the rate is planned with the first line's `α` (`Kin::alpha`, never the
  last resort), and a `Rate` goal changes the turn rate at most by `margin` × what the first line gives this
  tick in that sense (the request is clamped to `margin · (pure torque + the RCS couples of that sense that
  lasting translation leaves free)`), so gyros, cores and RCS couples serve it alone. A/D never call the last
  resort (no sideways push from the keys): its gate is reset while a key turns the ship. Since the 13 Oct 2026
  review fixes this also holds with W held on an RCS design (the thrust takes the couples' nozzles: the Runner
  with two pods opened differential thrust on W+A), for one-sided couples (a pod pair that turns only one way)
  and right after a lock or `Auto` turn left the gate open (tests in `tests/keyboard.rs`). The ramp's growth
  (1.5 rad/s² at full rate) stays below what the Runner follows (0.7 × 2.21 = 1.55 rad/s²). A ship without gyros
  or RCS rotates slowly on the keys; its lock and auto turns still get the last resort.
- Halvex Runner (test `rotate_input_turns_the_ship`, `tests/keyboard.rs`): a 0.1 s tap turns the nose 0.8°;
  holding A: 0.17 s 1.3° (0.26 rad/s), 0.33 s 5° (0.52), 0.5 s 11° (0.77), 0.67 s 20° (1.03), 0.83 s 31° (1.29),
  1.0 s 45° (1.50, the cap), 1.5 s 88°; released at the cap it stops within 0.7 s, 26° further on (about 60° at
  the old 4.6 rad/s). The browser (headless, F3 overlay readout) agrees: a tap 0.8°, holding 1.50 rad/s, last resort
  off, 0.00 rad/s after release.
- **Lock**: A/D drive the aim nudge (`nudge_step`, GDR-0040) as before, from the raw key. Holding A, the
  Runner's nose trails the growing nudge (1.2 rad/s) by up to 0.29 rad, then sits on it (test
  `the_nudge_springs_back`); with a nudge growth of 0.8 rad/s it would trail by 0.14 rad.

### Planning with what works

Since 12 Oct 2026 the flight computer plans only with the actuators that can produce this tick
(`ship::producing`: alive, not overheated, not idle (GDR-0044) and powered, i.e. its last power level above 0); a
dark or dormant vessel does not fly at all. Code: `ship::refresh_authority`, the `fmax`/`tmax` lines in
`flight::execute`.

- `Piece::authority` (`ThrustAuthority`) counts those thruster rows and torque sources, each at its last power
  level (partial power scales its output). It is refreshed after every systems step (after the heat step, so a
  part that just overheated is out) and on rebuilds, and is zero while the piece is not active. `Kin` takes the
  thrust per axis, the top speed per axis (`speeds`, `top_speed`) and the planning angular acceleration from it,
  so the turn-rate planning, the stick's full length, `Auto`'s flip decision and every maneuver's top speed follow
  what still works.
- The allocation's `fmax`/`tmax` leave the same actuators out (0) but are **not** scaled by partial power: power
  caps what the movers of a tier draw together, so the dead-beat request is met exactly whenever the power
  suffices, and in a brown-out the systems hand out what there is (scaling the request up by the last level
  would overshoot as soon as the demand drops).
- Cost: one pass over each active piece's thruster rows and torque sources per tick, plus, on pieces with RCS
  couples, two `alloc::couples` calls (one per sense, each two `couple_fill` passes per group pair to size the
  couple and two to fill it); while translation lasts (a group pushing flat out) the flight computer runs one
  more translation fill and one `couples` call per sense. No allocation.
- Test `a_runner_with_idle_actuators_turns_and_brakes_cleanly` (`tests/flight.rs`): a Halvex Runner that lost a
  logic core, one gyro set to the lowest power priority, so it idles together with a pulse laser (load 18,
  capacity 16): 90° in 1.97 s with 0.06° overshoot, and a stop from 21 m/s (flip and burn) in 4.5 s without
  flying backwards (13 Oct 2026 content; 1.07 s and 3.3 s with the turning of 12 Oct). Planned with the idle
  gyro, as it was before 12 Oct (measured with the content of 12 Oct), the same stop flew backwards at up to 5.7 m/s and was still spinning at 2.3 rad/s.

### Top speed and the cruise flame

Decided by Niklas on 12 Oct 2026 ([GDR-0041](gdr/0041-engine-rated-speed.md)); it replaces the soft top speed
`k·√a`. The fade by the whole speed: Niklas, 13 Oct 2026 ([GDR-0055](gdr/0055-slower-crisper-turning.md)). Code:
`ship::rated_falloff`, `Thruster::falloff`, `ThrustAuthority::speed`, `Kin::speeds`, `Kin::top_along`, the
cruise lines at the end of `flight::execute`.

- **Rated speed**: every engine (`engine.rated_speed`) and RCS pod (`rcs.rated_speed`, each nozzle) has one, with
  bonuses (stats `rated_speed`, `rcs_rated_speed`) and `SimConfig::speed_scale`. Base content (placeholders):
  thruster 20 m/s, torch drive 22 m/s, RCS pod 14 m/s.
- **Falloff** (`Thruster::falloff(v_local, speed)`): a row that pushes **with the motion** (the piece's linear
  velocity at the centre of mass has a positive component along its push direction) fades by the piece's
  **whole speed** `|v|` (`rated_falloff(speed, rated)`), not only by the speed along its axis (GDR-0055; until
  13 Oct 2026 by the speed along its axis). Braking (against the motion) and sideways (velocity across it) keep
  full thrust. Full thrust up to `RATED_KNEE` = 80% of the rating, then linear to 0 at the rating and beyond. Flat
  first, so the acceleration stays constant for 80% of the way and the last 20% takes about a quarter of the
  time. Rotation-induced velocity is ignored (cheap, and a turning ship's points move sideways to the thrust
  anyway). Cost: one square root per flown piece per tick (`|v_local|`, shared by its rows).
- **Where it applies**: the flight computer computes each row's falloff once per tick from the velocity at the
  start of the tick (`Piece::falloff`, also for raw throttles), allocates over `max_force × falloff`, and the
  systems use the same numbers for the wrench (`max_force × power × falloff`) and for the power demand
  (`throttle × falloff`). So dead-beat control stays exact: the ship approaches the rating and never passes it,
  and holding a speed costs no thrust, power or heat. Gimbal side force and differential thrust scale with the
  same falloff. **RCS couples** do not: their forces cancel, so they cannot push the ship past any rating, and
  they fire at the nozzles' full force (`Actuation::couple`), so RCS pods turn a ship at any speed.
- **No drag**: a coasting ship keeps its speed (test `raw_thrust_tops_out_at_the_rated_speed_and_coasting_keeps_it`).
- **Top speed per direction** (`ThrustAuthority::speed`, `Kin::speeds`): per thrust group (forward, backward,
  right, left) the highest rating among its rows that can push now
  ([Planning with what works](#planning-with-what-works)). With the whole-speed fade no ship gets faster than
  its fastest thrusters, and diagonally no faster than the slowest group still pushing that way: the old box (a
  ship with 22 m/s forward and 14 m/s RCS sideways reached 26 m/s crabwise) is gone.
- **`Kin::top_along(u)`** (the locked touch stick's full length): the fastest velocity goal along `u` the flight
  computer reaches from rest. It pushes each group flat out until its share of the goal is there; the group that
  finishes last caps the speed at its own top speed, and the group that finishes first must get there before
  the whole speed passes its top speed (`speed_first / (share_first · √(1 + (thrust_last / thrust_first)²))`), so
  a small sideways share does not cut the main axis's top speed (the slowest group would otherwise cap any stick
  that is not exactly along an axis). Groups without thrusters are left out. Tests
  `top_speed_per_direction_comes_from_the_thrusters_pushing_that_way` and
  `a_diagonal_is_no_faster_than_the_slowest_thrusters_pushing_that_way` (`tests/speed.rs`): an RCS box (pods
  14 m/s, thruster 20 m/s) locked with the stick at (0.6, 0.8) flies that diagonal at 14.0 m/s (the box allowed
  23 m/s); with W and E held in drift it ends at 20 m/s (the thruster) with 7.9 m/s sideways.
- "Fly that way" turns the best axis onto the stick and flies at its rating (unchanged). The keyboard's W with Q
  or E asks for both axes' top speeds; the side push stops once the whole speed passes the side thrusters'
  rating.
- **Cruise flame**: whenever the goal has a velocity, every engine row gets `Actuation::cruise` = (goal velocity
  in the piece frame · engine direction) / its group's top speed, clamped to 0..1; the systems publish
  `Piece::cruise` = that × the engine's power level (0 if idle, dead, overheated, unpowered or the piece is dark). Full
  stick at the top speed = 1 (the engine pushes nothing, `Piece::output` ≈ 0, but the flame stays); **partial
  stick** = the stick share (holding half the top speed shows a gentle half flame, decided by the agent so that a
  held speed never looks like dead engines); maneuvers and behaviors (an approach, an orbit, matching a target's
  velocity) show it the same way. Released stick (goal 0, brake) and drift (no goal) show none. RCS nozzles get
  none (they puff only when they push). The client draws `max(output, cruise)` with the calm look where `cruise`
  exceeds `output` ([client.md](client.md) "Plumes"). The cruise level is derived output and not hashed.

### Tuning (`SimConfig::flight`, placeholders)

| Field | Default | Meaning |
|---|---|---|
| `max_turn_rate` | 6 rad/s (3 until 12 Oct 2026) | cap on the commanded turn rate |
| `turn_margin` | 0.7 | share of `α` planned for braking a turn (no overshoot) |
| `turn_gain` | 12 /s (8 until 12 Oct 2026) | linear zone near the target heading |
| `stop_margin` | 0.6 | share of the deceleration planned for stopping (maneuvers) |
| `position_gain` | 1.2 /s | linear zone of position → velocity |
| `stick_deadzone` | 0.05 | shorter sticks count as released |
| `keep_heading_fraction` | 0.5 | `Auto` keeps the heading if this share of the best axis pushes the right way |
| `key_turn_rate` | 1.5 rad/s (13 Oct 2026, GDR-0055, GDR-0058) | keyboard rotation at the end of the ramp (or slower if the ship could not stop from it within about a quarter turn); tuning panel "key turn rate (A/D)", `?keyturn=` |
| `assist_after` | 6 ticks (13 Oct 2026, GDR-0055, GDR-0058) | ticks in a row the first line must miss torque before the last resort (gimbals, differential thrust) helps |

`SimConfig::thrust_scale`, `torque_scale` and `speed_scale` (default 1) scale the content's thrust, torque and
rated speeds for feel tests; `split_ghost_ticks` (10) is the collision grace after a split. `SimConfig` also holds `physics` (`PhysicsTuning`,
see [Physics](#physics-rapier2d)) and `mind_capacity` (player 24 cores, other owners 1000, see [Vessels](#vessels)). Client-side input
constants (placeholders, `flight.rs`): the keyboard ramp `KEY_TURN_START` 0.2 and `KEY_TURN_GROWTH` 1.0 /s
(GDR-0055, GDR-0058), the lock nudge `NUDGE_MAX` 0.6 rad, `NUDGE_RATE` 1.2 rad/s, `NUDGE_RETURN` 5 /s (GDR-0040).

Why the turn defaults changed (GDR-0042): with 3 rad/s and a gain of 8 /s no ship could turn 90° faster than
0.78 s however much torque it had (it reaches the cap, then spends ~0.4 s in the linear zone), so the small raider
could not be snappier than the Runner. 6 rad/s and 12 /s let the raider turn in 0.4 s (12 Oct content; 0.57 s since 13 Oct); 12 /s still settles without
overshoot (each tick closes 40% of the remaining error).

### Turning and speed of the base ships (content of 13 Oct 2026, GDR-0055, scales 1)

Targets: Halvex Runner 90° in about 1.5 s (Niklas, 13 Oct 2026, GDR-0055; it was 0.8 s, GDR-0042), 0 → top speed
in about 2.5 s and a top speed of about 22 m/s (GDR-0042, unchanged); small ships snappier; RCS pods make a
visible difference. Placeholder content (lead defaults, [GDR-0058](gdr/0058-turning-v2-defaults.md)): thruster 200 kN / 20 m/s, torch drive 700 kN / 22 m/s (unchanged), RCS
nozzle **200 kN** / 14 m/s (60 kN until 13 Oct), gyro **1250 kN·m** (5000), logic core **50 kN·m** (500: tiny, it
keeps a ship controllable), automaton core **300 kN·m** (600; the raider's only turner, so it stays clearly the
snappiest). First-line `α` = (pure torque + RCS couples) / inertia, what turns are planned with until the last
resort opens.

| Ship | Mass | Inertia | First line | First-line α | 90° turn | Max α (measured) | Thrust → accel | Top speed | 0 → 95% top |
|---|---|---|---|---|---|---|---|---|---|
| Automaton raider | 15.8 t | 17 t·m² | 300 kN·m (core) | 17.8 rad/s² | **0.57 s** | 17.8 rad/s² | 200 kN → 12.7 m/s² | 20 m/s | 1.70 s |
| Halvex Runner | 106.5 t | 1176 t·m² | 2600 kN·m (2 gyros, 2 cores) | 2.21 rad/s² | **1.50 s** | 2.8 rad/s² | 1100 kN → 10.3 m/s² | 22 m/s | 2.53 s |
| Sample early | 43 t | 214 t·m² | 100 kN·m (2 cores, no gyros) | 0.47 rad/s² | 1.60 s | 2.8 rad/s² | 400 kN → 9.3 m/s² | 20 m/s | 2.30 s |
| Sample mid | 88 t | 1010 t·m² | 100 kN·m (2 cores, no gyros) | 0.10 rad/s² | 3.40 s | 0.6 rad/s² | 1100 kN → 12.5 m/s² | 22 m/s | 2.10 s |
| Runner + 2 RCS pods (test design) | 108.5 t | 1261 t·m² | 2600 + 2600 couples | 4.12 rad/s² | **1.20 s** | 4.7 rad/s² | 1500 kN → 13.8 m/s² | 22 m/s | 2.27 s |
| Runner + 4 RCS pods (test design) | 110.5 t | 1374 t·m² | 2600 + 5600 couples | 5.97 rad/s² | **0.97 s** | 6.5 rad/s² | 1900 kN → 17.2 m/s² | 22 m/s | 2.10 s |

(90° turn from rest to within 1°, holding position, overshoot under 2°; max α as measured in that turn,
including the differential thrust of the last resort; 0 → 95% of the top speed with the stick along the nose.
Tests `base_ships_meet_the_agility_target` and `rcs_pods_make_the_runner_turn_faster` in `tests/speed.rs`
measure and print the measured columns with `--nocapture` (the pod test also the inertia); the first-line columns
follow from content, torque / inertia. The two-pod turn took 1.17 s until the 13 Oct 2026 review fix: the
velocity hold's corrections against the last resort's sideways push saturate the pods' nozzles for a few ticks,
and the turn is now planned without the couples those take.) The pod variants (`common::runner_with_pods`, not content) move
the Runner's hull two columns right and hang one pod, or two side by side, beside each wing-tip radiator (row 3),
front and back sides open, so the right pod's front nozzle and the left pod's back nozzle make a couple (and the
other way round); their back nozzles also push forward (full thrust up to 11.2 m/s, 80% of the pods' 14 m/s rating, none from
14 m/s). The sample ships have no gyros:
their cores keep them controllable and their side thrusters turn them as the last resort (after 6 ticks), which
is why they turn as fast as the Runner (early) or slowly (mid) with a tiny first line. The Runner crosses the
~70 m default view in about 3.2 s. 12 Oct 2026 (GDR-0042): raider 0.40 s, Runner 0.77 s, sample early 0.90 s, sample
mid 2.03 s; until 11 Oct the Runner took 3.27 s per 90° at 0.65 m/s² and 9.7 m/s. Keyboard rotation (with the ramp,
[Keyboard rotation](#keyboard-rotation)): up to 1.5 rad/s on the Runner and the raider, 1.0 rad/s on the sample
early and 0.47 rad/s on the sample mid (what their first line can stop within a quarter turn); it was about
4.6 rad/s on the Runner and 6 rad/s on the raider.

### Maneuvers (`maneuver.rs`)

`Maneuver::Brake`, `Hold { at }`, `FaceTarget { target, aim }`, `Approach { target, range, aim }` (close in at
the speed it can still stop from, then face the target or its lead point and keep pace), `Orbit { target,
radius, ccw }` (tangential speed
limited by the centripetal acceleration the ship can hold, radial error corrected). Each becomes a
`FlightGoal` every tick; an unresolvable target brakes.

`KeepRange { target, range, aim }` (back off with `HeadingGoal::Auto` when closer than `range`, so forward-only
ships turn away; otherwise like `Approach`) and `Flee { from }` (straight away at top speed) were added for
behaviors. "Top speed" in every maneuver (`Flee`, the cap of `arrive_speed`, the orbit's 60%) is `Kin::top_speed`,
the rated top speed of the best axis (GDR-0041); a target that flees with faster engines gets away. `Maneuver::goal_for(kin, target_state, tuning)` takes an already resolved target (behaviors fly
relative to an aim point on the target ship).

The hard-coded `RaiderBrain` was deleted on 12 Oct 2026 (Niklas; ADR-0025, no dead code): the content behavior
`raider_basic` flies the same fight (see [Behaviors](#behaviors)).

## Behaviors

Code: `src/behavior/` (`mod.rs` library and vocabulary, `compile.rs`, `eval.rs`, `script.rs`). Format:
`docs/behavior-format.md`; content kinds: `docs/content-model.md`; records: GDR-0034 to GDR-0037, ADR-0026 and
ADR-0027 (all Accepted, 12 Oct 2026); the behavior editor and saved behaviors: GDR-0057, GDR-0059, GDR-0060.

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
  fleet spreads its thinks) and on the first tick after it gets a behavior. Dark and dormant ships do not think.
- **Two phases** (order-free, like the flight plan): every due ship decides from the same snapshot (the state
  after this tick's input breakage) into a per-ship scratch slot; then all decisions are applied. Per-pass
  caches: each ship's centre (one sin/cos per ship per pass instead of per candidate) and hull share (lazy).
- **A think**: `Kin` of the controlling piece; reach = longest clear gun (60 m without guns). The target line
  picks the target among active vessels of other owners (until 12 Oct: other factions), keeping the current one unless another's cost is
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
  target was picked; both need a target). Target conditions without a target fail with `NoTarget`, regardless of `not`.
- **Actions** → maneuvers: `approach(r)` → `Approach`, `orbit(r, dir)` → `Orbit` (`auto` = by ship id
  parity), `kite(r)` → `KeepRange`, `face` → `FaceTarget`, `flee` → `Flee`, `hold` → `Brake`. All aim
  with `AimMode::Auto` (lead for projectile ships).
- **Targets**: `nearest` (distance), `weakest` (hull share, ties by distance), `threat` ((d + 50) / firepower,
  firepower × 2 if it aims at me), `attacker` (nearest enemy aiming at me), `locked` (the lock of
  `Ship::leader()`: its pilot lock or behavior target).
- **`raider_basic`** (the automatons' default; it replaced the raider brain): with a cannon raider (75 m reach)
  it approaches to 64 m (85%), and within 90 m (120%) orbits at 50 m (67%) for the last 4 s of every 9 s;
  its cannon fires at will within 70 m (93.3%). Until 12 Oct it stood off at 50 m (67%) and orbited at 45 m
  (60%); see [Sandbox fights at the new speeds](#sandbox-fights-at-the-new-speeds).

### Sandbox fights at the new speeds

Measured 12 Oct 2026 (GDR-0042 content; not re-measured with the slower turning of GDR-0055, 13 Oct: the Runner
now turns 90° in 1.5 s instead of 0.77 s, the raiders in 0.57 s instead of 0.40 s) with a throwaway harness (not in
the repo): one Halvex Runner at rest and
a sandbox wave of three raiders spawned 105–125 m away, 0.32 rad apart, coming in at 3 m/s, under
`raider_basic`; seeds 1–12, at most 180 s. Two stand-ins for the player: **passive** (locks the nearest raider and
fires, never moves) and **closing** (the same, plus the stick along the nose towards the target while it is
farther than 45 m, drift on). "After contact" = after the first raider shot; hp of the Runner's 72.

| Version | Pilot | Waves won | Last raider down | Wave over (after spawn) | Runner hp lost (won waves) | Raider shots |
|---|---|---|---|---|---|---|
| before (11 Oct content, old `raider_basic`) | passive | 12 / 12 | – (raiders often never fired) | 22–28 s | 0–4.6 (median 2.6) | 0–7 |
| before | closing | 12 / 12 | – | 17–24 s | 0–3.2 (median 0) | 0–6 |
| new speeds, old `raider_basic` (50 m / 45 m) | passive | 8 / 12 (4 unfinished: a raider out of reach) | 3.4–8.4 s (median 4.8) | 10.7–15.9 s | 1.6–8.5 (median 2.8) | 3–12 |
| new speeds, old `raider_basic` | closing | 12 / 12 | 3.5–27 s (median 4.3) | 10.7–34 s | 1.6–6.9 (median 3.0) | 3–12 |
| new speeds, `raider_basic` 85% / 67% | passive | 8 / 12 (3 Runners lost, 1 unfinished) | 3.4–6.7 s (median 4.8) | 11.6–14.7 s | 2.0–6.6 (median 4.1) | 4–10 |
| new speeds, `raider_basic` 85% / 67% | closing | 11 / 12 (1 unfinished) | 3.6–10.6 s (median 4.5) | 11.9–19.0 s | 2.0–7.8 (median 3.1) | 4–11 |
| same, ram damage off (as for same-team bumps) | passive | 9 / 12 (3 unfinished) | 3.9–11.1 s (median 7.6) | 12.2–19.2 s | 3.2–6.4 (median 4.4) | 6–10 |
| same, ram damage off | closing | 10 / 12 (1 Runner lost, 1 unfinished) | 4.3–7.9 s (median 7.0) | 12.5–16.3 s | 3.4–7.8 (median 5.1) | 6–11 |

- At the new speeds the pack arrives in about 7 s instead of 15–20 s, and the agile Runner keeps its nose on a
  raider at once. With the old 50 m stand-off the raiders sat inside the Runner's pulse lasers (60 m) and died
  within one or two seconds each. At 64 m they shoot from just outside the pulse lasers and come into their
  reach while circling at 50 m (just outside the 45 m beams).
- The balance is on a knife's edge around the beam range: orbiting at 70% (52.5 m) instead of 67% the raiders
  won most waves against both stand-ins; at 63% or less they died as fast as before. Placeholders: Niklas tunes
  with the tuning panel, U (weapon range scales every reach, so the reach-relative distances keep their geometry).
- On the way in, two raiders of the pack bump each other at 20–30 m/s closing speed in nearly every wave
  (160–310 kN·s, 3–6 hp to the touching part of each, often a cannon): same-team ram damage changes fights
  noticeably (rows "ram damage off").
- The `skirmisher` (unpiloted ship-mind ships) fights these raiders badly (it rarely brings its fixed guns to
  bear while orbiting): a Runner under it won 3 of 12 waves against the old raider tuning (lost the ship in 5)
  and 5 of 12 against the new one (lost it in 6).
- "Unfinished" waves: the passive stand-in never moves, so a raider that lost its thruster (or drifted off after a
  bump) stays out of its reach.

### API

```rust
let content = data::base_content();
let mut world = sim::Sim::new(seed);
world.set_behaviors(sim::BehaviorLibrary::new(&content)?);           // before commanding behaviors
let raider = world.spawn_ship(raider_design.clone(), sim::Owner(1), body);
world.command(raider, ShipCommand::Behavior(world.behaviors().default_for(&raider_design))); // raider_basic
let mine = world.behaviors_mut().add_text("my_escort", "target locked\nalways -> face")?; // player program
// or add_program(id, name, program) for a parsed program (TEST IN RANGE's unsaved text); get(id), iter(), nodes()
world.command(escort, ShipCommand::Behavior(Some(mine)));
world.command(escort, ShipCommand::Leader(Some(player_ship)));     // `locked` follows the player's lock
// debug view / "test in range":
let st = world.ship(escort).unwrap().behavior().unwrap();
st.active_rule(); st.rules();          // [RuleStatus]: Active, Skipped, NoSlot, NoTarget, Failed { cond, why }
st.describe(i);                        // "`hull < 30%` is false (hull is 82%)"
st.target(); st.aim_part(); st.maneuver(); st.slots(); st.last_think(); st.script_error();
st.program().program.to_text();       // the source, e.g. for a scan report
```

- The player's saved behaviors (GDR-0057) reach the sim as content: the game adds every one without errors after
  the content behaviors, in id order, so `BehaviorLibrary::new` gives them fixed handles (handles are hashed). A
  design naming a behavior the content lacks gets no `default_for`; the game falls back to the faction's.
- `ShipCommand::Behavior(Some(id))`: a behavior overrides the helm. `Behavior(None)` hands the ship back to the
  helm. Unknown ids are ignored. `set_behaviors` drops every ship's behavior, parked ones included (handles refer to the old library).
- `BehaviorState::inherit()` copies a running behavior (shared program, same target and timers, thinks on its
  next tick): a vessel that splits off inherits its parent's behavior this way (GDR-0013, see
  [Vessels](#vessels)).
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

## Vessels

The vessel model (12 Oct 2026; GDR-0013, GDR-0038, GDR-0044 and GDR-0045, decided by Niklas). Code: `src/vessel.rs`
(owners, vessels, focus, mind capacity), `src/control.rs` (control capacity, idle parts), the ram rules in
`world.rs` and `weapons.rs`. Every number is a **placeholder**. Rules marked *lead default* were set by the
vessel agent for Niklas to review.

### Owners

- Every ship has an `Owner` (`Owner(u8)`): `Sim::spawn_ship(design, owner, body)`. `Owner::PLAYER` (`Owner(0)`)
  is the player by convention: `SimConfig::default()` gives it the player's mind capacity. The scenarios and the
  client give ship-mind ships `Owner::PLAYER` and automatons `Owner(1)` (`scenario::scenario_owner`).
- An owner counts for game rules only while the ship is a vessel: `Ship::team()` is `Some(owner)` for a vessel,
  `None` for salvage (`Ship::owner()` keeps the owner on wrecks, for later salvage rights). Per body,
  `Ship::piece_team(piece)` / `Sim::piece_team(key)`: only the vessel's piece has an owner.
- `sim::friendly(a, b)`: both `Some` and equal. Salvage is friendly to nobody (GDR-0046 lead default). For now an
  owner is its own team; co-op players on one team would get their own owners and a team table behind
  `friendly`.
- Uses: behavior targets are active vessels of other owners (until now: another faction); same-owner rams deal
  no damage; shots and engine plumes pass through same-owner vessels (GDR-0048, GDR-0046); the client's friendly
  clicks, Tab targets and off-screen markers.

### Vessels and splits

- A ship holds at most **one vessel: its controlling piece**, the piece with the ship's logic cores. It is a
  vessel while that piece has an alive logic core (`Ship::is_vessel`); unpowered or overheated cores leave it a
  vessel that is dark until they work again (`VesselState::Dark`). The other pieces of a ship are salvage
  without a core.
- **On a split**, every piece other than the kept one that still has an alive logic core becomes a ship of its
  own (`Sim::form_vessels`, right after the breakage): a new `ShipId` (after every existing one), the same
  `PieceId`, the same owner. Part states, power settings, controls, helm, leader and gun cooldowns are copied;
  each ship keeps only its own parts alive and in its grid. Pieces never link to each other, so exposure,
  bonuses and clearances do not change (`verify_caches` holds for both). The Rapier body is re-keyed
  (`Physics::rekey`: the same body with its contacts), and its ghost group stays the ship that split, so the
  halves still ghost each other for `split_ghost_ticks`.
- **Behavior**: the new vessel inherits the parent's behavior (`BehaviorState::inherit`: a copy of the running
  one, or of the one the focus parked; same target and timers, thinks on its next tick), limited to the rule
  slots of its own cores (GDR-0035: the lowest rules drop). A piece split off the focus that had no parked
  behavior gets its content default (`skirmisher` for ship-mind ships), and never the pilot's input (its helm is
  `Maneuver::Brake`). A piece split off an unpiloted ship without a behavior keeps the parent's helm (tests and
  scenarios that fly maneuvers or raw throttles).
- A blueprint built in several pieces with cores in more than one: at spawn every further cored piece becomes
  a vessel of its own the same way.
- A shot never hits its own ship (all its pieces) nor any other vessel of its owner (GDR-0048), so a split-off
  vessel and its parent fire through each other.
- Enemies follow the same rules: a raider shot in half can be two raiders (test).

### Focus

- `ShipCommand::Focus` makes a vessel its owner's focus, at once (a tick input, so replays and co-op see it).
  The focus is **piloted**: its behavior is parked (`Ship::parked_behavior`; `ShipCommand::Behavior` on the focus
  sets the parked one), its helm is `Pilot` (a released stick, i.e. brake, until pilot input arrives), and it
  never goes dormant. The vessel the focus left runs its behavior again (its content default if it had none;
  without a library it brakes) and gets a fresh mind claim, so it stays awake.
- An owner has no focus until one is commanded (scenarios and tests fly their ships as before; AI owners never
  have one). `Sim::focus(owner)`.
- **A lost focus moves on** (the focus ship lost its last logic core or was removed), in the mind pass: to the
  owner's best other vessel, *lead default*: running before dark before dormant, then the heaviest vessel piece,
  then the lowest ship id. A dormant one wakes (the focus runs first). No vessel left: no focus.
  `SimEvent::FocusChanged { owner, from, to }`.
- *Lead default*: a focus that is only dark (cores unpowered or overheated) keeps the focus, so a brown-out never
  yanks the player to another ship; the player switches by hand.

### Control capacity and idle parts (GDR-0038, GDR-0044)

- Content: every part has a `control_cost` (default by kind: hull parts, logic cores and passive parts 0, parts
  that draw or make power 1; big machines 2 in the base content), every logic core a `logic.control_capacity`
  ("N"). See `docs/content-model.md`.
- Per vessel: **load** = the control cost of its parts; **capacity** = N summed over its running cores (alive,
  not overheated, the vessel not dormant). Salvage and dormant vessels have capacity 0 and no idle parts.
- Load over capacity: parts go **idle** in the brown-out order of GDR-0014, the reverse of the allocation order
  with every tier and weight setting (content, drydock placement, `ShipCommand::Power`): `Off` first, then `Low`,
  `Normal`, `High`; inside a tier the lowest weight first, the higher part index first on ties; reactors (they
  make power, they have no tier) after every consumer, *lead default*; parts with a cost but no power role last
  (content validation warns about those). Parts are idled until the idled cost covers the excess (a cost-2 part
  may idle where 1 would do).
- An idle part does nothing: it asks for no power (`PartState::power` and `work` 0), engines, RCS and gyros
  push nothing, guns hold, a shield emitter drops its bubble (keeps its charge), a reactor makes nothing. Idle
  parts run again as soon as the capacity covers them (a core cools down, a vessel wakes, the load drops).
- The flight computer plans only with actuators that can produce, so idle engines, RCS pods and gyros drop out of
  its plan at once ([Planning with what works](#planning-with-what-works); until 12 Oct 2026 it planned with the
  full thruster table, and a ship that lost a core misjudged its thrust and torque).
- Cost: per piece and tick one pass over its logic cores (the capacity); the idle set is recomputed only when
  the capacity changes, the piece was rebuilt or a power setting changed (one pass over its parts). No
  allocation. Exposed: `PartState::idle`, `Piece::control` (`ControlState { load, capacity, idle }`),
  `Ship::control()`.

Base content (placeholders): logic core N 16, automaton core N 4; fusion reactor, torch drive and beam laser cost
2. Every base ship runs all its parts:

| Ship | Cores | Control load | Capacity | Idle when one core is lost |
|---|---|---|---|---|
| Halvex Runner | 2 logic | 18 | 32 | 2: both pulse lasers (gun tier, highest part index first; test) |
| Automaton raider | 1 automaton | 2 | 4 | the raider is salvage without its core |
| Sample early | 2 logic | 8 | 32 | nothing |
| Sample mid | 2 logic | 16 | 32 | nothing |

### Mind capacity and dormant vessels (GDR-0013, GDR-0044)

- `SimConfig::mind_capacity[owner]`: how many logic cores of the owner's vessels run at once (owners beyond the
  table: no limit). Placeholders: player 24 cores (twelve Halvex Runners; the sandbox's `?fleet=8` uses 18), AI
  owners 1000. A vessel takes its alive cores (dark ones included).
- **Claim order** (*lead default* for the details the decision left open): the focus first; then vessels with a
  fresh claim (`ShipCommand::Wake`, or the vessel the focus just left), the most recent first; then the others by
  formation, oldest first. Each vessel runs if its cores fit what is left of the capacity, else it is
  **dormant**; a smaller vessel further down may still fit. So over capacity the newest vessels go dormant first,
  the focus never does, and freed capacity wakes the dormant vessels in the same order.
- `ShipCommand::Wake`: the vessel claims capacity before every vessel not woken since; the newest of those goes
  dormant to make room (a swap, GDR-0044's fleet bar).
- **Dormant** (*lead default*, as recommended): fully dark like salvage, but still owned: no behavior, no thrust,
  no power allocation (parts draw nothing, `power` 0), no shield bubble (the charge stays), no control capacity,
  not a target for enemy behaviors (they target active vessels); same-owner rams stay harmless, and it stays in
  the fleet list.
- The mind pass (`Sim::update_minds`) runs only when something changed: a vessel formed or was lost, a logic
  core was destroyed, a focus or wake command, a config change. One pass over the ships and a sort of the
  vessels in a reused buffer.

### Ramming between vessels (GDR-0045)

- Contacts between vessels of one owner resolve physically but deal **no ram damage** (`Contact::damage` 0; the
  impact is still reported). Salvage (no owner) rams and is rammed as before.
- **Shields absorb rams**: the ram damage to each side is first taken by the shield bubbles of this tick that
  cover the contact point (`|point − centre| ≤ radius`), each its kinetic share (`absorb[Kinetic]`, 0.5 for the
  base emitter; GDR-0021) of what reaches it, in bubble order (ship, piece, emitter), except the bubbles of the
  other side's piece (as a shot passes its shooter's bubbles). The same absorb step as shots
  (`ShieldHit { by: the rammer }`, a bubble that runs empty collapses); the rest hits the part. The bubbles are
  the ones collected in this tick's weapons phase (positions before the physics step).
- **Crush** (GDR-0047, decided by Niklas on 12 Oct 2026): what the shields leave hits the touching part, and
  what exceeds its hit points carries on to the next part inward, along the contact normal into the hull, until
  it is used up or leaves the hull (`Sim::ram_crush`, the grid walk of penetrating shots; the emptied cells of a
  big part just crushed are passed through). The threshold rose from 20 to 100 kN·s, so bumps are free; the
  numbers are in [Collisions](#collisions-impacts-and-ramming-v1).

### Events and queries (for the fleet bar)

| | |
|---|---|
| `SimEvent::VesselFormed { ship, owner, from }` | a vessel formed; `from: Some((ship, piece))` for a split-off piece (the client moves that view to the new ship), `None` at spawn |
| `SimEvent::VesselLost { ship, owner }` | lost its last logic core (after `ControlLost`) or removed |
| `SimEvent::VesselDormant` / `VesselWoken { ship, owner }` | mind capacity |
| `SimEvent::FocusChanged { owner, from, to }` | focus command, or the focus was lost |
| `Sim::vessels(owner)` | iterator of `VesselInfo { ship, piece, owner, state, focus, cores, control, formed }`, ascending ship id, no allocation |
| `Sim::vessel(ship)`, `Sim::focus(owner)`, `Sim::mind_load(owner)` | one vessel; the focus; `(cores running, capacity)` |
| `Ship::team()`, `is_vessel()`, `is_dormant()`, `vessel_state()`, `core_count()`, `control()`, `formed()`, `parked_behavior()` | per ship |

The state hash covers owners, dormant flags, claims and formation ticks, parked behaviors, per-part idle flags,
per-piece control load, capacity and idle cost, the focus per owner, the claim counter, the mind capacities in
the config and the physics ghost groups.

### Cost

Per tick: one pass over each piece's logic cores (the control capacity) and two more flag checks per consumer;
the idle set and the mind pass run only when something changed. A split that forms a vessel copies the ship's
per-part state (parts, stats, heat constants, settings, grid) into the new ship: microseconds for the base ships
(at most 56 parts). Measured 12 Oct 2026 (before the agile-ship merge and GDR-0047), native release,
interleaved with a build of the commit before (3 runs each, load average 2–7, medians in ms):

| What | Before | After |
|---|---|---|
| load test, 20 ships, step median | 0.022–0.024 | 0.023–0.025 |
| `sim_perf`, 20 × 2000 parts (25 logic cores each), raw throttles, step median | 0.384–0.410 | 0.397–0.409 |
| `sim_perf` combat (20 gunboats), step median | 0.68–0.73 | 0.68–0.76 |
| cut a 2000-part slab in two (`Sim::resolve_breakage`), both halves with cores | 0.08–0.11 | 0.13–0.15 (the second half becomes a vessel) |

The per-tick differences are within the VM's noise; the split pays about 0.05 ms for copying 2000 parts' state.

### Determinism and scenarios

The behavior scenario now makes its piloted leader the player's focus and caps the player's mind capacity at
8 cores (`scenario::BEHAVIOR_MIND_CAPACITY`; the ship-mind ships have 12), so dormant vessels, waking and the
focus are part of the cross-platform check; the core scenario forms a split vessel, and every 600 ticks it
launches a ship-mind vessel and an automaton (or a wreck) head-on at 20 m/s each, so a hard ram with the crush is
covered too (with the agile ships the flight, load and behavior scenarios ram on their own as well). 12 Oct
2026, on `dev` with the rated-speed and agile-ship merge, dev profile (optimised dependencies), 1800 ticks,
native x86_64 vs wasm32-wasip1 under Node 24: core, flight, load (20 ships) and behavior, seeds 42 and 7:
**identical** (8 runs). Counters with seed 42 (new: split vessels, vessels lost, dormant, woken, focus changes):

| Scenario | Parts destroyed | Splits | Shots | Hits | Impacts / rams | Split vessels | Lost | Dormant / woken / focus |
|---|---|---|---|---|---|---|---|---|
| core | 266 | 50 | 711 | 2 | 1 / 0 | 0 | 8 | 0 / 0 / 0 |
| flight | 141 | 23 | 980 | 166 | 213 / 1 | 0 | 4 | 0 / 0 / 0 |
| load (20) | 87 | 2 | 214 | 971 | 167 / 3 | 0 | 10 | 0 / 0 / 0 |
| behavior | 119 | 21 | 465 | 97 | 251 / 0 | 0 | 4 | 2 / 2 / 1 |

Seed 7 adds a split vessel in the core, flight and behavior scenarios, a ram in core (the ram run) and behavior,
and 3 dormant / 3 woken vessels in behavior. Most impacts are bumps: within one owner they are free, and below
100 kN·s between owners too (the load test had 24 rams before the vessel model, most of them a side ramming its
own ships).

## Weapons

All hit tests are ray casts through Rapier's query pipeline (`Physics::raycast`, see [Physics](#physics-rapier2d));
a shooter never hits any piece of its own ship, nor any vessel of its owner: **no friendly fire** (GDR-0048,
12 Oct 2026). Shots fly through the shooter's own vessels (no hit, no damage, no shield drain) and hit whatever
lies behind; salvage (no owner) and other owners are hit as before. A projectile keeps the owner it was fired for
(`Projectile::team`, hashed; GDR-0048's lead default), also after ricochets and when its ship has become salvage
since. A gun fires while `output > 0`: firing commanded
(`PilotInput.fire`, `ShipControls.fire` and its per-gun mask, or fire at will under a behavior), clear line of
fire, powered, not overheated, not idle (over the control capacity), piece active. Guns are fixed: the pilot aims by turning the ship; under a
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
  with `capacity`, regenerating `regen × work` per second. A shot whose path enters a bubble of a non-friendly
  ship (not its own, not a vessel of its owner) loses `damage × absorb[family]` (content: energy 1.0, kinetic 0.5, missile 1.0) up to what the bubble has
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
| `ShieldHit { by, ship, piece, part, point, absorbed, left, depleted }` | a shot entered a bubble (`point` = entry), or a bubble covering a plume's hit point or a ram's contact point took its share; `depleted` = it collapsed |
| `PlumeHit { by, engine, ship, piece, part, point, damage, heat, absorbed }` | an engine plume burned a part, one per lane and part per tick ([Engine plumes](#engine-plumes)) |

### Aiming and leading

- `flight::intercept_time(r, v, speed)` / `lead_point(shooter, shooter_vel, target, target_vel, speed)`:
  first-order intercept (constant target velocity) with the shot inheriting the shooter's velocity, as the sim
  fires them; only `sqrt`, no transcendentals. A target that cannot be caught is aimed at directly.
- `AimMode` (`PilotInput.aim`, `Maneuver::FaceTarget/Approach/KeepRange { aim }`): `Target` (nose on the target),
  `Lead` (nose on the lead point of the ship's strongest projectile gun), `Auto` (default). Under a pilot lock
  `Auto` is the smart lock aim of GDR-0040 (`flight::smart_aim`): the clear, working guns are grouped by the side
  they face, the group with the most damage per second wins (ties: front, back, right, left), and the ship turns
  that side onto the target, or onto the lead point if that group's strongest gun fires projectiles;
  `PilotInput.aim_offset` (the A/D nudge) is added on top. For maneuvers (behaviors) `Auto` leads if the ship's
  strongest gun by damage per second fires projectiles, else it aims at the target. The other guns still fire when
  they happen to bear.
- Guns start with **staggered cooldowns** (spread over one firing period, from a hash of ship id and part
  index), so a fleet's guns never fire in lock-step. A gun that is not firing reloads to ready, so a fresh
  trigger fires at once (a player's alpha strike).

Queries each frame: `Sim::projectiles()` (`pos`, `prev_pos`, `vel`; draw at `prev_pos + (pos − prev_pos) ×
alpha`), `Sim::beams()` (this tick's beams), `Sim::bubbles()` (`centre`, `radius`, `left`, `capacity` of every
bubble that is up), and per emitter `ship.part(i).shield` and `.shield_down`. `Sim::spawn_projectile` creates a
projectile directly (tools, tests).

## Engine plumes

Engines may sit anywhere (GDR-0039, decided by Niklas); the clear-exhaust rule (an engine with a part of its own
piece right behind it gave no thrust) was removed on 12 Oct 2026. Instead each firing engine's exhaust plume
burns the first part in its path: the ship's own (the layout rule), enemy vessels, salvage and debris, never
other vessels of the same owner (GDR-0046). Rules marked *lead default* are [GDR-0049](gdr/0049-engine-plume-v1.md)
(Accepted, provisional) for Niklas to review. Code: `src/plume.rs`; numbers are **placeholders**.

- **Content**: `engine.plume: (length, damage, heat)`, metres behind the engine's back side, damage and heat per
  second at full thrust (stats `plume_length`, `plume_damage`, `plume_heat`; bonuses can change them). Base:
  thruster 3 m, 1 /s, 1.5 /s; torch drive 6 m, 3 /s, 4 /s.
- **Lanes**: one per cell of the engine's back side (a thruster one, a 2×2 torch drive two), a straight line from
  the centre of that cell's back face along the exhaust (against the push direction). The damage and heat are
  shared evenly over the lanes.
- **How hard** (*lead default*): damage and heat scale with the engine's thrust output this tick
  (`Piece::output`: throttle × power level × rated-speed falloff), so an idle engine burns nothing and one holding
  its rated speed (a cruise flame, no thrust) next to nothing. Per lane and tick: `damage × output × DT / lanes`,
  likewise for heat, added to the part's heat level as `heat / heat capacity` (the units of `heat.output`).
- **Thrust** (*lead default*): a blocked plume gives full thrust; the cost is the burning.
- **Own vessel**: the parts of the engine's own piece in a lane come precomputed from the blueprint
  (`ShipDesign::plume_path`, to the grid edge); the first alive one within the plume's length is hit. The ship's
  own shield bubbles never stop its own exhaust (*lead default*).
- **Other bodies**: one Rapier ray cast per lane, up to the own hit (or the full length), skipping the engine's
  own piece and every vessel of the same owner (`Physics::raycast_skipping`). Enemy vessels, salvage (the ship's
  own wrecked pieces included) and debris are hit. Bubbles of non-friendly ships covering the hit point absorb the
  plume like energy damage (their `absorb[Energy]` share, in bubble order, `ShieldHit { by: the engine's ship }`),
  and the heat goes with the damage in proportion (*lead default*).
- **Depth** (*lead default*): each lane stops at the first part it hits, on whichever ship; that part takes the
  lane's damage and heat. Once it is destroyed the plume reaches further, from the next tick on (it burns its way
  through).
- **When**: in the weapons phase after the guns (the bubbles of this tick, bodies at their start-of-tick poses);
  the breakage is resolved with the weapons'. Events: `SimEvent::PlumeHit { by, engine, ship, piece, part, point,
  damage, heat, absorbed }` per part burned per lane and tick (`ship == by` for the own hull).
- **For the client**: `Thruster::plume_length` is each engine's reach (static content with bonuses; the client
  draws the flame at its own length and, while the plume burns something, a faint streak out to the `PlumeHit`
  point, GDR-0055); `PlumeHit` marks where a plume is blocked and burning.

Measured (tests in `tests/plume.rs`): a Halvex Runner holding its torch drive (all engines at full thrust) 0.5 m
from a raider's nose for one second deals 2.8 hp and wrecks 2 parts while it pulls 10 m away; a thruster buried
against a composite armour block (1.4 hp), held at rest at full thrust, burns through it in 1.40 s and through the
next one 1.4 s later. The base ships' engines all exhaust into open space, so they never burn themselves; in the
scenarios plumes also hit enemies on their own (seed 42, 1800 ticks: 8 plume hits in `flight`, 4 in `load`, 25 in
`behavior`; see [Determinism](#determinism)).

**Cost.** A piece whose plumes reach neither a part of its own (`Piece::plume_blocked`, cached on rebuilds) nor
another body is skipped after one scan of the bodies' bounding circles (`Piece::radius` about the frame origin,
against `Piece::plume_reach`); a lane casts only if a body near the piece can be within its reach. So plumes into
empty space cost a few comparisons per ship, and a ray cast happens only at close quarters. No allocation once
warmed up. Measured 12 Oct 2026 with a temporary profiling mark around the plume pass (not in the repo), native
release, load average 7.8–8.2, three runs each:

| Scene | Plume pass median | p95 |
|---|---|---|
| load test, 20 ships fighting (1800 ticks) | 0.64–0.71 µs | 1.5–1.9 µs |
| `sim_perf` raw throttles, 20 slabs × 50 thrusters firing (1000 lanes, nothing in reach) | 1.8–2.1 µs (25 µs before the per-piece skip) | 3.2–3.5 µs |
| `sim_perf` combat (20 gunboats) | 2.4–2.6 µs | 4.2–10 µs |

Whole steps, interleaved with a build of `dev` before this change (`05c7cc7`), five load-test and three
`sim_perf` runs each, native release, on an idle VM (load average 0.1–0.3), medians of the per-run medians / p95
in ms:

| What | Before | After |
|---|---|---|
| load test, 20 ships, step | 0.018 / 0.048 | 0.019 / 0.048 |
| the same, Rapier step alone | 0.008 / 0.025 | 0.009 / 0.024 |
| `sim_perf` raw throttles (1000 engines firing into space), step | 0.433 / 0.589 | 0.398 / 0.645 |
| `sim_perf` pilot + lock, step | 0.582 / 1.428 | 0.435 / 0.747 |
| `sim_perf` combat (two owners: the same fight before and after), step | 0.763 / 2.517 | 0.749 / 2.507 |

The load test's fight itself changed (no friendly fire: 214 → 202 shots, 971 → 839 hits, 167 → 175 impacts); the
plume pass is under 1 µs of its step. The `sim_perf` pilot run got cheaper because its slabs (all one owner) no
longer shoot each other apart; raw throttles and combat are the same within noise. (A first round at load
average 9–10 gave the same picture with medians 1.5–2× higher.)

## Physics (Rapier2D)

Rapier2D `=0.36.1` (ADR-0014) with `enhanced-determinism`, default features off (no `block-solver`: the
benchmarked configuration), single-threaded (`parallel` off; see [Performance](#performance) for whether it
would pay on native). `physics.rs` wraps it in the concrete type `Physics`; `Sim` owns one and makes every call
itself, in a fixed order. **No Rapier type appears in the public API**: the client sees `Body`, `BodyKey`,
`RayHit`, `Contact`, `PhysicsTuning` and `Sim::physics()` (read-only: `body`, `is_ghost`, `is_sleeping`,
`mass_props`, `raycast`, `len`).

The engine-free `FreeFlight` stand-in and the generic `PhysicsBackend` trait were removed on 12 Oct 2026
(ADR-0025: the main repo holds only chosen code). `Sim` has no backend type parameter; switching engines later
means rewriting `physics.rs` (about 620 lines with docs, plus about 190 lines of unit tests).

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
| `ram_threshold` | 100 kN·s | impulse per tick above which a contact deals ramming damage (GDR-0047; 20 until 12 Oct) |
| `ram_damage` | 0.02 hp per kN·s | ramming damage per kN·s above the threshold, to each side, crushing inward |

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

Every piece of a split (the kept one and the new ones) is a ghost for `split_ghost_ticks` (10 ticks): a ghost
group in its collider's user data (the id of the ship that split) plus Rapier's contact-pair filter hook, which
drops a pair when both colliders are ghosts of the same group. A piece that split off as a vessel of its own
moves to a new ship but keeps its group, so it ghosts its parent too. It is a pure function of the two
colliders (deterministic), and other pairs never run the hook. Ghosts still collide with other ships. So
freshly separated pieces never push each other apart; afterwards they collide like any others. Tests:
`split_pieces_ghost_each_other_then_collide`, `ghosts_of_one_group_pass_through_each_other`.

### Ray casts

`Physics::raycast(origin, dir, max, exclude)` → `RayHit { key, distance, point, local_point, normal }`, through
Rapier's query pipeline over the broad-phase BVH, with a predicate on the collider's user data that skips every
piece of one ship (`exclude`, the shooter). Rays are solid: one that starts inside a body hits it at distance 0. Inside the sim, shots and plumes cast with
`Physics::raycast_skipping`, a predicate on the body key that also skips friendly vessels (the owner lookup runs
only for bodies whose bounding box the ray crosses).
Bodies created or moved between steps (splits at the start of a tick, teleports) go into the BVH at once, so
guns see them in the same tick. Equal distances resolve in the BVH's traversal order (deterministic).
Shots, plumes and `Sim::raycast` map the hit to a part with one rule (`Sim::cast_part`): the alive part under the
hit point (`part_at_local` just inside the surface). A part destroyed earlier in the tick keeps its collider until
breakage is resolved (rebuilding a compound per hit would cost a piece rebuild each), so a ray that lands on its
cells walks on through the piece's grid along the ray to the next alive part of that piece and enters it at its
cell's boundary (normal `−dir`); if the walk leaves the piece first, the ray is cast again from there, at most
`weapons::tuning::MAX_RECASTS` (4) times. Until the 13 Oct 2026 review fix such a ray ended on the stale collider
without damage, so a second shot at a block in the tick that destroyed it was lost.

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
| `damage` | ramming damage to `part_a` and to `part_b` each, before shields (0 below the threshold and between vessels of one owner) |

**Ramming damage** (concept: "ramming damage by mass and speed; ram prows are a build"):

```
damage = max(0, impulse − ram_threshold) × ram_damage        to each side (GDR-0047: 100 kN·s, 0.02 hp/kN·s)
```

Each side's damage goes through that side's shield bubbles first (GDR-0045), then hits the touching part
(`part_a`, `part_b`); what exceeds a part's hit points **crushes inward** to the next part along the contact
normal, and so on until it is used up or the walk leaves the hull (GDR-0047, `Sim::ram_crush`; the grid walk of
penetrating shots, without their block limit). A touching part that an earlier contact of the same tick destroyed
passes the whole damage inward from its cells (until the 13 Oct 2026 review fix that ram was dropped).

The impulse already carries mass and speed: head-on, `impulse ≈ (1 + e) · μ · v` with the reduced mass
`μ = m_a·m_b / (m_a + m_b)` and the closing speed `v`. Both sides take the same damage (the same impulse acts on
both hulls), so armour and hit points decide who breaks: a ram prow is heavy, tough armour at the nose. Damage
is applied right after the step and the breakage is resolved in the same tick (`PartDestroyed`, `Split` follow
the `Impact`). Since 12 Oct 2026 (GDR-0045): vessels of one owner deal each other no ram damage, and shield
bubbles covering the contact point absorb their kinetic share first (see
[Ramming between vessels](#ramming-between-vessels-gdr-0045)). With the defaults, between two owners and
without shields:

| Collision | Impulse | Damage to each side |
|---|---|---|
| two raiders (15.8 t) head-on at 8 m/s each (measured, test) | 157 kN·s | 1.1 hp (the nose cannon, 1 hp, destroyed; 0.1 into the core) |
| raider rammed by a Halvex Runner (106.5 t), 10 m/s | ~165 kN·s | ~1.3 hp |
| two runners head-on, 10 m/s closing | ~640 kN·s | ~11 hp |
| a raider bumping another at 4 m/s each (test), or touching at 1 m/s | < 100 kN·s | none (impact only) |
| a runner pushing another at full thrust (69 kN, 11 Oct content) | ~2.3 kN·s per tick | none |

At the new speeds (GDR-0042; impulses measured 12 Oct 2026 on the speed branch, both ships coasting nose to
nose, one contact tick each; damage by the GDR-0047 rule, crushing inward from the touching part):

| Collision | Closing speed | Impulse | Damage to each side |
|---|---|---|---|
| Halvex Runner at its top speed (22 m/s) into a raider at rest | 22 m/s | 378 kN·s | 5.6 hp (2–3 raider blocks, a dent in the Runner's nose) |
| Runner (22) and raider (20) head-on | 42 m/s | 720 kN·s | 12.4 hp (the raider mostly wrecked) |
| raider at its top speed (20) into a Runner at rest | 20 m/s | 343 kN·s | 4.9 hp |
| two raiders head-on at 20 m/s each | 40 m/s | 404 kN·s | 6.1 hp: cannon, core and thruster (4 hp) crushed (test) |
| two Runners head-on at 22 m/s each | 44 m/s | 2946 kN·s | 57 hp |
| Runner (22) into a raider broadside | 22 m/s | 378 kN·s | 5.6 hp |
| raiders of one pack bumping on the way in (sandbox, every wave) | 20–30 m/s | 160–310 kN·s | none (one owner); 1.2–4.2 hp between owners |
| for comparison: Runner at 10 m/s into a raider at rest | 10 m/s | 172 kN·s | 1.4 hp |

A sustained push stays below the threshold: two Runners pushing nose to nose at full thrust (1100 kN each) press
~37 kN·s per tick into the contact, a Runner pushing a free raider ~5 kN·s per tick: no damage.

The part under a contact: the contact point moved 5 cm into the hull (against the normal) and looked up with
`Ship::part_at_local`; if that misses, the centre of the strongest manifold, then the strongest manifold's
sub-shape (a slope triangle names its part; a merged rectangle is looked up at the point clamped into it). The
centre of all points is used because two flat faces touch at the ends of their overlap, which usually sit on a
neighbouring slope's corner.

Limits: one contact point per side, per pair and tick (a long scrape spreads over ticks, not over parts); the
crush runs in a straight line from that point; no content stat for rams yet (a `ram` bonus on prow parts could
scale the damage later). A reinforced prow (high-hp armour at the nose) is the ramming build (GDR-0019,
GDR-0047).

## Damage and breakage

- `Sim::apply_damage(ship, DamageTarget::Part(i) | Cell((x, y)), amount)` → `Ignored`, `Damaged { hp_left }`
  or `Destroyed { excess }` (excess for penetration later). A destroyed part leaves the grid at once.
- Damage comes from scripted commands, weapons, engine plumes ([Engine plumes](#engine-plumes)) and rams
  ([Collisions](#collisions-impacts-and-ramming-v1)).
- `Sim::resolve_breakage()` (steps 3, 7 and 9 of every tick) processes each damaged piece once: drop dead parts,
  flood-fill the rest; one component → `SimEvent::PieceChanged`; several → `SimEvent::Split`; none →
  `SimEvent::PieceDestroyed`. Every destroyed part emits `SimEvent::PartDestroyed { volatile }`.
- On a split the main component keeps the piece id, frame and body. Every other component gets a new
  `PieceId`, a frame on its own centre of mass, the parent's angle and angular velocity, and
  `linvel = v + ω × (com_new − com_old)` (`ship::inherited_velocity`). The kept piece's `linvel` is corrected
  the same way. Tests check that no surviving part moves and that linear momentum is conserved. A new
  component that has a logic core then moves to a ship of its own, a vessel (`SimEvent::VesselFormed` after the
  `Split`; see [Vessels](#vessels)).
- `Sim::raycast(origin, dir, max, exclude)` → `PartHit { ship, piece, part, hit }`.
- Chain reactions: `Ship::chain_reaction(part)` is the hook (does nothing yet); the plan is an `explosion`
  block in content.

## Driving the sim from the client

```rust
let content = data::base_content();
let runner = Arc::new(sim::ShipDesign::compile(&content, content.ships.get("halvex_runner").unwrap())?);
let mut world = sim::Sim::new(seed);                               // or Sim::with_config(seed, config)
let me = world.spawn_ship(runner, sim::Owner::PLAYER, sim::Body::at(Vec2::ZERO, 0.0));
world.command(me, sim::ShipCommand::Focus);                         // the vessel the player pilots
// every FixedUpdate (30 Hz):
let input = sim::PilotInput { stick, lock, drift, fire, ..Default::default() }; // stick in world terms, |stick| ≤ 1;
// PC keyboard: axes: Some(PilotAxes { turn, thrust, strafe }) (turn ramped with flight::turn_ramp_step),
// aim_offset: the A/D nudge under a lock (flight::nudge_step)
world.step(&sim::TickInputs { commands: vec![(me, sim::ShipCommand::Pilot(input))], ..Default::default() });
for e in world.events() { /* ShipSpawned, Split (spawn visuals for `spawned`), VesselFormed (move a view), ... */ }
// every frame (60 fps), alpha = Time<Fixed>::overstep_fraction():
for ship in world.ships() { for piece in ship.pieces() {
    let (pos, angle) = piece.render_pose(alpha);                   // world, y up, CCW
    // parts: blueprint (x, y) → piece frame (x − origin.x, origin.y − y); glow if piece.active
    // plumes: piece.thrusters[i] with piece.output[i] (0..1: throttle × power × speed falloff, plus RCS couples at full force) and
    // piece.cruise[i] (0..1, the calm cruise flame of an engine holding a speed; 0 for RCS nozzles);
    // its reach: piece.thrusters[i].plume_length (m back from the engine; burns arrive as SimEvent::PlumeHit)
}}
```

- Weapons: draw `world.beams()` as lines, `world.projectiles()` as tracers (interpolate `prev_pos → pos`),
  `world.bubbles()` as dotted rings (alpha from `left / capacity`); flash on `ShieldHit`, sparks on `Hit` and
  `Ricochet`, muzzle flashes on `ShotFired`, a burn (or a blocked flame) at the point of each `PlumeHit`.
- Collisions: `SimEvent::Impact(contact)` at `contact.point` (scale sparks and sound by `impulse`; `damage > 0`
  is a ram). Ships now collide, so spawn them apart.
- Enemies: `world.set_behaviors(BehaviorLibrary::new(&content)?)` once, then
  `world.command(raider, ShipCommand::Behavior(world.behaviors().default_for(&design)))` (see
  [Behaviors](#behaviors)).
- Debug HUD: `ship.telemetry()` (top speed of the best axis, goal, heading target), `piece.power`,
  `ship.parts()[i].heat`, `piece.falloff[i]` (thrust left at this speed).
- Power: `ship.parts()[i].power` (0..1) dims a part's icon at partial power; the drydock writes the player's
  tiers and weights into the blueprint placements; in flight `ShipCommand::Power { part, setting }` changes
  them (see [Power](#power)).
- Keep stable ids: visuals keyed by `(ShipId, PieceId)`; a split keeps the parent's `PieceId` for the kept part,
  and a piece that becomes a vessel of its own keeps its `PieceId` under the new ship (`VesselFormed.from`).
- Vessels: `world.focus(owner)`, `world.vessels(owner)` (fleet bar), `ShipCommand::Focus` / `Wake`; see
  [Vessels](#vessels).
- Drydock: `sim::design_report(&content, &blueprint, &config)` after every edit, with the content and config the
  game flies with; see [Design report (drydock)](#design-report-drydock).

## Design report (drydock)

Added 12 Oct 2026 for Drydock v1 (GDR-0008: live stats and overlays after every edit). Code: `src/report.rs`,
tests `tests/report.rs`. All numbers in `report::tuning` are **placeholders**.

```rust
let report = sim::design_report(&content, &blueprint, &config)?;   // pure, deterministic, no world
report.flight_ready();                                            // no blocker among report.issues
```

`design_report(content, blueprint, config) -> Result<DesignReport, String>` works on any in-memory blueprint
(overlaps and the frame are not checked; `Err` only for unknown parts or materials). It builds the ship exactly
as `Sim::spawn_ship` does (`ShipDesign::compile`, then `Ship::new`: exposure, stats after bonuses, pieces, mass,
colliders, consumer rows, thruster and RCS tables, gun mounts) and runs the sim's own code on the main piece:
the control pass (`control::update`, the GDR-0044 idle order), the authority (`refresh_authority`, `Kin::of`)
and one power pass (`ship::power_pass`, the function `update_systems` calls every tick). The rules the report
needed were moved out of `Ship` into shared functions (`applied_bonuses`, `reactor_supply`,
`consumer_activity`, `reactor_work`, `power_pass`, `gun_blockers`, `plume::own_burn`, `Thruster::burns`,
`Piece::firepower`); the hash streams of all four determinism scenarios stayed byte-identical. Only the
steady-state heat solve and the turn estimate are the report's own, on the sim's constants and functions.

**What it holds** (blueprint frame, blueprint part indices; structured data, the client words it):

| Field | What |
|---|---|
| `pieces`, `main`, `falls_off` | the connected pieces (parts, `vessel` = has a logic core, `powered`), the main piece (the sim's rule: cored first, then most parts, then earlier), the parts not on it |
| `parts[i]` | stats after bonuses, open (venting) sides, steady-state heat and `heat / limit`, power level at full use, idle |
| `bonuses` | every bonus that applies (target, source, stat, effect), for the bonus overlay |
| `mass`, `centre_of_mass`, `inertia`, `hp`, `cargo`, `rule_slots` | main piece |
| `power` | supply (working reactors after bonuses), demand at full use, served, deficit, the consumer rows in allocation order with what each asks and gets |
| `heat` | peak `heat / limit` and its part, the parts above their limit (they would shut down) |
| `flight` | thrust and acceleration per direction (forward, backward, right, left), top speed per direction (GDR-0041), pure and differential torque, the planning `α` (first line), the 90° turn time, the yaw balance |
| `guns`, `firepower` | per gun: muzzle, facing, clear, blockers, range, dps, rate, family, delivery; Σ dps of the main piece's clear guns |
| `engines` | per engine: facing, thrust, its plume lanes (origin, exhaust direction, length, the own part it burns and how far it reaches) |
| `shields` | per emitter: centre and bubble radius after bonuses |
| `control` | load, capacity, the parts that idle |
| `build_points` | Σ part `build_points` × the material's `build_point_factor` for hull parts (`Content::ship_totals`, the same rule as `Content::part_stats`) |
| `device` | estimated native µs per tick, a class (`Light`, `Moderate`, `Heavy`, `TooHeavy`) and the cost drivers |
| `issues` | blockers: `NoLogicCore`, `CoresUnpowered`, `NoThrust`; warnings: `PowerDeficit`, `Overheat`, `GunBlocked`, `PlumeBurnsOwnPart`, `FallsOff`, `ControlOverCapacity` (part lists in each) |

**Assumptions.**

- *Capabilities at full power*: thrust, torque, top speeds, the turn and firepower are what the main piece's
  actuators and clear guns make at full power with bonuses, idle parts left out, as if the piece runs (a working,
  powered core). The power section says whether the reactors can feed it all at once; the client can scale by a
  part's level if it wants the brown-out view.
- *Full use* for power and heat: every consumer at full activity at once: engines and RCS pods at full thrust,
  gyros at full torque, guns with a clear line of fire firing, shields on, logic cores and other consumers on.
  Idle parts and blocked guns ask for nothing. It is the worst case.
- *Heat* is the steady state of the dynamic model at full use: per alive part `loss·T = out·work − Σ K·edges·(T −
  T_n)` (the fixed point of the tick in [Heat](#heat-dynamic-lumped-per-part)), with each part's work from that
  power pass (reactors `1/3 + 2/3 × served/supply`). The shutdown above a limit is not applied (those parts are
  flagged), and plume heat on own parts is left out (the burn is flagged). Solved with Jacobi-preconditioned
  conjugate gradients in f64 (gather-form rows, dot products in four fixed lanes: the same result everywhere),
  to a relative residual of 1e-7. Against 300 s of stepped ticks of a rig that runs everything flat out without
  moving (`steady_heat_matches_a_long_stepped_run`: opposed thrusters, lasers, a shield, a reactor between
  radiators, a capacitor) every part agrees within 3e-6 relative (the test allows 0.2%), and so do the power
  levels, exactly.
- Only the *main piece* is analysed (mass, power, heat, flight, control); other pieces fall off at spawn and
  report no heat and no power.
- *Turn time*: the flight computer's heading loop in one dimension, tick by tick, from rest to 90° within 1°
  (the agility test's criterion): each tick it asks `τ = I · (turn_rate(e, α_plan) − ω) / DT` with
  `Kin::plan_alpha`, gets at most the first line (pure torque plus the RCS couples of that sense,
  `ThrustAuthority::couple_ccw/cw`), runs the sim's last-resort gate (`TurnAssist`, `assist_after` ticks with
  torque missing) and, while it is on, adds the differential thrust of that sense and plans with it, and moves as
  Rapier integrates a constant torque over `S` substeps: `θ += ω·DT + (S+1)/(2S)·(τ/I)·DT²`, `ω += τ/I·DT`.
  Against the stepped agility turn (`tests/report.rs` `turn_estimate_matches_stepped_turns`), 13 Oct 2026:
  raider 0.567 s, runner 1.500 s, sample early 1.600 s, sample mid 3.400 s, all equal to the tick; Runner with 2
  RCS pods 1.133 s against 1.200 s stepped (two ticks; 1.167 s before the planning counted lasting translation), with 4 pods 0.967 s, equal (12 Oct: 0.400, 0.767, 0.900,
  2.033 s, all equal; without the substep term the raider came out 17% slow). It ignores what the velocity hold
  and the gimbals add.
- *Yaw balance*: `yaw` is the torque the forward thrust rows make at full thrust about the centre of mass
  (counter-clockwise positive); `yaw_hold` is what the flight computer can hold against it while flying straight:
  pure torque, the RCS couples that full forward thrust leaves free (right and left nozzles; the forward ones are
  busy pushing) and the forward engines' gimbals (the last resort, after a few ticks; differential thrust is
  only used while steering); `yaw_balanced = |yaw| ≤ yaw_hold`.
- The report's public fields did not change with GDR-0055: `torque_pure` is gyros and cores only, the RCS
  couples show in `alpha` (now the first line's: pure torque and couples, what a fresh ship plans with) and
  `turn_90`; `torque_ccw/cw` are the differential thrust beyond the couples.

**Base ships** (content of 13 Oct 2026, default config; only the turn changed since 12 Oct):

| Ship | Power supply / demand | Peak heat (full use) | Thrust → accel | Top speed | 90° turn | Firepower | Control | Device load (est / measured) |
|---|---|---|---|---|---|---|---|---|
| Automaton raider | 6 / 4 | 42% | 200 kN → 12.7 m/s² | 20 m/s | 0.57 s | 1.2 dps | 2 / 4 | 0.58 / 0.63 µs |
| Halvex Runner | 54 / 50 | **109%** (shield emitter; both beams and both emitters over) | 1100 kN → 10.3 m/s² | 22 m/s | 1.50 s | 8.3 dps | 18 / 32 | 1.21 / 1.15 µs |
| Sample early | 24 / 20 | 82% | 400 kN → 9.3 m/s² | 20 m/s | 1.60 s | 2.3 dps | 8 / 32 | 0.82 / 0.84 µs |
| Sample mid | 58 / 44 | 78% | 1100 kN → 12.5 m/s² | 22 m/s | 3.40 s | 8.3 dps | 16 / 32 | 1.07 / 1.11 µs |

The Runner overheats at full use: with both gyros, both beams and the shields running at once the emitters beside
the fusion reactor settle at 109% of their limit (consistent with [Heat](#heat-dynamic-lumped-per-part): "flat
out with the gyros working all the time the shield emitters overheat"). In combat it does not get there.

**Device load** (GDR-0006). The estimate is a cost model per step phase, fitted with `sim_perf --calibrate`
(10 copies of each design flying apart under pilot input with every gun firing, median step ÷ 10, split with
`Sim::step_profiled`; native release on the dev VM, 12 Oct 2026, three runs agreeing within 3%):

```
µs/tick ≈ 0.09 per ship + 0.0043 × parts (power, heat)  + 0.0045 × (thruster rows + torque sources) (flight)
        + 0.05 × guns + 0.27 × pieces + 0.018 × collider shapes (Rapier)
```

| Design | Parts | Flight / systems / weapons / physics µs | Measured | Estimate |
|---|---|---|---|---|
| Automaton raider | 9 | 0.07 / 0.06 / 0.06 / 0.44 | 0.63 | 0.58 |
| Halvex Runner | 56 | 0.09 / 0.26 / 0.17 / 0.63 | 1.15 | 1.21 |
| Sample early | 26 | 0.08 / 0.14 / 0.09 / 0.53 | 0.84 | 0.82 |
| Sample mid | 42 | 0.08 / 0.20 / 0.18 / 0.62 | 1.11 | 1.07 |
| Slab 12×10 (`sim_perf` layout) | 120 | 0.17 / 0.56 / 0.04 / 0.27 | 1.06 | 1.16 |
| Slab 16×12 | 192 | 0.20 / 0.86 / 0.06 / 0.26 | 1.40 | 1.56 |
| Slab 30×20 | 600 | 0.35 / 2.44 / 0.12 / 0.29 | 3.21 | 3.68 |
| Slab 50×40 | 2000 | 0.78 / 8.05 / 1.31 / 0.38 | 10.5 | 10.4 |
| Gunboat 50×40 (34 guns, 82 shield emitters) | 2000 | 0.77 / 8.1 / 9.2 / 0.39 | 18.8 | 11.4 |

Within 15% except the gunboat: its weapons phase is not the design's alone. Every shot segment walks the whole
bubble list of the world (`Sim::shield_pass`, friendly bubbles included until the owner check), so with 10
gunboats (~820 bubbles) a shot costs about five times what it costs among runners. Small ships are dominated by the Rapier body (~0.27 µs) and their
collider shapes; big ones by the power and heat passes (~4.3 ns per part). Classes (placeholders): `Light` up to
2.5 µs (a Runner is 1.2), `Moderate` up to 10, `Heavy` up to 40 (a 2000-part slab is 10.4), `TooHeavy` above.
The numbers are for this VM's native build; a device factor (phones are slower) is for later.

**Cost of the report** (`sim_perf --report`, native release, 600 runs (60 for the slabs), three runs at load
average 9.7 while other builds ran, medians of the run medians):

| Design | Parts | Report median / p95 | of which `ShipDesign::compile` |
|---|---|---|---|
| Automaton raider | 9 | 4.8 / 5.5 µs | 2.5 µs |
| Halvex Runner | 56 | 24 / 49 µs | 13 µs |
| Sample early / mid | 26 / 42 | 11 / 18 µs | 6 / 10 µs |
| Slab 50×40 | 2000 | 1.32 / 2.0 ms | 0.55 ms |

For the slab the rest is mostly `Ship::new` (~0.15 ms) and the heat solve (~0.6 ms). A diagonal incomplete
Cholesky preconditioner cut the CG iterations to a third (144 → 43 at 1e-9) but its triangular sweeps are
sequential chains and came out slower; Jacobi with gather-form rows and four-lane dot products was the fastest
of the three tried.

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

**New VM** (12 Oct 2026, late evening, `dev` `5b824b6`; AMD Ryzen 9 9950X3D, 16 vCPUs; the numbers above are from
the old VM, an Intel Core Ultra 7 155H), medians over 15 runs at load average 0.9–1.6, ms: raw throttles step
0.234 / 0.224 / 0.267 (mean / median / p95; Rapier 0.010 / 0.007 / 0.013), pilot + lock 0.250 / 0.240 / 0.293
(Rapier 0.010 / 0.008 / 0.017), combat 0.542 / 0.425 / 1.633 (Rapier 0.008 / 0.007 / 0.013); `state_hash` 0.285 ms
mean; lose one part 0.051 ms; cut a row and split 0.105 ms (Rapier 0.04 ms). That is about 1.8× faster than the
old VM idle on the same sim code (raw 0.398 / 0.645, pilot 0.435 / 0.747, combat 0.749 / 2.507 median / p95, see
[Engine plumes](#engine-plumes)). During a 6-job cargo build (load 9–11, 10 runs) the medians rose by 9–15 %.

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

**New VM** (12 Oct 2026, late evening, `dev` `5b824b6`; AMD Ryzen 9 9950X3D, 16 vCPUs; the table above is the old
VM, an Intel Core Ultra 7 155H, and an older fight), medians over 15 runs at load average 0.9–1.6, ms:

| Ships | Step mean / median / p95 | Rapier step median / p95 | Impacts / rams | Shots | Hits | Splits | Active at end |
|---|---|---|---|---|---|---|---|
| 1 | 0.003 / 0.002 / 0.003 | 0.002 / 0.002 | 0 / 0 | 552 | 0 | 0 | 1 |
| 5 | 0.004 / 0.003 / 0.007 | 0.002 / 0.004 | 11 / 0 | 29 | 166 | 0 | 3 |
| 20 | 0.015 / 0.011 / 0.028 | 0.006 / 0.015 | 175 / 1 | 202 | 839 | 3 | 10 |

The 20-ship fight is the one measured on the idle old VM in [Engine plumes](#engine-plumes) (0.019 / 0.048 median /
p95, Rapier 0.009 / 0.024): about 40 % less per tick. During a 6-job cargo build (load 9–11, 10 runs): 0.012 /
0.029 (Rapier 0.007 / 0.016). Final state hash with 20 ships `a884134ca1f6bfeb`. Browser comparison and the
spike's answers: `docs/results/spike-summary.md`.

## Design questions for Niklas

Answered (provisionally, lead session 11 Oct 2026; easy to change, see the sections above): a core needs power;
chain reactions later via content; heat per part. Split-off cored pieces were salvage until the vessel model
(GDR-0013, 12 Oct 2026); now they become vessels of their own ([Vessels](#vessels)). The fixed brown-out order was
replaced by Niklas's tiers and weights (GDR-0014, implemented 12 Oct 2026). Turning: decided by Niklas (cores,
gyros, RCS, gimbals; GDR-0010), refined on 13 Oct 2026 (GDR-0055: RCS pods are the strong turners, differential
thrust and gimbals only as the last resort). Halvex Runner layout: fixed in content.
Weapons (lead session, 11 Oct 2026, defaults Niklas may change): shields keep per-family shares with a collapse
until 30% recharged; a penetrating shot stops at the hull; raiders lead their shots (done, plus `AimMode` for
the player's lock). Fight length: Niklas tunes speed and ranges in the browser with live sliders.

Open:

1. **Linear speed feels slow.** **Answered by Niklas on 12 Oct 2026**: top speed from each engine's rated speed
   (GDR-0041) and the agility target "Agile" (GDR-0042), reached with content numbers; see
   [Turning and speed of the base ships](#turning-and-speed-of-the-base-ships-content-of-13-oct-2026-gdr-0055-scales-1).
2. **Braking and strafing under lock.** **Answered by Niklas on 11 Oct 2026** (GDR-0011): (a), accept it; RCS
   pods or side and front thrusters are the answer (a drydock choice). The flight computer never breaks a lock on
   its own.
3. **Feel of "fly that way".** Forward-only ships point their thrust along the needed change of velocity, so
   a hard turn swings the nose past the stick before it settles (a "drift turn"). Alternative: keep the nose on
   the stick and only use the thrust that happens to help (slower, sideways drift remains). Recommend (a)
   **current behaviour**; judge in the browser.
4. **`raider_basic` numbers** (engage 90 m, stand-off 64 m, orbit 50 m, fire range 70 m, cone 0.12 rad;
   stand-off 50 m and orbit 45 m until 12 Oct) are placeholders. At the new speeds a wave of three is over 4–11 s
   after the first raider shot and costs the Runner 2–8 of 72 hp (measured 12 Oct, before the slower turning of GDR-0055); a pilot who never moves loses about one wave
   in four ([Sandbox fights](#sandbox-fights-at-the-new-speeds)). The outcome flips around the Runner's beam
   range (45 m); tune in the browser with the weapon range and damage sliders.
5. **Missiles, disruption, drones** are not built yet (`absorb` already lists missiles: bubbles stop them).

Rated speed (GDR-0041, 12 Oct 2026):

14. **Diagonal top speed.** **Answered by Niklas on 13 Oct 2026** (GDR-0055): (b), a thruster pushing with the
    motion fades by the ship's whole speed. Done 13 Oct 2026, see
    [Top speed and the cruise flame](#top-speed-and-the-cruise-flame).
15. **Differential thrust at speed.** **Answered by Niklas on 13 Oct 2026** (GDR-0055): (b), differential thrust
    (and gimbals) only as the last resort, after gyros, cores and RCS have been at their limit for a few ticks.
    Done 13 Oct 2026, see [The last resort](#the-last-resort).

Engine plumes ([GDR-0049](gdr/0049-engine-plume-v1.md), lead defaults of 12 Oct 2026 for Niklas to review):

16. **Plume numbers.** Thruster 3 m, 1 damage/s, 1.5 heat/s; torch drive 6 m, 3 damage/s, 4 heat/s: a torch drive
    held on a raider for a second wrecks a part or two, a buried thruster burns through an armour block in 1.4 s.
    Tune in content.
17. **Does a plume push?** **Answered by Niklas on 13 Oct 2026** (GDR-0055): (a), no push, as built.
18. **Drawn flame vs reach.** **Answered by Niklas on 13 Oct 2026** (GDR-0055): (a), the full reach drawn faintly
    only while the plume burns something, as built.

Behaviors (asked 11 Oct 2026; **answered by Niklas on 12 Oct**, GDR-0034 to GDR-0037 accepted):

6. Fire at will with side and rear guns: **per gun facing** (done).
7. `RaiderBrain`: **deleted** after the client and the load test switched to behaviors.
8. One target line per behavior: **kept**.
9. No `or`: **confirmed** (two rules with the same action).
10. Unpiloted ship-mind vessels default to `skirmisher`: **confirmed**. Rule slots (6 per logic core, 2 per
    automaton core, lowest rules drop first): **confirmed**.

Physics (Rapier integration, 12 Oct 2026; defaults in [Collisions](#collisions-impacts-and-ramming-v1)):

11. **Ramming numbers.** **Answered by Niklas (12 Oct 2026, GDR-0047)**: `max(0, impulse − 100 kN·s) × 0.02`
    hp per side (placeholders), crushing inward from the touching part past the parts it destroys; bumps below
    100 kN·s are free. Done (see [Collisions](#collisions-impacts-and-ramming-v1)).
12. **Do shields stop rams?** **Answered by Niklas (12 Oct 2026, GDR-0045)**: bubbles absorb ram damage at
    the contact point first (the kinetic share), the rest hits the hull; the collision itself is unchanged.
    Done (see [Vessels](#ramming-between-vessels-gdr-0045)).
13. **Friendly collisions.** **Answered by Niklas (12 Oct 2026, GDR-0045)**: vessels of one owner collide
    physically but deal no ram damage. Done. A separation term in the flight computer, so own vessels avoid
    each other, is still a good idea later.
