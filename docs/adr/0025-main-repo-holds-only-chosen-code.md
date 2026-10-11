# ADR-0025: The main repo holds only chosen code; experiments and benchmarks live in separate local repos

- Status: Accepted
- Date: 2026-10-11
- Decided by: Niklas
- Supersedes: [ADR-0013](0013-physics-backend-trait.md)
- Related: [ADR-0003](0003-crate-architecture.md), [ADR-0008](0008-local-git-only.md), [ADR-0009](0009-spike-first-systems-before-content.md), [ADR-0014](0014-physics-engine-by-benchmark.md), `docs/results/physics-benchmark.md`

## Context

The physics benchmark (`crates/bench`: harness, adapters for four engines, wasm runners, a browser page, raw
results) lived inside `space-game` as its own cargo workspace. The sim had a generic `PhysicsBackend` trait and a
`FreeFlight` stand-in (ADR-0013) because the engine wasn't chosen yet. With Rapier2D chosen (ADR-0014), all of
that is either an experiment or a seam for options that lost.

## Options considered

- **Keep experiments and the generic seam in the main repo**: everything in one place, but code that the game
  doesn't use stays around and has to keep building.
- **Only chosen code in the main repo; experiments elsewhere** (chosen).

## Decision

- `space-game` contains only code the game has chosen to use. Experiments and benchmarks live in separate local
  repos (local git only, ADR-0008).
- The physics benchmark harness moves out of `crates/bench` into its own local repo, `space-game-physics-bench`,
  next to `space-game`. Only the report (`docs/results/physics-benchmark.md`) stays in `space-game`.
- No dead code: the `FreeFlight` stand-in and the generic `PhysicsBackend` trait are removed once Rapier is
  integrated.

## Consequences

- Future spikes and benchmarks start in their own repo; their reports come back to `docs/results/`.
- Links from the report to `crates/bench` have to point to the other repo.
- After the Rapier integration the sim talks to Rapier directly; switching engines later would mean real work,
  which is the accepted price.
