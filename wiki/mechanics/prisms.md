---
title: "Prisms and the Lattice"
description: "How a prism relays a laser's beam further and harder, how refractive crystal turns lasers away, and the Lattice of the inner band: their lances, their sites and their resonators."
---

# Prisms and the Lattice

The [[faction:lattice]] of the inner band build long glass lances: a laser at the back and a chain of prisms down the
spine, so their beams carry further than their emitters alone. Their armour is a crystal that turns lasers away.
Their salvage gives research data to [[family:lattice]], which teaches you both.

## The prism relay

A [[part:prism]] takes the beam of a laser (a part tagged `laser`: the [[part:pulse_laser]], the
[[part:beam_laser]], the [[part:burst_laser]], the Lattice's [[part:focus_emitter]]) and sends it on from its own
front side.

- **First in line.** The prism must be the first part in the laser's line of fire. The beam may enter by any side but
  the prism's front, and leaves by the front: a prism can turn a beam round a corner.
- **Chains.** A prism's beam can go on into the next prism, and so on, up to [[sim:max_prisms]] prisms. Each prism adds
  its share to the laser's reach ([[part:prism.relay_range]]) and damage ([[part:prism.relay_damage]]) on top of the
  laser's own; the shares add up.
- **The last one needs a clear line of fire**, like any gun. Anything else in the way blocks the laser: a part that is
  no prism, a prism facing back at the beam, a fifth prism, or a prism the beam has passed already.
- **Glass breaks.** A prism draws no power, but it has few hit points. Shot out, the beam crosses the gap to the next
  part in line: on a straight chain that is the next prism (the laser loses that prism's share); where the chain turned,
  the beam goes straight on and usually meets hull, and the laser is blocked.
- **Only lasers.** Cannons, missiles and ion guns are no lasers: a prism in their line of fire blocks them.

The drydock's ARCS overlay draws a relayed beam from the laser through each prism (a diamond) to where it leaves, and
the line of fire from there with the full reach.

::: tip Building a lance
A laser behind two or three prisms in a row makes a long, thin spine: protect its sides, since every prism lost costs
reach. A prism turned sideways lets a laser tucked inside the hull fire forwards.
:::

## Refractive crystal

[[material:refractive_crystal]] is the Lattice's armour. An energy shot (a beam or a laser pulse) that strikes it
glances off: a share of its damage leaves along the mirrored line ([[material:refractive_crystal.refract]]) and the
part takes the rest.

- **The glance is nobody's shot.** It hits whatever it meets: the shooter, the shooter's friends, other Lattice ships.
  Shields take it as they take any energy shot.
- **Angles matter.** A flat crystal face hit head on throws the beam straight back at the gun; a slope or a shot from
  the side sends it away. A glance that meets more crystal glances again, at most [[sim:max_refractions]] times.
- **Only energy.** Shells, missiles and ion pulses do not glance; crystal is a little weaker than steel against them,
  and lets blasts through.

## The Lattice

The Lattice are wary, like the [[faction:grown]]: a Lattice site does not wake when you come in sight. It wakes when
it is provoked (shot at, rammed, scanned close by, its wrecks stripped) or when a ship comes too close to it: first
it **warns**, a ring of light from its ships, and a little closer it wakes.

- [[ship:lattice_needle|Needles]] keep watch in twos and threes: one emitter behind two prisms. They sit at the edge
  of their reach, turn onto you and fire, and back off when you close in.
- A [[ship:lattice_lance]] doubles a needle: two spines of three prisms, two cores.
- A [[ship:lattice_guard]] guards their crystal sites: a flat mirror front that throws lasers back, and it closes in.
- A [[ship:lattice_resonator]], the band's boss, is anchored deep in the band at the end of a trail of its own: three
  spokes of prisms feed one central prism that fires all three beams out of its open mouth. Break the spokes; the
  central prism, seen straight down the mouth, blocks the side spokes when it goes.

Their materials: [[material:refractive_crystal]] (their armour), [[material:resonant_crystal]] (destroyed resonant
cores leave it) and [[material:crystal|Lattice crystal]] (destroyed prisms leave a shard; crystal rock in their
clusters holds it too). [[research:crystal_plating]] gives you their armour, [[research:prism_relay]] the prism and
the [[facility:crystal_annealer]], and [[research:crystal_ward]] the [[part:prism_ward]].
