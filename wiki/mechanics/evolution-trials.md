---
title: "Lab trials, goals and pilots"
description: "What the evolution lab's designs fight and how they are scored - the trials and their kinds, weights and seeds, the four goals and their presets, FLOWN BY and the stand-in pilots, and the ARMS RACE, an enemy that evolves against you."
---

# Lab trials, goals and pilots

A run of the [evolution lab](/mechanics/evolution-lab) scores every child by its fights. **Trials** say what it
fights, **goals** what counts as better, and **FLOWN BY** who flies it. All of it is set on the run's setup page and
kept with the run, so a run never changes once it is made.

## Trials

A trial is a set of fights. Each trial in the setup is a row: a pill that switches it on, its **WEIGHT** (0 to 3; 0
leaves it out) and its **SEEDS** (how many times each opponent is fought, on different random starts). A child's
score is the weighted mean over every trial that is on. A fight ends when one side has no running vessel left, or
at the trial's time limit.

| Trial | Kind | Weight | Seeds | Time limit | In a new run |
|---|---|---|---|---|---|
| [[lab:trial.enemies_you_meet]] | [[lab:trial.enemies_you_meet.kind]] | [[lab:trial.enemies_you_meet.weight]] | [[lab:trial.enemies_you_meet.seeds]] | [[lab:trial.enemies_you_meet.time_limit]] | [[lab:trial.enemies_you_meet.default]] |
| [[lab:trial.test_range_waves]] | [[lab:trial.test_range_waves.kind]] | [[lab:trial.test_range_waves.weight]] | [[lab:trial.test_range_waves.seeds]] | [[lab:trial.test_range_waves.time_limit]] | [[lab:trial.test_range_waves.default]] |
| [[lab:trial.beat_your_start]] | [[lab:trial.beat_your_start.kind]] | [[lab:trial.beat_your_start.weight]] | [[lab:trial.beat_your_start.seeds]] | [[lab:trial.beat_your_start.time_limit]] | [[lab:trial.beat_your_start.default]] |
| [[lab:trial.hall_of_fame]] | [[lab:trial.hall_of_fame.kind]] | [[lab:trial.hall_of_fame.weight]] | [[lab:trial.hall_of_fame.seeds]] | [[lab:trial.hall_of_fame.time_limit]] | [[lab:trial.hall_of_fame.default]] |
| [[lab:trial.enemy_ships]] | [[lab:trial.enemy_ships.kind]] | [[lab:trial.enemy_ships.weight]] | [[lab:trial.enemy_ships.seeds]] | [[lab:trial.enemy_ships.time_limit]] | [[lab:trial.enemy_ships.default]] |
| [[lab:trial.raider_gauntlet]] | [[lab:trial.raider_gauntlet.kind]] | [[lab:trial.raider_gauntlet.weight]] | [[lab:trial.raider_gauntlet.seeds]] | [[lab:trial.raider_gauntlet.time_limit]] | [[lab:trial.raider_gauntlet.default]] |
| [[lab:trial.your_fleet]] | [[lab:trial.your_fleet.kind]] | [[lab:trial.your_fleet.weight]] | [[lab:trial.your_fleet.seeds]] | [[lab:trial.your_fleet.time_limit]] | [[lab:trial.your_fleet.default]] |
| [[lab:trial.champions]] | [[lab:trial.champions.kind]] | [[lab:trial.champions.weight]] | [[lab:trial.champions.seeds]] | [[lab:trial.champions.time_limit]] | [[lab:trial.champions.default]] |
| [[lab:trial.voyages]] | [[lab:trial.voyages.kind]] | [[lab:trial.voyages.weight]] | [[lab:trial.voyages.seeds]] | [[lab:trial.voyages.time_limit]] | [[lab:trial.voyages.default]] |
| [[lab:trial.the_ladder]] | [[lab:trial.the_ladder.kind]] | [[lab:trial.the_ladder.weight]] | [[lab:trial.the_ladder.seeds]] | [[lab:trial.the_ladder.time_limit]] | [[lab:trial.the_ladder.default]] |
| [[lab:trial.arms_race]] | [[lab:trial.arms_race.kind]] | [[lab:trial.arms_race.weight]] | [[lab:trial.arms_race.seeds]] | [[lab:trial.arms_race.time_limit]] | [[lab:trial.arms_race.default]] |

What each fights:

- [[lab:trial.enemies_you_meet]]: [[lab:trial.enemies_you_meet.summary]]
- [[lab:trial.test_range_waves]]: [[lab:trial.test_range_waves.summary]]
- [[lab:trial.beat_your_start]]: [[lab:trial.beat_your_start.summary]]
- [[lab:trial.hall_of_fame]]: [[lab:trial.hall_of_fame.summary]]
- [[lab:trial.enemy_ships]]: [[lab:trial.enemy_ships.summary]]
- [[lab:trial.raider_gauntlet]]: [[lab:trial.raider_gauntlet.summary]]
- [[lab:trial.your_fleet]]: [[lab:trial.your_fleet.summary]]
- [[lab:trial.champions]]: [[lab:trial.champions.summary]]
- [[lab:trial.voyages]]: [[lab:trial.voyages.summary]]
- [[lab:trial.the_ladder]]: [[lab:trial.the_ladder.summary]]
- [[lab:trial.arms_race]]: [[lab:trial.arms_race.summary]]

Some rows have a number of their own: [[lab:trial.test_range_waves]] counts [[lab:trial.test_range_waves.target]]
waves cleared as a win, [[lab:trial.hall_of_fame]] fights the run's best [[lab:trial.hall_of_fame.count]] and
[[lab:trial.champions]] the best [[lab:trial.champions.count]] of the earlier runs; [[lab:trial.enemy_ships]] lists
each enemy ship with a count ([[lab:trial.enemy_ships.ships]] by default). The [[lab:trial.raider_gauntlet]] sends
[[lab:trial.raider_gauntlet.waves]] waves: [[lab:trial.raider_gauntlet.wave_ships]].

What a trial fights from outside the run is read when the setup opens: the voyages open to you, YOUR DESIGNS (FLEET's
escorts first) and the best design of every other run. A trial with nothing to fight says why in coral (NO VOYAGE
OPEN, NO SAVED DESIGN, NO EARLIER RUN YET). **WITH YOUR ESCORTS** flies FLEET's escorts beside the design in every
trial.

### Trial kinds

Every trial is one of these kinds with its own settings:

- [[lab:kind.encounters]]: [[lab:kind.encounters.summary]]
- [[lab:kind.enemy_ships]]: [[lab:kind.enemy_ships.summary]]
- [[lab:kind.waves]]: [[lab:kind.waves.summary]]
- [[lab:kind.gauntlet]]: [[lab:kind.gauntlet.summary]]
- [[lab:kind.beat_your_start]]: [[lab:kind.beat_your_start.summary]]
- [[lab:kind.fleet]]: [[lab:kind.fleet.summary]]
- [[lab:kind.hall_of_fame]]: [[lab:kind.hall_of_fame.summary]]
- [[lab:kind.champions]]: [[lab:kind.champions.summary]]
- [[lab:kind.voyages]]: [[lab:kind.voyages.summary]]
- [[lab:kind.ladder]]: [[lab:kind.ladder.summary]]
- [[lab:kind.co_evolution]]: [[lab:kind.co_evolution.summary]]

The new run's selection mixes what you will meet with bars that keep rising: the enemies of your open voyages count
double, the test-range waves are fair to a design of any size, beating your start says whether the run improves on
what you had, and the hall of fame keeps scores from stalling once the enemies are beaten.

## Goals

Every fight scores four goals, each from 0 to 1:

| Goal | What it scores | Weight in a new run |
|---|---|---|
| [[lab:goal.win]] | [[lab:goal.win.summary]] | [[lab:goal.win.weight]] |
| [[lab:goal.unhurt]] | [[lab:goal.unhurt.summary]] | [[lab:goal.unhurt.weight]] |
| [[lab:goal.quick]] | [[lab:goal.quick.summary]] | [[lab:goal.quick.weight]] |
| [[lab:goal.cheap]] | [[lab:goal.cheap.summary]] | [[lab:goal.cheap.weight]] |

A child's score is each goal's mean over its fights, weighted by the goals' weights (0 to 3, at least one above 0).
With [[lab:goal.win]] alone a design that wins every fight scores 1, and many may; the other goals then tell them
apart. The presets set the weights in one click:

| Preset | Weights |
|---|---|
| [[lab:goal_preset.win]] | [[lab:goal_preset.win.weights]] |
| [[lab:goal_preset.win_unhurt]] | [[lab:goal_preset.win_unhurt.weights]] |
| [[lab:goal_preset.win_cheaply]] | [[lab:goal_preset.win_cheaply.weights]] |
| [[lab:goal_preset.win_fast]] | [[lab:goal_preset.win_fast.weights]] |

SCORES on the run's page shows the picked design's score per trial and goal, the goals that weigh lit.

## FLOWN BY and the stand-in pilots

In a run nobody is at the controls, so a design flies its **behavior**. Your [pilot assists](/behavior/pilot-assists)
only act on a ship you pilot, so they would never be scored. **FLOWN BY** says who flies the design in the fight
trials:

- **BEHAVIOR**: the design's behavior, as an escort flies.
- **STAND-IN**: a **stand-in pilot** flies it as its side's focus, with the design's assists taking what it leaves
  alone, as they do for you.
- **BOTH**: the behavior on odd seeds, the stand-in on even ones. A new run takes BOTH while the assists may change,
  and BEHAVIOR otherwise.

The stand-ins:

| Pilot | Locks a target | Closes to (of its gun's reach) | Stick | Triggers |
|---|---|---|---|---|
| [[lab:pilot.casual]] | [[lab:pilot.casual.lock]] | [[lab:pilot.casual.close_to]] | [[lab:pilot.casual.stick]] | [[lab:pilot.casual.fire]] |
| [[lab:pilot.hands_off]] | [[lab:pilot.hands_off.lock]] | [[lab:pilot.hands_off.close_to]] | [[lab:pilot.hands_off.stick]] | [[lab:pilot.hands_off.fire]] |
| [[lab:pilot.busy]] | [[lab:pilot.busy.lock]] | [[lab:pilot.busy.close_to]] | [[lab:pilot.busy.stick]] | [[lab:pilot.busy.fire]] |

- [[lab:pilot.casual]]: [[lab:pilot.casual.summary]]
- [[lab:pilot.hands_off]]: [[lab:pilot.hands_off.summary]]
- [[lab:pilot.busy]]: [[lab:pilot.busy.summary]]

A stand-in is not you: assists tuned to it are a starting point. Try the result in the test range. The fight viewer
says FLOWN BY STAND-IN and the pilot's name for a fight a stand-in flew.

## The arms race

The [[lab:trial.arms_race]] (off in a new run) evolves an **enemy population** against yours in the same run. It
starts from enemy ships you pick under its row (THE ENEMY FROM; one faction at a time, its boss only when picked) and
breeds [[lab:trial.arms_race.size]] as many children as yours. The automatons may build with
[[lab:palette.automatons]] besides their ships' own parts.

- Your children fight the enemy's best [[lab:trial.arms_race.elites]] and [[lab:trial.arms_race.hall]] of its past
  champions; its children fight your best, your past champions and your starts, so neither side runs away from the
  enemies you really meet.
- Every [[lab:trial.arms_race.check_every]] generations, **PAST CHAMPIONS** fights both sides' best against the other
  side's champions of every [[lab:trial.arms_race.champions_every]]th generation. Beating old champions means a real
  arms race; losing to them means the two sides go round in circles.
- The run's page switches between **YOURS | THE ENEMY**: the archive, the picked design and the generations of
  either side. The score chart adds the enemy's best and the PAST CHAMPIONS checks as dots.
- An evolved enemy can be saved as a design (it keeps its faction and is tagged as an enemy) or copied with EXPORT
  as a content ship for a pack.

It costs about half a run more, and it can drift from the enemies the career has: keep a trial of the enemies you
meet beside it.

## Checking a result

- **VALIDATE** fights designs on new seeds to show how far a score fits the seeds the run was fought on, and fights
  their recorded fights again to show they end as recorded.
- **RE-SCORE** brings a run fought under old rules to today's: a new run that fights its best designs again first.
- Watch the fights, and fly the design yourself: the run's score says how it did against the trials, not how it
  feels to fly.
