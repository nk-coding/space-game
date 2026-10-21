# ADR-0049: The published web build compiles the client for size and the simulation for speed

- Status: Superseded in part by ADR-0052 (the client's opt-level is "s", not "z"; the rest stands)
- Date: 2026-10-20
- Decided by: lead session (default for Niklas to review), on Niklas's request to make the wasm smaller
- Related: ADR-0048, ADR-0034, `docs/client.md` "Publishing the web build"

## Context

Since ADR-0048 the web build is a player platform, so its download size is loading time for players. The published
build (`scripts/web-build.sh --full`, GitHub Pages) used the cargo profile `release` (opt-level 3, thin LTO, one
codegen unit) and wasm-opt -O3: the game's wasm was 57.8 MiB raw, 15.3 MiB gzip, 9.5 MiB brotli (20 Oct 2026). The
128 MB file Niklas saw on port 8080 is the everyday `web-dev` build (no wasm-opt, a 55 MB name section); players never
get that one.

Where the size goes (twiggy over the linked wasm): about a third of the code is `bevy_ecs` instantiated per system
(roughly a thousand systems: `run_unsafe`, `initialize`, the system closures), then `bevy_reflect`, serde/RON for
the content, wgpu and the WESL shader parser (a required dependency of `bevy_shader`). Bevy's features were already
trimmed (workspace `Cargo.toml`). The embedded data (content, fonts) is about 6 MiB and compresses well.

## Options considered

All sizes for the game's wasm; the sim cost from `sim`'s `cluster` example as wasm32-wasip1 under Node (V8), three
runs, step mean in ms at 20/40 awake ships with the cluster asleep around them.

| Build | Raw | gzip | Sim step (20 / 40 asleep) |
|---|---|---|---|
| `release` (opt 3, thin LTO), wasm-opt -O3 (before) | 57.8 MiB | 15.3 MiB | 0.362 / 0.440 |
| the same, wasm-opt -Oz | 49.9 MiB | 15.1 MiB | – |
| opt-level `s` | 39.8 MiB | 12.1 MiB | – |
| opt-level `z`, fat LTO | 31.3 MiB | 9.9 MiB | 0.422 / 0.492 (Rapier ~50 % slower, small fights up to 78 %) |
| **`web-release`: `z` + fat LTO, the sim's crates at opt 3** (chosen) | **33.1 MiB** | **10.5 MiB** (brotli 7.2) | 0.363 / 0.466 (within noise) |

- **Keep `release`**: fastest everywhere, but the largest download.
- **opt-level `z` everywhere**: smallest, but the sim, and with it the evolution lab's workers that run fights flat
  out, loses up to half its speed under wasm. Performance is the top priority when choosing.
- **wasm-opt -Oz**: saves raw bytes but almost nothing after compression; not worth a slower pass.
- **`web-release`** (chosen): the client (Bevy, UI, rendering, content loading) for size, the simulation and what it
  runs on for speed. opt-level is a per-function attribute in LLVM, so it holds through LTO.

## Decision

- A cargo profile `web-release` (workspace `Cargo.toml`): inherits `release`, opt-level `z`, `lto = "fat"`; opt-level
  3 for `sim`, `arena`, `galaxy`, rapier2d, parry2d, nalgebra, simba, glamx, wide, libm and rhai.
- `scripts/web-build.sh --full` and `.github/workflows/pages.yml` build with it (`WASM_PROFILE=release` still gives the
  all-speed build for comparisons). wasm-opt stays at -O3.
- Native builds keep `release`; the everyday web build keeps `web-dev`.

## Consequences

- The published game is 43 % smaller raw and 31 % smaller gzipped (the lab's worker: 5.7 to 4.7 MiB raw). GitHub
  Pages serves gzip, not brotli.
- Determinism: the cluster benchmark's state hashes are identical at opt 3, `z` and `web-release`; opt-levels don't
  change float results (no fast-math). `scripts/determinism-check.sh` keeps checking native against wasm.
- A cold `--full` build takes about 15 minutes on the VM (fat LTO), up from about five.
- Bevy's ECS and rendering code at `z` was not measured in a real browser (the VM renders with SwiftShader); the
  browser performance pass (ADR-0048) should compare `web-release` with `release` on Niklas's machine and a phone, and
  move hot client crates to opt 3 if frames suffer.
- Open, not decided: a loading screen with progress (the bigger win for players than further megabytes), dropping
  Bevy's `reflect_auto_register` (bevy_reflect is ~2.7 MiB of code), egui's default fonts (~1 MiB, used as fallbacks).
