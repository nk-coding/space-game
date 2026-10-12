# GDR-0046: Engine exhaust hits the ship's own parts and enemies, not other friendly vessels

- Status: Accepted
- Date: 2026-10-12
- Decided by: Niklas (options and recommendation by the lead session)
- Related: [GDR-0039](0039-engine-exhaust-hurts-parts.md) (refines its "other ships" reading), [GDR-0045](0045-shields-absorb-rams-no-friendly-ram-damage.md) (same team rule for rams)

## Context

GDR-0039 makes each engine's plume damage and heat the parts in its path; that other ships' parts are hit as
well was the lead session's reading, marked for Niklas to review.

## Options considered

- **Own parts and enemies** (chosen): the plume hurts the ship's own parts (the layout rule) and enemy ships it
  touches; other friendly vessels are spared, like friendly rams (GDR-0045).
- **Everything it touches**: own parts, friendly vessels and enemies alike. Not chosen.
- **Own ship only**: the plume only matters for the ship's own layout. Not chosen.

## Decision

The exhaust plume damages and heats the parts of its own vessel and of enemy ships (and of salvage/debris, lead
default: pieces without an owner count as not friendly). Other vessels of the same team are not affected.

## Consequences

- The plume cast needs the team check from the vessel model; implement the plume after (or together with) owners
  and teams.
- Still open (GDR-0039): plume numbers, throttle scaling, thrust when blocked, whether shields stop the plume.
