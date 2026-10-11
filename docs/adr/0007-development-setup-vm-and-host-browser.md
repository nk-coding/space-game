# ADR-0007: Develop in a GPU-less VM and view the game as a web build in the host browser

- Status: Accepted
- Date: 2026-10-11
- Decided by: Niklas (setup); lead session (headless browser check)
- Related: [ADR-0006](0006-web-as-development-platform.md), [ADR-0008](0008-local-git-only.md), [ADR-0009](0009-spike-first-systems-before-content.md), `CLAUDE.md` "Development environment"

## Context

Niklas develops in a Linux VM without GPU passthrough (15 cores, ~11 GB RAM, no sudo); Claude Code runs in the
VM. A native Bevy window there would be software-rendered and slow.

## Options considered

- **Native windows in the VM**: software-rendered, pointless for judging looks or speed.
- **Native sim and tools in the VM, the game viewed as a web build from the host** (chosen).
- Other setups (for example GPU passthrough): not recorded.

## Decision

- The simulation, tests, benchmarks and the arena run natively in the VM.
- The game is viewed as a web build served from the VM (`--address 0.0.0.0`; the VM is `10.77.6.2` on the LAN)
  and opened in the host browser.
- Agents check web work with a headless Chromium helper (`tools/headless/shot.mjs`): WebGL2 via SwiftShader,
  console messages and wasm panics, a screenshot to look at. SwiftShader timings say nothing about real
  performance.
- Android, later: APKs reach the flagship phone via adb over Wi-Fi (Wireless debugging); the VM network must
  reach the phone.
- Timing in the VM is noisy: repeat runs, report median and p95, record the load average.

## Consequences

- Everything visual must work in WebGL2 in a browser.
- Several agents build at once, so heavy builds use `-j 6` to keep RAM in check.
