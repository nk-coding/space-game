# Client (`crates/client/kit`, `world`, `game`)

The Bevy 0.20 client. Web (wasm32 + WebGL2) is the development platform; desktop and Android come later and
must keep compiling (`cargo check -p game` natively). Written 11–13 Oct 2026; split into three crates on 14 Oct 2026
(ADR-0038, see [Structure](#structure)).

## Build and run

| What | Command |
|---|---|
| Dev build with live reload | `scripts/web-serve.sh` → http://10.77.9.2:8081 (port via `PORT=`) |
| Optimised build, served | `scripts/web-serve.sh --release` |
| Optimised build into `crates/client/game/dist` + sizes | `scripts/web-build.sh` (cargo `release`, opt-level 3, wasm-opt -O3), then `scripts/web-serve-dist.sh` |
| Atlas preview / timing | `cargo run --release -p art --example preview [-- <tier> <scratch.png>]` (the game rasterises its atlas at runtime; nothing raster is committed); the longest step and frame of a job as the game spends it: `... --example profile -- <tier> [runs]` |
| Headless check | `node /home/devbox/workspace/tools/headless/shot.mjs <url> <png> <waitMs>` |
| One bounded check (build, serve on a free port, headless shot, console, stop) | `scripts/web-check.sh [--release] [--params '?load=5'] [--wait MS] [--size WxH] [--out FILE]` |
| Native load test table | `scripts/load-test-native.sh [--ships 1,5,20] [--ticks 1800] [--seed 42]` |
| Scripted input + screenshots | `node scripts/web-drive.mjs <url> <out-dir> "wait 20000; down KeyW; wait 3000; up KeyW; shot fly.png"` |

Port 8080 is Niklas's copy, served by the lead session from `/home/devbox/workspace/serve/game`; agents use 8081.

## Controls

PC keyboard controls are ship-relative ([GDR-0040](gdr/0040-keyboard-flight-and-smart-lock-aim.md)); touch keeps
the stick ([GDR-0009](gdr/0009-stick-and-lock-controls.md)). **Every key and mouse button below except Esc, Enter, the
left button as the pointer, middle-drag and the wheel is an action of the action map** ([GDR-0090](gdr/0090-action-map-settings-and-pause-menu.md);
`data::input`, defaults in `content/base/controls.ron`, the player's changes in `settings/controls`): the table names
the defaults, SETTINGS rebinds them, and the screens read them through `world::settings::Act` (tooltips with an
`ActionTip`, key badges with an `ActionLabel` and the flight help line show the bound inputs). The A/D rotation ramp is integrated by the scene once
per sim tick with `sim::flight::turn_ramp_step` (like the lock's nudge with `nudge_step`) and sent as
`PilotAxes::turn`; see `docs/sim.md` "Keyboard rotation".

| | PC | Touch (landscape) |
|---|---|---|
| Fly | A/D rotate (GDR-0055, numbers in GDR-0058: a tap nudges the nose a little, holding ramps up to the key turn rate, 1.5 rad/s, in 0.8 s; never faster than the ship's gyros, cores and RCS can stop), W/S thrust along the nose, Q/E strafe (arrows: rotate + thrust). The flight computer cancels sideways drift while keys are held; released keys coast, nose kept (GDR-0086) | virtual stick bottom left: "fly that way", push = speed |
| Drift toggle (pure momentum while keys are held: no drift cancelling; a release coasts either way, GDR-0086) | X | tuning panel |
| Lock a target | left-click a ship (or a salvage piece) | tap it |
| While locked | the ship turns its strongest working guns onto the target (lead point for projectiles); A/D nudge the aim and it springs back; W/S and Q/E still thrust and strafe | the stick strafes |
| Unlock | click empty space, or Esc (with nothing locked Esc opens the flight menu: RESUME, SETTINGS, SHIPYARD or ABANDON VOYAGE…; it pauses the test range, never a voyage; Esc closes it, GDR-0090) | tap empty space |
| Next target (nearest first) | Tab | – |
| Fire (by weapon group, GDR-0088: each gun's group is set in the drydock, a gun without one is group 1) | Space: every group; 1 / 2 / 3: that group; hold the left button on empty space: group 1; hold the right button: group 2. The held triggers OR into the pilot's fire mask (`PlayerInput::fire`, `sim::PilotInput::fire`); the help line shows `SPACE FIRE · 1/2/3 GROUPS` from the bindings | fire button bottom right: every group |
| Pilot assists (GDR-0088; the design's PILOT ASSISTS, set on the BEHAVIOR screen): switch the assists on toggle 1–4 on and off for the mission / switch every assist off and back on (as the toggles left them). Each switch is a `ShipCommand::SwitchAssists` for the flown ship; the banner says what changed (`BRAKE ON · AUTO-FIRE 2 OFF`, `ALL ASSISTS OFF`, `F3 · NO ASSIST ON TOGGLE 3`) | F1–F4 / Z | – |
| Switch vessel (the focus) | V / Shift+V (the sim's vessel list, ascending ship id), click a friendly vessel, or click its chip in the fleet bar | tap a friendly vessel or its chip |
| Wake a dormant vessel (swap) | click its chip in the fleet bar | tap its chip |
| Zoom (3–160 px per metre; while a target is locked the camera zooms out enough to keep it in view, and it widens up to 1.4 × at top speed: "Lock framing" and "Speed widening" under [Structure](#structure)) | wheel, + / − | pinch |
| Camera | F: follow (load test: frame) ↔ free; drag with the middle button (the left button too while nobody is piloted: showcase, load test, watch) | – |
| Drydock (back to the ship editor; the design is untouched); in TEST IN RANGE back to where the test started; nothing in a voyage | T | – |
| Home (the shipyard; the flight ends); in a voyage M opens the ABANDON VOYAGE? panel instead (GDR-0077: Enter abandons the voyage, lost; Esc or M again close it) | M, or the ring top left (not in a voyage) | the ring top left |
| Recall home from a voyage (when no armed awake enemy is within 200 m, or the fleet has no working gun left: the emergency recall; a 3 s warp-out) | R, or the RECALL pill | the RECALL pill |
| Raider waves off / on (sandbox; off removes the raiders) | C | – |
| Tuning panel (flight only; it was T until 12 Oct 2026) | U | – |
| Debug overlay (fps, frame and sim times, the focus ship's heading, turn rate, speed and whether the last resort turns it, the locked ship's active behavior rule; in TEST IN RANGE every rule of the tested behavior with why) | F8 (F3 until 15 Oct 2026: the assist toggles took F1–F4) | – |
| Render toggles | F5 debug shapes, F6 glow sprites, F7 bloom (1, 2 and 3 until GDR-0090: they are the fire groups' keys now) | – |
| Hide HUD / pause (`PAUSED · P` shows) / reset scene (not in a voyage) | H / P / Shift+R | – |

Ships with engines only at the back can't brake or strafe while locked (GDR-0011); S does nothing on them.

The keys reach the sim tick of the frame they are pressed in, and a tap shorter than a tick (33 ms, even a press
and release within one frame) still reaches one tick: the controls run before the frame's ticks and latch the
keys and fire buttons until the next tick takes them (`controls::PlayerInput::for_tick`, 13 Oct 2026; since GDR-0088
step 2 the latch ORs the weapon groups held at any frame since the last tick).
Home's keys are in [Home](#home), the drydock's in [Drydock](#drydock), the BEHAVIOR screen's in
[Behavior screen](#behavior-screen), a voyage's in [Voyages](#voyages).

**SETTINGS** (`game::settings`, GDR-0090): a layer over home (the gear ring top right, or O) and over flight (the flight
menu's SETTINGS; the HUD and the menu step aside, a dark ground covers the ships). CONTROLS lists every action by
section with two input chips: click one, then press a key or mouse button (with Ctrl, Shift or Alt held: a
combination; a modifier alone binds on release); Esc cancels, CLEAR empties it, an input another action on a shared
screen had moves over (the status line names it), RESET TO DEFAULTS. DISPLAY: GLOW, BLOOM, UI SIZE (100 / 90 / 80 % of
what fits the window, `kit::ui_kit::UiSize`, applied by every screen's UI scale). Both save at once on this device
(`settings/controls`, `settings/display`); URL `glow=` / `bloom=` still win for one run.

## URL parameters

| Parameter | Meaning |
|---|---|
| `scene=home` | start at home, the shipyard (the default) |
| `scene=sandbox` | start in the sandbox (home's TEST RANGE) |
| `scene=drydock` | start in the drydock |
| `scene=behavior` | start on the BEHAVIOR screen (the design's behavior) |
| `scene=research` | start on the research screen, over the lab ([Research](#research)) |
| `scene=map` | start on the voyage map ([Voyage map](#voyage-map)) |
| `behavior=<id>` | the BEHAVIOR screen opens this content or saved behavior (e.g. `raider_basic`) instead of the design's |
| `scene=showcase` | the player's design holding position, three raiders orbiting it, no combat |
| `scene=showcase&fire=1` | the same, but the raiders (`raiders=N` of them, default 3) attack the design under their behavior: for looking at shield hits (add `zoom=70` for a close-up) |
| `scene=watch&behavior=<id>` | TEST IN RANGE, watch: the player's design flies unpiloted on the behavior (a content or saved one; without `behavior` the design's own) against raider waves, the camera follows it; T returns to the drydock |
| `scene=alongside&behavior=<id>` | TEST IN RANGE, fly alongside: you fly the design, a second copy is a wingman on the behavior |
| `scene=voyage&voyage=<id>` | launch that voyage at once (without `voyage`: the one the voyage map selects when it opens), for development: recorded in the career when it may launch, else a practice run that is never saved ([Voyages](#voyages)) |
| `save=<slot>` | play the career save `saves/<slot>` instead of `saves/career` (a new game if it doesn't exist; [Career](#career)) |
| `newgame=1` | start over: a new career replaces the save (no confirm: a development switch; every load with it starts over again), and the drydock docks the start's ship |
| `unlock=all` | free mode: every part, material and frame in the drydock, nothing locked or NEW; no unlock is written into the save |
| `load=1\|5\|20` | browser load test: `sim::voyage::scenario::load_test(n, seed)`, camera frames the fight, stats overlay on |
| `seed=N` | sim seed (default 42; a whole number, exact up to 2^64) |
| `hashlog=1`, `hashticks=N` | log the per-tick state hash of the load scenario (20 ships unless `load=` is given) to the console, `N` ticks (default 600, at least 1) |
| `thrust`, `torque`, `speed`, `turn`, `keyturn`, `range`, `damage`, `raiders`, `drift`, `link` | tuning values (the panel prints them, `link` aside; `keyturn` is the A/D key turn rate, default 1.5 rad/s; `speed` replaced `top` on 12 Oct 2026). `range` scales every side's weapons, `damage` only the player's (the raiders keep their content damage, 12 Oct 2026). `raiders=N` sets every wave's size by hand; without it waves follow the flown design (GDR-0074) |
| `mind=N` | the player's mind capacity in logic cores (default 24; a runner has 2): `?fleet=3&mind=4` leaves two runners dormant, for trying the fleet bar's swap |
| `fleet=N` | N extra copies (at most 8; none in TEST IN RANGE) of the player's design fly with you under the behavior the design names (default its faction's, `skirmisher`), following your lock (`target locked`); switch with V or the fleet bar |
| `tuning=1` | open the tuning panel at start |
| `debug=1`, `glow=0`, `bloom=1`, `hud=0`, `stats=1` | render toggles, HUD, debug overlay |
| `zoom=N` | start zoom in px per metre (3–160) |
| `touch=1` | show the touch widgets without a touch first |
| `slowmo=N` | the game's clock runs N times slower (1–50; the sim steps the same ticks, only later): for looking at animations such as the voyage's warp in headless screenshots. Real-time parts (screen transitions, the lift after a voyage) keep their speed |

Natively the same `key=value&…` string comes from the `GAME_PARAMS` environment variable (`params.rs`). A bare
key means 1; `0`, `false`, `off` and `no` turn a flag off.

Where the game starts (`mode::AppMode::at_start`, since 13 Oct 2026, GDR-0062): `scene` decides when it is given
(an unknown one starts at home). Without it, a parameter that only means something in flight starts the sandbox, as
before home existed: `load`, `hashlog`, `hashticks`, `fleet`, `seed`, `zoom`, `fire`, `raiders`, `mind`, `tuning`,
`touch` (`params::FLIGHT_ONLY`); so `?load=20`, `?hashlog=1&load=20` and `?fleet=3&mind=4` fly as they did. Anything
else (`/`, `?debug=1`, `?stats=1`) starts at home; the sandbox alone is `?scene=sandbox`.

## Scenes (`scene.rs`)

- **Sandbox** (home's TEST RANGE, `?scene=sandbox`; the HUD title says `TEST RANGE`): the player flies their design (`player::PlayerDesign`; at the first start and a new game
  a copy of the career's start ship, the Rim skiff; after that the autosaved working design). Waves of enemies (the content ships tagged `enemy`:
  the base pack's automaton raider, hunter and warden; their faction's behavior, `raider_basic`) spawn 105–125 m away; a new wave starts 4 s after the last
  enemy vessel goes dark (a raider shot in half can be two raiders). **A wave follows the flown design** (GDR-0074, GDR-0089, `scene::wave_mix`, a placeholder rule):
  half the design's build points (`WAVE_SHARE`) are spent on the biggest enemies first, each as often as it fits whole, then one more
  of the smallest if at least half of it is left; at least 1 ship, at most 8 (`WAVE_CAP`). The starter `rim_skiff` (59 build points) meets a hunter
  (23.6), the Halvex Runner (132.5) two hunters and a raider (15.8), a 300-point design two wardens (75) (Niklas, 15 Oct 2026: a single ship
  against a swarm of small ones felt wrong; until then a wave was only raiders, 2 and 4 of them, each a wave a pilot stand-in won about nine times in ten,
  `docs/results/slice-balance-2026-10-13.md`); the
  tuning panel's "follow the flown ship" box (or `raiders=N`) sets the size by hand. C turns the waves off (every raider vessel
  is removed at once, their wrecks stay, no new wave comes) and on again (a wave 2 s later); the subtitle says
  `WAVES ON` / `WAVES OFF` (C turns them on, the help line says so), Shift+R keeps the setting. Wrecks and salvage drift away and are removed when
  they are more than 320 m from the player or dark for 75 s (the player's vessels, dark or dormant, stay). When
  the focus loses its last logic core the sim moves the focus to another vessel of the player (a split-off half
  too); when the player has no vessel left, a new one re-forms after 3 s (its wreck, with whatever its hold
  carried, is removed then).
- **Salvage in the sandbox** (GDR-0063, 13 Oct 2026; the first feel check of track 7B): every running vessel of
  the player collects (`SimConfig::salvage`, the sim's defaults): fly within 5 m of a wreck (raiders shot down,
  their split-off pieces) or a loose core fragment, no faster than 8 m/s relative to it (GDR-0079, 14 Oct 2026; 4
  m/s before), and one part every 1/3 s goes into the hold, best first (`docs/sim.md` "Salvage"). The HUD shows
  `CARGO n / m` under the hull; nothing is done with the haul yet. Loose items (core fragments, spilled cargo) last
  60 s here (`scene::SANDBOX_LOOSE_TICKS`; the sim's default keeps them until it ends, as in a voyage, but the
  sandbox's waves never end, so they would pile up); the tuning panel keeps the scene's salvage settings.
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
  `behavior_ui/live.rs`), the debug overlay (F8) every rule with why.
- Every sandbox flight flies the **player's saved behaviors**: they join the player's content, so the library has
  them and a design naming one flies it (`player::behaviors`).
- **Showcase**: no combat, free camera; for looking at the renderer. With `fire=1` the raiders (the tuning's
  raiders per wave set by hand, `raiders=N`, else 3) start 46 m out and attack the design, which holds still and doesn't fire back:
  its bubbles take hits at once. Cannon shots pass half their damage to the hull, so the Runner breaks up after
  about half a minute (Shift+R starts over), and with the base numbers (regen 1.5/s against about 0.4 per
  hit) the bubbles rarely empty; collapses need concentrated fire.
- **Voyage** (`?scene=voyage&voyage=<id>`, home's V): see [Voyages](#voyages).
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

**Three crates** (ADR-0038, 14 Oct 2026), each using only the ones below it; the module names stayed, so the file
names in this document are found in the crate that holds them:

| Crate | Modules |
|---|---|
| `crates/client/kit` (the toolkit: knows nothing about ships or the sim) | `atlas` (+ `atlas.wesl`), `style`, `meshbuf`, `ui_kit` (+ `widgets`, `field`, `tip`, `fonts`, `transition`, `scrollbar`), `ui_icons`, `clipboard`, `params` (+ `RenderToggles`), `view` (`ViewScale`: the camera's zoom and place as the atlas and the icons read them, written by `world::camera`), `rng`; the fonts in `assets/fonts` |
| `crates/client/world` (the running game) | `sim_bridge` (+ `PilotFocus`), `scene`, `ship_model`, `ship_view`, `part_icons` (part and ship icons: `ui_icons::IconShapes` for the content), `fx` (+ `blast`), `shield` (+ `shield.wesl`), `backdrop`, `camera` (+ `spawn_camera`), `controls`, `hud`, `flight_hud` (+ `text`, `markers`), `fleet_bar`, `test_range`, `tuning`, `mode`, `voyage` (+ `debrief`: the debrief's data), `warp_fx`, `storage`, `career`, `player` (`design`, `behaviors`, `library`: what the player owns) |
| `crates/client/game` (the screens and `main.rs`) | `home`, `shipyard` (+ `gate`: the ring after a recall), `fleet`, `voyage_log` (the LOG), `settings`, `research`, `map`, `debrief` (the panel), `drydock` (`panels/`: `spawn`, `sync`; `blueprints/`: `autosave`, `title`, `screens`, `handlers`, `sync`), `behavior_ui`; `index.html` and `Trunk.toml` for the web build |

```
RunFixedMainLoop      flight, before the ticks: fleet bar clicks → controls (keys, mouse, touch → PlayerInput, latched
                      until the next tick)
FixedUpdate (30 Hz)   scene::scene_tick (pilot input, waves)  →  sim_bridge::step_sim  →  sim_bridge::apply_events
  (flight only)                                                   (Sim::step, timing,       (views: spawn / rebuild /
                                                                   hash log)                 despawn; effects from events)
Update                ui_kit (hit testing, text fields) → flight: camera input, HUD text and the home ring, fleet bar,
                      touch widgets, tuning panel (egui) | drydock: DockSet::Input →
                      Apply → Show → ui_kit styles | behavior: BehaviorSet::Input → Apply → Show → ui_kit styles;
                      the TEXT view's editor in egui's pass | home: clicks and keys (not while FLEET is open) →
                      the camera's flight → apply → labels → shipyard (layout, meshes, rigging; also in the
                      drydock) → FLEET's open, input, sync → ui_kit styles
PreUpdate             career::grant (Granted follows the career)
PostUpdate            ship_view::sync_piece_views (render_pose(alpha) → Transform, lit/dark)  →  shield::draw_bubbles
                      camera::update_camera  →  fx (plumes, trails, beams, tracers, sparks; lock ring, markers;
                      the drydock's frame overlay, the behavior graph's dot grid, home's hover brackets)  →
                      fx::flush;  ui_icons (after the
                      UI layout and the camera)
```

- **App mode** (`mode.rs`): `AppMode::{Home, Flight, Drydock, Behavior, Research}`, a Bevy state; the game starts at home
  ([Home](#home); `?scene=` picks another start, see URL parameters). Each mode's systems run
  only in it (`run_if(in_state(..))`), so input never crosses over. The switch happens in the state's enter / exit schedules: `OnExit(Flight)` empties the
  world (views, effects, bubbles, the lock go; the clock is unpaused), `OnEnter(Flight)` builds what another mode
  asked for (`Scene::launch`: `Launch::TestFlight` from the drydock or the BEHAVIOR screen, `Launch::Range` from
  home, `Launch::Voyage` from the gate), the drydock's and home's enter / exit set and restore the camera and the UI scale. Flight
  UI roots carry `mode::FlightUi` and are hidden when flight ends. A `SimWorld` always exists (an empty one
  outside flight and before Startup), because many flight systems read it. T in flight goes to `mode::FlightExit`
  (the drydock; a behavior test sets the screen it came from; reset whenever flight ends), M home.
- **Career** (`career.rs`): the player's career save, loaded before the first frame (`CareerPlugin`, after the
  storage and before the drydock), and what it grants the screens. See [Career](#career).
- **Voyage map** (`map/`): `AppMode::Map`, the star map of the voyages and SET COURSE. See [Voyage map](#voyage-map).
- **Voyages** (`voyage/`, `debrief.rs`): the `StartVoyage` message and the URL launch, the run (`VoyageRun`:
  FixedUpdate after the events), its end through the career, and the debrief over home. `scene.rs` builds the
  voyage's world (`SceneKind::Voyage`); `warp_fx.rs` draws its transitions (PostUpdate, after the piece views are
  posed). See [Voyages](#voyages).
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
- **Machines** (GDR-0069, 13 Oct 2026): a machine is its ship drawing (`icons/<key>.ship.svg`, atlas
  `ship/<key>` with its glow `ship-glow/<key>`) on a cell in the hull's tone, drawn by `ship_view::body_mesh`
  in this order: tiles (seams, fills), hull halo and line, machine glows, machines, muzzle glows, nav lights. The
  drawing fills the part's rotated footprint (`region_box` with `rot` and `flip`, as icons did) and, for an engine,
  reaches its overhang past the stern (`PartVisual::overhang`, from the drawing's viewBox through
  `data::content::svg::ship_overhang`), over the hull line; plumes start at the nozzle's mouth (`fx.rs`). The cell's tone
  (`ship_model::HullTones`, the pure rule `hull_tone`, tested): the armour tone with the most hull cell sides
  touching the machine (a tie goes to the tone the design uses more, then name order), else the design's most
  used tone by hull area, else steel. Computed once per design (`design_visual`), so a piece that breaks off
  keeps its tones; the drydock's ghost asks the design for the spot under the cursor. The same tiles show in
  flight, the drydock and its ghost, the home screen's ship and FLEET's ship icons; only the UI's part tiles
  (palette, inspector) keep the icon on the dark machine tile (`part_icons::part_tile`).
- **Events** (`apply_events`): `ShipSpawned` / `PieceChanged` / `Split` (re)build views, `VesselFormed` with a
  parent moves a split-off piece's view to its new ship, `PieceDestroyed` / `ShipRemoved` despawn them;
  `PartDestroyed`, `Explosion`, `ShotFired`, `Hit`, `Ricochet` become effects; `ShieldHit` starts a ripple in the
  bubble (and its shatter when it emptied it).
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
- **Speed widening** (`camera.rs`, GDR-0079, 14 Oct 2026, placeholders; Niklas: "there should also be some auto
  zoom based on speed"): while the camera follows a vessel in flight (the focus, or the watched ship), the view
  widens with its speed's share of its top speed (`FlightTelemetry::top_speed`, the HUD's `TOP`): linear, none at
  rest, up to 1.4 × wider at top speed (`SPEED_WIDEN_MAX`; zoom ÷ 1.4), eased in and out with a time constant of
  0.8 s (`SPEED_WIDEN_TAU`, real time, the same at any frame rate; unit-tested), so the player sees further ahead
  at speed and the view never jumps. It sits on top of the chosen zoom: a zoom by hand changes the base, the
  widening stays. With the lock framing the camera takes **whichever is further out** (not the product): the
  locked target stays exactly 90 px inside the edge when the framing needs more room, and the two never stack
  into a view wider than either asks for (the framing's 35 % floor stays a share of the chosen zoom). A free
  camera or the load test's frame eases back to none; the drydock, home, the map and the other screens drop it
  at once (they set their own zoom, and remember the chosen one).
- **Draw order**: every piece view draws on its own layer, `ship_view::draw_layer`: its ship's layer
  (`10 + ship id mod 80`) plus an eighth per piece id mod 8, so z 10..90, fixed for the ship's whole life
  (13 Oct 2026; before, every rebuild took the next layer of a round robin, so a ship that lost a part could pop
  under or over its neighbours). A rebuilt view keeps its layer; a split-off piece's layer follows from its ids;
  a piece that becomes a vessel of its own takes its new ship's layer (in the tick it splits off). Ship ids wrap
  after 80 ships. The focused vessel's piece draws at z 100, decided every frame from `PilotFocus`
  (`ship_view::sync_piece_views`), so a switch reorders at once. A lit piece's tiles are a separate opaque mesh
  that writes depth (see Shield bubbles); aura and barrels sit 1/64 below the tiles in depth, so they stay under
  their own tiles and over every lower piece.
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
- **Chain-reaction blasts** (`fx/blast.rs`, 14 Oct 2026; GDR-0084): `SimEvent::Explosion` plays a blast where the
  part is drawn (its centroid through the view's transform; the sim's `at` only without a view), carried with its
  piece's velocity, in the power colour (`#e2b15c`) with coral embers: a white-hot flash (0.15 s), a soft glow over
  the reach (0.32 s), a ring that eases out to exactly the blast's `radius` in 0.2 s so its reach reads, then fades
  by 0.45 s, and a few sparks (half gold, half coral). `damage` sets the strength (√(damage / 1.5), clamped
  0.55–1.4: a capacitor's or thruster's blast is a fainter, thinner ring, a fusion reactor's flash is bigger). A
  destroyed part that explodes plays no burst of its own (`PartDestroyed { volatile: true }`); the blast is it.
  **Chains stay readable**: blasts that arrive before the previous one went off (a chain resolves in one tick) go
  off one after the other, 40 ms apart (at most 8 steps), each flash and glow dimmed by 1/√(n + 1); at most 48
  blasts live at once. The ring is a crisp core line over soft dots along it (strip segments showed seams at
  small radii). Every number is in one const block, `fx::blast::BLAST`. No camera shake and no sound: the client
  has neither yet. Pictures (a Rim skiff idle in the test range under five raiders, slowed down in the headless
  browser by a `web-drive.mjs` `eval` step that makes `Performance.prototype.now` run at 3 % speed):
  [`results/chain-blast.png`](results/chain-blast.png) (a fission reactor's chain: its ring growing over
  the flash, a capacitor's at full reach, a thruster's starting; a thruster's ring fading on the right),
  [`results/chain-blast-later.png`](results/chain-blast-later.png) (a moment later).
- **HUD** (`flight_hud/`, Barlow from `assets/fonts`, OFL): the home ring top left (what M does, for touch; its
  clicks never reach the world: `HudButton`), hull %, hull map (one cell per part of the shown ship's design,
  rebuilt when the ship or its compiled design changes: every new sim numbers its ships from 0, so the id alone
  can't tell a test flight of an edited design from the last flight), the hold (`CARGO 3 / 8`: cargo units
  in use / the alive cargo parts of the controlling piece, `Ship::cargo` and `cargo_capacity`; empty for a ship
  without a hold), speed with the top speed of the
  best axis (`TOP 22`, from `ship.telemetry().top_speed`; the block ends at mid-height so it stays between the
  hull block and the touch stick on a phone), under it the **pilot assists** of the ship flown (`flight_hud/assists.rs`,
  GDR-0088; hidden without any): `ASSISTS`, then one line per assist with its toggle key, `F2 · BRAKE`, teal while it
  decides its channel, white while on, dim while off; `ASSISTS OFF · Z` in gold while ALL ASSISTS has them off
  (`docs/results/flight-assists-hud.png`: F3 just switched auto-fire on; face target keeps the guns on the raider off screen top right;
  `docs/results/flight-assists-hold-station.png`, step 4: hold station and face target deciding, avoid collisions on
  and waiting for something on a collision course, auto-fire off at start),
  target panel (in watch mode the watched ship's behavior target),
  wave line, banners (`PAUSED · P` while paused), warnings, help; lock ring and off-screen markers are world-space
  FX (the markers in watch mode too). A marker sits where the ray to its ship meets the view's edge, moved in along the ray until it is clear of
  every HUD text and widget (`HudZone`: hull map, fleet bar, home ring, touch widgets). HUD colours are the
  `ui.*` tokens. Debug overlay and toggles: `hud.rs`.
- **Tuning** (`tuning.rs`, bevy_egui 0.43.1): thrust / torque scale (separate or linked), speed scale (every
  engine's and RCS pod's rated speed, so every top speed; `SimConfig::speed_scale`, replaced the soft top speed
  factor on 12 Oct 2026), max turn rate, key turn rate (A/D held, `keyturn=`, GDR-0055) and the mind capacity (`mind=`) apply live
  (`Sim::set_config`); raiders per wave (follow the flown ship, the default, or a number by hand) apply to the
  next wave; drift at once; weapon range and damage scale
  rewrite the content the designs compile from, so they apply with "Apply & reset".
- **Smaller modules**: `params.rs` (launch parameters: the page URL's query, natively `GAME_PARAMS`;
  `Params::flag` / `f32` / `text`), `controls.rs` (keyboard, mouse and touch → `PlayerInput`: stick, A/D ramp and
  nudge, lock, fire, drift, V / Tab / Esc; the touch widgets), `backdrop.rs` (the radial gradient, two parallax dust
  layers drifting slowly outside flight (0.16 m/s in the far layer), a far layer of stars twinkling in three groups, dotted range rings around
  the followed ship whose dots keep at least 2.6 px across and 11 px apart at any zoom, rebuilt per zoom step), `meshbuf.rs` (`MeshBuf`, the CPU builder for
  atlas-textured, vertex-coloured 2D meshes; index order = draw order; their data lives only on the GPU, so a
  PostUpdate system that replaces one runs before `TransformSystems::Propagate`, where `atlas.rs` moves Bevy's mesh
  change announcements to: announced earlier, a mesh replaced two frames in a row lost its GPU buffer and vanished),
  `style.rs` (`Style`, the typed view of
  `art/tokens.json`, and `ui_color`).

## Home

The start screen ([GDR-0062](gdr/0062-first-playable-loop.md)) is the shipyard itself
([GDR-0071](gdr/0071-shipyard-home-long-u.md), 13 Oct 2026; it replaced the ring menu of track 7D): a top-down world
scene after `../space-game-archive/mockups/shipyard-long-u.png`, drawn by the game's camera. The game opens here (`/`,
`?scene=home`). The station is `shipyard/`, the screen over it and the camera's flights `home.rs`. Screenshots:
`docs/results/home-shipyard-1280.png`, `home-shipyard-1600.png` (1280 × 720 and 1600 × 900) and
`home-shipyard-to-drydock.png` (mid-flight into the drydock).

| Keys | |
|---|---|
| Drydock: the building area | D, or a click on the U |
| BEHAVIOR screen (the working design's behavior; its DRYDOCK pill and Esc lead on to the drydock): the mind | B, or a click on the mind |
| Test range: the sandbox with raider waves (a lost ship re-forms; M comes back, T opens the drydock as in any sandbox) | T, or a click on the tower or its lane |
| FLEET: the escorts ([FLEET](#fleet)) at the moorings; Esc, CLOSE or its ring button closes it | F, or a click on the moorings |
| LOG: the last voyages ([LOG](#log)) at the records mast; Esc, CLOSE or its ring button closes it | L, or a click on the mast |
| Voyages: the camera flies to the gate, then the voyage map opens ([Voyage map](#voyage-map)); coming back the camera flies out from the gate | V, or a click on the gate |
| Research: inert until its screen exists (track 7G) | R (nothing yet) |
| Dismiss the debrief after a voyage | Esc, Enter, or DISMISS |
| Research: the lab ([Research](#research); Esc, `‹ HOME` or its ring button come back) | R, or a click on the lab |
| Voyages: inert until its screen exists (tracks 7H/7I) | V (nothing yet) |
| Debug overlay | F8 |

- **The station** (`shipyard::layout`, a pure function of the frame; sizes are the sketch's at 20 px per cell,
  placeholders): a long U on its side, walls 3.1 m thick, its base on the left, its mouth to the right. **The inside
  of the U is the drydock frame of the working design**: blueprint `(x, y)` sits at world `(x, −y)` exactly where the
  drydock draws its grid, so the building area spans `0..w × −h..0` and the ship is docked where it was built (nose
  up). A bigger frame (cruiser 40 × 40) makes a bigger U and moves the buildings along; their sizes stay. The arms
  reach 3.5 m past the area, the gate's ring (7.5 m) stands 13 m beyond the tips with chevrons leading to it.
  Research (with two dishes and an antenna), the fabricator and the LOG's records mast (on the arm's tip, the end
  nearest the gate; GDR-0091) sit on the top wall, the test range's tower and its target lane and the salvage bay
  under the bottom wall, the mind behind the base (joined by violet conduits), four
  escort moorings on two truss bridges running off the left edge (two in view, two past it).
- **Detail** (`shipyard::draw`): plate runs in four tones with dark seams, a lattice truss along each outer face,
  three conduits (teal, gold, violet) along the inner faces, window rows (mostly lit or mostly dark by state), radiator fins,
  chamfered tips and a base strip, the name plate (`YARD 7 · RIM`, a label), red and green navigation lights on the
  arm tips, the base, the gate pylons and the moorings, antennas, the building area's floor with a dot at every cell
  corner, work lights pooling from the walls, gold corner brackets, a teal light strip with lamps around the inside.
  Three gantry cranes span the arms with carriages riding them and hoist lines to the ship; four docking clamps
  reach in from the arms; six drones circle the ship. The mind has cognition banks, heat sinks, coolant runs and a
  collar into the base; research a dome, server racks either side of its emblem and mounts for its dishes; the LOG a
  small house with windows, a lattice mast with two signal yards and teal lamps, and a teal beacon at its top. The
  bridges bend 45° into the mind's corner chamfers, the gate pylons are trusses from the arm tips to the gate's
  pods, the target lane hangs off its tower on a strut (GDR-0089).
- **Life** (`shipyard::life`, GDR-0089; Niklas, 15 Oct 2026: "very static"): beacons (the navigation lights
  breathe, the antenna tips flash; their glows are small meshes of their own, the white cores stay in the static
  meshes), the gate's dashes and small emitters turning (one turn in 120 s) and, while it is awake, a wave of light
  up its approach every 5 s (the fixed chevrons light one after another towards the gate, 0.11 s apart, each rising
  and falling in 0.45 s, so a wave takes about a second; Niklas, 15 Oct 2026: the moving arrows ran "way too
  fast"), the mind's spokes turning around a beating core, research's dishes sweeping and its orbit turning while
  research runs, and the LOG's mast glowing gold while a voyage's result is new (a soft glow and a thin ring at its
  top, breathing between 0.7 and 1.15 of its size every 2.8 s; GDR-0091). **Two shuttles** roam the station's left end on their own (Niklas, 15 Oct 2026: "roam more
  freely ... not teleport"): each flies between 13 stops clear of the hulls (`Layout::shuttle_stops`: berths off
  the four moorings' mouths, beside the mind, research and the salvage bay, and open points between the bridges,
  past the gaps between the moorings and beside the mind), picking the nearest of three random stops next; it turns
  its nose towards the stop (1.6 rad/s) and thrusts along it (the sharper the turn, the less; top speed 2.6 m/s,
  1.2 m/s²), brakes into a berth (planned at 0.7 m/s²) and waits there 2–6 s six times in ten, and passes through
  an open point at speed. Its place only ever moves by its velocity (a frame counts at most 0.05 s), so it never
  jumps. **Windows** (Niklas, 15 Oct 2026: "very slowly change which are active"): each building's or wall's
  windows (an arm, the base, a block, a cognition bank, a server rack, a mooring) keep 35–75 % on while it is lit
  and 0–12 % while it is dark, start at random inside that band, and switch about once in 20–60 s (drawn evenly):
  one window on, one off, or (where either would leave the band) one off as another comes on, each fading over
  4 s. Outside flight the backdrop's dust drifts slowly and its far stars twinkle (`backdrop.rs`).
- **Colours**: `station.*` in `art/tokens.json` (plate tones, structure, floor, nav lights) and the UI accents
  (`ui.*`); glows are the atlas's soft dot and halo strip, as the ships' (the glow toggle turns them off).
- **Cost**: the station is three static meshes, the yard around the U (z 1), the floor (z 2) and the walls (z 3),
  rebuilt only when the frame size, the career's lights or the glow setting change (a frigate yard is a few thousand
  quads, built once). The docked ship is one mesh rebuilt with the design's revision (as the drydock's), each escort
  one. What moves is small meshes moved only by their transforms: the cranes (z 12, over the ship), their
  carriages and hoist lines, the clamps (z 2.5, under the walls), the drones (z 14), life's parts (z 1.3, the
  shuttles z 13) and some twenty beacons (respawned with the static meshes). The approach's lit chevrons and the
  windows' lights (one small mesh per building or wall, 16 at home, over its dark panes, z +0.2) are drawn again only
  while one of them fades (`life::relight`): the wave about a second in five, a building's windows 4 s about once
  in 40 s; a rebuild keeps the lights of every building whose windows did not change. Labels and click areas are a
  few UI nodes written only when they change.
- **States** (`shipyard::lights`, from the career; defaults for review): **research** is lit (windows, the orbit
  emblem's glow, the dishes in gold) once anything has been researched or any node can be paid now
  (`Career::can_pay`); a new game has no research data, so it starts dark. **The gate** is awake (glow, inner light,
  teal approach path) while any voyage is open (`Career::open_voyages`); a new game opens its first voyage, so it is
  awake from the start, and only a career without voyages (no start) shows it dormant. **The moorings** hold FLEET's
  escorts (`EscortDesigns`; escort 1 top near, 2 bottom near, 3 and 4 past the edge), drawn as the drydock draws
  them, nose up, at most 0.45 × the drydock's scale and smaller when they would not fit (5 × 5.2 m); an occupied
  mooring's windows and inner edge light up. **The LOG** glows while the career has a voyage's result the LOG has not
  shown (`Career::unread`). **The fabricator and the salvage bay** stay dark (`LATER`) until they have screens.
- **Places and flights** (`shipyard::layout::Place`, `shipyard::fly`): a click or a tap on a place (an invisible UI
  node over it, hit-tested by `ui_kit` like any button, so touch works) or its key flies the camera there in 0.6 s
  (a smootherstep of the time since it began, so the same at any frame rate) around the flight's zoom anchor: the one
  world point that sits at the same place on screen in both views stays put while the zoom changes geometrically, as
  a zoom around the cursor does; without a change of zoom it pans. On arrival the place sends `HomeAction` and its
  screen opens. Hover highlights a working place (teal corner brackets, its title white). Inert places (the dark
  buildings) neither highlight nor react; the lab works dark or lit. **The gate** opens the voyage map on arrival
  ([Voyage map](#voyage-map)); the debrief's VOYAGE MAP (`map::OpenMap`) flies there as V does.
- **The drydock hand-over**: the camera flies onto the drydock's own opening view of the frame
  (`drydock::view::DockCamera::opening_view`: its last view of this frame, else the frame fitted into its grid
  viewport, computed from the drydock's laid-out but hidden UI), while the cranes slide to the mouth (parked past the
  frame inside the arms) and the clamps retract under the walls, and the yard around the U, the floor and the drones
  fade out with the flight (`shipyard::station_alpha`); then the drydock's panels slide in from their edges and its
  world fades in ([Screen transitions](#screen-transitions)). In the drydock the walls and the parked cranes stay
  around the frame (dimmed by its veil outside the grid viewport); the yard, the floor (the drydock draws its own
  dots, at the same corners) and the drones are home's. **Going back** reverses it: the drydock's panels slide out
  and its world fades, home's camera waits on the drydock's last view for that (`EXIT_SECONDS`, `Fly::after`) and then
  flies out while the cranes return and the yard fades back in. The other screens go back the same way from their
  place (the mind, the lab, the test range for any flight, the gate after a voyage while its debrief is open); when
  FLEET or the LOG closes the camera flies back out from the moorings or the mast once it has closed. Home's words and labels fade out
  when the camera leaves the yard and fade in when it rests there again; the labels follow the camera meanwhile
  (moved by their transforms), the invisible click areas come and go at once.
- **Labels** are small UI text over the world (titles 11 px, the line under them 8.5 px, at the 1280 × 720 layout),
  placed at their anchors from the camera and shown only while the camera rests on the yard: `BUILDING AREA ·
  FRIGATE 24 × 24` (gold, in the area), `BEHAVIOR` / `THE MIND` (violet), `TEST RANGE` / `NO RISK · NO
  HAUL` (coral), `FLEET` / `MOORINGS · CORES 2 / 2` (the fleet's logic cores against the mind, coral when over),
  `RESEARCH` / `2 READY` (gold while lit, else dim; `NOTHING READY` while lit with nothing to pay, `NO DATA YET`
  while dark), `VOYAGES` / `THE GATE · HALVEX DRIFT` (teal: the voyage the map will select; `DORMANT` while no
  voyage is open), `LOG` / `NO VOYAGES YET` (dim), `NEW · SHIPYARD APPROACH` or `2 NEW` (gold while a result is new),
  `3 VOYAGES` (grey), `FABRICATOR` and `SALVAGE BAY` / `LATER`. Top left `THE SHIPYARD · RIM` and the working design's name;
  bottom right the career's notes (a head and a line each; the two most serious, coral before gold, and how many
  more the console has): a new game and its start's summary, a save kept as it is, content that changed under the
  save, `?save=`, `?unlock=all`, an escort whose design is gone, a refused launch ([Career](#career)). After a voyage
  the debrief covers the right side until dismissed ([Voyages](#voyages)); the LOG keeps the result (its tooltip is
  the last voyage in one line). The place names are UI chrome in code until content has places.
- **The camera**: on the yard it shows the station's overview (from the near moorings to past the gate, 80 × 45 m
  for the frigate frame) and every place's label, all at least 18 px inside the window (`MARGIN`) at any aspect
  (`shipyard::layout::fit_tagged`, tested at 1280 × 720, 1600 × 900, 4:3, 5:4, 3440 × 1305 and 1080 × 1350; 13 Oct
  2026, Niklas: things at the left and top were cut off at some aspects): the labels keep their pixel size while the
  world scales, so each label edge bounds the zoom (`home::LabelTags` measures them from the laid-out labels). About
  15 px per metre at 1280 × 720, 19 at 1600 × 900. The UI scale is the drydock's (`ui_kit::ui_scale_for`), so the
  drydock's layout is the one the camera flies onto.
  Entering home keeps the flight camera's zoom; leaving gives it back with the UI scale 1, so the drydock and flight
  find the camera as they left it.
- **Modes**: the station and the screen are spawned once and shown only at home (the walls and cranes in the
  drydock too), and fade out while FLEET opens (a covering screen, [Screen transitions](#screen-transitions)) (it stays a layer for now; the moorings view can take its place
  later). **The layers over home** (FLEET, SETTINGS, the LOG) leave the station barely showing, so home's words,
  labels and click areas and the debrief's panel step aside while one is open (15 Oct 2026, Niklas: the summary
  line showed on top of SETTINGS), and home's places and keys wait. Coming back: the ring button top left of the drydock and the BEHAVIOR screen (`DrydockAction::Menu`,
  `BehaviorAction::Menu`), and in any flight M or the ring top left of the HUD.

## Career

The player's progression in the game (track 7F, 13 Oct 2026; GDR-0062, GDR-0063, ADR-0033), `career.rs`: the save,
a new game, the development switches and what the career grants the screens. The rules are the pure model
`data::career`; where the save lives, when it is written and how a broken one is kept: `docs/career.md` "Where the
game keeps the save".

- **`CareerPlugin`** (after `StoragePlugin`, before `DrydockPlugin`) loads `saves/<slot>` in its build, before the
  first frame (`career::load`): reconciled with the content, or a new game from the content's first start
  (written at once; the drydock then docks a copy of the start's ship, `rim_skiff`), or a stand-in when the save
  doesn't read (kept as it is, never written). Every problem is a note (`PlayerCareer::notes`: home's note block
  and the console), never a panic.
- **Resources**: `PlayerCareer` (the `Career`, the slot, the switches, the notes; `update(storage, f)` changes and
  writes it), `Granted` (the unlocks the drydock offers: the career's, or everything with `?unlock=all`; the fresh
  parts for the NEW tags; the mind capacity; recomputed in `PreUpdate` whenever the career changes) and
  `EscortDesigns` (the escorts' saved designs, re-read when home or the drydock opens; a gone one leaves the fleet
  with a note).
- **Messages**: `SeenPart` (the drydock showed a fresh part: it is fresh no more, the save is written).
- **Words**: `career::readiness_words` and `voyage_line` turn `data::career::voyage_ready` into the drydock's and
  FLEET's lines (`VOYAGE-READY · CORES 2 / 2`, `NOT VOYAGE-READY · USES BEAM LASER · +2`, `3 CORES, YOUR MIND RUNS
  2`, `NO CARGO · YOU CAN'T BRING SALVAGE HOME` as a gold warning); `unlocked_by` names the research node that
  unlocks a locked part.
- **Switches** (URL parameters): `?save=<slot>`, `?newgame=1`, `?unlock=all`. Test flights, the test range and TEST
  IN RANGE never touch the save; a voyage writes it at launch and at its end, a practice run never
  ([Voyages](#voyages)).

## Voyages

Track 7H of the first playable loop, 14 Oct 2026 (GDR-0062, GDR-0065, GDR-0068, ADR-0033; the sim's side is
`docs/sim.md` "Voyages", the career's `docs/career.md` "Voyages"). Code: `voyage/mod.rs` (launch, run, end),
`scene.rs` (`SceneKind::Voyage`: the world), `flight_hud/` (the voyage HUD), `voyage/debrief.rs` and `debrief.rs` (the debrief's data and its panel).
Screenshots (14 Oct 2026, headless 1600 × 900): `docs/results/voyage-hud.png` (the start: sites, cargo, RECALL),
`voyage-recall-blocked.png` (after a fight: salvage in the hold, RECALL dimmed), `voyage-warp.png` (before the
transitions), `voyage-warp-out-1.png` (the fleet charging), `voyage-warp-out-2.png` (the jump: stretched, darting
off, the flash and its ring where it left), `voyage-warp-emergency.png` (the emergency recall mid-jump, amber),
`voyage-warp-in.png` (the arrival: streaking in from behind towards its place),
`voyage-abandon-panel.png` (M: the ABANDON VOYAGE? panel), `voyage-abandoned.png` (the lost banner after abandoning), `voyage-debrief.png`, `voyage-debrief-lost.png`,
`voyage-summary.png` (home after DISMISS).

- **Launch** (`voyage::StartVoyage { voyage }`, sent by the voyage map's SET COURSE with the selected voyage; `None`
  is the first open voyage whose goal is not cleared): the fleet must be voyage-ready (`career::voyage_ready` against what the career grants: unlocked parts,
  materials and frame, cores within the mind) and every ship flight-ready (the design report); a refusal is a coral
  note on home (`CAN'T LAUNCH`) and a `voyage::LaunchRefused(why)` the map shows. The career records the launch (`Career::voyage_launched`: the seed, `underway`)
  and the save is written at once, so closing or reloading mid-voyage loses the voyage (GDR-0062).
  `?scene=voyage&voyage=<id>` launches at once for development; what the career refuses (a voyage not open yet, a
  fleet not ready by the career's own unlocks, so also anything `?unlock=all` alone allows) flies as a **practice
  run**: rolled from the seed the next launch would get, `PRACTICE, NOT SAVED` in the subtitle, never written.
- **The world** (`scene::build_voyage`): `SimConfig::mind_capacity` of the player is the career's mind capacity
  (the tuning panel's other values apply as in the sandbox), the sim is seeded with the voyage's seed, the plan is
  rolled from the content the enemies fly with (`VoyagePlan::roll(others, voyage, seed)`, the tuning's damage scale
  is the player's only) and started (`Sim::start_voyage(plan, PLAYER, ENEMY)`). The flagship (the drydock's working
  design) enters at the plan's entry as the focus; the FLEET escorts take `?fleet=` wingman slots behind it (turned
  with the entry), fly the behavior their design names and follow the flagship (`Leader`). No raider waves (C does
  nothing), no re-form, no wreck clean-up; Shift+R and T do nothing, M asks in a panel whether to abandon the voyage.
- **The HUD** (after `../space-game-archive/mockups/voyage-hud-b.png`): the title `SHIPYARD APPROACH · RIM` (name and band),
  the subtitle `SITES 1 / 4 · 2:05` (sites cleared, and wreck-only sites once they spawned, of all; the sim time);
  under the hull the cargo block: `SALVAGE`, the shown ship's `5 / 8` in gold with one pip per cargo unit (20 at
  most, then shares), the name of its biggest cargo part (`CARGO HOLD`); right under it the RECALL pill (its tooltip names R) and
  what it does: `WARP HOME 3 S · KEEPS THE HAUL` (teal) while `Sim::can_recall(PLAYER)` allows it, else dimmed with
  `ARMED ENEMIES NEAR · NO RECALL`; when no vessel of the fleet has a working gun (`Sim::fleet_disarmed`, GDR-0074)
  the recall is always allowed and the line says `EMERGENCY RECALL · NO WORKING GUN · WARP HOME 3 S`. Sites off screen get a marker at the screen edge (the enemy markers' code: an
  arrow kept clear of the HUD) with the encounter's name and the distance from the focus: raiders coral, wrecks
  only gold, the goal brighter (`· GOAL`, a glow), cleared ones dim (`· CLEARED`); a sleeping site's ships get no
  enemy marker of their own. Banners (with a line under them): `RAIDER PAIR · AWAKE` (coral), `SITE CLEARED ·
  RAIDER PACK` / `SITES 2 / 4 · THE WRECKS ARE YOURS` (teal), `GOAL CLEARED · RAIDER PACK` / `… · RECALL WHEN CLEAR
  · R` (gold). The home ring top left is hidden. The help line names R RECALL and M ABANDON.
- **Recall**: R or the pill asks; the next tick starts the 3 s warp-out only if the sim allows it (else `NO
  RECALL · ARMED ENEMIES WITHIN 200 M`; a disarmed fleet always may, and the banner says `EMERGENCY WARP`). The warp-out can't be broken off (`WARPING HOME · 2`, for the emergency recall `EMERGENCY WARP · 2` in gold); what it looks like: **Transitions** below; losing every vessel
  during it still loses the voyage. At its end the haul is every player vessel's cargo (dormant and dark ones
  too, `voyage::fleet_haul`; sim items become `data::career::HaulItem`s one to one), the career unloads it
  (`VoyageEnd::Recalled { haul, cleared: goal cleared, ticks }`), the save is written, home opens with the debrief.
- **Lost**: when the player has no vessel left, `SHIP LOST` / `RECALLED HOME` / `THE HAUL STAYS IN THE WRECK · 5
  PARTS LOST · YOUR SHIPS RE-FORM AT HOME` with a 3 s countdown (`voyage-hud-lost.png`), the cargo block says
  `LEFT IN THE WRECK`; then `VoyageEnd::Lost { ticks }`, the save, home. **Abandoning** (GDR-0077): M opens the ABANDON
  VOYAGE? panel (`flight_hud/`; centred low over the help line, so the ship stays in sight; it slides up from
  the bottom and back): `THE HAUL IS LOST · 5 PARTS STAY BEHIND` (every hold's items), `The voyage counts as lost.
  Your ships re-form at home.`, `THE FLIGHT GOES ON WHILE YOU DECIDE`, and the pills ABANDON VOYAGE (coral, tooltip
  `ENTER`) and KEEP FLYING (tooltip `ESC / M`). The sim keeps running while it is open (a real-time sim, co-op
  later; P still pauses), the keys still fly and fire; clicks on the panel are its own (`HudButton`), and an Esc
  that closes it keeps the lock. ABANDON VOYAGE or Enter goes the same way as losing every vessel (`VOYAGE
  ABANDONED`, every hold's items lost), as closing the game would: the way out when a dark ship is stuck near an
  armed enemy. The panel closes by itself when the voyage is lost another way. Leaving flight
  any other way while a recorded voyage runs ends it as lost too (a guard; nothing in the game does it).
- **Transitions** (`warp_fx.rs`, [GDR-0087](gdr/0087-voyage-transitions.md); timings and looks are placeholders
  in its tuning block). Client visuals only, read from the sim's tick (plus the fixed step's
  overstep) and the voyage's phase: they pause with P, look the same at any frame rate and never touch the sim.
  - **Warp-out** (the 3 s of `Phase::Warping`): each vessel charges for 2.25 s: a soft glow under it, a ring around
    it that tightens and brightens, a dashed ring inside that spins up, faint streaks drawn in towards it. Then it
    stretches along its heading (5 × long, half as wide) and darts ahead out of sight in 0.3 s; a flash and a ring
    stay where it left, a streak along its way fades. The escorts jump a moment before the flagship (0.06 s each, at
    most 0.24 s). The camera eases in by 12 % over the charge and holds (`CameraRig::warp_zoom`, on top of the
    chosen zoom like the speed widening). The banner and the countdown stay. The emergency recall (no working gun,
    GDR-0074) charges unstable: its light amber (gold into coral), its rings gold, its streaks coral, all flickering,
    the rings and the jump shaking. The vessels are stretched and moved through their piece views' transforms after
    `ship_view::sync_piece_views` posed them, so their shield bubbles follow; the light is FX sprites (the atlas's
    glow dot and halo strip). Engine plumes stay with the sim's pose: they show for a moment under the flash.
  - **Arrival** (a voyage's first 1.8 s, counted from its first frame so the hitch of building the world doesn't
    eat it): the reverse. The world comes up out of the dark (0.45 s) while the HUD slides in; each vessel streaks
    in from behind, shrinks onto its place with a flash and a ring, and its glow fades; the camera eases back out
    from 12 % in.
  - **The hand-over** (every end: recalled, lost, abandoned; and a lost test flight going back to the drydock or the
    BEHAVIOR screen): over the last 0.36 s the HUD closes with its screen transition (its world markers, the lock
    ring and the enemy and site arrows, fade with it) and a veil over all the UI dips to the backdrop's darkest
    colour (`#010409`); the frame where the mode changes holds it, so the cut is never seen. The next screen comes up
    out of it in 0.5 s (real time). After a recall a ring of light spreads from home's gate while home's camera flies
    out of it. A loss (`SHIP LOST`, `VOYAGE ABANDONED`, a lost test flight) first dims the world behind its banner to
    40 % with a veil under the UI.
  - Checked (14 Oct 2026, headless 1600 × 900 with `?slowmo=`): the frames above, the abandon flow (the dim behind
    the banner, the dip, home coming up out of it at the gate) and the gate's ring at home.
- **Counts** for the debrief: kills are the enemy's `VesselLost` events (a split-off half counts on its own), ships
  lost the player's.
- **The debrief** (`debrief.rs`, its data `voyage/debrief.rs`, after `home-b.png`): after a voyage, a panel over home's right side (420 px, on a
  veil, above home's layout and independent of it; hidden while FLEET, SETTINGS or the LOG is open, whose Esc is
  theirs) until DISMISS, Esc or Enter: `VOYAGE
  HOME · HAUL UNLOADED` (or `SHIP LOST · RECALLED HOME`, `PRACTICE RUN · NOT SAVED`), the voyage's name, chips for
  the band, the outcome (`GOAL CLEARED`, `RECALLED`, `LOST`) and `FIRST CLEAR`; time, sites, kills, ships lost;
  `14 PARTS DISMANTLED` with a row per part (fragments apart, most first, six at most) and what it gave; research
  data per family with what is now in store; the materials; `RESEARCH READY · n` with the nodes the stores pay for
  now; `SIGNAL FOUND · HALVEX DRIFT` after a vector beacon glyph for each voyage it opened (GDR-0078: the name from
  the content; Barlow has no ◆ either, the materials' diamond is a vector glyph too); the voyage's `cleared` words at
  the first clear. The report scrolls when it is longer than the panel (14 Oct 2026; Niklas: "the voyage report is
  not scrollable, and thus can cut things off"): everything above the buttons is one area with the kit's
  `Scrollbar` (smooth wheel, overlay bar),
  and VOYAGE MAP and DISMISS stay at the bottom (`docs/results/debrief-scroll.png`: 1280 × 600 with a padded
  report). Once dismissed the result stays in the [LOG](#log) (GDR-0091; until 15 Oct 2026 a one-line summary sat
  over home instead). The panel is not saved: a game loaded after a voyage shows none.

## Voyage map

Where voyages are chosen (track 7I, 14 Oct 2026; GDR-0065's star map inside the rim band, after
`../space-game-archive/mockups/map-b.png`; GDR-0071: the gate is where you fly for voyages; GDR-0078: no route, the voyages are
found one signal at a time), `map/`: `AppMode::Map`. Home's gate (V, or a click) flies the camera to the gate, then
the map opens; Esc or the ring button top left goes back to the shipyard (the camera flies out from the gate);
`?scene=map` starts here. Screenshots: `docs/results/map-signals-new-game.png` (a new game: the shipyard and the
first voyage, nothing else), `map-signals-found.png` (the first voyage cleared: Halvex Drift a new signal). The older
`map-1280.png`, `map-1600.png`, `map-1600-locked.png`, `map-wide.png` (3440 × 1305) and `map-tall.png` (900 × 1400)
show the window sizes but still the chain and the locked voyages the map drew before GDR-0078.

| Keys | |
|---|---|
| Select a voyage | a click or a tap on its marker or its label; ← → (↑ ↓) step through the known ones |
| Launch the selected voyage | Enter, or SET COURSE |
| Back to the shipyard | Esc, or the ring button top left |

- **Layout** (`map::layout`, pure; placeholders): a stage of 1280 × 720 logical px, scaled to the largest size that
  fits the window whole (1 at 1280 × 720, 1.25 at 1600 × 900, any aspect: centred, nothing cut off; 0.3 to 2). The
  map fills the stage left of the panel (904 px); content's `map` coordinates (0 to 1) land in its rect
  `(28, 64)–(876, 650)`. The shipyard sits at `(0.12, 0.86)` (bottom left; content has no place for home yet). No
  route joins the voyages or the shipyard (GDR-0078). **The band**: the galaxy's centre lies off the stage to the
  right (`(1500, 400)`), so the band edges are arcs; the rim band's outer edge (the galaxy's edge) lies 34 px outside
  the place farthest from the centre, its inner edge (the gate to the next band) 34 px inside the nearest, the next
  band's edge as far again in at 0.6 of the depth (placed from every voyage, known or not, so the edges don't move as
  voyages are found). The band names come from the voyages' `band` key and the order rim, outer, middle, inner, core.
- **States** (from the career, `map::layout::VoyageState`): **unknown** (not open yet: not drawn at all, no
  marker, label, beacon or click area, not counted), a **new signal** (open because the voyages it `opens_after` were
  cleared, and never launched: the open ring and dot with two teal rings pulsing out of it every 1.6 s and a glow,
  `SIGNAL` in a teal outlined tag; its first launch, the career's launch count, makes it open, nothing else is
  saved), **open** (a teal ring with a glow, `OPEN`), **cleared** (a filled teal dot, `CLEARED · BEST 7:42`: the
  quickest voyage that cleared the goal and came home; it may launch again), and a **guardian** (a voyage whose goal
  encounter is tagged `boss`: a coral diamond, `OPEN · GUARDIAN`, a signal's pulses coral). Under a voyage's name a
  dot per site (coral enemies, gold wrecks, a diamond for the goal). The selection is a dotted ring; hover a faint
  ring and a brighter name. The legend bottom left (home, open, cleared, signal; enemies, wrecks, goal site,
  guardian); top left `RIM · 2 VOYAGES · 1 CLEARED · 1 SIGNAL` (the known voyages) and "Voyages".
- **The panel** (the selected voyage): `DESTINATION · VOYAGE 2` (no total: the rest is unknown), the name, chips (the band; the factions of its
  ships, coral; `WRECKS` gold when any site holds wrecks; `GUARDIAN`), the summary, `SITES · 5` with a row per site
  entry (`Raider pack ×2`, right `3 SHIPS` / `WRECKS` / `GOAL · 4–5 SHIPS` / `GUARDIAN`, under it what it gives: the
  three best non-hull parts of its wrecks by salvage value, or the ships waiting there), `≈ 10–15 MIN · RECALL WHEN
  CLEAR`; then FLEET with `CORES 2 / 2`, the flagship (the drydock's working design) and the escorts drawn as ships
  (`ui_icons::UiIcon::Ship`) with their cores and cargo, or `NO ESCORT · YOUR MIND RUNS 2 CORES`; the voyage-ready
  checklist (`VOYAGE-READY` or `NOT VOYAGE-READY`, four lines with a filled dot when they pass: `FLIGHT-READY`,
  `ONLY UNLOCKED PARTS` or the first locked part, `CORES n / m`, `CARGO 8` or `NO CARGO` as a gold warning;
  `data::career::voyage_ready` and the design report); the status line (`A NEW SIGNAL · THE GATE IS OPEN` for a
  signal); SET COURSE.
- **SET COURSE** (`map::layout::course_blocker`): dimmed while the selected voyage can't launch, the status line says
  why (in order: another voyage still underway, `LOCKED · CLEAR HALVEX DRIFT FIRST` (a guard: a voyage not open
  can't be selected), `NOT VOYAGE-READY · <the fleet's
  first blocker>`, `voyage::fleet_blocker`); a click then repeats it. Else it sends `voyage::StartVoyage { voyage:
  Some(id) }`; a launch `voyage/mod.rs` still refuses shows its reason. With `?unlock=all` a fleet the career's own unlocks
  don't allow flies as a practice run (`PRACTICE RUN, NOT SAVED · …` in gold). Entering the map selects the first
  open voyage whose goal is not cleared (after a first clear: the signal it found).
- **Zoom and pan** (`map::view`, Niklas 14 Oct 2026: "zoom in/out (even see the whole galaxy)"): the wheel or a
  trackpad pinch zooms around the cursor, + / − around the map's centre, 0 or a double click on empty space resets;
  drag empty space (left button), or anywhere on the map with the middle or right button, to pan; on touch two
  fingers pinch and pan, one finger on empty space pans. Bottom right of the map: −, the zoom in %, +, reset (their
  tooltips name the keys). From 200 % out to the whole galaxy fitting the map (about 20 %); entering always starts
  at 100 %, the map as it was. The zoom eases (0.1 s) around its anchor, which stays under the cursor all the way;
  a reset glides back along the same kind of path. The pan keeps the galaxy over the map. The panel, the header and
  the legend stay fixed. **Level of detail**: the voyages' labels and markers fade out from about 55 % down
  to 30 % (marker and label sizes stay in screen px at any zoom), the galaxy's band names (`RIM · CHARTED`, `OUTER ·
  UNCHARTED`, ...), the charted band, the known voyages' beacons and the shipyard's diamond fade in from 70 % down
  to 40 %.
  Zoomed out, a click on a voyage's beacon selects it and zooms in to it; ←/→ bring a voyage off the view into it.
- **Drawing** (`map::draw`, `map::galaxy`): the star map is four atlas meshes (z 550 and a little above: over the
  world, under the UI icons), placed over the stage node through the camera rig as `ui_icons` places icons, so they
  scale with the stage, each with its own material for its fade. Bottom to top: **the galaxy** (built once, in the
  default view's stage px, moved and scaled with the view): haze along four spiral arms and a warm core, fog over
  the uncharted bands under a faint coral tint inside the gate, seeded star dust with a few faint constellation
  lines in the charted band and a sparse halo past the rim, every band edge as a dotted ring (where the bands past
  the next one end is a placeholder); **the far layer** (the same frame; rebuilt when the states change): the
  charted band lit, a beacon per known voyage (a ring around a signal's; no routes), the shipyard's diamond; **the
  detail** (rebuilt when the states, the selection, the hover, the glow setting, the view or, while a signal shows,
  its pulse step (32 per pulse) change, a few hundred vertices): the known voyages' markers and the shipyard, faded
  where they reach the panel; **the chrome** (fixed): the legend's symbols, the panel's
  backing (it runs on past the stage, so wide and tall windows are covered). Everything else is `ui_kit` text and
  nodes, hit-tested by hand, so touch works; the map's labels move with the view (`UiTransform`) and are cut at the
  panel.
- **The debrief** after a voyage that found a signal says `SIGNAL FOUND · <its name>` and adds VOYAGE MAP beside
  DISMISS: it dismisses the panel and home flies to the gate (`map::OpenMap`), where the map selects the signal.

## FLEET

The fleet on the dock ([GDR-0065](gdr/0065-phase-7-screens-and-escort-action.md), after
`../space-game-archive/mockups/fleet-b.png`), `fleet.rs`, a layer over home (F or the moorings, once the camera has flown
there). Screenshots:
`docs/results/career-fleet*.png`.

- **The layer**: top left the ring button (closes it), `SHIPYARD › FLEET` and "Fleet", and the rule in words
  (escorts fly the behavior their design names and follow the flagship's lock); top right `CORES 3 / 4` with one dot
  per core of the mind (filled for the fleet's, coral past the capacity) and CLOSE (Esc). In the middle the flagship
  (the drydock's working design) in a bracketed box, under it its name, cores and cargo and the fleet's voyage-ready
  line. Around it the escort slots (`MAX_ESCORTS` = 4 places: left and right, upper and lower): each filled slot
  shows its design, `ESCORT n`, name, cores and cargo, and REMOVE; one empty slot more while there is room says how
  many cores are free for it. At the bottom `YOUR DESIGNS · PICK ONE FOR ESCORT n` and four cards of the saved designs
  (most recently saved first; the row scrolls sideways by pixels, `N OF M` says how far; a pool of 12 card nodes, so
  scrolling rarely gives a card node another design and builds its ship again): the ship, name, size, cores, cargo, and what a pick would do
  (`FITS · 1 CORE` teal, `IN THE FLEET · 1 CORE`, `USES BEAM LASER` gold: it may fly but the fleet is not voyage-ready,
  `NEEDS 2 CORES · 1 FREE`, `CAN'T FLY · NO THRUST`, `NO LOGIC CORE` coral). The last line says what happened.
- **Picking** (`fleet::pick`): a click on a slot picks it (teal outline); a click on a card puts that design into the
  picked slot (the empty slot adds an escort) when the mind runs the fleet then: the flagship's cores, the other
  escorts' and the design's together within the capacity (`Granted::mind_capacity`); a design without a logic core
  or that can't fly is refused too, with the reason. A design may fly in more than one slot. Every change writes the
  save (`escorts`); the slot then moves on to the next empty one.
- **Ships** are `ui_icons::UiIcon::Ship`: the design drawn as the drydock draws it, fitted into the node.
- Not yet: the mockup's preview of a picked card before ENTER adds it (one click adds), the dotted formation ring
  and the lines to the flagship, the escort's behavior name.

## LOG

The last voyages ([GDR-0091](gdr/0091-log-building-and-voyage-history.md)), `voyage_log.rs`, a layer over home (L or
the records mast by the gate, once the camera has flown there).

- **The layer**: top left the ring button (closes it), `SHIPYARD › LOG` and "Log"; top right the career's counts
  (`VOYAGES`, `HOME`, `LOST`, `SALVAGED`: `Career::log`, every voyage ever) and CLOSE (Esc). The list (it scrolls)
  starts with `THE LAST n VOYAGES · NEWEST FIRST`, then one row per voyage of the career's history
  (`Career::history`, the last 50): `NEW` (gold) for a result not shown before, the voyage's name, the outcome
  (`GOAL CLEARED` teal, `RECALLED`, `LOST` coral), the time, and what came home (`7 PARTS » HULL AND ARMOUR +1 ·
  PROGRAMMABLE MATTER +6`, or `THE HAUL STAYED BEHIND` in coral). Without a voyage: `NO VOYAGES YET` and a line about
  what it will hold.
- **Seen**: opening it marks every result seen (`Career::read_log`) and writes the save; the mast stops glowing.
  The rows are built when it opens.
- **The mast's tooltip** is the last voyage in one line (`LogRow::line`: `LAST VOYAGE · THE WARDEN'S GATE · GOAL
  CLEARED · 8:15 · 7 PARTS » HULL AND ARMOUR +1 · …`) with its key; `LOG` before the first voyage.
- A practice run is not in the history (it is not saved); kills and ships lost are not either (the debrief has them).

## Screen transitions

Niklas, 13 Oct 2026: "the stuff on the left and right should slide in … the whole transition should feel really
smooth, nothing should be hidden abruptly". `ui_kit::transition` (re-exported from `ui_kit`) is the one helper every
screen uses; new screens adopt it the same way:

1. Put `ScreenFx::closed()` on the screen's UI root (`.covering()` when the screen hides the world behind it: the
   shipyard fades out under it, `WorldCover`), spawned `Visibility::Hidden`; from then on the kit owns the root's
   visibility.
2. Put `ScreenPart::new(edge, order)` on each panel or bar: `Edge::Left/Right/Top/Bottom` slides it in from beyond
   that window edge, `Edge::Fade` fades it in place (use it for anything other code positions from its rectangle:
   the drydock's grid viewport, the BEHAVIOR screen's graph area). `order` 0–3 staggers them. Everything not inside
   a part fades with the root. A root may be a part itself (the debrief panel slides in from the right).
3. Open and close it with `show_screen(&mut fx, open)` (e.g. from the mode in a system on `state_changed`);
   `snap_screen` jumps without a transition where the UI scale or the world change at once (to and from flight).
4. World-space elements read `ScreenFx::alpha()` (FX colours × alpha, or a group material: `Atlas::group_material`
   and `atlas::set_alpha`) and draw while `ScreenFx::visible()`, so they fade out after the mode has been left.

How: the root's progress runs to 1 in `ENTER_SECONDS` (0.53 s: each part slides 0.38 s, eased out, 0.05 s after the
order before) and back to 0 in `EXIT_SECONDS` (0.32 s, the last part in going first); defaults for review. A part
moves by its `UiTransform` from fully past its edge (no layout); colours of texts, backgrounds, borders and glows
are written × alpha while it runs and given back exactly when it ends (a colour a screen writes meanwhile becomes
the faded one, per colour); UI icons follow their node's place and draw with their part's own atlas material whose
alpha is the part's (`IconMaterials`); `AtlasMaterial` has a `tint` uniform for this (alpha only). Nothing is
written once a transition has ended. Tested: the timing at 30–144 fps, the stagger, the slide distances, the colour
hand-back. There is no reduced-motion setting in the game yet; when one comes, `snap_screen` is the switch.

Screenshots (headless 1280 × 720, 13 Oct 2026): `docs/results/ui-drydock-enter-1.png` … `-4.png` (home's labels
fading as the camera flies in; arrived; the panels sliding in and the frame's dots fading in; nearly in place),
`ui-drydock-exit-1.png`, `-2.png` (the panels sliding out; the camera waiting before it flies out),
`ui-fleet-enter.png` (the station fading under FLEET); tooltips `ui-tooltip-drydock.png`, `ui-tooltip-home.png`; the
fits `ui-fit-*.png` (3440 × 1305 and 1080 × 1350); `ui-select-footprint.png`, `ui-erase-footprint.png` (a 2 × 2
reactor marked whole), `ui-zoom-rings.png`.

Who uses it: the drydock (top bars from the top, the palette from the left, stats and inspector from the right, the
tools from the top and the info line and bottom bar from the bottom; the frame's dots, brackets, veil, the pointer's
mark, the ghost and the overlays fade; the design fades too unless it comes from or goes home, where the docked ship
is the same picture; going to the BEHAVIOR screen the camera stays on the drydock's view, `DockCamera::parked`, so
the design fades in place), the BEHAVIOR screen (dimming; its dot grid fades), the research screen (dimming; its four
roots), the voyage map (covering: the panel from the right, the map and its drawing fade; SET COURSE
snaps it), FLEET (dimming), the debrief, home's words and labels, and the flight HUD (slides in when flight
starts; H hides it the same way; leaving flight snaps it shut). The station fades as a whole under covering screens
(`shipyard::station_alpha`, two group materials: the U with its rigging and the docked ship, and what is home's
alone). A dimming screen (BEHAVIOR, RESEARCH, FLEET) leaves `ui_kit::BEHIND` (3 %) of the station showing and
veils the space backdrop to its darkest colour (`backdrop::VeilsBackdrop`), so the three share one ground. Not faded yet: the BEHAVIOR screen's dashed outlines (a UI material), the BLUEPRINTS / BEHAVIORS layers
(hidden at once when their screen is left), the TEXT view's egui editor, and the flight world itself (a scene cut),
except where a voyage ends or a lost test flight goes back: there the HUD closes and the window dips to dark and
comes up again on the next screen ([Voyages](#voyages), `warp_fx.rs`).

## Research

The research building's screen (track 7G, 13 Oct 2026; [GDR-0063](gdr/0063-salvage-and-research-v1.md), the tree with
lanes of [GDR-0065](gdr/0065-phase-7-screens-and-escort-action.md) after `../space-game-archive/mockups/research-b.png`),
`research/`: research data from salvage becomes parts, materials, frames and mind capacity. Screenshots:
`docs/results/research-1280.png`, `research-1600.png` (1280 × 720 and 1600 × 900), `research-unlocked.png` (after an
UNLOCK), `research-drydock-new.png` (the part NEW in the drydock afterwards), `research-from-locked-row.png` (opened
from the drydock's LOCKED fusion reactor row), `research-flight.png` (home's camera on its way to the lab).

| Keys | |
|---|---|
| Show a node's detail | click its card |
| Research it | UNLOCK (enabled while the node can be paid now) |
| Back to the shipyard (home flies out from the lab) | Esc, `‹ HOME`, or the ring button top left |
| The drydock | D, or `DRYDOCK` |

- **Getting there**: home's lab (R, or a click on it; it works dark or lit) flies the camera onto the lab, then the
  screen opens over it: the station stays drawn (`shipyard::show`), all but 3 % hidden (`ui_kit::BEHIND`), on the
  veiled backdrop BEHAVIOR and FLEET share (`backdrop::VeilsBackdrop`: a quad in the gradient's outer colour at
  92 %, over the backdrop and under the station), and the camera rests exactly
  where home's flight ended. A LOCKED row of the drydock's palette or a locked armour pill
  (`DrydockAction::Research(node)`, `drydock::actions::locked_research`) opens the screen on the node that unlocks
  it (no camera flight from the drydock: the screen is just there). `?scene=research` starts here.
- **The tree** (`research::tree`, pure and tested): one lane per research family that has nodes, in content order
  (families without nodes show their data top right as `ALSO IN STORE`); a node's column is its requirement depth
  (0 without requirements, else one past its deepest requirement, in any family); nodes of one lane that share a
  column stack in rows. Connectors run from a requirement's right edge to the dependent's left edge, straight on one
  row, with an elbow halfway across the gap between rows; solid teal once the requirement is researched, dashed
  grey before. `Tree::new` is a function of the content alone, so the screen is built once at start;
  `Tree::place` puts it on the 1280 × 720 layout (cards up to 210 × 40 px, columns 250 px apart, lane rows 66 to
  96 px; more columns make narrower cards, more lanes than fit scroll with the wheel).
- **Cards**: the unlock's icon (the part as a flight tile; the armour block in an unlocked material; the formation
  for a frame; the mind's ring for mind capacity), the name, and a mark: a gold diamond while it can be paid
  (**ready**, gold outline and glow), `DONE` once researched (teal outline), a lock instead of the icon while
  its requirements are missing (**locked**, dim), nothing while the stores fall short (grey). Under the card its kind
  (`NEW PART`, `SIZE CLASS`, `REFINEMENT`, ...) and its cost chips: `ENERGY 18` in gold when the store pays it,
  `HULL 14 / 20` (held / cost) in grey when not, materials after a diamond. Data shows rounded down to a tenth, so a
  store never looks like it pays a cost it doesn't. The selected card is lit; hover brightens a card.
- **Top**: the ring button and `SHIPYARD › RESEARCH`; right, `ALSO IN STORE` for families without a lane and every
  material a node costs (`◆ PROGRAMMABLE MATTER 2`). Each lane's head shows its family and the data in store.
- **Detail** (right): kind and family, the node's name and summary, a preview of what it unlocks (a part: its
  icon in brackets and its content stat lines and bonuses, as the drydock's palette part; a material: the armour
  block in it; a frame: its grid; mind capacity: the mind before and after), COST (chips) against IN STORE, UNLOCKS
  in words (`Beam laser in the drydock, unlimited`, `One more logic core in the mind (runs 3)`), REQUIRES with each
  requirement done or not yet. Over UNLOCK one line: the last result or refusal, else the node's state
  (`READY · UNLOCK PAYS AND UNLOCKS AT ONCE`, `NEEDS TAMED AUTOMATON CORE FIRST`, `NOT ENOUGH DATA · HULL 14 /
  20`). UNLOCK names the data cost and shows the materials after a diamond; `RESEARCHED` once done.
- **UNLOCK** (`research::apply`, the message `ResearchAction::{Select, Unlock, Home, Drydock}`): asks the model
  (`Career::can_pay`) first; a refusal is said in coral and nothing is written. Else `Career::research` pays and
  unlocks at once and the save is written (`PlayerCareer::update`); the line says `BEAM LASER · NOW IN THE DRYDOCK`
  (or `MIND EXPANSION I · YOUR MIND RUNS 3 CORES`). A failed write keeps the unlock for this game and says why.
  `Granted` follows in the next `PreUpdate`: the drydock offers the part with a NEW tag (the career's seen-set),
  the lab lights up at home, home's label counts what is ready.
- **Bottom**: `‹ HOME`, the ready line (`2 READY · BEAM LASER · MIND EXPANSION I`, or `NOTHING READY · SALVAGE
  WRECKS FOR RESEARCH DATA`), `DRYDOCK`. The labels carry no key letters (Niklas, 13 Oct 2026: tooltips will name
  the keys).
- **Camera and scale**: entering keeps the flight zoom and leaving gives it back (home then starts on the lab and
  flies out); the layout is 1280 × 720 logical px scaled with the window (`ui_kit::ui_scale_for`).
- Not yet: the mockup's `SOURCE` line (where a part's salvage comes from: needs voyages and encounters to say it),
  a camera flight from the drydock to the lab, arrow keys between cards.

## Drydock

The ship editor (Drydock v1, decided by Niklas on 12 Oct 2026; the look follows
`../space-game-archive/concept/boards/ui-mockups/desktop-drydock.png`), `drydock/`, written 12 and 13 Oct 2026: the mode, the view
of the player's design, the screen with its widgets, editing (`data::design::edit`), the live stats, status line and
overlays from `sim::report`, saving (SAVE, the autosave), the BLUEPRINTS and EXPORT screens, share codes and files
(Storage and sharing below), the BEHAVIOR button and the test flight ([GDR-0056](gdr/0056-drydock-wave-2-defaults.md)).
Layout v2 since 14 Oct 2026 ([GDR-0082](gdr/0082-drydock-layout-v2-and-part-availability.md), details in
`docs/drydock.md` "Client: editing"). Parts are unlimited for now; each palette row shows how many are left (`∞`),
from the career's part availability.

Open it with DRYDOCK at home, `?scene=drydock`, or T in flight; the ring button top left goes home. Screenshots:
`docs/results/drydock-*.png`.

| Keys (only while no text field has the keyboard) | |
|---|---|
| Tools | B paint, G fill, X erase, V select; R / Shift+R (or E / Q, or Shift+wheel down / up over the grid) turn the selected part, else the part to place (a slope brush steps through its orientations, orienting itself to the hull until turned; picking a part resets it) |
| Grid (mouse) | left button: the tool (drag: paint and erase strokes, the fill rectangle, moving the selected part); right button: erase |
| Selected part | F flip, Delete / Backspace delete, Esc deselect (R / Shift+R, E / Q turn it); Esc first closes an open bottom bar menu |
| Mirror / bevel | M / Shift+B (bevel cuts a paint stroke's or fill's outer corners when it ends, GDR-0081) |
| Armour material | 1–9 (the armour swatches in content order; a locked material's key does nothing) |
| Undo / redo / save | Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y / Ctrl+S (the browser's save dialog stays shut) |
| Search the parts | / (Esc or Enter leaves the field; a click elsewhere too) |
| Zoom | wheel (around the cursor; Shift+wheel turns instead), + / −, the − / + buttons; 100 % = the whole frame fits |
| Pan | Space + drag, or drag with the middle button |
| Test flight | T or TEST-FLY (disabled, with the reason in the status line, while the design can't fly) |
| Debug overlay | F8 (H/P/Shift+R are flight keys) |
| Rename the design | click the name top left; Enter (or a click elsewhere) renames, Esc keeps the old name |
| BLUEPRINTS / EXPORT screens | the buttons; Esc or CLOSE closes them (Esc first leaves a rename, a delete question or a selected box); Ctrl+C copies the selected share code, Ctrl+V pastes into the paste box. While a screen covers the design the other keys do nothing (Ctrl+S still saves; the bottom bar's buttons work) |

The screen, top to bottom: the ring menu button, the breadcrumb `SHIPYARD › DRYDOCK · FRIGATE FRAME` (the
frame's content name) and the design's name (a click renames it; "UNSAVED"
or "SAVED" after it); the OVERLAY pills (HEAT, MASS, BONUS in gold, SHIELDS, ARCS:
any combination, drawn over the design); left the search, `ARMOR · <name>` over the armour swatches (every content
material with an `armor` block; one the career hasn't unlocked shows a lock and leads to its research, one no
research unlocks is left out), one line of category chips (content categories; it scrolls sideways and jumps the
list to a section) and every part in sections (a heading per category with its count, `WEAPONS 1 · 2 LOCKED`; the
unlocked parts: icon, name, footprint and one key stat from
`Content::part_stats`: the first stat after build points, control, mass, hit points, power draw and heat, else the
bonus it gives, else its hit points; a gold NEW tag on fresh ones; how many are left, `∞`; then the locked parts research can unlock, dimmed with a lock and
`LOCKED · <research node>`, not pickable, GDR-0065; the list scrolls by pixels since 14 Oct 2026, it paged whole entries before; the search filters it); right the stats grid (POWER, PEAK HEAT, THRUST/MASS, YAW,
FIREPOWER, CARGO, CONTROL, DEVICE LOAD, then a compact TOP SPEED, TURN 90°, MASS, BUILD POINTS block, all from
`sim::report`) and SELECTED PART (the selected placed part with the report's numbers and its controls, else the
picked part: icon, name, footprint and category, summary, stat lines and bonus lines; what is under the name
scrolls when it doesn't fit above the bottom bar; its bottom edge fades while more lies below); below the grid the hover
cell, mirror and axis, why the edit under the cursor can't happen (coral), and the zoom; at the bottom undo / redo,
the tools (PAINT, FILL, ERASE, SELECT, ROTATE), MIRROR, BEVEL, the status line over the voyage-ready line (each
shortened with `…` when it doesn't fit), SAVE, BLUEPRINTS and EXPORT as icons, the ⋯ menu (NEW DESIGN, FRAME,
COPY SHARE CODE, the `.ron` file, IMPORT), BEHAVIOR (opens the [Behavior screen](#behavior-screen); no key) and
TEST-FLY. When the bar is too narrow the tools fold into one dropdown (`PAINT ▾`) and mirror and bevel into icons. Laid out for 1280 × 720 logical pixels; other windows scale the whole
screen (`UiScale` = min(w / 1280, h / 720), 0.5 to 2, at home, in the drydock, on the BEHAVIOR screen and FLEET; flight
keeps 1). So the layout always fits: a window narrower or lower than 1280 × 720 scales it down (until 13 Oct 2026 the
scale stopped at 1 and a centred layout such as FLEET's lost its left and top edges), a wider aspect leaves room at
the sides, a taller one above and below (`ui_kit::ui_scale`, tested for every play-test size). The phone layout comes later.

- **Design** (`player/design.rs`, outside the drydock since ADR-0038): `PlayerDesign` holds the editing model (`data::design::edit::Drydock`) and the blueprint built
  from it in frame coordinates (`size` = the frame), plus `revision` and `generation`; every change goes through
  `PlayerDesign::edit`, which bumps `revision`; `replace` (another design opened) bumps both (cells of the old
  design, the selection, mean nothing in the new one). At start it is the last session's working design
  (`library::restore_working`, in `BlueprintsPlugin`'s build), else (and at every new game) the starter: a copy of
  the career's start ship (`PlayerCareer::start_ship`: `rim_skiff` for `rim_awakening`) centred in the first content
  frame it fits (`frames.ron`: the 24 × 24 frigate), mirror on, named "Rim skiff copy"; a new game saves the last
  design's unsaved changes first. It flies under the id `player:design` (content ids are snake_case, so it never clashes). The player, the
  `?fleet=` wingmen, re-forms and test flights fly it; raiders stay content ships.
- **Report** (`report.rs`): `DrydockReport`, the design's `sim::report`, recomputed once per revision or tuning
  change (during a paint or erase drag at most every 0.1 s, and once when it ends: a full 64 × 64 frame takes about
  5 ms). The BEHAVIOR screen reads the same report (the weapon reach, whether TEST IN RANGE can fly the design). **Panels** (`panels/`): the screen's nodes, spawned once and updated in place from the resources (the
  palette list and its key stat). **Behaviors** (`player/behaviors.rs`): `PlayerBehaviors`, the player's saved behaviors
  on the storage layer, the behavior a design names and its fallback with a banner ([drydock.md](drydock.md#behaviors-storage-designs-and-tests)).
- **Stats and status** (`stats.rs`): whenever `report::DrydockReport` is recomputed, `stats::update` fills
  `DrydockStats` and `DrydockStatus` from it (`CAN'T FLY` for blockers, an empty frame or a design that doesn't
  compile, which sets `grounded`; else `FLIGHT-READY` with the most important warning); `stats::voyage` adds the
  career's voyage-ready line for the fleet with this design as the flagship (`career::voyage_line`). Mapping,
  wording and tones: `docs/drydock.md` "Client: stats and overlays".
- **Overlays** (`overlays.rs`): the five overlays and the issue marks, one world-space mesh over the design (a
  child of its mesh), rebuilt when the report or the overlay pills change, and while ARCS is on when the view
  moves (in `PostUpdate` after the camera): the lines of fire and plume lanes (45–75 m) end at the grid
  viewport's edge instead of running through the pills and the status line. Details in `docs/drydock.md`.
- **State** (`state.rs`): `DrydockUi` (tool, armour material, overlays, the brush rotation, the picked part, a
  category chip's jump, the search text, the open bottom bar menu; the mirror and bevel modes are the editor's,
  `PlayerDesign::dock().mirror` and `.bevel`), `DrydockStatus` (`grounded`, the status line and its tone, the
  voyage-ready line and its tone), `DrydockStats` (the stats grid's cells: label, value, tone, optional bar);
  `palette_sections` (a section per category with the search's parts: the unlocked ones, then the locked ones,
  from `career::Granted`) and `armor_materials` (every armour material; the swatches show a lock on the locked ones).
  The widgets only show these. Locked parts and materials can't be picked: their rows and swatches are disabled, and
  `actions::apply` refuses them (the keys 1–9 reach locked materials too). A design that uses locked parts still
  opens, shows and test-flies. Part counts (`Granted::counts`, from `Career::available`; none yet) reach the editing
  model in `design::sync_available` when they change or another design opens.
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
  the selected part), `blueprints/` save, export and the blueprint list (below), `behavior_ui` the BEHAVIOR
  button; Menu (the ring button) goes home. New handlers read the message in a system in
  `DockSet::Apply` (`Input → Apply → Show`, drydock only), change `PlayerDesign` through `PlayerDesign::edit` (the revision goes up by itself), and
  write `DrydockStats` / `DrydockStatus`; `DrydockCursor::cell` is the frame cell under the pointer.
- **View** (`view.rs`): the design is drawn with the flight tiles: `ship_model::blueprint_visual` (every part;
  line of fire from `ShipGrid::line_of_fire_clear`, the hull edge around all parts, nav lights on the main piece,
  no aura) and `ship_view::build_body` into one mesh, replaced only when `revision` (or the glow toggle)
  changes. Blueprint `(x, y)` is world `(x, −y)`. The frame overlay is immediate-mode FX (dots at the cell
  corners, at least 6 px apart: on small cells only every 2nd, 4th, ... corner, brackets, the dashed mirror axis while mirroring) drawn inside the grid viewport (the pointer's mark, the cell or in SELECT and ERASE the whole part under it, is the ghost's); a
  veil in the backdrop's darkest colour (90 %; 62 % until 14 Oct 2026 left a zoomed ship too visible under the panels) dims the world outside it, so a ship zoomed under the side panels
  sinks into the background and the grid area reads lighter than the panels, as in the mockup.
- **Camera**: the drydock drives the shared `CameraRig` (the atlas tier follows its zoom). `CameraRig::view_offset`
  centres the rig on the grid viewport between the panels while the camera still covers the window, and the
  screen ↔ world helpers include it. Opening the drydock remembers the flight zoom; leaving restores it and keeps
  the drydock's view for the next visit.
- **Storage and sharing** (`storage.rs`, `clipboard.rs`, `player/library.rs`, `share.rs`, `blueprints/`;
  [drydock.md "Storage and sharing"](drydock.md#storage-and-sharing)): `Storage` (named RON documents per
  namespace with a format version; the browser's local storage on the web, files in the user's data directory on
  desktop, memory as the fallback), saved designs and the autosaved working design (restored at start), the
  BLUEPRINTS and EXPORT screens (one floating layer over everything above the bottom bar: `ShelfRoot`, a `Hot`
  veil so clicks reach nothing under it; hidden at once when the drydock is left, with the clipboard's offer and
  paste target let go), share codes and `.ron` files, copy and paste (`Clipboard`). The screens' handlers run
  before the editing tools and the report, so a design opened, renamed or moved to another frame gets its report
  and status in the same frame. Status messages go out through `DrydockStatus` once `stats::update` has made the
  status for the current revision (`blueprints::announce`).
- **Widgets** (`ui_kit/`): labels (tracked uppercase), numbers, prose, pills (Barlow medium, as the mockup's
  semi-bold pills; plain, active teal, active gold, the glowing primary, disabled, bare list rows; `PillLock`: a lock
  icon shown only while the pill is disabled, for locked armour and frames), ring buttons, value bars, corner
  brackets and a one-line
  text field (`TextField`, used for the names, the searches and the notes: a caret and a selection, both drawn in
  place as text spans of `field_text`; Left / Right / Home / End move the caret, with Shift they select, Backspace
  / Delete work at the caret, Ctrl+A selects all, Ctrl+C / Ctrl+X / Ctrl+V copy, cut and paste through
  `Clipboard::field`; a press on the field puts the caret at the end; the editing itself is the pure
  `ui_kit::edit_line`, 13 Oct 2026); hit testing by hand from the laid-out rectangles (`Hot`, `UiClick`; the topmost node wins; mouse and
  touch), `UiPointer::blocked` keeps the pointer over panels (`UiBlock`) and widgets out of the grid and the
  camera, `UiPointer::typing` turns the shortcuts off.
- **Scrollbars and glow room** (`ui_kit::Scrollbar`, `ui_kit/scrollbar.rs`, 14 Oct 2026; Niklas: "the system is
  missing scroll bars in general" and "the cut of glow looks weird"). One component on a node that scrolls gives it
  an overlay scrollbar: `Scrollbar::vertical()` over its right edge, `Scrollbar::horizontal()` along its bottom,
  `.outside()` just under a sideways strip (in the gap below it). The kit spawns the bar (a `Hot` hit area 10 px
  across with the thumb inside) and, for an area with its own `ScrollPosition`, two edge fades as the area's next
  siblings, and places them every frame from the area's layout (the parent must have no border on that side). The
  thumb is the visible share of the track (at least 24 px) and follows the scroll (`thumb`, `scroll_for`, tested);
  An area with its bar inside keeps a 10 px gutter for it (right or bottom padding, whether or not it scrolls, so
  the layout never shifts; Niklas: "the scrollbar overlaps with the content"). 4 px wide, 6 px while the pointer is
  on it or drags it, label grey at 38 % (62 % then), fully rounded, 2 px in
  from the edge. It shows while the area scrolls (any change of its place, so touch shows it too) and while the
  pointer is over the area, stays 1 s and fades out over 0.35 s (`bar_alpha`, tested); never while the content
  fits. A press on the thumb drags it, a press on the track beside it jumps the thumb's middle there and drags on.
  What scrolls is the area's `ScrollPosition` (its content size from the layout), or `ScrollRows` for a list that
  shows a fixed pool of row nodes: since 14 Oct 2026 such a list scrolls by pixels too (it paged whole rows
  before): its scroll is in UI px, the screen lays its rows out from it each frame (`ScrollRows::lay` over
  `row_window`: the rows' lengths and gap, the view, the pool's size; it returns the pool's first row, how many it
  holds, the rows in view and the shift) and the kit shifts the pool by the part above the view (the node's own
  `ScrollPosition`, so the node has `Overflow::scroll_*`). The pool keeps its rows while the view stays inside
  them and otherwise moves on as far as it can: a row node that shows another row builds its part or ship icon
  again, and that mesh missed a frame, so gliding past rows blinked the icons; the drydock's palette (48 nodes)
  holds the whole list, FLEET has 12 card nodes, the text-only BLUEPRINTS and BEHAVIORS lists one more than fit. A
  screen that starts a list at the top writes its scroll (BLUEPRINTS and BEHAVIORS through a `top` request, set
  where a new entry lands at the top). The kit also sets the area's
  `overflow_clip_margin` to `Scrollbar::room` px past its padding box (default 30: the kit's glow is a box shadow
  with a 10 px blur, a Gaussian with sigma 10 px, gone after three sigma; 16 px still left a visible line under a
  lit chip, Niklas: "this glow cutoff still looks weird"), so the glow of a lit row, chip, swatch or card at an edge fades out instead of ending in a straight
  line. An edge that content is scrolled past cuts it right at the edge instead (`cut_scrolled_edges`, after
  bevy_ui's clipping: every node inside gets one more clip rect, open across; 14 Oct 2026, Niklas: "the parts list
  overlaps this categories thingy", rows scrolled out showed in the room over the category chips). There are no fades over the edges (they were a
  band of backdrop colour over the edge while more lay past it; removed the same day, as in Chrome: Niklas found
  the panels' "weird gradient at top and bottom" odd over what lies behind them). Part and ship icons are world meshes that bevy_ui's clip does not reach, so an
  icon under a clip draws with an atlas material of its own cut to the clip's rectangle (`AtlasMaterial::clip`,
  `atlas::set_clip`, following its screen part's fade): a row half out of view shows half its icon (it hid the
  whole icon until fully in view before; Niklas: "it is unacceptable that the icon only shows when fully into
  view"). The
  bar is all that says an area scrolls: the `MORE BELOW · WHEEL` cue (`ScrollCue`) and FLEET's
  `THE WHEEL SCROLLS` are gone (14 Oct 2026; Niklas: "there is no need for the UI to say that you can scroll via
  wheel"). Every scrolling area has one: the drydock's part list (`ScrollRows`), armour swatches and
  category chips (outside, below them), the SELECTED PART lines (room 0: the name block is right above); research's
  lanes; the BEHAVIOR screen's graph, inspector and ADD NODE palette (room 0: its pills don't glow, the search is
  right above); the BLUEPRINTS and BEHAVIORS lists (`ScrollRows`); FLEET's design cards (`ScrollRows`, outside); the
  debrief's report. (Smaller rooms that kept a fade off a label close by went with the fades.) The egui TEXT editor keeps its own scrollbars.
- **Smooth scrolling** (`ui_kit::SmoothScroll`, `ui_kit/scrollbar.rs`, 14 Oct 2026; Niklas: "I want smooth
  scrolling like in chrome, not whatever the current thing is"). Every `Scrollbar` area has it (`Scrollbar` requires
  it), and the kit owns the wheel for all of them (the screens' own wheel systems are gone): a wheel event goes to
  the area whose bar is under the pointer, else to the topmost area under it that has more to show than fits,
  unless a layer above it has the pointer (the topmost `Hot` node under the pointer, `UiPointer`'s `over`, is above
  the area and not inside it: BLUEPRINTS over the drydock, the BEHAVIORS list over the graph). A sideways area takes
  the wheel's up and down too. What one event does (`wheel_move`, tested): a **notch glides** (`Glide`, tested):
  `WHEEL_STEP` = 100 UI px a notch (Chrome's step at a scale of 1; on screen it scales with the UI), at most
  `PAGE_SHARE` = 7/8 of the view, over `GLIDE_SECONDS` = 0.18 s on an ease-out cubic (`1 − (1 − t)³`: half way after
  about a fifth of the time, settling softly); a notch while a glide runs moves the running glide's target on by
  another step and starts the curve again from where the area is, so quick notches add up and speed it up, never
  stop it. A notch is a `Line` delta (one line natively, three in a browser that reports lines) or a `Pixel` delta
  of at least `PIXEL_NOTCH` = 40 CSS px: browsers report a mouse wheel's notch in pixels (100 in Chrome on Windows,
  120 on Linux), so in the web build that is what tells a wheel from a touchpad; smaller pixel deltas are a
  touchpad's own smooth stream and **move the area at once**, 1:1 with the fingers (a running glide moves along
  with them). Every glide ends inside the content: its target is clamped to the ends, a notch at an end does
  nothing, a list that gets shorter while it glides keeps it inside. Dragging the bar's thumb is direct and stops a
  glide; so does any place a screen writes itself (the system notes what it wrote last, a different place means
  someone else moved the area). A screen can glide an area somewhere with `SmoothScroll::to` (the drydock's lit
  chip into view, a category chip's jump to its section); asked again for the same place, the running glide runs
  on. There is no keyboard paging in the lists, and no touch drag on them (the chip line's mouse drag stays
  direct). Headless check at 8 fps (SwiftShader): a notch's glide takes two frames there, the first at 48–57 px of
  100, so `docs/results/smooth-scroll-glide.png` shows frames captured in the page (before, the first frame, settled).
- **Tooltips** (`ui_kit::Tip`, 13 Oct 2026; Niklas: labels carry no key letters, the tooltip names the key): a
  `Tip { name, key }` on any `Hot` node (pills, rings, fields, and home's invisible click areas over the world's
  places, so the shipyard's places get them too). Resting the mouse on it for 0.5 s (`TIP_DELAY`) shows one shared
  plate (`show_tip`, in `UiStyleSet`): the name in label grey, the key in white (`B`, `CTRL+S`, `Q / E`, `ESC`),
  12 px right of and 20 px below the pointer, flipped left or above it at the window's edges and kept 6 px inside the
  window (`tip_position`, tested). It moves by its `UiTransform` (no layout), is laid out for one frame before it
  shows, hides when the pointer leaves or presses (until it leaves the control), and never shows for touch. Every
  control with a key has one (the drydock's tools, MIRROR, BEVEL, SAVE, TEST-FLY, undo / redo, the zoom rings, the
  ARMOR pills `1`–`9`, the search `/`, FLIP, DELETE, the rotate rings; the BEHAVIOR screen's TEST IN RANGE, DRYDOCK,
  SAVE, undo / redo, search; CLOSE on FLEET, BLUEPRINTS and BEHAVIORS; DISMISS; RECALL, ABANDON VOYAGE and KEEP FLYING; the home rings; home's
  places); the keys work as before. Screens added later give their controls a `Tip` instead of a key in the label.
  Banners and help lines that tell what a key does (`PAUSED · P`, the flight help line) stay: they are not
  controls.
- **Icons in the UI** (`ui_icons.rs`): a `UiIcon` on a node (a part, a whole design for FLEET, or a glyph Barlow
  lacks: undo, redo, play, search, menu, diamond, lock; home's grid, tree, orbit, arrow, target, formation) is drawn
  as a world-space mesh of the main camera placed over the node's rectangle every
  frame (screen → world through the rig, scale 1 / zoom, z 600). A part icon is a one-part blueprint through
  `build_body`: a flight tile (a machine: its icon on the machine tile colour, where ships show its drawing,
  GDR-0069), composited in sRGB like the ships, so its colours are exactly the flight tiles' (bevy_ui image nodes
  would sample the sRGB atlas without encoding for the sRGB compositing and look too dark); `atlas::UiAtlasNeed`
  raises the atlas tier to the icons' pixels per cell (twice what a part icon shows: icons are stored at
  `art::ICON_DENSITY`, 0.5). bevy_ui draws after
  the world, so nodes over an icon should have at most a faint background. bevy_ui's clipping doesn't reach the
  meshes: an icon that a scrolled parent clips (`CalculatedClip`) is hidden until it is whole again. An icon follows
  its node when a screen transition slides it, and fades with its screen part (the part's atlas material,
  `ui_kit::IconMaterials`; [Screen transitions](#screen-transitions)).

## Behavior screen

How the design on the drydock's grid thinks (Behavior editor v1, [GDR-0057](gdr/0057-behavior-editor-v1.md); lead
defaults [GDR-0059](gdr/0059-behavior-editor-wave-a-defaults.md), [GDR-0060](gdr/0060-behavior-editor-wave-b-defaults.md); the
look follows `../space-game-archive/concept/boards/ui-mockups/phone-behavior-editor.png`, laid out for the desktop), `behavior_ui/`,
written 13 Oct 2026: the mode, the screen, the graph and text views, the inspector and the palette; behaviors as
saved documents (the name and SAVE, the BEHAVIORS list, the design's behavior) and TEST IN RANGE with a live rule
panel in flight. Both views edit through the editing model `data::behavior::edit` (`docs/behavior-format.md`
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
| Debug overlay | F8 (in every mode) |

The screen, top to bottom: the ring menu button, the breadcrumb `BEHAVIOR · HALVEX RUNNER` (the design's name) and
the behavior's name (a click renames it) with its saved state after it (TEMPLATE, SAVED, UNSAVED or NOT SAVED YET,
and `THE DESIGN FLIES <NAME>` when the open behavior is not the design's), the `6 / 12 RULE SLOTS` pill (rules / the
design's slots, `Content::ship_rule_slots`, from its logic cores); under them, over the graph area, the tabs
BEHAVIOR / PILOT ASSISTS (the design's two rule lists, PILOT ASSISTS below); top right GRAPH / TEXT (how the open list
is shown, one choice for both tabs), TEST IN RANGE with WATCH and FLY ALONGSIDE under it. Left the graph: the `TARGET` line
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
summary, then its controls (a tall inspector scrolls; the palette under it keeps about three rows; while either
has more below, its bottom edge fades).
Screenshots: `docs/results/behavior-editing-condition.png` (a condition with `not`, its comparison and its
threshold), `docs/results/behavior-editing-rule.png` (a rule added with `+ RULE` and a palette condition, its
action selected).

**PILOT ASSISTS** (the tab after BEHAVIOR; GDR-0088 step 3, 15 Oct 2026, `behavior_ui/assists.rs`;
`docs/behavior-format.md` "Pilot assists"): the design's pilot assists in the same GRAPH and TEXT views, inspector and
palette. Until step 4 (15 Oct 2026, Niklas: the split was weird) PILOT ASSISTS was a third pill after GRAPH and TEXT
and graph-only; now the two lists are the tabs (`BehaviorUi::tab`, `BehaviorTab`, `BehaviorAction::Tab`) and
GRAPH / TEXT (`BehaviorUi::view`) works the same in both, kept when the tab changes. The TEXT view edits the assist
list as rule text, exactly what the blueprint stores (`brake(toggle2, off)`), with the behavior text editor's
highlighting, live errors and apply (`BehaviorDraft::set_text` on the `ListKind::Assists` editor; no slot or
never-reached notes there); the status line reads `VALID · 4 ASSISTS` or the errors. The screen's draft becomes the
assist list (`BehaviorDraft::assists`, a `ListKind::Assists` editor; the behavior's draft waits in `AssistTab` and
comes back with the BEHAVIOR tab, BEHAVIORS or on leaving the screen; switching tabs tidies the text as leaving TEXT
does). The assists' draft waits in `AssistTab` too while BEHAVIOR is open, so a text with errors is still there on
the way back; the design keeps the last valid list, and leaving the screen drops the typed errors. What changes there: the
name reads `Pilot assists · SAVED WITH THE DESIGN` (no rename), the pill `3 ASSISTS · WHILE YOU FLY IT`, the root
`Assists · each channel`, no slot line and no never-reached line (each rule decides its own channel), `+ ASSIST`
(also into an empty list), the palette lists the assists' actions and every condition (the pilot's `lock_held` and
`idle` too), and the last rule can be deleted. An assist's inspector has its values as pills: TOGGLE (NONE, 1–4: the
ASSIST TOGGLE key that switches it in flight) and AT START (ON, OFF), auto-fire's GROUP and hold station's STATION (POINT, ALLY). Every change goes into
the working design at once (`PlayerDesign::edit_header`, written to the storage when no slider is held; the drydock
shows the design UNSAVED until it is saved); SAVE says so, TEST IN RANGE tests the behavior (the design flies with
its assists). A list line that no longer checks is left out, with a note. Screenshot
`docs/results/behavior-pilot-assists.png` (step 3: three assists: face target on toggle 1, brake when it can stop straight on
toggle 2, auto-fire on toggle 3 starting off, selected; the old tab arrangement), `docs/results/behavior-pilot-assists-tabs.png` and
`docs/results/behavior-pilot-assists-text.png` (step 4, 1600 × 900: the tabs, avoid collisions, hold station, face
target and auto-fire in the graph and as text; checked at 1280 × 720 and 2560 × 1440 too).

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
  `data::behavior::edit::BehaviorEditor` (`editor()`, `program()`), its text and the text's errors (line, column,
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
  `data::behavior::edit::NodeRef::{Target, Action(RuleId), Cond(CondId)}`, rows with their `RuleId`, `slot_line()`),
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

- **One mesh, one material, one draw call for every bubble.** `draw_bubbles` walks the piece views every
  frame and writes one quad per visible bubble (centre at the render pose, half-size = radius + 4 glow widths,
  more while shattering) into a world-space mesh, in place, like the FX meshes. Each quad carries its bubble's
  state as vertex attributes: offset from the centre, radius, seed; charge, presence, shatter progress; four
  hits (direction in world space, age, strength). The material (`BubbleMaterial`) holds only the style tokens
  and is never changed, so there are no per-frame material updates; the shimmer and flicker run on
  `globals.time` (virtual time: they pause with P). Why: per-bubble materials (the old rings) cost a draw call
  and a bind-group update per bubble per frame; WebGL2 has no storage buffers, and a uniform array would still
  change the material every frame.
- **Between the ship layers, by depth** (13 Oct 2026). The mesh draws after every ship (z 200, below the weapon
  effects), but each quad's vertex z puts it at its own piece's draw layer + 1/16 (`ship_view::BUBBLE_DZ`; the
  focus: z 100 + 1/16), and the lit ship tiles write depth: they are a second mesh per lit piece with the opaque
  twin of the atlas material (`Atlas::opaque`, Bevy's `Opaque2d` pass, drawn before every transparent mesh). The
  bubble pipeline tests depth (`GreaterEqual`, as every 2D mesh) without writing it, so a bubble is hidden under
  the tiles of every ship above its own and shows over every ship below, the focus's over all of them. Bevy sorts
  transparent 2D meshes per entity by z only, so no quad order inside one mesh could do this. History: until
  12 Oct all bubbles were one mesh above every ship; then two meshes (the focus's at z 200, every other bubble at
  z 95), which left two non-focus ships showing each other's bubbles over their hulls. What stays: a bubble can
  still show over the transparent parts of a higher ship that lie beside its tiles (hull halo, muzzle glows,
  aura) and over dark pieces (salvage and unpowered pieces keep their half-transparent tiles in one blended mesh);
  with max blending only where the bubble is brighter. Cost: one more mesh entity and draw per lit piece (opaque
  pass), one bubble draw instead of two. Screenshots: `docs/results/shield-order-*.png`.
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
  and capped by the GPU's largest 2D texture (tier 512 is 4096 × 4096 since the ship drawings, 13 Oct 2026; logged once at start). Up at once, down
  only when half the tier is clearly enough. A new tier must be wanted for 0.25 s first (zooming through tiers
  costs nothing).
- **Rebuild** (`art::Rasterizer::step`, small steps since 13 Oct 2026): zero the buffer 2 MB at a time, render
  every region a band of rows at a time (at most 32 K texels per step), then the mip chain (downsampling, then
  the colour pull) a band at a time; within 6 ms of work per frame, 16 ms while the atlas on screen is stretched
  more than 2× (it looks blurry then). The old atlas stays on screen until the new one is complete. A job for a
  lower tier than the zoom now needs is dropped for the new one (and a job that would rebuild the tier already on
  screen is dropped too).
  - **Soft layers** (the machines' and icons' glows, the plumes' blurred glow; a plume is its blurred glow polygon under its crisp
    gradient triangle, two SVG documents) look the same at every tier and are blurred over 5 px or more at tier
    128, up to 41 px at 512. They are rendered once per session at `art::SOFT_TIER` (128; kept in the `Layout`, a
    step of its own, at most 31 K texels) and resampled bilinearly into every higher tier a band at a time; lower
    tiers render them directly (small). A blur can't be cut into bands, so before this one plume region was one
    step: 25 ms natively (median, p95 36 ms) and 70 ms in wasm at tier 512.
    Soft layers small enough to blur in one step at the tier (at most `art::STEP_TEXELS`: the part icons' glows,
    13 Oct 2026) are rendered at the tier instead: at half density they are too few texels at tier 128 to resample.
  - **Crisp layers** (ship drawings, icons, the barrel, the plumes' triangle) render at the tier a band at a time;
    a crisp layer that uses a filter (no base drawing does) renders whole.
  - **Measured** (`cargo run --release -p art --example profile -- 512 21`: steps spent as the game spends
    them, a new layout per run; the same example on the code before, 13 Oct 2026, load average 8.6–9.2): longest
    step 25.1 → 1.4 ms (median; p95 36.0 → 5.0 ms), work 231 → 130 ms natively; in wasm (the example built for
    `wasm32-wasip1`, run under Node with `scripts/wasi-run.mjs`) 70.1 → 4.2 ms (p95 105.9 → 6.8 ms), work 549 →
    201 ms, and the job's start (84 MB of zeroes allocated at once) 3.1 → 0 ms. The busiest steps now are the
    plumes' soft layers at tier 128 (once per session) and the fusion reactor's icon bands.
  - **Same look**: tiers up to 64 are byte-identical; soft layers above 128 are within 2–3 levels of 255 of
    rendering them at the tier (mean 0.1–0.3); crisp icons differ on about 0.2 % of values, along curves
    (tiny-skia flattens a curve cut at a band edge slightly differently: sub-pixel). `art`'s test
    `bands_and_resampled_soft_layers_match_a_whole_render` checks this, `steps_are_bounded_and_report_every_row_once`
    the work per step.
- **Upload and switch** (`atlas::rasterise_step`, `upload_bands`, 13 Oct 2026): a job creates the new tier's
  texture without data (also a render target, so wgpu clears each mip level with a pass before its first band).
  Every band the pull pass finishes is final (`Rasterizer::finished`): the step hands it to the render world (a
  queue both worlds share), at most 350 KB per millisecond of the frame's budget (2.1 MB per frame, 5.6 MB while
  blurry), and `upload_bands` writes it with `RenderQueue::write_texture`. Once every band is written, the
  materials' texture handle is set to the new image (written through `get_mut`, so Bevy re-prepares them and
  rebuilds their bind groups) and the old image goes with its last handle. Before, the finished atlas was added
  as an image with all its data, so the render world uploaded all of it in one frame (84 MB at tier 512) and the
  switch waited a frame for that. A new image per tier also avoids Bevy's in-place update for an unchanged texture
  size, which writes mip level 0 only and would leave stale mips. Headless Chromium (SwiftShader, so the frame
  times say nothing; tier 512 at 160 px/m and device scale 2, so the blurry budget): 396 ms of work over 34 frames,
  at most 5.6 MB uploaded per frame (before: 300–410 ms over 18–23 frames, then 84 MB in one frame). Memory: one atlas on the GPU (tier 32: 0.3 MB, 64: 1.4 MB, 128: 5.6 MB, 256: 22 MB, 512:
  89 MB, mips included; 84 MB before the ship drawings), two while a job runs; the job's own buffer is as large and goes when it is done.
- **Sampling** (`AtlasMaterial`, `atlas.wesl`): every atlas mesh (ships, effects, backdrop, the drydock's view and
  its UI icons) draws with one material: atlas × vertex colour, alpha-blended, sRGB-encoded under `SRGB_OUTPUT`
  like Bevy's `ColorMaterial`, which it replaced on 13 Oct 2026 (the lit ship tiles use its opaque twin,
  `Atlas::opaque`, with the same texture: see Shield bubbles). The sampler is linear / linear / linear
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
- **What it holds** (13 Oct 2026, GDR-0069): the machines' ship drawings at full density (the world zooms in on
  them), the part icons at `art::ICON_DENSITY` = 0.5 (only the UI's part tiles draw them, at most about 40 px per
  cell; `UiAtlasNeed` asks for twice their pixels), no material or category icons (only the wiki shows those), the
  FX textures. Regions are packed on a skyline (tallest first, each at the lowest spot it fits) into 8 × 8 units:
  4096 × 4096 px at tier 512 (before: shelves, 8 × 7.5 units without the drawings; with them shelves needed 8 × 9,
  4608 px, over the 4096 the top tier keeps to). A mod's drawings grow it; past the GPU's limit `max_tier` caps the
  tier. Profile at tier 512 (`... --example profile -- 512 11`, load 8.4): longest step 1.6 ms median (p95 3.6),
  work 149 ms (130 before).
- **Soft by design**: glows and plumes are stored at half density (they are blurred; above tier 128 their
  blurred layers are resampled from it); at the highest zoom a plume's edge is about 1.5 px soft. Everything else (drawings, barrel) is 1:1 with the tier; tiles, seams, hull lines
  and shield bubbles are geometry or shaders and stay sharp at any zoom.
- **Checking blur**: the debug overlay (F8; F3 until 15 Oct 2026) shows `atlas <tier> px/cell  need <px> (zoom x scale)  max <tier>  window <physical size>`
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
  part. A machine also needs its ship drawing, `content/base/icons/<key>.ship.svg` (`docs/content-model.md`
  "Ship drawings": the footprint from `0 0`, nose up, an engine's nozzle past the stern; glows under the drawing,
  so leave a window in the housing). `data` embeds the pack, validation checks both, and `art`'s test
  (`content_icons_and_ship_drawings_rasterise_at_two_tiers`) checks that every one rasterises.
- **A gun's barrel**: `barrel: (width, length, muzzle)` on the part in the content (`docs/content-model.md`); a gun
  without one gets a small muzzle glow at its front edge (`ship_model::design_visual`).
- **A new effect**: add a constructor to `fx::Effects` (sparks, flash, line), call it from the event
  match in `sim_bridge::apply_events`, draw it in `fx::draw_effects`. Continuous visuals that follow sim state
  (like plumes) go in `fx::draw_ships_fx` and read the sim at the render pose.
- **A new FX texture**: add it to `Layout::new` in `crates/client/art` (as SVG: a blurred `soft` document under a `crisp`
  one, either optional, so no step blurs a large region; or a `Proc` function of `(u, v)`; with a size in atlas
  units), look it up once in `fx::FxRegions`.
- **HUD text**: add a `HudText` variant in `flight_hud/` and fill it in `update_text` (8 Hz, re-laid out only
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
