# GDR-0038: Each logic core runs up to N parts; mind capacity caps the running cores and so the ship's size

- Status: Accepted
- Date: 2026-10-12
- Decided by: Niklas
- Related: [GDR-0013](0013-vessels-and-piloting.md) (refined: mind capacity), [GDR-0006](0006-ship-scale-no-fixed-cap.md) (refined: what limits ship size), [GDR-0035](0035-rule-slots-per-core.md), [GDR-0004](0004-grid-ship-building-no-wiring.md), `docs/roadmap.md` "Queued"

## Context

GDR-0013 defined mind capacity as the number of logic cores the ship-mind can run at once and left open whether
it still limits ship size. GDR-0006 said ships grow with drydock frames and with mind capacity; on 10 Oct mind
capacity had meant "how many parts the ship-mind controls". Nothing tied a core to the parts it runs.

## Options considered

- **Mind capacity caps the running cores only, and ship size comes from the drydock frames alone** (the open
  reading of GDR-0013): reason not recorded.
- **Each logic core runs up to N parts; mind capacity caps the running cores** (chosen). Other options were not
  recorded.

## Decision

- Each working logic core can run up to **N parts**. N is a placeholder in the core's content data, per core
  type.
- **Mind capacity** caps how many cores run at once (GDR-0013), so it caps the total number of running parts
  indirectly (about mind capacity × N).
- The **drydock frames** still set the grid size (GDR-0006).

## Consequences

- Answers the open question of GDR-0013 and GDR-0006: mind capacity does limit ship size, through the cores.
- Implementation comes with the vessel model (queued in the roadmap): a field on the `logic` block (next to
  `rule_slots`), the drydock showing parts against the cores' limit, and the sim applying it per vessel.
- Open, to decide with the vessel model: which parts count (every part, or only parts that do something, not
  armor); what happens to parts beyond the limit (for example after a core is shot away mid-fight); N per core
  type; the mind capacity numbers (still open from GDR-0013).

12 Oct 2026: [GDR-0044](0044-control-cost-priority-idle-dormant-newest.md) answers the open questions: parts have a control
cost in their data (structure 0); parts over the limit go idle by power priority; the newest vessels go dormant
first. N per core type and the mind capacity numbers stay open.
