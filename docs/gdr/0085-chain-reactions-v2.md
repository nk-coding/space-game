# GDR-0085: Chain reactions v2: reach, armour shielding, more exploders

- Status: Accepted
- Date: 2026-10-14
- Decided by: Niklas (the rules: blasts reach other ships and shields absorb them, armour shields, which parts
  explode, reserve guns stay; 14 Oct 2026); the mechanics details and every number are lead defaults for him to
  review
- Related: [GDR-0084](0084-chain-reactions-v1.md) (v1, its open questions), [GDR-0019](0019-damage-model.md),
  [GDR-0045](0045-shields-absorb-rams-no-friendly-ram-damage.md), [GDR-0046](0046-exhaust-hits-own-parts-and-enemies.md),
  [GDR-0048](0048-no-friendly-fire.md), [GDR-0067](0067-rams-pass-friendly-bubbles.md), [GDR-0044](0044-control-cost-priority-idle-dormant-newest.md),
  `docs/sim.md` "Damage and breakage", `docs/content-model.md` "Parts", `docs/arena.md` "What the example runs found"

## Context

GDR-0084 built chain reactions inside one piece and left four questions open. Niklas answered them on 14 Oct 2026:

1. **Reach**: "Yes, shields absorb." A blast hits everything within its radius: other pieces of the same ship,
   other ships, wrecks and loose pieces. Another ship's shield absorbs blast damage like any other damage; the
   exploding ship's own shield still does not help its own parts (the blast is inside the bubble).
2. **Shape**: "Armour shields." Parts behind an intact armour block, seen from the blast's centre, take less or
   nothing. No distance falloff.
3. **More exploders**: the automaton core (a power plant), the shield emitter (stored energy) and RCS pods (small,
   like thrusters) explode too.
4. **Reserve guns** stacked behind guns, and more guns than the control capacity: "Fine, emergent play." No change.

## Options considered

- **Armour on the line**: a cell walk from the blast's centre to each target (chosen: each intact armour part
  crossed lets its `blast_factor` through, so steel halves what continues and reactive plate quarters it; the same
  number already says how much of a blast the plate itself takes), a flat "behind any armour: nothing", or a share
  per cell of armour crossed (a 1 × 3 slope would count three times).
- **Which cell of a target**: the line to the centre of its cell nearest the blast (chosen: one line per target,
  compiled), the least covered of its cells, or every cell weighted.
- **When armour counts as intact**: as each blast goes off, all its hits computed from the state before it
  (chosen: within one blast, plate the blast breaks still shields what lies behind it from that blast; a later
  blast of the chain meets only what is left), before the whole chain, or hit by hit (the result would depend on
  part order).
- **Other pieces and ships**: compiled per design pair (impossible: they move) or a proximity scan when the blast
  goes off (chosen: bounding circles in ship and piece order, then the grid cells in the piece's own frame; no
  physics query, so no order to sort out).
- **Radii**: GDR-0084's (1 m for a fission reactor, 1.5 m for the 2 × 2 machines) or two cells deep (chosen: 1.5 m
  and 2 m). With the old radii no part can lie behind another within a blast's reach, on its own ship or
  alongside, so armour shielding would never apply; and a fission reactor reaches only 0.5 m past its own cell, so
  another ship is hit only when it touches it.
- **Friendly vessels**: spared, as shots (GDR-0048), plumes (GDR-0046) and rams (GDR-0045) spare them, or hit
  (chosen: a blast is no weapon anyone aims; the exploding ship's own other pieces are hit anyway).

## Decision

- **Reach.** A part with an `explosion` that damage destroys blasts every alive part with a cell whose nearest
  point lies within `radius` m of its centre, on **any piece of any ship**: its own piece (as it was before this
  resolution) through the design's compiled lists, every other piece (its ship's split-off pieces, other ships,
  wrecks, salvage) found when the blast goes off. Loose fragments and cargo items are not parts and are not hit.
- **Damage**: `damage × the target's blast_factor`, then `× blast_factor` of every intact armour part (a part
  with a `blast_factor` below 1, alive on its piece when the blast goes off) whose cell the straight line from the
  blast's centre to the target's nearest cell centre passes through, not counting the target and the exploding
  part. Touching a cell's corner or running along its edge is not passing through it. Machines shield nothing. A
  slope blocks its whole cell. Across pieces both the target's piece's and the exploding piece's armour count; a
  third piece in between does not. No falloff.
- **Shields**: bubbles of other ships that the blast crosses (they cover the target's cell centre and not the
  blast's centre) absorb first, their `Missile` share (`sim::chain::BLAST_FAMILY`: an explosion is what a warhead
  is; the base emitter absorbs missiles whole), one target at a time, in bubble order. The exploding ship's own
  bubbles never do. **Blasts spare no one**: friendly vessels are hit and their bubbles absorb, unlike shots, plumes
  and rams (lead default).
- **Order**: one queue over every ship's destroyed parts (ships in index order, each ship's parts in the order they
  died); every part a blast destroys, on any ship, joins its end at once. Each blast computes all its hits first,
  then applies them: its own piece's targets ascending, then the other pieces in ship and piece order, their targets
  ascending. All of it at breakage resolution, before fragments, splits and events, in the same tick. Each ship
  reports its own `PartDestroyed` and `Explosion` events; `SimEvent::Explosion` is unchanged.
- **Numbers** (placeholders, lead defaults; changed or new in bold):

| Part | Damage | Radius | Reaches |
|---|---|---|---|
| Fission reactor (1 hp) | 1.5 | **1.5 m** (was 1) | the 8 cells around it and the next cell straight out on each side |
| Fusion reactor (2 × 2, 4 hp) | 2.5 | **2 m** (was 1.5) | the 12 cells around it and the 8 two cells straight out |
| Torch drive (2 × 2, 4 hp) | 1.2 | **2 m** (was 1.5) | as the fusion reactor |
| **Automaton core** (2 hp) | **1.5** | **1.5 m** | as the fission reactor: it is a power plant |
| **Shield emitter** (1 hp) | **1** | **0.6 m** | the 4 cells beside it: its stored field lets go, twice a capacitor's blast |
| Capacitor (1 hp) | 0.5 | 0.6 m | the 4 cells beside it |
| Thruster, **RCS pod** (1 hp) | 0.5 | 0.6 m | the 4 cells beside it |

  Armour `blast_factor` unchanged (steel, composite and automaton plate 0.5, reactive 0.25). So a fission reactor
  takes every 1-hit-point machine in the 8 cells around it and the one two cells out unless a plate stands between
  (then 0.75: it survives); a shield emitter takes the 1-hit-point machine beside it.
- **Reserve guns and control overload** stay as the rules make them (Niklas): guns behind guns fire once the
  front gun is shot off, idle guns wake as parts are lost (GDR-0044).

## Consequences

- A reactor or core next to the hull now hurts a rammer, an escort or a wreck alongside within about 1 m (1.5 m
  for the 2 × 2 machines); an enemy's bubble takes it first. Plating around a reactor now protects what lies behind
  it.
- Voyages (goal cleared and home of 120, lost in brackets; `docs/sim.md` "Voyages"): the starter 59 / 36 / 15 / 4
  → 60 / 36 / 18 / 2, about the same, but lost more often (0 / 4 / 7 / 0 → 0 / 5 / 15 / 9); the Runner 83 / 91 /
  70 / 38 → 73 / 79 / 74 / 27, lost 0 / 0 / 1 / 3 → 5 / 6 / 3 / 8. With GDR-0084's radii and the rest of v2 the
  numbers stayed within the noise (starter 63 / 39 / 18 / 3, Runner 80 / 83 / 75 / 30): the longer reach is what
  makes the voyages harder.
- Evolution (`docs/arena.md`): the design run's best no longer uses automaton cores (four logic cores, two clear
  nose cannons, reactors in the rear corners); over five seeds of that run, archive elites with automaton cores went
  from 54 % to 42 % and best designs with them from 4 of 5 to 2 of 5. Common, no longer dominant. GDR-0084's best
  design (reserve guns) still wins 59 of 60 under the new rules.
- Every hash stream changed again; native and wasm32 stay identical.
- The drydock could show blast reach and cover (a follow-up of GDR-0084).

## Open (for Niklas)

- The radii: two cells deep (chosen) or GDR-0084's one cell (no shielding in the base game, voyages as before)?
- Friendly vessels: hit by blasts (chosen) or spared like shots, plumes and rams?
- Blast and shields: the missile share (a bubble absorbs it whole) or the kinetic one (half passes)?
- Should a shield emitter's blast scale with the charge it holds, and should the automaton core's be smaller than a
  fission reactor's (it makes 6 power, the reactor 10)?

14 Oct 2026: Niklas confirmed two of the lead defaults: reactors and drives keep their two-cells-deep reach, and blasts keep hurting friendly vessels.
