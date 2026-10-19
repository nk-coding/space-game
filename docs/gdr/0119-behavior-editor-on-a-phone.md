# GDR-0119: The behavior editor on a phone: the graph pans, a held row drags its rule, the test modes in a popover

- Status: Proposed
- Date: 2026-10-19
- Decided by: behavior phones agent (proposal; defaults where `docs/plans/android.md` 4.4 leaves things open)
- Related: GDR-0057, GDR-0088, GDR-0118, ADR-0045, `docs/plans/android.md` 4.4, `docs/client.md` "Behavior screen" and "Phones"

## Context

On a phone in landscape (844 × 390 dp, the compact layout class of ADR-0045) the BEHAVIOR screen was the 1280 × 720
layout fitted down to a scale of 0.54: chips about 18 dp high, type 4 to 8 dp. A finger could press its buttons
(the kit's hit slop) and scroll its lists, but a rule's row started its reorder drag after 8 px, the same move that
scrolls the graph. The Android plan (4.4) sketches the phone screen from the concept mockup but leaves open where the
tabs, the test modes, DRYDOCK / BEHAVIORS / SAVE and the status line go, the hold's length and the graph's zoom.

## Options considered

- **The plan's ring menu holding DRYDOCK, BEHAVIORS and SAVE, no bottom bar**: the ring menu is not built; the
  drydock kept its bottom bar on a phone (GDR-0118) because it fits. **A bottom bar** (chosen): DRYDOCK, the tabs
  BEHAVIOR / PILOT ASSISTS, undo, redo, BEHAVIORS and SAVE fit at 844 dp; the status line moves into a strip between
  the graph and the bar (the plan's "over the graph's bottom edge").
- **WATCH / FLY ALONGSIDE under TEST IN RANGE** (the desktop's): they would sit on the inspector. **One pill that
  toggles**: the header has no room for its longer word. **A caret ring beside TEST IN RANGE opening a small
  popover** (the plan's, chosen); picking a mode or any other button closes it.
- **Zoom rings and pinch for the graph** (plan): the graph is laid out at phone sizes instead (chips 30 dp, type
  12.5 dp, rows 46 dp) and **pans sideways** under a finger (up and down is the kit's scroll), so a long rule and its
  note are reached without zoom. Zoom left for later if real rules turn out too wide.
- **Reorder**: a plain drag scrolls, so a finger **holds a row 350 ms** (plan) still, then drags it; the drop line
  shows at once, the graph keeps its place meanwhile and scrolls by itself within 36 dp of its top or bottom edge.
  UP / DOWN in the inspector stay. The mouse keeps its 8 px drag.

## Decision

- **Layout** (844 × 390 at `compact_scale`, `CompactText(9)`): the header as on every screen (the name cut at 240 dp,
  its note only the state: UNSAVED, TEMPLATE, SAVED), the RULE SLOTS pill, top right GRAPH / TEXT, the modes' caret
  ring and TEST IN RANGE; the graph 592 dp wide on the left, the right column 214 dp (the inspector, ADD NODE and
  the palette; no search field: no soft keyboard yet, and the column is short), the status line under the graph and
  the bottom bar described above. The BEHAVIORS list keeps its two lists side by side with narrow margins and NEW
  BEHAVIOR beside CLOSE.
- **Touch** (thresholds as the drydock's: 10 dp): a tap selects a chip, a rule (its row or `always`) or nothing
  (empty graph); a finger dragged sideways on the graph pans it; a row held 350 ms drags its rule and lifting it moves
  the rule there (one undo step); a slider moves once the finger goes sideways past 10 dp, or a tap puts it where it
  landed (up and down scrolls the inspector instead, nothing edited); a pan's or a drag's release clicks nothing.
- **TEXT** is read-only on a phone in this phase (plan 3.7): it scrolls, the graph edits.

All numbers are placeholders.

## Consequences

- The mouse works as before; wide windows are unchanged.
- To try on a real phone: whether 350 ms is found and not hit by accident, whether the sideways pan is found, and
  whether the right column is tall enough for an inspector with two sliders (it scrolls).
- Open: the ring menu, zoom of the graph, a fling, the soft keyboard (the name, notes, the palette's search, the
  BEHAVIORS list's rename, an editable TEXT view).
