# Plan: the evolution lab

Status: **plan** (14 Oct 2026, lead; L1–L3 built the same day), following [GDR-0083](../gdr/0083-evolution-lab.md) and
[ADR-0036](../adr/0036-arena-crate.md). It narrows `docs/plans/coop-arena.md` sections 3.7, 3.8 and 5.7 (the arena
half) to what the lab needs first. Numbers marked *(placeholder)* are for feel.

## Tracks

| Track | Goal | Depends on | Size |
|---|---|---|---|
| L1 Fights | `crates/tools/arena`: `FightSpec` (seed, two sides of entries: a design as content id or blueprint text, a behavior as content id or text, count, formation; start distance; time limit), `run_fight` → `FightResult` (winner, time, build points lost per side, damage dealt), the arena score (win 1 / loss 0; at the time limit 0.5 + 0.5 × (share of the enemy's build points destroyed − share of own lost)); a native CLI `arena fight|matrix` on every core; a determinism test (native vs wasm hash of a fight) | — | M |
| L2 Voyage fitness | Score a design by the headless voyage runner (clear rate, time, damage taken over seeds), as a library call | L1 | S |
| L3 Evolution core | Run configuration (what evolves: behavior / design / both / a sequence of stages; fitness: arena pool or voyages; limits: frame, build-point budget, unlocked parts, mirror lock; start design; population, generations, seed); genomes; mutation and crossover through `data::edit` and `data::behavior_edit` (every mutant legal); a quick pre-filter with `sim::design_report`; the archive (a small grid of niches, e.g. top speed × weapon reach, so results differ); a pool of the best so far plus archetypes; `Run::step(budget)` that does a slice of work and can be called every frame; CLI `arena evolve` | L1, L2 | L |
| L4 Lab screen | *Widened by [GDR-0092](../gdr/0092-evolution-lab-for-your-own-ships.md) (15 Oct 2026): a player-facing EVOLUTION building in nine steps; step 1, the fight viewer, built.* Was: a dev-only station place (behind a dev switch); set up a run, watch it (best per generation, the archive as ship cards), replay a card's fight in the client, SAVE AS DESIGN to the blueprints | L3 | M |
| L5 Benchmark rating | *Now THE LADDER, one trial of [GDR-0092](../gdr/0092-evolution-lab-for-your-own-ships.md) (later).* Was: in the test range: a fixed ladder of benchmark opponents over seeds → a rating, win rates per opponent, what killed you (player-facing) | L1 | M |

Order: L1 first (it is useful alone: balance numbers from the CLI); then L2 and L3; L4 once L3 steps; L5 any time
after L1.

**L1 done** (14 Oct 2026): `crates/tools/arena` and the `arena fight|matrix` CLI, native vs wasm fight hashes identical,
about 1200 (Runner vs 3 raiders) to 2300 (skiff mirror) fights per second on 16 threads; see [`docs/arena.md`](../arena.md).
Found on the way: the side spawned first won a mirror match about 3 to 1 (lower ship ids); fixed in the sim on the
same day ([ADR-0037](../adr/0037-simultaneous-fire-within-a-tick.md): simultaneous fire, shared think ticks, auto
orbits by both ids), so mirrors end the same in either spawn order.

**L2 done** (14 Oct 2026): the voyage runner's pilot is a library (`sim::voyage_runner`, steppable; the example's
output unchanged) and `arena::voyage` scores a design and behavior over voyages and seeds (goal cleared and home:
0.6 + 0.2 × hull kept + 0.2 × time left; else 0.5 × share of sites cleared, halved when lost; placeholders).
**L3 done** (14 Oct 2026): `arena::evolve` (`RunConfig` as RON with `Behavior` / `Design` / `Both` / `Stages`, arena
pool or voyage fitness, optional frame, build-point, part, material and mirror limits, a design or random start;
mutation and crossover only through `data::edit` and `data::behavior_edit`; the design-report pre-filter; a
configurable MAP-Elites archive; `Run::step(ticks)` for a client, `Run::jobs` / `submit` for the CLI) and
`arena evolve` on every core (identical results on 1 and 16 threads); 210–390 generations of 32 per minute on 16
threads. See [`docs/arena.md`](../arena.md) "Voyage fitness" and "Evolution".

Open after L3 (for the lead):

- **The archive's axes**: the default is acceleration × firepower, not top speed × weapon reach (both are part-type
  ratings, so the starter's kit fills one or two cells); behavior runs use `EngageDistance` × `Rules`. A behavior
  stage under design axes is a plain hill climb.
- **The automaton parts** (the core: power 6, 2 rule slots, a 300 gyro for 5 build points; the cannon) win every 60-build-point design run
  that may use them; locked for players, but a balance question once research unlocks them.
- **Fixed seeds** (1..=n for every child): scores stay comparable, but a run can fit its seeds; a validation pass on
  other seeds for the final elites would show it.
- **The hall of fame** makes the arena fitness move; elites keep the score they got (no re-scoring yet).
- **Breeding is not budgeted** in `Run::step`: 4–10 ms per generation of 32 at once (native), so a lab screen sees
  one short spike per generation.
- The pre-filter refuses designs with fewer rule slots than their faction's default behavior has rules (content
  validation of a blueprint without a behavior of its own), even when the evolved behavior would fit.

## Placeholders

| What | Value |
|---|---|
| Fight time limit | 90 s |
| Seeds per pairing | 4 |
| Population | 64 |
| Archive | 8 × 8 niches |
| Pool | the best 8 so far plus the archetypes (`raider_basic`, `skirmisher` ships) |
| Generations | 100 |

## Open (for the lead, then Niklas when it shows)

- The archive's two axes (what makes designs "different").
- How a lab card's fight is replayed in the client (a fight code in the URL like `?scene=fight&fight=<code>`, or
  directly from the lab). *Decided 15 Oct 2026 ([GDR-0092](../gdr/0092-evolution-lab-for-your-own-ships.md)): both;
  the URL form is built.*
- How many fights per frame the browser can afford while the lab is open.
