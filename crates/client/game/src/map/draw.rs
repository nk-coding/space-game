//! The voyage map's own meshes (atlas meshes in stage px: y down on the stage, flipped into the mesh's y up, origin
//! at the stage's centre), drawn under the map's UI text like home's world under its labels; the galaxy behind them
//! is `galaxy`'s.
//!
//! * **The areas** ([`build_areas`], galaxy plan E5, GDR-0097): the jump range as soft circles round home and every
//!   charted cluster (their union's edge, a crisp teal line over a glow fading inwards; no dots, no fill), and each
//!   signal heard as an area: a dotted gold edge like the band edges, a faint fill and a slow pulse running out to
//!   the edge; when a signal narrows the old edge fades out over [`FADE`] while the new one draws. Placed through
//!   the view but drawn in stage px (lines keep their width at any zoom); rebuilt when what it shows or the view
//!   changes, and every frame while a pulse runs or an edge fades (their phases are continuous: in steps of 15–20 a
//!   second they stuttered at 120 fps; Niklas, 18 Oct 2026).
//! * **The detail** ([`build_detail`]): the markers of the voyages found so far, no route between them
//!   (GDR-0078): open a ring, cleared a filled dot, a new signal a ring sending out pulses, a guardian a coral
//!   diamond, the shipyard a diamond with a ship, the selection a dotted ring around it; a charted cluster a small
//!   teal ring, a located signal's cluster a gold one, an opened gate a coral diamond; a blind jump's spot a
//!   crosshair, and a dotted line from where a jump to the pick starts; the warp, a light running along it. Unknown
//!   voyages and uncharted clusters are not drawn. Placed through the view (`view::MapView`) but sized in stage px,
//!   so they keep their size at any zoom; they fade out towards the panel's edge. Rebuilt when what it shows
//!   ([`Look`], with the pulse's phase while a signal shows, every frame then) or the view changes: a few hundred
//!   vertices.
//! * **The chrome** ([`build_chrome`]): the legend's symbols and the panel's backing, fixed to the stage. Built once.

use bevy::prelude::*;
use data::career::TrailSignal;
use data::skeleton::Area;
use kit::atlas::Region;
use kit::meshbuf::MeshBuf;
use kit::style::{Style, mix, vcol};

use super::known::{Known, Pick};
use super::layout::{Geometry, PANEL_X, PX_PER_S, STAGE, VoyageState};

/// The legend's rows (stage y) and its columns (stage x of the symbols; the words start [`LEGEND_GAP`] after).
pub const LEGEND_ROWS: [f32; 2] = [676.0, 698.0];
pub const LEGEND_COLUMNS: [f32; 6] = [38.0, 132.0, 226.0, 326.0, 430.0, 534.0];
pub const LEGEND_GAP: f32 = 14.0;
/// The legend's words, by row and column.
pub const LEGEND: [[&str; 6]; 2] = [["HOME", "OPEN", "CLEARED", "SIGNAL", "CHARTED", "GUARDIAN"], ["ENEMIES", "WRECKS", "GOAL SITE", "SIGNAL AREA", "LOCATED", "JUMP RANGE"]];
/// Marker sizes (stage px): the ring of an open voyage, of a known cluster, the selection's dotted ring, how far a
/// signal's pulses run.
const OPEN_R: f32 = 8.5;
const CLUSTER_R: f32 = 6.0;
const SELECT_R: f32 = 19.0;
const PULSE_R: f32 = 25.0;
/// A signal's pulse: one every this many seconds.
pub const PULSE_PERIOD: f32 = 1.6;
/// A signal area's pulse runs out to its edge in this many seconds.
pub const WAVE_PERIOD: f32 = 3.2;
/// A narrowed signal's old edge fades out over this many seconds.
pub const FADE: f32 = 1.6;
/// How far the panel's backing runs on past the stage (a much wider or taller window than the stage).
const PANEL_RUN: f32 = 6000.0;
/// The detail fades out over this many stage px before the panel's edge.
const EDGE_FADE: (f32, f32) = (8.0, 32.0);

/// What the hover is over (the rebuild key's part).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hover {
    Voyage(usize),
    Cluster(u32),
    Signal(TrailSignal),
}

/// A warp under way on the map: from where to where (galaxy px) and how far along (0 to 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WarpLook {
    pub from: Vec2,
    pub to: Vec2,
    pub t: f32,
}

/// What the meshes show besides the layout (the rebuild key).
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    /// The voyages' ids and states, in content order.
    pub voyages: Vec<String>,
    pub states: Vec<VoyageState>,
    pub guardian: Vec<bool>,
    pub known: Known,
    pub selected: Option<Pick>,
    pub hovered: Option<Hover>,
    pub glow: bool,
    /// The voyages' signal pulse phase (0 to 1; always 0 while there is no new voyage signal).
    pub pulse: f32,
    /// The signal areas' pulse phase (0 to 1; 0 without an area).
    pub wave: f32,
    /// Old edges of signals that narrowed, fading: the area and how far it has faded (0 to 1).
    pub fading: Vec<(Area, f32)>,
    /// Where a jump to the selection starts (galaxy px), drawn as a dotted line to it.
    pub course: Option<(Vec2, Vec2)>,
    pub warp: Option<WarpLook>,
}

impl Look {
    /// The pulse's phase at `seconds` while a signal shows, else 0 (so the look, the rebuild key, stays still).
    pub fn pulse_at(states: &[VoyageState], seconds: f32) -> f32 {
        if !states.contains(&VoyageState::Signal) {
            return 0.0;
        }
        (seconds / PULSE_PERIOD).fract()
    }

    /// The areas' pulse phase at `seconds` while an area shows, else 0.
    pub fn wave_at(known: &Known, seconds: f32) -> f32 {
        if !known.signals.iter().any(|s| s.is_area()) {
            return 0.0;
        }
        (seconds / WAVE_PERIOD).fract()
    }
}

/// The mesh builder in stage coordinates (also `galaxy`'s, in galaxy px).
pub struct Pen {
    m: MeshBuf,
    white: Region,
    soft: Region,
    uv: Vec2,
    /// The area of the glows and discs drawn so far (px²): how much a frame blends (`galaxy::overdraw`).
    area: f32,
}

/// A stage point in the mesh (origin at the stage's centre, y up).
fn p(s: Vec2) -> Vec2 {
    Vec2::new(s.x - STAGE.x / 2.0, STAGE.y / 2.0 - s.y)
}

impl Pen {
    pub fn new(white: &Region, soft: &Region) -> Self {
        Self { m: MeshBuf::default(), white: *white, soft: *soft, uv: white.uv_center(), area: 0.0 }
    }

    pub fn area(&self) -> f32 {
        self.area
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
        self.area += std::f32::consts::PI * r * r;
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

    /// An open line through `pts`.
    pub fn polyline(&mut self, pts: &[Vec2], half: f32, col: [f32; 4]) {
        self.path(pts, false, half, col);
    }

    /// A line through `pts`, back to the first when `closed` (its corners mitred).
    pub fn path(&mut self, pts: &[Vec2], closed: bool, half: f32, col: [f32; 4]) {
        let pts: Vec<Vec2> = pts.iter().map(|q| p(*q)).collect();
        self.m.strip(&pts, closed, half, &self.white, col);
    }

    /// Dots every `step` from `a` to `b`.
    pub fn dotted(&mut self, a: Vec2, b: Vec2, step: f32, dot: f32, col: [f32; 4]) {
        let n = (a.distance(b) / step).floor() as usize;
        for k in 0..=n {
            self.disc(a.lerp(b, k as f32 / n.max(1) as f32), dot, col);
        }
    }

    /// A soft glow (the atlas's soft dot) `size` across.
    pub fn glow(&mut self, c: Vec2, size: f32, col: [f32; 4]) {
        let (a, b) = (p(c) - Vec2::splat(size / 2.0), p(c) + Vec2::splat(size / 2.0));
        self.area += size * size;
        self.m.region_box(&self.soft, a, b, 0, false, col);
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

    /// A dashed circle: dashes about `dash` px long, `gap` px apart, `half` px either side of the circle.
    pub fn dashed_circle(&mut self, centre: Vec2, r: f32, dash: f32, gap: f32, half: f32, col: [f32; 4]) {
        let n = (std::f32::consts::TAU * r / (dash + gap)).ceil().max(1.0) as usize;
        let turn = std::f32::consts::TAU / n as f32;
        let on = turn * dash / (dash + gap);
        for k in 0..n {
            let pts: Vec<Vec2> = (0..=3).map(|i| centre + Vec2::from_angle(k as f32 * turn + on * i as f32 / 3.0) * r).collect();
            self.polyline(&pts, half, col);
        }
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

/// A new signal's pulses around `at`: two rings running out from an open voyage's ring, fading as they go (`t`
/// the pulse's phase, 0 to 1), over a glow; `f` fades a colour towards the panel.
fn pulses(pen: &mut Pen, at: Vec2, t: f32, glow: bool, tone: Color, f: impl Fn([f32; 4]) -> [f32; 4]) {
    if glow {
        pen.glow(at, 44.0, f(vcol(tone, 0.4)));
    }
    for k in [t, (t + 0.5).fract()] {
        let r = OPEN_R + (PULSE_R - OPEN_R) * k;
        pen.ring(at, r, 1.0, f(vcol(tone, 0.95 * (1.0 - k))));
    }
}

/// The areas' mesh (see the module doc), each galaxy point placed on the stage by `to`, `zoom` the view's.
pub fn build_areas(geo: &Geometry, look: &Look, to: impl Fn(Vec2) -> Vec2, zoom: f32, style: &Style, white: &Region, soft: &Region) -> Mesh {
    let ui = &style.ui;
    let mut pen = Pen::new(white, soft);
    let known = &look.known;

    // The jump range: the union of the circles round home and every charted cluster, its edge only (a run of the
    // circle is drawn where no other circle covers it), a crisp line over a glow fading inwards.
    let r = known.reach * PX_PER_S * zoom;
    let circles: Vec<Vec2> = known.starts.iter().filter_map(|k| geo.clusters.get(*k as usize)).map(|g| to(*g)).collect();
    if r > 0.0 {
        // The glows run along the edges of the unions of smaller circles: they meet in the same kind of corners as
        // the line, so nothing overhangs where two circles cross.
        for (inset, half, a) in [(0.0, 0.7, 0.2), (2.5, 2.0, 0.07), (7.0, 4.5, 0.035)] {
            for (outline, closed) in union_outline(&circles, r - inset, 6.0) {
                for (run, closed) in left_of(&outline, closed, PANEL_X - EDGE_FADE.0) {
                    pen.path(&run, closed, half, vcol(ui.teal, a));
                }
            }
        }
    }

    // Old edges of narrowed signals, fading.
    for (area, faded) in &look.fading {
        let k = 1.0 - faded.clamp(0.0, 1.0);
        dotted_edge(&mut pen, to(geo.to_galaxy(area.centre)), geo.px(area.radius) * zoom, 0.9, vcol(ui.gold, 0.6 * k));
    }
    // The signal areas: a faint fill, a dotted edge, a pulse running out to it.
    let wave = look.wave;
    for m in known.signals.iter().filter(|m| m.is_area()) {
        let c = to(geo.to_galaxy(m.area.centre));
        let r = geo.px(m.area.radius) * zoom;
        let lit = look.selected == Some(Pick::Signal(m.signal)) || look.hovered == Some(Hover::Signal(m.signal));
        pen.disc_n(c, r, ((r * 0.5) as usize).clamp(32, 128), vcol(ui.gold, if lit { 0.07 } else { 0.04 }));
        let ease = 1.0 - (1.0 - wave) * (1.0 - wave);
        if r * ease > 2.0 {
            pen.ring(c, r * ease, 0.9, vcol(ui.gold, 0.32 * (1.0 - wave)));
        }
        dotted_edge(&mut pen, c, r, if lit { 1.25 } else { 1.0 }, vcol(ui.gold, if lit { 0.95 } else { 0.7 }));
    }
    pen.into_mesh()
}

/// The outline of the union of the circles of radius `r` round `centres` (the jump range): for each circle the arcs
/// no other circle covers (cut where two circles cross, kept when their middle lies outside every other circle),
/// joined end to end into loops that turn exactly at the crossings; points about `step` px apart, each arc's ends
/// exact, so the outline holds still while the view zooms. Each loop with whether it closed (it always should).
pub fn union_outline(centres: &[Vec2], r: f32, step: f32) -> Vec<(Vec<Vec2>, bool)> {
    use std::f32::consts::TAU;
    if r <= 0.0 {
        return Vec::new();
    }
    // A circle on top of another (a cluster charted twice over) is left to the first of them.
    let same = |a: Vec2, b: Vec2| a.distance_squared(b) < 1e-6;
    let circles: Vec<Vec2> = centres.iter().enumerate().filter(|(i, c)| !centres[..*i].iter().any(|o| same(*o, **c))).map(|(_, c)| *c).collect();
    // (circle, from angle, to angle), counter-clockwise.
    let mut arcs: Vec<(usize, f32, f32)> = Vec::new();
    for (i, &c) in circles.iter().enumerate() {
        let mut cuts: Vec<f32> = Vec::new();
        for (j, &o) in circles.iter().enumerate() {
            let d = o.distance(c);
            if j != i && d < 2.0 * r {
                let (phi, alpha) = ((o - c).to_angle(), (d / (2.0 * r)).acos());
                cuts.extend([(phi - alpha).rem_euclid(TAU), (phi + alpha).rem_euclid(TAU)]);
            }
        }
        let outside = |a: f32| {
            let q = c + Vec2::from_angle(a) * r;
            circles.iter().enumerate().all(|(j, o)| j == i || o.distance(q) > r)
        };
        if cuts.is_empty() {
            arcs.push((i, 0.0, TAU));
            continue;
        }
        cuts.sort_by(f32::total_cmp);
        for k in 0..cuts.len() {
            let (a0, a1) = (cuts[k], if k + 1 < cuts.len() { cuts[k + 1] } else { cuts[0] + TAU });
            if a1 - a0 > 1e-6 && outside((a0 + a1) / 2.0) {
                arcs.push((i, a0, a1));
            }
        }
    }
    let end = |&(i, _, a1): &(usize, f32, f32)| circles[i] + Vec2::from_angle(a1) * r;
    let start = |&(i, a0, _): &(usize, f32, f32)| circles[i] + Vec2::from_angle(a0) * r;
    let close = (r * 1e-3).max(1e-3);
    let mut used = vec![false; arcs.len()];
    let mut out = Vec::new();
    for first in 0..arcs.len() {
        if used[first] {
            continue;
        }
        used[first] = true;
        let (mut pts, mut at, mut closed) = (Vec::new(), first, false);
        loop {
            let (i, a0, a1) = arcs[at];
            let n = (((a1 - a0) * r / step).ceil() as usize).max(2);
            pts.extend((0..n).map(|k| circles[i] + Vec2::from_angle(a0 + (a1 - a0) * k as f32 / n as f32) * r));
            let here = end(&arcs[at]);
            if a1 - a0 >= TAU - 1e-6 || start(&arcs[first]).distance(here) < close {
                closed = true;
                break;
            }
            let next = (0..arcs.len()).filter(|&k| !used[k]).map(|k| (start(&arcs[k]).distance(here), k)).min_by(|a, b| a.0.total_cmp(&b.0));
            match next {
                Some((d, k)) if d < close => {
                    used[k] = true;
                    at = k;
                }
                _ => {
                    pts.push(here);
                    break;
                }
            }
        }
        out.push((pts, closed));
    }
    out
}

/// The runs of the line through `pts` (back to the first when `closed`) left of `x`, cut exactly at `x`: the line
/// itself when it lies wholly left of it.
fn left_of(pts: &[Vec2], closed: bool, x: f32) -> Vec<(Vec<Vec2>, bool)> {
    let Some(right) = pts.iter().position(|q| q.x >= x) else {
        return vec![(pts.to_vec(), closed)];
    };
    // A closed line starts at a point right of `x`, so every run left of it has both its ends on `x`.
    let walk: Vec<Vec2> = if closed { pts[right..].iter().chain(&pts[..=right]).copied().collect() } else { pts.to_vec() };
    let cut = |a: Vec2, b: Vec2| a.lerp(b, (x - a.x) / (b.x - a.x));
    let mut runs = Vec::new();
    let mut run: Vec<Vec2> = Vec::new();
    for (k, &q) in walk.iter().enumerate() {
        let prev = k.checked_sub(1).map(|j| walk[j]);
        if q.x < x {
            if let Some(a) = prev.filter(|a| a.x >= x) {
                run.push(cut(a, q));
            }
            run.push(q);
        } else if let Some(a) = prev.filter(|a| a.x < x) {
            run.push(cut(a, q));
            runs.push((std::mem::take(&mut run), false));
        }
    }
    if run.len() > 1 {
        runs.push((run, false));
    }
    runs.retain(|r| r.0.len() > 1);
    runs
}

/// A dotted circle like the band edges, its dots faded towards the panel.
fn dotted_edge(pen: &mut Pen, c: Vec2, r: f32, dot: f32, col: [f32; 4]) {
    let n = ((std::f32::consts::TAU * r / 7.0).ceil() as usize).max(8);
    for k in 0..n {
        let q = c + Vec2::from_angle(k as f32 / n as f32 * std::f32::consts::TAU) * r;
        let e = edge(q.x);
        if e > 0.0 {
            pen.disc_n(q, dot, 6, faded(col, e));
        }
    }
}

/// The detail's mesh: the known voyages' and clusters' markers, the shipyard, the pick and the warp, each galaxy
/// point placed on the stage by `to`.
pub fn build_detail(geo: &Geometry, look: &Look, to: impl Fn(Vec2) -> Vec2, style: &Style, white: &Region, soft: &Region) -> Mesh {
    let ui = &style.ui;
    let mut pen = Pen::new(white, soft);
    let glow = look.glow;

    // The course to the pick: dots from where its jump starts; the warp's light running along it.
    if let Some((a, b)) = look.course {
        let (a, b) = (to(a), to(b));
        let n = (a.distance(b) / 9.0).floor() as usize;
        for k in 1..n {
            let q = a.lerp(b, k as f32 / n as f32);
            let e = edge(q.x);
            if e > 0.0 {
                pen.disc_n(q, 1.1, 6, faded(vcol(ui.number, 0.45), e));
            }
        }
    }
    if let Some(w) = look.warp {
        let (a, b) = (to(w.from), to(w.to));
        let t = w.t.clamp(0.0, 1.0);
        let head = a.lerp(b, t);
        pen.line(a, head, 1.2, vcol(ui.teal, 0.8));
        if glow {
            pen.glow(head, 46.0, vcol(ui.teal, 0.6));
        }
        pen.disc(head, 3.5, vcol(ui.number, 1.0));
        // Rings closing in on the target as the warp nears its end.
        for k in [t, (t + 0.5).fract()] {
            pen.ring(b, 30.0 * (1.0 - k) + 6.0, 0.8, vcol(ui.teal, 0.8 * k));
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

    // The known clusters: charted a small teal ring, a located signal's gold, an opened gate a coral diamond.
    for m in &look.known.clusters {
        let Some(&g) = geo.clusters.get(m.cluster as usize) else { continue };
        let at = to(g);
        let k = edge(at.x);
        if k <= 0.0 {
            continue;
        }
        let f = |col| faded(col, k);
        let tone = if m.gate { ui.coral } else if m.signals.is_empty() { ui.teal } else { ui.gold };
        if look.hovered == Some(Hover::Cluster(m.cluster)) && look.selected != Some(Pick::Cluster(m.cluster)) {
            pen.ring(at, 14.0, 0.6, f(vcol(ui.number, 0.35)));
        }
        if glow {
            pen.glow(at, 30.0, f(vcol(tone, 0.3)));
        }
        if m.gate {
            pen.diamond_outline(at, 9.0, 0.9, f(vcol(tone, 0.95)));
        } else {
            pen.ring(at, CLUSTER_R, 0.9, f(vcol(tone, 1.0)));
        }
        pen.disc(at, if m.charted { 2.6 } else { 2.0 }, f(vcol(tone, 1.0)));
        if look.selected == Some(Pick::Cluster(m.cluster)) {
            pen.dotted_ring(at, SELECT_R - 3.0, 24, 1.0, f(vcol(tone, 0.85)));
        }
    }

    // A blind jump's spot: a crosshair in a dotted ring.
    if let Some(Pick::Spot(p)) = &look.selected {
        let at = to(geo.to_galaxy(*p));
        let k = edge(at.x);
        if k > 0.0 {
            let col = faded(vcol(ui.number, 0.9), k);
            pen.dotted_ring(at, 11.0, 20, 0.9, col);
            for d in [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y] {
                pen.line(at + d * 4.0, at + d * 8.0, 0.7, col);
            }
        }
    }

    // The voyages.
    for (n, node) in geo.nodes.iter().copied().enumerate() {
        let at = to(node);
        let k = edge(at.x);
        if k <= 0.0 {
            continue;
        }
        let f = |col| faded(col, k);
        let state = look.states.get(n).copied().unwrap_or(VoyageState::Unknown);
        if !state.known() {
            continue;
        }
        let guardian = look.guardian.get(n).copied().unwrap_or(false);
        let selected = matches!(&look.selected, Some(Pick::Voyage(id)) if look.voyages.get(n) == Some(id));
        if look.hovered == Some(Hover::Voyage(n)) && !selected {
            pen.ring(at, 16.0, 0.6, f(vcol(ui.number, 0.35)));
        }
        let pulse = look.pulse;
        if guardian {
            if state == VoyageState::Signal {
                pulses(&mut pen, at, pulse, glow, ui.coral, f);
            } else if glow {
                pen.glow(at, 40.0, f(vcol(ui.coral, 0.25)));
            }
            pen.diamond_outline(at, 12.5, 1.0, f(vcol(ui.coral, 0.95)));
            let inner = if matches!(state, VoyageState::Cleared { .. }) { ui.teal } else { ui.coral };
            pen.diamond(at, 5.0, f(vcol(inner, 0.95)));
        } else {
            match state {
                VoyageState::Signal => {
                    pulses(&mut pen, at, pulse, glow, ui.teal, f);
                    pen.ring(at, OPEN_R, 1.1, f(vcol(ui.teal, 1.0)));
                    pen.disc(at, 3.4, f(vcol(ui.teal, 1.0)));
                }
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
                VoyageState::Unknown => {}
            }
        }
        if selected {
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
    // The legend's symbols ([`LEGEND`]'s words).
    let (r0, r1) = (LEGEND_ROWS[0], LEGEND_ROWS[1]);
    let at = |k: usize, r: f32| Vec2::new(LEGEND_COLUMNS[k], r);
    pen.diamond_outline(at(0, r0), 5.5, 0.7, vcol(ui.number, 0.85));
    pen.ring(at(1, r0), 4.5, 0.7, vcol(ui.teal, 1.0));
    pen.disc(at(2, r0), 4.0, vcol(mix(ui.teal, ui.number, 0.25), 1.0));
    pen.ring(at(3, r0), 2.6, 0.7, vcol(ui.teal, 1.0));
    pen.ring(at(3, r0), 5.6, 0.6, vcol(ui.teal, 0.5));
    pen.ring(at(4, r0), 3.6, 0.6, vcol(ui.teal, 1.0));
    pen.disc(at(4, r0), 1.4, vcol(ui.teal, 1.0));
    pen.diamond_outline(at(5, r0), 5.5, 0.7, vcol(ui.coral, 1.0));
    pen.disc(at(0, r1), 3.0, vcol(ui.coral, 1.0));
    pen.disc(at(1, r1), 3.0, vcol(ui.gold, 1.0));
    pen.diamond(at(2, r1), 4.5, vcol(ui.gold, 1.0));
    for k in 0..7 {
        let q = at(3, r1) + Vec2::from_angle(std::f32::consts::PI * (1.15 + 0.7 * k as f32 / 6.0)) * 9.0 + Vec2::new(0.0, 6.0);
        pen.disc_n(q, 0.9, 6, vcol(ui.gold, 0.8));
    }
    pen.ring(at(4, r1), 3.6, 0.6, vcol(ui.gold, 1.0));
    pen.disc(at(4, r1), 1.4, vcol(ui.gold, 1.0));
    let arc: Vec<Vec2> = (0..=10).map(|k| at(5, r1) + Vec2::from_angle(std::f32::consts::PI * (1.15 + 0.7 * k as f32 / 10.0)) * 9.0 + Vec2::new(0.0, 6.0)).collect();
    pen.polyline(&arc, 0.8, vcol(ui.teal, 0.6));

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

    /// The detail fades out over the last px before the panel.
    #[test]
    fn the_detail_stops_at_the_panel() {
        assert_eq!((edge(PANEL_X - 100.0), edge(PANEL_X - EDGE_FADE.0)), (1.0, 0.0));
        assert!((edge(PANEL_X - (EDGE_FADE.0 + EDGE_FADE.1) / 2.0) - 0.5).abs() < 1e-5);
    }

    /// The jump range's outline: two crossing circles make one loop that turns exactly at both crossings, a third
    /// apart its own; a circle inside the others' union adds nothing; the panel's edge cuts a loop into runs.
    #[test]
    fn the_range_outline_is_the_union_s_edge() {
        let r = 100.0;
        let (a, b, far) = (Vec2::ZERO, Vec2::new(120.0, 0.0), Vec2::new(500.0, 0.0));
        let loops = union_outline(&[a, b, far], r, 6.0);
        assert_eq!(loops.len(), 2);
        assert!(loops.iter().all(|(_, closed)| *closed));
        let pair = loops.iter().find(|(pts, _)| pts.iter().any(|q| q.x < 0.0)).unwrap();
        // Every point lies on the union's edge: on one circle, outside the other.
        for q in &pair.0 {
            let (da, db) = (q.distance(a), q.distance(b));
            assert!(((da - r).abs() < 1e-3 && db >= r - 1e-3) || ((db - r).abs() < 1e-3 && da >= r - 1e-3), "{q}");
        }
        // Both crossings are corners of the loop.
        let h = (r * r - 60.0f32 * 60.0).sqrt();
        for crossing in [Vec2::new(60.0, h), Vec2::new(60.0, -h)] {
            assert!(pair.0.iter().any(|q| q.distance(crossing) < 1e-3), "{crossing}");
        }
        // Points no further apart than the step, the last back to the first included.
        let n = pair.0.len();
        assert!((0..n).all(|k| pair.0[k].distance(pair.0[(k + 1) % n]) <= 6.0 + 1e-3));
        // A circle in the middle of three adds nothing; the same circle twice is one.
        let round = [Vec2::new(60.0, 0.0), Vec2::new(0.0, 60.0), Vec2::new(-60.0, 0.0), Vec2::new(0.0, -60.0)];
        let inner = union_outline(&[round.as_slice(), &[a, a]].concat(), r, 6.0);
        assert_eq!(inner.len(), 1);
        assert!(inner[0].1 && inner[0].0.iter().all(|q| q.length() > r - 1e-3));
        // Cut at x = 50: one open run of the pair's loop, ending on the cut both ways.
        let runs = left_of(&pair.0, true, 50.0);
        assert_eq!(runs.len(), 1);
        let run = &runs[0].0;
        assert!(!runs[0].1 && (run[0].x - 50.0).abs() < 1e-3 && (run[run.len() - 1].x - 50.0).abs() < 1e-3);
        assert!(run.iter().all(|q| q.x <= 50.0 + 1e-3));
    }

    /// The pulse runs smoothly (a new phase every frame, not in steps) while a signal shows and stands still at 0
    /// otherwise.
    #[test]
    fn the_pulse_runs_only_for_a_signal() {
        let (open, signal) = ([VoyageState::Open, VoyageState::Unknown], [VoyageState::Open, VoyageState::Signal]);
        assert_eq!(Look::pulse_at(&open, 0.7), 0.0);
        assert_eq!(Look::pulse_at(&signal, 0.0), 0.0);
        assert!((Look::pulse_at(&signal, PULSE_PERIOD / 2.0) - 0.5).abs() < 1e-5);
        // Two frames at 144 fps show two phases.
        assert!(Look::pulse_at(&signal, 1.0 + 1.0 / 144.0) > Look::pulse_at(&signal, 1.0));
        assert!((0..400).all(|k| (0.0..1.0).contains(&Look::pulse_at(&signal, k as f32 * 0.037))));
    }
}
