---
title: "Salvage and cargo"
description: "What counts as salvage, how your ships collect it by flying slowly over it or pulling it in with a tractor beam, cargo holds and what they spill, mining ore from rock, the fragments destroyed cores leave, why most parts break, and how the workshop turns a haul into stock, research data and materials."
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

Your escorts collect the same way when their behavior takes them there: the [[behavior_node:salvage]] action flies a
ship slowly over the nearest wreck or loose fragment its hold can take, [[behavior_node:wreck_in_range]] tells whether
there is one near, [[behavior_node:cargo_full]] whether the hold has room and [[behavior_node:cargo]] how full it
is, and [[behavior_node:leader_near]] whether its leader is still close. The [[behavior:salvager]] behavior strips
wrecks between fights this way, only while its leader is near and no enemy is, and keeps near its leader otherwise.
An escort busy with a wreck still helps a little less in the next fight than one that only escorts, so salvaging
escorts bring home more salvage and clear a voyage's goal a little less often.

### Tractor beams

A [[part:tractor_beam]] (the utility research's [[research:tractor_beam]]) saves flying over every piece. It works on
its own while its ship runs and the hold has room: it takes hold of the nearest salvage within
[[part:tractor_beam.tractor_range]] (loose fragments, spilled cargo, mined ore and wreck pieces, never a ship, yours
or an enemy's), [[part:tractor_beam.tractor_beams]] pieces at once, and pulls them towards the ship at up to
[[part:tractor_beam.tractor_speed]], slowing them as they reach the hull, which then collects them as above. Each beam
pulls with [[part:tractor_beam.tractor_force]], so a fragment comes at once and a heavy wreck piece slowly. With one
aboard, the [[behavior_node:salvage]] action stops short of the wreck and lets the beam do the rest, and on a voyage
escorts that carry one fly the [[behavior:salvager]] behavior. The [[ship:sample_salvager]] is a drydock sample with one.

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

## Mining

A [[part:mining_laser]] cuts ore out of asteroid rock. It fires like a gun, from its forward side and with a clear
line of fire, but against ships it barely scratches ([[part:mining_laser.damage]]). Held on a rock, it cuts
[[part:mining_laser.cut]] of ore at full power, and every whole unit goes into the hold as ore of the rock's material:
[[material:nickel_iron]] from [[rock:iron_rock|iron rock]] and [[rock:slag_rock|slag]], [[material:water_ice]] from
[[rock:ice_rock|ice rock]]. A unit of ore takes one unit of hold space and counts as the least valuable thing in a
hold, so a shrinking hold spills it first. With the hold full, the ore drifts loose beside the rock, to be collected
like any loose item.

A rock holds ore by its size, a big one up to a few dozen units; when the last is cut it is spent: it stays where it
is, as cover, drawn darker, and gives nothing more. Several mining lasers on one rock share its ore. Your escorts mine
with the [[behavior_node:mine]] action, as the [[behavior:miner]] behavior does between fights.

At home ore is its material, one unit each, under GAINED in the voyage's report. Researching the
[[research:smelter]] (it takes nickel-iron itself) installs the [[facility:smelter]] in the workshop: whenever a
recipe or a research cost needs more [[material:steel]] or [[material:coolant]] than the store holds, it refines the
rest from the ore in store, two units of ore for one. Coolant goes into the [[part:fusion_reactor]], the
[[part:torch_drive]] and [[research:radiator_vent_3]].

## Fragments

A destroyed [[part:logic_core]] or [[part:automaton_core]] leaves a **fragment** of
[[material:programmable_matter]]: a small loose item where the core was, drifting on with the speed it had. It stays
until you collect it or the voyage ends (the test range clears old ones after a while). Only cores destroyed by
damage leave one; a core you collect gives research data as well. A destroyed [[part:grown_heart]] leaves a fragment
of [[material:heartwood]] the same way.

## Bringing it home

On a voyage, press [[control:recall.keys]] or the RECALL pill to warp home. Recall works while no armed enemy is
within [[sim:recall_radius]] of your vessels, or as an emergency recall when none of your vessels has a working gun.
The haul is everything in the holds of all your vessels when you warp. If your last vessel is lost instead, the haul
stays behind in the wreck, and your ships re-form at home without it. See [voyages](/mechanics/voyages).

## Most parts break

Tearing a part off a wreck usually breaks it: only [[sim:intact_chance]] of the parts you strip come away intact,
the rest come aboard broken. Hull plating is never counted as either; it is taken apart at home anyway. Broken
parts take the same room in the hold, but a hold that shrinks drops them before intact ones.

## At home: the workshop

When the haul comes home (VOYAGE HOME · HAUL UNLOADED on the debrief, and the LOG keeps a record of it):

- A **broken part** is dismantled at once: it gives the research data of its kind to its research family and
  [[facility:workshop.broken]] of the materials in its recipe. A broken [[part:fission_reactor]] gives
  [[part:fission_reactor.salvage]] and a share of [[part:fission_reactor.recipe]].
- A **hull part** gives its armour's data for every cell of hull area, slopes counting half (steel gives
  [[material:steel.salvage_data]] per cell), and [[facility:workshop.broken]] of a unit of its armour per cell.
- A **fragment** gives its materials: [[part:logic_core.fragment]] from a logic core, [[part:automaton_core.fragment]]
  from an automaton core, [[part:grown_heart.fragment]] from a Grown heart.
- An **intact part** comes first in the voyage's report: KEEP puts it into the [[facility:workshop]]'s stock (your
  ships can carry it), DISMANTLE takes it apart for its research data and [[facility:workshop.dismantle]] of its
  recipe's materials (point at DISMANTLE to see what it would give). Keeping is what happens if you do nothing:
  whatever you leave goes into the stock when you dismiss the report, and you can still dismantle it in the
  workshop later. SORT AS SUGGESTED keeps while fewer than [[facility:workshop.keep]] are in stock or your fleet needs
  more and dismantles the rest, in one click.
- Below the intact parts the report sums what the dismantled parts **gained**, research data and materials side by
  side, and shows each dismantled part as an icon; point at one to see what it gave.

Shares are summed over the haul (or over the parts dismantled together) and rounded to whole units. Each part's page
lists what it gives under Salvage and what it is made of under Recipe. Research data and materials pay for research:
new parts, armour, frames and more mind capacity, such as [[research:tamed_core]], which lets you fly the
[[part:automaton_core]] yourself. See [research, the fleet and the career](/mechanics/career) and [[family:logic]].

## Fabricating and the stock

Your ships are fitted from the workshop's stock: a fleet launches only when the stock holds every part its ships
carry (hull plating is not counted). The workshop lists its parts like the drydock: a search, a section per category
(the parts your fleet is short of first) and a part's card when you rest the pointer on its row. The drydock shows each part's stock and what the design is short of. The
workshop fabricates parts from materials: an unlocked part with a recipe, such as a [[part:pulse_laser]] from
[[part:pulse_laser.recipe]]. Some recipes need a workshop upgrade: the [[facility:fusion_workshop]] for the
[[part:fusion_reactor]] and the [[part:torch_drive]], the [[facility:grown_vat]] for the Grown's parts, the
[[facility:munitions_bay]] for the missile and flare launchers, the [[facility:hive_press]] for the Swarm's parts.
The research that unlocks such a part installs its upgrade, except the automatons' own parts: they need the
[[facility:automaton_forge]], which its own research installs (it takes a gate shard), so until then they come only
from intact salvage.

Fitting takes nothing from the stock: the parts come back when a ship comes home, and when a ship is lost its
bonded parts re-form at home with it. Only the haul is at stake.

Parts in stock that your fleet does not carry can be dismantled again for [[facility:workshop.dismantle]] of their
recipe's materials, like an intact part from a haul but without research data: the workshop learns nothing from
its own parts. It never dismantles below what your fleet carries. For MAKE and DISMANTLE you choose how many with − and +
(shift-click: five at a time) or MAX, and the workshop shows what it costs or gives in all before you confirm.

The workshop shows the parts list on the left (every part you can make or have in stock, what your fleet is short
of first, with FABRICATE MISSING for those) and the materials in store on the right; point at a part and the
materials it needs light up beside the store, coral where you have too little.
