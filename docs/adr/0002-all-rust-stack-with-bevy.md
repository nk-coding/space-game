# ADR-0002: All-Rust stack with a Bevy client; no C++, Godot, Unity, Unreal or JS engines

- Status: Accepted
- Date: 2026-10-10
- Decided by: Niklas
- Related: [ADR-0003](0003-crate-architecture.md), [ADR-0014](0014-physics-engine-by-benchmark.md), [ADR-0019](0019-rhai-scripting.md), `docs/concept-notes.md` "Background", "Technical decisions", "Tech research"

## Context

v1 of the game had a ship builder, working peer-to-peer multiplayer and a scripting language. For v2 Niklas
stated his preferences: Rust first; he dislikes C++ (acceptable only for physics if absolutely necessary); avoid
Godot; Flutter acceptable for (parts of) the UI; Bevy "sounds reasonable". The brief asks for Android, Windows and
Linux, efficient native code and 2D physics, built by a solo hobby developer with AI help.

## Options considered

- **Godot** (4.7.x at the time): ruled out by Niklas. No reason recorded beyond "avoid Godot".
- **C++ engines, Unity, Unreal**: ruled out; C++ is disliked and they are not Rust.
- **JavaScript game engines**: ruled out; the web build comes from the same Rust code (WebAssembly).
- **Flutter for the UI** (via flutter_rust_bridge 2.12): dropped from the plan; it stays a fallback for menus
  only if Bevy's UI fails badly.
- **Other Rust engines**: not recorded.
- **Bevy client, Rust everywhere** (chosen).

## Decision

- All game code is Rust. No C++ anywhere. C (Box2D v3.1) is allowed only as a reference number in the physics
  benchmark, never in the game.
- Bevy is the client on every platform; the web build is the same Rust code compiled to WebAssembly.
- Engine-level crates are pinned to exact versions (`bevy = "=0.20.0"` style): Bevy 0.20 came out on
  8 Oct 2026 and Bevy breaks APIs every 4–5 months.
- Node is fine for development tooling (the headless screenshot helper, the benchmark's wasm runner), never for
  game code. (Rule in `CLAUDE.md`; when and by whom it was set is not recorded.)

## Consequences

- The physics engine must be pure Rust in the game (ADR-0014), and scripting must embed in Rust (ADR-0019).
- Bevy upgrades are deliberate work, not routine dependency bumps.
- UI is built with Bevy (bevy_egui for dev tools such as the tuning panel; it was also noted for node-graph
  prototyping).
