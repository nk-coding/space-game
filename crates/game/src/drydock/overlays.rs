//! The drydock's overlays (the OVERLAY pills: HEAT, MASS, BONUS, SHIELDS, ARCS, any combination) and the
//! issue marks, drawn over the design in world space from the design report ([`DrydockReport`]).
//!
//! One mesh (atlas material, vertex colours), a child of the design's mesh so it shows and hides with it, above
//! the tiles and below the frame's veil and hover cell. It is rebuilt only when the report is recomputed or an
//! overlay is toggled, never per frame; widths are in cells, so it zooms with the ship. Report positions are
//! blueprint frame = frame cells (y down); the world has blueprint `(x, y)` at `(x, −y)` (`view.rs`).
//!
//! * **Heat**: each part of the main piece tinted by its steady heat at full use over its limit, teal (cool) →
//!   gold (at the limit) → coral (at [`RUNS_HOT_MAX`]); parts over their limit outlined (gold while they only
//!   run hot, coral above).
//! * **Mass**: each part of the main piece shaded by its mass per cell, the centre of mass (the quartered
//!   disc), the forward thrust line through the thrust's centre, and when it misses the centre of mass a curved
//!   arrow showing which way full thrust turns the ship (gold if the gyros and gimbals hold it, coral if not).
//! * **Bonus**: a gold link across the shared side of every source and target pair, a diamond on the side.
//! * **Shields**: each bubble (centre, radius after bonuses) as a faint disc with a soft rim in the shield colour.
//! * **Arcs**: each gun's line of fire to its range (clear: dashed, weapons cyan; blocked: coral up to the first
//!   blocker, a cross there, blockers outlined); each engine's plume lanes to their reach (propulsion teal; a lane
//!   that burns an own part: coral, the part outlined).
//! * **Issue marks**, always (overlays on or off): a small triangle in the top-right corner of every part an issue
//!   names: coral for blocked guns, parts a plume burns, parts that fall off and parts overheating past
//!   [`RUNS_HOT_MAX`]; gold for parts that only run hot (over their limit, up to it).

use std::collections::BTreeSet;

use bevy::prelude::*;
use data::{Cell, Content, Dir, PlacedShape};
use sim::DesignReport;
use sim::report::Issue;

use crate::atlas::{Atlas, Region};
use crate::meshbuf::{MeshBuf, ensure_ccw, inset_convex};
use crate::sim_bridge::GameContent;
use crate::style::{Style, UiPalette, vcol};
use crate::ui_kit::Tone;

use super::PlayerDesign;
use super::report::DrydockReport;
use super::state::{DrydockUi, Overlay};
use super::stats::{RUNS_HOT_MAX, yaw_pull};
use super::view::DockShip;

/// Above the design's tiles (relative to its mesh), below the frame's veil and hover cell (FX layer at z 400).
const Z_OVERLAY: f32 = 30.0;

/// The overlay mesh and the overlay pills it was built for.
#[derive(Component)]
pub(super) struct DockOverlay {
    built: Option<u8>,
}

pub(super) fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, atlas: Res<Atlas>, ship: Single<Entity, With<DockShip>>) {
    commands.entity(*ship).with_child((
        DockOverlay { built: None },
        Mesh2d(meshes.add(MeshBuf::placeholder())),
        MeshMaterial2d(atlas.material.clone()),
        Transform::from_xyz(0.0, 0.0, Z_OVERLAY),
    ));
}

/// Rebuilds the overlay mesh when the report is recomputed or the overlay pills change.
#[allow(clippy::too_many_arguments)]
pub(super) fn rebuild(
    report: Res<DrydockReport>,
    design: Res<PlayerDesign>,
    ui: Res<DrydockUi>,
    content: Res<GameContent>,
    atlas: Res<Atlas>,
    style: Res<Style>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut overlay: Single<(&mut DockOverlay, &Mesh2d)>,
) {
    let bits = Overlay::ALL.iter().enumerate().filter(|(_, o)| ui.overlay(**o)).fold(0u8, |b, (k, _)| b | 1 << k);
    let (dock, mesh) = &mut *overlay;
    if (!report.is_changed() && dock.built == Some(bits)) || report.revision != Some(design.revision()) {
        return;
    }
    dock.built = Some(bits);
    let shapes = design.dock().shapes();
    let mut m = MeshBuf::default();
    if let Some(r) = report.get()
        && r.parts.len() == shapes.len()
    {
        let content = &content.0;
        let masses: Vec<f32> = design
            .blueprint()
            .parts
            .iter()
            .map(|p| content.parts.get(&p.part).and_then(|d| d.mass_in(content.placement_armor(p))).unwrap_or(0.0))
            .collect();
        let pen = Pen::new(&atlas, &style);
        let on = |o: Overlay| ui.overlay(o);
        draw(&mut m, &pen, r, shapes, &masses, on);
    }
    let new = if m.is_empty() { MeshBuf::placeholder() } else { m.into_mesh() };
    let _ = meshes.insert(&mesh.0, new);
}

/// Colours and atlas regions of the overlays.
struct Pen {
    /// The flat white texel (fills) and region (strips).
    white: Vec2,
    white_region: Region,
    halo: Region,
    ui: UiPalette,
    weapons: Color,
    propulsion: Color,
    shield: Color,
    shield_core: Color,
    ink: Color,
}

impl Pen {
    fn new(atlas: &Atlas, style: &Style) -> Self {
        let white_region = atlas.region("fx/white");
        Self {
            white: white_region.uv_center(),
            white_region,
            halo: atlas.region("fx/halo"),
            ui: style.ui,
            weapons: style.system_color("weapons"),
            propulsion: style.system_color("propulsion"),
            shield: style.shield.color,
            shield_core: style.shield.core,
            ink: style.seam,
        }
    }

    fn tone(&self, t: Tone) -> Color {
        t.color(&self.ui)
    }

    /// A straight segment of half width `hw`.
    fn seg(&self, m: &mut MeshBuf, a: Vec2, b: Vec2, hw: f32, c: Color) {
        let d = (b - a).normalize_or_zero();
        if d == Vec2::ZERO {
            return;
        }
        let n = Vec2::new(-d.y, d.x) * hw;
        m.quad_uv([a + n, b + n, b - n, a - n], [self.white; 4], vcol(c, 1.0));
    }

    /// Dashes from `a` to `b`, starting with a dash.
    fn dashed(&self, m: &mut MeshBuf, a: Vec2, b: Vec2, dash: f32, gap: f32, hw: f32, c: Color) {
        let len = a.distance(b);
        let d = (b - a) / len.max(1e-6);
        let mut s = 0.0;
        while s < len {
            self.seg(m, a + d * s, a + d * (s + dash).min(len), hw, c);
            s += dash + gap;
        }
    }

    /// A soft strip (the halo region across it) from `a` to `b`.
    fn soft(&self, m: &mut MeshBuf, a: Vec2, b: Vec2, hw: f32, c: Color) {
        m.strip(&[a, b], false, hw, &self.halo, vcol(c, 1.0));
    }

    /// A cross of half size `s` at `p`.
    fn cross(&self, m: &mut MeshBuf, p: Vec2, s: f32, hw: f32, c: Color) {
        self.seg(m, p + Vec2::new(-s, -s), p + Vec2::new(s, s), hw, c);
        self.seg(m, p + Vec2::new(-s, s), p + Vec2::new(s, -s), hw, c);
    }

    fn fill(&self, m: &mut MeshBuf, pts: &[Vec2], c: Color) {
        m.polygon(pts, self.white, vcol(c, 1.0));
    }

    /// A closed outline of half width `hw`.
    fn outline(&self, m: &mut MeshBuf, pts: &[Vec2], hw: f32, c: Color) {
        m.strip(pts, true, hw, &self.white_region, vcol(c, 1.0));
    }

    /// An open polyline of half width `hw`.
    fn polyline(&self, m: &mut MeshBuf, pts: &[Vec2], hw: f32, c: Color) {
        m.strip(pts, false, hw, &self.white_region, vcol(c, 1.0));
    }
}

/// Blueprint frame (y down) to world (y up).
fn world(p: [f32; 2]) -> Vec2 {
    Vec2::new(p[0], -p[1])
}

fn dir_world(d: Dir) -> Vec2 {
    let (x, y) = d.delta();
    Vec2::new(x as f32, -y as f32)
}

/// A part's outline in world space, counter-clockwise, pulled in by `inset` (so the tile seams stay visible).
fn outline_of(shape: &PlacedShape, inset: f32) -> Vec<Vec2> {
    let mut pts: Vec<Vec2> = shape.polygon().iter().map(|g| Vec2::new(g.x as f32, -g.y as f32)).collect();
    ensure_ccw(&mut pts);
    if inset > 0.0 { inset_convex(&pts, inset) } else { pts }
}

fn circle(c: Vec2, r: f32) -> Vec<Vec2> {
    let n = ((r * 14.0) as usize).clamp(40, 160);
    (0..n).map(|k| c + Vec2::from_angle(k as f32 / n as f32 * std::f32::consts::TAU) * r).collect()
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_srgba(), b.to_srgba());
    let t = t.clamp(0.0, 1.0);
    Color::srgba(a.red + (b.red - a.red) * t, a.green + (b.green - a.green) * t, a.blue + (b.blue - a.blue) * t, a.alpha + (b.alpha - a.alpha) * t)
}

/// The heat overlay's tint for a part at `ratio` = heat / limit at full use: teal and faint when cool, gold at
/// the limit, coral from [`RUNS_HOT_MAX`] on, stronger the hotter.
pub fn heat_tint(ratio: f32, ui: &UiPalette) -> Color {
    let r = ratio.max(0.0);
    if r <= 1.0 {
        mix(ui.teal, ui.gold, r).with_alpha(0.1 + 0.32 * r)
    } else if r <= RUNS_HOT_MAX {
        let t = (r - 1.0) / (RUNS_HOT_MAX - 1.0);
        mix(ui.gold, ui.coral, t).with_alpha(0.42 + 0.13 * t)
    } else {
        ui.coral.with_alpha(0.6)
    }
}

/// The parts the issues name and the tone of their mark (coral wins over gold), in part order.
pub fn issue_marks(r: &DesignReport) -> Vec<(usize, Tone)> {
    let mut coral = BTreeSet::new();
    let mut gold = BTreeSet::new();
    for i in &r.issues {
        match i {
            Issue::GunBlocked { gun, .. } => {
                coral.insert(*gun as usize);
            }
            Issue::PlumeBurnsOwnPart { parts, .. } | Issue::FallsOff { parts } => coral.extend(parts.iter().map(|&p| p as usize)),
            Issue::Overheat { parts } => {
                for &p in parts {
                    let hot = r.parts.get(p as usize).is_some_and(|q| q.heat_ratio > RUNS_HOT_MAX);
                    (if hot { &mut coral } else { &mut gold }).insert(p as usize);
                }
            }
            _ => {}
        }
    }
    let mut out: Vec<(usize, Tone)> = coral.iter().map(|&p| (p, Tone::Coral)).collect();
    out.extend(gold.difference(&coral).map(|&p| (p, Tone::Gold)));
    out.sort_by_key(|m| m.0);
    out
}

/// The two neighbouring cells where parts `a` and `b` touch: the pair nearest the middle between their centres.
fn link_cells(shapes: &[PlacedShape], a: usize, b: usize) -> Option<(Cell, Cell)> {
    let theirs: BTreeSet<Cell> = shapes[b].cells().into_iter().collect();
    let [ax, ay] = shapes[a].center();
    let [bx, by] = shapes[b].center();
    let mid = Vec2::new(ax + bx, ay + by) / 2.0;
    let mut best: Option<((Cell, Cell), f32)> = None;
    for c in shapes[a].cells() {
        for d in Dir::ALL {
            let (dx, dy) = d.delta();
            let n = (c.0 + dx, c.1 + dy);
            if theirs.contains(&n) {
                let at = Vec2::new(c.0 as f32 + n.0 as f32 + 1.0, c.1 as f32 + n.1 as f32 + 1.0) / 2.0;
                let k = at.distance_squared(mid);
                if best.is_none_or(|b| k < b.1) {
                    best = Some(((c, n), k));
                }
            }
        }
    }
    best.map(|b| b.0)
}

/// How far along a gun's line of fire (from the muzzle) the first blocker starts.
fn first_block(shapes: &[PlacedShape], muzzle: Vec2, dir: Vec2, blockers: &[sim::PartIdx]) -> Option<f32> {
    blockers
        .iter()
        .filter_map(|&b| shapes.get(b as usize))
        .flat_map(|s| outline_of(s, 0.0))
        .map(|p| (p - muzzle).dot(dir).max(0.0))
        .min_by(f32::total_cmp)
}

/// Everything the overlays `on` show, plus the issue marks, bottom to top.
fn draw(m: &mut MeshBuf, pen: &Pen, r: &DesignReport, shapes: &[PlacedShape], masses: &[f32], on: impl Fn(Overlay) -> bool) {
    let ui = &pen.ui;
    let main_part = |i: usize| r.main == Some(r.parts[i].piece);

    if on(Overlay::Heat) {
        for (i, s) in shapes.iter().enumerate().filter(|(i, _)| main_part(*i)) {
            pen.fill(m, &outline_of(s, 0.05), heat_tint(r.parts[i].heat_ratio, ui));
        }
    }
    if on(Overlay::Mass) {
        let density = |i: usize| masses[i] / shapes[i].area().max(1e-3);
        let max = (0..shapes.len()).filter(|&i| main_part(i)).map(density).fold(0.0f32, f32::max);
        if max > 0.0 {
            for (i, s) in shapes.iter().enumerate().filter(|(i, _)| main_part(*i)) {
                pen.fill(m, &outline_of(s, 0.05), ui.number.with_alpha(0.04 + 0.3 * density(i) / max));
            }
        }
    }
    if on(Overlay::Shields) {
        for s in &r.shields {
            let c = world(s.centre);
            let ring = circle(c, s.radius);
            pen.fill(m, &ring, pen.shield.with_alpha(0.04));
            m.strip(&ring, true, 0.22, &pen.halo, vcol(pen.shield, 0.3));
            pen.outline(m, &ring, 0.025, mix(pen.shield, pen.shield_core, 0.4).with_alpha(0.6));
            pen.fill(m, &circle(c, 0.09), pen.shield.with_alpha(0.8));
        }
    }
    if on(Overlay::Arcs) {
        for e in &r.engines {
            for l in &e.lanes {
                let (o, d) = (world(l.origin), dir_world(l.dir));
                let end = o + d * l.reach;
                let c = if l.burns.is_some() { ui.coral } else { pen.propulsion };
                pen.soft(m, o, end, 0.3, c.with_alpha(0.3));
                if l.burns.is_some() {
                    pen.seg(m, o, end, 0.03, c.with_alpha(0.9));
                    pen.cross(m, end, 0.16, 0.035, c);
                } else {
                    pen.dashed(m, o, end, 0.3, 0.22, 0.022, c.with_alpha(0.75));
                    pen.seg(m, end - d.perp() * 0.22, end + d.perp() * 0.22, 0.022, c.with_alpha(0.75));
                }
            }
        }
        for g in &r.guns {
            let (o, d) = (world(g.muzzle), dir_world(g.facing));
            if g.clear {
                let end = o + d * g.range;
                pen.dashed(m, o, end, 0.45, 0.3, 0.03, pen.weapons.with_alpha(0.85));
                pen.seg(m, end - d.perp() * 0.3, end + d.perp() * 0.3, 0.03, pen.weapons.with_alpha(0.85));
            } else {
                let t = first_block(shapes, o, d, &g.blockers).unwrap_or(0.0);
                pen.seg(m, o, o + d * t.max(0.15), 0.035, ui.coral.with_alpha(0.95));
                pen.cross(m, o + d * t, 0.16, 0.035, ui.coral);
                for &b in &g.blockers {
                    pen.outline(m, &outline_of(&shapes[b as usize], 0.07), 0.035, ui.coral.with_alpha(0.85));
                }
            }
        }
        for b in r.engines.iter().flat_map(|e| e.lanes.iter().filter_map(|l| l.burns)).collect::<BTreeSet<_>>() {
            pen.outline(m, &outline_of(&shapes[b as usize], 0.07), 0.035, ui.coral.with_alpha(0.85));
        }
    }
    if on(Overlay::Bonus) {
        let pairs: BTreeSet<(usize, usize)> = r.bonuses.iter().map(|b| (b.source as usize, b.target as usize)).map(|(a, b)| (a.min(b), a.max(b))).collect();
        for (a, b) in pairs {
            let Some((ca, cb)) = link_cells(shapes, a, b) else { continue };
            let (pa, pb) = (Vec2::new(ca.0 as f32 + 0.5, -(ca.1 as f32) - 0.5), Vec2::new(cb.0 as f32 + 0.5, -(cb.1 as f32) - 0.5));
            let (mid, d) = ((pa + pb) / 2.0, (pb - pa).normalize_or_zero());
            pen.seg(m, mid - d * 0.34, mid + d * 0.34, 0.035, ui.gold.with_alpha(0.9));
            let diamond = |s: f32| [mid + Vec2::new(0.0, s), mid + Vec2::new(-s, 0.0), mid + Vec2::new(0.0, -s), mid + Vec2::new(s, 0.0)];
            pen.fill(m, &diamond(0.16), pen.ink.with_alpha(0.9));
            pen.fill(m, &diamond(0.11), ui.gold);
        }
    }
    if on(Overlay::Heat) {
        for &p in &r.heat.over {
            let tone = if r.parts[p as usize].heat_ratio > RUNS_HOT_MAX { ui.coral } else { ui.gold };
            pen.outline(m, &outline_of(&shapes[p as usize], 0.07), 0.04, tone.with_alpha(0.9));
        }
    }
    if on(Overlay::Mass) && r.main.is_some() {
        let c = world(r.centre_of_mass);
        let f = &r.flight;
        if f.thrust[0] > 0.0 {
            let x = c.x + f.yaw / f.thrust[0];
            let (bottom, top) = (Vec2::new(x, c.y - 2.2), Vec2::new(x, c.y + 2.4));
            pen.dashed(m, bottom, top, 0.22, 0.16, 0.025, ui.teal.with_alpha(0.8));
            pen.fill(m, &[top + Vec2::new(0.0, 0.3), top + Vec2::new(-0.16, 0.0), top + Vec2::new(0.16, 0.0)], ui.teal.with_alpha(0.9));
        }
        if let Some((pull, held)) = yaw_pull(r)
            && pull != 0
        {
            let col = if held { ui.gold } else { ui.coral };
            let (a0, a1) = (0.22 * std::f32::consts::PI, 0.78 * std::f32::consts::PI);
            let arc: Vec<Vec2> = (0..=16).map(|k| c + Vec2::from_angle(a0 + (a1 - a0) * k as f32 / 16.0) * 0.8).collect();
            pen.polyline(m, &arc, 0.035, col);
            // The head where the turn goes: counter-clockwise (left) at a1, clockwise (right) at a0.
            let (at, tangent) = if pull > 0 { (a1, Vec2::from_angle(a1).perp()) } else { (a0, -Vec2::from_angle(a0).perp()) };
            let tip = c + Vec2::from_angle(at) * 0.8;
            let side = tangent.perp() * 0.13;
            pen.fill(m, &[tip + tangent * 0.26, tip + side, tip - side], col);
        }
        pen.fill(m, &circle(c, 0.3), pen.ink.with_alpha(0.85));
        let quarter = |from: f32| {
            let mut pts = vec![c];
            pts.extend((0..=8).map(|k| c + Vec2::from_angle(from + k as f32 / 8.0 * std::f32::consts::FRAC_PI_2) * 0.22));
            pts
        };
        pen.fill(m, &quarter(0.0), ui.number);
        pen.fill(m, &quarter(std::f32::consts::PI), ui.number);
        pen.outline(m, &circle(c, 0.22), 0.022, ui.number);
        pen.seg(m, c - Vec2::X * 0.42, c + Vec2::X * 0.42, 0.018, ui.number.with_alpha(0.9));
        pen.seg(m, c - Vec2::Y * 0.42, c + Vec2::Y * 0.42, 0.018, ui.number.with_alpha(0.9));
    }
    for (p, tone) in issue_marks(r) {
        let Some(s) = shapes.get(p) else { continue };
        let pts = outline_of(s, 0.0);
        let corner = pts.iter().fold(Vec2::splat(f32::MIN), |a, q| a.max(*q)) - Vec2::splat(0.08);
        pen.fill(m, &[corner, corner - Vec2::new(0.26, 0.0), corner - Vec2::new(0.0, 0.26)], pen.tone(tone).with_alpha(0.95));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drydock::PlayerDesign;

    fn palette() -> UiPalette {
        UiPalette {
            teal: Color::srgb(0.0, 1.0, 0.0),
            gold: Color::srgb(1.0, 1.0, 0.0),
            coral: Color::srgb(1.0, 0.0, 0.0),
            violet: Color::srgb(0.5, 0.0, 1.0),
            label: Color::srgb(0.5, 0.5, 0.5),
            dim: Color::srgb(0.3, 0.3, 0.3),
            number: Color::WHITE,
        }
    }

    #[test]
    fn heat_ramp_runs_cool_gold_coral() {
        let ui = palette();
        let rgb = |c: Color| {
            let s = c.to_srgba();
            [s.red, s.green, s.blue]
        };
        assert_eq!(rgb(heat_tint(0.0, &ui)), [0.0, 1.0, 0.0]);
        assert_eq!(rgb(heat_tint(1.0, &ui)), [1.0, 1.0, 0.0]);
        assert_eq!(rgb(heat_tint(RUNS_HOT_MAX, &ui)), [1.0, 0.0, 0.0]);
        assert_eq!(rgb(heat_tint(3.0, &ui)), [1.0, 0.0, 0.0]);
        assert_eq!(heat_tint(-1.0, &ui), heat_tint(0.0, &ui));
        // Halfway to the limit: between teal and gold. The tint grows stronger with the heat.
        assert_eq!(rgb(heat_tint(0.5, &ui)), [0.5, 1.0, 0.0]);
        let alpha: Vec<f32> = [0.0, 0.5, 1.0, 1.1, RUNS_HOT_MAX, 2.0].iter().map(|&r| heat_tint(r, &ui).alpha()).collect();
        assert!(alpha.windows(2).all(|w| w[0] < w[1] || (w[0] - w[1]).abs() < 1e-6), "{alpha:?}");
    }

    fn runner() -> (Content, PlayerDesign, DesignReport) {
        let content = data::base_content();
        let d = PlayerDesign::starter(&content);
        let r = sim::design_report(&content, d.blueprint(), &sim::SimConfig::default()).unwrap();
        (content, d, r)
    }

    #[test]
    fn the_runner_marks_the_parts_that_run_hot_in_gold() {
        let (_, _, r) = runner();
        let marks = issue_marks(&r);
        let over: Vec<(usize, Tone)> = r.heat.over.iter().map(|&p| (p as usize, Tone::Gold)).collect();
        assert_eq!(marks, over);
        assert_eq!(marks.len(), 4, "both beams and both shield emitters (docs/sim.md)");
    }

    #[test]
    fn coral_issues_win_over_gold() {
        let (_, _, mut r) = runner();
        let hot = r.heat.over[0];
        r.issues.push(Issue::GunBlocked { gun: hot, blockers: vec![0] });
        r.issues.push(Issue::FallsOff { parts: vec![1, 2] });
        let marks = issue_marks(&r);
        assert!(marks.contains(&(hot as usize, Tone::Coral)));
        assert!(!marks.contains(&(hot as usize, Tone::Gold)));
        assert!(marks.contains(&(1, Tone::Coral)) && marks.contains(&(2, Tone::Coral)));
        assert!(marks.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn bonus_links_sit_on_a_shared_side() {
        let (_, d, r) = runner();
        let shapes = d.dock().shapes();
        assert!(!r.bonuses.is_empty());
        for b in &r.bonuses {
            let (a, c) = link_cells(shapes, b.source as usize, b.target as usize).expect("bonus partners touch");
            assert_eq!((a.0 - c.0).abs() + (a.1 - c.1).abs(), 1);
            assert!(shapes[b.source as usize].cells().contains(&a));
            assert!(shapes[b.target as usize].cells().contains(&c));
        }
    }

    #[test]
    fn a_blocked_line_of_fire_ends_at_the_first_blocker() {
        // A 1×1 part at cell (3, 2) blocks a gun firing up from the top of cell (3, 6): the muzzle is at blueprint
        // (3.5, 6), the blocker's near side at y = 3, so 3 m away.
        let content = data::base_content();
        let d = PlayerDesign::starter(&content);
        let mut shape = d.dock().shapes()[0];
        shape.footprint = data::Footprint::rect(1, 1);
        (shape.x, shape.y, shape.rot, shape.flip) = (3, 2, 0, false);
        let far = PlacedShape { y: 0, ..shape };
        let t = first_block(&[shape, far], world([3.5, 6.0]), dir_world(Dir::Up), &[1, 0]).unwrap();
        assert!((t - 3.0).abs() < 1e-5, "{t}");
    }
}
