# Refinement chains check, 18 Oct 2026

The first refinement chains (GDR-0024's "refinement": `Unlock::PartStat`, `research/tuning.ron`) on the rim voyages:
is a full chain a modest gain, a few points rather than a new part? It is. One step size was cut (pulse cycling,
5 % → 3 % a step).

## Method

The balance harness's voyage mode (`../space-game-balance`, `--content content/base --voyages yard_approach,
halvex_drift,corran_reach,wardens_gate --fleets rim_skiff,halvex_runner --seeds 1-240 --threads 4`, release build).
Its new variant key `research=<node>` gives the player that node and every node it requires, and builds the fleet with
their part-stat steps (`ShipDesign::compile_upgraded`, `design_report_upgraded`), as the game and the lab do; the
enemies keep the content's numbers. `research=pulse_rate_3` is the whole chain I–III. The table gives the clear % (goal
cleared and home) of 240 runs; ± is about 6 points (95 %).

## Results (clear %: yard_approach / halvex_drift / corran_reach / wardens_gate)

| Chain (I–III) | Steps | rim_skiff | halvex_runner |
|---|---|---|---|
| none (base) | | 56 / 31 / 18 / 15 | 64 / 58 / 34 / 25 |
| Bubble recharge | +8 % shield regen ×3 | 56 / 31 / 17 / 16 | 64 / 58 / 34 / 26 |
| Fin tuning | +8 % venting ×3 | 56 / 31 / 18 / 15 | 64 / 58 / 34 / 25 |
| Nozzle tuning | +4 % thruster rated speed ×3 | 65 / 30 / 25 / 12 | 64 / 56 / 32 / 25 |
| Pulse cycling | +3 % pulse-laser fire rate ×3 | 66 / 44 / 26 / 18 | 64 / 58 / 29 / 25 |
| all four | | 68 / 44 / 30 / 25 | 68 / 56 / 29 / 26 |

- **Pulse cycling** is the one that counts: +3 to +13 points on the skiff (two pulse lasers are its guns), level on
  the Runner (its beams carry it). At **5 %** a step it was 68 / 46 / 36 / 28 (+12 to +18), more than a capacitor per
  laser (about 6 points, docs/results/sidegrades-2026-10-18.md); at 2 % 70 / 36 / 28 / 24. A ship's clear rate moves in
  jumps with its rate of fire (a raider dies a pulse sooner or later), so 2 % and 3 % land about level. Now **3 %**.
- **Nozzle tuning** trades: faster on the easy voyages (+7 to +9), level or a little worse where the closing pilot
  overshoots (the drift, the Warden). Within the interval on the Runner.
- **Bubble recharge** and **Fin tuning** do nothing measurable here: no rim fight waits on a bubble's refill (the
  regen alone up to +48 % was still level), and these ships never run their radiators to their limit (every run came
  out the same). Both are for later ships (beam builds, bigger bubbles); measured again when there are some.
- **All four**: +10 to +13 on the skiff, -5 to +4 on the Runner. A career that bought all twelve nodes (about 70 data
  and 36 materials) flies a few points better, never a new class of ship.

## Not measured

Player piloting, hot builds (beams, torch drives) where the venting chain should matter, and the Grown voyages.
