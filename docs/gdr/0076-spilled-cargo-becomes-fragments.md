# GDR-0076: Cargo that no longer fits spills as loose items that can be collected again

- Status: Accepted
- Date: 2026-10-14
- Decided by: Niklas (interactively)
- Supersedes: [GDR-0066](0066-phase-7-wave-1-defaults.md), only its rule "A shrinking hold drops the lowest-value items first (they vanish, they are not spilled)"; which items go first stays, and the rest of GDR-0066 stands
- Related: [GDR-0063](0063-salvage-and-research-v1.md), [GDR-0070](0070-phase-7-wave-2-defaults.md) ("A shrinking hold reports what it dropped"), `docs/sim.md` "Salvage"

## Context

A ship's hold is the summed capacity of the alive cargo parts of its controlling piece. When a cargo part is
destroyed or split off, the load can exceed the hold. The wave-1 default (GDR-0066) dropped the lowest-value items
and they vanished: a hit on a cargo bay simply deleted part of the haul, with nothing to see and nothing to do
about it.

## Options considered

- **They vanish** (the GDR-0066 lead default): simplest; but the loss is invisible and final.
- **They spill as loose items near the ship** (chosen): the loss is visible, and the player (or an escort) can fly
  over them and scoop them again while they drift, like a destroyed core's fragment.

## Decision

Niklas, 14 Oct 2026: when a ship's hold shrinks, the items that no longer fit spill as loose items that can be
flown over and collected again, instead of vanishing.

- Which items go stays as before: the lowest-value first, one at a time, until the load fits (GDR-0066).
- Each item that goes becomes one loose item (the same item: a whole part stays a whole part, with its cargo units
  and value), a point with no physics body, like a core's fragment. It starts at the lost hold part (the first cargo
  part destroyed in that breakage; else the controlling piece's centre of mass) with that point's velocity plus a
  small push in a random direction (the sim's debris stream; placeholder speed, `SalvageTuning::spill_speed`).
- Spilled items live as long as fragments (`fragment_ticks`, 60 s) and are collected by the same rules (the
  nearest loose item that fits, slow enough, within reach).
- The hold still reports what it dropped (`SimEvent::CargoLost`, GDR-0070); the haul is what the holds carry when the
  voyage ends, so an item dropped and scooped again counts once.

## Consequences

- A hit cargo bay costs the haul only if nobody goes back for it; a ship with free room may scoop a smaller item it
  just dropped right away (the drop order frees more than that item needs).
- Loose items are now whole parts as well as fragments; the client draws them all the same way for now.
- The salvage and voyage scenarios' hashes change (two draws on the debris stream per spilled item, a new
  setting in the hash).
