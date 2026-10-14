//! Mass properties: mass, centre of mass and moment of inertia about the centre of mass, from part masses
//! spread uniformly over their cell polygons (rectangles and slope triangles).

use data::GridPoint;

use crate::math::Vec2;

/// Mass properties of a piece in its piece frame (y up, metres). Units: mass in content mass units ("t"),
/// inertia in t·m².
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MassProps {
    pub mass: f32,
    /// Centre of mass in the piece frame. `(0, 0)` when the piece is created (its frame origin is placed on
    /// its centre of mass); it moves when the piece loses parts without splitting.
    pub com: Vec2,
    /// Moment of inertia about `com`.
    pub inertia: f32,
}

/// Centroid (blueprint frame) and moment of inertia about the centroid of a uniform polygon of `mass`.
/// Any simple polygon with consistent winding works; computed relative to the first vertex to keep the
/// numbers small.
pub fn polygon_mass(poly: &[GridPoint], mass: f32) -> ([f32; 2], f32) {
    let n = poly.len();
    if n < 3 {
        let p = poly.first().map_or([0.0, 0.0], |p| p.to_f32());
        return (p, 0.0);
    }
    let r = poly[0];
    let (mut area2, mut cx, mut cy, mut ii) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for k in 0..n {
        let (p, q) = (poly[k], poly[(k + 1) % n]);
        let (ax, ay) = ((p.x - r.x) as f32, (p.y - r.y) as f32);
        let (bx, by) = ((q.x - r.x) as f32, (q.y - r.y) as f32);
        let cross = ax * by - bx * ay;
        area2 += cross;
        cx += (ax + bx) * cross;
        cy += (ay + by) * cross;
        ii += cross * (ax * ax + ax * bx + bx * bx + ay * ay + ay * by + by * by);
    }
    if area2 == 0.0 {
        return (r.to_f32(), 0.0);
    }
    let area = area2 / 2.0;
    let (gx, gy) = (cx / (3.0 * area2), cy / (3.0 * area2));
    // Second moment about the reference vertex (unit density), then about the centroid.
    let i_ref = ii / 12.0;
    let i_c = i_ref - area * (gx * gx + gy * gy);
    let density = mass / area;
    ([r.x as f32 + gx, r.y as f32 + gy], (i_c * density).abs())
}

/// Combines parts `(mass, centroid in blueprint frame, inertia about own centroid)` into the mass properties
/// of a piece whose frame origin is the blueprint point `origin`. Also returns the centre of mass in the
/// blueprint frame. Two passes (centre first) so the parallel-axis sum never cancels catastrophically.
pub fn combine<I>(parts: I, origin: [f32; 2]) -> (MassProps, [f32; 2])
where
    I: Iterator<Item = (f32, [f32; 2], f32)> + Clone,
{
    let (mut m, mut sx, mut sy) = (0.0f32, 0.0f32, 0.0f32);
    for (pm, c, _) in parts.clone() {
        m += pm;
        sx += pm * c[0];
        sy += pm * c[1];
    }
    if m <= 0.0 {
        return (MassProps::default(), origin);
    }
    let c = [sx / m, sy / m];
    let mut inertia = 0.0f32;
    for (pm, pc, pi) in parts {
        let (dx, dy) = (pc[0] - c[0], pc[1] - c[1]);
        inertia += pi + pm * (dx * dx + dy * dy);
    }
    (
        MassProps {
            mass: m,
            com: to_piece(c, origin),
            inertia,
        },
        c,
    )
}

/// Blueprint point → piece frame with origin `origin` (blueprint point): x stays, y flips.
#[inline]
pub fn to_piece(p: [f32; 2], origin: [f32; 2]) -> Vec2 {
    Vec2::new(p[0] - origin[0], origin[1] - p[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gp(pts: &[(i32, i32)]) -> Vec<GridPoint> {
        pts.iter().map(|&(x, y)| GridPoint::new(x, y)).collect()
    }

    #[test]
    fn unit_square_and_right_triangle() {
        let (c, i) = polygon_mass(&gp(&[(0, 0), (1, 0), (1, 1), (0, 1)]), 1.0);
        assert_eq!(c, [0.5, 0.5]);
        assert!((i - 1.0 / 6.0).abs() < 1e-6);
        let (c, i) = polygon_mass(&gp(&[(3, 4), (5, 4), (5, 7), (3, 7)]), 6.0);
        assert_eq!(c, [4.0, 5.5]);
        assert!((i - 6.0 * (4.0 + 9.0) / 12.0).abs() < 1e-5);
        // Right triangle with legs 1, 1: centroid at 1/3 from the right angle, I = m (a² + b²) / 18.
        let (c, i) = polygon_mass(&gp(&[(0, 0), (0, 1), (1, 1)]), 1.0);
        assert!((c[0] - 1.0 / 3.0).abs() < 1e-6 && (c[1] - 2.0 / 3.0).abs() < 1e-6);
        assert!((i - 2.0 / 18.0).abs() < 1e-6);
        // Winding does not matter.
        let (_, i2) = polygon_mass(&gp(&[(1, 1), (0, 1), (0, 0)]), 1.0);
        assert!((i - i2).abs() < 1e-6);
        // 1×3 slope: legs 3 and 1.
        let (_, i) = polygon_mass(&gp(&[(0, 0), (0, 1), (3, 1)]), 2.0);
        assert!((i - 2.0 * 10.0 / 18.0).abs() < 1e-5);
    }

    #[test]
    fn combine_two_squares() {
        // Two unit squares side by side = a 2×1 rectangle.
        let parts = [(1.0, [0.5, 0.5], 1.0 / 6.0), (1.0, [1.5, 0.5], 1.0 / 6.0)];
        let (mp, c) = combine(parts.iter().copied(), [1.0, 0.5]);
        assert_eq!(c, [1.0, 0.5]);
        assert_eq!(mp.com, Vec2::ZERO);
        assert!((mp.inertia - 2.0 * (4.0 + 1.0) / 12.0).abs() < 1e-6);
    }
}
