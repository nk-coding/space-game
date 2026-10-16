---
title: "How ships think"
description: "Behaviors are priority lists of rules. Each think, the first rule that matches picks a built-in maneuver, the flight computer flies it, and the guns fire at will."
---

# How ships think

You fly one ship yourself. Every other ship of yours, and every enemy, flies on its own. What it does is its
**behavior**: a short list of rules that you write and edit on the [BEHAVIOR screen](/behavior/editor). This page
explains how a behavior runs. [The rule text](/behavior/rule-text) explains how to write one, and
[Worked examples](/behavior/examples) shows complete ones.

## Vessels and logic cores

A ship acts only through a **logic core**. A piece of a ship that still has an intact core is a **vessel**: it
thinks, thrusts, powers its systems and raises its shields. A piece without one is salvage: it drifts and can
be scooped up. A [[part:logic_core]] gives a ship [[part:logic_core.rule_slots]] rule slots, a tamed
[[part:automaton_core]] (from [[research:tamed_core]]) gives [[part:automaton_core.rule_slots]]. A ship's
[[stat:rule_slots|rule slots]] are the sum over its cores.

A core must be **working** to count: intact, powered and not overheated. A vessel whose cores are all
unpowered or overheated goes dark. It stays yours, but it does not think, thrust or fire until a core works
again. A vessel that is over your mind capacity goes dormant and does nothing at all.

When a ship breaks apart, every piece that keeps a core becomes a vessel of its own. It takes a copy of the
behavior along, with the same target, but runs only as many rules as its own cores give slots.

## A behavior is a priority list

A behavior has an optional **target line** and a list of **rules**, top to bottom. Each rule is a set of
**conditions** and one **action**:

```behavior
target nearest
when hull < 30% -> flee # too hurt to fight on
when shield_down and target_on_me -> orbit(75%, cw) # dodge while the shields recharge
always -> approach(70%)
```

Several times a second (at [[sim:think_hz]]) the ship **thinks**: it picks its target, then reads its rules from the top. **The first
rule whose conditions all hold decides.** Its action is what the ship flies until the next think. The rules
below it are not even looked at. So order is priority: put the rule that must win (run when badly hurt) above
the rules that fight, and the fallback (`always`) last.

Every ship thinks on the same ticks, and every ship decides from the same picture of the battle before any
decision is applied. No ship reacts a tick sooner because of where it sits in a list, so two equal ships in a
mirror fight stay equal. Between thinks the chosen maneuver keeps flying, and it follows the target as it moves.

<svg class="diagram" viewBox="0 0 600 300" role="img" aria-label="Rules are read from the top. The first rule whose conditions all hold decides; the rules below it are not read; rules beyond the rule slots never run; if nothing matches, the ship stops and holds.">
  <text x="40" y="18" text-anchor="middle">THINK</text>
  <line x1="40" y1="26" x2="40" y2="262" class="line" marker-end="url(#arrow)" />
  <circle cx="40" cy="60" r="4" class="f-dim" />
  <line x1="44" y1="60" x2="70" y2="60" class="line" />
  <rect x="70" y="47" width="130" height="26" rx="13" class="s-violet f-bg" />
  <text x="135" y="64" text-anchor="middle" class="t-violet">HULL &lt; 30%</text>
  <line x1="200" y1="60" x2="340" y2="60" class="line" />
  <rect x="340" y="47" width="100" height="26" rx="13" class="s-teal f-bg" />
  <text x="390" y="64" text-anchor="middle" class="t-teal">FLEE</text>
  <text x="456" y="64">FALSE · NEXT RULE</text>
  <circle cx="40" cy="110" r="4" class="f-gold" />
  <line x1="44" y1="110" x2="70" y2="110" class="line" />
  <rect x="70" y="97" width="122" height="26" rx="13" class="s-violet f-bg" />
  <text x="131" y="114" text-anchor="middle" class="t-violet">SHIELD DOWN</text>
  <rect x="200" y="97" width="128" height="26" rx="13" class="s-violet f-bg" />
  <text x="264" y="114" text-anchor="middle" class="t-violet">TARGET ON ME</text>
  <line x1="328" y1="110" x2="340" y2="110" class="line" />
  <rect x="340" y="97" width="100" height="26" rx="13" class="s-teal f-bg thick glow" />
  <text x="390" y="114" text-anchor="middle" class="t-teal">ORBIT</text>
  <text x="456" y="114" class="t-gold">ALL TRUE · DECIDES</text>
  <circle cx="40" cy="160" r="4" class="f-dim" />
  <line x1="44" y1="160" x2="70" y2="160" class="line" />
  <rect x="70" y="147" width="90" height="26" rx="13" class="s-dim f-bg" />
  <text x="115" y="164" text-anchor="middle">ALWAYS</text>
  <line x1="160" y1="160" x2="340" y2="160" class="line" />
  <rect x="340" y="147" width="100" height="26" rx="13" class="s-dim f-bg" />
  <text x="390" y="164" text-anchor="middle">APPROACH</text>
  <text x="456" y="164">NOT READ</text>
  <line x1="56" y1="196" x2="580" y2="196" class="s-coral dotted" />
  <text x="456" y="190" class="t-coral">RULE SLOTS END</text>
  <circle cx="40" cy="226" r="4" class="f-dim" />
  <line x1="44" y1="226" x2="70" y2="226" class="line" />
  <rect x="70" y="213" width="150" height="26" rx="13" class="s-dim f-bg" />
  <text x="145" y="230" text-anchor="middle">TARGET HULL &lt; 20%</text>
  <line x1="220" y1="226" x2="340" y2="226" class="line" />
  <rect x="340" y="213" width="100" height="26" rx="13" class="s-dim f-bg" />
  <text x="390" y="230" text-anchor="middle">APPROACH</text>
  <text x="456" y="230">NEVER RUNS</text>
  <text x="70" y="284">NO RULE MATCHES: STOP AND HOLD</text>
</svg>

*Rules are read from the top; the first one whose conditions all hold decides, and nothing below it is read. A rule beyond the rule slots never runs.*

## Conditions

Conditions come before the arrow. All conditions of a rule must hold for the rule to match; they are checked
left to right and the first one that fails ends the rule. There are two kinds:

- **Flags** are true or false and stand alone: `shield_down`, `under_attack`, `target_in_range(60m)`.
- **Values** measure something, and a rule compares them with `<`, `<=`, `>` or `>=`: `hull < 30%`,
  `enemies_near(150m) >= 3`, `cycle(9s) >= 5s`.

`not` in front of a condition turns it around: `not target_in_range(100%)` holds while the target is out of
reach. There is no `or`: for "this or that", write two rules with the same action.

A condition **about the target** ([[behavior_node:target_in_range]], [[behavior_node:target_hull]],
[[behavior_node:target_on_me]], [[behavior_node:cycle]], [[behavior_node:target_time]], and a script that asks
about the target) is **false while there is no target, with or without `not`**. A condition with nothing to
measure is false the same way: [[behavior_node:shield]] on a ship without a working shield emitter, or
[[behavior_node:speed]] while no thruster can push.

<NodeTable role="condition" />

## Actions

The action after the arrow is a **built-in maneuver**: approach to a distance, circle the target, keep a
distance, face it, flee, hold, escort a leader. A behavior never touches the thrusters itself. The ship's flight
computer flies the maneuver every tick with what the ship has, within its top speed, so a behavior can never
send a ship spinning. Maneuvers aim at the target's aim point, and lead a moving target when the ship's guns
fire projectiles.

**An action that needs a target lets the next rule run while there is none.** A rule like `always -> approach`
matches, finds no target and steps aside; the rules below it get their turn. [[behavior_node:hold]] and
[[behavior_node:escort]] need no target, so a rule with one of them always runs once its conditions hold.

If no rule decides, the ship **stops and holds**: it brakes to a standstill and waits for the next think.

<NodeTable role="action" />

### Escorts, wingmen and leaders

A ship can have a **leader**, the ship it escorts. Your escorts on a voyage have your flagship as their leader,
and in a test flight the wingman's leader is the ship you fly. Two words of the vocabulary work through the
leader:

- the target [[behavior_node:locked]] picks what the leader aims at: your lock while you fly the leader, else
  the leader's own behavior target. An escort with it never picks its own fights;
- the action [[behavior_node:escort]] keeps the ship near its leader: it closes in when farther than the
  distance and keeps pace when near. Without a leader it holds where it is. Put it last, below the rules that
  fight, so it runs between fights.

## Targets and aim points

The target line picks the one ship all the rules are about. Without a target line a behavior uses
[[behavior_node:nearest]]. Only active enemy vessels count: salvage, dark and dormant ships are no targets
(except for [[behavior_node:locked]], which takes whatever the leader aims at). When the current target goes
dark, for example because its reactors are gone, the ship picks another.

Each selector takes an **aim point**: the ship's centre, or the nearest working engine, gun, reactor or logic
core on it. `target weakest(engines)` goes for the weakest enemy and shoots at its engines. Maneuvers and guns
use the aim point; distances in conditions are measured to the target ship.

A ship **keeps its target** until another is clearly better: a new candidate must score better than
[[sim:target_sticky]] of the current target's score (for `nearest`, it must be that much closer). So a ship does
not flip between two enemies that are nearly equal. The timers [[behavior_node:cycle]] and
[[behavior_node:target_time]] count from the moment the current target was picked.

<NodeTable role="target" />

## Fire at will

Whatever the action, the guns **fire at will**, each gun on its own. A gun fires when the aim point is within
[[sim:fire_range]] of **that gun's** reach, measured from its muzzle, and within [[sim:fire_cone]] of the way
**that gun** faces. A cannon checks the point where its shot will meet the target, not where the target is now.
Front guns fire while the ship faces the target; side and rear guns fire when the target crosses their line, for
example while the ship circles it. A gun whose line of fire is blocked by the ship's own parts never fires.

Guns fire whenever there is a target, even while the ship flees, holds or escorts, and even when no rule
matches. A vessel that has lost every working engine fights on as a turret: whatever its rule says, it turns
its strongest guns onto its target with what it still has.

### Distances as a share of the reach

A distance written with `%` is a share of the ship's **weapon reach**, the range of its longest working gun:
`approach(67%)` stops at two thirds of it. One behavior then fits ships with short and long guns. A ship
without a working gun counts a reach of [[sim:default_reach]].

## Rule slots

A ship runs only as many rules as its working logic cores give **rule slots**. The rules beyond that line are
**inactive**: they stay in the behavior but are never read. The BEHAVIOR screen draws the line and dims the
rules below it.

When a core is shot away or overheats, the slot count drops and the **lowest rules switch off first**. Keep the
rules that matter most at the top. The [BEHAVIOR screen](/behavior/editor) shows your design's slots, and a
design's page shows its [[stat:rule_slots|rule slots]], for example [[ship:halvex_runner.rule_slots]] for the
[[ship:halvex_runner]].

## Which behavior a ship flies

- **A design's own behavior.** A design can name a behavior, one of the templates or one of yours. You set it
  on the BEHAVIOR screen.
- **The faction's default.** A design that names none flies its faction's: [[behavior:skirmisher]] for the
  [[faction:ship_mind]], [[behavior:raider_basic]] for the [[faction:automatons]]. A design whose behavior can't
  fly (it has errors or was deleted) flies its faction's too, and a banner says so.
- **The ship you fly.** The ship you pilot parks its behavior while you fly it; you have the helm, with its
  [pilot assists](/behavior/pilot-assists). The moment you switch to another vessel, the one you left runs its
  behavior again.

A behavior with more rules than its design has rule slots is kept as it is. The rules beyond the slots simply
don't run.

## Script conditions (Rhai)

Most conditions are built into the game. A condition can also be a small script in the Rhai language, shipped
in the content like every other word of the vocabulary. The editor draws them with a dashed outline and a
gold SCRIPT tag. The base game has one: [[behavior_node:closing_in]].

A script reads a few numbers about your ship and its target (hull, shield, heat, power, position, speed, weapon
reach, distance, bearing, closing speed, whether it aims at you) and answers true or false. It runs sealed off:
it can read but not change anything, it has no clock, no randomness, no files and no output, and its maths gives
the same result on every device, so fights replay the same everywhere. Each run has a budget of
[[sim:script_max_ops]] operations. A script that runs out, fails or answers anything but true or false counts as
**false**, with or without `not`, and the test panel shows why. Modders can add their own: see
[The rule text](/behavior/rule-text#script-conditions-for-modders).

::: tip Pilot assists
Your own ship can carry a second rule list, its PILOT ASSISTS, written in the same format. Assists help you fly
and run only while you pilot the ship. See [Pilot assists](/behavior/pilot-assists).
:::
