# ADR-0003: Crate architecture: engine-free `sim` and `data`, Bevy `game`, `bench` as its own workspace

- Status: Accepted
- Date: 2026-10-10 (crate plan); 2026-10-11 (`bench` workspace, tool crates)
- Decided by: Niklas (crate plan, concept phase); lead session (`bench` as a separate workspace, tool crates)
- Related: [ADR-0002](0002-all-rust-stack-with-bevy.md), [ADR-0010](0010-cross-platform-determinism.md), ADR-0013, ADR-0014, ADR-0017, ADR-0023, GDR-0032, `CLAUDE.md` "Crates"

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
- `bench`: the physics benchmark, a **separate cargo workspace** (own `[workspace]`, `Cargo.lock` and
  `target/`), excluded from the main one, so Box2D C, Avian's Bevy version and engine features never enter the
  game's dependency graph.
- Tools: `content-tool` (validate, export JSON) and `atlas-tool` (its committed PNG output conflicts with
  ADR-0021).
- Planned: `net` (ADR-0023), `arena` (headless fights, evolution, balance reports, determinism checks;
  GDR-0032), a wiki generator on `data`.
- Every game crate builds for wasm32 as well as Android and desktop; content comes in as bytes or strings.

## Consequences

- Visuals live only in the client: `sim` knows nothing about colours or icons.
- The client uses the sim's backend-independent API; the physics engine plugs in behind a trait (ADR-0013).
- If Avian won the benchmark it would conflict with this split; the spike plan says to discuss that with Niklas
  before building on it.
