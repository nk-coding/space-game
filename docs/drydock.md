# Drydock

The ship editor: Drydock v1 (decided by Niklas, 12 Oct 2026) follows the desktop mockup
(`docs/concept/boards/ui-mockups/desktop-drydock.png`: tools PAINT B, FILL G, ERASE X, SELECT V, ROTATE Q E,
MIRROR M, BEVEL Shift+B, undo/redo) and the concept board (`docs/concept/boards/concept-board/02-drydock.png`).
Layout v2 (14 Oct 2026, [GDR-0082](gdr/0082-drydock-layout-v2-and-part-availability.md)): every part in sections
on the left with the armour swatches, the tools in the bottom bar, icon buttons and a ⋯ menu. Parts are unlimited
for now; the plumbing for counts is in ([Part availability](#part-availability)).

## Where the pieces live

Decisions: [GDR-0051](gdr/0051-drydock-v1.md) (scope), [GDR-0052](gdr/0052-drydock-stats-full-use.md) (stats at full
use), [GDR-0053](gdr/0053-drydock-v1-lead-defaults.md) (defaults), [GDR-0054](gdr/0054-drydock-locked-parts-tools-saving-sharing.md)
(locked parts, saving, sharing), [GDR-0056](gdr/0056-drydock-wave-2-defaults.md) (wave 2 defaults),
[ADR-0030](adr/0030-drydock-architecture.md) (architecture), [ADR-0031](adr/0031-player-storage-and-design-sharing.md)
(storage and sharing); behaviors: [GDR-0057](gdr/0057-behavior-editor-v1.md),
[GDR-0059](gdr/0059-behavior-editor-wave-a-defaults.md), [GDR-0060](gdr/0060-behavior-editor-wave-b-defaults.md).

| Piece | Where | Doc |
|---|---|---|
| Editing model (legality, tools, mirror, undo) | `data::design::edit::Drydock` | below |
| One-blueprint validation, RON writer/reader, frames, ship totals, ship stat labels | `data::validate_ship`, `data::design::blueprint`, `Content.frames`, `Content::ship_totals`, `data::design::stats::SHIP_STATS` | `content-model.md` |
| Live stats and per-part flags | `sim::design_report` → `DesignReport` | `sim.md` "Design report (drydock)" |
| The screen: app mode, view, panels, widgets, test flight | `crates/game/src/drydock/`, `mode.rs`, `ui_kit.rs`, `ui_icons.rs` | `client.md` "Drydock" |
| What the career unlocks: locked rows, NEW tags, the voyage-ready line | `crates/game/src/career.rs`, `drydock/{state,panels,stats}.rs`, `data::career` | [below](#locked-parts-new-and-voyage-ready), `career.md` |
| Saving, autosave, BLUEPRINTS, EXPORT, share codes, copy / paste | `crates/game/src/storage.rs`, `clipboard.rs`, `drydock/{library,share,blueprints}.rs` | [below](#storage-and-sharing) |
| The player's behaviors (saved, named by designs, flown), TEST IN RANGE | `crates/game/src/drydock/behaviors.rs`, `test_range.rs`, `scene.rs` | [below](#behaviors-storage-designs-and-tests) |
| The BEHAVIOR screen (graph, text, inspector, saving, the live panel) | `crates/game/src/behavior_ui/`, `data::behavior::edit` | `client.md` "Behavior screen", `behavior-format.md` "Editing model" |

Status (13 Oct 2026): waves 1 and 2 are merged. The drydock edits (paint, fill, erase, select, rotate, flip,
mirror, bevel, undo/redo, ghost and selection, the inspector with power overrides), shows the live stats, status
line and overlays from `sim::report`, saves designs in the player storage layer (autosave of the working design,
the BLUEPRINTS list with the content ships as templates, frames, renaming), and shares designs as `.ron` files and
share codes. Since track 7F (13 Oct 2026) it offers what the career has unlocked and shows the rest locked
([Locked parts, NEW and voyage-ready](#locked-parts-new-and-voyage-ready)). Not built yet: the phone layout and touch input, upgrade materials
and variants, thumbnails, a native file dialog, Android storage.

## Editing model

`data::design::edit::Drydock` (`crates/data/src/design/edit.rs`) is the editing model: no Bevy, no threads, no filesystem, so it
builds for wasm32. The client's drydock uses it for every edit, and the arena's evolution operators will too.
Tests: `crates/data/tests/edit.rs`.

### Design, frame and part indices

- Coordinates are the blueprint conventions of `data::design::geometry`: x right, y down, forward = −y, `rot` = quarter
  turns clockwise, flip mirrors before rotating, a placement's `x, y` is the top-left cell of its rotated footprint.
- The editor holds one design in **frame coordinates**. The frame `(w, h)` (1 to 128 cells a side,
  `data::content::validate::MAX_FRAME_SIZE`, the same limit as content frames) is the editing area; every part lies inside
  it.
- `Drydock::open(content, blueprint, frame)` centres the parts' bounding box in the frame (rounded towards the
  top-left) and refuses a design that does not fit, an unknown part, `rot > 3` or overlapping parts. A blueprint
  without parts starts a new design. Coordinates anywhere in the `i32` range are safe: the box is computed in
  `i64`, parts too far apart give `DoesNotFit` (its size saturated at `u32::MAX`), a lone far part is centred. The
  editor copies what it needs from `Content` (footprints, the hull flag, which parts draw power, the armour
  materials), so it owns its data.
- `export()` trims to the bounding box: `size` = its size, parts shifted so the smallest `x` and `y` are 0, in part
  index order; every other blueprint field is kept from `open`, except `source` (cleared: a design comes from no
  pack, so a template copy's problems are not located in the template's file). Opening the Halvex Runner in a
  24×24 frame and exporting gives back the identical blueprint (without its `source`).
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
and it overlaps no part. `check(placement)` says why not (`Refusal::Outside`, `Refusal::Overlaps(k)`, ...); it stays
this strict test, while the place, slope and paint tools replace what is under them instead (below). An
occupancy grid over the frame (part index and straight-side bits per cell) keeps checks O(footprint). Moves and
turns ignore the part's own old cells. Legality is per placement: whether the design can fly (a logic core, power,
...) is the status line's (`docs/sim.md`), so replacing a logic core is as legal as erasing it.

### Tools

Materials are passed with each edit (the client keeps the current material); mirror and bevel are modes on the
editor (`dock.mirror`, `dock.bevel`).

**Placing and painting replace what is under them** ([GDR-0081](gdr/0081-drydock-editing-bevel-rotate-replace.md),
Niklas 14 Oct 2026): a placed part (slopes too) removes every part its footprint overlaps and then goes down, one
undo step, its mirror twin the same; paint replaces a non-hull part with the block, and a hull part under it just
takes the material. The same part put down exactly on itself replaces nothing and changes nothing.

- **Place** `place(placement)`: any part at `(x, y, rot, flip)`, appended after the replaced parts are removed. The
  footprint must lie inside the frame (`Refusal::Outside`).
- **Slope** `place_slope(part, cell, material, turn)` (the paint tool with a slope selected):
  - With `turn = None` the slope **orients itself to the hull**. Each distinct orientation (rot 0..3 unflipped,
    then rot 0..3 flipped; a 1×1 slope has 4, the longer ones 8) is centred on the cell (rounded towards the
    top-left) and pushed inside the frame; one larger than the frame drops out. The best wins, compared in this
    order (the parts a candidate would replace count as gone for 1 to 3):
    0. fewest **replaced parts** (none where it fits on free cells, so on empty space nothing changes);
    1. most **links**: straight unit sides whose neighbour shows a straight side back (they would join);
    2. fewest **blocked face cells**: occupied cells in front of the sloped face (the face should be open);
    3. **lean**: `2·nx·side − ny` with the face normal `(nx, ny)` and `side` = −1 left of the frame's centre
       line, +1 right, 0 on it: faces pointing away from the centre line (weight 2) and forward (weight 1),
       the concept board's tie-break;
    4. the first in turning order.

    The rules are mirror symmetric off the axis. No orientation fits the frame → `Refusal::NoRoom`.
  - With `turn = Some((rot, flip))` (the brush turned with R, below) the slope goes down in that orientation,
    centred on the cell and pushed inside the frame; `NoRoom` if it is larger than the frame, `BadRotation` for
    `rot > 3`. `slope_orientations(part)` lists the turning order, `next_orientation(part, from, forward)` steps
    through it (`from` is matched by shape, so any `(rot, flip)` of the same geometry works).
  - On a slope of the same part (whatever `turn` says): another material repaints it; the same material **turns**
    it to the next orientation in turning order that fits, centred on the clicked cell (so a 1×1 slope turns in
    place, and clicking again and again cycles through all of them). Turning does not replace.
  - `preview_slope(part, cell, material, turn)` gives the resulting placement for the ghost (see Previews).
- **Paint** `paint(cell, material)` (armour): an empty cell gets the 1×1 armour block; a hull part (block or slope)
  under the cursor takes the material; any other part is replaced by the block. The armour block is the first hull
  part with a 1×1 rect footprint in content order (`armor_block`), the bevel slope the first hull part with a 1×1
  slope footprint (`slope_1x1`): found from the content, not named in code.
- **Bevel** mode, **after the stroke** (GDR-0081): painting puts down blocks as usual; when the stroke ends
  (`end_stroke`; a paint outside a stroke is a stroke of its own, so a click bevels at once), every **outer corner**
  among the cells the stroke put a block in becomes the 1×1 slope, in the same undo step, mirror twins included.
  The corners are all decided on the finished shape before any block changes, so the same shape gives the same
  slopes whatever order it was painted in. Cells repainted (not newly blocked) and cells painted before the stroke
  stay as they are. Outer corner: of the cell's four neighbours, exactly two **adjacent** ones touch it (they are
  occupied and show a straight side towards the cell) and the other two are **open** (empty, or outside the frame).
  The slope (unflipped) has its two straight sides on the touching sides (so it links to both), its face looks out
  between the open sides, and it keeps the block's material. A neighbour that is occupied but shows a slope face is
  neither touching nor open, so the cell is no corner.
- **Fill** `fill(a, b, material)`: every empty cell of the rectangle spanned by the two corner cells (clipped to the
  frame) gets an armour block, row by row, as one undo step. In bevel mode the outer corners among the filled cells
  become slopes in the same step (a filled rectangle on open space gets its four corners cut); fill never
  replaces.
- **Erase** `erase(cell)`: removes the part covering the cell. `remove_part(i)` removes a part by index (Delete on
  the selected part).
- **Select** tools, on a part index: `move_part(i, dx, dy)`, `rotate(i, clockwise)`, `flip(i)`,
  `set_material(i, material)` (hull parts), `set_power(i, override)` (parts that draw power), each only if the
  result is legal.
  - **Rotate** turns about a pivot fixed in the part's own frame: its centre when the footprint's `w + h` is even
    (the footprint stays in place), else the point half a cell from the centre towards the part's front (the centre
    moves by half a cell in x and y, the least possible; a beam keeps its muzzle cell). Turning back restores the
    placement exactly; four turns are the identity.
  - **Flip** mirrors the part left-right on screen, in place (same cells): `rot' = (4 − rot) % 4`, flip toggled.

### Part availability

GDR-0082, 14 Oct 2026. `set_available(part, Availability)` tells the editor how many of a part the player has
(`data::career::Availability`: `Unlimited`, or `Count(n)` in all, the design's own parts included; the client passes
`Career::available`, every part unlimited for now). Not an edit: no undo step, no revision change. `left(part)` is
what remains (`Count(n − parts placed)`, never below 0). An edit that would put down a part with none left is
refused with `Refusal::NoneLeft(part)` ("NO BEAM LASER LEFT" on the info line): `check`, `place` and its preview,
a new slope, painting or bevelling an empty cell; turning, repainting, moving and erasing never are. A mirror image
that would need one more than is left is skipped (`Twin::Skipped(NoneLeft)`, the primary edit happens); fill fills
as many cells as are left, row by row (none left: refused). Parts already in the design stay when a count drops
below them, and undo and redo always restore their step. Tests: `crates/data/tests/edit_availability.rs`.

### Mirror mode

The axis is the frame's vertical centre line. The mirror image of a placement: cells reflected
(`x' = W − x − w`, with `w` the rotated width), flip toggled, `rot' = (4 − rot) % 4`. Tests prove it against the
geometry for every footprint (1×1, the 1×2 beam, 2×2, slopes 1×1, 1×2, 1×3), every rot and flip, in odd and even
frames: every local cell lands on the reflected cell, connectable sides and outlines reflect, facings swap left and
right, slope normals flip their x. A design is mirror-aligned in a frame only when its width has the frame width's
parity (the client should pick frames accordingly).

Every edit applies to its mirror image too:

- **Cell tools** act at the mirror location: place puts down the mirror image of the placed part (also for an
  auto-oriented or turned slope, so the twin is exactly mirrored), replacing what the image overlaps; paint paints
  the mirror cell; fill fills the mirrored rectangle; erase erases the part covering the mirror cell. Paint decides
  both cells (block, repaint or replace) on the design as it was before the call, so painting next to the axis
  stays symmetric; a part covering both cells is replaced once, by a block in each. Bevel then works on both
  cells' blocks together (the corner rule is mirror symmetric).
- **Removing a part** (`remove_part`) removes its **twin** (below) too, and nothing else: a part with no twin of
  its kind is removed alone (`Twin::Skipped(NoTwin)`), whatever covers its mirror cells.
- **Part tools** (move, rotate, flip, material, power, turning or repainting a slope) act on the part's **twin**:
  the part with the same id whose shape is the part's mirror image (rects compare cells and facing, the flip only
  mirrors the icon; slopes compare cells and the corner of the right angle). The twin moves by `(−dx, dy)`, turns
  the other way, and gets the same flip, material or power.
- A part (or cell, or rectangle) that is its own mirror image, such as a symmetric part centred on the axis, is
  edited once (`Twin::Itself`).
- If the mirror edit is impossible (blocked, outside, no twin, nothing at the mirror cell, or a placed part's image
  would overlap the part itself), the primary edit still happens and `Outcome::twin` is `Twin::Skipped(reason)`. If
  the primary edit is impossible, nothing happens.

Mirrored edits keep a symmetric design symmetric (tested on the Halvex Runner, through undo and redo too).

### Previews

The ghost under the cursor asks what an edit would do without doing it: `preview_place(placement)`,
`preview_slope(part, cell, material, turn)`, `preview_paint(cell, material)` and `preview_move(part, dx, dy)` return a
`Preview { placement, twin, mirrored, removed }` (the placement the edit puts down or leaves, the `Twin` outcome the
edit would report, the twin's placement when it applies, and the parts it replaces as indices before the edit) or
the edit's `Refusal`. The edits are planned by the same code (`place`, `paint`, the part tools and slope turns carry
out their plan), so a preview always matches the edit; `previews_match_the_edits` checks it on every cell of the
Halvex Runner's frame, turned slopes included, and that the replaced parts are gone. `preview_paint` shows what a
click would leave: in bevel mode the slope where the finished one-cell stroke has an outer corner.
`preview_stroke_end()` gives the slopes ending the open stroke would make (empty without bevel). A twin that would
overlap the primary edit's new part is refused with `Overlaps(k)`, `k` the index that part has after the edit (for
a new part `parts().len()` minus the replaced parts).

### Undo, strokes and revision

- Every edit that changes the design records its primitive changes (insert, remove, set, frame) as one undo step;
  undo applies the inverses in reverse order, so it restores the exact part order. `UNDO_STEPS` = 256 steps are
  kept; any new edit clears redo. An edit that changes nothing (painting a cell that already has the material)
  makes no step.
- **Strokes**: `begin_stroke()` … `end_stroke()` makes every edit in between one undo step (a paint or erase drag).
  Undo, redo, a frame change or a new `begin_stroke()` close an open stroke. In bevel mode closing it first turns
  the stroke's outer corners into slopes (one more revision, the same undo step).
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
0.58 µs, the 64×64 fill 0.41 ms: the same within the noise. With replace and bevel after the stroke (14 Oct 2026,
load average 0.6–1.6): mirrored place into a hole 0.24 µs, auto-oriented slope 0.63 µs, a 2×2 placed over four
blocks with its twin (8 parts removed, every later index shifts) 19 µs (p95 30 µs), paint over a machine with its
twin 0.32 µs, ending a bevel stroke of 128 cells 2.1 µs; the rest unchanged.

## Client: editing

Wave 2, 13 Oct 2026: editing in the client (`crates/game/src/drydock/tools.rs`, `ghost.rs`, `inspector.rs`). Every
edit goes through `PlayerDesign::edit` and the editing model above; the client keeps no rules of its own.
Screenshots (headless, 1280 × 720): `docs/results/drydock-editing-ghost.png` (a wing painted with mirror and bevel,
the ghost of a turned beam laser and its twin), `-refused.png` (a refused ghost and its reason), `-select.png` (a
selected beam laser on the HIGH tier in the inspector). GDR-0081 (14 Oct 2026): `drydock-bevel-stroke.png` (a BEVEL
wing stroke while dragging: the slopes its release will cut outlined teal) and `drydock-bevel-done.png` (after
release), `drydock-slope-turned.png` (a 1×1 slope brush turned with R at the wing's end, and its twin),
`drydock-replace.png` (a fusion reactor over the Rim skiff's middle: the replaced parts in coral),
`drydock-shield-radius.png` (a shield emitter's bubbles while placing it with mirror).

| Input (only while the search field does not have the keyboard) | |
|---|---|
| Left button on the grid | the current tool |
| Right button on the grid | erase (click or drag, one stroke), whatever the tool |
| Space + left drag, middle drag | pan (never edits) |
| B / G / X / V | paint / fill / erase / select |
| R / Shift+R, E / Q | turn the selected part (select tool) clockwise / back, else the brush: the next / previous orientation of a slope (first from the one the ghost shows), a quarter turn of any other part |
| Shift+wheel down / up (over the grid) | the same as R / Shift+R; the plain wheel zooms. Browsers often send Shift+wheel as a sideways scroll, so a sideways delta counts too; touchpad pixels add up to a turn every 40 px; one turn a frame at most |
| F | flip the selected part |
| Delete / Backspace | delete the selected part (and its mirror twin) |
| Esc | close the open bottom bar menu, else deselect (in the search field it only leaves the field) |
| M / Shift+B | mirror / bevel mode (bevel cuts a stroke's outer corners when it ends) |
| 1–9 | armour material (the swatches in content order) |
| Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y | undo / redo; also the two ring buttons, dimmed when there is nothing to undo or redo |

**Layout v2** (GDR-0082, 14 Oct 2026; `panels.rs`, the draft `docs/plans/mockups/drydock-left-column.html`, variant
A). Screenshots: `docs/results/drydock-v2-1280x720.png`, `-1280x720-more.png` (the ⋯ menu open),
`-1280x720-armor-tip.png` (a swatch's tooltip), `-1920x1080.png`, `-3440x1305.png`, `-1080x1350.png`,
`-600x960-tools.png` (folded, the tools dropdown open), `-search.png`.

- **Left column**, top to bottom: the search; `ARMOR · <name>` and the armour swatches (every armour material
  research can unlock, in content order: the armour block's tile in that material, a light outline so the
  near-black tones are told apart, the picked one outlined teal, a locked one a lock that leads to its research;
  the tooltip names it with its build points, mass and hit points from content and the key 1–9; the wheel scrolls
  the strip past eight, by pixels since 14 Oct 2026, it snapped to whole swatches before); one line of category chips
  (the wheel or a drag scrolls it sideways, a fade shows over an edge with more behind it; a click glides the list to
  the section; the chip of the section at the top of the list is lit, the clicked one while the list glides there and
  while its heading is in view, and the lit chip glides into view); then **every part in
  sections** (`state::palette_sections`): a heading per category with its count (`WEAPONS 1 · 2 LOCKED`), the
  unlocked parts, then the locked ones. Rows: icon, name, footprint and key stat, the NEW tag, and how many are left
  (`∞`, a number once counts exist, coral at 0). The wheel scrolls the list smoothly by pixels (14 Oct 2026; until
  then it scrolled by whole entries, headings and rows, so icons were never cut; now a row cut at an edge hides its
  icon until it is whole, under the edge's fade). See [client](client.md) "Smooth scrolling". The search filters the whole list; empty sections and their chips go. The list, the swatches and
  the chips have the kit's overlay scrollbars (draggable; the strips' under them), and a lit row's, chip's or
  swatch's glow fades out past their edges (`ui_kit::Scrollbar`, [client](client.md)).
- **Top bar**: the breadcrumb and name left, the OVERLAY pills right (the armour moved to the left column).
- **Bottom bar**: undo and redo; PAINT, FILL, ERASE, SELECT, ROTATE; MIRROR and BEVEL; the status line over the
  voyage-ready line; SAVE, BLUEPRINTS, EXPORT and ⋯ as icon rings (tooltips with name and key); BEHAVIOR and
  TEST-FLY as words. Each status line is shortened to its room when it doesn't fit (whole parts dropped from the
  end, `FLIGHT-READY  ·  …`, else cut with `…`; the tooltip then shows both in full). When the status line would
  get less than `panels::MIN_STATUS` (120 px) the tools **fold**: the five pills become one dropdown showing the
  current tool (`PAINT ▾`, its menu opens upward with each tool and its key) and MIRROR and BEVEL become icon
  toggles (teal when on); they unfold when there is room again (measured from the layout, with a margin). The UI
  scale keeps the layout at least 1280 logical px wide (`ui_kit::ui_scale`), so at 1280 × 720, 1920 × 1080,
  3440 × 1305 and 1080 × 1350 the tools stay pills; they fold in windows under about 640 px wide (the scale's
  floor, 0.5), and wherever the bar gets more crowded.
- **The ⋯ menu** (opens upward, right-aligned; `actions::Rare`, carried out by `blueprints.rs` exactly as the
  BLUEPRINTS and EXPORT buttons are): NEW DESIGN (an empty design in this frame; unsaved changes are saved first),
  FRAME (the content frames as pills, the current one lit, locked ones dimmed), COPY SHARE CODE, DOWNLOAD .RON FILE
  (desktop: WRITE .RON FILE), IMPORT .RON FILE. The screens themselves stay as they were.
- **Menus** (`DrydockUi::menu`): one open at a time; Esc, a click outside (a veil under the menu takes it, so it
  reaches nothing else) and any action close it. The menus hold text only: part icons are world meshes drawn under
  bevy_ui (`ui_icons`).

**Gestures** (`tools::Gesture`, pure and tested): a press, drags and a release on frame cells become edits. It knows
nothing of the input device: the mouse drives it now; touch will drive the same gestures with the phone layout.

- **Paint** (B), by the picked part (`tools::Brush`): the 1×1 hull block **paints**: a drag paints every cell of
  the straight line between the cells it reached (no gaps on fast drags) and the whole drag is one stroke, one
  undo step; it replaces any machine it crosses, and with BEVEL its outer corners become slopes on release. A
  **slope** goes down with `place_slope` on each click, oriented to the hull until R / Shift+R (or Shift+wheel)
  turn the brush (`DrydockUi::slope_turn`; picking a part sets it back to the hull's choice); clicking a slope of
  the same part turns or repaints it. **Any other part** is placed once per click, its footprint in the brush
  rotation centred on the cell (rounded towards the top-left, like a slope); dragging does not place more. Slopes
  and parts replace what their footprint overlaps.
- **Fill** (G): drag a rectangle; it shows (with its mirror image) while dragging and fills on release (with BEVEL
  its corners are cut then; the rectangle shows no slopes while dragging); a click fills one cell.
- **Erase** (X, or the right button): click or drag, one stroke.
- **Select** (V): a click selects the part under the cursor, a click on an empty cell deselects. The selection is a
  cell (part indices change with every edit), exists only in the select tool and is cleared by another tool, by
  picking a palette part (which also switches select and erase to paint), Esc, Delete, when the part goes (an
  undo), and when another design opens or the frame changes (FRAME, its undo: every part moves). Leaving the
  drydock ends a gesture in progress (a stroke is closed; a fill or a move is dropped). Dragging the selected part moves it on release by the dragged cells (`move_part`); while dragging, the
  ghost shows where it and its twin would go, coral with the reason when they can't. Turns and flips that don't
  fit leave the part as it is and say why.
- **Mirror and bevel** are the editor's flags (`dock().mirror`, `dock().bevel`), the one source for the pills and the
  info line; `PlayerDesign::replace` keeps them when another design opens.

**Ghost** (`ghost.rs`): under the cursor the edit's `Preview` (`preview_paint`, `preview_slope`, `preview_place`,
`preview_move`): the parts it would put down as flight tiles at 55 % opacity (one mesh, a child of the design's
mesh, rebuilt only when the tiles change), outlined teal; a refused edit shows the part (or the cell) outlined coral.
The mirror twin is drawn too; a skipped twin is a coral outline. The parts the edit would **replace** get a coral
wash and a coral outline drawn just inside their footprint (so it shows under a new part of the same size). While a
paint stroke runs with BEVEL, the slopes its release will cut are outlined teal over the blocks
(`preview_stroke_end`; the blocks stay solid until then); before a click the paint ghost already shows the slope a
corner would get. A **shield part** (any part with a `shield`) shows its bubble while it is placed, with its mirror
twin's: centre and radius after bonuses from one `sim::design_report` of the design as the edit would leave it (the
drydock report's tuned content and config; made only when the ghost changes, tens of µs for a frigate; the part's
own radius if that design can't be compiled), drawn like the SHIELDS overlay (faint disc, soft rim,
`overlays::bubbles_mesh`). The selected shield part shows its bubble from the report in the select tool. The pointer's mark (`ghost::hover_mark`, 13 Oct
2026, Niklas): the cell under it (a faint teal fill and outline), but in SELECT and ERASE (and a right-button erase
stroke) over a part the part's whole footprint instead, filled and outlined once (teal to select, coral to erase:
what a click takes), whichever of its cells is hovered; erase outlines the mirror twin in coral too. The selected
part has teal corner brackets. Why the edit under the
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

### Locked parts, NEW and voyage-ready

Track 7F, 13 Oct 2026 ([GDR-0063](gdr/0063-salvage-and-research-v1.md), [GDR-0065](gdr/0065-phase-7-screens-and-escort-action.md),
mockup `docs/plans/mockups/drydock-locked.png`). What the player may build with is the career's
(`career::Granted::unlocks`, `docs/career.md`): the content's entries that are not `locked`, plus what research
unlocked; with `?unlock=all` (free mode) everything, and nothing below is locked or NEW. Screenshots:
`docs/results/career-drydock*.png`.

- **Locked parts are shown, not hidden** (GDR-0065 replaces GDR-0054's hiding for the career's drydock): each
  section of the palette lists its category's unlocked parts, then its locked ones, each in content order
  (`state::palette_sections`, filtered by the search). A locked row is dimmed, has a lock in place of its icon and
  says `LOCKED · <the research node that unlocks it>` (`career::unlocked_by`, the first node in content order). The
  section heading counts them: `WEAPONS 1 · 2 LOCKED`. A locked row is a disabled pill: a click sends nothing, and
  `actions::apply` refuses a locked `PickPart` anyway, so it is never picked or placed.
- **What no research unlocks stays hidden** (default, track 7F: GDR-0065's "the career's drydock shows what research
  gives"): a locked part or armour material no research node unlocks (in the base pack: reactive armour and the
  automaton plate) is not shown at all, as before; free mode shows it.
- **Locked materials and frames**: every armour material research can unlock has a swatch (the 1–9 keys count
  every armour material, hidden ones too); a locked one shows a lock instead of its tile and is refused, and so are
  its pills in the inspector (`state::material_shown`). The ⋯ menu's FRAME pills of a locked frame are dimmed. BLUEPRINTS' NEW and FRAME pills of a locked frame
  (the cruiser and the capital) are dimmed with a lock.
- **NEW**: a part research unlocked carries a gold NEW tag in the palette until it is picked there or a placed one
  is selected (whatever the SELECTED PART block shows: `panels::see_fresh` sends `career::SeenPart`). The career
  keeps what was seen in the save (`seen`, written at once), so a tag goes for good and a new game shows them again.
  (Default, track 7F: the save rather than settings, because NEW belongs to one career.)
- The palette's first unlocked part is picked at start, and the starting armour is the design's if it is
  unlocked, else the first unlocked one.
- **Designs that use locked parts** (the Halvex Runner template, an import, the automaton raider) still open, show
  and test-fly (GDR-0062 plan 3.1 step 11); they are only not voyage-ready.
- **The voyage-ready line** after the status line (`stats::voyage`, `career::voyage_line` from
  `data::career::voyage_ready` with the design as the flagship and FLEET's escorts):

| Fleet | Line | Tone |
|---|---|---|
| ready | `VOYAGE-READY · CORES 2 / 2` | teal |
| ready, but no ship has cargo | `VOYAGE-READY · CORES 2 / 2 · NO CARGO · YOU CAN'T BRING SALVAGE HOME` | gold |
| a locked part or material (first in placement order) | `NOT VOYAGE-READY · USES BEAM LASER · +2` (an escort's: `ESCORT 1 USES …`) | coral |
| a frame the career hasn't unlocked | `NOT VOYAGE-READY · NEEDS THE CRUISER FRAME` | coral |
| more cores than the mind runs | `NOT VOYAGE-READY · 3 CORES, YOUR MIND RUNS 2` | coral |
| the design can't fly | `NOT VOYAGE-READY` (the status line says why) | coral |

## Client: stats and overlays

Written 13 Oct 2026 (wave 2, track 5F). Code: `crates/game/src/drydock/stats.rs` (the stats grid and the status
line), `overlays.rs` (the overlays and issue marks); both read `report::DrydockReport` (the design's
`sim::design_report`, recomputed once per design revision or tuning change) and never compute anything of their
own. Tests: `cargo test -p game drydock` (the pure mappings: report → cells, report → status line and tone, the
heat colour ramp, issue marks, bonus link cells, the first blocker of a line of fire). Screenshots (1280 × 720,
195 %): `docs/results/drydock-stats-heat.png` (the Halvex Runner runs hot: the beams and shield emitters over
their limit, gold marks), `drydock-stats-overlays.png` (all five overlays), `drydock-stats-grounded.png`
(`?scene=drydock&thrust=0`: the tuning's thrust scale 0 grounds it, `CAN'T FLY · NO THRUST`, TEST-FLY disabled;
bonus and shields).

### Stats grid

Labels, units and precision come from content (`stats.ron`: the `SHIP_STATS` ids, `cargo_capacity`, `mass`,
`build_points`). Values are shortened to three significant digits, never with more decimals than the content's
(`10.29` → `10.3`, `107.25 t` → `107 t`). The words (yaw, device load) are UI chrome in code.

| Cell | From the report | Shown | Tone |
|---|---|---|---|
| POWER | `power.demand / power.supply` at full use | `50 / 54` (no unit, to fit), gold bar of the share | teal; coral and the bar over 100 % on a deficit |
| PEAK HEAT | `heat.peak` | `109%`, bar of the share | teal up to 100 %, gold up to `RUNS_HOT_MAX` (120 %, GDR-0052), coral above |
| THRUST/MASS | `flight.accel[0]` (forward) | `10.3` (no unit, to fit) | teal; coral without forward thrust |
| YAW | `flight.yaw / thrust[0]` (the thrust line's offset from the centre of mass), `yaw_balanced` | `Balanced` (offset ≤ `BALANCED_ARM` = 0.05 m), `Pulls left` (counter-clockwise), `Pulls right`; `—` without forward thrust | teal balanced; gold if the gyros and gimbals hold the pull, coral if not |
| FIREPOWER | `firepower` | `8.3/s` | white |
| CARGO | `cargo` | `8 units` | white |
| CONTROL | `control.load / capacity` | `18 / 32`, violet bar | violet; coral and over 100 % when parts idle |
| DEVICE LOAD | `device.class` | `Light`, `Moderate`, `Heavy`, `Too heavy` | teal, teal, gold, coral |

**TOP SPEED, TURN 90°, MASS and BUILD POINTS** sit in a compact block under the main grid: the same two
columns, smaller type (8 px labels, 12.5 px values), no bars, units shown (`22 m/s`, `1.5 s`, `106 t`, `132 BP`;
TURN `—` in coral when the design can't turn). Chosen over a hover detail because it is always visible, works
on touch and costs about 40 px of the right panel. A bar over 100 % (power deficit, overheating, over the control
capacity) fills its track and a white notch marks 100 % (`ui_kit::ValueBar`, share > 1).

### Status line

| Report | Line | Tone |
|---|---|---|
| `Err` (doesn't compile) | `CAN'T FLY · <WHY>` (the error) | coral |
| no parts | `CAN'T FLY · EMPTY FRAME` | coral |
| a blocker (first in the report's order) | `CAN'T FLY · NO LOGIC CORE` / `LOGIC CORES UNPOWERED` / `NO THRUST` | coral |
| no warning | `FLIGHT-READY` | teal |
| warnings | `FLIGHT-READY · <the most important> · +N` (N more) | the warning's |

Every case with `CAN'T FLY` sets `DrydockStatus::grounded` (TEST-FLY disabled). Warnings, most important first
(placeholder ranking: what breaks or harms the ship before what only makes it worse):

1. `A PART FALLS OFF` / `N PARTS FALL OFF` (coral)
2. `A PLUME BURNS THE SHIP` / `N PLUMES BURN THE SHIP` (coral)
3. `OVERHEATS AT FULL LOAD` (coral: the peak is above `RUNS_HOT_MAX`)
4. `POWER SHORT BY 4 MW` (gold)
5. `OVER CONTROL CAPACITY` (gold)
6. `GUN BLOCKED` / `N GUNS BLOCKED` (gold)
7. `PULLS LEFT UNDER THRUST` / `RIGHT` (gold; the client's own, from the yaw balance, when not held)
8. `RUNS HOT AT FULL LOAD` (gold: parts over their limit, the peak up to `RUNS_HOT_MAX`; the Halvex Runner)

### Overlays

The OVERLAY pills switch any combination. All of them are one world-space mesh (atlas material, vertex colours)
over the design's tiles, a child of the design's mesh, rebuilt only when the report is recomputed or a pill
changes (never per frame); sizes are in cells, so it zooms with the ship. Bottom to top:

- **HEAT**: every part of the main piece tinted by `heat_ratio` at full use: teal and faint when cool, gold at
  its limit, coral from `RUNS_HOT_MAX`, more opaque the hotter (`overlays::heat_tint`); parts over their limit
  outlined (gold while they only run hot, coral above).
- **MASS**: every part of the main piece shaded white by its mass per cell (content mass in its material over
  its area, from faint for the lightest to strongest for the densest); the centre of mass (a dark disc with the
  quartered white symbol and a cross); the forward thrust line (dashed teal, with an arrow) through
  `com.x + yaw / thrust`; when it misses the centre of mass, a curved arrow above it showing which way full
  thrust turns the ship (gold if held, coral if not).
- **BONUS**: one gold link per source and target pair across the side they share (the neighbouring cell pair
  nearest the middle of the two parts), with a gold diamond on a dark backing on the side (the concept board's
  links, the mockup's diamonds).
- **SHIELDS**: every bubble (centre and radius after bonuses): a faint disc (4 %), a soft rim glow and a crisp
  rim in the shield colour, the emitter's centre dotted.
- **ARCS**: every gun's line of fire to its range (clear: dashed weapons cyan with an end tick; blocked: coral up
  to the first blocker, a cross there, the blockers outlined); every engine's plume lanes to their reach (a soft
  propulsion-teal lane, dashed centre line and end tick; a lane that burns an own part: coral, a cross where it
  hits, the part outlined). Ranges reach far beyond a frigate frame (45–75 m): outside the frame the lines fade
  to 30 %, and they end at the grid viewport's edge (before 13 Oct 2026 they ran on under the veil, through the
  pills and the status line).

**Issue marks** (always, overlays on or off): a small triangle in the top-right corner of the bounding box of
every part an issue names: coral for blocked guns, parts a plume burns, parts that fall off and parts
overheating past `RUNS_HOT_MAX`; gold for parts that only run hot. Idle and power-short parts get no mark (the
CONTROL and POWER cells say it).

Placeholders: `stats::RUNS_HOT_MAX` (1.2, GDR-0052), `stats::BALANCED_ARM` (0.05 m), the warning ranking, the
overlay opacities and sizes.

## Storage and sharing

Written 13 Oct 2026 (Niklas: "a player storage layer, but a sharable format in addition"). Code:
`crates/game/src/storage.rs` (the layer), `clipboard.rs` (copy and paste), `drydock/library.rs` (designs on the
layer), `drydock/share.rs` (files and share codes), `drydock/blueprints.rs` (the screens and handlers). Tests:
`cargo test -p game -- storage library share`. Screenshots (1280 × 720): `docs/results/drydock-blueprints-list.png`
(BLUEPRINTS), `drydock-blueprints-export.png` (EXPORT, the code selected), `drydock-blueprints-cruiser.png` (a
saved template copy moved to the cruiser frame).

### The storage layer

- `Storage` (a resource) holds one `Backend` (a trait: `list(namespace)`, `read(namespace, name)`,
  `write(namespace, name, text)`, `delete(namespace, name)`, `location()`) and stores **named RON documents in
  namespaces**: `designs/<id>`, `drydock/working`, `drydock/behavior` (the BEHAVIOR screen's working draft) and
  `behaviors/<id>` (see
  [Behaviors](#behaviors-storage-designs-and-tests)) now; `saves/` and `settings/` later. Names are `a-z 0-9 _ -`
  (at most 64), safe as file names and keys everywhere.
- A document type implements `Document` (`NAMESPACE`, `FORMAT`, `to_ron`, `from_ron(text, format)`).
  `Storage::write` puts one header line before the RON, a comment, so the document stays valid RON:
  `// space-game: designs, format 1, saved 1760351234` (namespace, format version, Unix seconds).
  `Storage::read` refuses another namespace and formats newer than the build knows (`StorageError::Newer`) and
  hands older ones to `from_ron`, where migrations live (ADR-0024). A file without the header reads as format 1.
  `Storage::set_aside` copies a document's text as stored to another name (a working document this build can't
  read is kept before it is written over); `Storage::keep_on_leave` keeps a document that waits for its write, and
  the web backend writes it when the page goes away or is hidden (made into text only then).
- Backends: **web** the browser's local storage, keys `space-game/<namespace>/<name>` (per origin: the copies on
  :8080 and :8081 don't share designs; about 5 MB per origin, a design is 2–250 kB); **desktop** files
  `<data dir>/<namespace>/<name>.ron`, written to a temporary file and renamed (data dir: `$XDG_DATA_HOME` or
  `~/.local/share` on Linux, `~/Library/Application Support` on macOS, `%APPDATA%` on Windows, each
  `/space-game`); **memory** for tests and as the fallback when the platform gives no storage (the status line
  says so; Android until its backend, the files backend on the app's internal data path, lands).
- `StorageError` reaches the status line in words: no storage, storage full (the browser's quota, a full disk),
  I/O, unreadable document, newer format, bad name.

### Designs

- **SAVE** (the button, Ctrl+S) writes `designs/<id>`: the content ship format (`data::blueprint_to_ron`) in the
  design's frame (`size` = the frame, parts in frame coordinates), so opening it brings the frame back. The id
  comes from the name on the first save (`library::slug`: lowercase ASCII, umlauts spelled out, `_` between
  words, at most 40 characters, starting with a letter), unique among saved designs and never a content ship's
  id (`_2`, `_3`, ...); renaming keeps it. A design with problems still saves; the status line says
  "SAVED AS WORK IN PROGRESS · n PROBLEMS: …" with the first one (`behaviors::validate_design`: `data::validate_ship`
  with the player's behaviors known).
- **The working design** (`drydock/working`: `(saved_as: "<id>" or None, unsaved: <bool>, design: (<the ship in
  its frame>))`, so it comes back saved or not, with or without unsaved changes) is written about a second
  after the last change (`library::Autosave`, every change restarts the wait), at once
  when the id it is saved under changes, when a test flight starts and when the desktop app closes (on the web, a
  change still waiting is written when the page goes away or is hidden: a reload, a closed tab); it comes
  back at the next start, before the first frame, so a plain sandbox start flies it too (if it can't fly, the
  sandbox starts without the player's ship; T opens the drydock). The first start, and every new game
  (`?newgame=1`, a new save slot), is a copy of the career's start ship (`PlayerDesign::starter` with
  `PlayerCareer::start_ship`) named "Rim skiff copy"; a new game saves the last design's unsaved changes to
  BLUEPRINTS first (if that fails, the last design stays). Opening centres the parts in the frame
  (`data::design::edit::Drydock::open`), so an off-centre design comes back centred; the undo history is not kept. The
  document is untrusted (a damaged storage entry, a hand-edited file): a `saved_as` that is no design id is dropped
  (the design comes back unsaved; SAVE gives it a new id), and a document that doesn't read or open (a newer
  build's format, parts this content lacks) is copied to `drydock/working-unread` before the starter's autosave
  writes over it; the status line says so.
- **The name** top left: a click turns it into a text field; Enter or a click elsewhere renames the design
  (`PlayerDesign::edit_header`, no undo step), Esc keeps the old name. "UNSAVED" or "SAVED" stands after it.
- The breadcrumb names the frame from content ("DRYDOCK · FRIGATE FRAME"; a size for frames content lacks).

### BLUEPRINTS

The BLUEPRINTS button (an icon ring in the bottom bar since layout v2) opens a screen over everything above the bottom bar (the bar stays usable; Esc or CLOSE
closes it; Esc first leaves a rename, a delete question or a selected box):

- **Your designs** (most recently saved first: name; frame, parts' size, part count, when saved; "IN THE
  DRYDOCK" for the open one, "· UNSAVED" when it changed since) and **templates**, the content ships, opened as
  copies with a fresh name ("Sample mid copy", "Sample mid copy 2", ...) and not saved until they change or SAVE.
  The wheel scrolls a list (smoothly, by pixels since 14 Oct 2026; whole rows before). An unreadable document is
  listed with the reason, so it can be deleted.
- On the picked row: **OPEN** (a template: as a copy), **DUPLICATE** (a saved copy with a fresh name; for a
  template, its copy saved as a design), **RENAME** (a field; Enter renames, Esc cancels; the open design follows),
  **DELETE** (asks first: DELETE / KEEP; deleting the open design leaves it in the drydock, unsaved).
- **NEW** in a content frame (an empty design named "New design"), **FRAME** of the design in the drydock (the
  content frames as pills; a design that doesn't fit is refused with its size in the status line; a frame change
  is an undo step).
- **IMPORT**: a `.ron` file (web: the browser's file chooser; desktop: drop the file onto the window) or a share
  code or blueprint text pasted into the paste box (click it, Ctrl+V). An import is saved under a new id and
  opened.
- Opening, NEW and imports replace the design in the drydock; when it has unsaved changes it is **saved
  first** (the status line names it), so no work is lost; if that save fails, nothing is replaced (an import
  writes nothing then, neither its design nor its behavior).
- While BLUEPRINTS or EXPORT covers the design, its keys do nothing but Ctrl+S (save); the bottom bar's buttons
  stay usable.

### Sharing: EXPORT, files and share codes

- **EXPORT** opens a page with the design's `.ron` file and its share code, both made from the design trimmed to
  its parts (`PlayerDesign::export`) under its id (or one from its name): DOWNLOAD .RON on the web (a Blob behind
  a temporary link), WRITE .RON FILE on desktop (`<data dir>/exports/<id>.ron`, the path in the status line).
  The file is the content ship format, so it can go into a pack's `ships/` folder as is. A design that names one
  of the player's behaviors carries it after the ship as a comment block (`share::export_text`): a line
  `// behavior: behaviors/<id>.ron`, then that behavior file with every line commented out. Pack loaders skip it
  (it is the file to put into the pack's `behaviors/` folder); an import reads it.
- **Share code** (share format 2 since 13 Oct 2026, GDR-0057): `SGD2:` + base64url (no padding) of the
  zlib-compressed payload, a RON `(packs: [("base", "0.1.0"), ...], ship: (...), behavior: (...))` with the content
  packs (id, version) the design was made with and, when the design names one of the player's behaviors, that
  behavior in the content format (a content behavior goes by its id in the ship only). The Halvex Runner is 955
  characters; with a copy of the skirmisher (six rules with notes, its summary and description) 1429. Format 1
  codes (`SGD1:`, no behavior) still import. zlib's checksum catches damaged codes; whitespace inside a code (a chat program's line breaks), a
  lower-case tag and `=` padding are ignored. The code shows in a box: click it (it turns teal) and press Ctrl+C,
  or COPY.
- **Imports are bounded**: `share::read` takes at most 1 MB of text (a pasted code, RON text, a file: files are
  checked before they are read), a code at most 64 KB of compressed payload, unpacking to at most 1 MB; larger input
  is refused unread ("too large for a design"). The largest design, a 64 × 64 frame full of armour, is about
  280 kB of RON and a code of about 15 000 characters. EXPORT writes only `<design id>.ron` (no path).
- **Imports are checked** (`share::read`, `share::check`): a newer share format ("made by a newer version of the
  game (share format 9; this one reads 2)"), a damaged code, text that is no blueprint (with the RON error's line
  and column) and parts or materials the game doesn't have ("it uses part plasma_lance this game doesn't have
  (made with lances 2.0)": the packs of the code that aren't loaded in that version) give a message instead of a
  crash. An unknown faction becomes the player's; a behavior the design carries is saved
  (`PlayerBehaviors::import`: under its own id when that is free or already holds the same behavior, else under a
  new one) and the design names it; any other unknown behavior is dropped (noted in the status line); a design no
  frame takes is refused; other problems (`behaviors::validate_design`) are named but don't block.
- **Limits on text from outside** (`data::content::limits`, added 13 Oct 2026 after the untrusted-input review): reading a
  blueprint with the content options (`parse_blueprint`, a share code's payload, a stored document) refuses
  placement coordinates further than `MAX_COORD` (65536) from 0, and RON nested deeper than `RON_RECURSION_LIMIT`
  (32); `parse_blueprint` refuses texts over `MAX_DOCUMENT_BYTES` (4 MiB) and skips a byte order mark. The limits
  for the readers outside `data` are constants there too: `MAX_DOCUMENT_BYTES` for a stored document, a pasted or
  dropped file and a code's unpacked payload, `MAX_SHARE_CODE_BYTES` (1 MiB) for a code before it is decoded. A
  blueprint built in code may still hold any `i32`: `Drydock::open`, `validate_ship` and `Content::ship_grid`
  compute in `i64` and refuse what doesn't fit.
- **Copy and paste** (`clipboard.rs`): screens set `Clipboard::offer` (the text Ctrl+C copies), `accept` (Ctrl+V
  pastes) or `copy` (copy now). Web: winit cancels every key on the canvas, so while something is offered or
  accepted a capturing `keydown` listener keeps Ctrl/Cmd+C and +V from it, and the `copy` / `paste` listeners
  hand the text over; COPY uses `document.execCommand("copy")` (then `navigator.clipboard` in secure contexts).
  This works over plain http (`http://10.77.9.2:8080` is no secure context), tested in headless Chromium on both
  kinds of origin. Desktop: `arboard`. Android: not yet (COPY says so).

## Behaviors: storage, designs and tests

Written 13 Oct 2026 for the behavior editor ([GDR-0057](gdr/0057-behavior-editor-v1.md): behaviors are saved on
their own, a design names one, TEST IN RANGE watches it or flies alongside it). The plumbing under the BEHAVIOR
screen: `crates/game/src/drydock/behaviors.rs` (behaviors on the storage layer, the checks, flights),
`drydock/share.rs` (they travel with designs, [above](#sharing-export-files-and-share-codes)), `test_range.rs` and
`scene.rs` (TEST IN RANGE); on the screen `behavior_ui/saving.rs` (SAVE, the working draft, the design's behavior,
starting a test), `behavior_ui/list.rs` (BEHAVIORS) and `behavior_ui/live.rs` (the live panel in flight). Tests:
`cargo test -p game -- behaviors share test_range scene behavior_ui`. Screenshots (1280 × 720):
`docs/results/behavior-test-watch.png` (F3 on; `?scene=watch&behavior=careful`: a saved copy of the skirmisher that
flees below 50% hull, every rule with why), `behavior-test-alongside.png` (F3 on; `?scene=alongside`: the working
design names `careful`; the wingman on the right runs it, the player's ship sits unpiloted at the centre),
`behavior-saving-linked.png` (the skirmisher template renamed "Careful" and saved: the player's copy, and the design
flies it now), `behavior-saving-list.png` (BEHAVIORS), `behavior-live-watch.png` (TEST IN RANGE from the screen,
watch: the live panel, the last rule deciding, why the five above don't match).

### Saved behaviors

- **`behaviors/<id>`** (format 1) holds exactly a content behavior (`content/base/behaviors/*.ron`, see
  `docs/behavior-format.md` "In content files"): `id`, `name`, `summary`, optional `description`, `lore` and
  `tags`, `target` (the target line without its keyword) and `rules` (one string per rule), written by
  `behaviors::behavior_to_ron`, so a saved behavior can go into a pack's `behaviors/` folder as is:

  ```ron
  // space-game: behaviors, format 1, saved 1760351234
  (
      id: "careful",
      name: "Careful",
      summary: "Attack and strafe, dodge while the shields recharge, and run when badly hurt.",
      tags: ["attack"],
      target: "nearest",
      rules: [
          "when hull < 50% -> flee # run sooner",
          "always -> approach(67%)",
      ],
  )
  ```

- **Ids**: snake_case from the name on the first save (`PlayerBehaviors::new_id`, `library::slug_or(name,
  "behavior")`: "3 rules" → `behavior_3_rules`), unique among saved behaviors and **never a content behavior's
  id** ("Skirmisher" → `skirmisher_2`); `save` refuses a content id, and a stored document under one (a file copied
  in by hand) is listed as a problem and never flies. Renaming keeps the id, so designs keep naming it. Content
  behaviors (`skirmisher`, `raider_basic`, `escort`) are **templates**: `copy_of` makes an unsaved copy ("Skirmisher copy",
  `skirmisher_copy`), `duplicate` saves one.
- **Two copies of the game on one storage** (two browser tabs of one origin): a new behavior or design gets an id
  no saved document has, also one the other copy saved after this one started (`save` checks the storage), so
  neither overwrites the other; updates of the same saved behavior or design, the working design and the working
  draft are last-writer-wins.
- **Checked against the vocabulary** (the `behavior_nodes` content) on load and on every save, exactly as content
  validation checks a content behavior (`behaviors::check`: `Program::from_parts` + `check_program`). A behavior
  with errors **still loads, as its lines**, so the player can fix it: `PlayerBehavior::errors` locates them in its
  text form (`text_of`: line 1 is `target <selector>` when it has a target line, then one line per rule), e.g.
  `line 3, column 6: unknown condition `sheild_down`; did you mean `shield_down`?`. Only behaviors without errors
  fly. A document whose RON doesn't read is listed with the reason, so it can be deleted.
- **The text form**: `text_of(&behavior)` is what the TEXT view shows; `set_text(&mut behavior, text)` stores the
  editor's text (the canonical print when it parses, else the lines as typed, so a program with errors comes back as
  written); `set_program(&mut behavior, &program)` stores a `data::behavior::Program` (the graph view's model);
  `check_text(text, &content)` gives the live errors (by text line and column) or the program.
- **`PlayerBehaviors`** (a resource, read from the storage at start by `BehaviorsPlugin`): `get(id)`, `iter()`,
  `behavior(&content, id)` (a template or a saved one), `list()` (`BehaviorEntry`: id, name, summary, rule count,
  when saved, the first error or why it doesn't read; most recently saved first), `names()`, `new_id`, `copy_of`,
  `save(&mut storage, &content, &behavior, now)` (an empty id gets one from the name; returns the id),
  `rename`, `duplicate`, `delete`, `import` (behaviors that come with a shared design), `add_to(&mut content)`
  (flights), `named_by(&content, &blueprint)` (the player behavior a design names, for exports). Every operation
  writes the storage and the resource together; errors are words for the status line.

### Designs name their behavior

- The blueprint's `behavior` field names the behavior the design's ships fly when nobody pilots them: a content
  behavior id or one of the player's; none (the default, as before) means the faction's (`skirmisher` for the
  ship-mind). It is part of the design's header, so it stays through edits, saves, the working design and exports;
  the BEHAVIOR screen sets it with `PlayerDesign::edit_header(|h| h.behavior = Some(id))`.
- `behaviors::validate_design(&content, &behaviors, &blueprint)` is `data::validate_ship` with the player's
  behaviors known: a design naming one has no "unknown behavior" problem, its rules must fit the design's rule
  slots like a content behavior's, and one with errors is a problem of its own. SAVE and IMPORT use it.
- **Flights** (the sandbox, test flights, TEST IN RANGE): `scene.rs` adds every saved behavior without errors to the
  player's content (`PlayerBehaviors::add_to`: after the content behaviors, **in id order**), so
  `sim::BehaviorLibrary::new` compiles them with handles in a fixed order (behaviors are inputs of the deterministic
  sim, and a ship's behavior handle is part of the state hash; a test builds the library from storages filled in
  different orders and checks the handles and 300 ticks of state hashes are equal) and a design naming one compiles
  with it (`ShipDesign::behavior`). The player's ship when unpiloted, its `?fleet=` wingmen (their leader is the
  player's ship, so `target locked` follows the player's lock), re-forms and test ships fly it. A design naming a
  behavior that can't fly (it has errors, or was deleted) flies its faction's instead, with a banner
  (`behaviors::fallback`: "BEHAVIOR CAREFUL HAS 1 ERROR: …; FLIES SKIRMISHER").

### On the BEHAVIOR screen

Written 13 Oct 2026 (GDR-0057; the screen itself: `docs/client.md` "Behavior screen").

- **What opens**: the behavior the design flies (`draft::design_behavior`: the one it names, a content template or a
  saved one, also one with errors, which opens in the TEXT view with its errors; else its faction's default; a name
  that is gone gives the faction's, as flights do, and the status line says `BEHAVIOR <ID> IS GONE: <DESIGN> FLIES
  SKIRMISHER`). The header says what the draft is: TEMPLATE (`SAVE MAKES THIS DESIGN'S OWN COPY`), SAVED, UNSAVED
  or NOT SAVED YET, and `THE DESIGN FLIES <NAME>` when it is not the design's.
- **SAVE** (Ctrl+S, the button; `saving::save_draft`): a **template**'s draft becomes the player's copy under a new
  id: named "Skirmisher copy" (`PlayerBehaviors::copy_of`), or the draft's own name when it was renamed (the id from
  that name, `careful`); its summary, description and tags come along. When the design flew the template, the design
  names the copy from now on (`PlayerDesign::edit_header(|h| h.behavior = Some(id))`; the working design is written
  at once, so a reload keeps the link; the drydock shows the design UNSAVED, like any header change, until it is
  saved). A **saved behavior**'s draft updates it under its id (renaming keeps the id). A **new** one gets an id from
  its name. A draft with errors is saved as its lines ("SAVED · 1 ERROR: IT FLIES ONCE FIXED"); flights fly the
  faction's behavior until it is fixed.
- **Unsaved edits are never lost.** The draft stays in memory while the screen is closed (the drydock, a test) and
  is written to `drydock/behavior` about a second after the last change and when the screen closes (on the web also
  when the page goes away or is hidden before that second is over):
  `(source: "<id>" or None, name: "…", unsaved: <bool>, flown: "<id>" or None, text: "…")`, the behavior it was
  opened from, its name and text (the last valid program is the source's until the text is fixed), whether it has
  unsaved edits, and the behavior the design flew when it was opened. It comes back at the next visit after a reload;
  one that doesn't read (a newer build's) is copied to `drydock/behavior-unread` before it is written over, and the
  status line says so.
  While the design still flies that behavior, opening the screen keeps the draft (also one picked in BEHAVIORS);
  when the design flies another one (another design opened in the drydock), the screen opens that one. Whenever a
  draft with unsaved edits would be replaced (that, OPEN, USE, NEW), it is **saved first** (a template's as a new
  copy, not linked) and the status line says so ("YOUR EDITS SAVED AS SKIRMISHER COPY · OPENED RAIDER PACK"), as the
  drydock does with designs; if that save fails, nothing is replaced (and the draft still counts as the behavior it
  was opened for, so the next visit tries again).
- **Graph and inspector edits wait while the typed text has errors** ("fix the text first"): the editing model holds
  only the last valid program, and printing it would throw the typed lines away; the graph is read-only until the
  text is fixed in the TEXT view.
- **BEHAVIORS** (the list): your behaviors and the templates; **OPEN** edits one (the design keeps flying its own),
  **USE FOR THIS DESIGN** makes the design name it (a template by its content id) and opens it, **DUPLICATE** saves a
  copy with a fresh name, **RENAME** renames a saved one at once (the open draft follows; designs keep naming it by
  id), **DELETE** asks first (`DELETE CAREFUL? THERE IS NO UNDO · THIS DESIGN FLIES IT`). A design naming a deleted
  behavior keeps the name and flies its faction's (`behaviors::fallback`), and the status line says so ("… NAMED IT:
  IT FLIES SKIRMISHER UNTIL YOU USE ANOTHER"); deleting the open behavior leaves it on the screen, not saved (it is
  not saved again unless it changes). **NEW BEHAVIOR**: "New behavior" (a fresh name) with one rule (`always ->` the
  vocabulary's first action), not saved until SAVE.
- Checked 13 Oct 2026 in headless Chromium at 1280 × 720 (dev build, `scripts/web-drive.mjs`): drydock → BEHAVIOR →
  rename the skirmisher template → SAVE (the copy `careful`, linked) → reload → the drydock's design still names it
  and the screen opens it SAVED; BEHAVIORS: DUPLICATE, DELETE with its question (of the design's own: the fallback
  note), NEW, USE FOR THIS DESIGN of a template; a text with an error saved (Ctrl+S in the TEXT view) and reopened
  after a reload in the TEXT view with its error; TEST IN RANGE refused for that text; WATCH and FLY ALONGSIDE with
  the live panel, T back to the screen with the draft as it was. No panics or page errors.

### TEST IN RANGE

- The BEHAVIOR screen (T, TEST IN RANGE; `saving::test_request`) sends the draft: `TestProgram::Id(id)` when it is a
  template or saved behavior without unsaved edits, else `TestProgram::Text { name, text }` (its rules, not saved);
  in the mode of the WATCH / FLY ALONGSIDE pills; `back` is the BEHAVIOR screen, so T in the test returns to it, the
  draft untouched. It refuses, with the reason in the status line, when the text has errors (`CAN'T TEST: THE TEXT
  HAS 1 ERROR: LINE 9, COLUMN 6: …`), `test_range::check` fails, or the design can't fly (its design report as the
  drydock's status line reads it: `THE DESIGN CAN'T FLY (NO LOGIC CORE)`).
- The request is `test_range::StartTest { test: BehaviorTest { mode, program }, back }`:
  `mode` is `TestMode::Watch` or `TestMode::Alongside`; `program` is `TestProgram::Id(id)` (a content or saved
  behavior), `TestProgram::Text { name, text }` (the editor's text, not saved yet: the flight's library gets it
  under the id `test:behavior`, so it never shadows a saved one) or `TestProgram::Design` (the one the design
  names); `back` is the mode T returns to (the behavior screen's). `test_range::check(&content, &behaviors,
  &program)` says beforehand whether it can fly (the first error, if not). From the drydock or the behavior screen
  the request launches a flight; in flight it rebuilds the scene.
- **Watch**: the player's design spawns at the centre, unpiloted, on the behavior, raider waves on; the camera
  follows it and the HUD shows it (hull, speed, warnings; title `WATCH · <BEHAVIOR> · WAVE n`). Nothing is
  piloted: focus switches (V, clicks, the fleet bar) are dropped. Losing it ends the test after 3 s ("SHIP LOST ·
  BACK TO THE …").
- **Fly alongside**: the player flies the design as usual (it runs the design's own behavior when unpiloted); a
  second copy starts 22 m to the left and 12 m behind as a wingman on the tested behavior, its leader the player's
  ship. Title `FLY ALONGSIDE · <BEHAVIOR> · WAVE n`. Losing every vessel ends the test like a test flight.
- In both: no `?fleet=` wingmen; T returns to where the test started (`mode::FlightExit`, back to the drydock
  whenever flight ends); Shift+R starts the test over; C turns the waves off and on.
- URL (for testing, no screen needed): `?scene=watch&behavior=<id>`, `?scene=alongside&behavior=<id>` (a content or
  saved behavior; without `behavior` the design's own). A behavior that can't fly leaves the design's own, with a
  banner. T returns to the drydock.
- **Live view**: `test_range::BehaviorWatch` (a resource) holds the tested ship's behavior, refreshed after each of
  its thinks (10 Hz; `is_changed()` says when): `mode`, `ship`, `running` (false while piloted, lost or before the
  first think), `behavior` (its id in the flight's library), `name` and `program` (its rules), `active` (the rule
  that decides, `None`:
  no rule matches and the ship holds), `rules` (every rule's `sim::RuleStatus`: `Active`, `Skipped`, `NoSlot`,
  `NoTarget`, `Failed { cond, why }`), `why` (one line per rule, `BehaviorState::describe`: "`hull < 50%` is false
  (hull is 96%)"), `slots` (working rule slots), `target`, `think` (its tick). Empty outside a test. The HUD's
  live panel (`behavior_ui/live.rs`) shows the rules as the graph's chip labels, the one that decides teal, rules
  without a slot dimmed and why the rules above it don't match; the F3 overlay lists every rule with why and `>` at
  the one that decides.
- Checked 13 Oct 2026 in headless Chromium (dev build, `scripts/web-drive.mjs`; the player's behavior and working
  design put into the browser's storage with `eval`, then `reload`): watch and fly alongside with a content
  behavior (`raider_basic`) and with a saved one (`careful`, by id and named by the design), a URL naming a behavior
  that isn't saved (flies the design's own, `skirmisher`, with a banner), V ignored while watching, T back to the
  drydock. No panics or page errors.
