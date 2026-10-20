---
title: "Blinks, lances and the Phase walkers"
description: "The blink drive that jumps a ship a short way, the lance that runs through a hull, and the Phase walkers of the inner band who strike with both."
---

# Blinks, lances and the Phase walkers

The [[faction:walkers]] share the inner band with the [[faction:lattice]], one race to a cluster, never both. They are
wary: a walker site waits unseen until it is provoked. Then its ships **blink** in behind you, run their **lances**
through your hulls and blink out again before the fight turns, coming back once their drives have cooled. Their
salvage gives research data to [[family:walkers]], which teaches you both.

## The blink drive

A [[part:blink_drive]] jumps its ship a short way, keeping its speed, heading and spin.

- **It charges first.** For [[part:blink_drive.blink_charge]] at full power the hull shimmers in the walkers' colour,
  a warning you can read; the drive draws its power only then, and charges slower on less. Then the ship jumps up to
  [[part:blink_drive.blink_range]] and the drive cools down for [[part:blink_drive.blink_cooldown]].
- **It carries so many tonnes.** Each drive carries [[part:blink_drive.blink_mass]]. A blink takes as many cool drives
  as the ship's mass needs, and all of them cool down; with too few, there is no blink at all. The drydock warns
  TOO HEAVY TO BLINK.
- **It needs a clear spot.** The jump passes through anything, but it lands only where no ship, wreck or rock lies
  within the ship's size and [[sim:blink_clearance]] more. Where the spot is taken it lands shorter, down to
  [[sim:blink_shortest]] of the jump; where every spot is taken it fizzles, and the drives cool
  [[sim:blink_fizzle]] of their cooldown.
- **On the controlling piece.** Only the drives on the piece with the vessel's cores count. A piece that breaks away
  with a core and a drive of its own blinks on its own.

Fly it with [[control:blink]] ([[control:blink.keys]], or the BLINK button above FIRE on a touch screen): the ship
jumps along your stick or the thrust keys you hold, else straight ahead. A behavior's [[behavior_node:blink|blink]] rule jumps `behind` the target (to a spot
[[sim:blink_behind]] past its tail, as far as the drives reach) or `away` from it; pair it with
[[behavior_node:blink_ready]], so the rule holds only when it can act, as the walkers do:

```
when blink_ready and target_in_range(70m) -> blink(away)
when blink_ready and target_in_range(180m) -> blink(behind)
when target_in_range(100m) -> approach(25m)
always -> kite(160m)
```

## The lance

A [[part:phase_lance]] fires slow pulses of [[part:phase_lance.damage]] at [[part:phase_lance.range]]. Each one strikes
the part it hits and then **every part behind it on the same line**, through the whole piece: after each cell of a
machine the shot keeps [[part:phase_lance.lance_keep]] of its strength, after each cell of armour
[[part:phase_lance.lance_armor]]. It stops where the line leaves the hull. A lance that enters by your armour and runs
on into a reactor and a core can take all three; a second row of plate halves what reaches them.

## The walkers

| Ship | What it does |
|---|---|
| [[ship:walker_skirmisher]] | blinks in behind its target, lances it until its drive is ready again, blinks away, waits and strikes again |
| [[ship:walker_striker]] | two lances and two drives; it stays in the fight until its hull hurts, and one drive shot off grounds it |
| [[ship:walker_colossus]] | their boss: four sections, each with its own [[part:phase_core]] and drive; whole, a blink takes all four drives; shot apart, every section blinks in on its own |

**Fighting them**: keep your reactors and cores behind more than one row of plate, watch for the shimmer and turn to
meet the ship that is about to land behind you, and shoot the drives: a walker without one has to stay and fight.

Their parts leave [[material:phase_residue]] (a drive) and [[material:rift_silt]] (a core). [[research:phase_lances]]
unlocks the lance and installs the [[facility:phase_chamber]] that makes their parts; [[research:blink_drive]] unlocks
the drive.
