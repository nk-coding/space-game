# GDR-0024: Big tech trees per family, with node kinds instead of Mk levels

- Status: Accepted
  2026-10-18: the first sidegrade pack (lead, content batch): variants with trade-offs (wide-bubble emitter, burst laser, long-barrel cannon, compact reactor, spore-lined radiator), a size class (the 1×2 heat sink) and the ammo feeder; the shields, thermal and utility families get their first nodes (`wide_bubble`, `heat_sink`, `ammo_feeder`), so every family now has research. The research screen's lanes now scroll (sixteen rows).
  2026-10-18: sidegrade check (lead, docs/results/sidegrades-2026-10-18.md): the wide-bubble emitter beat the plain one on every voyage (by 22–28 points on the Runner), now radius 3.0, capacity 1.6, regen 0.08; the other variants trade (the burst laser is the weak one: 0 % on the rim voyages, raiders keep out of its 40 m)
  2026-10-18: burst laser range 40 → 52 m (lead, docs/results/sidegrades-2026-10-18.md, follow-up): 44 / 31 / 14 / 16 % on the rim voyages against the pulse skiff's 56 / 31 / 18 / 15, a trade (faster, lighter pulses, shorter reach)
  2026-10-18: refinement built (lead, content batch; docs/results/refinement-2026-10-18.md): a node may unlock a step on one stat of the parts a selector picks (`PartStat`, an adjacency-bonus stat, added like bonuses), for the player's own designs only and passed in where a design is built (game, lab runs, specs, balance runner), never global; four I–III chains (bubble recharge, fin tuning, nozzle tuning, pulse cycling) of +3 to +8 % a step; a full chain is worth 0 to 13 points on the rim voyages (pulse cycling cut from 5 to 3 % a step)
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
