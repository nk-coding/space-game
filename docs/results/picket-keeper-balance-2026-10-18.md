# The automaton picket and the Keeper (content batch of 18 Oct 2026)

Measured with the balance harness's voyage mode (`../space-game-balance`, `--voyages --fleets`, the game's headless
voyage runner, the content read at run time) as in [the encounters of the same batch](encounters-balance-2026-10-18.md):
seeds 1–120 per row, clear = goal cleared and home, hull lost the mean share of the fleet's hull. Fleets: the Runner
(`halvex_runner`), the Runner with a plain skiff (S, `rim_skiff`) and with a cannon skiff (C, `rim_skiff` with
`reverse_engineered_cannon` for its lasers). A copy of the pack carried test voyages built from the sites (never in
the game). Load average about 2.

## Automaton picket (`automaton_picket`, `picket`, `picket_post`; [GDR-0111](../gdr/0111-an-enemy-scan-calls-its-factions-sites.md))

| Test voyage | Sites (the last the goal), radius 1200 m (hunters 1400 m), sites 450–800 m apart |
|---|---|
| t_base | raider_pair, raider_pair, precursor_wrecks, raider_pack |
| t_pick | **picket_post**, raider_pair, precursor_wrecks, raider_pack |
| t_hbase | raider_pair, hunter_pack, precursor_wrecks, hunter_pack |
| t_hpick | **picket_post**, hunter_pack, precursor_wrecks, hunter_pack |

A picket post is a raider pair with the picket, so each pair of rows differs only by the picket.

| Voyage | Runner clear / hull lost | Runner + S | Runner + C |
|---|---|---|---|
| t_base | 32 % / 56 % | 54 % / 46 % | 53 % / 47 % |
| t_pick | 42 % / 50 % | 62 % / 37 % | 61 % / 44 % |
| t_pick, the picket never scans | 38 % / 51 % | 48 % / 47 % | — |
| t_hbase | 45 % / 49 % | 57 % / 43 % | 60 % / 40 % |
| t_hpick | 54 % / 43 % | 69 % / 36 % | 67 % / 39 % |
| t_hpick, the picket never scans | 44 % / 46 % | 62 % / 39 % | — |

The picket calls 15–20 s after its site wakes (once it is 300 m clear, the runner's pilot flying on to the next
site), then every 10 s while it can see the fleet (800 m). The rule was chosen on a first layout (two thrusters, one
of them idle: five parts on a core that runs four; the final one has one thruster), clear % of t_pick / t_hpick,
Runner and Runner + S:

| Call (first layout) | t_pick | t_hpick |
|---|---|---|
| none (the picket only flees) | 39 / 49 | — |
| within `wake` (300 m), any sleeping site of its faction | 41 / 51 | 47 / 65 |
| **within `range` (1200 m), sleeping sites (chosen)** | **42 / 61** | **50 / 72** |
| the same, calling at once (no 300 m gate) | 44 / 66 | 61 / 69 |
| the same, gated at 200 m | 39 / 61 | 54 / 69 |
| within `range`, sites not spawned yet spawn to answer | 69 / 71 | 68 / 83 |

Every call makes the voyage easier for the runner, never harder: called raiders come to its fleet, in a group from
one side, and its pilot fights them at range; a sleeping site it flies into wakes round it. The trace of t_pick seed
2: the post's pair cleared 16 s after waking, the call 2 s later woke the next two sites (three raiders 720 m off),
which arrived together 35 s later and were beaten for 13 % of the hull (the same seed without the picket: 68 %).
So the runner cannot measure what the picket is for (the player's choice of fights, a quiet salvage run, not
being caught between sites); the chosen rule is the one that wakes the most sites near the fleet without spawning
far ones. Open in GDR-0111: whether called ships should gather on the caller first.

## The Keeper (`automaton_keeper`, `keeper_gate`; [GDR-0112](../gdr/0112-the-keeper-guards-the-middle-bands-gates.md))

| Test voyage | Sites (the last the goal), radius 1200 m, sites 450–800 m apart |
|---|---|
| t_war, t_bas, t_keep | the_warden, bastion_gate, keeper_gate alone |
| t_bas3, t_keep3 | raider_pack, hunter_pack, then bastion_gate or keeper_gate |

Runner + one skiff (S) and + two skiffs (SS), clear % / hull lost:

| Voyage | Runner + S | Runner + SS | Runner + C |
|---|---|---|---|
| t_war (the Warden and 2–3 raiders) | 86 / 23 % | 98 / 17 % | 92 / 21 % |
| t_bas (the bastion, a gunner and a hunter) | 82 / 30 % | 89 / 27 % | 76 / 34 % |
| t_keep, the Keeper and 2 hunters | 56 / 41 % | 76 / 30 % | 57 / 40 % |
| t_keep, the Keeper and 2 hunters, 2 gunners | 26 / 57 % | 42 / 52 % | — |
| **t_keep, the Keeper and 3 hunters (chosen)** | **38 / 50 %** | **57 / 42 %** | — |
| t_bas3 | 22 / 56 % | 39 / 55 % | — |
| t_keep3, 2 hunters | 20 / 52 % | 33 / 50 % | — |
| **t_keep3, 3 hunters (chosen)** | **10 / 58 %** | **20 / 61 %** | — |

A gate alone is far easier for the runner than at the end of a run (the bastion gate 82 % alone, 22 % behind two
packs, against the 52 % of GDR-0103's outer run). With two hunters the Keeper was barely harder than the bastion
behind two packs (20 against 22 %); with three it clears about half as often in both, a clear step above it. The
middle band's clusters are still the rim's automatons (`uncharted_deep`), and a fleet that gets this far should be
stronger than a Runner and skiffs: re-measure when the band has its race.
