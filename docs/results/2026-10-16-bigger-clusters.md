# Bigger clusters: step cost, `f32` at 3 km, spawn hitches (galaxy track E0, 16 Oct 2026)

Track E0 of `docs/plans/galaxy-voyages.md`: before E3 (sensor range, frozen sites, hidden sites, scans) is
final, what a cluster of 2–3 km radius ([GDR-0080](../gdr/0080-exploration-searching-and-story.md)) with 20–40
awake ships and about 200 sleeping ships and wrecks costs, how `f32` holds 3 km from the origin, and what spawning
or waking ships mid-fight costs. Measured natively and as wasm32-wasip1 under Node 24 (V8, the browser's engine;
the sim's browser cost, as in `spike-summary.md` question 5).

**In short.**

- **Cost**: a whole 2.5 km cluster spawned and asleep (60 sites, 103 ships and 99 wrecks) adds about
  **0.12–0.15 ms per tick natively and 0.16–0.21 ms as wasm** to a fight of 20–40 ships (0.015–0.03 ms on its
  own); a 3 km cluster with 375 sleeping ships and wrecks adds 0.28 / 0.48 ms. That is 0.6–1.3 µs per sleeping
  body, although Rapier puts nearly all of them to sleep. Sites kept as records (not spawned) cost nothing measurable.
- **`f32` at 3 km**: the `f32` step there is 0.24 mm. Steered flight, fights and rams play as at the origin
  (within the spread of starting spots), and every hash is identical native vs wasm at 0, 3, 8 and 30 km. The only
  effect is in **coasting at very low speed**: Rapier moves a body in whole `f32` steps per substep, so at 3 km a
  body drifting at 1.2 cm/s does not move, 5 cm/s covers 17 % too little, 0.2 m/s 6 %, 1 m/s 0.7 %. At 8 km slow
  drift is off by up to 66 %; at 30 km a holding ship freezes in place.
- **Hitches**: spawning a site (1–6 ships) costs 0.01–0.05 ms plus 0.02–0.04 ms on the next step; 24 ships at once
  stay under 0.25 ms. Waking and removing ships cost nothing to speak of. Spawning a whole cluster in one tick
  costs 1.5–2.3 ms natively and 1.3–2.5 ms as wasm (200 bodies), 3.9 / 5.2 ms (375).
- **For E3**: keep one `f32` space per cluster centred on the world origin (no re-origin, no `f64`, no local
  frames) with a radius of at most 4 km. Freezing is not needed for the step cost: keep lazy spawning (records
  until the fleet comes near) and let spawned sites sleep; no despawning in E3. Details in
  [Recommendation for E3](#recommendation-for-e3).

## The bench

`crates/core/sim/examples/cluster.rs`, run by `scripts/cluster-bench.sh [--wasm] [args]` (native, and with `--wasm`
also under `scripts/wasi-run.mjs`, then a check that every row's state hash matches). Three sections:

- **cost**: the shared load-test fight (`sim::voyage::scenario::load_test`, half Halvex Runners and half automaton
  raiders under `raider_basic`; the player's mind capacity raised so all 20 runners of a 40-ship fight run) moved to
  the entry of a voyage rolled with `VoyagePlan::roll` on a cluster of `--radius` metres with the base encounters
  of `--sites` (default: the slice's encounters, 60 sites). Three ways to hold the sites:
  - `none`: no voyage, the fight alone;
  - `records`: the voyage director runs but nothing spawns (frozen sites kept as records);
  - `asleep`: every site spawned on the first tick and asleep (the director's sleeping sites: their ships hold,
    their bodies come to rest and Rapier puts them to sleep).
  Columns: the first tick (where `asleep` spawns the whole cluster), the step's mean, median, p95 and max, the
  phases' means (`Sim::step_profiled`), the bodies asleep at the end and the sites the fight woke.
- **hitch**: mid-fight (20 ships, tick 300), k raiders spawned 600 m off as a site spawns them (holding), woken
  90 ticks later (`raider_basic`), removed (frozen back to a record) 90 ticks after that.
- **precision**: at 0, 3, 8 and 30 km from the origin (along (0.6, 0.8)): a raider holding where it spawned after a
  push at 2 m/s (the spread of its position over the last 10 s), raiders coasting with nothing commanded
  (distance covered in 10 s against speed × 10 s), a head-on ram (one raider at 20 m/s into another at rest), and
  the load-test fight from five spots 10 m apart (the fight ignores its seed, so the spots give its spread).

`--repeat 7`: each timed case run seven times; times are the median (min–max) over the runs. Release builds,
`dev` at `f9e4275` plus the bench, 16 Oct 2026, on the current VM (4 cores), load average 1.0–2.2.

## Step cost

Milliseconds per tick, median (min–max) over 7 runs of 1800 ticks.

| Cluster | Awake | Build | none: median / p95 | records: median / p95 | asleep: median / p95 | asleep: first tick |
|---|---|---|---|---|---|---|
| 2.5 km, 60 sites, 103 ships + 99 wrecks | 20 | native | 0.015 (0.013–0.018) / 0.045 | 0.018 (0.017–0.018) / 0.050 | **0.163** (0.113–0.169) / 0.270 | 2.26 (1.42–3.44) |
| | | wasm | 0.027 (0.018–0.047) / 0.071 | 0.019 (0.017–0.029) / 0.065 | **0.180** (0.159–0.210) / 0.333 | 2.54 (1.88–2.99) |
| | 40 | native | 0.023 (0.019–0.029) / 0.071 | 0.027 (0.020–0.034) / 0.081 | **0.139** (0.125–0.164) / 0.260 | 1.53 (1.39–2.41) |
| | | wasm | 0.029 (0.028–0.040) / 0.102 | 0.045 (0.039–0.059) / 0.141 | **0.240** (0.224–0.247) / 0.450 | 2.07 (2.02–2.47) |
| 2.5 km, 66 raider packs, 198 ships | 40 | native | 0.021 (0.019–0.027) / 0.063 | 0.030 (0.025–0.033) / 0.092 | **0.168** (0.144–0.225) / 0.290 | 1.47 (0.88–1.75) |
| | | wasm | 0.041 (0.030–0.080) / 0.143 | 0.034 (0.032–0.049) / 0.108 | **0.257** (0.233–0.311) / 0.424 | 1.31 (1.14–3.93) |
| 3 km, 120 sites, 193 ships + 182 wrecks | 40 | native | 0.020 (0.020–0.023) / 0.066 | 0.018 (0.018–0.026) / 0.070 | **0.300** (0.293–0.388) / 0.557 | 3.89 (2.97–5.25) |
| | | wasm | 0.037 (0.032–0.063) / 0.117 | 0.036 (0.029–0.052) / 0.123 | **0.513** (0.452–0.579) / 0.874 | 5.18 (3.62–7.77) |

Where the sleeping cluster's cost goes (2.5 km, 40 awake, native, phase means in ms, `none` → `asleep`): flight
(behaviors and the flight computer) 0.006 → 0.027, the ships' systems 0.007 → 0.033, weapons 0.003 → 0.040, Rapier
0.017 → 0.048, impacts and the director 0.001 → 0.010. Wasm shows the same split at 1.4–1.9×. Every phase walks all
ships, sleeping or not; the awake ships' behaviors look at every ship for targets (sensor range, E3, bounds that).

- Rapier sleeps 268 of 303 bodies at the end (the rest are the fight's); its step still grows with the sleeping
  bodies (broad phase), but stays under 0.1 ms native.
- The fight woke 4–8 of the 60–66 sites (its runners chased sleepers near the entry): the awake count in `asleep`
  is the fight plus those.
- The max column (not shown) was 0.2–11 ms in every mode, `none` included, and moved from run to run of the same
  deterministic world: the VM's preemption. The only ticks slow in every run are the first (the spawn) and the
  second (the new bodies' first step, 0.5–0.7 ms native).
- `records` costs the director's distance tests, 0.001–0.002 ms. (Starting the voyage makes the fight play out a
  little differently, so its counts differ from `none`.)

## Hitches

20 ships fighting; milliseconds, median (min–max) over 7 runs; native / wasm. The fight's median step around it:
0.027–0.035 ms native, 0.035–0.055 ms wasm.

| k ships | Spawn calls | Next step | Wake: max of the next 3 steps | Remove calls | Next step |
|---|---|---|---|---|---|
| 1 | 0.014 / 0.027 | 0.043 / 0.088 | 0.031 / 0.053 | 0.004 / 0.007 | 0.027 / 0.072 |
| 3 | 0.028 / 0.037 | 0.059 / 0.069 | 0.034 / 0.068 | 0.010 / 0.010 | 0.040 / 0.045 |
| 6 | 0.041 / 0.053 | 0.065 / 0.070 | 0.040 / 0.044 | 0.013 / 0.016 | 0.045 / 0.047 |
| 12 | 0.071 / 0.069 | 0.072 / 0.109 | 0.053 / 0.053 | 0.022 / 0.027 | 0.052 / 0.054 |
| 24 | 0.135 / 0.116 | 0.095 / 0.125 | 0.047 / 0.067 | 0.034 / 0.042 | 0.046 / 0.050 |

Compiling a design (`ShipDesign::compile`, mean of 20): raider and hunter 0.006–0.009 ms, Warden and `sample_mid`
0.015–0.029 ms, native and wasm alike. The ranges of single runs reach 0.1–0.3 ms (noise).

## `f32` far from the origin

Identical on native and wasm (the row hashes match). Fight: mean (min–max) over five spots 10 m apart, 900 ticks.

| Offset | `f32` step | Hold: jitter / max speed | Coast 0.012 / 0.05 / 0.2 / 1 m/s: moved ÷ expected | Ram: impulse / speeds after (m/s) | Fight: shots / hits / impacts / splits |
|---|---|---|---|---|---|
| 0 | — | 33.7 mm / 42 mm/s | 0.200 / 1.000 / 1.000 / 1.000 | 202.2 / 7.217, 12.790 | 119 (92–162) / 686 (664–698) / 129 (70–296) / 1.2 (0–2) |
| 3 km | 0.24 mm | 36.3 mm / 47 mm/s | 0.000 / 0.829 / 0.938 / 0.993 | 200.3 / 7.335, 12.670 | 111 (107–116) / 725 (697–741) / 150 (84–287) / 1.2 (1–2) |
| 8 km | 0.49 mm | 42.9 mm / 52 mm/s | 0.000 / 1.657 / 1.056 / 1.008 | 197.5 / 7.505, 12.498 | 115 (99–135) / 686 (637–706) / 151 (138–178) / 1.8 (0–4) |
| 30 km | 1.95 mm | 0 / 104 mm/s (stuck) | 0.000 / 0.000 / 1.657 / 0.994 | 197.1 / 7.529, 12.474 | 120 (116–123) / 707 (642–759) / 219 (186–275) / 1.4 (0–3) |

- **Steered ships**: the hold controller's own limit cycle (34 mm at the origin) dwarfs the `f32` step at 3 km;
  closed-loop flight corrects position error, so it does not drift. At 30 km the ship can no longer move by
  less than a step per substep and sits still while the controller pushes.
- **Coasting**: Rapier integrates the position in 4 substeps of 1/120 s, each rounded to the `f32` grid, so a
  coasting body's speed comes out quantized per axis (the 0.05 m/s case at 8 km rounds both axes up to one step:
  1.66×). At 3 km anything slower than about 1.5 cm/s stands still (the sim's sleep threshold is 1 cm/s; at the
  origin the 1.2 cm/s case falls asleep after 2 s, hence 0.2), up to 0.2 m/s is off by 6–17 %, 1 m/s by 0.7 %.
  Velocity itself is not affected. Slow drift is wreckage and debris settling (GDR-0089), where a few percent do
  not show.
- **Collisions**: the ram's impulse and rebound speeds are within 1–2 % of the origin's at 3–30 km (the contact
  happens a hair earlier or later on the grid). The fights' counts at 3 and 8 km lie within the spread of the
  starting spots; at 30 km impacts run higher at every spot (186–275 against 70–296 at the origin with a mean of
  129), the first sign of contact jitter.
- **Determinism**: all 15 rows (the cost rows with the fight 2.5–3 km from the origin, the precision rows at 0–30
  km) hash identically native vs wasm.

## Recommendation for E3

1. **One `f32` space per cluster, centred on the world origin, radius at most 4 km.** `VoyagePlan` already
   centres a cluster's sites on the origin with the entry at its edge. At 2–3 km nothing needs re-origining,
   `f64` or local frames. Above 4096 m the `f32` step doubles; by 8 km slow drift is visibly wrong. Ships flying
   well past the edge drift into that range, so E3 should keep them near the cluster (an edge that turns them back,
   or leaving the cluster as the recall: the lead's or Niklas's choice). Clusters beyond 4 km would need Rapier's
   `f64` build or per-area local frames; measure again with this bench first (`--radius`, the precision offsets).
2. **No despawning for cost.** A fully spawned, sleeping 2–3 km cluster costs 0.15–0.5 ms per tick in the browser
   (wasm), about 1–3 % of a 60 fps frame; natively half that. Keep the director's lazy spawning (sites are records
   until the fleet is within `spawn_radius`) and let spawned sites stay asleep; freezing back to records with
   hysteresis can wait until a cluster passes about 1000 bodies, or until the client's per-ship cost asks for it.
   If the step ever needs it, the cheap fast path is to skip flight, systems and weapons for the ships of sleeping
   sites whose bodies sleep (about 70 % of the sleeping overhead).
3. **Spawn per site, never the whole cluster mid-flight.** A site's spawn costs well under a tenth of a
   millisecond; the whole cluster at once is 2–5 ms in wasm, which belongs behind the warp transition if E3 wants
   it (for example to give scans a full set of bodies to hit).
4. **Sensor range bounds the rest.** The flight phase (behaviors choosing targets among every ship) grows with the
   ships in the world; GDR-0017's sensor range, planned for E3, caps it.
5. **Not measured here: the client.** Drawing and interpolating ~250 ship views, their markers and the HUD per frame
   is the client's cost, not the sim's; headless Chromium renders with SwiftShader and cannot time it. When E3 has
   a big cluster to fly, check it in Niklas's browser (a big-cluster scene next to `?load=N` would make that a
   one-line check).

## Reproduce

```
scripts/cluster-bench.sh --wasm --repeat 7                                   # the default cluster, all sections
scripts/cluster-bench.sh --wasm --section cost --repeat 7 --awake 40 --sites raider_pack:66
scripts/cluster-bench.sh --wasm --section cost --repeat 7 --awake 40 --radius 3000 \
  --sites raider_den:20,raider_pack:24,hunter_pack:16,hunter_den:8,derelict_field:20,core_graveyard:20,precursor_wrecks:12
```

Tables go to `target/cluster-bench/{native,wasm}.txt`. On the 4-core VM a full run takes well under a minute after the builds.
