# GDR-0048: No friendly fire: shots pass through the shooter's own vessels

- Status: Accepted
- Date: 2026-10-12
- Decided by: Niklas (options and recommendation by the lead session)
- Related: [GDR-0045](0045-shields-absorb-rams-no-friendly-ram-damage.md) (friendly rams), [GDR-0046](0046-exhaust-hits-own-parts-and-enemies.md) (friendly exhaust), [GDR-0013](0013-vessels-and-piloting.md) (vessels, owners)

## Context

With the vessel model (12 Oct 2026) a ship that splits leaves several vessels of one owner close together. Rams
and exhaust already spare friendly vessels, but shots hit everything, so a split-off vessel could shoot its parent.

## Options considered

- **No friendly fire** (chosen): shots pass through the shooter's own vessels; consistent with rams and exhaust,
  and the cheapest (one filter in the hit test).
- **Block, no damage**: own vessels stop shots without damage, so a wingman in the line of fire soaks them. Not
  chosen.
- **Full friendly fire**: not chosen.

## Decision

Shots (lasers, beams, projectiles) ignore every vessel of the shooter's owner: no hit, no damage, no shield drain;
they fly on. Salvage (no owner) and other owners are hit as before (lead default: a shot fired by a vessel that has
since lost its owner keeps the owner it was fired with).

## Consequences

- Wingmen can fire through each other, so formation behaviors don't need line-of-fire checks against friends.
- The hit test needs the shooter's owner on every shot.
