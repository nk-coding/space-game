# ADR-0003: Crate architecture: engine-free `sim` and `data`, a Bevy `game` client, the physics benchmark kept outside

- Status: Accepted. 14 Oct 2026: the crates are grouped in folders and the client is three crates (`kit`, `world`,
  `game`), [ADR-0038](0038-workspace-structure-feature-folders-and-client-split.md); the engine-free split here stands.
- Date: 2026-10-10 (crate plan); 2026-10-11 (tool crates; benchmark moved out of the repo, ADR-0025)
- Decided by: Niklas (crate plan, concept phase; benchmark in its own repo); lead session (tool crates)
- Related: [ADR-0002](0002-all-rust-stack-with-bevy.md), [ADR-0010](0010-cross-platform-determinism.md), [ADR-0013](0013-physics-backend-trait.md), [ADR-0014](0014-physics-engine-by-benchmark.md), [ADR-0025](0025-main-repo-holds-only-chosen-code.md), [ADR-0017](0017-embedded-content-in-memory-loader.md), [ADR-0023](0023-host-authoritative-netcode.md), [GDR-0032](../gdr/0032-balancing-arena-evolution.md), `CLAUDE.md` "Crates"

## Context

The same simulation has to run headless (tests, determinism checks, the balance arena), inside the client on
every platform, and later on a co-op host. The concept fixed the crate split on 10 Oct 2026; development started
on 11 Oct.

## Options considered

- **Simulation inside the engine's ECS** (for example with Avian physics): less glue, but the core would depend
  on Bevy and its release cycle, and the arena and tools would pull Bevy in. The recorded concern is "Avian ties
  core to Bevy".
- **An engine-free simulation library with a thin client** (chosen).

## Decision

- `sim`: engine-free simulation (ships, parts, physics, heat, power, damage, later behavior + Rhai). Fixed 30 Hz,
  deterministic (ADR-0010). No Bevy, no graphics, no threads, no filesystem, no wall-clock.
- `data`: content schema, pack loader from in-memory files (ADR-0017), validation, shared blueprint geometry.
  No Bevy. Used by `sim`, `game` and tools (wiki).
- `game`: the Bevy client: rendering at 60 fps with interpolation, UI, input, audio.
- The physics benchmark lives in a **separate local repo**, `space-game-physics-bench`, next to `space-game`;
  only its report (`docs/results/physics-benchmark.md`) stays here (Niklas, 11 Oct 2026, ADR-0025). Until then
  it was a separate cargo workspace inside the repo (`crates/bench`, set up by the lead session), so Box2D C,
  Avian's Bevy version and engine features never entered the game's dependency graph.
- Tools: `content-tool` (validate, export JSON) and `atlas-tool` (its committed PNG output conflicts with
  ADR-0021).
- Planned: `net` (ADR-0023), `arena` (headless fights, evolution, balance reports, determinism checks;
  GDR-0032), a wiki generator on `data`.
- Every game crate builds for wasm32 as well as Android and desktop; content comes in as bytes or strings.

## Consequences

- Visuals live only in the client: `sim` knows nothing about colours or icons.
- Physics is Rapier2D inside `sim` (ADR-0014); the client never talks to the engine directly. Avian, which ties
  the core to Bevy's ECS, would have conflicted with this split.
- Only chosen code lives in this repo (ADR-0025).

14 Oct 2026: the crate layout (folders, the client in three crates) is refined by [ADR-0038](0038-workspace-structure-feature-folders-and-client-split.md); the engine-free split here stands.
