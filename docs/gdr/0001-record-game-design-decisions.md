# GDR-0001: Record game design decisions

- Status: Accepted
- Date: 2026-10-11
- Decided by: Niklas
- Related: [ADR-0001](../adr/0001-record-architecture-decisions.md), `CLAUDE.md` "Decision records", `docs/concept-notes.md`

## Context

The concept phase produced many design decisions (ship rules, combat, controls, world, progression, look),
recorded as a table and dated bullets in `docs/concept-notes.md` and on the concept board. Development then
added more on 11 Oct 2026, some decided by Niklas, some set provisionally by the lead session. The reasons and
the options that lost were often missing.

## Options considered

- **Keep design decisions only in `concept-notes.md`**: easy to scan, but no reasons, no clear status, and
  changes overwrite history.
- **Mix design into the ADR series**: one list, but design readers would wade through tech records.
- **A separate numbered series for game design** (chosen), next to the ADRs (ADR-0001).

## Decision

- Gameplay and design decisions (ship rules, combat, controls, progression, world, look) get a Game Design
  Record: `docs/gdr/NNNN-<slug>.md`, numbered from 0001.
- Same format and status rules as the ADRs: status, date, who decided, context, options, decision,
  consequences, links; `Proposed` / `Accepted` / `Accepted (provisional, lead)` / `Superseded by …` /
  `Deprecated`; records are superseded, never rewritten.
- Questions that are still open get no record; they are listed under "Open" in `docs/gdr/README.md` until
  Niklas decides.

## Consequences

- Placeholder numbers (concept-board stats, tuning constants) are not design decisions; records name them only
  as examples and say they are placeholders.
- `docs/concept-notes.md` keeps the overview and links each decision to its record.
