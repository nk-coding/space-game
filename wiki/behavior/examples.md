---
title: "Worked examples"
description: "Eight behaviors from a simple sentry to a scripted picket, each explained rule by rule, and a walk through the three behaviors the game ships with."
---

# Worked examples

Each example below is a complete behavior you can type into the TEXT view of the
[BEHAVIOR screen](/behavior/editor). They go from simple to advanced. Every one fits the
[[part:logic_core.rule_slots]] rule slots of a single [[part:logic_core]]. Two things to keep in mind while
reading them (both explained on [How ships think](/behavior/)):

- **The first rule that matches decides.** Order is priority: what must win goes on top, the fallback last.
- **The guns fire at will** whatever the rule says. A behavior only decides where the ship goes and which way
  its nose points; each gun fires on its own when the target is in front of it and within its reach.

## 1. Sentry

A ship that stays where it is and turns its guns onto anything that comes close.

```behavior
target nearest
when target_in_range(110%) -> face # in reach: turn the guns onto it
always -> hold # else stay where you are
```

[[behavior_node:face]] turns the nose onto the target and matches its velocity, so a passing target doesn't
slip out of the guns' cone. The range is a little over the weapon reach: the sentry turns before the target
arrives, and the guns start firing once it is within [[sim:fire_range]] of their reach.

[[behavior_node:hold]] needs no target, so `always -> hold` decides whenever the first rule doesn't. Swap the
two lines and the sentry would never turn: `always` matches every time, and nothing below it is read.

::: tip
A ship that has lost every working engine already fights like this on its own: whatever its rule says, it turns
its strongest guns onto its target.
:::

## 2. Brawler

For short guns: get close and stay close, circle now and then so the side guns get their turn, and back off
before the parts overheat.

```behavior
target nearest
when heat > 85% -> kite(150%) # running hot: back off and cool down
when target_in_range(60%) and cycle(10s) >= 6s -> orbit(40%) # circle now and then so the side guns bear
always -> approach(40%)
```

- The heat rule is on top because nothing else matters once a part is about to shut down.
  [[behavior_node:heat]] is the hottest part's heat as a share of where it shuts down.
- [[behavior_node:cycle]] counts the seconds of a repeating cycle from the moment the target was picked:
  `cycle(10s) >= 6s` holds for the last four seconds of every ten. Together with `target_in_range(60%)` the
  brawler circles only once it is close.
- The rest of the time, [[behavior_node:approach]] closes in to 40% of the reach and keeps the nose on the target.

## 3. Kiter

For long guns: stay at the edge of your reach, where shorter guns can't answer.

```behavior
target threat
when target_in_range(70%) -> kite(95%) # it got too close: open the gap
always -> approach(90%)
```

[[behavior_node:threat]] picks the enemy with the most firepower, weighted up when it aims at you and down
with distance. While the target is far, [[behavior_node:approach]] flies in nose first and stops just inside
the reach. When it comes closer than 70%, [[behavior_node:kite]] opens the gap again.

The kite rule must come first: `always -> approach` would otherwise win every time. A ship that can thrust
backwards keeps its nose and its front guns on the target while it backs off; one with engines only at the back
has to turn away to open the gap, and its front guns fall silent meanwhile. Side or rear guns help such a kiter.

## 4. Hit and run

Fight until hurt, run while someone shoots at you, and come back when they look elsewhere.

```behavior
target nearest
when hull < 40% and under_attack -> flee # hurt and someone aims at me: run
when hull < 40% -> kite(85%) # hurt but left alone: come back and shoot from the edge of reach
always -> approach(70%)
```

Hull does not heal during a fight, so "come back" can't wait for the hull. It waits for
[[behavior_node:under_attack]] to turn false instead: no enemy aims at you any more. In a fleet that happens
often: enemies keep the target they have only until another is clearly better, so once the hurt ship runs off,
they turn to a closer one, and it returns, carefully, at the edge of its reach.

Both hurt rules start with `hull < 40%`. The narrower one (also under attack) must stand above the wider one,
or the wider one would always win.

## 5. Shield dancer

For a ship with a [[part:shield_emitter]]: keep moving while a bubble is down, and keep your distance while the
bubbles are low.

```behavior
target nearest
when shield_down and target_on_me -> orbit(85%, ccw) # a bubble is down and its guns are on me: keep moving
when shield < 40% -> kite(100%) # bubbles low: fight from the edge of reach
always -> approach(65%)
```

- [[behavior_node:shield_down]] holds while one of your bubbles has collapsed and is recharging.
  [[behavior_node:target_on_me]] holds while your target aims at you. Together: dodge only when it matters.
- [[behavior_node:shield]] measures the charge left. A ship without a working shield emitter has nothing to
  measure, so the condition is false and the rule steps aside: the same behavior still works on an unshielded
  ship, which then simply approaches.

## 6. Hunter

Pick off the weakest enemy and cripple it first, but stay out of a pack.

```behavior
target weakest(engines)
when enemies_near(120m) >= 3 -> kite(90%) # a pack close by: stay out of it
when target_hull < 25% -> approach(45%) # nearly done: close in for the kill
always -> approach(80%)
```

[[behavior_node:weakest]] picks the enemy with the least hull left. The aim point `engines` aims at its nearest
working engine: shot off, the target can no longer run. Once the last engine is gone the hunter aims at the
ship's centre.

The pack rule comes first: finishing a kill inside three enemies' guns is how hunters die. Swap the two upper
rules if your design is tough enough to dive in. [[behavior_node:enemies_near]] counts every active enemy ship
within the radius, not only the target.

## 7. Wingman

Fight only what your leader has locked, and stay at its side between fights.

```behavior
target locked(weapons)
when hull < 35% -> escort(15m) # badly hurt: tuck in beside the leader
when target_in_range(400%) -> approach(75%) # the leader's target, unless it is far off
always -> escort(40m) # nothing locked: keep near the leader
```

[[behavior_node:locked]] picks what the leader aims at: while you fly the leader, your lock. Your escorts on a
voyage have your flagship as their leader. Without a lock there is no target. Then the second rule is false (a
target condition without a target always is) and the third keeps the wingman near the leader. The aim point
`weapons` disarms the target first.

[[behavior_node:escort]] needs no target, which makes the first rule work in any situation: a badly hurt
wingman flies close to its leader, where the leader's guns cover it. On a design with only two rule slots the
last rule would not run, so give a wingman at least three.

## 8. Picket (a script condition)

Wait until something attacks, then meet it nose first.

```behavior
target attacker
when closing_in(3s) and not target_in_range(100%) -> face # coming at me: meet it nose first
when target_in_range(100%) -> orbit(70%)
always -> hold # nobody aims at me: wait
```

[[behavior_node:attacker]] picks the nearest enemy that aims at you, and none while nobody does. So the picket
holds until it is attacked, and never starts a fight by itself.

[[behavior_node:closing_in]] is a script condition (SCRIPT in the editor): it holds while, at the current closing
speed, the target will be within your reach in less than the given time. Combined with
`not target_in_range(100%)` it turns the picket towards an attacker that is still out of reach, so its front guns
are ready when the attacker arrives. Inside the reach, the second rule circles the attacker.

## The behaviors that come with the game

The game ships three behaviors. Their pages show their rules; on the BEHAVIOR screen they are TEMPLATES: open
one, change it, and SAVE makes it your own copy.

### Raider pack

[[behavior:raider_basic]] is the default of the [[faction:automatons]]. It has two rules, so it fits the
[[part:automaton_core.rule_slots]] rule slots of a single [[part:automaton_core]]. Out of range, the raider
approaches its nearest enemy. Once a little beyond its weapon reach, it alternates on a cycle: a short attack
run nose first, then a longer stretch circling the target. A pack never sits still in front of your guns, and
because each raider's cycle starts when it picked its target, the raiders of a pack are seldom in step.
All distances are shares of the reach, so the same two lines fit any raider gun.

### Skirmisher

[[behavior:skirmisher]] is what your own unpiloted vessels fly unless their design names another behavior: the
default of the [[faction:ship_mind]]. It reads top to bottom as a list of worries:

1. badly hurt: [[behavior_node:flee]];
2. a shield bubble down while the target aims at it: circle to dodge;
3. a fast attacker still out of reach ([[behavior_node:closing_in]], the script condition): turn and meet it
   nose first;
4. else approach and keep the nose on the target.

It was written for ships with engines only at the back. It keeps fighting nose first, since every turn away and
back again costs seconds of fire.

### Escort

[[behavior:escort]] is the wingman template. Its target line is [[behavior_node:locked]] with the aim point
`core`: it fights only what its leader has locked, so it never wakes a sleeping pack by itself, and it shoots at
the target's logic core, which leaves more of the wreck to salvage. Its rules:

1. badly hurt and under the target's guns: [[behavior_node:kite]], backing off well beyond its own reach;
2. `always ->` approach the target, inside every gun's reach;
3. `always ->` [[behavior_node:escort]], to keep near the leader.

The second rule is an `always` rule, yet the third still runs: approach needs a target and steps aside while the
leader has nothing locked. On a tamed automaton core with only two slots, the escort rule would not run.
