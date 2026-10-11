# GDR-0036: The behavior text format: one line per rule, `when ... and ... -> action`, no `end`

- Status: Proposed
- Date: 2026-10-11
- Decided by: behavior agent (proposal for Niklas to review)
- Related: [GDR-0015](0015-layered-behavior.md), [ADR-0019](../adr/0019-rhai-scripting.md), [ADR-0026](../adr/0026-behavior-programs-text-in-content.md), `docs/behavior-format.md`

## Context

GDR-0015 asks for a text format with one line per rule, switchable with the graph view. It must read well, be
typeable on a phone, have no `end` (Niklas dislikes Lua's), be precise, round-trip losslessly and give good
errors.

## Options considered

- **Keyword**: `when` (concept board) (chosen), `if` (accepted as an alias), or none.
- **Joining conditions**: `and` (chosen; reads plainly) vs commas (concept board; shorter but easy to misread
  next to argument commas). No `or` and no nesting: "this or that" is two rules, as in Gladiabots.
- **Arguments**: `orbit(45m, cw)` with brackets (chosen; unambiguous, defaults fill trailing values) vs
  space-separated `orbit 45m cw` (concept board; ambiguous next to comparisons).
- **Comparisons**: values are compared (`hull < 30%`) instead of one node per direction (`hull_below(30%)`):
  halves the vocabulary and reads like the editor mockup.
- **Units**: `m`, `s`, `%`; a `%` on a distance means a share of the ship's weapon reach (`approach(67%)`), so
  one behavior fits ships with different guns. A percentage always needs `%` (`hull < 30` is an error, not 30%
  or 0.3).
- **Comments**: a `# note` at the end of a rule, kept as part of the rule so it survives the graph view; lines
  that are only a comment are errors (they would be lost in the graph view).

## Decision

```text
target weakest(engines)
when hull < 30% -> flee                 # run when badly hurt
when target_in_range(120%) and cycle(9s) >= 5s -> orbit(60%)
always -> approach(67%)
```

- An optional `target <selector>` first line, then rules `when c and c -> action` or `always -> action`.
- `not` in front of a condition; `<`, `<=`, `>`, `>=`. Case-insensitive names; `if`, `→`, `≤`, `≥` accepted for
  phones and printed canonically. Errors give line, column, what was expected and a "did you mean".
- Parse → print → parse is lossless; the printed form is canonical.

## Consequences

- Content stores exactly these lines (ADR-0026); the wiki and scan reports can show them as-is.
- Future syntax (an `or`, named arguments, per-rule targets) can be added without breaking existing lines.
