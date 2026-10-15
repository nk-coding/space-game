---
title: "Voyages and the LOG"
description: "How voyages work - the voyage map, sites and what wakes them, recall and abandoning, losing ships, the debrief, the LOG, and how clearing a voyage opens the next."
---

# Voyages and the LOG

A voyage is one trip out of the shipyard into a small cluster of space. You fly your fleet through the gate, visit
the cluster's sites, fight what wakes up there, salvage what is left, and warp home with your hold full. Voyages are
where research data comes from, and clearing one is how you find the next.

## The voyage map

Fly to the gate from home ([[control:open_voyages.keys]], or a click on the gate) and the voyage map opens. It is a
star map of the galaxy's edge. The galaxy is split into bands, from the rim inwards: rim, outer, middle, inner and
core. Every voyage you can fly now lies in the rim band; the bands further in are shown as uncharted.

You only see the voyages you know about. A voyage that is not open yet is not drawn at all. A known voyage has a
marker and a label:

- **SIGNAL**: just found, never flown. Teal rings pulse out of it.
- **OPEN**: you can fly it.
- **CLEARED · BEST m:ss**: you cleared its goal and came home; the time is your quickest clear. You can fly it again:
  each launch rolls new sites.
- **GUARDIAN**: a voyage whose goal is a boss, drawn as a coral diamond.

Under each voyage's name, one dot per site: coral for enemies, gold for wrecks, a diamond for the goal.

Click a voyage (or step through them with [[control:next_voyage.keys]] and [[control:previous_voyage.keys]]) to
see it in the panel on the right: its band, the factions you will meet, WRECKS if any site holds them, its summary,
one row per site with what waits there, and its expected length (`≈ … MIN · RECALL WHEN CLEAR`). Under that, your
FLEET: the flagship and escorts with their cores and cargo, and a checklist:

- FLIGHT-READY: every ship can fly;
- ONLY UNLOCKED PARTS: no ship uses something your career hasn't unlocked;
- CORES n / m: the fleet's logic cores fit your mind capacity;
- CARGO: the fleet has a hold (a gold NO CARGO is only a warning: you can launch, but you can't bring salvage home).

SET COURSE (or Enter) launches the selected voyage. It is dimmed while the voyage can't launch, and the line above
it says why. The wheel zooms (out to the whole galaxy), a drag pans, [[control:reset_view.keys]] or a double click
resets the view. Esc or the ring button goes back to the shipyard.

### The voyages

| Voyage | Band | Sites | Length | Opens when you clear |
|---|---|---|---|---|
| [[voyage:yard_approach]] | [[voyage:yard_approach.band]] | [[voyage:yard_approach.sites]] | [[voyage:yard_approach.length]] | open from the start |
| [[voyage:halvex_drift]] | [[voyage:halvex_drift.band]] | [[voyage:halvex_drift.sites]] | [[voyage:halvex_drift.length]] | [[voyage:halvex_drift.opens_after]] |
| [[voyage:corran_reach]] | [[voyage:corran_reach.band]] | [[voyage:corran_reach.sites]] | [[voyage:corran_reach.length]] | [[voyage:corran_reach.opens_after]] |
| [[voyage:wardens_gate]] | [[voyage:wardens_gate.band]] | [[voyage:wardens_gate.sites]] | [[voyage:wardens_gate.length]] | [[voyage:wardens_gate.opens_after]] |

## Sites

A voyage is one cluster: a disc of space with a handful of **sites** in it. Your fleet enters at the cluster's edge,
pointing towards its centre. The sites lie spread out across the disc, never too close to each other or to where you
come in. One site is the **goal**: it is placed far from the entry, and clearing it is what clears the voyage.

<svg class="diagram" viewBox="0 0 640 320" role="img" aria-label="A voyage cluster: the fleet enters at the bottom edge; three sites lie spread across the disc, the goal farthest from the entry; around one site a wide dotted spawn ring and a smaller wake ring">
  <circle class="line" cx="320" cy="160" r="150" />
  <path class="f-teal" d="M314,316 L320,300 L326,316 Z" />
  <text x="320" y="296" text-anchor="middle" class="t-teal">ENTRY</text>
  <circle class="dotted s-dim" cx="220" cy="190" r="72" fill="none" />
  <circle class="s-coral" cx="220" cy="190" r="34" fill="none" />
  <circle class="f-coral" cx="220" cy="190" r="5" />
  <text x="220" y="140" text-anchor="middle" class="t-coral">WAKE</text>
  <text x="150" y="262" text-anchor="middle">SPAWN</text>
  <circle class="f-gold" cx="410" cy="200" r="5" />
  <text x="410" y="222" text-anchor="middle" class="t-gold">WRECKS</text>
  <path class="f-gold s-gold glow" d="M320,38 L330,48 L320,58 L310,48 Z" />
  <text x="320" y="28" text-anchor="middle" class="t-gold">GOAL</text>
  <line class="s-teal" x1="318" y1="292" x2="232" y2="206" marker-end="url(#arrow-teal)" />
  <line class="s-teal" x1="234" y1="182" x2="396" y2="198" marker-end="url(#arrow-teal)" />
  <line class="s-teal" x1="404" y1="188" x2="326" y2="62" marker-end="url(#arrow-teal)" />
</svg>

*A voyage cluster: you enter at the edge and fly from site to site; the goal lies far from the entry. A site spawns
when you reach its outer ring and wakes at its inner one.*

There is no travel screen between sites: the whole cluster is one space, and you fly from one site to the next
yourself. Markers at the screen edge point to every site that is off screen, with its name and distance: coral for
enemies, gold for wrecks only, brighter for the goal (`· GOAL`), dim once cleared (`· CLEARED`).

### What happens at a site

1. **It spawns.** A site's ships and wrecks appear when one of your vessels comes within
   [[sim:voyage_spawn_radius]] of it. Each launch rolls how many ships and wrecks a site holds, where they lie and
   which parts the wrecks have already lost.
2. **Its ships sleep.** They hold where they spawned and don't fire. A sleeping site doesn't stop your recall.
3. **It wakes** when one of your vessels comes within its **wake radius**, when one of its ships is hit (a shot, a
   shield hit, a plume, a ram that does damage), or when one of its ships stops working. Then all of its ships wake
   together and fly their behavior. A banner says `<ENCOUNTER> · AWAKE`, and under it THEY ARE COMING.
4. **It is cleared** when none of its ships is still an armed vessel: each one is destroyed, has lost its logic
   cores, or has no working gun left. Clearing is final. A banner says `SITE CLEARED · … · THE WRECKS ARE YOURS`, or for the goal
   `GOAL CLEARED · … · RECALL WHEN CLEAR`.

A site where no ships were rolled holds only wrecks and has nothing to wake; it counts as done once it has spawned. The voyage HUD's subtitle counts
the sites done of all (`SITES n / m`) and the time since you arrived.

| Encounter | Wake radius |
|---|---|
| [[encounter:raider_pair]] | [[encounter:raider_pair.wake_radius]] |
| [[encounter:raider_pack]] | [[encounter:raider_pack.wake_radius]] |
| [[encounter:raider_den]] | [[encounter:raider_den.wake_radius]] |
| [[encounter:hunter_pack]] | [[encounter:hunter_pack.wake_radius]] |
| [[encounter:hunter_den]] | [[encounter:hunter_den.wake_radius]] |
| [[encounter:derelict_field]] | [[encounter:derelict_field.wake_radius]] |
| [[encounter:precursor_wrecks]] | [[encounter:precursor_wrecks.wake_radius]] |
| [[encounter:core_graveyard]] | [[encounter:core_graveyard.wake_radius]] |
| [[encounter:the_warden]] | [[encounter:the_warden.wake_radius]] |

::: tip Pick your fights
Sleeping ships wait. Come in from one side, shoot first, and you fight one site at a time. Fly past a site too
closely and it wakes behind you.
:::

## Your fleet on a voyage

Your flagship is the drydock's working design. It comes in at the entry with you at the controls. Your escorts from
FLEET come in behind it, fly the behavior their design names and follow your target lock. You can take over any of
your vessels with [[control:focus_next.keys]] and [[control:focus_previous.keys]], or by clicking its chip in the
fleet bar. See [research, the fleet and the career](/mechanics/career).

Under the hull bar, the HUD shows the salvage in the hold of the ship you fly (SALVAGE, the units in use against the hold, one pip per
cargo unit). How collecting works is on [salvage and cargo](/mechanics/salvage-and-cargo).

A voyage never pauses. Esc opens the menu (MENU · THE FLIGHT GOES ON) over the running fight, with SETTINGS and
ABANDON VOYAGE….

## Recall

RECALL ([[control:recall.keys]], or the RECALL pill under the cargo block) warps your whole fleet home with
everything it carries. The warp takes a few seconds and can't be broken off.

You can recall only while no armed, awake enemy is within [[sim:recall_radius]] of any of your vessels. The line
under the pill says which it is: `WARP HOME … · KEEPS THE HAUL` in teal, or `ARMED ENEMIES NEAR · NO RECALL` dimmed.
Press [[control:recall.keys]] anyway and a banner tells you why it can't happen.

If none of your vessels has a working gun left, the recall is an **emergency recall**: it works whatever is near.
The line says `EMERGENCY RECALL · NO WORKING GUN`, and the warp flickers amber.

What comes home is the cargo of every vessel of yours, including dark and dormant ones. If every vessel is
destroyed during the warp, the voyage is still lost.

## Losing your ships

When your last vessel is gone, the voyage is lost: `SHIP LOST · RECALLED HOME`, and a few seconds later you
are back at the shipyard. The haul stays in the wrecks, and the goal doesn't count even if you cleared it.

Your ships are not gone. They are bonded matter: every design re-forms at home, free, ready for the next launch.
A loss costs you the haul and the time, nothing else.

If your flagship goes down but an escort survives, the fight goes on: you fly the escort, and the lost ship's cargo
stays in its wreck, where the escort can salvage it again.

## Abandoning a voyage

[[control:leave.keys]] (or ABANDON VOYAGE… in the Esc menu) opens the ABANDON VOYAGE? panel. It tells you how many
parts in your holds would stay behind. The fight goes on while you decide. ABANDON VOYAGE (or Enter) ends the voyage
as lost, exactly as losing every ship would; KEEP FLYING (or Esc, or [[control:leave.keys]] again) closes the panel.
This is the way out when a dark ship is stuck near an armed enemy and can't recall.

## The debrief

After every voyage, home opens with the debrief on the right:

- the head line: VOYAGE HOME · HAUL UNLOADED, or SHIP LOST · RECALLED HOME;
- the voyage's name and chips: its band, the outcome (GOAL CLEARED, RECALLED or LOST) and FIRST CLEAR the first time
  you clear it;
- TIME, SITES, KILLS and SHIPS LOST;
- `n PARTS DISMANTLED`, with the parts that came home, most first, and what each gave;
- RESEARCH DATA per family, with what is now in store, and the materials;
- RESEARCH READY: the research nodes you can pay for now;
- `SIGNAL FOUND · <voyage>` for every voyage this one opened, with VOYAGE MAP to fly straight to the gate, and the
  voyage's own closing words on a first clear.

Everything in the hold is dismantled at once: each part becomes research data for its family, and some give
materials too. Press DISMISS (Esc, Enter) to close it. The debrief is not kept, but the LOG is.

## The LOG

The LOG lives at the records mast on the shipyard's top arm, by the gate. Open it with [[control:open_log.keys]] or
a click on the mast. The mast glows gold while it holds a result you haven't seen, and its tooltip is the last
voyage in one line.

At the top right are your career's counts, over every voyage ever: VOYAGES, HOME, LOST and SALVAGED (items brought
home). Below is the list of the last voyages, newest first, up to [[sim:max_history]]: a gold NEW for a result you
haven't seen, the voyage's name, its outcome (GOAL CLEARED, RECALLED or LOST), its time, and what came home (the
part count and the data and materials it gave), or THE HAUL STAYED BEHIND. Opening the LOG marks everything seen.

Kills and ships lost are not kept in the LOG; the debrief shows them once.

## Clearing a voyage

A voyage is **cleared** when you clear its goal site and then come home with a recall. Clearing it:

- marks it CLEARED on the map, with your best time;
- opens every voyage that waits for it. A newly opened voyage shows up as a SIGNAL, the debrief says
  `SIGNAL FOUND`, and the map selects it the next time you open it.

A cleared voyage stays open. Fly it again for more salvage; each launch rolls a new layout, and a quicker clear
improves your best time.

<ControlsTable section="mission" />
