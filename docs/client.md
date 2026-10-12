# Client (`crates/game`)

The Bevy 0.20 client. Web (wasm32 + WebGL2) is the development platform; desktop and Android come later and
must keep compiling (`cargo check -p game` natively). Written 11–12 Oct 2026.

## Build and run

| What | Command |
|---|---|
| Dev build with live reload | `scripts/web-serve.sh` → http://10.77.6.2:8081 (port via `PORT=`) |
| Optimised build, served | `scripts/web-serve.sh --release` |
| Optimised build into `crates/game/dist` + sizes | `scripts/web-build.sh` (cargo `release`, opt-level 3, wasm-opt -O3), then `scripts/web-serve-dist.sh` |
| Atlas preview / timing | `cargo run --release -p art --example preview [-- <tier> <scratch.png>]` (the game rasterises its atlas at runtime; nothing raster is committed) |
| Headless check | `node /home/devbox/workspace/tools/headless/shot.mjs <url> <png> <waitMs>` |
| Scripted input + screenshots | `node scripts/web-drive.mjs <url> <out-dir> "wait 20000; down KeyW; wait 3000; up KeyW; shot fly.png"` |

Port 8080 is Niklas's copy, served by the lead session from `/home/devbox/workspace/serve/game`; agents use 8081.

## Controls

PC keyboard controls are ship-relative ([GDR-0040](gdr/0040-keyboard-flight-and-smart-lock-aim.md)); touch keeps
the stick ([GDR-0009](gdr/0009-stick-and-lock-controls.md)).

| | PC | Touch (landscape) |
|---|---|---|
| Fly | A/D rotate, W/S thrust along the nose, Q/E strafe (arrows: rotate + thrust). The flight computer cancels sideways drift and brakes when the keys are released | virtual stick bottom left: "fly that way", push = speed |
| Drift toggle (pure momentum: no drift cancelling, no braking) | X | tuning panel |
| Lock a target | left-click a ship (or a salvage piece) | tap it |
| While locked | the ship turns its strongest working guns onto the target (lead point for projectiles); A/D nudge the aim and it springs back; W/S and Q/E still thrust and strafe | the stick strafes |
| Unlock | click empty space, or Esc | tap empty space |
| Next target (nearest first) | Tab | – |
| Fire | hold the left button on empty space, hold the right button, or hold Space | fire button bottom right |
| Switch vessel (the focus) | V / Shift+V (the sim's vessel list, ascending ship id), or click a friendly vessel | tap a friendly vessel |
| Zoom | wheel, + / − | pinch |
| Camera | F: follow ↔ free (drag with the middle button) | – |
| Tuning panel | T | – |
| Debug overlay (fps, frame and sim times, the locked ship's active behavior rule) | F3 | – |
| Render toggles | 1 debug shapes, 2 glow sprites, 3 bloom | – |
| Hide HUD / pause / reset scene | H / P / Shift+R | – |

Ships with engines only at the back can't brake or strafe while locked (GDR-0011); S does nothing on them.

## URL parameters

| Parameter | Meaning |
|---|---|
| `scene=showcase` | runner holding position, three raiders orbiting it, no combat (default: the sandbox) |
| `load=1\|5\|20` | browser load test: `sim::scenario::load_test(n, seed)`, camera frames the fight, stats overlay on |
| `seed=N` | sim seed (default 42) |
| `hashlog=1`, `hashticks=N` | log the per-tick state hash of the load scenario (20 ships unless `load=` is given) to the console, `N` ticks (default 600) |
| `thrust`, `torque`, `top`, `turn`, `range`, `damage`, `raiders`, `drift`, `link` | tuning values (the panel prints this query) |
| `fleet=N` | N extra Halvex Runners fly with you under their default behavior (`skirmisher`); switch with V |
| `tuning=1` | open the tuning panel at start |
| `debug=1`, `glow=0`, `bloom=1`, `hud=0`, `stats=1` | render toggles, HUD, debug overlay |
| `zoom=N` | start zoom in px per metre |
| `touch=1` | show the touch widgets without a touch first |

## Scenes (`scene.rs`)

- **Sandbox** (default): the player flies the Halvex Runner. Waves of automaton raiders (raider brain) spawn
  105–125 m away; a new wave starts 4 s after the last raider vessel goes dark (a raider shot in half can be
  two raiders). Wrecks and salvage drift away and are removed when they are more than 320 m from the player or
  dark for 75 s (the player's vessels, dark or dormant, stay). When the focus loses its last logic core the sim
  moves the focus to another vessel of the player (a split-off half too); when the player has no vessel left,
  a new runner re-forms after 3 s.
- **Showcase**: no combat, free camera; for looking at the renderer.
- **Load test**: see below.

## Load test and determinism in the browser

1. Open `http://10.77.6.2:8081/?load=20` (or 1, 5). The overlay shows frame and sim step times (mean, median,
   p95 over the last 600 frames / 900 ticks), ships, pieces and shots. Toggles 1/2/3 switch debug shapes, glow
   sprites and bloom; F frees the camera.
2. After 60 s of wall time the console gets one line:
   `LOADTEST ships=20 seconds=60 ticks=... frame_ms mean=... median=... p95=... sim_ms mean=... style=2a glow=true bloom=false window=WxH`.
3. Determinism: `?hashlog=1&load=20&hashticks=600` prints `# ...` header, then `<tick> <hash>` lines (30 per
   console call) and `# final=...`. Save them (strip any console prefixes) and compare with the native stream:

   ```
   cargo run --release -p sim --example determinism -- --scenario load --ships 20 --ticks 600 --seed 42 > native.txt
   scripts/determinism-check.sh --compare native.txt browser.txt
   ```

   Changing tuning values during a hash run changes the hash (the config is part of the state).

## Structure

```
FixedUpdate (30 Hz)   scene::scene_tick (pilot input, waves)  →  sim_bridge::step_sim  →  sim_bridge::apply_events
                                                                  (Sim::step, timing,       (views: spawn / rebuild /
                                                                   hash log)                 despawn; effects from events)
Update                controls (keys, mouse, touch → PlayerInput), camera input, HUD text, tuning panel (egui)
PostUpdate            ship_view::sync_piece_views (render_pose(alpha) → Transform, lit/dark, shield rings)
                      camera::update_camera  →  fx (plumes, trails, beams, tracers, sparks; lock ring, markers)  →  fx::flush
```

- **`sim_bridge.rs`** owns `SimWorld { sim: SimT, content, designs, views, inputs, ... }`. `SimT` is the sim
  type (`sim::Sim` = FreeFlight); the bridge only uses the backend-independent API, so the physics engine plugs
  in by changing that alias. Views are keyed by `(ShipId, PieceId)`.
- **One view per sim piece** (`ship_view.rs`): `ship_model::piece_visual` builds the piece's parts in the sim's
  piece frame (pivot = `piece.origin`, so the transform is exactly `piece.render_pose(alpha)`), with the sim's
  line-of-fire flags, shield radii and hull edges (`Ship::piece_hull_edges`). Body mesh variants (style 2a lit /
  dark, without glow sprites lit / dark, debug shapes) are built lazily; `piece.active` picks lit or dark
  (salvage and unpowered pieces are dimmed and lose the hull glow). Shield rings are per-emitter meshes whose
  alpha follows the capacity left; hidden while the bubble is down.
- **Events** (`apply_events`): `ShipSpawned` / `PieceChanged` / `Split` (re)build views, `VesselFormed` with a
  parent moves a split-off piece's view to its new ship, `PieceDestroyed` / `ShipRemoved` despawn them;
  `PartDestroyed`, `ShotFired`, `Hit`, `Ricochet`, `ShieldHit` become effects.
- **Vessels** (`scene.rs`, 12 Oct 2026): the sim owns owners, vessels and the focus (`docs/sim.md` "Vessels").
  The player is `scene::PLAYER` (`Owner(0)`), the raiders `scene::ENEMY` (`Owner(1)`). The sandbox gives every
  vessel its default behavior and sends `ShipCommand::Focus` for the runner; `PilotFocus` mirrors
  `Sim::focus(PLAYER)` at the start of every tick (so a switch shows one tick later), and V / clicks send
  `Focus` commands. Friendly checks, Tab targets and off-screen markers use owners (`Ship::team`).
- **Effects** (`fx.rs`): immediate mode. Systems push `FxSprite`s into `FxQueue` every frame; `flush` writes them
  into a pool of reused sprites (all from the atlas, batched). Plumes come from `piece.thrusters[i]` ×
  `piece.output[i]` (RCS nozzles: small puffs; engine gimbal from `piece.act.deflect`), beams from
  `Sim::beams()` re-attached to the gun at the render pose, tracers from `Sim::projectiles()` interpolated.
- **HUD** (`flight_hud.rs`, Barlow from `assets/fonts`, OFL): hull %, hull map, speed, target panel, wave line,
  warnings, help; lock ring and off-screen markers are world-space FX. Debug overlay and toggles: `hud.rs`.
- **Tuning** (`tuning.rs`, bevy_egui 0.43.1): thrust / torque scale (separate or linked), top speed factor and
  max turn rate apply live (`Sim::set_config`); raiders per wave apply to the next wave; drift at once; weapon
  range and damage scale rewrite the content the designs compile from, so they apply with "Apply & reset".

## Adding visuals

- **A new part icon**: add `art/icons/<key>.svg` and `<key>.icon-only.svg` (glow in a
  `<g filter="url(#gl)" opacity="0.55">` group) and set `icon: "<key>"` on the part; the `art` crate embeds it
  at build time and its test checks that every content icon rasterises.
- **A new effect**: add a constructor to `fx::Effects` (sparks, flash, line or ring), call it from the event
  match in `sim_bridge::apply_events`, draw it in `fx::draw_effects`. Continuous visuals that follow sim state
  (like plumes) go in `fx::draw_ships_fx` and read the sim at the render pose.
- **A new FX texture**: add it to `Layout::new` in `crates/art` (as SVG or a `Proc` function of `(u, v)` with a
  size in atlas units), look it up once in `fx::FxRegions`.
- **HUD text**: add a `HudText` variant in `flight_hud.rs` and fill it in `update_text` (8 Hz, re-laid out only
  when the text changes). UI colours go through `style::ui_color` (the world composites in sRGB space and
  bevy_ui does not encode for it).

## Gotchas

- `CompositingSpace::Srgb` (browser-like blending) needs `Hdr` on WebGL2 in Bevy 0.20, see `main.rs`.
- Debug wasm builds are ~100 MB and the sim runs with debug assertions (state hash every tick); use
  `scripts/web-serve.sh --release` for timings.
- SwiftShader (headless Chromium) runs the style-2a scene at a few fps: its numbers say nothing about real GPUs.
