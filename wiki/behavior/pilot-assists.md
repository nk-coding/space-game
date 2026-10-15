---
title: "Pilot assists"
description: "A design's second rule list that helps you fly it: locking, braking, holding station, avoiding collisions and auto-fire, each on its own control channel, with toggles in flight."
---

# Pilot assists

A behavior flies a ship when nobody pilots it. **Pilot assists** are the other way round: a second rule list per
design that helps **you** while you fly it. They lock an enemy when you have none, brake when you let go, keep
your place beside a wingman, sidestep a wreck, or fire a weapon group whenever it bears on your lock. You stay in
charge: an assist only ever works a control you aren't using right now, and it never picks a target you can't see.

Every one of your vessels carries its design's list, but only the ship you fly (the focus) runs it. Your other
vessels fly their behaviors as usual.

::: info No assists by default
A new design starts without pilot assists, and so do the base ships. Without any, the ship simply coasts when you
let go. You add the ones you want.
:::

## The assists

Each assist is an action, written like a behavior's action:

<NodeTable role="action" pilot />

- [[behavior_node:auto_lock]] locks an enemy by itself while you have nothing locked, picked like a behavior's
  target: [[behavior_node:nearest]] (the default), [[behavior_node:weakest]], [[behavior_node:threat]] or
  [[behavior_node:attacker]]. The lock is your own (see [below](#your-lock)), so the ship aims at it as at any lock.
- [[behavior_node:brake]] asks the flight computer to stop the ship.
- [[behavior_node:hold_station]] keeps you where the assist took over (`point`, the default), or with `ally` at
  your place beside the nearest of your other vessels, moving with it.
- [[behavior_node:avoid_collisions]] sidesteps a ship, wreck or piece of debris on a collision course.
- [[behavior_node:auto_fire]] fires one weapon group by itself whenever its guns bear on your lock, with the same
  test as a behavior's fire at will: a gun fires when the locked ship (its lead point, for projectile guns) is within
  [[sim:fire_cone]] of where that gun points and within [[sim:fire_range]] of its reach. Without a lock it holds
  fire.

Every assist ends with two values of its own: **toggle** (none, or toggle1 to toggle4: which ASSIST TOGGLE key
switches it in flight) and **start** (on or off: whether each mission starts with it on). On the BEHAVIOR screen
these are the inspector's TOGGLE and AT START pills. Left out, toggle is [[behavior_node:brake.default.toggle]]
and start is [[behavior_node:brake.default.start]]. [[behavior_node:auto_fire]] is written
[[behavior_node:auto_fire.signature]], so `auto_fire(2, toggle3, off)` fires group 2, sits on toggle 3 and stays
off until you press that toggle; [[behavior_node:auto_lock]] is written [[behavior_node:auto_lock.signature]].

## Conditions

An assist rule may use every behavior condition, plus two that only exist here:

<NodeTable role="condition" pilot />

- [[behavior_node:lock_held]]: you hold a lock on a ship or a piece.
- [[behavior_node:idle]]: seconds since you last used a control: `turn` (turning keys, or the touch stick
  without a lock), `thrust` (the thrust and strafe keys, or the stick), `fire` (any trigger) or `any` of them. It
  counts from the start of the mission until you first touch that control.

Two conditions are made for assists but work in behaviors too: [[behavior_node:speed]] (how fast you fly, as a
share of your top speed) and [[behavior_node:can_stop_straight]] (thrusters already push against your motion, so
the ship can stop without turning). Conditions about the target are about your lock, and false while you hold none.

## Your lock {#your-lock}

An assist list has **no target line**. What needs a target acts on your lock, the one the HUD shows: the target
panel names it and a marker rings it. Without a lock, auto-fire holds fire.

You lock a ship by clicking it or with [[control:lock_next]] ([[control:lock_next.keys]]: the nearest enemy, then
the next by distance), and [[behavior_node:auto_lock]] locks one for you while you have none. Its lock is the same
lock: you drop it with Esc or a click on empty space, and change it with a click or [[control:lock_next.keys]].

- **Dropping a lock pauses auto-lock.** Otherwise it would lock again at once. It waits, and the HUD says PAUSED
  after its name, until you lock something yourself or switch it (its toggle, or ALL ASSISTS).
- **A destroyed target doesn't.** When the locked ship is destroyed or goes dark, auto-lock locks the next one.
- **Several auto-lock rules** make a priority list: `auto_lock(attacker)` above `auto_lock(nearest)` locks a ship
  that aims at you, and the nearest one while none does.

## Channels: you always win {#channels}

Each assist works one control **channel**:

- **The lock**: auto-lock.
- **Thrust**: brake, hold station, avoid collisions.
- **Each weapon group's trigger**: auto-fire for that group.

<svg class="diagram" viewBox="0 0 600 270" role="img" aria-label="Each channel takes your input when you use it, else the first matching assist rule on that channel; the lock, thrust and the three weapon groups are decided separately and go to the flight computer together.">
  <text x="70" y="24" text-anchor="middle">CHANNEL</text>
  <text class="t-teal" x="215" y="24" text-anchor="middle">YOU</text>
  <text class="t-violet" x="215" y="262" text-anchor="middle">FIRST MATCHING RULE</text>
  <text x="520" y="24" text-anchor="middle">SHIP</text>
  <text x="70" y="60" text-anchor="middle">LOCK</text>
  <text x="70" y="105" text-anchor="middle">THRUST</text>
  <text x="70" y="150" text-anchor="middle">FIRE 1</text>
  <text x="70" y="195" text-anchor="middle">FIRE 2</text>
  <text x="70" y="240" text-anchor="middle">FIRE 3</text>
  <circle class="f-teal" cx="215" cy="56" r="5" />
  <circle class="f-teal" cx="215" cy="101" r="5" />
  <circle class="f-teal" cx="215" cy="146" r="5" />
  <circle class="f-teal" cx="215" cy="191" r="5" />
  <circle class="f-teal" cx="215" cy="236" r="5" />
  <line class="s-teal thick glow" x1="220" y1="56" x2="340" y2="56" />
  <line class="s-teal thick glow" x1="220" y1="101" x2="340" y2="101" />
  <line class="s-teal thick glow" x1="220" y1="146" x2="340" y2="146" />
  <line class="s-teal thick glow" x1="220" y1="191" x2="340" y2="191" />
  <line class="s-teal thick glow" x1="220" y1="236" x2="340" y2="236" />
  <path class="s-violet dotted" d="M 280 76 L 340 56" fill="none" />
  <path class="s-violet dotted" d="M 280 121 L 340 101" fill="none" />
  <path class="s-violet dotted" d="M 280 166 L 340 146" fill="none" />
  <path class="s-violet dotted" d="M 280 211 L 340 191" fill="none" />
  <path class="s-violet dotted" d="M 280 256 L 340 236" fill="none" />
  <circle class="s-number f-bg" cx="345" cy="56" r="6" />
  <circle class="s-number f-bg" cx="345" cy="101" r="6" />
  <circle class="s-number f-bg" cx="345" cy="146" r="6" />
  <circle class="s-number f-bg" cx="345" cy="191" r="6" />
  <circle class="s-number f-bg" cx="345" cy="236" r="6" />
  <line class="line" x1="351" y1="56" x2="455" y2="140" marker-end="url(#arrow)" />
  <line class="line" x1="351" y1="101" x2="455" y2="143" marker-end="url(#arrow)" />
  <line class="line" x1="351" y1="146" x2="455" y2="146" marker-end="url(#arrow)" />
  <line class="line" x1="351" y1="191" x2="455" y2="149" marker-end="url(#arrow)" />
  <line class="line" x1="351" y1="236" x2="455" y2="152" marker-end="url(#arrow)" />
  <rect class="f-tile s-ring" x="460" y="116" width="120" height="60" />
  <text x="520" y="142" text-anchor="middle">FLIGHT</text>
  <text x="520" y="158" text-anchor="middle">COMPUTER</text>
  <text class="t-teal" x="370" y="40">YOU FIRST</text>
</svg>

*Each channel is decided on its own: your input if you use it this tick, else the first matching assist rule for
that channel. The five results fly the ship together.*

- **The pilot always wins.** A channel you use this tick is yours: a lock you hold takes the lock channel, so
  auto-lock never replaces it. The thrust keys or the stick take thrust. A group's trigger takes that group, and
  [[control:fire]] takes every group.
- **Letting go hands the channel back** on the very next tick.
- **Per channel, the first rule that matches decides.** The rules are read from the top at [[sim:think_hz]],
  together with the behaviors. For each channel, the first rule that is switched on, whose conditions all hold, and that has a
  target if it needs one (auto-fire: your lock) decides that channel. Avoid collisions decides only while
  something is on a collision course, auto-lock only while its pick finds a ship; the rest of the time the rule
  below keeps the channel.
- **Rules on different channels run together.** An `always` rule doesn't stop the rules below it, as it would in a
  behavior: "lock the nearest", "brake when I let go" and "fire group 2 when it bears" all act at once. Two rules on
  the same channel make a priority list of their own.

Assists need no rule slots; a list holds up to [[sim:max_rules]] rules, and may be empty.

## How the thrust assists fly

Brake, hold station and avoid collisions set the velocity the flight computer flies to. How they push depends on
whether **turning is free**: you don't steer and you hold no lock (a lock aims the ship).

- **Turning free**: the assist may turn the ship to bring its engines to bear, the flip and burn behaviors use. It
  keeps the heading if the thrusters already facing the needed way give at least [[sim:keep_heading_fraction]] of
  the ship's best push, and turns only for a velocity change of at least [[sim:assist_turn_dv]], so a slight drift
  never spins the ship around.
- **Turning taken**: only the thrusters that push the needed way act. A ship with engines only at the back then
  can't brake, hold station or sidestep at all. RCS pods fix that.

[[behavior_node:can_stop_straight]] in front of a brake keeps it to stops that need no turn.

### Hold station

The station is taken the moment the assist takes thrust over while you don't thrust: the ship's centre of mass
where you let go, or with `ally` its offset from the nearest of your other active vessels (a point while you have
none). A push drifts you off and the ship flies back, then keeps pace with the station. Thrusting yourself drops
the station, so flying moves it and letting go holds the new spot.

A rule above it that takes thrust for a while (a dodge, a brake) leaves the station waiting, and the ship returns
afterwards, unless the hold station rule itself is switched off. A ship that must turn around to stop comes back no
faster than it can still turn and stop, and within [[sim:station_slack]] of the station it only stops, instead of
chasing the last metre with half turns.

### Avoid collisions

At each think the assist looks [[sim:avoid_ahead]] ahead at every piece around (any ship's, wrecks, your own
debris), each as a circle around it, flying in straight lines. If the ship would come closer than
[[sim:avoid_clearance]] to the first of them, it adds the sidestep that keeps that gap, along the miss (or to the
left of the motion, head-on), until it is clear. Put it above brake or hold station: it takes thrust only while it
is needed.

## Switching assists in flight

Each rule can sit on one of four toggles. [[control:assist_toggle_1]] to [[control:assist_toggle_4]]
([[control:assist_toggle_1.keys]], [[control:assist_toggle_2.keys]], [[control:assist_toggle_3.keys]],
[[control:assist_toggle_4.keys]] by default) flip every rule on that toggle. [[control:all_assists]]
([[control:all_assists.keys]]) is the master switch, ALL ASSISTS: off, no assist acts; on again, each is back as
its toggle left it. Rebind them in SETTINGS like any [control](/mechanics/controls).

Each mission starts with every rule as its `start` value says. The switches then last for the mission; a vessel
that breaks off inherits them, and a ship re-formed at home starts over.

## What the HUD shows

Under your speed, on the left, the flight HUD lists the assists of the ship you fly, one line each: its toggle key
if it has one, then its name (BRAKE; auto-fire with its group, AUTO-FIRE 2). The colour says its state:

- **teal**: it decides its channel now, and acts whenever you leave that channel free;
- **white**: switched on, but not deciding (its conditions don't hold, or a rule above has the channel);
- **dim**: switched off.

An auto-lock you paused by dropping a lock reads `AUTO-LOCK · PAUSED`. While ALL ASSISTS is off, the title reads ASSISTS OFF with its key, in gold. A long list ends in `+ N MORE`; a ship
without assists shows no block. Each switch also says what it did in a banner: BRAKE ON · AUTO-FIRE 2 OFF, ALL
ASSISTS OFF, or that a toggle has no assist on it.

## Editing assists

Open the BEHAVIOR screen and pick the PILOT ASSISTS tab. It edits the design's assists with the same GRAPH and
TEXT views, inspector and palette as its behavior; the palette offers only the assists and the conditions they may
use. The graph's TARGET row reads YOUR LOCK: there is no target line to set. The assists are part of the design:
every change goes into it at once, and you save it in the drydock like the rest of the design. TEST IN RANGE flies
the design with its assists.

## Examples

Stop when you let go, but only when no turn is needed; lock the nearest enemy once you switch it on:

```assists
when idle(thrust) >= 1s and can_stop_straight -> brake(toggle1) # stop on release, no flip
always -> auto_lock(nearest, toggle2, off) # lock the nearest enemy, from toggle 2 on
```

A fighting list, for a design with its front guns in group 1 and its side guns in group 2. It locks a ship that
aims at you, else the weakest; group 1 fires by itself once you leave the triggers alone for two seconds, group 2
whenever it bears on the lock in reach:

```assists
always -> auto_lock(attacker, toggle1)
always -> auto_lock(weakest, toggle1)
when idle(fire) >= 2s -> auto_fire(1, toggle3)
when target_in_range(90%) -> auto_fire(2)
```

A wingman's list: dodge first, keep your place beside your nearest ship while no enemy is near, else brake when
you let go:

```assists
when idle(thrust) >= 1s and speed > 5% -> avoid_collisions(toggle4)
when enemies_near(150m) < 1 -> hold_station(ally, toggle2)
when idle(thrust) >= 2s -> brake
```

In the last list all three rules are on the thrust channel, so they form a priority list: avoidance when something
is in the way, the station when the area is quiet, a brake otherwise.
