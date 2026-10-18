---
title: "Research, the fleet and the career"
description: "How progression works - the career save, research families and research data, research nodes and what they unlock, mind capacity, and the FLEET of escorts that flies with you."
---

# Research, the fleet and the career

Your **career** is everything you have achieved: what you have researched, what you have in store, which escorts
fly with you and which voyages you have found and cleared. It grows one voyage at a time. Salvage comes home,
becomes research data, and research turns that data into new parts, armour, frames and room in your mind for more
ships.

## The career save

The game keeps one career. It is saved by itself, never with a save button:

- when a new game begins;
- when you change your FLEET;
- when you first pick (or select) a NEW part in the drydock;
- when you open the LOG with a result you haven't seen;
- when you research a node;
- when a voyage starts, and when it ends.

It is never saved during a voyage. So if you close or reload the game in the middle of one, that voyage counts as
lost when you come back.

The save holds only what you did:

- the start your game began from;
- the research nodes you have researched;
- the research data in store, per family, and the materials in store;
- the workshop's stock of parts, and the hauled parts still waiting in the last voyage's report to be sorted;
- your escorts;
- which NEW parts you have already seen;
- for every voyage: how often you launched it, whether you cleared it, and your best time;
- the counts the LOG shows (voyages, recalls, losses, items salvaged) and the last [[sim:max_history]] voyage results;
- your [galaxy](/mechanics/galaxy): the map it was generated with, the clusters you have charted, the signals you
  have heard and how far you have narrowed them, the logs waiting to be decoded and the gates you have opened.

Everything else (what is unlocked, your mind capacity, your jump range, which voyages are open) is worked out from that whenever it
is needed. So when a new version of the game changes a research cost or what a node unlocks, your old save simply
follows the new rules. Your designs are not part of the career: they live in BLUEPRINTS.

If a save can't be read (it is damaged, or was written by a newer version of the game), the game never writes over
it. It runs a fresh career that is not saved, and says so in the notes at the shipyard (THE SAVE IS KEPT AS IT IS).
Until the game is released, a save from an older version is not carried over: a new career starts in its place,
and the notes say so (A NEW GAME).

::: details Starting over
The launch option `?newgame=1` starts a new career in place of the old one. Your last design is saved to
BLUEPRINTS first, and the drydock starts again with a copy of the start's ship.
:::

## Research data

Research is paid with **research data**, and data comes only from salvage. What your fleet brings home is
**dismantled**: broken parts and hull the moment you arrive, intact parts when you dismantle them in the voyage's
report instead of keeping them for the [[facility:workshop]] ([salvage and cargo](/mechanics/salvage-and-cargo)). Each part gives
research data to one **research family**:

[[family:hull]], [[family:power]], [[family:energy]], [[family:kinetic]], [[family:shields]],
[[family:thermal]], [[family:propulsion]], [[family:logic]] and [[family:utility]]. Each family is one tech tree in
the lab, and each research node's cost names the family data it takes.

A part gives its own amount of data to its family, and a share of the materials in its recipe. A hull part gives its armour's
data for every cell it covers (a slope counts half a cell). A **fragment** (what a destroyed logic core leaves
behind) gives materials but no data. A few examples:

| Salvaged | Gives |
|---|---|
| [[part:automaton_cannon]] | [[part:automaton_cannon.salvage]] |
| [[part:automaton_core]] | [[part:automaton_core.salvage]] |
| [[part:logic_core]] | [[part:logic_core.salvage]] |
| [[part:pulse_laser]] | [[part:pulse_laser.salvage]] |
| [[part:fission_reactor]] | [[part:fission_reactor.salvage]] |
| [[part:thruster]] | [[part:thruster.salvage]] |
| a hull cell in [[material:steel]] | [[material:steel.salvage]] |

Raiders and hunters are built from [[part:automaton_cannon|automaton cannons]], [[part:automaton_core|automaton
cores]], [[part:thruster|thrusters]] and plating, so their wrecks feed
[[family:kinetic]], [[family:logic]], [[family:propulsion]] and [[family:hull]] first. Old precursor hulls in the
wreck fields carry the parts of other families.

### Programmable matter

Some research also costs a material: [[material:programmable_matter]], the precursor matter that hosts a copy of
the ship-mind. It comes from logic cores: an intact [[part:automaton_core]] or [[part:logic_core]] gives some when
it is dismantled, and a core destroyed in a fight leaves a small fragment of it to scoop up. In a fight a raider's
core is usually what you destroy to stop it, so look for intact cores in wrecks, such as the
[[encounter:core_graveyard]].

### Gate shards

A [[material:gate_shard]] is a shard of precursor gate lattice. Each gate's guardian gives one when it is beaten: the
Warden the first time you clear [[voyage:wardens_gate]], every other guardian at the end of its gate's trail (a
[[encounter:gate_sentinel]] at the rim, a [[encounter:bastion_gate|bastion]] in the outer reach,
[[encounter:keeper_gate|the Keeper]] further in). Nothing else gives them, so the rim holds three and each band
further in three more. Only research spends them:
[[research:reactive_plating]] and the [[research:automaton_forge]] take one each, the [[research:capital_drydock]]
three.

## Research nodes

Open the lab from home ([[control:open_research.keys]], or a click on the lab with the two dishes). The screen shows
one lane per family that has research, and in each lane its nodes as cards from left to right in tiers: a node
sits one tier after the deepest node it needs. Connectors run from each requirement to the nodes that need it,
solid teal once the requirement is done. The head of each lane shows the family's data in store; families without
research show theirs at the top right as ALSO IN STORE, next to your materials.

A card shows what it unlocks, its name and a mark:

- a **gold diamond** and a gold outline: you can pay for it now (ready);
- **DONE**: researched;
- a **lock**: a node it needs isn't researched yet;
- nothing: your stores fall short.

Under the card, its kind and its cost: each cost in gold when your store pays it, `HELD / COST` in grey when not.

Click a card (or move between cards with the arrow keys) to see its detail on the right: what it unlocks (a part with its stats, an armour, a frame's grid, or
your mind before and after), COST against IN STORE, UNLOCKS in words, and REQUIRES. **UNLOCK** pays and unlocks at
once. There is no research time. The line above the button says what happened, or why it can't happen
(`NEEDS … FIRST`, `NOT ENOUGH DATA`, `NOT ENOUGH MATERIALS`). At the bottom, `‹ HOME`, the nodes that are ready now, and DRYDOCK
([[control:open_drydock.keys]]).

### Kinds

| Kind | What it is |
|---|---|
| NEW PART | a new part with its own shape and role (or a new armour) |
| VARIANT | a sidegrade of a part you have, with a trade-off |
| SIZE CLASS | a bigger frame for bigger hulls |
| REFINEMENT | a small step in a chain, such as more mind capacity or a longer jump |
| ALIEN TECH | needs reverse-engineered alien technology |
| FACILITY | opens a building of the shipyard |

### The nodes

| Node | Cost | Needs | Unlocks |
|---|---|---|---|
| [[research:automaton_cannon]] | [[research:automaton_cannon.cost]] | [[research:automaton_cannon.requires]] | [[research:automaton_cannon.unlocks]] |
| [[research:composite_plating]] | [[research:composite_plating.cost]] | [[research:composite_plating.requires]] | [[research:composite_plating.unlocks]] |
| [[research:cruiser_drydock]] | [[research:cruiser_drydock.cost]] | [[research:cruiser_drydock.requires]] | [[research:cruiser_drydock.unlocks]] |
| [[research:beam_laser]] | [[research:beam_laser.cost]] | [[research:beam_laser.requires]] | [[research:beam_laser.unlocks]] |
| [[research:fusion_reactor]] | [[research:fusion_reactor.cost]] | [[research:fusion_reactor.requires]] | [[research:fusion_reactor.unlocks]] |
| [[research:torch_drive]] | [[research:torch_drive.cost]] | [[research:torch_drive.requires]] | [[research:torch_drive.unlocks]] |
| [[research:jump_calibration_1]] | [[research:jump_calibration_1.cost]] | [[research:jump_calibration_1.requires]] | [[research:jump_calibration_1.unlocks]] |
| [[research:jump_calibration_2]] | [[research:jump_calibration_2.cost]] | [[research:jump_calibration_2.requires]] | [[research:jump_calibration_2.unlocks]] |
| [[research:tamed_core]] | [[research:tamed_core.cost]] | [[research:tamed_core.requires]] | [[research:tamed_core.unlocks]] |
| [[research:mind_expansion_1]] | [[research:mind_expansion_1.cost]] | [[research:mind_expansion_1.requires]] | [[research:mind_expansion_1.unlocks]] |
| [[research:mind_expansion_2]] | [[research:mind_expansion_2.cost]] | [[research:mind_expansion_2.requires]] | [[research:mind_expansion_2.unlocks]] |
| [[research:simulation_vault]] | [[research:simulation_vault.cost]] | [[research:simulation_vault.requires]] | [[research:simulation_vault.unlocks]] |
| [[research:reactive_plating]] | [[research:reactive_plating.cost]] | [[research:reactive_plating.requires]] | [[research:reactive_plating.unlocks]] |
| [[research:automaton_forge]] | [[research:automaton_forge.cost]] | [[research:automaton_forge.requires]] | [[research:automaton_forge.unlocks]] |
| [[research:capital_drydock]] | [[research:capital_drydock.cost]] | [[research:capital_drydock.requires]] | [[research:capital_drydock.unlocks]] |

A node unlocks one or more of six things: a part, an armour material, a drydock frame, more mind capacity, a
building of the shipyard (the [evolution lab](/mechanics/evolution-lab)), or a longer **jump range** (the
[galaxy](/mechanics/galaxy#home-and-the-jump-range)'s reach from home and every charted cluster; the research screen
shows it before and after).
Unlocks are for good. A newly unlocked part shows a gold NEW tag in the drydock until you pick it.

## Locked content

A new game has every part, armour and frame that isn't marked locked. The rest is the research tree's:

- In the drydock, parts research can unlock show at the end of their section, dimmed, with a lock and
  `LOCKED · <the node that unlocks it>`. Locked armour shows a lock on its swatch, locked frames are dimmed. Click a
  locked part or armour and the research screen opens on its node.
- Parts and armour that no research unlocks yet are not shown at all.
- A design that uses locked parts can still be opened, looked at and test-flown, but it is not **voyage-ready**: you
  can't take it on a voyage until you have researched what it uses.

See [the drydock](/mechanics/drydock#locked-parts). A locked building at home (the [[facility:evolution]] lab) is
dark, says LOCKED and names the node that opens it in its tooltip; a click on it opens the research screen on that
node.

## Mind capacity

Your mind runs a limited number of logic cores at once: your **mind capacity**. A new game starts with
[[start:rim_awakening.mind_capacity]]; [[research:mind_expansion_1]] and [[research:mind_expansion_2]] add to it.

Every logic core counts, on every ship of your fleet. A voyage refuses a fleet with more cores than your mind runs
(`NOT VOYAGE-READY · n CORES, YOUR MIND RUNS m`), and FLEET refuses an escort that would not fit.

If more cores are ever in play than your mind can run, the vessels that don't fit go **dormant**: dark, like
salvage, with no behavior, thrust, power or shields, but still yours. The fleet bar marks them DORMANT. Click a
dormant vessel's chip to wake it; another vessel goes dormant in its place. The vessel you fly always runs.

::: tip Room for escorts
Check the CORES count on the FLEET label at home. Your first ship's own cores may already fill your mind, so the
first escort needs a mind expansion, or a flagship with fewer cores.
:::

## FLEET

FLEET is at the moorings, left of the mind ([[control:open_fleet.keys]], or a click on the moorings). It is where you
choose the escorts that fly with you on every voyage.

- In the middle is your **flagship**: always the drydock's working design. Under it its name, cores and cargo, and
  the fleet's voyage-ready line.
- Around it, outside a dotted formation ring, are the **escort slots** (the screen shows how many), each escort
  joined to the flagship by a line. Each filled slot shows its design, `ESCORT n`, its cores and the behavior it flies
  (`FLIES SKIRMISHER`), and REMOVE. One empty slot more says how many cores are free for it.
- At the bottom, your saved designs as cards (`YOUR DESIGNS · PICK ONE FOR ESCORT n`). Each card says what picking
  it would do: FITS, IN THE FLEET, a gold USES … (it can fly, but a locked part keeps the fleet from being
  voyage-ready), NEEDS n CORES · n FREE, CAN'T FLY, or NO LOGIC CORE.
- Top right: CORES with one dot per core of your mind, filled for the fleet's, coral past your capacity.

Click a slot to pick it, then click a design (or step through them with ←/→) to preview it in the slot, dimmed
and joined by a dashed line; **Enter**, or a second click on it, puts it there (the empty slot adds an escort), and
Esc drops the preview. A design that would take the fleet over your mind capacity, has no logic core or can't fly
is refused, with the reason (the preview already says so). One design may fly in several slots. Every change is saved at once, and your escorts are drawn at the moorings at home.

Escorts must be saved designs: save a design in the drydock (or in BLUEPRINTS) first. If you delete a design that
flies as an escort, its slot empties and the shipyard notes it.

### Escorts on a voyage

On a voyage the escorts come in behind your flagship. Each flies the behavior its design names (set on the
[BEHAVIOR screen](/behavior/)) and follows your target lock. You can switch to flying any of them with
[[control:focus_next.keys]]; the one you leave goes back to its behavior. Their holds count: what every one of your
vessels carries comes home with the recall. See [voyages](/mechanics/voyages).

## Your starting position

| | |
|---|---|
| Start | [[start:rim_awakening]] |
| Ship | a copy of the [[ship:rim_skiff]] in the [[frame:frigate]] frame |
| Mind capacity | [[start:rim_awakening.mind_capacity]] |
| Open voyage | [[voyage:yard_approach]] |
| Research data | none |
| Escorts | none |

From here, a good first path: fly [[voyage:yard_approach]] a few times for [[family:kinetic]] and [[family:hull]]
data, research [[research:automaton_cannon]], clear the goal to find [[voyage:halvex_drift]], and look for intact
cores there for [[research:tamed_core]] and the mind expansions.
