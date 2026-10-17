//! The cluster drawn small, shared by the cluster map (N, GDR-0097) and the minimap (GDR-0106): one painter, two
//! frames. A [`Chart`] says where on screen a point of the cluster lands (north up: the camera never turns) and which
//! disc it is cut to; [`paint`] draws what the fleet knows into it, in the HUD's vector look, faded with the HUD:
//!
//! * the cluster's edge a dotted ring, the entry a small diamond;
//! * the rocks spawned so far (the fields the fleet came near, GDR-0104) as faint specks, a field a faint shape;
//! * every scan of the voyage a faint violet circle of what it swept; the bearings dashed gold lines from where they
//!   were taken (two crossing mark the place), until their site is found;
//! * every site found a dot in its marker's colour in a ring (a signal's site, gold, a ring more);
//! * how far each vessel's sensors reach, a faint dotted teal ring;
//! * contacts within the fleet's sensors (`sim::Voyage::sensors`, GDR-0095): enemies coral, dark or dormant vessels
//!   of another side grey, wrecks (salvage of at least `minimap.wreck_parts` parts) gold; a ship of a site not yet
//!   found and a sleeping site's ships stay hidden (the site's ring stands for them);
//! * the fleet: a teal dot per vessel, the flown one brighter with a tick along its heading.

use super::*;
use super::markers::site_look;
use std::f32::consts::TAU;

/// A bearing's dashes (screen px).
const DASH: f32 = 7.0;
const GAP: f32 = 5.0;

/// Where the cluster lands on screen: `origin` (cluster metres) at `centre` (logical window px), `scale` px a metre,
/// everything cut to the disc of `clip` px round `centre`.
#[derive(Clone, Copy)]
pub(super) struct Chart {
    pub centre: Vec2,
    pub origin: Vec2,
    pub scale: f32,
    pub clip: f32,
}

impl Chart {
    /// A cluster point (y up) on screen (y down).
    pub fn at(&self, p: Vec2) -> Vec2 {
        let d = (p - self.origin) * self.scale;
        self.centre + Vec2::new(d.x, -d.y)
    }

    /// Is a screen point within the disc, give or take `pad` px?
    pub fn inside(&self, q: Vec2, pad: f32) -> bool {
        (q - self.centre).length_squared() <= (self.clip + pad).powi(2)
    }
}

/// What the chart draws with: the camera (screen → world, where the queue draws), the regions, the fade.
pub(super) struct Pen<'a> {
    pub rig: &'a CameraRig,
    pub regions: &'a FxRegions,
    pub queue: &'a mut FxQueue,
    pub fade: f32,
}

impl Pen<'_> {
    /// A dot `d` px across at a screen point.
    pub fn dot(&mut self, q: Vec2, d: f32, c: Color) {
        let w = self.rig.screen_to_world(q);
        self.queue.dot(&self.regions.hard, w, d / self.rig.zoom, c.with_alpha(c.alpha() * self.fade), Z_HUD);
    }

    /// A line `w` px wide between screen points.
    pub fn line(&mut self, a: Vec2, b: Vec2, w: f32, c: Color) {
        let (a, b) = (self.rig.screen_to_world(a), self.rig.screen_to_world(b));
        self.queue.line(&self.regions.white, a, b, w / self.rig.zoom, c.with_alpha(c.alpha() * self.fade), Z_HUD);
    }

    /// A ring of `r` px round a screen point in segments, those outside the chart left out.
    pub fn ring(&mut self, chart: &Chart, at: Vec2, r: f32, w: f32, c: Color) {
        let m = ((TAU * r / 6.0) as usize).clamp(24, 160);
        let p = |j: usize| at + Vec2::from_angle(j as f32 / m as f32 * TAU) * r;
        for k in 0..m {
            let (a, b) = (p(k), p(k + 1));
            if chart.inside(a, 0.0) && chart.inside(b, 0.0) {
                self.line(a, b, w, c);
            }
        }
    }

    /// A ring of dots every `step` px, those outside the chart left out.
    pub fn dotted(&mut self, chart: &Chart, at: Vec2, r: f32, step: f32, d: f32, c: Color) {
        let n = ((TAU * r / step) as usize).max(24);
        for k in 0..n {
            let q = at + Vec2::from_angle(k as f32 / n as f32 * TAU) * r;
            if chart.inside(q, 0.0) {
                self.dot(q, d, c);
            }
        }
    }
}

/// What the fleet knows of the cluster, drawn into the chart (see the module doc).
#[allow(clippy::too_many_arguments)]
pub(super) fn paint(
    chart: &Chart,
    pen: &mut Pen,
    world: &SimWorld,
    v: &sim::Voyage,
    scans: &[(Vec2, f32)],
    flown: Option<ShipId>,
    wreck_parts: usize,
    ui: &kit::style::UiPalette,
) {
    // The edge, the entry.
    pen.dotted(chart, chart.at(Vec2::ZERO), v.radius() * chart.scale, 8.0, 2.2, ui.label.with_alpha(0.5));
    let entry = chart.at(v2(v.entry().pos));
    if chart.inside(entry, 0.0) {
        for (a, b) in [(Vec2::Y, Vec2::X), (Vec2::X, Vec2::NEG_Y), (Vec2::NEG_Y, Vec2::NEG_X), (Vec2::NEG_X, Vec2::Y)] {
            pen.line(entry + a * 5.0, entry + b * 5.0, 1.2, ui.number.with_alpha(0.7));
        }
    }

    // The rocks the fleet came near: specks at least 1.8 px across, a field a faint shape.
    for r in v.rocks() {
        let q = chart.at(v2(r.pos));
        if chart.inside(q, 0.0) {
            pen.dot(q, (2.0 * r.radius * chart.scale).max(1.8), ui.label.with_alpha(0.4));
        }
    }

    // What the scans swept.
    for &(at, range) in scans {
        pen.ring(chart, chart.at(at), range * chart.scale, 1.0, ui.violet.with_alpha(0.35));
    }
    // Bearings, dashed to the edge of the chart (a little past it).
    let pad = 12.0;
    for b in v.bearings() {
        if v.sites().get(b.site as usize).is_some_and(|x| x.known()) {
            continue;
        }
        let (a, d) = (chart.at(v2(b.from)), v2(b.dir));
        let d = Vec2::new(d.x, -d.y);
        // Where the ray runs inside the disc: |a + d t − centre| ≤ clip + pad.
        let o = a - chart.centre;
        let (h, c) = (o.dot(d), o.length_squared() - (chart.clip + pad).powi(2));
        let disc = h * h - c;
        if disc < 0.0 {
            continue;
        }
        let (t0, t1) = ((-h - disc.sqrt()).max(0.0), -h + disc.sqrt());
        let mut t = (t0 / (DASH + GAP)).floor() * (DASH + GAP);
        while t < t1 {
            let (p, q) = (a + d * t.max(t0), a + d * (t + DASH).min(t1));
            if t + DASH > t0 {
                pen.line(p, q, 1.6, ui.gold.with_alpha(0.8));
            }
            t += DASH + GAP;
        }
        if chart.inside(a, 0.0) {
            pen.dot(a, 5.0, ui.gold.with_alpha(0.9));
        }
    }
    // The sites found.
    for site in v.sites().iter().filter(|x| x.known()) {
        let at = chart.at(v2(site.pos()));
        if !chart.inside(at, 0.0) {
            continue;
        }
        let (c, a, _) = site_look(&world.content, site, ui);
        pen.dot(at, 6.0, c.with_alpha(a));
        let rings: &[f32] = if site.signal() { &[9.0, 13.0] } else { &[9.0] };
        for &rr in rings {
            pen.ring(chart, at, rr, 1.0, c.with_alpha(0.7 * a));
        }
    }

    // How far the fleet's sensors reach, then the contacts within.
    let sensors = v.sensors();
    for &(at, r) in sensors {
        pen.dotted(chart, chart.at(v2(at)), r * chart.scale, 6.0, 1.3, ui.teal.with_alpha(0.3));
    }
    let seen = |p: Vec2| sensors.iter().any(|&(q, r)| (v2(q) - p).length_squared() <= r * r);
    let found = |id: ShipId| v.site_of(id).is_none_or(|k| v.sites()[k].known()) && !v.asleep(id);
    for s in world.sim.ships() {
        let mine = s.owner() == crate::scene::PLAYER;
        let control = s.control_piece();
        for p in s.pieces() {
            let vessel = control == Some(p.id);
            if (mine && vessel) || (!vessel && p.parts.len() < wreck_parts) {
                continue;
            }
            let at = piece_com(p, 1.0);
            let q = chart.at(at);
            if !chart.inside(q, 0.0) || !seen(at) || !found(s.id()) {
                continue;
            }
            let (c, d) = match (vessel, p.active) {
                (true, true) => (ui.coral.with_alpha(0.95), 4.5),
                (true, false) => (ui.label.with_alpha(0.85), 4.0),
                (false, _) => (ui.gold.with_alpha(0.7), 3.0),
            };
            pen.dot(q, d, c);
        }
    }

    // The fleet.
    for vessel in world.sim.vessels(crate::scene::PLAYER) {
        let Some(ship) = world.sim.ship(vessel.ship) else { continue };
        let Some(p) = ship.control_piece().and_then(|c| ship.piece(c)) else { continue };
        let at = chart.at(piece_com(p, 1.0));
        if !chart.inside(at, 0.0) {
            continue;
        }
        let me = flown == Some(vessel.ship);
        pen.dot(at, if me { 7.0 } else { 5.0 }, ui.teal.with_alpha(if me { 1.0 } else { 0.75 }));
        if me {
            let (_, angle) = p.render_pose(1.0);
            let h = Vec2::from_angle(angle + std::f32::consts::FRAC_PI_2);
            pen.line(at, at + Vec2::new(h.x, -h.y) * 11.0, 1.4, ui.teal);
        }
    }
}
