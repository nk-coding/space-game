# Spike summary

The answers to the seven questions of `docs/spike-plan.md`, as far as they can be answered without a phone and
without Niklas's browser. Written 12 Oct 2026 (late evening) on `dev` `5b824b6`, after re-measuring on the new VM.
Phone measurements are deferred until Android work starts ([ADR-0006](../adr/0006-web-as-development-platform.md));
the browser measurements wait for Niklas (steps in [Steps for Niklas](#steps-for-niklas-host-browser)). The
numbers are those of 12 Oct 2026; later sim changes (the slower turning of 13 Oct 2026, GDR-0055) change the
flight numbers and the state hashes, as noted where it matters.

| # | Question | Status | Key numbers |
|---|---|---|---|
| 1 | Which physics engine? | **Answered**: Niklas chose Rapier2D (11 Oct 2026). Phone column deferred; browser column has hashes, no host-browser timings | combined scene 2.79 ms median step (old VM); collider layout matters 5–100× more than the engine |
| 2 | Deterministic across platforms? | **Answered for native vs wasm** (Node and headless Chromium); phone deferred; a one-minute check in Niklas's browser is optional | 8 of 8 scenario runs identical on the new VM; the new VM (AMD) reproduces the old VM's (Intel) hashes and counters exactly |
| 3 | Does breakage work? | **Answered** | split of a 2000-part piece 0.10 ms; nothing moves, momentum conserved, 10-tick ghosting; 129 sim tests pass |
| 4 | Is the phone fast enough? | **Deferred to Android** | native proxy: 20 ships 0.011 ms median / 0.028 ms p95 sim step, against a 4 ms phone budget |
| 5 | How does the browser compare? | **Answered natively, browser numbers pending** | wasm under Node (V8) 1.5× native for 20 ships (1.2–1.9× in the physics benchmark) |
| 6 | Is the style 2a look affordable? | **Browser numbers pending; phone deferred** | none yet (headless SwiftShader timings say nothing) |
| 7 | Does flying feel right? | **Browser: changed after the play-tests of 12 and 13 Oct 2026; phone deferred** | Runner 0 → 22 m/s in 2.5 s; 90° in 1.50 s since 13 Oct (0.77 s on 12 Oct, 3.3 s before) |

Machines: the old VM was an Intel Core Ultra 7 155H (15 vCPUs, 11 GB), the new VM (since 12 Oct 2026) is an AMD
Ryzen 9 9950X3D (16 vCPUs, 17 GB). Rust 1.99.0, Node 24.11.1, release builds.

## 1. Which physics engine?

**Answered.** Niklas chose **Rapier2D 0.36 with one compound collider per ship piece** on 11 Oct 2026 from
`docs/results/physics-benchmark.md` ([ADR-0014](../adr/0014-physics-engine-by-benchmark.md)). Pass criterion:
"benchmark table filled for Rapier2D, box2d-rust and Avian on native, phone and browser".

- Native (old VM, lockstep, load ~2): Rapier-compound was the fastest or tied in every realistic scene:
  combined scene with merged rectangles (2 320 bodies, 6 976 shapes, 5000 rays, 30 splits) **2.79 ms median /
  6.60 ms p95 step**, 6.14 ms for the whole tick; Avian-compound 3.44, box2d-rust 5.01, Box2D C 3.92 ms. A split
  costs ~0.04 ms with merged rectangles.
- wasm: Node (V8) costs 1.2–1.9× native on the realistic scenes, the same ranking for every engine.
- Browser: the same wasm builds ran full-length in headless Chromium and gave the native hashes (25 of 25).
  Host-browser timings of the benchmark page were never taken, and the benchmark repo was not copied to the new
  VM, so the benchmark has no new-VM numbers. Since the engine is chosen, the game's own browser load test
  (question 5) covers the browser cost instead.
- Phone column: deferred (ADR-0006).

How Rapier does inside the game: in the 20-ship load test the Rapier step is **0.006 ms median / 0.015 ms p95**
on the new VM (old VM, idle: 0.009 / 0.024); with 20 slabs of 2000 parts each (`sim_perf`) 0.007–0.008 ms
median. The sim's own systems cost more than the physics.

## 2. Can the sim be deterministic across platforms?

**Answered for native x86_64 vs wasm32** (Node's WASI and headless Chromium); the phone (aarch64) is deferred.
Pass criterion: identical per-tick hashes natively, on the phone and in the host browser.

New VM, 12 Oct 2026, `scripts/determinism-check.sh --scenario S --seed N --ticks 1800` (release, native x86_64 vs
`wasm32-wasip1` under Node 24), load average 0.7–0.8; run on `5b824b6` and again on `58c0630` (after the
`sim::report` merge) with the same results:

| Scenario | Seed 42 | Seed 7 |
|---|---|---|
| core | **IDENTICAL**, final `f5e16cf113c7670e` | **IDENTICAL**, final `d9441bd6e7145833` |
| flight | **IDENTICAL**, final `cd847bda91652eba` | **IDENTICAL**, final `f3b762c477c25b46` |
| load (20 ships) | **IDENTICAL**, final `ba98a6c1e7ec84e5` | **IDENTICAL**, final `34fccbaae2c4e339` |
| behavior | **IDENTICAL**, final `99dfe4dbc78fb681` | **IDENTICAL**, final `998e4eef66924bd9` |

(`final` is the hash over all 1800 per-tick hashes.)

- **Old VM vs new VM**: every seed-42 counter (parts destroyed, splits, shots, hits, shield hits, impacts, rams,
  burns, overheated part-ticks) and every seed-7 burn count equals the table in `docs/sim.md` "Determinism",
  measured on the old VM. So native Intel and native AMD agree too.
- **Browser**: on the old VM a dev wasm build of the game in headless Chromium gave the native hash stream for the
  20-ship load scenario: seed 42, 600 ticks, final `86eb72a03591bff2`, and seed 7, 1800 ticks, final
  `34fccbaae2c4e339` (`docs/client.md`). The new VM's native runs give the same two finals, so that check still
  holds: the `crates/sim` commits since then (the drydock's design report) change no hash. Niklas can confirm it in
  his own browser in a minute (step C below).
- History: no divergence has been recorded since the check was set up on 11 Oct 2026, through Rapier, weapons,
  Rhai behaviors, power tiers, the vessel model and engine plumes (`docs/sim.md` "Determinism").
- Not covered: aarch64 (Android). Everything is software math without FMA, so it should hold; it has to be
  measured on the phone.

## 3. Does breakage work in the chosen engine?

**Answered.** Pass criterion: ships split into pieces that keep their velocity and spin, with their own power pool,
and no explosions of overlapping bodies.

- **Velocity and spin**: every piece of a split gets the parent's angle and spin and
  `v + ω × (com_new − com_old)` (`ship::inherited_velocity`); Rapier does not correct the velocity for a centre-of-mass
  shift by itself, so the sim hands it over. Tests: `runner_splits_down_the_middle` (no surviving part moves, linear
  momentum is conserved, each half has its own closed hull), `split_pieces_carry_their_inherited_velocity_in_rapier`.
- **Own power pool**: power is computed per piece from that piece's reactors; a piece without a working core is
  salvage and draws nothing (`raider_breaks_into_salvage`, `unpowered_piece_goes_dark_and_wakes_up`); a half that
  keeps a core becomes a vessel of its own (GDR-0013).
- **No explosions**: all pieces of a split ghost each other for 10 ticks (a Rapier contact filter), so freshly
  separated pieces never push each other apart (`split_pieces_ghost_each_other_then_collide`). In the benchmark
  every engine split 30 times per run without anything exploding (checked visually from SVG snapshots).
- **Cost** (`sim_perf`, new VM, 15 runs): cut a 50-part row and split a 2000-part piece **0.105 ms** median (Rapier's
  part 0.04 ms); lose one part without a split 0.051 ms. Old VM: 0.09–0.11 ms and 0.08 ms (0.13–0.15 ms once a
  split half became a vessel, at load 2–7).
- In the scenarios: `core` seed 42 has 48 splits in 1800 ticks, identical on native and wasm.
- `cargo test -p sim --release` on the new VM: **129 passed, 0 failed**.

## 4. Is the phone fast enough?

**Deferred to Android** (ADR-0006). Pass criterion: 20 ships with debris and projectiles at 60 fps on the flagship
in half the frame budget (about 8 ms per frame, sim step under 4 ms), temperature after 10 minutes.

What the native numbers say in the meantime: the 20-ship load test's sim step is **0.011 ms median / 0.028 ms
p95** on the new VM, about 140× under the phone's 4 ms at p95. Heavier synthetic scenes stay under 4 ms natively
too: 20 slabs of 2000 parts fighting with 34 guns each, 0.43 ms median / 1.6 ms p95 (`sim_perf` combat). The worst
case measured is the benchmark's combined scene (2 320 bodies, 5000 rays per tick), 6.1 ms per tick on the old
VM, which is far beyond what the game spawns today. Rendering cost and temperature on a phone are unknown.

## 5. How does the browser compare?

**Answered natively; browser numbers pending (Niklas).** Pass criterion: the same load test in the host browser,
numbers recorded. (The "web bar" Niklas was to set from them no longer applies: the browser only needs to be
comfortable for development, ADR-0006.)

Native load test, `scripts/load-test-native.sh` (n = 1, 5, 20 ships, 1800 ticks, seed 42), sim step per tick in
ms, **median / p95**, as medians over the runs (the range of the per-run p95 in brackets):

| Ships | Old VM, `fa08197` (before power tiers), load ~7, 3 runs | Old VM, after plumes, idle (0.1–0.3), 5 runs | **New VM, idle (0.9–1.6), 15 runs** | New VM during a 6-job cargo build (9.1–10.9), 10 runs |
|---|---|---|---|---|
| 1 | 0.003 / 0.004 | – | **0.002 / 0.003** (0.003) | 0.003 / 0.003 (0.003–0.004) |
| 5 | 0.007 / 0.013 | – | **0.003 / 0.007** (0.006–0.007) | 0.004 / 0.007 (0.007–0.009) |
| 20 | 0.025 / 0.047 | 0.019 / 0.048 | **0.011 / 0.028** (0.027–0.034) | 0.012 / 0.029 (0.028–0.039) |
| 20, Rapier step alone | 0.014 / 0.025 | 0.009 / 0.024 | **0.006 / 0.015** (0.014–0.018) | 0.007 / 0.016 (0.015–0.021) |

- The last three columns run the identical fight (202 shots, 839 hits, 175 impacts, 3 splits, 10 ships active at
  the end, final state hash `a884134ca1f6bfeb`); the first column is an older fight. The mean for 20 ships is
  0.015 ms; single ticks reach 0.1–0.8 ms (load spikes).
- New VM vs old VM, same fight, both idle: **about 40 % less** per tick (0.011 vs 0.019 ms). A parallel 6-job
  build barely moves it.
- wasm vs native, as a stand-in for the browser's V8: the `determinism` example on the load scenario, 9000
  ticks, (t(9000) − t(1)) / 8999, median of 7 runs, new VM at load 0.7–1.1, per tick including a state hash:

| Ships | Native | wasm32-wasip1 under Node 24 | Ratio |
|---|---|---|---|
| 1 | 3.4 µs | 7.0 µs | 2.1× |
| 5 | 5.8 µs | 10.4 µs | 1.8× |
| 20 | 19.9 µs | 29.6 µs | 1.5× |

So the browser's sim step for 20 ships should be a few hundredths of a millisecond, and the browser's frame
time will be almost all rendering. Browser results (to fill in from step A):

| Ships | Frame ms mean / median / p95 | Sim ms mean | Browser, refresh rate, window |
|---|---|---|---|
| 1 | pending | pending | |
| 5 | pending | pending | |
| 20 | pending | pending | |

## 6. Is the style 2a look affordable?

**Browser numbers pending (Niklas); phone deferred.** Pass criterion: frame time with the look on vs debug shapes,
and with full-screen bloom on vs off, on phone and browser.

The renderer draws each sim piece as one atlas mesh with glow sprites by default; full-screen bloom is a toggle
([ADR-0020](../adr/0020-style-2a-renderer.md)). The VM has no GPU and headless Chromium renders with SwiftShader,
so no meaningful frame time exists yet. The client is ready: the URL parameters `debug=1`, `glow=0`, `bloom=1` and
the keys 1/2/3 switch the modes, and the load test logs a `LOADTEST` line with the mode in it. Results (to fill in
from step B, 20 ships):

| Mode | URL | Frame ms mean / median / p95 |
|---|---|---|
| style 2a, glow sprites, no bloom (default) | `?load=20` | pending |
| debug shapes | `?load=20&debug=1` | pending |
| style 2a without glow sprites | `?load=20&glow=0` | pending |
| style 2a, glow sprites and bloom | `?load=20&bloom=1` | pending |
| style 2a, bloom instead of glow sprites | `?load=20&glow=0&bloom=1` | pending |

## 7. Does flying feel right?

**Browser: changed after the first play-test; Niklas's verdict on the new flight is pending. Phone deferred.** Pass
criterion: Niklas flies the ship on the phone and in the browser and is happy with stick + lock, braking and the
speed cap.

Niklas flew the web build on 12 Oct 2026. What he said and what changed the same day:

- WASD as the touch stick felt very weird on a keyboard → ship-relative keys (A/D rotate, W/S thrust, Q/E strafe),
  the flight computer still cancels drift and brakes on release; the lock turns the strongest working guns onto
  the target with an A/D nudge ([GDR-0040](../gdr/0040-keyboard-flight-and-smart-lock-aim.md)).
- The Runner felt really sluggish (90° in 3.3 s, 0.65 m/s², ~10 m/s) → "Agile": 90° in 0.77 s, 0 → 95 % of
  22 m/s in 2.53 s, the raider snappier (0.40 s) ([GDR-0042](../gdr/0042-agility-target.md)).
- The engines going dark at the speed cap looked weird, and a fixed cap is wrong → each engine's rated speed sets
  the top speed, a calm cruise flame at the cap, no drag ([GDR-0041](../gdr/0041-engine-rated-speed.md)).

Niklas flew it again on 13 Oct 2026: the acceleration stays, but turning was far too strong and the A/D aim
slippery → keys turn at about 1.5 rad/s with a ramp, logic cores barely turn, gyros moderately, RCS pods strongly,
the Runner turns 90° in 1.50 s (the raider in 0.57 s), differential thrust only as a last resort, and a thruster
pushing with the motion fades by the whole speed ([GDR-0055](../gdr/0055-slower-crisper-turning.md),
[GDR-0058](../gdr/0058-turning-v2-defaults.md)). Still open and touching the feel (`docs/sim.md`
"Design questions"): the feel of "fly that way" (3). Braking and strafing under lock with rear engines only is
decided (GDR-0011: RCS pods or side thrusters are the answer).
Stick + lock with the touch stick is a phone question.

## Steps for Niklas (host browser)

Before you start: the lead session serves the current optimised build at `http://10.77.9.2:8080` (Niklas's copy).
If it is not up, run `scripts/web-build.sh` and then `PORT=8080 scripts/web-serve-dist.sh` in `~/workspace/space-game`
(the release build takes a few minutes).

Setup: use the browser you develop in, on mains power, window maximised, with nothing heavy running. Keep the
game tab in front the whole time (background tabs are throttled). Open DevTools as a separate window (F12, then
⋮ → Dock side → Undock) on the Console tab, so the game keeps the whole browser window.

**A. Load test (question 5).**

1. Open `http://10.77.9.2:8080/?load=1` and don't touch anything. The overlay at the bottom left shows FPS, frame
   ms and sim ms.
2. After a bit more than 60 s the console prints one line that starts with `LOADTEST ships=1`. Copy it.
3. Do the same with `http://10.77.9.2:8080/?load=5` and `http://10.77.9.2:8080/?load=20` (a fresh page load each).

**B. Cost of the look (question 6).** The same as A, a fresh page load each, copy each `LOADTEST` line:

- `http://10.77.9.2:8080/?load=20&debug=1` (debug shapes)
- `http://10.77.9.2:8080/?load=20&glow=0` (style 2a without glow sprites)
- `http://10.77.9.2:8080/?load=20&bloom=1` (style 2a with glow sprites and bloom)
- `http://10.77.9.2:8080/?load=20&glow=0&bloom=1` (bloom instead of glow sprites)

The `?load=20` run from A is the default (style 2a, glow sprites, no bloom). To compare live instead, the keys 1
(debug shapes), 2 (glow sprites) and 3 (bloom) switch the same things; the overlay's frame times cover the last 600
frames, so wait about 15 s after a switch before reading them.

**C. Browser hash (question 2, optional, one minute).** Open
`http://10.77.9.2:8080/?hashlog=1&load=20&hashticks=600` and wait about half a minute until the console prints
`# hashlog done`. The line before it reads `# final=<hash> label=load ships=20 seed=42 (browser) ticks=600`. Copy it.
On 12 Oct 2026 the hash was `86eb72a03591bff2`; sim changes since then (the turning of 13 Oct) change it, so
the lead compares it with a native run of the same commit (`docs/client.md` "Load test and determinism in the
browser").

**D. Flight feel (question 7).** Open `http://10.77.9.2:8080/?scene=sandbox` (the sandbox; since 13 Oct 2026 `/`
opens the home screen, where TEST RANGE leads to the same flight) and fly the Halvex Runner for a few
waves: W/S thrust, A/D rotate, Q/E strafe, let go to brake, X for drift, click a raider to lock (Tab for the next
one), hold Space or the right mouse button to fire. Then say for each of these whether it feels right, or what is
wrong: keyboard flight; the lock (it turns your guns onto the target, A/D nudge the aim); braking when you let go;
top speed and acceleration (~22 m/s, 90° in ~1.5 s) with the calm cruise flame at top speed. If you change
anything in the tuning panel (U; T now opens the drydock), copy the query string it prints.

**What to send back:**

- the seven `LOADTEST` lines (A and B), with your browser and its version, the monitor's refresh rate and the GPU;
- the `# final=` line (C);
- the feel answers and any tuning query string (D).

How to read them: the frame time is the time between frames, so with vsync it sits at the refresh interval
(16.7 ms at 60 Hz, 6.9 ms at 144 Hz) whenever a frame fits. If all lines show that, the browser keeps up in every
mode; differences show in the p95 and in the modes that miss the interval. The page is not cross-origin isolated,
so the browser's timer has a grain of about 0.1 ms: for the sim step, use the mean (the median and p95 of a
0.02 ms step read as 0 or 0.1).

## What surprised us

- **Collider layout mattered more than the engine.** One collider per 1×1 cell cost 5–100× more than one compound
  shape per body, in every engine; Avian's idiomatic entity per collider was the slowest setup measured (~27 ms per
  split).
- **Determinism was cheap.** Every engine was deterministic across native, Node and headless Chromium (with its
  determinism switch where it has one); no native-vs-wasm divergence of the sim has been recorded, and moving from
  an Intel to an AMD VM reproduced every hash and counter.
- **The sim is far cheaper than the budget.** 20 fighting ships cost about 11 µs per tick natively, of which Rapier
  is about 6 µs; even 20 ships of 2000 parts stay under 0.5 ms median. The spike plan's 4 ms budget is 100× away
  for the game's current ships, so on the phone the open part is most likely rendering and heat, not the sim.
- **The first play-test changed more than any measurement.** The Runner felt sluggish (90° in 3.3 s), so thrust
  went up about 16× and the turn-rate cap doubled. WASD as a stick, natural for touch, felt weird on a keyboard,
  and a speed cap from a formula felt wrong because a fleeing enemy could never be caught.
- **Small API traps.** Rapier does not correct the velocity when the centre of mass moves (the sim adds
  `ω × Δcom` itself), and its prelude's `vector!` macros produce the wrong types since 0.36.
- **wasm is close to native.** Under V8 the sim takes 1.5–2× the native time, so the browser is a fair
  development platform.
- **The new VM is about 1.7–1.8× faster and much quieter.** The same fight costs 40 % less per tick, `sim_perf`
  about 45 % less, and a parallel 6-job build moves the 20-ship median from 0.011 to 0.012 ms (on the old VM, load
  moved medians by 1.5–2×).

## How the new-VM numbers were measured

12 Oct 2026, 23:35–23:41, `dev` `5b824b6`, own target dir, release builds (`codegen-units 1`, thin LTO).

| What | Command | Runs | Load average (1 min) |
|---|---|---|---|
| Load test | `scripts/load-test-native.sh` (1, 5, 20 ships, 1800 ticks, seed 42) | 15, then 10 during a `cargo test -j 6` build | 0.86–1.55, then 9.1–10.9 |
| `sim_perf` | `cargo run --release -p sim --example sim_perf` (after each load test run) | 15, then 10 | the same |
| Determinism | `scripts/determinism-check.sh --scenario core\|flight\|load\|behavior --seed 42\|7 --ticks 1800` | 1 each (deterministic) | 0.73–0.75 |
| Node vs native | `determinism --scenario load --ships N --ticks 1\|9000` natively and under `scripts/wasi-run.mjs` | 7 | 0.68–1.11 |
| Tests | `cargo test -p sim --release` | 1 | 1.3–10.7 (its own build) |

`uptime` was recorded before and after every run. `sim_perf` on the new VM (medians over 15 runs, ms, mean /
median / p95): raw throttles 0.234 / 0.224 / 0.267, pilot + lock 0.250 / 0.240 / 0.293, combat 0.542 / 0.425 /
1.633, `state_hash` 0.285 mean; the old VM's idle numbers are in `docs/sim.md` "Engine plumes" and the new ones
next to the old ones in `docs/sim.md` "Performance" and "The load test".
