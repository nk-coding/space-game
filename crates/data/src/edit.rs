//! The drydock's editing model: a ship design held in frame coordinates, with the drydock tools (place,
//! slope, paint, fill, erase, move, rotate, flip, material, power), mirror and bevel modes, strokes and
//! undo/redo. Engine-free and wasm-safe; the client's drydock and the arena's evolution operators both edit
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

use crate::geometry::{
    Bounds, Cell, Dir, Footprint, PlacedShape, transform_point, transform_vector,
};
use crate::load::Content;
use crate::model::{Blueprint, MAX_POWER_WEIGHT, Placement, PowerOverride, Shape};

/// Undo steps kept; older ones are dropped.
pub const UNDO_STEPS: usize = 256;

/// Largest frame side in cells.
pub const MAX_FRAME: u32 = 1024;

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
    /// A frame side must be 1 to [`MAX_FRAME`] cells.
    BadFrame,
    /// Mirror mode: the part has no mirror twin (no part of the same kind at its mirror image).
    NoTwin,
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

/// Why a blueprint cannot be opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenError {
    /// A frame side must be 1 to [`MAX_FRAME`] cells.
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
    /// The design's bounding box (`w`, `h`) is larger than the frame.
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
        let unit = |shape: Shape| {
            c.parts
                .iter()
                .find(|p| p.hull && p.size == (1, 1) && p.shape == shape)
                .map(|p| p.id.clone())
        };
        Catalog {
            parts: c
                .parts
                .iter()
                .map(|p| {
                    let info = PartInfo {
                        footprint: p.footprint(),
                        hull: p.hull,
                        draws_power: p.power.as_ref().is_some_and(|x| x.draw > 0.0),
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
        shift: (i32, i32),
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
enum PaintAction {
    /// The cell already has a hull part of this material.
    Nothing(usize),
    Recolor(usize),
    Insert(Placement),
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
    /// Bevel mode: painting an outer corner of the hull places a 1×1 slope instead of a block.
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
    /// The open stroke's changes (one undo step), if a stroke is open.
    stroke: Option<Vec<Change>>,
    revision: u64,
}

impl Drydock {
    /// Opens a blueprint in a frame of `frame = (w, h)` cells, its parts' bounding box centred (rounded
    /// towards the top-left). The blueprint's other fields are kept for [`Drydock::export`]. Pass a blueprint
    /// without parts to start a new design.
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
        };
        let shift = dock.centring(frame).ok_or_else(|| {
            let b = dock.bounds().unwrap();
            OpenError::DoesNotFit {
                design: (b.width() as u32, b.height() as u32),
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
        self.shapes.iter().fold(None, |b, s| {
            let (w, h) = s.dims();
            let (ax, ay, bx, by) = (s.x, s.y, s.x + w as i32, s.y + h as i32);
            Some(match b {
                None => Bounds {
                    min_x: ax,
                    min_y: ay,
                    max_x: bx,
                    max_y: by,
                },
                Some(o) => Bounds {
                    min_x: o.min_x.min(ax),
                    min_y: o.min_y.min(ay),
                    max_x: o.max_x.max(bx),
                    max_y: o.max_y.max(by),
                },
            })
        })
    }

    /// Goes up by one with every change of the design (edits, undo, redo, frame changes); the client caches
    /// visuals and reports per revision. Starts at 0 for each opened design.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Could [`Drydock::place`] place this? Checks the part, rotation, material and power override, then that
    /// the footprint lies inside the frame and overlaps no part (the first overlapped part in the footprint's
    /// cell order is named).
    pub fn check(&self, p: &Placement) -> Result<(), Refusal> {
        let s = self.validate(p)?;
        self.fits(&s, &[])
    }

    /// The mirror image of a placement across the frame's vertical centre line: cells reflected
    /// (`x' = W − x − w`, with `w` the rotated width), flip toggled, `rot' = (4 − rot) % 4`.
    pub fn mirror_of(&self, p: &Placement) -> Placement {
        let s = self.shape_of(p);
        with_pose(p, &mirrored(&s, self.frame.0))
    }

    /// The mirror image of a cell: `(W − 1 − x, y)`.
    pub fn mirror_cell(&self, (x, y): Cell) -> Cell {
        (self.frame.0 - 1 - x, y)
    }

    /// What [`Drydock::place_slope`] would leave at this cell: the new, repainted or turned slope (for the
    /// ghost under the cursor).
    pub fn slope_preview(
        &self,
        part: &str,
        cell: Cell,
        material: &str,
    ) -> Result<Placement, Refusal> {
        Ok(match self.slope_plan(part, cell, material)? {
            SlopePlan::New(p) => p,
            SlopePlan::Recolor(k) => Placement {
                material: Some(material.to_string()),
                ..self.parts[k].clone()
            },
            SlopePlan::Turn(k, s) => with_pose(&self.parts[k], &s),
        })
    }

    /// Places any part (appended to the parts). A hull part needs an armour material, other parts none; a
    /// power override only goes on parts that draw power (an empty override is dropped). Mirror mode also
    /// places the mirror image, unless it is the same shape (a symmetric part centred on the axis).
    pub fn place(&mut self, p: Placement) -> Result<Outcome, Refusal> {
        let p = Placement {
            power: nonempty(p.power),
            ..p
        };
        let s = self.validate(&p)?;
        self.fits(&s, &[])?;
        let mut log = Vec::new();
        let index = self.parts.len();
        let twin = if self.mirror {
            let m = mirrored(&s, self.frame.0);
            self.exec(
                Change::Insert {
                    index,
                    placement: p.clone(),
                },
                &mut log,
            );
            if same_shape(&m, &s) {
                Twin::Itself
            } else {
                match self.fits(&m, &[]) {
                    Ok(()) => {
                        let placement = with_pose(&p, &m);
                        self.exec(
                            Change::Insert {
                                index: index + 1,
                                placement,
                            },
                            &mut log,
                        );
                        Twin::Applied
                    }
                    Err(r) => Twin::Skipped(r),
                }
            }
        } else {
            self.exec(
                Change::Insert {
                    index,
                    placement: p,
                },
                &mut log,
            );
            Twin::Off
        };
        Ok(self.finish(log, Some(index), twin))
    }

    /// The slope tool. On an empty cell it places the slope oriented to the hull (see `docs/drydock.md`). On a
    /// slope of the same part it repaints it if the material differs, else turns it to its next orientation
    /// that fits. Mirror mode places, repaints or turns the mirror image too.
    pub fn place_slope(
        &mut self,
        part: &str,
        cell: Cell,
        material: &str,
    ) -> Result<Outcome, Refusal> {
        match self.slope_plan(part, cell, material)? {
            SlopePlan::New(p) => self.place(p),
            SlopePlan::Recolor(k) => self.set_material(k, material),
            SlopePlan::Turn(k, s) => {
                let m = mirrored(&s, self.frame.0);
                self.edit_part(k, |old, _, twin| {
                    Ok(with_pose(old, if twin { &m } else { &s }))
                })
            }
        }
    }

    /// The armour paint tool: an empty cell gets the 1×1 block (in bevel mode an outer corner gets the 1×1
    /// slope, oriented to the hull); a hull part under the cursor takes the material. Refused on other
    /// parts. Mirror mode paints the mirror cell too, decided on the design as it was before this call, so a
    /// symmetric design stays symmetric.
    pub fn paint(&mut self, cell: Cell, material: &str) -> Result<Outcome, Refusal> {
        self.armor(material)?;
        let first = self.paint_action(cell, material)?;
        let mirror_cell = self.mirror_cell(cell);
        let second = match self.mirror {
            false => None,
            true if mirror_cell == cell => None,
            true => Some(self.paint_action(mirror_cell, material)),
        };
        let mut log = Vec::new();
        let (part, first_part) = match first {
            PaintAction::Nothing(k) => (k, Some(k)),
            PaintAction::Recolor(k) => {
                self.recolor(k, material, &mut log);
                (k, Some(k))
            }
            PaintAction::Insert(p) => {
                let index = self.parts.len();
                self.exec(
                    Change::Insert {
                        index,
                        placement: p,
                    },
                    &mut log,
                );
                (index, None)
            }
        };
        let twin = match (self.mirror, second) {
            (false, _) => Twin::Off,
            (true, None) => Twin::Itself,
            (true, Some(Err(r))) => Twin::Skipped(r),
            (true, Some(Ok(PaintAction::Nothing(j) | PaintAction::Recolor(j))))
                if Some(j) == first_part =>
            {
                Twin::Itself
            }
            (true, Some(Ok(PaintAction::Nothing(_)))) => Twin::Applied,
            (true, Some(Ok(PaintAction::Recolor(j)))) => {
                self.recolor(j, material, &mut log);
                Twin::Applied
            }
            (true, Some(Ok(PaintAction::Insert(p)))) => {
                let index = self.parts.len();
                self.exec(
                    Change::Insert {
                        index,
                        placement: p,
                    },
                    &mut log,
                );
                Twin::Applied
            }
        };
        Ok(self.finish(log, Some(part), twin))
    }

    /// The fill tool: every empty cell of the rectangle spanned by two corner cells (clipped to the frame) gets
    /// the 1×1 block in this material, row by row, as one step. Mirror mode fills the mirrored rectangle too.
    pub fn fill(&mut self, a: Cell, b: Cell, material: &str) -> Result<Outcome, Refusal> {
        self.armor(material)?;
        let block = self.catalog.block.clone().ok_or(Refusal::NoBlockPart)?;
        let (x0, x1) = (a.0.min(b.0).max(0), a.0.max(b.0).min(self.frame.0 - 1));
        let (y0, y1) = (a.1.min(b.1).max(0), a.1.max(b.1).min(self.frame.1 - 1));
        if x0 > x1 || y0 > y1 {
            return Err(Refusal::Outside);
        }
        let mut log = Vec::new();
        self.fill_rect((x0, x1), (y0, y1), &block, material, &mut log);
        let twin = if !self.mirror {
            Twin::Off
        } else {
            let (m0, m1) = (self.frame.0 - 1 - x1, self.frame.0 - 1 - x0);
            if (m0, m1) == (x0, x1) {
                Twin::Itself
            } else {
                self.fill_rect((m0, m1), (y0, y1), &block, material, &mut log);
                Twin::Applied
            }
        };
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

    /// Moves a part by `(dx, dy)` cells if it still fits (its own old cells do not count). The mirror twin
    /// moves by `(−dx, dy)`.
    pub fn move_part(&mut self, i: usize, dx: i32, dy: i32) -> Result<Outcome, Refusal> {
        self.edit_part(i, |old, _, twin| {
            Ok(Placement {
                x: old.x + if twin { -dx } else { dx },
                y: old.y + dy,
                ..old.clone()
            })
        })
    }

    /// Turns a part a quarter turn if it still fits. It turns about a fixed point of the part: its centre when
    /// the footprint's `w + h` is even, else the centre's neighbour half a cell towards the part's front (so
    /// the centre moves as little as possible, and turning back restores the placement exactly). The mirror
    /// twin turns the other way.
    pub fn rotate(&mut self, i: usize, clockwise: bool) -> Result<Outcome, Refusal> {
        self.edit_part(i, |old, s, twin| {
            Ok(with_pose(old, &turned(s, clockwise != twin)))
        })
    }

    /// Mirrors a part left-right in place (same cells). The mirror twin flips too.
    pub fn flip(&mut self, i: usize) -> Result<Outcome, Refusal> {
        self.edit_part(i, |old, s, _| Ok(with_pose(old, &flipped(s))))
    }

    /// Sets a hull part's armour material (and its mirror twin's).
    pub fn set_material(&mut self, i: usize, material: &str) -> Result<Outcome, Refusal> {
        self.armor(material)?;
        let part = self.parts.get(i).ok_or(Refusal::NoSuchPart(i))?;
        if !self.catalog.parts[&part.part].hull {
            return Err(Refusal::NotHull);
        }
        self.edit_part(i, |old, _, _| {
            Ok(Placement {
                material: Some(material.to_string()),
                ..old.clone()
            })
        })
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
        self.edit_part(i, |old, _, _| {
            Ok(Placement {
                power,
                ..old.clone()
            })
        })
    }

    /// Opens a stroke: every edit until [`Drydock::end_stroke`] becomes one undo step (a paint or erase drag).
    /// An open stroke is closed first; undo, redo and frame changes close it too.
    pub fn begin_stroke(&mut self) {
        self.end_stroke();
        self.stroke = Some(Vec::new());
    }

    /// Closes the open stroke, if any.
    pub fn end_stroke(&mut self) {
        if let Some(step) = self.stroke.take()
            && !step.is_empty()
        {
            self.push_undo(step);
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
        !self.undo.is_empty() || self.stroke.as_ref().is_some_and(|s| !s.is_empty())
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    // ---- edits, internals ----

    /// Replaces part `i` by `make(placement, shape, false)` if it fits, and in mirror mode its twin by
    /// `make(twin placement, twin shape, true)`.
    fn edit_part(
        &mut self,
        i: usize,
        make: impl Fn(&Placement, &PlacedShape, bool) -> Result<Placement, Refusal>,
    ) -> Result<Outcome, Refusal> {
        let old = self.parts.get(i).ok_or(Refusal::NoSuchPart(i))?.clone();
        let new = make(&old, &self.shapes[i], false)?;
        if new.rot > 3 {
            return Err(Refusal::BadRotation);
        }
        self.fits(&self.shape_of(&new), &[i])?;
        let twin_of = self.mirror.then(|| self.twin(i));
        let mut log = Vec::new();
        if new != old {
            self.exec(Change::Set { index: i, old, new }, &mut log);
        }
        let twin = match twin_of {
            None => Twin::Off,
            Some(None) => Twin::Skipped(Refusal::NoTwin),
            Some(Some(j)) if j == i => Twin::Itself,
            Some(Some(j)) => {
                let old = self.parts[j].clone();
                let new = make(&old, &self.shapes[j], true)
                    .and_then(|n| self.fits(&self.shape_of(&n), &[j]).map(|()| n));
                match new {
                    Ok(new) => {
                        if new != old {
                            self.exec(Change::Set { index: j, old, new }, &mut log);
                        }
                        Twin::Applied
                    }
                    Err(r) => Twin::Skipped(r),
                }
            }
        };
        Ok(self.finish(log, Some(i), twin))
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
    ) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                if self.part_at((x, y)).is_none() {
                    let placement = unit_placement(block, (x, y), 0, material);
                    let index = self.parts.len();
                    self.exec(Change::Insert { index, placement }, log);
                }
            }
        }
    }

    fn paint_action(&self, cell: Cell, material: &str) -> Result<PaintAction, Refusal> {
        if self.slot(cell).is_none() {
            return Err(Refusal::Outside);
        }
        if let Some(k) = self.part_at(cell) {
            return match self.catalog.parts[&self.parts[k].part].hull {
                false => Err(Refusal::Overlaps(k)),
                true if self.parts[k].material.as_deref() == Some(material) => {
                    Ok(PaintAction::Nothing(k))
                }
                true => Ok(PaintAction::Recolor(k)),
            };
        }
        let corner = match (&self.catalog.corner, self.bevel) {
            (Some(slope), true) => self.corner_rot(cell).map(|r| (slope, r)),
            _ => None,
        };
        let (part, rot) = match corner {
            Some(c) => c,
            None => (self.catalog.block.as_ref().ok_or(Refusal::NoBlockPart)?, 0),
        };
        Ok(PaintAction::Insert(unit_placement(
            part, cell, rot, material,
        )))
    }

    /// Bevel: if the empty `cell` is an outer corner of the hull (two adjacent sides touch a part's straight
    /// side, the other two are open), the `rot` of the 1×1 slope (unflipped) whose straight sides are the
    /// touching sides.
    fn corner_rot(&self, cell: Cell) -> Option<u8> {
        let (mut touch, mut open) = (0u8, 0u8);
        for d in Dir::ALL {
            let n = step(cell, d);
            match self.part_at(n) {
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

    fn slope_plan(&self, part: &str, cell: Cell, material: &str) -> Result<SlopePlan, Refusal> {
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
        if let Some(k) = self.part_at(cell) {
            if self.parts[k].part != part {
                return Err(Refusal::Overlaps(k));
            }
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
        // `max_by_key` keeps the last of equal maxima; reversed, ties go to the first in turning order.
        let best = orientations
            .iter()
            .filter_map(|&(rot, flip)| self.anchored(fp, cell, rot, flip))
            .filter(|s| self.fits(s, &[]).is_ok())
            .rev()
            .max_by_key(|s| {
                let (links, blocked) = self.slope_contacts(s);
                (links, std::cmp::Reverse(blocked), self.lean(s))
            })
            .ok_or(Refusal::NoRoom)?;
        Ok(SlopePlan::New(with_pose(&new, &best)))
    }

    /// `(links, blocked)` of a slope candidate: its straight unit sides that meet a neighbour's straight side,
    /// and the occupied cells in front of its sloped face.
    fn slope_contacts(&self, s: &PlacedShape) -> (u32, u32) {
        let cells = s.cells();
        let sides = s.connectable_sides();
        let (mut links, mut blocked) = (0, 0);
        for &c in &cells {
            for d in Dir::ALL {
                let n = step(c, d);
                if cells.contains(&n) || self.part_at(n).is_none() {
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

    /// Inside the frame and on free cells (parts in `ignore` do not count).
    fn fits(&self, s: &PlacedShape, ignore: &[usize]) -> Result<(), Refusal> {
        let (w, h) = s.dims();
        if s.x < 0 || s.y < 0 || s.x + w as i32 > self.frame.0 || s.y + h as i32 > self.frame.1 {
            return Err(Refusal::Outside);
        }
        for c in s.cells() {
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
                Some(stroke) => stroke.extend(log),
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

    /// The shift that centres the design's bounding box in `frame`; `None` if it does not fit.
    fn centring(&self, frame: (i32, i32)) -> Option<(i32, i32)> {
        let Some(b) = self.bounds() else {
            return Some((0, 0));
        };
        if b.width() > frame.0 || b.height() > frame.1 {
            return None;
        }
        Some((
            (frame.0 - b.width()) / 2 - b.min_x,
            (frame.1 - b.height()) / 2 - b.min_y,
        ))
    }

    fn shift_parts(&mut self, (dx, dy): (i32, i32)) {
        for (p, s) in self.parts.iter_mut().zip(&mut self.shapes) {
            (p.x, p.y) = (p.x + dx, p.y + dy);
            (s.x, s.y) = (s.x + dx, s.y + dy);
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
    let ok = |n: u32| (1..=MAX_FRAME).contains(&n);
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
    PlacedShape {
        x: width - s.x - s.dims().0 as i32,
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
