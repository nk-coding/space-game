//! A small CPU-side mesh builder for atlas-textured, vertex-coloured 2D geometry.
//! Draw order inside one mesh is index order, so callers add layers bottom to top.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::atlas::Region;

#[derive(Default)]
pub struct MeshBuf {
    pos: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
    /// The z of the vertices added from now on (0 unless set; only the depth test sees it, see `ship_view`).
    z: f32,
}

impl MeshBuf {
    pub fn is_empty(&self) -> bool {
        self.idx.is_empty()
    }

    pub fn set_z(&mut self, z: f32) {
        self.z = z;
    }

    fn vert(&mut self, p: Vec2, uv: Vec2, c: [f32; 4]) -> u32 {
        let i = self.pos.len() as u32;
        self.pos.push([p.x, p.y, self.z]);
        self.uv.push([uv.x, uv.y]);
        self.col.push(c);
        i
    }

    /// A convex polygon (triangle fan), one flat colour, sampling a single texel position (e.g. white).
    pub fn polygon(&mut self, pts: &[Vec2], uv: Vec2, c: [f32; 4]) {
        if pts.len() < 3 {
            return;
        }
        let base = self.vert(pts[0], uv, c);
        for p in &pts[1..] {
            self.vert(*p, uv, c);
        }
        for k in 1..pts.len() as u32 - 1 {
            self.idx.extend_from_slice(&[base, base + k, base + k + 1]);
        }
    }

    /// A quad with explicit corner UVs. Corners in order: a, b, c, d around the quad.
    pub fn quad_uv(&mut self, p: [Vec2; 4], uv: [Vec2; 4], c: [f32; 4]) {
        let base = self.vert(p[0], uv[0], c);
        self.vert(p[1], uv[1], c);
        self.vert(p[2], uv[2], c);
        self.vert(p[3], uv[3], c);
        self.idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// A quad with a colour per corner (a gradient), sampling a single texel position (e.g. white). Corners in order
    /// around the quad.
    pub fn gradient_quad(&mut self, p: [Vec2; 4], uv: Vec2, c: [[f32; 4]; 4]) {
        let base = self.vert(p[0], uv, c[0]);
        for k in 1..4 {
            self.vert(p[k], uv, c[k]);
        }
        self.idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Draws a whole atlas region into the axis-aligned box `min..max` (local, y up). `turns` rotates the
    /// art by quarter turns clockwise (as seen on screen) and `flip` mirrors it horizontally first.
    pub fn region_box(&mut self, r: &Region, min: Vec2, max: Vec2, turns: u8, flip: bool, c: [f32; 4]) {
        // Screen corners: top-left, top-right, bottom-right, bottom-left (y up).
        let p = [Vec2::new(min.x, max.y), Vec2::new(max.x, max.y), Vec2::new(max.x, min.y), Vec2::new(min.x, min.y)];
        // Texture corners in the same order before rotation.
        let mut t = [r.uv_at(Vec2::new(0.0, 0.0)), r.uv_at(Vec2::new(1.0, 0.0)), r.uv_at(Vec2::new(1.0, 1.0)), r.uv_at(Vec2::new(0.0, 1.0))];
        if flip {
            t = [t[1], t[0], t[3], t[2]];
        }
        // A clockwise turn of the art means the art's top-left lands on the screen's top-right, etc.
        t.rotate_right((turns % 4) as usize);
        self.quad_uv(p, t, c);
    }

    /// A region centred on its origin at `at`, scaled to `size` cells, rotated by `angle` (radians, CCW).
    pub fn region_at(&mut self, r: &Region, at: Vec2, size: Vec2, angle: f32, c: [f32; 4]) {
        let o = r.origin / r.cells; // origin as a fraction of the region (y down)
        let (s, co) = angle.sin_cos();
        let rot = |v: Vec2| Vec2::new(v.x * co - v.y * s, v.x * s + v.y * co);
        let corner = |fx: f32, fy: f32| at + rot(Vec2::new((fx - o.x) * size.x, (o.y - fy) * size.y));
        let p = [corner(0.0, 0.0), corner(1.0, 0.0), corner(1.0, 1.0), corner(0.0, 1.0)];
        let t = [r.uv_at(Vec2::new(0.0, 0.0)), r.uv_at(Vec2::new(1.0, 0.0)), r.uv_at(Vec2::new(1.0, 1.0)), r.uv_at(Vec2::new(0.0, 1.0))];
        self.quad_uv(p, t, c);
    }

    /// A strip of constant width along a polyline, with mitred joins. `u` runs across the strip from the
    /// left side (0) to the right side (1) of the travel direction, sampled at the region's middle row.
    pub fn strip(&mut self, pts: &[Vec2], closed: bool, half_width: f32, r: &Region, c: [f32; 4]) {
        let n = pts.len();
        if n < 2 {
            return;
        }
        let v_mid = 0.5;
        let uv_l = r.uv_at(Vec2::new(0.0, v_mid));
        let uv_r = r.uv_at(Vec2::new(1.0, v_mid));
        let dir = |a: Vec2, b: Vec2| (b - a).normalize_or_zero();
        let left = |d: Vec2| Vec2::new(-d.y, d.x);
        let first = self.pos.len() as u32;
        for i in 0..n {
            let prev = if i > 0 { Some(pts[i - 1]) } else if closed { Some(pts[n - 1]) } else { None };
            let next = if i + 1 < n { Some(pts[i + 1]) } else if closed { Some(pts[0]) } else { None };
            let p = pts[i];
            let off = match (prev, next) {
                (Some(a), Some(b)) => {
                    let n0 = left(dir(a, p));
                    let n1 = left(dir(p, b));
                    let m = (n0 + n1).normalize_or_zero();
                    let k = m.dot(n1).max(0.5); // clamp the miter at 2x
                    m * (half_width / k)
                }
                (None, Some(b)) => left(dir(p, b)) * half_width,
                (Some(a), None) => left(dir(a, p)) * half_width,
                (None, None) => Vec2::ZERO,
            };
            self.vert(p + off, uv_l, c);
            self.vert(p - off, uv_r, c);
        }
        let segs = if closed { n } else { n - 1 } as u32;
        for s in 0..segs {
            let a = first + 2 * s;
            let b = first + 2 * ((s + 1) % n as u32);
            self.idx.extend_from_slice(&[a, b, b + 1, a, b + 1, a + 1]);
        }
    }

    /// A mesh that draws nothing (one degenerate triangle: the renderer warns about meshes without vertices),
    /// for a mesh handle whose content comes later.
    pub fn placeholder() -> Mesh {
        let mut m = MeshBuf::default();
        let p = Vec2::ZERO;
        m.quad_uv([p; 4], [p; 4], [0.0; 4]);
        m.into_mesh()
    }

    pub fn into_mesh(self) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uv);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.col);
        if self.idx.len() < u16::MAX as usize && self.idx.iter().all(|&i| i < u16::MAX as u32) {
            mesh.insert_indices(Indices::U16(self.idx.into_iter().map(|i| i as u16).collect()));
        } else {
            mesh.insert_indices(Indices::U32(self.idx));
        }
        mesh
    }
}

/// Insets a convex polygon (CCW, y up) by `d` along each edge normal.
pub fn inset_convex(pts: &[Vec2], d: f32) -> Vec<Vec2> {
    let n = pts.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let a = pts[(i + n - 1) % n];
        let p = pts[i];
        let b = pts[(i + 1) % n];
        // Inward normals of the two edges meeting at p (CCW polygon: inward = left of travel).
        let n0 = { let e = (p - a).normalize_or_zero(); Vec2::new(-e.y, e.x) };
        let n1 = { let e = (b - p).normalize_or_zero(); Vec2::new(-e.y, e.x) };
        // Intersection of the two offset lines.
        let m = n0 + n1;
        let denom = 1.0 + n0.dot(n1);
        let off = if denom.abs() > 1e-4 { m * (d / denom) } else { n0 * d };
        out.push(p + off);
    }
    out
}

/// Makes a polygon counter-clockwise (y up).
pub fn ensure_ccw(pts: &mut [Vec2]) {
    let mut area = 0.0;
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        area += a.x * b.y - b.x * a.y;
    }
    if area < 0.0 {
        pts.reverse();
    }
}
