# GDR-0013: Every piece with a working logic core is a vessel; you pilot one and switch focus instantly

- Status: Accepted
- Date: 2026-10-11
- Decided by: Niklas (concept developed by the lead session)
- Supersedes: [GDR-0003](0003-pilot-one-ship-with-escort.md)
- Related: [GDR-0002](0002-awakened-ship-mind-light-story.md), [GDR-0005](0005-fully-physical-ships.md), [GDR-0006](0006-ship-scale-no-fixed-cap.md), [GDR-0014](0014-power-tiers-and-weights.md), [GDR-0015](0015-layered-behavior.md), GDR-0017, `docs/roadmap.md` "Queued"

## Context

The player's role was "pilot one ship with a small scripted escort; the fleet is secondary" (GDR-0003). Breakage
(GDR-0005) raised the question of what happens to split-off pieces that still have a logic core. The lead
session's provisional rule (11 Oct, `docs/sim.md`) made every piece other than the controlling one salvage, even
with a core, and a ship whose controlling piece lost its last core went dark for good.

## Options considered

- **Split-off pieces are salvage, even with a core** (the lead's provisional rule).
- **Every cored piece is a vessel of its owner** (chosen). Other options were not recorded.

## Decision

- Every working logic core hosts a copy of the ship-mind. Any connected piece with at least one working logic
  core is a **vessel** of its owner: the main ship, escorts, drones, and pieces split off a ship.
- The player pilots exactly one vessel directly (the **focus**); every other own vessel runs its behavior (AI).
  Focus can switch to any own vessel at any time, **instantly** (click or tap a friendly vessel, a hotkey, or a
  fleet bar in the HUD); the camera glides over.
- **Splits:** each piece that still has a working core becomes its own vessel and inherits the parent's behavior,
  limited to the rule slots its own cores provide. If the piloted piece loses its last core, focus moves to
  another vessel automatically. Pieces without a working core become salvage; pieces with an unpowered core stay
  dark until power returns.
- Enemies follow the same rules: shooting a raider in half can leave two raiders.
- **Mind capacity:** the number of logic cores the mind can run at once is limited and grows with progression (an
  exclusive resource, upgrades, research; details later). Cores beyond the limit stay dormant. During development
  the limit is a placeholder setting.

## Consequences

- Escorts are just vessels; "command capacity" (GDR-0003) is replaced by mind capacity.
- The sim still implements the provisional salvage rule; the vessel model (owners and teams, focus switching, fleet
  bar, camera glide) is queued in the roadmap.
- Open: mind capacity numbers and the exclusive resource; whether mind capacity still limits ship size (GDR-0006).
