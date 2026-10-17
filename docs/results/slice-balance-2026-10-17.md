# Slice balance re-measured (17 Oct 2026)

Niklas's targets of 17 Oct ([GDR-0100](../gdr/0100-fight-balance-targets.md)): GDR-0073's curve (each slice voyage
harder than the last, a Runner with an escort clears the Warden about 40 %), shields that collapse under sustained
fire (about one wave in five at capacity 3, regen 0.2), encounter counts as the first knob; turn rates and test-range
waves stay. Nothing had been measured since [GDR-0074](../gdr/0074-test-range-waves-emergency-recall-cannon.md)
(`slice-balance-2026-10-13.md`, its last section). Since then: chain reactions v2 (GDR-0085), the speed governor and
the test range's waves by size (GDR-0089), the scanner in place of one of the starter's cargo bays (hold 8 → 4,
GDR-0093), the 16 Oct shield fix (a shell inside a bubble is taken by it) with the skiff's emitters moved forward
over its lasers, sensors (800 m) and hidden sites (GDR-0095), the slice's goals as ordinary sites and the Warden's
gate as the rim's first gate (GDR-0098).

**In short.**

- Before (today's `dev`, `fee9407`), clear rates over 240 seeds: the starter 96 / 62 / 40 / 27 %, the Halvex Runner
  66 / 68 / 63 / 32 %, a Runner with the starter as escort 86 / 79 / 73 / **62 %** at the Warden. The starter got
  much stronger since 13 Oct (58 / 37 / 15 / 3 %), the Runner did not; the Warden's gate was far easier than the
  curve.
- After (two counts): the starter 96 / 62 / 42 / **15 %**, the Runner 66 / 68 / **49** / **26 %**, Runner + starter
  86 / 79 / 62 / **44 % ± 6** at the Warden (two Runners 57 %). Every fleet finds each voyage harder than the one
  before, except the Runner's first two (66 and 68 %, within noise; the same inversion GDR-0073 accepted).
- Changed: `the_warden`'s raiders 1–2 → 2–3 (its escort count), Corran Reach's hunter packs 1 → 2.
- Counts cannot fix two things (they move both ships together; numbers below): raiders no longer threaten the
  starter (three packs of three on the first voyage still leave it 85 %; the Runner falls to 25 %), so the first two
  voyages stay well above the curve's 58 / 37 %; and a starter with the cannon (the first research node) alone
  clears the Warden 37 %, about as often as a Runner with an escort.
- Shields almost never collapse now: in a bubble in 0–9 % of the test range's waves (the Runner's 2 hunters and a
  raider under a passive pilot 8.5 %, flown closing 0 %, the skiff's hunter 0 %), against GDR-0061's 20 %. The fights
  are short (7–15 s). Capacity 2 gives 16.5 % under the passive pilot, capacity 1.5 31 %; not changed (B2 keeps 3 /
  0.2), open for Niklas.

## Method

The headless voyage runner (`crates/core/sim/examples/voyage.rs`, the pilot in `sim::voyage::runner`, unchanged
since GDR-0074's measurement apart from what the sim gained: sensors, hidden sites, the scanner), release build. Per
voyage and fleet 240 seeds (1–240, in two halves of 120 to show the spread); the same seeds and pilot before and
after. The steps of the search ran on seeds 1–120. Load average 0.3–1.9 (no other builds). Columns as in the
13 Oct report: **clear** = goal cleared and home, **home** = recalled without the goal, **lost**, **out of time**
(30 minutes). The ± is a 95 % interval (1.96 standard errors of a share of 240, 4–6 points): differences under
about 8 points are noise.

Fleets (as documented in the 13 Oct report): `rim_skiff` (the starter, now with a scanner and a 4-unit hold),
`skiff_cannon` (the starter with `automaton_cannon` in place of both pulse lasers: a blueprint file, the starter's
layout with the two parts swapped), `halvex_runner`, the Runner with `rim_skiff` as escort (the `escort` template),
and at the Warden two Runners.

## Before and after (240 seeds)

Clear / home / lost / out of time (of 240), clear rate before → after, and after on seeds 1–120 / 121–240. The first
two voyages are unchanged (the same content: before = after).

| Voyage | Fleet | Before | After | Clear % | Halves (after) |
|---|---|---|---|---|---|
| yard_approach | rim_skiff | 230 / 8 / 2 / 0 | 230 / 8 / 2 / 0 | 96 ± 3 | 94 / 98 |
| yard_approach | skiff_cannon | 237 / 3 / 0 / 0 | 237 / 3 / 0 / 0 | 99 ± 1 | 99 / 98 |
| yard_approach | Runner | 158 / 76 / 6 / 0 | 158 / 76 / 6 / 0 | 66 ± 6 | 64 / 68 |
| yard_approach | Runner + rim_skiff | 206 / 32 / 2 / 0 | 206 / 32 / 2 / 0 | 86 ± 4 | 84 / 88 |
| halvex_drift | rim_skiff | 150 / 85 / 5 / 0 | 150 / 85 / 5 / 0 | 62 ± 6 | 62 / 63 |
| halvex_drift | skiff_cannon | 177 / 58 / 5 / 0 | 177 / 58 / 5 / 0 | 74 ± 6 | 73 / 74 |
| halvex_drift | Runner | 164 / 67 / 9 / 0 | 164 / 67 / 9 / 0 | 68 ± 6 | 69 / 68 |
| halvex_drift | Runner + rim_skiff | 189 / 39 / 10 / 2 | 189 / 39 / 10 / 2 | 79 ± 5 | 80 / 78 |
| corran_reach | rim_skiff | 96 / 107 / 37 / 0 | 100 / 120 / 20 / 0 | 40 → 42 ± 6 | 42 / 42 |
| corran_reach | skiff_cannon | 148 / 72 / 20 / 0 | 119 / 101 / 20 / 0 | 62 → 50 ± 6 | 56 / 43 |
| corran_reach | Runner | 151 / 74 / 15 / 0 | 118 / 118 / 4 / 0 | 63 → **49** ± 6 | 48 / 51 |
| corran_reach | Runner + rim_skiff | 175 / 49 / 15 / 1 | 149 / 73 / 18 / 0 | 73 → 62 ± 6 | 62 / 62 |
| wardens_gate | rim_skiff | 65 / 154 / 21 / 0 | 36 / 168 / 36 / 0 | 27 → **15** ± 5 | 17 / 13 |
| wardens_gate | skiff_cannon | 102 / 114 / 24 / 0 | 88 / 118 / 34 / 0 | 42 → 37 ± 6 | 38 / 36 |
| wardens_gate | Runner | 77 / 149 / 14 / 0 | 62 / 168 / 10 / 0 | 32 → **26** ± 6 | 20 / 32 |
| wardens_gate | Runner + rim_skiff | 148 / 84 / 7 / 1 | 105 / 128 / 7 / 0 | 62 → **44** ± 6 | 39 / 48 |
| wardens_gate | Runner + Runner | 175 / 57 / 6 / 2 | 137 / 86 / 13 / 4 | 73 → 57 ± 6 | 57 / 58 |

As curves (clear %, the four voyages in order):

| Fleet | 13 Oct (GDR-0073/0074) | Before | After |
|---|---|---|---|
| rim_skiff | 58 / 37 / 15 / 3 (62 / 32 / 17 / 3 after GDR-0074) | 96 / 62 / 40 / 27 | 96 / 62 / 42 / 15 |
| skiff_cannon | – (68 / 53 / 35 / 18 after GDR-0074) | 99 / 74 / 62 / 42 | 99 / 74 / 50 / 37 |
| Runner | 60 / 67 / 45 / 20 (63 / 77 / 67 / 28) | 66 / 68 / 63 / 32 | 66 / 68 / 49 / 26 |
| Runner + rim_skiff | – / 78 / 65 / 42 (95 / 80 / 72 / 68) | 86 / 79 / 73 / 62 | 86 / 79 / 62 / 44 |

Scripted minutes (median of the runs home) stay 2.5–3.5 per voyage; Corran Reach with its second hunter pack 2.4 →
3.3 (doubled for a human 6.6, inside its `minutes: (5, 8)`). Step time median 0.004–0.007 ms, p95 0.009–0.015 ms.

## The search (seeds 1–120, clear of 120)

| Step | rim_skiff | skiff_cannon | Runner | Runner + skiff | Runner + Runner |
|---|---|---|---|---|---|
| V1 content (before) | 113 | 119 | 77 | 101 | |
| V1: goal pair → pack of 3 | 109 | 119 | 63 | | |
| V1: two packs, derelict, goal pack (9 raiders) | 102 | 108 | 30 | | |
| V2 content (before) | 74 | 88 | 83 | 96 | |
| V2: two raider pairs instead of one | 79 | 96 | 65 | 79 | |
| V3 content (before) | 48 | 77 | 70 | 87 | |
| V3: hunter packs ×2 (**chosen**) | 50 | 67 | 57 | 74 | |
| V4 content (before) | 33 | 57 | 33 | 78 | 87 |
| V4: the Warden's raiders 2–3 (**chosen**) | 20 | 45 | 24 | 47 | 68 |
| V4: the Warden with a hunter and one raider | 21 | 44 | 34 | 50 | 80 |

Raiders hardly touch the starter now: the 16 Oct move of its emitters puts both pulse lasers under the bubbles, and
it shoots a raider's nose cannon off before it is hurt (the runs clear sites with 0 kills and every raider
disarmed). The Runner still loses its nose beams first (GDR-0073) and comes home without the goal from 76 of 240
first voyages. So more raiders
cost the Runner three to six times what they cost the starter, and making the first voyages harder by raider counts
would make the Runner hopeless there. They were left as they are. The Warden's two variants move the fleets alike;
the escort count (a count, as B3 asks) was chosen. Hunters move both ships about equally, so the second hunter pack
brings the Runner's third voyage to the curve's 45 %.

## Shields (the balance harness, 200 seeds)

`../space-game-balance` (ADR-0025), brought up to today's sim and `crates/core/` layout (its commits `8856d0c`,
`2e8f771`), with the test range's real waves added (`sim::voyage::waves`: half the design's build points on the
biggest enemies first; the Runner meets two hunters and a raider, the skiff one hunter) and raider-only waves of 2–5.
Win / loss / timeout of 200, waves with at least one collapsed bubble:

| Setup | Win / loss / timeout | Over (won, median s) | Waves with a collapse |
|---|---|---|---|
| Runner, passive pilot, its wave (2 hunters, 1 raider) | 156 / 34 / 10 | 14.9 | **17 (8.5 %)** |
| Runner, closing pilot, its wave | 199 / 1 / 0 | 10.4 | 0 |
| rim_skiff, passive pilot, its wave (1 hunter) | 197 / 0 / 3 | 10.5 | 0 |
| rim_skiff, closing pilot, its wave | 200 / 0 / 0 | 7.2 | 0 |
| Runner, passive, 3 raiders (GDR-0061's measure: 20 %) | 199 / 0 / 1 | 11.5 | 0 |
| Runner, passive, 4 raiders | 193 / 3 / 4 | 14.3 | 1 |
| Runner, passive, 5 raiders | 156 / 36 / 8 | 16.9 | 23 (11.5 %) |
| rim_skiff, passive, 2 / 3 raiders | 200 / 0 / 0, 188 / 8 / 4 | 11.6, 16.0 | 0, 3 |
| Runner under `skirmisher`, 3 raiders | 76 / 81 / 43 | 36.1 | 15 (7.5 %) |

Waves end in 7–17 s (GDR-0061 measured 26–77 s contact to last kill), and the bubbles take 0.5–4.8 hit points a wave:
the lock feed-forward (GDR-0064), the raiders' 3 s of 9 attacking (GDR-0074) and the shell fix make fights short, and
a kinetic shot gives a bubble half its damage. The skiff's bubbles never empty. What would bring back one wave in
five (not changed; B2 keeps capacity 3 and regen 0.2), the Runner's wave under the passive pilot: capacity 2 → 16.5 %,
capacity 1.5 → 31 %, regen 0.1 → 14.5 %, capacity 2 with regen 0.1 → 28.5 %; win rates do not move (156 / 34 / 10
in each), and the closing pilot and the skiff stay at 0–2 %.

## Findings for Niklas

- **The first voyages are easy for the starter** (96 and 62 % against the curve's 58 and 37 %). Not by counts: the
  starter shrugs off raiders (above). It needs a part or layout change (the raider's aim at a shielded gun, or the
  starter's emitters) or a different first-voyage enemy; outside B3.
- **The cannon starter skips the Runner.** `skiff_cannon` alone clears the Warden 37 %, the Runner alone 26 %, the
  Runner with the plain starter as escort 44 %. A first research node that matches the mid-game fleet at the last
  voyage is a part-stat question (B4's neighbours: weapon numbers), not a count.
- **The Runner is not an upgrade against raiders**: 66 % on the first voyage against the starter's 96 %, about the
  same on the second (its nose beams, GDR-0073). It is better only against hunters (49 against 42 %).
- **Shields rarely collapse** (above); the numbers that would restore B2's one in five are listed.
- The evening estimate of the 13 Oct report is not recomputed (the hold is now 4 units and the first voyages are
  easier, so it is shorter than 2.7 h).

## Determinism

`scripts/determinism-check.sh` (release, native x86_64 vs wasm32-wasip1 under Node 24), all **IDENTICAL**: voyage
seeds 42 and 7 (1800 ticks, last `1ee2e676d71140a9` / `593ff73d1bb62349`), seed 42 at 900 ticks voyage
`8d0bbd96024e6e0e`, core `19fc1649dc96cb92`, salvage `fd1adeafbf7261f2`, behavior `b17220b1d4499f1d`; the probes
`warden` `068c80007134bdb6` and `close_quarters` `3adc763abcdabfab`, `runner_vs_raiders` `de48601be0d811c1`;
`--run mixed_trials` and `--run two_starts_locked`; `--galaxy 4` (seed 1 still `8a509f91bd8fd0b8…`). The scenario
and probe hashes are the same with the old content (the voyage scenario flies the first voyage, which did not
change). The content's fingerprint changes, so the rules' fingerprint does too: fight codes and lab runs made before
are stale (they replay with the note and restore read-only, `docs/arena.md` "Determinism").

## Reproduce

```sh
CARGO_BUILD_JOBS=2 cargo build --release -p sim --example voyage
target/release/examples/voyage --voyage wardens_gate --design halvex_runner --escort rim_skiff --seeds 1..240
target/release/examples/voyage --voyage corran_reach --design skiff_cannon.ron --seeds 1..240   # a blueprint file
# shields: in ../space-game-balance
CARGO_TARGET_DIR=$PWD/target cargo build --release
target/release/space-game-balance --setups range_runner_passive,range_runner_closing,range_skiff_passive,range_skiff_closing --variants base,cap=2,cap=1.5,regen=0.1 --seeds 1-200
```

`skiff_cannon.ron` is `content/base/ships/rim_skiff.ron` with `pulse_laser` replaced by `automaton_cannon` (its id
`skiff_cannon`). "Before" is `dev` `fee9407`.
