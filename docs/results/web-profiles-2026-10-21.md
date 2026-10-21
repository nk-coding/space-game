# Web profiles in the browser: web-release against release (21 Oct 2026)

The comparison ADR-0049 asked for, as far as the VM can make it. Both builds are commit e353830, trunk `--release`
with wasm-opt -O3; only the cargo profile differs: `web-release` (the client at opt-level `z`, the sim crates at 3,
fat LTO; what GitHub Pages gets) against `release` (everything at opt 3, the all-speed build before ADR-0049).

| Build | wasm raw | gzip | brotli |
|---|---|---|---|
| `web-release` | 31.95 MiB | 9.82 MiB | 6.55 MiB |
| `release` | 56.71 MiB | 14.60 MiB | 8.85 MiB |

Load test `?scene=sandbox&load=20` (the overlay's 60 s `LOADTEST` line), headless Chromium with SwiftShader, 1280×720,
three runs each, alternating, one at a time (load average 5–8 from the browser itself, nothing compiling):

| Build | frame ms mean (3 runs) | frame ms p95 | sim ms mean | sim ms p95 |
|---|---|---|---|---|
| `web-release` | 375.1 / 370.2 / 358.2 | 491.8 / 459.6 / 446.2 | 0.216 / 0.205 / 0.212 | 0.8 / 0.9 / 1.0 |
| `release` | 365.1 / 366.0 / 397.8 | 469.2 / 456.2 / 495.5 | 0.219 / 0.247 / 0.264 | 1.1 / 1.1 / 1.3 |

- No difference beyond the noise. The sim step is the same, as expected (the sim crates are at opt 3 in both).
- The frame time is SwiftShader's software rendering (about 2.7 fps here), so it cannot show what the client's
  ECS and UI code at `z` costs on a real GPU. That part of ADR-0049's check is still open: Niklas's browser and a phone
  with the overlay of the same load test, both builds.

## In Niklas's browser (same day)

The same load test on his machine (1440×765): `web-release` at `z` 20.5 ms mean frame (p95 24.5), `release` 16.7
(p95 20.8), the client at `s` 16.8 (p95 20.0) at 11.1 MiB gzip. So the client is at `s` (ADR-0052).
