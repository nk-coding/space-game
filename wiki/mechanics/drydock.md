---
title: "The drydock"
description: "How to build ships in the drydock - frames, placing parts, the tools, mirror and armour, the warnings, power and weapon settings, the stats panel, saving, sharing and test flights."
---

# The drydock

The drydock is where you build. It is the inside of the shipyard's U: your working design sits there on a grid, and
everything you change is the ship you fly next. Open it from home with [[control:open_drydock.keys]] or a click on
the building area. The ring button at the top left takes you back to the shipyard.

The screen, from top to bottom:

- **Top bar**: where you are (`SHIPYARD › DRYDOCK · <FRAME> FRAME`, with the name of your frame), the design's name
  (click it to rename; Enter keeps the new name, Esc the old one) followed by UNSAVED or SAVED, and the OVERLAY pills
  on the right.
- **Left column**: the search, the ARMOR swatches, a line of category chips, and every part in sections.
- **Middle**: the grid. Under it the info line: the cell under the pointer, the mirror state, why an edit can't
  happen (in coral), and the zoom.
- **Right column**: the stats panel and SELECTED PART.
- **Bottom bar**: undo and redo, the tools (PAINT, FILL, ERASE, SELECT, ROTATE), MIRROR and BEVEL, the status line
  over the voyage-ready line, SAVE, BLUEPRINTS, EXPORT and the ⋯ menu, then BEHAVIOR and TEST-FLY.

When the window is narrow, the five tool pills fold into one dropdown (`PAINT ▾`) and MIRROR and BEVEL become small
toggles. Every control with a key names it in its tooltip.

A new game starts with a copy of the [[ship:rim_skiff]] in the drydock, in the first frame. It is a good ship to
learn from: every part has a twin on the other side of the centre line.

<ShipCard id="rim_skiff" />

## Frames and size classes

A design is built on a **frame**: the grid it must fit inside. The frame is the only limit on a ship's size.

| Frame | Grid | How you get it |
|---|---|---|
| [[frame:frigate]] | [[frame:frigate.grid]] | at the start |
| [[frame:cruiser]] | [[frame:cruiser.grid]] | research: [[research:cruiser_drydock]] |
| [[frame:capital]] | [[frame:capital.grid]] | locked, no research unlocks it yet |

Change the frame under ⋯ → FRAME, or in BLUEPRINTS (FRAME OF THIS DESIGN). The design is centred in the new frame;
a frame it doesn't fit into is refused, and the status line says how big the design is. A frame change is one undo
step. Locked frames show dimmed, with a lock. The frame also sizes the shipyard: a bigger frame makes a bigger U
at home.

## Build points

Every part costs [[stat:build_points]]. Hull parts multiply theirs by their armour's [[stat:build_point_factor]].
The total is shown in the stats panel. There is no budget: build points don't limit what you build. They are a
measure of how big and expensive a design is, and the test range sizes its raider waves from them.

## Placing parts

The left column lists every part in sections, one per part category ([[category:hull]], [[category:power]],
[[category:weapons]] and so on). The chips above the list jump to a section; the search
([[control:search.keys]]) filters the whole list by name. A section heading counts its parts and how many of them
are locked (`WEAPONS n · n LOCKED`). Each row shows the part's icon, name, footprint and one key stat, and how many
you have left (`∞`: parts are unlimited).

- Click a row to pick the part. It becomes your brush; if the select or erase tool was on, the paint tool takes
  over.
- Click on the grid to put it down. The part is centred on the cell under the pointer.
- A part put down **replaces** whatever its footprint covers. Before you click, the ghost under the pointer shows
  the new part outlined in teal and the parts it would replace with a coral wash.
- An edit that can't happen shows its outline in coral, and the info line says why (`OUTSIDE THE FRAME`,
  `OVERLAPS …`).
- A part research has just unlocked carries a gold NEW tag until you pick it or select a placed one.

### Locked parts

Parts research can unlock are listed at the end of their section, dimmed, with a lock instead of their icon and the
words `LOCKED · <research node>`. You can't pick or place them. Click one and the research screen opens on the node
that unlocks it. Parts and armour that no research unlocks yet are not shown at all.

A design that already uses locked parts (an imported design, or a template such as one of the sample ships) still
opens, shows its stats and test-flies. It is only not voyage-ready.

::: details Free drydock
The development launch option `?unlock=all` opens a free drydock: every part, armour and frame is offered, nothing
is locked and nothing is NEW. It never writes anything into your career: the save keeps your real progress.
:::

## Turning and flipping

[[control:rotate_right.keys]] turns the brush a quarter turn clockwise, [[control:rotate_left.keys]] turns it back.
Shift and the mouse wheel over the grid do the same, and so does the ROTATE pill. With the select tool these keys
turn the selected part instead. [[control:flip.keys]] flips the selected part left to right, in place.

Slopes orient themselves: with a slope picked, the brush turns to fit the hull under it (straight sides against
the hull, the sloped face out). Turn the brush yourself and it keeps that turn until you pick another part. Clicking
a slope of the same kind turns it to its next orientation; clicking it with another armour repaints it.

## The tools

| Tool | Key | What it does |
|---|---|---|
| PAINT | [[control:tool_paint.keys]] | Places the picked part. With the armour block picked, a drag paints every cell it crosses (one undo step); a hull part under the brush takes the armour, any other part is replaced by a block. |
| BEVEL | [[control:tool_bevel.keys]] | A mode on top of painting and filling: when the stroke ends, every outer corner of the cells it painted becomes a slope in the same armour. While you paint, the corners it will cut are outlined in teal. |
| FILL | [[control:tool_fill.keys]] | Drag a rectangle; every empty cell in it gets an armour block when you let go (a click fills one cell). Fill never replaces. |
| ERASE | [[control:tool_erase.keys]] | Removes the part under the pointer; drag to erase many. The right mouse button erases with any tool. |
| SELECT | [[control:tool_select.keys]] | Selects the part under the pointer. Drag it to move it. [[control:delete.keys]] deletes it; Esc deselects. |

Pan with Space and a drag, or with the middle button. The wheel zooms around the pointer; [[control:zoom_in.keys]]
and [[control:zoom_out.keys]] zoom too. At 100 % the whole frame fits.

### Mirror

[[control:mirror.keys]] (or the MIRROR pill) turns mirrored building on and off. The axis is the frame's vertical
centre line, drawn dashed. Every edit then happens on the mirror side too: placing, painting, filling, erasing,
moving, turning, repainting, power and group settings. A part's mirror twin is the part of the same kind whose
shape is its mirror image. When the mirror side can't take the edit (something is in the way, or there is no
twin), the edit still happens on your side and the ghost outlines the skipped twin in coral. Your first design
opens with mirror on.

### Undo and redo

[[control:undo.keys]] takes back the last edit, [[control:redo.keys]] does it again; the two ring buttons at the
left of the bottom bar do the same. A paint or erase drag is one step. The drydock keeps the last
[[sim:undo_steps]] steps. Undo is not kept when you open another design.

## Armour

Hull parts (the armour block and the slopes) are made of an armour material. The swatches under `ARMOR · <name>`
pick it; so do the number keys ([[control:armour_1.keys]] picks the first armour material, [[control:armour_2.keys]]
the second, and so on). A locked armour shows a lock: its key does nothing, and a click opens its research.

| Armour | Mass per cell | Hit points per cell | Build point factor |
|---|---|---|---|
| [[material:steel]] | [[material:steel.mass_per_cell]] | [[material:steel.hp_per_cell]] | [[material:steel.build_point_factor]] |
| [[material:composite]] | [[material:composite.mass_per_cell]] | [[material:composite.hp_per_cell]] | [[material:composite.build_point_factor]] |

A slope counts half a cell. To change the armour of a placed part, paint over it or pick another armour in its
SELECTED PART block.

## Pieces and the warnings

Parts that touch through straight sides form a **piece**. Slopes join only through their two straight sides,
never through the sloped face. Each piece has its own power pool. Only the piece with your logic cores flies:
everything else falls off when the ship launches. The [[page:drydock_rules]] page sums up the rules.

The **status line** in the bottom bar says whether the design can fly:

- `CAN'T FLY · …` in coral: an empty frame, NO LOGIC CORE, LOGIC CORES UNPOWERED or NO THRUST. TEST-FLY is
  disabled until you fix it.
- `FLIGHT-READY` in teal when nothing is wrong.
- `FLIGHT-READY · <warning> · +N` when something is: the most important warning first, and how many more.

The warnings, most important first:

| Warning | Tone | What it means |
|---|---|---|
| A PART FALLS OFF | coral | some parts are not joined to the main piece |
| A PLUME BURNS THE SHIP | coral | an engine's exhaust hits one of your own parts |
| OVERHEATS AT FULL LOAD | coral | at full load the hottest part runs well past its limit |
| POWER SHORT BY … | gold | at full load the reactors can't feed every consumer |
| OVER CONTROL CAPACITY | gold | the parts cost more control than the cores have: some go idle |
| GUN BLOCKED | gold | a gun's line of fire runs into your own parts |
| PULLS LEFT / RIGHT UNDER THRUST | gold | forward thrust misses the centre of mass and the turning parts can't hold it |
| RUNS HOT AT FULL LOAD | gold | parts go over their heat limit, but only a little |
| A BLAST TAKES A LOGIC CORE / A GUN, A BLAST SETS OFF A CHAIN | gold | if a part that explodes is shot off, its blast destroys a logic core, a gun or another part that explodes |

Parts named in a warning get a small triangle at their corner on the grid: coral for blocked guns, burned parts,
parts that fall off and parts that overheat badly, gold for parts that only run hot.

Under the status line, the **voyage-ready line** checks your whole fleet (this design as the flagship, plus the
escorts from FLEET) against your career: `VOYAGE-READY · CORES n / m` in teal; gold with
`NO CARGO · YOU CAN'T BRING SALVAGE HOME`; coral with the first problem, such as a locked part, a locked frame, or
more cores than your mind runs. See [research, the fleet and the career](/mechanics/career).

### Overlays

The OVERLAY pills draw over the design, in any combination:

- **HEAT**: each part tinted by how hot it runs at full load: teal when cool, gold at its limit, coral well past it.
- **MASS**: how dense each part is, the centre of mass, and the forward thrust line; a curved arrow shows which way
  full thrust turns the ship when the line misses the centre.
- **BONUS**: a gold link between every pair of parts that boost each other.
- **SHIELDS**: every shield bubble at its real size.
- **ARCS**: every gun's line of fire to its range (coral up to the first part that blocks it) and every engine's
  plume (coral where it burns your own part).
- **BLAST**: every part that explodes when destroyed: its reach (a dashed coral ring), the logic cores and guns its
  blast would destroy (filled coral), the other parts it would destroy (outlined coral), the ones that would survive
  it (gold) and the armour on the line that covers them (teal). See
  [explosions and chain reactions](/mechanics/damage#explosions-and-chain-reactions).

A shield part you place or select shows its bubble, and a part that explodes its blast, with what it would take
where it is.

## The stats panel

The right column shows the design's numbers, worked out with the game's own rules every time you edit.

| Cell | What it shows |
|---|---|
| [[stat:power_use]] | power the parts draw against what the reactors make; coral when short |
| [[stat:peak_heat]] | how hot the hottest part runs, as a share of its limit; teal up to the limit, gold a little over it, coral well over it |
| [[stat:acceleration]] | forward thrust over mass; coral without forward thrust |
| [[stat:yaw]] | Balanced, Pulls left or Pulls right; gold if the turning parts hold the pull, coral if not |
| [[stat:firepower]] | damage per second of every gun together |
| [[stat:cargo_capacity]] | how much salvage the main piece can hold |
| [[stat:control_load]] | control cost of the parts against the cores' capacity; coral when parts go idle |
| [[stat:device_load]] | Light, Moderate, Heavy or Too heavy: how hard the design is to simulate |
| [[stat:top_speed]] | the fastest speed along the ship's best thrust direction |
| [[stat:turn_time]] | time to turn a quarter circle from rest; a coral dash if the ship can't turn |
| [[stat:mass]] | the whole ship's mass |
| [[stat:build_points]] | the design's total |

A bar over 100 % fills its track, and a white notch marks 100 %.

::: info Full power, full use
The panel shows the worst case. Every consumer is assumed to run at once at full power: engines at full thrust,
gyros at full torque, every gun with a clear line firing, shields on. Power drawn only in bursts (a scanner while
it charges) is shown apart under POWER. Thrust, speeds, turning and firepower are
what the main piece makes at full power with its bonuses. Only the main piece counts. In a real fight you rarely
use everything at once, so a design that runs a little hot here may be fine.
:::

## SELECTED PART

With the select tool, click a placed part to see it in the right column: its name, footprint, cell and state
(INSTALLED, or IDLE when it is over the control capacity, or FALLS OFF when it is not on the main piece). Below
that: its heat at full load against its limit, its power level, and its stats after bonuses (boosted values in
gold), with one gold line for each bonus it gets or gives. With nothing selected, the block shows the part picked
in the list.

Its controls:

- **Armour pills** for a hull part.
- **POWER PRIORITY** for a part that draws power: CRITICAL, HIGH, NORMAL, LOW or OFF, and its **WEIGHT**. When a
  piece runs short of power, the tiers are fed in that order; inside the first tier that can't be fed in full, the
  power left is shared by weight, and the tiers below get nothing. OFF never draws. Picking the part's own default
  clears the setting. See [power and heat](/mechanics/power-and-heat).
- **GROUP** for a gun: weapon group 1, 2 or 3. [[control:fire_group_1]], [[control:fire_group_2]] and
  [[control:fire_group_3]] fire one group, [[control:fire]] fires them all. Each pill's tooltip names its keys.
  See [weapons](/mechanics/weapons).
- Rotate, FLIP and DELETE.

With mirror on, every change here applies to the twin too.

## Saving and BLUEPRINTS

SAVE ([[control:save.keys]]) keeps the design in your designs. The first save names it after the design; renaming
keeps it the same design. A design with problems still saves, as work in progress, and the status line names the
first problem. Your working design is also kept for you on its own, a moment after every change, and it comes back
when you start the game again, saved or not.

BLUEPRINTS opens your library over the drydock:

- **Your designs**, most recently saved first, with their frame, size and part count. The one in the drydock says
  IN THE DRYDOCK.
- **Templates**: the game's own ships. They open as a copy with a fresh name, and are only saved once you change
  them or press SAVE.
- On a row: OPEN, DUPLICATE, RENAME and DELETE (which asks: DELETE or KEEP).
- NEW EMPTY DESIGN in a frame, FRAME OF THIS DESIGN, and IMPORT (a `.ron` file, or the paste box for a code).

Opening another design never loses work: if the one in the drydock has unsaved changes, it is saved first. The ⋯
menu has the rarer actions: NEW DESIGN, FRAME, COPY SHARE CODE, the `.ron` file and IMPORT .RON FILE.

## Sharing designs

EXPORT shows the design as a file and as a share code.

- **The file**: DOWNLOAD .RON in the browser (WRITE .RON FILE in the desktop game). It is the game's own ship format,
  so it can go straight into a content pack.
- **The share code**: a line of text that starts with `SGD3:`. Click the box (it turns teal) and press Ctrl+C, or
  press COPY. Send it to anyone.

Both carry the design trimmed to its parts, with its pilot assists and the behavior it flies when that is one of
your own. The share code also names the content packs the design was made with, so a player who lacks one is told
which.

To **import**, open BLUEPRINTS, click the paste box and paste a code (or the text of a `.ron` file) with Ctrl+V. To
import a file, use .RON FILE in BLUEPRINTS' IMPORT row or IMPORT .RON FILE in the ⋯ menu (in the desktop game, drop the file onto
the window). The import is saved as a new design and opened;
a behavior it carries is saved with it. A code from a newer version of the game, a damaged code, or a design with
parts your game doesn't have gives a message instead.

## Test flight

TEST-FLY ([[control:test_flight.keys]]) flies the design in the test range against raider waves. Nothing there is
saved and nothing is at risk. [[control:waves.keys]] turns the waves off and on, [[control:reset_scene.keys]]
starts over, [[control:test_return.keys]] brings you back to the drydock. Lose every ship and you are back in the
drydock a moment later, the design untouched. BEHAVIOR opens the [BEHAVIOR screen](/behavior/) for this design.

<ControlsTable section="drydock" />
