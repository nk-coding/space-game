//! Collider shapes per piece, for the physics layer: convex polygons in the piece frame (y up, counter-
//! clockwise). The square cells of rectangular parts are merged greedily into rectangles (fewer shapes =
//! faster physics); slope triangles stay one shape each. Every candidate engine takes convex polygons.

use crate::design::{PartIdx, ShipDesign};
use crate::mass::to_piece;
use crate::math::Vec2;

/// A convex polygon of 3 (slope) or 4 (merged rectangle) points, counter-clockwise in the piece frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConvexPolygon {
    points: [Vec2; 4],
    len: u8,
    /// The slope part this triangle belongs to; `None` for merged rectangles (map hits to parts through
    /// the grid instead, see `Ship::part_at_local`).
    pub part: Option<PartIdx>,
}

impl ConvexPolygon {
    pub fn points(&self) -> &[Vec2] {
        &self.points[..self.len as usize]
    }

    /// Signed area (positive = counter-clockwise).
    pub fn area(&self) -> f32 {
        let p = self.points();
        let n = p.len();
        let mut a2 = 0.0;
        for k in 0..n {
            a2 += p[k].cross(p[(k + 1) % n]);
        }
        a2 / 2.0
    }

    /// An axis-aligned rectangle (counter-clockwise).
    pub fn rect(min: Vec2, max: Vec2) -> ConvexPolygon {
        ConvexPolygon {
            points: [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)],
            len: 4,
            part: None,
        }
    }
}

/// Reusable buffers for [`build_colliders`] (no per-call allocation once warmed up).
#[derive(Clone, Debug, Default)]
pub struct ColliderScratch {
    full: Vec<bool>,
}

/// Builds the colliders of a piece made of `parts` (alive parts only) with frame origin `origin`
/// (blueprint point). Output order: merged rectangles in row-major order of their top-left cell, then
/// slope triangles in part order. Deterministic.
pub fn build_colliders(
    design: &ShipDesign,
    parts: &[PartIdx],
    origin: [f32; 2],
    scratch: &mut ColliderScratch,
    out: &mut Vec<ConvexPolygon>,
) {
    out.clear();
    // Bounding box of the piece's rectangular parts.
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for &p in parts {
        let spec = &design.parts[p as usize];
        if spec.slope {
            continue;
        }
        let s = spec.shape;
        let (w, h) = s.dims();
        x0 = x0.min(s.x);
        y0 = y0.min(s.y);
        x1 = x1.max(s.x + w as i32);
        y1 = y1.max(s.y + h as i32);
    }
    if x0 < x1 {
        let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
        let full = &mut scratch.full;
        full.clear();
        full.resize(w * h, false);
        for &p in parts {
            let spec = &design.parts[p as usize];
            if spec.slope {
                continue;
            }
            let s = spec.shape;
            let (pw, ph) = s.dims();
            for yy in s.y..s.y + ph as i32 {
                for xx in s.x..s.x + pw as i32 {
                    full[(yy - y0) as usize * w + (xx - x0) as usize] = true;
                }
            }
        }
        // Greedy merge: extend right as far as possible, then down while the whole row segment is full.
        for row in 0..h {
            let mut col = 0;
            while col < w {
                if !full[row * w + col] {
                    col += 1;
                    continue;
                }
                let mut rw = 1;
                while col + rw < w && full[row * w + col + rw] {
                    rw += 1;
                }
                let mut rh = 1;
                while row + rh < h && (col..col + rw).all(|c| full[(row + rh) * w + c]) {
                    rh += 1;
                }
                for r in row..row + rh {
                    for c in col..col + rw {
                        full[r * w + c] = false;
                    }
                }
                // Blueprint rectangle [bx0, bx1] × [by0, by1] (y down) → piece frame (y up).
                let (bx0, by0) = ((x0 + col as i32) as f32, (y0 + row as i32) as f32);
                let (bx1, by1) = (bx0 + rw as f32, by0 + rh as f32);
                let a = to_piece([bx0, by1], origin); // bottom-left in y-up
                let b = to_piece([bx1, by0], origin); // top-right in y-up
                out.push(ConvexPolygon::rect(a, b));
                col += rw;
            }
        }
    }
    for &p in parts {
        let spec = &design.parts[p as usize];
        if !spec.slope {
            continue;
        }
        let poly = spec.shape.polygon();
        let mut pts = [Vec2::ZERO; 4];
        for (k, g) in poly.iter().take(3).enumerate() {
            pts[k] = to_piece([g.x as f32, g.y as f32], origin);
        }
        let mut tri = ConvexPolygon {
            points: pts,
            len: 3,
            part: Some(p),
        };
        if tri.area() < 0.0 {
            tri.points.swap(1, 2);
        }
        out.push(tri);
    }
}
