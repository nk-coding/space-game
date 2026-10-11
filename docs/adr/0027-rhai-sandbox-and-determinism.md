# ADR-0027: Rhai runs as a raw, sandboxed engine with f32 floats, no `**`, libm math and an operation budget

- Status: Proposed
- Date: 2026-10-11
- Decided by: behavior agent (proposal for Niklas to review)
- Related: [ADR-0019](0019-rhai-scripting.md), [ADR-0010](0010-cross-platform-determinism.md), [ADR-0004](0004-target-platforms-and-performance-first.md), [ADR-0026](0026-behavior-programs-text-in-content.md), [GDR-0015](../gdr/0015-layered-behavior.md), `docs/behavior-format.md` "Rhai conditions"

## Context

Custom behavior conditions are Rhai scripts in content (ADR-0019). They must be sandboxed (read-only API, budget,
no I/O), build for wasm32 and Android, keep the sim `Send + Sync` (a Bevy resource) and be bit-exact across
platforms.

## Options considered

- **Engine setup**: `Engine::new()` with the standard packages: brings `print`, time, maths through the
  platform libm, a file module resolver; **`Engine::new_raw()`** (chosen): nothing but what we register.
- **Floats**: `no_float` (integers only: deterministic but awkward for shares and distances); f64; **`f32_float`**
  (chosen; matches the sim). `+ − × ÷ %`, comparisons, `abs`, `floor`, `sqrt` are exact IEEE operations.
- **Powers**: Rhai's built-in `**` on floats calls `powf`, i.e. the platform libm (not bit-identical across
  targets). An overload would only win with Rhai's fast operators turned off (slower for every operator), so
  **`**` and `**=` are disabled**.

## Decision

- `rhai = "=1.26.1"`, `default-features = false`, features `std`, `sync`, `f32_float`, `no_time`, `no_module`,
  `no_custom_syntax`. Builds for wasm32-unknown-unknown and wasm32-wasip1 without `wasm-bindgen` (no time
  needed); pure Rust, so Android is fine. Its `ahash` uses compile-time keys (no runtime RNG); hash values never
  reach script results (Rhai maps are ordered).
- Raw engine; `eval` disabled; `print`/`debug` silenced; strict variables (typos fail at load).
- Registered API only: `me` / `target` views (getters, no setters), the node's parameters, and `abs min max clamp
  floor ceil sqrt sin cos atan2 exp pi to_float to_int` (transcendentals via `sim::math` = libm).
- Budget: 2000 operations per evaluation, plus limits on call depth, expression depth, strings, arrays, maps,
  variables, functions. Running out, an error or a non-bool result counts as false; the debug view shows why.
- Scope variables are plain (not constants: Rhai's optimizer would fold constants into the compiled script) and
  are overwritten before every run.

## Consequences

- Checked: the behavior scenario with 1548 Rhai evaluations is IDENTICAL native vs wasm (1800 ticks).
- About 0.5–1 µs and one small allocation per evaluation (Rhai's function-resolution cache); the rest of the
  think pass allocates nothing.
- Scripts cannot raise to a power; write `x * x`. A deterministic `pow` could be registered later if needed.
