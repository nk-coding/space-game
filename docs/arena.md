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

**Spawn order.** The side spawned first gets the lower ship ids, and the sim favours them: in a mirror match of two
`rim_skiff`s the first-spawned side won 1173 of 2000 fights and the other 378 (449 undecided); spawning side 1 first
turned it round (425 / 1141), so it is the order, not the geometry. The cause lies in the sim's id-ordered passes
(not investigated further: behaviors think on a cadence staggered by ship id, and the ships are processed in id
order within a tick); first strikes decide skiff fights, which magnifies it. The arena cancels it rather than
changing the sim: **odd seeds spawn side 1 first**, so seeds in pairs
balance out (the same mirror match: 1607 / 1555 / 838 of 4000). A single fight still carries the bias of its seed's
parity; average over an even number of seeds.

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
| halvex_runner | 21% (0.52) | 100% (1.00) | 100% (1.00) | 74% (0.82) | 100% (0.99) |
| automaton_raider | 0% (0.00) | 17% (0.48) | 2% (0.06) | 2% (0.15) | 22% (0.22) |
| rim_skiff | 0% (0.00) | 92% (0.94) | 36% (0.47) | 0% (0.24) | 84% (0.85) |
| sample_mid | 10% (0.18) | 71% (0.85) | 27% (0.76) | 0% (0.48) | 52% (0.64) |
| automaton_hunter | 0% (0.01) | 78% (0.78) | 14% (0.15) | 18% (0.36) | 9% (0.45) |
```

(`--seeds 100`, 2500 fights, 14 Oct 2026 content; 1v1 at 100 m, 90 s.)

## Determinism

The same spec gives the same result and hash on every platform: the arena adds only spawn positions (`+ − × ÷` on
the sim's PCG stream) and reads the sim. `scripts/determinism-check.sh --fight NAME [--seed N]` builds the
`fight_hashes` example (the specs of `crates/arena/fights/`, compiled in) natively and for `wasm32-wasip1`, runs both
until the fight ends and compares the per-tick hashes.

**Result (14 Oct 2026, release builds, `dev` with the arena):** native x86_64 vs wasm32-wasip1 under Node 24
**identical** for `runner_vs_raiders` seeds 1 (a loss, 2624 ticks), 2 (a loss, 1855), 11 (the time limit, 2700) and
`skiff_mirror` seeds 3 and 42 (wins, 551–552 ticks); also before the spawn-order change, `runner_vs_raiders` seeds
1, 5, 6 and `skiff_mirror` 1, 7. The final hashes equal the CLI's for the same seeds.

`tests/fights.rs`: the same spec twice gives the same result and hash, another seed another hash; stepping in slices
gives the same result as one go; the starter against one raider ends; a short time limit ends undecided at 0.5 each;
the score's edge cases; seeds averaged; the spec's RON round trip (blueprint and behavior texts included); every
error kind; the spawn order by seed parity.

## Performance

Native release, `arena fight --seeds 4000` on all 16 threads (AMD Ryzen 9 9950X3D, 16 vCPUs), 7 runs each, load
average 6–12 (other agents building, and the runs themselves), 14 Oct 2026:

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
