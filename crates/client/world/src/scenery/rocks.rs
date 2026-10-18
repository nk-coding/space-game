//! The sim's asteroid rocks (GDR-0104), drawn as they spawn, in the backdrop study's approach C (Niklas, 18 Oct 2026:
//! "soft bands, vector detail"; GDR-0104's status line of 18 Oct 2026). The outline is the sim's own
//! (`sim::voyage::rocks::curve` through the rock's control points, the same points it collides with, so hits land on
//! what is drawn): star-shaped round the rock's centre, so it never crosses itself. On it, as on the sky's style-B
//! planets:
//!
//! * five nested **bands** from the shadow to the lit tone, each the outline scaled towards a point on the lit side
//!   with a soft skirt, then a faint **rim light** on the shadow side;
//! * a **ridge** (a lit and a shadowed line), **craters** foreshortened towards the limb (a lip lit towards the
//!   light, a bowl whose near wall is in shadow and far wall lit, a darker floor) and **speckle**;
//! * the kind's **marks** (`RockKind::look`): bright cracks (ice), small pits and ember veins on the shadow side
//!   (slag), a metal glint on the lit side (iron); embers and glints in a small additive mesh ([`Atlas::additive`]);
//! * one **edge line** round the outline as a single closed strip with mitred joins (no gaps or overlaps at corners),
//!   brighter on the sides facing the light.
//!
//! The light comes from the sky ([`Light`]): the hot star in a scorched cluster (from each rock towards it), else the
//! sky's first planet's light, else from the upper left. Colours are the kind's tone in `art/tokens.json`
//! (`world.rock.<tone>`: shadow, mid, lit, edge). Each rock's marks roll from the voyage's seed and its index (the
//! same rock looks the same every time). A rock never moves (an anchored body): the rocks that spawn together (a
//! field, a piece of a belt) are one mesh in world coordinates, drawn in one call, and one additive mesh.
//!
//! **Near rocks** (GDR-0107's status line of 18 Oct 2026, ninth: what you can touch reads apart from the backdrop):
//! a rock near the ship the camera follows gets a faint bright line over its edge, full within `world.rock_near`'s
//! `range_m` of the ship (from the rock's edge), fading out smoothly over the next `fade_m` ([`near_rocks`]: one
//! small additive mesh, rebuilt only when what it shows changes).

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;
use data::RockLook;
use kit::atlas::Atlas;
use kit::meshbuf::MeshBuf;
use kit::rng::Rng;
use kit::style::vcol;
use sim::voyage::rocks::{OUTLINE_POINTS, curve};

use super::{Scenery, SceneryPart, salted};
use crate::camera::CameraRig;
use crate::sim_bridge::{SimWorld, v2};

/// Rocks lie under the trails and every ship (z 5 and up); their glints and embers a hair above them.
const Z_ROCKS: f32 = 3.0;
const Z_GLOW: f32 = 3.01;
/// The near rocks' line, over the glints.
const Z_NEAR: f32 = 3.02;
/// A near rock's strength is kept in this many steps (the mesh is rebuilt when one changes).
const NEAR_STEPS: f32 = 24.0;
/// The edge line's half width (m): about 1.4 px at the starting zoom (18 px/m).
const EDGE: f32 = 0.08;
/// Where the light comes from without a sky (world, unit): the upper left.
const FALLBACK: Vec2 = Vec2::new(-0.6, 0.8);
/// The bands: each the outline scaled to this share towards the lit point, and its colour between the tone's
/// shadow, mid and lit (0, 1, 2); the lit point lies this share of the radius towards the light; a band's soft skirt
/// reaches this share further out.
const BANDS: [(f32, f32); 5] = [(1.0, 0.0), (0.9, 0.65), (0.74, 1.0), (0.53, 1.45), (0.3, 1.8)];
const LIT_POINT: f32 = 0.4;
const SKIRT: f32 = 0.06;
/// The rim light on the shadow side: its strength and how far in it reaches (share of the outline).
const RIM: (f32, f32) = (0.32, 0.9);
/// Craters: at most this many; the share that are large and the sizes (shares of the radius) of the large and the
/// others; how far out they may lie (share of the edge's distance).
const MOST_CRATERS: usize = 7;
const LARGE_CRATERS: f32 = 0.25;
const CRATER_SIZE: [(f32, f32); 2] = [(0.15, 0.23), (0.06, 0.13)];
const CRATER_REACH: f32 = 0.72;
/// A crater's lip reaches this share of its radius out: craters keep their lips apart.
const CRATER_LIP: f32 = 1.24;
/// Speckle: specks a metre of radius, at most, and their size (m: a pixel or two at the starting zoom).
const SPECKS_PER_M: f32 = 3.5;
const MOST_SPECKS: usize = 70;
const SPECK_SIZE: (f32, f32) = (0.05, 0.11);

/// Where the rocks' light comes from (GDR-0104's status line of 18 Oct 2026, Niklas: from the sky).
#[derive(Clone, Copy, Debug)]
pub(super) enum Light {
    /// A hot star at this point of the cluster: each rock is lit from its side towards it.
    Star(Vec2),
    /// From this side (world, unit) everywhere.
    From(Vec2),
}

impl Default for Light {
    fn default() -> Self {
        Light::From(FALLBACK.normalize())
    }
}

impl Light {
    /// The sky's light: its hot star's, else its first planet's (`planet`: the angle its light comes from in the
    /// sky's design frame, y down), else from the upper left.
    pub(super) fn of_sky(star: Option<Vec2>, planet: Option<f32>) -> Light {
        match (star, planet) {
            (Some(s), _) => Light::Star(s),
            (None, Some(a)) => Light::From(Vec2::new(a.cos(), -a.sin())),
            (None, None) => Light::default(),
        }
    }

    /// Where the light comes from at `p` (world, unit).
    fn at(self, p: Vec2) -> Vec2 {
        match self {
            Light::Star(s) => (s - p).normalize_or(FALLBACK.normalize()),
            Light::From(d) => d,
        }
    }
}

/// A rock tone's colours (`world.rock.<tone>`).
struct Tone {
    shadow: Color,
    mid: Color,
    lit: Color,
    edge: Color,
}

impl Tone {
    /// A shade from the shadow (0) through the mid tone (1) to the lit tone (2).
    fn shade(&self, t: f32) -> Color {
        if t <= 1.0 { self.shadow.mix(&self.mid, t.max(0.0)) } else { self.mid.mix(&self.lit, (t - 1.0).min(1.0)) }
    }
}

/// The marks' colours (`world.rock_*`).
struct Marks {
    rim: Color,
    crack: Color,
    glint: Color,
    ember: Color,
    ember_core: Color,
}

/// One rock's outline round its centre (world orientation) and what is worked out from it.
struct Shape {
    pts: Vec<Vec2>,
    /// The angle of the first point; the points go round evenly from it.
    start: f32,
    radius: f32,
}

impl Shape {
    /// The outline's distance from the centre towards `angle`.
    fn reach(&self, angle: f32) -> f32 {
        let n = self.pts.len();
        let f = (angle - self.start).rem_euclid(TAU) / TAU * n as f32;
        let (i, k) = (f as usize % n, f.fract());
        self.pts[i].length() * (1.0 - k) + self.pts[(i + 1) % n].length() * k
    }

    /// The point `share` of the way out towards `angle`.
    fn at(&self, angle: f32, share: f32) -> Vec2 {
        Vec2::from_angle(angle) * self.reach(angle) * share
    }
}

/// Draws the rocks spawned since the last frame.
pub(super) fn draw_rocks(
    mut commands: Commands,
    mut state: ResMut<Scenery>,
    world: Res<SimWorld>,
    atlas: Res<Atlas>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(v) = world.sim.voyage().filter(|_| state.key.is_some()) else { return };
    let rocks = v.rocks();
    if state.rocks_drawn >= rocks.len() {
        return;
    }
    let seed = state.spec.as_ref().map_or(1, |s| salted(s.seed, 6));
    let uv = atlas.region("fx/white").uv_center();
    let marks = Marks {
        rim: atlas.token_color("world.rock_rim"),
        crack: atlas.token_color("world.rock_crack"),
        glint: atlas.token_color("world.rock_glint"),
        ember: atlas.token_color("world.rock_ember"),
        ember_core: atlas.token_color("world.rock_ember_core"),
    };
    let (mut m, mut glow) = (MeshBuf::default(), MeshBuf::default());
    for (i, r) in rocks.iter().enumerate().skip(state.rocks_drawn) {
        let Some(kind) = world.content.rocks.get(&r.kind) else { continue };
        let t = |k: &str| atlas.token_color(&format!("world.rock.{}.{k}", kind.tone));
        let tone = Tone { shadow: t("shadow"), mid: t("mid"), lit: t("lit"), edge: t("edge") };
        let mut rng = Rng::new(seed ^ (i as u32).wrapping_mul(0x9e37_79b9) | 1);
        let (pos, turn) = (v2(r.pos), Vec2::from_angle(r.angle));
        let pts: Vec<Vec2> = curve(&r.points, OUTLINE_POINTS).into_iter().map(|p| turn.rotate(v2(p))).collect();
        let shape = Shape { start: pts[0].to_angle(), radius: r.radius, pts };
        let mut d = Draw { m: &mut m, glow: &mut glow, uv, at: pos, light: state.light.at(pos), tone: &tone, marks: &marks };
        d.rock(&shape, &kind.look, &mut rng);
    }
    info!("rocks: {} drawn ({} vertices, {} glowing)", rocks.len() - state.rocks_drawn, m.len(), glow.len());
    for (mesh, material, z) in [(m, atlas.material.clone(), Z_ROCKS), (glow, atlas.additive.clone(), Z_GLOW)] {
        if !mesh.is_empty() {
            commands.spawn((SceneryPart, Mesh2d(meshes.add(mesh.into_mesh())), MeshMaterial2d(material), Transform::from_xyz(0.0, 0.0, z)));
        }
    }
    state.rocks_drawn = rocks.len();
}

/// Drawing one rock: its meshes, where it lies and where its light comes from.
struct Draw<'a> {
    m: &'a mut MeshBuf,
    glow: &'a mut MeshBuf,
    uv: Vec2,
    at: Vec2,
    light: Vec2,
    tone: &'a Tone,
    marks: &'a Marks,
}

impl Draw<'_> {
    fn rock(&mut self, s: &Shape, look: &RockLook, rng: &mut Rng) {
        let (l, r, n) = (self.light, s.radius, s.pts.len());
        // How each point of the outline faces the light (its outward normal against the light; counter-clockwise).
        let facing: Vec<f32> = (0..n)
            .map(|k| {
                let tangent = s.pts[(k + 1) % n] - s.pts[(k + n - 1) % n];
                Vec2::new(tangent.y, -tangent.x).normalize_or_zero().dot(l)
            })
            .collect();
        let lit = s.at(l.to_angle(), LIT_POINT);
        let toward = |share: f32| -> Vec<Vec2> { s.pts.iter().map(|&p| lit + (p - lit) * share).collect() };

        // The bands, the outermost (the shadow) first, each with a soft skirt over the one below.
        for (k, &(share, t)) in BANDS.iter().enumerate() {
            let c = self.tone.shade(t);
            let row = toward(share);
            if k > 0 {
                let skirt = toward(BANDS[k - 1].0.min(share + SKIRT));
                self.band(&row, &vec![vcol(c, 1.0); n], &skirt, &vec![vcol(c, 0.0); n], true);
            }
            self.fan(lit, vcol(c, 1.0), &row, &vec![vcol(c, 1.0); n]);
        }
        // The rim light on the shadow side.
        let rim: Vec<[f32; 4]> = facing.iter().map(|&f| vcol(self.marks.rim, RIM.0 * (-f).max(0.0).powf(1.5))).collect();
        let inner: Vec<Vec2> = s.pts.iter().map(|&p| p * RIM.1).collect();
        self.band(&s.pts, &rim, &inner, &vec![[0.0; 4]; n], true);

        if look.ridge {
            self.ridge(s, rng);
        }
        let craters = ((r / look.crater_every).round() as usize).clamp(1, MOST_CRATERS);
        let mut placed: Vec<(Vec2, f32)> = Vec::with_capacity(craters);
        for _ in 0..craters {
            self.crater(s, rng, &mut placed);
        }
        self.speckle(s, rng);
        self.kind_marks(s, look, rng);

        // The edge: one closed strip with mitred joins, brighter where it faces the light.
        let edge = self.tone.edge;
        let lit_edge = edge.mix(&self.tone.lit, 0.35);
        let cols: Vec<[f32; 4]> =
            facing.iter().map(|&f| if f > 0.0 { vcol(lit_edge, (0.5 + 0.4 * f) * 0.9) } else { vcol(edge, (0.3 + 0.12 * (1.0 + f)) * 0.9) }).collect();
        let (outer, inner) = mitred(&s.pts, EDGE);
        self.band(&outer, &cols, &inner, &cols, true);
    }

    /// A ridge across the rock: a lit line on its side towards the light, a shadowed one on the other.
    fn ridge(&mut self, s: &Shape, rng: &mut Rng) {
        let a = rng.range(0.0, TAU);
        let b = a + rng.range(2.2, 3.4);
        let (p, q, bend) = (s.at(a, 0.55), s.at(b, 0.55), rng.range(-0.25, 0.25));
        let mid = (p + q) * 0.5 + (q - p).perp() * bend;
        let line: Vec<Vec2> = (0..14)
            .map(|i| {
                let t = i as f32 / 13.0;
                p * (1.0 - t) * (1.0 - t) + mid * 2.0 * t * (1.0 - t) + q * t * t
            })
            .collect();
        let off = self.light * s.radius * 0.02;
        let (lit, dark): (Vec<Vec2>, Vec<Vec2>) = line.iter().map(|&x| (x + off, x - off)).unzip();
        let (tl, ts) = (vcol(self.tone.lit, 0.45), vcol(self.tone.shadow, 0.6));
        self.ribbon(&lit, s.radius * 0.018, &vec![tl; 14], false);
        self.ribbon(&dark, s.radius * 0.022, &vec![ts; 14], false);
    }

    /// A crater foreshortened towards the limb: a lip lit towards the light, a bowl whose near wall is in shadow and
    /// far wall lit, a darker floor; none where it would overlap one already `placed` (centre, radius).
    fn crater(&mut self, s: &Shape, rng: &mut Rng, placed: &mut Vec<(Vec2, f32)>) {
        let angle = rng.range(0.0, TAU);
        let edge = s.reach(angle);
        let large = rng.next_f32() < LARGE_CRATERS;
        let (lo, hi) = CRATER_SIZE[if large { 0 } else { 1 }];
        let cr = s.radius * rng.range(lo, hi);
        let rho = (CRATER_REACH * rng.next_f32().sqrt()).min(0.9 - cr / edge * 1.25);
        if rho < 0.0 {
            return;
        }
        let u = Vec2::from_angle(angle);
        let c = u * rho * edge;
        if placed.iter().any(|&(o, or)| o.distance(c) < (or + cr) * CRATER_LIP) {
            return;
        }
        placed.push((c, cr));
        let squash = (1.0 - rho.min(0.9).powi(2)).sqrt();
        let ellipse = |scale: f32, shift: Vec2| -> Vec<Vec2> {
            (0..20)
                .map(|k| {
                    let t = k as f32 / 20.0 * TAU;
                    c + shift + u.rotate(Vec2::new(t.cos() * squash, t.sin()) * cr * scale)
                })
                .collect()
        };
        let rim = ellipse(1.0, Vec2::ZERO);
        let face: Vec<f32> = rim.iter().map(|&p| (p - c).normalize_or_zero().dot(self.light)).collect();
        let tone = self.tone;
        let lip: Vec<[f32; 4]> = face.iter().map(|&q| if q > 0.0 { vcol(tone.lit, 0.5 * q) } else { vcol(tone.shadow, 0.4 * -q) }).collect();
        self.band(&rim, &lip, &ellipse(CRATER_LIP, Vec2::ZERO), &[[0.0; 4]; 20], true);
        let wall: Vec<[f32; 4]> = face.iter().map(|&q| if q > 0.0 { vcol(tone.shade(0.2), 1.0) } else { vcol(tone.shade(1.0 + 0.7 * -q), 1.0) }).collect();
        self.fan(c, vcol(tone.shade(0.55), 1.0), &rim, &wall);
        let shift = self.light * -cr * 0.16;
        let floor = vcol(tone.shade(0.5), 0.9);
        self.fan(c + shift, floor, &ellipse(0.52, shift), &[floor; 20]);
    }

    /// Regolith speckle: small triangles, half lit and half in shadow.
    fn speckle(&mut self, s: &Shape, rng: &mut Rng) {
        let n = ((s.radius * SPECKS_PER_M).round() as usize).min(MOST_SPECKS);
        for _ in 0..n {
            let p = s.at(rng.range(0.0, TAU), 0.85 * rng.next_f32().sqrt());
            let size = rng.range(SPECK_SIZE.0, SPECK_SIZE.1);
            let b = rng.range(0.0, TAU);
            let c = if rng.next_f32() < 0.5 { vcol(self.tone.lit, 0.28) } else { vcol(self.tone.shadow, 0.5) };
            let tri = [0.0, 2.1, 4.2].map(|o| self.at + p + Vec2::from_angle(b + o) * size);
            self.m.polygon(&tri, self.uv, c);
        }
    }

    /// The kind's marks: cracks, pits, embers on the shadow side, a glint on the lit side.
    fn kind_marks(&mut self, s: &Shape, look: &RockLook, rng: &mut Rng) {
        let (r, l) = (s.radius, self.light);
        for _ in 0..look.cracks {
            let start = Vec2::new(rng.range(-0.4, 0.4), rng.range(-0.4, 0.4)) * r;
            let line = walk(s, rng, start, 0.07, 11);
            if line.len() > 2 {
                let last = (line.len() - 1) as f32;
                let cols: Vec<[f32; 4]> = (0..line.len()).map(|i| vcol(self.marks.crack, 0.5 * (PI * i as f32 / last).sin() + 0.05)).collect();
                self.ribbon(&line, r * 0.014, &cols, false);
            }
        }
        for _ in 0..look.pits {
            let p = s.at(rng.range(0.0, TAU), 0.75 * rng.next_f32().sqrt());
            let (dark, size) = (self.tone.shadow, r * rng.range(0.018, 0.04));
            let ring = |d: f32| -> Vec<Vec2> { (0..8).map(|k| p + Vec2::from_angle(k as f32 / 8.0 * TAU) * d).collect() };
            let pit = ring(size);
            self.fan(p, vcol(dark, 0.8), &pit, &[vcol(dark, 0.6); 8]);
            self.band(&pit, &[vcol(dark, 0.6); 8], &ring(size * 1.4), &[[0.0; 4]; 8], true);
        }
        for _ in 0..look.embers {
            let a = (-l).to_angle() + rng.range(-0.9, 0.9);
            let start = s.at(a, rng.range(0.35, 0.7));
            let line = walk(s, rng, start, 0.06, 9);
            if line.len() < 3 {
                continue;
            }
            let w: Vec<f32> = line.iter().map(|p| (-p.dot(l) / r * 1.6 + 0.2).clamp(0.0, 1.0)).collect();
            let wide: Vec<[f32; 4]> = w.iter().map(|&w| vcol(self.marks.ember, 0.14 * w)).collect();
            let core: Vec<[f32; 4]> = w.iter().map(|&w| vcol(self.marks.ember_core, 0.7 * w)).collect();
            self.ribbon(&line, r * 0.05, &wide, true);
            self.ribbon(&line, r * 0.011, &core, true);
        }
        if look.glint > 0.0 {
            let at = s.at(l.to_angle(), 0.55);
            let ring: Vec<Vec2> = (0..14).map(|k| self.at + at + Vec2::from_angle(k as f32 / 14.0 * TAU) * r * 0.12).collect();
            self.glow.fan_shaded(self.at + at, vcol(self.marks.glint, look.glint), &ring, &[[0.0; 4]; 14], self.uv);
        }
    }

    /// A fan round `c` (the rock's frame) to `pts`, a colour per point.
    fn fan(&mut self, c: Vec2, cc: [f32; 4], pts: &[Vec2], cols: &[[f32; 4]]) {
        let pts: Vec<Vec2> = pts.iter().map(|&p| self.at + p).collect();
        self.m.fan_shaded(self.at + c, cc, &pts, cols, self.uv);
    }

    /// Quads between two rows (the rock's frame), a colour per point.
    fn band(&mut self, a: &[Vec2], ca: &[[f32; 4]], b: &[Vec2], cb: &[[f32; 4]], closed: bool) {
        let (a, b): (Vec<Vec2>, Vec<Vec2>) = a.iter().zip(b).map(|(&p, &q)| (self.at + p, self.at + q)).unzip();
        self.m.band(&a, ca, &b, cb, closed, self.uv);
    }

    /// A soft line along `line` (the rock's frame): full colour along its middle, fading to nothing `half` either
    /// side; into the additive mesh if `glow`.
    fn ribbon(&mut self, line: &[Vec2], half: f32, cols: &[[f32; 4]], glow: bool) {
        let n = line.len();
        let (mut left, mut right) = (Vec::with_capacity(n), Vec::with_capacity(n));
        for i in 0..n {
            let d = (line[(i + 1).min(n - 1)] - line[i.saturating_sub(1)]).normalize_or_zero().perp() * half;
            left.push(self.at + line[i] + d);
            right.push(self.at + line[i] - d);
        }
        let mid: Vec<Vec2> = line.iter().map(|&p| self.at + p).collect();
        let clear: Vec<[f32; 4]> = cols.iter().map(|c| [c[0], c[1], c[2], 0.0]).collect();
        let m = if glow { &mut *self.glow } else { &mut *self.m };
        m.band(&left, &clear, &mid, cols, false, self.uv);
        m.band(&mid, cols, &right, &clear, false, self.uv);
    }
}

/// A wandering line from `start` (the rock's frame) in `steps` steps of `step` of the radius, turning a little each
/// step, ending before it nears the edge.
fn walk(s: &Shape, rng: &mut Rng, start: Vec2, step: f32, steps: usize) -> Vec<Vec2> {
    let (mut p, mut a) = (start, rng.range(0.0, TAU));
    let mut out = Vec::with_capacity(steps);
    for _ in 0..steps {
        if p.length() > 0.88 * s.reach(p.to_angle()) {
            break;
        }
        out.push(p);
        a += rng.range(-0.6, 0.6);
        p += Vec2::from_angle(a) * step * s.radius;
    }
    out
}

/// The two sides of a closed outline's edge line `half` either side of it, each point moved along its mitre (at
/// most twice `half`): one strip whose joins share their two vertices, so no corner gaps or overlaps.
fn mitred(pts: &[Vec2], half: f32) -> (Vec<Vec2>, Vec<Vec2>) {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let (a, p, b) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            let (n1, n2) = ((p - a).normalize_or_zero().perp(), (b - p).normalize_or_zero().perp());
            let m = (n1 + n2).normalize_or(n1);
            let d = m * half / m.dot(n1).max(0.5);
            (p - d, p + d)
        })
        .unzip()
}

/// The near rocks' line mesh (a scenery part: it goes with the voyage's scenery).
#[derive(Component)]
pub(super) struct NearMesh;

/// What the near rocks' mesh shows: each rock (index) and its strength in [`NEAR_STEPS`].
#[derive(Resource, Default)]
pub(super) struct NearRocks(Vec<(usize, u8)>);

/// The near rocks' line (see the module docs), round the ship the camera follows.
#[allow(clippy::too_many_arguments)]
pub(super) fn near_rocks(
    mut commands: Commands,
    mut near: ResMut<NearRocks>,
    state: Res<Scenery>,
    world: Res<SimWorld>,
    rig: Res<CameraRig>,
    atlas: Res<Atlas>,
    mut meshes: ResMut<Assets<Mesh>>,
    shown: Query<&Mesh2d, With<NearMesh>>,
) {
    let (range, fade) = (atlas.token_f32("world.rock_near.range_m"), atlas.token_f32("world.rock_near.fade_m"));
    let rocks = world.sim.voyage().filter(|_| state.key.is_some()).map_or(&[][..], |v| v.rocks());
    let mut want: Vec<(usize, u8)> = Vec::new();
    if let Some(ship) = rig.focus {
        for (i, r) in rocks.iter().enumerate() {
            let d = (v2(r.pos) - ship).length() - r.radius;
            let t = ((d - range) / fade.max(1e-3)).clamp(0.0, 1.0);
            let k = ((1.0 - t * t * (3.0 - 2.0 * t)) * NEAR_STEPS).round() as u8;
            if k > 0 {
                want.push((i, k));
            }
        }
    }
    let mesh = shown.iter().next();
    if want == near.0 && (mesh.is_some() || want.is_empty()) {
        return;
    }
    let (color, alpha, half) = (atlas.token_color("world.rock_near.color"), atlas.token_f32("world.rock_near.alpha"), atlas.token_f32("world.rock_near.half_width_m"));
    let uv = atlas.region("fx/white").uv_center();
    let mut m = MeshBuf::default();
    for &(i, k) in &want {
        let r = &rocks[i];
        let (pos, turn) = (v2(r.pos), Vec2::from_angle(r.angle));
        let pts: Vec<Vec2> = curve(&r.points, OUTLINE_POINTS).into_iter().map(|p| pos + turn.rotate(v2(p))).collect();
        let c = vec![vcol(color, alpha * k as f32 / NEAR_STEPS); pts.len()];
        let (outer, inner) = mitred(&pts, half);
        m.band(&outer, &c, &inner, &c, true, uv);
    }
    let built = if m.is_empty() { MeshBuf::placeholder() } else { m.into_mesh() };
    match mesh {
        Some(h) => {
            if let Some(mut slot) = meshes.get_mut(&h.0) {
                *slot = built;
            }
        }
        None => {
            commands.spawn((SceneryPart, NearMesh, Mesh2d(meshes.add(built)), MeshMaterial2d(atlas.additive.clone()), Transform::from_xyz(0.0, 0.0, Z_NEAR)));
        }
    }
    near.0 = want;
}
