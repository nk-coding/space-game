# Arena (`crates/arena`)

Headless, deterministic fights between designs on the real simulation: track **L1** of the evolution lab
(`docs/plans/evolution-lab.md`), written 14 Oct 2026. Decisions: [ADR-0036](adr/0036-arena-crate.md) (the crate),
[GDR-0083](gdr/0083-evolution-lab.md) (the lab), [GDR-0032](gdr/0032-balancing-arena-evolution.md) (balance by arena
and evolution); the fight spec and the score follow `docs/plans/coop-arena.md` 3.7 and 5.7. Every number marked
*placeholder* is for feel.

The crate depends on `data` and `sim` (plus `serde` and `ron`, already used by `data`); no Bevy, no I/O, no threads,
no wall clock in the library; it builds for `wasm32-unknown-unknown`. `crates/arena/clippy.toml` carries the sim's
bans on std's transcendental float functions. The native CLI is the crate's binary (`src/bin/arena.rs`, below).

## The fight spec

A `FightSpec` is plain data with a RON text form (`FightSpec::from_ron`, `to_ron`; read with the content's RON
options: `Some(..)` may be left out, unknown fields are errors, comments are fine). It is what a lab card will replay
and what a fight code will carry later.

```ron
// crates/arena/fights/runner_vs_raiders.ron
(
    seed: 1,
    sides: [
        (entries: [(design: Ship("halvex_runner"))]),
        (entries: [(design: Ship("automaton_raider"), count: 3)]),
    ],
    start_distance: 100.0,   // optional, default 100 m (placeholder)
    time_limit: 90.0,        // optional, default 90 s (placeholder)
)
```

| Field | What |
|---|---|
| `seed` | The world seed: the sim's random streams and the arena's spawn jitter |
| `sides` | Exactly two sides, each a list of `entries`, placed one behind the other in order (the first in front) |
| `design` | `Ship("<content id>")` or `Blueprint("<blueprint RON>")`: a saved or exported drydock design, the format of `content/base/ships/*.ron`, checked with `data::validate_ship` |
| `behavior` | Optional. `Content("<id>")` or `Text("<program>")` (the text form of `docs/behavior-format.md`). Left out: the design's own behavior, else its faction's default |
| `count` | Ships of this entry, default 1 (at most 64 per side) |
| `formation` | `Line` (side by side across the line of the fight, the default) or `Column` (one behind the other) |
| `start_distance` | Metres between the two sides' front rows (centre to centre), above 0, at most 5000 |
| `time_limit` | Seconds, above 0, at most 1800 |

Blueprint and behavior texts are written as multi-line strings by `to_ron`, so a written spec stays readable.

**Placement.** Side 0 stands at the origin with its noses to +y, side 1 is the same layout turned half a turn about
the middle of the field, `start_distance` ahead. Ships of an entry stand `FORMATION_GAP` (5 m, placeholder) apart
by their grid bounds; the next entry stands the same gap behind. Every ship is moved by up to `JITTER` (2 m,
placeholder) in x and y, rolled on the sim's `Scenario` stream from the seed: a plain fight rolls nothing in the sim,
so without the jitter every seed would give the same fight.

**Owners.** Side 0 is `Owner(1)`, side 1 `Owner(2)` (`arena::OWNERS`): neither is the player's `Owner(0)`, so both
sides play by the same rules (only the player collects salvage). Both get unlimited mind capacity: every core runs.
Every ship flies its behavior from the first tick; nobody pilots.

**Spawn order.** Side 0 spawns first, so its ships get the lower ids; the sim gives lower ids no edge
([ADR-0037](adr/0037-simultaneous-fire-within-a-tick.md)). Until 14 Oct 2026 it did: in a mirror match of two
`rim_skiff`s the side spawned first won 1173 of 2000 fights and the other 378 (449 undecided), and spawning side 1
first turned it round (425 / 1141), so the arena alternated the spawn order by seed parity. Three id-order effects in
the sim caused it, measured one at a time (2000 skiff mirrors and 1000 `automaton_hunter` mirrors, side 0 spawned
first / side 1 spawned first; won by side 0 / side 1 / undecided):

| Sim | Skiff mirror, side 0 first | Side 1 first | Hunter mirror, side 0 first | Side 1 first |
|---|---|---|---|---|
| Before | 1173 / 378 / 449 | 425 / 1141 / 434 | 286 / 81 / 633 | 77 / 282 / 641 |
| Simultaneous fire (hits applied after every gun has fired) | 863 / 392 / 745 | 450 / 835 / 715 | | |
| Shared think ticks alone (fire still in id order) | 1181 / 0 / 819 | 0 / 1167 / 833 | | |
| Both | 0 / 0 / 2000 | 0 / 0 / 2000 | 353 / 49 / 598 | 46 / 344 / 610 |
| All three (`orbit(auto)` by both ids) | 0 / 0 / 2000 | 0 / 0 / 1000 * | 0 / 0 / 1000 | 1 / 0 / 999 * |

(\* 1000 seeds, the spawn jitter rolled per side so that both orders place the ships alike; side 0 first gave
0 / 0 / 1000 and 1 / 0 / 999 that way. The rows above it roll the jitter in spawn order, as the arena did.)
With all three, a fight with the same placement gives each side the same end in either spawn order (1000 of 1000
seeds for the skiff, hunter and raider mirrors; `tests/fights.rs` checks it). Equal ships now open fire on the same
tick and shoot each other's nose guns off together: every skiff mirror is a draw at 0.500 / 0.500. Left over is the
physics: Rapier's contact solver resolves a collision a little differently depending on the bodies' order, so
head-on rams can still tip a fight (`sample_mid` mirror: 37 of 1000 seeds end differently with the spawn order
swapped, 22 / 4 against 3 / 16 wins; the Runner mirror 4 of 1000).

## A fight

`Fight::new(&content, &spec)` checks the spec and builds the world as the load test does
(`sim::scenario::load_test_with`): the content's `BehaviorLibrary` plus the spec's behavior texts, each entry's design
compiled once (`Arc<ShipDesign>`), ships spawned and commanded. `fight.step(ticks)` runs up to that many ticks and
returns `Some(FightResult)` once the fight has ended (later calls step nothing and return the same result), so a
client can run a slice per frame; `fight.sim()` is the world for drawing. `run_fight(&content, &spec)` does it in one
go. The sim's per-tick state hash runs only in debug builds (as in every runner); the result's `state_hash` is taken
once at the end.

**The end** (coop-arena 3.7): a side is out once it has had no running vessel (`VesselState::Running`) for 5 s
(`OUT_AFTER`, placeholder), so a short brown-out is no loss. If one side is out while the other still has a running
vessel, the other wins (`Outcome::Won(side)`); if neither has one, `Outcome::BothOut`; otherwise the fight ends at the
time limit (`Outcome::TimeLimit`). A disarmed vessel that still runs keeps its side in the fight until the time
limit.

**The result** (`FightResult`): the outcome (`winner()`), the ticks, the final state hash and per side
(`SideResult`):

| Field | What |
|---|---|
| `build_points` | Build points of the side's vessels at the start (each part's `build_points` times its material's factor) |
| `build_points_lost` | Build points no longer on one of the side's vessels: parts destroyed, and parts on pieces that broke off as salvage or lost their last logic core |
| `damage_dealt` | Damage the side's shots and engine plumes did to the other side's ships, after shields (`SimEvent::Hit`, `PlumeHit`; ram damage is not counted, it hits both hulls alike) |
| `vessels_left` | Vessels at the end (running, dark or dormant) |
| `score` | The arena score |

**The score** (`arena_score`, coop-arena 3.7, the recommended first row): a win 1, a loss 0. Otherwise (the time
limit, or both out) 0.5 + 0.5 × (share of the enemy's build points lost − share of its own lost), clamped to 0..1;
the two sides' scores then add up to 1. `run_seeds(&content, &spec, &seeds)` runs the spec once per seed;
`Summary::of(&results)` counts wins and undecided fights and averages the scores and the length.

**Errors** are values (`ArenaError`, with `Display`): a spec that does not read, a number out of range, an empty side,
a count of 0 or too many ships, an unknown ship or behavior id, a blueprint text that does not read, fails the content
checks or does not compile, a behavior text that does not parse or check (by text line), a design without a default
behavior, content behaviors that do not compile. Errors name the side and entry.

## The CLI

The crate's binary (`src/bin/arena.rs`), not an example: it is the arena's tool for balance numbers and later
`arena evolve` (like `content-tool`), not a one-off measurement; the examples hold checks (`fight_hashes`).

```sh
cargo run --release -p arena -- fight crates/arena/fights/runner_vs_raiders.ron --seeds 200
cargo run --release -p arena -- matrix halvex_runner automaton_raider rim_skiff my_design.ron --seeds 50
```

- `arena fight <spec.ron> [--seeds N] [--threads N]`: runs the spec with its seed and the N − 1 after it, prints a
  Markdown row per seed (outcome, time, build points lost of the start, damage dealt, vessels left, score, hash) and a
  summary (wins per side, undecided, mean score, mean time).
- `arena matrix <design>... [--seeds N] [--seed S] [--distance M] [--time S] [--threads N]`: every pair of designs
  (a content ship id, or a blueprint file `*.ron`) one against one under their default behaviors, N seeds (from S,
  default 1) in each starting position, and each design once against itself. Prints the row design's win rate and
  mean score against the column's; the diagonal is side 0's in the mirror match.
- Both run on `std::thread::available_parallelism` threads (`std::thread::scope`, each thread takes the next fight)
  and print fights per second and million ticks per second. Only that line depends on the machine.

```text
| vs | halvex_runner | automaton_raider | rim_skiff | sample_mid | automaton_hunter |
|---|---|---|---|---|---|
| halvex_runner | 0% (0.50) | 100% (1.00) | 100% (1.00) | 64% (0.79) | 100% (1.00) |
| automaton_raider | 0% (0.00) | 0% (0.50) | 3% (0.05) | 4% (0.18) | 60% (0.64) |
| rim_skiff | 0% (0.00) | 92% (0.95) | 0% (0.50) | 0% (0.22) | 92% (0.94) |
| sample_mid | 9% (0.21) | 68% (0.82) | 33% (0.78) | 1% (0.50) | 66% (0.77) |
| automaton_hunter | 0% (0.00) | 27% (0.36) | 5% (0.06) | 6% (0.23) | 0% (0.50) |
```

(`--seeds 100`, 2500 fights, 14 Oct 2026 content with ADR-0037; 1v1 at 100 m, 90 s.) Before ADR-0037 the diagonal
was 21 / 17 / 36 / 0 / 9 % (mean scores 0.45–0.52, the spawn order alternating by seed); now every mirror ends at
0.50 (`sample_mid` wins one fight in a hundred by a ram, see the spawn order above). Fights with the hunter moved
most (it orbits under `raider_basic`, and `orbit(auto)` now goes by both ids): the raider against it 22 → 60 %,
`sample_mid` 52 → 66 %, the starter 84 → 92 %; the Runner against `sample_mid` 74 → 64 %; the rest within a few
fights in a hundred.

## Determinism

The same spec gives the same result and hash on every platform: the arena adds only spawn positions (`+ − × ÷` on
the sim's PCG stream) and reads the sim. `scripts/determinism-check.sh --fight NAME [--seed N]` builds the
`fight_hashes` example (the specs of `crates/arena/fights/`, compiled in) natively and for `wasm32-wasip1`, runs both
until the fight ends and compares the per-tick hashes.

**Result (14 Oct 2026, release builds, `dev` with the arena):** native x86_64 vs wasm32-wasip1 under Node 24
**identical** for `runner_vs_raiders` seeds 1 (a loss, 2624 ticks), 2 (a loss, 1855), 11 (the time limit, 2700) and
`skiff_mirror` seeds 3 and 42 (wins, 551–552 ticks); also before the spawn-order change, `runner_vs_raiders` seeds
1, 5, 6 and `skiff_mirror` 1, 7. The final hashes equal the CLI's for the same seeds.

**With ADR-0037** (14 Oct 2026, release builds; simultaneous fire, shared think ticks, auto orbits by both ids, side 0
spawned first): **identical** for `runner_vs_raiders` seeds 1 (the time limit, 2700 ticks, `21a0ee0c17aa5233`), 2 (a
loss, 2661, `3d1b0698c87bd461`), 11 (a loss, 1738, `293ae01a0f0b6d2a`) and `skiff_mirror` seeds 3 (the time limit,
2700, `988f8d8fc87fa72e`) and 42 (the time limit, 2700, `57cb79ccd322c539`), the same final hashes as the CLI's.

`tests/fights.rs`: the same spec twice gives the same result and hash, another seed another hash; stepping in slices
gives the same result as one go; the starter against one raider ends; a short time limit ends undecided at 0.5 each;
the score's edge cases; seeds averaged; the spec's RON round trip (blueprint and behavior texts included); every
error kind; the sides' placement; and mirror fights (the starter's and the hunter's, 4 seeds each, 30 s) ending the
same for each side when the ships are spawned again in the opposite order (the swap shows any id-order effect
exactly, in a debug build in well under a second, so it runs with every `cargo test`).

## Performance

Native release, `arena fight --seeds 4000` on all 16 threads (AMD Ryzen 9 9950X3D, 16 vCPUs), 7 runs each, load
average 6–12 (other agents building, and the runs themselves), 14 Oct 2026, before ADR-0037:

| Fight | Mean length | Fights/s (16 threads), median (range) | M ticks/s | Fights/s (1 thread) |
|---|---|---|---|---|
| Halvex Runner vs 3 raiders (`runner_vs_raiders`) | 58.4 s (1750 ticks) | **1189** (1152–1325) | 2.1 | 97 (93–98) |
| starter vs starter (`skiff_mirror`) | 38.2 s (1150 ticks) | **2277** (2168–2373) | 2.6 | 190 (171–190) |

One thread: about 10 ms per Runner-vs-3 fight (5.9 µs per tick) and 5.3 ms per skiff fight (4.6 µs per tick); 16
threads scale about 12×. Building a fight (`Fight::new`: the behavior library, the designs, the spawns) costs about
0.2 ms, 2–4 % of a fight, so the arena builds each fight from its spec without caching. coop-arena 5.7 estimated
1000–2000 fights per second; one evolution generation (64 × 16 opponents × 4 seeds = 4096 fights) takes 2–4 s.

What the 4000-seed runs say about the content (side 0 won / side 1 won / undecided): the Runner beats three raiders
in 56 % (2227 / 543 / 1230, mean score 0.73); almost a third end undecided (the time limit or both out). The skiff
mirror: 1607 / 1555 / 838 (mean score 0.507 / 0.493).

**With ADR-0037** (14 Oct 2026; simultaneous fire, shared think ticks; load average 5–8; the build before, run
alternately, in brackets): `runner_vs_raiders` 1398 fights/s on 16 threads, median of 5 (1374–1406; before 1433),
2.53 M ticks/s (2.51), one thread 114 fights/s (119): the fights last a little longer (60.4 s mean), the cost per
tick is the same. `skiff_mirror` now always runs to the time limit (90 s, 2700 ticks: both lose their lasers on the
same tick and drift), so it gives fewer fights per second for cheaper ticks: 1261 fights/s on 16 threads, median of 5
(973–1338), 3.4 M ticks/s; one thread 113 fights/s (233), 0.31 M ticks/s (0.28). The 4000-seed runs now: the Runner
against three raiders 2076 / 590 / 1334 (mean score 0.708 / 0.292, mean 60.4 s), the skiff mirror 0 / 0 / 4000
(0.500 / 0.500).
