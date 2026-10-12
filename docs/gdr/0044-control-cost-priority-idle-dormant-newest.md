# GDR-0044: Parts have a control cost; parts over the cores' limit idle by power priority; the newest vessels go dormant first

- Status: Accepted
- Date: 2026-10-12
- Decided by: Niklas (options and recommendations by the lead session)
- Related: [GDR-0038](0038-logic-cores-control-parts.md) (answers its open questions), [GDR-0013](0013-vessels-and-piloting.md) (mind capacity, dormant cores), [GDR-0014](0014-power-tiers-and-weights.md) (tiers and weights), `docs/roadmap.md` "Queued"

## Context

GDR-0038 says each working logic core runs up to N parts and mind capacity caps the running cores. It left open
which parts count against N and what happens to parts beyond the limit (for example after a core is shot away).
GDR-0013 says cores beyond the mind capacity stay dormant, but not which ones.

## Options considered

Which parts count:
- **Control cost in the part data** (chosen): each part has a control cost; structure (armour, slopes, hull)
  costs 0, machines 1, big machines more. Armour-heavy ships aren't punished and costs can be balanced per part.
- **Machines count 1, structure is free**: a fixed rule. Not chosen.
- **Every part counts 1**: simplest to explain; big armoured ships would need many cores. Not chosen.

Parts beyond the limit:
- **Idle by priority** (chosen): the excess parts go idle in the same tier-and-weight order as a power brown-out,
  so the drydock settings decide what fails first; they come back when control returns.
- **Everything degrades evenly** (capacity / load): not chosen.
- **Farthest from a core first**: easy to read, but not controllable. Not chosen.

Which vessels go dormant over mind capacity:
- **Newest first, swappable** (chosen): the most recently formed vessels go dormant first (dark, like salvage,
  until capacity frees up); in the fleet bar the player can wake one, and another goes dormant. The focus vessel
  never goes dormant.
- **Smallest first**: not chosen.
- **Farthest from the focus first**: not chosen.

## Decision

- Every part has a **control cost** in its content data (placeholder numbers): 0 for structure, 1 for machines,
  more for big machines. A vessel's **control capacity** is the sum of N over its running cores.
- When a vessel's control load exceeds its capacity, parts go **idle by power priority** (lowest tier first,
  lowest weight first inside a tier, the same order as a brown-out, GDR-0014); they run again as soon as
  capacity returns.
- When the mind runs more cores than its **mind capacity**, the **newest vessels go dormant first**; the player
  can swap which vessels run in the fleet bar; the **focus vessel never goes dormant**.

## Consequences

- The vessel model in `sim` needs: a `control` cost on parts (default from the part kind), N on the `logic`
  block, the idle-by-priority pass (next to the power allocation), vessel age for the dormant order and a
  command to wake a dormant vessel. The drydock shows the control load against the cores' capacity.
- Still open: N per core type and the mind capacity numbers (start value, growth, the exclusive resource): lead
  placeholders until Niklas sets them.
