//! Editing on the grid: the tools turn pointer gestures into `data::edit` calls on the player's design, and the
//! editing actions (undo, redo, mirror, bevel, rotate, flip, delete, the inspector's material and power) run
//! here. Every edit goes through [`PlayerDesign::edit`]; the editor's `mirror` and `bevel` flags are the modes.
//!
//! * [`Gesture`] is the pure part: a press, drags and a release on frame cells become [`Op`]s (a paint or erase
//!   drag is one stroke and one undo step, fill drags a rectangle, select grabs a part and moves it on
//!   release). It knows nothing of the input device: [`pointer`] feeds it the mouse now (left button: the tool,
//!   right button: erase), touch will feed it with the phone layout.
//! * [`Selection`]: the selected part, kept as a cell (part indices change with every edit). It exists only in
//!   the select tool.
//! * Refusals (and skipped mirror edits) go to [`EditNote`], shown after the grid's info line.
//!
//! What the paint tool does depends on the picked part ([`Brush`]): the 1×1 hull block paints (a drag paints
//! every cell it crosses; with BEVEL the stroke's outer corners become slopes when it ends), a slope orients itself
//! on a click (clicking it again turns it) unless R / Shift+R turned the brush, any other part is placed on a click
//! with its footprint centred on the cell in the brush rotation (R / Shift+R, Q / E). Painting and placing replace
//! what is under them (the editing model's rule).

use bevy::prelude::*;
use data::edit::{Outcome, Refusal, Twin};
use data::{Cell, Content, MAX_POWER_WEIGHT, Part, Placement, PowerOverride, PowerTier, Shape};

use crate::sim_bridge::GameContent;
use crate::style::Style;
use crate::ui_icons::UiIcon;
use crate::ui_kit::{Ring, UiPointer};

use super::PlayerDesign;
use super::actions::{ActionButton, DrydockAction};
use super::state::{DrydockUi, Tool};
use super::view::DrydockCursor;

/// How long a refusal stays after the info line, in seconds.
const NOTE_SECS: f32 = 3.0;
/// The power weight stepper: steps and range (UI placeholders; content may set any weight up to
/// [`MAX_POWER_WEIGHT`]).
pub const WEIGHT_STEP: f32 = 0.5;
pub const WEIGHT_RANGE: (f32, f32) = (0.5, 10.0);

/// The selected part, as a cell it covers; `None`: nothing selected.
#[derive(Resource, Default, Debug)]
pub struct Selection {
    pub cell: Option<Cell>,
}

impl Selection {
    /// The selected part's index in the design right now.
    pub fn part(&self, design: &PlayerDesign) -> Option<usize> {
        self.cell.and_then(|c| design.dock().part_at(c))
    }
}

/// Why the last edit could not happen (or happened without its mirror image), for the info line.
#[derive(Resource, Default, Debug)]
pub struct EditNote {
    pub text: Option<String>,
    /// When it was written (real seconds); it fades after [`NOTE_SECS`].
    at: f32,
}

impl EditNote {
    /// The note while it is fresh.
    pub fn current(&self, now: f32) -> Option<&str> {
        self.text.as_deref().filter(|_| now - self.at < NOTE_SECS)
    }
}

/// What the paint tool does with the picked part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Brush {
    /// The 1×1 hull block: paints cells (and repaints hull parts) in the armour material.
    Paint,
    /// A slope: placed oriented to the hull (or as R / Shift+R turned it); clicking it again turns it.
    Slope,
    /// Any other part: placed on a click, centred on the cell, in the brush rotation.
    Place,
}

impl Brush {
    pub fn of(part: &Part) -> Brush {
        match (part.hull, part.shape, part.size) {
            (_, Shape::Slope, _) => Brush::Slope,
            (true, Shape::Rect, (1, 1)) => Brush::Paint,
            _ => Brush::Place,
        }
    }
}

/// The placement [`Brush::Place`] puts down: the footprint in `rot` centred on the cell (rounded towards the
/// top-left, like an auto-oriented slope), hull parts in the armour material.
pub fn brush_placement(part: &Part, (cx, cy): Cell, rot: u8, material: &str) -> Placement {
    let (w, h) = part.footprint().dims(rot);
    Placement {
        part: part.id.clone(),
        x: cx - (w as i32 - 1) / 2,
        y: cy - (h as i32 - 1) / 2,
        rot,
        flip: false,
        material: part.hull.then(|| material.to_string()),
        power: None,
    }
}

/// One pointer gesture on the grid, from press to release.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Gesture {
    #[default]
    Idle,
    /// A paint or erase drag, one stroke; `last` is the last cell it reached.
    Stroke { erase: bool, last: Cell },
    /// A fill rectangle between two corner cells.
    Rect { from: Cell, to: Cell },
    /// The selected part, grabbed at `from`, is dragged to `to`.
    Move { from: Cell, to: Cell },
    /// A press that has done its edit and waits for the release.
    Once,
}

/// An edit a gesture asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    BeginStroke,
    EndStroke,
    Paint(Cell),
    Slope(Cell),
    Place(Cell),
    Erase(Cell),
    Fill(Cell, Cell),
    /// Select the part covering the cell, or nothing.
    Select(Option<Cell>),
    /// Move the part covering `from` by `(dx, dy)`.
    Move { from: Cell, dx: i32, dy: i32 },
}

impl Gesture {
    /// A press on `cell` with `tool` (the paint tool with the picked part's `brush`); `part_here`: a part
    /// covers the cell (the select tool grabs it).
    pub fn press(tool: Tool, brush: Option<Brush>, cell: Cell, part_here: bool) -> (Gesture, Vec<Op>) {
        match (tool, brush) {
            (Tool::Paint, Some(Brush::Paint)) => (Gesture::Stroke { erase: false, last: cell }, vec![Op::BeginStroke, Op::Paint(cell)]),
            (Tool::Paint, Some(Brush::Slope)) => (Gesture::Once, vec![Op::Slope(cell)]),
            (Tool::Paint, Some(Brush::Place)) => (Gesture::Once, vec![Op::Place(cell)]),
            (Tool::Paint, None) => (Gesture::Once, Vec::new()),
            (Tool::Erase, _) => (Gesture::Stroke { erase: true, last: cell }, vec![Op::BeginStroke, Op::Erase(cell)]),
            (Tool::Fill, _) => (Gesture::Rect { from: cell, to: cell }, Vec::new()),
            (Tool::Select, _) if part_here => (Gesture::Move { from: cell, to: cell }, vec![Op::Select(Some(cell))]),
            (Tool::Select, _) => (Gesture::Once, vec![Op::Select(None)]),
        }
    }

    /// The pointer, still pressed, reached `cell`. A stroke edits every cell of the straight line from the last
    /// one, so a fast drag leaves no gaps.
    pub fn drag(&mut self, cell: Cell) -> Vec<Op> {
        match self {
            Gesture::Stroke { erase, last } if *last != cell => {
                let erase = *erase;
                let ops = line(*last, cell).into_iter().skip(1).map(|c| if erase { Op::Erase(c) } else { Op::Paint(c) }).collect();
                *last = cell;
                ops
            }
            Gesture::Rect { to, .. } | Gesture::Move { to, .. } => {
                *to = cell;
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// The press ended: close the stroke, fill the rectangle, move the part.
    pub fn release(&mut self) -> Vec<Op> {
        let ops = match *self {
            Gesture::Stroke { .. } => vec![Op::EndStroke],
            Gesture::Rect { from, to } => vec![Op::Fill(from, to)],
            Gesture::Move { from, to } if from != to => vec![Op::Move { from, dx: to.0 - from.0, dy: to.1 - from.1 }],
            _ => Vec::new(),
        };
        *self = Gesture::Idle;
        ops
    }
}

/// The cells of the straight line from `a` to `b`, both included (Bresenham).
pub fn line(a: Cell, b: Cell) -> Vec<Cell> {
    let (dx, dy) = ((b.0 - a.0).abs(), -(b.1 - a.1).abs());
    let (sx, sy) = ((b.0 - a.0).signum(), (b.1 - a.1).signum());
    let (mut x, mut y, mut err) = (a.0, a.1, dx + dy);
    let mut out = vec![(x, y)];
    while (x, y) != b {
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
        out.push((x, y));
    }
    out
}

/// The gesture in progress and the mouse button that drives it (the ghost draws the fill rectangle and the
/// moved part from it).
#[derive(Resource, Default, Debug)]
pub struct DrydockGesture {
    pub gesture: Gesture,
    button: Option<MouseButton>,
}

/// The mouse on the grid: the left button runs the current tool, the right button erases. Space + left and the
/// middle button pan (the camera), so they never edit; a press over a panel never reaches the grid.
#[allow(clippy::too_many_arguments)]
pub(super) fn pointer(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    pointer: Res<UiPointer>,
    cursor: Res<DrydockCursor>,
    content: Res<GameContent>,
    ui: Res<DrydockUi>,
    time: Res<Time<Real>>,
    mut design: ResMut<PlayerDesign>,
    mut sel: ResMut<Selection>,
    mut g: ResMut<DrydockGesture>,
    mut note: ResMut<EditNote>,
) {
    let content = &content.0;
    let mut ops = Vec::new();
    if let Some(button) = g.button {
        if let Some(cell) = cursor.cell {
            ops.extend(g.gesture.drag(cell));
        }
        if !buttons.pressed(button) {
            ops.extend(g.gesture.release());
            g.button = None;
        }
    } else if !keys.pressed(KeyCode::Space)
        && !pointer.blocked
        && let Some(cell) = cursor.cell
        && let Some((button, tool)) = [(MouseButton::Left, ui.tool), (MouseButton::Right, Tool::Erase)].into_iter().find(|(b, _)| buttons.just_pressed(*b))
    {
        let brush = ui.picked.as_deref().and_then(|p| content.parts.get(p)).map(Brush::of);
        let (gesture, first) = Gesture::press(tool, brush, cell, design.dock().part_at(cell).is_some());
        *g = DrydockGesture { gesture, button: Some(button) };
        ops = first;
    }
    for op in ops {
        if let Some(why) = run(op, content, &ui, &mut design, &mut sel) {
            note.text = Some(why);
            note.at = time.elapsed_secs();
        }
    }
}

/// Leaving the drydock (T during a drag): a gesture in progress ends here, its stroke closed and nothing more done
/// (no fill, no move), so the first frame of the next visit doesn't continue it from wherever the pointer is.
pub(super) fn leave(mut design: ResMut<PlayerDesign>, mut g: ResMut<DrydockGesture>) {
    if g.button.is_some() {
        design.edit(|d| d.end_stroke());
        *g = DrydockGesture::default();
    }
}

/// Carries out one op; returns why it could not happen (or happened without its mirror image).
fn run(op: Op, content: &Content, ui: &DrydockUi, design: &mut PlayerDesign, sel: &mut Selection) -> Option<String> {
    let m = ui.material.as_str();
    let picked = ui.picked.as_deref().and_then(|p| content.parts.get(p));
    let result = match op {
        Op::BeginStroke => {
            design.edit(|d| d.begin_stroke());
            return None;
        }
        Op::EndStroke => {
            design.edit(|d| d.end_stroke());
            return None;
        }
        Op::Select(cell) => {
            sel.cell = cell.filter(|&c| design.dock().part_at(c).is_some());
            return None;
        }
        Op::Paint(cell) => design.edit(|d| d.paint(cell, m)),
        Op::Slope(cell) => {
            let part = picked?.id.as_str();
            design.edit(|d| d.place_slope(part, cell, m, ui.slope_turn))
        }
        Op::Place(cell) => {
            let p = brush_placement(picked?, cell, ui.rot, m);
            design.edit(|d| d.place(p))
        }
        Op::Erase(cell) => design.edit(|d| d.erase(cell)),
        Op::Fill(a, b) => design.edit(|d| d.fill(a, b, m)),
        Op::Move { from, dx, dy } => {
            let i = design.dock().part_at(from)?;
            let r = design.edit(|d| d.move_part(i, dx, dy));
            if r.is_ok() {
                sel.cell = Some((from.0 + dx, from.1 + dy));
            }
            r
        }
    };
    outcome_note(content, design, &result)
}

/// The words for a refused edit or a skipped mirror image (`None` when all went through).
fn outcome_note(content: &Content, design: &PlayerDesign, result: &Result<Outcome, Refusal>) -> Option<String> {
    match result {
        Err(r) => Some(refusal_text(content, design, r, None)),
        Ok(Outcome { twin: Twin::Skipped(r), .. }) => Some(format!("MIRROR SKIPPED  ·  {}", refusal_text(content, design, r, None))),
        Ok(_) => None,
    }
}

/// Why an edit can't happen, in the UI's words (part names from content). `new_part`: the index a part an edit
/// would add gets after the edit, and its id, named when a mirror image would overlap it (a preview's
/// `Refusal::Overlaps` of that index).
pub fn refusal_text(content: &Content, design: &PlayerDesign, r: &Refusal, new_part: Option<(usize, &str)>) -> String {
    let name = |id: &str| content.parts.get(id).map_or(id.to_uppercase(), |p| p.name.to_uppercase());
    match r {
        Refusal::Outside => "OUTSIDE THE FRAME".into(),
        Refusal::Overlaps(k) => match new_part.filter(|n| n.0 == *k).map(|n| n.1).or_else(|| design.dock().parts().get(*k).map(|p| p.part.as_str())) {
            Some(id) => format!("OVERLAPS {}", name(id)),
            None => "OVERLAPS A PART".into(),
        },
        Refusal::Empty => "NOTHING THERE".into(),
        Refusal::NoSuchPart(_) => "NO SUCH PART".into(),
        Refusal::UnknownPart(id) => format!("UNKNOWN PART {}", id.to_uppercase()),
        Refusal::NotArmor(m) => format!("{} IS NO ARMOR", content.materials.get(m).map_or(m.to_uppercase(), |m| m.name.to_uppercase())),
        Refusal::NeedsMaterial => "NEEDS AN ARMOR MATERIAL".into(),
        Refusal::NotHull => "ONLY HULL PARTS TAKE ARMOR".into(),
        Refusal::DrawsNoPower => "DRAWS NO POWER".into(),
        Refusal::BadWeight => "POWER WEIGHT OUT OF RANGE".into(),
        Refusal::BadRotation => "BAD ROTATION".into(),
        Refusal::NotSlope => "NOT A SLOPE".into(),
        Refusal::NoRoom => "NO ROOM FOR THE SLOPE".into(),
        Refusal::NoBlockPart => "NO ARMOR BLOCK IN THE CONTENT".into(),
        Refusal::BadFrame => "BAD FRAME SIZE".into(),
        Refusal::NoTwin => "NO MIRROR TWIN".into(),
        Refusal::NoneLeft(id) => format!("NO {} LEFT", name(id)),
    }
}

/// The editing actions: undo / redo, the mirror and bevel modes, rotate (the selected part, else the brush),
/// flip, delete and deselect, the selected part's material and power; the selection follows the tools.
#[allow(clippy::too_many_arguments)]
pub(super) fn apply(
    mut actions: MessageReader<DrydockAction>,
    content: Res<GameContent>,
    time: Res<Time<Real>>,
    cursor: Res<DrydockCursor>,
    mut design: ResMut<PlayerDesign>,
    mut ui: ResMut<DrydockUi>,
    mut sel: ResMut<Selection>,
    mut g: ResMut<DrydockGesture>,
    mut note: ResMut<EditNote>,
    mut placed: Local<Option<(u64, (u32, u32))>>,
) {
    let content = &content.0;
    // Another design (OPEN, NEW, an import) or another frame (FRAME, or its undo: every part moved): the selected
    // cell would land on another part, and a gesture in progress on another place. Checked before the actions (a
    // design opened this frame) and after them (an undo of a frame change).
    let mut forget_if_moved = |design: &PlayerDesign, sel: &mut ResMut<Selection>| {
        let now = (design.generation(), design.frame());
        if placed.is_some_and(|p| p != now) {
            sel.cell = None;
            *g = DrydockGesture::default();
        }
        *placed = Some(now);
    };
    forget_if_moved(&design, &mut sel);
    for a in actions.read() {
        let selected = sel.part(&design);
        let result = match (a, selected) {
            (DrydockAction::Tool(t), _) => {
                if *t != Tool::Select {
                    sel.cell = None;
                }
                continue;
            }
            (DrydockAction::PickPart(_), _) => {
                if matches!(ui.tool, Tool::Select | Tool::Erase) {
                    ui.tool = Tool::Paint;
                }
                sel.cell = None;
                continue;
            }
            (DrydockAction::Mirror, _) => {
                design.edit(|d| d.mirror = !d.mirror);
                continue;
            }
            (DrydockAction::Bevel, _) => {
                design.edit(|d| d.bevel = !d.bevel);
                continue;
            }
            (DrydockAction::Undo, _) => {
                design.edit(|d| d.undo());
                continue;
            }
            (DrydockAction::Redo, _) => {
                design.edit(|d| d.redo());
                continue;
            }
            (DrydockAction::Deselect, _) => {
                sel.cell = None;
                continue;
            }
            (DrydockAction::RotateCcw | DrydockAction::RotateCw, None) => {
                let cw = *a == DrydockAction::RotateCw;
                match ui.picked.as_deref().and_then(|p| content.parts.get(p)).filter(|p| Brush::of(p) == Brush::Slope) {
                    Some(part) => ui.slope_turn = turn_slope(&design, part, ui.slope_turn, cursor.cell, &ui.material, cw),
                    None => ui.rot = (ui.rot + if cw { 1 } else { 3 }) % 4,
                }
                continue;
            }
            (DrydockAction::RotateCcw | DrydockAction::RotateCw, Some(i)) => design.edit(|d| d.rotate(i, *a == DrydockAction::RotateCw)),
            (DrydockAction::Flip, Some(i)) => design.edit(|d| d.flip(i)),
            // The selected part and, mirroring, its twin (not whatever covers its mirror cells).
            (DrydockAction::Delete, Some(i)) => {
                sel.cell = None;
                design.edit(|d| d.remove_part(i))
            }
            (DrydockAction::PartMaterial(m), Some(i)) => design.edit(|d| d.set_material(i, m)),
            (DrydockAction::PartTier(t), Some(i)) => {
                let o = power_with(content, &design.dock().parts()[i], Some(*t), None);
                design.edit(|d| d.set_power(i, o))
            }
            (DrydockAction::PartWeight(up), Some(i)) => {
                let p = &design.dock().parts()[i];
                let w = power_setting(content, p).map_or(1.0, |s| s.1);
                let step = if *up { WEIGHT_STEP } else { -WEIGHT_STEP };
                let w = ((w / WEIGHT_STEP).round() * WEIGHT_STEP + step).clamp(WEIGHT_RANGE.0, WEIGHT_RANGE.1.min(MAX_POWER_WEIGHT));
                let o = power_with(content, p, None, Some(w));
                design.edit(|d| d.set_power(i, o))
            }
            _ => continue,
        };
        // The selection follows the edited part (a turn can leave the selected cell).
        if let Ok(Outcome { part: Some(k), .. }) = &result
            && sel.cell.is_some()
        {
            let p = &design.dock().parts()[*k];
            sel.cell = Some((p.x, p.y));
        }
        if let Some(why) = outcome_note(content, &design, &result) {
            note.text = Some(why);
            note.at = time.elapsed_secs();
        }
    }
    forget_if_moved(&design, &mut sel);
    // An undo can take the selected part away.
    if sel.cell.is_some() && sel.part(&design).is_none() {
        sel.cell = None;
    }
}

/// A slope brush's next orientation in turning order (back with `cw` false): from the one it was turned to, else
/// (on auto) from the one the hull gives it under the cursor, so the first turn starts from what the ghost shows.
/// With neither, the first orientation.
fn turn_slope(design: &PlayerDesign, part: &Part, turned: Option<(u8, bool)>, cell: Option<Cell>, material: &str, cw: bool) -> Option<(u8, bool)> {
    let dock = design.dock();
    let auto = || cell.and_then(|c| dock.preview_slope(&part.id, c, material, None).ok()).map(|p| (p.placement.rot, p.placement.flip));
    match turned.or_else(auto) {
        Some(from) => dock.next_orientation(&part.id, from, cw).ok(),
        None => dock.slope_orientations(&part.id).ok()?.first().copied(),
    }
}

/// A consumer's effective power tier and weight: the placement's override, else the part's defaults. `None` for
/// parts that draw no power.
pub fn power_setting(content: &Content, p: &Placement) -> Option<(PowerTier, f32)> {
    let spec = content.parts.get(&p.part)?.power.as_ref().filter(|s| s.draw > 0.0)?;
    let o = p.power.unwrap_or_default();
    Some((o.tier.unwrap_or(spec.tier), o.weight.unwrap_or(spec.weight)))
}

/// The override that gives a placement this tier and / or weight (the other kept); a value equal to the part's
/// default is no override, so going back to the defaults leaves the blueprint clean (`None`).
fn power_with(content: &Content, p: &Placement, tier: Option<PowerTier>, weight: Option<f32>) -> Option<PowerOverride> {
    let spec = content.parts.get(&p.part)?.power.as_ref()?;
    let (t, w) = power_setting(content, p)?;
    let (t, w) = (tier.unwrap_or(t), weight.unwrap_or(w));
    let o = PowerOverride { tier: (t != spec.tier).then_some(t), weight: (w != spec.weight).then_some(w) };
    (o != PowerOverride::default()).then_some(o)
}

/// Undo and redo look disabled (dim ring and arrow) when there is nothing to undo or redo.
pub(super) fn sync_rings(
    design: Res<PlayerDesign>,
    style: Res<Style>,
    mut rings: Query<(&ActionButton, &mut Ring, &Children)>,
    mut icons: Query<&mut UiIcon>,
) {
    if !design.is_changed() {
        return;
    }
    for (b, mut ring, children) in &mut rings {
        let disabled = match b.0 {
            DrydockAction::Undo => !design.dock().can_undo(),
            DrydockAction::Redo => !design.dock().can_redo(),
            _ => continue,
        };
        if ring.disabled != disabled {
            ring.disabled = disabled;
        }
        let want = if disabled { style.ui.dim } else { style.ui.number };
        for child in children {
            if let Ok(mut icon) = icons.get_mut(*child)
                && let UiIcon::Glyph { color, .. } = &mut *icon
                && *color != want
            {
                *color = want;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_have_no_gaps() {
        assert_eq!(line((2, 3), (2, 3)), vec![(2, 3)]);
        assert_eq!(line((0, 0), (3, 0)), vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
        assert_eq!(line((0, 0), (-2, -2)), vec![(0, 0), (-1, -1), (-2, -2)]);
        for (a, b) in [((0, 0), (7, 3)), ((5, 9), (1, 0)), ((-3, 4), (4, -6))] {
            let l = line(a, b);
            assert_eq!((l[0], *l.last().unwrap()), (a, b));
            // Every step moves to a neighbouring cell (diagonals included) and never back.
            for w in l.windows(2) {
                let (dx, dy) = (w[1].0 - w[0].0, w[1].1 - w[0].1);
                assert!(dx.abs() <= 1 && dy.abs() <= 1 && (dx, dy) != (0, 0), "{l:?}");
            }
            assert_eq!(l.len() as i32, (b.0 - a.0).abs().max((b.1 - a.1).abs()) + 1);
        }
    }

    #[test]
    fn a_paint_drag_is_one_stroke_over_every_crossed_cell() {
        let (mut g, ops) = Gesture::press(Tool::Paint, Some(Brush::Paint), (1, 1), false);
        assert_eq!(ops, [Op::BeginStroke, Op::Paint((1, 1))]);
        assert_eq!(g.drag((1, 1)), []);
        assert_eq!(g.drag((4, 1)), [Op::Paint((2, 1)), Op::Paint((3, 1)), Op::Paint((4, 1))]);
        assert_eq!(g.drag((4, 2)), [Op::Paint((4, 2))]);
        assert_eq!(g.release(), [Op::EndStroke]);
        assert_eq!(g, Gesture::Idle);
        // Erase (and the right button) drags the same way.
        let (mut g, ops) = Gesture::press(Tool::Erase, Some(Brush::Place), (0, 0), true);
        assert_eq!(ops, [Op::BeginStroke, Op::Erase((0, 0))]);
        assert_eq!(g.drag((0, 2)), [Op::Erase((0, 1)), Op::Erase((0, 2))]);
        assert_eq!(g.release(), [Op::EndStroke]);
    }

    #[test]
    fn machines_and_slopes_go_down_once_per_click() {
        for (brush, op) in [(Brush::Place, Op::Place((3, 4))), (Brush::Slope, Op::Slope((3, 4)))] {
            let (mut g, ops) = Gesture::press(Tool::Paint, Some(brush), (3, 4), false);
            assert_eq!(ops, [op]);
            assert_eq!(g.drag((5, 4)), []);
            assert_eq!(g.release(), []);
        }
        let (_, ops) = Gesture::press(Tool::Paint, None, (3, 4), false);
        assert_eq!(ops, []);
    }

    #[test]
    fn fill_drags_a_rectangle() {
        let (mut g, ops) = Gesture::press(Tool::Fill, Some(Brush::Paint), (2, 2), false);
        assert_eq!(ops, []);
        assert_eq!(g.drag((6, 1)), []);
        assert_eq!(g, Gesture::Rect { from: (2, 2), to: (6, 1) });
        assert_eq!(g.release(), [Op::Fill((2, 2), (6, 1))]);
        // A click fills one cell.
        let (mut g, _) = Gesture::press(Tool::Fill, None, (2, 2), false);
        assert_eq!(g.release(), [Op::Fill((2, 2), (2, 2))]);
    }

    #[test]
    fn select_grabs_a_part_and_moves_it_on_release() {
        let (mut g, ops) = Gesture::press(Tool::Select, None, (5, 5), true);
        assert_eq!(ops, [Op::Select(Some((5, 5)))]);
        g.drag((6, 5));
        g.drag((7, 3));
        assert_eq!(g.release(), [Op::Move { from: (5, 5), dx: 2, dy: -2 }]);
        // Back where it started: no move.
        let (mut g, _) = Gesture::press(Tool::Select, None, (5, 5), true);
        g.drag((6, 5));
        g.drag((5, 5));
        assert_eq!(g.release(), []);
        // A click on an empty cell deselects.
        let (mut g, ops) = Gesture::press(Tool::Select, None, (0, 0), false);
        assert_eq!(ops, [Op::Select(None)]);
        assert_eq!(g.release(), []);
    }

    #[test]
    fn brushes_and_placements_follow_the_part() {
        let c = data::base_content();
        let part = |id: &str| c.parts.get(id).unwrap();
        assert_eq!(Brush::of(part("armor_block")), Brush::Paint);
        assert_eq!(Brush::of(part("slope_1x1")), Brush::Slope);
        assert_eq!(Brush::of(part("slope_1x3")), Brush::Slope);
        assert_eq!(Brush::of(part("fusion_reactor")), Brush::Place);
        // Centred on the cell, rounded towards the top-left; turned footprints too.
        let p = brush_placement(part("fusion_reactor"), (5, 5), 0, "steel");
        assert_eq!((p.x, p.y, p.material), (5, 5, None));
        let p = brush_placement(part("beam_laser"), (5, 5), 1, "steel");
        assert_eq!((p.x, p.y, p.rot), (5, 5, 1));
        let p = brush_placement(part("slope_1x3"), (5, 5), 1, "steel");
        assert_eq!((p.x, p.y, p.material.as_deref()), (5, 4, Some("steel")));
    }

    /// Delete with mirror on removes the selected part and its twin (drydock.md), not an unrelated part that covers
    /// its mirror cells.
    #[test]
    fn delete_takes_the_selected_part_and_its_twin_only() {
        use bevy::ecs::system::RunSystemOnce;
        let content = data::base_content();
        let empty = data::Blueprint { parts: Vec::new(), ..content.ships.get("halvex_runner").unwrap().clone() };
        let mut design = PlayerDesign::open(&content, &empty, (10, 10)).unwrap();
        let reactor = Placement { part: "fusion_reactor".into(), x: 1, y: 1, rot: 0, flip: false, material: None, power: None };
        design.edit(|d| {
            d.place(reactor).unwrap();
            d.paint((8, 1), "steel").unwrap();
            d.mirror = true;
        });
        let ui = DrydockUi::new(&content, &design, &data::career::Unlocks::all(&content));
        let mut world = World::new();
        world.init_resource::<Messages<DrydockAction>>();
        world.init_resource::<Time<Real>>();
        world.init_resource::<EditNote>();
        world.init_resource::<DrydockCursor>();
        world.init_resource::<DrydockGesture>();
        world.insert_resource(Selection { cell: Some((2, 2)) });
        world.insert_resource(design);
        world.insert_resource(ui);
        world.insert_resource(GameContent(content));
        world.write_message(DrydockAction::Delete);
        world.run_system_once(apply).unwrap();
        let design = world.resource::<PlayerDesign>();
        assert_eq!(design.dock().parts().iter().map(|p| p.part.as_str()).collect::<Vec<_>>(), ["armor_block"]);
        assert_eq!(world.resource::<Selection>().cell, None);
        assert_eq!(world.resource::<EditNote>().text.as_deref(), Some("MIRROR SKIPPED  ·  NO MIRROR TWIN"));
    }

    /// R (RotateCw) on a slope brush turns it from the orientation the hull gives it under the cursor to the next one
    /// in turning order, and Shift+R back; other brushes keep their quarter turns.
    #[test]
    fn a_slope_brush_turns_from_what_the_ghost_shows() {
        let content = data::base_content();
        let design = PlayerDesign::starter(&content, "halvex_runner");
        let mut ui = DrydockUi::new(&content, &design, &data::career::Unlocks::all(&content));
        ui.picked = Some("slope_1x1".into());
        let cell = (2, 2);
        let auto = design.dock().preview_slope("slope_1x1", cell, &ui.material, None).unwrap().placement;
        let next = design.dock().next_orientation("slope_1x1", (auto.rot, auto.flip), true).unwrap();
        let mut world = World::new();
        world.init_resource::<Messages<DrydockAction>>();
        world.init_resource::<Time<Real>>();
        world.init_resource::<EditNote>();
        world.init_resource::<DrydockGesture>();
        world.init_resource::<Selection>();
        world.insert_resource(DrydockCursor { cell: Some(cell) });
        world.insert_resource(ui);
        world.insert_resource(design);
        world.insert_resource(GameContent(content.clone()));
        let apply = world.register_system(apply);
        world.write_message(DrydockAction::RotateCw);
        world.run_system(apply).unwrap();
        assert_eq!(world.resource::<DrydockUi>().slope_turn, Some(next));
        world.write_message(DrydockAction::RotateCcw);
        world.run_system(apply).unwrap();
        let back = world.resource::<DrydockUi>().slope_turn.unwrap();
        assert_eq!(world.resource::<PlayerDesign>().dock().next_orientation("slope_1x1", back, true), Ok(next));
        // A machine's brush turns a quarter.
        world.resource_mut::<DrydockUi>().picked = Some("beam_laser".into());
        world.write_message(DrydockAction::RotateCw);
        world.run_system(apply).unwrap();
        assert_eq!(world.resource::<DrydockUi>().rot, 1);
    }

    /// The selection is a cell: after another design opens, or the frame changes (FRAME re-centres every part, and
    /// its undo does it back), that cell lies under another part or none, so the selection (and a gesture in
    /// progress) goes.
    #[test]
    fn the_selection_goes_with_another_design_or_frame() {
        let content = data::base_content();
        let design = PlayerDesign::starter(&content, "halvex_runner");
        let reactor = design.dock().parts().iter().find(|p| p.part == "fusion_reactor").map(|p| (p.x, p.y)).unwrap();
        let mut world = World::new();
        world.init_resource::<Messages<DrydockAction>>();
        world.init_resource::<Time<Real>>();
        world.init_resource::<EditNote>();
        world.init_resource::<DrydockCursor>();
        world.init_resource::<DrydockGesture>();
        world.init_resource::<Selection>();
        world.insert_resource(DrydockUi::new(&content, &design, &data::career::Unlocks::all(&content)));
        world.insert_resource(design);
        world.insert_resource(GameContent(content.clone()));
        let apply = world.register_system(apply);
        let select = |world: &mut World| {
            world.resource_mut::<Selection>().cell = Some(reactor);
            world.resource_mut::<DrydockGesture>().gesture = Gesture::Move { from: reactor, to: reactor };
        };
        world.run_system(apply).unwrap();
        select(&mut world);
        world.run_system(apply).unwrap();
        assert_eq!(world.resource::<Selection>().cell, Some(reactor));
        // OPEN: another design.
        world.resource_mut::<PlayerDesign>().replace(PlayerDesign::starter(&content, "halvex_runner"));
        world.run_system(apply).unwrap();
        assert_eq!((world.resource::<Selection>().cell, world.resource::<DrydockGesture>().gesture), (None, Gesture::Idle));
        // FRAME CRUISER, then its undo.
        select(&mut world);
        world.resource_mut::<PlayerDesign>().edit(|d| d.set_frame((40, 40))).unwrap();
        world.run_system(apply).unwrap();
        assert_eq!(world.resource::<Selection>().cell, None);
        select(&mut world);
        world.write_message(DrydockAction::Undo);
        world.run_system(apply).unwrap();
        assert_eq!(world.resource::<PlayerDesign>().frame(), (24, 24));
        assert_eq!(world.resource::<Selection>().cell, None);
    }

    /// T during a paint drag: the stroke ends with the drydock, and the next visit starts without a gesture.
    #[test]
    fn leaving_ends_a_gesture_in_progress() {
        use bevy::ecs::system::RunSystemOnce;
        let content = data::base_content();
        let mut design = PlayerDesign::starter(&content, "halvex_runner");
        design.edit(|d| {
            d.begin_stroke();
            d.paint((0, 0), "steel").unwrap();
        });
        let mut world = World::new();
        world.insert_resource(design);
        world.insert_resource(DrydockGesture { gesture: Gesture::Stroke { erase: false, last: (0, 0) }, button: Some(MouseButton::Left) });
        world.run_system_once(leave).unwrap();
        let g = world.resource::<DrydockGesture>();
        assert_eq!((g.gesture, g.button), (Gesture::Idle, None));
        // The stroke is one closed undo step: undoing it takes the painted blocks away.
        let mut design = world.resource_mut::<PlayerDesign>();
        let n = design.blueprint().parts.len();
        assert!(design.edit(|d| d.undo()));
        assert_eq!(design.blueprint().parts.len(), n - 2);
    }

    #[test]
    fn power_overrides_stay_clean() {
        let c = data::base_content();
        let laser = c.parts.iter().find(|p| p.power.as_ref().is_some_and(|s| s.draw > 0.0)).unwrap();
        let spec = laser.power.clone().unwrap();
        let mut p = brush_placement(laser, (0, 0), 0, "steel");
        assert_eq!(power_setting(&c, &p), Some((spec.tier, spec.weight)));
        let other = PowerTier::ALL.into_iter().find(|&t| t != spec.tier).unwrap();
        p.power = power_with(&c, &p, Some(other), None);
        assert_eq!(p.power, Some(PowerOverride { tier: Some(other), weight: None }));
        p.power = power_with(&c, &p, None, Some(spec.weight + 1.0));
        assert_eq!(power_setting(&c, &p), Some((other, spec.weight + 1.0)));
        // Back to the defaults: no override at all.
        p.power = power_with(&c, &p, Some(spec.tier), Some(spec.weight));
        assert_eq!(p.power, None);
        let core = brush_placement(c.parts.get("fission_reactor").unwrap(), (0, 0), 0, "steel");
        assert_eq!(power_setting(&c, &core), None);
    }
}
