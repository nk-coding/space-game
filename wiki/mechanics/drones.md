---
title: "Drones and the Swarm"
description: "Hangars that launch drones, how drones fight and die, and the Swarm of the middle band: their carriers, their hives and what their salvage teaches."
---

# Drones and the Swarm

The [[faction:swarm]] of the middle band fight with drones: small, fast ships launched by the dozen from hangars. Their
salvage gives research data to [[family:swarm]], whose last node gives you a hangar of your own.

## Hangars

A hangar holds a number of drones, out and in its bay together, and launches them **while its ship has a target**: a
pilot's lock, or the target its behavior picked. A ship without one keeps its drones in the bay.

- **One at a time.** Drones leave by the hangar's front side, one every [[sim:drone_launch_interval]], facing out,
  with the carrier's speed and a little more.
- **The mouth must be open.** A part right in front of the hangar's front side closes it: no drone leaves. The
  drydock's ARCS overlay shows each hangar's launch lane, a chevron per drone, or a coral cross where a part blocks it.
- **Lost drones are rebuilt.** While the hangar works (powered, not overheated, not idle, on the piece with the ship's
  core), it builds a lost drone again in its rebuild time, slower on less power.
- **A limit for the whole fight.** At most [[sim:max_drones]] drones fly at once, everyone's together; a hangar waits
  while the sky is full.

| Hangar | Drones | Rebuild | Leash |
|---|---|---|---|
| [[part:brood_hangar]] (the Swarm's) | [[part:brood_hangar.drone_capacity]] | [[part:brood_hangar.drone_rebuild]] | [[part:brood_hangar.drone_leash]] |
| [[part:drone_bay]] (yours) | [[part:drone_bay.drone_capacity]] | [[part:drone_bay.drone_rebuild]] | [[part:drone_bay.drone_leash]] |

## Drones

A drone is a vessel of its own, with its own core-equivalent, a [[part:swarm_node]], and it flies the
[[behavior:drone]] behavior: it fights **what its carrier aims at**, circling in close; when the chase takes it more
than its hangar's leash from the carrier, it turns back ([[behavior_node:off_leash]]); with nothing to fight it keeps
near the carrier.

- **They die with their hangar.** Destroy a hangar, or the carrier's last core, and every drone that hangar launched
  dies at once. A drone whose core is shot out dies too. Drones leave no wreck: the carrier's parts are the salvage.
- **They are not the fleet.** Your drones take none of your mind's capacity, never go dormant, are never your focus and
  are not on the fleet bar; they neither block a recall nor count as ships lost.

::: tip Fighting a carrier
A flood cannot be outrun: a drone's [[part:swarm_jet]] is faster than any engine. Thin the flood and dive for a
flank: every drone of that hangar goes with it.
:::

## The Swarm

The Swarm fill the middle band. They are hostile by nature: a Swarm site wakes when a ship comes near.

- [[ship:swarm_spitter|Spitters]] patrol in twos and threes and circle what they find, stinging with
  [[part:stinger|stingers]].
- A [[ship:swarm_carrier]] keeps its distance and floods the space round it from the brood hangars in its flanks.
- A [[ship:swarm_hive]], the band's boss, is anchored deep in the band at the end of a trail of its own: four pods
  round a hub, each with its own nerve node and brood hangar. Shot off, a pod floods on alone.
- Near the outer reach's border the Swarm raid the [[faction:grown]]: their raids leave Grown salvage behind.

Their materials: [[material:chitin]] (their armour), [[material:resin]] (destroyed brood hangars leave it) and
[[material:nerve_core|nerve cores]] (destroyed nerve nodes leave them). [[research:chitin_plating]] gives you their
armour, [[research:stingers]] their gun and [[research:drone_bay]] the drone bay.
