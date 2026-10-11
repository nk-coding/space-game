# ADR-0006: The web build is the development platform (web first), not an end-user platform

- Status: Accepted
- Date: 2026-10-11
- Decided by: Niklas
- Supersedes: [ADR-0005](0005-web-as-full-end-user-platform.md)
- Related: [ADR-0004](0004-target-platforms-and-performance-first.md), [ADR-0007](0007-development-setup-vm-and-host-browser.md), [ADR-0009](0009-spike-first-systems-before-content.md), ADR-0014, `CLAUDE.md` "Current priorities", `docs/concept-notes.md` "Development start"

## Context

Niklas develops in a VM without a GPU and looks at the game as a web build in the host browser (ADR-0007).
Earlier on 11 Oct the web had been made a full end-user platform (ADR-0005), which turned wasm size, browser
performance and browser-only features (IndexedDB saves, zip/URL mods, WebRTC) into requirements.

## Options considered

- **Keep the web as a full end-user platform** (ADR-0005).
- **The web build is the development platform; end users get Android and desktop** (chosen).

## Decision

- **Web first** (Niklas, 11 Oct): the web build is what matters now. Android comes later, so the phone parts of
  the spike (APK, phone timings, temperature, phone hash) are deferred.
- **Development platform, not end-user platform** (Niklas, later on 11 Oct): Niklas develops and views the game in
  the browser; end users get Android and desktop.
- Wasm size doesn't matter (don't optimise for it). Browser performance only needs to be comfortable for
  development. Native performance weighs most in engine and tech choices.
- Browser-only end-user features (IndexedDB saves, mods from a zip or URL, WebRTC co-op transport) are no longer
  requirements unless Niklas brings them back.
- Everything still compiles to wasm32 and stays deterministic across native, wasm and Android.

## Consequences

- Web release builds use the cargo release profile (opt-level 3) and `wasm-opt -O3`: speed over size.
- The physics benchmark weighs native numbers most; browser numbers are recorded for comfort (ADR-0014).
- Everything stays Android-compatible.
- Older text written for the full web platform (the "web bar" in `docs/spike-plan.md`, web mods in
  `docs/content-model.md`, the WebRTC transport for `net`) describes requirements that no longer apply.
