# Sidegrade check, 18 Oct 2026

A cheap sanity check of the sidegrade pack of 18 Oct (GDR-0024's variants, GDR-0018's ammo feeder): is any new part
strictly better than the part it sits beside? Two were. The wide-bubble emitter and the ammo feeder were cut back.
The other parts trade.

## Method

The balance harness's voyage mode (`../space-game-balance`, `--content content/base --voyages yard_approach,
halvex_drift,corran_reach,wardens_gate --seeds 1-240 --threads 4`, release build against `38eb947`). The fleets are
blueprints: `ships/rim_skiff.ron` (and `halvex_runner.ron` for the shield, reactor and radiator) with one part swapped
throughout. The cannon designs are the skiff with `reverse_engineered_cannon` in place of `pulse_laser`. In the
feeder skiff its capacitors (which don't feed cannons) become ammo feeders. The long-cannon skiff has two
`long_cannon` (1×2) over the gun and capacitor cells. The heat sink replaces the outer radiator pair and the empty
cell above each, and the skiff with two radiators there is its baseline. The table gives the clear % (goal cleared
and home) of 240 runs. ± is about 6 points (95 %).

## Results (clear %: yard_approach / halvex_drift / corran_reach / wardens_gate)

| Part | Baseline | Before | Verdict |
|---|---|---|---|
| wide_shield_emitter (skiff) | rim_skiff 56 / 31 / 18 / 15 | 85 / 60 / 48 / 18 | **dominated**, changed |
| wide_shield_emitter (Runner) | halvex_runner 64 / 58 / 34 / 25 | 86 / 75 / 58 / 53 | **dominated**, changed |
| burst_laser (skiff) | rim_skiff 56 / 31 / 18 / 15 | 0 / 0 / 0 / 0 | worse (see below) |
| long_cannon (skiff) | cannon + feeder 85 / 70 / 47 / 38 | 83 / 71 / 46 / 31 | sidegrade (same cells, more losses) |
| ammo_feeder (cannon skiff) | cannon skiff 87 / 61 / 30 / 28 | 95 / 72 / 53 / 41 | **too strong**, changed |
| heat_sink (skiff) | two radiators 59 / 34 / 28 / 19 | 61 / 27 / 23 / 17 | sidegrade (1 bp cheaper) |
| compact_reactor (skiff) | rim_skiff 56 / 31 / 18 / 15 | 60 / 34 / 20 / 11 | sidegrade (lost 2 vs 32 of 240 on the yard) |
| compact_reactor (Runner) | halvex_runner 64 / 58 / 34 / 25 | 67 / 55 / 37 / 26 | sidegrade |
| spore_radiator (skiff) | rim_skiff 56 / 31 / 18 / 15 | 64 / 38 / 26 / 21 | slight edge (+1 bp each, Grown vat, spores) |
| spore_radiator (Runner) | halvex_runner 64 / 58 / 34 / 25 | 64 / 59 / 35 / 25 | level |

- **Wide-bubble emitter.** It was radius 3.4, capacity 2.4, regen 0.12. The radius does it: capacity 2.4 → 1.2 cost
  the Runner only 1–6 points, but at radius 3.0 its gain drops from 22–28 points to 3–11 (its bubbles at 3.4 reach
  the nose). Now **radius 3.0, capacity 1.6, regen 0.08**: skiff 79 / 43 / 30 / 11, Runner 75 / 62 / 37 / 29. It
  helps on the easy voyages, it is level or worse at the Warden and it loses more ships (49 against 34 lost at the
  Warden on the skiff). The skiff's own bubbles leave its nose bare, so any wider bubble helps it early.
- **Ammo feeder.** At +20 % per feeder the cannon starter cleared the Warden 41 %, past the 37 % it had before
  GDR-0100's cut to 28 %. A capacitor per pulse laser adds about 6 points (skiff without capacitors
  54 / 25 / 18 / 9). Now **+15 %, at most two** (the capacitor's step, one stack fewer, a bigger cook-off):
  85 / 70 / 47 / 38. +10 % gave 83 / 67 / 42 / 40, the same within the interval: a cannon's rate counts this much.
- **Burst laser** (not changed, for Niklas). It cleared none of 960 voyage runs, with no kills: raiders orbit beyond
  its 40 m. Against one hunter it wins like the pulse laser (40 of 40), against two raiders 26 of 40 (pulse 40 of 40).
  At 50 m it clears 27 / 19 on the first two voyages. It doesn't dominate, but it is a trap as it stands.

## Not measured

Player piloting (the runner's closing pilot only), the heat sink's and spore radiator's value on hot builds (beam
ships), and the Grown voyages.

## Screenshots (1280×720, a test save)

- `sidegrades-research-1280.png`: the research screen with the new nodes (compact reactor, burst laser, long-barrel
  cannon, wide bubble selected, heat sink).
- `sidegrades-ammo-feeder-card-1280.png`: the drydock's hover card of the ammo feeder (+15 %, at most two).

## Burst laser follow-up (18 Oct 2026)

The same voyage mode (`--seeds 1-240 --threads 4`, the skiff with both pulse lasers swapped, `--content` a copy of
the pack with the range patched), the burst laser's range only (damage 1.3 per second at rate 7, a pulse 0.19 against
the pulse laser's 0.25):

| Range | Clear % (yard / halvex / corran / warden) | Lost (of 240, yard) |
|---|---|---|
| 40 (before) | 0 / 0 / 0 / 0 | |
| 48 | 23 / 20 / 6 / 5 | 38 |
| **52** (chosen) | 44 / 31 / 14 / 16 | 31 |
| 55 | 50 / 35 / 19 / 16 | 13 |
| rim_skiff (pulse, 60) | 56 / 31 / 18 / 15 | 32 |

Now **range 52**: below the pulse laser on the easy voyages, level at the drift and the Warden (within ± 6), so it
trades. At 55 it was level everywhere and lost far fewer ships, which is too close to strictly better.
