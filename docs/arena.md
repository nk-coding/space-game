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
| `pilot` | Optional (written only when there is one; GDR-0092 step 6): a **stand-in pilot** flies the entry's first ship, `(profile: "casual", flies: (lock: true, close_to: 0.8, stick: Some((on: 3, off: 3)), fire: Some((on: 3, off: 2))))`, inlined from `lab.ron`'s `pilots` so a code doesn't change when `lab.ron` does. That ship is its side's focus (its behavior parked) with the design's pilot assists (its blueprint's `assists`), and the referee gives it the pilot's input on every think tick (`sim::flight::stand_in`): its lock (the one it holds, else its auto-lock's pick, else the nearest enemy when `lock`), the stick towards its lock or the nearest enemy while that is farther than `close_to` of its longest gun's reach, every trigger, stick and triggers each on for `on` s then off for `off` (left out: never); the assists take the channels it leaves idle, as for a player (GDR-0088). The others of the entry fly the behavior. One entry a side at most, none in a wave (`ArenaError::Pilot`) |
| `start_distance` | Metres between the two sides' front rows (centre to centre), above 0, at most 5000 |
| `time_limit` | Seconds, above 0, at most 1800 |
| `waves` | Optional (written only when there are any): side 1's reinforcements, a list of sides. A **gauntlet** (GDR-0092 step 5): each wave comes in `WAVE_DELAY` (4 s, the test range's pause, placeholder) after the one before has no running vessel left, its ships on an arc around side 0's first running vessel heading for it, as the test range's waves come in (`sim::voyage::waves::wave_bodies`, rolled on the `Scenario` stream); side 1 is not out while a wave is still to come. The result counts the waves cleared (`FightResult::waves`, side 1's own entries the first; written only for a gauntlet), and side 1's build points lost count every wave it brought in. `fights/runner_gauntlet.ron` is one |

Blueprint and behavior texts are written as multi-line strings by `to_ron`, so a written spec stays readable.

**Placement.** Side 0 stands at the origin with its noses to +y, side 1 is the same layout turned half a turn about
the middle of the field, `start_distance` ahead. Ships of an entry stand `FORMATION_GAP` (5 m, placeholder) apart
by their grid bounds; the next entry stands the same gap behind. Every ship is moved by up to `JITTER` (2 m,
placeholder) in x and y, rolled on the sim's `Scenario` stream from the seed: a plain fight rolls nothing in the sim,
so without the jitter every seed would give the same fight.

**Owners.** Side 0 is `Owner(1)`, side 1 `Owner(2)` (`arena::OWNERS`): neither is the player's `Owner(0)`, so both
sides play by the same rules (only the player collects salvage). Both get unlimited mind capacity: every core runs.
Every ship flies its behavior from the first tick; nobody pilots, unless an entry has a stand-in `pilot`.

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
[What a run keeps](#what-a-run-keeps). Since step 6 (16 Oct 2026) everything mutates, the pilot assists included,
within the genes a run may change, and fights can be flown by a stand-in pilot so the assists are scored:
[Genomes, mutation and crossover](#genomes-mutation-and-crossover), FLOWN BY under [Trials and goals](#trials-and-goals).
Since step 7 (16 Oct 2026) an enemy population can evolve against yours in the same run, the ARMS RACE:
[Co-evolution](#co-evolution-the-arms-race).

### Trials and goals

Since GDR-0092 step 5 (16 Oct 2026) a run is scored against any number of **trials**, each with a weight, and its
score weighs four **goals** (`src/evolve/trial.rs`). `RunConfig` gets `trials: [...]` and `goals: (...)`; the old
`fitness` still reads (and writes) as it was, as one trial, so the runs before step 5 and their files are unchanged:
`two_starts_locked` seeds 1 and 7 still hash `4f126e780aff05c0` and `887374c2560c32e7`.

```ron
trials: [
    (preset: "enemies_you_meet", kind: Encounters(voyages: ["yard_approach"]), weight: 2, seeds: 4, time_limit: 60),
    (kind: Waves(target: 5), seeds: 2, time_limit: 180),
    (kind: BeatYourStart, seeds: 2, time_limit: 60, escorts: [(design: Blueprint("…"), behavior: Some(Text("…")))]),
    (kind: HallOfFame(count: 4), seeds: 2, time_limit: 60),
],
goals: (win: 1, unhurt: 1),   // left out: WIN alone
```

Every trial shares `weight` (default 1; 0: not fought), `seeds` (each opponent on seeds 1..=n), `count` (ships of the
contestant), `start_distance`, `time_limit`, `escorts` (WITH YOUR ESCORTS: these fly beside it on side 0; on a voyage
they come along under the content's escort behavior), `preset` (the `lab.ron` preset it came from, for its name) and
`flown_by` (FLOWN BY, step 6; left out: `Behavior`, as before): `StandIn(pilot)` has a stand-in pilot fly the
contestant's first ship in every fight (the spec's entry `pilot`, [The fight spec](#the-fight-spec)), `Both(pilot)` on
the even seeds while the behavior flies the odd ones, at no extra cost: the assists are scored on half the fights, the
behavior on the other half. A voyage has its own pilot (`Voyages`' `pilot`). The pilots are `lab.ron`'s `pilots`
(CASUAL, the default: locks the nearest enemy, flies at it until it is within 80 % of its longest gun's reach, lets go
of the stick 3 s every 6 s, fires 3 s on and 2 s off; HANDS OFF: only steers, the lock and the triggers are the
assists'; BUSY: locks, steers and fires all the time; placeholders). The voyage runner keeps its own closing pilot.
At `Run::new` each spec resolves against the content into a `TrialKind` (the one abstraction: `jobs(contestant,
ctx)`, `hall_of_fame()`), which then makes a genome's jobs against the generation's frozen starts and hall of fame;
the run is self-contained (player designs inlined as text, content by id).

| Kind | What it fights |
|---|---|
| `EnemyShips(opponents, hall_of_fame)` | ENEMY SHIPS: each entry a fight per seed, and the run's best `hall_of_fame` in the same mean (the old `fitness: ArenaPool` reads as this) |
| `Encounters(voyages)` | THE ENEMIES YOU MEET: every encounter of these voyages that has ships (each once, in their sites' order), its ships at their largest counts, a fight each; the guardian with its voyage (`evolve::encounters`) |
| `Waves(target)` | TEST-RANGE WAVES: a gauntlet of `target` (5) equal waves as the test range sends them, sized from the contestant's build points (`sim::voyage::waves::wave`); WIN is the waves cleared of `target` |
| `Gauntlet(waves)` | fixed waves, one after the other (`lab.ron`'s raider gauntlet) |
| `BeatYourStart` | every start under its own behavior |
| `Fleet(designs)` | YOUR FLEET: the player's designs, inlined |
| `HallOfFame(count)` | the run's best `count` so far (frozen per generation; none in generation 0, so it leaves the sums there) |
| `Champions(designs)` | CHAMPIONS: the best designs of earlier runs, inlined |
| `Voyages(ids, minutes, pilot)` | VOYAGES: headless voyages (the old `fitness: Voyages` reads as this) |
| `CoEvolution` | CO-EVOLUTION, the ARMS RACE (step 7): the run's `enemy` population's frozen best and past champions, each in a pack sized to the contestant ([Co-evolution](#co-evolution-the-arms-race)); its settings are the enemy's fights too |

**Goals** (per job, 0..1): WIN (the arena score; a gauntlet's waves cleared of all; the voyage score), UNHURT (one less
the share of side 0's build points lost; of the hull, on a voyage), QUICK (one less the share of the time limit a win
took, 0 otherwise), CHEAP (one less the design's build points over its build-point limit, else its start's). A child's
score: per trial, Σ goal weight × the goal's mean over the trial's jobs ÷ Σ goal weight; then Σ trial weight × that ÷ Σ
trial weight, trials without jobs and of weight 0 left out. One trial with WIN alone is the mean score of its jobs, bit
for bit the score before step 5. Records of runs with trials or goals keep every job's goals (`JobRecord::goals`; the
old form's files stay as they were); `Run::breakdown(generation, child)` takes a score apart by trial and goal,
`Run::tagged_jobs` says which trial each job is of.

**Presets** come from `lab.ron` (`content/base/lab.ron`, `data::Lab`: the goals' words and default weights, the goal
presets WIN, WIN UNHURT, WIN CHEAPLY, WIN FAST, and the trial presets with their weights, seeds and time limits; the
default selection THE ENEMIES YOU MEET (weight 2, 4 seeds), TEST-RANGE WAVES, BEAT YOUR START and HALL OF FAME (4)).
`TrialSpec::from_preset(preset, weight, seeds, escorts, &Sources)` makes a run's trial of one, filling in what comes
from outside (`Sources`: the open voyages, YOUR FLEET's designs, CHAMPIONS, FLEET's escorts), refused in words when it
has nothing to fight. The game's setup and the CLI's `--trial` use it.

**VALIDATE** (`Run::validation(genomes, seeds)` → a steppable `Validation`, `step(budget)` or `jobs()`/`submit()`,
then `Run::validated_by`): the genomes fight the run's trials on `seeds` new seeds (the ones after each trial's own;
`VALIDATE_SEEDS` 20 and `VALIDATE_BEST` 8, placeholders) against the starts and today's hall of fame, and their
recorded fights again under today's rules; each gets a `Validated` (its score on the new seeds, per trial, and how
many recorded fights ended as recorded), kept in the run file.

**RE-SCORE** (`Run::rescored(&content, &save)`): a stale run as a run under today's rules: its history and fights are
kept (each generation fought before keeps the rules it was fought under, `GenerationRecord::rules`, so its fight codes
say so), and its next generation (`GenerationStats::rescore`) fights the archive's elites again and rebuilds the
archive from their new scores; an elite that no longer passes is left out, counted as refused. It takes none of the
run's generations and then goes on breeding. Deterministic in slices and on threads, and from a file written in the
middle of it (`tests/trials.rs`).

### What a trial costs

Measured 16 Oct 2026, native release, 4 threads (the 4-core VM, load 0.9–1.4), `skiff_design_60bp.ron` (the starter,
32 children) with one trial at a time, 3 generations, the lab's preset numbers; a new career's open voyage
(`yard_approach`: a raider pair and the derelict field's raider):

| Trial | Jobs a child | Mean ticks a job (share of its limit) | Generations per minute (32 children) |
|---|---|---|---|
| THE ENEMIES YOU MEET (2 encounters × 4 seeds, 60 s) | 8 | 936 (0.52) | 195 |
| TEST-RANGE WAVES (5 waves × 2 seeds, 180 s) | 2 | 1839 (0.34) | 372 |
| BEAT YOUR START (1 start × 2 seeds) | 2 | 920 (0.51) | 712 |
| HALL OF FAME (4 × 2 seeds; none in generation 0) | 5.3 | 719 (0.40) | 345 |
| ENEMY SHIPS (2 packs × 4 seeds) | 8 | 857 (0.48) | 213 |
| Raider gauntlet (4 waves × 2 seeds, 180 s) | 2 | 1366 (0.25) | 438 |
| YOUR FLEET (1 design × 2 seeds) | 2 | 660 (0.37) | 745 |
| CHAMPIONS (1 design × 2 seeds) | 2 | 466 (0.26) | 990 |
| VOYAGES (1 voyage × 2 seeds, 10 minutes) | 2 | 2329 (0.13; early designs, lost early) | 249 |

A tick costs 5–6 µs on one thread whatever the kind (a voyage's too). The default selection is about 20 jobs and
19 000 ticks a child, 0.6 M ticks a STANDARD generation: about 6 s on the web with the layer open (wasm 4.7–7.9 µs a
tick, GDR-0092 step 4), about 0.8 s natively on 4 threads (0.8 M ticks a second). A voyage job is 3–6 fights' worth once designs survive it
(minutes, not seconds), so VOYAGES stay off by default and the setup's estimate shows their cost; they run on the web
too. All nine trials together: 1.0–1.2 M ticks a generation (42 generations a minute on 4 threads).

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
| `evolve` | `MayChange((...))`: the genes a run may change (GDR-0092 "MAY CHANGE", step 6), each left out on, so `MayChange(())` is everything ([Genomes, mutation and crossover](#genomes-mutation-and-crossover)); `Behavior` (the behavior's genes: numbers and rules), `Design` (the design's: layout, parts, armour, power), `Both` (both, not the assists); or `Stages([(evolve: Behavior, generations: 10), (evolve: Design, generations: 10), (evolve: MayChange(()), generations: 10)])`: one after the other, one archive throughout. Nothing to change is an error |
| `fitness` | `ArenaPool(opponents, seeds, hall_of_fame, count, start_distance, time_limit)`: the genome (`count` ships of it, side 0) fights each opponent entry and the best `hall_of_fame` elites so far over seeds 1..=`seeds`; the mean arena score. `Voyages(ids, seeds, minutes, pilot)`: each voyage over seeds 1..=`seeds`, the mean voyage score; `pilot: Behavior` (default: the genome's behavior flies the fights) or `Closing` (the runner's scripted pilot) |
| `limits` | Each optional. `frame`: a content frame id (24, 40 or 64 cells; default the smallest every start fits). `build_points`: the most a design may cost: `Start` (the default since GDR-0092: its own start's build points, "same cost, better ship"), a number (`build_points: 60`) or `Off`. `min_hold`: the smallest cargo hold (`Content::ship_hold`): `Start` (the default: its start's, so every result still brings salvage home), a number or `Off`. `max_cores`: the most logic cores (VOYAGE-READY: the career's mind capacity less the escorts' cores; default any). `counts`: how many of a part a design may hold (`{"pulse_laser": 2}`, `Career::available`; the drydock refuses more with `NoneLeft`). `parts`, `materials`: only these (e.g. a career's `Unlocks`; default all, locked ones too). `mirror`: the mirror lock (every edit mirrored, designs stay symmetric; the starts must be symmetric) |
| `start`, `starts` | One start (`start`, the old form) or a list (`starts: [...]`), one to four in all. `Design(design, behavior, locks)`: a content ship or blueprint text (with its pilot assists, `assists`), a behavior (default: the design's own, else its faction's) and what the run keeps of it ([Starts and locks](#starts-and-locks)). `Random(faction: "ship_mind")`: a random design that passes the pre-filter, under the faction's behavior |
| `archive` | `(axes: [(measure, min, max, cells), ...])`, one to three axes (at most 65 536 cells) |
| `enemy` | Optional (written only when there is one; step 7): THE ENEMY of an ARMS RACE, a `Population` ([Co-evolution](#co-evolution-the-arms-race)); a run with one has exactly one `CoEvolution` trial, and one with that trial an `enemy` |
| `population`, `generations`, `seed`, `crossover`, `mutations` | Children per generation; generations; the run's stream; the share of children bred by crossover (0.2); a child gets 1 to `mutations` (3) mutations. All *placeholders* |

Every error is a `RunError` value: `Config` (does not read, a number out of range, `generations` missing or given with
`Stages`, an unknown frame, `EngageDistance` with the voyage fitness, no start or more than four, fewer children than
starts), `Start` (an unknown ship or behavior, a start breaks a limit, its locks or the pre-filter, a lock names a
part, rule or category it lacks, a KEEP EMPTY cell it fills), `Fitness` (an opponent or voyage that does not resolve:
`Run::new` builds one fight or voyage of each with every start), `Save` (a run file that is damaged, of a newer
format, or doesn't match its configuration).

### Genomes, mutation and crossover

A `Genome` is a blueprint (trimmed, as the drydock exports it; named "<start> (evolved)", tagged `evolved`, no
behavior field, its pilot assists in `assists`), a behavior text (canonical, without the start's notes), its `root` (the
start it descends from) and `at`, where the blueprint's top-left cell sits in that start's frame.
`genome.blueprint_text()` is the content RON form, `genome.behavior_text()` and `genome.assists_text()` the text forms,
so the game can SAVE AS DESIGN; `genome.placed(run.frame(root))` is the
design as the drydock holds and saves it (`size` = the frame, parts in frame coordinates).

**Genomes keep frame coordinates** (GDR-0092 step 3): each start is opened centred in its frame once
(`Drydock::open`), and from then on every genome is opened where its parts are (`Drydock::open_placed`, `docs/drydock.md`),
never centred again, so a design that grows on one side keeps its cells and the locks (frame cells) never drift.
Designs change only through `data::design::edit::Drydock` (in the start's frame: the run's, one column narrower under
the mirror lock when the start's width has the other parity), so every child is a legal placement; under the mirror
lock an edit whose mirror image is impossible (`Twin::Skipped`) is undone, and so is an edit that breaks a lock (a
locked part's mirror twin, a part over a KEEP EMPTY cell). Behaviors change only through
`data::behavior::edit::BehaviorEditor`, and the pilot assists through the same editor as an assist list
(`from_text_as(.., ListKind::Assists)`: only the assists' vocabulary, no target line), so every program is legal.

**What may change** (GDR-0092 "MAY CHANGE", step 6): the genes, `Gene` (`lab.ron`'s `genes` hold their words, by id):
LAYOUT, PARTS (of the categories not in `fixed`), ARMOUR, POWER, BEHAVIOR NUMBERS, BEHAVIOR RULES, ASSIST NUMBERS,
ASSIST RULES (the assists split in two, so NUMBERS ONLY can keep every structure). A run's `MayChange` names them
(`evolve: MayChange((assist_rules: false, fixed: ["weapons"]))`); the old forms are genes too: `Design` the design's
four, `Behavior` the behavior's two, `Both` all six. `lab.ron`'s `gene_presets` are the setup's pills (EVERYTHING,
DESIGN ONLY, BEHAVIOR ONLY, ASSISTS ONLY, NUMBERS ONLY: the behavior's and the assists' numbers). One mutation draws one
operator among those of the genes on, by these weights (*placeholders*, GDR-0092's tables), tried up to 8 times; no
operator picks a locked part, KEEP EMPTY cell, locked rule or a locked target line:

| Gene | Operator | Weight | How |
|---|---|---|---|
| PARTS | add a part | 30 | an allowed part (of a category not kept) on a free cell next to a random part: slopes oriented to the hull (`place_slope`), others in a random pose; only where the preview replaces nothing (since GDR-0081 `place` replaces what is under it). A gun goes into the weapon group of the nearest gun (its mirror twin with it; none: group 1) |
| PARTS | remove a part | 15 | `remove_part` (and its twin); like every operator on a part, only an unlocked one |
| PARTS | swap a part | 15 | another allowed part of the same footprint and shape in its place and pose: `place`, accepted only when it replaces just that part (and its twin); a gun for a gun keeps its group |
| LAYOUT | move / rotate / flip | 15 / 10 / 5 | by −2..2 cells each way (`move_part`, refused into other parts); `rotate`; `flip` |
| ARMOUR | material | 10 | another allowed armour material on a hull part |
| POWER | power priority | 5 | a part that draws power: its tier one up or down (Critical to Low) or its weight × 0.6–1.4 (two decimals, within `MAX_POWER_WEIGHT`); the twin follows (`set_power`) |
| NUMBERS | nudge a number | 25 | a comparison's threshold (one time in ten its comparator too) or a numeric parameter, × 0.6–1.4, rounded (`set_compare`, `set_param`, refused out of range) |
| NUMBERS | change a choice | 5 | another option (an orbit's direction, a target's aim point) |
| RULES | move / swap two rules | 8 / 4 | `move_rule` to a random place; two unlocked rules trade places |
| RULES | duplicate a rule | 4 | `duplicate_rule`, then one more mutation of the copy (a number, a choice, its action or a condition; none took: undone) |
| RULES | replace an action / the target | 8 / 5 | `replace_node` with a random action or selector |
| RULES | add / remove a rule | 8 / 7 | `add_rule` (with a random condition four times in five), `delete_rule` |
| RULES | add / remove / negate / replace a condition | 7 / 5 / 4 / 4 | `add_condition`, `remove_condition`, `toggle_not`, `replace_node` |
| ASSIST NUMBERS | nudge a number / change a choice | 25 / 10 | as the behavior's (`idle(thrust) >= 1s`, auto-fire's group, auto-lock's pick, hold station's point or ally) |
| ASSIST NUMBERS | switch `start` | 8 | on ↔ off: whether a rule acts at all (an assist's last value, as the sim compiles it) |
| ASSIST RULES | move / add / remove a rule | 10 / 12 / 8 | an assist with a random condition four times in five; a new rule's toggle is `none` |
| ASSIST RULES | add / remove / negate / replace a condition | 8 / 6 / 4 / 4 | the pilot's conditions (`lock_held`, `idle(..)`) and the behaviors' |

An assist's toggle (its last but one value) never changes: nobody presses a toggle in the lab, so the player sets the
keys after saving. Rules are added only below the design's rule slots (behaviors) or `MAX_RULES` (assists). Weapon
groups are no gene: they only say which key fires a gun, and a stand-in fires every group. Before step 6 a mutation
first picked design or behavior (a coin under `Both`), with other weights and without power, swap, duplicate or the
assists: runs breed differently since (their files still read, restore and replay, [Determinism](#what-a-run-keeps)).

**Crossover** (a `crossover` share of the children, before their mutations): designs splice `a`'s parts above a
random row with `b`'s from that row down (both in one frame: two starts of different frames don't cross; `b`'s parts
that overlap or cover one of `a`'s KEEP EMPTY cells are left out, `a`'s locked parts below the row stay), behaviors
`a`'s first rules with `b`'s from a random rule on (the target line of either, `a`'s when it is locked; `a`'s locked
rules past the splice stay, before `b`'s, which bring no second copy), the pilot assists likewise (`a`'s first rules,
any number, then `b`'s from a random rule on, `a`'s locked assists kept). The child is `a`'s: its start, its locks.
The genes on decide which halves cross (the design's, the behavior's, the assists').

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
            assists: [1],                 // its pilot assists' rule 1 (step 6)
            order: true,                  // LOCK ORDER: the locked rules keep their order (each list's)
        )),
    Design(design: Ship("halvex_runner")),
],
```

Under the mirror lock a locked part or cell locks its mirror image too. Locked assist rules (GDR-0092 step 6) are kept
like locked behavior rules (never changed, moved or deleted; LOCK ORDER keeps their order too). Rules may still be added around a locked rule,
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

### Co-evolution (the ARMS RACE)

GDR-0092 step 7 (16 Oct 2026; `src/evolve/`, `tests/coevolve.rs`): an enemy population evolves against yours in the
same run, each improving against the other ("self-improving enemies"). `RunConfig::enemy`:

```ron
// crates/core/arena/runs/arms_race.ron (with a `CoEvolution` trial among `trials`)
enemy: (
    starts: [Design(design: Ship("automaton_raider")), Design(design: Ship("automaton_hunter"))],
    population: 4,                // its children per generation (the lab: half of yours)
    palette: ["shield_emitter"],  // parts beyond those its starts use (lab.ron's palette of the faction)
    mirror: true,                 // its designs stay symmetric
    elites: 2, hall: 2,           // the frozen sets (default 3 and 2)
    anchor: true,                 // its children also fight your starts (default)
    check_every: 10, champions_every: 5,   // PAST CHAMPIONS (the defaults; 0: never)
    // evolve: MayChange((...)),  // default the design's and the behavior's genes; never the assists
    // frame, archive             // default the smallest frame its starts fit, the default axes
),
```

- **One run, one table.** THE ENEMY's starts are genomes n to n + m − 1 after your n starts (`Bred::root` ≥ n: its
  side, `Run::camp`), its children follow yours in every generation (`GenerationRecord::enemy`, a `Rivalry` with
  `first`, where its children begin, `Run::children_of`), its headers are tagged `evolved` and `enemy` with their
  faction kept. It breeds with a kit of its own (`Kit::first` numbers its roots after yours): only the parts and
  armour its starts use and its palette, no build-point cap, hold or core limit, on its own stream (the run's seed on
  the sequence `"enemy"`), so your breeding draws as it would without it. Its archive is its own (`Run::enemy_archive`).
- **Pairing.** Generation g fights frozen sets from the end of g − 1: your children THE ENEMY's best `elites` and
  `hall` of its past champions, its children your best `elites`, `hall` of yours and (`anchor`) your starts;
  generation 0 the other side's starts. A champion is a generation's best child of its side; champions are drawn
  uniformly over the whole history, each once and none among the best already, on the enemy's stream (its first,
  then yours), and the drawn genome numbers are kept per generation (`Rivalry::opponents`, `rivals`), so every job
  rebuilds. Every fight has your side as side 0 and the enemy as side 1 (`Job::Fight::side`, 1 for an enemy's own
  jobs, written only then), the trial's seeds, time limit, copies, escorts and FLOWN BY applied to your side either
  way, so a replay looks the same from either population.
- **Fair packs** (`arena::evolve::pack`): an enemy design fights in a pack sized by the test range's wave rule to
  your side's build points (the contestant's times its copies; `sim::voyage::waves::wave_mix` with that one design),
  at least one: a bigger enemy comes in fewer copies, so it evolves strength per build point without a cap.
- **Scores.** Yours as before, the `CoEvolution` trial one of your trials with its weight. THE ENEMY's score is side
  1's WIN, the mean over its fights (its goals kept in the records; `Run::breakdown` gives its one trial).
- **PAST CHAMPIONS** (`check_every`, `champions_every`): in every `check_every`th generation (not the first, nor a
  RE-SCORE) your archive's best fights THE ENEMY's champions of every `champions_every`th generation so far and its
  best yours, after the children's jobs (`GenerationRecord::enemy.check`, `Check`). `Run::beaten` gives per checked
  generation how many of the other side's champions each best beat (a mean score above 0.5 over its seeds): a rising
  line is an arms race, a saw is cycling. `Run::check_jobs`, `check_fight_code`: its fights, re-watchable.
- **Saving evolved enemies.** SAVE AS DESIGN keeps an enemy's faction and its `evolved` and `enemy` tags, with its
  origin. `Run::export_enemy(id, run, n)` is EXPORT TO CONTENT: the content ship format (`data::export_text`) under
  `evolved_<faction>_<n>`, its origin, naming its behavior of the same id, which follows as a content behavior file in
  a comment block, for a pack's `ships/` and `behaviors/` by hand (curated, GDR-0032; never into voyages by itself).
  `arena evolve --out` writes the enemy's best as `enemy_NN.ron` so.
- **VALIDATE** is of your designs (an enemy's genome is refused); your designs fight THE ENEMY's best `elites + hall`
  of today (no draws). A RE-SCORE fights both archives' elites again against the sets frozen as usual.
- **Run files** keep the enemy's stream and archive (`enemy` in the payload and in the head's, written only then);
  everything else is in the table and the records, so the parts (ADR-0041) need nothing new. A run without an enemy
  reads, writes and hashes as before.
- **Cost** (the CLI, 16 Oct 2026, 4 threads): the starter with the default trials and the ARMS RACE (16 children, 8
  enemy children, 12 generations): 6.6 s; the ARMS RACE 9 jobs a child of yours (22 % of your ticks), the enemy 10 a
  child (11 % of the run's ticks).

What the first run found (`--trial default --trial arms_race`, seed 4): the enemy's archive best was 1.000 from
generation 3 on (an evolved hunter: its two cannons on top of its armour, `face` in bursts within 140 % of its reach, else close in), your
generation bests fell from 0.95 to about 0.8 as it improved, and at generation 10 your best beat 1 of 2 of its past
champions and its best both of yours.

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
"Evolution"; it keeps each run in parts since ADR-0041, [The run in parts](#what-a-run-keeps), with the head's name,
creation time and status):

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
| `Run::trials()`, `Run::tagged_jobs(generation, child)`, `Run::breakdown(generation, child)` | The run's trials; a child's jobs with their trial; its score by trial and goal ([Trials and goals](#trials-and-goals)) |
| `Run::validation(genomes, seeds)`, `Run::validated_by(v)`, `Run::validated(id)` | VALIDATE as work, its results into the run, a genome's result |
| `Run::rescored(&content, &save)` | RE-SCORE: a stale run as a new run under today's rules |
| `Run::camp(id)`, `camp_of(root)`, `children_of(generation, camp)`, `best_of(generation, camp)`, `start(root)`, `start_config(root)`, `enemy_starts()`, `enemy_archive()`, `archive_of(camp)` | An ARMS RACE's two populations ([Co-evolution](#co-evolution-the-arms-race)): whose a genome is, a generation's children of a side, its champion, any start, THE ENEMY's starts and archive |
| `Run::check(generation)`, `check_jobs(generation)`, `check_fight_code(generation, job)`, `beaten()`, `export_enemy(id, run, n)` | PAST CHAMPIONS: a generation's check, its fights rebuilt and as codes, the line of what each best beat; EXPORT TO CONTENT |
| `Run::tidied(id)` | TIDY at SAVE (GDR-0092 step 6): genome `id`'s behavior without the rules that never decided in any of its recorded fights its behavior flew (the records' `decided` bits, ORed; a stand-in's fights and voyages tell nothing), its start's locked rules kept and the top rule when none decided; with the dropped rules' indices and the fights that told. `None` when no fight told. The lab saves an evolved behavior so (on by default) |
| `RunSave::genome(&content, id)`, `RunParts::genome(&content, id)` | A genome of a run file or of a run's parts without building the run (the lab's CHAMPIONS) |
| `Run::head()`, `Run::part(generation)`, `Run::parts()`, `RunParts::of(&save)`, `Run::restore_parts(&content, &parts)`, `RunHead`/`RunPart::to_ron`/`from_ron` | The run in parts (the lab's storage): the small head, a closed generation's part, all of them, a run file split; going on from them is going on from the run file |

`Job`, `JobResult`, `JobRecord`, `VoyageSpec`, `FightSpec`, `FightResult`, `GenerationStats`, `GenerationRecord`,
`Bred` and `RunSave` are serde types with a RON form.

### What a run keeps

Every fight of a run can be re-watched (GDR-0092 step 2): a run keeps how every genome was bred and how every job
ended, and rebuilds any job's spec from them.

- **The genome table** (`Run::table()`, `arena::evolve::Bred`): every genome bred, numbered in breeding order (the
  starts are 0 to n − 1), as a change against its parent: the generation, its start (`root`), its parent and a
  crossover partner, the parent's parts it lost (indices) and the parts it gained (index and placement, in frame
  coordinates; the longest common run of the two part lists is kept, so a mutation stores a part or two), its behavior
  text when it changed, its pilot assists when they changed (`assists`, step 6; no rules: none; a start's are its
  header's), and its size. A start stores all its parts and its behavior. `Run::genome(id)` replays the
  changes from the start: the identical genome, header included (`tests/runs.rs` checks every genome against the
  blueprint and behavior its fights carried).
- **The records** (`Run::state().records`): per generation the hall of fame its children fought (genome numbers),
  per child its genome number, score, measures and whether it took a cell, and per job a `JobRecord`: the score, the
  ticks and how it ended (`JobEnd::Fight`: outcome, build points lost, damage dealt and vessels left per side, the
  final state hash, and a bit for every rule of the child's behavior that decided at least once,
  `Referee::decided`, for GDR-0092's TIDY, `Run::tidied`; `Voyage`: the final state hash; `Failed`). Each side's build points at the
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
- **The run in parts** (`RunParts`, ADR-0041, 16 Oct 2026; the game's lab keeps runs so, `docs/client.md` "Evolution"):
  the same run as a head, the configuration and one part per generation done, so a store that writes the run as it
  grows writes each generation once. `RunHead` is the run file's head without the configuration and the numbers per
  generation (`format`, the game's `name`, `created`, `status`, `rules`, `generations` done, `best`) and `payload:
  "SGL1:…"` with what changes between generations, packed: the random stream, the archive's elites, a due RE-SCORE,
  VALIDATE's results, and the genomes no part holds yet (a run with no generation done keeps its starts there).
  `RunPart` is a generation's `GenerationStats`, readable, and `payload` with the genomes bred for it (generation 0's
  begin with the starts) and its `GenerationRecord`, packed; nothing changes a closed generation, so a part is
  written once. `Run::head()` and `Run::part(g)` are what a generation's close writes, `Run::parts()` all of it,
  `RunParts::of(&save)` splits a run file, and `Run::restore_parts` goes on from the parts through the same restore
  as `Run::restore`: the run file's run, byte for byte (`tests/runs.rs` `a_run_in_parts_is_its_run_file`, also before
  the first generation and in the middle of one; `tests/trials.rs` with a due re-score and with VALIDATE's results;
  `run_hashes` restores from the parts too, so `determinism-check.sh --run` covers them). Sizes, measured 16 Oct 2026
  in the browser: the head about 0.3 kB, the configuration 4 kB, a part 9–14 kB for the starter's QUICK run (16
  children, the default trials) and 25–30 kB for 64 children × 12 fights; a 200-generation run file of 5.2 MB became
  5.6 MB of parts (each part compressed alone, and its numbers readable). The CLI keeps its one file.
- **Stale runs**: the run file keeps the rules' fingerprint the run started under (`rules_fingerprint`, built in step
  1). `Run::restore` on content whose fingerprint differs marks the run stale (`Run::stale()`): it keeps its fingerprint
  (its fight codes say which rules they were fought under), it is read-only (`step` does nothing, `jobs` is empty),
  and the CLI refuses to resume, replay or trace one. RE-SCORE ([Trials and goals](#trials-and-goals), step 5) makes a
  new run of it under today's rules.

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

**With trials and goals** (GDR-0092 step 5, 16 Oct 2026, release builds): `two_starts_locked` (the old `fitness`
form) seeds 1 and 7 unchanged (`4f126e780aff05c0`, `887374c2560c32e7`), native and wasm identical. The new
`runs/mixed_trials.ron` (the starter and the Runner against the first voyage's enemies, the test-range waves as a
gauntlet of two, their starts, the run's best and a one-minute voyage, weighing all four goals; 4 generations of 6):
seed 1 resumed after generation 2 **identical** (last line `222c9bee00fa033e`), seed 4 resumed after generation 1
**identical** (`8d85b8edf59c7a64`). The gauntlet `fights/runner_gauntlet.ron` seeds 1 (lost, 2883 ticks, 1 wave of 3,
`6bab099e86643a52`) and 3 (the time limit, 5400 ticks, 2 waves of 3, `d4205aa2207a2ca6`) identical; `runner_vs_raiders`
seed 2 unchanged (`4f4334e1218ed181`).

**Everything mutates** (GDR-0092 step 6, 16 Oct 2026, release builds): one mutation now draws among the operators of
every gene on, so breeding draws differ and every run breeds otherwise than before (its fights, fight codes and the
rules' fingerprint, `87f8e44db8186359`, are unchanged; old run files read, restore and replay their recorded fights).
Native and wasm identical, each resumed after generation 2 identical: `two_starts_locked` seed 1 `76a00f92091034f7`,
`mixed_trials` seed 1 `d544b205648f22a1`, and the new `runs/everything.ron` (the starter, every gene, its enemy ships
FLOWN BY BOTH with the casual stand-in; 5 generations of 8) seed 1 `ef0039c55cfebe6d`. A stand-in fight's code (that
run's generation 0, child 0, seed 2: won, 646 ticks) replays native = wasm (`5526178cace9f4f9`).

**The ARMS RACE** (GDR-0092 step 7, 16 Oct 2026, release builds): `runs/arms_race.ron` (the starter against the
raider and the hunter evolving beside it, PAST CHAMPIONS every second generation; 5 generations of 6 and 4) seed 1
resumed after generation 1 and seed 2 after generation 2: native and wasm **identical** (`cf042041f708d5e6`,
`d642fc5d8e93e758`); `two_starts_locked` seed 1 still `76a00f92091034f7`, `mixed_trials` `d544b205648f22a1`,
`everything` `ef0039c55cfebe6d`, the rules' fingerprint `87f8e44db8186359`.

### The CLI: `arena evolve`

```sh
cargo run --release -p arena-cli -- evolve crates/core/arena/runs/skiff_design_60bp.ron --out /tmp/skiff_design [--best 8] [--threads N]
cargo run --release -p arena-cli -- evolve crates/core/arena/runs/skiff_design_60bp.ron --save /tmp/run.ron --until 10
cargo run --release -p arena-cli -- evolve --resume /tmp/run.ron        # goes on, and keeps saving to it
cargo run --release -p arena-cli -- evolve crates/core/arena/runs/skiff_design_60bp.ron --from my_ship.ron --from rim_skiff --lock category:weapons --lock rule:0
cargo run --release -p arena-cli -- evolve crates/core/arena/runs/everything.ron --trial default --flown-by both:casual
cargo run --release -p arena-cli -- evolve crates/core/arena/runs/everything.ron --trial default --trial arms_race --out /tmp/race   # and enemy_NN.ron
cargo run --release -p arena-cli -- replay /tmp/run.ron 12 3 5         # generation 12, child 3, job 5: its fight code
cargo run --release -p arena-cli -- lineage /tmp/run.ron [GENOME]       # a genome's line from its start (default: the best)
```

Breeds on the main thread and runs each generation's jobs on every core (`--threads`, default all), prints a
Markdown row per generation (evolves, children, refused, best, mean, archive best, cells, took a cell, million
ticks, breeding ms, seconds; what evolves as `Design`, `Both`, `Everything` or the genes on, `Layout+Numbers`), the generations per minute, the best elites (rank, genome number, cell, score, measures,
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
| `--lock LOCK` | (repeatable) a lock of every start: `part:N` (its `parts[N]`), `category:ID` (`category:weapons`), `cell:X,Y` (KEEP EMPTY, in the start's coordinates), `rule:N` (its behavior's rule N, 0 = the top), `target`, `assist:N` (its pilot assists' rule N), `order` |
| `--flown-by WHO[:PILOT]` | every fight trial FLOWN BY `behavior`, `stand-in` or `both` (odd seeds the behavior, even the stand-in), with `lab.ron`'s pilot `PILOT` (default its first, `casual`); a run file's old `fitness` becomes a trial for it |
| `--trial PRESET[:WEIGHT[:SEEDS]]` | (repeatable) the run's trials from `lab.ron`'s presets instead of its file's (`default`: the lab's default selection; `arena trials` lists them) |
| `--goal ID:WEIGHT`, `--goals PRESET` | (repeatable) the goals' weights (`win`, `unhurt`, `quick`, `cheap`; the others weigh 0), or a goal preset (`win_unhurt`, ...) |
| `--enemy SHIP` | (repeatable) with `--trial arms_race`: THE ENEMY's starts (content ships of one faction; default the first enemy faction's ships, its boss left out); the rest is the preset's (`Population::from_preset`). The table then adds the enemy's best, archive best and cells, and after the elites come the enemy's best with their lineage and the PAST CHAMPIONS line; `--out` also writes `enemy_NN.ron` (EXPORT TO CONTENT) |
| `--open V,V`, `--fleet DESIGN`, `--champion DESIGN`, `--escort DESIGN` | what the presets fight: the open voyages (default: a new career's), YOUR FLEET's and CHAMPIONS' designs, the escorts every trial flies beside the design |

After its tables `evolve` prints the run's cost per trial (jobs a child, mean ticks a job, share of the ticks) and the
best elite by trial and goal. `arena trials` lists the presets; `arena validate <run file> [--genome G]... [--best N]
[--seeds N] [--save FILE]` fights the best (or the named) genomes on new seeds and their recorded fights again and
prints each one's run score, validated score and fights as recorded; `arena rescore <run file> --save FILE [--until
N]` re-scores a stale run under today's rules into a new run file (the re-score generation, then on to `--until`).

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
