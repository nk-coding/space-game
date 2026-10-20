//! A blueprint on its grid ([`ShipGrid`]): which part covers which cell, links between parts, pieces (connected groups)
//! and the main piece, exposure, open sides, the line of fire, hull edges.

use super::*;

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

pub(super) const NONE: u32 = u32::MAX;

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
    /// the shared cells (as in the concept board's drydock rules). `None` when the parts lie further than
    /// [`crate::content::limits::MAX_COORD`] from 0 or span more than [`crate::content::validate::MAX_FRAME_SIZE`] cells in either
    /// direction: no ship fits a frame like that.
    pub fn new(shapes: Vec<PlacedShape>) -> Option<Self> {
        let (x0, y0, x1, y1) = match span(&shapes) {
            Some([x0, y0, x1, y1]) => {
                let reach = crate::content::limits::MAX_COORD as i64;
                let side = crate::content::validate::MAX_FRAME_SIZE as i64;
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

    pub(super) fn slot(&self, (x, y): Cell) -> Option<usize> {
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

    /// Puts a removed part back (a heart re-grew it, GDR-0099). Refused (false) when any of its cells is taken, and
    /// for a part that is present.
    pub fn restore(&mut self, i: usize) -> bool {
        if self.present[i] || !self.is_free(i) {
            return false;
        }
        self.present[i] = true;
        self.stamp(i);
        true
    }

    /// Are all of part `i`'s cells free (for a removed part: could it be restored)?
    pub fn is_free(&self, i: usize) -> bool {
        self.shapes[i].cells().into_iter().all(|c| self.occupant(c).is_none())
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
    pub(super) fn border_cells(&self, i: usize, dir: Dir) -> Vec<Cell> {
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
