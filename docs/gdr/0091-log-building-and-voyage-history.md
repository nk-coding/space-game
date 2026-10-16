# GDR-0091: The LOG: a records mast by the gate and a screen of the last voyages, in place of the summary line

- Status: Accepted. 2026-10-16: a format 1 save's `last_haul` is no longer read (lead session, removing legacy readers; every save is written again on its first change).
  2026-10-16: the LOG also decodes the data cores brought home into signals, and its mast glows while one waits
  (galaxy track E6, [GDR-0097](0097-galaxy-map-and-voyage-flow.md)).
- Date: 2026-10-15
- Decided by: Niklas (a LOG building instead of the line; its place by the gate, the glow, the hover and the
  screen: interactively); lead session (the look, the screen's layout, the history's size; defaults for Niklas to
  review)
- Related: [GDR-0065](0065-phase-7-screens-and-escort-action.md) (the debrief and its one-line summary),
  [GDR-0071](0071-shipyard-home-long-u.md) (the shipyard), [GDR-0089](0089-play-test-15-oct-speed-salvage-waves-station.md)
  (the station's life), [GDR-0090](0090-action-map-settings-and-pause-menu.md) (SETTINGS, the action map),
  `docs/client.md` "Home" and "LOG", `docs/career.md`

## Context

After a voyage the debrief's panel was followed by one line over home (GDR-0065): `LAST VOYAGE · THE WARDEN'S GATE
· GOAL CLEARED · 8:15 · 7 PARTS » HULL AND ARMOUR +1 · …`. Niklas reported two problems:

- The line stayed visible on top of SETTINGS. The layers over home leave the station barely showing, so anything
  left on screen reads through them.
- A long line ran into the research building.

He suggested a "log" building instead. The save also kept only the last voyage (`last_haul`), so there was no
history to show.

## Options considered

- **Keep the line**, hide it under the layers and shorten it. This fixes the bug, but the line still competes with
  the station's labels, and there is still no history.
- **A LOG building with its own screen** (chosen by Niklas). The result moves onto the station, and the voyages
  so far get a place.

## Decision

- **The building**: a small records mast on the top arm by its tip, the end nearest the gate. It has a house with
  windows, a lattice mast with two signal yards and a teal beacon that flashes slowly at its top. Its label is
  `LOG`, with `NO VOYAGES YET`, `NEW · <VOYAGE>`, `n NEW` or `n VOYAGES` under it.
- **New results**: when a voyage ends, its result is new until the LOG is opened. Meanwhile the mast's top glows
  gold, breathing slowly (2.8 s, lead default), and the label is gold.
- **The hover**: like the other buildings, teal corner brackets and a brighter title. The tooltip is the last
  voyage in one line, the old summary's words, with the key.
- **Opening it**: a click on the mast, or L (`open_log`, the action map). The camera flies to the mast, then the
  LOG opens as a layer over home, like FLEET. Esc, CLOSE or the ring button close it, and the camera flies back out.
- **The screen** lists the last voyages, newest first, one row each:
  - NEW for results not seen before;
  - the voyage's name;
  - the outcome (`GOAL CLEARED`, `RECALLED`, `LOST`);
  - the time;
  - what came home and what it gave (`7 PARTS » HULL AND ARMOUR +1 · PROGRAMMABLE MATTER +6`, or `THE HAUL STAYED
    BEHIND`).

  The career's counts (voyages, home, lost, salvaged) sit top right. Without a voyage it says `NO VOYAGES YET`.
  Opening it marks every result seen and writes the save.
- **The save** keeps the last 50 voyages (`history`, save format 2; 50 is a placeholder). A format 1 save's
  `last_haul` becomes the first entry, so old saves load with their last voyage in the LOG.
- **The summary line is gone.** Home's words, labels and click areas and the debrief's panel step aside while any
  layer over home is open (FLEET, SETTINGS, the LOG).

## Consequences

- The debrief panel is only the moment of return. The LOG keeps the record, kills and ships lost excepted (they
  are not saved).
- A history of voyages is there for later uses: statistics, the wiki, a voyage's best runs.
- The station has one more working place, so its labels and hover rects are tested apart at every frame size.
