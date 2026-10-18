# Encounters of the content batch of 18 Oct 2026

The content batch's new encounters, measured with the balance harness's voyage mode (`../space-game-balance`,
`--voyages --fleets`, the game's headless voyage runner with the content read at run time) as in
[the outer reach's balance](outer-balance-2026-10-18.md): seeds 1–120 per row, clear = goal cleared and home, the
fleets the Runner (`halvex_runner`) and the Runner with one cannon skiff (C: `rim_skiff` with
`reverse_engineered_cannon` for its lasers, the rim researched). A copy of the pack carried test voyages built from
the sites (radius 950 m, sites 400–700 m apart), never in the game. Load average about 3.

## Seedling shoal (`seedling_shoal`, `grown_seedling`)

| Test voyage | Sites (the last the goal) |
|---|---|
| t_shoal1 | seedling_shoal |
| t_shoal | grove, grown_dead, seedling_shoal |
| t_grove | grove, grove_pair, grown_dead, grove (the outer reach's, for scale) |

A seedling has no gun, so under GDR-0068's old rule a shoal cleared the moment it woke and the runner's pilot never
fought it: a ram prow and a working engine now arm a ship ([GDR-0109](../gdr/0109-a-ram-prow-and-an-engine-arm-a-ship.md)).
Under the `grown_rammer` behavior a seedling never charged: `approach(80%)` of a weapon reach it does not have held
it at 48 m, outside the charge's 45 m. Its own behavior closes in to 30 m and charges from 60 m:

| Seedling behavior (charge window, range / approach) | t_shoal1 Runner | t_shoal1 Runner + C | t_shoal Runner | t_shoal Runner + C | Runner hull lost, t_shoal1 (mean of 72) |
|---|---|---|---|---|---|
| last 4 s of every 8, 50 m / 35 m | 98 | 100 | 75 | 90 | — |
| last 5 s of every 8, 50 m / 20 m | 95 | 100 | 72 | 91 | 6.1 |
| last 7 s of every 10, 80 m / 30 m | 93 | 100 | 73 | 91 | 7.2 |
| **last 4 s of every 6, 60 m / 30 m** | **92** | **100** | **72** | **85** | **10.8** |

The shoal is a light fight on its own: a seedling's one thruster (1 hit point) is soon shot off, and then it drifts,
disarmed. What it adds is a run of rams from every side (a tenth of the Runner's hull on average, up to a third) on
top of the grove before it: t_shoal (grove + shoal) is easier than t_grove (two groves). A ship with the rim
researched handles it.

With GDR-0109 a rammer or guardian whose thorn guns are gone stays armed while its prow and thrusters work. The grove
run hardly moved: t_grove Runner 40 → 42 %, Runner + C 55 → 52 % (seeds 1–120).
