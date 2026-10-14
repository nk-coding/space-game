# GDR-0081: Drydock editing: bevel after the stroke, R turns the brush (slopes auto first), placing replaces what is under it, shields show their radius

- Status: Accepted
- Date: 2026-10-14
- Decided by: Niklas (bevel, rotate, replace, shield radius); details by the drydock editing agent (marked)
- Supersedes: GDR-0053 in part (bevel turned an outer corner into a slope while painting; fill never bevelled; a
  machine placed on another part was refused)
- Related: [GDR-0053](0053-drydock-v1-lead-defaults.md), [GDR-0054](0054-drydock-locked-parts-tools-saving-sharing.md) (bevel stays a mode), [GDR-0056](0056-drydock-wave-2-defaults.md), [GDR-0082](0082-drydock-layout-v2-and-part-availability.md) (the layout), `docs/drydock.md` "Tools", "Mirror mode", "Previews", "Client: editing"

## Context

Playing with the drydock showed four rough edges. Bevel sloped a cell only if it was an outer corner at the moment
it was painted, so the result depended on the order of the stroke (paint a square left to right and only some
corners were cut). Slopes always oriented themselves, with no way to choose an orientation before placing (only
by clicking the placed slope until it turned the right way). Painting or placing over an existing part was
refused, so changing a machine meant erasing it first. And a shield emitter's reach was visible only after
placing it, with the SHIELDS overlay on.

## Options considered

- **Bevel**:
  - **After the stroke** (chosen): paint blocks as usual; when the stroke ends, every outer corner among the cells
    it painted becomes the 1×1 slope. Same shape, same result, whatever the order.
  - A click tool that bevels the corner under the cursor: one more tool for a mode that already exists.
  - A one-shot command that bevels the whole hull: too coarse (it would cut corners the player built on purpose).
- **Choosing a slope's orientation before placing**:
  - **R / Shift+wheel, auto first** (chosen): slopes keep orienting themselves; R (Shift+wheel down) turns the brush
    to the next orientation in turning order, Shift+R (Shift+wheel up) back; once turned it stays until another
    part is picked. The same keys turn any other part's brush; Q / E keep working.
  - Arrow keys for the four facings: no flipped orientations for the longer slopes, and the arrows are free for
    other uses.
  - Only Q / E: no wheel, and Q / E did not reach slopes.
- **Painting and placing over parts**:
  - **Replace whatever is under it** (chosen): paint replaces a machine with the block; a placed part (slopes too)
    removes every part its footprint overlaps; one undo step, mirror twin too.
  - Replace hull only (machines still refuse): erasing a machine first stays a chore.
  - Replace only with Shift held: one more thing to remember for the common case.
- **Shield radius while placing** (chosen): the ghost of a shield part shows its bubble like the SHIELDS overlay.

## Decision

- **Bevel after the stroke.** With BEVEL on, painting places blocks; when the paint stroke ends, every outer corner
  (the same definition and orientation rule as before) among the cells the stroke put a block in becomes the 1×1
  slope, decided on the finished shape, in the same undo step, mirror twins included. A single click is a stroke.
  Details (agent): **fill bevels too** when BEVEL is on (its filled cells, on release, in the same step: a filled
  rectangle on open space gets its corners cut), because BEVEL is one mode for every way of adding armour; repainted
  cells and cells from earlier strokes are never touched. While dragging, the ghost outlines the slopes the release
  will cut (teal) over the blocks; before a click the paint ghost shows the slope a corner would get.
- **R turns the brush, auto first.** A slope brush orients itself to the hull by default. R and Shift+wheel down
  turn it to the next orientation in turning order, Shift+R and Shift+wheel up back; the first turn starts from the
  orientation the ghost shows (agent). Once turned, the slope goes down in that orientation (centred on the cell and
  pushed inside the frame, like an auto-oriented one) until another part is picked. The same keys turn every other
  part's brush a quarter turn and the selected part in the select tool; Q / E work as before. The plain wheel still
  zooms; Shift+wheel never zooms. Browsers often turn Shift+wheel into a sideways scroll, so a sideways delta counts
  the same (agent). An orientation that can't go down (only one larger than the frame) shows a coral ghost with the
  reason. Clicking a slope of the same part still turns or repaints it.
- **Painting and placing replace what is under them.** Paint replaces a non-hull part with the block (a hull part
  just takes the material, as before); placing any part removes every part its footprint overlaps, then places it;
  one undo step, the mirror twin the same way. Erase, fill and select are unchanged. The ghost marks the parts that
  would go (a coral wash and outline). The design stays legal: legality in the editing model is per placement, and
  a design without a logic core is legal but can't fly (the status line says so), as after erasing it. Putting the
  same part exactly on itself changes nothing (agent).
- **Shields show their radius while placing.** The ghost of a shield part shows the bubble it would make (and its
  mirror twin's), centre and radius after bonuses (a design report of the design as the edit would leave it), drawn
  like the SHIELDS overlay; the selected shield part shows its bubble in the select tool.

## Consequences

- `data::edit`: `place` and new slopes replace (`Preview::removed` lists what goes); `paint` replaces non-hull
  parts; bevel moved from `paint` to `end_stroke` (and `fill`); `place_slope` / `preview_slope` take an optional
  `(rot, flip)`; `slope_orientations`, `next_orientation` and `preview_stroke_end` are new. `check` stays the strict
  free-cells test (for operators that want it, such as the coming evolution lab's).
- The client: `DrydockUi::slope_turn`, R / Shift+R and Shift+wheel, the ghost's replaced parts, bevel outlines and
  shield bubbles. The layout (GDR-0082) is not touched.
- Auto-oriented slopes prefer orientations that replace nothing, so on free space they behave as before; where
  every orientation overlaps something, the one replacing the fewest parts wins.
