# The player's cannon toned down (18 Oct 2026)

After the second pass of 17 Oct (`slice-balance-2026-10-17b.md`) the cannon starter (`skiff_cannon`) cleared the
Warden 37 %, level with a Runner and an escort (42 %). Niklas decided to tone the player's cannon down so that a lone
cannon starter clears the Warden about 25–30 % and the Runner with an escort clearly beats it
([GDR-0100](../gdr/0100-fight-balance-targets.md)'s status).

**In short.**

- The player no longer builds the automatons' own gun. Research `automaton_cannon` ("Reverse-engineered cannon")
  now unlocks a new part, **`reverse_engineered_cannon`** (`parts/weapons.ron`): the same gun with **rate and damage
  1.2 → 1.1** (a shot still 1 hit point). `automaton_cannon` stays the automatons' (locked, unlocked by nothing), so
  the raiders, hunters, gunners and the Warden fire exactly as before and no enemy fight changed: the starter's,
  the Runner's and the Runner + escort's runs are identical to the 17 Oct report, run for run.
- Why a separate part: the research node is already a distinct item (the drydock only builds unlocked parts, a
  hauled cannon becomes kinetic data), and the shared gun would have moved every enemy that GDR-0101 had just tuned
  (the Warden fires five, the gunner two), so every voyage and count would need re-measuring.
- The cannon starter: 93 / 68 / 46 / 37 % → **91 / 63 / 38 / 28 %**. The Warden at 28 ± 6 % (seeds 241–480 give
  34 %; 31 % over 480 seeds); the Runner with the plain starter as escort 42 % now beats it by 14 points. Each voyage
  is still harder than the last, and the curve stays above the starter's (57 / 37 / 22 / 15).
- A Runner with the cannon starter as escort: 48 → 44 % at the Warden, now level with the plain escort.
- No count was re-tuned (no enemy changed).

## Method

As on 17 Oct: the headless voyage runner (`crates/core/sim/examples/voyage.rs`), release build, seeds 1–240 per voyage
and fleet, **clear** = goal cleared and home, **home** = recalled without the goal, **lost**, **out of time**. ± is a
95 % interval (about 6 points at 240 seeds). `skiff_cannon.ron` is `ships/rim_skiff.ron` with `pulse_laser`
replaced by `reverse_engineered_cannon` (its id `skiff_cannon`). The search ran in the balance harness's voyage mode
(`--content` a copy of the pack with the new part); its numbers match the example's run for run.

## The search (cannon starter, clear % on the four voyages, seeds 1–240)

| Rate and damage (a shot) | yard_approach | halvex_drift | corran_reach | wardens_gate | Warden, seeds 241–480 |
|---|---|---|---|---|---|
| 1.2 / 1.2 (1 HP, before) | 93 | 68 | 46 | 37 | 39 |
| **1.1 / 1.1 (1 HP)** | **91** | **63** | **38** | **28** | 34 |
| 1.05 / 1.05 (1 HP) | 88 | 59 | 33 | 29 | 32 |
| 1.0 / 1.0 (1 HP) | 89 | 58 | 26 | 21 | 28 |
| 0.9 / 0.9 (1 HP) | 82 | 50 | 10 | 21 | – |
| 0.8 / 0.8 (1 HP) | 73 | 35 | 8 | 10 | – |
| rate 1.2, damage 1.0 (0.83 HP) | 15 | 2 | 2 | 0 | – |
| rate 1.2, damage 0.9 (0.75 HP) | 12 | 3 | 2 | 1 | – |

A shot below 1 hit point no longer takes a 1-hit-point part off in one hit, and the gun collapses (0–15 %): only
the rate can come down. At 1.0 Corran Reach falls to the Warden's level (26 against 21 %; 27 / 25 % over 480
seeds), so the curve flattens; 1.1 keeps each voyage clearly harder than the last.

## Before and after (240 seeds, the game's example)

Clear / home / lost / out of time (of 240); halves are seeds 1–120 / 121–240.

| Voyage | Fleet | Before (`5922cf7`) | After | Clear % | Halves (after) |
|---|---|---|---|---|---|
| yard_approach | skiff_cannon | 223 / 13 / 4 / 0 | 219 / 15 / 6 / 0 | 93 → 91 ± 4 | 92 / 91 |
| halvex_drift | skiff_cannon | 164 / 44 / 32 / 0 | 151 / 45 / 44 / 0 | 68 → 63 ± 6 | 65 / 61 |
| corran_reach | skiff_cannon | 110 / 101 / 29 / 0 | 92 / 120 / 28 / 0 | 46 → 38 ± 6 | 38 / 39 |
| wardens_gate | skiff_cannon | 88 / 113 / 39 / 0 | 66 / 143 / 31 / 0 | 37 → **28** ± 6 | 26 / 29 |
| wardens_gate | Runner + skiff_cannon | 116 / 110 / 13 / 1 | 106 / 119 / 13 / 2 | 48 → 44 ± 6 | 44 / 44 |

Unchanged, run for run (the enemies and these fleets have no new part): rim_skiff 137 / 89 / 53 / 36 clears
(57 / 37 / 22 / 15 %), the Runner 177 / 156 / 95 / 61 (74 / 65 / 40 / 25 %), the Runner + rim_skiff 196 / 168 /
122 / 102 (82 / 70 / 51 / **42** %), two Runners at the Warden 130 (54 %).

| Fleet | Clear % (the four voyages) |
|---|---|
| rim_skiff | 57 / 37 / 22 / 15 |
| skiff_cannon | 93 / 68 / 46 / 37 → **91 / 63 / 38 / 28** |
| Runner | 74 / 65 / 40 / 25 |
| Runner + rim_skiff | 82 / 70 / 51 / 42 |

## Notes

- Saves: a career that researched the node now builds `reverse_engineered_cannon`; a saved design that carries
  `automaton_cannon` reads as a locked part in the drydock and must be refitted. The new part shows NEW once.
- The wiki gains the part's page (unlocked by the node); the automaton cannon's page no longer lists a node.

## Determinism

`scripts/determinism-check.sh`, native x86_64 vs wasm32-wasip1, all **IDENTICAL** and every last hash as on 17 Oct
(nothing in a scenario flies the new part; `docs/sim.md`). The content's fingerprint changes with the part, so the
rules' fingerprint does too: fight codes and lab runs made before replay with the note.

## Reproduce

```sh
CARGO_BUILD_JOBS=2 cargo build --release -p sim --example voyage
sed 's/"pulse_laser"/"reverse_engineered_cannon"/; s/id: "rim_skiff"/id: "skiff_cannon"/' \
  content/base/ships/rim_skiff.ron > skiff_cannon.ron
target/release/examples/voyage --voyage wardens_gate --design skiff_cannon.ron --seeds 1..240
target/release/examples/voyage --voyage wardens_gate --design halvex_runner --escort skiff_cannon.ron --seeds 1..240
```
