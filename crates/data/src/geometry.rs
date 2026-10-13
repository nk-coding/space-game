//! Blueprint geometry shared by the sim, the renderer and wiki diagrams. A port of the drydock rules in
//! `docs/concept/reference/drydock-model.js` (`xfCell`, `xfDir`, `xfPoint`, `fullEdges`, `polygon`, `linked`,
//! `exposure`, the piece flood fill of `analyze`) and of `hullPath` in `concept-board-script.js`.
//!
//! # Conventions
//!
//! * **Blueprint frame**, 1 cell = 1 m. `x` grows to the right, `y` grows **down**. The ship's forward
//!   direction is up (−y). Cell `(x, y)` covers the square `[x, x+1] × [y, y+1]`; corners are integer points.
//! * A part's footprint is `w × h` cells before rotation. Placement `(x, y)` is the top-left cell of the
//!   *rotated* footprint.
//! * `flip` mirrors the footprint left-right (`x → w − x`) **before** rotating.
//! * `rot` is the number of quarter turns **clockwise** as seen on screen (y down).
//! * Slopes are right triangles over a `w × 1` footprint. Unrotated, the right angle is at the bottom-left
//!   corner, the straight sides are the left side and the bottom side, and the slope face runs from the
//!   top-left to the bottom-right corner (it faces up and to the right).
//! * Two cells of different parts are **linked** when both parts have a straight (connectable) side on the
//!   shared edge. Slopes therefore connect only through their two straight sides. Cells of the same part
//!   are always linked. Linked cells form **pieces**.
//! * Polygons have positive shoelace area in this frame (clockwise on screen with y down, which becomes
//!   counter-clockwise once a renderer flips y to point up). Walking a polygon or a hull edge, the solid is
//!   on the right-hand side (screen view).
//!
//! For a y-up renderer (Bevy): `world = (x, −y)`; a placement's rotation is `−rot · 90°` about z, and the flip
//! is a mirror in local x applied before that rotation.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::Shape;

/// A cell position `(x, y)` in the blueprint frame.
pub type Cell = (i32, i32);

/// The four cell sides. `Up` is −y, the ship's forward direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Dir {
    Up,
    Right,
    Down,
    Left,
}

impl Dir {
    pub const ALL: [Dir; 4] = [Dir::Up, Dir::Right, Dir::Down, Dir::Left];

    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn from_index(i: u8) -> Dir {
        Dir::ALL[(i % 4) as usize]
    }

    /// Step to the neighbouring cell on this side.
    pub fn delta(self) -> (i32, i32) {
        match self {
            Dir::Up => (0, -1),
            Dir::Right => (1, 0),
            Dir::Down => (0, 1),
            Dir::Left => (-1, 0),
        }
    }

    pub fn opposite(self) -> Dir {
        Dir::from_index(self.index() + 2)
    }

    pub fn bit(self) -> u8 {
        1 << self.index()
    }

    /// Outward unit normal of this side in the blueprint frame (y down): `Up` = `[0, -1]`.
    pub fn normal(self) -> [f32; 2] {
        let (x, y) = self.delta();
        [x as f32, y as f32]
    }

    /// The two corners of this side of `cell`, ordered so the cell is on the right (screen view).
    pub fn side_segment(self, (x, y): Cell) -> (GridPoint, GridPoint) {
        match self {
            Dir::Up => (GridPoint::new(x, y), GridPoint::new(x + 1, y)),
            Dir::Right => (GridPoint::new(x + 1, y), GridPoint::new(x + 1, y + 1)),
            Dir::Down => (GridPoint::new(x + 1, y + 1), GridPoint::new(x, y + 1)),
            Dir::Left => (GridPoint::new(x, y + 1), GridPoint::new(x, y)),
        }
    }

    /// Midpoint of this side of `cell` in metres (where an RCS nozzle sits).
    pub fn side_center(self, (x, y): Cell) -> [f32; 2] {
        let (dx, dy) = self.delta();
        [
            x as f32 + 0.5 + dx as f32 * 0.5,
            y as f32 + 0.5 + dy as f32 * 0.5,
        ]
    }

    /// Direction after a placement's flip and rotation (`xfDir`): mirror Left/Right, then turn clockwise.
    pub fn transform(self, rot: u8, flip: bool) -> Dir {
        let mut d = self.index();
        if flip && d % 2 == 1 {
            d = 4 - d;
        }
        Dir::from_index(d + rot % 4)
    }
}

/// An integer point (cell corner) in the blueprint frame, in metres.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GridPoint {
    pub x: i32,
    pub y: i32,
}

impl GridPoint {
    pub const fn new(x: i32, y: i32) -> Self {
        GridPoint { x, y }
    }
    pub fn to_f32(self) -> [f32; 2] {
        [self.x as f32, self.y as f32]
    }
}

/// Local cell `(i, j)` of an unrotated `w × h` footprint → cell offset after flip and rotation (`xfCell`).
pub fn transform_cell(i: i32, j: i32, w: i32, h: i32, rot: u8, flip: bool) -> (i32, i32) {
    let (mut i, mut j, mut w, mut h) = (i, j, w, h);
    if flip {
        i = w - 1 - i;
    }
    for _ in 0..rot % 4 {
        (i, j) = (h - 1 - j, i);
        std::mem::swap(&mut w, &mut h);
    }
    (i, j)
}

/// Local corner `(x, y)` of an unrotated `w × h` footprint → corner offset after flip and rotation (`xfPoint`).
pub fn transform_point(x: i32, y: i32, w: i32, h: i32, rot: u8, flip: bool) -> (i32, i32) {
    let (mut x, mut y, mut w, mut h) = (x, y, w, h);
    if flip {
        x = w - x;
    }
    for _ in 0..rot % 4 {
        (x, y) = (h - y, x);
        std::mem::swap(&mut w, &mut h);
    }
    (x, y)
}

/// A direction vector after flip and rotation (no translation).
pub fn transform_vector(x: f32, y: f32, rot: u8, flip: bool) -> (f32, f32) {
    let (mut x, mut y) = (if flip { -x } else { x }, y);
    for _ in 0..rot % 4 {
        (x, y) = (-y, x);
    }
    (x, y)
}

/// Unrotated footprint of a part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Footprint {
    pub w: u32,
    pub h: u32,
    pub shape: Shape,
}

impl Footprint {
    pub fn rect(w: u32, h: u32) -> Self {
        Footprint {
            w,
            h,
            shape: Shape::Rect,
        }
    }

    pub fn slope(w: u32) -> Self {
        Footprint {
            w,
            h: 1,
            shape: Shape::Slope,
        }
    }

    /// `(w, h)` after `rot` quarter turns.
    pub fn dims(&self, rot: u8) -> (u32, u32) {
        if rot % 2 == 1 {
            (self.h, self.w)
        } else {
            (self.w, self.h)
        }
    }

    /// Area in cells: `w·h`, or half of it for slopes.
    pub fn area(&self) -> f32 {
        let a = self.w as f32 * self.h as f32;
        if self.shape == Shape::Slope {
            a / 2.0
        } else {
            a
        }
    }

    /// Polygon corners of the unrotated footprint, in the order of `drydock-model.js`.
    fn local_polygon(&self) -> Vec<(i32, i32)> {
        let (w, h) = (self.w as i32, self.h as i32);
        match self.shape {
            Shape::Slope => vec![(0, 0), (0, h), (w, h)],
            Shape::Rect => vec![(0, 0), (w, 0), (w, h), (0, h)],
        }
    }

    /// Straight (connectable) outer sides of the unrotated footprint (`fullEdges` before transforming).
    fn local_sides(&self) -> Vec<(i32, i32, Dir)> {
        let (w, h) = (self.w as i32, self.h as i32);
        let mut v = Vec::new();
        match self.shape {
            Shape::Slope => {
                for j in 0..h {
                    v.push((0, j, Dir::Left));
                }
                for i in 0..w {
                    v.push((i, h - 1, Dir::Down));
                }
            }
            Shape::Rect => {
                for i in 0..w {
                    v.push((i, 0, Dir::Up));
                    v.push((i, h - 1, Dir::Down));
                }
                for j in 0..h {
                    v.push((0, j, Dir::Left));
                    v.push((w - 1, j, Dir::Right));
                }
            }
        }
        v
    }
}

/// A footprint placed on the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlacedShape {
    pub footprint: Footprint,
    pub x: i32,
    pub y: i32,
    pub rot: u8,
    pub flip: bool,
}

impl PlacedShape {
    /// `(w, h)` of the rotated footprint.
    pub fn dims(&self) -> (u32, u32) {
        self.footprint.dims(self.rot)
    }

    fn xf_cell(&self, i: i32, j: i32) -> Cell {
        let (a, b) = transform_cell(
            i,
            j,
            self.footprint.w as i32,
            self.footprint.h as i32,
            self.rot,
            self.flip,
        );
        (self.x + a, self.y + b)
    }

    /// Occupied cells, in local row-major order (`partCells`).
    pub fn cells(&self) -> Vec<Cell> {
        let (w, h) = (self.footprint.w as i32, self.footprint.h as i32);
        let mut out = Vec::with_capacity(self.footprint.w as usize * self.footprint.h as usize);
        for j in 0..h {
            for i in 0..w {
                out.push(self.xf_cell(i, j));
            }
        }
        out
    }

    /// Straight sides through which this part can link to neighbours (`fullEdges`): `(cell, side)`.
    pub fn connectable_sides(&self) -> Vec<(Cell, Dir)> {
        self.footprint
            .local_sides()
            .into_iter()
            .map(|(i, j, d)| (self.xf_cell(i, j), d.transform(self.rot, self.flip)))
            .collect()
    }

    /// Outline in metres: 4 corners for rects, 3 for slopes. Positive shoelace area (see module docs).
    pub fn polygon(&self) -> Vec<GridPoint> {
        let (w, h) = (self.footprint.w as i32, self.footprint.h as i32);
        let mut pts: Vec<GridPoint> = self
            .footprint
            .local_polygon()
            .into_iter()
            .map(|(px, py)| {
                let (a, b) = transform_point(px, py, w, h, self.rot, self.flip);
                GridPoint::new(self.x + a, self.y + b)
            })
            .collect();
        if signed_area2(&pts) < 0 {
            pts.reverse();
        }
        pts
    }

    /// Unit outward normal of the slope face (`slopeNormal`), `None` for rects.
    pub fn slope_normal(&self) -> Option<[f32; 2]> {
        if self.footprint.shape != Shape::Slope {
            return None;
        }
        let (x, y) = transform_vector(
            self.footprint.h as f32,
            -(self.footprint.w as f32),
            self.rot,
            self.flip,
        );
        let len = (x * x + y * y).sqrt();
        Some([x / len, y / len])
    }

    /// The side the part faces: guns fire and engines push this way. `Up` (forward) when `rot == 0`.
    pub fn facing(&self) -> Dir {
        Dir::Up.transform(self.rot, self.flip)
    }

    /// Centre of the rotated footprint in metres. Icons are drawn here at the unrotated size, mirrored in
    /// x first if `flip`, then turned `rot` quarter turns clockwise.
    pub fn center(&self) -> [f32; 2] {
        let (w, h) = self.dims();
        [
            self.x as f32 + w as f32 / 2.0,
            self.y as f32 + h as f32 / 2.0,
        ]
    }

    pub fn area(&self) -> f32 {
        self.footprint.area()
    }

    /// Centroid of the polygon in metres (the centre for rects, the mean of the corners for slopes):
    /// where the part's mass sits.
    pub fn centroid(&self) -> [f32; 2] {
        let poly = self.polygon();
        let n = poly.len() as f32;
        let (sx, sy) = poly
            .iter()
            .fold((0i32, 0i32), |(x, y), p| (x + p.x, y + p.y));
        if self.footprint.shape == Shape::Slope {
            [sx as f32 / n, sy as f32 / n]
        } else {
            self.center()
        }
    }
}

/// Twice the signed shoelace area (positive = clockwise on screen with y down).
fn signed_area2(pts: &[GridPoint]) -> i64 {
    let n = pts.len();
    (0..n)
        .map(|k| {
            let (a, b) = (pts[k], pts[(k + 1) % n]);
            a.x as i64 * b.y as i64 - b.x as i64 * a.y as i64
        })
        .sum()
}

/// The bounding box `[min_x, min_y, max_x, max_y]` (max exclusive) of placed shapes, `None` for none. In
/// `i64`, so shapes anywhere in the `i32` range cannot overflow it.
pub(crate) fn span<'a>(shapes: impl IntoIterator<Item = &'a PlacedShape>) -> Option<[i64; 4]> {
    shapes.into_iter().fold(None, |b, s| {
        let (w, h) = s.dims();
        let (x, y) = (s.x as i64, s.y as i64);
        let a = [x, y, x + w as i64, y + h as i64];
        Some(match b {
            None => a,
            Some(o) => [o[0].min(a[0]), o[1].min(a[1]), o[2].max(a[2]), o[3].max(a[3])],
        })
    })
}

/// Axis-aligned bounds in cells (`max` exclusive) = metres.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bounds {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}

impl Bounds {
    pub fn width(&self) -> i32 {
        self.max_x - self.min_x
    }
    pub fn height(&self) -> i32 {
        self.max_y - self.min_y
    }
    pub fn center(&self) -> [f32; 2] {
        [
            (self.min_x + self.max_x) as f32 / 2.0,
            (self.min_y + self.max_y) as f32 / 2.0,
        ]
    }
}

/// One stretch of the outer hull edge. `a → b` keeps the solid on the right (screen view).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HullSegment {
    pub a: GridPoint,
    pub b: GridPoint,
    /// Index of the part this edge belongs to.
    pub part: usize,
    /// True for a slope face (always boundary); false for a unit-length straight side.
    pub slope: bool,
}

impl HullSegment {
    /// Unit normal pointing away from the solid.
    pub fn outward_normal(&self) -> [f32; 2] {
        let (dx, dy) = ((self.b.x - self.a.x) as f32, (self.b.y - self.a.y) as f32);
        let len = (dx * dx + dy * dy).sqrt();
        [dy / len, -dx / len]
    }
}

/// A chained hull outline: the vertices of a polyline, collinear points merged.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HullLoop {
    /// Closed loops do not repeat the first point at the end.
    pub points: Vec<GridPoint>,
    pub closed: bool,
}

/// The main piece among pieces given in order as `(has a logic core, number of parts)`: a piece with a core
/// beats one without, then more parts wins, ties keep the earlier piece (the drydock model). The one rule for the
/// drydock (`ShipGrid::main_piece`) and the simulation (spawns, splits, `sim::Ship::main_piece`); each caller
/// says what "earlier" means by its order.
pub fn main_of(pieces: impl IntoIterator<Item = (bool, usize)>) -> Option<usize> {
    let mut main: Option<(usize, bool, usize)> = None;
    for (id, (core, size)) in pieces.into_iter().enumerate() {
        main = match main {
            None => Some((id, core, size)),
            Some((_, mc, ms)) if (core && !mc) || (core == mc && size > ms) => Some((id, core, size)),
            m => m,
        };
    }
    main.map(|m| m.0)
}

/// Connected groups of parts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pieces {
    /// Part indices of each piece, ascending. Pieces are numbered in row-major order of their first cell.
    pub parts: Vec<Vec<usize>>,
    /// Piece of each part (`None` for removed parts).
    pub piece_of_part: Vec<Option<usize>>,
}

impl Pieces {
    pub fn len(&self) -> usize {
        self.parts.len()
    }
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }
}

const NONE: u32 = u32::MAX;

/// Occupancy grid of a set of placed parts, with the queries the drydock rules need.
///
/// Parts are addressed by their index in the slice given to [`ShipGrid::new`] (for a blueprint: the index
/// in `Blueprint::parts`). Parts can be removed (breakage) without renumbering.
#[derive(Clone, Debug)]
pub struct ShipGrid {
    shapes: Vec<PlacedShape>,
    present: Vec<bool>,
    origin: Cell,
    width: i32,
    height: i32,
    occ: Vec<u32>,
    /// Straight-side bits ([`Dir::bit`]) of the part occupying each cell.
    sides: Vec<u8>,
}

impl ShipGrid {
    /// Builds the grid. Overlapping parts are invalid content; if they occur anyway the later part wins
    /// the shared cells (as in `drydock-model.js`). `None` when the parts lie further than
    /// [`crate::limits::MAX_COORD`] from 0 or span more than [`crate::validate::MAX_FRAME_SIZE`] cells in either
    /// direction: no ship fits a frame like that.
    pub fn new(shapes: Vec<PlacedShape>) -> Option<Self> {
        let (x0, y0, x1, y1) = match span(&shapes) {
            Some([x0, y0, x1, y1]) => {
                let reach = crate::limits::MAX_COORD as i64;
                let side = crate::validate::MAX_FRAME_SIZE as i64;
                if x0.min(y0) < -reach || x1.max(y1) > reach || x1 - x0 > side || y1 - y0 > side {
                    return None;
                }
                // Within ±MAX_COORD, so every coordinate below fits an i32 with room to spare.
                (x0 as i32, y0 as i32, x1 as i32, y1 as i32)
            }
            None => (0, 0, 0, 0),
        };
        let (width, height) = (x1 - x0, y1 - y0);
        let n = (width * height) as usize;
        let mut grid = ShipGrid {
            present: vec![true; shapes.len()],
            shapes,
            origin: (x0, y0),
            width,
            height,
            occ: vec![NONE; n],
            sides: vec![0; n],
        };
        for i in 0..grid.shapes.len() {
            grid.stamp(i);
        }
        Some(grid)
    }

    fn stamp(&mut self, i: usize) {
        let s = self.shapes[i];
        for c in s.cells() {
            if let Some(k) = self.slot(c) {
                self.occ[k] = i as u32;
                self.sides[k] = 0;
            }
        }
        for (c, d) in s.connectable_sides() {
            if let Some(k) = self.slot(c)
                && self.occ[k] == i as u32
            {
                self.sides[k] |= d.bit();
            }
        }
    }

    fn slot(&self, (x, y): Cell) -> Option<usize> {
        let (lx, ly) = (x - self.origin.0, y - self.origin.1);
        if lx < 0 || ly < 0 || lx >= self.width || ly >= self.height {
            None
        } else {
            Some((ly * self.width + lx) as usize)
        }
    }

    /// Removes a part (it broke off or was destroyed). Indices of other parts stay valid.
    pub fn remove(&mut self, i: usize) {
        if !self.present[i] {
            return;
        }
        self.present[i] = false;
        for c in self.shapes[i].cells() {
            if let Some(k) = self.slot(c)
                && self.occ[k] == i as u32
            {
                self.occ[k] = NONE;
                self.sides[k] = 0;
            }
        }
    }

    pub fn shapes(&self) -> &[PlacedShape] {
        &self.shapes
    }

    pub fn shape(&self, i: usize) -> &PlacedShape {
        &self.shapes[i]
    }

    pub fn len(&self) -> usize {
        self.shapes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.shapes.is_empty()
    }

    pub fn is_present(&self, i: usize) -> bool {
        self.present[i]
    }

    /// The part occupying a cell.
    pub fn occupant(&self, cell: Cell) -> Option<usize> {
        self.slot(cell)
            .map(|k| self.occ[k])
            .filter(|&p| p != NONE)
            .map(|p| p as usize)
    }

    /// Does the part in `cell` have a straight, connectable side on `dir`?
    pub fn connectable(&self, cell: Cell, dir: Dir) -> bool {
        self.slot(cell)
            .is_some_and(|k| self.occ[k] != NONE && self.sides[k] & dir.bit() != 0)
    }

    /// Are `cell` and its neighbour on `dir` joined (same part, or both sides straight)? (`linked`)
    pub fn linked(&self, cell: Cell, dir: Dir) -> bool {
        let (dx, dy) = dir.delta();
        let other = (cell.0 + dx, cell.1 + dy);
        match (self.occupant(cell), self.occupant(other)) {
            (Some(a), Some(b)) if a == b => true,
            (Some(_), Some(_)) => {
                self.connectable(cell, dir) && self.connectable(other, dir.opposite())
            }
            _ => false,
        }
    }

    /// Parts linked to part `i`, ascending.
    pub fn neighbors(&self, i: usize) -> Vec<usize> {
        let mut out = Vec::new();
        if !self.present[i] {
            return out;
        }
        for c in self.shapes[i].cells() {
            for d in Dir::ALL {
                let (dx, dy) = d.delta();
                if let Some(b) = self.occupant((c.0 + dx, c.1 + dy))
                    && b != i
                    && self.linked(c, d)
                {
                    out.push(b);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Number of open faces of a cell (`exposure`): straight sides not linked to a neighbour, plus one for a
    /// slope face. 0 for empty cells.
    pub fn exposure(&self, cell: Cell) -> u32 {
        let Some(a) = self.occupant(cell) else {
            return 0;
        };
        let mut n = 0;
        for d in Dir::ALL {
            let (dx, dy) = d.delta();
            if self.occupant((cell.0 + dx, cell.1 + dy)) == Some(a) {
                continue;
            }
            if self.connectable(cell, d) && !self.linked(cell, d) {
                n += 1;
            }
        }
        if self.shapes[a].footprint.shape == Shape::Slope {
            n += 1;
        }
        n
    }

    /// Straight sides of part `i` whose neighbouring cell is empty: `(cell, side)`, in the order of
    /// [`PlacedShape::connectable_sides`]. These are the RCS nozzles of a part with an `rcs` block. Stricter
    /// than [`ShipGrid::exposure`] (the drydock's venting rule), which also counts a straight side that
    /// faces a neighbour without linking to it (e.g. a slope face).
    pub fn open_sides(&self, i: usize) -> Vec<(Cell, Dir)> {
        if !self.present[i] {
            return Vec::new();
        }
        self.shapes[i]
            .connectable_sides()
            .into_iter()
            .filter(|&(c, d)| {
                let (dx, dy) = d.delta();
                self.occupant(c) == Some(i) && self.occupant((c.0 + dx, c.1 + dy)).is_none()
            })
            .collect()
    }

    /// True if any cell of part `i` is open to space (a radiator vents; `venting` in the drydock model).
    pub fn is_exposed(&self, i: usize) -> bool {
        self.present[i]
            && self.shapes[i]
                .cells()
                .into_iter()
                .any(|c| self.exposure(c) > 0)
    }

    /// Connected pieces (flood fill over linked cells, row-major like `analyze`).
    pub fn pieces(&self) -> Pieces {
        let n = self.occ.len();
        let mut cell_piece = vec![usize::MAX; n];
        let mut pieces: Vec<Vec<usize>> = Vec::new();
        let mut stack = Vec::new();
        for ly in 0..self.height {
            for lx in 0..self.width {
                let k = (ly * self.width + lx) as usize;
                if self.occ[k] == NONE || cell_piece[k] != usize::MAX {
                    continue;
                }
                let id = pieces.len();
                let mut parts = Vec::new();
                cell_piece[k] = id;
                stack.push((lx + self.origin.0, ly + self.origin.1));
                while let Some(c) = stack.pop() {
                    parts.push(self.occupant(c).unwrap());
                    for d in Dir::ALL {
                        if !self.linked(c, d) {
                            continue;
                        }
                        let (dx, dy) = d.delta();
                        let nc = (c.0 + dx, c.1 + dy);
                        let nk = self.slot(nc).unwrap();
                        if cell_piece[nk] == usize::MAX {
                            cell_piece[nk] = id;
                            stack.push(nc);
                        }
                    }
                }
                parts.sort_unstable();
                parts.dedup();
                pieces.push(parts);
            }
        }
        let mut piece_of_part = vec![None; self.shapes.len()];
        for (id, parts) in pieces.iter().enumerate() {
            for &p in parts {
                piece_of_part[p] = Some(id);
            }
        }
        Pieces {
            parts: pieces,
            piece_of_part,
        }
    }

    /// The main piece as in the drydock model ([`main_of`]), over the pieces in their row-major order.
    pub fn main_piece(&self, pieces: &Pieces, is_core: impl Fn(usize) -> bool) -> Option<usize> {
        main_of(
            pieces
                .parts
                .iter()
                .map(|parts| (parts.iter().any(|&p| is_core(p)), parts.len())),
        )
    }

    /// Outer hull edge of the parts selected by `keep` (usually one piece), as in `hullPath`: slope faces are
    /// always boundary; a straight side is boundary unless a kept neighbour covers it with a straight side.
    /// Straight sides come as unit segments; chain them with [`hull_loops`].
    pub fn hull_edges(&self, keep: impl Fn(usize) -> bool) -> Vec<HullSegment> {
        let mut out = Vec::new();
        for (i, shape) in self.shapes.iter().enumerate() {
            if !self.present[i] || !keep(i) {
                continue;
            }
            let poly = shape.polygon();
            for k in 0..poly.len() {
                let (p1, p2) = (poly[k], poly[(k + 1) % poly.len()]);
                if p1.x != p2.x && p1.y != p2.y {
                    out.push(HullSegment {
                        a: p1,
                        b: p2,
                        part: i,
                        slope: true,
                    });
                    continue;
                }
                let (sx, sy) = ((p2.x - p1.x).signum(), (p2.y - p1.y).signum());
                let n = (p2.x - p1.x).abs() + (p2.y - p1.y).abs();
                for t in 0..n {
                    let (ax, ay) = (p1.x + sx * t, p1.y + sy * t);
                    // The two cells beside this unit edge: A above/left, B below/right.
                    let (cell_a, cell_b) = if sy == 0 {
                        let a = (ax.min(ax + sx), ay - 1);
                        (a, (a.0, ay))
                    } else {
                        let a = (ax - 1, ay.min(ay + sy));
                        (a, (ax, a.1))
                    };
                    let in_a = self.occupant(cell_a) == Some(i);
                    let outside = if in_a { cell_b } else { cell_a };
                    let d_out = match (sy == 0, in_a) {
                        (true, true) => Dir::Down,
                        (true, false) => Dir::Up,
                        (false, true) => Dir::Right,
                        (false, false) => Dir::Left,
                    };
                    if let Some(j) = self.occupant(outside)
                        && j != i
                        && keep(j)
                        && self.connectable(outside, d_out.opposite())
                    {
                        continue;
                    }
                    out.push(HullSegment {
                        a: GridPoint::new(ax, ay),
                        b: GridPoint::new(ax + sx, ay + sy),
                        part: i,
                        slope: false,
                    });
                }
            }
        }
        out
    }

    /// Hull edge of one piece.
    pub fn piece_hull_edges(&self, pieces: &Pieces, piece: usize) -> Vec<HullSegment> {
        self.hull_edges(|i| pieces.piece_of_part[i] == Some(piece))
    }

    /// Bounds of the present parts, `None` if there are none.
    pub fn bounds(&self) -> Option<Bounds> {
        self.bounds_of(|_| true)
    }

    /// Bounds of the present parts selected by `keep`.
    pub fn bounds_of(&self, keep: impl Fn(usize) -> bool) -> Option<Bounds> {
        let kept = self
            .shapes
            .iter()
            .enumerate()
            .filter(|&(i, _)| self.present[i] && keep(i))
            .map(|(_, s)| s);
        // `new` keeps every shape within ±MAX_COORD, so the box fits an i32.
        span(kept).map(|[min_x, min_y, max_x, max_y]| Bounds {
            min_x: min_x as i32,
            min_y: min_y as i32,
            max_x: max_x as i32,
            max_y: max_y as i32,
        })
    }

    /// Cells of part `i` on its `dir` side whose neighbour in `dir` is not part of `i`.
    fn border_cells(&self, i: usize, dir: Dir) -> Vec<Cell> {
        let (dx, dy) = dir.delta();
        self.shapes[i]
            .cells()
            .into_iter()
            .filter(|c| self.occupant((c.0 + dx, c.1 + dy)) != Some(i))
            .collect()
    }

    /// A gun's line of fire: every cell straight ahead of its facing side, to the edge of the grid, is empty
    /// (`gunClear`; generalised to rotated guns).
    pub fn line_of_fire_clear(&self, i: usize) -> bool {
        let dir = self.shapes[i].facing();
        let (dx, dy) = dir.delta();
        self.border_cells(i, dir).into_iter().all(|c| {
            let mut p = (c.0 + dx, c.1 + dy);
            while self.slot(p).is_some() {
                if self.occupant(p).is_some() {
                    return false;
                }
                p = (p.0 + dx, p.1 + dy);
            }
            true
        })
    }
}

/// Chains hull segments into polylines. Straight runs and collinear slope faces are merged into single
/// edges. Closed loops start at their top-most, then left-most vertex. Where several edges leave one vertex
/// (parts touching only at a corner), the sharpest right turn is taken, so loops never cross.
pub fn hull_loops(segments: &[HullSegment]) -> Vec<HullLoop> {
    let mut outgoing: BTreeMap<GridPoint, Vec<usize>> = BTreeMap::new();
    for (k, s) in segments.iter().enumerate() {
        outgoing.entry(s.a).or_default().push(k);
    }
    let mut used = vec![false; segments.len()];
    let mut loops = Vec::new();
    for start in 0..segments.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let first = segments[start].a;
        let mut pts = vec![first];
        let mut cur = start;
        let closed = loop {
            let s = segments[cur];
            if s.b == first {
                break true;
            }
            pts.push(s.b);
            let incoming = (s.b.x - s.a.x, s.b.y - s.a.y);
            let next = outgoing
                .get(&s.b)
                .into_iter()
                .flatten()
                .copied()
                .filter(|&k| !used[k])
                .max_by(|&p, &q| {
                    let back = (-incoming.0, -incoming.1);
                    let dp = (
                        segments[p].b.x - segments[p].a.x,
                        segments[p].b.y - segments[p].a.y,
                    );
                    let dq = (
                        segments[q].b.x - segments[q].a.x,
                        segments[q].b.y - segments[q].a.y,
                    );
                    cw_order(back, dp, dq)
                });
            match next {
                Some(k) => {
                    used[k] = true;
                    cur = k;
                }
                None => break false,
            }
        };
        loops.push(HullLoop {
            points: simplify(pts, closed),
            closed,
        });
    }
    loops
}

/// Orders `p` against `q` by the clockwise angle (screen, y down) from `from`, in [0°, 360°).
fn cw_order(from: (i32, i32), p: (i32, i32), q: (i32, i32)) -> std::cmp::Ordering {
    let cross = |a: (i32, i32), b: (i32, i32)| a.0 as i64 * b.1 as i64 - a.1 as i64 * b.0 as i64;
    let dot = |a: (i32, i32), b: (i32, i32)| a.0 as i64 * b.0 as i64 + a.1 as i64 * b.1 as i64;
    // Half 0: [0°, 180°) clockwise from `from`; half 1: [180°, 360°).
    let half = |v: (i32, i32)| {
        let c = cross(from, v);
        if c > 0 || (c == 0 && dot(from, v) > 0) {
            0
        } else {
            1
        }
    };
    half(p).cmp(&half(q)).then_with(|| 0.cmp(&cross(p, q)))
}

/// Drops collinear middle points; closed loops are rotated to start at the top-most, left-most vertex.
fn simplify(pts: Vec<GridPoint>, closed: bool) -> Vec<GridPoint> {
    let collinear = |a: GridPoint, b: GridPoint, c: GridPoint| {
        let (ux, uy) = ((b.x - a.x) as i64, (b.y - a.y) as i64);
        let (vx, vy) = ((c.x - b.x) as i64, (c.y - b.y) as i64);
        ux * vy - uy * vx == 0 && ux * vx + uy * vy > 0
    };
    // A point goes when it lies straight on between its neighbours; removing one never changes whether another
    // does, so one pass with a stack finds them all (O(n); the ends of an open line stay).
    let mut out: Vec<GridPoint> = Vec::with_capacity(pts.len());
    for p in pts {
        while let [.., a, b] = out[..]
            && collinear(a, b, p)
        {
            out.pop();
        }
        out.push(p);
    }
    let mut pts = out;
    // A closed loop also wraps: its last and first points may lie on the run through the seam.
    while closed && pts.len() >= 3 {
        let n = pts.len();
        if collinear(pts[n - 2], pts[n - 1], pts[0]) {
            pts.pop();
        } else if collinear(pts[n - 1], pts[0], pts[1]) {
            pts.remove(0);
        } else {
            break;
        }
    }
    if closed && let Some(start) = (0..pts.len()).min_by_key(|&k| (pts[k].y, pts[k].x)) {
        pts.rotate_left(start);
    }
    pts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rects_all_orientations() -> Vec<PlacedShape> {
        let mut v = Vec::new();
        for fp in [
            Footprint::rect(1, 2),
            Footprint::rect(2, 3),
            Footprint::slope(1),
            Footprint::slope(2),
            Footprint::slope(3),
        ] {
            for rot in 0..4 {
                for flip in [false, true] {
                    v.push(PlacedShape {
                        footprint: fp,
                        x: 3,
                        y: 5,
                        rot,
                        flip,
                    });
                }
            }
        }
        v
    }

    #[test]
    fn cells_stay_inside_rotated_footprint() {
        for s in rects_all_orientations() {
            let (w, h) = s.dims();
            let cells = s.cells();
            assert_eq!(cells.len() as u32, w * h);
            for (x, y) in &cells {
                assert!(
                    *x >= s.x && *x < s.x + w as i32 && *y >= s.y && *y < s.y + h as i32,
                    "{s:?}"
                );
            }
            let mut sorted = cells.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), cells.len());
        }
    }

    #[test]
    fn four_turns_and_double_flip_are_identity() {
        for (w, h) in [(1, 1), (2, 1), (3, 1), (2, 3)] {
            for j in 0..h {
                for i in 0..w {
                    // rot 4 == rot 0
                    assert_eq!(transform_cell(i, j, w, h, 4, false), (i, j));
                    // rot r then (4 - r) on the rotated dims gets back
                    for r in 0..4u8 {
                        let (a, b) = transform_cell(i, j, w, h, r, false);
                        let (rw, rh) = if r % 2 == 1 { (h, w) } else { (w, h) };
                        assert_eq!(transform_cell(a, b, rw, rh, (4 - r) % 4, false), (i, j));
                    }
                    // flip twice
                    let (a, b) = transform_cell(i, j, w, h, 0, true);
                    assert_eq!(transform_cell(a, b, w, h, 0, true), (i, j));
                }
            }
        }
        for d in Dir::ALL {
            assert_eq!(d.transform(4, false), d);
            assert_eq!(d.transform(0, true).transform(0, true), d);
            for r in 0..4 {
                assert_eq!(d.transform(r, false).transform((4 - r) % 4, false), d);
            }
        }
    }

    #[test]
    fn rotation_is_clockwise() {
        // A 1×2 beam turned once lies 2 wide, 1 high; its top cell moves to the right.
        let s = PlacedShape {
            footprint: Footprint::rect(1, 2),
            x: 0,
            y: 0,
            rot: 1,
            flip: false,
        };
        assert_eq!(s.dims(), (2, 1));
        assert_eq!(s.cells(), vec![(1, 0), (0, 0)]);
        assert_eq!(s.facing(), Dir::Right);
        assert_eq!(Dir::Up.transform(1, true), Dir::Right);
        assert_eq!(Dir::Right.transform(0, true), Dir::Left);
    }

    #[test]
    fn transformed_sides_match_transformed_polygon() {
        // Every connectable side must lie on a straight polygon edge, and slopes expose exactly w + h sides.
        for s in rects_all_orientations() {
            let poly = s.polygon();
            assert!(signed_area2(&poly) > 0);
            assert_eq!(signed_area2(&poly) as f32 / 2.0, s.area());
            let sides = s.connectable_sides();
            let fp = s.footprint;
            let expect = if fp.shape == Shape::Slope {
                fp.w + fp.h
            } else {
                2 * (fp.w + fp.h)
            };
            assert_eq!(sides.len() as u32, expect);
            for ((x, y), d) in sides {
                let (a, b) = match d {
                    Dir::Up => ((x, y), (x + 1, y)),
                    Dir::Down => ((x, y + 1), (x + 1, y + 1)),
                    Dir::Left => ((x, y), (x, y + 1)),
                    Dir::Right => ((x + 1, y), (x + 1, y + 1)),
                };
                let on_edge = (0..poly.len()).any(|k| {
                    let (p, q) = (poly[k], poly[(k + 1) % poly.len()]);
                    let on = |pt: (i32, i32)| {
                        let cross = (q.x - p.x) * (pt.1 - p.y) - (q.y - p.y) * (pt.0 - p.x);
                        cross == 0
                            && pt.0 >= p.x.min(q.x)
                            && pt.0 <= p.x.max(q.x)
                            && pt.1 >= p.y.min(q.y)
                            && pt.1 <= p.y.max(q.y)
                    };
                    (p.x == q.x || p.y == q.y) && on(a) && on(b)
                });
                assert!(
                    on_edge,
                    "{s:?} side {d:?} at {x},{y} not on polygon {poly:?}"
                );
            }
        }
    }

    #[test]
    fn slope_normal_points_away_from_right_angle() {
        let s = PlacedShape {
            footprint: Footprint::slope(1),
            x: 0,
            y: 0,
            rot: 0,
            flip: false,
        };
        let n = s.slope_normal().unwrap();
        assert!(n[0] > 0.0 && n[1] < 0.0);
        let s = PlacedShape {
            footprint: Footprint::slope(2),
            x: 0,
            y: 0,
            rot: 2,
            flip: true,
        };
        let n = s.slope_normal().unwrap();
        // flip: face up-left; rot 2: face down-right
        assert!(n[0] > 0.0 && n[1] > 0.0);
    }
}
