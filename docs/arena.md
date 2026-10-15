# Arena (`crates/core/arena`)

Headless, deterministic fights between designs on the real simulation: track **L1** of the evolution lab
(`docs/plans/evolution-lab.md`), written 14 Oct 2026; the voyage fitness (**L2**) and the evolution (**L3**) the same
day: [Voyage fitness](#voyage-fitness), [Evolution](#evolution). Decisions: [ADR-0036](adr/0036-arena-crate.md) (the crate),
[GDR-0083](gdr/0083-evolution-lab.md) (the lab), [GDR-0032](gdr/0032-balancing-arena-evolution.md) (balance by arena
and evolution); the fight spec and the score follow `docs/plans/coop-arena.md` 3.7 and 5.7. Every number marked
*placeholder* is for feel.

The crate depends on `data` and `sim` (plus `serde` and `ron`, already used by `data`); no Bevy, no I/O, no threads,
no wall clock in the library; it builds for `wasm32-unknown-unknown`. `crates/core/arena/clippy.toml` carries the sim's
bans on std's transcendental float functions. It lives in `crates/core/` since 15 Oct 2026 (it was `crates/tools/arena`,
[ADR-0040](adr/0040-arena-library-in-core.md)), so the game can use it; the native CLI is the binary `arena` of its own
package `arena-cli` (`crates/tools/arena-cli`, below).

## The fight spec

A `FightSpec` is plain data with a RON text form (`FightSpec::from_ron`, `to_ron`; read with the content's RON
options: `Some(..)` may be left out, unknown fields are errors, comments are fine). It is what a lab card will replay
and what a fight code will carry later.

```ron
// crates/core/arena/fights/runner_vs_raiders.ron
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
(`sim::voyage::scenario::load_test_with`): the content's `BehaviorLibrary` plus the spec's behavior texts, each entry's design
compiled once (`Arc<ShipDesign>`), ships spawned and commanded. `fight.step(ticks)` runs up to that many ticks and
returns `Some(FightResult)` once the fight has ended (later calls step nothing and return the same result), so a
client can run a slice per frame; `fight.sim()` is the world for drawing. `run_fight(&content, &spec)` does it in one
go. The sim's per-tick state hash runs only in debug builds (as in every runner); the result's `state_hash` is taken
once at the end.

A client that steps the world itself (the game's fight viewer, [GDR-0092](gdr/0092-evolution-lab-for-your-own-ships.md))
builds it with `FightSetup::build(&content, &spec)`: the `Sim`, its `Referee` and every entry's blueprint and
compiled design (per side, in entry order; the client draws ships whose design is no content ship from them). It
steps the sim with no inputs and calls `referee.observe(&sim)` after every tick, which tallies the tick's damage
and returns `Some(FightResult)` from the tick the fight ends on; that is all `Fight` does. Both are `Clone`, so a
viewer keeps keyframes and seeks (`tests/codes.rs`: a cloned pair goes on to the same end). `arena::standing(&sim,
side)` gives a side's build points and vessels now.

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

## Fight codes

A fight is re-watched by simulating its spec again ([GDR-0092](gdr/0092-evolution-lab-for-your-own-ships.md)): equal
specs give equal fights everywhere, so a fight is stored as what was fought and how it ended, never as frames.
`FightCode::new(&content, rules, spec, record)` holds:

| Field | What |
|---|---|
| `packs` | The content packs it was fought with (id, version), to explain a code whose ships this game lacks |
| `rules` | The rules' fingerprint then (`arena::rules_fingerprint`, below) |
| `spec` | The `FightSpec`: seed, both sides' entries (designs as content ids or blueprint texts, behaviors as content ids or program texts, counts, formations), start distance, time limit |
| `record` | The `FightResult`: outcome, ticks, both sides' numbers and the final state hash |

The rest of a fight's setup is fixed by the arena: owners 1 and 2, every core running, `SimConfig::default()`, the
packs' content only (no tuning scales, no player storage), nobody piloting.

`code.encode()` gives one line: `SGF1:` (space game fight, format 1) and the base64url text (no padding, so it goes
into a URL as it is) of the zlib-compressed RON, made like the drydock's share codes (ADR-0031). About 430
characters for the Runner against three raiders, 1–2 kB with an evolved blueprint and its behavior.
`FightCode::decode(text)` ignores whitespace (a wrapped line pastes whole) and refuses, with a `CodeError`: text that
is no code, a newer format, a damaged code (not base64url, not zlib, a wrong checksum, cut short), more than 256 kB
packed or 4 MB unpacked, and a payload that is no fight. A code that decodes may still not build (a ship the content
lacks): `run_fight` says why.

**The rules' fingerprint** (`rules_fingerprint(&content)`): FNV-1a 64 over `Content::fingerprint` (the RON of
everything a fight or voyage reads: stats, categories, factions, materials, parts, frames, ships, behavior nodes,
behaviors, voyages, encounters; not pack versions, research, controls, icons or wiki prose) and the final state hashes
of the two rules probes in `probes/` (fixed fights of 300 ticks at close range: guns, hits, plumes, shields, chain
reactions), so a change to the sim's code shows without a version number someone must remember to raise. 4.7 ms
natively (median of 20, p95 5.4 ms, release, load 0.9; the content's part 0.35 ms). A code whose `rules` differ from today's was fought under other rules: the viewer says so at once, plays it
under today's rules and says at the end whether it still ends as recorded. A probe the content can't build counts
as 0.

`tests/codes.rs`: a code with a blueprint and a behavior text round-trips (also wrapped and with a lower-case tag), is
URL-safe, and replays as recorded; every refusal; the fingerprint is stable, includes the probes and changes with a
part's mass, which also changes the fight's end; the setup stepped with the referee gives `run_fight`'s result, and a
cloned world and referee go on to the same end.

## The CLI

The binary `arena` of the package `arena-cli` (`crates/tools/arena-cli/src/main.rs`), not an example: it is the arena's
tool for balance numbers and `arena evolve` ([Evolution](#the-cli-arena-evolve)) (like `content-tool`), not a one-off
measurement; the library's examples hold checks (`fight_hashes`). It keeps only arguments, its thread pool, tables and
files; everything else is the library's.

```sh
cargo run --release -p arena-cli -- fight crates/core/arena/fights/runner_vs_raiders.ron --seeds 200
cargo run --release -p arena-cli -- matrix halvex_runner automaton_raider rim_skiff my_design.ron --seeds 50
```

- `arena fight <spec.ron> [--seeds N] [--threads N]`: runs the spec with its seed and the N − 1 after it, prints a
  Markdown row per seed (outcome, time, build points lost of the start, damage dealt, vessels left, score, hash), a
  summary (wins per side, undecided, mean score, mean time) and then each seed's fight code (`- seed 3: SGF1:…`):
  pasted into the game as `?scene=fight&fight=<code>`, the fight replays in the flight view (`docs/client.md`
  "Fight viewer").
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
`fight_hashes` example (the specs of `crates/core/arena/fights/`, compiled in) natively and for `wasm32-wasip1`, runs both
until the fight ends and compares the per-tick hashes. It also runs the rules probes (`--fight close_quarters --seed 1`,
`--fight warden --seed 2`) and fight codes (`--fight SGF1:…`: the code's own spec and seed to its end, then
`# record=as recorded` or `DIFFERS`); results in `docs/sim.md` "Fight codes and the rules probes".

**Result (14 Oct 2026, release builds, `dev` with the arena):** native x86_64 vs wasm32-wasip1 under Node 24
**identical** for `runner_vs_raiders` seeds 1 (a loss, 2624 ticks), 2 (a loss, 1855), 11 (the time limit, 2700) and
`skiff_mirror` seeds 3 and 42 (wins, 551–552 ticks); also before the spawn-order change, `runner_vs_raiders` seeds
1, 5, 6 and `skiff_mirror` 1, 7. The final hashes equal the CLI's for the same seeds.

**With ADR-0037** (14 Oct 2026, release builds; simultaneous fire, shared think ticks, auto orbits by both ids, side 0
spawned first): **identical** for `runner_vs_raiders` seeds 1 (the time limit, 2700 ticks, `21a0ee0c17aa5233`), 2 (a
loss, 2661, `3d1b0698c87bd461`), 11 (a loss, 1738, `293ae01a0f0b6d2a`) and `skiff_mirror` seeds 3 (the time limit,
2700, `988f8d8fc87fa72e`) and 42 (the time limit, 2700, `57cb79ccd322c539`), the same final hashes as the CLI's.

**With chain reactions** (GDR-0084, 14 Oct 2026, release builds): **identical** for `runner_vs_raiders` seeds 1 (the
time limit, 2700 ticks, `bd3e258ebfe888fd`), 2 (a loss, 1554, `8b08eb23b1973618`), 11 (the time limit, 2700,
`5dc38c6c72ed573a`) and `skiff_mirror` seeds 3 (the time limit, 2700, `b8c97bc4f43ddc20`) and 42 (the time limit,
2700, `573b7d11e6fd6c6f`), the same final hashes as the CLI's.

**With chain reactions v2** ([GDR-0085](gdr/0085-chain-reactions-v2.md), 14 Oct 2026, release builds): **identical** for
`runner_vs_raiders` seeds 1 (the time limit, 2700 ticks, `9ec00a3352c3346f`), 2 (a loss, 1335, `b543f035fd7774db`) and
11 (a loss, 1643, `41f5ffe35d6bf208`) and `skiff_mirror` seeds 3 and 42 (the time limit, 2700, `b8c97bc4f43ddc20` and
`573b7d11e6fd6c6f`: unchanged, no part with an explosion is destroyed in them), the same final hashes as the CLI's.

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

## Voyage fitness

Track L2 (`src/voyage.rs`): a design and a behavior flown through a content voyage by the headless voyage runner,
scored 0..1. The runner's pilot moved out of the `voyage` example into the sim as a library, `sim::voyage::runner`
(`docs/sim.md` "The headless voyage runner"; the example's output is unchanged), so the example, the arena and later
the game share one pilot.

- `VoyageSpec { voyage, seed, design, behavior, minutes }`: a content voyage id, the seed (the voyage's roll and the
  world), a `DesignRef` (content ship or blueprint text, checked like a fight's), a `BehaviorRef` the ship flies in
  fights (`None`: the runner's scripted *closing* pilot) and the time limit (`DEFAULT_VOYAGE_MINUTES` = 10,
  placeholder; the scripted runs take 2.5–3.5 minutes).
- `start_voyage(&content, &spec)` builds a steppable `sim::VoyageRunner`; `run_voyage` runs it to the end and gives
  the `VoyageRun` (outcome, goal, ticks, hull lost, sites cleared of those with ships, kills, haul).
- `voyage_score(&run, limit_ticks)`, *placeholder* weights:
  - the goal cleared and home: **0.6 + 0.2 × (1 − share of the hull lost) + 0.2 × (1 − ticks / time limit)**;
  - otherwise: **0.5 × (sites cleared / sites with ships)**, halved when the ship was lost.

  So any cleared voyage beats any uncleared one, a faster and less damaged clear beats a slower one, and clearing
  more sites counts before the goal falls.
- `voyage_fitness(&content, &spec, voyages, seeds)`: the mean score over every voyage and seed, in order.

Errors are `ArenaError` values (`UnknownVoyage`, `Voyage` for a design, behavior or roll that does not work, `Number`
for the minutes). The starter on `yard_approach` with the skirmisher behavior in fights scores 0.156 over seeds 1–4
(8-minute limit); 30 generations of evolution reach 0.897 (below).

## Evolution

Track L3 (`src/evolve/`): designs and behaviors bred through the editing models, scored by fights against a pool or by
the voyages, kept in a MAP-Elites archive (GDR-0032, GDR-0083; the method of `docs/plans/coop-arena.md` 3.8). One
`arena::Run` per run; single-threaded and steppable in the library, every core in the CLI, the same results either way.
Since GDR-0092 steps 2 and 3 (15 Oct 2026) a run starts from one to four designs, keeps what is locked, produces only
designs within the mind capacity and with the start's hold, and keeps every fight: [Starts and locks](#starts-and-locks),
[What a run keeps](#what-a-run-keeps).

### A run is a text file

`RunConfig` has a RON form (`RunConfig::from_ron`, `to_ron`; the content's RON options: `Some(..)` may be left out,
unknown fields are errors, comments are fine). Examples in `crates/core/arena/runs/`:

```ron
// crates/core/arena/runs/skiff_design_60bp.ron
(
    seed: 1,                          // the run's own random stream
    evolve: Design,                   // Behavior | Design | Both | Stages([...]), below
    generations: 30,                  // left out with Stages
    population: 32,                   // children per generation (default 32)
    fitness: ArenaPool(
        opponents: [
            (design: Ship("automaton_raider"), count: 2),   // fight spec entries (docs above)
            (design: Ship("automaton_raider"), count: 3),
            (design: Ship("automaton_hunter")),
        ],
        seeds: 4,                     // seeds 1..=4 against each opponent (default 4)
        time_limit: 60,               // optional: start_distance, time_limit (fight defaults), count (1),
                                      // hall_of_fame (0)
    ),
    limits: (frame: "frigate", build_points: 60, mirror: true),  // all optional, below
    start: Design(design: Ship("rim_skiff"), behavior: Content("skirmisher")),  // or starts: [...], below
    // archive: (axes: [...]),        // default: acceleration × firepower, 8 × 8
    // crossover: 0.2, mutations: 3,  // the defaults
)
```

| Field | What |
|---|---|
| `evolve` | `Behavior` (the design stays), `Design` (the behavior stays), `Both` (each mutation picks one), or `Stages([(evolve: Behavior, generations: 10), (evolve: Design, generations: 10), (evolve: Both, generations: 10)])`: one after the other, one archive throughout |
| `fitness` | `ArenaPool(opponents, seeds, hall_of_fame, count, start_distance, time_limit)`: the genome (`count` ships of it, side 0) fights each opponent entry and the best `hall_of_fame` elites so far over seeds 1..=`seeds`; the mean arena score. `Voyages(ids, seeds, minutes, pilot)`: each voyage over seeds 1..=`seeds`, the mean voyage score; `pilot: Behavior` (default: the genome's behavior flies the fights) or `Closing` (the runner's scripted pilot) |
| `limits` | Each optional. `frame`: a content frame id (24, 40 or 64 cells; default the smallest every start fits). `build_points`: the most a design may cost: `Start` (the default since GDR-0092: its own start's build points, "same cost, better ship"), a number (`build_points: 60`) or `Off`. `min_hold`: the smallest cargo hold (`Content::ship_hold`): `Start` (the default: its start's, so every result still brings salvage home), a number or `Off`. `max_cores`: the most logic cores (VOYAGE-READY: the career's mind capacity less the escorts' cores; default any). `counts`: how many of a part a design may hold (`{"pulse_laser": 2}`, `Career::available`; the drydock refuses more with `NoneLeft`). `parts`, `materials`: only these (e.g. a career's `Unlocks`; default all, locked ones too). `mirror`: the mirror lock (every edit mirrored, designs stay symmetric; the starts must be symmetric) |
| `start`, `starts` | One start (`start`, the old form) or a list (`starts: [...]`), one to four in all. `Design(design, behavior, locks)`: a content ship or blueprint text, a behavior (default: the design's own, else its faction's) and what the run keeps of it ([Starts and locks](#starts-and-locks)). `Random(faction: "ship_mind")`: a random design that passes the pre-filter, under the faction's behavior |
| `archive` | `(axes: [(measure, min, max, cells), ...])`, one to three axes (at most 65 536 cells) |
| `population`, `generations`, `seed`, `crossover`, `mutations` | Children per generation; generations; the run's stream; the share of children bred by crossover (0.2); a child gets 1 to `mutations` (3) mutations. All *placeholders* |

Every error is a `RunError` value: `Config` (does not read, a number out of range, `generations` missing or given with
`Stages`, an unknown frame, `EngageDistance` with the voyage fitness, no start or more than four, fewer children than
starts), `Start` (an unknown ship or behavior, a start breaks a limit, its locks or the pre-filter, a lock names a
part, rule or category it lacks, a KEEP EMPTY cell it fills), `Fitness` (an opponent or voyage that does not resolve:
`Run::new` builds one fight or voyage of each with every start), `Save` (a run file that is damaged, of a newer
format, or doesn't match its configuration).

### Genomes, mutation and crossover

A `Genome` is a blueprint (trimmed, as the drydock exports it; named "<start> (evolved)", tagged `evolved`, no
behavior field), a behavior text (canonical, without the start's notes), its `root` (the start it descends from) and
`at`, where the blueprint's top-left cell sits in that start's frame. `genome.blueprint_text()` is the content RON form
and `genome.behavior_text()` the text form, so the game can SAVE AS DESIGN; `genome.placed(run.frame(root))` is the
design as the drydock holds and saves it (`size` = the frame, parts in frame coordinates).

**Genomes keep frame coordinates** (GDR-0092 step 3): each start is opened centred in its frame once
(`Drydock::open`), and from then on every genome is opened where its parts are (`Drydock::open_placed`, `docs/drydock.md`),
never centred again, so a design that grows on one side keeps its cells and the locks (frame cells) never drift.
Designs change only through `data::design::edit::Drydock` (in the start's frame: the run's, one column narrower under
the mirror lock when the start's width has the other parity), so every child is a legal placement; under the mirror
lock an edit whose mirror image is impossible (`Twin::Skipped`) is undone, and so is an edit that breaks a lock (a
locked part's mirror twin, a part over a KEEP EMPTY cell). One mutation is one of (weights are *placeholders*):

| Design operator | Weight | How |
|---|---|---|
| add a part | 30 | an allowed part on a free cell next to a random part: slopes oriented to the hull (`place_slope`), others in a random pose; only where the preview replaces nothing (since GDR-0081 `place` replaces what is under it) |
| remove a part | 15 | `remove_part` (and its twin); like every operator on a part, only an unlocked one |
| move a part | 15 | by −2..2 cells each way (`move_part`, refused into other parts) |
| rotate / flip | 10 / 5 | `rotate`, `flip` |
| swap a part | 15 | another allowed part of the same footprint and shape in its place and pose: `place`, accepted only when it replaces just that part (and its twin) |
| material | 10 | another allowed armour material on a hull part |

Behaviors change only through `data::behavior::edit::BehaviorEditor`, so every program is legal; rules are added only
below the design's rule slots, and no operator picks a locked rule (its nodes, its conditions) or a locked target line:

| Behavior operator | Weight | How |
|---|---|---|
| nudge a number | 30 | a comparison's threshold (one time in ten its comparator too) or a numeric parameter, × 0.6–1.4, rounded (`set_compare`, `set_param`, refused out of range) |
| change a choice | 5 | another option (an orbit's direction, a target's aim point) |
| move a rule | 10 | `move_rule` to a random place |
| swap an action / the target | 10 / 5 | `replace_node` with a random action or selector |
| add / remove a rule | 10 / 8 | `add_rule` (with a random condition four times in five), `delete_rule` |
| add / remove / negate / replace a condition | 8 / 6 / 4 / 4 | `add_condition`, `remove_condition`, `toggle_not`, `replace_node` |

**Crossover** (a `crossover` share of the children, before their mutations): designs splice `a`'s parts above a
random row with `b`'s from that row down (both in one frame: two starts of different frames don't cross; `b`'s parts
that overlap or cover one of `a`'s KEEP EMPTY cells are left out, `a`'s locked parts below the row stay), behaviors
`a`'s first rules with `b`'s from a random rule on (the target line of either, `a`'s when it is locked; `a`'s locked
rules past the splice stay, before `b`'s, which bring no second copy). The child is `a`'s: its start, its locks.
What evolves decides which halves cross.

**The rule slots are repaired** (GDR-0092): after a child's mutations, a behavior with more rules than its own design's
rule slots (a design mutation took a logic core) loses its lowest unlocked rules until it fits; if its locked rules
alone don't fit, the try is refused.

**The pre-filter** (`Kit::assess`, part of the breeding time below) runs before any fight: the limits (parts,
materials, part counts, the start's frame, symmetry under the mirror lock, build points from `sim::design_report`, the
hold, logic cores), the start's locks (every locked placement exactly where it was, every KEEP EMPTY cell empty, every
locked rule there, in order under LOCK ORDER, a locked target line unchanged), the behavior in its canonical text,
`data::validate_ship_flying` without errors (the content's checks with the genome's **own** behavior: its rules
against its own design's rule slots, not the faction's default behavior as before GDR-0092), and the design report: it
must fly (no blocker: a powered logic core and thrust), be one piece (no `FallsOff`) and carry a gun. A child that is
refused, or equals its parent, is bred again (16 tries, then the generation is one smaller); the CLI's `refused`
column counts the refusals. Fights check a blueprint the same way: an entry with a behavior of its own is validated
with that behavior (`validate_ship_flying`), one without with its own or its faction's.

### Starts and locks

`starts: [...]` (with the old `start`, one to four, `MAX_STARTS`, lead default): generation 0 is every start, then
mutants of each in turn (start 1, start 2, ..., start 1, ...); later generations breed from the whole archive, and a
child descends from its first parent's start (its root). The run's frame is the configured one, else the smallest
content frame every start fits; each start opens centred in it once.

What a run keeps of a start is `locks` on its `Design` (GDR-0092 "LOCKS"; `arena::evolve::Locks`):

```ron
starts: [
    Design(design: Ship("rim_skiff"), behavior: Content("skirmisher"),
        locks: (
            parts: [0],                   // the start's parts[0]: never moves, turns, changes material or power, or goes
            categories: ["weapons"],      // every part of the category (categories.ron): KEEP ALL WEAPONS
            cells: [(3, -1), (4, -1)],    // KEEP EMPTY: cells in the start's own coordinates (a clear line of fire)
            rules: [0],                   // the behavior's rule 0 (the top): never changed, moved or deleted
            target: true,                 // the target line stays
            order: true,                  // LOCK ORDER: the locked rules keep their order
        )),
    Design(design: Ship("halvex_runner")),
],
```

Under the mirror lock a locked part or cell locks its mirror image too. Rules may still be added around a locked rule,
and the rules above it may change. The operators never pick what is locked, and the pre-filter refuses a child that
lost any of it, so crossover can't break a lock either. `crates/core/arena/runs/two_starts_locked.ron` is a small
example (the skiff's guns, first part, top rule and the cells in front of its guns, with the Runner beside it).

### The archive and the loop

The archive is a grid over one to three **measures**: `TopSpeed`, `Acceleration`, `WeaponReach`, `BuildPoints`,
`Mass`, `Firepower`, `HitPoints`, `Parts` (from the design report), `Rules` (the rules that run, at most the slots),
`EngageDistance` (the mean distance from the genome's first running ship to the nearest enemy vessel in its fights,
sampled every half second; arena pool only). An axis has `cells` equal bins from `min` to `max` (values outside go to
the end bins); each cell keeps its best elite (genome, score, measures, generation); a child takes a cell when it is
empty or the child scores higher (ties keep the elite).

`Firepower` is **latent firepower** (Niklas, 14 Oct 2026; `sim::DesignReport::latent_firepower`): the damage per
second of every gun of the main piece as if each were clear and powered, at full power with bonuses, so blocked and
idle guns count. Designs that hide their guns behind their own parts then sit with the other gun-heavy designs
instead of under "no firepower". The drydock keeps the report's `firepower` (the guns that work as built).

The default axes are **acceleration (0–24 m/s²) × (latent) firepower (0–16 damage per second), 8 × 8** (*placeholders*), not
coop-arena 3.8's top speed × weapon reach: those are ratings of the part types (every base ship flies 20–22 m/s and
reaches 45–75 m), so with the starter's parts every design sat in one or two cells (the first design run filled 2
cells in 3 generations). Behavior runs want behavior measures (`EngageDistance` × `Rules`, as
`skiff_behavior.ron`); in a behavior stage under design axes every child shares its parent's cell (a plain hill
climb).

Each generation: generation 0 is the starts and `population − n` mutants of them (shared out evenly); later ones are
`population` children of elites drawn uniformly from the occupied cells. Each child's jobs (every opponent × seed, or every voyage × seed)
are evaluated; its score is their mean; then the children are offered to the archive in order. All random draws
come from the run's PCG stream (`seed`, its own sequence) in a fixed order; fights and voyages use seeds 1..=n, the
same for every child and generation, so scores stay comparable. The hall of fame (`hall_of_fame` best elites joining
the pool) makes the fitness move with the archive; elites are not re-scored.

### The API

```rust
let mut run = arena::Run::new(&content, config)?;   // checks the config, resolves the starts, checks the fitness
let p = run.step(20_000);                           // up to 20 000 ticks of breeding and fights this frame -> Progress
let s = run.state();                                // generation, generations, stage, history, records, archive
for (cell, elite) in s.archive.cells() { /* elite.genome, elite.id, elite.score, elite.measures, elite.generation */ }
let r = run.results();                              // history + every elite, best first
let (blueprint, behavior) = (r.elites[0].genome.blueprint_text(), r.elites[0].genome.behavior_text());
let save = run.snapshot().to_ron();                 // the run file (after every generation)
let run = arena::Run::restore(&content, &arena::RunSave::from_ron(&save)?)?;  // goes on exactly; run.stale()
let code = run.fight_code(generation, child, job);  // any fight of any child, to re-watch (`?scene=fight&fight=`)
```

What the lab builds on (the game's EVOLUTION lab since GDR-0092 step 4, 16 Oct 2026: `world::evolution`, `docs/client.md`
"Evolution"; it keeps each run as `lab/<id>`, this run file with its head's name, creation time and status):

| Call | What |
|---|---|
| `Run::step(budget)` | Breeds the next generation child by child when none is open (each child counts `BREED_COST` = 60 ticks, *placeholder*: about 0.3 ms natively, below), then runs its jobs for the rest of `budget` sim ticks (a fight or voyage is stepped in slices, `Fight::step` / `VoyageRunner::step`), and closes the generation after its last job. The client converts its 4–10 ms frame budget to ticks from the measured cost per tick (GDR-0092). Every slicing gives the same run. A stale run doesn't step (`Progress::stale`) |
| `Run::jobs()` / `Run::submit(results)` | A generation's `Job`s (a `FightSpec` or `VoyageSpec` each, plain data; breeding the rest of it first) and their `JobResult`s in the same order; `Job::run(&content)` runs one anywhere (the native executor's threads). Don't mix them with `step` within a generation |
| `Run::breed(budget)` | Breeds the open generation within `budget` ticks (`BREED_COST` a child) and fights nothing: `true` once every child is bred, and `jobs()` then hands them out without more breeding (the game's native executor breeds on its frame budget with it, GDR-0092 step 4) |
| `Run::worked()`, `Run::generation_done()` | The ticks of work `step` and `breed` have done since the run was built or restored (the client measures its cost per tick against it); how far the open generation is, 0..1 (its jobs done of those it will have) |
| `Run::state()` | Generations done and in all, the stage, the history (`GenerationStats`: stage and genes, children evaluated, refused, best and mean score, the archive's best, occupied cells, children that took a cell, ticks), the records (`GenerationRecord` per generation: the hall of fame its children fought, each child's genome number, score, measures, whether it took a cell and its job records) and the archive |
| `Run::best_child(generation)` | Its best child (the first of equal scores) |
| `Run::child_jobs(generation, child)`, `Run::job(generation, child, job)` | A child's jobs rebuilt (the same jobs it ran), one with its `JobRecord` |
| `Run::fight_code(generation, child, job)` | The job as a fight code under the rules the run was fought under: it replays to the recorded end while they hold (`None` for a voyage job) |
| `Run::genome(id)`, `Run::table()`, `Run::lineage(id)`, `Run::scored(id)` | Genome `id` rebuilt from the genome table; how every genome was bred (`Bred`); a genome's line from its start (genome numbers); where it was scored (generation, child) |
| `Run::starts()`, `Run::frame(root)` | The starts as opened in their frames; the frame genomes of a start are edited in (`Genome::placed`) |
| `Run::snapshot()`, `Run::restore(&content, &save)`, `RunSave::to_ron` / `from_ron` | The run file and going on from it ([What a run keeps](#what-a-run-keeps)) |
| `Run::rules()`, `Run::stale()` | The rules' fingerprint the run started under; whether today's differ |

`Job`, `JobResult`, `JobRecord`, `VoyageSpec`, `FightSpec`, `FightResult`, `GenerationStats`, `GenerationRecord`,
`Bred` and `RunSave` are serde types with a RON form.

### What a run keeps

Every fight of a run can be re-watched (GDR-0092 step 2): a run keeps how every genome was bred and how every job
ended, and rebuilds any job's spec from them.

- **The genome table** (`Run::table()`, `arena::evolve::Bred`): every genome bred, numbered in breeding order (the
  starts are 0 to n − 1), as a change against its parent: the generation, its start (`root`), its parent and a
  crossover partner, the parent's parts it lost (indices) and the parts it gained (index and placement, in frame
  coordinates; the longest common run of the two part lists is kept, so a mutation stores a part or two), its behavior
  text when it changed, and its size. A start stores all its parts and its behavior. `Run::genome(id)` replays the
  changes from the start: the identical genome, header included (`tests/runs.rs` checks every genome against the
  blueprint and behavior its fights carried).
- **The records** (`Run::state().records`): per generation the hall of fame its children fought (genome numbers),
  per child its genome number, score, measures and whether it took a cell, and per job a `JobRecord`: the score, the
  ticks and how it ended (`JobEnd::Fight`: outcome, build points lost, damage dealt and vessels left per side, the
  final state hash, and a bit for every rule of the child's behavior that decided at least once,
  `Referee::decided`, for GDR-0092's TIDY; `Voyage`: the final state hash; `Failed`). Each side's build points at the
  start and the scores follow from the spec and the rest (`JobRecord::fight_result`). The config, the records and the
  table rebuild every job (`Run::job`), so `Run::fight_code(generation, child, job)` is any fight as a code; a test
  replays every fight of a run to its recorded result.
- **The run file** (`Run::snapshot()` → `RunSave`, `to_ron`): the state between two generations. A readable RON head
  (`format: 1`; the game's optional `name`, `created` (Unix seconds) and `status` (`Running`, `Paused`: whether it
  computes when it may), left out by a snapshot and by the CLI, so files without them read and write as before, GDR-0092
  step 4; the rules' fingerprint when the run started, the `RunConfig`, the `GenerationStats` per generation, the
  best genome) and `payload: "SGL1:…"`, the base64url text (no padding) of the zlib-compressed RON of the rest: the
  run's random stream (`Pcg32::raw`), the genome table, the records and the archive's elites (genome numbers; their
  scores and measures come from the records). A snapshot taken during a generation is the state before it: a restored
  run breeds that generation again from the same random state, so at most one generation's work is lost. Reading
  refuses a newer format, a damaged payload and one past 16 MB packed or 128 MB unpacked (a run file is untrusted text,
  IMPORT RUN).
- **Resume** (`Run::restore(&content, &save)`): builds the run from its configuration again (which must still build
  on the content), takes the stored starts, stream, table, records and history, and rebuilds the archive's elites from
  the table. It goes on exactly as the run would have: the same run files byte for byte (`tests/runs.rs`: six
  generations straight against three, a run file written and read, a restore and three more, and one interrupted in
  the middle of a generation; `scripts/determinism-check.sh --run`, natively and in wasm, below).
- **Stale runs**: the run file keeps the rules' fingerprint the run started under (`rules_fingerprint`, built in step
  1). `Run::restore` on content whose fingerprint differs marks the run stale (`Run::stale()`): it keeps its fingerprint
  (its fight codes say which rules they were fought under), it is read-only (`step` does nothing, `jobs` is empty),
  and the CLI refuses to resume, replay or trace one. RE-SCORE comes with step 5.

**Size** (`skiff_design_60bp.ron`, 32 children × 12 fights a generation, 20 generations, 15 Oct 2026): a job record is
133 bytes of RON and **22 bytes packed**; a genome 382 bytes of RON and 24 packed; a generation **about 10 kB packed**
(13.6 kB of run file with the base64 and its row in the head). The file after 20 generations: 272 kB (1.3 MB of RON
unpacked), so a STANDARD run of 50 generations is about 0.7 MB.

**Determinism**: a job's result depends only on the job and the content (it is a fight or voyage, deterministic on
every platform), results are applied in job order, the archive is a `BTreeMap`, and breeding draws from the run's
stream in a fixed order whatever the slicing. `tests/evolve.rs` checks that the same config and seed give the same
results run twice, stepped in slices of 97 ticks, and on 1 and 4 threads; another seed gives another run.
`tests/runs.rs` checks that slices of 1, 59, 97 and 5000 ticks, one go and the jobs handed out in reverse give the same
run file byte for byte, and that a resumed run is the uninterrupted one. The CLI on 1 and on 16 threads printed
identical tables and wrote identical files (`skiff_behavior.ron`, 14 Oct 2026, before and after ADR-0037).

**Across platforms** (GDR-0092 step 2): `scripts/determinism-check.sh --run NAME [--seed N] [--resume-at K]` builds the
`run_hashes` example natively and for `wasm32-wasip1`: the run file `crates/core/arena/runs/NAME.ron` (compiled in;
`two_starts_locked`) with its seed, run straight in slices of 700 ticks, one line per generation (FNV-1a 64 of the run
file after it); the example also runs it to generation K (default 3) in slices of 50 000, writes and reads the run
file, restores it and runs on, and fails unless every later generation hashes the same. Result (15 Oct 2026, release
builds): `two_starts_locked` seed 1 resumed after generations 0, 2 and 4, and seed 7 resumed after generation 2: native
x86_64 vs wasm32-wasip1 **identical** (6 generations, 48 genomes; last line `4f126e780aff05c0` for seed 1,
`887374c2560c32e7` for seed 7), every resume identical to the straight run.

### The CLI: `arena evolve`

```sh
cargo run --release -p arena-cli -- evolve crates/core/arena/runs/skiff_design_60bp.ron --out /tmp/skiff_design [--best 8] [--threads N]
cargo run --release -p arena-cli -- evolve crates/core/arena/runs/skiff_design_60bp.ron --save /tmp/run.ron --until 10
cargo run --release -p arena-cli -- evolve --resume /tmp/run.ron        # goes on, and keeps saving to it
cargo run --release -p arena-cli -- evolve crates/core/arena/runs/skiff_design_60bp.ron --from my_ship.ron --from rim_skiff --lock category:weapons --lock rule:0
cargo run --release -p arena-cli -- replay /tmp/run.ron 12 3 5         # generation 12, child 3, job 5: its fight code
cargo run --release -p arena-cli -- lineage /tmp/run.ron [GENOME]       # a genome's line from its start (default: the best)
```

Breeds on the main thread and runs each generation's jobs on every core (`--threads`, default all), prints a
Markdown row per generation (evolves, children, refused, best, mean, archive best, cells, took a cell, million
ticks, breeding ms, seconds), the generations per minute, the best elites (rank, genome number, cell, score, measures,
generation, parts) and the lineage of the best three (`0 → 34 → 120 → 812`). With `--out DIR` it writes the best
`--best` (8) elites: `elite_NN.ron` (the blueprint, id `evolved_NN`, with a comment naming the run, rank, score,
generation and genome) and `elite_NN.txt` (its behavior). Last, a fight code per generation: its best child in the fight
it scored lowest in, its hardest (`- gen 12, score 0.833, seed 2 (arena replay 12 3 5): SGF1:…`; runs scored by
voyages print none). Every code comes from the run's records; no fight runs again.

| Option | What |
|---|---|
| `--save FILE` | Writes the run file after every generation (whole or not at all: a `.part` file renamed) |
| `--resume FILE` | Goes on from a run file instead of a run's configuration (its rows so far are printed with `–` for the times), and saves to it again unless `--save` names another; refused for a stale run |
| `--until N` | Stops once N generations are done (with `--save`, a run to resume later) |
| `--from DESIGN` | (repeatable, at most four) the run's starts instead of its file's: a content ship id or a blueprint file `*.ron`, whose behavior is the `.txt` of the same name beside it if there is one (as `--out` writes them), else its own or its faction's |
| `--lock LOCK` | (repeatable) a lock of every start: `part:N` (its `parts[N]`), `category:ID` (`category:weapons`), `cell:X,Y` (KEEP EMPTY, in the start's coordinates), `rule:N` (its behavior's rule N, 0 = the top), `target`, `order` |

`arena replay <run file> <generation> <child> <job>` prints that fight's code with the seed, how it ended, the score and
the final hash; `arena lineage <run file> [<genome>]` a table of the genome's line from its start (generation, start,
parent, crossover partner, parts lost and gained, whether the behavior changed, score). A resumed run prints the same
rows, elites and codes as one run straight through (checked on `skiff_design_60bp.ron`, 10 + 10 generations, and a
two-start locked run).

### What the example runs found

`crates/core/arena/runs/`, 30 generations of 32 each, 14 Oct 2026 `dev` with ADR-0037 (simultaneous fire, shared think
ticks, no spawn-order stopgap); the archive's best after generations 0, 10, 20 and 29. (Runs from GDR-0092 steps 2 and
3 on, 15 Oct 2026, breed differently: genomes keep their frame cells instead of being centred at every mutation, the
rule slots count the genome's own behavior, and the build points and the hold are capped at the start's by default;
the sim changes since 14 Oct also moved the start's own score, 0.274 against these opponents on 15 Oct.)

| Run | Start's score | Archive best, gen 0 / 10 / 20 / 29 | Cells | Best elite |
|---|---|---|---|---|
| `skiff_behavior.ron`: the skiff's behavior against 2 and 3 raiders and a hunter (archive `EngageDistance` × `Rules`) | 0.643 (skirmisher) | 0.726 / 0.896 / 0.980 / **0.980** | 38 of 48 | `target threat(weapons)`, `when power < 28% -> approach(81%)`, `always -> face` (a third rule below never runs): nose on the most dangerous enemy's guns |
| `skiff_design_60bp.ron`: the skiff's design, 60 build points, mirror lock, frigate, all parts | 0.643 | 0.758 / 0.758 / 0.989 / **1.000** | 24 of 64 | 20 parts in the starter's 8 × 4: four **automaton cannons** and two **automaton cores** (in place of the logic cores and gyros), the reactors moved to the nose in front of the centre cannons, the pulse lasers turned to the sides, no capacitors, cargo or armour blocks; the report counts none of its guns as clear (firepower 0) |
| `skiff_voyage_stages.ron`: behavior 10, design 10, both 10, starter parts, mirror lock, `yard_approach` × 4 seeds | 0.156 | 0.479 / 0.884 / 0.884 / **0.897** | 25 of 64 | 28 parts: the starter with its cargo bays traded for two more shield emitters and two more pulse lasers; behavior `target nearest(weapons)`, flee below 25 % hull, else `face` (three rules below never run) |

Findings for the lead: the automaton parts win every 60-build-point design run that may use them (an earlier run on
the sim before ADR-0037 kept four automaton cores and no reactor or thruster at all): the automaton core is a power
plant (6), a logic core (2 slots) and a gyro (300) in one 5-build-point cell, where the starter's reactor, logic core
and gyro cost 11; locked for players, but a balance question once research unlocks them. Facing the enemy (`face`,
aiming at its guns) beats the skirmisher's orbit against raiders, in fights and on the voyage.

**The best design's blocked guns** (checked 14 Oct 2026): they stay silent; the enemies shoot them clear. Its four
cannons sit behind its own parts (the two reactors and two slopes) and its control load is 12 on a capacity of 8, so
the two outer cannons and both lasers idle: as built nothing can fire, and the report's firepower 0 is right
(GDR-0004: guns need a clear line of fire). A blocker that is destroyed clears the gun from the next tick (GDR-0070:
"the blocker can be shot off"), and lost parts lower the control load, so idle guns wake. Counting every `ShotFired`
of the best 8 elites over 20 seeds against each pool opponent (480 fights), no shot came from a gun whose blocker was
still on its piece or that idled. The best design's first shot comes at tick 324 (median of its 60 fights,
about 11 s), when a raider's 1-damage shot has taken off a 1-hit-point reactor; with the cannons in front it fires
at tick 76.
The shutters win: 52 of 60 fights (19 / 15 / 18 of 20 against 2 raiders / 3 raiders / the hunter), against 33 of 60
with the two centre cannons moved in front of the reactors and 35 of 60 with all four cannons in front (every gun
clear; firepower 2.4, as the two outer cannons still idle). An automaton cannon has 1 hit point and the raiders' shots
take a gun off in one hit (GDR-0074), so a part in front buys time, and a reactor there costs nothing extra because
reactors did not explode yet (GDR-0019 decided chain reactions, GDR-0020 deferred them; built since, see below). All 8 elites have blocked guns
(3 of them firepower 0). The archive's firepower axis was then the design report's, as built, so it filed these designs
under little or no firepower. `sim/tests/weapons.rs` (`a_gun_behind_its_own_parts_fires_only_once_they_are_shot_off`)
and `sim/tests/report.rs` (`report_counts_blocked_and_idle_guns_as_the_sim_fires_them`, this design) hold the rule.

**With chain reactions and latent firepower** (14 Oct 2026, [GDR-0084](gdr/0084-chain-reactions-v1.md): destroyed
reactors, capacitors and drives explode into their piece; the archive's firepower axis counts every gun as if clear and
powered). The same three runs, same seeds, on 10 threads; the `dev` build before reproduces the table above exactly:

| Run | Archive best, gen 0 / 10 / 20 / 29 | Cells | Best elite |
|---|---|---|---|
| `skiff_behavior.ron` | 0.726 / 0.917 / 0.980 / **0.980** | 37 of 48 | the same play as before: `target threat(weapons)`, `when shield_down -> orbit(80%, ccw)`, else `face`, at about 75 m (the starter's guns are all clear, so neither change touches it) |
| `skiff_design_60bp.ron` | 0.732 / 0.895 / 1.000 / **1.000** | 32 of 64 | 20 parts, latent firepower 9.1 (2.4 as built): two automaton cannons clear in the nose with two pulse lasers (and two capacitors) right behind them, two more lasers on the outer edges facing forward, two cannons low on the sides facing each other, the two fission reactors and two automaton cores on the shoulders beside the outer lasers, shields and RCS pods inboard; control load 14 on 8, so all but the two nose cannons idle at the start |
| `skiff_voyage_stages.ron` | 0.229 / 0.925 / 0.925 / **0.926** | 45 of 64 | 34 parts: the starter's two pulse lasers with four reactors, four capacitors, six radiators, four logic cores and RCS pods in place of the cargo bays; every gun clear (latent 2.3 = as built); behavior `target threat(weapons)`, flee below 18 % hull under attack, `face` when closing in, else `approach` |

**Reactor shutters are gone.** None of the best 8 design elites has a reactor (or any part that explodes) in a gun's
line of fire. Five have blocked guns, all of them **reserve guns** behind their own guns (lasers or cannons behind
the nose cannons, sometimes with a logic core or gyro between; one also has two lasers facing backwards through its
hull), which fire when the front gun is shot off; the best one also runs more guns than its control capacity, so the
idle ones wake as parts are lost (GDR-0044). The front guns are 1-hit-point cannons, so it is the same ablative idea
with guns in place of reactors, and nothing in it explodes.

Fights of 60 s, seeds 1–20 against each pool opponent (2 raiders / 3 raiders / the hunter):

| Design | Rules | Won | Lost | Undecided |
|---|---|---|---|---|
| Last run's best (reactor and slope shutters) | before | **52** (19 / 15 / 18) | 1 | 7 |
| Last run's best (reactor and slope shutters) | with chain reactions | **12** (3 / 1 / 8) | 16 | 32 |
| This run's best (reserve guns) | before | 60 (20 / 20 / 20) | 0 | 0 |
| This run's best (reserve guns) | with chain reactions | **59** (20 / 19 / 20) | 1 | 0 |

A reactor shot off now takes the cannon it covers and, since the two reactors sit side by side, the other reactor,
the second centre cannon and both slopes with it (`sim/tests/report.rs`). The new best design wins its fights in 12–22 s
(mean per opponent), the old one took 31–40 s. It also wins all 60 under the old rules, so the old run had not found
it; with latent firepower the archive files designs with reserve guns by all their guns (32 cells filled against 24).
Open for Niklas: whether reserve guns behind guns and a control load above the capacity are wanted (both follow the
rules as decided). Answered on 14 Oct 2026: fine, emergent play ([GDR-0085](gdr/0085-chain-reactions-v2.md)).

**With chain reactions v2** (14 Oct 2026, [GDR-0085](gdr/0085-chain-reactions-v2.md): blasts reach every piece and
ship, armour on the line shields, reactors and drives reach two cells deep, automaton cores, shield emitters and RCS
pods explode). The same three runs, same seeds, on 10 threads; the `dev` build before reproduces the table above
exactly:

| Run | Archive best, gen 0 / 10 / 20 / 29 | Cells | Best elite |
|---|---|---|---|
| `skiff_behavior.ron` | 0.647 / 1.000 / 1.000 / **1.000** | 40 of 48 | `target threat(weapons)`, flee below 25 % hull, `face` while closing in and not yet within 46 % of its range, else `approach(81%)` (two rules below never run); the starter's start scores lower (0.647 against 0.726, the same with GDR-0084's radii) |
| `skiff_design_60bp.ron` | 0.583 / 0.917 / 0.917 / **1.000** | 33 of 64 | 20 parts: two automaton cannons clear in the nose between reactive slopes, four logic cores in a 2 × 2 block behind them with cargo bays beside, gyros and RCS pods, the two fission reactors in the rear corners, away from guns and cores; **no automaton core**; latent firepower 2.4 (= as built) |
| `skiff_voyage_stages.ron` | 0.198 / 0.918 / 0.923 / **0.927** | 29 of 64 | 32 parts in five rows: the starter's rows plus a second row of radiators, reactors, cargo bays and logic cores, shield emitters in place of the shoulder armour, the shoulder radiators gone; every gun clear; behavior `target threat(weapons)`, flee below 25 % hull, orbit while its shield is down under fire, `face` when closing in, else `approach(98%)` |

**Does the automaton core still dominate?** Less. In the design run's archive 2 of 32 elites had automaton cores
before (the best among them) and 12 of 33 after (not the best); one run is one path, so the design run went again
with seeds 1–5 under each rule set, every archive elite written:

| Rules | Archive elites with automaton cores | Automaton cores in them | Best elites with automaton cores |
|---|---|---|---|
| GDR-0084 | 88 of 163 (54 %) | 196 | 4 of 5 |
| v2 with GDR-0084's radii | 67 of 145 (46 %) | 154 | 4 of 5 |
| v2 (chosen) | 63 of 149 (42 %) | 142 | 2 of 5 |

An automaton core now goes off like a fission reactor (1.5 within 1.5 m), so one shot out takes the 1-hit-point
machines around it: still common, no longer the default.

Fights of 60 s, seeds 1–20 against each pool opponent (2 raiders / 3 raiders / the hunter), as above:

| Design | Rules | Won | Lost | Undecided |
|---|---|---|---|---|
| GDR-0084's best (reserve guns, two automaton cores) | GDR-0084 | **59** (20 / 19 / 20) | 1 | 0 |
| GDR-0084's best | v2 | **59** (20 / 19 / 20) | 0 | 1 |
| v2's best (no automaton core) | GDR-0084 | **53** (20 / 13 / 20) | 6 | 1 |
| v2's best | v2 | **54** (20 / 14 / 20) | 4 | 2 |

The fitness (4 seeds per opponent) saturates at 1.000, so the archive's best is one of many perfect scores, not the
strongest design: GDR-0084's best still wins more of these 60 fights under the new rules, and the run did not find it
again from this seed.

### Performance

Native release on all 16 threads (`arena evolve`, 3 runs each, load average 7–13 from other agents' builds),
14 Oct 2026, with ADR-0037:

| Run | Generations per minute, median (range) | Fights or voyages per generation | Breeding ms per generation, median (p95) |
|---|---|---|---|
| `skiff_behavior.ron` | **339** (312–345) | 384 fights | 4.0 (5.0) |
| `skiff_design_60bp.ron` | **386** (386–430) | 384 fights | 6.5 (9.0) |
| `skiff_voyage_stages.ron` | **211** (209–214) | 128 voyage runs | 4.8 (6.9) |

With GDR-0092 steps 2 and 3 (15 Oct 2026, the 4-core VM, load average 0.9, `skiff_design_60bp.ron`, 20 generations
on 4 threads): breeding **10.3 ms** per generation of 32, median (p95 12.1), so about 0.3 ms a child (`BREED_COST`
60 ticks); it now also opens each child where its parts are for the locks and the symmetry, cuts the behavior to the
rule slots, checks the canonical text and the hold, and stores the change against the parent. 170–174 generations per
minute on 4 threads; writing the run file after every generation is part of that.

`skiff_behavior.ron` on one thread (before ADR-0037): 32.7 generations per minute (16 threads about 11× that). A generation is 0.4–0.75
million ticks; the generation wall clock is 0.15–0.4 s.
