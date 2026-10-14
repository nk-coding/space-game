# GDR-0084: Chain reactions v1: destroyed reactors, capacitors and drives explode into their piece

- Status: Accepted (provisional, lead). 14 Oct 2026: Niklas answered the open questions; blasts now reach other pieces
  and ships (their shields absorb), intact armour on the line shields what lies behind it, the radii grew to two cells
  deep and the automaton core, shield emitter and RCS pod explode too: [GDR-0085](0085-chain-reactions-v2.md). The
  rest stands.
- Date: 2026-10-14
- Decided by: Niklas (build chain reactions now, 14 Oct 2026); the numbers and v1 rules are lead defaults for him to
  review
- Related: [GDR-0019](0019-damage-model.md), [GDR-0020](0020-damage-v1-defaults.md), [GDR-0004](0004-grid-ship-building-no-wiring.md), [GDR-0070](0070-phase-7-wave-2-defaults.md), [GDR-0083](0083-evolution-lab.md), `docs/sim.md` "Damage and breakage", `docs/content-model.md` "Parts", `docs/arena.md` "What the example runs found"

## Context

GDR-0019 decided that destroyed reactors, capacitors and drives explode into their neighbours; GDR-0020 deferred it
("an `explosion` block in content; the sim has a hook"). The evolution lab then found that the best 60-build-point
design put its own 1-hit-point fission reactors and slopes in front of its cannons as shutters: the cannons stay
silent as the rules say, the enemies shoot the shutters off, then the cannons fire (52 of 60 fights won, 33–35 with
the guns clear). Niklas (14 Oct 2026): build chain reactions now, so reactor shutters hurt their own ship; slope and
armour shutters stay a legitimate trick (ablative cover). Same day: the archive's firepower axis counts every gun as
if clear and powered (latent firepower), so shuttered designs sit with the other gun-heavy designs.

## Options considered

- **Reach**: the parts linked to it (shared straight edges, like heat and bonuses), or every part with a cell within a
  radius of its centre (chosen: a blast is spatial, the radius is one number in content, and the same rule can serve
  missile splash later).
- **Armour**: no special rule (a steel block has 1 hit point, so a blast that reliably kills a 1-hit-point gun would
  also take the plating), a share per explosion, or a share per armour material (chosen: armour is what resists, and
  a new armour in a mod sets its own).
- **When a chain resolves**: at once inside damage (a projectile's penetration would meet parts a blast just
  removed, mid-weapons pass), or at breakage resolution, all of it in the same tick (chosen), or one link per tick.
- **Falloff**: none in v1 (chosen: one damage number inside the radius, easy to read in the drydock).

## Decision

- A part with an `explosion: (damage, radius)` block explodes when **damage destroys it** (shots, plumes, rams,
  other blasts, scripted damage). Salvage stripping and the pre-breaking of spawned wrecks remove parts without a
  blast.
- **Reach**: every other part of its **piece** (as the piece was before this tick's breakage) with a cell whose
  nearest point lies within `radius` metres of the part's centre (blueprint frame, 1 cell = 1 m). Nothing blocks
  it; other pieces and other ships are not hit.
- **Damage**: `damage` once to each part in reach, the same at any distance, times the target's share: a hull part's
  armour `blast_factor`, every other part 1.
- **Shields** do not help: the blast goes off inside the ship's own bubbles.
- **Order**: chains resolve at breakage resolution (after the inputs, after weapons and plumes, after rams), all in
  the same tick: a queue over the destroyed parts, first destroyed first; each blast hits its targets in ascending
  part order; parts it destroys join the queue. Then fragments, events and splits as before. Deterministic: integer
  part order, IEEE `+ − ×` only, the target lists compiled once per design.
- **Event**: `SimEvent::Explosion { ship, piece, part, at, radius, damage }` right after the part's `PartDestroyed`
  (whose `volatile` now means "has an explosion"). The client does not draw it yet.
- **Numbers** (placeholders, lead defaults):

| Part | Damage | Radius | Reaches |
|---|---|---|---|
| Fission reactor (1 hp) | 1.5 | 1 m | the 8 cells around it |
| Fusion reactor (2 × 2, 4 hp) | 2.5 | 1.5 m | the 12 cells around it (not the far corners) |
| Torch drive (2 × 2, 4 hp) | 1.2 | 1.5 m | the 12 cells around it |
| Capacitor (1 hp) | 0.5 | 0.6 m | the 4 cells beside it |
| Thruster (1 hp) | 0.5 | 0.6 m | the 4 cells beside it |

  Armour `blast_factor`: steel, composite and automaton plate 0.5; reactive 0.25. So a fission reactor takes every
  1-hit-point machine around it (guns, logic cores, gyros, radiators, shield emitters, capacitors, thrusters, another
  fission reactor: a chain) and leaves armour blocks standing (steel 1 → 0.25 hp; 1 × 1 slopes of 0.5–0.7 hp go, reactive ones stay);
  beam lasers and automaton cores (2 hp) survive it. A capacitor's or thruster's blast kills nothing on its own
  (half a hit point) but adds up. The automaton core (it makes power but is a core), RCS pods, gyros and the
  rest do not explode.

## Consequences

- Reactor shutters now destroy the guns they cover: the shuttered design wins 12 of 60 instead of 52. The design run
  now evolves front cannons with reserve guns behind them (lasers behind the cannons, a control load above the
  capacity) and keeps its reactors to the sides: 59 of 60 (`docs/arena.md`).
- Voyages got a little harder for the starter (goal cleared and home of 120: 67 / 37 / 21 / 8 → 59 / 36 / 15 / 4);
  the Runner moved within the noise except `corran_reach` (89 → 70) (`docs/sim.md` "Voyages").
- Every hash stream changed; native and wasm32 stay identical.
- Follow-ups: a blast visual in the client (`SimEvent::Explosion`); the drydock could show blast reach and warn about
  a reactor next to a logic core or a gun.

## Open (for Niklas)

- Should blasts reach other pieces and ships nearby (a rammer, an escort, a wreck alongside), with shields absorbing
  them there?
- Should armour between the blast and a part shield it (line of sight in the grid), and should damage fall off with
  distance?
- Which other parts explode: the automaton core (a power plant), RCS pods, gyros, shield emitters?
- Is the armour share right as a material property, and should 1 × 1 slopes (half a cell of hit points) survive a
  reactor too?
- Overheating that ends in a blast (a meltdown) is not part of this.
