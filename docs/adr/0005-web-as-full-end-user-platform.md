# ADR-0005: The web build is a full end-user platform

- Status: Superseded by [ADR-0006](0006-web-as-development-platform.md)
- Date: 2026-10-11
- Decided by: Niklas
- Related: [ADR-0004](0004-target-platforms-and-performance-first.md), [ADR-0015](0015-ron-content-packs-and-mods.md), [ADR-0023](0023-host-authoritative-netcode.md), [ADR-0024](0024-saves-seed-and-deltas.md), `docs/concept-notes.md` "Technical decisions" (Web)

## Context

The web was on the 10 Oct target list (ADR-0004). On 11 Oct, during the concept phase, Niklas decided what that
meant, and the concept board was updated: "a full platform, not just a dev build".

## Options considered

- **Web as a development or preview build only.**
- **Web as a full end-user platform** (chosen at the time). Reasons were not recorded.

## Decision

As decided on 11 Oct 2026:

- The same Rust code is compiled to WebAssembly and served as static files.
- Single player, co-op, mods and saves all work in the browser.
- Rendering: WebGL2 baseline, WebGPU where available. Audio starts after the first tap or click.
- Bevy's web build is probably single-threaded, so the web gets its own performance bar (set after measuring),
  and the drydock's device warning knows about it.
- Every crate compiles to wasm32: no threads or filesystem required inside `sim`.
- Co-op transport works in browsers: WebRTC data channels with a WebSocket fallback; the relay doubles as the
  signalling server.
- Web mods load from a picked zip or a URL; saves use browser storage (IndexedDB) plus the same export.
- Rapier's determinism docs list WASM; box2d-rust and Avian had to be checked in the benchmark.

## Consequences

Superseded later the same day by ADR-0006: the web build is a development platform. What carries over:
everything still compiles to wasm32 and stays deterministic across native, wasm and Android, and WebGL2 is the
rendering baseline of the development build. The browser-only end-user features above are no longer
requirements unless Niklas brings them back.
