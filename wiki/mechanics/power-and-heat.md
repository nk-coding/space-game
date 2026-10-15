---
title: "Power, heat and control"
description: "One shared power pool per piece, which parts get power first when it runs short, how heat builds up and leaves through radiators, adjacency bonuses, control capacity and mind capacity."
---

# Power, heat and control

Your ships have no wiring. Every part of a piece draws from one shared power pool, makes heat that spreads to its
neighbours, and counts against what its logic cores can run. This page explains the three budgets every design
lives within, and what happens when one of them runs out.

## One pool per piece

A **piece** is a group of parts joined through their straight sides (see [hull, armour and damage](/mechanics/damage#pieces-and-breaking-apart)).
Every piece has its own pool:

- **Reactors** add their [[stat:power_output]] to it: [[part:fission_reactor]] [[part:fission_reactor.power_output]],
  [[part:fusion_reactor]] [[part:fusion_reactor.power_output]]. The [[part:automaton_core]] is a mind and a power
  plant in one: it feeds its own pool.
- **Consumers** take their [[stat:power_draw]] from it, but only while they work. Engines draw by how hard you
  thrust (less as the ship nears its rated speed), gyros by how much turning they give, guns only while they fire
  with a clear line, shield emitters while the shields are up, logic cores all the time.
- **Passive parts** need no power: hull, radiators, cargo and the [[part:capacitor]].

Placement doesn't matter: a reactor at the tail feeds a gun at the nose as long as both sit in the same piece. A
piece that breaks off takes only its own reactors with it. A vessel's systems run only while it has a working
logic core and its reactors make some power; without power it goes **DARK** (no thrust, guns, shields or flight
computer) and wakes up again when power returns.

::: info Capacitors
A [[part:capacitor]] stores no power for later. It draws nothing, makes a little heat while its piece runs, and
makes the lasers beside it fire faster (see [adjacency bonuses](#adjacency-bonuses)). When it is destroyed it
explodes.
:::

## When power runs short

The pool is shared out all the time by **power priority**. Every part that draws power has a tier and a weight. You
set both for each part in the drydock, under SELECTED PART → POWER PRIORITY and WEIGHT; picking the part's own
default clears the setting.

1. The tiers are fed in order: CRITICAL, HIGH, NORMAL, LOW. OFF never draws.
2. A tier whose demand fits into what is left gets everything it asks for.
3. The first tier that doesn't fit shares what is left **by weight**. Every part gets the same multiple of its
   weight as a share of its demand: with equal weights every part runs at the same level; a part with twice the
   weight runs at twice the level, up to full.
4. A part whose share would fall below its [[stat:power_min_level]] switches off instead, one at a time, lowest
   weight first, and its share goes back to the others of its tier.
5. While parts of that tier run at partial output, the tiers below get **nothing**. If the tier ends up fully fed
   (because parts that didn't fit switched off), what is left goes on to the next tier.

<svg class="diagram" viewBox="0 0 560 150" role="img" aria-label="Power tiers fed in order: critical and high in full, normal shared by weight, low gets nothing, off never draws">
  <text x="20" y="14">supply</text>
  <rect x="20" y="20" width="360" height="12" class="f-teal"/>
  <line x1="380" y1="20" x2="380" y2="136" class="line dotted"/>
  <rect x="20" y="60" width="90" height="30" class="f-tile s-teal"/>
  <rect x="120" y="60" width="90" height="30" class="f-tile s-teal"/>
  <rect x="220" y="60" width="200" height="30" class="f-tile s-gold"/>
  <rect x="220" y="60" width="160" height="30" class="f-gold"/>
  <rect x="430" y="60" width="60" height="30" class="f-tile s-coral"/>
  <rect x="500" y="60" width="44" height="30" class="f-tile s-dim"/>
  <text x="26" y="52">critical</text>
  <text x="126" y="52">high</text>
  <text x="226" y="52">normal</text>
  <text x="436" y="52">low</text>
  <text x="506" y="52">off</text>
  <text x="26" y="112" class="t-teal">full</text>
  <text x="126" y="112" class="t-teal">full</text>
  <text x="226" y="112" class="t-gold">shared by weight</text>
  <text x="436" y="112" class="t-coral">nothing</text>
  <text x="506" y="112">never</text>
  <text x="226" y="132">partial output</text>
</svg>

*The supply runs out inside NORMAL: its parts share what is left by weight, LOW gets nothing, OFF never draws.*

The game's defaults feed the mind first: [[part:logic_core|logic cores]] are [[part:logic_core.power_tier]],
[[part:shield_emitter|shield emitters]] [[part:shield_emitter.power_tier]], everything that moves the ship
[[part:thruster.power_tier]] (so thrust and turning brown out together) and guns [[part:pulse_laser.power_tier]].
Some parts have a minimum: a shield bubble can't hold below [[part:shield_emitter.power_min_level]] of its draw, a
[[part:beam_laser]] can't hold its focus below [[part:beam_laser.power_min_level]].

**Partial output** scales what a part does: guns fire slower (each shot still hits as hard) and beams burn weaker,
shields recharge slower, engines and RCS pods push less and gyros turn less. A part at partial power also makes less heat.

Because guns sit in the last tier by default, a ship short of power goes quiet while it thrusts hard. Move a gun to
NORMAL or HIGH if it must keep firing, or give it a higher weight than the engines in the same tier.

### Seeing it

- The drydock's [[stat:power_use]] cell shows what everything draws at full use against what the reactors make, and
  the status line says POWER SHORT BY … when it doesn't fit. SELECTED PART shows each part's POWER LEVEL at full
  use: 100 %, a gold share, OFF or IDLE.
- In flight the HUD says POWER and the share served while the pool falls short, and NO POWER when your vessel is
  dark.

<StatTable stats="power_output,power_draw,power_min_level" />

## Heat

Every part with a [[stat:heat_output]] heats up while it works, in proportion to how hard it works. Reactors are
the exception: a reactor makes [[sim:reactor_idle_heat]] of its heat even when little is drawn, rising to full heat
when the whole supply is used. A reactor whose piece draws nothing at all (a dark piece) makes no heat. Capacitors
make their heat whenever their piece runs.

Heat then moves:

- **Conduction.** Heat flows from a hotter part to each cooler part it is linked to, through every cell edge they
  share. A big part with a long shared side passes more.
- **Loss to the void.** Every part loses a little heat to space, and much more through each **open side**: a
  straight side with no linked neighbour (empty space, or the face of a slope next to it). Loss grows with the
  part's heat, so a hot part sheds heat faster than a cool one. An ordinary open side lets out
  [[sim:heat_loss_open_side]] per unit of heat each second.
- **Radiators** vent [[part:radiator.heat_vent]] through each open side instead, far more. A
  [[part:radiator]] buried inside the hull vents almost nothing: it only soaks up heat from its neighbours.
- **Engine plumes** put heat into the part they burn, yours or an enemy's (see
  [engine plumes](/mechanics/damage#engine-plumes)).

A part settles where what it makes and receives equals what it loses. Bigger parts warm up and cool down more
slowly.

<svg class="diagram" viewBox="0 0 480 190" role="img" aria-label="A reactor between a venting radiator and a buried radiator; heat flows into both, but leaves only through the venting radiator's open side">
  <rect x="120" y="40" width="30" height="30" class="f-steel s-dim"/>
  <rect x="150" y="40" width="30" height="30" class="f-tile s-blue"/>
  <line x1="155" y1="50" x2="175" y2="50" class="s-blue"/>
  <line x1="155" y1="55" x2="175" y2="55" class="s-blue"/>
  <line x1="155" y1="60" x2="175" y2="60" class="s-blue"/>
  <rect x="180" y="40" width="30" height="30" class="f-steel s-dim"/>
  <rect x="210" y="40" width="30" height="30" class="f-steel s-dim"/>
  <rect x="120" y="70" width="30" height="30" class="f-steel s-dim"/>
  <rect x="150" y="70" width="30" height="30" class="f-tile s-gold thick"/>
  <circle cx="165" cy="85" r="7" class="s-gold"/>
  <rect x="180" y="70" width="30" height="30" class="f-tile s-blue"/>
  <line x1="185" y1="80" x2="205" y2="80" class="s-blue"/>
  <line x1="185" y1="85" x2="205" y2="85" class="s-blue"/>
  <line x1="185" y1="90" x2="205" y2="90" class="s-blue"/>
  <rect x="210" y="70" width="30" height="30" class="f-steel s-dim"/>
  <rect x="120" y="100" width="30" height="30" class="f-steel s-dim"/>
  <rect x="150" y="100" width="30" height="30" class="f-steel s-dim"/>
  <rect x="180" y="100" width="30" height="30" class="f-steel s-dim"/>
  <rect x="210" y="100" width="30" height="30" class="f-steel s-dim"/>
  <line x1="165" y1="78" x2="165" y2="58" class="s-coral thick" marker-end="url(#arrow-coral)"/>
  <line x1="172" y1="85" x2="190" y2="85" class="s-coral thick" marker-end="url(#arrow-coral)"/>
  <line x1="158" y1="36" x2="152" y2="14" class="s-gold" marker-end="url(#arrow-gold)"/>
  <line x1="165" y1="36" x2="165" y2="12" class="s-gold" marker-end="url(#arrow-gold)"/>
  <line x1="172" y1="36" x2="178" y2="14" class="s-gold" marker-end="url(#arrow-gold)"/>
  <rect x="161" y="66" width="8" height="8" transform="rotate(45 165 70)" class="f-gold"/>
  <text x="260" y="26">open side: vents</text>
  <line x1="256" y1="22" x2="186" y2="22" class="line"/>
  <text x="260" y="56" class="t-gold">venting radiator: + output</text>
  <line x1="256" y1="52" x2="178" y2="52" class="line"/>
  <text x="260" y="88">buried radiator: no vent, no bonus</text>
  <line x1="256" y1="84" x2="208" y2="84" class="line"/>
  <text x="260" y="118">reactor</text>
  <line x1="256" y1="114" x2="172" y2="94" class="line"/>
  <text x="20" y="160" class="t-coral">heat flows to linked neighbours</text>
  <text x="20" y="178" class="t-gold">and leaves through open sides</text>
</svg>

*Both radiators take heat from the reactor, but only the one with an open side gets rid of it, and only that one
raises the reactor's output.*

### Overheating

A part hotter than its [[stat:heat_limit]] **shuts down**: it does no work, draws no power and gives no output. It
starts again once it has cooled below [[sim:overheat_resume]] of its limit, so a part that overheats stays off for
a while. What that means:

- An overheated reactor adds nothing to the pool, so a hot reactor can brown out its whole piece.
- An overheated gun stops firing; an overheated shield emitter stops recharging.
- An overheated logic core gives no control capacity. When every core of a vessel is overheated, the vessel goes
  DARK until one cools down.

Hull parts, radiators and cargo have no heat limit and never shut down.

In the drydock, [[stat:peak_heat]] is how hot the hottest part runs at full use, as a share of its limit, worked out
for a ship that runs everything at once for a long time. SELECTED PART shows each part's heat as `heat / limit`,
and the HEAT overlay tints every part. Teal is fine; gold means it runs hot (a little over its limit), and coral means it overheats at full load. Real fights rarely use everything at once, so a
little gold is often fine. In flight the HUD names the hottest parts near their limit with their heat.

<StatTable stats="heat_output,heat_limit,heat_vent" />

## Adjacency bonuses

Some parts make their neighbours better. A bonus goes from the part that grants it to each **linked** neighbour of
the right kind: touching through a straight side, the same rule that joins parts into a piece. Diagonal neighbours
and slope faces don't count.

- Each granting neighbour counts once. Several bonuses on the same stat add up: flat amounts first, then the
  percentages added together.
- Some bonuses count only a few neighbours per part.
- Some have a condition: a radiator's bonus counts only while the radiator **vents**, with at least one open side.
- A destroyed neighbour takes its bonus with it. A bonus needs no power: a capacitor helps even when its piece is
  short of power.

The base parts grant these:

| From | To | Bonus | Limit |
|---|---|---|---|
| [[part:capacitor]] | each laser beside it ([[part:pulse_laser]], [[part:beam_laser]]) | [[part:capacitor.bonus.fire_rate]] [[stat:fire_rate]] | at most [[part:capacitor.bonus.fire_rate.max_stacks]] per laser |
| [[part:radiator]] | each reactor beside it, while the radiator vents | [[part:radiator.bonus.power_output]] [[stat:power_output]] | none |

A faster fire rate raises a pulse laser's shots per second and a beam's damage per second; a shot still hits as hard.
The drydock's BONUS overlay draws a gold link between every pair that boost each other, and SELECTED PART lists each
bonus a part gets or gives as a gold line.

## Control capacity

Every part has a [[stat:control_cost]]: by default nothing for hull, logic cores and passive parts (radiators,
capacitors, cargo), and one for everything that draws or makes power. Big machines cost more, such as the
[[part:fusion_reactor]] at [[part:fusion_reactor.control_cost]].

Each running logic core adds its [[stat:control_capacity]]: [[part:logic_core.control_capacity]] for a
[[part:logic_core]], [[part:automaton_core.control_capacity]] for an [[part:automaton_core]]. A core counts while it
is intact and not overheated.

When a vessel's parts cost more than its cores can run, the extra parts go **idle**, in the order a brown-out
switches them off: OFF parts first, then LOW, then up the tiers, the lowest weight first inside a tier. Reactors go
idle only after every consumer. An idle part does nothing at all: no work, no power, no heat. It runs again as soon
as the capacity covers it, for example when an overheated core cools down. Losing a core in a fight can idle your
guns.

The drydock's [[stat:control_load]] cell shows cost against capacity, the status line says OVER CONTROL CAPACITY,
and SELECTED PART marks the parts that would idle as IDLE. In flight the fleet bar's control bar turns coral while
parts idle.

## Mind capacity and dormant vessels

The ship-mind can run only so many logic cores at once, across all your vessels: its **mind capacity**. A new game
starts with [[start:rim_awakening.mind_capacity]]; research such as [[research:mind_expansion_1]] and
[[research:mind_expansion_2]] adds more. A fleet with more cores than that can't launch on a voyage (the drydock's
voyage-ready line says so).

In flight, a ship that breaks apart can leave a second piece with a core, which becomes a vessel of its own. When
your vessels hold more cores than the mind runs, the newest go **DORMANT**: dark like salvage, with no behavior,
thrust, power or shields, but still yours. The vessel you fly never goes dormant. Click a dormant vessel's chip on
the fleet bar to wake it: it swaps in, and the newest other vessel goes dormant instead.

## Design notes

Power, heat and the bonuses push you to build compact machine cores with radiators on the outside, and to decide
what keeps working when things go wrong. See also the [[page:drydock_rules]], [the drydock](/mechanics/drydock) and
[[category:power]].
