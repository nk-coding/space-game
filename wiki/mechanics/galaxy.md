---
title: "The galaxy"
description: "How a game's galaxy is generated - bands, clusters, home, gates, trails of signals and their sources, lore - and the jump range that limits where you go. Coming: the voyage map does not use it yet."
---

# The galaxy

Every game has its own galaxy, generated once from the game's seed and kept in its save, so it never changes under
you. It is mostly dark: you find places by following **signals**, or by jumping blind into space nearby. This page
describes how the galaxy is put together.

::: info Coming
The galaxy is generated but not played yet. The voyage map still shows the first four voyages of the rim
([voyages](/mechanics/voyages) has those); flying into the generated galaxy, scanning and following signals come next.
:::

## Distances: S

Distances on the galaxy map are measured in **S** ([[stat:galaxy_distance]]),
about the distance between two neighbouring clusters. No two clusters lie closer than
[[galaxy:galaxy.spacing]]. Inside a cluster, distances are metres as usual.

## Bands and clusters

[[galaxy:galaxy]] is a disc of [[galaxy:galaxy.bands]] bands around the centre: [[band:rim]] (out to
[[band:rim.edge]] from the centre), [[band:outer]], [[band:middle]], [[band:inner]] and [[band:core]]. Each band is a
ring filled with clusters at the galaxy's spacing, so there is no empty stretch wider than about one spacing. Only the
rim is fully made so far; the bands past it are stand-ins until their races and guardians exist.

A cluster is one place you fly a voyage in: one space a couple of kilometres across. Its **cluster type** says what
kind of place it is and what it holds:

- [[cluster_type:wreck_drift]] ([[cluster_type:wreck_drift.radius]]): old hulls and the raiders that pick them over;
- [[cluster_type:hunting_reach]] ([[cluster_type:hunting_reach.radius]]): open space where hunters run in packs;
- [[cluster_type:quiet_field]] ([[cluster_type:quiet_field.radius]]): a still field of wreckage, little to guard it.

What a cluster holds is rolled from its own seed the first time a fleet warps in, so it is the same every time you
come back.

## Home and the jump range

Home, the shipyard, is a cluster at the rim's outer edge with at least [[galaxy:galaxy.home_neighbours]] other rim
clusters within reach. Your drive jumps [[galaxy:galaxy.jump_range]] at the start, from home or from any cluster you
have charted; research will grow it. A jump takes [[galaxy:galaxy.warp]] of distance.

Every cluster of a band can be reached from where the band is entered, one jump at a time within the starting range:
the generator checks it for every galaxy.

## Gates

Each band but the core has [[band:rim.gates]] **gates** near its inner edge, spread around the disc. A gate is a
cluster with an old guardian; beat it and the gate opens the next band inwards. Any one gate of a band opens the next.
Gates are hidden: each lies at the end of a trail of its own ([[trail:rim_gate]] in the rim).

## Trails and signals

A **trail** is a chain of signals that ends at something worth the trip. Each signal points from where it was found to
the next place of the trail; the last place holds the reward:

| Trail | Signals | Leads to |
|---|---|---|
| [[trail:rim_gate]] | [[trail:rim_gate.length]] | a gate and its guardian, [[encounter:the_warden]] |
| [[trail:precursor_cache]] | [[trail:precursor_cache.length]] | a precursor cache: a research node or a part found nowhere else |
| [[trail:graveyard]] | [[trail:graveyard.length]] | a [[encounter:core_graveyard]] with far more salvage than usual |
| [[trail:memory]] | [[trail:memory.length]] | a fragment of what happened, for the memory log |

A signal says roughly where and what kind of thing: on the map it marks an area [[galaxy:galaxy.signal_area]] across
that holds the place it leads to, though not in its middle. Its first line names what it leads to
(<span class="sig">SIGNAL</span> and the trail's hint, such as "[[trail:rim_gate.hint]]"), its second what found it. Signals come from:

- [[signal_source:hauler_log]] and [[signal_source:precursor_log]]: a data core in a wreck ("[[signal_source:hauler_log.line]]").
  Salvage it, bring it home and it is decoded into a signal for free.
- [[signal_source:old_beacon]] and [[signal_source:distress_call]]: something calling in the cluster
  ("[[signal_source:old_beacon.line]]"). Scan it and it is a signal at once.

A trail's first source lies in an ordinary cluster of its band; each later one waits at the place the signal before
it led to, so following a trail walks you across the band, a [[galaxy:galaxy.trail_step]] step at a time, towards its
end. No two rewards share a cluster.

## Lore

Lore trails end at a fragment of the story, kept in the memory log: in the rim [[lore:last_launch]],
[[lore:the_quiet]] and [[lore:a_note_to_myself]]. Each fragment is placed once per galaxy.
