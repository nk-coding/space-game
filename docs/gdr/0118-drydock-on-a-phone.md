# GDR-0118: The drydock on a phone: a tool strip, stats or the selected part, fingers that hold machines above them

- Status: Proposed
- Date: 2026-10-19
- Decided by: drydock phones agent (proposal; defaults where `docs/plans/android.md` 4.3 leaves things open)
- Related: GDR-0009, GDR-0082, ADR-0045, `docs/plans/android.md` 4.3, `docs/drydock.md` "Phones and touch", `docs/client.md` "Phones"

## Context

On a phone in landscape (844 × 390 dp, the compact layout class of ADR-0045) the drydock was the 1280 × 720 layout
fitted down to a scale of 0.54: type 4 to 7 dp high, pills about 15 dp, and painting, erasing, selecting and panning
worked with the mouse only. The Android plan (4.3) sketches a phone drydock but leaves the tools' place, the
inspector, one-finger panning and several numbers open or as placeholders.

## Options considered

- **The plan's palette as one list with category headers and search behind a lens**: not done yet; the kit's grouped
  list (search field, category chips that scroll sideways, sections) works by finger as it is and is the workshop's
  too.
- **Tools in the bottom bar's dropdown** (what the wide layout folds to when narrow): two taps per tool change; not
  chosen.
- **A vertical tool strip between the palette and the grid** (the plan's proposal, chosen): PAINT, FILL, ERASE,
  SELECT and ROTATE as pills; the bottom bar keeps undo, redo, mirror and bevel (as rings), the status lines, save,
  BLUEPRINTS, EXPORT, the ⋯ menu, BEHAVIOR and TEST-FLY (they fit at 844 dp, so nothing moves behind the ring menu).
- **A floating bar beside a selected part** (plan): not needed: the selected part's own controls (rotate, flip,
  delete, material, power, group) are in the right column.
- **Placing a machine where the finger lands** (as the mouse does): the thumb hides the ghost; not chosen. **Hold and
  drop** (chosen): the ghost hangs 48 dp above the finger and lands where the finger lifts.

## Decision

- **Layout** (844 × 390 at `compact_scale`, `CompactText(9)`): palette 190 dp on the left, the tool strip (74 dp)
  beside it, the grid between it and a 214 dp right column, the bottom bar 42 dp. The right column shows the stats
  (their labels over the values) or, while a placed part is selected, the selected part with a ring back to the stats
  (it deselects); the plan's "no room for both". BLUEPRINTS scrolls (four-row lists, no preview card).
- **Opening zoom**: about 28 dp per cell (at least the fit), as the plan says; home's flight in lands on it.
- **One finger on the grid** runs the current tool once it has been down alone for 120 ms; a second finger before
  that makes it a pan and pinch with nothing edited, after it the gesture ends as when leaving the drydock (a stroke
  is kept and closed, a fill or a move is dropped). Paint and erase strokes, the fill rectangle and SELECT's
  grab-and-move follow the finger. A machine or slope is held 48 dp above the finger and dropped where it lifts; a
  tap (under 0.3 s, under 10 dp of movement) puts it under the finger.
- **One finger that starts off the frame, or on an empty cell with SELECT, pans**; a tap there deselects. **Two
  fingers** pan and pinch zoom at once (the world point under their midpoint stays under it).
- **From the palette**: a row dragged sideways (more across than along the list, past 10 dp) picks the part and the
  paint tool and carries the ghost 48 dp above the finger; lifted over the grid it drops the part. Up and down the
  list scrolls; a tap picks.
- No double tap to fit yet (the zoom rings and the % stay).

All numbers are placeholders.

## Consequences

- The mouse works as before; wide windows are unchanged.
- To try on a real phone: the hold, the lift, whether one-finger pan on empty cells in SELECT is found, and whether
  28 dp cells are too close for a whole frame (a cruiser frame needs much panning).
- Open: the plan's palette (headers, lens), double tap to fit, a fling, text entry for the name and the paste box on
  Android (soft keyboard), SHARE on EXPORT.
