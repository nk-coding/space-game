# GDR-0070: Phase 7 wave 2 defaults: flight and AI numbers, voyage clearing details, the career save, the drydock gating

- Status: Accepted (provisional, lead)
- Date: 2026-10-13
- Decided by: lead session (defaults for Niklas to review); details by the wave-2 agents
- Related: [GDR-0061](0061-fight-balance-after-turning-v2.md), [GDR-0064](0064-lock-feed-forward-and-ai-flight-fixes.md), [GDR-0065](0065-phase-7-screens-and-escort-action.md), [GDR-0066](0066-phase-7-wave-1-defaults.md), [GDR-0068](0068-voyage-sites-clear-when-enemies-are-disarmed.md), `docs/sim.md`, `docs/career.md`, `docs/client.md`, `docs/drydock.md`

## Context

The second wave built the decisions of GDR-0061 to GDR-0068 (balance content, the lock feed-forward and AI flight
fixes, the escort action, voyage clearing, the career save and the drydock's unlock gating) and fixed the
reviews of the first wave. The choices below were made on the way. All numbers are placeholders.

## Decision

Flight and AI:
- The whole-speed fade uses `min(whole speed, 4 × the thruster's own along-axis speed)`, continuous in the push
  angle; a push across the motion can overshoot the rating by at most 3 %.
- The lock's feed-forward rate is capped at √(margin · α · π); the A/D nudge is not fed forward.
- "Engine-less" means no working non-RCS engine; such a vessel's turret aim uses the lock's smart aim.
- A split-off vessel's hull share uses its own piece's intact hit points.
- `escort(distance)`: default 30 m, at least 5 m; it holds when the leader is missing.
- The lock framing camera zooms out just enough to keep the target 90 px inside the edges, never below 35 % of
  the chosen zoom; a hand zoom during a lock ends framing until unlock.

Voyages and salvage:
- "Armed" = an alive gun and an alive reactor on the controlling piece (overheated, idle or blocked guns still
  count). The Warden's heart carries a cannon (one shield emitter became armour).
- Sleeping sites also wake on shield hits and damaging rams. Collecting needs a relative speed of at most 4 m/s;
  one strip per piece per collect pass. A shrinking hold reports what it dropped.
- The voyage plan has its own seeded generator; the roller version is 2 (version-1 saves migrate with a note,
  newer ones are refused, never reset).
- Validation: content needs a start; a start ship needs a core (a warning without a hold); a voyage that can
  never open is a warning; a goal encounter must always have an armed ship.

Career and drydock:
- The NEW tags' "seen" set lives in the save. `?unlock=all` never writes unlocks to the save. A new game seeds
  from the clock. A bad `?save=` name falls back to `career`. An escort whose design is gone leaves the fleet.
- Locked parts and materials that no research unlocks (reactive armour, the automaton plate) stay hidden.
- FLEET: four escort slots; one click on a design card adds it; a design may fill several slots.
- Behind BLUEPRINTS / EXPORT only Ctrl+S acts. Graph edits, undo and sliders wait while the TEXT view has errors.

## Consequences

- All easy to change; the slice balance pass (7K) revisits the numbers.
