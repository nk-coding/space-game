# Architecture Decision Records (ADR)

Technical decisions for the space game: crates, determinism, content format, rendering, physics, tooling,
platforms. Game design decisions are in [`docs/gdr`](../gdr/README.md). [`docs/concept-notes.md`](../concept-notes.md)
is the overview and links to both series. Format and status rules: `CLAUDE.md`, section "Decision records".

## Index

<!-- index:start -->
| ADR | Title | Status | Date |
|---|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Accepted | 2026-10-11 |
<!-- index:end -->

## How to add, supersede and deprecate

- **Add:** copy [`0000-template.md`](0000-template.md) to the next free number (`NNNN-short-slug.md`), keep it readable in under two minutes, add its row above and link it from `docs/concept-notes.md`. Agents write `Proposed`; Niklas's decisions are `Accepted`; lead-session defaults are `Accepted (provisional, lead)`.
- **Supersede:** never rewrite a decision. Write a new record with a `Supersedes: ADR-NNNN` line, then change only the old record's status to `Superseded by ADR-MMMM` (and its row here).
- **Deprecate:** when a decision stops applying and nothing replaces it, set its status to `Deprecated` and add one dated line saying why.

## Open

Technical questions without a decision yet (no record until decided, except where a `Proposed` record exists):

- **Physics engine**: [ADR-0014](0014-physics-engine-by-benchmark.md) (`Proposed`). The benchmark harness is in `crates/bench`; Niklas picks from the numbers.
- **Glow method**: glow sprites vs full-screen bloom, measured in the browser load test ([ADR-0020](0020-style-2a-renderer.md)).
- **Co-op transport on native** now that the browser is not an end-user platform ([ADR-0023](0023-host-authoritative-netcode.md), [ADR-0006](0006-web-as-development-platform.md)).
- **Phone measurements** (spike questions 2, 4–7 on the phone): deferred until Android work starts ([ADR-0006](0006-web-as-development-platform.md), [ADR-0009](0009-spike-first-systems-before-content.md)).
