---
title: "Flight model"
description: "How a ship moves and turns, from engines, RCS pods and gyros to the top speed, the flight computer and the drydock's flight numbers."
---

# Flight model

Every ship is a physical body. Its parts give it mass and a centre of mass, its engines push at the cells where
they sit, and nothing slows it down but its own thrust. You never fly the engines one by one: an always-on flight
computer turns what you ask for (a direction, a heading, a lock) into thrust for each engine, nozzle and gyro.

## What moves a ship

### Engines

An engine pushes the ship towards its forward side, along a line through the engine's centre. Its exhaust leaves
through the back. You place engines in quarter turns, so each one pushes along one of the ship's four axes: forward,
back, right or left. The axis with the most thrust is the ship's **best axis** (forward wins a tie); its top speed
and its push are what most of the flight computer plans with.

An engine may sit anywhere in the hull, even buried. Its plume burns the first part in its path, your own ship's
included, up to its [[stat:plume_length]] behind it. The drydock warns you when a plume burns the ship.

An engine with a [[stat:gimbal]] can swing its thrust a little to either side. That adds a small sideways push
with a turn, so the flight computer uses it only as a last resort (see [Turning](#turning)).

<StatTable stats="thrust,rated_speed,gimbal" />

### RCS pods

An [[part:rcs_pod]] has no fixed direction. It fires one nozzle out of every straight side that is open to space
(an empty cell, or a cell of another piece). Each nozzle pushes the ship away from its side with
[[part:rcs_pod.rcs_thrust]]. Nozzles come and go as neighbours are shot away or the ship breaks apart.

Pods do two jobs:

- **Push**: they let the ship slide sideways or back, which an engine-only ship can't.
- **Turn**: two nozzles of opposite sides that turn the ship the same way fire together with equal force. Their
  pushes cancel and only the turn is left: a **turning couple**. A pod far from the centre of mass with its front
  and back sides open is the strongest turner per part.

### Gyros and logic cores

A [[part:gyro]] turns the ship without any thrust, with [[part:gyro.torque]] either way. It works anywhere in the
hull and needs no open side. Every [[part:logic_core]] has a tiny built-in reaction wheel too
([[part:logic_core.torque]]): enough to keep a ship controllable, not enough to make it nimble.

<PartGrid category="propulsion" />

## Mass and the centre of mass

Heavier ships gain speed and turn more slowly: the same thrust gives a heavy ship less acceleration, and the same
torque turns a long ship more slowly than a compact one. Each push acts at its engine. If an engine's thrust line misses the centre of
mass, the push also turns the ship.

<svg class="diagram" viewBox="0 0 600 250" role="img" aria-label="Left: a ship whose only working engine sits left of the centre of mass turns right as it pushes. Right: two RCS nozzles on opposite sides fire together and turn the ship without pushing it.">
  <text x="150" y="22" text-anchor="middle">THRUST OFF THE CENTRE</text>
  <rect class="f-tile s-dim" x="90" y="60" width="120" height="100" />
  <rect class="f-steel s-dim" x="110" y="140" width="20" height="20" />
  <rect class="f-dim s-dim" x="170" y="140" width="20" height="20" />
  <line class="s-dim dotted" x1="150" y1="40" x2="150" y2="200" />
  <circle class="s-gold thick" cx="150" cy="110" r="5" />
  <text x="150" y="130" text-anchor="middle">CENTRE OF MASS</text>
  <line class="s-teal thick glow" x1="120" y1="185" x2="120" y2="125" marker-end="url(#arrow-teal)" />
  <text class="t-teal" x="78" y="205">ENGINE</text>
  <text x="180" y="185">LOST</text>
  <line class="line" x1="120" y1="110" x2="150" y2="110" />
  <path class="s-gold thick" d="M 128 52 A 40 40 0 0 1 190 72" fill="none" marker-end="url(#arrow-gold)" />
  <text class="t-gold" x="196" y="58">TURNS RIGHT</text>
  <text x="450" y="22" text-anchor="middle">A TURNING COUPLE</text>
  <rect class="f-tile s-dim" x="390" y="70" width="120" height="80" />
  <rect class="f-violet s-dim" x="370" y="100" width="20" height="20" />
  <rect class="f-violet s-dim" x="510" y="100" width="20" height="20" />
  <circle class="s-gold thick" cx="450" cy="110" r="5" />
  <text x="450" y="132" text-anchor="middle">CENTRE OF MASS</text>
  <line class="s-teal thick glow" x1="380" y1="160" x2="380" y2="124" marker-end="url(#arrow-teal)" />
  <line class="s-teal thick glow" x1="520" y1="60" x2="520" y2="96" marker-end="url(#arrow-teal)" />
  <text class="t-teal" x="334" y="180">BACK NOZZLE</text>
  <text class="t-teal" x="470" y="50">FRONT NOZZLE</text>
  <path class="s-gold thick" d="M 428 58 A 40 40 0 0 1 482 62" fill="none" marker-end="url(#arrow-gold)" />
  <text x="450" y="215" text-anchor="middle">EQUAL PUSHES CANCEL, THE TURN STAYS</text>
</svg>

*Left: with one rear engine lost, the other pushes left of the centre of mass and the ship turns right. Right: the
front nozzle of the right pod pushes back, the back nozzle of the left pod pushes forward; both turn the ship
right, and the pushes cancel.*

The flight computer fights an uneven push for you. Among the engines pushing one way it picks the mix that turns
the ship least (or the way it wants), then holds the rest with gyros, cores and RCS couples, and after a moment
with gimbals and differential thrust. If all that can't hold it, the ship drifts off its heading while it
accelerates. The parts' masses also set the centre of mass, so losing parts moves it, and the same engines may
push off-centre after a fight.

## Speed

### The rated-speed falloff

Every engine and RCS pod has a **rated speed** ([[stat:rated_speed|engines]], [[stat:rcs_rated_speed|RCS pods]]). While it pushes
with the ship's motion, it gives full thrust until the ship's speed reaches [[sim:rated_knee]] of its rating (the
knee), then fades evenly to nothing at the rating.

<svg class="diagram" viewBox="0 0 560 250" role="img" aria-label="Thrust share against ship speed: full thrust up to the knee, then a straight fade to nothing at the rated speed; a push a little off the motion fades later; braking and sideways pushes keep full thrust.">
  <line class="line" x1="60" y1="200" x2="530" y2="200" marker-end="url(#arrow)" />
  <line class="line" x1="60" y1="200" x2="60" y2="30" marker-end="url(#arrow)" />
  <text x="530" y="222" text-anchor="end">SHIP SPEED</text>
  <text x="66" y="28">THRUST</text>
  <text class="num" x="52" y="64" text-anchor="end">FULL</text>
  <text class="num" x="52" y="204" text-anchor="end">0</text>
  <line class="s-dim dotted" x1="60" y1="60" x2="520" y2="60" />
  <text x="520" y="52" text-anchor="end">BRAKING OR SIDEWAYS</text>
  <polyline class="s-violet" fill="none" points="60,60 415,60 504,200 520,200" />
  <text class="t-violet" x="430" y="120">A LITTLE OFF THE MOTION</text>
  <polyline class="s-teal thick glow" fill="none" points="60,60 380,60 460,200 520,200" />
  <text class="t-teal" x="200" y="84">WITH THE MOTION</text>
  <line class="line" x1="380" y1="60" x2="380" y2="200" />
  <line class="line" x1="460" y1="60" x2="460" y2="200" />
  <text x="380" y="222" text-anchor="middle">KNEE</text>
  <text x="460" y="240" text-anchor="middle">RATED SPEED</text>
</svg>

*Thrust while pushing with the motion: flat up to the knee at [[sim:rated_knee]] of the rated speed, then a
straight fade. A push off the motion fades later; braking and sideways pushes never fade.*

The flat start keeps the acceleration steady for most of the way, so the last stretch to the top speed is short.
Only a push **with** the motion fades:

- **Braking** (pushing against the motion) and **sideways** pushes keep full thrust at any speed.
- A push at an angle to the motion fades less the further it points away. From about [[sim:rated_full_angle]] off
  the motion it keeps full thrust even at the rated speed, so a push in a new direction always bends your course.
- **RCS couples** fire at full force at any speed: their pushes cancel, so they can't make the ship faster. Pods
  turn a ship just as well at its top speed.
- Gimbals and differential thrust come from engines, so they fade with them. An engine-only ship at its top speed
  turns on its gyros and cores alone until its course and nose part ways.

Engines with different ratings mix: the slower ones fade first, and past their rating only the faster ones push.

### The top speed and the governor

A ship is never faster on its own thrust than its fastest engines allow. The **top speed** you see on the HUD
(TOP, next to your speed) and in the drydock is the rated speed of the fastest engines along the best axis. On top
of the falloff, a **speed governor** cuts any part of your own thrust that would take the ship past the highest
rating among the thrusters that can push right now.

At the top speed, a push in a new direction swings your velocity onto it at the top speed: the speed the old way
falls away while the new way builds up. A blast or a collision can still knock you faster than your top speed (the
governor never cuts what isn't your own thrust); then your thrust holds that speed instead of adding to it.

### No drag

Space has no drag. Let go and the ship coasts at the speed it had, forever. Holding a speed costs no thrust, power
or heat. The engines still show a calm **cruise flame** while you hold a speed, so they don't look dead.

## The flight computer

It is always on, on every ship you or a behavior fly. Each tick it takes a goal (a velocity and a heading) and
spreads it over what can push and turn right now.

- **Holds the nose**: with nothing turning it, it stops any rotation.
- **Flies where it points**: while you hold thrust keys, it cancels sideways drift, so the ship goes where its nose
  points. With [[control:drift]] on, every key is pure thrust and your momentum is kept.
- **Flips and burns**: when the needed push points where the ship has little thrust (to stop a ship with engines
  only at the back, say), it turns the best axis that way first, and the burn fades in only once the nose is
  nearly round. The touch stick, the behaviors and the brake [pilot assist](/behavior/pilot-assists) use this.
- **Never overshoots**: it asks only for what the thrusters can give at this speed, and plans each turn so it can
  stop it in time.

Letting go of every key (or the stick) coasts with the nose kept. Stopping is your job, or a
[pilot assist's](/behavior/pilot-assists).

## Flying it

### Keyboard

[[control:thrust_forward.keys]] and [[control:thrust_back.keys]] push along the nose,
[[control:strafe_left.keys]] and [[control:strafe_right.keys]] sideways. A full press asks for the top speed of
the thrusters that push that way; a ship with no thrusters facing that way can't push that way at all. [[control:turn_left.keys]]
and [[control:turn_right.keys]] rotate the ship with a ramp: a fresh press starts at [[sim:key_turn_start]] of the
key turn rate and grows to the full rate in [[sim:key_turn_ramp]], so a tap nudges the nose and a hold swings it
round. The key turn rate is [[sim:key_turn_rate]], or less on a ship whose gyros, cores and RCS couples couldn't
stop that turn within about a quarter circle. Key turns never use the last resort, so they never push you
sideways.

### The touch stick

Without a lock the stick means "fly that way": the ship turns its best axis onto the stick and flies that way, the
stick's length setting the speed as a share of the top speed. A released stick coasts, nose kept.

### Target lock

Lock a ship (click or tap it, or [[control:lock_next.keys]]: the nearest enemy, then the next by distance) and the flight
computer turns your **strongest guns** onto it: the side of the ship whose working guns deal the most damage per
second faces the target. If that side's strongest gun fires projectiles, it aims at the **lead point**, where the
target will be when the shot gets there. A ship without working guns points its nose at the target. The nose
follows a crossing target without lagging behind.

Under a lock, [[control:turn_left.keys]] and [[control:turn_right.keys]] nudge the aim off the target, up to
[[sim:nudge_max]], and it springs back when you let go. The thrust keys work as before; the touch stick strafes
instead, with whatever thrusters push that way. A ship with engines only at the back can't brake or strafe while
locked: it would have to turn away from the target to do so.

### Where you are

The camera never turns: north is always up, on the screen as on the minimap in the corner and on the cluster map.
On a voyage the minimap shows what your sensors and scans have found round you; see
[Controls](/mechanics/controls#the-minimap-and-the-cluster-map).

## Turning

The flight computer turns with a **first line** of sources that push the ship nowhere: gyros, the cores' reaction
wheels and RCS couples. Nozzles busy pushing the ship along can't also make a couple, so a ship under full thrust
turns with fewer of them. Every turn is planned with what the first line gives, braked in time, and capped at
[[sim:max_turn_rate]].

When the first line has been at its limit for [[sim:assist_after]], the **last resort** opens: gimbals of the
firing engines, then **differential thrust** (throttling up engines whose push turns the ship the right way). Both
add a push nobody asked for, which is why they wait. Differential thrust helps only while the flight computer
steers to a heading (a lock, the stick, a behavior), never with the turn keys and never just to stop a spin. A ship
without gyros or RCS turns slowly on the keys, but a lock or the stick still swings it round with its engines.

## When parts stop working

The flight computer plans only with parts that can work right now: alive, powered, not overheated and not idle.

- **Power shortage**: engines, RCS pods and gyros share one power tier, so thrust and turning brown out together,
  each at the share it gets. Weapons lose power before them, logic cores last.
- **Heat**: an overheated engine or gyro shuts down until it cools below [[sim:overheat_resume]] of its limit.
- **Idle parts**: if the parts need more control than the running logic cores give, the lowest power priorities go
  idle and do nothing.
- **Damage**: a lost engine takes its thrust and maybe its balance with it (see the diagram above). If the ship
  breaks apart, only the piece holding its logic cores flies on; engines on other pieces are wreckage.
- **Dark and dormant**: a vessel whose cores lose power goes dark and drifts without any flight at all until power
  returns. A dormant vessel doesn't fly either.

The top speed, the turn plan and the stick's full length all follow what still works, so a damaged ship flies
slower but never wilder.

## The drydock's flight numbers

The drydock's stats panel shows four numbers for flight, from the design at full power:

| Number | What it means |
|---|---|
| [[stat:acceleration]] | Forward thrust divided by mass: how quickly the ship gains speed. |
| [[stat:top_speed]] | The rated speed of the fastest engines along the best axis. A dash: no thrust at all. |
| [[stat:turn_time]] | How long a quarter turn from rest takes, the last resort included. A dash: it can't turn (or would take too long). |
| [[stat:yaw]] | Whether full forward thrust turns the ship: Balanced (teal), Pulls left or Pulls right. Gold: the flight computer can hold the pull while flying straight. Coral: it can't, and the status line warns PULLS LEFT UNDER THRUST (or RIGHT). |

The starter [[ship:rim_skiff]], for example, has a thrust/mass of [[ship:rim_skiff.acceleration]], a top speed of
[[ship:rim_skiff.top_speed]] and turns a quarter circle in [[ship:rim_skiff.turn_time]]. The bigger
[[ship:halvex_runner]] reaches [[ship:halvex_runner.top_speed]] and turns in [[ship:halvex_runner.turn_time]].

::: tip Read the numbers together
A high thrust/mass with a slow turn makes a ship that is quick in a straight line and clumsy in a fight. Gyros and
RCS pods far from the centre of mass shorten the turn; RCS pods also let you brake and strafe under a lock.
:::
