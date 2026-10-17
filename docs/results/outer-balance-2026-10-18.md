# The outer reach's difficulty (18 Oct 2026)

Phase 2 of the outer reach ([GDR-0103](../gdr/0103-the-grown-in-play-defaults.md)) put the Grown in play. GDR-0099 A6:
a Runner-class flagship with one or two escorts and the rim researched can handle the outer reach. This is how the
counts and the rammers' timing were set.

**In short.** A Runner with one cannon skiff clears a grove run 55 %, a guarded grove 74 %, a border drift 75 %, the
bastion's gate 52 % and an elder grove 44 %; a second skiff adds 5–10 points; the Runner alone 40 / 63 / 60 / 26 /
18 %. For scale, the rim (unchanged, run for run as on 18 Oct): the Runner with the plain skiff clears Corran Reach
51 % and the Warden 42 %; at the bastion's gate it clears 42 %.

## Method

The balance harness's voyage mode (`../space-game-balance`, `--voyages --fleets`, the game's headless voyage runner
with the content read at run time), seeds 1–120 per row, clear = goal cleared and home. A cluster has no goal, so a
copy of the pack carried five test voyages built from the outer sites (radius 900–1000 m, sites 400–700 m apart),
never in the game:

| Test voyage | Sites (the last the goal) |
|---|---|
| t_grove | grove, grove_pair, grown_dead, grove |
| t_guarded | grove, grown_dead, guarded_grove |
| t_border | rim_stragglers, grown_dead, grove_pair, rim_stragglers |
| t_gate | grove_pair, bastion_gate |
| t_elder | grove_pair, elder_grove |

Fleets: the Runner (`halvex_runner`), + `rim_skiff` (S), + one or two `skiff_cannon` (C: the starter with
`reverse_engineered_cannon` for its laser, as on 18 Oct: the rim researched). Hot stars are not in these runs (a
voyage has no star); `scorched_deep` holds the same sites as a grove reach. Load average about 2.

The runner's pilot fights only awake enemies, and a wary grove never wakes on sight, so it now opens fire on the
nearest armed ship of a wary site it heads for (as a player picks the fight). No rim site is wary: the rim's runs
are unchanged.

## What was tried (t_grove / t_guarded, Runner + C, 60 seeds)

| Rammers | Groves | Clear % |
|---|---|---|
| charge 4 s of every 7 within 80 m, circle between | grove 1–2 rammers + a thornback | 12 / 23 |
| the same, no ram at all | the same | 65 / 75 |
| charge 2 s of every 9 | grove 1 rammer + 0–1 thornbacks, guarded grove 1 rammer | 58 / 72 |

The last looked right but its charges never connected: out of an orbit, a charge (nose on the target, stern thrust
only) cannot take out the orbit's sideways speed, so it slid past at 9 m or hit with its thrusters. The rammers now
close in to thorn range facing the target (`approach`) and charge from within 45 m, and the prow sits out in front
(with the thorn guns beside it, a charge off the centre line touched with a thorn gun). Then every charge lands
(a prow ram at top speed takes some 14 hit points off a Runner):

| Charge window (rammer / guardian) | t_grove | t_guarded | t_gate | t_elder (2 rammers) |
|---|---|---|---|---|
| first 3 s of every 8 / 12 | 33 | 37 | 38 | 8 |
| first 3 s of every 12 / 15 | 30 | 32 | 33 | 3 |
| last 3 s of every 10 / 12 | **55** | **75** | 47 | 13 |
| last 3 s of every 12 / 15 | 45 | 73 | 53 | 23 |

The period hardly matters while the first charge comes at once; the first charge 7 s after the target is picked (the
thorns cover the approach) is the knob. The bastion with a gunner alone cleared 65 %, so its gate gained a hunter
(39 % in that pass); the elder grove's second rammer went (13 → 44 %).

## Result (seeds 1–120; clear / home / lost / out of time)

| Voyage | Runner | Runner + S | Runner + C | Runner + C + C |
|---|---|---|---|---|
| t_grove | 48 / 52 / 17 / 3 (40 %) | 61 / 30 / 28 / 1 (51 %) | 66 / 29 / 22 / 3 (**55 %**) | 72 / 29 / 17 / 2 (60 %) |
| t_guarded | 76 / 28 / 14 / 2 (63 %) | 92 / 13 / 14 / 1 (77 %) | 89 / 14 / 16 / 1 (**74 %**) | 100 / 8 / 10 / 2 (83 %) |
| t_border | 72 / 43 / 4 / 1 (60 %) | 84 / 30 / 6 / 0 (70 %) | 90 / 22 / 7 / 1 (**75 %**) | 99 / 14 / 6 / 1 (82 %) |
| t_gate | 31 / 84 / 5 / 0 (26 %) | 51 / 58 / 11 / 0 (42 %) | 62 / 44 / 12 / 2 (**52 %**) | 71 / 41 / 8 / 0 (59 %) |
| t_elder | 22 / 62 / 36 / 0 (18 %) | 48 / 42 / 30 / 0 (40 %) | 53 / 36 / 31 / 0 (**44 %**) | 65 / 37 / 17 / 1 (54 %) |

Seeds 121–240, Runner + C: t_grove 54 %, t_gate 50 %. "Home" is mostly the pilot disarmed (thornbacks aim at guns)
or giving up after 3 minutes without progress. A grove run is about as hard as Corran Reach, the gate as the Warden.

## Reproduce

```sh
cd ../space-game-balance && cargo build --release
cp -r ../space-game/content/base /tmp/pack   # plus the five test voyages above as voyages/zz_outer.ron
target/release/space-game-balance --content /tmp/pack --voyages t_grove,t_guarded,t_border,t_gate,t_elder \
  --fleets halvex_runner,halvex_runner+rim_skiff,halvex_runner+skiff_cannon.ron,halvex_runner+skiff_cannon.ron+skiff_cannon.ron \
  --seeds 1-120 --threads 12
```

`VOYAGE_DETAIL=1` prints each run (outcome, time, hull lost, sites, kills).
