//! The drydock's editing model: a ship design held in frame coordinates, with the drydock tools (place,
//! slope, paint, fill, erase, move, rotate, flip, material, power), mirror and bevel modes, strokes and
//! undo/redo. Placing a part or painting armour replaces what is under it; bevel cuts the outer corners of what a
//! stroke painted when the stroke ends. Engine-free and wasm-safe; the client's drydock and the arena's evolution operators both edit
//! designs through it. The rules are documented in `docs/drydock.md` ("Editing model").
//!
//! Coordinates follow [`crate::geometry`]: x right, y down, forward = −y, `rot` = quarter turns clockwise, flip
//! mirrors before rotating, a placement's `x, y` is the top-left cell of its rotated footprint.
//!
//! Part indices are indices into [`Drydock::parts`], which become the exported blueprint's `parts`. Erasing
//! shifts the indices after the erased part, and undo can put a part back in the middle, so an index is only
//! valid until the next change of [`Drydock::revision`]: keep cells (and [`Drydock::part_at`]) across edits,
//! not indices.
//!
//! ```no_run
//! let content = data::base_content();
//! let runner = content.ships.get("halvex_runner").unwrap();
//! let mut dock = data::edit::Drydock::open(&content, runner, (24, 24)).unwrap();
//! dock.mirror = true;
//! dock.begin_stroke();
//! dock.paint((5, 16), "steel").unwrap();
//! dock.paint((5, 17), "steel").unwrap();
//! dock.end_stroke();
//! dock.undo(); // both cells (and their mirror twins) in one step
//! let blueprint = dock.export();
//! ```

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::career::Availability;
use crate::geometry::{
    Bounds, Cell, Dir, Footprint, PlacedShape, span, transform_point, transform_vector,
};
use crate::load::Content;
use crate::model::{Blueprint, MAX_POWER_WEIGHT, Placement, PowerOverride, Shape};
use crate::validate::MAX_FRAME_SIZE;

/// Undo steps kept; older ones are dropped.
pub const UNDO_STEPS: usize = 256;

const NONE: u32 = u32::MAX;

/// Why an edit (or the mirror twin of an edit) is not possible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The footprint (or cell, or design) does not fit inside the frame.
    Outside,
    /// The footprint would overlap this part (index into [`Drydock::parts`]).
    Overlaps(usize),
    /// There is no part at this cell.
    Empty,
    /// There is no part with this index.
    NoSuchPart(usize),
    /// The content has no part with this id.
    UnknownPart(String),
    /// Not an armour material (unknown, or without an `armor` block).
    NotArmor(String),
    /// A hull part needs an armour material.
    NeedsMaterial,
    /// Only hull parts take a material.
    NotHull,
    /// Only parts that draw power take a power override.
    DrawsNoPower,
    /// A power weight must be above 0 and at most [`MAX_POWER_WEIGHT`].
    BadWeight,
    /// `rot` is a number of quarter turns, 0 to 3.
    BadRotation,
    /// [`Drydock::place_slope`] needs a slope part.
    NotSlope,
    /// No orientation of the slope fits at this cell.
    NoRoom,
    /// The content has no 1×1 hull block to paint or fill with.
    NoBlockPart,
    /// A frame side must be 1 to [`MAX_FRAME_SIZE`] cells.
    BadFrame,
    /// Mirror mode: the part has no mirror twin (no part of the same kind at its mirror image).
    NoTwin,
    /// None of this part is left: the design holds as many as the player has ([`Drydock::set_available`]).
    NoneLeft(String),
}

/// What happened to the mirror image of an edit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Twin {
    /// Mirror mode is off (or the edit has no mirror image, like a frame change).
    Off,
    /// The edit is its own mirror image (a part or cell on the axis), so it was applied once.
    Itself,
    /// The mirror image of the edit was applied as well.
    Applied,
    /// The mirror image was not possible, for this reason; the edit itself happened.
    Skipped(Refusal),
}

/// The result of a successful edit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// False when the edit changed nothing (painting a cell that already has the material, moving by 0, ...).
    /// Only changes bump [`Drydock::revision`] and make an undo step.
    pub changed: bool,
    /// Index of the placed or edited part after the edit (for the selection). `None` for erase, fill and
    /// frame changes.
    pub part: Option<usize>,
    pub twin: Twin,
}

/// What an edit would do, without doing it (the ghost under the cursor). The edit itself is planned by the same
/// code, so a preview and the edit agree.
#[derive(Clone, Debug, PartialEq)]
pub struct Preview {
    /// The placement the edit puts down or leaves: the new part, or the edited part as it would be.
    pub placement: Placement,
    /// What would happen to the mirror image, as [`Outcome::twin`] would say.
    pub twin: Twin,
    /// The mirror image's placement after the edit, when `twin` is [`Twin::Applied`].
    pub mirrored: Option<Placement>,
    /// The parts the edit replaces (indices into [`Drydock::parts`] before the edit, ascending): what a placed
    /// part's footprint, or its mirror image's, overlaps; the non-hull part under a paint.
    pub removed: Vec<usize>,
}

/// Why a blueprint cannot be opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenError {
    /// A frame side must be 1 to [`MAX_FRAME_SIZE`] cells.
    BadFrame,
    UnknownPart {
        index: usize,
        part: String,
    },
    BadRotation {
        index: usize,
    },
    /// Two parts share a cell.
    Overlap {
        index: usize,
        other: usize,
    },
    /// The design's bounding box (`w`, `h`, saturated at `u32::MAX`) is larger than the frame.
    DoesNotFit {
        design: (u32, u32),
    },
}

/// What the editor needs to know about a part.
#[derive(Clone, Debug)]
struct PartInfo {
    footprint: Footprint,
    hull: bool,
    draws_power: bool,
}

/// The content facts the editor uses, copied out of [`Content`] so the editor owns its data.
#[derive(Clone, Debug)]
struct Catalog {
    parts: BTreeMap<String, PartInfo>,
    armor: BTreeSet<String>,
    /// The paint and fill block: the first hull part with a 1×1 rect footprint (content order).
    block: Option<String>,
    /// The bevel slope: the first hull part with a 1×1 slope footprint.
    corner: Option<String>,
}

impl Catalog {
    fn new(c: &Content) -> Catalog {
        let unit = |shape: Shape| c.unit_hull_part(shape).map(|p| p.id.clone());
        Catalog {
            parts: c
                .parts
                .iter()
                .map(|p| {
                    let info = PartInfo {
                        footprint: p.footprint(),
                        hull: p.hull,
                        draws_power: p.draws_power(),
                    };
                    (p.id.clone(), info)
                })
                .collect(),
            armor: c
                .materials
                .iter()
                .filter(|m| m.armor.is_some())
                .map(|m| m.id.clone())
                .collect(),
            block: unit(Shape::Rect),
            corner: unit(Shape::Slope),
        }
    }
}

/// One primitive change; an undo step is a list of them in the order they were applied.
#[derive(Clone, Debug)]
enum Change {
    Insert {
        index: usize,
        placement: Placement,
    },
    Remove {
        index: usize,
        placement: Placement,
    },
    Set {
        index: usize,
        old: Placement,
        new: Placement,
    },
    /// The frame size changed and every part moved by `shift`.
    Frame {
        old: (i32, i32),
        new: (i32, i32),
        shift: (i64, i64),
    },
}

impl Change {
    fn inverse(&self) -> Change {
        match self {
            Change::Insert { index, placement } => Change::Remove {
                index: *index,
                placement: placement.clone(),
            },
            Change::Remove { index, placement } => Change::Insert {
                index: *index,
                placement: placement.clone(),
            },
            Change::Set { index, old, new } => Change::Set {
                index: *index,
                old: new.clone(),
                new: old.clone(),
            },
            Change::Frame { old, new, shift } => Change::Frame {
                old: *new,
                new: *old,
                shift: (-shift.0, -shift.1),
            },
        }
    }
}

/// What painting one cell does.
#[derive(Clone)]
enum PaintAction {
    /// The cell already has a hull part of this material.
    Nothing(usize),
    Recolor(usize),
    /// A block goes in (the cell is empty, or the part on it is replaced).
    Insert(Placement),
}

/// What [`Drydock::paint`] does: the cell, its mirror cell, what that means for the twin, and the non-hull parts
/// the blocks replace (ascending).
struct PaintPlan {
    first: PaintAction,
    second: Option<PaintAction>,
    twin: Twin,
    removed: Vec<usize>,
}

/// An open stroke: its changes (one undo step) and the cells its paints put a block in (bevelled when it ends).
#[derive(Clone, Debug, Default)]
struct Stroke {
    changes: Vec<Change>,
    painted: Vec<Cell>,
}

/// What [`Drydock::place_slope`] does.
enum SlopePlan {
    New(Placement),
    /// The clicked slope has another material: repaint it.
    Recolor(usize),
    /// The clicked slope has this material: turn it to this shape.
    Turn(usize, PlacedShape),
}

/// The drydock editor: one design in a frame of `w × h` cells.
#[derive(Clone, Debug)]
pub struct Drydock {
    /// Mirror mode: every edit also applies to its mirror image across the frame's vertical centre line.
    pub mirror: bool,
    /// Bevel mode: when a paint stroke (or a fill) ends, every outer corner among the cells it put a block in
    /// becomes the 1×1 slope.
    pub bevel: bool,
    catalog: Catalog,
    /// The opened blueprint without its parts (id, name, faction, ...), for [`Drydock::export`].
    header: Blueprint,
    frame: (i32, i32),
    parts: Vec<Placement>,
    shapes: Vec<PlacedShape>,
    /// Part index per frame cell, row-major (`NONE` = empty).
    occ: Vec<u32>,
    /// Straight-side bits ([`Dir::bit`]) of the part in each cell.
    sides: Vec<u8>,
    undo: VecDeque<Vec<Change>>,
    redo: Vec<Vec<Change>>,
    /// The open stroke, if any.
    stroke: Option<Stroke>,
    revision: u64,
    /// How many of a part the design may hold, for the parts with a count ([`Drydock::set_available`]).
    limits: BTreeMap<String, u32>,
}

impl Drydock {
    /// Opens a blueprint in a frame of `frame = (w, h)` cells, its parts' bounding box centred (rounded
    /// towards the top-left). The blueprint's other fields are kept for [`Drydock::export`], except `source`:
    /// the design comes from no pack. Pass a blueprint without parts to start a new design. Placements anywhere
    /// in the `i32` range are fine: parts too far apart for the frame give [`OpenError::DoesNotFit`].
    pub fn open(
        content: &Content,
        blueprint: &Blueprint,
        frame: (u32, u32),
    ) -> Result<Drydock, OpenError> {
        let frame = frame_dims(frame).ok_or(OpenError::BadFrame)?;
        let catalog = Catalog::new(content);
        let mut shapes = Vec::with_capacity(blueprint.parts.len());
        for (index, p) in blueprint.parts.iter().enumerate() {
            let info = catalog
                .parts
                .get(&p.part)
                .ok_or_else(|| OpenError::UnknownPart {
                    index,
                    part: p.part.clone(),
                })?;
            if p.rot > 3 {
                return Err(OpenError::BadRotation { index });
            }
            shapes.push(PlacedShape {
                footprint: info.footprint,
                x: p.x,
                y: p.y,
                rot: p.rot,
                flip: p.flip,
            });
        }
        let mut dock = Drydock {
            mirror: false,
            bevel: false,
            catalog,
            header: Blueprint {
                parts: Vec::new(),
                source: Default::default(),
                ..blueprint.clone()
            },
            frame: (0, 0),
            parts: blueprint.parts.clone(),
            shapes,
            occ: Vec::new(),
            sides: Vec::new(),
            undo: VecDeque::new(),
            redo: Vec::new(),
            stroke: None,
            revision: 0,
            limits: BTreeMap::new(),
        };
        let shift = dock.centring(frame).ok_or_else(|| {
            let [x0, y0, x1, y1] = span(&dock.shapes).unwrap_or_default();
            let side = |n: i64| u32::try_from(n).unwrap_or(u32::MAX);
            OpenError::DoesNotFit {
                design: (side(x1 - x0), side(y1 - y0)),
            }
        })?;
        dock.frame = frame;
        dock.shift_parts(shift);
        dock.rebuild()
            .map_err(|(index, other)| OpenError::Overlap { index, other })?;
        Ok(dock)
    }

    /// The design as a blueprint: trimmed to the parts' bounding box (`size` = its size, parts shifted so the
    /// smallest `x` and `y` are 0), in part index order, every other field as opened. An empty design exports
    /// with `size` (0, 0).
    pub fn export(&self) -> Blueprint {
        let (size, (dx, dy)) = match self.bounds() {
            Some(b) => ((b.width() as u32, b.height() as u32), (-b.min_x, -b.min_y)),
            None => ((0, 0), (0, 0)),
        };
        Blueprint {
            size,
            parts: self
                .parts
                .iter()
                .map(|p| Placement {
                    x: p.x + dx,
                    y: p.y + dy,
                    ..p.clone()
                })
                .collect(),
            ..self.header.clone()
        }
    }

    /// The frame `(w, h)` in cells.
    pub fn frame(&self) -> (u32, u32) {
        (self.frame.0 as u32, self.frame.1 as u32)
    }

    /// Changes the frame and centres the design in it, as one undoable step. Refused with
    /// [`Refusal::Outside`] if the design does not fit (the frame stays as it is).
    pub fn set_frame(&mut self, frame: (u32, u32)) -> Result<Outcome, Refusal> {
        let new = frame_dims(frame).ok_or(Refusal::BadFrame)?;
        let shift = self.centring(new).ok_or(Refusal::Outside)?;
        self.end_stroke();
        let mut log = Vec::new();
        if new != self.frame || shift != (0, 0) {
            let old = self.frame;
            self.exec(Change::Frame { old, new, shift }, &mut log);
        }
        Ok(self.finish(log, None, Twin::Off))
    }

    /// The design's header: the opened blueprint without its parts (id, name, summary, faction, behavior, ...).
    /// Its `size` and `parts` are not used; [`Drydock::export`] takes everything else from it.
    pub fn header(&self) -> &Blueprint {
        &self.header
    }

    /// The header to rename or re-describe the design. Not an edit: no undo step, no revision change.
    pub fn header_mut(&mut self) -> &mut Blueprint {
        &mut self.header
    }

    /// The parts in frame coordinates; their order is the exported blueprint's.
    pub fn parts(&self) -> &[Placement] {
        &self.parts
    }

    /// The geometry of every part, same indices as [`Drydock::parts`].
    pub fn shapes(&self) -> &[PlacedShape] {
        &self.shapes
    }

    /// The part covering a cell.
    pub fn part_at(&self, cell: Cell) -> Option<usize> {
        self.slot(cell)
            .map(|k| self.occ[k])
            .filter(|&p| p != NONE)
            .map(|p| p as usize)
    }

    /// The parts' bounding box in frame coordinates, `None` for an empty design.
    pub fn bounds(&self) -> Option<Bounds> {
        // Every part lies inside the frame, so the box fits an i32.
        span(&self.shapes).map(|[min_x, min_y, max_x, max_y]| Bounds {
            min_x: min_x as i32,
            min_y: min_y as i32,
            max_x: max_x as i32,
            max_y: max_y as i32,
        })
    }

    /// Goes up by one with every change of the design (edits, undo, redo, frame changes); the client caches
    /// visuals and reports per revision. Starts at 0 for each opened design.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Is this placement legal on free cells? Checks the part, rotation, material and power override, then that
    /// the footprint lies inside the frame and overlaps no part (the first overlapped part in the footprint's
    /// cell order is named). [`Drydock::place`] replaces the parts under the footprint instead.
    pub fn check(&self, p: &Placement) -> Result<(), Refusal> {
        let s = self.validate(p)?;
        self.fits(&s, &[])?;
        self.in_stock(&p.part, 1)
    }

    /// The mirror image of a placement across the frame's vertical centre line: cells reflected
    /// (`x' = W − x − w`, with `w` the rotated width), flip toggled, `rot' = (4 − rot) % 4`. `p` must name a part
    /// of the content the drydock was opened with (as every placement of the palette does).
    pub fn mirror_of(&self, p: &Placement) -> Placement {
        let s = self.shape_of(p);
        with_pose(p, &mirrored(&s, self.frame.0))
    }

    /// The mirror image of a cell: `(W − 1 − x, y)`.
    pub fn mirror_cell(&self, (x, y): Cell) -> Cell {
        ((self.frame.0 - 1).saturating_sub(x), y)
    }

    /// What [`Drydock::place`] would do: the placement (an empty power override dropped), its mirror image and
    /// the parts they replace.
    pub fn preview_place(&self, p: &Placement) -> Result<Preview, Refusal> {
        let p = Placement {
            power: nonempty(p.power),
            ..p.clone()
        };
        let s = self.validate(&p)?;
        self.inside(&s)?;
        self.in_stock(&p.part, 1)?;
        Ok(self.new_part_plan(p, &s))
    }

    /// What [`Drydock::place_slope`] would do at this cell: the new, repainted or turned slope, its mirror image
    /// and the parts a new one replaces.
    pub fn preview_slope(
        &self,
        part: &str,
        cell: Cell,
        material: &str,
        turn: Option<(u8, bool)>,
    ) -> Result<Preview, Refusal> {
        match self.slope_plan(part, cell, material, turn)? {
            SlopePlan::New(p) => {
                let s = self.shape_of(&p);
                Ok(self.new_part_plan(p, &s))
            }
            SlopePlan::Recolor(k) => Ok(self.recolor_plan(k, material)?.0),
            SlopePlan::Turn(k, s) => Ok(self.turn_plan(k, s)?.0),
        }
    }

    /// What a click of [`Drydock::paint`] would do at this cell (a stroke of one cell): the block (in bevel mode
    /// the slope, where the finished shape has an outer corner) or repainted hull part left there, at the mirror
    /// cell, and the parts the blocks replace.
    pub fn preview_paint(&self, cell: Cell, material: &str) -> Result<Preview, Refusal> {
        let plan = self.paint_plan(cell, material)?;
        let inserted: Vec<Cell> = [Some(&plan.first), plan.second.as_ref()]
            .into_iter()
            .flatten()
            .filter_map(|a| match a {
                PaintAction::Insert(p) => Some((p.x, p.y)),
                _ => None,
            })
            .collect();
        let left = |a: &PaintAction| {
            let p = self.painted(a, material);
            match (a, self.bevel, &self.catalog.corner) {
                (PaintAction::Insert(_), true, Some(slope)) => self
                    .corner_rot_with((p.x, p.y), &inserted, &plan.removed)
                    .map_or_else(
                        || p.clone(),
                        |rot| Placement {
                            part: slope.clone(),
                            rot,
                            ..p.clone()
                        },
                    ),
                _ => p,
            }
        };
        Ok(Preview {
            placement: left(&plan.first),
            mirrored: plan.second.as_ref().map(left),
            twin: plan.twin,
            removed: plan.removed,
        })
    }

    /// What ending the open stroke would do in bevel mode: the slopes the outer corners among the cells it
    /// painted become. Empty without bevel or an open stroke.
    pub fn preview_stroke_end(&self) -> Vec<Placement> {
        match (&self.stroke, self.bevel) {
            (Some(stroke), true) => self
                .bevel_plan(&stroke.painted)
                .into_iter()
                .map(|(_, p)| p)
                .collect(),
            _ => Vec::new(),
        }
    }

    /// A slope's orientations `(rot, flip)` in turning order (rot 0..3 unflipped, then rot 0..3 flipped, without
    /// repeats: a 1×1 slope has 4, the longer ones 8).
    pub fn slope_orientations(&self, part: &str) -> Result<Vec<(u8, bool)>, Refusal> {
        let info = self
            .catalog
            .parts
            .get(part)
            .ok_or_else(|| Refusal::UnknownPart(part.to_string()))?;
        match info.footprint.shape {
            Shape::Slope => Ok(orientations(info.footprint)),
            Shape::Rect => Err(Refusal::NotSlope),
        }
    }

    /// The orientation after (`forward`) or before `from` in the slope's turning order. `from` may be any
    /// `(rot, flip)` with that geometry (it is matched by shape, not by number).
    pub fn next_orientation(
        &self,
        part: &str,
        from: (u8, bool),
        forward: bool,
    ) -> Result<(u8, bool), Refusal> {
        let all = self.slope_orientations(part)?;
        let probe = PlacedShape {
            footprint: self.catalog.parts[part].footprint,
            x: 0,
            y: 0,
            rot: from.0 % 4,
            flip: from.1,
        };
        let at = all
            .iter()
            .position(|&(r, f)| same_orientation(&probe, r, f))
            .unwrap_or(0);
        let n = all.len();
        Ok(all[(at + if forward { 1 } else { n - 1 }) % n])
    }

    /// What [`Drydock::move_part`] would do: the part at its new place, and its twin's.
    pub fn preview_move(&self, i: usize, dx: i32, dy: i32) -> Result<Preview, Refusal> {
        Ok(self.part_plan(i, move_by(dx, dy))?.0)
    }

    /// Places any part (appended to the parts), replacing every part its footprint overlaps, as one step. A hull
    /// part needs an armour material, other parts none; a power override only goes on parts that draw power (an
    /// empty override is dropped); the footprint must lie inside the frame. Mirror mode also places the mirror
    /// image (replacing what it overlaps), unless it is the same shape (a symmetric part centred on the axis) or
    /// would overlap the placed part.
    pub fn place(&mut self, p: Placement) -> Result<Outcome, Refusal> {
        let plan = self.preview_place(&p)?;
        Ok(self.apply_new_part(plan))
    }

    /// The slope tool. On a slope of the same part it repaints it if the material differs, else turns it to its
    /// next orientation that fits. Anywhere else it places the slope, replacing what its footprint overlaps:
    /// oriented to the hull (`turn` = `None`, see `docs/drydock.md`), or in the orientation `turn` =
    /// `(rot, flip)`, centred on the cell. Mirror mode places, repaints or turns the mirror image too.
    pub fn place_slope(
        &mut self,
        part: &str,
        cell: Cell,
        material: &str,
        turn: Option<(u8, bool)>,
    ) -> Result<Outcome, Refusal> {
        match self.slope_plan(part, cell, material, turn)? {
            SlopePlan::New(p) => {
                let s = self.shape_of(&p);
                let plan = self.new_part_plan(p, &s);
                Ok(self.apply_new_part(plan))
            }
            SlopePlan::Recolor(k) => self.set_material(k, material),
            SlopePlan::Turn(k, s) => {
                let plan = self.turn_plan(k, s)?;
                Ok(self.apply_part_plan(k, plan))
            }
        }
    }

    /// The armour paint tool: an empty cell gets the 1×1 block, a hull part under the cursor takes the material,
    /// any other part is replaced by the block. Mirror mode paints the mirror cell too, decided on the design as
    /// it was before this call, so a symmetric design stays symmetric. Bevel mode cuts the outer corners among the
    /// painted cells when the stroke ends (at once outside a stroke: a click is a stroke of one cell).
    pub fn paint(&mut self, cell: Cell, material: &str) -> Result<Outcome, Refusal> {
        let plan = self.paint_plan(cell, material)?;
        let mut log = Vec::new();
        for a in [Some(&plan.first), plan.second.as_ref()].into_iter().flatten() {
            if let PaintAction::Recolor(k) = a {
                self.recolor(*k, material, &mut log);
            }
        }
        for &index in plan.removed.iter().rev() {
            let placement = self.parts[index].clone();
            self.exec(Change::Remove { index, placement }, &mut log);
        }
        let mut inserted = Vec::new();
        for a in [Some(plan.first), plan.second].into_iter().flatten() {
            if let PaintAction::Insert(placement) = a {
                inserted.push((placement.x, placement.y));
                let index = self.parts.len();
                self.exec(Change::Insert { index, placement }, &mut log);
            }
        }
        self.blocks_in(&inserted, &mut log);
        let part = self.part_at(cell);
        Ok(self.finish(log, part, plan.twin))
    }

    /// The fill tool: every empty cell of the rectangle spanned by two corner cells (clipped to the frame) gets
    /// the 1×1 block in this material, row by row, as one step. Mirror mode fills the mirrored rectangle too. In
    /// bevel mode the outer corners among the filled cells become slopes, in the same step.
    pub fn fill(&mut self, a: Cell, b: Cell, material: &str) -> Result<Outcome, Refusal> {
        self.armor(material)?;
        let block = self.catalog.block.clone().ok_or(Refusal::NoBlockPart)?;
        self.in_stock(&block, 1)?;
        let (x0, x1) = (a.0.min(b.0).max(0), a.0.max(b.0).min(self.frame.0 - 1));
        let (y0, y1) = (a.1.min(b.1).max(0), a.1.max(b.1).min(self.frame.1 - 1));
        if x0 > x1 || y0 > y1 {
            return Err(Refusal::Outside);
        }
        let mut log = Vec::new();
        let mut filled = Vec::new();
        let mut fill = |dock: &mut Self, xs| dock.fill_rect(xs, (y0, y1), &block, material, &mut log, &mut filled);
        fill(self, (x0, x1));
        let twin = if !self.mirror {
            Twin::Off
        } else {
            let (m0, m1) = (self.frame.0 - 1 - x1, self.frame.0 - 1 - x0);
            if (m0, m1) == (x0, x1) {
                Twin::Itself
            } else {
                fill(self, (m0, m1));
                Twin::Applied
            }
        };
        self.blocks_in(&filled, &mut log);
        Ok(self.finish(log, None, twin))
    }

    /// The erase tool: removes the part covering the cell (later parts move down one index). Mirror mode also
    /// removes the part covering the mirror cell.
    pub fn erase(&mut self, cell: Cell) -> Result<Outcome, Refusal> {
        if self.slot(cell).is_none() {
            return Err(Refusal::Outside);
        }
        let k = self.part_at(cell).ok_or(Refusal::Empty)?;
        let (twin, other) = match self.mirror {
            false => (Twin::Off, None),
            true => match self.part_at(self.mirror_cell(cell)) {
                None => (Twin::Skipped(Refusal::Empty), None),
                Some(j) if j == k => (Twin::Itself, None),
                Some(j) => (Twin::Applied, Some(j)),
            },
        };
        let mut log = Vec::new();
        let mut doomed = [Some(k), other];
        doomed.sort_unstable_by(|a, b| b.cmp(a));
        for index in doomed.into_iter().flatten() {
            let placement = self.parts[index].clone();
            self.exec(Change::Remove { index, placement }, &mut log);
        }
        Ok(self.finish(log, None, twin))
    }

    /// The part tool of erase (Delete on a selected part): removes part `i` (later parts move down one index).
    /// Mirror mode also removes its twin, the part that is its mirror image; whatever else covers the mirror cells
    /// stays (`Twin::Skipped(NoTwin)` when it has no twin), unlike [`Drydock::erase`] on a cell.
    pub fn remove_part(&mut self, i: usize) -> Result<Outcome, Refusal> {
        if i >= self.parts.len() {
            return Err(Refusal::NoSuchPart(i));
        }
        let (twin, other) = match self.mirror.then(|| self.twin(i)) {
            None => (Twin::Off, None),
            Some(None) => (Twin::Skipped(Refusal::NoTwin), None),
            Some(Some(j)) if j == i => (Twin::Itself, None),
            Some(Some(j)) => (Twin::Applied, Some(j)),
        };
        let mut log = Vec::new();
        let mut doomed = [Some(i), other];
        doomed.sort_unstable_by(|a, b| b.cmp(a));
        for index in doomed.into_iter().flatten() {
            let placement = self.parts[index].clone();
            self.exec(Change::Remove { index, placement }, &mut log);
        }
        Ok(self.finish(log, None, twin))
    }

    /// Moves a part by `(dx, dy)` cells if it still fits (its own old cells do not count). The mirror twin
    /// moves by `(−dx, dy)`.
    pub fn move_part(&mut self, i: usize, dx: i32, dy: i32) -> Result<Outcome, Refusal> {
        let plan = self.part_plan(i, move_by(dx, dy))?;
        Ok(self.apply_part_plan(i, plan))
    }

    /// Turns a part a quarter turn if it still fits. It turns about a fixed point of the part: its centre when
    /// the footprint's `w + h` is even, else the centre's neighbour half a cell towards the part's front (so
    /// the centre moves as little as possible, and turning back restores the placement exactly). The mirror
    /// twin turns the other way.
    pub fn rotate(&mut self, i: usize, clockwise: bool) -> Result<Outcome, Refusal> {
        let plan = self.part_plan(i, |old, s, twin| {
            Ok(with_pose(old, &turned(s, clockwise != twin)))
        })?;
        Ok(self.apply_part_plan(i, plan))
    }

    /// Mirrors a part left-right in place (same cells). The mirror twin flips too.
    pub fn flip(&mut self, i: usize) -> Result<Outcome, Refusal> {
        let plan = self.part_plan(i, |old, s, _| Ok(with_pose(old, &flipped(s))))?;
        Ok(self.apply_part_plan(i, plan))
    }

    /// Sets a hull part's armour material (and its mirror twin's).
    pub fn set_material(&mut self, i: usize, material: &str) -> Result<Outcome, Refusal> {
        let plan = self.recolor_plan(i, material)?;
        Ok(self.apply_part_plan(i, plan))
    }

    /// Sets or clears a part's power override (and its mirror twin's). Only parts that draw power take one; an
    /// empty override counts as none.
    pub fn set_power(
        &mut self,
        i: usize,
        power: Option<PowerOverride>,
    ) -> Result<Outcome, Refusal> {
        let part = self.parts.get(i).ok_or(Refusal::NoSuchPart(i))?;
        let power = nonempty(power);
        if let Some(o) = &power {
            power_ok(&self.catalog.parts[&part.part], o)?;
        }
        let plan = self.part_plan(i, |old, _, _| {
            Ok(Placement {
                power,
                ..old.clone()
            })
        })?;
        Ok(self.apply_part_plan(i, plan))
    }

    /// Opens a stroke: every edit until [`Drydock::end_stroke`] becomes one undo step (a paint or erase drag).
    /// An open stroke is closed first; undo, redo and frame changes close it too.
    pub fn begin_stroke(&mut self) {
        self.end_stroke();
        self.stroke = Some(Stroke::default());
    }

    /// Closes the open stroke, if any. In bevel mode the outer corners among the cells it painted become slopes
    /// first (in the same undo step), decided on the finished shape, so the result does not depend on the order
    /// the cells were painted in.
    pub fn end_stroke(&mut self) {
        let Some(Stroke {
            mut changes,
            painted,
        }) = self.stroke.take()
        else {
            return;
        };
        if self.bevel {
            let mut log = Vec::new();
            self.bevel_cells(&painted, &mut log);
            if !log.is_empty() {
                self.revision += 1;
                changes.extend(log);
            }
        }
        if !changes.is_empty() {
            self.push_undo(changes);
        }
    }

    /// Undoes the last step (closing an open stroke first). False if there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        self.end_stroke();
        let Some(step) = self.undo.pop_back() else {
            return false;
        };
        for c in step.iter().rev() {
            self.apply(&c.inverse());
        }
        self.redo.push(step);
        self.revision += 1;
        true
    }

    /// Redoes the last undone step. False if there is nothing to redo (any new edit clears the redo list).
    pub fn redo(&mut self) -> bool {
        self.end_stroke();
        let Some(step) = self.redo.pop() else {
            return false;
        };
        for c in &step {
            self.apply(c);
        }
        self.push_undo(step);
        self.revision += 1;
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.stroke.as_ref().is_some_and(|s| !s.changes.is_empty())
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// A stroke is open (a paint or erase drag): the design may change again next frame.
    pub fn in_stroke(&self) -> bool {
        self.stroke.is_some()
    }

    /// How many of `part` the player has to build with (`career::Career::available`, GDR-0082): the design may
    /// hold that many, and an edit that would put down more is refused with [`Refusal::NoneLeft`] (a mirror
    /// image that would is skipped; fill fills as many cells as are left). Not an edit: no undo step, no revision
    /// change; parts already placed stay, and undo and redo always restore their step.
    pub fn set_available(&mut self, part: &str, available: Availability) {
        match available {
            Availability::Unlimited => self.limits.remove(part),
            Availability::Count(n) => self.limits.insert(part.to_string(), n),
        };
    }

    /// How many more of `part` the design may take: the count of [`Drydock::set_available`] less the parts of it
    /// placed (0 when the design holds more), or unlimited.
    pub fn left(&self, part: &str) -> Availability {
        match self.limits.get(part) {
            None => Availability::Unlimited,
            Some(&n) => {
                let used = self.parts.iter().filter(|p| p.part == part).count();
                Availability::Count(n.saturating_sub(u32::try_from(used).unwrap_or(u32::MAX)))
            }
        }
    }

    // ---- edits, internals ----

    /// Plans replacing part `i` by `make(placement, shape, false)` if it fits (its own old cells do not count),
    /// and in mirror mode its twin `j` by `make(twin placement, twin shape, true)` if that fits beside the new
    /// part `i`. Returns the plan and `j` when the twin edit applies.
    fn part_plan(
        &self,
        i: usize,
        make: impl Fn(&Placement, &PlacedShape, bool) -> Result<Placement, Refusal>,
    ) -> Result<(Preview, Option<usize>), Refusal> {
        let old = self.parts.get(i).ok_or(Refusal::NoSuchPart(i))?;
        let new = make(old, &self.shapes[i], false)?;
        if new.rot > 3 {
            return Err(Refusal::BadRotation);
        }
        let s = self.shape_of(&new);
        self.fits(&s, &[i])?;
        let (twin, image) = match self.mirror.then(|| self.twin(i)) {
            None => (Twin::Off, None),
            Some(None) => (Twin::Skipped(Refusal::NoTwin), None),
            Some(Some(j)) if j == i => (Twin::Itself, None),
            Some(Some(j)) => {
                let new = make(&self.parts[j], &self.shapes[j], true).and_then(|n| {
                    self.fits_beside(&self.shape_of(&n), &[j, i], Some((&s, i)))
                        .map(|()| n)
                });
                match new {
                    Ok(n) => (Twin::Applied, Some((j, n))),
                    Err(r) => (Twin::Skipped(r), None),
                }
            }
        };
        let (j, mirrored) = image.unzip();
        let plan = Preview {
            placement: new,
            twin,
            mirrored,
            removed: Vec::new(),
        };
        Ok((plan, j))
    }

    /// Carries out a [`Drydock::part_plan`] of part `i`.
    fn apply_part_plan(&mut self, i: usize, (plan, j): (Preview, Option<usize>)) -> Outcome {
        let mut log = Vec::new();
        for (k, new) in std::iter::once((i, plan.placement)).chain(j.zip(plan.mirrored)) {
            let old = self.parts[k].clone();
            if new != old {
                self.exec(Change::Set { index: k, old, new }, &mut log);
            }
        }
        self.finish(log, Some(i), plan.twin)
    }

    /// The plan of [`Drydock::set_material`].
    fn recolor_plan(&self, i: usize, material: &str) -> Result<(Preview, Option<usize>), Refusal> {
        self.armor(material)?;
        let part = self.parts.get(i).ok_or(Refusal::NoSuchPart(i))?;
        if !self.catalog.parts[&part.part].hull {
            return Err(Refusal::NotHull);
        }
        self.part_plan(i, |old, _, _| {
            Ok(Placement {
                material: Some(material.to_string()),
                ..old.clone()
            })
        })
    }

    /// The plan of turning slope `k` to the shape `s` (its twin to the mirror image).
    fn turn_plan(&self, k: usize, s: PlacedShape) -> Result<(Preview, Option<usize>), Refusal> {
        let m = mirrored(&s, self.frame.0);
        self.part_plan(k, |old, _, twin| {
            Ok(with_pose(old, if twin { &m } else { &s }))
        })
    }

    /// The plan of putting down a new part `p` with the shape `s` (inside the frame): it replaces every part its
    /// footprint overlaps. In mirror mode also its mirror image, unless that is the same shape, replacing what it
    /// overlaps, if it keeps clear of `p` (else `Skipped(Overlaps(k))`, `k` the index `p` gets).
    fn new_part_plan(&self, p: Placement, s: &PlacedShape) -> Preview {
        let mut removed = self.under(s);
        let (twin, image) = if !self.mirror {
            (Twin::Off, None)
        } else {
            let m = mirrored(s, self.frame.0);
            let cells = s.cells();
            if same_shape(&m, s) {
                (Twin::Itself, None)
            } else if m.cells().iter().any(|c| cells.contains(c)) {
                let k = self.parts.len() - removed.len();
                (Twin::Skipped(Refusal::Overlaps(k)), None)
            } else {
                match self.in_stock(&p.part, 2) {
                    Ok(()) => {
                        removed.extend(self.under(&m));
                        removed.sort_unstable();
                        removed.dedup();
                        (Twin::Applied, Some(with_pose(&p, &m)))
                    }
                    Err(r) => (Twin::Skipped(r), None),
                }
            }
        };
        // The very same parts already there: nothing is replaced, nothing changes.
        let news = [Some(&p), image.as_ref()];
        let count = news.iter().flatten().count();
        if removed.len() == count && removed.iter().all(|&r| news.contains(&Some(&self.parts[r]))) {
            removed.clear();
        }
        Preview {
            placement: p,
            twin,
            mirrored: image,
            removed,
        }
    }

    /// Carries out a [`Drydock::new_part_plan`]: removes the replaced parts, then appends the part and its mirror
    /// image. A plan that replaces nothing on an occupied footprint puts down what is already there: no change.
    fn apply_new_part(&mut self, plan: Preview) -> Outcome {
        if plan.removed.is_empty()
            && let Some(&k) = self.under(&self.shape_of(&plan.placement)).first()
        {
            return self.finish(Vec::new(), Some(k), plan.twin);
        }
        let mut log = Vec::new();
        for &index in plan.removed.iter().rev() {
            let placement = self.parts[index].clone();
            self.exec(Change::Remove { index, placement }, &mut log);
        }
        let index = self.parts.len();
        let news = std::iter::once(plan.placement).chain(plan.mirrored);
        for (k, placement) in news.enumerate() {
            let at = index + k;
            self.exec(Change::Insert { index: at, placement }, &mut log);
        }
        self.finish(log, Some(index), plan.twin)
    }

    /// The parts a shape overlaps, ascending.
    fn under(&self, s: &PlacedShape) -> Vec<usize> {
        let mut out: Vec<usize> = s.cells().into_iter().filter_map(|c| self.part_at(c)).collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// The plan of [`Drydock::paint`]: both cells are decided on the design as it is. A non-hull part covering
    /// both cells is replaced once, by a block in each.
    fn paint_plan(&self, cell: Cell, material: &str) -> Result<PaintPlan, Refusal> {
        self.armor(material)?;
        let (first, gone) = self.paint_action(cell, material)?;
        let first_part = match first {
            PaintAction::Nothing(k) | PaintAction::Recolor(k) => Some(k),
            PaintAction::Insert(_) => None,
        };
        let mut removed: Vec<usize> = gone.into_iter().collect();
        let mirror_cell = self.mirror_cell(cell);
        let (second, twin) = if !self.mirror {
            (None, Twin::Off)
        } else if mirror_cell == cell {
            (None, Twin::Itself)
        } else {
            match self.paint_action(mirror_cell, material) {
                Err(r) => (None, Twin::Skipped(r)),
                Ok((PaintAction::Nothing(j) | PaintAction::Recolor(j), _)) if Some(j) == first_part => {
                    (None, Twin::Itself)
                }
                Ok((a, gone)) => match self.pair_in_stock(&first, &a) {
                    Ok(()) => {
                        removed.extend(gone);
                        (Some(a), Twin::Applied)
                    }
                    Err(r) => (None, Twin::Skipped(r)),
                },
            }
        };
        removed.sort_unstable();
        removed.dedup();
        Ok(PaintPlan {
            first,
            second,
            twin,
            removed,
        })
    }

    /// What a planned paint leaves in its cell.
    fn painted(&self, a: &PaintAction, material: &str) -> Placement {
        match a {
            PaintAction::Nothing(k) => self.parts[*k].clone(),
            PaintAction::Recolor(k) => Placement {
                material: Some(material.to_string()),
                ..self.parts[*k].clone()
            },
            PaintAction::Insert(p) => p.clone(),
        }
    }

    fn recolor(&mut self, k: usize, material: &str, log: &mut Vec<Change>) {
        let old = self.parts[k].clone();
        let new = Placement {
            material: Some(material.to_string()),
            ..old.clone()
        };
        self.exec(Change::Set { index: k, old, new }, log);
    }

    fn fill_rect(
        &mut self,
        (x0, x1): (i32, i32),
        (y0, y1): (i32, i32),
        block: &str,
        material: &str,
        log: &mut Vec<Change>,
        filled: &mut Vec<Cell>,
    ) {
        let mut left = self.left(block);
        for y in y0..=y1 {
            for x in x0..=x1 {
                if self.part_at((x, y)).is_none() {
                    // Fills as many cells as blocks are left.
                    if let Availability::Count(n) = &mut left {
                        let Some(k) = n.checked_sub(1) else { return };
                        *n = k;
                    }
                    let placement = unit_placement(block, (x, y), 0, material);
                    let index = self.parts.len();
                    self.exec(Change::Insert { index, placement }, log);
                    filled.push((x, y));
                }
            }
        }
    }

    /// What painting one cell does, and the non-hull part it replaces.
    fn paint_action(
        &self,
        cell: Cell,
        material: &str,
    ) -> Result<(PaintAction, Option<usize>), Refusal> {
        if self.slot(cell).is_none() {
            return Err(Refusal::Outside);
        }
        let under = self.part_at(cell);
        if let Some(k) = under
            && self.catalog.parts[&self.parts[k].part].hull
        {
            return Ok(match self.parts[k].material.as_deref() == Some(material) {
                true => (PaintAction::Nothing(k), None),
                false => (PaintAction::Recolor(k), None),
            });
        }
        let block = self.catalog.block.as_ref().ok_or(Refusal::NoBlockPart)?;
        self.in_stock(block, 1)?;
        let placement = unit_placement(block, cell, 0, material);
        Ok((PaintAction::Insert(placement), under))
    }

    /// Blocks went into these cells: an open stroke keeps them for its end, else bevel mode cuts their corners
    /// now (a single edit is a stroke of its own).
    fn blocks_in(&mut self, cells: &[Cell], log: &mut Vec<Change>) {
        match &mut self.stroke {
            Some(stroke) => stroke.painted.extend_from_slice(cells),
            None if self.bevel => self.bevel_cells(cells, log),
            None => {}
        }
    }

    /// Bevel: which of these cells hold a 1×1 block on an outer corner of the design as it is, and the slope each
    /// becomes (same material, oriented by [`Drydock::corner_rot_with`]). All are decided before any change.
    fn bevel_plan(&self, cells: &[Cell]) -> Vec<(usize, Placement)> {
        let (Some(block), Some(slope)) = (&self.catalog.block, &self.catalog.corner) else {
            return Vec::new();
        };
        let mut out: Vec<(usize, Placement)> = Vec::new();
        for &cell in cells {
            let Some(k) = self.part_at(cell) else { continue };
            let p = &self.parts[k];
            if p.part != *block || out.iter().any(|(j, _)| *j == k) {
                continue;
            }
            if let Some(rot) = self.corner_rot_with(cell, &[], &[]) {
                let slope = Placement {
                    part: slope.clone(),
                    rot,
                    flip: false,
                    ..p.clone()
                };
                out.push((k, slope));
            }
        }
        out
    }

    /// Turns the blocks of [`Drydock::bevel_plan`] into their slopes.
    fn bevel_cells(&mut self, cells: &[Cell], log: &mut Vec<Change>) {
        for (index, new) in self.bevel_plan(cells) {
            // A corner stays a block when no slope is left.
            if self.in_stock(&new.part, 1).is_err() {
                continue;
            }
            let old = self.parts[index].clone();
            self.exec(Change::Set { index, old, new }, log);
        }
    }

    /// Bevel: if `cell` is an outer corner (two adjacent sides touch a part's straight side, the other two are
    /// open), the `rot` of the 1×1 slope (unflipped) whose straight sides are the touching sides. Only the
    /// neighbours count, as the design would be with blocks in the cells `extra` and without the parts `gone`.
    fn corner_rot_with(&self, cell: Cell, extra: &[Cell], gone: &[usize]) -> Option<u8> {
        let (mut touch, mut open) = (0u8, 0u8);
        for d in Dir::ALL {
            let n = step(cell, d);
            if extra.contains(&n) {
                touch |= d.bit();
                continue;
            }
            match self.part_at(n).filter(|k| !gone.contains(k)) {
                None => open |= d.bit(),
                Some(_) if self.straight(n, d.opposite()) => touch |= d.bit(),
                Some(_) => {}
            }
        }
        if touch.count_ones() != 2 || touch | open != 0b1111 {
            return None;
        }
        (0..4).find(|&r| {
            Dir::Left.transform(r, false).bit() | Dir::Down.transform(r, false).bit() == touch
        })
    }

    fn slope_plan(
        &self,
        part: &str,
        cell: Cell,
        material: &str,
        turn: Option<(u8, bool)>,
    ) -> Result<SlopePlan, Refusal> {
        let info = self
            .catalog
            .parts
            .get(part)
            .ok_or_else(|| Refusal::UnknownPart(part.to_string()))?;
        if info.footprint.shape != Shape::Slope {
            return Err(Refusal::NotSlope);
        }
        let fp = info.footprint;
        let new = unit_placement(part, cell, 0, material);
        self.validate(&new)?;
        if self.slot(cell).is_none() {
            return Err(Refusal::Outside);
        }
        let orientations = orientations(fp);
        if let Some(k) = self.part_at(cell).filter(|&k| self.parts[k].part == part) {
            if self.parts[k].material.as_deref() != Some(material) {
                return Ok(SlopePlan::Recolor(k));
            }
            let current = self.shapes[k];
            let at = orientations
                .iter()
                .position(|&(rot, flip)| same_orientation(&current, rot, flip))
                .unwrap_or(0);
            let n = orientations.len();
            return (1..n)
                .map(|t| orientations[(at + t) % n])
                .filter_map(|(rot, flip)| self.anchored(fp, cell, rot, flip))
                .find(|s| self.fits(s, &[k]).is_ok())
                .map(|s| SlopePlan::Turn(k, s))
                .ok_or(Refusal::NoRoom);
        }
        if let Some((rot, flip)) = turn {
            if rot > 3 {
                return Err(Refusal::BadRotation);
            }
            let s = self.anchored(fp, cell, rot, flip).ok_or(Refusal::NoRoom)?;
            return Ok(SlopePlan::New(with_pose(&new, &s)));
        }
        // Fewest replaced parts first (none where it fits), then the hull rules on the design without them.
        // `max_by_key` keeps the last of equal maxima; reversed, ties go to the first in turning order.
        let best = orientations
            .iter()
            .filter_map(|&(rot, flip)| self.anchored(fp, cell, rot, flip))
            .rev()
            .max_by_key(|s| {
                let gone = self.under(s);
                let (links, blocked) = self.slope_contacts(s, &gone);
                let fewest = std::cmp::Reverse(gone.len());
                (fewest, links, std::cmp::Reverse(blocked), self.lean(s))
            })
            .ok_or(Refusal::NoRoom)?;
        self.in_stock(part, 1)?;
        Ok(SlopePlan::New(with_pose(&new, &best)))
    }

    /// `(links, blocked)` of a slope candidate: its straight unit sides that meet a neighbour's straight side,
    /// and the occupied cells in front of its sloped face; the parts `gone` (which it replaces) do not count.
    fn slope_contacts(&self, s: &PlacedShape, gone: &[usize]) -> (u32, u32) {
        let cells = s.cells();
        let sides = s.connectable_sides();
        let (mut links, mut blocked) = (0, 0);
        for &c in &cells {
            for d in Dir::ALL {
                let n = step(c, d);
                if cells.contains(&n) || self.part_at(n).is_none_or(|k| gone.contains(&k)) {
                    continue;
                }
                if !sides.contains(&(c, d)) {
                    blocked += 1;
                } else if self.straight(n, d.opposite()) {
                    links += 1;
                }
            }
        }
        (links, blocked)
    }

    /// Tie-break of slope orientations: how much the face normal points away from the frame's centre line
    /// (weight 2) and forward (weight 1), the concept board's preference.
    fn lean(&self, s: &PlacedShape) -> i32 {
        let (nx, ny) =
            transform_vector(s.footprint.h as f32, -(s.footprint.w as f32), s.rot, s.flip);
        let side = (2 * s.x + s.dims().0 as i32 - self.frame.0).signum();
        2 * nx as i32 * side - ny as i32
    }

    /// A footprint in an orientation, centred on `cell` (rounded towards the top-left) and pushed inside the
    /// frame; `None` if it is larger than the frame.
    fn anchored(&self, fp: Footprint, (cx, cy): Cell, rot: u8, flip: bool) -> Option<PlacedShape> {
        let (w, h) = fp.dims(rot);
        let (w, h) = (w as i32, h as i32);
        if w > self.frame.0 || h > self.frame.1 {
            return None;
        }
        Some(PlacedShape {
            footprint: fp,
            x: (cx - (w - 1) / 2).clamp(0, self.frame.0 - w),
            y: (cy - (h - 1) / 2).clamp(0, self.frame.1 - h),
            rot,
            flip,
        })
    }

    /// The part that is the mirror image of part `i` (same part id, same shape as `i` mirrored); `i` itself
    /// for a part that mirrors onto itself.
    fn twin(&self, i: usize) -> Option<usize> {
        let m = mirrored(&self.shapes[i], self.frame.0);
        let j = self.part_at((m.x, m.y))?;
        (self.parts[j].part == self.parts[i].part && same_shape(&self.shapes[j], &m)).then_some(j)
    }

    // ---- validation ----

    /// [`Refusal::NoneLeft`] unless `more` more of `part` are left.
    fn in_stock(&self, part: &str, more: u32) -> Result<(), Refusal> {
        match self.left(part) {
            Availability::Count(n) if n < more => Err(Refusal::NoneLeft(part.to_string())),
            _ => Ok(()),
        }
    }

    /// Painting the mirror cell as well: a second new part of the same kind as the first needs two left.
    fn pair_in_stock(&self, first: &PaintAction, second: &PaintAction) -> Result<(), Refusal> {
        match (first, second) {
            (PaintAction::Insert(a), PaintAction::Insert(b)) if a.part == b.part => self.in_stock(&b.part, 2),
            _ => Ok(()),
        }
    }

    fn armor(&self, material: &str) -> Result<(), Refusal> {
        match self.catalog.armor.contains(material) {
            true => Ok(()),
            false => Err(Refusal::NotArmor(material.to_string())),
        }
    }

    fn validate(&self, p: &Placement) -> Result<PlacedShape, Refusal> {
        let info = self
            .catalog
            .parts
            .get(&p.part)
            .ok_or_else(|| Refusal::UnknownPart(p.part.clone()))?;
        if p.rot > 3 {
            return Err(Refusal::BadRotation);
        }
        match (&p.material, info.hull) {
            (None, true) => return Err(Refusal::NeedsMaterial),
            (Some(m), true) => self.armor(m)?,
            (Some(_), false) => return Err(Refusal::NotHull),
            (None, false) => {}
        }
        if let Some(o) = &nonempty(p.power) {
            power_ok(info, o)?;
        }
        Ok(self.shape_of(p))
    }

    /// Inside the frame.
    fn inside(&self, s: &PlacedShape) -> Result<(), Refusal> {
        let (w, h) = s.dims();
        let (x, y) = (s.x as i64, s.y as i64);
        match x >= 0 && y >= 0 && x + w as i64 <= self.frame.0 as i64 && y + h as i64 <= self.frame.1 as i64 {
            true => Ok(()),
            false => Err(Refusal::Outside),
        }
    }

    /// Inside the frame and on free cells (parts in `ignore` do not count).
    fn fits(&self, s: &PlacedShape, ignore: &[usize]) -> Result<(), Refusal> {
        self.fits_beside(s, ignore, None)
    }

    /// [`Drydock::fits`], and clear of `other`: the shape part `k` is about to have (the first edit of a pair,
    /// for its mirror image). The first overlap in the shape's cell order is named.
    fn fits_beside(
        &self,
        s: &PlacedShape,
        ignore: &[usize],
        other: Option<(&PlacedShape, usize)>,
    ) -> Result<(), Refusal> {
        self.inside(s)?;
        let other = other.map(|(o, k)| (o.cells(), k));
        for c in s.cells() {
            if let Some((cells, k)) = &other
                && cells.contains(&c)
            {
                return Err(Refusal::Overlaps(*k));
            }
            if let Some(k) = self.part_at(c)
                && !ignore.contains(&k)
            {
                return Err(Refusal::Overlaps(k));
            }
        }
        Ok(())
    }

    fn shape_of(&self, p: &Placement) -> PlacedShape {
        PlacedShape {
            footprint: self.catalog.parts[&p.part].footprint,
            x: p.x,
            y: p.y,
            rot: p.rot,
            flip: p.flip,
        }
    }

    // ---- state, undo ----

    fn exec(&mut self, change: Change, log: &mut Vec<Change>) {
        self.apply(&change);
        log.push(change);
    }

    /// Records the applied changes of one edit (into the open stroke, or as a new undo step).
    fn finish(&mut self, log: Vec<Change>, part: Option<usize>, twin: Twin) -> Outcome {
        let changed = !log.is_empty();
        if changed {
            self.revision += 1;
            self.redo.clear();
            match &mut self.stroke {
                Some(stroke) => stroke.changes.extend(log),
                None => self.push_undo(log),
            }
        }
        Outcome {
            changed,
            part,
            twin,
        }
    }

    fn push_undo(&mut self, step: Vec<Change>) {
        if self.undo.len() == UNDO_STEPS {
            self.undo.pop_front();
        }
        self.undo.push_back(step);
    }

    fn apply(&mut self, change: &Change) {
        match change {
            Change::Insert { index, placement } => {
                let i = *index;
                if i < self.parts.len() {
                    self.renumber(i, true);
                }
                self.shapes.insert(i, self.shape_of(placement));
                self.parts.insert(i, placement.clone());
                self.stamp(i);
            }
            Change::Remove { index, .. } => {
                let i = *index;
                self.unstamp(i);
                self.parts.remove(i);
                self.shapes.remove(i);
                if i < self.parts.len() {
                    self.renumber(i + 1, false);
                }
            }
            Change::Set { index, new, .. } => {
                let i = *index;
                self.unstamp(i);
                self.shapes[i] = self.shape_of(new);
                self.parts[i] = new.clone();
                self.stamp(i);
            }
            Change::Frame { new, shift, .. } => {
                self.frame = *new;
                self.shift_parts(*shift);
                self.rebuild()
                    .expect("a frame change keeps the parts apart");
            }
        }
    }

    /// The shift that centres the design's bounding box in `frame`; `None` if it does not fit. In `i64`: a
    /// blueprint being opened may lie anywhere in the `i32` range.
    fn centring(&self, (fw, fh): (i32, i32)) -> Option<(i64, i64)> {
        let Some([x0, y0, x1, y1]) = span(&self.shapes) else {
            return Some((0, 0));
        };
        let (w, h, fw, fh) = (x1 - x0, y1 - y0, fw as i64, fh as i64);
        if w > fw || h > fh {
            return None;
        }
        Some(((fw - w) / 2 - x0, (fh - h) / 2 - y0))
    }

    /// Moves every part by a [`Drydock::centring`] shift, which puts them all inside the frame.
    fn shift_parts(&mut self, (dx, dy): (i64, i64)) {
        let to = |v: i32, d: i64| (v as i64 + d) as i32;
        for (p, s) in self.parts.iter_mut().zip(&mut self.shapes) {
            (p.x, p.y) = (to(p.x, dx), to(p.y, dy));
            (s.x, s.y) = (to(s.x, dx), to(s.y, dy));
        }
    }

    /// Rebuilds the occupancy grid; `Err((part, other))` if two parts share a cell.
    fn rebuild(&mut self) -> Result<(), (usize, usize)> {
        let n = (self.frame.0 * self.frame.1) as usize;
        self.occ = vec![NONE; n];
        self.sides = vec![0; n];
        for i in 0..self.shapes.len() {
            if let Some(k) = self.shapes[i]
                .cells()
                .into_iter()
                .find_map(|c| self.part_at(c))
            {
                return Err((i, k));
            }
            self.stamp(i);
        }
        Ok(())
    }

    fn slot(&self, (x, y): Cell) -> Option<usize> {
        let (w, h) = self.frame;
        (x >= 0 && y >= 0 && x < w && y < h).then(|| (y * w + x) as usize)
    }

    /// Does the part in `cell` have a straight side on `dir`?
    fn straight(&self, cell: Cell, dir: Dir) -> bool {
        self.slot(cell)
            .is_some_and(|k| self.occ[k] != NONE && self.sides[k] & dir.bit() != 0)
    }

    fn stamp(&mut self, i: usize) {
        let s = self.shapes[i];
        for c in s.cells() {
            let k = self.slot(c).expect("parts stay inside the frame");
            self.occ[k] = i as u32;
            self.sides[k] = 0;
        }
        for (c, d) in s.connectable_sides() {
            let k = self.slot(c).expect("parts stay inside the frame");
            self.sides[k] |= d.bit();
        }
    }

    fn unstamp(&mut self, i: usize) {
        for c in self.shapes[i].cells() {
            let k = self.slot(c).expect("parts stay inside the frame");
            self.occ[k] = NONE;
            self.sides[k] = 0;
        }
    }

    /// Moves every grid index `>= from` up or down by one (a part inserted or removed before them).
    fn renumber(&mut self, from: usize, up: bool) {
        let from = from as u32;
        for o in &mut self.occ {
            if *o != NONE && *o >= from {
                *o = if up { *o + 1 } else { *o - 1 };
            }
        }
    }
}

fn frame_dims((w, h): (u32, u32)) -> Option<(i32, i32)> {
    let ok = |n: u32| (1..=MAX_FRAME_SIZE).contains(&n);
    (ok(w) && ok(h)).then_some((w as i32, h as i32))
}

/// An override that sets nothing counts as none.
fn nonempty(power: Option<PowerOverride>) -> Option<PowerOverride> {
    power.filter(|o| o.tier.is_some() || o.weight.is_some())
}

fn power_ok(info: &PartInfo, o: &PowerOverride) -> Result<(), Refusal> {
    if !info.draws_power {
        return Err(Refusal::DrawsNoPower);
    }
    match o.weight {
        Some(w) if !(w.is_finite() && w > 0.0 && w <= MAX_POWER_WEIGHT) => Err(Refusal::BadWeight),
        _ => Ok(()),
    }
}

fn step((x, y): Cell, d: Dir) -> Cell {
    let (dx, dy) = d.delta();
    (x + dx, y + dy)
}

/// A 1×1 hull placement at `cell`.
fn unit_placement(part: &str, (x, y): Cell, rot: u8, material: &str) -> Placement {
    Placement {
        part: part.to_string(),
        x,
        y,
        rot,
        flip: false,
        material: Some(material.to_string()),
        power: None,
    }
}

/// The edit of [`Drydock::move_part`]: the twin moves the other way in x. A move past the `i32` range is
/// outside any frame.
fn move_by(
    dx: i32,
    dy: i32,
) -> impl Fn(&Placement, &PlacedShape, bool) -> Result<Placement, Refusal> {
    move |old, _, twin| {
        let dx = if twin { dx.checked_neg() } else { Some(dx) };
        Ok(Placement {
            x: dx.and_then(|dx| old.x.checked_add(dx)).ok_or(Refusal::Outside)?,
            y: old.y.checked_add(dy).ok_or(Refusal::Outside)?,
            ..old.clone()
        })
    }
}

/// `p` with the position, rotation and flip of `s`.
fn with_pose(p: &Placement, s: &PlacedShape) -> Placement {
    Placement {
        x: s.x,
        y: s.y,
        rot: s.rot,
        flip: s.flip,
        ..p.clone()
    }
}

/// The mirror image across the vertical centre line of a frame `width` cells wide.
fn mirrored(s: &PlacedShape, width: i32) -> PlacedShape {
    // Saturating: a placement far outside the frame stays far outside, it never wraps into it.
    PlacedShape {
        x: width.saturating_sub(s.x).saturating_sub_unsigned(s.dims().0),
        ..flipped(s)
    }
}

/// Mirrored left-right in place: the same cells, the image reflected about the footprint's vertical centre.
fn flipped(s: &PlacedShape) -> PlacedShape {
    PlacedShape {
        rot: (4 - s.rot % 4) % 4,
        flip: !s.flip,
        ..*s
    }
}

/// A quarter turn about the part's pivot: local point `(w/2, h/2)` if `w + h` is even (the centre), else
/// `(w/2, (h − 1)/2)` (half a cell from the centre towards the local front, −y). The pivot is a cell corner or
/// cell centre, so the turned footprint stays on the grid; it is fixed in the part's own frame, so turning back
/// restores the placement exactly and turning commutes with mirroring (the mirror image's pivot is the mirror
/// image of the pivot, and it turns the other way).
fn turned(s: &PlacedShape, clockwise: bool) -> PlacedShape {
    let (w, h) = (s.footprint.w as i32, s.footprint.h as i32);
    // Doubled coordinates, so half cells are integers.
    let q = if (w + h) % 2 == 0 { (w, h) } else { (w, h - 1) };
    let rot = (s.rot + if clockwise { 1 } else { 3 }) % 4;
    let (ax, ay) = transform_point(q.0, q.1, 2 * w, 2 * h, s.rot, s.flip);
    let (bx, by) = transform_point(q.0, q.1, 2 * w, 2 * h, rot, s.flip);
    debug_assert!((ax - bx) % 2 == 0 && (ay - by) % 2 == 0);
    PlacedShape {
        x: s.x + (ax - bx) / 2,
        y: s.y + (ay - by) / 2,
        rot,
        ..*s
    }
}

/// Do two placements of one footprint have the same geometry? Rects: same cells and facing (the flip only
/// mirrors the icon). Slopes: same cells and the right angle in the same corner.
fn same_shape(a: &PlacedShape, b: &PlacedShape) -> bool {
    a.footprint == b.footprint && (a.x, a.y) == (b.x, b.y) && same_orientation(a, b.rot, b.flip)
}

/// Does `s` have the orientation `(rot, flip)` (same dims, and for slopes the right angle in the same corner)?
fn same_orientation(s: &PlacedShape, rot: u8, flip: bool) -> bool {
    let fp = s.footprint;
    match fp.shape {
        Shape::Rect => s.rot % 4 == rot % 4,
        Shape::Slope => {
            let corner =
                |rot, flip| transform_point(0, fp.h as i32, fp.w as i32, fp.h as i32, rot, flip);
            fp.dims(s.rot) == fp.dims(rot) && corner(s.rot, s.flip) == corner(rot, flip)
        }
    }
}

/// The distinct orientations of a footprint in turning order: rot 0..3 unflipped, then rot 0..3 flipped,
/// without repeats (a 1×1 slope has 4).
fn orientations(fp: Footprint) -> Vec<(u8, bool)> {
    let mut out: Vec<(u8, bool)> = Vec::new();
    for flip in [false, true] {
        for rot in 0..4 {
            let s = PlacedShape {
                footprint: fp,
                x: 0,
                y: 0,
                rot,
                flip,
            };
            if !out.iter().any(|&(r, f)| same_orientation(&s, r, f)) {
                out.push((rot, flip));
            }
        }
    }
    out
}
