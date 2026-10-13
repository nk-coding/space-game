//! The star map's mesh: one atlas mesh in stage px (y down on the stage, flipped into the mesh's y up, origin at the
//! stage's centre), drawn under the map's UI text like home's world under its labels. Bottom to top: the panel's
//! backing, star dust and faint constellation lines, the band edges as dotted arcs (the next band's fainter, a faint
//! tint inside the gate), the chain (dotted and glowing to open voyages, dashed and dim to locked ones), the markers
//! (the shipyard a diamond with a ship, open a ring, cleared a filled dot, locked a dotted ring, a guardian a coral
//! diamond, the selection a dotted ring around it) and the legend's symbols.
//!
//! Rebuilt only when what it shows changes ([`Look`]): a few thousand vertices.

use bevy::prelude::*;

use super::layout::{Geometry, PANEL_X, STAGE, VoyageState};
use crate::atlas::Region;
use crate::backdrop::Rng;
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
/// Stars on the map.
const STARS: usize = 150;

/// What the mesh shows besides the content's layout (the rebuild key).
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub states: Vec<VoyageState>,
    pub guardian: Vec<bool>,
    pub selected: Option<usize>,
    pub hovered: Option<usize>,
    pub glow: bool,
}

/// The mesh builder in stage coordinates.
struct Pen {
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
    fn disc(&mut self, c: Vec2, r: f32, col: [f32; 4]) {
        let n = (r * 2.5).clamp(8.0, 40.0) as usize;
        let pts: Vec<Vec2> = (0..n).map(|k| p(c + Vec2::from_angle(k as f32 / n as f32 * std::f32::consts::TAU) * r)).collect();
        self.m.polygon(&pts, self.uv, col);
    }

    fn ring(&mut self, c: Vec2, r: f32, half: f32, col: [f32; 4]) {
        let n = (r * 2.0).clamp(16.0, 96.0) as usize;
        let pts: Vec<Vec2> = (0..n).map(|k| p(c + Vec2::from_angle(k as f32 / n as f32 * std::f32::consts::TAU) * r)).collect();
        self.m.strip(&pts, true, half, &self.white, col);
    }

    fn dotted_ring(&mut self, c: Vec2, r: f32, dots: usize, dot: f32, col: [f32; 4]) {
        for k in 0..dots {
            self.disc(c + Vec2::from_angle(k as f32 / dots as f32 * std::f32::consts::TAU) * r, dot, col);
        }
    }

    fn line(&mut self, a: Vec2, b: Vec2, half: f32, col: [f32; 4]) {
        self.m.strip(&[p(a), p(b)], false, half, &self.white, col);
    }

    /// Dots every `step` from `a` to `b`.
    fn dotted(&mut self, a: Vec2, b: Vec2, step: f32, dot: f32, col: [f32; 4]) {
        let n = (a.distance(b) / step).floor() as usize;
        for k in 0..=n {
            self.disc(a.lerp(b, k as f32 / n.max(1) as f32), dot, col);
        }
    }

    fn dashed(&mut self, a: Vec2, b: Vec2, dash: f32, gap: f32, half: f32, col: [f32; 4]) {
        let (len, d) = (a.distance(b), (b - a).normalize_or_zero());
        let mut s = 0.0;
        while s < len {
            self.line(a + d * s, a + d * (s + dash).min(len), half, col);
            s += dash + gap;
        }
    }

    /// A soft glow (the atlas's soft dot) `size` across.
    fn glow(&mut self, c: Vec2, size: f32, col: [f32; 4]) {
        let (a, b) = (p(c) - Vec2::splat(size / 2.0), p(c) + Vec2::splat(size / 2.0));
        self.m.region_box(&self.soft, a, b, 0, false, col);
    }

    /// A soft line (the soft dot's cross-section along a strip) `half` px to each side.
    fn glow_line(&mut self, a: Vec2, b: Vec2, half: f32, col: [f32; 4]) {
        self.m.strip(&[p(a), p(b)], false, half, &self.soft, col);
    }

    fn diamond(&mut self, c: Vec2, r: f32, col: [f32; 4]) {
        let pts = [c + Vec2::new(0.0, -r), c + Vec2::new(r, 0.0), c + Vec2::new(0.0, r), c + Vec2::new(-r, 0.0)].map(p);
        self.m.polygon(&pts, self.uv, col);
    }

    fn diamond_outline(&mut self, c: Vec2, r: f32, half: f32, col: [f32; 4]) {
        let pts = [c + Vec2::new(0.0, -r), c + Vec2::new(r, 0.0), c + Vec2::new(0.0, r), c + Vec2::new(-r, 0.0)].map(p);
        self.m.strip(&pts, true, half, &self.white, col);
    }

    fn rect(&mut self, min: Vec2, max: Vec2, col: [f32; 4]) {
        let pts = [Vec2::new(min.x, max.y), max, Vec2::new(max.x, min.y), min].map(p);
        self.m.polygon(&pts, self.uv, col);
    }

    /// A dotted arc of the circle (`centre`, `r`): dots every `step` px where they fall inside `clip`.
    fn dotted_arc(&mut self, centre: Vec2, r: f32, step: f32, dot: f32, clip: Rect, col: [f32; 4]) {
        if r <= 0.0 {
            return;
        }
        let n = (std::f32::consts::TAU * r / step).ceil() as usize;
        for k in 0..n {
            let at = centre + Vec2::from_angle(k as f32 / n as f32 * std::f32::consts::TAU) * r;
            if clip.contains(at) {
                self.disc(at, dot, col);
            }
        }
    }
}

/// The star map's mesh.
pub fn build(geo: &Geometry, look: &Look, style: &Style, white: &Region, soft: &Region) -> Mesh {
    let ui = &style.ui;
    let mut pen = Pen { m: MeshBuf::default(), white: *white, soft: *soft, uv: white.uv_center() };
    let glow = look.glow;
    // Everything that may show past the stage in a wider window: the arcs and the panel run on.
    let clip = Rect::new(-500.0, -300.0, STAGE.x + 500.0, STAGE.y + 300.0);

    // The band: a faint tint inside the gate, the edges dotted.
    let b = &geo.band;
    {
        let n = 160;
        let pts: Vec<Vec2> = (0..n).map(|k| p(b.centre + Vec2::from_angle(k as f32 / n as f32 * std::f32::consts::TAU) * b.inner)).collect();
        pen.m.polygon(&pts, pen.uv, vcol(ui.coral, 0.025));
    }
    pen.dotted_arc(b.centre, b.outer, 7.0, 0.9, clip, vcol(ui.label, 0.3));
    pen.dotted_arc(b.centre, b.inner, 7.0, 1.0, clip, vcol(mix(ui.label, ui.coral, 0.35), 0.45));
    pen.dotted_arc(b.centre, b.next, 7.0, 0.8, clip, vcol(ui.label, 0.14));

    // Star dust and a few faint constellation lines (the same sky every time).
    let mut rng = Rng::new(0x5EED_7A1);
    let stars: Vec<(Vec2, f32, f32)> =
        (0..STARS).map(|_| (Vec2::new(rng.range(0.0, PANEL_X), rng.range(0.0, STAGE.y)), rng.range(0.6, 1.7), rng.range(0.12, 0.45))).collect();
    for (k, (a, _, _)) in stars.iter().enumerate().filter(|(k, _)| k % 9 == 0) {
        let near = stars.iter().enumerate().filter(|(j, _)| *j != k).map(|(_, s)| s.0).filter(|s| s.distance(*a) > 40.0).min_by(|x, y| x.distance(*a).total_cmp(&y.distance(*a)));
        if let Some(z) = near.filter(|z| z.distance(*a) < 170.0) {
            pen.dotted(*a, z, 9.0, 0.7, vcol(ui.teal, 0.1));
        }
    }
    for (at, r, a) in &stars {
        pen.disc(*at, *r, vcol(mix(ui.label, ui.teal, 0.3), *a));
    }

    // The chain.
    for &route in &geo.routes {
        let (a, z) = geo.ends(route);
        let d = (z - a).normalize_or_zero();
        let (a, z) = (a + d * ROUTE_GAP, z - d * ROUTE_GAP);
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
    let h = geo.home;
    if glow {
        pen.glow(h, 40.0, vcol(ui.teal, 0.18));
    }
    pen.diamond_outline(h, 13.0, 0.9, vcol(ui.number, 0.9));
    pen.m.polygon(&[h + Vec2::new(0.0, -5.5), h + Vec2::new(4.5, 3.5), h + Vec2::new(-4.5, 3.5)].map(p), pen.uv, vcol(ui.teal, 1.0));

    // The voyages.
    for (k, at) in geo.nodes.iter().copied().enumerate() {
        let state = look.states.get(k).copied().unwrap_or(VoyageState::Locked);
        let guardian = look.guardian.get(k).copied().unwrap_or(false);
        let lit = state != VoyageState::Locked;
        if look.hovered == Some(k) && look.selected != Some(k) {
            pen.ring(at, 16.0, 0.6, vcol(ui.number, 0.35));
        }
        if guardian {
            let a = if lit { 0.95 } else { 0.7 };
            if glow && lit {
                pen.glow(at, 40.0, vcol(ui.coral, 0.25));
            }
            pen.diamond_outline(at, 12.5, 1.0, vcol(ui.coral, a));
            let inner = if matches!(state, VoyageState::Cleared { .. }) { ui.teal } else { ui.coral };
            pen.diamond(at, 5.0, vcol(inner, a));
        } else {
            match state {
                VoyageState::Open => {
                    if glow {
                        pen.glow(at, 36.0, vcol(ui.teal, 0.35));
                    }
                    pen.ring(at, OPEN_R, 1.0, vcol(ui.teal, 1.0));
                    pen.disc(at, 3.4, vcol(ui.teal, 1.0));
                }
                VoyageState::Cleared { .. } => {
                    if glow {
                        pen.glow(at, 32.0, vcol(ui.teal, 0.45));
                    }
                    pen.disc(at, 6.5, vcol(mix(ui.teal, ui.number, 0.25), 1.0));
                }
                VoyageState::Locked => {
                    pen.dotted_ring(at, 5.5, 8, 0.9, vcol(ui.label, 0.55));
                    pen.disc(at, 1.3, vcol(ui.label, 0.5));
                }
            }
        }
        if look.selected == Some(k) {
            let tone = if guardian { ui.coral } else { ui.teal };
            pen.dotted_ring(at, SELECT_R, 28, 1.05, vcol(tone, 0.85));
        }
    }

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

    // The panel's backing (over the band's far arcs, which show through faintly) and its edge; it runs on past the
    // stage for wider windows.
    let back = mix(style.background.outer(), ui.number, 0.02);
    pen.rect(Vec2::new(PANEL_X, clip.min.y), Vec2::new(clip.max.x, clip.max.y), vcol(back, 0.62));
    pen.line(Vec2::new(PANEL_X, 24.0), Vec2::new(PANEL_X, STAGE.y - 24.0), 0.5, vcol(ui.label, 0.22));
    pen.m.into_mesh()
}
