# GDR-0083: An evolution lab: configurable evolution of designs and behaviors, a dev-only place in the shipyard first, a benchmark rating for players later

- Status: Accepted. Superseded in part on 2026-10-15 by [GDR-0092](0092-evolution-lab-for-your-own-ships.md): the lab is
  player-facing (a station building unlocked by research), not a dev-only place, and its purpose is improving your
  own ships, with GDR-0083's benchmark rating one trial among several; its configurable runs, the engine-free crate
  and SAVE AS DESIGN stay.
  2026-10-16: GDR-0092 step 8 built: the lab is in the career (the research node Simulation vault opens it), the
  dev-only switch `?lab=1` is gone; the benchmark rating (THE LADDER) stays an optional later step of GDR-0092.
- Date: 2026-10-14
- Decided by: Niklas (eight questions, interactively)
- Related: [GDR-0032](0032-balancing-arena-evolution.md), [GDR-0030](0030-no-economy.md), [GDR-0033](0033-multiplayer-coop-and-arena.md), [ADR-0036](../adr/0036-arena-crate.md), `docs/plans/coop-arena.md`, `docs/plans/evolution-lab.md`

## Context

GDR-0032 decided a headless arena and evolutionary search over layouts and behaviors for balance and for
offline-evolved enemies, but nothing of it is built. Niklas (14 Oct 2026) wants it now as a dev feature, "just some
button in the shipyard", which may become player-facing, "in particular to evaluate their ships".

## Options considered

- **Where it runs**: an engine-free crate used by both the game and a native CLI (chosen); the game only (one
  browser thread); the CLI only.
- **What evolves first**: Niklas: "rather flexible and configurable": evolve the behavior only, the design only,
  both at once, or one after the other, chosen per run (instead of the recommended fixed order behaviors → designs →
  both).
- **Fitness**: chosen per run: fights against a pool (the best so far plus fixed archetypes) or the slice's
  encounters (the headless voyage runner's clear rate, time and damage taken).
- **Limits**: all configurable per run: frame size, build-point budget, unlocked parts only, mirror lock.
- **The button opens**: a lab screen (chosen); a run with a report; a test-range wave option.
- **Player-facing later**: a benchmark rating (chosen); evolve-to-improve; both.
- **The button lives**: in the shipyard, dev-only (chosen); always visible; in the test range.

## Decision

- **An `arena` crate** (ADR-0036) runs fights and evolution, deterministic and engine-free; the game runs it
  time-sliced (watchable, works in the browser), a native CLI runs it on every core. Same seed, same results.
- **A run is configured**: what evolves (behavior, design, both together, or a sequence of stages), the fitness
  (arena pool or voyage encounters), the limits (frame, build-point budget, unlocked parts only, mirror lock, each
  on or off), the starting point (a saved design or random), population, generations, seed.
- **The lab screen**: set up a run, watch it (best score per generation, the archive as a grid of ship cards), click
  a card to watch its fight replayed, SAVE AS DESIGN puts it into the blueprints.
- **For now a dev-only place** on the station (shown with a dev switch, hidden for players).
- **For players later**: a benchmark rating in the test range: your design against a fixed ladder of benchmark
  opponents over several seeds, giving a rating, win rates per opponent and what killed you; built on the same crate.

## Consequences

- GDR-0032's method (mutation and crossover through the drydock and behavior editing models, so every mutant is
  legal; a diverse archive; ratings) is the starting point; `docs/plans/evolution-lab.md` lays out the tracks.
- Fights need a replayable spec (seed plus sides) so a lab card can be watched in the client.
- Evolved designs are ordinary blueprints with ordinary behaviors (shareable, flyable).
