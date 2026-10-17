# Slice balance after the gunner and shield capacity 2 (17 Oct 2026, second pass)

The morning's re-measure (`slice-balance-2026-10-17.md`) left three gaps against [GDR-0100](../gdr/0100-fight-balance-targets.md):
the starter too strong on the first two voyages, shields that hardly ever collapse, and the cannon starter as strong
at the Warden as a Runner with an escort. Niklas decided the same day: a different first enemy that answers shields
(content only, the starter stays; [GDR-0101](../gdr/0101-the-automaton-gunner.md)), shield capacity 3 → 2
([GDR-0021](../gdr/0021-shields.md), B2 of GDR-0100), then re-measure everything, the cannon starter at the Warden
included (he decides on the cannon after it), and re-tune counts if the curve moved.

**In short.**

- The new enemy is the **automaton gunner** (`automaton_gunner`, 4×3, two automaton cannons side by side, one core)
  flying its own **`gunner_pack`** (the raider's 3-of-9-seconds cycle, aimed at the target's engines). It waits in
  three new encounters: `gunner_pair` (2), `raiding_party` (a gunner and a raider), `gunner_den` (3). The Shipyard
  approach is now gunner pair ×2, derelict field, goal raiding party; Halvex Drift gunner pair, precursor wrecks,
  core graveyard, goal gunner den; Corran Reach gained a gunner pair; a wreck drift holds 0–1 gunner pairs.
- **Shield capacity 3 → 2** (regen 0.2 stays): under sustained fire (the Runner's test-range wave, passive pilot) a
  bubble collapses in **16 % of waves** (39 of 240; 8 % at capacity 3), about one in six as asked. The closing pilot
  and the starter's own wave still never collapse one.
- Clear rates over 240 seeds, before → after (the curve GDR-0073 set in brackets):
  - starter 96 / 62 / 42 / 15 % → **57 / 37 / 22 / 15 %** (58 / 37 / 15 / 3)
  - Runner 66 / 68 / 49 / 26 % → **74 / 65 / 40 / 25 %** (60 / 67 / 45 / 20)
  - Runner + starter 86 / 79 / 62 / 44 % → **82 / 70 / 51 / 42 %** at the Warden (about 40 % asked)
  - Every fleet now finds each voyage harder than the one before (the Runner's first-two inversion is gone).
- **The cannon starter** (`skiff_cannon`) is hardly touched by gunners: 99 / 74 / 50 / 37 % → 93 / 68 / 46 / **37 %**.
  Alone it clears the Warden about as often as a Runner with the plain starter as escort (42 ± 6 %; the two are
  within noise of each other); a Runner with the cannon starter as escort clears it 48 %, two Runners 54 %.
- The Warden needed no new count (Runner + escort 42 %). Corran Reach needed one (its gunner pair): with the first
  two voyages harder it had become easier than the second for the starter (40 % against 37 %).

## Method

As in the morning's report: the headless voyage runner (`crates/core/sim/examples/voyage.rs`, its pilot in
`sim::voyage::runner`, unchanged), release build, seeds 1–240 per voyage and fleet (halves 1–120 / 121–240 shown),
**clear** = goal cleared and home, **home** = recalled without the goal, **lost**, **out of time** (30 min). The ± is
a 95 % interval (1.96 standard errors of a share of 240): differences under about 8 points are noise. "Before" is
`dev` `2fb07ff` (the morning report's "after", the same content). Load average 1.7–1.9, no other builds.

The search ran in the balance harness's new voyage mode (`../space-game-balance`, `--voyages … --fleets …`: the same
runner on four threads, the content read from a pack directory and edited between runs, so a content change needs
no rebuild; its numbers for today's content match the example's run for run). The final numbers below are the
example's.

Fleets: `rim_skiff` (the starter), `skiff_cannon` (the starter with `automaton_cannon` in place of both pulse
lasers: a blueprint file, see Reproduce), `halvex_runner`, the Runner with `rim_skiff` as escort, and at the Warden
two Runners and a Runner with `skiff_cannon` as escort.

## Before and after (240 seeds)

Clear / home / lost / out of time (of 240), clear rate before → after, after on seeds 1–120 / 121–240.

| Voyage | Fleet | Before | After | Clear % | Halves (after) |
|---|---|---|---|---|---|
| yard_approach | rim_skiff | 230 / 8 / 2 / 0 | 137 / 86 / 17 / 0 | 96 → **57** ± 6 | 52 / 62 |
| yard_approach | skiff_cannon | 237 / 3 / 0 / 0 | 223 / 13 / 4 / 0 | 99 → 93 ± 3 | 91 / 95 |
| yard_approach | Runner | 158 / 76 / 6 / 0 | 177 / 62 / 1 / 0 | 66 → 74 ± 6 | 77 / 71 |
| yard_approach | Runner + rim_skiff | 206 / 32 / 2 / 0 | 196 / 43 / 0 / 1 | 86 → 82 ± 5 | 82 / 82 |
| halvex_drift | rim_skiff | 150 / 85 / 5 / 0 | 89 / 103 / 48 / 0 | 62 → **37** ± 6 | 42 / 32 |
| halvex_drift | skiff_cannon | 177 / 58 / 5 / 0 | 164 / 44 / 32 / 0 | 74 → 68 ± 6 | 72 / 65 |
| halvex_drift | Runner | 164 / 67 / 9 / 0 | 156 / 75 / 9 / 0 | 68 → 65 ± 6 | 65 / 65 |
| halvex_drift | Runner + rim_skiff | 189 / 39 / 10 / 2 | 168 / 60 / 12 / 0 | 79 → 70 ± 6 | 72 / 68 |
| corran_reach | rim_skiff | 100 / 120 / 20 / 0 | 53 / 157 / 30 / 0 | 42 → **22** ± 5 | 23 / 21 |
| corran_reach | skiff_cannon | 119 / 101 / 20 / 0 | 110 / 101 / 29 / 0 | 50 → 46 ± 6 | 45 / 47 |
| corran_reach | Runner | 118 / 118 / 4 / 0 | 95 / 133 / 12 / 0 | 49 → **40** ± 6 | 46 / 33 |
| corran_reach | Runner + rim_skiff | 149 / 73 / 18 / 0 | 122 / 113 / 5 / 0 | 62 → 51 ± 6 | 52 / 50 |
| wardens_gate | rim_skiff | 36 / 168 / 36 / 0 | 36 / 170 / 34 / 0 | 15 → 15 ± 5 | 18 / 12 |
| wardens_gate | skiff_cannon | 88 / 118 / 34 / 0 | 88 / 113 / 39 / 0 | 37 → **37** ± 6 | 37 / 37 |
| wardens_gate | Runner | 62 / 168 / 10 / 0 | 61 / 170 / 9 / 0 | 26 → 25 ± 6 | 20 / 31 |
| wardens_gate | Runner + rim_skiff | 105 / 128 / 7 / 0 | 102 / 129 / 9 / 0 | 44 → **42** ± 6 | 39 / 46 |
| wardens_gate | Runner + Runner | 137 / 86 / 13 / 4 | 130 / 96 / 11 / 3 | 57 → 54 ± 6 | 54 / 54 |
| wardens_gate | Runner + skiff_cannon | – | 116 / 110 / 13 / 1 | 48 ± 6 | 48 / 49 |

As curves (clear %, the four voyages in order):

| Fleet | GDR-0073's curve | Before (`2fb07ff`) | After |
|---|---|---|---|
| rim_skiff | 58 / 37 / 15 / 3 | 96 / 62 / 42 / 15 | 57 / 37 / 22 / 15 |
| skiff_cannon | – (68 / 53 / 35 / 18 after GDR-0074) | 99 / 74 / 50 / 37 | 93 / 68 / 46 / 37 |
| Runner | 60 / 67 / 45 / 20 | 66 / 68 / 49 / 26 | 74 / 65 / 40 / 25 |
| Runner + rim_skiff | – / 78 / 65 / 42 | 86 / 79 / 62 / 44 | 82 / 70 / 51 / 42 |

The Warden's gate did not change (its content is the same; capacity 2 moved nothing outside the noise), so the
Runner with an escort stays at 42 %. Scripted minutes (median of the runs home): 3.1 / 3.0 / 4.1 / 2.6 for the
starter; Corran Reach with its fifth site 3.3 → 4.1 (doubled for a human 8.2), so its `minutes` went from (5, 8) to
(6, 9). Step time median 0.006–0.008 ms, p95 0.013–0.022 ms.

What the gunners cost: the starter is **lost** far more often on the first two voyages (17 and 48 runs of 240, were
2 and 5); runs home without the goal grew from 8 to 86 and 85 to 103. The cannon starter is lost more too (4 and
32, were 0 and 5) but still clears.

## The search

### The gunner (seeds 1–60, shield capacity 2, every raider of the first two voyages replaced)

Clear % on the Shipyard approach / Halvex Drift. "Raider" is the raider's own layout under the changed behavior,
"twin" the gunner's layout; the attack window is the seconds of every 9 spent attacking (`raider_basic`: 3).

| Layout | Aim | Attack | rim_skiff | Runner |
|---|---|---|---|---|
| raider | centre | 3 s (today's raider) | 95 / 56 | 68 / 66 |
| raider | centre | 5 s | 68 / 45 | 13 / 41 |
| raider | engines | 3 s | 91 / 56 | 86 / 83 |
| raider | power | 5 s | 41 / 43 | 23 / 41 |
| twin | centre | 3 s | 43 / 28 | 30 / 31 |
| twin | power | 3 s | 50 / 35 | 23 / 21 |
| **twin** | **engines** | **3 s** | **51 / 48** | **80 / 61** |
| twin | engines | 5 s | 58 / 41 | 51 / 51 |

Seeds 1–120, the raider's layout and aim: attacking all the time costs both ships everything (0–4 % clear); 4, 5, 6
and 7 s of 9 give the starter 82 / 66 / 22 / 8 % on the first voyage and the Runner 33 / 13 / 12 / 3 %; aiming at
weapons 87 % against the Runner's 47 %; kiting at 92 % of the reach (outside both ships' pulse lasers) 0 % for
both. Everything that hurt the starter hurt the Runner more, except aiming at the engines: the Runner gains while
the starter loses. Two cannons then bring the starter down; one raider aimed at the engines alone does not.

### Where (240 seeds, clear %: rim_skiff / skiff_cannon / Runner / Runner + rim_skiff)

| Voyage | Sites | Clear % |
|---|---|---|
| yard_approach | gunner pair ×2, derelict field, goal gunner pair | 48 / 90 / 75 / 85 |
| yard_approach | gunner pair ×2, derelict field, goal raider pair | 69 / 92 / 72 / 86 |
| yard_approach | **gunner pair ×2, derelict field, goal raiding party** | **57 / 92 / 73 / 82** |
| yard_approach | gunner pair, raiding party, derelict field, goal gunner pair | 64 / 93 / 72 / 82 |
| halvex_drift | raider pair, …, goal gunner den (2–3) | 45 / 67 / 62 / 72 |
| halvex_drift | gunner pair, …, goal gunner den (2–3) | 52 / 73 / 68 / 74 |
| halvex_drift | gunner pair, …, goal a gunner and a raider each 1–2 | 44 / 77 / 67 / 75 |
| halvex_drift | **gunner pair, …, goal gunner den (3)** | **37 / 68 / 65 / 70** |
| corran_reach | as before (hunter pack ×2, precursor wrecks, goal hunter den) | 40 / 49 / 50 / 62 |
| corran_reach | **+ gunner pair** | **22 / 45 / 39 / 50** (seeds 241–480: 27 / 48 / 42 / 52) |
| corran_reach | + raiding party | 28 / 38 / 31 / 50 |
| corran_reach | + raider pair | 37 / 46 / 28 / 43 |
| corran_reach | hunter den 3 | 18 / 36 / 38 / 57 |
| corran_reach | hunter pack ×3 | 28 / 38 / 38 / 46 |
| corran_reach | hunter pack, gunner pair (one pack fewer) | 32 / 57 / 52 / 59 |

Corran Reach (searched with the first two voyages' gunners in place): every option that makes it harder for the
starter costs the Runner about ten points (the curve's 45 % now 40 ± 6). The gunner pair (chosen) puts the starter
between its second voyage and the Warden (22–27 % over 480 seeds) with the Runner at 39–42 %; a den of three hunters
brings the starter to 18 %, too close to the Warden's 15 %.

## Shields (the balance harness, 240 seeds)

The test range's waves (`sim::voyage::waves`: the Runner meets two hunters and a raider, the starter one hunter;
the gunner's 21.2 build points change neither, `waves.rs`'s test) and raider waves, capacity 3 (before) and 2
(after), regen 0.2. Win / loss / timeout, waves with at least one collapsed bubble (collapses per wave):

| Setup | Win / loss / timeout | Capacity 3 | Capacity 2 |
|---|---|---|---|
| Runner, passive pilot, its wave (sustained fire) | 192 / 38 / 10 (both) | 19 (7.9 %; 0.10) | **39 (16.3 %; 0.30)** |
| Runner, closing pilot, its wave | 239 / 1 / 0 | 0 | 0 |
| rim_skiff, passive pilot, its wave (1 hunter) | 237 / 0 / 3 | 0 | 0 |
| rim_skiff, closing pilot, its wave | 240 / 0 / 0 | 0 | 0 |
| Runner, passive, 3 raiders | 239 / 0 / 1 | 0 | 1 (0.4 %) |
| Runner, passive, 5 raiders | 185 / 45 / 10 → 186 / 45 / 9 | 27 (11.3 %) | 56 (23.3 %) |
| rim_skiff, passive, 3 raiders | 228 / 8 / 4 → 227 / 9 / 4 | 3 (1.3 %) | 15 (6.3 %) |

Under sustained fire a bubble now collapses in about one wave in six; win rates and fight lengths (7–17 s) do not
move. The starter's bubbles still never empty against its hunter: it kills it in 7–11 s.

## For Niklas

- **The cannon:** the cannon starter alone clears the Warden 37 % (± 6), a Runner with the plain starter as escort
  42 %, with the cannon starter as escort 48 %. Its curve 93 / 68 / 46 / 37 % stays above the starter's
  (57 / 37 / 22 / 15) and level with the Runner + escort's from the third voyage on; the gunners barely touch it
  (99 → 93 % on the first voyage). The first research node still matches the mid-game fleet at the last voyage.
- **Still off the curve:** the starter at the Warden (15 % against 3 %: the Warden is unchanged and B3 tunes it for
  the Runner + escort), the starter at Corran Reach (22 against 15 %), the Runner on the first voyage (74 against
  60 %: gunners are easier for it than raiders were). The Runner at Corran Reach is 40 % (45 asked), within noise.
- The starter is lost on 20 % of its Halvex Drift runs (48 of 240); before, 2 %.

## Determinism

`scripts/determinism-check.sh` (release, native x86_64 vs wasm32-wasip1 under Node 24), all **IDENTICAL**: voyage
seeds 42 and 7 (1800 ticks, last `e2404c9d5557eb3f` / `9be576eee0c5aa07`), seed 42 at 900 ticks voyage
`f53bdfa38cba1588`, core `31b368b328356716`, salvage `58ad85ac07925283`, behavior `be47fae608752449`; the probes
`warden` `20ae64d030a238de` and `close_quarters` `476b934495061c92`, `runner_vs_raiders` `1bd3a842a2c5dfdd`; `--run
mixed_trials` and `--run two_starts_locked`; `--galaxy 4` (seed 1's skeleton still `8a509f91bd8fd0b8`, its interiors
`799aaac8d26d27ef`). Every stream changed: the core, salvage and behavior scenarios spawn two of every blueprint
(the gunner too), the voyage scenario flies the first voyage, and every fight with shields changed with the
capacity. The content's fingerprint changes, so the rules' fingerprint does too: fight codes and lab runs made
before are stale (they replay with the note and restore read-only, `docs/arena.md` "Determinism").

Tests that encode the content changed with it: the slice chain's goals and site counts (`data`; the raider pair and
den now count as used through the kinds of cluster), the waves' enemy list (`sim`, the client's test range), the
lab's encounters and ARMS RACE starts (`arena`, `world`), a voyage banner and the map panel's Halvex Drift rows
(`world`, `game`), the wiki's voyage and encounter pages (`content-tool`), the shield test's first second of beam (20
ticks now stay under capacity 2). `rapier_mass_properties_are_the_pieces` (`sim`) now skips pieces that changed in
the checked tick: with the gunner in the core scenario a ram crushed parts at tick 150, after that tick's physics
step, and Rapier sums a changed piece's mass at its next step (the sim's own mass was right; one tick later Rapier's
matched).

## Reproduce

```sh
CARGO_BUILD_JOBS=2 cargo build --release -p sim --example voyage
target/release/examples/voyage --voyage wardens_gate --design halvex_runner --escort rim_skiff --seeds 1..240
target/release/examples/voyage --voyage wardens_gate --design skiff_cannon.ron --seeds 1..240   # a blueprint file
# the search and the shields: in ../space-game-balance
CARGO_TARGET_DIR=$PWD/target cargo build --release
target/release/space-game-balance --content ../space-game/content/base --voyages yard_approach,halvex_drift \
  --fleets rim_skiff,halvex_runner,skiff_cannon.ron,halvex_runner+rim_skiff --seeds 1-240 --threads 4
target/release/space-game-balance --setups range_runner_passive,range_runner_closing,range_skiff_passive,range_skiff_closing \
  --variants cap=3,base --seeds 1-240
```

`skiff_cannon.ron` is `content/base/ships/rim_skiff.ron` with `pulse_laser` replaced by `automaton_cannon` (its id
`skiff_cannon`).
