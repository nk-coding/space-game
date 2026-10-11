# ADR-0009: Spike first, keep the spike as the foundation, build systems before content

- Status: Accepted
- Date: 2026-10-10 (spike plan); 2026-10-11 (foundation, minimal content)
- Decided by: Niklas (spike plan, minimal content); the "foundation, not throwaway" rule is unattributed in the sources (written by the lead session)
- Related: [ADR-0004](0004-target-platforms-and-performance-first.md), [ADR-0006](0006-web-as-development-platform.md), [ADR-0014](0014-physics-engine-by-benchmark.md), `docs/spike-plan.md`, `docs/roadmap.md`

## Context

Before any game code the big risks were the physics engine (speed, compound bodies, breakage), cross-platform
determinism, phone performance, the cost of the style 2a look and the feel of the controls.

## Options considered

- **Physics benchmark later** (the first plan) or **first** (chosen by Niklas, 10 Oct).
- **Throwaway spike** or **spike code as the real foundation** (chosen, 11 Oct).
- **Content early** or **minimal content, systems first** (chosen by Niklas, 11 Oct).

## Decision

- A spike, Bevy only, with no fixed time box: it runs until every pass/fail question in `docs/spike-plan.md` has
  an answer. Steps: physics benchmark → core sim with breakage and a same-hash test → web, Android and feel →
  load test with 1, 5 and 20 ships → cross-device state hash.
- Claude Code writes and runs it in the VM, in a local folder with local git.
- The spike code is written as the real foundation of the game, not as throwaway code.
- Minimal content at the start: a few materials, the ~16 drydock parts, a couple of ships.
- The phone steps are deferred (web first, ADR-0006).

## Consequences

- Spike code meets the full rules (determinism, content in data); the phase plan lives in `docs/roadmap.md`.
- `docs/results/spike-summary.md` answers the questions that don't need the phone.
- The spike plan listed behavior trees and Rhai as out of scope; the roadmap has since started the behavior
  system after the sim core.
