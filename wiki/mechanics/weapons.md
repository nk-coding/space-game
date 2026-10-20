---
title: "Weapons"
description: "How guns fire - beams, pulses, projectiles and missiles, point defence and flares, ion guns that blind, range and the line of fire, ricochets, aiming and leading, weapon groups, fire at will, firepower, and what power, heat and capacitors do to guns."
---

# Weapons

Every gun is fixed: it fires along the side it faces, and you aim it by turning the ship. A gun needs a clear line
of fire, power, and a cool enough barrel. This page explains how shots fly and hit, and how you and your behaviors
decide when guns fire.

## The guns

<PartGrid category="weapons" />

Each weapon belongs to a **family**, and shields treat the families differently
([shields](/mechanics/damage#shields)). The lasers are energy weapons; the [[part:automaton_cannon]] is a kinetic
gun (research builds you a slower copy, the [[part:reverse_engineered_cannon]]); the [[part:rocket_pod]] and the
[[part:torpedo_launcher]] fire missiles ([below](#missiles)); the [[part:point_defence]] is an energy gun that aims
itself ([point defence](#point-defence)); the [[part:ion_cannon]] is a disruption gun that blinds an enemy's mind
([disruption](#disruption)). The [[part:mining_laser]] is a utility gun: its beam barely scratches a
ship but cuts ore from asteroid rock ([mining](/mechanics/salvage-and-cargo#mining)). Drones fly from hangars, not
guns ([drones](/mechanics/drones)).

<StatTable stats="damage,range,shot_rate,shot_speed,penetration,recoil" />

[[stat:damage]] is always **damage per second** at full power. A gun that fires shots splits it over its
[[stat:shot_rate|shots]]: more shots per second means less damage per shot, the same damage per second.

## How shots travel

Guns deliver their damage in one of three ways.

- **Beam** (the [[part:beam_laser]]): a continuous ray. While it fires, it burns the first block in its
  [[stat:range]] continuously, its damage per second spread evenly over time. It hits at once, wherever the
  target is.
- **Pulse** (the [[part:pulse_laser]]): separate shots at [[stat:shot_rate]] per second. Each pulse hits at once,
  like a beam, and burns the first block it touches.
- **Projectile** (the [[part:automaton_cannon]]): a real shot that flies. It leaves the muzzle at
  [[stat:shot_speed]] on top of your ship's own velocity, and flies until it has covered its range (counted from
  your ship as it moves). It can miss a target that dodges, so aim ahead of a moving one
  ([aiming](#aiming-and-leading)).
- **Missile** (the [[part:rocket_pod]], the [[part:torpedo_launcher]]): a shot that steers itself
  ([missiles](#missiles)).

Lasers burn the first block only. A projectile hits a block and, if that block breaks, carries the damage it had
left into the next block behind it, up to [[stat:penetration]] blocks in all (1: it stops in the first). The rest
stops when it leaves the hull; it never flies on to another ship. Each shot also gives your ship a kick backwards at
the muzzle, its [[stat:recoil]]: a gun far off the centre of mass also twists the ship a little.

### Ricochets

A kinetic projectile that hits a slope's angled face more than [[sim:ricochet_angle]] away from straight on glances
off. It bounces off the face, keeps [[sim:ricochet_speed]] of its speed and [[sim:ricochet_damage]] of its damage,
and the slope takes nothing. A shot ricochets at most [[sim:max_ricochets]] times. Lasers never ricochet. Slopes on
your nose and edges therefore turn many glancing cannon hits away.

### No friendly fire

Your shots fly straight through your own ship and every one of your vessels: no hit, no damage, no drain on their
shields. They hit enemies, wrecks and other salvage. Your own ship's debris is part of your ship, so your guns pass
it too.

## Missiles

A launcher fires a **salvo**: [[part:rocket_pod.salvo]] rockets at a time from the [[part:rocket_pod]], one torpedo
from the [[part:torpedo_launcher]], [[stat:shot_rate]] salvos per second. A missile leaves slowly, speeds up by its
[[stat:missile_accel]] to its [[stat:shot_speed]], and turns its course at up to [[stat:missile_turn]] towards the
nearest enemy vessel ahead of it (within [[sim:missile_seek_cone]] of its course), leading it as it moves. It picks
a new one if that one is destroyed. After [[stat:missile_life]] it is spent and falls dark.

Where a missile strikes a hull, its **warhead** goes off: every part within its [[stat:warhead_radius]] takes its
damage once, the way a [chain reaction](/mechanics/damage#explosions-and-chain-reactions) hits. Armour takes its share and shields
the parts behind it. A missile's damage per second is shared over its salvo: a rocket of the pod deals
[[part:rocket_pod.damage]] ÷ [[part:rocket_pod.shot_rate]] ÷ [[part:rocket_pod.salvo]] to each part it reaches.
Your missiles never hurt your own vessels.

Missiles have their counters:

- **Shields** absorb a missile whole, up to what the bubble has left.
- **Point defence** shoots them down: each missile takes its [[stat:missile_hp]] in damage, then pops.
- **Flares** pull them off: see [flares](#flares) below.
- **Speed**: a small ship that turns hard can out-turn a slow torpedo.

At most [[sim:max_missiles]] missiles fly at once in a fight; a launcher waits while the sky is full.

### Point defence

The [[part:point_defence]] is a quick, short-range pulse turret that **picks its own targets**: it aims anywhere
within [[part:point_defence.defence_arc]] of the side it faces and fires on its own whenever its vessel runs,
whatever the trigger. Each shot goes at the nearest enemy missile in its reach; with none it shoots at the nearest
enemy vessel of at most [[part:point_defence.defence_mass]], so it also stings small raiders that come close. It
shoots only what it **sees**: a hull between the turret and its target (your own ship's too, a friend's, a wreck's)
or an asteroid rock hides the target, and the turret looks for another. It draws its power all the time, and like any
gun it needs a clear line of fire through the side it faces. It is never part of your firepower, your reach or the
lock's aim.

### Flares

The [[part:flare_launcher]] throws out [[part:flare_launcher.flare_decoys]] hot decoys at a time, fanned around the
side it faces, at [[part:flare_launcher.decoy_speed]] on top of your own speed; they slow down and burn for
[[part:flare_launcher.decoy_life]]. Every enemy homing missile that comes within [[part:flare_launcher.decoy_lure]]
of a burning decoy turns onto it and is spent on it without a blast; it no longer counts as
[[behavior_node:missiles_incoming|incoming]]. If the decoy burns out first, the missile looks for a ship again.

A launcher has [[part:flare_launcher.flare_charges]] releases per fight and needs
[[part:flare_launcher.flare_cooldown]] between them, so time them: release when the missiles are close.
[[control:flare]] ([[control:flare.keys]]) releases them from the vessel you fly; a behavior's
[[behavior_node:flare|flare]] action and the [[behavior_node:auto_flare|auto-flare]] assist do it on their own, best
under `when missiles_incoming >= 1`. A launcher works only on the vessel's controlling piece and needs no power.

## Disruption

The [[part:ion_cannon]] fires slow pulses that burn only [[part:ion_cannon.damage]] but carry
[[part:ion_cannon.disrupt]] of [[stat:disrupt|static]] into the logic cores of the vessel they hit: every pulse that
reaches its hull adds its share, [[sim:static_core_hit]] as much when it lands on a core itself. Armour does not stop
the static, but a shield bubble takes the pulse whole, static and all, so knock the shields down first or come from
where they do not cover.

A vessel holds [[sim:static_per_core]] static for each logic or automaton core of its main piece, so bigger minds take
longer to fill; it sheds [[sim:static_bleed]]. When it is full, the vessel's mind is **blinded** for
[[sim:blind_time]]: its sensors see nothing, its behavior drops its target and holds its fire, and its conditions
see no enemies (`enemies_near` is 0, `under_attack` false, `missiles_incoming` 0; rules that need a target fail).
It flies on by the rules that need none, so a blinded raider brakes or drifts; a rule with
[[behavior_node:blinded]] can pick what it does meanwhile, such as keeping near its leader. Then it shakes the static off and
takes none for [[sim:blind_immune]], so it cannot be held blind for good. The ship you fly yourself keeps its stick,
lock and trigger while blinded; only its assists lose sight.

## Lances

The [[part:phase_lance]] of the [[faction:walkers]] is a short pulse gun whose shot does not stop at the first block:
it strikes every part on its line through the hull it hits, each with what is left of the shot. After each cell of a
machine it keeps [[part:phase_lance.lance_keep]] of its strength, after each cell of armour
[[part:phase_lance.lance_armor]], and it stops where the line leaves the hull or once it is down to
[[sim:lance_spent]]. So it finds the reactors and cores behind thin plating; thick plating still helps. Shield bubbles
take it whole, unless it is fired from inside one. More on the walkers' blinks and lances:
[Phase walkers](./phase-walkers).

## The line of fire

A gun fires through the side it faces, and **its own piece must not be in the way**: no part of the same piece may
sit in the cells straight ahead of it, all the way to the edge of the design. A blocked gun doesn't fire and draws
no power. If the part in front is shot off, or breaks away, the gun has a clear line again and fires from then on.

Guns can face any side. A gun on the flank fires sideways, one at the tail fires backwards.

In the drydock the ARCS overlay draws every gun's line of fire to its range: dashed when clear, coral up to the first
part in the way. It also shows the arc a point-defence turret aims within and the cone a missile launcher launches
within. The status line warns GUN BLOCKED.

## Aiming and leading

You aim by turning the ship. Two things help:

- **The lock.** Click a ship, or press [[control:lock_next.keys]] for the nearest enemy. While a lock is held, your
  ship turns its strongest side onto the target by itself: the guns are grouped by the side they face, and the group
  with the most damage per second among the guns that can fire now wins. If that group's strongest gun fires
  projectiles, the ship aims at the **lead point**, where the target will be when the shot gets there.
  [[control:turn_left]] and [[control:turn_right]] nudge the aim, and it springs back when you let go. See
  [flight](/mechanics/flight).
- **Pilot assists** can lock an enemy for you and fire groups at your lock by themselves (below).

The lead point assumes the target keeps its course. A target that turns or brakes makes a projectile miss; lasers
always hit where they point.

## Firing

| Control | Fires |
|---|---|
| [[control:fire]] ([[control:fire.keys]]) | every gun, whatever its group |
| [[control:fire_group_1]] ([[control:fire_group_1.keys]], or hold the left button on empty space) | the guns of group 1 |
| [[control:fire_group_2]] ([[control:fire_group_2.keys]]) | the guns of group 2 |
| [[control:fire_group_3]] ([[control:fire_group_3.keys]]) | the guns of group 3 |

On a touch screen the fire button fires every group.

Guns fire only while you hold the trigger, and a gun that has been quiet is loaded: the first shot of a fresh burst
leaves at once. When two ships open fire in the same instant, both get their shots off.

### Weapon groups

Each gun belongs to one of three **weapon groups**. Every gun starts in group 1. To change it, select the gun in
the drydock and pick 1, 2 or 3 in the GROUP row of SELECTED PART; each pill's tooltip names the keys that fire that
group. Put the guns you want together in one group, for example the forward lasers in 1 and a rear cannon in 2.

Groups are the pilot's: ships flying a behavior fire each gun at will instead.

### Fire at will

A ship flying a behavior, and every escort you don't pilot, fires **each gun on its own**: a gun fires when the
target is within [[sim:fire_range]] of that gun's range and within [[sim:fire_cone]] of the direction it faces,
both measured from its muzzle. A projectile gun checks the lead point instead of the target. A missile launcher
launches within the wider [[sim:missile_fire_cone]]: its missiles turn onto the target themselves. So broadside and
rear guns fire whenever the target crosses their line.

<svg class="diagram" viewBox="0 0 440 210" role="img" aria-label="A forward gun with a clear line of fire and a narrow fire cone, and a side gun blocked by its own armour">
  <rect x="100" y="110" width="20" height="20" class="f-steel s-dim"/>
  <rect x="120" y="110" width="20" height="20" class="f-tile s-blue thick"/>
  <rect x="140" y="110" width="20" height="20" class="f-steel s-dim"/>
  <rect x="100" y="130" width="20" height="20" class="f-steel s-dim"/>
  <rect x="120" y="130" width="20" height="20" class="f-tile s-violet"/>
  <rect x="140" y="130" width="20" height="20" class="f-steel s-dim"/>
  <rect x="100" y="150" width="20" height="20" class="f-steel s-dim"/>
  <rect x="120" y="150" width="20" height="20" class="f-tile s-blue thick"/>
  <rect x="140" y="150" width="20" height="20" class="f-steel s-coral thick"/>
  <rect x="100" y="170" width="20" height="20" class="f-steel s-dim"/>
  <rect x="120" y="170" width="20" height="20" class="f-steel s-dim"/>
  <rect x="140" y="170" width="20" height="20" class="f-steel s-dim"/>
  <path d="M130 110 L118 14 L142 14 Z" class="f-gold"/>
  <line x1="130" y1="110" x2="130" y2="14" class="s-blue dotted"/>
  <line x1="140" y1="160" x2="156" y2="160" class="s-coral thick"/>
  <line x1="144" y1="156" x2="152" y2="164" class="s-coral thick"/>
  <line x1="152" y1="156" x2="144" y2="164" class="s-coral thick"/>
  <text x="160" y="30" class="t-gold">fire cone</text>
  <line x1="156" y1="26" x2="140" y2="30" class="line"/>
  <text x="160" y="70">clear line of fire to its range</text>
  <line x1="156" y1="66" x2="134" y2="70" class="line"/>
  <text x="190" y="164" class="t-coral">blocked: own part in the way</text>
  <line x1="186" y1="160" x2="162" y2="160" class="line"/>
  <text x="190" y="124">logic core</text>
  <line x1="186" y1="120" x2="140" y2="138" class="line"/>
</svg>

*The nose gun fires at will when the target is inside its narrow cone and range. The flank gun faces a block of its
own piece and never fires until that block is gone.*

The [[behavior_node:auto_fire]] pilot assist uses the same test for one group while you fly: the guns of that group
fire by themselves whenever they bear on your lock (and never without one). Holding that group's trigger fires it
as usual. See
[pilot assists](/behavior/pilot-assists).

## Power, heat and control

Guns are machines like any other ([power, heat and control](/mechanics/power-and-heat)):

- A gun draws power only while it fires. By default guns are fed last, in [[part:pulse_laser.power_tier]], so a ship
  short of power loses its guns first.
- At **partial power** a pulse or projectile gun fires slower, each shot as hard as before; a beam burns weaker. A
  [[part:beam_laser]] below [[part:beam_laser.power_min_level]] of its draw switches off.
- Firing makes heat. A gun over its [[stat:heat_limit]] stops until it has cooled below
  [[sim:overheat_resume]] of it.
- A gun idled by the [control capacity](/mechanics/power-and-heat#control-capacity) doesn't fire. On a dark or
  dormant vessel no gun fires.

### Capacitors

A [[part:capacitor]] beside a laser raises its [[stat:fire_rate]] by [[part:capacitor.bonus.fire_rate]], up to
[[part:capacitor.bonus.fire_rate.max_stacks]] capacitors per laser. A pulse laser fires more pulses per second; a
beam burns harder. Damage per second rises either way, the damage per shot does not. The bonus works only on lasers,
needs no power, and is gone when the capacitor is destroyed. A capacitor explodes when destroyed, so keep it off the
outer hull.

### Ammo feeders

An [[part:ammo_feeder]] does the same for the cannons beside it ([[part:reverse_engineered_cannon]],
[[part:long_cannon]]): [[part:ammo_feeder.bonus.fire_rate]] [[stat:fire_rate]] each, at most
[[part:ammo_feeder.bonus.fire_rate.max_stacks]] per cannon. Its rounds go off when it is destroyed.

### Sidegrades

Research adds parts beside the ones you start with, each with a trade-off instead of a straight upgrade: the
[[part:burst_laser]] fires lighter pulses much faster than the pulse laser, more damage per second, but reaches less far and runs hot; the [[part:long_cannon]]
reaches far with a heavy, piercing shell but fires slowly. Elsewhere: the [[part:wide_shield_emitter]] (a wider, thinner
bubble that refills slowly), the [[part:heat_sink]] (a 1×2 radiator), the [[part:compact_reactor]] (less power, less heat)
and the [[part:spore_radiator]] (vents better, feeds no reactor).

### Railguns

The [[part:railgun]] is the kinetic research's heavy gun after the long-barrel cannon: a 1×3 machine that throws a
slug at [[part:railgun.shot_speed]] out to [[part:railgun.range]]. A slug that breaks a block carries what is left on
into the next, through up to [[part:railgun.penetration]] blocks, so it reaches machines behind thin armour; like
every shot it stops where the struck ship's hull ends. It fires slowly, draws [[part:railgun.power_draw]] of power and
kicks the ship back by [[part:railgun.recoil]] a shot, so mount it on the centre line, and give it the reactors to
match. Its slug draws as a white-hot needle with a thin wake, and the gun throws sparks back past its rails as it
fires.

## Firepower in the drydock

The drydock's [[stat:firepower]] cell adds up the damage per second of every gun on the main piece that can fire: a
clear line, not idle, at full power, with its bonuses. A blocked gun counts for nothing. SELECTED PART shows a gun's
own damage and shots per second after its bonuses, boosted numbers in gold. For example the [[ship:rim_skiff]] has
[[ship:rim_skiff.firepower]] and the [[ship:halvex_runner]] [[ship:halvex_runner.firepower]].

## Which gun for which ship

- **Pulse lasers** hit at once, so they never miss a dodging target, and capacitors make them much better. Shields
  take energy hits in full.
- **Beam lasers** are strong but hungry and hot: plan the power and radiators for them, and keep their tier high
  enough that they don't drop below their minimum in a fight.
- **Cannons** throw shots that must be led, and slopes can turn them away, but shields take only part of their
  damage. A cannon whose penetration is above 1 also carries on through the blocks it breaks; a railgun's slug
  through three, which tells against thinly plated machines more than against thick armour.
- **Missiles** need no aim and hit big, slow ships hard, but shields absorb them whole and point defence shoots
  them down. Rockets hit everything around where they strike, so they strip machines off a lightly plated hull.
- **Ion cannons** do almost no damage, but a few seconds of a blinded enemy are a few seconds it does not shoot
  back: pair them with guns that kill, against ships with few cores and no shields up.
- **Point defence** pays off against a missile-heavy enemy: one or two turrets facing the threat turn most rockets
  away.
- **Facing**: a forward battery suits a ship that turns well; broadside guns suit a behavior that circles its target.

See also [[category:weapons]] and the [[page:drydock_rules]].
