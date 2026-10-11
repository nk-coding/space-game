# GDR-0024: Big tech trees per family, with node kinds instead of Mk levels

- Status: Accepted
- Date: concept phase (exact day not recorded; settled by 2026-10-11)
- Decided by: Niklas
- Related: [GDR-0025](0025-research-from-salvage.md), [GDR-0027](0027-races.md), [GDR-0032](0032-balancing-arena-evolution.md), [ADR-0015](../adr/0015-ron-content-packs-and-mods.md)

## Context

The game is meant for long play with lots of content. Progress should show on the ship itself.

## Options considered

- **Mk I–III levels** for parts: rejected. The concept board's reasoning for the chosen trees: most nodes are
  trade-offs, and every new part or variant gets its own silhouette, so a ship's tech shows on its hull.
- **Big trees per family with distinct node kinds** (chosen).

## Decision

- One tree per system family (hull and armor, power, energy weapons, kinetic weapons, missiles, shields, thermal,
  propulsion, logic and sensors, utility and drones), about 200 nodes in total.
- Node kinds: **new part** (new silhouette, new role), **variant** (a sidegrade with a trade-off), **size class**
  (bigger footprint for bigger hulls), **refinement** (a short I–V chain of small stat steps), **alien** (needs
  reverse-engineered tech).
- Cross-links between trees (for example cooling jackets need Thermal), so specialists still branch out a little.
- All data-driven, so the balance arena can test the whole tree without a rebuild.

## Consequences

- Two hundred nodes can't be balanced by hand, hence the arena (GDR-0032).
- Open: the tree size per family that stays fun instead of tedious.
