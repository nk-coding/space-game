# ADR-0001: Record architecture decisions

- Status: Accepted
- Date: 2026-10-11
- Decided by: Niklas
- Related: [GDR-0001](../gdr/0001-record-game-design-decisions.md), `CLAUDE.md` "Decision records", `docs/concept-notes.md`

## Context

Until 11 Oct 2026 every decision lived in `docs/concept-notes.md` (with dates), the concept board, `CLAUDE.md`
and the docs of each crate. The reasons and the rejected options were often only implied, the lead session's
provisional defaults sat next to Niklas's decisions, and several agents work in the repo at the same time.

## Options considered

- **Keep `concept-notes.md` as the only record**: one file, but it keeps growing, mixes statuses and loses the
  reasons.
- **One numbered series for everything**: simpler numbering, but technical and design readers look for different
  things.
- **Two numbered series** (chosen): ADRs for technical decisions, GDRs for game design (GDR-0001).

## Decision

- Technical decisions (crates, determinism, content format, rendering, physics, tooling, platforms) get an
  Architecture Decision Record: `docs/adr/NNNN-<slug>.md`, numbered from 0001.
- Records are short (MADR-like): status, date, who decided, context, options considered, decision,
  consequences, links. Where a reason was never recorded, the record says so instead of guessing.
- Statuses: `Proposed` (agents may add these), `Accepted` (Niklas decided), `Accepted (provisional, lead)` (a
  lead-session default for Niklas to review), `Superseded by …`, `Deprecated`.
- A decision is never changed by rewriting its record: a new record supersedes it, and the old record's status
  says so.
- `docs/concept-notes.md` stays the overview and links to the records.

## Consequences

- Any change that produces a decision (or a provisional default) writes or proposes its record in the same change.
- The backlog up to 11 Oct 2026 was written from `concept-notes.md`, the concept board, the docs and the git
  log. Where the sources disagree, the record notes it.
