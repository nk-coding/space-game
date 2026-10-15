//! The drydock's editing model: a ship design held in frame coordinates, with the drydock tools (place,
//! slope, paint, fill, erase, move, rotate, flip, material, power), mirror and bevel modes, strokes and
//! undo/redo. Placing a part or painting armour replaces what is under it; bevel cuts the outer corners of what a
//! stroke painted when the stroke ends. Engine-free and wasm-safe; the client's drydock and the arena's evolution operators both edit
//! designs through it. The rules are documented in `docs/drydock.md` ("Editing model").
//!
//! [`Drydock`]'s impl is split by responsibility: opening, queries and previews here, the edits (`edits.rs`), how
//! they are planned and carried out (`plans.rs`), the checks (`checks.rs`), the state and undo (`state.rs`).
//!
//! Coordinates follow [`crate::design::geometry`]: x right, y down, forward = −y, `rot` = quarter turns clockwise, flip
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
//! let mut dock = data::design::edit::Drydock::open(&content, runner, (24, 24)).unwrap();
//! dock.mirror = true;
//! dock.begin_stroke();
//! dock.paint((5, 16), "steel").unwrap();
//! dock.paint((5, 17), "steel").unwrap();
//! dock.end_stroke();
//! dock.undo(); // both cells (and their mirror twins) in one step
//! let blueprint = dock.export();
//! ```

mod checks;
mod edits;
mod plans;
mod state;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::career::Availability;
use crate::design::geometry::{
    Bounds, Cell, Dir, Footprint, PlacedShape, span, transform_point, transform_vector,
};
use crate::content::load::Content;
use crate::content::model::{Blueprint, MAX_POWER_WEIGHT, MAX_WEAPON_GROUP, Placement, PowerOverride, Shape};
use crate::content::validate::MAX_FRAME_SIZE;

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
    /// Only weapons have a weapon group (GDR-0088).
    NotWeapon,
    /// A weapon group is 1 to [`MAX_WEAPON_GROUP`].
    BadGroup,
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
    /// The design's bounding box (`w`, `h`, saturated at `u32::MAX`) is larger than the frame; for
    /// [`Drydock::open_placed`], the box from the frame's corner that its parts need.
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
    weapon: bool,
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
                        weapon: p.weapon.is_some(),
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
        Drydock::open_with(content, blueprint, frame, true)
    }

    /// Opens a blueprint whose placements are already in frame coordinates (as [`Drydock::parts`] gives them),
    /// keeping every part where it is: the evolution lab's genomes keep their cells from generation to generation,
    /// so its locks (cells, parts) never drift (GDR-0092). A part outside the frame gives
    /// [`OpenError::DoesNotFit`] with the size of the box from the frame's corner that the parts would need.
    pub fn open_placed(
        content: &Content,
        blueprint: &Blueprint,
        frame: (u32, u32),
    ) -> Result<Drydock, OpenError> {
        Drydock::open_with(content, blueprint, frame, false)
    }

    fn open_with(
        content: &Content,
        blueprint: &Blueprint,
        frame: (u32, u32),
        centre: bool,
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
        let side = |n: i64| u32::try_from(n).unwrap_or(u32::MAX);
        if centre {
            let shift = dock.centring(frame).ok_or_else(|| {
                let [x0, y0, x1, y1] = span(&dock.shapes).unwrap_or_default();
                OpenError::DoesNotFit {
                    design: (side(x1 - x0), side(y1 - y0)),
                }
            })?;
            dock.shift_parts(shift);
        } else if let Some([x0, y0, x1, y1]) = span(&dock.shapes) {
            let (fw, fh) = (frame.0 as i64, frame.1 as i64);
            if x0 < 0 || y0 < 0 || x1 > fw || y1 > fh {
                return Err(OpenError::DoesNotFit {
                    design: (side(x1.max(fw) - x0.min(0)), side(y1.max(fh) - y0.min(0))),
                });
            }
        }
        dock.frame = frame;
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
            group: group_of(p.group),
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
}

fn frame_dims((w, h): (u32, u32)) -> Option<(i32, i32)> {
    let ok = |n: u32| (1..=MAX_FRAME_SIZE).contains(&n);
    (ok(w) && ok(h)).then_some((w as i32, h as i32))
}

/// An override that sets nothing counts as none.
fn nonempty(power: Option<PowerOverride>) -> Option<PowerOverride> {
    power.filter(|o| o.tier.is_some() || o.weight.is_some())
}

/// Group 1 is the default: stored as none.
fn group_of(group: Option<u8>) -> Option<u8> {
    group.filter(|g| *g != 1)
}

fn group_ok(info: &PartInfo, group: u8) -> Result<(), Refusal> {
    if !info.weapon {
        return Err(Refusal::NotWeapon);
    }
    if !(1..=MAX_WEAPON_GROUP).contains(&group) {
        return Err(Refusal::BadGroup);
    }
    Ok(())
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
        group: None,
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
