# GDR-0031: No generic scrap: materials come from places, and the fabricator resolves recipe chains without factory logistics

- Status: Accepted
- Date: 2026-10-10
- Decided by: Niklas
- Related: [GDR-0025](0025-research-from-salvage.md), [GDR-0026](0026-procedural-galaxy-and-bands.md), [GDR-0027](0027-races.md), [GDR-0029](0029-mild-death-bonded-matter.md), [GDR-0030](0030-no-economy.md)

## Context

Without an economy (GDR-0030), what you can build should depend on where you have been.

## Options considered

- **Generic scrap** as a universal material: rejected.
- **Factory buildings and logistics** for processing: rejected ("no buildings to place, no logistics"); reason not
  recorded beyond that.

## Decision

- No generic scrap: 30+ materials tied to where you have been (board sketch: 46 = 25 raw + 21 refined).
- Sources: harvesting in clusters (asteroids, comets, gas, star skimming), salvage (dismantling returns a part's
  materials and research data), race-specific sources, precursor sites, mission rewards (the rarest race
  materials). Materials follow the hazards (He-3 needs heat shielding, ionized gas comes from ion storms, hot
  isotopes from radiation zones).
- Recipes are chains, but the fabricator resolves them automatically if the raw materials are in storage and the
  hub has the needed upgrades (smelter, kiln, cryo plant, electronics bench, magnetics lab, fuel works, munitions
  bay, fusion workshop; race upgrades; the precursor forge last).
- Every logic core needs programmable matter (it hosts a copy of you); automaton cores at the rim are the first
  source.
- Harvesting uses tool parts (mining laser, gas scoop, tractor); harvest drones come later.
- Storage at home is unlimited; the voyage cargo hold is the only limit. Every source refills slowly, rare ones
  too. Fabrication happens only at home; on voyages, repair material only patches damage.

## Consequences

- Environmental gates also gate materials (GDR-0026).
- Materials are content (the `materials` kind already allows entries without an armor block).
