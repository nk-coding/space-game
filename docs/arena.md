# Arena (`crates/arena`)

Headless, deterministic fights between designs on the real simulation: track **L1** of the evolution lab
(`docs/plans/evolution-lab.md`), written 14 Oct 2026; the voyage fitness (**L2**) and the evolution (**L3**) the same
day: [Voyage fitness](#voyage-fitness), [Evolution](#evolution). Decisions: [ADR-0036](adr/0036-arena-crate.md) (the crate),
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

The crate's binary (`src/bin/arena.rs`), not an example: it is the arena's tool for balance numbers and
`arena evolve` ([Evolution](#the-cli-arena-evolve)) (like `content-tool`), not a one-off measurement; the examples
hold checks (`fight_hashes`).

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

## Voyage fitness

Track L2 (`src/voyage.rs`): a design and a behavior flown through a content voyage by the headless voyage runner,
scored 0..1. The runner's pilot moved out of the `voyage` example into the sim as a library, `sim::voyage_runner`
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

### A run is a text file

`RunConfig` has a RON form (`RunConfig::from_ron`, `to_ron`; the content's RON options: `Some(..)` may be left out,
unknown fields are errors, comments are fine). Examples in `crates/arena/runs/`:

```ron
// crates/arena/runs/skiff_design_60bp.ron
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
    limits: (frame: "frigate", build_points: 60, mirror: true),  // all optional, also parts: [...], materials: [...]
    start: Design(design: Ship("rim_skiff"), behavior: Content("skirmisher")),
    // archive: (axes: [...]),        // default: acceleration × firepower, 8 × 8
    // crossover: 0.2, mutations: 3,  // the defaults
)
```

| Field | What |
|---|---|
| `evolve` | `Behavior` (the design stays), `Design` (the behavior stays), `Both` (each mutation picks one), or `Stages([(evolve: Behavior, generations: 10), (evolve: Design, generations: 10), (evolve: Both, generations: 10)])`: one after the other, one archive throughout |
| `fitness` | `ArenaPool(opponents, seeds, hall_of_fame, count, start_distance, time_limit)`: the genome (`count` ships of it, side 0) fights each opponent entry and the best `hall_of_fame` elites so far over seeds 1..=`seeds`; the mean arena score. `Voyages(ids, seeds, minutes, pilot)`: each voyage over seeds 1..=`seeds`, the mean voyage score; `pilot: Behavior` (default: the genome's behavior flies the fights) or `Closing` (the runner's scripted pilot) |
| `limits` | Each optional. `frame`: a content frame id (24, 40 or 64 cells; default the smallest the start fits). `build_points`: the most a design may cost. `parts`, `materials`: only these (e.g. a career's `Unlocks`; default all, locked ones too). `mirror`: the mirror lock (every edit mirrored, designs stay symmetric; the start must be symmetric) |
| `start` | `Design(design, behavior)`: a content ship or blueprint text, and a behavior (default: the design's own, else its faction's). `Random(faction: "ship_mind")`: a random design that passes the pre-filter, under the faction's behavior |
| `archive` | `(axes: [(measure, min, max, cells), ...])`, one to three axes (at most 65 536 cells) |
| `population`, `generations`, `seed`, `crossover`, `mutations` | Children per generation; generations; the run's stream; the share of children bred by crossover (0.2); a child gets 1 to `mutations` (3) mutations. All *placeholders* |

Every error is a `RunError` value: `Config` (does not read, a number out of range, `generations` missing or given with
`Stages`, an unknown frame, `EngageDistance` with the voyage fitness), `Start` (an unknown ship or behavior, the start
breaks a limit or the pre-filter), `Fitness` (an opponent or voyage that does not resolve: `Run::new` builds one fight
or voyage of each with the start).

### Genomes, mutation and crossover

A `Genome` is a blueprint (trimmed, as the drydock exports it; named "<start> (evolved)", tagged `evolved`, no
behavior field) and a behavior text (canonical, without the start's notes). `genome.blueprint_text()` is the content
RON form and `genome.behavior_text()` the text form, so the game can SAVE AS DESIGN.

Designs change only through `data::edit::Drydock` (opened in the run's frame, one column narrower under the mirror
lock when the design's width has the other parity), so every child is a legal placement; under the mirror lock an
edit whose mirror image is impossible (`Twin::Skipped`) is undone. One mutation is one of (weights are
*placeholders*):

| Design operator | Weight | How |
|---|---|---|
| add a part | 30 | an allowed part on a free cell next to a random part: slopes oriented to the hull (`place_slope`), others in a random pose; only where the preview replaces nothing (since GDR-0081 `place` replaces what is under it) |
| remove a part | 15 | `remove_part` (and its twin) |
| move a part | 15 | by −2..2 cells each way (`move_part`, refused into other parts) |
| rotate / flip | 10 / 5 | `rotate`, `flip` |
| swap a part | 15 | another allowed part of the same footprint and shape in its place and pose: `place`, accepted only when it replaces just that part (and its twin) |
| material | 10 | another allowed armour material on a hull part |

Behaviors change only through `data::behavior_edit::BehaviorEditor`, so every program is legal; rules are added only
below the design's rule slots:

| Behavior operator | Weight | How |
|---|---|---|
| nudge a number | 30 | a comparison's threshold (one time in ten its comparator too) or a numeric parameter, × 0.6–1.4, rounded (`set_compare`, `set_param`, refused out of range) |
| change a choice | 5 | another option (an orbit's direction, a target's aim point) |
| move a rule | 10 | `move_rule` to a random place |
| swap an action / the target | 10 / 5 | `replace_node` with a random action or selector |
| add / remove a rule | 10 / 8 | `add_rule` (with a random condition four times in five), `delete_rule` |
| add / remove / negate / replace a condition | 8 / 6 / 4 / 4 | `add_condition`, `remove_condition`, `toggle_not`, `replace_node` |

**Crossover** (a `crossover` share of the children, before their mutations): designs splice `a`'s parts above a
random row with `b`'s from that row down (both in `a`'s frame; `b`'s parts that overlap are left out), behaviors
`a`'s first rules with `b`'s from a random rule on (the target line of either). What evolves decides which halves
cross.

**The pre-filter** (`Kit::assess`, part of the breeding time below) runs before any fight: the limits (parts, materials, frame, build
points from `sim::design_report`, symmetry under the mirror lock), `data::validate_ship` without errors (this includes
the rule-slot check of the faction's default behavior: a design with fewer slots than it has rules is refused), and
the design report: it must fly (no blocker: a powered logic core and thrust), be one piece (no `FallsOff`) and carry a
gun. A child that is refused, or equals its parent, is bred again (16 tries, then the generation is one smaller); the
CLI's `refused` column counts the refusals.

### The archive and the loop

The archive is a grid over one to three **measures**: `TopSpeed`, `Acceleration`, `WeaponReach`, `BuildPoints`,
`Mass`, `Firepower`, `HitPoints`, `Parts` (from the design report), `Rules` (the rules that run, at most the slots),
`EngageDistance` (the mean distance from the genome's first running ship to the nearest enemy vessel in its fights,
sampled every half second; arena pool only). An axis has `cells` equal bins from `min` to `max` (values outside go to
the end bins); each cell keeps its best elite (genome, score, measures, generation); a child takes a cell when it is
empty or the child scores higher (ties keep the elite).

The default axes are **acceleration (0–24 m/s²) × firepower (0–16 damage per second), 8 × 8** (*placeholders*), not
coop-arena 3.8's top speed × weapon reach: those are ratings of the part types (every base ship flies 20–22 m/s and
reaches 45–75 m), so with the starter's parts every design sat in one or two cells (the first design run filled 2
cells in 3 generations). Behavior runs want behavior measures (`EngageDistance` × `Rules`, as
`skiff_behavior.ron`); in a behavior stage under design axes every child shares its parent's cell (a plain hill
climb).

Each generation: generation 0 is the start and `population − 1` mutants of it; later ones are `population` children
of elites drawn uniformly from the occupied cells. Each child's jobs (every opponent × seed, or every voyage × seed)
are evaluated; its score is their mean; then the children are offered to the archive in order. All random draws
come from the run's PCG stream (`seed`, its own sequence) in a fixed order; fights and voyages use seeds 1..=n, the
same for every child and generation, so scores stay comparable. The hall of fame (`hall_of_fame` best elites joining
the pool) makes the fitness move with the archive; elites are not re-scored.

### The API

```rust
let mut run = arena::Run::new(&content, config)?;   // checks the config, resolves the start, checks the fitness
let p = run.step(20_000);                           // up to 20 000 ticks of fights this frame -> Progress
let s = run.state();                                // generation, generations, stage, history, archive
for (cell, elite) in s.archive.cells() { /* elite.genome, elite.score, elite.measures, elite.generation */ }
let r = run.results();                              // history + every elite, best first
let (blueprint, behavior) = (r.elites[0].genome.blueprint_text(), r.elites[0].genome.behavior_text());
```

- `Run::step(budget)`: breeds the next generation when none is open, then runs its jobs for up to `budget` sim ticks
  (a fight or voyage is stepped in slices, `Fight::step` / `VoyageRunner::step`), and closes the generation after its
  last job. Breeding is not counted against the budget: 4–10 ms per generation of 32 (native release, below).
- `Run::jobs()` / `Run::submit(results)`: a generation's `Job`s (a `FightSpec` or `VoyageSpec` each, plain data) and
  their `JobResult`s in the same order; `Job::run(&content)` runs one anywhere. The CLI uses these; don't mix them
  with `step` within a generation.
- `GenerationStats` per generation: stage and genes, children evaluated, refused, best and mean score, the archive's
  best, occupied cells, children that took a cell, ticks.

**Determinism**: a job's result depends only on the job and the content (it is a fight or voyage, deterministic on
every platform), results are applied in job order, the archive is a `BTreeMap`. `tests/evolve.rs` checks that the
same config and seed give the same results run twice, stepped in slices of 97 ticks, and on 1 and 4 threads; another
seed gives another run. The CLI on 1 and on 16 threads printed identical tables and wrote identical files
(`skiff_behavior.ron`, 14 Oct 2026, before and after ADR-0037).

### The CLI: `arena evolve`

```sh
cargo run --release -p arena -- evolve crates/arena/runs/skiff_design_60bp.ron --out /tmp/skiff_design [--best 8] [--threads N]
```

Breeds on the main thread and runs each generation's jobs on every core (`--threads`, default all), prints a
Markdown row per generation (evolves, children, refused, best, mean, archive best, cells, took a cell, million
ticks, breeding ms, seconds), the generations per minute, and the best elites (rank, cell, score, measures,
generation, parts). With `--out DIR` it writes the best `--best` (8) elites: `elite_NN.ron` (the blueprint, id
`evolved_NN`, with a comment naming the run, rank, score and generation) and `elite_NN.txt` (its behavior).

### What the example runs found

`crates/arena/runs/`, 30 generations of 32 each, 14 Oct 2026 `dev` with ADR-0037 (simultaneous fire, shared think
ticks, no spawn-order stopgap); the archive's best after generations 0, 10, 20 and 29:

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
reactors do not explode yet (GDR-0019 decided chain reactions, GDR-0020 deferred them). All 8 elites have blocked guns
(3 of them firepower 0). The archive's firepower axis is the design report's, as built, so it files these designs
under little or no firepower. `sim/tests/weapons.rs` (`a_gun_behind_its_own_parts_fires_only_once_they_are_shot_off`)
and `sim/tests/report.rs` (`report_counts_blocked_and_idle_guns_as_the_sim_fires_them`, this design) hold the rule.

### Performance

Native release on all 16 threads (`arena evolve`, 3 runs each, load average 7–13 from other agents' builds),
14 Oct 2026, with ADR-0037:

| Run | Generations per minute, median (range) | Fights or voyages per generation | Breeding ms per generation, median (p95) |
|---|---|---|---|
| `skiff_behavior.ron` | **339** (312–345) | 384 fights | 4.0 (5.0) |
| `skiff_design_60bp.ron` | **386** (386–430) | 384 fights | 6.5 (9.0) |
| `skiff_voyage_stages.ron` | **211** (209–214) | 128 voyage runs | 4.8 (6.9) |

`skiff_behavior.ron` on one thread (before ADR-0037): 32.7 generations per minute (16 threads about 11× that). A generation is 0.4–0.75
million ticks; the generation wall clock is 0.15–0.4 s.
