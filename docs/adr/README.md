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
| [0003](0003-crate-architecture.md) | Crate architecture: engine-free `sim` and `data`, a Bevy `game` client, the physics benchmark kept outside | Accepted | 2026-10-10; 2026-10-11 |
| [0004](0004-target-platforms-and-performance-first.md) | Targets are mid-range Android and desktop; performance comes first | Accepted | 2026-10-10 |
| [0005](0005-web-as-full-end-user-platform.md) | The web build is a full end-user platform | Superseded by [ADR-0006](0006-web-as-development-platform.md) | 2026-10-11 |
| [0006](0006-web-as-development-platform.md) | The web build is the development platform (web first), not an end-user platform | Accepted | 2026-10-11 |
| [0007](0007-development-setup-vm-and-host-browser.md) | Develop in a GPU-less VM and view the game as a web build in the host browser | Accepted | 2026-10-11 |
| [0008](0008-local-git-only.md) | Local git only, no remote repositories | Accepted | 2026-10-10 |
| [0009](0009-spike-first-systems-before-content.md) | Spike first, keep the spike as the foundation, build systems before content | Accepted | 2026-10-10; 2026-10-11 |
| [0010](0010-cross-platform-determinism.md) | The simulation is bit-exact deterministic across platforms | Accepted | 2026-10-10; 2026-10-11 |
| [0011](0011-30hz-sim-60fps-interpolation.md) | 30 Hz fixed simulation, 60 fps rendering with interpolation | Accepted | 2026-10-10 |
| [0012](0012-coordinate-frames-and-units.md) | Coordinate frames and units (1 block = 1 m) | Accepted (provisional, lead) | 2026-10-11; 2026-10-10 |
| [0013](0013-physics-backend-trait.md) | Physics sits behind a `PhysicsBackend` trait, with `FreeFlight` as the stand-in | Superseded by [ADR-0025](0025-main-repo-holds-only-chosen-code.md) | 2026-10-11 |
| [0014](0014-physics-engine-by-benchmark.md) | Physics engine: Rapier2D with one compound collider per ship piece, chosen by benchmark | Accepted | 2026-10-11; 2026-10-10 |
| [0015](0015-ron-content-packs-and-mods.md) | Content lives in RON packs; the base game loads like a mod; mods from day one | Accepted | 2026-10-10 |
| [0016](0016-wiki-ready-content-model.md) | Content is wiki-ready: stable ids, keyed text, stat descriptors, prose next to the data | Accepted | 2026-10-11; 2026-10-15 |
| [0017](0017-embedded-content-in-memory-loader.md) | The base content is embedded at compile time behind a pure in-memory loader | Accepted (provisional, lead) | 2026-10-11 |
| [0018](0018-english-only-keyed-text.md) | English only at launch; every string is keyed in data | Accepted | 2026-10-10 |
| [0019](0019-rhai-scripting.md) | Rhai is the scripting language | Accepted | 2026-10-10 |
| [0020](0020-style-2a-renderer.md) | Style 2a renderer: one atlas mesh per ship piece, glow sprites, sRGB compositing on an HDR camera | Accepted (provisional, lead) | 2026-10-11 |
| [0021](0021-no-fixed-raster-graphics.md) | No fixed raster graphics: vector art, rasterised at runtime at display resolution | Accepted | 2026-10-11 |
| [0022](0022-one-simulation-space-per-cluster.md) | One simulation space per cluster | Accepted | 2026-10-10 |
| [0023](0023-host-authoritative-netcode.md) | Co-op netcode is host-authoritative, with join codes and a tiny relay | Accepted | 2026-10-10 |
| [0024](0024-saves-seed-and-deltas.md) | Saves store seed, generator version and deltas, versioned with migrations | Accepted; refined by [ADR-0035](0035-saves-keep-the-galaxy-skeleton.md) | 2026-10-10 |
| [0025](0025-main-repo-holds-only-chosen-code.md) | The main repo holds only chosen code; experiments and benchmarks live in separate local repos | Accepted | 2026-10-11 |
| [0026](0026-behavior-programs-text-in-content.md) | Behavior programs are text lines in RON, parsed in `data`, compiled once and evaluated from one snapshot without allocation | Accepted | 2026-10-11; 2026-10-12 |
| [0027](0027-rhai-sandbox-and-determinism.md) | Rhai runs as a raw, sandboxed engine with f32 floats, no `**`, libm math and an operation budget | Accepted | 2026-10-11; 2026-10-12 |
| [0028](0028-icons-in-the-content-pack.md) | Icons live in the content pack as SVG text, loaded and validated by `data` | Accepted (provisional, lead) | 2026-10-12 |
| [0029](0029-shield-bubbles-batched-procedural-mesh.md) | Shield bubbles are one batched world-space mesh with a procedural WESL material and max blending | Accepted (provisional, lead) | 2026-10-12 |
| [0030](0030-drydock-architecture.md) | Drydock architecture: a pure editing model in `data`, a static design report in `sim`, player designs as content blueprints | Accepted (provisional, lead) | 2026-10-12 |
| [0031](0031-player-storage-and-design-sharing.md) | A player storage layer of named, versioned RON documents; designs shared as files and share codes | Accepted (provisional, lead) | 2026-10-13 |
| [0032](0032-wiki-generator.md) | Wiki generator: `content-tool wiki` writes a static HTML site straight from `data` | Superseded by [ADR-0039](0039-vitepress-wiki-from-game-data.md) | 2026-10-13 |
| [0033](0033-career-model-save-and-voyage-director.md) | The career: progression rules in `data::career`, salvage and the voyage director in `sim`, one career save | Accepted (provisional, lead) | 2026-10-13 |
| [0034](0034-fast-everyday-web-builds-and-shared-compile-cache.md) | Everyday web builds without LTO and wasm-opt (`--full` for performance); sccache shared by every cargo build on the VM | Accepted | 2026-10-14 |
| [0035](0035-saves-keep-the-galaxy-skeleton.md) | Saves keep the galaxy skeleton (clusters, gates, trails) next to the seed; interiors are still generated | Accepted | 2026-10-14 |
| [0036](0036-arena-crate.md) | An engine-free `arena` crate for headless fights, evolution and benchmarks, used by the game (time-sliced) and a native CLI | Accepted; amended by [ADR-0040](0040-arena-library-in-core.md) | 2026-10-14 |
| [0037](0037-simultaneous-fire-within-a-tick.md) | Ship ids decide nothing in a fight: simultaneous fire within a tick, shared think ticks, auto orbits by both ids | Accepted (provisional, lead) | 2026-10-14 |
| [0038](0038-workspace-structure-feature-folders-and-client-split.md) | Workspace structure: crates grouped in `core/`, `client/`, `tools/`; the client split into `kit`, `world`, `game`; feature folders inside every crate | Accepted; amended by [ADR-0040](0040-arena-library-in-core.md) | 2026-10-14 |
| [0039](0039-vitepress-wiki-from-game-data.md) | The wiki is a VitePress site in the game's look; its content pages and every number come from the game's data | Proposed | 2026-10-15 |
| [0040](0040-arena-library-in-core.md) | The `arena` library moves to `crates/core/arena` (the client replays its fights); its CLI is the package `arena-cli` in `crates/tools/` | Accepted | 2026-10-15 |
<!-- index:end -->

## How to add, supersede and deprecate

- **Add:** copy [`0000-template.md`](0000-template.md) to the next free number (`NNNN-short-slug.md`), keep it readable in under two minutes, add its row above and link it from `docs/concept-notes.md`. Agents write `Proposed`; Niklas's decisions are `Accepted`; lead-session defaults are `Accepted (provisional, lead)`.
- **Supersede:** never rewrite a decision. Write a new record with a `Supersedes: ADR-NNNN` line, then change only the old record's status to `Superseded by ADR-MMMM` (and its row here).
- **Deprecate:** when a decision stops applying and nothing replaces it, set its status to `Deprecated` and add one dated line saying why.

## Open

Technical questions without a decision yet (no record until decided):

- **Glow method**: glow sprites vs full-screen bloom, measured in the browser load test ([ADR-0020](0020-style-2a-renderer.md)).
- **Co-op transport on native** now that the browser is not an end-user platform ([ADR-0023](0023-host-authoritative-netcode.md), [ADR-0006](0006-web-as-development-platform.md)).
- **Phone measurements** (spike questions 2, 4–7 on the phone): deferred until Android work starts ([ADR-0006](0006-web-as-development-platform.md), [ADR-0009](0009-spike-first-systems-before-content.md)).
