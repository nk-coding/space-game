# Spike plan

Decided 10–11 Oct 2026. Bevy only. No fixed time box: the spike runs until every question below has an answer. Claude Code writes and runs it in Niklas's Linux VM, in a local folder with local git (no remote repo).

## Questions the spike must answer

| # | Question | Pass when |
|---|---|---|
| 1 | Which physics engine? | Benchmark table filled for Rapier2D, box2d-rust and Avian on native, phone and browser. Niklas picks the winner from the numbers. |
| 2 | Can the sim be deterministic across platforms? | The same scenario gives identical per-tick state hashes natively in the VM, on the phone (APK) and in the host browser (wasm). |
| 3 | Does breakage work in the chosen engine? | Ships split into pieces that keep their velocity and spin, with their own power pool, and no explosions of overlapping bodies. |
| 4 | Is the phone fast enough? | 20 ships with debris and projectiles at 60 fps on the flagship, using at most half the frame budget: about 8 ms per frame, sim step under 4 ms. Phone temperature after 10 minutes is noted. |
| 5 | How does the browser compare? | Same load test in the host browser. Numbers recorded; Niklas sets the web bar from them. |
| 6 | Is the style 2a look affordable? | Frame time with the look on vs. debug shapes, and with full-screen bloom on vs. off, on phone and browser. |
| 7 | Does flying feel right? | Niklas flies the ship on the phone and in the browser and is happy with stick + lock, braking and the speed cap. |

## Setup in the VM

- Rust via rustup, plus the `wasm32-unknown-unknown` target and the Android targets (at least `aarch64-linux-android`).
- Web builds: Trunk (or wasm-bindgen-cli). Serve with `trunk serve --address 0.0.0.0` and open from the host browser.
- Android builds: the Android SDK and NDK plus a JDK. Follow the mobile example of the Bevy version you use; recent ones ship a Gradle project and build the Rust library with cargo-ndk.
- adb over Wi-Fi: on the phone, enable Developer options, then Wireless debugging. Run `adb pair <ip>:<pair-port>` once, then `adb connect <ip>:<port>`. The VM needs a network path to the phone, which usually means bridged networking.
- Check versions before you start. Bevy 0.20 came out on 8 Oct 2026, and it breaks APIs every 4–5 months. Pin exact versions in `Cargo.toml`.

## Suggested workspace

```
space-game/
  Cargo.toml          # workspace
  crates/sim/         # engine-free simulation (no Bevy dependency unless Avian wins)
  crates/game/        # Bevy client (native, web, Android)
  crates/bench/       # physics benchmark harness (native + wasm + Android)
  data/               # RON: parts, sample ships (start from reference/data-draft/)
  docs/               # this context folder's docs, kept up to date
```

## Step 1: Physics benchmark

- **Candidates**:
  - Rapier2D with `enhanced-determinism`. Its docs list WASM among the covered targets. It can't be combined with `simd8`.
  - box2d-rust, the pure Rust port of Box2D 3.1.
  - Avian 2D. It ties the sim to Bevy's ECS; if it wins, discuss that with Niklas before building on it.
  - Box2D v3.1 in C via bindgen, as a reference number only.
- **Scenarios**:
  - 20 compound ships of 200–2000 blocks
  - 2000 debris chunks
  - 5000 raycasts per tick
  - a breakage event every second (remove blocks, split bodies)
  - an asteroid field
- **Metrics**:
  - mean and p95 step time at 30 Hz, single thread and multi-thread where available;
  - native (VM), phone (native APK), browser (wasm in the host browser);
  - determinism: same per-tick hash on all three.
- **Output**: `docs/results/physics-benchmark.md` with the table, notes on each engine's API for compound bodies and splitting, and a recommendation. Niklas decides.

## Step 2: Core sim

- One cell = 1 m. A ship is a grid of parts, each part with a footprint, rotation and flip. See `reference/drydock-model.js` for the rules and `reference/data-draft/` for parts and sample ships.
- Each connected piece is one rigid body with a compound collider (one shape per cell, or merged rectangles plus the slope triangles). Mass and centre of mass come from the parts.
- Slopes connect only through their straight sides. When a part is destroyed, recompute connectivity. Split pieces into new bodies that keep their linear and angular velocity. Each piece keeps only its own reactors.
- Damage per part. Power pool per piece. Heat can stay simple in the spike.
- Fixed 30 Hz step, ordered containers, seeded RNG, a state hash every tick. A headless test runs the same scenario twice and compares hashes.

## Step 3: Web, Android and feel

- **Rendering, style 2a** (see `docs/style-guide.md`):
  - Build a texture atlas from `assets/icons/*.svg`, rasterised at build time (for example with resvg).
  - Draw tiles as sprites or instanced quads, with the armour colour per material and a dark seam.
  - Hull edge: a line mesh plus a soft glow.
  - Engines: plume sprites and dotted trail particles.
  - Measure full-screen bloom on vs. off.
- **Flight model**:
  - Without a lock, the stick means "fly that way". The flight computer turns the nose and fires the thrusters that push the right way, taking their position and the torque they cause into account. How far you push sets the speed.
  - Letting go brakes to a stop with the ship's own thrusters. A drift toggle (player setting) turns braking off.
  - Soft top speed from thrust and mass.
  - Tap or click a ship to lock it: the flight computer keeps the nose on the target while the stick strafes.
- **PC controls**: WASD acts as the stick; the mouse locks onto targets and fires.
- **Timing**: 30 Hz simulation, 60 fps rendering with interpolation between sim states.
- **Builds**: the web build is served from the VM and opened in the host browser; the APK is installed over adb Wi-Fi.
- **Ships**: the Halvex Runner (`assets/ships/halvex-runner.layout.json`) as the player ship, automaton raiders as targets.

## Step 4: Load test

- 1, 5 and 20 ships (runners and raiders with a trivial AI: approach, orbit, fire), with debris and projectiles.
- Phone: frame time, sim step time, temperature after 10 minutes. Browser: the same numbers, recorded.
- Run with the style 2a look and with debug shapes, and with bloom on and off.

## Step 5: Cross-device hash

- A scripted scenario with recorded inputs runs for N ticks natively, on the phone and in the browser.
- Compare the per-tick hashes and report the first tick where they diverge, if any.

## Out of scope for the spike

Behavior trees, Rhai, netcode, the galaxy, menus and real UI screens (a debug HUD is enough), audio, materials and fabrication.

## When it's done

Write `docs/results/spike-summary.md` with the answers to the seven questions, the numbers, and what surprised you. Then ask Niklas how to continue.
