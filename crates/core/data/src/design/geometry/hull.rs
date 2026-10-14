//! Hull outlines: the boundary segments of a piece and the closed loops through them ([`hull_loops`]), as the
//! concept art draws its hull glow.

use super::*;

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

/// A chained hull outline: the vertices of a polyline, collinear points merged.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HullLoop {
    /// Closed loops do not repeat the first point at the end.
    pub points: Vec<GridPoint>,
    pub closed: bool,
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
pub(super) fn cw_order(from: (i32, i32), p: (i32, i32), q: (i32, i32)) -> std::cmp::Ordering {
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
pub(super) fn simplify(pts: Vec<GridPoint>, closed: bool) -> Vec<GridPoint> {
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
