# Arena report, 20 Oct 2026: the content's ships one against one

The first ratings and part report (coop-arena track A4, [GDR-0124](../gdr/0124-ratings-and-part-reports.md),
`docs/arena.md` "Ratings and part reports"). Nothing in the content was changed for or after it.

```sh
cargo run --release -p arena-cli -- report --markdown docs/results/arena-report-2026-10-20.md   # then this reading added
```

- **Pool** (`crates/core/arena/report/pool.ron`): the 15 content ships that fly one against one (no bosses,
  stations or drones), each under its own behavior; every pair on seeds 1–16 in each starting position, 1v1 at
  100 m, 90 s: 3360 fights in **5.1 s** on this VM's 4 threads (655 fights/s, release; load average 2.0).
  `dev` at `d4f2e9f`, rules fingerprint `796d18739bf987cb`.
- **Re-roll** (`--seed 1001`, 3360 other fights): the same top eight in nearly the same order (5 and 6 swap, the
  automatons reshuffle inside their 30-point band), ratings within 52 points (the carrier 1866 → 1918), and the
  same flags on the same parts. So the order and the flags hold; gaps under about 50 points do not.

## The report

15 designs, 3360 fights (mirror matches left out); ratings: Bradley–Terry on the Elo scale (mean 1500, +400 = odds of 10 to 1); top designs: the best 8 (50 %).

| # | design | rating | bp | fights | won | score | top |
|---|---|---|---|---|---|---|---|
| 1 | swarm_carrier | 1866 | 59 | 448 | 85 % | 0.88 | yes |
| 2 | halvex_runner | 1859 | 132 | 448 | 81 % | 0.88 | yes |
| 3 | rim_skiff | 1668 | 60 | 448 | 60 % | 0.71 | yes |
| 4 | grown_guardian | 1613 | 61 | 448 | 30 % | 0.65 | yes |
| 5 | grown_thornback | 1581 | 38 | 448 | 54 % | 0.61 | yes |
| 6 | sample_mid | 1575 | 112 | 448 | 48 % | 0.61 | yes |
| 7 | grown_rammer | 1552 | 31 | 448 | 27 % | 0.58 | yes |
| 8 | automaton_picket | 1406 | 17 | 448 | 0 % | 0.39 | yes |
| 9 | sample_early | 1388 | 50 | 448 | 26 % | 0.37 |  |
| 10 | automaton_hunter | 1361 | 24 | 448 | 24 % | 0.34 |  |
| 11 | automaton_raider | 1346 | 16 | 448 | 25 % | 0.32 |  |
| 12 | automaton_gunner | 1342 | 21 | 448 | 25 % | 0.31 |  |
| 13 | automaton_lancer | 1337 | 19 | 448 | 19 % | 0.31 |  |
| 14 | swarm_spitter | 1316 | 25 | 448 | 20 % | 0.28 |  |
| 15 | grown_seedling | 1289 | 12 | 448 | 0 % | 0.25 |  |

Parts: pick rate among the top designs; win rate and mean score of the fights of the designs that carry it. DOMINANT: picked by ≥ 50 % and scoring ≥ 0.60; UNDERUSED: carried, but picked by < 20 % (*placeholders*).

| part | category | designs | pick rate | win rate when used | score when used | flag |
|---|---|---|---|---|---|---|
| thruster | propulsion | 14 | 100 % | 36 % | 0.52 |  |
| armor_block | hull | 13 | 88 % | 41 % | 0.53 |  |
| slope_1x1 | hull | 13 | 88 % | 38 % | 0.51 |  |
| cargo_bay | cargo | 4 | 38 % | 54 % | 0.64 |  |
| shield_emitter | defense | 4 | 38 % | 54 % | 0.64 |  |
| radiator | heat | 4 | 38 % | 54 % | 0.64 |  |
| logic_core | logic | 4 | 38 % | 54 % | 0.64 |  |
| fission_reactor | power | 4 | 38 % | 54 % | 0.64 |  |
| capacitor | power | 4 | 38 % | 54 % | 0.64 |  |
| pulse_laser | weapons | 4 | 38 % | 54 % | 0.64 |  |
| grown_heart | defense | 3 | 38 % | 37 % | 0.61 |  |
| thorn_gun | weapons | 3 | 38 % | 37 % | 0.61 |  |
| grown_ganglion | logic | 4 | 38 % | 28 % | 0.52 |  |
| gyro | propulsion | 2 | 25 % | 71 % | 0.80 |  |
| slope_1x2 | hull | 2 | 25 % | 65 % | 0.74 |  |
| fusion_reactor | power | 2 | 25 % | 65 % | 0.74 |  |
| torch_drive | propulsion | 2 | 25 % | 65 % | 0.74 |  |
| beam_laser | weapons | 2 | 25 % | 65 % | 0.74 |  |
| scanner | logic | 2 | 25 % | 30 % | 0.55 |  |
| ram_prow | weapons | 3 | 25 % | 19 % | 0.49 |  |
| brood_hangar | weapons | 1 | 12 % | 85 % | 0.88 | UNDERUSED |
| slope_1x3 | hull | 1 | 12 % | 48 % | 0.61 | UNDERUSED |
| swarm_node | logic | 2 | 12 % | 53 % | 0.58 | UNDERUSED |
| stinger | weapons | 2 | 12 % | 53 % | 0.58 | UNDERUSED |
| rcs_pod | propulsion | 1 | 12 % | 0 % | 0.39 | UNDERUSED |
| automaton_core | logic | 5 | 12 % | 19 % | 0.33 | UNDERUSED |
| automaton_cannon | weapons | 3 | 0 % | 25 % | 0.32 | UNDERUSED |
| rocket_pod | weapons | 1 | 0 % | 19 % | 0.31 | UNDERUSED |
| swarm_jet | propulsion | 1 | 0 % | 20 % | 0.28 | UNDERUSED |

NEVER PICKED (15): prism_ward, cargo_hold, wide_shield_emitter, spore_radiator, heat_sink, mining_laser, torpedo_launcher, point_defence, flare_launcher, compact_reactor, drone_bay, reverse_engineered_cannon, burst_laser, long_cannon, ammo_feeder

## Reading

- **No part is DOMINANT** under the placeholder thresholds. The strongest parts by score when used are the Swarm
  carrier's `brood_hangar` (0.88) and the kit of the Runner and the starter (`gyro`, `beam_laser`, `fusion_reactor`, `torch_drive`:
  0.74–0.80), but each sits in one or two designs, so their pick rate stays at 12–25 %.
- **The ratings follow the factions more than the parts.** The pool's parts are mostly faction-bound, so a part's
  numbers are its faction's: every automaton ship sits at 1337–1406, and with them `automaton_core` and
  `automaton_cannon` (score when used 0.31–0.33) are flagged UNDERUSED. That is the automatons being the early
  fodder they are meant to be, not two weak parts. A part report that speaks about parts needs designs drawn from
  one palette: the best elites of an evolution run (`arena report --run <run file>`), which this report does not have
  yet.
- **The Swarm carrier rates first at 59 build points**, level with the 132-point Runner (85 % against 81 % won).
  Its drones come from the `brood_hangar`; whether the drones it launches should count toward its build points is a
  question for the drone records (GDR-0117), not something this report settles.
- **Score and wins part ways for the stayers**: `grown_guardian` wins 30 % but scores 0.65 (it outlasts at the time
  limit), and `automaton_picket` wins nothing yet ranks eighth (0.39: a scout that keeps away). Bradley–Terry rates by
  score, so a design that survives to the time limit with the upper hand rates well without winning.
- **Build points buy less on the ship-mind side**: `sample_mid` (112 bp) rates below the starter `rim_skiff`
  (60 bp), and `sample_early` (50 bp) sits among the automatons.
- **NEVER PICKED (15)**: the researched parts no content ship carries (burst laser, long cannon, torpedo, point
  defence, flares, heat sink, the wide shield, the prism ward and others). The report says nothing about their
  strength; they need designs that carry them in the pool (hand-made or evolved).
- `ram_prow` (the rammer, the seedling and the guardian) scores 0.49 when used: neutral in 1v1 duels at 100 m.
