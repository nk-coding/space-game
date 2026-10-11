# ADR-0010: The simulation is bit-exact deterministic across platforms

- Status: Accepted
- Date: 2026-10-10 (rules); 2026-10-11 (implementation choices)
- Decided by: Niklas (rules, concept phase); lead session (PCG32, FNV-1a, libm, clippy bans, the native-vs-wasm check)
- Related: [ADR-0003](0003-crate-architecture.md), [ADR-0011](0011-30hz-sim-60fps-interpolation.md), [ADR-0013](0013-physics-backend-trait.md), [ADR-0014](0014-physics-engine-by-benchmark.md), [ADR-0023](0023-host-authoritative-netcode.md), [ADR-0024](0024-saves-seed-and-deltas.md), GDR-0032, `docs/sim.md` "Determinism"

## Context

The balance arena replays any fight in the client, co-op wants desync detection, and exact replays and a later
rollback path in the arena need identical results on desktop, Android and wasm. Floating-point results can
differ between platforms: platform math libraries, fused multiply-add, hash-map iteration order, wall-clock time.

## Options considered

- **No cross-platform guarantee**: the host-authoritative netcode (ADR-0023) doesn't need determinism, but then
  there is no desync detection, no cross-device replays and no rollback path.
- **Bit-exact determinism on every platform** (chosen).

## Decision

Rules for `sim`:

- Fixed 30 Hz step (ADR-0011); no wall-clock time anywhere.
- Seeded RNG streams per system: in-crate PCG32 (checked against the reference vector), one stream each for
  damage, weapons, debris, AI and scenarios, seeded via splitmix64. Stream ids are part of the save/replay format:
  append only, never renumber.
- Ordered containers only (`Vec`, `BTreeMap`); never iterate a `HashMap`/`HashSet` in simulation logic. Ids are
  monotonic counters.
- `f32` with exact IEEE operations only. Transcendentals go through `sim::math` (pure-Rust `libm`, pinned
  `=0.2.16`, `arch` feature off). `crates/sim/clippy.toml` bans std's `sin`, `cos`, `atan2`, `exp`, `powi`,
  `mul_add` and friends. No FMA, no fast-math.
- The physics engine runs in its deterministic mode.
- A state hash (FNV-1a 64 over 32-bit words of the whole world) every tick in debug builds, on demand in release.
- Check: `scripts/determinism-check.sh` runs a scenario natively and as `wasm32-wasip1` under Node and reports the
  first divergent tick; browser hash logs can be compared the same way.

## Consequences

- 11 Oct 2026: native x86_64 and wasm32 give identical hashes for the core, flight and 20-ship load scenarios
  (FreeFlight physics). The phone comparison waits for Android.
- Every new sim feature, and the physics engine, must keep the check green.
- Some conveniences are off-limits in `sim`: std trig, hash-map iteration, RNG crates whose numbers may change on
  update.
