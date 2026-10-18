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

## Sun-scorched wrecks (`scorched_wrecks`)

Wrecks only (2–3 `sample_mid`s 20 % gone, 1–2 `automaton_hunter`s 30 % gone), one per scorched deep, `near_star`:
placed before the other sites, the nearest to the star of 64 spots in the disc that keep the spacing's min. The
voyage runner has no star (a voyage has none), so no runs: the placement test (`sim/tests/voyage.rs`,
`a_near_star_site_lies_by_the_hot_star`, a 2400 m cluster, the star 1.8 radii out in four directions) puts the
wrecks where a part takes 2.6, 3.9 and twice 4.0 (`STAR_HEAT_CAP`) times the centre's heat a second per open side:
0.8–1.2 instead of 0.3, where GDR-0102's shield emitter open on two sides overheats without a radiator.

## Precursor outpost (`automaton_outpost`, `precursor_outpost`, `outpost_garrison`)

| Test voyage | Sites (the last the goal) |
|---|---|
| t_den | raider_pair, precursor_wrecks, hunter_den (the rim's, for scale) |
| t_out1 | precursor_outpost |
| t_out | raider_pair, precursor_wrecks, precursor_outpost |
| t_outg | rim_stragglers, grown_dead, outpost_garrison |

The station is anchored ([GDR-0110](../gdr/0110-anchored-stations.md)): it turns under the `outpost` behavior
(`face` the nearest enemy within 110 % of reach, else `hold`) and never moves. Runner + plain skiff (S) and Runner +
cannon skiff (C), clear %, seeds 1–120 unless noted:

| Outpost | t_out S | t_out C | t_outg S | t_outg C |
|---|---|---|---|---|
| 2 cannons per spoke, 2 on the hub (8), spokes not yet fixed after a split | 12 | 13 | — | — |
| 1 per spoke (3), spokes not yet fixed | 71 | 68 | 58 (1–2 gunners) | 61 |
| 3, spokes fixed (a breakage now gives an anchored piece no velocity) | 48 | 42 | 51 (1–2 gunners) | 44 |
| 3, one or no shield emitter | 46–49 | 45–49 | 48 | 47–48 |
| **2 (a scanner mast in front)** | 69 | 66 | 52 (1–2 gunners + a hunter) | 52 |
| **the same with the docked wrecks (final)** | **64** | **58** | **53** (1 gunner + a hunter, seeds 1–240) | **54** |

For scale: t_den 72 / 72 %; the Warden 42 % and the outer grove run 52–55 % (Runner + one escort). The shields
hardly matter: the hub has no gun, so the site clears once the two cannon spokes are disarmed; most runs that fail
end "home" (the pilot gave up after three minutes without progress), few lost (3–11 %). With the Runner alone t_out
clears 45 %, t_outg 20 %. The final rim outpost alone (t_out1): 60 / 74 / 69 % (Runner / + S / + C); its haul home
(*now*) 3.6–5.4 against a hunter den's 7–9, the docked wrecks counted (the pilot strips what it can reach and its
hold fills).
