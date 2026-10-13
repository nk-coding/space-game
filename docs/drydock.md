# Drydock

The ship editor: Drydock v1 (decided by Niklas, 12 Oct 2026) follows the desktop mockup
(`docs/concept/boards/ui-mockups/desktop-drydock.png`: tools PAINT B, FILL G, ERASE X, SELECT V, ROTATE Q E,
MIRROR M, BEVEL Shift+B, undo/redo) and the concept board (`docs/concept/boards/concept-board/02-drydock.png`).
Parts are unlimited in v1.

## Where the pieces live

Decisions: [GDR-0051](gdr/0051-drydock-v1.md) (scope), [GDR-0052](gdr/0052-drydock-stats-full-use.md) (stats at full
use), [GDR-0053](gdr/0053-drydock-v1-lead-defaults.md) (defaults), [ADR-0030](adr/0030-drydock-architecture.md)
(architecture).

| Piece | Where | Doc |
|---|---|---|
| Editing model (legality, tools, mirror, undo) | `data::edit::Drydock` | below |
| One-blueprint validation, RON writer/reader, frames, ship totals, ship stat labels | `data::validate_ship`, `data::blueprint`, `Content.frames`, `Content::ship_totals`, `data::stats::SHIP_STATS` | `content-model.md` |
| Live stats and per-part flags | `sim::design_report` → `DesignReport` | `sim.md` "Design report (drydock)" |
| The screen: app mode, view, panels, widgets, test flight | `crates/game/src/drydock/`, `mode.rs`, `ui_kit.rs`, `ui_icons.rs` | `client.md` "Drydock" |

Status (13 Oct 2026): wave 1 is merged. The screen shows the design with the flight tiles, the mockup's layout,
the palette with search, the selected part, and test flights with the waves toggle. Not wired yet (wave 2):
editing in the client (tools, selection, inspector with power overrides, undo/redo through `data::edit`), the
stats panel, status line and overlays from `sim::report`, and saving, loading and exporting designs plus the
blueprints list and frame choice.

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
  - `preview_slope(part, cell, material)` gives the resulting placement for the ghost (see Previews).
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

### Previews

The ghost under the cursor asks what an edit would do without doing it: `preview_place(placement)`,
`preview_slope(part, cell, material)`, `preview_paint(cell, material)` and `preview_move(part, dx, dy)` return a
`Preview { placement, twin, mirrored }` (the placement the edit puts down or leaves, the `Twin` outcome the edit
would report, and the twin's placement when it applies) or the edit's `Refusal`. The edits are planned by the same
code (`place`, `paint`, the part tools and slope turns carry out their plan), so a preview always matches the edit;
`previews_match_the_edits` checks it on every cell of the Halvex Runner's frame. A twin that would overlap the
primary edit's new part is refused with `Overlaps(k)`, `k` the index that part has after the edit (for a new
part `parts().len()`).

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
`cargo test -p data --release --test edit -- --ignored --nocapture`. After the edits moved to plan-then-apply
(previews, 13 Oct 2026; load average 11–13 with other agents building) the same runs gave repaint 0.24 µs, mirrored
repaint 0.30 µs, mirrored move 0.33 µs, mirrored place 0.20 µs, mirrored rotate 0.30 µs, auto-oriented slope
0.58 µs, the 64×64 fill 0.41 ms: the same within the noise.

## Client: editing

Wave 2, 13 Oct 2026: editing in the client (`crates/game/src/drydock/tools.rs`, `ghost.rs`, `inspector.rs`). Every
edit goes through `PlayerDesign::edit` and the editing model above; the client keeps no rules of its own.

| Input (only while the search field does not have the keyboard) | |
|---|---|
| Left button on the grid | the current tool |
| Right button on the grid | erase (click or drag, one stroke), whatever the tool |
| Space + left drag, middle drag | pan (never edits) |
| B / G / X / V | paint / fill / erase / select |
| Q / E | turn the selected part (select tool), else the part to place (the brush rotation) |
| F | flip the selected part |
| Delete / Backspace | delete the selected part (and its mirror twin) |
| Esc | deselect (in the search field it only leaves the field) |
| M / Shift+B | mirror / bevel mode |
| Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y | undo / redo; also the two ring buttons, dimmed when there is nothing to undo or redo |

**Gestures** (`tools::Gesture`, pure and tested): a press, drags and a release on frame cells become edits. It knows
nothing of the input device: the mouse drives it now; touch will drive the same gestures with the phone layout.

- **Paint** (B), by the picked part (`tools::Brush`): the 1×1 hull block **paints**: a drag paints every cell of
  the straight line between the cells it reached (no gaps on fast drags) and the whole drag is one stroke, one
  undo step. A **slope** goes down with `place_slope` on each click (oriented to the hull; clicking it again turns
  or repaints it). **Any other part** is placed once per click, its footprint in the brush rotation centred on
  the cell (rounded towards the top-left, like a slope); dragging does not place more.
- **Fill** (G): drag a rectangle; it shows (with its mirror image) while dragging and fills on release; a click fills
  one cell.
- **Erase** (X, or the right button): click or drag, one stroke.
- **Select** (V): a click selects the part under the cursor, a click on an empty cell deselects. The selection is a
  cell (part indices change with every edit), exists only in the select tool and is cleared by another tool, by
  picking a palette part (which also switches select and erase to paint), Esc, Delete, or when the part goes (an
  undo). Dragging the selected part moves it on release by the dragged cells (`move_part`); while dragging, the
  ghost shows where it and its twin would go, coral with the reason when they can't. Turns and flips that don't
  fit leave the part as it is and say why.
- **Mirror and bevel** are the editor's flags (`dock().mirror`, `dock().bevel`), the one source for the pills and the
  info line; `PlayerDesign::replace` keeps them when another design opens.

**Ghost** (`ghost.rs`): under the cursor the edit's `Preview` (`preview_paint`, `preview_slope`, `preview_place`,
`preview_move`): the parts it would put down as flight tiles at 55 % opacity (one mesh, a child of the design's
mesh, rebuilt only when the tiles change), outlined teal; a refused edit shows the part (or the cell) outlined coral.
The mirror twin is drawn too; a skipped twin is a coral outline. Erase outlines what it would take (and the twin)
in coral, select outlines the hovered part. The selected part has teal corner brackets. Why the edit under the
cursor can't happen goes after the info line in coral (`OUTSIDE THE FRAME`, `OVERLAPS FUSION REACTOR`,
`MIRROR SKIPPED · OVERLAPS ARMOR BLOCK`, ...; part names from content); without one, the last refusal stays there
for 3 s, and the pan help gives way to it.

**Inspector** (`inspector.rs`, the right column's SELECTED PART): with a placed part selected, its icon, name and
`2×2 · CELL 5,2 · INSTALLED` (`IDLE` when it is over the control capacity, `FALLS OFF` when it is not on the main
piece, both coral), its peak heat at full use as `T / limit` (teal up to the limit, gold up to 120 %, coral above:
GDR-0052) and its power level at full use (`100 %`, a partial level in gold, `OFF`, `IDLE`), then its stat lines from
`Content::part_stats` with the values after bonuses from `DesignReport::parts` (boosted values in gold; damage per
second and shots per second include the fire-rate multiplier, as the sim's guns do; build points, control cost and
minimum power stay with the palette part), and one gold line per bonus it receives or gives, grouped by stat,
amount and part (`◆ +15% FIRE RATE FROM CAPACITOR`, `◆ +2 MW POWER OUTPUT TO FISSION REACTOR ×2`). Below: the armor
pills of a hull part (`set_material`), the power tier pills (CRITICAL to OFF) and weight stepper (steps of 0.5 from
0.5 to 10, UI placeholders) of a part that draws power (`set_power`; a tier or weight equal to the part's default is
no override, so going back leaves the blueprint clean), and rotate, flip and delete. Mirror mode changes the twin
too. With nothing selected it shows the part picked in the palette (content numbers, as before).

**Locked parts and materials** (content `locked`, `content-model.md`): the palette, its search, the ARMOR pills,
the inspector's armor pills and the 1–9 keys skip them (`state::palette_parts`, `state::armor_materials`); the first
category's first unlocked part is picked at start, and the starting armor is the design's if it is unlocked.
Designs that use locked parts (the automaton raider) still open, show and fly; nothing unlocks yet.
