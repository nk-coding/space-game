---
title: "Living parts"
description: "The Grown's living parts: hearts that grow lost blocks back, tissue that heals, the ram prow and the thorn gun, and why a Grown grove stays calm until you provoke it."
---

# Living parts

The [[faction:grown]] of the outer reach fly living ships. Their hulls heal and grow back, they love ramming with
living prows, and they cover the approach with thorn guns. Their parts are not in a new game yet; the drydock offers
them in free mode (`?unlock=all`) until the Grown's research comes.

## Hearts and regrowth

A [[part:grown_heart]] makes its ship grow lost blocks back. While the heart works, one lost part re-forms every
[[part:grown_heart.regrow_interval]]:

- **Nearest the heart first.** The ship grows back from the heart outwards.
- **Only from a living neighbour.** A part re-forms only where it touches a living part of the heart's piece, so a
  block whose neighbours are gone waits until one of them is back.
- **Only what the ship was built with**, in its own place: a part destroyed or stripped. Parts that broke off and
  drift nearby never grow back on (their place waits until the chunk is gone), nor do the parts of a vessel that
  split off. A heart never grows another heart.
- **Never inside something else.** While another hull or wreck lies in the place, the part waits.
- **It starts weak**: with [[part:grown_heart.regrow_hp]] of its hit points, cold, a shield bubble down until it
  recharges.

The heart works only while it is alive, not overheated and on the piece with the ship's logic core. **Kill the heart
and the ship stays broken**; a ship that loses its core is salvage, and its heart does nothing. Two hearts grow
twice as fast. In flight a re-forming part swells softly in the hull's glow.

::: tip Fighting a heart ship
Go for the heart. Every few seconds you leave it alive, a block you shot off comes back.
:::

## Living tissue

[[material:grown_tissue]] is armour that heals: a hull part made of it regains
[[material:grown_tissue.heal_per_cell]] for every cell of its area while it is on a vessel, up to its full hit
points. Salvage does not heal. The [[part:ram_prow]] heals too ([[part:ram_prow.heal]]). A part a heart grew back
heals only if it is tissue itself.

## The ram prow

The [[part:ram_prow]] is built for [ramming](/mechanics/damage#ramming). When it is the part that touches, the other
ship takes [[part:ram_prow.ram_dealt]] the ram damage and the prow's own side [[part:ram_prow.ram_taken]]. Everything
else is the usual ram: below the threshold a bump is free, shields take their share first, what is left crushes
inward.

## The thorn gun

The [[part:thorn_gun]] throws kinetic spines: [[part:thorn_gun.damage]] at only [[part:thorn_gun.range]], fast and
short. Its spines are kinetic, so a [shield bubble](/mechanics/damage#shields) stops only part of their damage.

## Calm until provoked

The Grown are **wary**. A site whose ships are all Grown doesn't wake when you fly close; it wakes when you provoke
it: a shot or a damaging ram on one of its ships, a scan within the scanner's [[part:scanner.scan_wake]], or when you
strip one of its wrecks. See [voyages](/mechanics/voyages#sites).

## The outer reach's hot stars

Some clusters of the outer reach, like the [[cluster_type:scorched_deep]], lie beside a hot star. Its heat comes in
through every side open to space; see [power, heat and control](/mechanics/power-and-heat#hot-stars).
