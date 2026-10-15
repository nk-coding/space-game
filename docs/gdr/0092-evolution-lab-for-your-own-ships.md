# GDR-0092: The evolution lab is a station building for improving your own ships: runs from your designs, locks, combinable trials with co-evolving enemies, everything mutable, every fight re-watchable, results saved as designs

- Status: Accepted (Niklas's requirements and his four answers; the rest are the design agent's proposals and lead
  defaults, marked, for him to review). Step 1 built 2026-10-15: the arena library in `crates/core/arena`
  ([ADR-0040](../adr/0040-arena-library-in-core.md)), fight codes with the rules' fingerprint, the fight viewer
  `?scene=fight&fight=<code>` and side looks by owner (see "Phased build plan").
- Date: 2026-10-15
- Decided by: Niklas (the requirements quoted below, and four answers on 15 Oct 2026: a research node unlocks the
  building and runs are free; runs compute at home and on every screen and pause in flight; as many trials as
  possible, co-evolving enemies included; everything mutates by default, assists too). The rest is a proposal by
  the design agent; parts marked "lead default" are defaults for Niklas to review.
- Supersedes: [GDR-0083](0083-evolution-lab.md) in part: its "dev-only place for now" and "for players later a
  benchmark rating". The lab is player-facing, and its purpose is evolve-to-improve. GDR-0083's configurable runs,
  its engine-free crate and SAVE AS DESIGN stay.
- Related: [GDR-0032](0032-balancing-arena-evolution.md), [GDR-0083](0083-evolution-lab.md),
  [GDR-0030](0030-no-economy.md), [GDR-0054](0054-drydock-locked-parts-tools-saving-sharing.md),
  [GDR-0063](0063-salvage-and-research-v1.md), [GDR-0071](0071-shipyard-home-long-u.md),
  [GDR-0074](0074-test-range-waves-emergency-recall-cannon.md), [GDR-0085](0085-chain-reactions-v2.md),
  [GDR-0088](0088-pilot-assists-weapon-groups-and-key-bindings.md), [GDR-0091](0091-log-building-and-voyage-history.md),
  [ADR-0010](../adr/0010-cross-platform-determinism.md), [ADR-0031](../adr/0031-player-storage-and-design-sharing.md),
  [ADR-0036](../adr/0036-arena-crate.md), [ADR-0037](../adr/0037-simultaneous-fire-within-a-tick.md),
  [ADR-0038](../adr/0038-workspace-structure-feature-folders-and-client-split.md),
  [ADR-0040](../adr/0040-arena-library-in-core.md), `docs/arena.md`, `docs/plans/evolution-lab.md` (tracks L4 and
  L5), `docs/client.md` "Fight viewer", `docs/drydock.md`, `docs/career.md`

## Context

Tracks L1–L3 of `docs/plans/evolution-lab.md` are built (`docs/arena.md`). The `arena` crate (then in
`crates/tools/arena`) runs headless fights from a `FightSpec`. It scores designs by fights or by headless voyages, and it evolves designs
and behaviors through the editing models (`Drydock`, `BehaviorEditor`). Every mutant is therefore a legal placement.
A MAP-Elites archive keeps different designs apart. `Run::step(ticks)` is steppable, and `Run::jobs`/`submit` lets
the CLI use every core. Results are identical on 1 and 16 threads, in slices and in one go, and fight hashes are
identical natively and in wasm.

The roadmap's next item is L4, the lab screen, dev-only (GDR-0083). Niklas widened it on 15 Oct 2026:

> it should be possible to re-watch fights, so determinism is really important there / also ship designs that
> result from this should be saveable / this whole thing should be as flexible as possible, and be designed like an
> end-user facing feature (building etc.) / in particular, it should be possible to select ships as starting points,
> so that I can use this to optimize my ships rather than creating completely new ones

His answers to the draft's questions (15 Oct 2026):

1. **The unlock**: a research node unlocks the building, and runs are free.
2. **Background**: runs compute at home and on every screen, and pause in flight.
3. **Trials**: "as many options as possible, including against self-improving enemies or so". Every kind of trial
   can be picked and combined with weights, and an enemy population co-evolves against yours.
4. **What changes by default**: everything: the design, the behavior's rules and their structure, and the pilot
   assists. Per-run checkboxes narrow it, and locks still apply.

What the code has, and what it lacks for this (read 15 Oct 2026, `dev` at `ad40b0c`; the pilot assists act on the
pilot's lock only since `4f46564` the same day, GDR-0088: no target line, no face target, an `auto_lock` assist and
LOCK TARGET):

- **A fight is fully determined by its spec and the rules.** The spec holds the seed, both sides' designs (content
  id or blueprint text), their behaviors (id or text), counts, formation, distance and time limit. The rules are the
  content and the sim code. The arena adds only the spawn jitter, drawn on the sim's `Scenario` stream. Owners 1
  and 2 get unlimited mind capacity and `SimConfig::default()`. Nobody pilots, so pilot assists and weapon groups
  play no part. `Sim` is `Clone`.
- **What a run lacks:**
  - One start only (`Start::Design` or `Random`).
  - No locks.
  - One fitness kind per run (`ArenaPool` or `Voyages`) and no player weights.
  - No lineage: an `Elite` has no parent.
  - Per-job results are dropped when a generation closes, so no fight can be found again.
  - No snapshot or resume: the `Pcg32` state is private, and `Run` cannot be serialised.
  - Breeding is not budgeted: 4–10 ms per generation, natively.
  - `Job` and `VoyageSpec` are not serde.
- **The pre-filter's gaps:**
  - It checks the rule slots against the faction's default behavior, not against the genome's own
    (`docs/plans/evolution-lab.md`, "Open after L3").
  - It does not check logic cores against the career's mind capacity. A design with too many cores fails
    `voyage_ready` (`TooManyCores`).
  - It does not protect cargo: one run traded the starter's cargo bays for shields (`docs/arena.md`).
- **`Drydock::open` re-centres the parts' box in the frame** on every open. A design that grows on one side shifts
  all its cells, so locks kept as frame cells would drift.
- **Behaviors are compiled into a `BehaviorLibrary` in content order.** In flights the player's behaviors are added
  in id order (`PlayerBehaviors::add_to`). A fight that depends on the player's storage would change when they save
  or delete a behavior.
- **The client draws only `SimWorld`.** Its content may carry the tuning panel's weapon scales. It draws a ship
  teal or coral by its design's faction (`Faction::glow`), and the HUD's markers and lock colours are relative to
  the player's focus (`Owner(0)`). An arena fight's owners are 1 and 2, so a fight of two automatons would be all
  coral and a mirror of the starter all teal. (The draft of this record said the client drew `Owner(0)` teal and
  every other owner coral; building step 1 found the faction rule.)
- **Test-range waves live in the client** (`world::scene::wave_mix` and the spawning around it), so the arena
  cannot use them.
- **`arena` lives in `crates/tools/`.** ADR-0038 groups the engine-free libraries under `core/`, and the client
  should not depend on a tool.
- **Web:** no workers, single-threaded wasm. The web build is the development platform, not an end-user platform
  (CLAUDE.md, ADR-0006); desktop and Android are. wasm costs about 1.5× native (1.2–1.9×, `docs/results/spike-summary.md`).
- **Speed, measured natively** (`docs/arena.md`):
  - About 5–6 µs per tick on one thread, so about 100–190 fights per second per thread.
  - On 16 threads: 1200–2300 fights per second, or 340–390 generations of 32 children (384 fights each) per minute.
  - A generation is 0.4–0.75 M ticks.
  - Building a fight costs 0.2 ms.
- **Naming:** the research building is already called "the lab" in `docs/client.md` and in the action map
  ("Opens the research lab").

## Options considered

- **Where it lives**:
  - **a building on the station with its own screen, like the LOG** (chosen; Niklas: "building etc.");
  - a page of the test range;
  - a ⋯ entry in the drydock only.

  The drydock gets a shortcut to it all the same (EVOLVE THIS DESIGN).
- **Purpose**:
  - **evolve-to-improve, from your own designs** (chosen; Niklas). A run starts from a ship you have and keeps what
    you lock.
  - GDR-0083's benchmark rating for players. It becomes one trial among several (a fixed ladder), not the feature.
- **How a fight is re-watched**:
  - **re-simulated from its spec in the flight view, checked against the hash stored with the fight** (chosen):
    tiny to store, exact, and it shows whether the rules have changed since;
  - recorded frames or snapshots: large, and they cannot show a rules change.
- **What a run stores so that any fight can be found again**:
  - **every child as a small change against its parent, plus each job's result** (chosen): any fight of any
    generation can be re-watched;
  - only the archive's elites: small, but most fights are lost.
- **Running on the web**:
  - **time-sliced on the main thread, with a budget per frame** (chosen for the dev platform);
  - web workers: a second wasm binary and a heavier build. Kept as an optional later phase.
- **Running natively**: **the client's background threads through `jobs`/`submit`** (chosen); time slices only.
- **The arena crate**:
  - **its library moves to `crates/core/arena`, the CLI stays a thin binary in `crates/tools/`** (chosen);
  - the client depends on `crates/tools/arena`;
  - a copy of the logic in the client (ruled out by Niklas's "move rather than duplicate").
- **Trials**:
  - **one extensible trial abstraction**, where most kinds only build fight specs and presets are content (chosen;
    new kinds are cheap);
  - a fixed enum of fitness kinds in the arena, as today.
- **Self-improving enemies**:
  - **co-evolution: a second, enemy population in the same run, each scored against the other's frozen elites and
    hall of fame** (chosen);
  - only the run's own hall of fame (built; self-play, but the enemies are your own designs);
  - alternating runs, player then enemy (slower, and you can't watch the arms race in one place).
- **Scoring pilot assists**: assists only act on a piloted ship, and nobody pilots in a lab fight.
  - **a stand-in pilot from content flies part of the seeds** (chosen; it is needed because Niklas wants assists
    to evolve);
  - not scoring assists (they would drift at random);
  - evolving assists only in the test range by hand.
- **Lineage of a saved design**:
  - **an optional `origin` field on the blueprint** (chosen, lead default): it travels in files and share codes,
    and content ships can carry it too (GDR-0032's curated evolved enemies). It costs a new designs format and a new
    share format;
  - tags and description text only: no format change, but nothing a program can follow;
  - a side document in `lab/`: lost when the design is exported.

## Decision

### The building and the player flow

- **The building** (lead default: look, place, label):
  - The place: on the base's outer face above the mind, between the upper truss bridge and the top arm's corner.
    The mind's violet conduit joins them, since the lab is the mind's own simulation.
  - The look: a squat stack of compute rings with window bands.
  - Label `EVOLUTION`, never "lab" (the research building has that word). Key E (`open_evolution`, the action map,
    context `home`; E is free at home).
  - Placed by `shipyard::layout` like the LOG's mast, its hover rect tested apart from the others at every frame
    size.
- **Its states** (label line, windows):
  - `LOCKED · RESEARCH SIMULATION VAULT`: dark until its research node is researched (Niklas). The node is in the
    logic family and cheap (3 logic data, *placeholder*) and unlocks `Unlock::Facility("evolution")`. Runs cost
    nothing (GDR-0030).
  - `IDLE · NO RUN`: dim;
  - `RUNNING · GEN 12 / 50 · BEST 0.81`: the window bands flicker as fights run, and a ring of segments on the roof
    fills with the generation;
  - `PAUSED`;
  - `DONE · NEW BEST`: the roof glows gold, breathing like the LOG's, until the run is opened.

  The tooltip names the run and its best design. While a run is running, a small chip `EVOLVING 12/50` sits next to
  the gear ring on every layer over home; a click on it opens the lab.
- **The flow**:
  1. **Open** the building (click, E, or EVOLVE THIS DESIGN in the drydock's ⋯ menu, which preselects the working
     design). The camera flies to it and the EVOLUTION layer opens over home, like FLEET and the LOG.
  2. **RUNS**: a list of runs (name, starts, status, generation, best score, when started) with NEW RUN, OPEN,
     DUPLICATE (the same setup, a new seed), RENAME and DELETE (asks first). One run runs at a time; starting
     another pauses the current one (lead default).
  3. **SET UP** a run, in five short steps on one page (each with a summary line, so a run can start from the
     defaults with one click): START FROM, MAY CHANGE, LIMITS, TRIALS AND GOALS, SIZE. See "The run setup" below.
     START RUN.
  4. **WATCH PROGRESS**: the run's page, described below.
  5. **INSPECT** a design or a generation: its fights, its lineage, a comparison with its start.
  6. **RE-WATCH** any fight. The flight view opens as a spectator (the fight viewer, built in step 1); M, or the
     flight menu's way out, returns to the same place in the lab (Esc opens the flight menu, GDR-0090).
  7. **SAVE AS DESIGN**: a name, then it is in YOUR DESIGNS with an EVOLVED tag and "from <start> · gen 37". Or
     REPLACE <START>, which keeps the start's id so FLEET's escorts fly the new version (asks first).

### The run's page (lead default for the layout)

- **Top**:
  - the run's name, status and seed;
  - PAUSE / RESUME;
  - generation n of N, with a progress bar inside the generation;
  - fights per second and an estimate of the time left (measured, so it says what this device does);
  - `START 0.64 → BEST 0.91`.
- **Left: SCORES BY GENERATION.**
  - A chart of three lines: the archive's best, the generation's best and its mean. The bars underneath show the
    children that took a cell.
  - A click on a generation opens GENERATION n: its children, best first, each with score, cell, parent, what
    changed (`+2 PULSE LASER · −1 CARGO BAY · STEEL → COMPOSITE`), whether it took a cell, and its fights.
- **Middle: THE ARCHIVE.**
  - A grid over the run's two measures (labels and units from content), each occupied cell a ship card: the design
    drawn as FLEET and BLUEPRINTS draw it (`UiIcon::Ship`), tinted by score.
  - Hover shows the BLUEPRINTS preview card (CLASS, MASS, TOP SPEED, WEAPONS) plus SCORE, BUILD POINTS and
    GENERATION.
  - The best design has a gold corner, the start a teal one.
- **Right: the picked design.**
  - Its drawing, stats and score, broken down by goal and by trial.
  - FIGHTS: one row per fight (the opponent and its count, seed, `WON 0:23`, `LOST 1:02`, `DRAW 1:30 · 0.62`, build
    points lost on both sides, damage dealt), each with WATCH. WATCH BEST and WATCH WORST at the top.
  - LINEAGE: a strip from the start to this design, each ancestor a small card with its generation, score and what
    changed. A click picks that ancestor.
  - COMPARE: the start and this design side by side, added parts outlined teal, removed ones as coral ghosts, moved
    ones with a short arrow.
  - Buttons: SAVE AS DESIGN, OPEN IN DRYDOCK (as an unsaved copy; the lab keeps running), VALIDATE (below), COPY
    FIGHT CODE.
- **At three UI sizes** (CLAUDE.md): the grid scales its cards, and the right column scrolls.

### The run setup

All of it is one `arena::RunConfig`, extended (see "The arena crate"). The game and the CLI read and write the same
RON, so a run made in the game runs in `arena evolve`, and a CLI run imports into the game.

- **START FROM** (one or more, at most 4, lead default):
  - the drydock's working design;
  - YOUR DESIGNS;
  - TEMPLATES: the content ships the career can build (player faction, unlocked parts);
  - CHAMPIONS: the best designs of earlier runs;
  - RANDOM.

  Generation 0 is every start plus mutants of each, shared out evenly. All starts must fit one frame. The run's
  frame is the largest of theirs among the unlocked frames, and a start opens centred in it once. From then on
  genomes keep frame coordinates (below). Each genome remembers which start it came from (its root). Crossover
  between roots is allowed, but a child must still keep its own root's locks.
- **MAY CHANGE** (checkboxes, from content). **Everything is on by default** (Niklas); a run narrows it here. The
  operators are listed in "The mutation operators" below.

  | Gene | What the operators may do | Default |
  |---|---|---|
  | LAYOUT | move, rotate, flip parts | on |
  | PARTS by category: HULL, POWER, WEAPONS, DEFENSE, HEAT, PROPULSION, LOGIC, CARGO (from `categories.ron`) | add, remove, swap parts of that category. LOGIC stays within the mind capacity and CARGO within the hold limit (LIMITS) | on |
  | ARMOUR | change a hull part's material | on |
  | POWER PRIORITIES | set a part's power tier and weight (`Placement::power`, GDR-0014); a new operator | on |
  | BEHAVIOR NUMBERS | nudge thresholds and parameters, change comparators and choices | on |
  | BEHAVIOR RULES | the program's structure: add, remove, duplicate and reorder rules; add, remove, negate and replace conditions; replace actions and the target line | on |
  | ASSISTS | the pilot assist list: the same operators over the assist vocabulary, plus each rule's `start` (on or off); an assist list has no target line (GDR-0088) | on |

  - **One-click presets** (pills over the checkboxes): EVERYTHING (the default), DESIGN ONLY, BEHAVIOR ONLY,
    ASSISTS ONLY, NUMBERS ONLY (layout kept; behavior and assist numbers only).
  - **Weapon groups** are not a gene. They only decide which key fires a gun, and the stand-in pilot fires all
    groups. A swapped gun keeps its group, and a new gun takes its mirror twin's group, else the nearest gun's,
    else 1. Toggle keys of assists likewise (see the operators).
  - **Stages**: the arena's `Stages` stay available under MORE (e.g. behavior 10 generations, then everything).
- **LOCKS** (a small grid of the start in the setup; mirrored when MIRROR is on):
  - Click a part to lock it (a gold outline with a padlock tick). A locked part never moves, turns, changes material
    or power, or goes.
  - Paint empty cells KEEP EMPTY (hatched), e.g. a clear line of fire.
  - Lock a whole category: KEEP ALL WEAPONS keeps every gun and its position, and is the same as locking each one.
  - Lock behavior rules and assist rules, in their lists (each rule a padlock): a locked rule is neither changed,
    moved nor deleted, and neither is a behavior's target line when it is locked (assist lists have none). Rules
    may still be added around a locked
    rule, and the rules above it may change, which can change when it gets to decide. LOCK ORDER on a list keeps the
    order of its locked rules too.
  - The locks are per start. They are checked by the pre-filter, not only avoided by the operators, so crossover
    cannot break them either.
- **LIMITS** (each shown with its value, each switchable):
  - **FRAME**: the run's frame. Locked frames are not offered.
  - **UNLOCKED PARTS ONLY**: always on in a career. `Career::unlocks` fills `limits.parts` and
    `limits.materials`, and `Career::available` fills a new `limits.counts`, so the operators respect part counts
    once fabrication fills them (`Drydock::set_available` refuses `NoneLeft` today already). `?unlock=all` lifts
    it, as in the drydock.
  - **BUILD POINTS ≤ n**: content has no budget, so this is a goal tool. Lead default: on, at the start's build
    points ("same cost, better ship"), so a result stays comparable to what you had. TEST-RANGE WAVES in the
    default trials also size their enemies from the design, so the cap is not the only check against bloat.
  - **VOYAGE-READY** (on by default; lead default):
    - logic cores at most the mind capacity minus the cores of FLEET's escorts (a new `limits.max_cores`);
    - a cargo hold at least the start's (`Content::ship_hold`; a new `limits.min_hold`).

    So every result can launch and still bring salvage home.
  - **MIRROR**: on when the start is symmetric. The symmetry test is `Kit::symmetric`, as today.
- **TRIALS**: what a design is scored against. Any number, each with a weight. See "Trials" below for the kinds,
  the abstraction and the default selection, and "Co-evolution" for the enemy population.
- **GOALS** (player weights, sliders 0–3; the labels are content; the preset pills are WIN, WIN UNHURT, WIN
  CHEAPLY, WIN FAST):

  | Goal | Per job, 0..1 | Default weight |
  |---|---|---|
  | WIN | the arena score (a win 1, a loss 0, otherwise by build points; `arena_score`), or the voyage score | 1 |
  | UNHURT | 1 − the share of its own build points lost | 0 |
  | QUICK | 1 − ticks ÷ the time limit on a win, 0 otherwise | 0 |
  | CHEAP | 1 − build points ÷ the run's build-point limit (else ÷ the start's) | 0 |

  Fitness is Σ weight × the goal's mean over the jobs, divided by Σ weight. With WIN alone it is today's score
  exactly, so the CLI's runs stay as they are. The goals also break ties at 1.000: fitness saturates there in
  today's runs (`docs/arena.md`, chain reactions v2).
- **SPREAD BY** (the archive's axes): two measures from content, with labels, units and default ranges
  (`lab.ron`). The defaults are acceleration × firepower for design runs and engage distance × rules for
  behavior-only runs, as the arena's defaults are today.
- **SIZE** (presets in content, all *placeholders*): QUICK (16 children, 20 generations, 2 seeds), STANDARD (32, 50,
  4), DEEP (64, 100, 6), or custom. The setup shows the estimated time on this device from the measured rate.
- **SEED**: shown and editable, with a dice button. The page says `SAME SEED, SAME SETUP, SAME RULES: THE SAME RUN`.
- **VALIDATE** (on the run's page, and offered at SAVE): the best 8 designs (or the picked one) fight the run's
  trials on new seeds (n+1 … n+20 by default, *placeholder*) and get a VALIDATED score next to their run score. Runs
  fit their fixed seeds; this shows by how much.

### The mutation operators

Every operator edits through an editing model, so every child is legal by construction:
- designs through `data::design::edit::Drydock`, opened with `open_placed` (below);
- behaviors through `BehaviorEditor::from_text` (list kind `Behavior`);
- assists through `BehaviorEditor::from_text_as(.., ListKind::Assists)`.

The pre-filter then checks the whole genome again, so a bug in an operator shows up as a refusal, not as an illegal
design. The weights are *placeholders* in `lab.ron`. One mutation picks a gene among those switched on (by their
weights), then an operator of that gene. Locked parts, KEEP EMPTY cells and locked rules are never picked.

- **Design** (the arena's operators as built, plus one):

  | Operator | Weight | How |
  |---|---|---|
  | add a part | 30 | an allowed part of a category switched on, next to a random part, replacing nothing; slopes oriented to the hull |
  | remove a part | 15 | and its twin under MIRROR |
  | move | 15 | −2..2 cells each way |
  | rotate / flip | 10 / 5 | |
  | swap a part | 15 | another allowed part of the same footprint and shape |
  | material | 10 | another allowed armour on a hull part |
  | power priority | 5 | new: a powered part's tier one up or down, or its weight × 0.6–1.4, within `MAX_POWER_WEIGHT`; the twin follows |

- **Behavior** (the arena's operators, two new; `BehaviorEditor`):

  | Operator | Weight | How |
  |---|---|---|
  | nudge a number | 25 | a threshold or parameter × 0.6–1.4, rounded; one time in ten its comparator too |
  | change a choice | 5 | another option of a choice parameter |
  | move a rule | 8 | to a random place (the order decides: first match) |
  | swap two rules | 4 | new |
  | duplicate a rule | 4 | new: a copy below it, then one more mutation on the copy |
  | replace an action | 8 | another action from the vocabulary |
  | replace the target line | 5 | another target selector |
  | add a rule | 8 | a random action, with a random condition four times in five |
  | remove a rule | 7 | |
  | add / remove / negate / replace a condition | 7 / 5 / 4 / 4 | |

- **Assists** (new; the same editor in the assist list kind, so only the assist vocabulary: nodes marked
  `pilot: true` and every condition; there is no target line to replace, the assists act on the pilot's lock):

  | Operator | Weight | How |
  |---|---|---|
  | nudge a number | 25 | e.g. `idle(thrust) >= 1s`, `speed > 60%` |
  | change a choice | 10 | `auto_lock`'s pick, `hold_station`'s point or ally, `auto_fire`'s group (only groups that have a gun) |
  | move a rule | 10 | the order decides which rule wins a channel |
  | add a rule | 12 | an assist action with a random condition four times in five; a new rule's toggle is `none` |
  | remove a rule | 8 | |
  | add / remove / negate / replace a condition | 8 / 6 / 4 / 4 | the pilot conditions (`lock_held`, `idle(..)`) and the behavior conditions |
  | switch `start` | 8 | on ↔ off: whether it acts at all (nobody presses a toggle in the lab) |

  **Toggle keys** (`toggle1`–`toggle4`) are not scored: no stand-in presses them. Rules keep theirs, and the
  pre-filter keeps the list valid (at most `MAX_ASSIST_TOGGLES` slots). The player sets the keys after saving.
- **Crossover** (a share of the children, as today): the design splices rows; the behavior and the assist list each
  splice "first rules of `a`, then `b`'s from a random rule on". Locked rules come from `a`, the child's own root.
- **What every child must pass** (the pre-filter, `Kit::assess`, extended):
  - **Rule slots**: the behavior has at most `Content::ship_rule_slots` of the child's own design, counted from the
    logic cores on its main piece. A design mutation that removes slots is repaired by dropping the lowest unlocked
    rules until it fits (lead default); if the locked rules alone don't fit, the child is refused. Assists need no
    slots, but at most `MAX_RULES`.
  - **Mind capacity**: logic cores at most `limits.max_cores` (VOYAGE-READY: the career's mind capacity minus
    FLEET's escorts' cores).
  - **Cargo**: a hold at least `limits.min_hold`.
  - **The rest as today**: limits, symmetry, the locks, `data::validate_ship` with the genome's own behavior known
    (not the faction's), and the design report (flies, one piece, a gun).
  - **Valid text**: the behavior passes `check_program` and the assists `check_assists`, and each prints and reads
    back to the same canonical text.

### Trials

**One abstraction.** A trial turns a contestant (a genome, plus its escorts) into jobs and a job's outcome into
goal scores. In the arena:

```rust
/// One kind of trial. Implementations hold no state; everything they need is in the spec and the context.
pub trait TrialKind {
    /// Checks the spec against the content at `Run::new` and inlines what it needs (texts, ids), so the run is
    /// self-contained.
    fn resolve(&self, spec: &TrialSpec, ctx: &ResolveCtx) -> Result<Resolved, RunError>;
    /// The jobs of one contestant in one generation, in a fixed order. `ctx` has the generation's frozen opponent
    /// sets (hall of fame, the enemy population) and the seeds.
    fn jobs(&self, r: &Resolved, me: &Contestant, ctx: &GenCtx) -> Vec<Job>;
    /// The goal scores of one finished job, 0..1 each.
    fn goals(&self, r: &Resolved, job: &Job, out: &JobOutcome) -> GoalScores;
}
```

- **Three kinds of job**, each re-watchable:
  - `Fight(FightSpec)`;
  - `Fight` with **reinforcements**: a gauntlet is a `FightSpec` with a wave rule for side 1, whose referee spawns
    the next wave when side 1 is out. Fight codes and the viewer cover gauntlets unchanged;
  - `Voyage(VoyageSpec)`.

  Every job also carries a **pilot**: `Behavior` (nobody flies; the ship flies its behavior) or `StandIn(profile)`.
- **Cheap to extend**: a new trial kind that reduces to fights (most do) is one small `TrialKind` impl with no new
  job type. A new **preset** of an existing kind is content only. `lab.ron` lists them:

  ```ron
  (id: "enemies_you_meet", name: "The enemies you meet", summary: "...",
   kind: Encounters(from: OpenVoyages, count: Largest), weight: 2, seeds: 4, time_limit: 60),
  ```

  The setup's picker shows the presets. The run file stores the resolved `TrialSpec`s, so a run doesn't change
  when `lab.ron` does.
- **Shared settings of every trial**: weight, seeds, time limit, start distance, copies of the contestant, WITH YOUR
  ESCORTS (side 0 is the design and FLEET's escorts, as a voyage launches them), and FLOWN BY (BEHAVIOR, STAND-IN or
  BOTH).
- **The kinds**:

  | Trial | What it is | Status |
  |---|---|---|
  | THE ENEMIES YOU MEET | every encounter of the voyages open to you (`Career::open_voyages` → voyages → `encounters`), its ships at their largest count, one fight each; the guardian once its voyage is open. The ids are resolved into the run | new |
  | ENEMY SHIPS | chosen content ships (tag `enemy`) and counts: today's `ArenaPool` | built |
  | TEST-RANGE WAVES | a gauntlet: waves as the test range sends them, sized from the design's build points (GDR-0074, GDR-0089), one after the other until the design is out or time is up. WIN = waves cleared ÷ a target count (`lab.ron`, *placeholder* 5); a fair fight for any size of design | new; the wave rule moves into shared code (below) |
  | VOYAGES | your open voyages flown headless (`sim::voyage::runner`) | built |
  | YOUR FLEET | your saved designs as opponents (FLEET's escorts preselected), inlined as text | new |
  | BEAT YOUR START | every start, under its own behavior and assists, as the opponent: "is it better than what I had" | new |
  | HALL OF FAME | the run's own best `n` so far | built (`hall_of_fame`) |
  | CHAMPIONS | the best designs of earlier runs, inlined | new |
  | CO-EVOLUTION | opponents drawn from a co-evolving enemy population (next section) | new |
  | THE LADDER | GDR-0083's benchmark: a fixed ladder from `lab.ron`, also shown as a rating on designs | later |

- **The default selection** (lead default; *placeholder* weights and seeds):

  | Trial | Weight | Seeds | Why |
  |---|---|---|---|
  | THE ENEMIES YOU MEET | 2 | 4 | what voyages actually throw at you; it grows with the career; fights are cheap |
  | TEST-RANGE WAVES | 1 | 2 | sized to the design, so it rewards strength per build point and checks bloat; it is also the place where the player tries ships |
  | BEAT YOUR START | 1 | 2 | the question the player asks; cheap |
  | HALL OF FAME (4) | 1 | 2 | a bar that keeps rising, so scores don't stall at 1.000 |

  - **Off by default**:
    - VOYAGES: about 5× a fight per job. It is offered as VALIDATE's last check instead.
    - YOUR FLEET: your escorts are allies, not the enemy.
    - CHAMPIONS: none exist at first.
    - CO-EVOLUTION: it doubles a run's cost and can drift from the real enemies. It is one click away, as the
      ARMS RACE preset.
  - **The cost**: with three encounters open (early career), a child has about 24 jobs, 2× today's 12. A STANDARD
    generation then takes about 0.3–0.8 s on 16 threads, and about 8–28 s on the web with the layer open
    (estimates).
- **The stand-in pilot** (needed to score assists, since assists act only on a piloted ship): `sim::flight::stand_in`.
  - It generalises the balance study's passive and closing stand-ins, which today live in the voyage runner. It is
    deterministic, runs at the think rate and produces `PilotInput`. The assists then fill the channels it leaves
    idle, exactly as for the player (GDR-0088).
  - **Profiles** in `lab.ron` (*placeholders*):
    - `casual` (the default) locks the nearest enemy (LOCK TARGET's order), holds the stick towards it while it is
      beyond 80 % of the longest gun's reach, lets go of the stick for 3 s every 6 s, and fires all groups 3 s on,
      2 s off;
    - `hands_off` steers only, so it leaves the lock and the triggers to the assists;
    - `busy` steers and fires all the time.
  - **FLOWN BY BOTH** (the default while ASSISTS is on): odd seeds fly the behavior, even seeds the stand-in, at no
    extra cost. Assists are then scored on half of the jobs, the behavior on the other half.
  - The viewer shows the stand-in's inputs in the HUD, so a replay says who flew. It is a stand-in, not the player:
    the risks say so.

### Co-evolution

The ARMS RACE: an enemy population evolves against yours in the same run, each improving against the other.

- **Two populations**:
  - **YOURS** as set up (starts, genes, limits, archive).
  - **THE ENEMY**, with its own starts, genes, archive, size and random stream (the run's stream with another
    sequence):
    - **Seeds**: an enemy faction from content (lead default: the faction of the content ships tagged `enemy`, so
      the automatons in the base pack). Its starts are those ships under their content behaviors (raider, hunter;
      the warden only if picked).
    - **Parts**: the parts the faction's content ships use (derived, no new content); `lab.ron` may widen a
      faction's palette.
    - **Frame**: the largest of its starts' frames.
    - **Genes**: design and behavior, no assists, since enemies are never piloted.
    - **Size**: half of yours (*placeholder*).
- **Fair pairing**: an enemy fights in a pack sized by the test range's wave rule, to the share of your side's build
  points. A bigger enemy design comes in fewer copies, so the enemy evolves strength per build point and needs no
  build-point cap of its own.
- **How they are paired**: generation g evaluates both populations against frozen sets from the end of g − 1:
  - your children fight the enemy's best 3 elites and 2 drawn from its hall of fame;
  - the enemy's children fight your best 3 elites, 2 drawn from your hall of fame, and your starts as an anchor
    (lead default), so the enemy stays able to beat the ships you actually have;
  - 2 seeds per pairing;
  - generation 0: the starts of each side.

  All jobs of a generation are independent, so they run in slices or on threads with the same results.
- **Against cycling**:
  - **The hall of fame**: each population's champion of every generation joins its hall of fame. Opponents are
    drawn from it uniformly over the whole history, not the newest, on the run's stream in a fixed order. The drawn
    genome numbers are stored per generation, so every job rebuilds.
  - **Your other trials still count**, so your designs don't specialise against an odd enemy.
  - **PAST CHAMPIONS check** (every 10 generations, *placeholder*): your current best fights the enemy's champions of
    every 5th generation so far, 2 seeds each, and the enemy's best does the same against yours. The chart plots
    "beats n of m past champions" for both. A rising line is an arms race; a saw is cycling. These fights are
    recorded and re-watchable too.
- **Inspecting the enemy**: the run's page gets a YOURS | ENEMY switch over the archive and the picked design. The
  enemy's archive, fights, lineage and comparison with its start work as yours do. The score chart shows both
  populations' bests and the PAST CHAMPIONS lines. In every fight your side is side 0 (teal) and the enemy side 1
  (coral). An enemy child's score is side 1's. So replays look the same from either population.
- **Saving evolved enemies**:
  - SAVE AS DESIGN works for them too: YOUR DESIGNS, faction kept, tags `evolved` and `enemy`, `origin` set. They
    can be picked as opponents in later runs (YOUR FLEET, CHAMPIONS), and the drydock opens them with their locked
    parts shown as usual (not voyage-ready).
  - EXPORT TO CONTENT gives the content ship format, its behavior as a content behavior file in a comment block (as
    the drydock's EXPORT does), id `evolved_<faction>_<n>`, tags `enemy` and `evolved`, and `origin`. It goes into a
    pack's `ships/` by hand: curated, as GDR-0032 decided ("evolved offline, curated"). Encounters and the test
    range then use it like any enemy. `arena evolve --out` writes the enemy's best as `enemy_NN.ron` the same way.
  - Evolved enemies never enter voyages by themselves (lead default; GDR-0032).

### Determinism and replays

- **A fight is re-watched by simulating it again from its spec**: the same seed, sides, behaviors, distance and time
  limit. Its stored record says how it ended and gives its final state hash.
- **A run is self-contained**:
  - Every player design and player behavior it uses (starts, YOUR FLEET, champions, escorts) is inlined as
    blueprint and behavior text.
  - Its fights use the packs' content only: never the player's behavior storage, and never the tuning panel's
    scales.

  So deleting or editing a saved design or behavior later changes nothing in the run.
- **What a run stores** (in `lab/<run-id>`, see "Persistence"):
  - **The config** (the run file) and its **inputs** (the inlined texts).
  - **Every genome bred**, as a change against its parent: the parent's genome number, a crossover partner's, the
    removed part indices, the added placements, and the new behavior text if it changed. A few hundred bytes each;
    any genome is rebuilt by replaying the changes from its root.
  - **Per generation**: the stats (as today's `GenerationStats`) and the frozen opponent sets: the genome numbers
    drawn from each population's elites and hall of fame. These, with the config and the resolved trials, rebuild
    every job's spec (`TrialKind::jobs`).
  - **Both populations' genomes** under co-evolution, in one genome table.
  - **Per child**: its score, measures and cell, and per job a record: outcome, ticks, the score per goal, build
    points lost on each side, damage dealt, the final state hash, the pilot, and a bit mask of the behavior's and
    the assists' rules that decided at least once (for TIDY). About 35 bytes per job.
  - **A fingerprint of the rules** when the run started (below).

  So any fight of any child of any generation is found by (generation, child, job) and re-watched.
- **The replay viewer reuses the flight view** (a new `SceneKind::Fight`, built like TEST IN RANGE's watch mode;
  `world::replay`, `docs/client.md` "Fight viewer"):
  - **Who is who**: `world::scene` builds the world through the arena, not by its own spawning. `Fight` is split
    into `FightSetup::build(content, spec)` (the `Sim`, its `Referee`, and each entry's blueprint and compiled
    design) and `Referee::observe(&Sim) -> Option<FightResult>`, called after each step. `run_fight` and `Fight` keep
    working on top of them. `SimWorld` adopts that `Sim` and the blueprints of its compiled designs for drawing. The
    world uses the packs' content only (no tuning scales, no player behaviors); nobody pilots: whatever the controls
    or the fleet bar queue for a tick is dropped, and the tuning panel leaves the replay alone. Side 0 is drawn in
    the player's colours (teal) and side 1 as enemies (coral) whatever their designs' faction: a viewer mapping
    from owner to look (`SimWorld::looks`), in place of the faction rule (see the context). Off-screen markers show
    every ship of a framed fight in its side's colour.
  - **The camera** frames both sides, as the load test does, keeping the HUD's top and bottom lines clear. A click
    on a ship follows it (zooming in to flight's default zoom if framing had zoomed out), and the HUD then shows its
    hull, speed and what its behavior targets, as in watch mode; a click on empty space frames again; F frees the
    camera. The behavior's live panel (`BehaviorWatch`) for the followed ship comes with the lab screen (step 4).
  - **The HUD**:
    - the title, `REPLAY · RIM SKIFF COPY (EVOLVED) VS 3 × AUTOMATON RAIDER · SEED 3`;
    - the subtitle: the time `0:23 / 1:30`, the speed and each side's build points left of its start
      (`BUILD POINTS 120 / 132 VS 12 / 47`);
    - a timeline under the view: how far the fight is (teal), how far it has been simulated, and a mark for every
      vessel lost (teal for side 0's, coral for side 1's); marks for lost parts and chain reactions come with the
      lab screen (lead default);
    - the help line with the replay keys;
    - when a stand-in flies (step 6): `FLOWN BY STAND-IN · CASUAL`, its inputs, and the assists' channels as the
      flight HUD shows them.
  - **Playback**: new actions in a `replay` context and section of the action map (`controls.ron`; nobody pilots a
    replay, so they share keys with flying): PAUSE / PLAY (Space, K; at the end it plays again from the start),
    SLOWER and FASTER (`,` and `.`: 0.25×, 0.5×, 1×, 2×, 4×, 8×, through the virtual clock's speed), BACK 1 S and
    FORWARD 1 S (← → or J L), FROM THE START (R, Home). The flight menu (Esc) pauses it too.
  - **Seeking**: the first pass copies the `Sim` and the `Referee` every 10 s (both are `Clone`). Going back restores
    the keyframe before the target and runs on to it; going forward runs ahead. Either happens within one frame (a
    step is 1 s, 30 ticks; a whole 90 s fight is about 15 ms natively, 2700 ticks at 5–6 µs), and every piece is
    drawn afresh after it. Seeking to a point that was never seen is the same as going forward.
  - **Checked at the end** (the world then holds still: no more ticks, the clock paused):
    - `AS RECORDED` (teal) with how it ended and the final hash, when the result (outcome, ticks, both sides'
      numbers, final state hash) equals the record;
    - otherwise `THE RULES HAVE CHANGED SINCE THIS FIGHT` (coral) and `NOW LOST AT 0:41 (RECORDED: WON AT 0:23)`.
    - A code whose rules fingerprint differs from today's says `THE RULES HAVE CHANGED SINCE THIS FIGHT` for its
      first 6 seconds too (`IT PLAYS UNDER TODAY'S RULES · THE END SAYS WHETHER IT STILL ENDS AS RECORDED`); a code
      that doesn't read or build says `THIS FIGHT CODE DOESN'T PLAY` and why, over an empty world.
  - **Fight codes** (`arena::FightCode`): `SGF1:` + base64url (no padding, so it goes into a URL as it is) of the
    zlib-compressed RON of the spec, the packs (id, version), the rules fingerprint and the record (as share codes
    are made, ADR-0031). The spec holds the seed, both sides' designs (content ids or blueprint texts), behaviors
    (content ids or program texts), counts, formations, the start distance and the time limit; the rest of a fight's
    setup is fixed by the arena (owners 1 and 2, every core running, `SimConfig::default()`, the packs' content,
    nobody piloting). About 430 characters for the Runner against three raiders, 1–2 kB with an evolved blueprint
    and its behavior. Decoding refuses damaged, newer, oversized (256 kB packed, 4 MB unpacked) and foreign text with
    a reason. `?scene=fight&fight=<code>` opens it (the open point of `docs/plans/evolution-lab.md`). The CLI prints
    one per fight of `arena fight` (after its table) and, after `arena evolve`, one per generation: its best child in
    the fight it scored lowest in. COPY FIGHT CODE in the lab gives the same. A fight code replays outside any run,
    so Niklas can paste one from an `arena` report straight into the browser.
- **When content or the sim changes**:
  - **The fingerprint** (`arena::rules_fingerprint`, built in step 1) is FNV-1a 64 (`data::hash`) over two things:
    - the loaded packs' content in canonical form (`Content::fingerprint`: the RON of everything a fight or voyage
      reads, in load order; pack versions, research, starts, controls, icons and wiki prose left out, as they
      change no fight);
    - the final hashes of the rules probes: two fixed fights of 300 ticks at close range (`crates/core/arena/probes/`:
      guns, hits, plumes, shields, chain reactions), 4.7 ms natively, run once when a viewer (later the lab) opens.
      They live with the arena's code, not in content: they probe the code, and a pack must not be able to change
      what they probe.

    The probes catch most changes to the sim's code without a version number someone has to remember to raise. The
    hash check at the end of every replay catches the rest.
  - **A run whose fingerprint differs** shows `THE RULES HAVE CHANGED SINCE THIS RUN`. Its replays still play under
    the current rules and say whether they match. It can be RE-SCORED: every current elite is evaluated again
    (about one generation's work), the archive is rebuilt from the new scores, and the run may go on. Or it is kept
    as it is, read-only.
  - **A genome that no longer builds** (a part the content lost) is marked `NO LONGER BUILDS` and left out.
  - **Saved designs are not affected**: they are ordinary designs.
- **New tests** (each in `arena`'s tests unless noted):
  - snapshot and resume give the same results as an uninterrupted run (6 generations straight against 3, a
    snapshot, a restore, and 3 more);
  - rebuilding a genome from its changes gives the identical genome;
  - a rebuilt job gives the stored record (all fields and the hash);
  - a cloned `Sim` continued gives the same hashes as the original (`sim`'s tests);
  - fight codes round-trip, and damaged or foreign codes are refused (built, `tests/codes.rs`);
  - a stale fingerprint is detected: a changed part changes it, and the fight no longer ends as recorded (built);
  - the setup stepped with the referee gives `run_fight`'s result, and a cloned world and referee go on to the same
    end (built);
  - the replay scene's world, stepped and seeked through its keyframes, ends as the CLI recorded the fight (client
    tests, headless; built, `world::replay`);
  - the rules probes are pinned like the other determinism streams (`docs/sim.md`; `fight_hashes` runs them);
  - `scripts/determinism-check.sh --fight` keeps covering native against wasm, and takes a fight code
    (`--fight SGF1:…`: the code's own fight to its end, with `# record=as recorded`) (built).

### Running it: native, web, background, persistence

- **The executor** (`world::evolution`, one Bevy plugin). One `arena::Run` is held on the main thread, and only it
  breeds and closes generations. Its jobs are run by one of two executors. Both give identical results (tested
  today for the step and jobs paths).
  - **Native (desktop, later Android)**: `Run::jobs()` → Bevy's `AsyncComputeTaskPool` (one task per chunk of jobs;
    cores − 1 threads at home, *placeholder*; on Android two, for heat) → `Run::submit` in job order. On 16 threads
    a STANDARD generation (32 children × 12 fights) takes about 0.15–0.4 s. On a 4-core laptop it is about 1–2 s,
    an estimate to be measured.
  - **Web (the dev platform)**: `Run::step(budget)` on the main thread, with a time budget per frame (lead
    defaults, *placeholders*):

    | Where | Budget |
    |---|---|
    | the EVOLUTION layer open | 10 ms |
    | anywhere else at home and on the screens | 4 ms |
    | in flight | 0: the run pauses (Niklas) |

    The budget is converted to ticks from a running mean of the measured cost per tick, so frames hold their time
    whatever the fight. The estimate below assumes 5–6 µs per tick natively, wasm at 1.2–1.9× that (6–11 µs), and
    60 fps:

    | Budget | Throughput | A STANDARD generation (0.4–0.75 M ticks) | A 50-generation run |
    |---|---|---|---|
    | 10 ms (the layer open) | about 55–100 k ticks/s | 4–14 s | 3–12 minutes |
    | 4 ms | about 20–40 k ticks/s | 10–35 s | |

    These are estimates; the run's page shows the measured rate.
  - **Breeding is budgeted too**: `Run::step` breeds child by child, and each child's breeding counts against the
    budget (a new `breed_cost` in ticks). The 4–10 ms per generation spike (natively; more in wasm) then no longer
    lands in one frame.
  - **Web workers** (optional, later): a small `lab-worker` binary (`arena`, `data`, `sim`, wasm-bindgen, no Bevy)
    in `navigator.hardwareConcurrency − 1` workers, loaded by Trunk's worker support. Jobs and results go as RON
    messages (`Job` and `JobResult` become serde). It needs no shared memory, so no COOP/COEP headers, and it works
    over plain `http://10.77.9.2`. The results are identical through `jobs`/`submit`. Its cost is a second wasm
    build: wasm-bindgen needs about 3.4 GB, so run the two builds one after the other on the VM.
- **Background running** (Niklas): a run computes at home and on every screen (the drydock, BEHAVIOR, research,
  FLEET, the map, the LOG, SETTINGS), with the budgets above. It pauses in flight: the test range, test flights,
  replays and voyages. It resumes when flight ends. The `Run` lives in a resource, not in the layer, so a screen
  change never loses work. Natively the threads stop taking new jobs when flight starts, and the jobs underway finish
  (each is under about 50 ms) before the first flight frame needs the cores.
- **Pause and resume**: PAUSE on the run's page and on the chip. A run also pauses when another starts, while a
  flight is on (above) and when the page is hidden (the web's `visibilitychange`; lead default).
- **Persistence**:
  - Each run is a document `lab/<run-id>` (ADR-0031's storage layer, a new namespace, format 1), written after every
    generation. That point is the run's only checkpoint: the snapshot is the state between two generations (the
    `Pcg32` state, the archive, the history, the genome table, the records). A reload resumes exactly, re-breeding
    the open generation from the same random state. At most one generation's work is lost.
  - Document shape: a readable RON head (name, created, status, fingerprint, the `RunConfig`, the stats per
    generation, the best design's genome number) and one field `payload: "SGL1:…"` holding the genome table and the
    records as zlib-compressed, base64url RON (`miniz_oxide` and `base64`, already dependencies).
  - Size estimate (to be measured): a STANDARD run (1600 children) is about 1–2 MB of RON, a few hundred kB
    compressed.
  - The web's local storage holds about 5 MB per origin, so on the web the lab keeps at most 8 runs (*placeholder*)
    and warns before the quota. Runs over 4000 children keep job records only for archive elites and generation
    champions; their other children keep their score. Desktop has no such limit (files).
  - **The CLI shares the format**: `arena evolve --save <file>` and `--resume <file>`. An IMPORT RUN in the RUNS
    list reads one, so a deep run on 16 cores at the CLI can be opened and re-watched in the game.

### Saving a result

- **SAVE AS DESIGN** opens a small panel:
  - **Name**: lead default `<start name> · evolved g<generation>`, editable.
  - **Its behavior**: when the behavior evolved, it is saved as a player behavior (`behaviors/<id>`, named
    `<behavior name> (evolved)`) and the design names it. Otherwise the design keeps naming the behavior the start
    named.
  - **Its assists and weapon groups**: the start's, as carried.
  - **The design** goes to `designs/<id>` in the run's frame, as the drydock saves designs (frame coordinates), and
    is checked with `behaviors::validate_design`, like SAVE.
  - **REPLACE <START>** (only for a start that is one of YOUR DESIGNS): writes over the start under its id, asking
    first, so FLEET keeps it.
- **Lineage**: the blueprint gets an optional `origin` field (lead default), written only when it is set:

  ```ron
  origin: (
      run: "skiff_vs_raiders",
      from: ["rim_skiff_copy"],   // the start's design id, or content id, and a crossover partner's
      generation: 37,
      score: 0.91,
      validated: Some(0.86),
  ),
  ```

  - **Formats**: designs become format 2, and share codes `SGD3:`. Older formats are still read. An older build
    refuses a newer document with its usual message (ADR-0031).
  - **Tags and text**: `tags` gets `evolved`, and `summary` is "Evolved from <start> in the evolution lab."
- **Where results show up**:
  - **YOUR DESIGNS (BLUEPRINTS)**: an `EVOLVED` tag on the row and `FROM RIM SKIFF COPY · G37` under the name. The
    preview card gets an ORIGIN line. A click on the origin opens the run at that design, if the run still exists.
  - **The drydock's header**: the same line, while the design is unchanged since it was saved.
  - **START FROM**: champions of earlier runs are offered there.
  - **The wiki**: a content ship with an `origin` gets an "Evolved from" line on its page.

### The arena crate, the CLI and the shared code

- **The library moves to `crates/core/arena`.** It is engine-free and deterministic, like `data` and `sim`, and
  builds for wasm32. The binary moves to `crates/tools/arena-cli`, still a binary named `arena`, so the commands
  stay as they are. This takes an ADR amending ADR-0036 and ADR-0038 ([ADR-0040](../adr/0040-arena-library-in-core.md),
  built in step 1).
- **The CLI keeps only arguments, its thread pool, tables and file writing.** What it does beyond printing moves into
  the library, so the game and the CLI share it:
  - elite naming and the blueprint and behavior texts with their comment header (`Genome::save_texts`, used by
    `--out` and by SAVE AS DESIGN);
  - fight codes and the rules' fingerprint (built in step 1);
  - a generation's best child and its jobs (`Run::champion`, built in step 1; the CLI codes its hardest fight);
  - snapshots.
- **What changes in the library** (each with tests; `docs/arena.md` updated):
  - **RunConfig**:
    - `starts: [...]` (the old `start` is still read);
    - `trials: [...]` as `TrialSpec`s with weights (the old `fitness` is still read, as one trial), behind the
      `TrialKind` abstraction ("Trials");
    - `enemy: Option<Population>` for co-evolution, with its own starts, genes, limits, archive and size;
    - `goals`, `genes` (assists included), `locks` (per start: parts, cells, rules, LOCK ORDER);
    - limits `counts`, `max_cores`, `min_hold`;
    - stand-in pilot profiles by id, and a `validate` count of seeds.
  - **Genomes**:
    - kept in frame coordinates, through a new `Drydock::open_placed` that keeps positions as given, so locks are
      frame cells that never drift;
    - numbered in breeding order;
    - each records its parents and its changes;
    - `Elite` and the per-child records keep them.
  - **The pre-filter**:
    - validates the rule slots against the genome's own behavior;
    - enforces the locks, `max_cores` and `min_hold`.
  - **The operators** ("The mutation operators") skip locked parts, KEEP EMPTY cells, locked rules and genes that
    are off. New: power priority, swap and duplicate rules, and the whole assist set (through
    `BehaviorEditor::from_text_as(.., ListKind::Assists)`, which exists).
  - **The pre-filter** also repairs the rule slots by dropping the lowest unlocked rules, and checks the assists
    (`check_assists`, toggle slots) and the canonical round trip.
  - **Running and storing**:
    - per-job records are kept;
    - breeding is budgeted;
    - snapshot and restore, with `Pcg32` state accessors in `sim::rng`;
    - `Job`, `JobResult`, `VoyageSpec` and `FightResult` are serde (built in step 1).
  - **`Fight`** is split into setup and referee for the client (built in step 1). The referee takes the wave rule as
    reinforcements, and a job's pilot (`StandIn`) is applied as `PilotInput` each tick.
- **Moved down, out of the client**:
  - the test range's wave rule (`wave_mix`) and its wave spawning move from `world::scene` into `sim::voyage`
    (beside the load-test scenarios), so the test range and the lab's gauntlet share one rule;
  - the balance study's stand-ins, which today live in `sim::voyage::runner`, become `sim::flight::stand_in`
    profiles that the voyage runner and the arena share;
  - `Content::fingerprint` goes into `data` (built in step 1).
- **The client**:
  - `world::replay`: the fight viewer (built in step 1): the replay scene's world building, playback, seeking and
    the check against the record; the lab's WATCH opens it;
  - `world::evolution`: the executor, persistence, the run resource and the fingerprint;
  - `game::evolution`: the EVOLUTION layer;
  - `game::shipyard`: the building.
- **Content and the wiki**: everything the lab shows by name or number lives in data. A new content kind `lab.ron`
  (`docs/content-model.md`; validated; loaded like `controls.ron`) holds:
  - the goals: id, name, summary;
  - the trial presets (id, name, summary, kind and its data, default weight and seeds) and the kinds' names;
  - the stand-in pilot profiles (*placeholders*);
  - the mutation operators' weights (*placeholders*);
  - per enemy faction, an optional palette for co-evolution;
  - the archive measures: id, name, summary, unit, decimals, default range;
  - the size presets (population, generations, seeds; *placeholders*);
  - the default time budgets (*placeholders*);
  - THE LADDER (later).

  The rules probes are not content: they live with the arena's code (`crates/core/arena/probes/`, see "When
  content or the sim changes").

  `content-tool wiki-data` generates its page. A hand-written `wiki/mechanics/evolution-lab.md` explains the
  building, runs, locks, goals and replays, and references the data by id (`[[lab:goal.unhurt]]`,
  `[[lab:preset.standard.population]]`); `cargo test -p content-tool` checks it. The research node (Simulation vault) and its new
  `Unlock::Facility("evolution")` are content too.
  - **Facilities**: lead default `facilities.ron` with `evolution`, and later the fabricator and the salvage bay.
    These are the station's buildings as content, as GDR-0071 asks ("each area needs states from content").

## Consequences

### Phased build plan

Each step ships something usable. The complications found in the code are steps of their own (marked ⚑). Until
step 6 the default MAY CHANGE is what the arena can already do (design and behavior); EVERYTHING becomes the
default once the assist operators exist.

1. **The arena library moves, and fights can be watched** (built 2026-10-15):
   - the library to `crates/core/arena`, the CLI to `crates/tools/arena-cli`, and the ADR (ADR-0040);
   - serde on `FightSpec`, `FightResult`, `Job`, `JobResult`, `VoyageSpec`;
   - `Fight` split into `FightSetup` and `Referee`;
   - fight codes (`SGF1:`), and with them `Content::fingerprint` and the rules probes (`arena::rules_fingerprint`),
     taken over from step 2 so a code says at once whether it was fought under today's rules;
   - ⚑ **side looks**: a viewer mapping from owner to look in `world` (`SimWorld::looks`), so side 0 shows teal
     and side 1 coral whatever their faction;
   - the replay scene `?scene=fight&fight=<code>`, using pack content only and no tuning scales: the spectator
     camera, speeds, seeking by `Sim` keyframes, AS RECORDED or changed rules at the end, the `replay` actions;
   - the CLI prints a fight code for every fight of `arena fight` and for each generation's best child (its
     hardest fight) of `arena evolve`, after the tables;
   - `scripts/determinism-check.sh --fight SGF1:…` replays a code native against wasm.

   *Usable*: Niklas pastes any CLI fight into the browser and watches it.
2. **Every fight of a run is kept** (arena and CLI):
   - ⚑ **lineage and per-fight results**: genome numbers, parents and changes (the genome table), per-job records
     with the final hash and the rules that decided;
   - snapshot and resume (`Pcg32` state accessors);
   - ⚑ **budgeted breeding**: child by child, counted against the budget;
   - the run's fingerprint (`rules_fingerprint`, built in step 1) stored with the run;
   - in the CLI: `arena evolve --save/--resume`, and `arena replay <run> <gen> <child> <job>`, which prints the
     fight code.

   *Usable*: CLI runs resume, and any fight of any generation can be watched.
3. **Results that can launch** (arena; run files):
   - ⚑ **`Drydock::open_placed`**: genomes keep frame coordinates;
   - locks (parts, cells, categories, rules, LOCK ORDER) in the run file;
   - ⚑ **the pre-filter**: rule slots from the genome's own design and behavior, with the repair; logic cores within
     `max_cores` (the mind capacity); the hold kept (`min_hold`); part counts.

   *Usable*: CLI runs that only produce voyage-ready designs and keep what is locked.
4. **The evolution lab v1 in the game** (behind `?lab=1` until step 8):
   - the building and its states;
   - the EVOLUTION layer: RUNS, and a setup with one start (the working design, a saved design or a template), the
     ENEMY SHIPS trial, size presets, the career's limits and MIRROR;
   - both executors: threads natively, budgeted slices on the web;
   - computing at home and on every screen, paused in flight;
   - persistence in `lab/`, and the stale banner when the fingerprint differs;
   - the run's page: progress, the archive grid, the picked design's fights, WATCH;
   - SAVE AS DESIGN with its behavior and `origin` (designs format 2).

   *Usable*: evolve the working design in the game and keep the result.
5. **Trials**:
   - the `TrialKind` abstraction and the presets in `lab.ron`;
   - ⚑ **the test-range waves move to shared code**: the wave rule and spawning from `world::scene` into
     `sim::voyage`. The test range then uses them from there, with the same waves, checked by its tests;
   - gauntlets as reinforcements in the referee and the viewer;
   - the trials ENEMIES YOU MEET, TEST-RANGE WAVES, BEAT YOUR START, YOUR FLEET, HALL OF FAME, CHAMPIONS and
     VOYAGES; WITH YOUR ESCORTS;
   - weights, the four goals and the default selection;
   - VALIDATE, RE-SCORE, IMPORT RUN.

   *Usable*: ships optimised against what the career throws at them.
6. **Everything mutates**:
   - the behavior structure operators and the assist operators;
   - `sim::flight::stand_in` (from the runner's stand-ins), its profiles, FLOWN BY BOTH;
   - MAY CHANGE with EVERYTHING as the default, and its presets;
   - several starts;
   - the LOCKS UI;
   - COMPARE, the LINEAGE strip, GENERATION n;
   - EVOLVE THIS DESIGN in the drydock, REPLACE <START>, the EVOLVED tag in YOUR DESIGNS.

   *Usable*: Niklas's default run: everything, from his own ship, with his locks.
7. **Co-evolution**:
   - the enemy population, with its pairing, hall-of-fame draws and pack sizing;
   - the PAST CHAMPIONS check and its chart;
   - the YOURS | ENEMY switch;
   - the ARMS RACE preset;
   - saving and EXPORT TO CONTENT for enemies, and `enemy_NN.ron` in the CLI.

   *Usable*: an arms race to watch, and evolved enemies to curate.
8. **The career and the wiki**:
   - the Simulation vault node, `Unlock::Facility`, `facilities.ron`; `?lab=1` goes;
   - the chip on every layer;
   - `lab.ron` complete, its generated page and `wiki/mechanics/evolution-lab.md`;
   - `docs/client.md` "Evolution", `docs/arena.md`, `docs/career.md`, `docs/content-model.md`, the roadmap, the
     concept notes, and GDR-0083's status.
9. **Optional**: web workers; re-watching voyage jobs in the voyage view; THE LADDER as a rating on designs.

### Risks

- **Designs tuned to fixed seeds and opponents.** Mitigation: VALIDATE on new seeds, several trials, the goals as
  tie-breakers.
- **Fitness saturates at 1.000.** Many designs then score perfectly, and the best is not the strongest
  (`docs/arena.md`, chain reactions v2). Mitigation: the hall of fame and the waves in the default trials; weighted
  goals; co-evolution.
- **The stand-in is not the player.** Assists are tuned to a scripted pilot, and the behavior to an unpiloted ship.
  Mitigation:
  - the profiles are content, so they can be tuned;
  - FLOWN BY BOTH;
  - the test range, to try the result;
  - the replay shows who flew.
- **EVERYTHING drifts furthest from what you built**, and an evolved program can grow rules that never run (as the
  CLI's runs found). Mitigation: locks, COMPARE, the NUMBERS ONLY preset, and TIDY at SAVE (lead default): it drops
  rules that never decided in any of the design's recorded fights, using the "rules that decided" in each record.
- **Co-evolution cycles or drifts** from the enemies the career really has. Mitigation: hall-of-fame draws over the
  whole history, your starts as the enemy's anchor, your other trials, and the PAST CHAMPIONS chart. It also costs
  about 1.5–2× a run, which is why it is off by default.
- **Legal but odd designs** (reserve guns, guns behind guns): emergent play, accepted in GDR-0085. Locks and KEEP
  EMPTY rule them out for a player who doesn't want them.
- **Storage on the web**: about 5 MB per origin, shared with designs. Co-evolution doubles the genome table.
  Mitigation: compression, a run cap, records thinned for big runs, a warning before the quota.
- **Cost of the snapshot**: writing a large run after every generation may take a few ms on the web. If measured
  higher, write every k generations (*placeholder*).
- **A cloned `Sim` might not continue bit for bit** (Rapier's state). This is tested first. If it fails, seeking
  simulates from tick 0: natively 15 ms for 90 s, in wasm about 25 ms.
- **Content and sim changes make runs stale.** Mitigation: the fingerprint and probes, the hash check on every
  replay, RE-SCORE.
- **Hidden inputs leak into fights** (player behaviors, tuning scales, `SimConfig`). Mitigation: runs are
  self-contained (texts inlined, packs only, `SimConfig::default()`), and a test builds a run's fights with
  different player storages and compares the hashes.
- **Moving the waves** could change the test range. Its waves must stay identical; a test of the wave sequence for
  fixed seeds, before and after, holds it.
- **The second wasm build for workers** strains the VM's memory. It is optional, and the builds run one after the
  other.
- **The station gets crowded.** The building's place and label are checked at the four frame sizes and three UI
  sizes, like the LOG.
- **Balance**: evolved player ships may make the slice's voyages easy. The lab also makes such dominance visible
  early, which is GDR-0032's purpose: it is reported, not prevented.

### What stays open

- None of the draft's questions; all four are answered above.
- After building:
  - the measured rates on the web and natively (the budget table and the default trials' seeds may move);
  - the size of real run documents;
  - the stand-in profiles' numbers;
  - the default trial weights after the first runs;
  - the building's place, once Niklas sees it;
  - whether the archive's default axes say enough to a player.
  - how far framing zooms out for a fight that spreads wide (the camera's floor of 3 px per metre leaves ships of a
    wide fight off screen; the side-coloured markers show where they are).
- Done on acceptance (15 Oct 2026): GDR-0083's status says "superseded in part by GDR-0092", and
  `docs/plans/evolution-lab.md` L4/L5 point here.
