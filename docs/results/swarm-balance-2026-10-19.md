# The Swarm and drones (19 Oct 2026, GDR-0117)

## Tick cost of a drone flood

The balance harness's flood mode (`../space-game-balance`, `--flood N`): `N` Swarm hives round the Halvex Runner and
two rim skiffs, 1800 ticks, three seeds, every `Sim::step` timed (native release, load average about 1). "Idle":
the same world with the hives given no behavior (no target, so no launch). The world's cap is 48 drones
(`sim::ship::drones::tuning::MAX_DRONES`), a full hive flood 24.

| World | drones (mean / max) | step median | step p95 | step mean | physics median |
|---|---|---|---|---|---|
| 1 hive, idle | 0 / 0 | 0.003 ms | 0.003 ms | 0.003 ms | 0.001 ms |
| 1 hive, full flood | 23.7 / 24 | 0.029 ms | 0.055 ms | 0.032 ms | 0.019 ms |
| 2 hives, idle | 0 / 0 | 0.003 ms | 0.003 ms | 0.003 ms | 0.001 ms |
| 2 hives, the cap | 47.4 / 48 | 0.059 ms | 0.100 ms | 0.061 ms | 0.038 ms |

About 1.2 µs per drone per tick, two thirds of it the Rapier step (a body each): a capped flood costs 0.3 % of a
tick's 33 ms. The drone pass itself (a scan of the ships, a sort of the live drones' ties, one compare per hangar)
does not show beside the physics. Lost drones are removed at once, so a long fight leaves no wrecks to step.

Determinism: `scripts/determinism-check.sh --fight hive_flood --ticks 3600` (`crates/core/arena/fights/hive_flood.ron`:
the hive against the Runner and two skiffs), native against wasm32-wasip1: **identical**, seed 42
(`69bb8b4d8c3f7b85`) and seed 7 (`91e6a3352c1bd8e3`).

## Voyages

The harness's voyage mode (the game's headless voyage runner, the content read at run time), seeds 1–120 per row,
clear % (goal cleared and home). A copy of the pack carried test voyages built from the sites (never in the game),
radius 1000 m, sites 400–700 m apart: t_grove (outer: grove, grove_pair, grown_dead, grove), t_keep (keeper_gate
alone), t_patrol (swarm_patrol, swarm_dead, swarm_patrol), t_raid (swarm_raid, grown_dead, swarm_raid), t_carrier
(swarm_patrol, swarm_dead, hive_carrier), t_nest (swarm_dead, brood_nest), t_hive (swarm_patrol, hive_site). Fleets:
the Runner with a cannon skiff (R + C, an outer-reach fleet), with two plain skiffs (R + 2S) and two cannon skiffs
(R + 2C); the latter two clear the Keeper's gate about 60 %, a Keeper-capable fleet.

| Voyage | R + C | R + 2S | R + 2C |
|---|---|---|---|
| t_grove (outer, for scale) | 52 % | 61 % | 62 % |
| t_keep (the middle band's gate) | 42 % | 62 % | 61 % |
| t_patrol | 100 % | 100 % | 100 % |
| t_raid | 100 % | 100 % | 100 % |
| t_carrier | 30 % | 51 % | 67 % |
| t_nest | 51 % | 55 % | 62 % |
| t_hive (the boss site) | 28 % | 41 % | 57 % |

So a carrier or a hive is a step up from a grove for an outer-reach fleet (30 and 28 % against 52 %) and about a
Keeper's gate for a Keeper-capable one (51–67 %, the hive 41–57 %). Patrols and raids are filler: the runner's fleet
outranges the spitters.

What was tried (60 seeds unless noted, R + C / R + 2S):

| Change | t_carrier | t_nest | t_hive |
|---|---|---|---|
| first cut: 6 drones a hangar, 7 s, energy stingers, the carrier kiting at 200 m | 0 / 10 | 2 / 5 | 2 / 3 |
| the runner's pilot goes for the carrier before its drones (kept) | 5 / 15 | 8 / 13 | 33 / 32 |
| 4 drones, 10 s rebuild | 25 / 20 | 12 / 23 | 60 / 67 |
| the carrier closes to 150 m and holds instead of kiting (kept): it ran from the 60 % pilot, which gave up | 58 / 62 | 65 / 65 | 33 / 32 |
| stingers kinetic, 55 m (kept): the starters' shield bubbles took energy stings whole (a spitter dealt 1–5 damage in 90 s) | 32 / 55 | 53 / 48 | 22 / 23 |
| spitter 4×4 (a chitin row more, kept) | 23 / 48 | 45 / 48 | 17 / 25 |
| the hive without its two spitters, brood hangar 3.5 hit points (kept; 120 seeds) | 39 / 57 | — | 33 / 49 |

The runner's pilot: drones are fought only while no other armed enemy is within 400 m (a player dives for the
hangar; drones die with it). A drone's loss is no kill (no `VesselLost`), so before that change the pilot shot drones
for three minutes without progress and gave up.

## Reproduce

```sh
cd ../space-game-balance && cargo build --release
target/release/space-game-balance --flood 1; target/release/space-game-balance --flood 2
cp -r ../space-game/content/base /tmp/pack   # plus the test voyages above as voyages/zz_middle.ron
target/release/space-game-balance --content /tmp/pack --voyages t_grove,t_keep,t_patrol,t_raid,t_carrier,t_nest,t_hive \
  --fleets halvex_runner+fits/skiff_cannon.ron,halvex_runner+rim_skiff+rim_skiff,halvex_runner+fits/skiff_cannon.ron+fits/skiff_cannon.ron \
  --seeds 1-120 --threads 3
```

## Follow-up: spitters that matter (19 Oct 2026, later)

Patrols and raids cleared every time, and not because the runner's fleet outranged the spitters: a spitter has no
side thrust, so while it orbited (`orbit(75%)` for five seconds of every eight) its nose stingers never pointed at
the target. An arena fight of four spitters against the Runner and two rim skiffs (`arena fight`, 20 seeds, 250 m
apart): the spitters died in about 30 s having dealt 3–17 damage (their eight stingers could deal over a hundred in that time).
More spitters (patrols of ten) or four stingers a spitter barely moved it.

The spitter now turns its stingers onto the target for five seconds of every eight
(`when target_in_range(100%) and cycle(8s) >= 3s -> face`) and closes in for the other three; a Swarm raid has
three or four spitters (two before). Seeds 1–240, the test voyages above plus t_carr1 (`hive_carrier` alone), clear %:

| Voyage | R + C | R + 2S | R + 2C | before (120 seeds) |
|---|---|---|---|---|
| t_patrol | **73 %** | **76 %** | **80 %** | 100 / 100 / 100 |
| t_raid | **70 %** | **77 %** | **79 %** | 100 / 100 / 100 |
| t_carrier (a patrol, Swarm dead, the carrier) | 21 % | 32 % | 37 % | 30 / 51 / 67 |
| t_carr1 (the carrier site alone) | 45 % | 62 % | 62 % | 42 / 57 / 70 |
| t_nest | 50 % | 55 % | 63 % | 51 / 55 / 62 |
| t_hive (a patrol, the hive) | 14 % | 37 % | 35 % | 28 / 41 / 57 |

Tried on the way (seeds 1–120; patrol / raid clear % for R + C, R + 2S, R + 2C):

| Change | t_patrol | t_raid |
|---|---|---|
| patrols 5–6, raids 3–4 (spitters orbiting) | 99 / 99 / 99 | 98 / 99 / 99 |
| patrols 10, raids 8 (orbiting) | 80 / 72 / 81 | 91 / 98 / 96 |
| four stingers a spitter (orbiting) | 98 / 99 / 99 | 98 / 100 / 100 |
| orbit at 45 % | 100 / 99 / 100 | 98 / 100 / 99 |
| `face` whenever in reach, raids 3–4 | 68 / 78 / 68 | 61 / 74 / 72 |
| `face` five seconds of every eight, raids 3–4 (chosen) | 80 / 80 / 81 | 69 / 79 / 75 |

Patrols and raids are now a modest threat (70–80 %). The carrier and nest sites are unchanged; a voyage that meets a
patrol first is harder (the fleet is not repaired between sites): the carrier voyage 21–37 %, the hive voyage
14–37 %, below the Keeper's gate for a Keeper-capable fleet. The drones' own behavior orbits too (four seconds of
every six, diving in for two); it was tuned with that and is unchanged.
