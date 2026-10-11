# GDR-0004: Ships are built on a grid of parts with no wiring: power pool per piece, heat and radiators, adjacency bonuses

- Status: Accepted
- Date: concept phase (exact day not recorded; settled by 2026-10-11)
- Decided by: Niklas
- Related: [GDR-0005](0005-fully-physical-ships.md), [GDR-0012](0012-halvex-runner-follows-rules.md), [GDR-0014](0014-power-tiers-and-weights.md), [GDR-0022](0022-heat-per-part.md), GDR-0030, `docs/concept/reference/drydock-model.js`, `docs/content-model.md`

## Context

The brief: grid ship building from components (weapons, structure, energy, engines), an easy editor, no wiring.
Layout should matter through mass, heat and power. The concept board's drydock sketch implements the rules
("the numbers are placeholders; the rules are the proposal").

## Options considered

- **Wiring** (power or data cables between parts): rejected by the brief ("easy editor, no wiring").
- Other systems: not recorded.

## Decision

- Parts sit on a grid with footprints (1×1, 1×2, 2×2, later 3×3 and up), rotation and mirroring.
- Armor: blocks and slopes 1×1, 1×2 and 1×3 (three angles), in materials (steel, composite, reactive, …). Slopes
  connect only through their straight sides.
- **No wiring:** every connected piece shares one power pool (allocation: GDR-0014).
- **Heat:** parts heat their neighbours; heat leaves only through edges open to space, and radiators vent the
  most. Overheated parts shut down by default; an **overdrive** option gives burst output at the risk of damage or
  an explosion.
- **Adjacency bonuses:** a few explicit pairings, drawn as links (capacitor beside laser, venting radiator beside
  reactor).
- Guns need a clear line of fire ahead; engines need open space behind; off-centre thrust turns the ship.
- Every part has a build-point cost (GDR-0030).

## Consequences

- The drydock shows overlays for heat, mass, bonuses, shields and arcs, and live stats.
- Content declares bonuses on the granting part, so mods can add parts without editing others.
- Open: engines anywhere with open space behind, or only on the rear edge? (The evolution lab found nose and
  mid-ship engines legal.) Overdrive is not built yet.
