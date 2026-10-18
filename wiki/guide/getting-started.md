---
title: "Getting started"
description: "Who you are, how the game's loop runs from the shipyard to a voyage and back, and what to do in your first hour."
---

# Getting started

You are a ship-mind: a precursor intelligence that wakes up inside a dead shipyard at the rim of the galaxy. You
don't have a body. You run ships through their logic cores, and you can fly any of them yourself. The shipyard
still has a working drydock, a research lab and a gate. Everything else you have to earn by flying out, fighting
the rogue automatons that roam the rim, and bringing their wrecks home.

## The loop

Every session follows the same circle:

1. **Build** a ship in the drydock, block by block on a grid. There are no wires: every connected group of parts
   shares one power pool, heat spreads between neighbours, and some parts boost the parts next to them.
2. **Decide how it thinks** on the BEHAVIOR screen: the rules a ship follows when nobody pilots it, and the PILOT
   ASSISTS that help you while you fly it.
3. **Try it** in the test range against waves of raiders. Nothing is at stake there.
4. **Pick a voyage** on the voyage map behind the gate and set course.
5. **Fly the voyage**: reach its sites, fight what wakes up there, and salvage wrecks into your cargo hold.
6. **Recall** home once no armed enemy is close. The warp takes your fleet and everything it carries.
7. **Debrief**: at home the haul is dismantled into research data and materials.
8. **Research** new parts, armour, frames and mind capacity in the lab.
9. **Build a better ship**, and go further out.

Your ships are bonded matter: a ship destroyed on a voyage re-forms at home for free. What you lose is the haul.

## Where you start

A new game begins with [[start:rim_awakening]]. You get:

- a copy of the [[ship:rim_skiff]] docked in the building area. It is your **working design**: the ship the
  drydock edits and the ship you fly;
- a mind that runs [[start:rim_awakening.mind_capacity]] logic cores at once (your **mind capacity**);
- one open voyage, [[voyage:yard_approach]];
- every part, armour material and frame that is not locked. The rest waits for research.

<ShipCard id="rim_skiff" />

The notes in the bottom right corner of the shipyard greet a new game and later tell you about anything that went
wrong with your save.

## The shipyard

The game opens on the shipyard itself, seen from above. It is a long U lying on its side, its mouth facing right.
Each building is a place: click it (or tap it), or press its key, and the camera flies there and opens its screen.
Hover a place to highlight it; its tooltip names the key.

<svg class="diagram" viewBox="0 0 640 300" role="img" aria-label="The shipyard seen from above: the U-shaped station with the building area inside, the mind at its base, the moorings to the left, the lab and the records mast on the top wall, the test range under the bottom wall, and the gate beyond the mouth">
  <rect class="f-tile s-dim" x="150" y="70" width="300" height="24" />
  <rect class="f-tile s-dim" x="150" y="206" width="300" height="24" />
  <rect class="f-tile s-dim" x="150" y="70" width="24" height="160" />
  <rect class="line" x="190" y="104" width="250" height="92" />
  <text x="315" y="154" text-anchor="middle" class="t-gold">BUILDING AREA</text>
  <circle class="s-violet thick" cx="120" cy="150" r="18" fill="none" />
  <text x="120" y="186" text-anchor="middle" class="t-violet">MIND</text>
  <line class="line" x1="102" y1="150" x2="70" y2="110" />
  <line class="line" x1="102" y1="150" x2="70" y2="190" />
  <rect class="s-dim" x="30" y="96" width="40" height="28" fill="none" />
  <rect class="s-dim" x="30" y="176" width="40" height="28" fill="none" />
  <text x="50" y="226" text-anchor="middle">FLEET</text>
  <rect class="s-gold" x="250" y="34" width="56" height="30" fill="none" />
  <text x="278" y="24" text-anchor="middle" class="t-gold">RESEARCH</text>
  <line class="s-teal" x1="440" y1="70" x2="440" y2="30" />
  <circle class="f-teal" cx="440" cy="28" r="4" />
  <text x="440" y="16" text-anchor="middle">LOG</text>
  <rect class="s-coral" x="300" y="236" width="20" height="24" fill="none" />
  <line class="s-coral" x1="320" y1="248" x2="420" y2="248" />
  <text x="360" y="280" text-anchor="middle" class="t-coral">TEST RANGE</text>
  <circle class="s-teal thick glow" cx="560" cy="150" r="34" fill="none" />
  <line class="s-teal" x1="470" y1="150" x2="520" y2="150" marker-end="url(#arrow-teal)" />
  <text x="560" y="206" text-anchor="middle" class="t-teal">VOYAGES</text>
</svg>

*The shipyard: the drydock is the inside of the U, the gate lies beyond its mouth.*

| Place | Where | Opens | Key |
|---|---|---|---|
| Building area | inside the U, where your ship is docked | the drydock | [[control:open_drydock.keys]] |
| The mind | behind the base of the U, joined by violet conduits | the BEHAVIOR screen for the working design | [[control:open_behavior.keys]] |
| Research lab | on the top wall, with two dishes | RESEARCH | [[control:open_research.keys]] |
| The gate | beyond the mouth of the U | the voyage map | [[control:open_voyages.keys]] |
| Tower and target lane | under the bottom wall | the TEST RANGE | [[control:open_test_range.keys]] |
| Moorings | on the truss bridges left of the mind | FLEET | [[control:open_fleet.keys]] |
| Records mast | on the tip of the top arm, by the gate | the LOG | [[control:open_log.keys]] |
| Workshop | on the top wall, right of research | the [[facility:workshop]]: your parts and materials, fabricate parts | [[control:open_workshop.keys]] |
| Gear ring | top right of the screen | SETTINGS: controls and display | [[control:open_settings.keys]] |

The compute stack above the mind, the EVOLUTION lab ([[control:open_evolution.keys]]), stays dark until its research
comes: it breeds your
designs against enemies and keeps the best of each kind, every fight to watch again.

The labels on the yard tell you the state of things at a glance. The building area shows the frame and its grid
size. FLEET counts the cores your fleet uses against your mind (coral when it is over). RESEARCH says how many
nodes you can pay for now, or NO DATA YET. VOYAGES names the voyage the map will select, and the LOG says NEW in
gold while a voyage result waits for you there. The WORKSHOP says how many parts your stock holds, or how many your fleet is short of. The lab lights up once you can pay for a node (and stays lit after your first research), and the records
mast glows gold while it holds a result you haven't read.

To come back from a screen, use the ring button at its top left, at the same place on every screen (or Esc on most
screens). In flight, the ring at the top left of the HUD or [[control:leave.keys]] brings you back from the test
range.

<ControlsTable section="shipyard" />

## Your first steps

1. **Open the drydock** ([[control:open_drydock.keys]]). Look at your ship and the stats panel on the right: power,
   heat, thrust, firepower and cargo. The status line at the bottom says FLIGHT-READY, and the line under it says
   whether the ship is VOYAGE-READY. See [the drydock](/mechanics/drydock).
2. **Test-fly it** ([[control:test_flight.keys]] in the drydock). You fly against raider waves. Learn the
   [controls](/mechanics/controls): thrust, turning, strafing, the target lock ([[control:lock_next.keys]]) and
   firing ([[control:fire.keys]]). Press [[control:test_return.keys]] to go back to the drydock. The
   [flight model](/mechanics/flight) page explains why ships drift and how fast they can go.
3. **Look at the BEHAVIOR screen** ([[control:open_behavior.keys]] at home). Your ship comes with a default behavior;
   you don't need to change it to start. Read [behaviors](/behavior/) and
   [pilot assists](/behavior/pilot-assists) when you want your ships to fight smarter, and the
   [examples](/behavior/examples) for ready-made rules.
4. **Go through the gate** ([[control:open_voyages.keys]]). The voyage map shows [[voyage:yard_approach]], the only
   voyage you know. Check the panel: your fleet, its cores and its cargo. Press SET COURSE.
5. **Fly the voyage.** The markers at the screen edge point to its sites. Ships at a site sleep until you come close
   or shoot one. Fight them, then salvage: fly slowly over a wreck (closer than [[sim:salvage_reach]], slower than
   [[sim:salvage_max_speed]] relative to it) and your ship strips its parts into the hold, the best first. See
   [salvage and cargo](/mechanics/salvage-and-cargo), [weapons](/mechanics/weapons) and
   [damage](/mechanics/damage).
6. **Recall** ([[control:recall.keys]]) when your hold is full or the goal is cleared. The recall works only while no
   armed, awake enemy is near your fleet. See [voyages](/mechanics/voyages).
7. **Read the debrief** at home. It lists the parts that were dismantled, the research data they gave, and what you
   can research now.
8. **Research** ([[control:open_research.keys]] at home). [[research:automaton_cannon]] is the cheapest start: it
   lets you build a copy of the raiders' own gun, the [[part:reverse_engineered_cannon]]. Unlocked parts show a NEW tag in the drydock. See
   [research, the fleet and the career](/mechanics/career).
9. **Clear the goal.** Each voyage has one goal site. Clear it and come home, and a new voyage signal appears on the
   map.

::: tip Your first ship is enough
The [[ship:rim_skiff]] can clear the first voyage as it is. Use the first runs to learn flying and salvaging, and
to fill your stores with research data, before you rebuild it.
:::

::: warning Closing the game mid-voyage
The game saves when a voyage starts and when it ends, never in between. If you close or reload the game during a
voyage, that voyage counts as lost and its haul is gone. Your ships still re-form at home.
:::

## Power, heat and the rest

Before your first big rebuild, read how a ship works inside:

- [Power and heat](/mechanics/power-and-heat): one power pool per piece, which parts get power first when it runs
  short, and why radiators need open sides.
- [Damage](/mechanics/damage): hit points, armour, explosions, and how ships break into pieces.
- [Weapons](/mechanics/weapons): lines of fire, weapon groups, and which guns suit which ship.
- [Flight](/mechanics/flight): thrust, the flight computer, rated speed and turning.
