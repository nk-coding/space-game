# GDR-0039: Engines may sit anywhere; their exhaust damages and heats the parts in its path

- Status: Accepted
- Date: 2026-10-12
- Decided by: Niklas (that other ships' parts are hit as well is the lead session's reading, part of the decision for Niklas to review)
- Supersedes: [GDR-0004](0004-grid-ship-building-no-wiring.md), only its engine rule ("engines need open space behind") and the open question on engine placement; the rest of GDR-0004 stands
- Related: [GDR-0005](0005-fully-physical-ships.md), [GDR-0010](0010-turning-sources.md), [GDR-0011](0011-locked-flight-rear-engines-trade-off.md), [GDR-0019](0019-damage-model.md), [GDR-0022](0022-heat-per-part.md), `docs/content-model.md` "Coordinates and geometry", `docs/roadmap.md` "Queued"

## Context

GDR-0004 required open space behind every engine and left open whether engines may sit anywhere with open space
behind or only on the rear edge; the concept board's evolution lab had found nose and mid-ship engines legal.
The sim implements the rule as a clear exhaust: an engine with a part of its own piece in the row directly
behind it gives no thrust, and the validator warns about a blocked exhaust.

## Options considered

- **Engines only on the rear edge**: reason not recorded.
- **Engines anywhere with open space behind** (the rule so far): replaced.
- **Engines anywhere; the exhaust plume damages and heats whatever is in its path** (chosen). Reason not
  recorded beyond the decision itself.

## Decision

- Engines may be placed anywhere; there is no open-space rule any more.
- Each engine's plume **damages and heats the parts in its path**: the ship's own parts, and parts of other
  ships and debris (lead's reading: natural for a physical plume; marked here for Niklas to review).
- **Plume length and damage** (and heat) come from the engine's data.

## Consequences

- Burying an engine in the hull becomes a layout trade-off (it burns its own ship) instead of a forbidden build;
  a plume can also be aimed at enemies or used to clear debris.
- Implementation comes after the Rapier merge (the plume is a cast along the exhaust). Then the clear-exhaust
  rule goes: `exhaust_blockers` and `Thruster::clear` for engines in `sim`, `ShipGrid::exhaust_clear` and the
  "exhaust blocked" warning in `data`, the text "needs open space behind it" in the base content
  (`parts/propulsion.ron`) and in `docs/content-model.md`.
- RCS pods are unchanged: engine rules never apply to them (GDR-0010).
- Open, to decide with the implementation: the plume numbers per engine (placeholders), whether damage and
  heat scale with throttle, whether a blocked plume still gives full thrust, and whether shields stop it.

12 Oct 2026: [GDR-0046](0046-exhaust-hits-own-parts-and-enemies.md) settles the lead's reading: the plume hits the
ship's own parts and enemies, not other friendly vessels.

12 Oct 2026: implemented; the clear-exhaust rule is gone. [GDR-0049](0049-engine-plume-v1.md) records the lead
defaults for the open questions (Accepted, provisional): damage and heat scale with the engine's thrust output, a
blocked plume keeps full thrust, bubbles of non-friendly ships absorb the plume like energy damage (own bubbles
never), each plume lane stops at the first part it hits; plume numbers per engine in the content (placeholders).
