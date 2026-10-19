# GDR-0031: No generic scrap: materials come from places, and the fabricator resolves recipe chains without factory logistics

- Status: Accepted
  2026-10-17: asteroid rock names its material (rock kinds' `material`, [GDR-0104](0104-scenery-rocks-filler-landmarks.md)): two ore materials, nickel-iron and water ice, are the hook for harvesting asteroids; nothing gives them yet (no mining).
  2026-10-18: built (Niklas, [GDR-0108](0108-materials-economy-workshop.md)): parts have recipes in content, the WORKSHOP (one building for the fabricator and the salvage bay) fabricates them at home, three fabrication resources (copper, ceramic, lattice crystal) beside steel and programmable matter (logic cores take it), the first hub upgrades as facilities within the workshop (fusion workshop, growing vat, automaton forge); dismantling returns a share of a part's recipe (40 %, broken parts 25 %).
  2026-10-18: gate shards (lead, content batch): `gate_shard`, a precursor material, one from each gate's guardian beaten (a trail's `gives` for the gate sentinels, the bastions and the deep bands' Warden stand-ins; a voyage's new `gives` on its first clear for the Warden's gate). Research spends them on the three locks that had no way in: reactive plating (1), the automaton forge (1) and the capital drydock (3), so the rim's three gates open the first two and the frame waits for the outer reach.
  2026-10-19: harvesting begins ([GDR-0114](0114-mining-and-ore-refining.md)): the mining laser (a tool part) cuts nickel-iron and water ice from asteroid rock into the hold; the smelter, the first refining upgrade, resolves the chain when paying (steel from nickel-iron, coolant from ice), for recipes and research alike.
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
