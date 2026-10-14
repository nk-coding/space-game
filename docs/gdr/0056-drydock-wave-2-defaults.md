# GDR-0056: Drydock wave 2 defaults: editing gestures, stats wording, saving behaviour

- Status: Accepted (provisional, lead); the right mouse button erasing confirmed by Niklas in [GDR-0079](0079-review-of-lead-defaults-14-oct.md)
- Date: 2026-10-13
- Decided by: lead session (defaults for Niklas to review); details by the wave-2 drydock agents
- Related: [GDR-0051](0051-drydock-v1.md), [GDR-0052](0052-drydock-stats-full-use.md), [GDR-0053](0053-drydock-v1-lead-defaults.md), [GDR-0054](0054-drydock-locked-parts-tools-saving-sharing.md), [ADR-0031](../adr/0031-player-storage-and-design-sharing.md), `docs/drydock.md`

## Context

Wave 2 wired editing, the live stats and saving into the drydock. Each needed small behaviour choices nobody had
decided; they are collected here so Niklas can review them in one place after trying the drydock.

## Decision

Editing (`docs/drydock.md` "Client: editing"):
- Only armour paints while dragging; machines and slopes go down once per click, a machine's footprint centred
  on the cursor cell. The right mouse button erases in every tool; F flips the selected part; Delete removes it.
- A selection exists only in the select tool; a selected part moves when the drag is released (the ghost shows
  where it would go, or why it can't).
- Mirror and bevel stay on when another design opens. Locked entries are hidden, not shown as LOCKED rows.
- The power weight stepper goes from 0.5 to 10 in steps of 0.5 (placeholder).

Stats (`docs/drydock.md` "Client: stats and overlays"):
- TOP SPEED, TURN 90°, MASS and BUILD POINTS sit in a small block under the mockup's eight cells.
- Heat tones: teal up to 100 %, gold up to 120 %, coral above (GDR-0052); the yaw reads "Balanced" while the
  forward thrust passes within 0.05 m of the centre of mass (placeholder), else "Pulls left/right".
- The status line names the most important warning (parts falling off, plumes burning the ship and overheating
  first; power, control, blocked guns, yaw and running hot after) plus "+N".
- Small issue marks on the parts concerned are always on, overlays or not.

Saving (`docs/drydock.md` "Storage and sharing"):
- Opening another design first saves the current one if it has unsaved changes (an untouched template copy is
  not saved). Renaming keeps a design's id; imports are saved under a new id.
- The first design is "Halvex Runner copy"; new and imported designs without a summary get a default one.
- A plain sandbox start flies the last working design; if it can't fly, the sandbox starts without it.
- Reopening a design centres it in its frame.
- Desktop imports a `.ron` by dropping it on the window (no file dialog yet).

## Consequences

- All of these are easy to change; Niklas's play-test of the drydock decides which stay.
