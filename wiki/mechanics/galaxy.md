---
title: "The galaxy"
description: "How a game's galaxy is generated - bands, clusters, home, gates, trails of signals and their sources, lore - the jump range that limits where you go, blind jumps, following a signal and decoding logs."
---

# The galaxy

Every game has its own galaxy, generated once from the game's seed and kept in its save, so it never changes under
you. It is mostly dark: you find places by following **signals**, or by jumping blind into space nearby. This page
describes how the galaxy is put together.

The voyage map is this galaxy: home, the clusters you have charted, the signals you have heard and how far your
jumps reach ([voyages](/mechanics/voyages) has the map itself). The first four voyages of the rim sit on clusters of it;
everything else you find by jumping.

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
have charted. A cluster is **charted** once a fleet of yours has arrived there, even if that voyage is lost. Research
in the propulsion family grows the range: [[research:jump_calibration_1]] and [[research:jump_calibration_2]] add
[[research:jump_calibration_1.unlocks]] each. A jump takes [[galaxy:galaxy.warp]] of distance.

Every cluster of a band can be reached from where the band is entered, one jump at a time within the starting range:
the generator checks it for every galaxy. Jumps reach only the bands a gate has opened: the rim at first.

On the map the jump range is drawn as soft teal circles round home and every charted cluster (only their outer
edge). A jump starts from the nearest of them; the warp runs on the map before your fleet arrives, and Esc ends it
before anything launches.

## Blind jumps

Most of the galaxy is dark: clusters you haven't charted are not drawn. Click a spot of unknown space within your
range and the panel offers a **blind jump**: it goes to the cluster nearest that spot (within about one and a half
clusters' spacing; farther is EMPTY SPACE). It is an ordinary cluster of its band: patrols, wrecks, now and then a
beacon or a data core that starts a trail. Arriving charts it, even if the voyage is lost.

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
  Salvage it, bring it home and it is decoded into a signal for free. A data core takes one unit of your hold, and
  your ships take it before anything else there; if the voyage is lost it stays in its wreck, and waits there for
  your next visit.
- [[signal_source:old_beacon]] and [[signal_source:distress_call]]: something calling in the cluster
  ("[[signal_source:old_beacon.line]]"). Scan it and it is a signal at once.

On the map a signal you have heard is a gold area: a dotted edge, a faint fill, a slow pulse running out to the edge,
and its two lines over it. A signal you have heard **narrows** as you search: a scan in another cluster inside its
area halves the area towards the place (NARROWED; the old edge fades out), still holding it, and once no other
cluster lies inside, the signal is **LOCATED**: you know the cluster, and the map marks it in gold. A scan in the
place's own cluster locates it at once. One voyage narrows a signal once: scanning again in the same cluster tells
you nothing new about where else to look.

In the place's cluster its site is hidden until you come close or a scan sweeps it; once you see it the signal is
**FOUND**, and when you clear it (or it holds nothing to clear) you have followed it to its end. A voyage keeps what
it found even when it is lost: charts and signals are knowledge, not haul. The debrief lists it all.

A data core you bring home waits at the **LOG** at home (its mast glows): DECODE turns it into a signal, free.

A trail's first source lies in an ordinary cluster of its band; each later one waits at the place the signal before
it led to, so following a trail walks you across the band, a [[galaxy:galaxy.trail_step]] step at a time, towards its
end. No two rewards share a cluster.

## Inside a cluster: sensors and scans

A cluster is dark when you arrive. Your ships see as far as their **sensors** reach: [[part:logic_core.sensor_range]]
for a [[part:logic_core]], and never less than [[sim:always_seen]]. A site shows once one of your vessels has it
within its sensors; behaviors only pick targets they see, so an enemy that gets away beyond the sensors is let go.

A **signal's place** (a trail's place in the cluster, and for now the goal of each of the first four voyages) is
quieter: it shows only within [[sim:signal_reveal]] of one of your vessels, or under a scan.

A **scan** needs a [[part:scanner]] built into the ship ([[part:scanner.build_points]]; the starter ship carries one).
Press [[control:scan.keys]] ([[control:scan]]) or the SCAN pill under the target panel; ships under a behavior scan
with the [[behavior_node:scan]] action. The scanner charges for [[part:scanner.scan_charge]], drawing its
[[part:scanner.power_draw]] only then, and sweeps:

- every site within [[part:scanner.scan_range]] shows;
- every hidden signal's place beyond that range gets a **bearing**: a dashed gold line from where you scanned,
  pointing straight at it. Scan again from somewhere else and the two lines cross at the place;
- sleeping ships within [[part:scanner.scan_wake]] wake up: where you scan matters.

Then it cools down for [[part:scanner.scan_cooldown]]. A banner says what the scan found, and what it did to your
signals. [[control:cluster_map.keys]] ([[control:cluster_map]]) lays the whole cluster over your view: its edge,
where you came in, what your scans swept, the bearings, the sites found and your fleet. A cluster has an edge
[[sim:edge_margin]] outside its radius (never more than [[sim:max_edge]] from its centre): beyond it your ship is
pushed back.

## Lore

Lore trails end at a fragment of the story, kept in the memory log: in the rim [[lore:last_launch]],
[[lore:the_quiet]] and [[lore:a_note_to_myself]]. Each fragment is placed once per galaxy.
