---
title: "Salvage and cargo"
description: "What counts as salvage, how your ships collect it by flying slowly over it, cargo holds and what they spill, the fragments destroyed cores leave, and how a haul becomes research data and materials at home."
---

# Salvage and cargo

Wrecks are where new parts and research come from. Your vessels strip them part by part as they fly slowly over
them, carry the parts home in their holds, and at home the haul is taken apart into research data and materials.

## What is salvage

Anything that is not a vessel can be salvaged:

- **Debris**: a piece that broke off a ship and has no logic core (see
  [pieces and breaking apart](/mechanics/damage#pieces-and-breaking-apart)). Your own debris counts too.
- **Wrecks**: a ship that lost its last logic core. A wreck keeps whatever its hold carried.
- **Derelicts**: the old wrecks waiting at voyage sites. Their cores never wake, so you can take them intact.
- **Loose items**: the fragments destroyed cores leave, and cargo spilled from a hold.

Salvage belongs to nobody. The target panel calls it SALVAGE, guns hit it (except the guns of the ship it broke off
from), and ramming it hurts. A vessel
that is only DARK (no power) or DORMANT is still a vessel: you can't strip it until its last core is gone.

## Collecting

You don't press anything to collect. Every **running** vessel of yours with room in its hold takes one item at a
time, [[sim:salvage_rate]] at most, from salvage that is:

- within [[sim:salvage_reach]] of its hull (hull to hull, not centre to centre), and
- moving at most [[sim:salvage_max_speed]] relative to it.

So fly slowly over a wreck, or alongside it at its speed. Escorts and other vessels you don't pilot collect too; a
dark or dormant vessel doesn't, and enemies never collect.

<svg class="diagram" viewBox="0 0 520 200" role="img" aria-label="The collect reach around your hull: a wreck that reaches into it and a fragment inside it are collected, a wreck outside is not">
  <rect x="140" y="12" width="144" height="168" rx="60" class="s-teal dotted"/>
  <rect x="200" y="70" width="12" height="12" class="f-teal s-teal"/>
  <rect x="212" y="70" width="12" height="12" class="f-teal s-teal"/>
  <rect x="200" y="82" width="12" height="12" class="f-teal s-teal"/>
  <rect x="212" y="82" width="12" height="12" class="f-teal s-teal"/>
  <rect x="200" y="94" width="12" height="12" class="f-teal s-teal"/>
  <rect x="212" y="94" width="12" height="12" class="f-teal s-teal"/>
  <rect x="200" y="106" width="12" height="12" class="f-teal s-teal"/>
  <rect x="212" y="106" width="12" height="12" class="f-teal s-teal"/>
  <rect x="270" y="122" width="12" height="12" class="f-steel s-dim"/>
  <rect x="282" y="122" width="12" height="12" class="f-steel s-dim"/>
  <rect x="294" y="122" width="12" height="12" class="f-steel s-dim"/>
  <rect x="282" y="134" width="12" height="12" class="f-steel s-dim"/>
  <rect x="294" y="134" width="12" height="12" class="f-steel s-dim"/>
  <line x1="276" y1="128" x2="226" y2="104" class="s-gold" marker-end="url(#arrow-gold)"/>
  <circle cx="172" cy="44" r="4" class="f-violet"/>
  <line x1="176" y1="48" x2="198" y2="68" class="s-gold" marker-end="url(#arrow-gold)"/>
  <rect x="420" y="60" width="12" height="12" class="f-steel s-dim"/>
  <rect x="432" y="60" width="12" height="12" class="f-steel s-dim"/>
  <rect x="432" y="72" width="12" height="12" class="f-steel s-dim"/>
  <text x="196" y="136" class="t-teal">your hull</text>
  <text x="296" y="30" class="t-teal">collect reach</text>
  <line x1="294" y1="26" x2="262" y2="34" class="line"/>
  <text x="314" y="132">wreck in reach</text>
  <text x="314" y="148">nearest first</text>
  <text x="20" y="40" class="t-violet">fragment</text>
  <line x1="78" y1="36" x2="166" y2="44" class="line"/>
  <text x="400" y="104">out of reach</text>
</svg>

*Salvage counts once any part of it comes within [[sim:salvage_reach]] of your hull and it moves no faster than
[[sim:salvage_max_speed]] against you.*

What each vessel takes, in order:

1. The nearest **loose item** that fits.
2. Otherwise the nearest salvage piece: a wreck's own **cargo** first (its most valuable item), then the piece's most
   valuable **part**. Value is build points per unit of hold space, so machines usually go before plating.

Only what fits counts: with little room left, a big part waits and smaller ones go first. A stripped part leaves
quietly, with no explosion and no fragment, and the wreck may fall apart further as you take it.

Salvage settles: a wreck or loose item faster than [[sim:settle_speed]] slows down by [[sim:settle_decel]] until it
drifts at that speed, so cargo dropped at full speed is easy to catch.

## Cargo holds

Your hold is the summed [[stat:cargo_capacity]] of the cargo parts on your vessel's main piece:

<StatTable stats="cargo_capacity" />

- A collected part takes as many units of hold space as cells in its footprint (a slope its whole footprint). A
  fragment takes one, and so does a **data core**, a log from a wreck that is decoded at home into a signal (see
  [the galaxy](/mechanics/galaxy#trails-and-signals)). A data core counts as the most valuable thing in a hold:
  collected first, spilled last.
- The flight HUD shows the hold as used / capacity: CARGO in the test range, SALVAGE on a voyage with the name of
  your biggest hold under it, or NO HOLD. The drydock's [[stat:cargo_capacity]] cell shows your design's hold, and
  the voyage-ready line warns when no ship of your fleet has one (you may still launch).
- **A hold that is hit**: when a cargo part is destroyed or breaks off, the hold shrinks, and the least valuable
  items that no longer fit **spill out** where the hold was, pushed off at up to [[sim:spill_speed]]. You can scoop
  them up again.
- **A vessel that is destroyed**: its wreck keeps everything it carried. Your other vessels can collect it back, the
  wreck's cargo first.
- A piece that splits off as a vessel of its own starts with an empty hold; the cargo stays with the vessel that
  keeps the main piece.

## Fragments

A destroyed [[part:logic_core]] or [[part:automaton_core]] leaves a **fragment** of
[[material:programmable_matter]]: a small loose item where the core was, drifting on with the speed it had. It stays
until you collect it or the voyage ends (the test range clears old ones after a while). Only cores destroyed by
damage leave one; a core you collect whole gives research data as well.

## Bringing it home

On a voyage, press [[control:recall.keys]] or the RECALL pill to warp home. Recall works while no armed enemy is
within [[sim:recall_radius]] of your vessels, or as an emergency recall when none of your vessels has a working gun.
The haul is everything in the holds of all your vessels when you warp. If your last vessel is lost instead, the haul
stays behind in the wreck, and your ships re-form at home without it. See [voyages](/mechanics/voyages).

## Dismantling at home

At home the haul is taken apart (VOYAGE HOME · HAUL UNLOADED on the debrief), and the LOG keeps a record of it:

- A **whole part** gives the research data of its kind to its research family, and some parts give materials too.
  A [[part:fission_reactor]] gives [[part:fission_reactor.salvage_data]]; a [[part:logic_core]] gives
  [[part:logic_core.salvage_data]] and [[part:logic_core.salvage_materials]]; an [[part:automaton_core]]
  [[part:automaton_core.salvage_data]] and [[part:automaton_core.salvage_materials]]. Damage doesn't matter: a part
  that came home counts whole.
- A **hull part** gives its armour's data for every cell of hull area, slopes counting half: steel gives
  [[material:steel.salvage_data]] per cell.
- A **fragment** gives its materials: [[part:logic_core.fragment]] from a logic core, [[part:automaton_core.fragment]]
  from an automaton core.

Each part's page lists what it gives under Salvage. Research data and materials pay for research: new parts, armour,
frames and more mind capacity, such as [[research:tamed_core]], which lets you build the [[part:automaton_core]]
yourself. See [research, the fleet and the career](/mechanics/career) and [[family:logic]].
