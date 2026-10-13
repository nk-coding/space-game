# Client (`crates/game`)

The Bevy 0.20 client. Web (wasm32 + WebGL2) is the development platform; desktop and Android come later and
must keep compiling (`cargo check -p game` natively). Written 11–13 Oct 2026.

## Build and run

| What | Command |
|---|---|
| Dev build with live reload | `scripts/web-serve.sh` → http://10.77.9.2:8081 (port via `PORT=`) |
| Optimised build, served | `scripts/web-serve.sh --release` |
| Optimised build into `crates/game/dist` + sizes | `scripts/web-build.sh` (cargo `release`, opt-level 3, wasm-opt -O3), then `scripts/web-serve-dist.sh` |
| Atlas preview / timing | `cargo run --release -p art --example preview [-- <tier> <scratch.png>]` (the game rasterises its atlas at runtime; nothing raster is committed); per region and mip step: `... --example profile -- <tier>` |
| Headless check | `node /home/devbox/workspace/tools/headless/shot.mjs <url> <png> <waitMs>` |
| One bounded check (build, serve on a free port, headless shot, console, stop) | `scripts/web-check.sh [--release] [--params '?load=5'] [--wait MS] [--size WxH] [--out FILE]` |
| Native load test table | `scripts/load-test-native.sh [--ships 1,5,20] [--ticks 1800] [--seed 42]` |
| Scripted input + screenshots | `node scripts/web-drive.mjs <url> <out-dir> "wait 20000; down KeyW; wait 3000; up KeyW; shot fly.png"` |

Port 8080 is Niklas's copy, served by the lead session from `/home/devbox/workspace/serve/game`; agents use 8081.

## Controls

PC keyboard controls are ship-relative ([GDR-0040](gdr/0040-keyboard-flight-and-smart-lock-aim.md)); touch keeps
the stick ([GDR-0009](gdr/0009-stick-and-lock-controls.md)). The A/D rotation ramp is integrated by the scene once
per sim tick with `sim::flight::turn_ramp_step` (like the lock's nudge with `nudge_step`) and sent as
`PilotAxes::turn`; see `docs/sim.md` "Keyboard rotation".

| | PC | Touch (landscape) |
|---|---|---|
| Fly | A/D rotate (GDR-0055, numbers in GDR-0058: a tap nudges the nose a little, holding ramps up to the key turn rate, 1.5 rad/s, in 0.8 s; never faster than the ship's gyros, cores and RCS can stop), W/S thrust along the nose, Q/E strafe (arrows: rotate + thrust). The flight computer cancels sideways drift and brakes when the keys are released | virtual stick bottom left: "fly that way", push = speed |
| Drift toggle (pure momentum: no drift cancelling, no braking) | X | tuning panel |
| Lock a target | left-click a ship (or a salvage piece) | tap it |
| While locked | the ship turns its strongest working guns onto the target (lead point for projectiles); A/D nudge the aim and it springs back; W/S and Q/E still thrust and strafe | the stick strafes |
| Unlock | click empty space, or Esc | tap empty space |
| Next target (nearest first) | Tab | – |
| Fire | hold the left button on empty space, hold the right button, or hold Space | fire button bottom right |
| Switch vessel (the focus) | V / Shift+V (the sim's vessel list, ascending ship id), click a friendly vessel, or click its chip in the fleet bar | tap a friendly vessel or its chip |
| Wake a dormant vessel (swap) | click its chip in the fleet bar | tap its chip |
| Zoom (3–160 px per metre; while a target is locked the camera zooms out enough to keep it in view: "Lock framing" under [Structure](#structure)) | wheel, + / − | pinch |
| Camera | F: follow (load test: frame) ↔ free; drag with the middle button (the left button too while nobody is piloted: showcase, load test, watch) | – |
| Drydock (back to the ship editor; the design is untouched); in TEST IN RANGE back to where the test started | T | – |
| Home (the ring menu; the flight ends) | M, or the ring top left | the ring top left |
| Raider waves off / on (sandbox; off removes the raiders) | C | – |
| Tuning panel (flight only; it was T until 12 Oct 2026) | U | – |
| Debug overlay (fps, frame and sim times, the focus ship's heading, turn rate, speed and whether the last resort turns it, the locked ship's active behavior rule; in TEST IN RANGE every rule of the tested behavior with why) | F3 | – |
| Render toggles | 1 debug shapes, 2 glow sprites, 3 bloom | – |
| Hide HUD / pause (`PAUSED · P` shows) / reset scene | H / P / Shift+R (left Shift) | – |

Ships with engines only at the back can't brake or strafe while locked (GDR-0011); S does nothing on them.

The keys reach the sim tick of the frame they are pressed in, and a tap shorter than a tick (33 ms, even a press
and release within one frame) still reaches one tick: the controls run before the frame's ticks and latch the
keys and fire buttons until the next tick takes them (`controls::PlayerInput::for_tick`, 13 Oct 2026).
Home's keys are in [Home](#home), the drydock's in [Drydock](#drydock), the BEHAVIOR screen's in
[Behavior screen](#behavior-screen).

## URL parameters

| Parameter | Meaning |
|---|---|
| `scene=home` | start at home, the ring menu (the default) |
| `scene=sandbox` | start in the sandbox (home's TEST RANGE) |
| `scene=drydock` | start in the drydock |
| `scene=behavior` | start on the BEHAVIOR screen (the design's behavior) |
| `behavior=<id>` | the BEHAVIOR screen opens this content or saved behavior (e.g. `raider_basic`) instead of the design's |
| `scene=showcase` | the player's design holding position, three raiders orbiting it, no combat |
| `scene=showcase&fire=1` | the same, but the raiders (`raiders=N` of them, default 3) attack the design under their behavior: for looking at shield hits (add `zoom=70` for a close-up) |
| `scene=watch&behavior=<id>` | TEST IN RANGE, watch: the player's design flies unpiloted on the behavior (a content or saved one; without `behavior` the design's own) against raider waves, the camera follows it; T returns to the drydock |
| `scene=alongside&behavior=<id>` | TEST IN RANGE, fly alongside: you fly the design, a second copy is a wingman on the behavior |
| `load=1\|5\|20` | browser load test: `sim::scenario::load_test(n, seed)`, camera frames the fight, stats overlay on |
| `seed=N` | sim seed (default 42; a whole number, exact up to 2^64) |
| `hashlog=1`, `hashticks=N` | log the per-tick state hash of the load scenario (20 ships unless `load=` is given) to the console, `N` ticks (default 600, at least 1) |
| `thrust`, `torque`, `speed`, `turn`, `keyturn`, `range`, `damage`, `raiders`, `drift`, `link` | tuning values (the panel prints them, `link` aside; `keyturn` is the A/D key turn rate, default 1.5 rad/s; `speed` replaced `top` on 12 Oct 2026). `range` scales every side's weapons, `damage` only the player's (the raiders keep their content damage, 12 Oct 2026) |
| `mind=N` | the player's mind capacity in logic cores (default 24; a runner has 2): `?fleet=3&mind=4` leaves two runners dormant, for trying the fleet bar's swap |
| `fleet=N` | N extra copies (at most 8; none in TEST IN RANGE) of the player's design fly with you under the behavior the design names (default its faction's, `skirmisher`), following your lock (`target locked`); switch with V or the fleet bar |
| `tuning=1` | open the tuning panel at start |
| `debug=1`, `glow=0`, `bloom=1`, `hud=0`, `stats=1` | render toggles, HUD, debug overlay |
| `zoom=N` | start zoom in px per metre (3–160) |
| `touch=1` | show the touch widgets without a touch first |

Natively the same `key=value&…` string comes from the `GAME_PARAMS` environment variable (`params.rs`). A bare
key means 1; `0`, `false`, `off` and `no` turn a flag off.

Where the game starts (`mode::AppMode::at_start`, since 13 Oct 2026, GDR-0062): `scene` decides when it is given
(an unknown one starts at home). Without it, a parameter that only means something in flight starts the sandbox, as
before home existed: `load`, `hashlog`, `hashticks`, `fleet`, `seed`, `zoom`, `fire`, `raiders`, `mind`, `tuning`,
`touch` (`params::FLIGHT_ONLY`); so `?load=20`, `?hashlog=1&load=20` and `?fleet=3&mind=4` fly as they did. Anything
else (`/`, `?debug=1`, `?stats=1`) starts at home; the sandbox alone is `?scene=sandbox`.

## Scenes (`scene.rs`)

- **Sandbox** (home's TEST RANGE, `?scene=sandbox`; the HUD title says `TEST RANGE`): the player flies their design (`drydock::PlayerDesign`; at the first start a copy of the
  Halvex Runner, after that the autosaved working design). Waves of raiders (the first content ship tagged `enemy`:
  the base pack's automaton raider; their faction's behavior, `raider_basic`) spawn 105–125 m away; a new wave starts 4 s after the last
  raider vessel goes dark (a raider shot in half can be two raiders). C turns the waves off (every raider vessel
  is removed at once, their wrecks stay, no new wave comes) and on again (a wave 2 s later); the subtitle says
  `WAVES ON` / `WAVES OFF (C)`, Shift+R keeps the setting. Wrecks and salvage drift away and are removed when
  they are more than 320 m from the player or dark for 75 s (the player's vessels, dark or dormant, stay). When
  the focus loses its last logic core the sim moves the focus to another vessel of the player (a split-off half
  too); when the player has no vessel left, a new one re-forms after 3 s (its wreck, with whatever its hold
  carried, is removed then).
- **Salvage in the sandbox** (GDR-0063, 13 Oct 2026; the first feel check of track 7B): every running vessel of
  the player collects (`SimConfig::salvage`, the sim's defaults): fly within 5 m of a wreck (raiders shot down,
  their split-off pieces) or a loose core fragment, and one part every 1/3 s goes into the hold, best first
  (`docs/sim.md` "Salvage"). The HUD shows `CARGO n / m` under the hull; nothing is done with the haul yet.
- **Test flight**: the sandbox launched from the drydock (T or TEST-FLY), waves on, title `TEST FLIGHT`. Losing
  every vessel returns to the drydock after the same 3 s (`SHIP LOST · BACK TO THE DRYDOCK`) instead of
  re-forming; T returns at any time. Shift+R restarts the test flight.
- **TEST IN RANGE** (`test_range.rs`, GDR-0057; `docs/drydock.md` "Behaviors: storage, designs and tests"): a test
  flight that flies a behavior, started by the behavior screen (`test_range::StartTest`) or the URL
  (`scene=watch|alongside&behavior=<id>`). **Watch** (title `WATCH · <BEHAVIOR> · WAVE n`): the design flies
  unpiloted on the behavior, the camera and the HUD follow it, focus switches are dropped, losing it ends the test.
  **Fly alongside** (`FLY ALONGSIDE · …`): you fly the design, a second copy is a wingman on the behavior. T and a
  lost test return to where the test started (`mode::FlightExit`: the screen that sent it, the drydock for the
  URL); Shift+R starts over. `test_range::BehaviorWatch` has the tested ship's rules live: a compact panel in the
  HUD shows them (right, under the target panel; the rule that decides in teal, why the rules above don't match;
  `behavior_ui/live.rs`), F3 every rule with why.
- Every sandbox flight flies the **player's saved behaviors**: they join the player's content, so the library has
  them and a design naming one flies it (`drydock::behaviors`).
- **Showcase**: no combat, free camera; for looking at the renderer. With `fire=1` the raiders (the tuning's
  raiders per wave, `raiders=N`) start 46 m out and attack the design, which holds still and doesn't fire back:
  its bubbles take hits at once. Cannon shots pass half their damage to the hull, so the Runner breaks up after
  about half a minute (Shift+R starts over), and with the base numbers (regen 1.5/s against about 0.4 per
  hit) the bubbles rarely empty; collapses need concentrated fire.
- **Load test**: see below.

## Load test and determinism in the browser

1. Open `http://10.77.9.2:8081/?load=20` (or 1, 5). The overlay shows frame and sim step times (mean, median,
   p95 over the last 600 frames / 900 ticks), ships, pieces and shots. Toggles 1/2/3 switch debug shapes, glow
   sprites and bloom; F frees the camera.
2. After 60 s of wall time the console gets one line:
   `LOADTEST ships=20 seconds=60 ticks=... frame_ms mean=... median=... p95=... max=... sim_ms mean=... median=... p95=... max=... style=2a glow=true bloom=false window=WxH`.
3. Determinism: `?hashlog=1&load=20&hashticks=600` prints `# ...` header, then `<tick> <hash>` lines (30 per
   console call) and `# final=...`. Save them (strip any console prefixes) and compare with the native stream:

   ```
   cargo run --release -p sim --example determinism -- --scenario load --ships 20 --ticks 600 --seed 42 > native.txt
   scripts/determinism-check.sh --compare native.txt browser.txt
   ```

   Changing tuning values during a hash run changes the hash (the config is part of the state).

   Result 12 Oct 2026 (dev wasm build in headless Chromium vs native release x86_64, `dev` after the plume merge
   60945ae): `load` 20 ships, seed 42, 600 ticks: **IDENTICAL** (final `86eb72a03591bff2`; also identical on the
   commit before the merge, final `9ea129f5b628badc`); seed 7, 1800 ticks (`&seed=7&hashticks=1800`):
   **IDENTICAL** (final `34fccbaae2c4e339`). Headless capture: `node scripts/web-drive.mjs
   "<url>/?hashlog=1&load=20&debug=1&hud=0" <dir> "wait 120000" 480 270`, then keep the lines matching
   `^[0-9]+ [0-9a-f]{16}$` and `^# ` (the console prints 30 lines per call).

## Structure

```
RunFixedMainLoop      flight, before the ticks: fleet bar clicks → controls (keys, mouse, touch → PlayerInput, latched
                      until the next tick)
FixedUpdate (30 Hz)   scene::scene_tick (pilot input, waves)  →  sim_bridge::step_sim  →  sim_bridge::apply_events
  (flight only)                                                   (Sim::step, timing,       (views: spawn / rebuild /
                                                                   hash log)                 despawn; effects from events)
Update                ui_kit (hit testing, text fields) → flight: camera input, HUD text and the home ring, fleet bar,
                      touch widgets, tuning panel (egui) | drydock: DockSet::Input →
                      Apply → Show → ui_kit styles | behavior: BehaviorSet::Input → Apply → Show → ui_kit styles;
                      the TEXT view's editor in egui's pass | home: clicks and keys → apply → the ship and the
                      camera → ui_kit styles
PostUpdate            ship_view::sync_piece_views (render_pose(alpha) → Transform, lit/dark)  →  shield::draw_bubbles
                      camera::update_camera  →  fx (plumes, trails, beams, tracers, sparks; lock ring, markers;
                      the drydock's frame overlay, the behavior graph's dot grid, home's dotted ring and dock)  →
                      fx::flush;  ui_icons (after the
                      UI layout and the camera)
```

- **App mode** (`mode.rs`): `AppMode::{Home, Flight, Drydock, Behavior}`, a Bevy state; the game starts at home
  ([Home](#home); `?scene=` picks another start, see URL parameters). Each mode's systems run
  only in it (`run_if(in_state(..))`), so input never crosses over. The switch happens in the state's enter / exit schedules: `OnExit(Flight)` empties the
  world (views, effects, bubbles, the lock go; the clock is unpaused), `OnEnter(Flight)` builds what another mode
  asked for (`Scene::launch`: `Launch::TestFlight` from the drydock or the BEHAVIOR screen, `Launch::Range` from
  home), the drydock's and home's enter / exit set and restore the camera and the UI scale. Flight
  UI roots carry `mode::FlightUi` and are hidden when flight ends. A `SimWorld` always exists (an empty one
  outside flight and before Startup), because many flight systems read it. T in flight goes to `mode::FlightExit`
  (the drydock; a behavior test sets the screen it came from; reset whenever flight ends), M home.
- **TEST IN RANGE** (`test_range.rs`): the `StartTest` message (handled in every mode: from another mode it launches
  a flight, in flight it rebuilds the scene), `check` / `resolve` of the tested program, and `BehaviorWatch`
  (FixedUpdate, after the events: the tested ship's behavior state after each think). `scene.rs` builds the test
  (`Scene::begin_test`, `test_mode`, `test_ship`, `watched`); the camera and the HUD follow `Scene::watched()` while
  nobody pilots.

- **`sim_bridge.rs`** owns `SimWorld { sim: SimT, content, designs, views, inputs, ... }`. `SimT` is an
  alias for `sim::Sim` (Rapier2D inside the sim; the FreeFlight stand-in was removed on 12 Oct 2026). Views are keyed by `(ShipId, PieceId)`. Content ships compile by id
  (`SimWorld::design`, cached); the player's design compiles from its in-memory blueprint
  (`SimWorld::design_from`, under the id `player:design`, never cached by id). Design visuals are cached per
  compiled design (`Arc` identity), so a design edited and flown again never meets a stale visual.
  `GameContent` holds the content packs, parsed once.

- **One view per sim piece** (`ship_view.rs`): `ship_model::piece_visual` builds the piece's parts in the sim's
  piece frame (pivot = `piece.origin`, so the transform is exactly `piece.render_pose(alpha)`), with the sim's
  line-of-fire flags, shield radii and hull edges (`Ship::piece_hull_edges`). Body mesh variants (style 2a lit /
  dark, without glow sprites lit / dark, debug shapes) are built lazily; `piece.active` picks lit or dark
  (salvage and unpowered pieces are dimmed and lose the hull glow).
- **Events** (`apply_events`): `ShipSpawned` / `PieceChanged` / `Split` (re)build views, `VesselFormed` with a
  parent moves a split-off piece's view to its new ship, `PieceDestroyed` / `ShipRemoved` despawn them;
  `PartDestroyed`, `ShotFired`, `Hit`, `Ricochet` become effects; `ShieldHit` starts a ripple in the bubble (and
  its shatter when it emptied it).
- **Vessels** (`scene.rs`, 12 Oct 2026): the sim owns owners, vessels and the focus (`docs/sim.md` "Vessels").
  The player is `scene::PLAYER` (`Owner(0)`), the raiders `scene::ENEMY` (`Owner(1)`). The sandbox gives every
  vessel its default behavior and sends `ShipCommand::Focus` for the runner; `PilotFocus` mirrors
  `Sim::focus(PLAYER)` at the start of every tick (so a switch shows one tick later), and V / clicks / the fleet
  bar send `Focus` commands. Friendly checks, Tab targets and off-screen markers use owners (`Ship::team`).
- **Fleet bar** (`fleet_bar.rs`, [GDR-0050](gdr/0050-fleet-bar.md)): see below.
- **Camera glide** (`camera.rs`): when the focus changes (V, a click, the fleet bar, or the sim moving a lost
  focus), the follow point snaps to the new vessel and the camera eases from where it was onto it with a
  smootherstep over 0.3 s (a short hop) to 0.5 s (150 m or more), time-based, so the same at any frame rate (a
  unit test checks 30 and 150 fps). Only in follow mode: a free camera stays put, a drag during the glide ends
  it, the zoom is untouched.
- **Lock framing** (`camera.rs`, 13 Oct 2026, a default for review): at the default 18 px/m a 1600 × 900 window
  shows about 89 × 50 m, so a raider locked at 58 m (inside the Runner's 60 m gun range) was off screen. While the
  camera follows and a target is locked, it zooms out just enough to keep the target's centre 90 px inside the
  view's edges (`LOCK_MARGIN`), at most to 35 % of the chosen zoom (`LOCK_MIN_SHARE`; beyond that the off-screen
  marker shows the way), and eases back (time constant 0.1 s, like every zoom) when the lock ends. The player's
  zoom stays the choice (`CameraRig::zoom_target`, what the drydock and home remember); a zoom by hand during a
  lock starts from what is on screen and ends the framing until the lock ends. Not in a free camera, the load
  test's frame or watch mode (nobody locks there).
- **Draw order**: every piece view gets its own layer (z 10..89, round robin) when it is built; the focused
  vessel's piece draws at z 100, decided every frame from `PilotFocus` (`ship_view::sync_piece_views`), so a
  switch reorders at once (before, the z was fixed when the view was built: after a switch the old focus stayed
  on top until its view was rebuilt).
- **Shield bubbles** (`shield.rs`, `shield.wesl`): see below.
- **Effects** (`fx.rs`): immediate mode. Systems push `FxSprite`s into `FxQueue` every frame; `flush` writes them
  as quads into three dynamic meshes (under the ships; weapons and sparks above them; HUD markers on top), one
  draw call each with the atlas material, rewritten in place. Plumes come from `piece.thrusters[i]` ×
  `piece.output[i]` and `piece.cruise[i]` (see Plumes below; RCS nozzles: small puffs; engine gimbal from
  `piece.act.deflect`), beams from `Sim::beams()` re-attached to the gun at the render pose, tracers from
  `Sim::projectiles()` interpolated (beams, tracers, muzzle flashes and pulses all take the design's weapon colour,
  `ship_model::DesignVisual::weapon_color`), loose salvage fragments from `Sim::loose()` interpolated the same way (a
  small diamond of lines in the logic colour, `#ae8cec`, turning slowly in a soft breathing glow; atlas regions
  only, no new texture). `SimEvent::Collected` plays a teal flash where the item was and a pulse streak from there
  into the collecting vessel (`Effects::flash`, `Effects::pulse`); a piece emptied by stripping goes without the
  usual burst.
- **HUD** (`flight_hud.rs`, Barlow from `assets/fonts`, OFL): the home ring top left (what M does, for touch; its
  clicks never reach the world: `HudButton`), hull %, hull map (one cell per part of the shown ship's design,
  rebuilt when the ship or its compiled design changes: every new sim numbers its ships from 0, so the id alone
  can't tell a test flight of an edited design from the last flight), the hold (`CARGO 3 / 8`: cargo units
  in use / the alive cargo parts of the controlling piece, `Ship::cargo` and `cargo_capacity`; empty for a ship
  without a hold), speed with the top speed of the
  best axis (`TOP 22`, from `ship.telemetry().top_speed`; the block ends at mid-height so it stays between the
  hull block and the touch stick on a phone), target panel (in watch mode the watched ship's behavior target),
  wave line, banners (`PAUSED · P` while paused), warnings, help; lock ring and off-screen markers are world-space
  FX (the markers in watch mode too). A marker sits where the ray to its ship meets the view's edge, moved in along the ray until it is clear of
  every HUD text and widget (`HudZone`: hull map, fleet bar, home ring, touch widgets). HUD colours are the
  `ui.*` tokens. Debug overlay and toggles: `hud.rs`.
- **Tuning** (`tuning.rs`, bevy_egui 0.43.1): thrust / torque scale (separate or linked), speed scale (every
  engine's and RCS pod's rated speed, so every top speed; `SimConfig::speed_scale`, replaced the soft top speed
  factor on 12 Oct 2026), max turn rate, key turn rate (A/D held, `keyturn=`, GDR-0055) and the mind capacity (`mind=`) apply live
  (`Sim::set_config`); raiders per wave apply to the next wave; drift at once; weapon range and damage scale
  rewrite the content the designs compile from, so they apply with "Apply & reset".
- **Smaller modules**: `params.rs` (launch parameters: the page URL's query, natively `GAME_PARAMS`;
  `Params::flag` / `f32` / `text`), `controls.rs` (keyboard, mouse and touch → `PlayerInput`: stick, A/D ramp and
  nudge, lock, fire, drift, V / Tab / Esc; the touch widgets), `backdrop.rs` (the radial gradient, two parallax dust
  layers, dotted range rings around the followed ship), `meshbuf.rs` (`MeshBuf`, the CPU builder for
  atlas-textured, vertex-coloured 2D meshes; index order = draw order), `style.rs` (`Style`, the typed view of
  `art/tokens.json`, and `ui_color`).

## Home

The start screen ([GDR-0062](gdr/0062-first-playable-loop.md)), `home.rs`, written 13 Oct 2026: the shipyard's ring
menu around the docked ship ([GDR-0008](gdr/0008-ui-ship-mind-interface.md)), after
`docs/concept/boards/ui-mockups/phone-home-ring-menu.png`, laid out for the desktop. The game opens here (`/`,
`?scene=home`). Screenshots: `docs/results/home-*.png`.

| Keys | |
|---|---|
| Drydock | D, or DRYDOCK |
| BEHAVIOR screen (the working design's behavior; its DRYDOCK pill and Esc lead on to the drydock) | B, or BEHAVIOR |
| Test range: the sandbox with raider waves (a lost ship re-forms; M comes back, T opens the drydock as in any sandbox) | T, or TEST RANGE |
| Debug overlay | F3 |

- **The screen**: top left `THE SHIPYARD · RIM` and the working design's name; in the middle the docked ship in a
  dotted dock rectangle with gold corners; around it a dotted ellipse through six ring buttons (`ui_kit::ring_node`
  with a `RingAccent`, glyphs from `ui_icons`), each labelled beside it (the key dim after the name): DRYDOCK
  (white, top), BEHAVIOR (violet, top left), TEST RANGE (coral, bottom right), and RESEARCH (top right), VOYAGES
  (the big one at the bottom, the mockup's LAUNCH VOYAGE) and FLEET (bottom left) as disabled rings with
  `COMING SOON` until their tracks (7G, 7I, 7F) give them an action. The mockup's galaxy map, fabricator and storage
  wait for their phases; RESEARCHING (top right) and HAUL UNLOADED (bottom left) come with research. The place
  name is UI chrome in code until content has places.
- **The ship** is `drydock::PlayerDesign` drawn exactly as the drydock draws it (`ship_model::design_visual` →
  `blueprint_visual` → `ship_view::build_body`, one mesh, rebuilt when the revision or the glow toggle changes),
  framed by the camera rig in the dock rectangle with a cell of margin (at most 48 px per cell on the 1280 × 720
  layout, so a tiny design stays small). The dotted ring and the dock are immediate-mode FX dots in screen pixels,
  skipping the buttons.
- **Layout**: a 1280 × 720 stage centred in the window; larger windows scale it (`UiScale` = min(w / 1280,
  h / 720), 1 to 2), so 1600 × 900 shows the same layout 1.25× larger.
- **Actions**: the working buttons and their keys send `HomeAction::{Drydock, Behavior, TestRange}`; `apply` sets
  the next mode (TEST RANGE sets `Scene::launch = Some(Launch::Range)`: `OnEnter(Flight)` builds the sandbox as at a
  plain start, not a test flight). A disabled ring carries no action, so it sends nothing. Coming back: the ring
  button top left of the drydock and the BEHAVIOR screen (`DrydockAction::Menu`, `BehaviorAction::Menu`), and in any
  flight M or the ring top left of the HUD.
- **Modes**: the screen and the ship's mesh are spawned once and hidden outside home, like the drydock's. Entering
  home keeps the flight camera's zoom; leaving gives it back with the UI scale 1, so the drydock and flight find the
  camera as they left it.

## Drydock

The ship editor (Drydock v1, decided by Niklas on 12 Oct 2026; the look follows
`docs/concept/boards/ui-mockups/desktop-drydock.png`), `drydock/`, written 12 and 13 Oct 2026: the mode, the view
of the player's design, the screen with its widgets, editing (`data::edit`), the live stats, status line and
overlays from `sim::report`, saving (SAVE, the autosave), the BLUEPRINTS and EXPORT screens, share codes and files
(Storage and sharing below), the BEHAVIOR button and the test flight ([GDR-0056](gdr/0056-drydock-wave-2-defaults.md)).
Parts are unlimited in v1 (no spare counts).

Open it with DRYDOCK at home, `?scene=drydock`, or T in flight; the ring button top left goes home. Screenshots:
`docs/results/drydock-*.png`.

| Keys (only while no text field has the keyboard) | |
|---|---|
| Tools | B paint, G fill, X erase, V select, Q / E rotate (the selected part, else the part to place) |
| Grid (mouse) | left button: the tool (drag: paint and erase strokes, the fill rectangle, moving the selected part); right button: erase |
| Selected part | F flip, Delete / Backspace delete, Esc deselect (Q / E turn it) |
| Mirror / bevel | M / Shift+B |
| Armour material | 1–9 (the ARMOR pills in content order; locked materials are left out) |
| Undo / redo / save | Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y / Ctrl+S (the browser's save dialog stays shut) |
| Search the parts | / (Esc or Enter leaves the field; a click elsewhere too) |
| Zoom | wheel (around the cursor), + / −, the − / + buttons; 100 % = the whole frame fits |
| Pan | Space + drag, or drag with the middle button |
| Test flight | T or TEST-FLY (disabled, with the reason in the status line, while the design can't fly) |
| Debug overlay | F3 (1/2/3/H/P/Shift+R are flight keys) |
| Rename the design | click the name top left; Enter (or a click elsewhere) renames, Esc keeps the old name |
| BLUEPRINTS / EXPORT screens | the buttons; Esc or CLOSE closes them (Esc first leaves a rename, a delete question or a selected box); Ctrl+C copies the selected share code, Ctrl+V pastes into the paste box. While a screen covers the design the other keys do nothing (Ctrl+S still saves; the bottom bar's buttons work) |

The screen, top to bottom: the ring menu button, the breadcrumb `SHIPYARD › DRYDOCK · FRIGATE FRAME` (the
frame's content name) and the design's name (a click renames it; "UNSAVED" or "SAVED" after it); the ARMOR pills
(every unlocked content material with an `armor` block) and the OVERLAY pills (HEAT, MASS, BONUS in gold, SHIELDS,
ARCS: any combination, drawn over the design); left the search, the category pills (content categories) and the
part list (unlocked parts; icon, name, footprint and one key stat from `Content::part_stats`: the first stat after
build points, control, mass, hit points, power draw and heat, else the bonus it gives, else its hit points; the
wheel scrolls the list); the tools above the grid; right the stats grid (POWER, PEAK HEAT, THRUST/MASS, YAW,
FIREPOWER, CARGO, CONTROL, DEVICE LOAD, then a compact TOP SPEED, TURN 90°, MASS, BUILD POINTS block, all from
`sim::report`) and SELECTED PART (the selected placed part with the report's numbers and its controls, else the
picked part: icon, name, footprint and category, summary, stat lines and bonus lines; what is under the name
scrolls with the wheel when it doesn't fit above the bottom bar, with a `MORE BELOW · WHEEL` cue); below the grid the hover
cell, mirror and axis, why the edit under the cursor can't happen (coral), and the zoom; at the bottom undo / redo,
MIRROR, BEVEL, the status line, BEHAVIOR (opens the [Behavior screen](#behavior-screen); no key), BLUEPRINTS,
EXPORT, SAVE and TEST-FLY. Laid out for 1280 × 720 logical pixels; larger windows scale the whole
screen (`UiScale` = min(w / 1280, h / 720), 1 to 2, only while the drydock or the BEHAVIOR screen is open). The phone layout comes later.

- **Design** (`design.rs`): `PlayerDesign` holds the editing model (`data::edit::Drydock`) and the blueprint built
  from it in frame coordinates (`size` = the frame), plus `revision` and `generation`; every change goes through
  `PlayerDesign::edit`, which bumps `revision`; `replace` (another design opened) bumps both (cells of the old
  design, the selection, mean nothing in the new one). At start it is the last session's working design
  (`library::restore_working`, in `BlueprintsPlugin`'s build), else the starter: a copy of `halvex_runner` centred
  in the first content frame it fits (`frames.ron`: the 24 × 24 frigate), mirror on, named "Halvex Runner copy";
  it flies under the id `player:design` (content ids are snake_case, so it never clashes). The player, the
  `?fleet=` wingmen, re-forms and test flights fly it; raiders stay content ships.
- **Report** (`report.rs`): `DrydockReport`, the design's `sim::report`, recomputed once per revision or tuning
  change (during a paint or erase drag at most every 0.1 s, and once when it ends: a full 64 × 64 frame takes about
  5 ms). The BEHAVIOR screen reads the same report (the weapon reach, whether TEST IN RANGE can fly the design). **Panels** (`panels.rs`): the screen's nodes, spawned once and updated in place from the resources (the
  palette list and its key stat). **Behaviors** (`behaviors.rs`): `PlayerBehaviors`, the player's saved behaviors
  on the storage layer, the behavior a design names and its fallback with a banner ([drydock.md](drydock.md#behaviors-storage-designs-and-tests)).
- **Stats and status** (`stats.rs`): whenever `report::DrydockReport` is recomputed, `stats::update` fills
  `DrydockStats` and `DrydockStatus` from it (`CAN'T FLY` for blockers, an empty frame or a design that doesn't
  compile, which sets `grounded`; else `FLIGHT-READY` with the most important warning). Mapping, wording and
  tones: `docs/drydock.md` "Client: stats and overlays".
- **Overlays** (`overlays.rs`): the five overlays and the issue marks, one world-space mesh over the design (a
  child of its mesh), rebuilt when the report or the overlay pills change, and while ARCS is on when the view
  moves (in `PostUpdate` after the camera): the lines of fire and plume lanes (45–75 m) end at the grid
  viewport's edge instead of running through the pills and the status line. Details in `docs/drydock.md`.
- **State** (`state.rs`): `DrydockUi` (tool, armour material, overlays, the brush rotation, the picked part, the
  category, the search text; the mirror and bevel modes are the editor's, `PlayerDesign::dock().mirror` and
  `.bevel`), `DrydockStatus` (`grounded`, the status line and its tone), `DrydockStats` (the stats grid's cells:
  label, value, tone, optional bar); `palette_parts` and `armor_materials` (what the player has: content without
  the `locked` entries). The widgets only show these.
- **Editing** (`tools.rs`, `ghost.rs`, `inspector.rs`, 13 Oct 2026; `docs/drydock.md` "Client: editing"): the
  tools on the grid (`tools::Gesture`, device-free), `tools::Selection` (a cell), undo / redo, the ghost of the edit
  under the cursor from the editing model's previews, the fill rectangle and selection brackets, the refusal after
  the info line (`tools::EditNote`), and the SELECTED PART block (`inspector::Inspector`: the selected placed part
  with the design report's numbers, its armor, power tier and weight, rotate / flip / delete; else the palette
  part).
- **Actions** (`actions.rs`): every button (`ActionButton(DrydockAction)`) and every shortcut sends one message,
  `DrydockAction::{Tool, RotateCcw, RotateCw, Mirror, Bevel, Undo, Redo, Save, Export, Blueprints, TestFly, Behavior, Menu,
  Material, Overlay, Category, PickPart, ZoomIn, ZoomOut, Flip, Delete, Deselect, PartMaterial, PartTier,
  PartWeight}`. Handled here: the UI state and TestFly (when the design can fly: `Scene::launch` +
  `NextState(Flight)`); the camera handles the zoom, `tools::apply` the editing (undo, redo, mirror, bevel, rotate,
  the selected part), `blueprints.rs` save, export and the blueprint list (below), `behavior_ui` the BEHAVIOR
  button; Menu (the ring button) goes home. New handlers read the message in a system in
  `DockSet::Apply` (`Input → Apply → Show`, drydock only), change `PlayerDesign` through `PlayerDesign::edit` (the revision goes up by itself), and
  write `DrydockStats` / `DrydockStatus`; `DrydockCursor::cell` is the frame cell under the pointer.
- **View** (`view.rs`): the design is drawn with the flight tiles: `ship_model::blueprint_visual` (every part;
  line of fire from `ShipGrid::line_of_fire_clear`, the hull edge around all parts, nav lights on the main piece,
  no aura) and `ship_view::build_body` into one mesh, replaced only when `revision` (or the glow toggle)
  changes. Blueprint `(x, y)` is world `(x, −y)`. The frame overlay is immediate-mode FX (dots at the cell
  corners, at least 6 px apart: on small cells only every 2nd, 4th, ... corner, brackets, the dashed mirror axis while mirroring, the hover cell) drawn inside the grid viewport; a
  veil in the backdrop's darkest colour (62 %) dims the world outside it, so a ship zoomed under the side panels
  sinks into the background and the grid area reads lighter than the panels, as in the mockup.
- **Camera**: the drydock drives the shared `CameraRig` (the atlas tier follows its zoom). `CameraRig::view_offset`
  centres the rig on the grid viewport between the panels while the camera still covers the window, and the
  screen ↔ world helpers include it. Opening the drydock remembers the flight zoom; leaving restores it and keeps
  the drydock's view for the next visit.
- **Storage and sharing** (`storage.rs`, `clipboard.rs`, `drydock/library.rs`, `share.rs`, `blueprints.rs`;
  [drydock.md "Storage and sharing"](drydock.md#storage-and-sharing)): `Storage` (named RON documents per
  namespace with a format version; the browser's local storage on the web, files in the user's data directory on
  desktop, memory as the fallback), saved designs and the autosaved working design (restored at start), the
  BLUEPRINTS and EXPORT screens (one floating layer over everything above the bottom bar: `ShelfRoot`, a `Hot`
  veil so clicks reach nothing under it; hidden at once when the drydock is left, with the clipboard's offer and
  paste target let go), share codes and `.ron` files, copy and paste (`Clipboard`). The screens' handlers run
  before the editing tools and the report, so a design opened, renamed or moved to another frame gets its report
  and status in the same frame. Status messages go out through `DrydockStatus` once `stats::update` has made the
  status for the current revision (`blueprints::announce`).
- **Widgets** (`ui_kit.rs`): labels (tracked uppercase), numbers, prose, pills (Barlow medium, as the mockup's semi-bold pills; plain, active teal, active
  gold, the glowing primary, disabled, bare list rows), ring buttons (plain, or glowing in an accent colour:
  `RingAccent`, home's ring), value bars, corner brackets and a one-line
  text field (`TextField`, used for the names, the searches and the notes: a caret and a selection, both drawn in
  place as text spans of `field_text`; Left / Right / Home / End move the caret, with Shift they select, Backspace
  / Delete work at the caret, Ctrl+A selects all, Ctrl+C / Ctrl+X / Ctrl+V copy, cut and paste through
  `Clipboard::field`; a press on the field puts the caret at the end; the editing itself is the pure
  `ui_kit::edit_line`, 13 Oct 2026), a scroll cue (`ScrollCue`: a fade into the backdrop with `MORE BELOW ·
  WHEEL` over a scrolled area's bottom edge while more of it lies below); hit testing by hand from the laid-out rectangles (`Hot`, `UiClick`; the topmost node wins; mouse and
  touch), `UiPointer::blocked` keeps the pointer over panels (`UiBlock`) and widgets out of the grid and the
  camera, `UiPointer::typing` turns the shortcuts off.
- **Icons in the UI** (`ui_icons.rs`): a `UiIcon` on a node (a part, or a glyph Barlow lacks: undo, redo, play,
  search, menu, diamond; home's grid, tree, orbit, arrow, target, formation) is drawn as a world-space mesh of the main camera placed over the node's rectangle every
  frame (screen → world through the rig, scale 1 / zoom, z 600). A part icon is a one-part blueprint through
  `build_body`: the flight tile itself, composited in sRGB like the ships, so its colours are exactly the
  flight tiles' (bevy_ui image nodes would sample the sRGB atlas without encoding for the sRGB compositing and
  look too dark); `atlas::UiAtlasNeed` raises the atlas tier to the icons' pixels per cell. bevy_ui draws after
  the world, so nodes over an icon should have at most a faint background. bevy_ui's clipping doesn't reach the
  meshes: an icon that a scrolled parent clips (`CalculatedClip`) is hidden until it is whole again.

## Behavior screen

How the design on the drydock's grid thinks (Behavior editor v1, [GDR-0057](gdr/0057-behavior-editor-v1.md); lead
defaults [GDR-0059](gdr/0059-behavior-editor-wave-a-defaults.md), [GDR-0060](gdr/0060-behavior-editor-wave-b-defaults.md); the
look follows `docs/concept/boards/ui-mockups/phone-behavior-editor.png`, laid out for the desktop), `behavior_ui/`,
written 13 Oct 2026: the mode, the screen, the graph and text views, the inspector and the palette; behaviors as
saved documents (the name and SAVE, the BEHAVIORS list, the design's behavior) and TEST IN RANGE with a live rule
panel in flight. Both views edit through the editing model `data::behavior_edit` (`docs/behavior-format.md`
"Editing model"): every graph edit is legal (or refused with a reason) and one undo step, and the text view follows.
The storage under it: `docs/drydock.md` "Behaviors: storage, designs and tests".

Open it with the drydock's BEHAVIOR button (bottom bar), BEHAVIOR at home, or `?scene=behavior` (`&behavior=raider_basic` opens a
content or saved behavior instead of the design's). Screenshots: `docs/results/behavior-screen-*.png`,
`behavior-saving-*.png` (a template saved as the design's own copy; the BEHAVIORS list), `behavior-live-watch.png`
(TEST IN RANGE, watch, with the live panel).

| Keys (only while neither a text field nor the TEXT view has the keyboard, and the BEHAVIORS list is closed; Ctrl+S also with the list open) | |
|---|---|
| Back to the drydock (the design is untouched; the draft stays) | Esc, or the DRYDOCK pill (in the TEXT view the first Esc leaves the text; with REPLACE armed Esc disarms it) |
| Test in range (the WATCH / FLY ALONGSIDE pills pick the mode) | T, or TEST IN RANGE; T in the test comes back |
| Save | Ctrl+S (also while the TEXT view types), or SAVE |
| Rename the behavior | click the name top left; Enter (or a click elsewhere) renames, Esc keeps the old name |
| BEHAVIORS list | the button; Esc or CLOSE closes it (Esc first leaves a rename or a delete question) |
| Search the palette | / (Esc or Enter leaves the field) |
| Scroll the graph, the palette or a list | wheel |
| Select a node / a rule / nothing | click it / click its `always` chip or its row's empty space / click the graph's empty space |
| Add a rule | `+ RULE` |
| Add a condition to the selected rule; replace its action; set the target | click a condition / an action / a target selector in ADD NODE |
| Replace the selected condition | REPLACE in the inspector, then a condition in ADD NODE |
| Remove the selected condition / delete the selected rule / back to the default target | Delete or Backspace (or REMOVE / DELETE / USE THE DEFAULT TARGET) |
| Move a rule | drag its row (a chip or the row) up or down; or UP / DOWN in the inspector |
| Undo / redo | Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y, or the two ring buttons after DRYDOCK (dim when there is nothing) |
| A note | click the NOTE field, type, Enter (or click elsewhere) |
| TEXT view | type; Ctrl+C / Ctrl+V copy and paste (through `clipboard.rs` on the web); Ctrl+Z is egui's undo while the text has the keyboard |
| Debug overlay | F3 (in every mode) |

The screen, top to bottom: the ring menu button, the breadcrumb `BEHAVIOR · HALVEX RUNNER` (the design's name) and
the behavior's name (a click renames it) with its saved state after it (TEMPLATE, SAVED, UNSAVED or NOT SAVED YET,
and `THE DESIGN FLIES <NAME>` when the open behavior is not the design's), the `6 / 12 RULE SLOTS` pill (rules / the
design's slots, `Content::ship_rule_slots`, from its logic cores); GRAPH / TEXT; TEST IN RANGE with WATCH and FLY
ALONGSIDE under it. Left the graph: the `TARGET` line
on top (`Nearest` and `DEFAULT` when the program has none), the `Priority · first match` root, the trunk with a
dot per rule, each rule as violet condition chips (content name, values grey (white while the chip is selected), the comparison as written: `Enemies
near 100m >= 3`, `not` in violet) → a connector → a teal action chip (`Orbit 80% · ccw`), actions in one column, the
rule's note dim after it; Rhai conditions dashed with a gold RHAI tag; `always` as a plain chip; the dashed coral
slot line above the first rule without a slot (`6 RULE SLOTS · THE RULES BELOW DO NOT RUN`) and those rules
dimmed (they are kept, not deleted; moving a rule up is how it gets a slot); likewise a never-reached line under
an `always` rule that has rules with a slot below it (`ALWAYS MATCHES · THE RULES BELOW ARE NEVER REACHED`: the
first match decides, and `always` always matches) and those rules dimmed (`graph::never_reached`, 13 Oct 2026); the dashed `+ RULE · N SLOTS LEFT`
pill; a dot grid behind (immediate-mode FX dots). The selected rule's row is lit faintly, the selected chip glows.
Right the inspector (below), ADD NODE (its line says where a pick goes: `ADD NODE · TO RULE 3`, `REPLACE HULL ·
PICK A CONDITION`) with its search and the palette (the content vocabulary: conditions violet, actions teal, Rhai
gold, target selectors white; nothing is listed in code, so the `escort` action and the `escort` template of
13 Oct 2026, GDR-0065, show up in the palette and the BEHAVIORS list without client changes). Bottom the DRYDOCK pill, undo / redo, the status line (a message from saving, opening or testing until the draft
changes; else text errors, rules without a slot, rules never reached, or how rules run; a refusal in coral after it for 3 s, like the
drydock's info line), BEHAVIORS and SAVE (as in the drydock). Larger windows
scale the screen like the drydock. The phone layout and touch come later.

The inspector shows the selection with its kicker (`ACTION · MANEUVER`), its name in its role's colour and its
summary, then its controls (a tall inspector scrolls with the wheel; the palette under it keeps about three rows;
while either has more below, a `MORE BELOW · WHEEL` cue fades its bottom edge).
Screenshots: `docs/results/behavior-editing-condition.png` (a condition with `not`, its comparison and its
threshold), `docs/results/behavior-editing-rule.png` (a rule added with `+ RULE` and a palette condition, its
action selected).

- **Numbers** (`RANGE · 67%  ≈ 100 M`): a −/+ ring on each side of a slider. Steps are round numbers that grow
  with the value and land on multiples (5 m below 50 m, 10 m below 200 m, 50 m below 1000 m; 5 % of the weapon
  reach below 200 %; 5 % for percentages; ½ s below 10 s; whole counts); the slider runs over the node's content
  range (`min`–`max`; 0–1000 % for a share of the reach), soft-logarithmic for distances and times, linear for
  percentages and counts, and snaps to the step; a drag is one undo step. Distances switch between metres and a
  share of the weapon reach (`M` / `% REACH`): with the design's reach (its longest clear gun on the main piece, with
  the tuning's range) the distance stays the same, else the number stays. `DEFAULT` marks a value the rule leaves
  out; RESET writes the default back (and drops it when it is the last value). A threshold's slider runs 0–100 %
  for percentages, 0 to the node's own period for times (`cycle(9s) >= 5s`), 0–20 for counts.
- **Choices** as pills, the chosen one teal (a click picks one), with RESET when written.
- **A condition**: a `NOT` switch (teal when on), for a value condition the `COMPARE` pills (`<`, `<=`, `>`, `>=`) and
  `THAN`, its values, REPLACE (armed: the next palette condition takes its place, the rest of the rule stays) and
  REMOVE; then its rule's block.
- **An action**: its values and its rule's block (a palette action replaces it).
- **A rule's block** (also for a selected rule, with the rule as text): `RULE 2 OF 6` (`· NO SLOT` below the slot
  line, `· NEVER REACHED` below an `always` rule, also in the kicker), UP, DOWN, DUPLICATE (a copy right below) and DELETE (off for the last rule, with the reason), and the
  NOTE field.
- **The target line**: the selector pills, `AIM AT` (centre, engines, weapons, power, core), USE THE DEFAULT TARGET
  when the program has a target line, and the target line's note.
- **A Rhai condition**: its script in gold. **A palette node** with nothing to add it to: its defaults, read-only,
  and how to add it. **Nothing**: the behavior's name and content summary.

- **Draft** (`draft.rs`): `BehaviorDraft`, the behavior being edited: a copy of a content behavior
  (`BehaviorDraft::source_for`: `?behavior=`, else the design's `behavior`, else its faction's default) held in a
  `data::behavior_edit::BehaviorEditor` (`editor()`, `program()`), its text and the text's errors (line, column,
  message). `edit(|e| …)` runs an editing-model operation and prints a changed program canonically into the text;
  `set_text` checks a typed text: valid → the program changes (one undo step); invalid → the program stays,
  `errors` fill. While the text has errors, `edit` refuses (`locked()`: "fix the text first"): the model holds
  only the last valid program, and printing it would throw the typed lines away; the graph is read-only then.
  `revision()` goes up with every change (the graph and the inspector follow it), `generation()` with every
  `replace` (another draft: the selection and an armed REPLACE go). Leaving the TEXT view tidies a valid text to
  canonical form. Opening the screen keeps the draft while the design still flies the behavior the draft was opened
  for (also one opened from the BEHAVIORS list; typed text and the undo history survive a look at the drydock),
  else opens the design's behavior (`DraftSlot`, below).
- **Saving** (`saving.rs`; the rules in `docs/drydock.md` "On the BEHAVIOR screen"): the name field and the saved
  state, SAVE (`save_draft`: a template becomes the player's copy and, when the design flew the template, the
  design names it; a saved behavior is updated), the working draft (`drydock/behavior`, so unsaved edits survive
  leaving the screen and a reload; `DraftSlot` remembers the behavior the design flew when the draft was opened:
  while it still does, the screen keeps the draft, else it opens the design's behavior and saves unsaved edits
  first), `ScreenNote` (the status line's message), and TEST IN RANGE (`test_request`: the draft by id when saved
  and unchanged, else its text; refused with the reason when the text has errors, the behavior can't fly or the
  design can't).
- **BEHAVIORS** (`list.rs`): a layer over the screen above the bottom bar like the drydock's BLUEPRINTS: your
  behaviors (most recently saved first; rules, when saved or the first error; `THIS DESIGN`, `IN THE EDITOR`,
  `UNSAVED` tags) and the templates; OPEN, USE FOR THIS DESIGN, DUPLICATE, RENAME, DELETE (with a confirm) on the
  picked row, NEW BEHAVIOR. The TEXT view's editor (egui) would draw over the layer, so the screen shows the graph
  while the list is open and goes back to the view it had when it closes; only another behavior with errors,
  opened from the list, shows the TEXT view (the same draft with errors stays in GRAPH: switching put the caret
  into the text, and the next T was typed, 13 Oct 2026). A save while it is open (Ctrl+S, SAVE) shows in
  its rows; the wheel over it scrolls only its lists; leaving the screen hides it at once.
- **Context** (`mod.rs`): `EditorContext { design_name, slots }`, set when the screen opens; `BehaviorUi { view,
  selected, test_mode, search }`, the UI state (`BehaviorView::{Graph, Text}`, `TestMode::{Watch, FlyAlongside}`,
  `inspect::Selection::{Node(NodeRef), Rule(RuleId), Vocabulary(id)}`: the model's stable ids, so a selection keeps
  naming the same node while rules move; a selection that an undo or a text edit removes goes, and another draft
  (OPEN, USE, NEW, the design's behavior) clears it).
- **Data for the widgets** (pure, tested): `graph::GraphView::of(editor, slots)` (chips with
  `data::behavior_edit::NodeRef::{Target, Action(RuleId), Cond(CondId)}`, rows with their `RuleId`, `slot_line()`),
  `slots_pill`, `add_rule_label`, `slot_line_label`; `inspect::inspect(editor, selection, Context { slots, reach,
  replacing })` / `behavior_inspection` (the inspector's controls, each carrying the action it sends),
  `inspect::palette_hint`, `inspect::palette(nodes, search)`; `values` (`Kind`, `Scale`: ranges, steps, the
  slider's scale, `switch_unit`, `reach_hint`); `edit::palette_click`, `edit::add_rule_edit`, `edit::delete_edit`,
  `edit::apply_edit`; `reorder::drop_index`; `screen::status_line`; `text_view::highlight` (a line's coloured pieces).
- **Actions** (`actions.rs`): every button (`BehaviorButton(BehaviorAction)`) and key sends one message,
  `BehaviorAction::{Back, Menu, View, Select, Deselect, TestMode, TestInRange}`, the editing group `{AddRule,
  AddNode, SelectRule, Edit(edit::Edit), Replace, Delete, Undo, Redo}` and `{Save, Behaviors}`. Handled here: Back (`NextState(Drydock)`),
  View, Select / Deselect, TestMode, Menu (home); `edit::apply` handles the editing group. `saving.rs` handles Save and TestInRange, `list.rs` Behaviors.
  New handlers read the message in a system in `BehaviorSet::Apply`
  (`Input → Apply → Show`, behavior screen only) and change the program through `BehaviorDraft::edit`; the graph
  and the text follow `revision`. Disabled pills and rings send nothing.
- **Editing** (`edit.rs`, `controls.rs`, `reorder.rs`): `edit::Edit` is one editing-model operation
  (`SetParam`, `ResetParam`, `SetCompare`, `ToggleNot`, `AddCondition`, `RemoveCondition`, `ReplaceNode`,
  `AddRule`, `MoveRule`, `DuplicateRule`, `DeleteRule`, `SetRuleNote`, `SetTargetNote`, `ClearTarget`);
  `apply_edit` runs it and says what the selection becomes (a new condition or rule is selected; a removed
  condition selects its rule). A palette click (`palette_click`): with a rule or one of its nodes selected, a
  condition is added to that rule (or, with REPLACE armed on a condition, takes its place), an action replaces the
  rule's action, a target selector sets the target line; with the target selected only selectors apply; with nothing
  selected the inspector shows the node. `+ RULE` (`add_rule_edit`) adds `always ->` the palette's first action
  (`approach` in the base content: a no-op above an `approach` fallback) above the last rule when that one is
  `always` (the fallback stays last), else at the bottom, and selects it. Refusals (`Refusal`'s `Display`) go to
  `EditState::note`. Sliders (`controls::Slider`) are dragged by `drag_sliders` inside one stroke
  (`begin_stroke` / `end_stroke`); neither the inspector nor the graph is rebuilt while one is held (`SliderDrag`;
  the graph follows when it is let go), and undo / redo wait until then. Note fields (`controls::NoteField`,
  `ui_kit` text fields) write the note when they let go of the keyboard; a refused edit (`EditState::refuse`)
  rebuilds the inspector, so a refused note's field shows the note again. `DesignReach` (the design's weapon reach,
  from the drydock's `DrydockReport`) is measured when the screen opens. Rule rows are
  dragged by `reorder::drag_rows` (a press on a row that moves more than 8 px up or down; a teal FX line shows the
  drop place; letting go sends `MoveRule`).
- **Screen** (`screen.rs`): the frame is spawned once (hidden outside the screen); the graph, the inspector and the
  palette are rebuilt when what they show changes (program revision and slots; selection, name and source, a
  refusal; search). The dashed outline materials are reused from one graph rebuild to the next.
  The graph is a flex row (root column, then a grid: trunk, conditions as max-content, actions as max-content,
  notes), so actions line up and shorter condition groups get longer connectors. Chips carry `ChipLook` and are
  coloured by `style_chips` from their state (idle, hovered, pressed, selected with a glow, dimmed without a slot).
  The graph and the palette scroll (`ScrollPosition`, the wheel); `ui_kit`'s hit test skips nodes clipped out of a
  scrolled parent (`CalculatedClip`). The screen keeps the camera rig's viewport in step with the window, so
  `ui_icons` and the dot grid sit right.
- **TEXT view** (`text_view.rs`): bevy_ui has no text input, so the editor is an egui multi-line `TextEdit` over
  the graph area's node, styled to the look: Barlow (added to egui as the `barlow` family, egui's fonts as fallback),
  no frame, the names coloured like the graph (`highlight`), dim line numbers in a gutter, each error in coral
  after its line (wrapped to the room there, or over the line's end on the backdrop's colour) with the column
  underlined, `no rule slot` after rules beyond the slots and `never reached` after rules below an `always` rule
  (both in the label grey). egui's shader linearises its colours but the main pass
  composites in sRGB, so `egui_color` pre-encodes them (they match bevy_ui's). Switching to TEXT puts the caret in.
  While the editor has the keyboard, the game's `Clipboard` offers the selection (Ctrl+C) and accepts pastes
  (Ctrl+V), which lets the keys through to the browser on the web; egui inserts the text itself. egui keeps the
  editor's undo history under its id; it is cleared whenever the text changed outside the editor (a graph edit,
  another draft, tidying), so Ctrl+Z never brings back another behavior's text. Headless tests must hold modifiers
  across frames (`down ControlLeft; wait 300; press KeyC; …`): egui reads them once per frame.
- **Live rule panel** (`live.rs`, in flight): while TEST IN RANGE runs, a compact panel right under the target
  panel: `BEHAVIOR · <NAME>`, which rule decides (or `NO RULE MATCHES · IT HOLDS`), every rule as one line of the
  graph's chip labels (`Shield down and Target on me  ›  Orbit 80% · ccw`), the deciding rule teal in a pill, rules
  without a slot dimmed, and under each rule above it, in the label grey, why it doesn't match (the sim's `describe`: `hull < 25% is
  false (hull is 96%)`). It follows `test_range::BehaviorWatch` (10 Hz): the rules' labels are made and the rows
  spawned when the behavior changes, only colours and why-lines change in between. Hidden outside tests and with
  the HUD (H).
- **Dashed outlines** (`dashed.rs`, `dashed.wesl`): a `UiMaterial` (`DashedOutline { color }`) on the chip node
  itself draws a dashed stadium as wide as the node's border (physical px), dashes spaced to close evenly. Its colour
  is an sRGB colour as numbers (`ui_color`), returned unchanged.

## Fleet bar


[GDR-0050](gdr/0050-fleet-bar.md) (provisional), `fleet_bar.rs`, written 12 Oct 2026. A strip at the top centre,
under the scene title, shown while the player flies a vessel and has two or more vessels or any dormant one.

- **Mind row**: `MIND 8 / 24`, `Sim::mind_load(PLAYER)` (logic cores running against the mind capacity), and a thin
  violet bar.
- **Chips**, one per vessel in `Sim::vessels(PLAYER)` order (ascending ship id, the V order), numbered from 1:
  violet pips for the logic cores (`VesselInfo::cores`), a thin bar of the control load against the capacity
  (`VesselInfo::control`: teal, gold above 85 %, coral while parts idle; empty while dormant) and a state word:
  `PILOT` (the focus: teal number and corner brackets), `AUTO`, `DARK` (coral; a dark focus keeps its teal
  number and brackets), `DORMANT` (dimmed). Hover
  brightens the brackets. Chips are 68 × 50 px.
- **Input**: a click or tap on a chip sends `ShipCommand::Focus` (running or dark vessel) or `ShipCommand::Wake`
  (dormant: it swaps in, the newest vessel not woken since goes dormant, GDR-0044); a tap on the focus does
  nothing. The bar hit-tests its laid-out node rectangles itself (bevy_ui's `Interaction` is deprecated in
  0.20), before the controls; `FleetBar::blocks` keeps those clicks and taps out of the world (no lock, no fire,
  no stick).
- **Many vessels**: as many chips as fit between the hull block and the target panel (3 to 12); beyond that a
  window around the focus and `+N`.
- The chips are a fixed pool of nodes updated in place every frame (`Display::None` when unused); text, colours
  and bar widths are written only when they change.
- Colours: `ui.*` in `art/tokens.json` (`style::UiPalette`).

## Plumes

An engine plume shows two things from the sim (GDR-0041, `docs/sim.md` "Top speed and the cruise flame"):
`piece.output[i]`, the share of the engine's thrust it actually makes this tick (after power and the rated-speed
falloff), and `piece.cruise[i]`, the calm cruise level: how much of its top speed the helm asks the engine to hold
(full stick at the top speed = 1, half stick = 0.5; released stick and drift = 0).

- The flame's strength is `level = max(output, cruise)` (length from idle to full, opacity, width).
- `calm = max(cruise − output, 0) / level`: 0 while the engine burns at least as hard as it cruises
  (accelerating, braking, turning: the normal long flickering flame), 1 when it only cruises (at the top speed,
  or holding a lower speed). The calm look, blended by `calm`: the flame is shorter (`ship.plume.cruise.length`,
  0.75), flickers less (`flicker`, 0.2 of the burn's wobble), has a hot core (a soft streak in `core_color` /
  `core_color_enemy` down the first `core_length` = 60% of the flame, `core_width` = 32% of its width, opacity
  `core_opacity` × calm × level) and sheds fewer trail particles (`trail`, 0.35).
- RCS nozzles have no cruise level: they puff only while they push.
- **A burning plume** (GDR-0049): while `SimEvent::PlumeHit` arrives for an engine (every tick its plume burns a
  part, its own hull, an enemy or salvage), a faint streak in the ship's glow colour runs from the nozzle out to
  the burning point (the sim's reach is 3–6 m, the drawn flame 0.4–2 m) and a small orange ember glows there; both
  fade 0.12 s after the last hit (`fx::Effects::burn`). Not seen in a screenshot yet: the base ships' engines
  exhaust into open space, so it shows only at close quarters.
- All style numbers live in `art/tokens.json` under `ship.plume.cruise` (read once into `style::PlumeStyle`).

## Shield bubbles

The look is [GDR-0043](gdr/0043-shields-as-energy-bubbles.md) (values: `ship.shield_bubble` in `art/tokens.json`);
the technique is [ADR-0029](adr/0029-shield-bubbles-batched-procedural-mesh.md). Written 12 Oct 2026.

- **Two meshes, one material, two draw calls for every bubble.** `draw_bubbles` walks the piece views every
  frame and writes one quad per visible bubble (centre at the render pose, half-size = radius + 4 glow widths,
  more while shattering) into a world-space mesh, in place, like the FX meshes: the focused vessel's bubbles at
  z 200 (above it, below the weapon effects), every other bubble at z 95 (above every other ship, below the
  focus). So the focus flying over another ship's bubble stays on top of it (12 Oct 2026; before, all bubbles
  were one mesh above every ship). Two non-focus ships still show the other's bubble over their hull: per-ship
  layers would need a mesh per ship (ADR-0029's rejected option) or depth writes from the ship meshes. Each quad carries its bubble's state as vertex attributes: offset from the centre, radius,
  seed; charge, presence, shatter progress; four hits (direction in world space, age, strength). The material
  (`BubbleMaterial`) holds only the style tokens and is never changed, so there are no per-frame material
  updates; the shimmer and flicker run on `globals.time` (virtual time: they pause with P).
  Why: per-bubble materials (the old rings) cost a draw call and a bind-group update per bubble per frame;
  WebGL2 has no storage buffers, and a uniform array would still change the material every frame.
- **Max blending** (colour and alpha, factors One/One): overlapping bubbles of a ship show the brighter of the two
  instead of a bright lens, and quad order does not matter. A bubble only adds light: its faint fill vanishes over
  hull tiles (keeps the ship readable), the rim and the hits still show. The shader outputs `colour × intensity`,
  with the colour sRGB-encoded under `SRGB_OUTPUT` (the camera composites in sRGB, see Gotchas).
- **State** lives in `ShieldBubbles`, keyed by `(ShipId, emitter part)`, not in the piece view: the shot that
  empties a bubble often destroys a part too, and the rebuilt view would cut the shatter short. A bubble shows
  when the sim projects it (`piece.active`, emitter alive, `work > 0`, charge > 0, not `shield_down`: the same
  test as `weapons::collect_bubbles`); it grows in over 0.35 s and fades out over 0.12 s. Hits closer than
  0.16 s and about 20° to an existing one strengthen it instead of adding a ripple (beams hit every tick); a fifth
  hit replaces the oldest. A ripple's direction is taken at the pose the hit was resolved at (shots and plumes:
  the tick's start pose, before physics; rams: its end pose). A piece that splits off and becomes a vessel
  (`VesselFormed`) takes its bubbles' state to its new ship id. States without a view are dropped each frame;
  Shift+R clears them.
- **The shader** (`shield.wesl`, WESL, embedded with `embedded_asset!`): rim line with an energy-conserving width
  when it is thinner than a pixel (`fwidth`), glow, fresnel-like fill, two value-noise layers for the shimmer and
  veins, a time-hashed stutter below `low_share`, four hit terms (ripple ring, impact flash, light along the rim),
  and while shattering a Voronoi crack pattern plus 20 rim shards. The Voronoi only runs while a bubble shatters.
- **Debug shapes** (`1`) hide the bubbles; the glow-sprite toggle (`2`) does not affect them (they are not sprites).

## The atlas at high zoom

Icons, glows, plumes and the FX dots come from one texture atlas that `atlas.rs` rasterises at runtime
(ADR-0021, ADR-0028) at a *tier*: pixels per ship cell, a power of two from 32 to 512. Written 12 Oct 2026.

- **Which tier**: what the screen needs, `zoom × window scale factor` (px per cell; the UI's part icons' need,
  `UiAtlasNeed`, when larger), rounded up to a power of two
  and capped by the GPU's largest 2D texture (tier 512 is 4096 × 3840; logged once at start). Up at once, down
  only when half the tier is clearly enough. A new tier must be wanted for 0.25 s first (zooming through tiers
  costs nothing).
- **Rebuild**: regions one by one, then the mip chain a band of rows at a time (`art::Rasterizer::step`), within
  6 ms of work per frame, 16 ms while the atlas on screen is stretched more than 2× (it looks blurry then). The old
  atlas stays on screen until the new one is complete. A job for a lower tier than the zoom now needs is dropped
  for the new one (and a job that would rebuild the tier already on screen is dropped too). Native release
  timings (12 Oct 2026): tier 256 ≈ 0.13 s of work, tier 512 ≈ 0.45 s (regions 0.42 s, mips 0.15 s in 400 steps
  of at most a few ms); the browser takes a few times longer. Before, the mip chain was built in one frame
  (178 ms at tier 256 in the browser).
- **Switch** (`atlas::rasterise_step`): a finished atlas is added as a *new* image; the next frame the material's
  texture handle is set to it (written through `get_mut`, so Bevy re-prepares the material and rebuilds its bind
  group) and the old image goes with its last handle. One frame later because Bevy 0.20 does not order a
  material's preparation after its images' (both run unordered in `RenderSystems::PrepareAssets`): switching in
  the same frame could look up a texture that is not uploaded yet (Bevy would retry the material, and every
  atlas mesh would vanish for a frame). A new image per tier also avoids Bevy's in-place update for an unchanged
  texture size, which writes mip level 0 only and would leave stale mips. Memory: one atlas on the GPU (tier 32:
  0.3 MB, 64: 1.3 MB, 128: 5.2 MB, 256: 21 MB, 512: 84 MB, mips included), two for the switch frame.
- **Sampling** (`AtlasMaterial`, `atlas.wesl`): every atlas mesh (ships, effects, backdrop, the drydock's view and
  its UI icons) draws with one material: atlas × vertex colour, alpha-blended, sRGB-encoded under `SRGB_OUTPUT`
  like Bevy's `ColorMaterial`, which it replaced on 13 Oct 2026. The sampler is linear / linear / linear
  (trilinear), clamp to edge. The shader samples with a mip LOD bias of −0.5 (`textureSampleBias`; WebGPU
  samplers have no bias): the tier is the next power of two above what the screen needs, so the GPU's level of
  detail lies between 0 and 1 at the tier's own zooms, and plain trilinear sampling blended in up to all of the
  next level, which has less resolution than the screen (at 160 px/m on tier 256: 68 % from the 128 px/cell
  level; icon edges ran over about 2 px where tile edges run over 1). With the bias the full level is used
  until it is minified about 1.4×, and past 2× the next level takes over as before, so minified icons don't
  alias. Same cost as before (one texture sample, one material, the same batching).
- **What was wrong until 13 Oct 2026** (Niklas: pixelated, stair-stepped icons at 160 px/m while F3 said
  `atlas 256 px/cell need 160`): the image was replaced in place and the material "touched" with
  `let _ = materials.get_mut(id)`. In Bevy 0.20 `Assets::get_mut` returns an `AssetMut` guard that marks the
  asset modified only when it is written through, so that touch did nothing: the material kept the bind group
  of its first preparation, and every atlas mesh sampled the start atlas (64 px/cell, or 32 when the first
  rebuild landed before the material was prepared) for the whole session, stretched 2.5–5× at 160 px/m, while
  every later tier was rasterised and uploaded for nothing. Evidence: headless shots at 160 px/m with different
  tier histories (64 → 256 and 64 → 32 → 256) had identical icons (mean difference ≤ 0.2 of 255, against 6.2
  between before and after the fix); icon edges ran over 3–5 px. Comparison:
  `docs/results/atlas-sharpness-flight-3440.png` (showcase, 3440 × 1305) and
  `docs/results/atlas-sharpness-drydock.png` (drydock at 160 px/m).
- **Soft by design**: icon glows and plumes are stored at half density (they are blurred); at the highest zoom a
  plume's edge is about 1.5 px soft. Everything else (icons, barrel) is 1:1 with the tier; tiles, seams, hull lines
  and shield bubbles are geometry or shaders and stay sharp at any zoom.
- **Checking blur**: F3 shows `atlas <tier> px/cell  need <px> (zoom x scale)  max <tier>  window <physical size>`
  and the last rebuild; the tier is the one the material samples (it changes with the switch). If the tier stays
  below `need` the rebuild is slow or capped; if `window` is not the CSS size × the device pixel ratio, the canvas
  renders at a lower resolution than the screen. Icons that look stretched while the tier is at or above `need`
  point at the binding (what went wrong before).
- **Headless HiDPI**: Playwright's `deviceScaleFactor` alone renders the canvas at CSS resolution
  (`devicePixelContentBoxSize` reports CSS pixels), which looks blurry for the wrong reason; add Chromium's
  `--force-device-scale-factor` with the same value to test a real HiDPI screen.

## Adding visuals

- **A new part icon**: add `content/base/icons/<key>.svg` (one file; its glow is the
  `<g filter="url(#gl)" opacity="0.55">` group, which the `art` crate splits off) and set `icon: "<key>"` on the
  part. `data` embeds the pack, validation checks that the icon exists, and `art`'s test
  (`content_icons_rasterise_at_two_tiers`) checks that every content icon rasterises.
- **A gun's barrel**: `barrel: (width, length, muzzle)` on the part in the content (`docs/content-model.md`); a gun
  without one gets a small muzzle glow at its front edge (`ship_model::design_visual`).
- **A new effect**: add a constructor to `fx::Effects` (sparks, flash, line), call it from the event
  match in `sim_bridge::apply_events`, draw it in `fx::draw_effects`. Continuous visuals that follow sim state
  (like plumes) go in `fx::draw_ships_fx` and read the sim at the render pose.
- **A new FX texture**: add it to `Layout::new` in `crates/art` (as SVG or a `Proc` function of `(u, v)` with a
  size in atlas units), look it up once in `fx::FxRegions`.
- **HUD text**: add a `HudText` variant in `flight_hud.rs` and fill it in `update_text` (8 Hz, re-laid out only
  when the text changes). UI colours go through `style::ui_color` (the world composites in sRGB space and
  bevy_ui does not encode for it).

## Gotchas

- `CompositingSpace::Srgb` (browser-like blending) needs `Hdr` on WebGL2 in Bevy 0.20, see `main.rs`. A custom
  `Material2d` shader has to encode its output itself when `SRGB_OUTPUT` is defined (like `ColorMaterial`,
  `atlas.wesl` and `shield.wesl` do).
- Bevy 0.20's `Assets::get_mut` marks an asset modified only when it is written through the returned
  `AssetMut`; `let _ = assets.get_mut(id)` changes nothing. A material's bind group is built from the images
  present when it is prepared and is not rebuilt when an image changes (texture size and so the GPU texture
  changing included): change the material itself, a frame after adding the new image (see the atlas switch).
- Bevy 0.20 shaders are WESL: plain `.wgsl` files get no imports. Embed a shader with `embedded_asset!(app,
  "x.wesl")` and refer to it as `embedded_path!("x.wesl")` with the `embedded` source; import the 2D mesh
  bindings as `bevy_sprite_render::mesh2d::{functions, view_bindings::globals, …}`. Take derivatives (`fwidth`)
  before any branch on varyings (WGSL uniformity rules).
- Debug wasm builds are ~100 MB and the sim runs with debug assertions (state hash every tick); use
  `scripts/web-serve.sh --release` for timings.
- SwiftShader (headless Chromium) runs the style-2a scene at a few fps: its numbers say nothing about real GPUs.
