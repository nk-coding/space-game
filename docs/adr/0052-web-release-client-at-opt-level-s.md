# ADR-0052: The published web build's client at opt-level `s`, not `z`

- Status: Accepted
- Date: 2026-10-21
- Decided by: Niklas's measurements in his browser (the lead picked the variant they pointed to)
- Supersedes: ADR-0049 in part (the client's opt-level; the sim crates at 3, fat LTO and wasm-opt stand)
- Related: ADR-0048, ADR-0051, `docs/results/web-profiles-2026-10-21.md`

## Context

ADR-0049 compiled the client at opt-level `z` and left one check open: whether Bevy's ECS and rendering at `z` slow
the frames in a real browser (the VM renders with SwiftShader, so its frame times can't tell). Niklas ran the
20-ship load test (`?scene=sandbox&load=20`, the overlay's 60 s `LOADTEST` line) on his machine at 1440×765, every
build from commit e353830 with fat LTO and wasm-opt -O3 except where noted:

| Client | Sim crates | wasm raw | gzip | brotli | frame ms mean / p95 |
|---|---|---|---|---|---|
| `z` (ADR-0049) | 3 | 32.0 MiB | 9.8 MiB | 6.6 MiB | 20.5 / 24.5 |
| `s` | 3 | 37.4 MiB | 11.1 MiB | 7.2 MiB | 16.8 / 20.0 |
| 3 (profile `release`, thin LTO) | 3 | 56.7 MiB | 14.6 MiB | 8.9 MiB | 16.7 / 20.8 |

A fourth try, everything at 3 and only the parsers, decoders, SVG rasteriser and shader translator at `z`, saved just
1 MiB gzip against all-speed (most of the code is what runs every frame); its one measurement (30.8 ms, in a smaller
window) looked off and was not repeated.

## Decision

`web-release` compiles the client at opt-level `s`; the sim, arena, galaxy, physics, maths and Rhai crates stay at 3.

## Consequences

- Frames as fast as the all-speed build (16.7 ms is 60 Hz vsync on that screen), 1.3 MiB gzip more than `z`, 3.5 MiB
  less than all-speed.
- A phone was not measured; if frames suffer there, the next step is opt-level 3 for the hot Bevy crates.
