# Drydock

The ship editor: Drydock v1 (decided by Niklas, 12 Oct 2026) follows the desktop mockup
(`docs/concept/boards/ui-mockups/desktop-drydock.png`: tools PAINT B, FILL G, ERASE X, SELECT V, ROTATE Q E,
MIRROR M, BEVEL Shift+B, undo/redo) and the concept board (`docs/concept/boards/concept-board/02-drydock.png`).
Parts are unlimited in v1.

## Editing model

`data::edit::Drydock` (`crates/data/src/edit.rs`) is the editing model: no Bevy, no threads, no filesystem, so it
builds for wasm32. The client's drydock uses it for every edit, and the arena's evolution operators will too.
Tests: `crates/data/tests/edit.rs`.

### Design, frame and part indices

- Coordinates are the blueprint conventions of `data::geometry`: x right, y down, forward = −y, `rot` = quarter
  turns clockwise, flip mirrors before rotating, a placement's `x, y` is the top-left cell of its rotated footprint.
- The editor holds one design in **frame coordinates**. The frame `(w, h)` (1 to 1024 cells a side) is the editing
  area; every part lies inside it.
- `Drydock::open(content, blueprint, frame)` centres the parts' bounding box in the frame (rounded towards the
  top-left) and refuses a design that does not fit, an unknown part, `rot > 3` or overlapping parts. A blueprint
  without parts starts a new design. The editor copies what it needs from `Content` (footprints, the hull flag,
  which parts draw power, the armour materials), so it owns its data.
- `export()` trims to the bounding box: `size` = its size, parts shifted so the smallest `x` and `y` are 0, in part
  index order; every other blueprint field is kept from `open`. Opening the Halvex Runner in a 24×24 frame and
  exporting gives back the identical blueprint.
- `set_frame(w, h)` re-centres the design in the new frame; refused (frame unchanged) if the design does not fit.
  A frame change is an undoable step.
- **Part indices** are the indices of `parts()`, which become the exported blueprint's `parts`. New parts are
  appended; erasing shifts the later indices down; undoing an erase puts the part back at its old index. An index
  is valid only until `revision()` changes, so the client keeps cells (selection, hover) and resolves them with
  `part_at(cell)`. Each edit's `Outcome::part` is the index of the placed or edited part right after the edit.

### Legality

A placement is legal when the part exists, `rot` is 0..3, a hull part has an armour material (a material with an
`armor` block) and other parts have none, a power override sits only on a part that draws power (an override that
sets nothing counts as none; weights above 0 and at most `MAX_POWER_WEIGHT`), the footprint lies inside the frame
and it overlaps no part. `check(placement)` says why not (`Refusal::Outside`, `Refusal::Overlaps(k)`, ...). An
occupancy grid over the frame (part index and straight-side bits per cell) keeps checks O(footprint). Moves and
turns ignore the part's own old cells.

### Tools

Materials are passed with each edit (the client keeps the current material); mirror and bevel are modes on the
editor (`dock.mirror`, `dock.bevel`).

- **Place** `place(placement)`: any part at `(x, y, rot, flip)`, appended.
- **Slope** `place_slope(part, cell, material)` (the paint tool with a slope selected):
  - On an empty cell the slope **orients itself to the hull**. Each distinct orientation (rot 0..3 unflipped,
    then rot 0..3 flipped; a 1×1 slope has 4, the longer ones 8) is centred on the cell (rounded towards the
    top-left) and pushed inside the frame; illegal ones drop out. The best wins, compared in this order:
    1. most **links**: straight unit sides whose neighbour shows a straight side back (they would join);
    2. fewest **blocked face cells**: occupied cells in front of the sloped face (the face should be open);
    3. **lean**: `2·nx·side − ny` with the face normal `(nx, ny)` and `side` = −1 left of the frame's centre
       line, +1 right, 0 on it: faces pointing away from the centre line (weight 2) and forward (weight 1),
       the concept board's tie-break;
    4. the first in turning order.

    The rules are mirror symmetric off the axis. No legal orientation → `Refusal::NoRoom`.
  - On a slope of the same part: another material repaints it; the same material **turns** it to the next
    orientation in turning order that fits, centred on the clicked cell (so a 1×1 slope turns in place, and
    clicking again and again cycles through all of them).
  - `slope_preview(part, cell, material)` gives the resulting placement for the ghost.
- **Paint** `paint(cell, material)` (armour): an empty cell gets the 1×1 armour block; a hull part (block or slope)
  under the cursor takes the material; other parts refuse. The armour block is the first hull part with a 1×1
  rect footprint in content order (`armor_block`), the bevel slope the first hull part with a 1×1 slope footprint
  (`slope_1x1`): found from the content, not named in code.
- **Bevel** mode: painting an empty cell that is an **outer corner** places the 1×1 slope instead of a block.
  Outer corner: of the cell's four neighbours, exactly two **adjacent** ones touch it (they are occupied and show a
  straight side towards the cell) and the other two are **open** (empty, or outside the frame). The slope's two
  straight sides are the touching sides (so it links to both) and its face looks out between the open sides. A
  neighbour that is occupied but shows a slope face is neither touching nor open, so the cell is no corner.
- **Fill** `fill(a, b, material)`: every empty cell of the rectangle spanned by the two corner cells (clipped to the
  frame) gets an armour block, row by row, as one undo step. Fill never bevels.
- **Erase** `erase(cell)`: removes the part covering the cell.
- **Select** tools, on a part index: `move_part(i, dx, dy)`, `rotate(i, clockwise)`, `flip(i)`,
  `set_material(i, material)` (hull parts), `set_power(i, override)` (parts that draw power), each only if the
  result is legal.
  - **Rotate** turns about a pivot fixed in the part's own frame: its centre when the footprint's `w + h` is even
    (the footprint stays in place), else the point half a cell from the centre towards the part's front (the centre
    moves by half a cell in x and y, the least possible; a beam keeps its muzzle cell). Turning back restores the
    placement exactly; four turns are the identity.
  - **Flip** mirrors the part left-right on screen, in place (same cells): `rot' = (4 − rot) % 4`, flip toggled.

### Mirror mode

The axis is the frame's vertical centre line. The mirror image of a placement: cells reflected
(`x' = W − x − w`, with `w` the rotated width), flip toggled, `rot' = (4 − rot) % 4`. Tests prove it against the
geometry for every footprint (1×1, the 1×2 beam, 2×2, slopes 1×1, 1×2, 1×3), every rot and flip, in odd and even
frames: every local cell lands on the reflected cell, connectable sides and outlines reflect, facings swap left and
right, slope normals flip their x. A design is mirror-aligned in a frame only when its width has the frame width's
parity (the client should pick frames accordingly).

Every edit applies to its mirror image too:

- **Cell tools** act at the mirror location: place puts down the mirror image of the placed part (also for an
  auto-oriented slope, so the twin is exactly mirrored); paint paints the mirror cell; fill fills the mirrored
  rectangle; erase erases the part covering the mirror cell. Paint decides both cells (block, bevel slope or
  repaint) on the design as it was before the call, so painting next to the axis stays symmetric.
- **Part tools** (move, rotate, flip, material, power, turning or repainting a slope) act on the part's **twin**:
  the part with the same id whose shape is the part's mirror image (rects compare cells and facing, the flip only
  mirrors the icon; slopes compare cells and the corner of the right angle). The twin moves by `(−dx, dy)`, turns
  the other way, and gets the same flip, material or power.
- A part (or cell, or rectangle) that is its own mirror image, such as a symmetric part centred on the axis, is
  edited once (`Twin::Itself`).
- If the mirror edit is impossible (blocked, outside, no twin, nothing at the mirror cell), the primary edit still
  happens and `Outcome::twin` is `Twin::Skipped(reason)`. If the primary edit is impossible, nothing happens.

Mirrored edits keep a symmetric design symmetric (tested on the Halvex Runner, through undo and redo too).

### Undo, strokes and revision

- Every edit that changes the design records its primitive changes (insert, remove, set, frame) as one undo step;
  undo applies the inverses in reverse order, so it restores the exact part order. `UNDO_STEPS` = 256 steps are
  kept; any new edit clears redo. An edit that changes nothing (painting a cell that already has the material)
  makes no step.
- **Strokes**: `begin_stroke()` … `end_stroke()` makes every edit in between one undo step (a paint or erase drag).
  Undo, redo, a frame change or a new `begin_stroke()` close an open stroke.
- `revision()` goes up by one with every change (each edit inside a stroke, undo, redo, frame changes); the
  client caches visuals and reports per revision. It starts at 0 for each opened design.

### Performance

Release build, 64×64 frame full of 1×1 blocks, 400 runs each (12 Oct 2026, load average 2.8–2.9 with other agents
building): median (p95) repaint 0.17 µs (1.2 µs), mirrored repaint 0.26 µs (0.35 µs), erase part 0 (all 4095
indices shift) 3.2 µs (3.3 µs) and its undo 3.3 µs (3.4 µs), mirrored erase 5.4 µs (5.7 µs), mirrored move / place /
rotate 0.16–0.27 µs (≤ 0.31 µs), auto-oriented slope 0.53 µs (0.59 µs). Filling an empty 64×64 frame (4096 blocks,
one step) 0.49 ms (0.64 ms), undoing it 0.15 ms (0.19 ms), exporting 4096 parts 0.18 ms (0.23 ms). Run with
`cargo test -p data --release --test edit -- --ignored --nocapture`.
