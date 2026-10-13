# Slice balance: the first playable loop measured and tuned (13 Oct 2026)

Tracks 7J/7K of Phase 7. The slice's content (the starter, four voyages, the encounters, the hunter and the Warden,
nine research nodes) measured with the headless voyage runner (`crates/sim/examples/voyage.rs`, `docs/sim.md` "The
headless voyage runner") against the evening of `docs/plans/first-playable-loop.md` 3.5, then tuned in content only.
The change set is proposed in [GDR-0073](../gdr/0073-slice-tuning-v1.md). Every number in the content stays a
placeholder; this report says how they play against each other for a scripted pilot.

**In short.**

- Before: the starter `rim_skiff` cleared the first voyage in 43% of runs and nothing after it; the Halvex Runner (the
  mid-game target) cleared 37% of the first voyage and none of the others; a Runner with an escort beat the Warden in
  2 of 60 runs. A run home brought ~9 research data over eight families, against node costs of 8–30 per family.
- Fights decide it. In a voyage a pack wakes together and charges; one slice ship beats two raiders about 90% of the
  time and three about half the time, and damage stays for the whole voyage. The Runner is not much stronger than
  the starter here: its nose beams (45 m) and the fusion reactor behind them are shot off by raiders that stand off
  at 64 m, and it collapses against three at once.
- After (content only: smaller packs and dens, the hunters and the Warden with fewer raiders, a pair as the first
  goal, costs about halved): the starter clears 58 / 37 / 15 / 3% of the four voyages, the Runner 60 / 67 / 45 /
  20%, a Runner with the starter as escort 42% of the Warden's gate (two Runners 70%). Two first voyages pay one
  node; with a cargo hold about four second voyages pay the Runner's four nodes.
- Evening estimate from these numbers: about **2.7 h** (scripted voyage times doubled for a human, plus launch,
  debrief, research and building), against the plan's ≈3 h.
- Not changed: the sandbox wave (its size is client code; the test range flies the player's own design, and the
  starter wins only 50% of waves of 3). Open: the reverse-engineered cannon is a step back for the scripted pilot; a
  disarmed ship cannot escape a raider of its own top speed.
- 14 Oct 2026, [GDR-0074](../gdr/0074-test-range-waves-emergency-recall-cannon.md): a disarmed fleet may always
  recall, the cannon is an upgrade (with `raider_basic` attacking 3 s of 9), test-range waves follow the flown
  design; see the last section.

## Method

The runner rolls a content voyage per seed (`VoyagePlan::roll`, the seed also seeds the `Sim`), puts the design at
the entry and flies it with a scripted pilot until it recalls, is lost or 30 minutes pass. Setups: 60 seeds each
(1–60); the same seeds and the same pilot before and after every content change; the steps of the search were
measured on 30 seeds (1–30) and the close calls again on 60 new ones (31–90). Release build on `dev` `a7749c4` plus
the runner commit; load average 6–9 from other agents' builds (the wall clock times only the steps; nothing else
depends on it). Columns: **clear** = goal cleared and home, **home** = recalled without the goal (the haul still
counts), **lost** = the flagship is no vessel any more (no haul), **out of time** = 30 minutes passed (in practice a
disarmed ship chased by a raider it cannot outrun and that keeps it from recalling: a lost haul for a player too).

**The pilot.** The first version (track 7E) met a waking pack at its top speed and, with engines only at the back,
could not brake with its nose on the target; switched its lock to the nearest raider every 10 ticks (each switch
costs a turn since GDR-0055); left disarmed enemies alone (they ram, and only a kill makes a wreck); filled the hold
with whatever piece was nearest; and closed to 75% of its *longest* gun's reach (a Runner's beams never reached).
With it, on the old content and 20 seeds, the starter cleared 6/20 first voyages, the Runner 3/20, and nothing else
was ever cleared. The pilot now (the example's doc has the details):

- stops 40 m outside a sleeping site's wake radius, then creeps in at a quarter of its top speed;
- in a fight: locks one armed enemy and keeps it while it is armed and within 400 m, fires, and pushes the stick
  towards it while it is farther than 75% of its *shortest* clear gun's reach, eased off over the last 40 m and never
  above 60% of the top speed;
- finishes off disarmed awake enemies within 200 m while the hold has room (gives up on one after 20 s);
- strips the salvage piece with the best value per cargo unit (the strip order's value), a point of value worth 50 m
  of the way; strips what is in reach before it recalls;
- goes home once the goal is cleared and nothing is left to do or the hold is full, when it is disarmed, or after 3
  minutes without progress; escorts (`--escort`) fly the `escort` template behind it and fight its lock.

**How a human differs.** The pilot never dodges, strafes or retreats to let a bubble recover; it holds its nose on the
target and drifts. It never picks fights (it clears every site), always salvages, and decides instantly. A human
flies slower between sites, salvages by hand (fiddly), but can pull back when a gun is shot off, fight one raider at a
time at the edge of a pack, and choose which wreck to strip. So clear rates for a careful human should be higher
than these and times about twice as long. The ratios between voyages and designs are what this report tunes.

**The designs.** `rim_skiff` (the starter: two pulse lasers, 24 hp, an 8-unit hold) and `halvex_runner` (the
mid-game target: composite, fusion, beams, torch drive; 72 hp, 8 units) are content. Two player designs, as blueprint
files (not content, kept here for reproduction):

- `skiff_hauler`: `rim_skiff` with a cargo hold under its gyros: `size: (8, 6)` and `(part: "cargo_hold", x: 3, y:
  4)` added (28 units, 5 t more).
- `skiff_cannon`: `rim_skiff` with `automaton_cannon` in place of its two pulse lasers (at (3, 0) and (4, 0)).

## Before and after (60 seeds, 1–60)

Per setup: clear / home / lost / out of time (of 60); median time of all runs (scripted minutes); median hull lost
of the runs home (of 24 or 72); research data per run home (and programmable matter).

| Voyage | Design | Before | After | Time | Hull lost (home) | Data per home run (PM) |
|---|---|---|---|---|---|---|
| yard_approach | rim_skiff | 26 / 19 / 3 / 12 | **35 / 15 / 2 / 8** | 3.5 → 3.4 | 4.0 → 3.4 | 8.7 (1.8) → 10.2 (2.0) |
| yard_approach | Runner | 22 / 26 / 7 / 5 | 36 / 17 / 5 / 2 | 3.8 → 3.3 | 48.5 → 11.0 | 9.4 (1.2) → 10.8 (1.5) |
| halvex_drift | rim_skiff | 0 / 32 / 11 / 17 | **22 / 14 / 12 / 12** | 3.3 → 3.0 | 17.9 → 7.1 | 4.9 (0.9) → 11.7 (1.1) |
| halvex_drift | skiff_hauler | 1 / 35 / 7 / 17 | 15 / 26 / 8 / 11 | 3.1 → 3.7 | 21.8 → 9.8 | 8.2 (1.0) → **26.3** (1.8) |
| halvex_drift | Runner | 0 / 42 / 15 / 3 | 40 / 15 / 4 / 1 | 3.4 → 3.2 | 59.3 → 13.9 | 5.9 (0.8) → 15.4 (1.1) |
| corran_reach | rim_skiff | 0 / 32 / 12 / 16 | 9 / 27 / 9 / 15 | 3.1 → 2.7 | 19.9 → 15.8 | 4.1 (1.0) → 8.7 (0.7) |
| corran_reach | Runner | 0 / 47 / 8 / 5 | **27 / 24 / 5 / 4** | 3.5 → 2.6 | 58.6 → 29.8 | 3.8 (0.7) → 11.9 (0.8) |
| wardens_gate | rim_skiff | 0 / 37 / 4 / 19 | 2 / 31 / 8 / 19 | 3.3 → 3.4 | 19.5 → 18.0 | 3.1 (1.1) → 8.9 (1.4) |
| wardens_gate | Runner | 0 / 47 / 10 / 3 | 12 / 39 / 6 / 3 | 3.2 → 3.2 | 61.2 → 42.7 | 4.1 (0.8) → 11.3 (1.8) |
| wardens_gate | Runner + rim_skiff | 2 / 41 / 13 / 4 | **25 / 29 / 2 / 4** | 4.2 → 3.2 | 60.0 → 32.7 | 7.9 (1.6) → 13.5 (2.3) |
| wardens_gate | Runner + Runner | 2 / 43 / 12 / 3 | 42 / 16 / 2 / 0 | 4.6 → 2.8 | 53.5 → 17.6 | 7.6 (1.5) → 15.9 (2.2) |

After only (same seeds): the Runner with the starter as escort clears halvex_drift 47/60 (78%) and corran_reach 39/60
(65%); `skiff_cannon` clears yard_approach 7/60 (12%, 26 out of time) and halvex_drift 11/60 (18%).

As clear rates: the starter 58 / 37 / 15 / 3%, each voyage harder than the last; the Runner 60 / 67 / 45 / 20% (the
first voyage's three fights cost it as much as the second's two and a half; it arrives after the second anyway); the
Warden falls to a Runner alone one run in five, to a Runner and an escort two in five, to two Runners seven in ten.
Kills per run 1.5–4.1; step time median 0.007–0.009 ms, p95 0.013–0.016 ms.

Where the data comes from (per run home, after): the first voyage gives logic 2.4, propulsion 2.3, hull 1.5, power
1.4, energy 1.0, kinetic 0.8 and 2.0 programmable matter; the second with a hold power 5.7, energy 5.3, logic 4.5,
propulsion 4.0, hull 2.9, kinetic 1.4 (the precursor wrecks); the Warden's gate logic 5.6–6.6, kinetic 2.5–3.3 (the
graveyard). The hold is the limit: the starter comes home with 6.3 items in its 8 units on the first voyage
(fragments and cores go first), 10 with a hold on the second.

## The search (30 seeds unless noted)

Each step on top of the one before, starter / Runner clear rate of the voyage changed:

| Step | yard_approach | halvex_drift | corran_reach | wardens_gate (Runner / + starter / + Runner) |
|---|---|---|---|---|
| content before | 43% / 37% (60 seeds) | 0 / 0 | 0 / 0 | 0 / 3% / 3% (60) |
| V1: one pair, derelict, goal pack of 3 | 47% / 43% | | | |
| V1: two pairs, derelict, goal pair (chosen) | 60% / 63% | | | |
| V1: one pair, derelict, goal pair | 63% / 83% | | | |
| V2: two pairs, precursor and graveyard with 0–1 raider, den of 3 | | 27% / 10% | | |
| V2: one pair, the same, den of 3 | | 23% / 60% | | |
| V2: one pair, den of 2–3 (chosen) | | 30% / 70% | | |
| V3: hunter pack of 1 + 1, a raider pair, precursor, den of 2 hunters + 0–1 raider | | | 17% / 27% | |
| V3: hunter packs of 1 + 1 ×2, precursor, den of 2 + 0–1 | | | 3% / 23% | |
| V3: hunter pack of 1 + 1, precursor, den of 2 hunters | | | 17% / 57% | |
| V3: the same, den of 2–3 hunters (chosen; 60 seeds) | | | 15% / 45% | |
| V3: hunter packs ×2, den of 2 (60 seeds) | | | 15% / 38% | |
| V4: one pair, graveyard 0–1, Warden + 0–1 raider | | | | 43% / 73% / 90% |
| V4: two pairs, Warden + 1–2 raiders (60 new seeds) | | | | 13% / 32% / – |
| V4: one pack of 3, Warden + 1–2 raiders (chosen; 60 new seeds) | | | | 15% / 35% / – |

## Research costs and the evening

Node costs were set so that the evening's order works for a player who adds a cargo hold after the first voyage
(the debrief says the hold is full; GDR-0063 makes cargo a design trade-off):

| Node | Before | After | Paid by (expected) |
|---|---|---|---|
| Reverse-engineered cannon | kinetic 12 | kinetic 3 (one cannon) | a cannon that survives: 1–2 first voyages (40% after two), surely in the second |
| Composite plating | hull 20 | hull 8 | the slowest of the four: ~4 second voyages with a hold (plus the first two) |
| Beam laser | energy 18 | energy 10 | ~3 second voyages with a hold |
| Fusion reactor | power 20 | power 10 | ~3 |
| Torch drive | propulsion 20 | propulsion 10 | ~3 |
| Tamed automaton core | logic 8 + PM 1 | logic 3 + PM 1 | two first voyages (logic 4.0, PM 3.3 expected) |
| Mind expansion I | logic 10 + PM 2 | logic 10 + PM 3 | by the end of the second voyages |
| Mind expansion II | logic 20 + PM 4 | logic 16 + PM 5 | ~3 runs at the Warden's gate (logic 4.8 per run), which are also tries at the Warden alone |
| Cruiser drydock | hull 30 | hull 20 | later (not on the way to the Warden) |

Expected data per run (data per home run × the share of runs home): two first voyages bring logic 4.0, propulsion 3.8,
hull 2.5, power 2.3, energy 1.7, kinetic 1.3, PM 3.3: the tamed core (or, 40% of the time, the cannon). Four second
voyages with a hold add power 15.6, energy 14.4, logic 12.4, propulsion 10.8, hull 8.0, kinetic 4.0, PM 4.9: the
Runner's four nodes, the cannon and Mind expansion I. Mind expansion II (for a 2-core escort beside the Runner) then
needs about 13 more logic.

The evening, from the numbers (scripted minutes × 2 for a human, plus 2 minutes per run for the launch, the warp
and the debrief):

| Step | Runs | Minutes |
|---|---|---|
| New game, look at home, first launch | – | 5 |
| Shipyard approach (58% per run: 82% cleared within two) | 2 | 18 |
| First research, first rebuild (a cargo hold), test range | – | 15 |
| Halvex Drift with a hold (25–37% per run; the Runner's data) | 4 | 38 |
| Research beams, fusion, composite, torch drive; build a Runner-like ship | – | 20 |
| Corran Reach with the Runner (45%: 70% within two) | 2 | 14 |
| Data runs for Mind expansion II (the Warden's gate alone: 20% to win) | 3 | 25 |
| The escort's design and behavior | – | 10 |
| The Warden with the Runner and the starter as escort (42%) | 2.4 | 20 |
| **Total** | **~13** | **≈ 165 (2.7 h)** |

With a human factor of 1.5 instead of 2: about 2.3 h; with 3: about 3.4 h. The plan's table has 15 + 35 + 30 + 40 =
120 minutes of voyages; these numbers give about 115.

## The sandbox wave

The basic wave after GDR-0064 (the balance harness, `../space-game-balance`, 200 seeds, win / loss / timeout %,
`raider_basic` as in content; the harness copy used here has setups for 4 and 5 raiders and for the starter):

| Setup | 3 raiders | 4 raiders | 5 raiders |
|---|---|---|---|
| Runner, passive pilot | 98 / 2 / 0 | 88 / 6 / 6 | 61 / 36 / 4 |
| Runner, closing pilot | 98 / 2 / 0 | 86 / 10 / 4 | 58 / 36 / 6 |
| Runner under `skirmisher` | 50 / 24 / 26 | 10 / 64 / 26 | – |
| rim_skiff, passive pilot (2 raiders: 98 / 0 / 2) | 50 / 42 / 8 | 10 / 83 / 7 | – |
| rim_skiff, closing pilot (2 raiders: 97 / 2 / 2) | 54 / 36 / 10 | 24 / 72 / 4 | – |

Raider numbers (content) instead, wave of 3, passive / closing pilot: orbit 58% (now) 98 / 98; 62%: 92 / 98; 64%: 51 /
98. Only the passive stand-in feels the orbit, and 64% is 3–6 points from the 67–70% cliff GDR-0061 moved away from.

Not changed: the wave size is a client default (`crates/game/src/tuning.rs`, `raiders_per_wave`, `?raiders=`), and
since Phase 7 the test range flies the player's own design: a wave of 4 would be fair for a Runner (86–88%) but
crushing for the starter (10–24%). Raider numbers that would make the wave fairer move the voyages too, which are
already hard in their geometry (a pack at rest that charges together: the closing Runner wins 70–74% against three,
the starter about half). Open for Niklas (below).

## Findings and open questions

- **The cannon is a trap.** The first research node (the raiders' cannon, 75 m, projectiles at 90 m/s) makes the
  starter weaker for the scripted pilot: `skiff_cannon` clears 12% of the first voyage against 58% with pulse lasers,
  and is out of time in 26 of 60 runs. Its shots miss orbiting raiders, and at 56 m it stands where the raiders' fire
  is best. A human may lead better; the node still gives no clear upgrade.
- **A disarmed ship cannot get away.** 8–30% of the starter's runs end out of time: its guns (1 hp each, in its nose)
  are shot off, it runs, and a raider with the same top speed (20 m/s) follows at ~90 m, inside the recall radius
  (200 m), forever. The player's only way out is abandoning the voyage (M twice, GDR-0072), losing the haul.
- **The Runner is fragile against three.** Against a pack of three in voyage geometry the closing Runner wins 70–74%
  (the harness, packs at rest 250 m ahead) and loses 45–60 hull when it goes wrong: the raiders stand off at 64 m and
  shoot the nose beams, then the fusion reactor behind them (volatile). Against pairs it loses ~11.
- **The hold is the haul.** Data per run is limited by 8 units (fragments and cores first). The costs assume one
  cargo hold after the first voyage; a skiff with a hold is a little weaker in fights (25% vs 37% on the second
  voyage).
- **Escorts help.** The starter as escort lifts the Runner from 20% to 42% at the Warden and from 45% to 65% at
  Corran Reach; a second Runner to 70%.

## Determinism

`scripts/determinism-check.sh --scenario <s> --seed <n> --ticks 1800`, release, native x86_64 vs wasm32-wasip1 under
Node 24: voyage, salvage, core and behavior, seeds 42 and 7: **IDENTICAL** (8 runs). Compared with `dev` `a7749c4`'s
content (native streams), only the voyage scenario's hashes changed (from tick 1: the plan of the first voyage is in
the hash; last tick seed 42 `9d89d5a1c016a4dd` → `ad6f866e1ebd4a03`, seed 7 `2b382061be6af7b3` →
`6527a815b69705c0`); salvage, core and behavior are bit-identical (no ship or part changed). Research costs are not in
the sim.

## Reproduce

```sh
cargo build --release -j 6 -p sim --example voyage
target/release/examples/voyage --voyage halvex_drift --design rim_skiff --seeds 1..60
target/release/examples/voyage --voyage wardens_gate --design halvex_runner --escort rim_skiff --seeds 1..60
target/release/examples/voyage --voyage halvex_drift --design skiff_hauler.ron --seeds 1..60   # a blueprint file
```

"Before" is `dev` `a7749c4` with the runner commit and the old content (`git show a7749c4:content/base/...`); "after"
is this change. The sandbox numbers come from a copy of `../space-game-balance` pointed at this worktree, with the
orbit variant's base rule moved to 58% and the shield defaults to 3 / 0.2 (the content since GDR-0061), setups for
waves of 4 and 5 and for the starter, and an environment switch for packs at rest 250 m ahead of a moving Runner.

## GDR-0074 (14 Oct 2026): emergency recall, the cannon, test-range waves

[GDR-0074](../gdr/0074-test-range-waves-emergency-recall-cannon.md) answered the three open findings above. Same
runner, pilot and seeds (1–60), release build on `dev` `19f252c` plus this change, load average about 1–2.

**Emergency recall** (`Sim::can_recall`: also when none of the owner's vessels has a working gun). The pilot already
goes home when disarmed; now the sim lets it. Old content, before → after the rule (clear / home / lost / out of time):

| Voyage | `rim_skiff` | Halvex Runner | Runner + `rim_skiff` |
|---|---|---|---|
| yard_approach | 35 / 15 / 2 / 8 → 37 / 22 / 1 / 0 | 36 / 17 / 5 / 2 → 36 / 21 / 3 / 0 | – → 53 / 6 / 1 / 0 |
| halvex_drift | 22 / 14 / 12 / 12 → 22 / 33 / 5 / 0 | 40 / 15 / 4 / 1 → 40 / 17 / 3 / 0 | – → 47 / 8 / 2 / 3 |
| corran_reach | 9 / 27 / 9 / 15 → 9 / 48 / 3 / 0 | 27 / 24 / 5 / 4 → 27 / 28 / 3 / 2 | – → 39 / 17 / 1 / 3 |
| wardens_gate | 2 / 31 / 8 / 19 → 2 / 57 / 1 / 0 | 12 / 39 / 6 / 3 → 12 / 44 / 4 / 0 | 25 / 29 / 2 / 4 → 24 / 32 / 1 / 3 |

The runs that ended chased until the time ran out now come home with their haul; fewer are lost (a disarmed ship
warps out instead of being rammed apart). Clears are unchanged. With an armed escort the fleet is still armed, so a
few escort runs still end out of time.

**The cannon.** In content `damage` is per second and a projectile deals `damage / rate`: the old cannon's 1.2 at
1.5 shots/s is 0.8 per shot, two hits for a 1-hit-point gun. That is why `skiff_cannon` lost: a raider's nose cannon
took two hits to come off while the starter's pulse lasers took two raider hits each. Damage per second barely
matters; a shot of a full hit point does (a starter with cannons, the raiders unchanged, 60 seeds: damage 1.2–1.49 at
rate 1.5 clears 5–6, 1.5 clears 50; rate 3 at the same shot clears 12; speed 160 alone clears 13). But the raiders,
the hunters and the Warden carry the same gun, and a one-hit raider shot takes the starter's lasers and the Runner's
beams off just as fast (the stock starter fell from 37 to 16–22). So the change comes in two parts:

- `automaton_cannon`: rate 1.5 → 1.2, projectile speed 90 → 150 m/s, damage per second 1.2 (unchanged), range 75:
  a shot of 1 hit point, fewer and faster shots (kinetic: heavy rounds, recoil as before).
- `raider_basic`: attacks for 3 s of every 9 instead of 5 (orbits for the other 6). Searched with the cannon above:
  4 s left the Runner at 19–21 of 60 on the first voyage, 3.5 s at 24; 3 s was the only value tried that kept every setup
  within noise of where it was (±4 of 60) or above.

Goal cleared and home (of 60, emergency recall in both), before → after; `skiff_cannon` is `rim_skiff` with
`automaton_cannon` in place of both pulse lasers:

| Voyage | `rim_skiff` | `skiff_cannon` | Halvex Runner | Runner + `rim_skiff` |
|---|---|---|---|---|
| yard_approach | 37 → 37 | 5 → **41** | 36 → 38 | 53 → 57 |
| halvex_drift | 22 → 19 | 9 → 32 | 40 → 46 | 47 → 48 |
| corran_reach | 9 → 10 | 5 → 21 | 27 → 40 | 39 → 43 |
| wardens_gate | 2 → 2 | 1 → 11 | 12 → 17 | 24 → 41 |

On 60 other seeds (61–120): `rim_skiff` 25 → 35 and `skiff_cannon` 5 → 45 on the first voyage, the Runner 28 → 38,
Runner + starter at the Warden's gate 24 → 32. So the cannon is now the upgrade GDR-0074 asks for (first voyage 41
and 45 against the stock starter's 37 and 35), and the raiders are not stronger: about the same for the starter,
weaker for the Runner from the third voyage on (the Warden with a Runner and an escort 40% → 68% on seeds 1–60,
40% → 53% on 61–120). The Warden's gate is now easier than the GDR-0073 curve meant; its pack or the Warden's
escort count is the knob if that matters in play.

**Sandbox waves** (the balance harness, a copy pointed at this worktree with setups for 1–6 raiders and the
starter; 200 seeds, win / loss / timeout %, before → after the cannon and `raider_basic` change):

| Setup | 2 raiders | 3 raiders | 4 raiders | 5 raiders | 6 raiders |
|---|---|---|---|---|---|
| Runner, passive pilot | 100 → 100 | 98 → 100 | 88 → **96** | 61 → 78 | 34 → 46 |
| Runner, closing pilot | – | 98 → 100 | 86 → **94** | 58 → 86 | – |
| rim_skiff, passive pilot | 98 → **96** | 50 → 67 | 10 → 16 | – | – |
| rim_skiff, closing pilot | 97 → **88** | 54 → 54 | 24 → 18 | – | – |
| Runner under `skirmisher`, 3 raiders | | 50 / 24 / 26 → 54 / 28 / 18 | | | |
| 3 Sample early under `skirmisher` vs 3 | | 58 / 24 / 19 → 32 / 46 / 22 | | | |
| Sample early under `skirmisher` vs 1 | 23 / 70 / 7 → 16 / 80 / 4 (1 raider) | | | | |

AI-flown early ships with 1-hit-point guns lose more (a one-hit raider shot); piloted ships and the Runner win more.

**Test-range waves follow the flown ship** (`scene::wave_size`, placeholder): the design's build points over a
raider's (15.8), halved and rounded, at least 1, at most 8. The starter (59) meets 2 raiders (96% / 88% won above),
the Runner (132.5) 4 (96% / 94%), `sample_mid` (112) 4, `sample_early` (50) 2. Halving keeps both slice ships at
about nine wins in ten; the starter's next size up (3) it wins 54–67%. The tuning panel's raiders per wave (and
`?raiders=N`) still sets a size by hand.

Determinism: `scripts/determinism-check.sh --ticks 1800`, voyage, salvage and core, seeds 42 and 7: **IDENTICAL**
(native vs wasm32-wasip1). Last ticks: voyage `ef60b15278439964` / `5ace45868f103729`, salvage `970981a54a9a4882` /
`35ebab6317c1cb03`, core `d787112be99026e9` / `9ebd8fad6fa9bcc6`.
