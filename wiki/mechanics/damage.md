---
title: "Hull, armour and damage"
description: "Hit points, armour materials, what happens when a part is destroyed, how ships break into pieces, explosions and chain reactions, engine plumes, ramming and shields."
---

# Hull, armour and damage

Ships are built from parts on a grid of 1 m cells, and every part takes damage on its own. Nothing has a shared
health bar: a ship loses parts one by one, and when the parts that hold it together are gone, it breaks apart.

## Parts and hit points

Every part has [[stat:mass]] and [[stat:hp|hit points]]. A machine (a reactor, a gun, an engine) has its own
values, for example [[part:fission_reactor.hp]] for a [[part:fission_reactor]] and [[part:logic_core.hp]] for a
[[part:logic_core]]. Damage takes hit points away; at zero the part is **destroyed**.

Damage comes from guns ([weapons](/mechanics/weapons)), explosions, engine plumes and rams, all explained below.

## Hull and armour

Hull parts ([[part:armor_block]], [[part:slope_1x1]], [[part:slope_1x2]], [[part:slope_1x3]]) have no numbers of
their own: you pick an armour material for each one in the drydock, and the material sets its weight, toughness and
cost per cell.

| Material | [[stat:mass_per_cell]] | [[stat:hp_per_cell]] | [[stat:build_point_factor]] | [[stat:blast_factor]] |
|---|---|---|---|---|
| [[material:steel]] | [[material:steel.mass_per_cell]] | [[material:steel.hp_per_cell]] | [[material:steel.build_point_factor]] | [[material:steel.blast_factor]] |
| [[material:composite]] | [[material:composite.mass_per_cell]] | [[material:composite.hp_per_cell]] | [[material:composite.build_point_factor]] | [[material:composite.blast_factor]] |
| [[material:reactive]] | [[material:reactive.mass_per_cell]] | [[material:reactive.hp_per_cell]] | [[material:reactive.build_point_factor]] | [[material:reactive.blast_factor]] |
| [[material:automaton_plate]] | [[material:automaton_plate.mass_per_cell]] | [[material:automaton_plate.hp_per_cell]] | [[material:automaton_plate.build_point_factor]] | [[material:automaton_plate.blast_factor]] |

- A hull part's mass and hit points are the material's value per cell times the part's area. **Slopes count half**
  their footprint: a [[part:slope_1x1]] in steel has [[part:slope_1x1@steel.hp]], an [[part:armor_block]]
  [[part:armor_block@steel.hp]].
- Its build points are the part's own times the material's build point factor.
- Its blast factor is the share of an explosion it takes, and the share it lets through to the parts behind it
  (see [explosions](#explosions-and-chain-reactions)).
- A slope's angled face makes kinetic shots that come in at a shallow angle glance off
  ([ricochets](/mechanics/weapons#ricochets)).

## When a part is destroyed

A destroyed part is gone from the grid at once, and everything around it notices:

- Its neighbours gain open sides: a radiator next to it may start venting, a part beside it loses heat faster.
- Bonuses it gave are gone: a laser that lost its capacitor fires slower.
- A gun it blocked has a clear line of fire again; an engine plume it stopped now reaches further.
- If it was a **volatile** part, it explodes ([below](#explosions-and-chain-reactions)).
- A destroyed logic core or automaton core leaves a fragment of [[material:programmable_matter]] to collect (see
  [salvage and cargo](/mechanics/salvage-and-cargo#fragments)).
- A destroyed cargo part shrinks the hold, and what no longer fits spills out.
- The piece may break apart.

## Pieces and breaking apart

Parts join into a **piece** only through **straight sides**. A slope joins its neighbours through its two straight
sides, never through its angled face. Touching corners don't join anything. Each piece is one rigid body with its
own power pool ([power, heat and control](/mechanics/power-and-heat)).

When destroyed parts leave a ship in several groups, it **splits**. Every new piece keeps flying with the speed and
spin it had, as part of the old body. What becomes of each:

- The piece with a logic core (with several, the one with more parts) stays the vessel you had.
- Any other piece with an intact logic core becomes a **vessel of its own**: still yours (or the enemy's), with the
  same behavior, and with only the rule slots and control capacity of its own cores.
- A piece without a logic core is **salvage**: it belongs to nobody, its systems are dark, its hull doesn't glow,
  and you can collect its parts (see [salvage and cargo](/mechanics/salvage-and-cargo)).

When a vessel loses its last logic core, the whole ship is salvage for good: a wreck. A vessel whose cores are only
unpowered or overheated is not salvage: it is DARK and still owned, and it wakes when its cores work again.

In the drydock a part that isn't joined to the main piece is marked FALLS OFF, and the status line says
A PART FALLS OFF: it would drift away the moment the ship launches.

## Explosions and chain reactions

Some parts are **volatile**, the ones in the table below: when damage destroys one, it explodes. The blast deals
its [[stat:blast_damage]] once to every part, on any ship, that has a cell within its [[stat:blast_radius]] of the
exploding part's centre, measured to the nearest point of that cell.

<StatTable stats="blast_damage,blast_radius" />

<svg class="diagram" viewBox="0 0 560 196" role="img" aria-label="Cells a blast reaches on the grid for a small blast, a reactor blast and the blast of a big machine">
  <g transform="translate(20 20)">
    <rect x="0" y="0" width="126" height="126" class="f-bg s-dim"/>
    <rect x="54" y="36" width="18" height="18" class="f-coral"/>
    <rect x="54" y="72" width="18" height="18" class="f-coral"/>
    <rect x="36" y="54" width="18" height="18" class="f-coral"/>
    <rect x="72" y="54" width="18" height="18" class="f-coral"/>
    <rect x="54" y="54" width="18" height="18" class="f-gold"/>
    <line x1="18" y1="0" x2="18" y2="126" class="line"/>
    <line x1="0" y1="18" x2="126" y2="18" class="line"/>
    <line x1="36" y1="0" x2="36" y2="126" class="line"/>
    <line x1="0" y1="36" x2="126" y2="36" class="line"/>
    <line x1="54" y1="0" x2="54" y2="126" class="line"/>
    <line x1="0" y1="54" x2="126" y2="54" class="line"/>
    <line x1="72" y1="0" x2="72" y2="126" class="line"/>
    <line x1="0" y1="72" x2="126" y2="72" class="line"/>
    <line x1="90" y1="0" x2="90" y2="126" class="line"/>
    <line x1="0" y1="90" x2="126" y2="90" class="line"/>
    <line x1="108" y1="0" x2="108" y2="126" class="line"/>
    <line x1="0" y1="108" x2="126" y2="108" class="line"/>
    <circle cx="63" cy="63" r="10.8" class="dotted"/>
    <text x="0" y="150">small blast</text>
  </g>
  <g transform="translate(210 20)">
    <rect x="0" y="0" width="126" height="126" class="f-bg s-dim"/>
    <rect x="36" y="36" width="54" height="54" class="f-coral"/>
    <rect x="54" y="18" width="18" height="18" class="f-coral"/>
    <rect x="54" y="90" width="18" height="18" class="f-coral"/>
    <rect x="18" y="54" width="18" height="18" class="f-coral"/>
    <rect x="90" y="54" width="18" height="18" class="f-coral"/>
    <rect x="54" y="54" width="18" height="18" class="f-gold"/>
    <line x1="18" y1="0" x2="18" y2="126" class="line"/>
    <line x1="0" y1="18" x2="126" y2="18" class="line"/>
    <line x1="36" y1="0" x2="36" y2="126" class="line"/>
    <line x1="0" y1="36" x2="126" y2="36" class="line"/>
    <line x1="54" y1="0" x2="54" y2="126" class="line"/>
    <line x1="0" y1="54" x2="126" y2="54" class="line"/>
    <line x1="72" y1="0" x2="72" y2="126" class="line"/>
    <line x1="0" y1="72" x2="126" y2="72" class="line"/>
    <line x1="90" y1="0" x2="90" y2="126" class="line"/>
    <line x1="0" y1="90" x2="126" y2="90" class="line"/>
    <line x1="108" y1="0" x2="108" y2="126" class="line"/>
    <line x1="0" y1="108" x2="126" y2="108" class="line"/>
    <circle cx="63" cy="63" r="27" class="dotted"/>
    <text x="0" y="150">reactor blast</text>
  </g>
  <g transform="translate(400 20)">
    <rect x="0" y="0" width="144" height="144" class="f-bg s-dim"/>
    <rect x="18" y="18" width="108" height="108" class="f-coral"/>
    <rect x="0" y="54" width="18" height="36" class="f-coral"/>
    <rect x="126" y="54" width="18" height="36" class="f-coral"/>
    <rect x="54" y="0" width="36" height="18" class="f-coral"/>
    <rect x="54" y="126" width="36" height="18" class="f-coral"/>
    <rect x="54" y="54" width="36" height="36" class="f-gold"/>
    <line x1="18" y1="0" x2="18" y2="144" class="line"/>
    <line x1="0" y1="18" x2="144" y2="18" class="line"/>
    <line x1="36" y1="0" x2="36" y2="144" class="line"/>
    <line x1="0" y1="36" x2="144" y2="36" class="line"/>
    <line x1="54" y1="0" x2="54" y2="144" class="line"/>
    <line x1="0" y1="54" x2="144" y2="54" class="line"/>
    <line x1="72" y1="0" x2="72" y2="144" class="line"/>
    <line x1="0" y1="72" x2="144" y2="72" class="line"/>
    <line x1="90" y1="0" x2="90" y2="144" class="line"/>
    <line x1="0" y1="90" x2="144" y2="90" class="line"/>
    <line x1="108" y1="0" x2="108" y2="144" class="line"/>
    <line x1="0" y1="108" x2="144" y2="108" class="line"/>
    <line x1="126" y1="0" x2="126" y2="144" class="line"/>
    <line x1="0" y1="126" x2="144" y2="126" class="line"/>
    <circle cx="72" cy="72" r="36" class="dotted"/>
    <text x="0" y="168">big machine</text>
  </g>
</svg>

*The exploding part is gold, the cells its blast reaches are coral. Left: a part with
[[part:capacitor.blast_radius]] of reach, like the [[part:capacitor]] or the [[part:thruster]]. Middle:
[[part:fission_reactor.blast_radius]], the [[part:fission_reactor]]. Right: [[part:fusion_reactor.blast_radius]]
from the centre of a big part, the [[part:fusion_reactor]].*

- **Armour shields what lies behind it.** A hull part takes only its material's blast factor of the damage. Every
  intact hull part that the straight line from the blast to the target crosses also lets only its blast factor
  through. Machines in between shield nothing: a reactor in front of a gun is no cover for it. There is no falloff
  with distance.
- **Chain reactions.** A part the blast destroys explodes in turn, in the same instant. A reactor next to another
  reactor takes it with it; machines with low hit points around a reactor usually go too, while the plating
  around it usually survives.
- **Blasts spare no one.** They hit your own ship, a vessel alongside (yours too), wrecks and debris.
- **Shields.** Your own ship's shields never help: the blast goes off inside them. Another ship's bubble between
  the blast and its hull takes its share first, as it would take a missile's.

Only parts destroyed by damage explode. Parts you collect as salvage leave quietly.

## Engine plumes

An engine's exhaust reaches [[stat:plume_length]] straight back from the engine and burns the **first part in its
path**, with [[stat:plume_damage]] and [[stat:plume_heat]] per second at full thrust, less at lower thrust. Engines
may sit anywhere, but a part behind one burns, your own included. Once that part is gone, the plume reaches the next.

- The plume burns your own parts, enemies, wrecks and debris, but never your other vessels.
- Your own shields never stop your own plume. An enemy's bubble covering the point it burns takes the plume's
  damage first, as it takes an energy shot.
- A wide engine has one lane of exhaust per cell of its back side. The lanes share the plume's damage and heat, and
  each burns the first part in its own path.

The drydock's ARCS overlay draws every plume to where it stops, in coral where it burns your own part, and the
status line warns A PLUME BURNS THE SHIP.

<StatTable stats="plume_length,plume_damage,plume_heat" />

## Ramming

When two hulls hit each other, the push of the contact decides what happens. Below [[sim:ram_threshold]] in one
instant it is a free bump. Above it, both sides take the same damage: [[sim:ram_damage]] for every unit of push
over the threshold. Armour and hit points decide who breaks.

- Shield bubbles that cover the contact point take their share first, as they take a kinetic shot.
- What is left hits the part that touches. What it doesn't need to destroy that part goes on into the next part
  further in, and so on: a hard ram crushes its way into the hull.
- Your own vessels bump each other without damage. Salvage is nobody's friend: ramming a wreck, or your own debris,
  hurts both. Right after a split, the pieces pass through each other for a moment.

## Shields

A [[part:shield_emitter]] projects a bubble of [[stat:shield_radius]] around itself. A shot from outside that enters
the bubble loses part of its damage to it: the bubble takes the damage times its share for that kind of weapon, up
to the charge it has left, and the rest flies on.

| Weapon family | Share the bubble takes |
|---|---|
| Energy (lasers) | [[part:shield_emitter.absorb.energy]] |
| Kinetic (cannons, rams) | [[part:shield_emitter.absorb.kinetic]] |
| Missile (and blasts from other ships) | [[part:shield_emitter.absorb.missile]] |

- **Overlapping bubbles stack**: a shot crosses them one after the other, and each takes its share of what reaches
  it.
- **A shot fired from inside a bubble passes it.** A gun whose muzzle is inside an enemy's bubble, right up against
  its hull, gets past that shield.
- The bubble holds [[stat:shield_capacity]] and recharges [[stat:shield_regen]] while its emitter has power, slower
  at partial power. It needs at least [[part:shield_emitter.power_min_level]] of its draw to stand at all.
- When the charge runs out, the bubble **collapses** and stays down until it has recharged
  [[sim:shield_restart]] of its capacity. The HUD shows SHIELD DOWN while one of yours is down.
- A dark or dormant vessel has no bubbles.
- Your own bubbles never stop your own blasts or plumes, and your shots fly straight through the bubbles of your
  own vessels.

The drydock's SHIELDS overlay draws every bubble at its real size.

<StatTable stats="shield_radius,shield_capacity,shield_regen" />

## The numbers you see

- **Hit points** in the drydock and on part pages are a part's hit points, after its material for hull parts. A
  ship's total, such as [[ship:rim_skiff.hp]] for the [[ship:rim_skiff]], is the sum over the parts of its main
  piece.
- **HULL** on the flight HUD is what your vessel has left of the whole design's hit points, in percent. The target
  panel shows the same for your target, and SALVAGE instead of its name once it has no core left, or DARK while it
  has no power.
- **Peak heat** is explained under [power, heat and control](/mechanics/power-and-heat#overheating).

See also the [[page:drydock_rules]] and the [[category:hull]] and [[category:defense]] parts.
