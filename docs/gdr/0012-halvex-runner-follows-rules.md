# GDR-0012: The Halvex Runner's layout follows the drydock rules; the content blueprint is the source of truth

- Status: Accepted
- Date: 2026-10-11
- Decided by: Niklas
- Related: [GDR-0004](0004-grid-ship-building-no-wiring.md), [GDR-0007](0007-ship-style-2a.md), [GDR-0010](0010-turning-sources.md), [GDR-0022](0022-heat-per-part.md), `docs/content-model.md` "Ships"

## Context

The sample ship from the concept art, converted tile for tile, broke the drydock rules: its radiators couldn't
vent, slopes blocked the wing-tip lasers' line of fire, and parts overheated at normal load.

## Options considered

- **Keep the concept-art layout exactly**, with the art as the source of truth.
- **Adjust the layout to the rules**, keeping silhouette and symmetry (chosen).

## Decision

- Adjust the layout so the ship works under the drydock rules: radiators vent, the wing-tip lasers have a clear
  line of fire, no part overheats at normal load. Keep the silhouette and the left/right symmetry.
- The content blueprint (`content/base/ships/halvex_runner.ron`) is the source of truth;
  `art/ships/halvex-runner.*` stays as the original concept art.

## Consequences

- The change (11 Oct): the pulse lasers moved into the wing shoulders (the outline differs only there), the
  radiators vent at the wing tips and the tail, logic cores and shield emitters moved away from the fusion
  reactor, and two gyros were added. Nothing overheats in combat with placeholder numbers.
- Tests check silhouette, symmetry, the unchanged spine and that there are no rule warnings.
