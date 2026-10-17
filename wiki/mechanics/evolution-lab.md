---
title: "The evolution lab"
description: "How the EVOLUTION building at home breeds your designs - opening it, how a run works, the setup with what may change and the locks, the run's page, watching and sharing fights and voyages, THE LADDER's rating of your designs, and saving a result with TIDY, REPLACE START and its ORIGIN."
---

# The evolution lab

The [[facility:evolution]] lab is the stack of compute rings above the mind, the mind's own simulation. It takes
designs you have built, breeds variants of them, lets every variant fight the trials you pick, and keeps the best
of each kind. Every fight it fought can be watched again, and any result can be saved as a design. Runs cost
nothing: no data, no materials, no voyage.

What each trial fights, how the goals score and how a stand-in pilot flies your assists are on
[Lab trials, goals and pilots](/mechanics/evolution-trials). Every name and number of the lab is on its data page,
[Evolution lab](/reference/lab), too.

## Opening it

The lab is dark in a new game. Its label says LOCKED, and its tooltip names the research that opens it: the
[[research:simulation_vault]] in the [[family:logic]] tree. It costs [[research:simulation_vault.cost]] and unlocks
[[research:simulation_vault.unlocks]]. A click on the dark building, or its key, takes you to RESEARCH on that node.

Once it is open:

- **[[control:open_evolution.keys]]** or a click on the stack opens it, as a layer over home like FLEET and the LOG;
- **EVOLVE THIS DESIGN** in the drydock's ⋯ menu opens a new run's setup with the design on the dock as its start;
- a click on an evolved design's **ORIGIN** in BLUEPRINTS opens the run it came from, with that design picked.

Esc, CLOSE or the ring top left go back a page, and close the lab from RUNS.

The building says what the lab does: NO RUN, `12 / 50 · BEST 0.81` while a run computes (its racks flicker and a
ring of segments on its roof fills with the generations), PAUSED, and DONE in gold once a run has finished, until
you open it.

## How a run works

A run starts from one to four of your designs (its **starts**). Generation 0 is the starts and variants of them.
Every later generation breeds **children** from the best designs so far: each child is a copy with one or more
**mutations** (a part moved, added or swapped, a behavior rule changed, a number nudged), and some are a cross of two
parents. Every child fights every trial of the run on its seeds, and its **score** is the weighted mean of the goals
over all its fights, from 0 to 1.

The run keeps an **archive**: a grid of cells over two measures of a design, each cell holding the best design found
for it. So a run does not end with one answer but with the best design of each kind: a fast light one, a slow heavy
one, a sniper, a brawler. A run whose design may change spreads its archive by [[lab:spread.design]], one that only
changes the behavior or the assists by [[lab:spread.behavior]].

- **It runs in the background**, at home and on every screen (the drydock, BEHAVIOR, research, the map), within
  [[lab:budget.layer_open]] of every frame while the lab is open and [[lab:budget.elsewhere]] anywhere else, so the
  game stays smooth. In flight (the test range, test flights, replays and voyages) it waits.
- **One run computes at a time.** Starting or resuming another pauses the one that computed.
- While a run computes, a chip **EVOLVING 12 / 50** sits beside the gear ring at home and after the title of FLEET,
  the LOG and SETTINGS. A click on it opens the lab.
- **It is saved after every generation.** Closing or reloading the game loses at most the generation in progress,
  and the run goes on where it was.
- **The same seed, the same setup and the same rules make the same run**, on every device.
- **When the rules change** (a new version of the game changes a part, a ship or the simulation), a run made before
  is marked RULES CHANGED and kept read-only. RE-SCORE makes a new run from it that fights its best designs again
  under today's rules and goes on.

## Setting up a run

NEW RUN opens the setup: one page, top to bottom.

### Start from

One to four starts from YOUR DESIGNS and the templates (the ships of your faction). Each start is a chip; a click on
one shows its locks.

### Locks

What the run keeps of a start, whatever it changes:

- **KEEP ALL** of a category: every part of that category stays where it is.
- **The grid**: the start's cells and two around them. A click on a part locks it (gold): it stays, in its place.
  A click on an empty cell keeps it empty (coral, **KEEP EMPTY**): nothing may be built there, which keeps a line of
  fire clear or a gap where you want one. A second click frees it. Each cell's tip says what it holds.
- **RULES** and **ASSISTS**: a lock on each rule of the behavior and of the pilot assists, ALL of a list, the target
  line, and **LOCK ORDER**, which keeps the rules in their order.
- Under MIRROR a lock holds its mirror image too.

### Limits

- **BUILD POINTS**: at most the start's (the default), a number, or NO CAP.
- **HOLD**: at least the start's cargo, or any.
- **VOYAGE-READY**: no more logic cores than your mind runs beside FLEET's escorts, so the result can launch.
- **MIRROR**: designs stay symmetric (on when the first start is).

A run only builds with the parts and materials you have unlocked.

### May change

What the run's mutations may touch, gene by gene:

| Gene | What it changes |
|---|---|
| [[lab:gene.layout]] | [[lab:gene.layout.summary]] |
| [[lab:gene.parts]] | [[lab:gene.parts.summary]] |
| [[lab:gene.armour]] | [[lab:gene.armour.summary]] |
| [[lab:gene.power]] | [[lab:gene.power.summary]] |
| [[lab:gene.numbers]] | [[lab:gene.numbers.summary]] |
| [[lab:gene.rules]] | [[lab:gene.rules.summary]] |
| [[lab:gene.assist_numbers]] | [[lab:gene.assist_numbers.summary]] |
| [[lab:gene.assist_rules]] | [[lab:gene.assist_rules.summary]] |

The first four are the DESIGN, the others the MIND. PARTS OF picks the categories whose parts may be added, removed
or swapped. The presets switch them in one click:

| Preset | Genes |
|---|---|
| [[lab:gene_preset.everything]] (a new run's) | [[lab:gene_preset.everything.genes]] |
| [[lab:gene_preset.design_only]] | [[lab:gene_preset.design_only.genes]] |
| [[lab:gene_preset.behavior_only]] | [[lab:gene_preset.behavior_only.genes]] |
| [[lab:gene_preset.assists_only]] | [[lab:gene_preset.assists_only.genes]] |
| [[lab:gene_preset.numbers_only]] | [[lab:gene_preset.numbers_only.genes]] |

Everything drifts furthest from what you built; locks and [[lab:gene_preset.numbers_only]] keep a run close to it.
While the assists may change, the fights are FLOWN BY a stand-in pilot as well
([FLOWN BY](/mechanics/evolution-trials#flown-by-and-the-stand-in-pilots)).

### Trials and goals

What the designs fight and what counts as better: [Lab trials, goals and pilots](/mechanics/evolution-trials).

### Size, seed and name

| Size | Children a generation | Generations | |
|---|---|---|---|
| [[lab:size.quick]] | [[lab:size.quick.population]] | [[lab:size.quick.generations]] | [[lab:size.quick.summary]] |
| [[lab:size.standard]] (a new run's) | [[lab:size.standard.population]] | [[lab:size.standard.generations]] | [[lab:size.standard.summary]] |
| [[lab:size.deep]] | [[lab:size.deep.population]] | [[lab:size.deep.generations]] | [[lab:size.deep.summary]] |

Or set CHILDREN and GENERATIONS yourself. The SEED picks the run's random choices (DICE draws a new one), and the
NAME is yours. THE RUN's line under them says how many fights a child and a generation get and how long the run
will take on this device with the lab open, from what this device has measured. START RUN makes it; a setup that
can't run says why.

## Runs

RUNS lists every run, newest first: its name, starts, status, generation, its start's score and its best, and when
it was made. PAUSE or RESUME, OPEN, DUPLICATE (the same setup with a new seed), RENAME and DELETE (it asks first).
The lab keeps at most 32 runs; the list scrolls, and its head says how much storage they take. IMPORT RUN reads a
run file made by the arena tool on a computer.

## The run's page

- **Progress**: generation n of N with the one in progress, fights a second, the time left, the start's score and
  the best; PAUSE or RESUME.
- **SCORES BY GENERATION**: the archive's best, each generation's best and its mean over the generations. A click on
  a generation lists its children, best first, with where each came from and what changed; a click on a child picks
  it.
- **THE ARCHIVE**: a ship card per cell, tinted by its score, the best with a gold edge, a start with a teal corner.
- **The picked design** (the best until you pick another): SCORES per trial and goal, its FIGHTS, its LINEAGE back to
  its start, and COMPARE with the start (the parts gained and lost, the behavior, the assists).
- **VALIDATE**: a run fits the seeds it was fought on. VALIDATE fights the picked design, or the best eight, on twenty
  new seeds and fights their recorded fights again, and shows the VALIDATED score beside the run's.

## Watching and sharing fights

Every fight of a run is kept: what fought, on which seed, and how it ended. WATCH plays it again in the fight
viewer (the run waits meanwhile), and M brings you back to the lab where you were. The fight is simulated again from
the start and checked against the record, so what you watch is what was fought; the viewer says FLOWN BY STAND-IN
when a stand-in pilot flew it.

CODE copies the fight's **fight code** (`SGF1:` and a line of letters). It holds the whole fight, so anyone can
watch it: open the game with `?scene=fight&fight=` and the code. A code carries a fingerprint of the rules it was
fought under: when the game's rules have changed since, the viewer says so and plays it under today's.

A run's **voyages** ([[lab:trial.voyages]]) are watched the same way: WATCH flies the voyage again, the camera on your
ship, as the lab's pilot flew it, and its end is checked against the record; CODE copies its **voyage code**
(`SGV1:`), which plays at the same address.

## The ladder

[[lab:trial.the_ladder]] is the same fixed set of fights for every design: [[lab:trial.the_ladder.rungs]] rungs of
enemies, the weakest first ([[lab:trial.the_ladder.rung_ships]]), each fought on [[lab:trial.the_ladder.seeds]] seeds.
A design's **rating** is the rungs it wins added up, from 0 to the number of rungs (a rung won on two of three seeds
counts about two thirds), so ratings of different designs compare.

THE LADDER at the top of RUNS lists YOUR DESIGNS with their ratings. RATE rates one, RATE ALL every saved design;
the lab fights them one after the other in a run's place (the run waits, as for VALIDATE). A click on a design shows
its rungs: how often it won on each, and WATCH plays its worst fight there, the one that beat it. A rating made
before you changed the design or its behavior (or before the rules changed) says so, and RATE makes it again.
BLUEPRINTS shows a saved design's rating on its preview card. In a run, [[lab:trial.the_ladder]] is a trial like the
others, off in a new run.

## Saving a result

- **SAVE AS DESIGN** puts the picked design into YOUR DESIGNS under a name of your choosing, tagged EVOLVED, its
  line `FROM RIM SKIFF COPY · G37` (its start and generation). An evolved behavior is saved beside it as a behavior
  of yours, `(evolved)` after its name; one that didn't change stays the start's. The pilot assists and weapon groups
  are the design's own.
- **TIDY** (on by default, switched off on the save line): an evolved behavior can grow rules that never run. TIDY
  leaves out every rule that never decided in any of the design's fights; its tip names them. Locked rules stay.
- **REPLACE START** (when the start is one of YOUR DESIGNS) writes the result over that design under its name, so
  FLEET's escorts fly the new version. It asks first.
- **ORIGIN**: an evolved design remembers where it came from. Its preview card in BLUEPRINTS says `G37 · 0.91` and
  the run's name; a click opens the run with the design picked (ITS RUN IS DELETED once you have deleted it). The
  origin travels with the design in files and share codes.
