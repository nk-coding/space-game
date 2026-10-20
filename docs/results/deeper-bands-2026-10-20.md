# The deeper bands' lore, caches and graveyards, more callers, a mining sample (20 Oct 2026, GDR-0128)

## Placement in the galaxy

`content-tool galaxy --seed N` over seeds 1–500 (the base content with this change), how often each new trail is in a
galaxy. Clusters per band over seeds 1–200: outer 13–23 (median 18), middle 8–17 (13), inner 6–12 (9), each with 3
gates.

| Band | Trail | In galaxies |
|---|---|---|
| outer | lore `older_than_the_yard`, `facing_inwards`, `a_hollow_core` | 100 % each |
| outer | `overgrown_cache`, `overgrown_fleet` (spare) | 100 % each |
| middle | lore `the_second_chart`, `not_the_cargo`, `only_loud` | 100 % each |
| middle | `resin_cache` / `picked_graveyard` (spare) | 99.6 % / 99.0 % |
| inner | (for scale) `lattice_resonator` 1.98 a galaxy, `phase_colossus` | 85.6 % |
| inner | lore `forty_names` / `logged_twice` | 99.0 % / 88.2 % |
| inner | `crystal_cache` / `rift_graveyard` / `crystal_wreck_field` (spare, `ends_in` their race's types) | 55.8 % / 22.8 % / 22.4 % |

With a third inner fragment (`ask_what_they_heard`, written and dropped) the fragments were placed 99 / 88 / 67 % and
the spares 26 / 11 / 7 %: the inner band has 3–9 clusters free after its gates, and its three bosses take three. Two
fragments keep the second at 88 % and leave the spares worth having.

Before the generator change a band whose end clusters ran out failed the whole galaxy (`no cluster left`); now any
trail but a gate's is left out there (as `ends_in` trails already were), and a band's `spare` trails come after its
lore. Galaxies of the old content generate exactly as before (no generator version bump); the pin of seed 1 moved
`256b011b9688cc79` → `af364f2879722a2d` with the new content.

## Voyages

The game's headless voyage runner (`target/release/examples/voyage`) with `--pack dir`: a scratch mod pack of test
voyages built from the content's encounters (radius 1000 m, sites 400–700 m apart; never in the game). Fleets: the
Halvex Runner with a rim skiff (R + S), a cannon skiff (R + C: the skiff with `reverse_engineered_cannon` for its pulse
lasers), two skiffs (R + 2S) and two cannon skiffs (R + 2C). Seeds 1–120 (callers 1–240), clear = goal cleared and
home. Load average 5–17 (three agents building).

| Test voyage (sites, the last the goal) | R + S | R + C | R + 2S | R + 2C |
|---|---|---|---|---|
| t_sealed (raider_pair, sealed_cache: the rim's, for scale) | 93 | 94 | 95 | 97 |
| t_deepgy (raider_pair, deep_graveyard: the rim's) | 98 | 98 | 100 | 98 |
| t_grove (grove, grove_pair, grown_dead, grove: for scale) | 53 | 55 | 61 | 64 |
| t_guarded (grove, grown_dead, guarded_grove: for scale) | 74 | 78 | 78 | 82 |
| t_elder (grove_pair, elder_grove: the outer boss) | 35 | 36 | 66 | 60 |
| t_ocache (grove_pair, **overgrown_cache**) | 48 | 46 | 60 | 53 |
| t_ocache2 (grove, grown_dead, **overgrown_cache**) | 66 | 61 | 73 | 72 |
| t_ofleet (grown_dead, **overgrown_fleet**) | 81 | 82 | 87 | 88 |
| t_patrol (swarm_patrol, swarm_dead, swarm_patrol: for scale) | 73 | 74 | 81 | 80 |
| t_keep (keeper_gate: the middle band's gate, for scale) | 35 | 39 | 63 | 61 |
| t_rcache (swarm_patrol, **resin_cache**) | 84 | 88 | 88 | 89 |
| t_pgy (swarm_dead, **picked_graveyard**) | 100 | 100 | 100 | 100 |
| t_watch (lance_watch, lattice_dead, lance_watch: for scale) | 48 | 54 | 50 | 53 |
| t_site (lattice_dead, crystal_site: for scale) | 80 | 83 | 82 | 85 |
| t_wpack (walker_pack, walker_dead, walker_pack: for scale) | 46 | 46 | 56 | 58 |
| t_ccache (lance_watch, **crystal_cache**) | 41 | 43 | 44 | 48 |
| t_cwf (lattice_dead, **crystal_wreck_field**) | 76 | 74 | 76 | 78 |
| t_rgy (walker_dead, **rift_graveyard**) | 83 | 83 | 82 | 88 |

So each band's **cache** is about as hard as its ordinary sites and easier than its boss or gate (the overgrown cache
61–73 % after a grove like the guarded grove's 74–82 %, below it because the cache's precursor hulls are stripped
mid-fight and wake the keepers; the resin cache 84–89 % after a patrol; the crystal cache a crystal site behind a lance
watch, 41–48 %, the band's hardest cache for its richest store), and each **graveyard** is the easier rich salvage the
rim's is (74–100 %). No count was tuned.

## Callers (GDR-0113's way of measuring)

A test voyage with the caller's site first, against the same with the scout that never scans (a pack overriding
`picket` with `always -> flee`: no call, no group), seeds 1–240. GDR-0113's aim: a call costs 5–15 points.

| Voyage, called group | R + S | R + C | R + 2S | R + 2C |
|---|---|---|---|---|
| t_watched (**watched_grove**, grove, grown_dead, grove), never scans | 57 | 58 | 67 | 69 |
| the same, calling a rammer and a thornback (first cut) | 29 | 36 | 40 | 44 |
| the same, calling a rammer (**kept**) | 45 | 47 | 55 | 60 |
| t_wbase (grove_pair in its place, for scale) | 55 | 55 | 64 | 63 |
| t_scout (**scout_patrol**, swarm_patrol, swarm_dead, swarm_patrol), never scans | 58 | 58 | 64 | 62 |
| the same, calling 2–3 spitters (first cut) | 34 | 36 | 38 | 40 |
| the same, calling 1–2 spitters | 43 | 40 | 45 | 45 |
| the same, calling a spitter (**kept**) | 43 | 44 | 50 | 50 |
| t_pbase (swarm_patrol in its place, for scale) | 40 | 38 | 43 | 44 |

A call costs the watched grove 8–12 points and the scout patrol 12–15 (the scout's sweep also wakes the patrols round
it, and the Swarm are not wary, so they all come). The first cuts' groups cost 22–28.

## The mining sample

`sample_miner` (two mining lasers, two pulse lasers, a 28-unit hold, no shields; flies `miner`) on a quiet test voyage
(t_mine: a derelict field, precursor wrecks and a raider pair, radius 900 m, two ice fields of 12–20 rocks and 20–30
scattered iron rocks), 120 seeds:

| Fleet | Clear | Lost | Haul items (median / p95) |
|---|---|---|---|
| Runner + rim skiff | 89 % | 3 | 8 / 12 |
| Runner + sample_miner | 79 % | 4 | 10 / 17 |
| rim skiff + rim skiff | 88 % | 2 | 4 / 8 |
| sample_early + rim skiff | 65 % | 7 | 8 / 12 |
| sample_miner (flown) + rim skiff | 9 % | 80 | 24 / 31 |

Flown as the flagship it mines its hold full (the runner's pilot mines when there is no fight) and loses most fights,
as GDR-0114's one-gun miner did; as an escort the runner flies it `escort` (GDR-0123), so it mines little there. It is
not in the arena report's pool: the pool is ships that fly one against one under their own behaviors, and `miner`
fights only its leader's lock.

## Checks

`cargo test -p data -p galaxy -p content-tool -p sim`: all pass (galaxy pin re-pinned; the content-tool export test
counts the new encounters). `scripts/arena-check.sh`: all seven fights the same, fight for fight; the rules
fingerprint moved with the new content (baseline refreshed). `scripts/determinism-check.sh --galaxy 200`: see GDR-0128.
Wiki: `npm run build` and `npm run check`.

## Reproduce

```sh
cargo build --release -p sim --example voyage
# a pack dir: pack.ron = (id: "bands_test", ..., depends: ["base"]), voyages/zz_test.ron with the test voyages above;
# for "never scans" a second pack whose behaviors/picket.ron is the picket with only `always -> flee`
target/release/examples/voyage --pack <dir> --voyage t_scout --design halvex_runner --escort rim_skiff,rim_skiff --seeds 1..240
for s in $(seq 1 500); do target/debug/content-tool galaxy --seed $s | grep ^trail; done   # placement
```
