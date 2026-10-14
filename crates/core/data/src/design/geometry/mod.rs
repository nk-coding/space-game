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

mod grid;
mod hull;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::content::model::Shape;

pub use grid::{Pieces, ShipGrid, main_of};
pub use hull::{HullLoop, HullSegment, hull_loops};

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
