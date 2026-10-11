# Architecture Decision Records (ADR)

Technical decisions for the space game: crates, determinism, content format, rendering, physics, tooling,
platforms. Game design decisions are in [`docs/gdr`](../gdr/README.md). [`docs/concept-notes.md`](../concept-notes.md)
is the overview and links to both series. Format and status rules: `CLAUDE.md`, section "Decision records".

## Index

<!-- index:start -->
| ADR | Title | Status | Date |
|---|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Accepted | 2026-10-11 |
| [0002](0002-all-rust-stack-with-bevy.md) | All-Rust stack with a Bevy client; no C++, Godot, Unity, Unreal or JS engines | Accepted | 2026-10-10 |
| [0003](0003-crate-architecture.md) | Crate architecture: engine-free `sim` and `data`, Bevy `game`, `bench` as its own workspace | Accepted | 2026-10-10; 2026-10-11 |
| [0004](0004-target-platforms-and-performance-first.md) | Targets are mid-range Android and desktop; performance comes first | Accepted | 2026-10-10 |
| [0005](0005-web-as-full-end-user-platform.md) | The web build is a full end-user platform | Superseded by [ADR-0006](0006-web-as-development-platform.md) | 2026-10-11 |
| [0006](0006-web-as-development-platform.md) | The web build is the development platform (web first), not an end-user platform | Accepted | 2026-10-11 |
| [0007](0007-development-setup-vm-and-host-browser.md) | Develop in a GPU-less VM and view the game as a web build in the host browser | Accepted | 2026-10-11 |
| [0008](0008-local-git-only.md) | Local git only, no remote repositories | Accepted | 2026-10-10 |
| [0009](0009-spike-first-systems-before-content.md) | Spike first, keep the spike as the foundation, build systems before content | Accepted | 2026-10-10; 2026-10-11 |
| [0010](0010-cross-platform-determinism.md) | The simulation is bit-exact deterministic across platforms | Accepted | 2026-10-10; 2026-10-11 |
| [0011](0011-30hz-sim-60fps-interpolation.md) | 30 Hz fixed simulation, 60 fps rendering with interpolation | Accepted | 2026-10-10 |
| [0012](0012-coordinate-frames-and-units.md) | Coordinate frames and units (1 block = 1 m) | Accepted (provisional, lead) | 2026-10-11; 1 block = 1 m was decided 2026-10-10 |
<!-- index:end -->

## How to add, supersede and deprecate

- **Add:** copy [`0000-template.md`](0000-template.md) to the next free number (`NNNN-short-slug.md`), keep it readable in under two minutes, add its row above and link it from `docs/concept-notes.md`. Agents write `Proposed`; Niklas's decisions are `Accepted`; lead-session defaults are `Accepted (provisional, lead)`.
- **Supersede:** never rewrite a decision. Write a new record with a `Supersedes: ADR-NNNN` line, then change only the old record's status to `Superseded by ADR-MMMM` (and its row here).
- **Deprecate:** when a decision stops applying and nothing replaces it, set its status to `Deprecated` and add one dated line saying why.

## Open

Technical questions without a decision yet (no record until decided, except where a `Proposed` record exists):

- **Physics engine**: [ADR-0014](0014-physics-engine-by-benchmark.md) (`Proposed`). The benchmark harness is in `crates/bench`; Niklas picks from the numbers.
- **Glow method**: glow sprites vs full-screen bloom, measured in the browser load test ([ADR-0020](0020-style-2a-renderer.md)).
- **Co-op transport on native** now that the browser is not an end-user platform ([ADR-0023](0023-host-authoritative-netcode.md), [ADR-0006](0006-web-as-development-platform.md)).
- **Phone measurements** (spike questions 2, 4–7 on the phone): deferred until Android work starts ([ADR-0006](0006-web-as-development-platform.md), [ADR-0009](0009-spike-first-systems-before-content.md)).
