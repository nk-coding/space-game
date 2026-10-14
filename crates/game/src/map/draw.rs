//! The voyage map's own meshes (atlas meshes in stage px: y down on the stage, flipped into the mesh's y up, origin
//! at the stage's centre), drawn under the map's UI text like home's world under its labels; the galaxy behind them
//! is `galaxy`'s.
//!
//! * **The detail** ([`build_detail`]): the chain (dotted and glowing to open voyages, dashed and dim to locked
//!   ones) and the markers (open a ring, cleared a filled dot, locked a dotted ring, a guardian a coral diamond,
//!   the shipyard a diamond with a ship, the selection a dotted ring around it), placed through the view
//!   (`view::MapView`) but sized in stage px, so they keep their size at any zoom; they fade out towards the
//!   panel's edge. Rebuilt when what it shows ([`Look`]) or the view changes: a few hundred vertices.
//! * **The chrome** ([`build_chrome`]): the legend's symbols and the panel's backing, fixed to the stage. Built once.

use bevy::prelude::*;

use super::layout::{Geometry, PANEL_X, STAGE, VoyageState};
use crate::atlas::Region;
use crate::meshbuf::MeshBuf;
use crate::style::{Style, mix, vcol};

/// The legend's rows (stage y) and its columns (stage x of the symbols; the words start [`LEGEND_GAP`] after).
pub const LEGEND_ROWS: [f32; 2] = [676.0, 698.0];
pub const LEGEND_COLUMNS: [f32; 4] = [38.0, 132.0, 226.0, 326.0];
pub const LEGEND_GAP: f32 = 14.0;
/// Marker sizes (stage px): the ring of an open voyage, the selection's dotted ring.
const OPEN_R: f32 = 8.5;
const SELECT_R: f32 = 19.0;
/// How far a route stops short of the markers it joins.
const ROUTE_GAP: f32 = 15.0;
/// How far the panel's backing runs on past the stage (a much wider or taller window than the stage).
const PANEL_RUN: f32 = 6000.0;
/// The detail fades out over this many stage px before the panel's edge.
const EDGE_FADE: (f32, f32) = (8.0, 32.0);

/// What the mesh shows besides the content's layout (the rebuild key).
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub states: Vec<VoyageState>,
    pub guardian: Vec<bool>,
    pub selected: Option<usize>,
    pub hovered: Option<usize>,
    pub glow: bool,
}

/// The mesh builder in stage coordinates (also `galaxy`'s, in galaxy px).
pub struct Pen {
    m: MeshBuf,
    white: Region,
    soft: Region,
    uv: Vec2,
}

/// A stage point in the mesh (origin at the stage's centre, y up).
fn p(s: Vec2) -> Vec2 {
    Vec2::new(s.x - STAGE.x / 2.0, STAGE.y / 2.0 - s.y)
}

impl Pen {
    pub fn new(white: &Region, soft: &Region) -> Self {
        Self { m: MeshBuf::default(), white: *white, soft: *soft, uv: white.uv_center() }
    }

    pub fn into_mesh(self) -> Mesh {
        self.m.into_mesh()
    }

    pub fn disc(&mut self, c: Vec2, r: f32, col: [f32; 4]) {
        self.disc_n(c, r, (r * 2.5).clamp(8.0, 40.0) as usize, col);
    }

    /// A disc of `n` sides.
    pub fn disc_n(&mut self, c: Vec2, r: f32, n: usize, col: [f32; 4]) {
        let pts: Vec<Vec2> = (0..n).map(|k| p(c + Vec2::from_angle(k as f32 / n as f32 * std::f32::consts::TAU) * r)).collect();
        self.m.polygon(&pts, self.uv, col);
    }

    pub fn ring(&mut self, c: Vec2, r: f32, half: f32, col: [f32; 4]) {
        let n = (r * 2.0).clamp(16.0, 96.0) as usize;
        let pts: Vec<Vec2> = (0..n).map(|k| p(c + Vec2::from_angle(k as f32 / n as f32 * std::f32::consts::TAU) * r)).collect();
        self.m.strip(&pts, true, half, &self.white, col);
    }

    pub fn dotted_ring(&mut self, c: Vec2, r: f32, dots: usize, dot: f32, col: [f32; 4]) {
        for k in 0..dots {
            self.disc(c + Vec2::from_angle(k as f32 / dots as f32 * std::f32::consts::TAU) * r, dot, col);
        }
    }

    pub fn line(&mut self, a: Vec2, b: Vec2, half: f32, col: [f32; 4]) {
        self.m.strip(&[p(a), p(b)], false, half, &self.white, col);
    }

    /// Dots every `step` from `a` to `b`.
    pub fn dotted(&mut self, a: Vec2, b: Vec2, step: f32, dot: f32, col: [f32; 4]) {
        let n = (a.distance(b) / step).floor() as usize;
        for k in 0..=n {
            self.disc(a.lerp(b, k as f32 / n.max(1) as f32), dot, col);
        }
    }

    pub fn dashed(&mut self, a: Vec2, b: Vec2, dash: f32, gap: f32, half: f32, col: [f32; 4]) {
        let (len, d) = (a.distance(b), (b - a).normalize_or_zero());
        let mut s = 0.0;
        while s < len {
            self.line(a + d * s, a + d * (s + dash).min(len), half, col);
            s += dash + gap;
        }
    }

    /// A soft glow (the atlas's soft dot) `size` across.
    pub fn glow(&mut self, c: Vec2, size: f32, col: [f32; 4]) {
        let (a, b) = (p(c) - Vec2::splat(size / 2.0), p(c) + Vec2::splat(size / 2.0));
        self.m.region_box(&self.soft, a, b, 0, false, col);
    }

    /// A soft line (the soft dot's cross-section along a strip) `half` px to each side.
    pub fn glow_line(&mut self, a: Vec2, b: Vec2, half: f32, col: [f32; 4]) {
        self.m.strip(&[p(a), p(b)], false, half, &self.soft, col);
    }

    pub fn diamond(&mut self, c: Vec2, r: f32, col: [f32; 4]) {
        let pts = [c + Vec2::new(0.0, -r), c + Vec2::new(r, 0.0), c + Vec2::new(0.0, r), c + Vec2::new(-r, 0.0)].map(p);
        self.m.polygon(&pts, self.uv, col);
    }

    pub fn diamond_outline(&mut self, c: Vec2, r: f32, half: f32, col: [f32; 4]) {
        let pts = [c + Vec2::new(0.0, -r), c + Vec2::new(r, 0.0), c + Vec2::new(0.0, r), c + Vec2::new(-r, 0.0)].map(p);
        self.m.strip(&pts, true, half, &self.white, col);
    }

    pub fn rect(&mut self, min: Vec2, max: Vec2, col: [f32; 4]) {
        let pts = [Vec2::new(min.x, max.y), max, Vec2::new(max.x, min.y), min].map(p);
        self.m.polygon(&pts, self.uv, col);
    }

    /// A dotted circle: dots about every `step` px.
    pub fn dotted_circle(&mut self, centre: Vec2, r: f32, step: f32, dot: f32, col: [f32; 4]) {
        self.dotted_ring(centre, r, (std::f32::consts::TAU * r / step).ceil() as usize, dot, col);
    }
}

/// `col` with its alpha times `k`.
fn faded(mut col: [f32; 4], k: f32) -> [f32; 4] {
    col[3] *= k;
    col
}

/// How much of the detail shows at stage x `x`: all of it left of the panel, fading out over [`EDGE_FADE`].
fn edge(x: f32) -> f32 {
    ((PANEL_X - EDGE_FADE.0 - x) / (EDGE_FADE.1 - EDGE_FADE.0)).clamp(0.0, 1.0)
}

/// Cuts the segment `a`–`z` at stage x `end`: the part left of it, if any.
fn cut(a: Vec2, z: Vec2, end: f32) -> Option<(Vec2, Vec2)> {
    let at = |from: Vec2, to: Vec2| from.lerp(to, (end - from.x) / (to.x - from.x));
    match (a.x > end, z.x > end) {
        (true, true) => None,
        (false, false) => Some((a, z)),
        (true, false) => Some((at(z, a), z)),
        (false, true) => Some((a, at(a, z))),
    }
}

/// The detail's mesh: the chain and the markers, each galaxy point placed on the stage by `to`.
pub fn build_detail(geo: &Geometry, look: &Look, to: impl Fn(Vec2) -> Vec2, style: &Style, white: &Region, soft: &Region) -> Mesh {
    let ui = &style.ui;
    let mut pen = Pen::new(white, soft);
    let glow = look.glow;

    // The chain, cut where it reaches the panel.
    for &route in &geo.routes {
        let (a, z) = geo.ends(route);
        let (a, z) = (to(a), to(z));
        let d = (z - a).normalize_or_zero();
        if a.distance(z) <= 2.0 * ROUTE_GAP {
            continue;
        }
        let Some((a, z)) = cut(a + d * ROUTE_GAP, z - d * ROUTE_GAP, PANEL_X - EDGE_FADE.1) else { continue };
        if look.states.get(route.1).is_some_and(|s| *s != VoyageState::Locked) {
            if glow {
                pen.glow_line(a, z, 5.0, vcol(ui.teal, 0.16));
            }
            pen.dotted(a, z, 8.0, 1.5, vcol(ui.teal, 0.95));
        } else {
            pen.dashed(a, z, 5.0, 6.0, 0.6, vcol(ui.label, 0.35));
        }
    }

    // The shipyard: a diamond with a ship in it.
    let h = to(geo.home);
    let k = edge(h.x);
    if k > 0.0 {
        if glow {
            pen.glow(h, 40.0, faded(vcol(ui.teal, 0.18), k));
        }
        pen.diamond_outline(h, 13.0, 0.9, faded(vcol(ui.number, 0.9), k));
        pen.m.polygon(&[h + Vec2::new(0.0, -5.5), h + Vec2::new(4.5, 3.5), h + Vec2::new(-4.5, 3.5)].map(p), pen.uv, faded(vcol(ui.teal, 1.0), k));
    }

    // The voyages.
    for (n, node) in geo.nodes.iter().copied().enumerate() {
        let at = to(node);
        let k = edge(at.x);
        if k <= 0.0 {
            continue;
        }
        let f = |col| faded(col, k);
        let state = look.states.get(n).copied().unwrap_or(VoyageState::Locked);
        let guardian = look.guardian.get(n).copied().unwrap_or(false);
        let lit = state != VoyageState::Locked;
        if look.hovered == Some(n) && look.selected != Some(n) {
            pen.ring(at, 16.0, 0.6, f(vcol(ui.number, 0.35)));
        }
        if guardian {
            let a = if lit { 0.95 } else { 0.7 };
            if glow && lit {
                pen.glow(at, 40.0, f(vcol(ui.coral, 0.25)));
            }
            pen.diamond_outline(at, 12.5, 1.0, f(vcol(ui.coral, a)));
            let inner = if matches!(state, VoyageState::Cleared { .. }) { ui.teal } else { ui.coral };
            pen.diamond(at, 5.0, f(vcol(inner, a)));
        } else {
            match state {
                VoyageState::Open => {
                    if glow {
                        pen.glow(at, 36.0, f(vcol(ui.teal, 0.35)));
                    }
                    pen.ring(at, OPEN_R, 1.0, f(vcol(ui.teal, 1.0)));
                    pen.disc(at, 3.4, f(vcol(ui.teal, 1.0)));
                }
                VoyageState::Cleared { .. } => {
                    if glow {
                        pen.glow(at, 32.0, f(vcol(ui.teal, 0.45)));
                    }
                    pen.disc(at, 6.5, f(vcol(mix(ui.teal, ui.number, 0.25), 1.0)));
                }
                VoyageState::Locked => {
                    pen.dotted_ring(at, 5.5, 8, 0.9, f(vcol(ui.label, 0.55)));
                    pen.disc(at, 1.3, f(vcol(ui.label, 0.5)));
                }
            }
        }
        if look.selected == Some(n) {
            let tone = if guardian { ui.coral } else { ui.teal };
            pen.dotted_ring(at, SELECT_R, 28, 1.05, f(vcol(tone, 0.85)));
        }
    }
    pen.into_mesh()
}

/// The chrome's mesh: the legend's symbols and the panel's backing.
pub fn build_chrome(style: &Style, white: &Region, soft: &Region) -> Mesh {
    let ui = &style.ui;
    let mut pen = Pen::new(white, soft);
    // The legend's symbols: home, open, cleared, locked; enemies, wrecks, goal site, guardian.
    let (r0, r1) = (LEGEND_ROWS[0], LEGEND_ROWS[1]);
    let c = LEGEND_COLUMNS;
    pen.diamond_outline(Vec2::new(c[0], r0), 5.5, 0.7, vcol(ui.number, 0.85));
    pen.ring(Vec2::new(c[1], r0), 4.5, 0.7, vcol(ui.teal, 1.0));
    pen.disc(Vec2::new(c[2], r0), 4.0, vcol(mix(ui.teal, ui.number, 0.25), 1.0));
    pen.dotted_ring(Vec2::new(c[3], r0), 4.5, 7, 0.7, vcol(ui.label, 0.6));
    pen.disc(Vec2::new(c[0], r1), 3.0, vcol(ui.coral, 1.0));
    pen.disc(Vec2::new(c[1], r1), 3.0, vcol(ui.gold, 1.0));
    pen.diamond(Vec2::new(c[2], r1), 4.5, vcol(ui.gold, 1.0));
    pen.diamond_outline(Vec2::new(c[3], r1), 5.5, 0.7, vcol(ui.coral, 1.0));

    // The panel's backing (over the galaxy, which shows through faintly) and its edge; it runs on past the stage
    // for wider windows.
    let back = mix(style.background.outer(), ui.number, 0.02);
    pen.rect(Vec2::new(PANEL_X, -PANEL_RUN), Vec2::new(STAGE.x + PANEL_RUN, STAGE.y + PANEL_RUN), vcol(back, 0.8));
    pen.line(Vec2::new(PANEL_X, 24.0), Vec2::new(PANEL_X, STAGE.y - 24.0), 0.5, vcol(ui.label, 0.22));
    pen.into_mesh()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A route reaching under the panel is cut at the line, one wholly under it is dropped; the detail fades out
    /// over the last px before the panel.
    #[test]
    fn the_detail_stops_at_the_panel() {
        let (a, z) = (Vec2::new(800.0, 100.0), Vec2::new(1000.0, 300.0));
        assert_eq!(cut(a, z, 900.0), Some((a, Vec2::new(900.0, 200.0))));
        assert_eq!(cut(z, a, 900.0), Some((Vec2::new(900.0, 200.0), a)));
        assert_eq!(cut(z, z + Vec2::X, 900.0), None);
        assert_eq!(cut(a, a + Vec2::Y, 900.0), Some((a, a + Vec2::Y)));
        assert_eq!((edge(PANEL_X - 100.0), edge(PANEL_X - EDGE_FADE.0)), (1.0, 0.0));
        assert!((edge(PANEL_X - (EDGE_FADE.0 + EDGE_FADE.1) / 2.0) - 0.5).abs() < 1e-5);
    }
}
