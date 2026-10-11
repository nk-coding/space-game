# ADR-0004: Targets are mid-range Android and desktop; performance comes first

- Status: Accepted
- Date: 2026-10-10
- Decided by: Niklas
- Related: [ADR-0005](0005-web-as-full-end-user-platform.md), [ADR-0006](0006-web-as-development-platform.md), [ADR-0009](0009-spike-first-systems-before-content.md), [ADR-0011](0011-30hz-sim-60fps-interpolation.md), [ADR-0014](0014-physics-engine-by-benchmark.md), [GDR-0006](../gdr/0006-ship-scale-no-fixed-cap.md), `docs/spike-plan.md`

## Context

The brief: a 2D game for Android, Windows and Linux with efficient native code and 2D physics. Ships are large,
physical and break apart, and fights have up to about 20 ships with debris. Maybe a Play Store release in the far
future; Steam is unlikely (heavy AI use in development).

## Options considered

- Alternatives to the platform list were not recorded. The web's role was decided separately (ADR-0005,
  superseded by ADR-0006).
- For trade-offs, Niklas set one rule: performance is the top priority when choosing between options.

## Decision

- End-user platforms: **Android mid-range 2021+** (Snapdragon 7-series class, 6 GB RAM), **Windows** and
  **Linux**. (The 10 Oct list also had the web; ADR-0006 defines its role now.)
- 60 fps rendering with a 30 Hz fixed simulation (ADR-0011).
- Headroom rule, because the test phone is Niklas's flagship: 20 ships with debris at 60 fps using at most half
  the frame budget (about 8 ms per frame, sim step under 4 ms), plus the phone's temperature after 10 minutes.
- **Performance is the top priority** when choosing between options; for example, the physics engine is picked
  by benchmark (ADR-0014).

## Consequences

- Everything stays Android-compatible (no desktop-only APIs) even while development happens on the web.
- The phone measurements wait until Android work starts (ADR-0006).
- Ship size is limited by measured cost per device rather than a fixed cap (GDR-0006).
