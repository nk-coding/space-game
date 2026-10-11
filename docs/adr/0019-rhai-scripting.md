# ADR-0019: Rhai is the scripting language

- Status: Accepted
- Date: 2026-10-10
- Decided by: Niklas
- Related: [ADR-0002](0002-all-rust-stack-with-bevy.md), [ADR-0010](0010-cross-platform-determinism.md), [ADR-0015](0015-ron-content-packs-and-mods.md), GDR-0015, `docs/concept-notes.md` "Scripting candidates"

## Context

v1's scripting drove the ship at too low a level. In v2, behavior is layered (GDR-0015) and most players edit rule
lists, but custom nodes and mods need a real language. A script only returns a goal for the flight computer, runs
sandboxed with an instruction budget, is safe to share, and must be deterministic.

## Options considered

- **Rhai** (chosen): Rust-like syntax with braces, embeds in Rust, built-in operation limit, scripts see only the
  registered API. Catch: a tree-walking interpreter, fine for goal-level logic but not for per-frame math on
  hundreds of ships.
- **pocketpy**: Python 3 subset, embeds in C/C++ (the pick if the engine were C++ or Godot); an instruction budget
  needs checking or adding.
- **QuickJS** (JavaScript/TypeScript): TypeScript needs a compile step; no operator overloading, so vector math
  gets wordy.
- **Godot Sandbox**: Godot only, so out with Godot.
- **WASM**: strongest isolation and fuel metering, but players need an external compiler: expert tier only.
- **Lua / Luau**: mature and built for sandboxed game scripts, but rejected: Niklas dislikes Lua's `end` syntax.

## Decision

- Rhai for custom behavior nodes and for mods, with a deterministic API. The core stays Rust.
- The behavior text format is for everyone; Rhai is for custom nodes. Don't propose Lua.
- Status note: the concept notes' decision table still says "Leaning", while the concept board (revision 16) and
  the original `CLAUDE.md` list Rhai as settled and a hard rule. Recorded as `Accepted`.

## Consequences

- Rhai nodes are tagged in the behavior editor; scripts can only reach what the game registers.
- Interpreter speed keeps scripts at goal level, not per-frame control.
- Behavior system v1 started with Rhai conditions (roadmap, 11 Oct).
