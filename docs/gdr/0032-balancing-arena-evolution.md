# GDR-0032: Balance with an AI-vs-AI arena and evolutionary search; enemies are handcrafted plus evolved

- Status: Accepted; the lab and its configurable runs specified by [GDR-0083](0083-evolution-lab.md) (2026-10-14)
- Date: concept phase (exact day not recorded; settled by 2026-10-11)
- Decided by: Niklas
- Related: [GDR-0015](0015-layered-behavior.md), [GDR-0024](0024-tech-trees-no-mk-levels.md), [GDR-0027](0027-races.md), [GDR-0030](0030-no-economy.md), [ADR-0003](../adr/0003-crate-architecture.md), [ADR-0008](../adr/0008-local-git-only.md), [ADR-0010](../adr/0010-cross-platform-determinism.md)

## Context

"Two hundred nodes cannot be balanced by hand." The rules are open enough that players will find unintended
designs; evolution "finds whatever the rules allow", which makes it the best tester. A toy evolution lab on the
concept board found nose and mid-ship engines, no radiators and long reactive slope wings on its first run.

## Options considered

- **Balancing by hand**: not feasible at this size.
- **Headless arena with evolutionary search** (chosen).

## Decision

- A headless arena runs the real simulation many times faster than real time on every CPU core, with fixed seeds,
  so any fight replays in the game client.
- Designs: hand-made archetypes plus evolutionary search over layouts and behaviors within a build-point budget.
  The genome is the part list (optionally mirror-locked) plus the behavior; mutation and crossover work on both.
- Fitness comes from real fights against a rotating pool (hall of fame and archetypes); a MAP-Elites archive keeps
  diversity; Elo or TrueSkill ratings; reports on pick rates, win rates and unused parts.
- An outer loop tunes part numbers to grow the viable niches; a human reviews its proposals. Exploit hunts
  (ramming, spinning, recoil, AI loopholes) become regression scenarios. A short run checks every data change.
- Enemy designs: handcrafted per race, plus designs evolved offline in each race's style and curated, plus runtime
  variation (not full generation).

## Consequences

- Requires determinism (ADR-0010), data-driven content and the planned `arena` crate (ADR-0003).
- With local git only, the "short run on every data change" is a local check (ADR-0008).
