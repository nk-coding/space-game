---
title: "The galaxy"
description: "How a game's galaxy is generated - bands, clusters, home, gates, trails of signals and their sources, lore - the jump range that limits where you go, blind jumps, following a signal and decoding logs."
---

# The galaxy

Every game has its own galaxy, generated once from the game's seed and kept in its save, so it never changes under
you. It is mostly dark: you find places by following **signals**, or by jumping blind into space nearby. This page
describes how the galaxy is put together.

The voyage map is this galaxy: home, the clusters you have charted, the signals you have heard and how far your
jumps reach ([voyages](/mechanics/voyages) has the map itself). The first four voyages of the rim are **set pieces**
placed into every galaxy, each on a rim cluster that takes its name, from [[voyage:yard_approach]] close to home out to
[[voyage:wardens_gate]]; everything else you find by jumping.

## Distances: S

Distances on the galaxy map are measured in **S** ([[stat:galaxy_distance]]),
about the distance between two neighbouring clusters. No two clusters lie closer than
[[galaxy:galaxy.spacing]]. Inside a cluster, distances are metres as usual.

## Bands and clusters

[[galaxy:galaxy]] is a disc of [[galaxy:galaxy.bands]] bands around the centre: [[band:rim]] (out to
[[band:rim.edge]] from the centre), [[band:outer]], [[band:middle]], [[band:inner]] and [[band:core]]. Each band is a
ring filled with clusters at the galaxy's spacing, so there is no empty stretch wider than about one spacing. The rim
and the outer reach are made so far; the bands past them are stand-ins until their races and guardians exist.

A cluster is one place you fly a voyage in: one space a couple of kilometres across. Its **cluster type** says what
kind of place it is and what it holds:

- [[cluster_type:wreck_drift]] ([[cluster_type:wreck_drift.radius]]): old hulls and the raiders that pick them over;
- [[cluster_type:hunting_reach]] ([[cluster_type:hunting_reach.radius]]): open space where hunters run in packs;
- [[cluster_type:quiet_field]] ([[cluster_type:quiet_field.radius]]): a still field of wreckage, little to guard it.

The outer reach is the [[faction:grown]]'s ([living parts](/mechanics/living-parts)):

- [[cluster_type:grove_reach]] ([[cluster_type:grove_reach.radius]]): the Grown's groves, calm until provoked;
- [[cluster_type:scorched_deep]] ([[cluster_type:scorched_deep.radius]]): groves beside a hot star whose heat soaks
  into every part open to space, and [[encounter:scorched_wrecks]] as near the star as the cluster goes;
- [[cluster_type:border_drift]] ([[cluster_type:border_drift.radius]]): only near the rim's border, where automaton
  stragglers run into the first groves.

What a cluster holds is rolled from its own seed the first time a fleet warps in, so it is the same every time you
come back.

## Home and the jump range

Home, the shipyard, is a cluster at the rim's outer edge with at least [[galaxy:galaxy.home_neighbours]] other rim
clusters within reach that hold no set piece: somewhere to jump blind from the start. Your drive jumps [[galaxy:galaxy.jump_range]] at the start, from home or from any cluster you
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
Gates are hidden: each lies at the end of a trail of its own ([[trail:rim_gate]] in the rim). The rim's first gate is
the one [[encounter:the_warden]] keeps: the first four voyages are its trail, and clearing [[voyage:wardens_gate]] opens
it. The rim's other gates are kept by a [[encounter:gate_sentinel]]. The outer reach's gates ([[trail:outer_gate]]) are
kept by a precursor guardian of their own, the [[ship:automaton_bastion]] ([[encounter:bastion_gate]]); the Grown's
giants are no gate guardians but rest at an [[encounter:elder_grove]] at the end of a trail of their own.

## Trails and signals

A **trail** is a chain of signals that ends at something worth the trip. Each signal points from where it was found to
the next place of the trail; the last place holds the reward:

| Trail | Signals | Leads to |
|---|---|---|
| [[trail:rim_gate]] | [[trail:rim_gate.length]] | a gate and its guardian, a [[encounter:gate_sentinel]]: beaten, the gate opens and a [[material:gate_shard]] goes into store |
| [[trail:precursor_cache]] | [[trail:precursor_cache.length]] | a [[encounter:sealed_cache]]: beat its keepers and research data and programmable matter go straight into store, beside rich wrecks |
| [[trail:graveyard]] | [[trail:graveyard.length]] | a [[encounter:deep_graveyard]] with far more salvage than usual |
| [[trail:outer_gate]] | [[trail:outer_gate.length]] | an outer gate and its guardian, a [[encounter:bastion_gate]]: beaten, the gate opens and a [[material:gate_shard]] goes into store |
| [[trail:elder_grove]] | [[trail:elder_grove.length]] | an [[encounter:elder_grove]]: a [[ship:grown_elder]] at rest, rich salvage if it can be beaten |
| [[trail:memory]] | [[trail:memory.length]] | a [[encounter:memory_wreck]]: see it and its fragment of what happened joins the memory log |

Following a trail to its end gives its reward once, even if the voyage is lost on the way home: a gate, a cache's
store and a memory are knowledge, not haul. A banner says so in flight, and the debrief lists it.

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
area shrinks it to [[galaxy:galaxy.signal_narrow]] of its radius towards the place (NARROWED; the old edge fades out), still holding it, and once no other
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

A **signal's place** (a trail's place in the cluster that a signal you have heard leads to) is quieter: it shows only within [[sim:signal_reveal]] of one of your vessels, or under a scan.

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
where you came in, what your scans swept, the bearings, the sites found, the rock you came near, the ships within your
sensors and your fleet; the minimap in the corner ([[control:minimap.keys]]) shows the same round your ship. A cluster has an edge
[[sim:edge_margin]] outside its radius (never more than [[sim:max_edge]] from its centre): beyond it your ship is
pushed back.

## Between the sites: rock, debris and what lies far off

Each kind of cluster has its own **scenery**, listed on its page. **Asteroid rock** is real: fields, belts and
scattered rocks of [[rock:iron_rock]], [[rock:ice_rock]] or, beside a hot star, [[rock:slag_rock]]. A rock never
moves. It stops ships, shots and engine plumes, so it is **cover**: put one between you and a gun. Hitting one hurts
like any ram, scaled by the rock ([[rock:iron_rock.ram]] for iron rock, [[rock:ice_rock.ram]] for ice), and a fast ship
loses parts: fly round it. Ships under a behavior steer round rocks by themselves, and the pilot assist
[[behavior_node:avoid_collisions]] sidesteps them. Rock is made of [[material:nickel_iron]] or [[material:water_ice]];
mining it comes later.

The rest is only seen, never in the way: dust and glints, bits of hull drifting about (thicker round the sites),
dead hulls drifting along the way, a landmark far off such as a [[landmark:derelict_station]], beside a hot star its
[[landmark:star_corona]] at the edge of your view, and now and then a fight flashing in the distance.

Far behind it all every cluster has its own **sky**: a planet, a few moons, a nebula and a distant galaxy. The kind
of cluster decides which planets come up: rock and ice giants at the rim, gardens like the [[planet:garden]] in the
outer reach, worlds like the [[planet:lava]] beside a hot star. A cluster keeps its sky every time you come back.

## Lore

Lore trails end at a fragment of the story, kept in the memory log, which the **LOG** at home shows under MEMORY: in
the rim [[lore:last_launch]], [[lore:the_quiet]] and [[lore:a_note_to_myself]]. Each fragment is placed once per
galaxy.
