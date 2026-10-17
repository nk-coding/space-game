# Simulation core (`crates/core/sim`)

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

**Salvage v1 on 13 Oct 2026** (GDR-0063, track 7B): cargo per ship, collecting by flying over wrecks, core fragments
and wrecks spawned as wrecks. See [Salvage](#salvage).

**Voyages on 13 Oct 2026** (GDR-0062, ADR-0033, track 7E): the voyage director rolls a content voyage's sites from a
seed and spawns, wakes and clears them around the player's vessels; the recall check; a `voyage` determinism scenario
and the headless voyage runner. See [Voyages](#voyages).

**The Grown's mechanics on 17 Oct 2026** (GDR-0099, lead defaults in GDR-0102): hearts re-grow lost parts, living
parts heal, ram prows scale the ram damage, wary sites wake only when provoked (and every site when its wreck is
stripped), a hot star heats every part open to space; a `grown` determinism scenario. See
[Living parts](#living-parts-gdr-0099-gdr-0102), [Hot stars](#hot-stars-gdr-0102) and [Running it](#running-it).

**The Grown in play on 18 Oct 2026** (GDR-0103): the `ram` behavior action (`Maneuver::Ram`); the headless voyage
runner opens fire on a wary site it heads for (it never wakes on sight); the test range's waves leave out wary
factions' ships; the scenarios that spawn every blueprint leave out the outer reach's (`scenario::OUTER_TAG`), so
they run as before.

No Bevy, no graphics, no threads, no filesystem, no wall-clock. Builds for `wasm32-unknown-unknown` and
`wasm32-wasip1`. Dependencies: `data` (content), `libm` (pinned `=0.2.16`, `arch` feature off), `rhai`
(pinned `=1.26.1`, sandboxed; see [Rhai conditions](#rhai-conditions)) and `rapier2d` (pinned `=0.36.1`,
`enhanced-determinism`, default features off, single thread).

## Module map

Feature folders (ADR-0038, 14 Oct 2026): `ship/` (the ship and its parts), `flight/` (the flight computer,
maneuvers, physics), `combat/` (weapons, plumes, chain reactions, salvage), `behavior/`, `voyage/` (the director,
the runner, the scenarios); at the root what everything uses.

| Module | What it holds |
|---|---|
| `math` | `Vec2`, `Rot`, and the only allowed transcendental functions (`sin`, `cos`, `atan2`, `exp` via libm) |
| `rng` | `Pcg32` (reference PCG32), `splitmix64` (from `data::hash`), `Stream` (one per system), `RngStreams` |
| `hash` | `StateHasher` (FNV-1a 64 over 32-bit words, `str` for content ids), `StateHash` trait, `fnv1a64` (from `data::hash`, where both save-format hashes are defined once and pinned) |
| `ids` | `ShipId`, `PieceId` (monotonic `u32`, never reused), `IdAlloc` |
| `config` | `SimConfig` (thrust/torque/speed scales, split ghost ticks, `FlightTuning`, `PhysicsTuning`, mind capacity per owner, `SalvageTuning`, `VoyageTuning`) |
| `ship::control` | `ControlState`, the per-piece control capacity and idle pass (GDR-0044) |
| `ship::design` | `ShipDesign::compile(content, blueprint)`: the blueprint compiled once (part specs, links, bonus links, gun mounts and their blockers, engine mounts with their plume lanes and paths), shared by `Arc` |
| `ship::mass` | `MassProps`, `polygon_mass`, `combine`, `to_piece` (blueprint → piece frame) |
| `ship::collider` | `ConvexPolygon`, greedy rectangle merge + slope triangles |
| `ship` | (`piece.rs`: a piece, its thrusters and guns; `caches.rs`: rebuilding a piece's rows; `systems.rs`: the per-tick systems) `Ship`, `Piece`, `PartState`, `Body`, `ShipControls`, systems (`update_systems`), `Thruster`, `GunMount`, `Wrench`, `ThrustAuthority` (what the actuators that can produce do now), `producing`, `gun_works`, `rated_falloff`, `inherited_velocity`, heat and falloff `tuning` constants |
| `ship::power` | power allocation by tier and weight (`allocate`), `PowerSetting`, `Consumer` rows, `Ship::set_power_setting` |
| `report` | the static design report for the drydock (`design_report`, `DesignReport`, `Issue`), see [Design report (drydock)](#design-report-drydock) |
| `ship::alloc` | control allocation: `AllocTable`, `Actuation`, `TorqueSource`, `allocate` (first line; `translate` is its step 1), `couples` (RCS couples), `lasting` (translation that lasts beyond the tick), `assist` (last resort), `wrench_of` |
| `flight` | (`pilot.rs`: the pilot's input and goal; `aim.rs`: leads and smart aim; `controllers.rs`: the plan into actuator commands) flight computer: `PilotInput`, `Target`, `FlightGoal`, `HeadingGoal`, `HelmMode`, `Kin`, `FlightTelemetry`, `TurnAssist`, `nudge_step`, `turn_ramp_step`, `next_lock` (LOCK TARGET's order); `stand_in.rs`: the evolution lab's stand-in pilot |
| `flight::maneuver` | `Maneuver` (Brake, Hold, FaceTarget, Approach, Orbit, KeepRange, Flee, Escort), goals capped at the top speed |
| `behavior` | `BehaviorLibrary`, `BehaviorId`, `BehaviorState` (debug view), `RuleStatus`, `Why`, `BUILTIN_NODES`, `check_vocabulary`; `compile.rs` (program → flat code), `eval.rs` (think pass, fly), `script.rs` (Rhai host), `assist.rs` (pilot assists: `AssistsId`, `Assists`, `AssistState`, `AssistSwitch`, `Channel`) |
| `combat::weapons` | beams, pulses, projectiles (`Projectile`, `ProjectileId`), shields (`Bubble`, `absorbs_from`: which bubbles take damage from a source), no friendly fire, `BeamTrace`, `ShotKind`, `GoneReason`, weapon `tuning` |
| `flight::physics` | `Physics` (Rapier2D: one body per piece, compound colliders, ghost filter, ray casts, contacts), `BodyKey`, `RayHit`, `Contact`, `wrap_angle` |
| `combat::plume` | engine exhaust plumes (crate-private): lanes, the own-hull path, ray casts against other bodies, shields, `SimEvent::PlumeHit` |
| `ship::living` | living parts (GDR-0099, GDR-0102): `Ship::heal`, the hearts' timers (`Ship::regrowth`), the regrowth candidates and `Ship::regrow` (the reverse of `remove_part`), `Sim::living` (the pass, asking the physics for overlaps) |
| `ship::breakage` | `Ship::apply_damage`, `Ship::remove_part` (the destroy path without the destruction), `Ship::resolve_breakage`, `DamageTarget`, `DamageOutcome` |
| `combat::chain` | chain reactions (GDR-0084, GDR-0085): the world's blast queue, the reach scan of other pieces and ships, armour cover (`cover`, `in_reach`, shared with the design's compiled lists), shields (`BLAST_FAMILY`) |
| `combat::salvage` | `Item`, `Cargo`, `Stack`, `Loose`, `LooseId`, `Salvaged`; the collect pass, fragments, `Sim::spawn_wreck`, `Sim::loose` (see [Salvage](#salvage)) |
| `voyage` | `VoyagePlan` (`roll`), `SitePlan`, `Spawn`, `Voyage`, `Site`, `SiteState`, `VoyageTuning`, `PLAN_STREAM`; `Sim::start_voyage`, `voyage`, `can_recall`, `fleet_disarmed`, `awake_enemy`, `armed_enemy` and the director's pass (see [Voyages](#voyages)) |
| `voyage::runner` | the headless voyage runner's scripted pilot as a library: `VoyageRunner` (steppable), `Flagship`, `VoyageRun`, `closing_reach` (see [The headless voyage runner](#the-headless-voyage-runner)) |
| `events` | `SimEvent`, `SplitEvent`, `PieceSnapshot` |
| `ship::vessel` | `Owner`, `friendly`, `VesselState`, `VesselInfo`; the vessel split, focus and mind pass (`Sim::focus`, `vessels`, `mind_load`) |
| `world` | `Sim` (tick, config, RNG, ships, physics, events, impacts and ramming, state hash), `TickInputs`, `ShipCommand`, `DamageCommand`, `PartHit`, `StepPhase` |
| `voyage::scenario` | (`salvage.rs`, `voyage.rs`, `grown.rs`: those three scenarios; `fixtures.rs`: the behaviors and ships they add) the deterministic `core`, `flight`, `load`, `behavior`, `salvage` and `voyage` scenarios |

Examples: `determinism` (prints per-tick hashes; used by `scripts/determinism-check.sh`), `sim_perf` (load
numbers, Rapier vs the rest; `--report` times the design report, `--calibrate` measures its device-load model),
`load_test` (also Rapier vs the rest), `behavior_perf` (behavior cost), `voyage` (the headless voyage runner, the
balance tool of track 7K: [Voyages](#the-headless-voyage-runner)), `cluster` (a 2–3 km cluster's cost, `f32` far from
the origin, spawn hitches: [Bigger clusters](#bigger-clusters)).

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
   maneuver, goal, behavior, leader, pilot assists and their switches, power setting, focus, wake), scripted `damage`. No NaN or infinity from
   outside reaches the state (13 Oct 2026 review fix): a pilot input, maneuver or goal with one is ignored, a raw
   throttle counts it as 0, damage ignores it; so do `spawn_ship`, `teleport` and `spawn_projectile` (as 0),
   and `with_config`, `set_config` and `design_report` replace it with the default
   (`SimConfig::finite_or_default`). Test `non_finite_inputs_never_reach_the_state`.
3. `resolve_breakage`: each damaged piece is processed once (connectivity, rebuild, splits) → events; bodies
   are updated, created and removed in the physics, and split pieces ghost each other for a few ticks. Cored
   pieces of a split become vessels of their own, and the mind pass runs if anything changed (see
   [Vessels](#vessels)).
4. **Behaviors think**: every ship with a behavior thinks on the same ticks (every 3; not staggered by id,
   ADR-0037) and picks its target and rule from one snapshot of the world; the decisions are applied afterwards
   (see [Behaviors](#behaviors)). Then the **pilot assists**: a piloted ship with assists notes which controls
   its pilot uses and, on the same ticks, decides which assist takes each channel ([Pilot assists](#pilot-assists)).
5. **Flight**: every ship is planned from the same snapshot of the world (behaviors, maneuvers, pilot input →
   `FlightGoal` and what the guns do), then the flight computer allocates the actuators of each controlling
   piece and fire at will picks the guns that fire (per gun facing).
6. **Systems** per ship: per piece the control pass (which parts idle), power, shield regeneration, the wrench
   (engines, RCS, gimbals, gyros, cores) and gun output; then heat (ship-wide; a hot star's heat through the open
   sides, [Hot stars](#hot-stars-gdr-0102)); then each piece's authority (what can push and turn, for the next plan).
   Then the **living pass** (`Sim::living`): tissue heals, hearts that are due re-grow a part, and a ship that grew
   is resolved at once (rebuilt pieces and bodies before the weapons fire; [Living parts](#living-parts-gdr-0099-gdr-0102)).
7. **Weapons**: shield bubbles are collected, live projectiles fly (ray-cast segment), then every gun fires
   **at once** (beams and pulses are traced against the state at the start of the fire pass and their hits
   applied after the last gun, so fire is mutual: no ship shoots an enemy's gun off before it fires in the same
   tick, ADR-0037; projectiles spawn, recoil adds to the wrench), then every firing engine's plume burns the
   first part in its path ([Engine plumes](#engine-plumes)); breakage is resolved again. Every
   `SalvageTuning::interval` ticks collecting vessels take salvage, and what they stripped breaks up
   ([Salvage](#salvage)).
8. **Physics**: expired ghosts are cleared, this tick's wrenches replace the last ones (force rotated to world,
   at the centre of mass), Rapier steps `DT` (4 substeps); loose fragments drift.
9. **Impacts**: contacts of the step become `SimEvent::Impact` (closing speed from the velocities before the
   step), bodies are read back (the previous pose is kept for interpolation), impacts above the ramming
   threshold crush inward from the touching part on both sides (after shields; none within one owner), and
   breakage is resolved once more.
10. **Voyage director** (if a voyage is underway, [Voyages](#voyages)): sites near the player's vessels spawn,
    sleeping sites wake, awake sites clear.
11. `tick += 1`; debug builds store `state_hash()` (`last_tick_hash()`).

`Sim::step_profiled(&inputs, |phase| ...)` is the same step with a callback at the start of every `StepPhase`
(`Inputs`, `Flight`, `Systems`, `Weapons`, `Physics`, `Impacts`, `Hash`, `Done`), so tools can time the phases
with their own clock (`load_test` and `sim_perf` report the Rapier step separately).

## Determinism

The rules (also in `src/math.rs`, enforced where possible):

- Fixed step; no wall-clock anywhere. `Sim::time_seconds()` is display-only.
- **RNG**: in-crate PCG32 (checked against the reference vector of pcg-random.org), so a dependency update can
  never change the numbers. One stream per system (`Stream::Damage`, `Weapons`, `Debris`, `Ai`, `Scenario`,
  `Encounter`: reserved for encounter rolls during a voyage, unused so far; the voyage plan and wreck damage use
  generators of their own, [Voyages](#rolling-a-plan) and [Salvage](#wrecks-spawned-as-wrecks)),
  seeded with `splitmix64(world_seed ^ splitmix64(stream_id))` and using `stream_id` as the PCG sequence.
  Stream ids are part of the save/replay format: append only, never renumber. (Behaviors use no
  randomness.)
- **Math**: `f32` everywhere. `+ − × ÷`, `sqrt`, `abs`, `floor`, `min/max`, comparisons and casts are exact
  IEEE operations. Transcendentals go through `sim::math` (libm, pure Rust). `crates/core/sim/clippy.toml` bans
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
  the last-resort turning state (`Piece::assist`), and per ship the burned-out flag, the hull base (GDR-0064) and
  the cargo; then every projectile; then
  the loose salvage items; then the voyage, if one is underway ([Voyages](#state-hash-1)); then Rapier's own state
  per body in key
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
- `scripts/determinism-check.sh [--seed N] [--ticks N] [--scenario core|flight|assists|load|behavior|salvage|voyage|grown]` builds the `determinism`
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

**With the 13 Oct 2026 review fixes** (release builds, 1800 ticks): core, flight, load (20 ships) and behavior
with seed 42, flight and load with seed 7: native x86_64 vs wasm32-wasip1 under Node 24 **identical** (6 runs).
Against the `dev` build before the fixes (`4f4d68b`), seed 42: core differs only in ticks 908–918 (a ram split now
ghosts one step longer; the state converges again); flight from tick 326 (the same ghost fix, and from tick 1501
the fight itself; counters unchanged); load from tick 261 (ghost) and 285 (a shot that hits a block destroyed
earlier in the tick now reaches the part behind it: 681 → 704 hits, 77 → 80 parts destroyed, 133 → 125 shots);
behavior from tick 601 (idle guns no longer count for reach and firepower, slots only from working cores: 231 →
183 shots, 125 → 122 parts destroyed, 896 → 1123 Rhai evaluations). The other fixes (keys and the last resort, NaN
inputs, the shot cap, the main-piece rule, the per-piece heat pass) leave all four streams bit-identical. Counters
with seed 42:

| Scenario | Parts destroyed | Splits | Shots | Hits | Shield hits | Impacts / rams | Burns | Overheated part-ticks |
|---|---|---|---|---|---|---|---|---|
| core | 271 | 48 | 603 | 35 | 0 | 22 / 2 | 244 | 124 |
| flight | 137 | 22 | 946 | 90 | 2 | 49 / 5 | 0 | 0 |
| load (20) | 80 | 3 | 125 | 704 | 18 | 140 / 2 | 9 | 0 |
| behavior | 122 | 18 | 183 | 123 | 8 | 119 / 0 | 14 | 0 |

**With the fight balance and the flight fixes of 13 Oct 2026** (GDR-0061 content, GDR-0064 feed-forward, goal cap,
continuous fade, turrets and split-off hull share, GDR-0065 `escort`, GDR-0067 friendly bubbles; release builds,
1800 ticks): core, flight, load (20 ships), behavior and salvage, seeds 42 and 7: native x86_64 vs wasm32-wasip1
under Node 24 **identical** (10 runs). The hash streams changed once (new hashed state: `HeadingGoal::Track`, the
ship's hull base, `Maneuver::Escort`; and the fights). The feed-forward uses one cross product and one division,
the fade one multiplication and a `min`. Counters with seed 42 (collected = salvage items):

| Scenario | Parts destroyed | Splits | Shots | Hits | Shield hits | Impacts / rams | Burns | Overheated part-ticks |
|---|---|---|---|---|---|---|---|---|
| core | 311 | 60 | 823 | 38 | 0 | 209 / 1 | 221 | 248 |
| flight | 135 | 29 | 1547 | 126 | 11 | 19 / 2 | 50 | 0 |
| load (20) | 92 | 5 | 138 | 787 | 13 | 104 / 2 | 23 | 0 |
| behavior | 123 | 22 | 431 | 84 | 5 | 106 / 0 | 12 | 0 |
| salvage | 85 | 26 | 15 | 13 | 15 | 196 / 12 | 132 | 0 |

**With simultaneous fire, shared think ticks and auto orbits by both ids** (ADR-0037, 14 Oct 2026, release builds,
1800 ticks): core, flight, load (20 ships), behavior, salvage and voyage, seeds 42 and 7: native x86_64 vs
wasm32-wasip1 under Node 24 **identical** (12 runs). Every stream differs from the last recorded ones (the fights
changed; no new state is hashed, the weapons roll nothing). Last ticks, seed 42 / 7: core `10817d0eb6e4971b` /
`0536f58db7b5841e`, flight `51b15d9f74e6ddc6` / `c0a61a9d5fd02065`, load `40715855bc7f3a44` / `32e271f92923a209`,
behavior `93f2a77c360d022d` / `1bcaed90dd071046`, salvage `fae5854d5ad5af30` / `4ff6be9d0e5cceb0`, voyage
`c426696903caa92a` / `1403ed03ee22045d`. Counters with seed 42 (voyage: 2 sites woken and cleared, recall 1755 ticks):

| Scenario | Parts destroyed | Splits | Shots | Hits | Shield hits | Impacts / rams | Burns | Overheated part-ticks |
|---|---|---|---|---|---|---|---|---|
| core | 418 | 70 | 1290 | 141 | 7 | 36 / 3 | 265 | 248 |
| flight | 228 | 32 | 1013 | 478 | 77 | 160 / 2 | 50 | 0 |
| load (20) | 83 | 2 | 98 | 867 | 14 | 142 / 0 | 0 | 0 |
| behavior | 188 | 23 | 239 | 197 | 77 | 32 / 2 | 57 | 0 |
| salvage | 94 | 31 | 13 | 11 | 9 | 317 / 13 | 65 | 0 |
| voyage | 48 | 6 | 40 | 0 | 0 | 682 / 0 | 0 | 0 |

**With chain reactions** (GDR-0084, 14 Oct 2026, release builds, 1800 ticks): core, flight, load (20 ships), behavior,
salvage and voyage, seeds 42 and 7: native x86_64 vs wasm32-wasip1 under Node 24 **identical** (12 runs). Every stream
differs from the ADR-0037 ones (blasts destroy more parts; no new state is hashed: the chain resolves within the
breakage and the target lists are part of the design). The `determinism` example now counts `explosions`
(`SimEvent::Explosion`). Last ticks, seed 42 / 7: core `04ce4f42d8de5acf` / `73268639788b929a`, flight
`4bc8860035bb02f2` / `941ab112a90c9970`, load `6453618e3f9be298` / `86aa82a090628845`, behavior `c089c009baa0f1aa` /
`090c65cf387057c3`, salvage `c79c3ec6c24dd137` / `d59fdca46779669e`, voyage `c54f1d7317fa892d` / `18e3fa300f587056`.
Counters with seed 42 (voyage: 2 sites woken and cleared, recall 1755 ticks):

| Scenario | Parts destroyed | Splits | Shots | Hits | Shield hits | Impacts / rams | Burns | Explosions | Overheated part-ticks |
|---|---|---|---|---|---|---|---|---|---|
| core | 433 | 67 | 1132 | 114 | 6 | 46 / 5 | 227 | 71 | 248 |
| flight | 282 | 21 | 960 | 484 | 84 | 99 / 1 | 40 | 52 | 0 |
| load (20) | 78 | 2 | 94 | 818 | 12 | 163 / 1 | 0 | 5 | 0 |
| behavior | 220 | 25 | 261 | 155 | 78 | 47 / 8 | 81 | 35 | 0 |
| salvage | 104 | 29 | 13 | 11 | 8 | 398 / 11 | 124 | 14 | 0 |
| voyage | 53 | 4 | 44 | 0 | 0 | 634 / 0 | 0 | 5 | 0 |

**With chain reactions v2** ([GDR-0085](gdr/0085-chain-reactions-v2.md), 14 Oct 2026, release builds, 1800 ticks: blasts
reach other pieces and ships, armour on the line shields, reactors reach two cells deep, cores, emitters and RCS pods
explode): core, flight, load (20 ships), behavior, salvage and voyage, seeds 42 and 7: native x86_64 vs wasm32-wasip1
under Node 24 **identical** (12 runs). Every stream differs from the GDR-0084 ones (more parts destroyed; no new state
is hashed: the chain resolves within the breakage, the cover lists are part of the design and the reach scan reads the
pieces' poses). Last ticks, seed 42 / 7: core `64b1b7a21ae3be76` / `685dde0be1258913`, flight `730ecc5c4e7ea76f` /
`fc4fc75c2f6dd69a`, load `4a5901d79902bbdf` / `bb081c57ebd25ae0`, behavior `b35d7818ee707466` / `f9384c380d9bb1ab`,
salvage `9216af47e53a36f6` / `6084ef2acac986d8`, voyage `a2397a3937187f33` / `21ae3b24b14dcf82`. Counters with seed 42
(voyage: 3 sites woken and cleared, recall 1673 ticks):

| Scenario | Parts destroyed | Splits | Shots | Hits | Shield hits | Impacts / rams | Burns | Explosions | Overheated part-ticks |
|---|---|---|---|---|---|---|---|---|---|
| core | 433 | 57 | 939 | 110 | 7 | 79 / 5 | 214 | 98 | 248 |
| flight | 289 | 25 | 872 | 369 | 58 | 137 / 8 | 49 | 70 | 0 |
| load (20) | 101 | 0 | 98 | 757 | 16 | 124 / 1 | 0 | 21 | 0 |
| behavior | 263 | 27 | 269 | 342 | 117 | 27 / 3 | 73 | 56 | 0 |
| salvage | 125 | 31 | 13 | 11 | 8 | 165 / 8 | 64 | 28 | 0 |
| voyage | 84 | 6 | 89 | 0 | 0 | 562 / 0 | 0 | 17 | 0 |

**Letting go coasts** ([GDR-0086](gdr/0086-letting-go-coasts.md), 14 Oct 2026, release builds, 1800 ticks: a released
stick or released keys no longer brake): core, flight, load (20 ships), behavior, salvage and voyage, seeds 42 and 7:
native x86_64 vs wasm32-wasip1 **identical** (12 runs). Only the streams with a released player input change: flight
`742c08eaaa2fabdb` / `051ce217c6f12e12`, behavior `a2589d01b5eff9b7` / `2df82abadfeb7fcc` (last ticks, seed 42 / 7);
core, load, salvage and voyage are unchanged from chain reactions v2.

**Weapon groups** ([GDR-0088](gdr/0088-pilot-assists-weapon-groups-and-key-bindings.md) step 2, 15 Oct 2026, release
builds, seed 42, 900 ticks: `PilotInput::fire` became the weapon-group mask): flight, behavior and voyage, and the
arena fight `runner_vs_raiders`: native x86_64 vs wasm32-wasip1 **identical**. The flight, behavior and voyage
streams change, from the first tick a scenario pilot fires, only because the helm hashes the pilot's mask (7 for
every group) where it hashed a bool (1): hashed as the bool, all seven streams (core, flight, behavior, salvage, voyage,
`runner_vs_raiders`, `skiff_mirror`) stay byte-identical to the build before, so no ship flies or fires differently.
Last ticks: flight `2a51382797b4736b`, behavior `1b84c5d19d5d393d`, voyage `58c3b65781708409`; core, salvage and both
arena fights unchanged.

**Pilot assists** ([GDR-0088](gdr/0088-pilot-assists-weapon-groups-and-key-bindings.md) step 3, 15 Oct 2026, release
builds, seed 42, 900 ticks; `runner_vs_raiders` to its end at 1273): core, flight, behavior, salvage, voyage and
`runner_vs_raiders`: native x86_64 vs wasm32-wasip1 **identical**, and every stream byte-identical to the build
before: no scenario ship has assists, the assist state is hashed only for a ship that has some, and the behavior
compile was only reorganised. Last ticks: core `04c38a80a753f50f`, flight `2a51382797b4736b`, behavior
`1b84c5d19d5d393d`, salvage `d53cf56822803318`, voyage `58c3b65781708409`, `runner_vs_raiders` `8d5b00d21e45e771`.
No determinism scenario flies with assists yet (`tests/assists.rs` covers them natively).

**Hold station and avoid collisions** ([GDR-0088](gdr/0088-pilot-assists-weapon-groups-and-key-bindings.md) step 4,
15 Oct 2026, release builds, seed 42, 900 ticks; `runner_vs_raiders` to its end at 1273): core, flight, behavior,
salvage, voyage and `runner_vs_raiders`: native x86_64 vs wasm32-wasip1 **identical**, and every stream byte-identical
to the build before (no AI ship has assists; the new assist state is hashed only for ships with assists). The new
**assists** scenario (`ScenarioKind::Assists`, `--scenario assists`: the flight scenario with every pilot assist on the
player's ships, `scenario::fixtures::ASSISTS`; the pilots let go more often (50%) and fire less (30%) so the assists get
channels, and 15% of the rerolls flip a toggle slot or ALL ASSISTS) is **identical** too, last tick
`ff0c7503bceb7356`; over the run a station is held for 2478 ship-ticks, something is dodged for 465, and every
rule decides at times (`assists_scenario_is_deterministic_and_the_assists_act`).
With the thrust assists turning while turning is free (same day, after step 4): the six streams above again
byte-identical and native = wasm32-wasip1; only `assists` changes (no AI ship has assists), last tick
`a8562be7b6064c01`, identical.

**The assists act on the lock only** ([GDR-0088](gdr/0088-pilot-assists-weapon-groups-and-key-bindings.md), dated
line of 15 Oct 2026: no target line, `face_target` removed, `auto_lock` and its pause, LOCK TARGET's order in the sim;
release builds, seed 42, 900 ticks; `runner_vs_raiders` to its end at 1273): core `04c38a80a753f50f`, flight
`2a51382797b4736b`, behavior `1b84c5d19d5d393d`, salvage `d53cf56822803318`, voyage `58c3b65781708409` and
`runner_vs_raiders` `8d5b00d21e45e771`: native x86_64 vs wasm32-wasip1 **identical** and byte-identical to the build
before (no AI ship has assists; the flight scenario's pilots draw their locks as before). `assists` changes, last tick
`ab49d109c0b68d3f`, identical: its list swaps face target for two auto-locks (`attacker` above `nearest`, on toggle 3),
its pilots keep their lock as a player does (each reroll: 1 in 8 drops it, Esc, which pauses auto-lock; 1 in 8 presses
LOCK TARGET, `flight::next_lock`; else the lock held, or with none the one auto-lock asks for, as the client locks it),
and the assist state hashes the lock, the pause and the ask instead of the target line's aim part; the test
`assists_scenario_is_deterministic_and_the_assists_act` checks that auto-lock asks and pauses at times and that every
rule decides.

**Fight codes and the rules probes** ([GDR-0092](gdr/0092-evolution-lab-for-your-own-ships.md) step 1, 15 Oct 2026,
release builds; `docs/arena.md` "Fight codes"): no sim change, and every stream above byte-identical (core, flight,
behavior, salvage, voyage, assists at seed 42 and 900 ticks, `runner_vs_raiders` to its end at 1273), native x86_64 vs
wasm32-wasip1 **identical**. A fight is re-watched by simulating its spec again, so a fight code (`SGF1:…`) is checked
the same way: `scripts/determinism-check.sh --fight SGF1:…` runs the code's fight native and in wasm to its end and
says whether it ends as recorded. Three codes printed by the native CLI: the Runner against three raiders (seed 3, won
at 1711 ticks, `afa541fe50b16945`), an automaton hunter against two raiders (seed 3, won at 830, `920793be9270f63e`)
and an evolved skiff from `arena evolve` against two raiders (won at 641, `09b55f03c07161da`): **identical**, and both
sides `as recorded`, with today's rules fingerprint equal in wasm (`rules=same`). The two rules probes
(`crates/core/arena/probes/`, pinned here like the streams; `--fight close_quarters --seed 1`, `--fight warden --seed
2`): `close_quarters` 300 ticks `dcd603e2f48f8a88`, `warden` 300 ticks `bcda68d2360256f4`, **identical**; the base
pack's rules fingerprint with them is `87f8e44db8186359`. The browser's fight viewer replays the same codes to the
same final hashes (`docs/client.md` "Fight viewer"). A change to the sim changes the probes' hashes and with them
every code's fingerprint: write the new values here.

**Runs kept and resumed** ([GDR-0092](gdr/0092-evolution-lab-for-your-own-ships.md) steps 2 and 3, 15 Oct 2026, release
builds; `docs/arena.md` "What a run keeps"): no sim change; core `04c38a80a753f50f`, flight `2a51382797b4736b`, behavior
`1b84c5d19d5d393d`, salvage `d53cf56822803318`, voyage `58c3b65781708409`, assists `ab49d109c0b68d3f` (seed 42, 900
ticks), `runner_vs_raiders` `8d5b00d21e45e771` (to its end at 1273), the probes `close_quarters` `dcd603e2f48f8a88` and
`warden` `bcda68d2360256f4`, and the Runner's code against three raiders (seed 3, `afa541fe50b16945`, as recorded):
native x86_64 vs wasm32-wasip1 **identical** and byte-identical to the build before; the base pack's rules fingerprint
is still `87f8e44db8186359`. A code made from a run's job records (`arena replay`) replays as recorded natively and in
wasm. An evolution run is checked the same way: `scripts/determinism-check.sh --run two_starts_locked [--seed N]
[--resume-at K]` hashes the run file after every generation, straight and resumed after K: seed 1 at K = 1, 3, 5 and
seed 7 at K = 3, **identical** native against wasm and resumed against straight (last line `4f126e780aff05c0` and
`887374c2560c32e7`).

**The evolution lab in the game** ([GDR-0092](gdr/0092-evolution-lab-for-your-own-ships.md) step 4, 16 Oct 2026): no sim
change; the blueprint's optional `origin` is left out of the content's serde form when none, so the rules fingerprint
stays `87f8e44db8186359`. Core, flight, assists, behavior, salvage and voyage (seed 42, 900 ticks), `runner_vs_raiders`,
the probe `close_quarters`, the Runner's code against three raiders (as recorded) and `--run two_starts_locked` (seeds 1
and 7): native x86_64 vs wasm32-wasip1 **identical**, every last hash as above.

**Trials** ([GDR-0092](gdr/0092-evolution-lab-for-your-own-ships.md) step 5, 16 Oct 2026): the test range's wave rule
and where its waves come in moved from the client into `sim::voyage::waves` (below); nothing in a scenario or a step
uses it, so no stream changed: core `04c38a80a753f50f` (seed 42, 900 ticks) identical native against wasm and as
before, the rules fingerprint still `87f8e44db8186359` (`lab.ron` is not part of it). The arena's gauntlets bring
their waves in with it on the `Scenario` stream (`fights/runner_gauntlet.ron` seeds 1 and 3 identical native against
wasm); `--run two_starts_locked` seeds 1 and 7 unchanged (`4f126e780aff05c0`, `887374c2560c32e7`), and the new
`--run mixed_trials` (every kind of trial, all four goals) identical, seed 1 `222c9bee00fa033e` and seed 4
`8d85b8edf59c7a64` (`docs/arena.md` "Determinism").

**The point-blank pass at the muzzle only, and the skiff's emitters forward** ([GDR-0021](gdr/0021-shields.md) Notes,
16 Oct 2026, release builds): a projectile in flight is taken by a bubble it finds itself inside ([Weapons](#weapons),
new hashed state `Projectile::crossed`), and the content changed (the `rim_skiff`'s emitters in row 1). Native x86_64
vs wasm32-wasip1 **identical**, last ticks (seed 42, 900 ticks): core `c94ee627199edcbb`, flight `797803bc06624f60`,
behavior `8be2be6123e0040f`, salvage `ceff1b3db1904a3b`, voyage `c2974339dd10cbde`, assists `377a6da647a8ba3b`; the
probes `close_quarters` `4851c82acb8c04fd` and `warden` `afff2bb6cac0a09b`. `runner_vs_raiders` is unchanged
(`8d5b00d21e45e771`, won at 1273: its last tick has no shell left in flight, and the fight ended the same). The base pack's rules fingerprint is now
`7aaf91e998fb2f63` (was `87f8e44db8186359`), so every fight code and lab run made before is fought under other rules:
codes replay under today's rules with the note, runs restore stale and read-only, to be re-scored (`docs/arena.md`
"Determinism").

**The Grown's mechanics** ([GDR-0102](gdr/0102-grown-mechanics-and-parts-defaults.md), 17 Oct 2026, release builds,
900 ticks): core, flight, assists, behavior, salvage and voyage (seed 42) run byte-identical to the build before
(`3cf0f35`, compared with `--compare`): nothing they fly has a heart, tissue, a prow or a star, a zero star heat adds
exactly nothing, and a wreck stripped at a sleeping site never happens in them. Last ticks: core `31b368b328356716`,
flight `370a1a72031d7d4b`, assists `dfbab88fdb5f6035`, behavior `be47fae608752449`, salvage `58ad85ac07925283`, voyage
`f53bdfa38cba1588` (core, flight and voyage native = wasm32-wasip1 re-checked). The new **grown** scenario
(`--scenario grown`, below) native x86_64 vs wasm32-wasip1 **identical**: seed 42 last tick `6bda03722e7ee707` (14
parts re-grown, 5 rams, the grove woken once, 1927 overheated part-ticks), seed 7 `a6caf3d6b5b3afc4` (22 re-grown).
The rules probes, native = wasm: `close_quarters` (seed 1, 300 ticks) `d5e0fe975b176235`, `warden` (seed 2)
`341daf33b8fe8add` (both changed with the 17 Oct content before this, the gunner and shield capacity 2); the content's
fingerprint changed with the new parts, so earlier fight codes and lab runs are fought under other rules. The galaxy
(`--galaxy 20`, seeds 1–20) native = wasm, last line `997ed53d1b648abf44a63da6ac8e8194`; seed 1's skeleton
`6c2845db355809ce` (the outer band's clusters changed).

**The player's cannon** (GDR-0100's status of 18 Oct 2026, `docs/results/slice-balance-2026-10-18.md`, release
builds): a new part, `reverse_engineered_cannon`, that nothing in a scenario flies; every stream byte-identical to
the build before and native x86_64 = wasm32-wasip1: core, flight, assists, behavior, salvage, voyage and grown (seed
42, 900 ticks) as above, voyage seeds 42 and 7 at 1800 ticks `e2404c9d5557eb3f` / `9be576eee0c5aa07`, grown seed 7,
the probes, `runner_vs_raiders` `1bd3a842a2c5dfdd`, `--galaxy 20`; `--run mixed_trials` and `two_starts_locked`
(seed 1) native = wasm, last lines `891760365f7ece43`, `5799a287e48aef13`. The content's fingerprint changed with the part, so earlier
fight codes and lab runs are fought under other rules.

**The Grown in play** ([GDR-0103](gdr/0103-the-grown-in-play-defaults.md), 18 Oct 2026, release builds), native
x86_64 vs wasm32-wasip1 **identical** everywhere. The outer reach's new ships are tagged `outer` and left out of the
scenarios that spawn every blueprint, so those fly what they flew before; but the Grown's three content behaviors
load before `gunner_pack` (`behaviors/grown.ron`), the library's ids after them shift by three, and a behavior's id
is in the state hash, so every stream with a behavior hashes anew. Checked: with the Grown's behaviors loaded last
instead (the file renamed for the test), flight (`370a1a72031d7d4b`), voyage (`f53bdfa38cba1588`) and
`runner_vs_raiders` (`1bd3a842a2c5dfdd`, ending at tick 1247) are byte-identical to the build before. Last ticks now (seed
42, 900 ticks): core `31b368b328356716` (no behavior: unchanged), flight `5b57ad1328766ee9`, assists
`cc347ad03a8628c6`, behavior `46170123f1496acc`, salvage `d8693e4105581fb1`, voyage `6636aafb3fe98b21`, grown
`a14b909055122d30` (seed 7 `cbd986eae19f28c5`, 22 re-grown); `runner_vs_raiders` `92f877f393ca148f` (1247 ticks),
the probes `close_quarters` (seed 1, 300 ticks) `d75310c995d5753e`, `warden` (seed 2) `11186687c4a56d6b`;
`--run mixed_trials` and `two_starts_locked` (seed 1) last lines `12dc217c6b728149`, `2130a78823454d40` (the `ram`
action joined the mutations' vocabulary). The galaxy (`--galaxy 20`, generator version 3) last line
`5c71f969886ec4ec85f965ef31269bdf`; seed 1's skeleton `f898e1535374653d` (the outer reach's cluster types). The
content's fingerprint changed, so earlier fight codes and lab runs are fought under other rules.

Scenarios (`sim::voyage::scenario`); `core` and `flight` spawn two copies of every blueprint in the content (but
those tagged `outer`, since 18 Oct 2026):

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
- `salvage`: wrecks spawned as wrecks, collectors, core fragments, a shrinking hold and a lost ship's cargo; see
  [Salvage](#determinism-and-scenarios-1).
- `grown` (17 Oct 2026, GDR-0102, `scenario/grown.rs`): a voyage built in code, 600 m, a wary grove 450 m in from the
  entry (two thornbacks, fixtures with a heart, a ram prow, two thorn guns and tissue armour, flying `raider_basic`, and
  a thornback wreck) beside a hot star (heat 0.4 at the centre); the player's ship-mind ships with a hold and a mender
  (a fixture with a heart in tissue) fly the voyage scenario's script without its kills (they pass within the wake
  radius, strip the wreck, scan, fight), random hits (5 %) and a blast every 300 ticks, a ram run every 300 ticks (the
  first thornback's prow into the first player vessel). Counts `regrown` (`PartRegrown`).

For the browser: the game build (`wasm32-unknown-unknown`) can call `sim::voyage::scenario::run_kind(kind, seed, ticks)`
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
  mounts and its plume reach, rebuilt when it loses a part and when `Sim::set_config` changes a scale; its
  authority is refreshed every tick.
- Per-part caches (exposure, bonus-adjusted stats, heat constants) are updated only around a destroyed part
  (its linked neighbours, and their neighbours for bonuses). `Ship::verify_caches` checks this equals a full
  recompute (tested over 900 scenario ticks).

### Control, salvage and dark pieces

- The **controlling piece** (`Ship::control_piece()`, `Piece::controlled`) is the blueprint's main piece if it
  has a logic core: the ship's vessel. One rule, `data::design::geometry::main_of`, picks the main piece for the drydock
  (`ShipGrid::main_piece`), at spawn, on splits and in `Ship::main_piece`: a piece with a core first, then the
  most parts, then the earlier one (at spawn the pieces are in the drydock's row-major order, so a blueprint of
  equal pieces gets the vessel the drydock describes; 13 Oct 2026 review fix). On a split it stays with the part
  that keeps the parent's id (the component with a core, then the most parts). It holds control only while it has a core: if it loses its last
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
`src/ship/power.rs`.

Per piece and tick:

- **Supply** = output of alive, non-overheated, non-idle reactors (with bonuses). `Piece::power.supply` shows
  it on a dark, dormant or salvage piece too, but such a piece hands out nothing (the allocation gets 0).
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

In a cluster beside a hot star (GDR-0102) the generation also takes `star × open_sides × DT`, `star` the star's heat
per side at the ship's controlling piece ([Hot stars](#hot-stars-gdr-0102); 0 adds exactly nothing, so worlds without
a star keep their hashes). Reactors make `heat × (1/3 + 2/3 × load)` (none when nothing draws). The steady state equals the drydock
model's (`loss·T = gen − Σ K (T − T_n)`, K = 1). A part above its `heat.limit` shuts down (no work, no draw,
no output) from the next tick until it cools below **75%** of the limit (hysteresis). Constants in
`ship::tuning`, all placeholders. With the current Halvex Runner layout nothing overheats under combat load;
flat out with the gyros working all the time the shield emitters overheat after about 15 s. Links never cross
pieces, so a ship that owns little of its blueprint (a split-off vessel) runs the step over its pieces' own parts
and links (`Piece::heat_links`); see [Vessels, Cost](#cost-1).

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

**Weapon groups** (GDR-0088): every gun slot's `GunSpec::group` (1 to `data::MAX_WEAPON_GROUP` = 3) comes from its
placement's `group` (none = 1; an out-of-range group in an unvalidated blueprint counts as the nearest). The pilot's
`PilotInput::fire` is a mask, bit g−1 for group g, `sim::FIRE_ALL` (0b111) for every gun. The plan turns it into
`Fire::Groups(mask)` (0 = `Fire::Hold`); the flight computer sets `ShipControls::fire` with an empty per-gun mask for
`FIRE_ALL` (as before), else fills `ShipControls::guns` per slot from the gun's group. Helm commands
(`ShipCommand::Goal`/`Maneuver { fire: true }`) fire every group; behaviors keep firing each gun at will (groups are
for the pilot and the pilot's auto-fire assist, [Pilot assists](#pilot-assists)). Test `the_fire_mask_fires_only_its_groups` (`tests/keyboard.rs`).

## Flight

### The flight computer (`flight/mod.rs`)

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
   ([Keyboard rotation](#keyboard-rotation)). `Track { angle, rate }` (the lock and `face`,
   [Feed-forward](#feed-forward-the-lock-and-face-lead-a-moving-target)) asks
   `ω_des = clamp(rate, ±√(margin · α · π)) + turn_rate(e)`, within `ω_max`.
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
- **Released stick or keys**: coast and keep the heading (no velocity goal), with or without `drift`
  ([GDR-0086](gdr/0086-letting-go-coasts.md), 14 Oct 2026; until then a release braked to a stop, flipping and
  burning if needed). `Auto` headings now only come from behaviors' maneuvers.
- **Lock** (`Target::Ship / Piece / Point`): by default (`AimMode::Auto`, GDR-0040, `flight::smart_aim`) the ship
  turns its strongest working guns onto the target (the side with the most damage per second, front first on
  ties; their lead point if that side's strongest gun fires projectiles); `AimMode::Target` / `Lead` point the
  nose at the target or its lead point (see [Aiming](#aiming-and-leading)); the target's bearing rate is fed
  forward ([Feed-forward](#feed-forward-the-lock-and-face-lead-a-moving-target)); A/D add the nudge (`aim_offset`); the
  stick strafes with whatever thrusters push that
  way (RCS), the full stick being the top speed reachable along the stick from this heading
  (`Kin::top_along`); released = coast (GDR-0086). A ship with engines only at the back cannot brake or strafe while locked.
- **Keyboard** (GDR-0040): a full W, S, Q or E is the top speed of the thrusters pushing that way; W with Q or E
  asks for both (with the whole-speed fade the side push stops once the speed passes the side thrusters'
  rating). A/D: [Keyboard rotation](#keyboard-rotation).

`Ship::telemetry()` shows the top speed (best axis), goal, heading target, requested force/torque, the expected
wrench and whether the last resort was on (`assist`).

### Feed-forward: the lock and `face` lead a moving target

Decided by Niklas on 13 Oct 2026 ([GDR-0064](gdr/0064-lock-feed-forward-and-ai-flight-fixes.md)). Code:
`HeadingGoal::Track`, `Kin::track`, `flight::smart_aim`, `maneuver::face`, the `omega_des` match in
`flight::execute`.

- **The problem**: the heading controller turned the nose towards where the target *is*. Against a target whose
  bearing turns at `ω`, the nose settled `max(ω / turn_gain, ω² / (2 · margin · α))` behind it. Since the slower
  turning of GDR-0055 the second term dominates for the Halvex Runner (first-line α 2.21 rad/s²): a raider
  crossing 33 m ahead at 20 m/s (up to 0.6 rad/s) was trailed by 6.7°, 3.9 m off, a clean miss
  (`docs/results/balance-2026-10-13.md`, sim issue 4).
- **The fix**: the lock (smart aim, GDR-0040, including side or rear gun groups and the lead point) and every
  `face` (the `face` action, an approach or a kite at range) give a `HeadingGoal::Track { angle, rate }`: the
  angle as before, and the bearing rate of the aim point as seen from the ship, `(r × v) / |r|²` (`r` from the
  centre of mass to the aim point, `v` the target's velocity relative to the ship). The flight computer asks for
  that rate plus the usual turn towards the angle: `ω_des = clamp(rate, ±√(margin · α · π)) + turn_rate(e)`,
  within `max_turn_rate`. The feed-forward is capped at a rate the ship could stop from within about a quarter
  turn on what it turns with this tick (the same cap as keyboard rotation, without the key rate), so a target
  whipping past at close range cannot spin the ship faster than it can stop; the torque still comes from the
  allocation, and the last resort follows its usual gate (GDR-0055).
- **Unchanged**: keyboard rotation (`Rate`, the A/D ramp and its 1.5 rad/s), the lock's A/D nudge (the nudge
  offsets the angle; its own rate is not fed forward, so the nose still trails a growing nudge by up to
  0.29 rad), the turning table below and the agility tests (90° turns aim at a fixed heading).
- Tests `the_lock_leads_a_crossing_target` (that crossing raider: the Runner's nose within 0.04° of it, 8.95°
  before) and `face_leads_a_crossing_target` (a small turret under `face`: 0.03°, 2.89° before) in
  `tests/flight.rs`.
- Cost: one cross product and one division per locked or facing ship per tick.

### The last resort

Decided by Niklas on 13 Oct 2026 ([GDR-0055](gdr/0055-slower-crisper-turning.md)): gimbals and differential thrust
(off-axis engines) push the ship sideways, so they turn it only after gyros, cores and RCS couples have been at
their limit for a few ticks. The numbers and details (6 ticks; differential thrust only while steering to a
heading, never for the keys, never to stop a spin) are lead defaults ([GDR-0058](gdr/0058-turning-v2-defaults.md)).
Code: `flight::TurnAssist`, `Piece::assist`, the end of `flight::execute`.

- **Gate** (`TurnAssist { ticks, on }`, per piece, hashed): every tick the first line reports the torque it could not
  give. `assist_after` (6) ticks in a row with torque missing turn the last resort **on**; it stays on while torque is
  still missing or the ship is still turning (heading error above `flight::TURNING`, 0.05 rad or about 2.9°), so a
  turn planned with it can brake with it; then it is off and the count starts over. Pieces the flight computer does
  not fly (not controlling, dark, raw throttles, no goal) are reset, and so is a piece under keyboard rotation (a
  `Rate` goal): the keys close a gate that a lock, `Auto` or behavior turn left open, and never open it (13 Oct 2026
  review fix: a lock turn's open gate raised the key rate of the sample mid to about 1.15 rad/s with differential
  thrust).
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
  the old 4.6 rad/s). The browser (headless, the debug overlay's readout) agrees: a tap 0.8°, holding 1.50 rad/s, last resort
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
  part that just overheated is out), on rebuilds and when a split-off piece becomes a vessel of its own (13 Oct
  2026 review fix: its first plan saw no thrust), and is zero while the piece is not active (a woken vessel gets
  power, and so authority, in its first systems step). `Kin` takes the
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
- Test `a_runner_with_idle_actuators_turns_cleanly_and_coasts` (`tests/flight.rs`; until GDR-0086 it also braked): a Halvex Runner that lost a
  logic core, one gyro set to the lowest power priority, so it idles together with a pulse laser (load 18,
  capacity 16): 90° in 1.97 s with 0.06° overshoot, and a stop from 21 m/s (flip and burn) in 4.5 s without
  flying backwards (13 Oct 2026 content; 1.07 s and 3.3 s with the turning of 12 Oct). Planned with the idle
  gyro, as it was before 12 Oct (measured with the content of 12 Oct), the same stop flew backwards at up to 5.7 m/s and was still spinning at 2.3 rad/s.

### Top speed and the cruise flame

Decided by Niklas on 12 Oct 2026 ([GDR-0041](gdr/0041-engine-rated-speed.md)); it replaces the soft top speed
`k·√a`. The fade by the whole speed: Niklas, 13 Oct 2026 ([GDR-0055](gdr/0055-slower-crisper-turning.md)); made
continuous in the push angle and maneuver goals capped at the top speed: 13 Oct 2026
([GDR-0064](gdr/0064-lock-feed-forward-and-ai-flight-fixes.md), lead). Code: `ship::rated_falloff`,
`Thruster::falloff`, `ThrustAuthority::speed`, `Kin::speeds`, `Kin::top_along`, `Maneuver::goal_for`, the cruise
lines at the end of `flight::execute`.

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
- **Weighted by the push angle** (GDR-0089, 15 Oct 2026; before it GDR-0064's `min(|v|, 4 × the speed along the
  row's axis)`): the fade uses `|v| cos²θ`, θ the push's angle to the motion (`along² / |v|`). Straight along
  the motion that is the whole speed (unchanged: the knee, the cruise flame); off the motion the fade lets go
  smoothly, and at the rating a push about 27° or more off the motion (cos²θ ≤ 0.8) has full thrust. Continuous
  in the angle (test `the_fade_is_continuous_in_the_push_angle`). GDR-0064's version went from nothing to
  everything between 75.5° and 78.5° off the motion, and a ship that kept turning while it pushed in the full
  band could gain speed without a bound (Niklas's play-test, 15 Oct 2026: "sometimes beyond the max speed, and
  sometimes I barely can").
- **The governor** (`ship::govern`, GDR-0089): in the physics phase, the world force a piece's own actuators put
  on it is cut so that this tick it never takes the piece's speed past its top speed (the highest rating among
  its groups that can push now, `ThrustAuthority::speed`), or past its speed now if a blast made it faster
  already. What is left turns the velocity towards the push: at the top speed, pushing a new way swings the
  velocity onto it at the top speed, the speed the old way falling away. Braking passes unchanged; collisions
  and blasts are not the piece's force and are never cut. Cost: two square roots per flown piece per tick.
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
  ([Planning with what works](#planning-with-what-works)). Straight along a group each row fades by its own
  rating; the governor caps every direction at the ship's top speed (the fastest group's), so a diagonal is no
  longer held to the slower group's rating (GDR-0055's rule, until GDR-0089), and the old box (a ship with
  22 m/s forward and 14 m/s RCS sideways reached 26 m/s crabwise) stays gone.
- **`Kin::top_along(u)`** (the locked touch stick's full length): the fastest velocity goal along `u` the flight
  computer reaches from rest. It pushes each group flat out until its share of the goal is there; the group that
  finishes last caps the speed at its own top speed, and the group that finishes first must get there before
  the whole speed passes its top speed (`speed_first / (share_first · √(1 + (thrust_last / thrust_first)²))`), so
  a small sideways share does not cut the main axis's top speed (the slowest group would otherwise cap any stick
  that is not exactly along an axis). Groups without thrusters are left out. Tests
  `top_speed_per_direction_comes_from_the_thrusters_pushing_that_way` and
  `a_diagonal_is_no_faster_than_the_slowest_thrusters_pushing_that_way` (`tests/speed.rs`): an RCS box (pods
  14 m/s, thruster 20 m/s) locked with the stick at (0.6, 0.8) flies that diagonal at 14.0 m/s (the box allowed
  23 m/s; `top_along` still asks for this conservative goal); with W and E held in drift it ends at 20 m/s (the
  thruster) with 12.7 m/s sideways (7.9 before GDR-0089). Test `at_top_speed_the_velocity_swings_onto_the_push_and_never_passes_it`:
  a 20 m/s ship at its rating pushing 30°, 60°, 89° and 120° off the motion ends on its nose (within 4°) at
  20.00 m/s, never past it.
- "Fly that way" turns the best axis onto the stick and flies at its rating (unchanged). The keyboard's W with Q
  or E asks for both axes' top speeds; the side push stops once the whole speed passes the side thrusters'
  rating.
- **Cruise flame**: whenever the goal has a velocity, every engine row gets `Actuation::cruise` = (goal velocity
  in the piece frame · engine direction) / its group's top speed, clamped to 0..1; the systems publish
  `Piece::cruise` = that × the engine's power level (0 if idle, dead, overheated, unpowered or the piece is dark). Full
  stick at the top speed = 1 (the engine pushes nothing, `Piece::output` ≈ 0, but the flame stays); **partial
  stick** = the stick share (holding half the top speed shows a gentle half flame, decided by the agent so that a
  held speed never looks like dead engines); maneuvers and behaviors (an approach, an orbit, matching a target's
  velocity) show it the same way. A released stick or released keys (no velocity goal: the ship coasts, GDR-0086;
  with or without drift) show none. RCS nozzles get none (they puff only when they push). The client draws
  `max(output, cruise)` with the calm look where `cruise` exceeds `output` ([client.md](client.md) "Plumes"). The
  cruise level is derived output and not hashed.

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
constants (placeholders, `flight/mod.rs`): the keyboard ramp `KEY_TURN_START` 0.2 and `KEY_TURN_GROWTH` 1.0 /s
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

`Maneuver::Brake`, `Hold { at }`, `FaceTarget { target, aim }`, `Escort { leader, distance }` (stay within
`distance` of a leader: the behavior action `escort`, see [Evaluation](#evaluation)), `Approach { target, range, aim }` (close in at
the speed it can still stop from, then face the target or its lead point and keep pace), `Orbit { target,
radius, ccw }` (tangential speed
limited by the centripetal acceleration the ship can hold, radial error corrected). Each becomes a
`FlightGoal` every tick; an unresolvable target brakes. Facing a target (`FaceTarget`, an approach or a kite at
range) feeds its bearing rate forward ([Feed-forward](#feed-forward-the-lock-and-face-lead-a-moving-target)).

`KeepRange { target, range, aim }` (back off with `HeadingGoal::Auto` when closer than `range`, so forward-only
ships turn away; otherwise like `Approach`) and `Flee { from }` (straight away at top speed) were added for
behaviors. "Top speed" in every maneuver (`Flee`, the cap of `arrive_speed`, the orbit's 60%) is `Kin::top_speed`,
the rated top speed of the best axis (GDR-0041); a target that flees with faster engines gets away. `Maneuver::goal_for(kin, target_state, tuning)` takes an already resolved target (behaviors fly
relative to an aim point on the target ship).

**Goals are capped at the top speed** (GDR-0064, lead): `goal_for` scales every velocity goal down to
`Kin::top_speed`. Matching a faster target (`face`, an approach at range), adding a moving target's velocity to an
orbit or an approach, asked for more; a ship at its top speed then needed a push a hair "with" its motion, the
whole-speed fade cut it off, and the heading logic (which plans with thrust, not with the fade) never turned:
AI ships flew straight on, raiders for up to 34% of a fight (`docs/results/balance-2026-10-13.md`, sim issue 1).
With the goal at most the top speed, the change it asks for at the top speed never points with the motion.
Test `maneuver_goals_never_ask_for_more_than_the_top_speed`.

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

- **Cadence**: every ship thinks when `tick % 3 == 0` (`tuning::THINK_PERIOD`, 10 Hz) and on the first tick
  after it gets a behavior. Dark and dormant ships do not think. Until 14 Oct 2026 the thinks were staggered by
  ship id (`(tick + ship_id) % 3`, GDR-0034) to spread a fleet's thinks over the ticks; but then of two enemies
  one always reacted a tick sooner, and in mirror fights that decided the winner (ADR-0037). Thinking costs
  little next to the rest of a tick (the 200-ship load test: median step the same, p95 within noise).
- **Two phases** (order-free, like the flight plan): every due ship decides from the same snapshot (the state
  after this tick's input breakage) into a per-ship scratch slot; then all decisions are applied. Per-pass
  caches: each ship's centre (one sin/cos per ship per pass instead of per candidate) and hull share (lazy).
- **A think**: `Kin` of the controlling piece; reach = longest working gun (clear, alive, not overheated, not idle: `ship::gun_works`; 60 m without, which is also `me.reach` in Rhai). The target line
  picks the target among active vessels of other owners (until 12 Oct: other factions), keeping the current one unless another's cost is
  below 0.8 × its own; the aim point is the nearest working part of the chosen kind on the target's controlling
  piece (or its centre). Then the rules in order, within the working slots: conditions short-circuit left to
  right; the first rule whose conditions hold and whose action has what it needs wins.
- **Fly** (every tick, in `flight::plan`): the stored maneuver's goal relative to the aim point's current
  position and velocity (`aim_state`). No matching rule: `Maneuver::Brake`.
- **Turret** (GDR-0064, lead): a vessel with no working engine (none left, or all idle, unpowered or overheated;
  RCS nozzles do not count) turns its strongest working guns onto its target whatever its rule flies, with what
  it has (gyros, cores, RCS couples): the lock's smart aim, with the feed-forward. RCS pods still follow the
  maneuver's velocity goal. Before, an approach from afar, an orbit or a flee flew with `HeadingGoal::Auto`,
  which with nothing to push kept the heading: a Runner whose tail was shot off drifted on with its guns
  pointing anywhere (`docs/results/balance-2026-10-13.md`, sim issue 2). Without a target, or without working
  guns, the maneuver's heading stays. Test `an_engine_less_vessel_turns_its_guns_onto_its_target`.
- **Fire at will, per gun** (Niklas, 12 Oct 2026; GDR-0034): the plan carries the aim point's state
  (`Fire::AtWill`); when the flight computer runs, `guns_at_will` checks every clear gun of the controlling
  piece on its own: it fires when the target is within 0.933 × **its** reach from its muzzle and the aim point
  (for projectile guns its own lead point, from muzzle position and velocity) is within 0.12 rad of **its**
  facing. The result is the per-slot mask `ShipControls::guns` (weapon power demand reads
  `ShipControls::gun_fires(slot)`, so a gun that holds draws no power and does not fire). Broadside and rear
  guns fire whenever the target crosses their line, e.g. while orbiting. One `atan2` per clear gun per tick;
  no allocation once warmed up. Helm fire and the pilot's `FIRE_ALL` set `fire` with an empty mask (all guns); a
  pilot mask of fewer weapon groups fills the per-slot mask by each gun's group ([Guns](#guns)).
- **Slots**: `Ship::rule_slots()` = `rule_slots_of(control piece)`: the sum of `rule_slots` of the piece's
  working logic cores (GDR-0035: alive, not overheated, powered, and not idle: `ship::producing`; a woken
  vessel's cores count from its first power pass), 0 while the piece is dark. Rules beyond are `NoSlot`.
- **Conditions** (built-in): `hull` (Σ hp on the controlling piece / intact hp: of the whole design, or for a
  vessel split off another of the parts it took along, see [Vessels and splits](#vessels-and-splits)), `shield` (charge / capacity of
  working emitters; no emitters → `NoValue`), `shield_down`, `heat` (hottest part / its limit), `power`
  (served / demand), `target_in_range(d)`, `target_hull`, `target_on_me` (its behavior target, or its pilot
  lock, is me), `under_attack`, `enemies_near(r)`, `cycle(p)` and `target_time` (integer ticks since the
  target was picked; both need a target). Target conditions without a target fail with `NoTarget`, regardless of `not`.
- **Actions** → maneuvers: `approach(r)` → `Approach`, `orbit(r, dir)` → `Orbit` (`auto` = by the parity of
  the ship's and its target's ids together: half of a fleet circles a target each way, and two ships circling each
  other go the same way, whichever has the lower id; ADR-0037), `kite(r)` → `KeepRange`, `face` → `FaceTarget`, `flee` → `Flee`, `hold` → `Brake`,
  `escort(d)` → `Escort` (GDR-0065, below), `ram` → `Ram` (GDR-0103, 18 Oct 2026: the velocity goal is the
  target's velocity plus top speed straight at it, capped at the top speed like every goal, the nose on the target
  with `AimMode::Target`, never braking for it; the Grown's rammers fly it in a cycle). All others aim with
  `AimMode::Auto` (lead for projectile ships).
- **`escort(d)`** (GDR-0065, 13 Oct 2026; default 30 m): keep within `d` of the leader (`ShipCommand::Leader`, the
  vessel whose lock `locked` follows: for the sandbox's `?fleet=` wingmen and TEST IN RANGE's wingman the
  player's ship). It needs no target (a rule with it never fails with `NoTarget`), so it goes last, below the
  rules that fight, and runs between fights. `Maneuver::Escort { leader, distance }`: farther than `d` it closes
  in like an approach (`seek` to `d` off the leader, plus the leader's velocity, `HeadingGoal::Auto`), within
  `d` it keeps pace (the leader's velocity); it flies relative to the leader, not the target, and guns still
  fire at will at the target if there is one. Without a leader, or when the leader is no vessel any more, the
  think picks `Maneuver::Brake` (it holds). Like every maneuver it never asks for more than the top speed, so an
  escort of the same design as a leader flying flat out falls behind until the leader slows. Test
  `an_escort_keeps_near_its_leader_and_fights_its_lock` (`tests/behavior.rs`): behind a Runner flying at
  13 m/s the escort stays within 24.5 m, stops 11 m from it when it stops, and attacks what it locks.
- **Targets**: `nearest` (distance), `weakest` (hull share, ties by distance), `threat` ((d + 50) / firepower,
  firepower × 2 if it aims at me), `attacker` (nearest enemy aiming at me), `locked` (the lock of
  `Ship::leader()`: its pilot lock or behavior target).
- **`escort`** (the ship-mind's escort template, GDR-0065, from `../space-game-archive/plans/slice-content.md` section 6): `target
  locked(core)`; kite to 150% of its reach when below 30% hull with the target aiming at it; approach to 70%
  (every gun within reach, nose on the target); `escort(30m)` when the leader has nothing locked. It never picks
  its own fights. Not the default of any design yet (the BEHAVIOR screen lists it as a template); measured as
  sandbox wingmen in `docs/results/balance-2026-10-13.md`.
- **`raider_basic`** (the automatons' default; it replaced the raider brain): with a cannon raider (75 m reach)
  it approaches to 64 m (85%), and within 90 m (120%) orbits at 43.5 m (58%) for the last 6 s of every 9 s (the
  last 4 s until 14 Oct 2026: the cannon's shot grew to a full hit point, GDR-0074); its cannon fires at will within
  70 m (93.3%). Until 12 Oct it stood off at 50 m (67%) and orbited at 45 m
  (60%); from 12 to 13 Oct it orbited at 50 m (67%), exactly where the Halvex Runner's beams stop touching a
  raider (GDR-0061); see [Sandbox fights at the new speeds](#sandbox-fights-at-the-new-speeds).
- **`gunner_pack`** (the automaton gunner's own, its blueprint's `behavior`; 17 Oct 2026, GDR-0101): the rules of
  `raider_basic` with the target `nearest(engines)`. Aimed at the engines (behind the starter's shielded nose)
  rather than the centre, it costs the starter far more than raiders do and the Halvex Runner less
  (`docs/results/slice-balance-2026-10-17b.md`).
- **`skirmisher`** (the ship-mind's default for unpiloted vessels): flee below 25% hull, orbit at 80% while its
  shield is down and the target aims at it, face a fast attacker that is still out of reach, else approach to
  67% and keep the nose on the target. Its two other orbit rules (`enemies_near(100m) >= 3 -> orbit(90%)` and a
  4 s orbit every 10 s) were deleted on 13 Oct 2026 (GDR-0061): since GDR-0055 every switch back from an orbit
  costs 1.5–3 s of turning, and a lone Runner fired in 12% of the time a raider was within its reach.

### Sandbox fights at the new speeds

**13 Oct 2026, after the slower turning (GDR-0055):** re-measured with the headless balance harness (the separate
local repo `../space-game-balance`, 200 seeds per setup): [`docs/results/balance-2026-10-13.md`](results/balance-2026-10-13.md).
The content changed with it (GDR-0061): `raider_basic` orbits at 58% (was 67%, on the beams' edge), the
`skirmisher` lost its two orbit rules, and shield emitters hold 3 and regenerate 0.2/s (were 6 and 1.5/s), so
bubbles collapse under sustained fire. The sim fixes of GDR-0064 (feed-forward for the lock and `face`, goals
capped at the top speed and the continuous speed fade, engine-less vessels aiming, split-off hull share) were
measured in the same report's last section. The table below is the 12 Oct measurement, kept for the history.

Measured 12 Oct 2026 (GDR-0042 content, before the slower turning of GDR-0055) with a throwaway harness (not in
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
let assists = world.behaviors_mut().add_assists(&blueprint.assists.unwrap())?; // a design's pilot assists
world.command(player_ship, ShipCommand::Assists(Some(assists)));   // run while it is piloted
world.command(player_ship, ShipCommand::SwitchAssists(sim::AssistSwitch::Toggle(2))); // ASSIST TOGGLE 2
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

### Pilot assists

Code: `src/behavior/assist.rs` (GDR-0088 step 3, 15 Oct 2026). A design's PILOT ASSISTS list
(`docs/behavior-format.md` "Pilot assists") is compiled like a behavior: `BehaviorLibrary::add_assists(&list)`
(`data::AssistList`, checked with `data::behavior::check_assists`) → `AssistsId`; `ShipCommand::Assists(Some(id))`
gives it to a ship (`None` takes it away; unknown ids are ignored), `ShipCommand::SwitchAssists(AssistSwitch)` flips
a toggle slot's rules (`Toggle(1..=4)`) or ALL ASSISTS (`All`). The game gives every player vessel its design's list;
only a **piloted** ship runs it (`HelmMode::Pilot`, no behavior: the focus), so behaviors and AI ships fly exactly
as before.

- **State** (`Ship::assists()` → `AssistState`): per rule whether it is switched on (bit `i`, starting as its
  `start` value), the master switch, the tick the pilot last used each control (turning, thrust, a trigger), the
  target (the ship of the pilot's lock), the pilot's lock and the last tick it was held, auto-lock's pause
  (`lock_paused()`) and the ship it asks for (`auto_lock()`), per channel the rule that decides it, the station `hold_station` keeps (`station()`) and the piece
  `avoid_collisions` steers clear of (`avoiding()`). It lasts for the ship's life (a mission); a vessel split off
  inherits it.
- **The pass** (`assist_all`, after the behaviors' think): every tick each piloted ship with assists notes the
  controls its pilot uses (`PilotInput`: A/D or the stick without a lock = turning; W/S/Q/E or the stick = thrust;
  any fire bit; a key counts from `FlightTuning::stick_deadzone`, as the flight computer reads it) and its lock (see
  auto-lock below). On think ticks (every 3), on the tick after a list is set or switched and on the tick auto-lock
  pauses or resumes, the rules decide from one snapshot: the target is the pilot's lock and nothing else (no target
  line, GDR-0088's dated line of 15 Oct 2026); then for each rule in order, if it is on (and ALL ASSISTS is on), its
  channel not yet decided, it has a target if it needs one (auto-fire: the lock) and its conditions hold, it decides
  its channel (avoid collisions only with something on a collision course, auto-lock only while not paused and its
  pick finds a ship). Channels: the lock, thrust, one per weapon group.
- **Auto-lock** (`auto_lock(pick)`, the lock channel; picks `nearest`, `weakest`, `threat`, `attacker`, run by the
  behaviors' `Ctx::select` with no current target): a deciding rule, while the pilot holds no lock, sets
  `AssistState::auto_lock()` to the ship it picks; the client makes that the pilot's lock on its next tick, as a click
  would (`world::scene::tick_lock`), so `PilotInput::lock` carries it from then on and the HUD, the target panel and
  every assist see one lock. A held lock clears the ask (auto-lock never replaces a lock). The pause: a lock held on
  the tick before and gone now while its ship is still there (`lock_there`: a ship still active, a piece not gone)
  was dropped by the pilot (Esc, a click on empty space), and auto-lock waits (`paused`, no rule decides the lock
  channel) until the pilot holds a lock again or a switch touches an auto-lock rule (its toggle slot, ALL ASSISTS);
  a lock gone with its ship (the client drops a lock on a ship no longer active) is no drop, so it locks the next.
- **Per tick** (`assist_plan`, in the pilot's plan after `pilot_goal`): a decided channel the pilot doesn't use this
  tick is the assist's. `brake`: the velocity goal becomes zero (the heading: see below); `auto_fire(g)`:
  `Fire::Assisted { held, auto, at }` with `at` the pilot's lock: guns of held groups fire, guns of auto groups fire
  at will at the lock (`guns_at_will`, per gun cone and reach); without a lock nothing. The lock aims the ship by
  itself (`pilot_goal`), which is why there is no `face_target` any more (it acted only without a lock, on the
  target line's pick; on the lock alone it would never act).
- **Conditions**: every behavior condition, plus `speed` (share of `Kin::top_speed`; no value without thrust) and
  `can_stop_straight` (for each body axis carrying at least 25% of the motion a thrust group pushes against it;
  true below 0.1 m/s), which behaviors may use too, and the pilot's own `lock_held` and `idle(any|turn|thrust|fire)`
  (seconds since the last use; `Why::NoValue` outside an assist list, which the checks never let happen).
- **State hash**: the assist state (id, program fingerprint, switches, last uses, target, acquisition tick, the
  held lock and its tick, the pause, the ship auto-lock asks for, decisions, last think, station, the piece avoided) after the ship's behavior, and nothing at all for a ship without assists, so worlds
  without assists hash as before.
- **Hold station** (step 4, 15 Oct 2026; `hold_station(point|ally)`, thrust): the station is taken on the first tick
  the rule has thrust while the pilot doesn't thrust (`keep_stations`, every tick after the thinks): the ship's centre
  of mass, or with `ally` its offset (world frame) from the nearest other active vessel of its owner (first in id
  order on a tie; a point without one). The pilot's thrust drops it, so flying moves the station; while a rule above
  has thrust (a dodge, a brake) it waits, unless the rule that took it is switched off; an ally that is no vessel any
  more is replaced on the next tick. Per tick the velocity goal is an escort of the station point at distance 0
  (`Maneuver::Escort::goal_for`: back at the speed it can still stop from, then the station's velocity; capped at
  the top speed); the heading: see below.
- **Avoid collisions** (step 4; `avoid_collisions`, thrust): at each think, if its conditions hold, it looks for the
  piece it would meet first: of every ship (vessels of any owner, wrecks, the ship's own broken-off pieces), the
  bounding circles (`Piece::radius` about the frame origin) on straight lines, the first closest approach within
  `tuning::AVOID_AHEAD` (6 s) that comes nearer than the two radii plus `tuning::AVOID_CLEARANCE` (2 m). Only then
  does it decide thrust (else the rules below keep it). Per tick the velocity goal is the ship's own velocity plus the
  sidestep that makes that closest approach the clearance: along the miss vector (to the left of the relative motion
  when head-on), `(reach − miss) / t`, capped at the top speed; once clear (or the piece is gone) it asks nothing
  until the next think hands thrust down again (the heading: see below). Behaviors don't get it (GDR-0088 left that
  open; it would change every AI stream).
- **Thrust assists turn while turning is free** (Niklas, 15 Oct 2026, after step 4: no base ship has RCS pods, so the
  Rim Skiff, two rear thrusters, could neither brake nor hold station nor sidestep): when brake, hold station or avoid
  collisions sets the velocity goal, the pilot doesn't steer (A/D, the stick without a lock) or hold a lock and the
  velocity change asked is at least `tuning::ASSIST_TURN_DV` (0.5 m/s), the
  heading becomes `HeadingGoal::Auto`: the behaviors' flip and burn (kept while the thrusters facing the needed way
  give `keep_heading_fraction` of the best axis, else the best axis turned that way, the force faded in from 25° to
  8°). Otherwise the heading stays the pilot's and only the thrusters pushing the needed way act, as before. The
  threshold keeps small corrections from spinning the ship around. Hold station, on a ship that must turn around to
  stop (opposite axis under half the best, `Kin::stop_accel` plans with half the best then), also comes back no faster
  than it can still turn around and stop from, `a·(√(t² + 2d/a) − t)` with the half turn `t = 2·√(π / α)` at the
  planning angular acceleration and `a = stop_margin · stop_accel`, and within `tuning::STATION_SLACK` (1 m) of the
  station it only stops (matches the station's velocity): without both the Skiff circled its station at about 1 m and
  1–2 m/s for good, a half turn for every correction; with them it stays within about 1 m, slower than 1.2 m/s.
- Tests: `tests/assists.rs` (auto-lock asks for the nearest while nothing is locked, never replaces a lock, and a
  pick that finds nobody leaves the channel to the rule below; a lock the pilot drops pauses it until they lock again
  or switch it, a lock gone with its ship doesn't, toggles and ALL ASSISTS switch it; LOCK TARGET's order, nearest
  first and round again; brake kills drift without turning and W wins; auto-fire does nothing without a lock, fires
  only its group's guns that bear on the lock (not on a nearer raider), never out of reach, and a held trigger wins;
  `lock_held`, `idle(thrust)`, `speed`, `can_stop_straight` gate rules; toggles and ALL ASSISTS; only the piloted ship;
  hold station flies back after a push and W moves the station; beside an ally it keeps its place as the ally drifts;
  avoidance sidesteps a raider on a collision course (without it they collide), does nothing on a clear path (a brake
  below keeps thrust) and takes thrust over from it on a collision course; the pilot's W wins over it; the station
  waits while avoidance dodges a raider drifting through it, and the ship comes back; on the Rim Skiff with turning
  free, brake stops a 4 m/s forward drift by turning around, hold station comes back and stays within 1.3 m, and
  avoidance turns and passes a raider it otherwise hits; with a held lock or the pilot steering the heading goal
  stays theirs, and a 0.3 m/s drift never turns the ship).
- **Stand-in pilots** (`flight::stand_in`, GDR-0092 step 6): a scripted pilot from content (`data::StandIn`, `lab.ron`'s
  `pilots`) that gives a piloted ship `PilotInput` on think ticks, so its assists act where no player is (the
  evolution lab's fights FLOWN BY a stand-in, `docs/arena.md`). It keeps no state: the lock it holds while that ship
  is active (the helm's), else the auto-lock's pick as a client takes it, else LOCK TARGET's nearest when it locks
  itself; the stick towards its lock (or the nearest enemy) while that is farther than its share of the longest gun's
  reach; every trigger; stick and triggers each on and off by the clock from tick 0. The voyage runner keeps its own
  closing pilot.

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

The vessel model (12 Oct 2026; GDR-0013, GDR-0038, GDR-0044 and GDR-0045, decided by Niklas). Code: `src/ship/vessel.rs`
(owners, vessels, focus, mind capacity), `src/ship/control.rs` (control capacity, idle parts), the ram rules in
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
- **Hull share** (GDR-0064, lead): a split-off vessel measures its `hull` against the intact hit points of the
  parts it took along (`Ship::hull_base`, summed at the split), the vessel that kept the controlling piece against
  the whole design as before (it did lose the rest). Until 13 Oct 2026 both counted against the whole design, so
  a split-off half of a Runner read about 40% and a cored chunk under 25%: the skirmisher's flee rule sent it away
  at once (75–96% of its time, `docs/results/balance-2026-10-13.md`, sim issue 6). Test
  `a_split_off_vessel_measures_its_hull_against_its_own_piece` (a Runner cut down the middle: the kept half reads
  40%, the split-off half 100%).
- A blueprint built in several pieces with cores in more than one: at spawn every further cored piece becomes
  a vessel of its own the same way.
- A shot never hits its own ship (all its pieces) nor any other vessel of its owner (GDR-0048), so a split-off
  vessel and its parent fire through each other.
- Enemies follow the same rules: a raider shot in half can be two raiders (test).

### Focus

- `ShipCommand::Focus` makes a vessel its owner's focus, at once (a tick input, so replays and co-op see it).
  The focus is **piloted**: its behavior is parked (`Ship::parked_behavior`; `ShipCommand::Behavior` on the focus
  sets the parked one), its helm is `Pilot` (a released stick, which coasts with the heading kept since GDR-0086,
  until pilot input arrives), and it never goes dormant. The vessel the focus left runs its behavior again (its
  content default if it had none; without a library it brakes) and gets a fresh mind claim, so it stays awake.
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
  base emitter; GDR-0021) of what reaches it, in bubble order (ship, piece, emitter), except the bubbles friendly
  to the rammer: its own ship's and every other vessel of its owner (GDR-0067, decided by Niklas on 13 Oct 2026:
  rams pass friendly bubbles, like shots and plumes). Until then only the rammer's own piece was skipped, so a
  wingman whose bubble covered the contact point took the enemy's share of the ram and shielded the enemy (sim
  review finding 4). Shots, plumes and rams now share one rule for which bubbles take damage from a source
  (`weapons::absorbs_from`: not the source's own ship, not a vessel friendly to it, charge left, the family
  absorbed, the emitter alive); only the geometry differs (a shot's path entering the bubble, a plume's or a
  ram's point inside it). The same absorb step as shots (`ShieldHit { by: the rammer }`, a bubble that runs
  empty collapses); the rest hits the part. The bubbles are the ones collected in this tick's weapons phase
  (positions before the physics step). Test `a_ram_passes_the_bubbles_of_the_rammers_friends` (`tests/vessels.rs`):
  a raider rams an unshielded block beside the raider's shielded wingman: the block takes the whole ram, the
  wingman's bubble only its share of the damage dealt to the raider.
- **Crush** (GDR-0047, decided by Niklas on 12 Oct 2026): what the shields leave hits the touching part, and
  what exceeds its hit points carries on to the next part inward, along the contact normal into the hull, until
  it is used up or leaves the hull (`Sim::ram_crush`, the grid walk of penetrating shots; the emptied cells of a
  big part just crushed are passed through). The threshold rose from 20 to 100 kN·s, so bumps are free; the
  numbers are in [Collisions](#collisions-impacts-and-ramming-v1).

### Events and queries (for the fleet bar)

| | |
|---|---|
| `SimEvent::VesselFormed { ship, owner, from }` | a vessel formed; `from: Some((ship, piece))` for a piece that moved out of `ship` (split off a vessel, or a further cored piece of a blueprint built in several pieces, at spawn; the client moves that view to the new ship), `None` for a ship spawned with a core |
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

**Vessels split off a big blueprint** (13 Oct 2026 review fix): each vessel keeps the whole blueprint's per-part
state (dead outside its own pieces), and until then its heat pass walked every part and link of the blueprint,
so a 2000-part slab shot into 25 cored chunks cost 25 times the slab per tick. Each piece now keeps its own heat
links (`Piece::heat_links`, built on rebuilds), and a ship that owns less than half of its blueprint's parts
walks only its pieces' parts and links; one that owns more keeps the single pass over everything (cheaper per
part). Both give bit-identical heat (the state hashes of all four scenarios are unchanged). Measured with a
throwaway harness (a 50×40 slab with a core in every 10×8 block, cut into 25 cored chunks: 25 vessels, 1575
parts; 600 ticks, medians of five runs, native release, load average 5–13): step 0.073–0.085 ms before,
0.011 ms after; the intact slab 0.0069–0.0071 ms before, 0.0066–0.0067 ms after. A rebuild walks the design's
links once more (about 4 µs for 2000 parts; "lose one part" in `sim_perf` 0.052–0.054 → 0.057 ms). The copy at
the split itself is unchanged (once per split, not per tick).

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
(`PilotInput.fire`, a weapon-group mask; `ShipControls.fire` and its per-gun mask, or fire at will under a behavior), clear line of
fire, powered, not overheated, not idle (over the control capacity), piece active. Guns are fixed: the pilot aims by turning the ship; under a
behavior each gun fires when the target crosses its own facing. A blocked gun (a part of its own piece in its line of
fire, GDR-0004) fires nothing, although its shots would pass through its own hull; when the blocker is destroyed or
breaks off, the piece's rebuild clears the gun and it fires from the next tick (GDR-0070: "the blocker can be shot
off"). The design report counts guns the same way (`tests/weapons.rs`, `tests/report.rs`; `docs/arena.md` "What the
example runs found": evolved designs shutter their cannons behind reactors that the enemy shoots off).

| Delivery (content) | Behaviour |
|---|---|
| `Beam` | continuous hit-scan; each tick `dps × work × DT` to the first block in range; `Sim::beams()` lists this tick's beams (`from`, `to`, `hit`) |
| `Pulse(rate)` | a cooldown per gun (`rate × fire_rate` shots/s, slower when browned out; at most `weapons::tuning::MAX_SHOTS_PER_TICK` = 8 shots per tick, 240/s, also for projectiles: a faster rate is capped there and the rest waits in the cooldown); each pulse is a hit-scan dealing `damage / rate` to the first block |
| `Projectile(rate, speed, penetration, recoil)` | same cooldown; a `Projectile` entity at the muzzle with the muzzle's velocity (`v + ω × r`) plus `speed`, living `range / speed` s |

- **Lasers** (beam, pulse) burn the first block only.
- **Simultaneous fire** (ADR-0037, 14 Oct 2026): the guns of all ships fire at once within a tick. Every beam
  and pulse is traced (ray cast, shield bubbles along the path) against the state at the start of the fire
  pass: which guns fire, where they point, which parts are alive. The hits (`weapons::GunHit`) are applied after
  the last gun has fired, in firing order (ship, piece, gun); a hit whose part an earlier hit of the pass
  destroyed goes on along its shot to the part behind (one more ray cast, no second shield pass). So two ships
  that open fire on the same tick both fire, whatever their ids. Before, ships fired one after another in id
  order and each hit landed at once: a lower-id ship could shoot an enemy's gun or reactor off before it fired,
  and the side spawned first won 1173 of 2000 starter mirror matches to 378. Projectiles in flight move before
  the guns fire (their impacts are part of the state the guns see) and engine plumes burn after the hits are
  applied, both for every ship alike. Events: a pulse's `ShotFired` now comes before its `Hit`.
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
  of what reaches it). A shot fired from inside a bubble passes it (point blank: at the muzzle only, a beam's or
  a pulse's ray and a projectile whose muzzle was inside when it was fired). A projectile is taken once by each
  bubble (`Projectile::crossed`, hashed: the bubbles it started inside and those that took their share, the last
  8), and one it finds itself inside in flight takes its share at once, at the start of the tick's segment: ships
  move after the weapons, so a ship closing on a shell carries its bubble past the shell's last position between
  two ticks (0.67 m a tick at 20 m/s; until 16 Oct 2026 the shell then passed the bubble and hit the hull whole,
  [GDR-0021](gdr/0021-shields.md)). A bubble that runs empty collapses and
  stays down until it has recharged 30% of its capacity. A beam that a bubble fully absorbs ends at the bubble.
- Tuning constants (`weapons::tuning`, placeholders): ricochet cosine 0.5, speed 0.6, damage 0.5, max 2
  ricochets, shield restart 0.3, 8 bubbles a projectile remembers, at most 8 shots per gun per tick (a safety cap, 13 Oct 2026 review: a rate
  whose period was below half an ulp of the cooldown hung the step), 4 re-casts past destroyed parts.

Events (in `Sim::events()`, for visuals and sound):

| Event | When |
|---|---|
| `ShotFired { ship, piece, gun, kind, from, to, projectile }` | every pulse (`to` = where it ended) and every projectile (`to` = end of its range); not for beams |
| `Hit { by, ship, piece, part, point, normal, damage, absorbed, family }` | damage reached a part (after shields); beams: once per tick on target |
| `ProjectileSpawned { id, owner, pos, vel }`, `ProjectileGone { id, pos, reason }` | `reason` = `Hit`, `Absorbed` or `Expired` |
| `Ricochet { id, point, normal, vel }` | a kinetic shot glanced off a slope |
| `ShieldHit { by, ship, piece, part, point, absorbed, left, depleted }` | a shot entered a bubble (`point` = entry), or a bubble covering a plume's hit point or a ram's contact point took its share; `depleted` = it collapsed |
| `PlumeHit { by, engine, ship, piece, part, point, damage, heat, absorbed }` | an engine plume burned a part, one per lane and part per tick ([Engine plumes](#engine-plumes)) |
| `Explosion { ship, piece, part, at, radius, damage }` | a destroyed part exploded (a chain reaction, after its `PartDestroyed`; [Damage and breakage](#damage-and-breakage)) |

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
(Accepted, provisional) for Niklas to review. Code: `src/combat/plume.rs`; numbers are **placeholders**.

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
  and the heat goes with the damage in proportion (*lead default*). A plume that does only heat (content
  `damage: 0`) loses each covering bubble's energy share of its heat without draining the bubble (heat is not
  damage; 13 Oct 2026 review fix, the review agent's default: until then such a plume ignored shields).
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

Every piece of a split (the kept one and the new ones) is a ghost for `split_ghost_ticks` (10) physics steps
from the next one (a split resolved after a tick's physics step, by a ram, counts from the next tick's step; until
the 13 Oct 2026 review fix those ghosted one step less): a ghost group in its collider's user data (the id of the ship that split) plus Rapier's contact-pair filter hook, which
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

A **ram prow** (content `ram: (dealt, taken)`, GDR-0102) as the touching part scales what each side takes: the other
side `damage × dealt` (2), its own `damage × taken` (0.5); `Contact::damage` stays the unscaled value. Each side's
damage goes through that side's shield bubbles first (GDR-0045), then hits the touching part
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
crush runs in a straight line from that point. A ram prow (above) or a reinforced prow (high-hp armour at the nose) is the ramming build (GDR-0019,
GDR-0047).

## Living parts (GDR-0099, GDR-0102)

Code: `src/ship/living.rs`; numbers in content (`grown_heart`, `ram_prow`, the `grown_tissue` material), lead defaults.

- **Healing**: a part with `PartSpec::heal` > 0 (a hull part in an armour with `heal`: `heal × area`; another part its
  own `heal`) regains `heal × DT` hit points a tick while it is alive on the ship's controlling piece, up to its
  `PartSpec::hp`. Salvage pieces and wrecks do not heal. One pass over `ShipDesign::healing`.
- **Regrowth**: each heart (`ShipDesign::hearts`, content `heart: (interval, hp)`) has a timer (`Ship::regrowth`, in
  the state hash only for ships with hearts). It runs while the heart is alive, not overheated and not idle on the
  controlling piece; at `interval` it starts over and the heart re-grows the best candidate: a part of the design
  that is not alive with 0 hit points (destroyed or stripped; a part that left with a split-off vessel keeps its hit
  points and lives on there), not a heart, whose cells are free in the ship's grid (`ShipGrid::is_free`), linked to an
  alive part of the heart's piece and to no alive part of another piece (linked alive parts always share a piece),
  nearest the heart's centroid first (then the lower index). `Physics::overlaps_other` refuses a candidate whose
  polygon (shrunk a tenth towards its centroid) overlaps any body but the heart's own piece; the next one is tried.
- **Re-growing** (`Ship::regrow`, the reverse of `remove_part`): alive, `heart.hp` of its hit points, cold, not
  overheated, its bubble down (an emitter recharges from 0), back in the grid; its own and its neighbours' exposure,
  and their and their neighbours' bonuses, refreshed; added to the piece's parts (ascending) and the piece marked.
  `SimEvent::PartRegrown { ship, piece, part }`; the sim then resolves the ship at once (`resolve_ship`): the piece
  is rebuilt (mass, colliders, gun lines, power and heat lists, thrusters; the centre-of-mass velocity follows as on a
  lost part), `PieceChanged`, the Rapier body gets the new shape and mass. A logic core back runs the mind pass.
- Cost: healing one pass over the healing parts a tick; a heart one comparison a tick and, once an interval, one pass
  over the design's parts and their neighbours (plus a sort and a shape query per tried candidate).
- Tests (`tests/living.rs`): one part an interval nearest first at 35 %, the body's mass; only from a living
  neighbour; a dead heart and a salvage heart re-grow nothing; a broken-off chunk is never grown back on (its link waits
  until it is gone); never inside another body; tissue heals on a vessel, not on salvage; a prow deals 2 × and takes
  0.5 ×; the `grown` scenario deterministic, its caches right.

## Hot stars (GDR-0102)

A cluster type's `star: (heat, distance)` lies `distance` cluster radii from the centre, in a direction the galaxy's
interior rolls after its sites (`galaxy::InteriorStar`, whole metres; none drawn for a type without one, so other
interiors roll and hash as before). `VoyagePlan::roll_interior` makes it a `sim::Star { pos, heat, reach }` (`reach`:
its distance from the centre); `Voyage::star` holds it (in the voyage's hash only when there is one). Per ship and
tick, `Star::heat_at(p) = heat × min((reach / |p − pos|)², data::STAR_HEAT_CAP)` at the controlling (else first)
piece's centre of mass goes into `Ship::update_systems(star)`, and every part gains `star × open_sides × DT` heat
([Heat](#heat-dynamic-lumped-per-part)): overheating, the restart, conduction and radiators as before. One
multiplication and addition per part a tick. Only `+ − × ÷` and `min`.

## Damage and breakage

- `Sim::apply_damage(ship, DamageTarget::Part(i) | Cell((x, y)), amount)` → `Ignored`, `Damaged { hp_left }`
  or `Destroyed { excess }` (the excess carries on in penetrating shots and ram crush). A destroyed part leaves
  the grid at once; its collider stays until breakage is resolved (shots pass through it, see [Ray casts](#ray-casts)).
- Damage comes from scripted commands, weapons, engine plumes ([Engine plumes](#engine-plumes)) and rams
  ([Collisions](#collisions-impacts-and-ramming-v1)).
- `Sim::resolve_breakage()` (steps 3, 7 and 9 of every tick) first sets off the chain reactions (below), then
  processes each damaged piece once: drop dead parts, flood-fill the rest; one component → `SimEvent::PieceChanged`;
  several → `SimEvent::Split`; none → `SimEvent::PieceDestroyed`. Every destroyed part emits
  `SimEvent::PartDestroyed { volatile }` (in the order the parts died; `volatile` = it has an explosion), an exploding
  one followed by its `SimEvent::Explosion`.
- On a split the main component keeps the piece id, frame and body. Every other component gets a new
  `PieceId`, a frame on its own centre of mass, the parent's angle and angular velocity, and
  `linvel = v + ω × (com_new − com_old)` (`ship::inherited_velocity`). The kept piece's `linvel` is corrected
  the same way. Tests check that no surviving part moves and that linear momentum is conserved. A new
  component that has a logic core then moves to a ship of its own, a vessel (`SimEvent::VesselFormed` after the
  `Split`; see [Vessels](#vessels)).
- `Sim::raycast(origin, dir, max, exclude)` → `PartHit { ship, piece, part, hit }`.
- **Chain reactions** (GDR-0019, [GDR-0084](gdr/0084-chain-reactions-v1.md), [GDR-0085](gdr/0085-chain-reactions-v2.md),
  14 Oct 2026; the rules decided by Niklas, the details and numbers lead placeholders in content; code: `src/combat/chain.rs`).
  A part with an `explosion: (damage, radius)` block that damage destroys (shots, plumes, rams, blasts, scripted
  damage; never salvage stripping or a spawned wreck's pre-breaking, which use `Ship::remove_part`) blasts every
  other alive part with a cell whose nearest point lies within `radius` m of the part's centre, **on any piece of
  any ship**: its own piece, its ship's split-off pieces, other ships, wrecks, salvage (loose fragments and cargo are
  not parts and are not hit).
  - **Damage**: `damage × PartSpec::blast_factor` of the target (a hull part: its armour's `blast_factor`; others 1),
    then `× blast_factor` of every **intact armour part on the line**: a part with a `blast_factor` below 1, alive on
    its piece when the blast goes off, whose cell the straight line from the blast's centre to the centre of the
    target's nearest cell passes through (a stretch of positive length: touching a corner or running along an edge
    does not count), not the target, not the exploding part. Machines shield nothing; a slope blocks its whole
    cell. No falloff.
  - **Its own piece** (as it was before this resolution): the design compiles the targets once
    (`ShipDesign::blast`: `BlastTarget { part, damage, cover }`, ascending part) with the armour on each line
    (`ShipDesign::blast_cover`); the sim skips targets and cover that are dead or on another piece by then.
  - **Every other piece**: found when the blast goes off. The blast's centre in the world (its piece's pose now);
    each piece of each ship (ship index, then piece order) whose bounding circle (`Piece::radius` + the blast radius
    + 1 m of slack for slope cells) can reach it; the blast's centre in that piece's blueprint frame and the grid
    cells in reach there (`chain::in_reach`, the same rule as the compiled lists). The cover counts the target
    piece's armour on its stretch of the line and the exploding piece's on its stretch (both blueprint frames); a third
    piece in between does not count. No physics query, no allocation once warmed up.
  - **Shields**: on other pieces, the bubbles of this tick that the blast crosses (they cover the target's cell centre
    and not the blast's centre) take their share first, `absorb[Missile]` (`chain::BLAST_FAMILY`), one target at a
    time, in bubble order, up to what they hold (`SimEvent::ShieldHit` with `by` = the exploding ship). The exploding
    ship's own bubbles never (the blast goes off inside them). **Blasts spare no one**: unlike shots, plumes and rams
    (GDR-0048, GDR-0046, GDR-0067), friendly vessels are hit and their bubbles absorb (lead default).
  - **Order**: `Sim::resolve_breakage` sets off every chain before any ship's breakage is resolved (and
    `Sim::resolve_ship` before its fragments, so every part a blast destroys leaves its fragment): one queue over every
    ship's destroyed parts, ships in index order, each ship's parts in the order they died; every part a blast destroys,
    on any ship, joins the end at once, so a whole chain resolves **in the same tick**, first destroyed first. Each
    blast computes all its hits from the state before it (what is alive and what covers what: armour the blast breaks
    still shields what lies behind it from that blast; a later blast of the chain meets only what is left), then
    applies them through `apply_damage`: its own piece's targets ascending, then the other pieces in ship and piece
    order, their targets ascending. `Ship::resolve_breakage` on its own (a ship without a world) sets off only the
    own-piece part of what is left.
  - `SimEvent::Explosion { ship, piece, part, at, radius, damage }` follows the part's `PartDestroyed`: `at` is the
    part's centre in the world as the piece was before the breakage. Parts it destroys on other ships appear with
    those ships' events. The client draws nothing for it yet (a follow-up).
  - Base content (GDR-0085): fission reactor and automaton core 1.5 within 1.5 m (the 8 cells around a one-cell part
    and the next one straight out on each side), fusion reactor 2.5 and torch drive 1.2 within 2 m (the 12 cells
    around a 2 × 2 and the 8 two cells straight out), shield emitter 1 within 0.6 m, capacitor, thruster and RCS pod
    0.5 within 0.6 m (the 4 cells beside them); armour `blast_factor` 0.5 (reactive 0.25). A fission reactor takes
    every 1-hit-point machine around it, another reactor included (a chain), leaves armour blocks standing, and a
    machine two cells out behind a steel block takes 0.75 and survives.

## Salvage

Salvage v1 (13 Oct 2026; GDR-0063, track 7B of the first playable loop, `docs/plans/first-playable-loop.md` 3.4 and
5.5). Code: `src/combat/salvage.rs`. Every number is a **placeholder** (`SimConfig::salvage`); rules marked *lead default*
were set by the salvage agent for Niklas to review.

### Items and cargo

- **Items** (`Item`): `Part { part, material }`, a whole part by content id (hull parts carry their armour
  material), `Fragment { part }`, what a destroyed part leaves if its `salvage` block names a `fragment` (the
  content id of that part), and `DataCore { trail, signal }` (16 Oct 2026, galaxy track E4, GDR-0096): a log of the
  galaxy's trail signal, one cargo unit worth `salvage::DATA_CORE_VALUE` (1000) a unit, so a hold takes it first and a
  shrinking hold drops it last; a voyage puts it into the first wreck of a log's site when the site spawns
  (`Sim::stow_data_core`; loose at the site, never expiring, without a wreck). `Item::haul()` is the career's copy
  (`data::career::HaulItem`). Ordered: cargo lists and ties follow this order.
- **What counts** (the content's `salvage` blocks, `docs/content-model.md` "Salvage", compiled into the design as
  `PartSpec::salvageable` and `PartSpec::fragment`, so the sim never looks content up per tick): only a part with a
  `salvage` block is collected (a hull part: its armour material's block), and only a part whose block has a
  non-empty `fragment` leaves a fragment when destroyed. In the base pack every part and armour material has a
  block and exactly the two cores name a fragment.
- **Cargo units** (code rule): a part takes its footprint cells, `size.0 × size.1` (a slope its whole footprint;
  `PartSpec::cargo_units`): a beam laser 2, a fusion reactor 4; a fragment 1.
- **Value** (*lead default*; the sim must not depend on the career's research values): build points per cargo unit,
  hull parts with their material's `build_point_factor` (`PartSpec::build_points`); a fragment is worth its part's
  build points. What an item gives in research data and materials at home is the career's business
  (`data::career::unload`); the sim reads only whether a block exists and names a fragment.
- **The hold** (`Ship::cargo()`, `Cargo`): per item the count, the units each takes and its value per unit
  (`Stack`), in item order (a `BTreeMap`), and `used()`. `Ship::cargo_capacity()`: the summed `cargo.capacity` of
  the alive cargo parts of the controlling piece (`Piece::cargo_capacity`, summed in part order on piece rebuilds,
  rounded down); 0 for salvage. Base content: cargo bay 4, cargo hold 20; the Runner and the two sample ships carry
  8.
- **A hold that shrinks** (a cargo part destroyed or split off): the lowest-value items go, one at a time (ties: the
  last in item order first), until the load fits (`Sim::shed_cargo`, after each breakage of a vessel), and **spill as
  loose items** that can be collected again (GDR-0076, Niklas 14 Oct 2026; until then they vanished, a GDR-0066 lead
  default): `SimEvent::CargoLost { ship, item, count }` per kind dropped, in the order they went, and one `Loose`
  per item dropped (see [Fragments](#fragments)). The haul is what the holds carry when a voyage ends, so an item
  dropped and scooped again counts once. A ship with free room may scoop a smaller item it just dropped right away
  (the drop order can free more units than that item needs).
- **A lost vessel's ship keeps its cargo**: its wreck carries it (no capacity check once the ship is no vessel), and
  a collector takes it before the wreck's parts. A vessel that splits off starts with an empty hold; the cargo stays
  with the ship that keeps the controlling piece.

### Collecting

Every `interval` ticks (10, so at most 3 items a second per vessel), right after the weapons phase's breakage, each
running vessel (`VesselState::Running`: not dormant, not dark) of an owner that collects (`collects`: the player
only) with free units takes one item, in ship order, from salvage that moves at most `max_speed` (8 m/s since
GDR-0079, 14 Oct 2026; 4 m/s before) relative to it (GDR-0063's "fly slowly over a wreck": the velocities of the two centres of mass, a fragment's own; checked
before the distance, two subtractions per candidate):

1. the nearest loose item (a fragment or spilled cargo) that fits, within `reach` (5 m) of its hull (ties: the
   lower id); else
2. the salvage pieces within reach, nearest hull first (ties: ship, then piece order), the first one with something
   that fits: its ship's cargo if that ship is no vessel (a wreck's own cargo before its parts: the best item, ties
   the first in item order), else the piece's best part (highest value per unit, ties the lowest part index).

Salvage pieces are every piece that is no controlling piece: wrecks, split-off debris (the collector's own too) and
the cored pieces of a burned-out derelict. Only what fits counts: with 2 units free a 4-unit fusion reactor waits and
smaller parts go first; a full hold takes nothing; only salvageable parts count (`PartSpec::salvageable`).

- **A stripped part** leaves through the destroy path without the destruction (`Ship::remove_part`, which
  `apply_damage` shares): dead, out of the grid, its neighbours' exposure and bonuses refreshed, its piece re-checked;
  no `PartDestroyed`, no chain reaction, no fragment. Breakage then runs as for any destroyed part (`PieceChanged`,
  `Split`, `PieceDestroyed`): the wreck may break up further.
- `SimEvent::Collected { ship, from, item, units, at }`: `from` is `Salvaged::Part { ship, piece, part }`,
  `Cargo { ship }` or `Loose(id)`; `at` where the item was (world). It comes before the breakage events of the pass.
- **One strip per piece and pass**: the stripped pieces' breakage is resolved once, after the pass, so their
  colliders are stale until then; a later collector of the same pass leaves a piece stripped earlier in it for the
  next pass (13 Oct 2026 review: it could reach parts through a part already taken). In ship order, so the first
  collector at a piece goes first every pass.
- **Reach**, hull to hull: a broad test on bounding circles about the frame origins (`Piece::radius`), then the exact
  distance between the two pieces' convex colliders in the collector's frame (corner to edge; 0 when a corner lies
  inside the other); a fragment by its distance to the collector's colliders.
- The pass runs before the physics step, so Rapier takes the stripped pieces' new mass properties in the same tick
  (as for weapon damage).

### Fragments

- A part whose `salvage` block names a `fragment` (in the base pack: logic cores and automaton cores) **destroyed by
  damage** leaves a fragment (GDR-0063:
  programmable matter to scoop): a `Loose` item (`id`, `item`, `units` 1, `value`, `pos`, `prev_pos`, `vel`,
  `expires`) at the part's centroid with the velocity of that point of its piece (`v + ω × r`), made in
  `Sim::resolve_breakage` before the piece is rebuilt.
- **The cheapest representation**: a point in `Sim::loose()` (ascending `LooseId`, monotonic), no Rapier body, no
  collisions, moved after the physics step by `pos += vel × DT` (exact IEEE operations), until it is collected or
  the sim ends (GDR-0079, 14 Oct 2026: loose items last until the voyage ends; 60 s before). A config with a
  `fragment_ticks` lifetime ends it after that many ticks (`expires`: the tick it is gone at, `None` without
  one); the client's endless sandbox keeps the old 60 s so they cannot pile up there (`docs/client.md`). Parts
  stripped by collectors and parts removed when a wreck spawns leave none.
- **Spilled cargo** (GDR-0076): each item a shrinking hold drops becomes a `Loose` of that item (a whole part stays a
  whole part, with its cargo units and value per unit), at the lost hold, the centroid of the first cargo part
  destroyed in that breakage (`Ship::part_world`, the pose of the piece it was on; else, when the hold only split
  off, the controlling piece's centre of mass), with the velocity of that point plus a push of
  `spill_speed × [0.5, 1)` in a direction rolled on `Stream::Debris` (two draws per item, in drop order; the stream's
  only use so far). Same lifetime and collecting rules as a fragment; the client draws every loose item the same
  way for now.

### Wrecks spawned as wrecks

`Sim::spawn_wreck(design, owner, body, damage, seed)`: the ship spawns **burned out** (`Ship::is_burned_out`: no
controlling piece, every piece salvage, and `form_vessels` skips it, so its cores never form a vessel and can be
stripped intact), then `floor(damage × parts)` distinct parts (a partial Fisher–Yates shuffle on a generator of its
own, `Pcg32::new(splitmix64(seed), 5)`, so the same seed gives the same wreck whenever it spawns and no stream of the
sim is drawn) are removed without destruction, and only the wreck's breakage is resolved (damage pending on other
ships stays pending until the next step): it splits into pieces as usual. A NaN or infinite `body` counts as 0, as
in `spawn_ship`. Events:
`ShipSpawned`, then `Split` / `PieceChanged` / `PieceDestroyed`; no `PartDestroyed`, no `VesselFormed`, no fragments.
A derelict's core shot later leaves a fragment like any other. `owner` is kept (`Ship::owner`) for salvage rights
later; the wreck counts for nobody. Used by the salvage scenario and the voyage director ([Voyages](#voyages)).

### Settings (`SimConfig::salvage`: `SalvageTuning`, placeholders, lead 13 Oct 2026; Niklas's review 14 Oct 2026, GDR-0079)

| Field | Default | What |
|---|---|---|
| `interval` | 10 ticks | one item per collecting vessel per pass; 0 turns collecting off |
| `reach` | 5 m | hull to hull |
| `max_speed` | 8 m/s | relative speed of the salvage to the collector (centres of mass; a fragment's velocity) above which it is passed by (GDR-0063 "fly slowly"; 13 Oct 2026 review; 4 m/s until GDR-0079) |
| `fragment_ticks` | `None` | how long an uncollected fragment (or spilled item) drifts, in ticks; `None`: until the sim ends (GDR-0079; 1800 = 60 s before, which the sandbox keeps) |
| `spill_speed` | 1.5 m/s | spilled cargo's push away from the lost hold: half to all of this, a random direction (GDR-0076) |
| `collects` | owner 0 only | owners whose running vessels collect (by `Owner.0`; owners beyond the table never) |
| `settle_speed` | 3 m/s | salvage (a piece that is not a vessel, a loose item) faster than this slows down to it (GDR-0089; Niklas, 15 Oct 2026: cargo dropped near his top speed was very hard to reach) |
| `settle_decel` | 4 m/s² | how fast it slows: a wreck from 20 m/s drifts slowly after about 4 s and 50 m (`SalvageTuning::settle`; pieces by a force in the physics phase, loose items in `drift_loose`) |

### State hash

Per ship the burned-out flag and the cargo (units in use; per item its kind, its content ids as bytes, count, units
and value bits); after the projectiles the loose items (the id counter, then per item id, item, units, value,
position, previous position, velocity, and whether it expires and when: a flag, then the tick or 0); the salvage
settings with the config (`fragment_ticks` the same way, then `settle_speed` and `settle_decel`).

### Cost

Every `interval` ticks, per running collecting vessel with free units: one bounding-circle test per loose item and
salvage piece in the world; for those that pass, the two pieces' colliders pair by pair, skipping pairs whose
bounding boxes are farther apart than the reach (or the best distance so far), then one sort of the few pieces in
reach (a reused buffer). A full hold, a vessel without a hold and every other owner cost one check. Every tick:
fragments move (one add each; usually none) and three empty checks per ship in breakage. No allocation once warm,
except the item strings of what is collected (at most once per vessel per pass).

Measured 13 Oct 2026 (native release, VM load average 9.4–10 from other agents' builds, 7 interleaved runs of each
build, medians of the runs' medians and p95, ms):

| What | Before (`dev` 4f4d68b) | With salvage |
|---|---|---|
| `sim_perf` raw throttles, 20 × 2000 parts (no collectors), step median / p95 | 0.370 / 0.567 | 0.375 / 0.504 |
| `sim_perf` pilot + lock, step median / p95 | 0.394 / 0.542 | 0.322 / 0.508 |
| `sim_perf` combat, step median / p95 | 0.608 / 2.486 | 0.631 / 2.002 |
| `state_hash`, 20 × 2000 parts, mean | 0.329 | 0.324 |
| load test, 20 ships (10 Runners collect), step median / p95 | 0.019 / 0.044 | 0.017 / 0.040 |

All within the VM's noise. The collect pass alone (a throwaway timing, not in the repo: 20 Runners, 12 of them
running, each beside a `sample_mid` wreck with 30 % of its parts gone, 600 ticks): the weapons phase on collect ticks
took 0.0055 ms (median) against 0.0017 ms on the others with every wreck just out of reach; with the wrecks in reach
(77 parts stripped while the holds filled) the collect ticks' p95 was 0.10–0.14 ms, most of it the rebuild of the
stripped pieces (as for a destroyed part). Comparing every collider pair in full (the first version) cost 0.34 ms
per pass in the out-of-reach case.

### Determinism and scenarios

`ScenarioKind::Salvage` (`--scenario salvage`): a wreck of every blueprint spawned as a wreck in a ring 30 m out
(20–50 % of its parts gone, drifting and turning a little), one collector per ship-mind blueprint with a hold 45 m out
(Runner, sample early, sample mid) flying to the nearest salvage (`Approach` on the piece, `Hold` on a fragment) every
30 ticks, two raiders under `raider_basic` whose cores are shot out at ticks 120 and 240 (fragments), the second
collector's first cargo bay destroyed at 450 (its hold sheds; the items spill), and the first collector stopped and
then lost at 540 (its wreck keeps its cargo), which the last collector, idle until then, strips: fragments, the
wreck's cargo, then its parts. Plus random hits (4 % a tick) and a blast every 300 ticks.

13 Oct 2026, release builds, 1800 ticks, native x86_64 vs wasm32-wasip1 under Node 24: salvage, core, flight, load
(20 ships) and behavior, seeds 42 and 7: **identical** (10 runs). Seed 42: salvage 16 items collected (19 units), 5
from a wreck's cargo, 2 fragments of 6 spawned; seed 7: 22 items (24 units), 1 fragment.

The default config lets the player collect, so the player's ships in the core, flight, load and behavior scenarios
collect too: their hash streams change from tick 1 (the salvage settings, the sixth RNG stream and the cargo are
hashed) and so do their fights (seed 42, 1800 ticks: core 24 items collected, flight 10, load 19, behavior 12).

**After the 13 Oct 2026 review fixes** (salvage blocks, the slow pass, wreck seeds, one strip per piece and pass,
`CargoLost`, `Ship::part_world`; on `dev` with the GDR-0064 flight changes; release builds, 1800 ticks): voyage,
salvage, core and behavior, seeds 42 and 7, native x86_64 vs wasm32-wasip1 under Node 24: **identical** (8 runs). All
four streams differ from before from tick 1 (the salvage settings hash `max_speed`; the salvage scenario rolls its
wreck seeds on the scenario stream; the behaviors' part aim uses `Ship::part_world`). Seed 42: salvage 27 items (28
units), 3 fragments of 8; core 18 items; behavior 11. Seed 7: salvage 29 items, 0 of 6 fragments; core 13; behavior
19.

**After spilled cargo (GDR-0076, 14 Oct 2026**; release builds, 1800 ticks, seed 1): salvage and voyage, native
x86_64 vs wasm32-wasip1 under Node 24: **identical**. Every stream differs from before from tick 1 (`spill_speed` is
hashed with the settings). Salvage: 19 loose items instead of 8 (the bay shot at 450 spills 11; its collector's hold
stays full, so none is scooped again), the same 21 items collected, final hash `920a76757615f4e4` → `5e5f19c9f6a52559`.
Voyage: no hold shrinks, every counter as before, `c4decb04b0dde2e8` → `e51add61e9e4465f`.

**After Niklas's review of the lead defaults (GDR-0079, 14 Oct 2026**: `max_speed` 8 m/s, loose items without a
lifetime; release builds, 1800 ticks, seeds 1 and 42): salvage, voyage and core, native x86_64 vs wasm32-wasip1
under Node 24: **identical** (6 runs). Every stream differs from before from tick 1 (`max_speed` and the new
`fragment_ticks` encoding are hashed with the settings), and the faster pass changes what is collected (before →
after): salvage seed 1 21 → 26 items (`5e5f19c9f6a52559` → `4aa1e9d8e9089bd6`), seed 42 28 → 32
(`b27b59cd96f41f17` → `e3c6d1d7df8e6ff4`); voyage seed 1 7 → 8 (`e51add61e9e4465f` → `1af36d348b10dac0`), seed 42
12 → 13 (`708bc0c46ea3bb53` → `ec9d31c4fda326d7`); core seed 1 17 → 14, seed 42 23 → 27.

## Voyages

The voyage director (13 Oct 2026; GDR-0062, ADR-0033, track 7E of the first playable loop,
`docs/plans/first-playable-loop.md` 3.3, 3.4 and 5.5). Code: `src/voyage/mod.rs`, tests `tests/voyage.rs`. A voyage is one
small cluster, one `Sim` (ADR-0022). Every number is a **placeholder**; rules marked *lead default* were set by the
voyage agent for Niklas to review. The content side (`voyages`, `encounters`) is in `docs/content-model.md`, the
career side (open voyages, voyage seeds, unloading the haul) in `docs/career.md`.

### Rolling a plan

`VoyagePlan::roll(&content, &voyage, seed) -> Result<VoyagePlan, String>` (the client passes the career's voyage
seed, `Career::voyage_launched`; compile the plan from the content the enemies fly with: the client's other owners'
content when the player's tuning differs). It draws on a generator of its own,
`Pcg32::new(splitmix64(seed ^ splitmix64(PLAN_STREAM)), PLAN_STREAM)` with `voyage::PLAN_STREAM` (`"voyage"` in
ASCII, no stream's id), so it never repeats the draws of any of the sim's streams for the same seed (the voyage
runner, the scenario and the tests seed the `Sim` with the voyage seed too). The draw order is the replay contract
(`data::career::GENERATOR` 2, `docs/career.md` "Voyage roller versions"):

1. **Sites**: the voyage's site entries, each repeated `count` times, keep the content's order in `plan.sites`. Each
   gets a spot uniform in the cluster's disc (radius `voyage.radius` about the world origin; `sqrt` and libm trig,
   so it is the same everywhere): the first spot whose nearest already placed spot (the sites placed so far and the
   **entry** at `(0, −radius)`) is at least `spacing.0` and at most `spacing.1` away. Every other site is placed
   first, in order; the goal last, as the farthest from the entry of the first 8 good spots. With no good spot in 64
   tries, the spot that misses the spacing least (none of the base voyages needs it for seeds 1–20, a test checks).
2. Then **per site, in plan order**, all of its draws before the next site's:
   1. **Counts**: each ship entry of its encounter, then each wreck entry, rolls its count in `(min, max)`, both
      included.
   2. **Layout** (*lead default*): a spot for each ship, then each wreck, uniform within 30 m of the site, their hull
      circles at least 5 m apart (the disc widens by 10 m after every 16 misses; a hull circle is centred where the
      pose puts the main piece's centre of mass and reaches the blueprint's farthest corner, so the gap holds for
      lopsided blueprints too); then a heading for each, in the same
      order; all at rest. Wrecks keep their entry's `damage` share.
   3. **Wreck seeds**: one `u64` per wreck (`Spawn::seed`), from which `Sim::spawn_wreck` picks the parts already
      gone, so a wreck does not depend on when or in which order the sites spawn.

The plan holds the compiled designs (`Arc<ShipDesign>`, one per blueprint), every pose, the site's encounter id, goal
flag and wake radius, and `plan.entry`: where the fleet enters, at `(0, −radius)` with the nose to the centre.

### Running it

`Sim::start_voyage(plan, player, enemy)` hands the plan to the sim (replacing a voyage underway); the caller sets the
behavior library (`set_behaviors`) and spawns the fleet at `plan.entry`. From then on, **at the end of every step**
(after the impacts, before the hash; phase 10 of [The tick](#the-tick)) the director:

1. **Spawns** every site that has not spawned yet once a vessel of `player` (not dormant) is within
   `VoyageTuning::spawn_radius` (600 m) of it: its ships for `enemy` (`spawn_ship`; each holds where it spawned:
   `ShipCommand::Maneuver { Hold, fire: false }`, and so do the vessels a blueprint in several cored pieces forms at
   spawn, which join the site there), then its wrecks (`spawn_wreck` with each wreck's seed, owner `enemy`). A site
   with ships is **asleep**; a site that rolled none (a derelict field without its raider) is **quiet**.
2. Reads the events since its last pass: a vessel that splits off one of a site's ships (`VesselFormed { from }`)
   joins that site (inserted in id order); a `Hit`, `PlumeHit` or `ShieldHit` (a shot its bubble took whole) on a
   site's ship, or an `Impact` with ram damage on either side, marks its site; so does a `Collected` from one of its
   wrecks (a part or its cargo stripped, GDR-0102).
3. **Wakes** a sleeping site when a player vessel is within the encounter's `wake_radius` of the site (not a **wary**
   site, `SitePlan::wary`: every ship of its encounter is of a faction with `wary: true`, the Grown's, GDR-0099 A3;
   it wakes only when provoked), one of its
   ships was marked, or one of them is no running vessel any more (*lead default*, the last two: a sniped, shot at or
   rammed sleeper wakes its pack): every ship of the site gets its behavior (`BehaviorLibrary::default_for`: its own or its
   faction's); `SimEvent::SiteWoke { site }`.
4. **Clears** an awake site when each of its ships (split-off vessels included) is destroyed or disarmed
   (GDR-0068): none is a vessel (running, dark or dormant) that is still armed: `SimEvent::SiteCleared { site, goal }`.
   Clearing is final, so the passing states don't count: a dark or dormant enemy that is armed keeps its site open
   (13 Oct 2026 review). A quiet site is never cleared.

**Armed** (`Ship::is_armed`, GDR-0068; the rule is a *lead default*): the controlling piece still has a gun and a
power source, both alive parts (`Piece::guns` and the piece's reactors, rebuilt on every breakage, so the check is
one look-up). Only losses that never come back disarm: a gun that overheats, idles (control capacity), browns out
or has its line of fire blocked by its own hull still counts (the blocker can be shot off); a piece without a
reactor never fires again. `Sim::awake_enemy(owner, ship)`: a running vessel of another owner that is not one of a
sleeping site's ships (`Voyage::asleep`); `Sim::armed_enemy(owner, ship)`: an awake enemy that is armed. The
recall, the voyage scenario and the voyage runner use these two, so they cannot drift apart.

Sites are indexed in plan order (`site` in the events). Owners: everything is keyed on the `player` and `enemy`
owners, so co-op can add player owners later (the spawn and wake tests would take every player owner's vessels).

**Recall** (`Sim::can_recall(owner)`, GDR-0062, GDR-0068, GDR-0074): `owner` has at least one vessel, and either no
armed awake enemy (`Sim::armed_enemy`) is within `VoyageTuning::recall_radius` (200 m, centre of mass to centre of
mass) of any of `owner`'s vessels (a disarmed enemy, a sleeper and a dark or dormant enemy never block it), or none of
`owner`'s vessels (running, dark or dormant) has a working gun (`Sim::fleet_disarmed(owner)`, `Ship::is_armed`): the
**emergency recall**, the same warp, so a disarmed ship is never kept from home by a raider of its own top speed. One
armed escort makes the fleet armed again. It works without a voyage too (sandbox waves count as awake). The 3 s
warp-out is the client's.

Queries: `Sim::voyage() -> Option<&Voyage>`: `id`, `seed`, `radius`, `entry`, `player`, `enemy`, `sites()`,
`site_of(ship)` (a site's ship, split-off vessel or wreck), `goal_cleared()`; per `Site`: `encounter`, `goal`, `pos`,
`wake_radius`, `state()` (`Unspawned`, `Quiet`, `Asleep`, `Awake`, `Cleared`), `ships()`, `wrecks()` (ascending
ids; some may be gone or salvage by now), `plan()`.

### Settings (`SimConfig::voyage`: `VoyageTuning`, placeholders)

| Field | Default | What |
|---|---|---|
| `spawn_radius` | 600 m | a site spawns when a player vessel comes this close (the plan's lazy spawning) |
| `recall_radius` | 200 m | no armed awake enemy vessel this close to recall (GDR-0062, GDR-0068), unless the fleet is disarmed (GDR-0074) |
| `signal_reveal` | 400 m | a signal's site shows when a player vessel comes this close (if its sensors reach it) or under a sweep (GDR-0095) |
| `edge_margin` | 500 m | the cluster's edge lies this far outside its radius, at most `voyage::MAX_EDGE` (4 km) from the centre (GDR-0095) |

The layout numbers of the roll (30 m spread, 5 m gap, 64 tries, 8 goal choices) are constants in `voyage/mod.rs`.

### State hash

After the loose items: the voyage, if one is underway: its id, seed, radius, entry, owners, and per site the plan
(encounter id, goal, spot, wake radius, every spawn's design id, pose, damage and seed), the state and the ids of its ships
and wrecks. `VoyageTuning` with the config.

### Cost

Per step with a voyage: one pass over the step's events, the player's vessels, one distance test per site and
player vessel, and for sleeping and awake sites one ship look-up per site ship (the wake test, the clearing test);
nothing allocates once warm except the ships a
site spawns. Spawning a site costs its ships' `Ship::new` and bodies once (a raider pair or a den: a few
microseconds). The voyage runner below steps whole voyages (a ship, one to three sites' ships and wrecks in the
world) at 0.005–0.007 ms median and 0.008–0.017 ms p95 per step (native release, 13 Oct 2026, load average 8–11 from
other agents' builds, 20 seeds per voyage).

### Determinism and scenarios

`ScenarioKind::Voyage` (`--scenario voyage`): the content's first voyage (`yard_approach`) rolled from the seed and
started for the player against `Owner(1)`, one ship per ship-mind blueprint with a hold (the starter, the Runner and
the two samples) in a row at the entry, the first the focus. Every 30 ticks (staggered) each fights the nearest armed awake
enemy (`Sim::armed_enemy`) within 300 m with random pilot input (locked on it, firing, the stick roughly towards it), else flies to the
nearest salvage within 150 m (as the salvage scenario), else holds at a site of its own among those not cleared;
the cores of the first armed awake enemy are shot out every 150 ticks, plus random hits (3 % a tick) and a blast every 300
ticks. The summary counts sites woken and cleared and the ticks the player could recall.

13 Oct 2026, release builds, native x86_64 vs wasm32-wasip1 under Node 24: voyage 1800 ticks seeds 42 and 7, core and
salvage 900 ticks seed 42: **identical**. Seed 42: 3 sites woken and cleared, 13 items collected, recall allowed 1399
of 1800 ticks; seed 7: 3 and 3, 19 items, 1313 ticks. Adding the hunter and the Warden to the content changed the
other scenarios' streams once (they spawn every blueprint).

After the 13 Oct 2026 review fixes (GDR-0068 clearing and recall, the salted plan and wreck seeds, the wake on
shields and rams; release builds, 1800 ticks): voyage seeds 42 and 7 **identical** native vs wasm. Seed 42: 2 sites
woken and 2 cleared, 6 items collected, recall allowed 1755 of 1800 ticks; seed 7: 4 woken, 3 cleared, 7 items, 1682
ticks.

After the slice tuning (13 Oct 2026, GDR-0073: the first voyage's goal is a raider pair, the encounters smaller; release
builds, 1800 ticks): voyage, salvage, core and behavior, seeds 42 and 7, native x86_64 vs wasm32-wasip1 under Node 24:
**identical**. Only the voyage stream changed (from tick 1: the plan is hashed); salvage, core and behavior are
bit-identical to before. Seed 42: 2 sites woken and 2 cleared, 6 items, recall 1755 ticks; seed 7: 4 woken, 4 cleared,
7 items, 1682 ticks.

### The headless voyage runner

`cargo run --release -p sim --example voyage -- --voyage <id> --design <ship> --seeds 1..20 [--escort <ship>[,...]]
[--behavior <id>] [--minutes 30]`: the balance tool of track 7K. It rolls the voyage per seed, puts the design at the
entry (and the escorts behind it, under the `escort` template with the flagship as leader) and flies it with a
scripted pilot (see the docs of `sim::voyage::runner`: in fights the balance study's *closing* stand-in, or a content behavior with
`--behavior`; salvage it can catch; the sites in turn, the goal last; home once the goal and the sites are done or
the hold is full, when it is disarmed (`Ship::is_armed`; an overheated or idle gun still counts), or after 3 minutes
without progress; it fights armed awake enemies only, `Sim::armed_enemy`, and, since 18 Oct 2026 (GDR-0103), the
armed ships of a wary site it heads for, which never wake on sight: it opens fire on the nearest within 400 m),
and prints per seed the outcome
(recalled, lost, out of time), goal, sites cleared, time, hull lost, kills, awake enemies left (how many disarmed),
haul items and research data per family (`data::career::unload`; every player vessel's hold), then medians, p95 and
the step time. `--design` and `--escort` also take a blueprint file (`*.ron`, a saved or exported drydock design),
for ships a player would build that are not content.

The pilot and the run are a library since 14 Oct 2026 (`sim::voyage::runner`, for the evolution lab's voyage fitness,
`arena::voyage`): `VoyageRunner::new(content, voyage, Flagship { design, reach, behavior }, behaviors, escorts,
seed, max_ticks)` sets the run up (`behaviors` is the library the world flies with, so a behavior text that is not
content can fly the fights; `reach` is `closing_reach(&report)`), `step(ticks)` runs a slice and returns the
`VoyageRun` once it has ended, `run()` runs it to the end. The example only parses, runs the seeds and prints; its
step time now times each tick of a run (the pilot's decision and the sim step). Moving it changed no number: the
example's output before and after is identical (`halvex_drift` with an escort, seeds 1–12; `yard_approach` with
`--behavior skirmisher`, seeds 1–8). `tests/voyage.rs` checks that a run stepped in slices equals one in one go.

First numbers (13 Oct 2026, content and sim of `dev` after the 7C merge and the sim review fixes, before the GDR-0064
flight fixes; 20 seeds each, closing pilot):

| Voyage | Design | Goal cleared and home | Home without the goal | Lost | Kills per run | Sites cleared | Hull lost (home, median) | Haul items per run |
|---|---|---|---|---|---|---|---|---|
| yard_approach | rim_skiff | 0 | 20 | 0 | 2.9 | 1.2 / 4 | 9.8 of 24 | 3.0 |
| halvex_drift | rim_skiff | 0 | 18 | 2 | 2.0 | 0.5 / 5 | 20.0 of 24 | 1.9 |
| corran_reach | rim_skiff | 0 | 19 | 1 | 1.6 | 0.3 / 5 | 17.3 of 24 | 0.8 |
| wardens_gate | rim_skiff | 0 | 17 | 3 | 1.8 | 0.3 / 4 | 7.4 of 24 | 1.5 |
| yard_approach | halvex_runner | 0 | 16 | 4 | 2.0 | 0.9 / 4 | 56.5 of 72 | 1.6 |
| halvex_drift | halvex_runner | 0 | 17 | 3 | 1.6 | 0.3 / 5 | 58.1 of 72 | 2.2 |
| corran_reach | halvex_runner | 0 | 13 | 7 | 2.2 | 0.7 / 5 | 57.9 of 72 | 0.9 |
| wardens_gate | halvex_runner | 0 | 14 | 6 | 1.8 | 0.3 / 4 | 55.4 of 72 | 1.2 |

Nobody clears a goal yet. What the runs show, for 7K:

- **The fights are lost, not the voyages**: a raider pair out-damages the starter and even the Runner (one Runner run
  on `yard_approach`: 69 cannon hits taken for 40 damage against 197 hits given for 22, spread over armour rather
  than cores). The starter's two pulse lasers have 1 hp each and sit in its nose, the side that faces the raiders'
  75 m cannons, so most runs end disarmed within two or three minutes; the pilot then goes home.
- **Sim issues still in play** (GDR-0064, fixes pending): ships stuck at top speed (with drift on, the closing pilot
  overshot and could not turn back; it now brakes when in reach), the lock's lag on crossing targets, raiders that
  fly off at top speed.
- **Unarmed remnants**: 83 of the 160 runs end with an enemy without a working gun still running. It keeps its
  site awake (it is a running vessel) and, while within 200 m, blocks the recall. The pilot gives up after 3 minutes
  without progress.
- `--behavior skirmisher` on `yard_approach` with the starter: 0 / 17 / 3, worse than the closing pilot.

After the 13 Oct 2026 review fixes (GDR-0068: a site clears when its enemies are destroyed or disarmed, only armed
awake enemies block the recall, the Warden's heart has a cannon; the pilot fights armed enemies only and gives up only
when it is disarmed itself, `Ship::is_armed`), on `dev` with the GDR-0061/0064 balance and flight changes, 20 seeds,
closing pilot, `rim_skiff` (load average 6–9 from other agents' builds):

| Voyage | Goal cleared and home | Home without the goal | Lost | Out of time | Kills per run | Sites cleared | Awake enemies left (disarmed) | Haul items per run |
|---|---|---|---|---|---|---|---|---|
| yard_approach | 6 | 7 | 1 | 6 | 0.0 | 2.4 / 4 | 6.5 (5.6) | 4.2 |
| halvex_drift | 0 | 10 | 4 | 6 | 0.0 | 1.2 / 5 | 5.6 (4.2) | 1.9 |
| corran_reach | 0 | 10 | 6 | 4 | 0.45 | 1.3 / 5 | 6.0 (3.3) | 1.1 |
| wardens_gate | 0 | 12 | 3 | 5 | 0.0 | 1.0 / 4 | 5.9 (4.1) | 0.2 |

Step time median 0.008–0.010 ms, p95 0.011–0.016 ms. What changed, for 7K:

- **The first goals fall** (`yard_approach`, 6 of 20): the starter shoots the raiders' nose cannons off long before
  their cores, so sites now clear by disarming; kills almost vanish (the pilot leaves disarmed enemies alone), and
  most awake enemies at the end are disarmed hulks that still fly (salvage-to-be, but their cores still run, so they
  never become wrecks to strip).
- **Out of time** (19 of 80): the pilot is still armed, the goal is not cleared, and one armed enemy keeps it from
  recalling (it flees and is chased, or the fight never ends); before, it gave up when its guns overheated.
- Disarmed sites leave no wrecks to strip (their cores still run), so the haul stays small.

**The slice tuning** (13 Oct 2026, track 7J/7K, [GDR-0073](gdr/0073-slice-tuning-v1.md), report
[`results/slice-balance-2026-10-13.md`](results/slice-balance-2026-10-13.md)). The pilot first became closer to
sensible play: it stops 40 m outside a sleeping site's wake radius and creeps in at a quarter of its top speed (it
used to meet a waking pack at 22 m/s and, without forward thrust, could not brake with its nose on the target),
eases the stick off as it closes (to 75% of its *shortest* clear gun's reach, so a Runner's beams reach), keeps its
lock on one armed enemy (it switched to the nearest every 10 ticks), finishes off disarmed enemies within 200 m while
the hold has room, strips the best value per cargo unit first and strips what is in reach before recalling. Then the
encounters, sites and research costs were tuned. Goal cleared and home, 60 seeds (1–60), same pilot, before → after
the content change (load average 6–9):

| Voyage | `rim_skiff` | Halvex Runner | Runner + `rim_skiff` escort | Data per home run (starter / Runner) |
|---|---|---|---|---|
| yard_approach | 43% → **58%** | 37% → 60% | – | 8.7 → 10.2 / 9.4 → 10.8 |
| halvex_drift | 0% → **37%** | 0% → 67% | – → 78% | 4.9 → 11.7 / 5.9 → 15.4 |
| corran_reach | 0% → 15% | 0% → **45%** | – → 65% | 4.1 → 8.7 / 3.8 → 11.9 |
| wardens_gate | 0% → 3% | 0% → 20% | 3% → **42%** | 3.1 → 8.9 / 4.1 → 11.3 |

Scripted times 2.5–3.5 min per voyage (median); step time median 0.007–0.009 ms, p95 0.013–0.016 ms.

**GDR-0074** (14 Oct 2026, report section "GDR-0074" in the same file). The emergency recall (a fleet without a
working gun may always recall) ends the runs a disarmed starter used to spend chased until the 30 minutes ran out:
the starter's *out of time* went from 8–19 of 60 per voyage to 0, the Runner's from 1–4 to 0–2 (escort setups keep
3: an armed escort makes the fleet armed). The automaton cannon (rate 1.5 → 1.2, projectile 90 → 150 m/s, the same 1.2 damage per second, so a shot is
1 hit point instead of 0.8 and takes a gun off in one hit) and `raider_basic` (attacks 3 s of every 9 instead of 5)
changed together. Goal cleared and home of 60 (seeds 1–60), emergency recall in both columns:

| Voyage | `rim_skiff` | `skiff_cannon` (two cannons for the lasers) | Halvex Runner | Runner + `rim_skiff` |
|---|---|---|---|---|
| yard_approach | 37 → 37 | 5 → **41** | 36 → 38 | 53 → 57 |
| halvex_drift | 22 → 19 | 9 → 32 | 40 → 46 | 47 → 48 |
| corran_reach | 9 → 10 | 5 → 21 | 27 → 40 | 39 → 43 |
| wardens_gate | 2 → 2 | 1 → 11 | 12 → 17 | 24 → 41 |

**ADR-0037** (14 Oct 2026: simultaneous fire, shared think ticks, auto orbits by both ids). The `dev` build before and
after (`6bffe58` and this change, the same content and pilot), goal cleared and home of 120 (seeds 1–60 + 61–120), lost
in brackets:

| Voyage | `rim_skiff` | Halvex Runner |
|---|---|---|
| yard_approach | 66 (0) → 67 (0) | 79 (2) → 84 (3) |
| halvex_drift | 43 (6) → 37 (2) | 94 (1) → 83 (4) |
| corran_reach | 24 (3) → 21 (8) | 80 (5) → 89 (2) |
| wardens_gate | 3 (0) → 8 (0) | 42 (0) → 43 (3) |

As clear rates: the starter 55 / 36 / 20 / 3 % → 56 / 31 / 18 / 7 %, the Runner 66 / 78 / 67 / 35 % → 70 / 69 / 74 / 36 %.
The player's ship usually spawns before the enemies (lower id), but the scripted pilot fires and flies by its own
inputs, not by a behavior, so only the fire order helped it before; the enemies' behaviors now think on shared ticks
and orbit by both ids. Most changes are within the noise of 60 seeds (about ±5); `halvex_drift` got a little harder
for both (−6 and −11 of 120), the Warden's gate a little easier for the starter.

**Chain reactions** ([GDR-0084](gdr/0084-chain-reactions-v1.md), 14 Oct 2026). The `dev` build before (`95cfcc9`) and
after (the same pilot; the content gains the blast numbers), goal cleared and home of 120 (seeds 1–120), lost in
brackets; the before column equals the ADR-0037 one above:

| Voyage | `rim_skiff` | Halvex Runner |
|---|---|---|
| yard_approach | 67 (0) → 59 (0) | 84 (3) → 83 (0) |
| halvex_drift | 37 (2) → 36 (4) | 83 (4) → 91 (0) |
| corran_reach | 21 (8) → 15 (7) | 89 (2) → 70 (1) |
| wardens_gate | 8 (0) → 4 (0) | 43 (3) → 38 (3) |

As clear rates: the starter 56 / 31 / 18 / 7 % → 49 / 30 / 13 / 3 %, the Runner 70 / 69 / 74 / 36 % → 69 / 76 / 58 / 32 %.
No run ran out of time. The starter's two fission reactors sit beside its radiators, cargo bays, thrusters and shield
emitters, so a reactor shot off now can take a corner of the ship with it; the raiders' thrusters (and the Warden's)
explode too. Within the noise of 120 seeds (about ±7) except the starter on
`yard_approach` (−8) and the Runner on `corran_reach` (−19).

**Chain reactions v2** ([GDR-0085](gdr/0085-chain-reactions-v2.md), 14 Oct 2026). The `dev` build before (`aa9c68e`,
reproducing the GDR-0084 column above exactly), v2's rules with GDR-0084's radii (blasts reach every piece and ship,
armour shields, cores, emitters and RCS pods explode), and v2 as chosen (reactors and drives two cells deep); the same
pilot, goal cleared and home of 120 (seeds 1–120), lost in brackets:

| Voyage | `rim_skiff`: before | v2, old radii | **v2** | Halvex Runner: before | v2, old radii | **v2** |
|---|---|---|---|---|---|---|
| yard_approach | 59 (0) | 63 (0) | **60 (0)** | 83 (0) | 80 (0) | **73 (5)** |
| halvex_drift | 36 (4) | 39 (4) | **36 (5)** | 91 (0) | 83 (0) | **79 (6)** |
| corran_reach | 15 (7) | 18 (6) | **18 (15)** | 70 (1) | 75 (3) | **74 (3)** |
| wardens_gate | 4 (0) | 3 (3) | **2 (9)** | 38 (3) | 30 (3) | **27 (8)** |

As clear rates: the starter 49 / 30 / 13 / 3 % → 50 / 30 / 15 / 2 %, the Runner 69 / 76 / 58 / 32 % → 61 / 66 / 62 /
23 %. No run ran out of time. With the old radii everything stays within the noise of 120 seeds (about ±7); the longer
reach makes the voyages harder: the starter clears as often but is lost more often on the two harder voyages (7 → 15
and 0 → 9 of 120; a reactor shot off now reaches the logic core two cells away through the cargo bay, nothing
shields it), the Runner clears 10–12 fewer of 120 on three voyages and is lost a few times more (its reactors' blasts
reach two cells out too).

## Test-range waves (`sim::voyage::waves`)

The test range's waves (GDR-0074 sized them, GDR-0089 fills them), in the sim since GDR-0092 step 5 so the test range
(`world::scene`) and the arena's TEST-RANGE WAVES gauntlet (`docs/arena.md` "Trials and goals") share one rule. Pure
functions, no state, no random stream of their own:

- `wave_mix(design_bp, enemies)`: a budget of the flown design's build points × `WAVE_SHARE` (0.5) spent on the
  biggest enemies first (each as often as it fits whole), then one more of the smallest if at least half of it is
  left; at least one ship, at most `WAVE_CAP` (8). `wave(design_bp, enemies)` the wave's ships in the order they come
  in, the biggest first. `wave_enemies(content)` the content ships tagged `enemy` (`ENEMY_TAG`) with their build
  points, smallest first, none of a wary faction (GDR-0103: the Grown wait to be provoked; the bastion is not tagged
  `enemy`, so the waves keep to the rim's); `build_points(design)` a compiled design's.
- `wave_bodies(centre, n, roll)`: where the `n` ships come in around the ship they are sent at: on an arc at a random
  bearing, `WAVE_SPREAD` (0.32 rad) apart, each `WAVE_RADIUS` (105–125 m) out, nose and velocity (`WAVE_SPEED`, 3
  m/s) towards it; `sim::math`'s trigonometry. The caller rolls: the arena on the `Scenario` stream, the test range on
  its own generator (`kit::rng::Rng`), where the positions match those before the move to well under a millimetre
  (`world::scene`'s test pins the ships, their order and the positions taken from the code before).

## Driving the sim from the client

```rust
let content = data::base_content();
let runner = Arc::new(sim::ShipDesign::compile(&content, content.ships.get("halvex_runner").unwrap())?);
let mut world = sim::Sim::new(seed);                               // or Sim::with_config(seed, config)
let me = world.spawn_ship(runner, sim::Owner::PLAYER, sim::Body::at(Vec2::ZERO, 0.0));
world.command(me, sim::ShipCommand::Focus);                         // the vessel the player pilots
// every FixedUpdate (30 Hz):
let input = sim::PilotInput { stick, lock, drift, fire, ..Default::default() }; // stick in world terms, |stick| ≤ 1;
// fire: the weapon groups held, bit g-1 for group g (sim::FIRE_ALL: every gun)
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
- Salvage: `ship.cargo()` and `ship.cargo_capacity()` (the HUD's `CARGO n / m`), `world.loose()` (draw fragments at
  `prev_pos → pos` like projectiles), `SimEvent::Collected` (an effect from `at` into the collector; a piece it
  emptied goes without a burst); see [Salvage](#salvage).
- Voyages: roll the plan from the career's voyage seed, `start_voyage`, spawn the fleet at `plan.entry`; markers from
  `world.voyage()` (sites, states, goal), banners on `SiteWoke` / `SiteCleared { goal }`, the RECALL pill from
  `world.can_recall(owner)`; see [Voyages](#voyages).
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
(overlaps and the frame are not checked; `Err` for unknown parts or materials and for a power weight (the part's or
its placement override) outside `(0, MAX_POWER_WEIGHT]` or a minimum level outside 0..1). It builds the ship exactly
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
| `pieces`, `main`, `falls_off` | the connected pieces (parts, `vessel` = has a logic core, `powered`), the main piece (`data::design::geometry::main_of`: cored first, then most parts, then earlier in row-major order), the parts not on it |
| `parts[i]` | stats after bonuses, open (venting) sides, steady-state heat and `heat / limit`, power level at full use, idle |
| `bonuses` | every bonus that applies (target, source, stat, effect), for the bonus overlay |
| `mass`, `centre_of_mass`, `inertia`, `hp`, `cargo`, `rule_slots` | main piece |
| `power` | supply (working reactors after bonuses), demand at full use, served, deficit, the draws in bursts left out of full use per their words (`intermittent`), the consumer rows in allocation order with what each asks and gets |
| `heat` | peak `heat / limit` and its part, the parts above their limit (they would shut down) |
| `flight` | thrust and acceleration per direction (forward, backward, right, left), top speed per direction (GDR-0041), pure and differential torque, the planning `α` (first line), the 90° turn time, the yaw balance |
| `guns`, `firepower` | per gun: muzzle, facing, clear, blockers, range, dps, rate, family, delivery; Σ dps of the main piece's working guns (clear, not idle) |
| `engines` | per engine: facing, thrust, its plume lanes (origin, exhaust direction, length, the own part it burns and how far it reaches) |
| `shields` | per emitter: centre and bubble radius after bonuses |
| `blasts` | per part with an `explosion` (all pieces, 17 Oct 2026): centre, damage, radius and every part it reaches, each with the damage it takes (its `blast_factor` and the cover's), the cover (the armour on the line, on the exploding piece or the target's, as `chain.rs` counts it) and whether it is destroyed, on the design as built; from the compiled `ShipDesign::blast` and `blast_cover`, so the drydock's BLAST overlay shows the sim's numbers (`blasts_agree_with_the_sim`: a blast without a chain destroys in a world exactly what it says and leaves the rest with what it says) |
| `control` | load, capacity, the parts that idle |
| `build_points` | Σ part `build_points` × the material's `build_point_factor` for hull parts (`Content::ship_totals`, the same rule as `Content::part_stats`) |
| `device` | estimated native µs per tick, a class (`Light`, `Moderate`, `Heavy`, `TooHeavy`) and the cost drivers |
| `issues` | blockers: `NoLogicCore`, `CoresUnpowered`, `NoThrust`; warnings: `PowerDeficit`, `Overheat`, `GunBlocked`, `PlumeBurnsOwnPart`, `FallsOff`, `ControlOverCapacity`, `BlastTakes` (a part whose blast would destroy logic cores, guns or parts that explode in turn; GDR-0084's warning) (part lists in each) |

**Assumptions.**

- *Capabilities at full power*: thrust, torque, top speeds, the turn and firepower are what the main piece's
  actuators and clear guns make at full power with bonuses, idle parts (guns too) left out, as if the piece runs (a working,
  powered core). The power section says whether the reactors can feed it all at once; the client can scale by a
  part's level if it wants the brown-out view.
- *Full use* for power and heat: every consumer at full activity at once: engines and RCS pods at full thrust,
  gyros at full torque, guns with a clear line of fire firing, shields on, logic cores and other consumers on.
  Idle parts, blocked guns and consumers set `Off` ask for nothing (`want` 0; an `Off` part is never "short" in
  a `PowerDeficit`). It is the worst case.
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
  (counter-clockwise positive); `yaw_hold` is what the flight computer can hold against it while flying straight
  at rest: pure torque, the torque of every thruster row outside the forward group that turns against it (RCS
  couples, then differential thrust; the forward rows are busy pushing) and the forward engines' gimbals (those
  two are the last resort, after a few ticks); `yaw_balanced = |yaw| ≤ yaw_hold` (test
  `yaw_hold_counts_the_thrusters_that_turn_against_it`: it agrees with one allocation of the first line and the
  last resort). Until the 13 Oct 2026 review fix it left differential thrust out, although flying on the stick
  (`HeadingGoal::Thrust`) or under a lock (`Angle`) steers to a heading and so gets it. Two limits: differential
  thrust fades with speed like all thrust (a row pushing sideways with the drift it causes fades by the whole
  speed), and keyboard W without a lock flies with `HeadingGoal::Keep`, which gets only gimbals.
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
(10 copies of each design 150 m apart flying in parallel along +y under pilot input with every gun firing,
median step ÷ 10, split with
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

**Simultaneous fire and shared think ticks** (ADR-0037, 14 Oct 2026; the `dev` build before and after, run
alternately, 7 runs each, load average 14–18 from other agents' builds; medians of the per-run median / p95, ms):
`sim_perf` raw throttles 0.247 / 0.364 → 0.236 / 0.294, pilot + lock 0.285 / 0.412 → 0.271 / 0.361, combat 0.857 /
1.678 → 0.750 / 1.636; `load_test` 20 ships 0.020 / 0.042 → 0.017 / 0.039, 200 ships 0.160 / 0.236 → 0.163 / 0.223.
No cost beyond the noise: the fire pass keeps its hits in a reused buffer, and thinking on shared ticks moves the
same work onto every third tick.

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

`sim::voyage::scenario::load_test(n, seed) -> Sim` (and `load_test_with(&content, n, seed)`): `n` ships on the base
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

## Bigger clusters

The `cluster` example (galaxy track E0, 16 Oct 2026; `scripts/cluster-bench.sh [--wasm] [args]` runs it natively
and as wasm32-wasip1 under Node and checks that every row's hash matches) prints three tables: **cost** (the load-test
fight at the entry of a voyage rolled on a `--radius` cluster with `--sites` (`encounter:count,…`), its sites not in the
world, kept as records by the director, or all spawned and asleep; step times per phase, the bodies asleep), **hitch**
(spawning, waking and removing k raiders mid-fight) and **precision** (a holding ship, coasting at low speeds, a ram
and the fight at 0, 3, 8 and 30 km from the origin). `--repeat N` gives "median (min–max)" over N runs. The numbers
and what they mean for E3 are in `docs/results/2026-10-16-bigger-clusters.md`: a whole 2.5 km cluster asleep (about
200 ships and wrecks) adds 0.12–0.15 ms per tick natively, 0.16–0.21 ms as wasm; `f32` holds up to about 4 km from
the origin, where only slow coasting (under ~1 m/s) shows the grid.

## Sensors and scans

Galaxy track E3, 16 Oct 2026 ([GDR-0093](gdr/0093-galaxy-exploration-decisions-16-oct.md) the scanner's numbers,
[GDR-0095](gdr/0095-sensors-hidden-sites-and-scans.md) the lead defaults). Code: `src/ship/scan.rs`, the director in
`src/voyage/mod.rs`, the `scan` action in `src/behavior/`; tests `tests/scan.rs`.

- **Sensors** (content `sensor: (range)`, on logic and automaton cores, 800 m): `Ship::sensor_range()` is the
  longest range of the working sensor parts (alive, not overheated, not idle) on the controlling piece while it is
  active, at least `ship::sensor_tuning::ALWAYS_SEEN` (150 m). A think (behaviors and pilot assists) prepares it once;
  `select` skips enemies farther away (the `locked` selector, the leader's lock, is not limited). Cost: a pass over
  the sensor parts per think.
- **Scans** (content `scanner: (range, charge, cooldown, wake)`, power drawn only while charging:
  `ConsumerKind::Scanner`): `ShipCommand::Scan` starts one when `Ship::can_scan()` (ready, active, a working scanner
  on the controlling piece; the one with the longest range is used). `Ship::scan()` is `Scan::Ready`,
  `Charging { part, charged }` or `Cooling { part, left }`. In the systems phase, after `update_systems`, a charging
  scan adds the scanner's `work × DT` (its power level), sweeps when it reaches `charge` (`SimEvent::Scanned { ship, at,
  range, wake }`, `at` the controlling piece's centre of mass) and cools down for `cooldown` seconds; a scanner that
  died or left the controlling piece ends it (no sweep). The scan state is in the ship's hash only while not `Ready`.
  The design report's full use leaves a draw out when its part's `power.intermittent` says it comes in bursts (the
  scanner's), and sums such draws apart per their words (`PowerReport::intermittent`, the drydock's "+3 WHILE
  SCANNING"; 16 Oct 2026: the starter skiff read 29 / 28 with its scanner counted as always charging).
- **The `scan` action** (`Action::Scan`, `RuleStatus::Scan { started }`, `behavior::action_decides`): when its
  rule's conditions hold it starts a scan if the ship can (one per think) and steps aside; the next rule that holds
  decides the maneuver. It needs no target.
- **Hidden sites and bearings** (the director, each pass after spawning): it collects this step's sweeps of the
  player's ships; each reveals every site within its range (`Site::known`, `SimEvent::SiteRevealed`), gives every
  hidden signal's site beyond it a `voyage::Bearing { site, from, dir, tick }` (`Voyage::bearings`,
  `SimEvent::SiteBearing`: exact, a unit vector from the scan spot) and marks the sleeping sites within its `wake`
  radius to wake. Then every hidden site within a player vessel's sensor range shows (a signal's site,
  `SitePlan::signal`, only within `signal_reveal` as well). Signals' sites: the trail places of a generated cluster that a signal the career has heard leads to (E4: `Leads::signals`; E3 had
  every trail place; a content voyage's goal until E7, 17 Oct 2026: now an ordinary site, its voyage the signal).
  Found is final.
- **Generated clusters**: `VoyagePlan::roll_interior(content, id, &galaxy::Interior, &Leads, seed)` rolls an interior
  on the same generator and draw order as `roll` (no goal; a trail place in `leads.signals` is a signal's site, one in
  `leads.cores` holds its data core, `SitePlan::core`; one without an encounter, a beacon alone, rolls no ships or wrecks
  and is a quiet site). `data::career::Leads` comes from the career (`Career::leads`, `docs/career.md` "The galaxy").
  `roll` itself draws exactly as before; a site's hash takes its core only when it has one, so plans without (the
  slice's) hash as before.
- **The edge** (`voyage::edge_force`, in the physics phase with the wrenches): with a voyage underway, a piece whose
  centre of mass lies beyond `Voyage::edge` (radius + `edge_margin`, at most `MAX_EDGE`) gets a force towards the
  centre: `mass × (EDGE_PUSH × min(depth / EDGE_DEPTH, 1) + outward speed / EDGE_BRAKE)` (3 m/s², 100 m, 2 s); it is
  not the piece's own, so the speed governor never cuts it.
- **Determinism**: the voyage scenario's ships scan whenever they decide. 16 Oct 2026, release builds, native x86_64
  vs wasm32-wasip1 under Node 24: **identical**: core seed 42 900 ticks (final `f47052940ca8ca75`), voyage seed 42
  3600 ticks (`539fc2dd23846817`), `--fight skiff_mirror` (a draw at the time limit, `d1deef39e6c7b7b5`), `--fight
  runner_vs_raiders` (won in 1273 ticks, `de48601be0d811c1`), `--run two_starts_locked` (resumed identical, last line
  `c4372c8cd861fe24`), `--galaxy 5`. Every stream with the starter or a core changed (the scanner, the sensors in
  the content, the new action).
  Galaxy track E4 (16 Oct 2026: data cores, `Leads`, the skeleton's types in `data`): no hash changed, native = wasm:
  voyage seed 42 3600 ticks `539fc2dd23846817`, core `f47052940ca8ca75`, salvage `4388b608ceda33b3`, `--galaxy 5`
  identical (a plan hashes a data core only when a site holds one; the slice's never do).

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
