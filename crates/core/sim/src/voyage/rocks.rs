//! Asteroid rock (GDR-0104): real bodies of a voyage, placed by its scenery (`data::Scenery::rocks`, a cluster
//! type's or a voyage's) when the plan is rolled, on a generator of its own ([`ROCK_STREAM`], after the sites, so the
//! sites' draws stay as they were).
//!
//! * **Rolling** ([`roll`]): each group of the scenery, in the content's order, rolls how many fields, belts or
//!   scatterings it places, then for each how many rocks and where they lie: a field's in a disc (thicker in the
//!   middle), a belt's along an arc round the cluster's centre (thicker along its middle line), a scattering's
//!   anywhere in the cluster. A rock keeps [`ROCK_GAP`] from every other rock, [`SITE_CLEAR`] from every site's spot
//!   and [`ENTRY_CLEAR`] from the entry, and stays inside the cluster; a rock with no such spot in [`ROCK_TRIES`]
//!   tries is left out. Each rock's outline ([`outline`]) is rolled from its kind before its spot.
//! * **Outline** (GDR-0104's status line of 18 Oct 2026): a convex hull of the kind's corners, then control points
//!   round it (each corner rounded in a little, each edge's middle dented in by up to the kind's `dent`), and through
//!   them a periodic spline of radius over angle ([`curve`]): star-shaped round the rock's centre, so it never crosses
//!   itself. The client draws the same curve. A rock dented in by more than [`SPLIT_DENT`] of its radius below its
//!   hull collides as two or three convex pieces cut at its deepest dents through its centre ([`pieces`]), so hits
//!   land on what is drawn; any other as the curve's hull.
//! * **Spawning**: rocks come in [`FieldPlan`]s (a field, a belt cut into pieces of about [`BELT_PIECE`] metres, one
//!   scattered rock), each spawned as anchored bodies once a player vessel comes within
//!   `VoyageTuning::spawn_radius` of its reach, as the sites spawn (E0: records cost nothing until then, and an
//!   anchored body costs the solver nothing).
//! * **What rocks do**: they block ships (a ship that hits one takes ramming damage like any ram, GDR-0047: the
//!   impulse carries its mass and speed; an anchored rock gives nothing, so the kind's `ram` scales what the ship
//!   takes, as a ram prow's factors do), shots (`SimEvent::RockHit`) and plumes, so they give cover. A mining
//!   laser's shot cuts its ore (GDR-0114, `combat::mining`): a rock holds its kind's `ore` per square metre of its
//!   circle and is spent when none is left (it stays, as cover). Behaviors and maneuvers bend their velocity goal round the rocks ahead ([`steer`]), and the
//!   pilot assist `avoid_collisions` sidesteps them like any piece.
//!
//! All placement math is `+ − × ÷`, `sqrt` and libm trig: the same rocks on every platform.

use data::{Content, RockKind, RockLayout, Scenery};

use crate::hash::{StateHash, StateHasher};
use crate::math::{Vec2, atan2, cos, sin, sqrt};
use crate::rng::{Pcg32, splitmix64};

/// The rocks' generator's stream (`"rock"` in ASCII): never a sim stream's id, nor the plan's.
pub const ROCK_STREAM: u64 = 0x726f_636b;
/// Least gap (m) between two rocks' circles.
pub const ROCK_GAP: f32 = 4.0;
/// Least gap (m) between a rock's circle and a site's spot (the site's ships and wrecks lie within 30 m of it).
pub const SITE_CLEAR: f32 = 70.0;
/// Least gap (m) between a rock's circle and the entry.
pub const ENTRY_CLEAR: f32 = 150.0;
/// Tries for a rock's spot before it is left out.
pub const ROCK_TRIES: u32 = 12;
/// A belt spawns in pieces of about this many metres along its arc.
pub const BELT_PIECE: f32 = 180.0;
/// Rocks of a belt lie this far apart along its arc on average (m), which sets how long it is.
const BELT_SPACING: f32 = 28.0;
/// A belt's arc lies between these shares of the cluster's radius from its centre.
const BELT_RADIUS: (f32, f32) = (0.4, 0.85);
/// Scattered rocks lie within this share of the cluster's radius.
const SCATTER_REACH: f32 = 0.95;
/// Behaviors look at least this many seconds ahead at their speed for rocks in the way ([`steer`]) ...
const STEER_AHEAD: f32 = 3.0;
/// ... and keep this gap (m) from them, besides their own and the rock's radius.
const STEER_CLEAR: f32 = 8.0;
/// [`steer`] bends round at most this many rocks one after the other.
const STEER_PASSES: u32 = 3;
/// A rock's corners sit this far out along their ray as control points (shares, rolled per corner): the corners
/// rounded in a little.
const CORNER_IN: (f32, f32) = (0.9, 0.95);
/// The share of edges with a second dent point, a third along the edge, and how deep it dents (share of `dent`).
const SECOND_DENT: (f32, f32) = (0.5, 0.6);
/// The outline is sampled at this many points ([`curve`]) for the pieces and the radius, and drawn at them.
pub const OUTLINE_POINTS: usize = 96;
/// A rock is cut where its outline dents in by more than this share of its radius below the outline's hull ...
pub const SPLIT_DENT: f32 = 0.05;
/// ... into at most this many pieces, cut at least this many samples apart.
const MOST_PIECES: usize = 3;
const CUT_APART: usize = 16;
/// Below this speed (m/s) a ship's own motion is no threat ([`Mover::heading_into`]).
const MOMENTUM_SPEED: f32 = 2.0;
/// A ship at rest against a rock (its circle within the rock's) is pushed off at this speed (m/s) ([`steer`]).
const PUSH_OFF: f32 = 2.0;

/// One rock: its kind (content id), where it lies, how it is turned, its outline's control points ([`outline`],
/// counter-clockwise round its position; the outline is [`curve`] through them), the convex pieces it collides as
/// (each counter-clockwise; [`pieces`]), the radius of the circle round it and the share of a ram's damage a ship
/// takes against it (its kind's `ram`). Mining (GDR-0114, `combat::mining`): the material it gives (its kind's), the
/// ore units left in it (its kind's `ore` per square metre of its circle, rounded) and the cut towards the next unit.
#[derive(Clone, Debug, PartialEq)]
pub struct Rock {
    pub kind: String,
    pub ram: f32,
    pub material: String,
    pub ore: u32,
    pub cut: f32,
    pub pos: Vec2,
    pub angle: f32,
    pub points: Vec<Vec2>,
    pub parts: Vec<Vec<Vec2>>,
    pub radius: f32,
}

/// Rocks that spawn together once a player vessel comes within the spawn radius of `reach` metres round `centre`.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldPlan {
    pub centre: Vec2,
    pub reach: f32,
    pub rocks: Vec<Rock>,
}

impl StateHash for Rock {
    fn hash_state(&self, h: &mut StateHasher) {
        h.str(&self.kind);
        h.f32(self.ram);
        h.vec2(self.pos);
        h.f32(self.angle);
        h.f32(self.radius);
        h.u32(self.points.len() as u32);
        for &p in &self.points {
            h.vec2(p);
        }
        h.u32(self.parts.len() as u32);
        for part in &self.parts {
            h.u32(part.len() as u32);
            for &p in part {
                h.vec2(p);
            }
        }
    }
}

/// A rock's outline's control points rolled from its kind: `corners` corners round its radius, each pulled in by up
/// to `roughness` of it, the whole stretched along x (and squeezed along y, keeping its area), the convex hull; then
/// round it each corner rounded in ([`CORNER_IN`]), each edge's middle dented in by up to `dent` of its distance and
/// every other edge on average a point a third along it by up to [`SECOND_DENT`] of that. Counter-clockwise round
/// the origin (which lies inside: the corners are never more than 0.6 of a step off their place).
pub fn outline(kind: &RockKind, rng: &mut Pcg32) -> Vec<Vec2> {
    let r = rng.range_f32(kind.size.0, kind.size.1);
    let n = kind.corners.0 + rng.below(kind.corners.1.saturating_sub(kind.corners.0) + 1);
    let n = n.max(3);
    let s = rng.range_f32(kind.stretch.0, kind.stretch.1);
    let (sx, sy) = (sqrt(s), 1.0 / sqrt(s));
    let step = core::f32::consts::TAU / n as f32;
    let mut pts: Vec<Vec2> = (0..n)
        .map(|k| {
            let a = step * (k as f32 + rng.range_f32(-0.3, 0.3));
            let d = r * (1.0 - kind.roughness * rng.next_f32());
            Vec2::new(cos(a) * d * sx, sin(a) * d * sy)
        })
        .collect();
    hull(&mut pts);
    let mut out = Vec::with_capacity(pts.len() * 3);
    for (k, &a) in pts.iter().enumerate() {
        let b = pts[(k + 1) % pts.len()];
        out.push(a * rng.range_f32(CORNER_IN.0, CORNER_IN.1));
        if rng.next_f32() < SECOND_DENT.0 {
            out.push((a * 0.7 + b * 0.3) * (1.0 - kind.dent * SECOND_DENT.1 * rng.next_f32()));
        }
        out.push((a + b) * 0.5 * (1.0 - kind.dent * rng.next_f32()));
    }
    out
}

/// The outline through the control points `points` (counter-clockwise round the origin) at `n` evenly spaced
/// angles from the first point's on: a periodic Catmull-Rom spline of the radius over the angle, so it is
/// star-shaped round the origin and never crosses itself. libm trig: the same everywhere.
pub fn curve(points: &[Vec2], n: usize) -> Vec<Vec2> {
    let m = points.len() as isize;
    let tau = core::f32::consts::TAU;
    let mut angle: Vec<f32> = Vec::with_capacity(points.len());
    for p in points {
        let mut a = atan2(p.y, p.x);
        if let Some(&last) = angle.last() {
            while a <= last {
                a += tau;
            }
        }
        angle.push(a);
    }
    let radius: Vec<f32> = points.iter().map(|p| p.length()).collect();
    // The `i`th control point counting on round and round: its angle (unwrapped) and radius.
    let at = |i: isize| {
        let k = i.rem_euclid(m) as usize;
        (angle[k] + tau * i.div_euclid(m) as f32, radius[k])
    };
    let mut i = 0isize;
    (0..n)
        .map(|j| {
            let t = angle[0] + tau * j as f32 / n as f32;
            while at(i + 1).0 <= t {
                i += 1;
            }
            let ((t1, p1), (t2, p2), p0, p3) = (at(i), at(i + 1), at(i - 1).1, at(i + 2).1);
            let u = (t - t1) / (t2 - t1);
            let d = 0.5 * (2.0 * p1 + (p2 - p0) * u + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * u * u + (3.0 * p1 - p0 - 3.0 * p2 + p3) * u * u * u);
            Vec2::new(cos(t), sin(t)) * d
        })
        .collect()
}

/// The convex pieces a rock of outline `outline` (a [`curve`], counter-clockwise round the origin) and `radius`
/// collides as: the outline's hull, or, where it dents in by more than [`SPLIT_DENT`] of the radius below that
/// hull, pieces cut from its centre through its deepest dents (at most [`MOST_PIECES`], at least [`CUT_APART`]
/// samples apart; a single dent is cut through the sample opposite it too), each the hull of its stretch of the
/// outline and the centre. Counter-clockwise.
pub fn pieces(outline: &[Vec2], radius: f32) -> Vec<Vec<Vec2>> {
    let mut whole = outline.to_vec();
    hull(&mut whole);
    let n = outline.len();
    let depth: Vec<f32> = outline
        .iter()
        .map(|&p| {
            whole.iter().zip(whole.iter().cycle().skip(1)).map(|(&a, &b)| (b - a).cross(p - a) / (b - a).length().max(1e-6)).fold(f32::MAX, f32::min)
        })
        .collect();
    let mut order: Vec<usize> = (0..n).filter(|&k| depth[k] > SPLIT_DENT * radius).collect();
    order.sort_by(|&a, &b| depth[b].total_cmp(&depth[a]).then(a.cmp(&b)));
    let mut cuts: Vec<usize> = Vec::new();
    for k in order {
        let apart = |c: usize| {
            let g = c.abs_diff(k);
            g.min(n - g) >= CUT_APART
        };
        if cuts.len() < MOST_PIECES && cuts.iter().all(|&c| apart(c)) {
            cuts.push(k);
        }
    }
    match cuts.len() {
        0 => return vec![whole],
        1 => cuts.push((cuts[0] + n / 2) % n),
        _ => {}
    }
    cuts.sort_unstable();
    (0..cuts.len())
        .map(|i| {
            let (a, b) = (cuts[i], cuts[(i + 1) % cuts.len()]);
            let mut p: Vec<Vec2> = (0..=(b + n - a) % n).map(|k| outline[(a + k) % n]).collect();
            p.push(Vec2::ZERO);
            hull(&mut p);
            p
        })
        .collect()
}

/// The convex hull of `pts` in place, counter-clockwise from the lowest-leftmost point (Andrew's monotone chain).
fn hull(pts: &mut Vec<Vec2>) {
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    let turn = |o: Vec2, a: Vec2, b: Vec2| (a - o).cross(b - o);
    let mut out: Vec<Vec2> = Vec::with_capacity(pts.len() + 1);
    for pass in 0..2 {
        let start = out.len();
        let iter: Box<dyn Iterator<Item = &Vec2>> = if pass == 0 { Box::new(pts.iter()) } else { Box::new(pts.iter().rev()) };
        for &p in iter {
            while out.len() >= start + 2 && turn(out[out.len() - 2], out[out.len() - 1], p) <= 0.0 {
                out.pop();
            }
            out.push(p);
        }
        out.pop();
    }
    *pts = out;
}

/// Where a rock of `radius` may lie: inside the cluster, clear of the sites, the entry and the rocks placed so far.
fn clear(p: Vec2, radius: f32, cluster: f32, sites: &[Vec2], entry: Vec2, placed: &[Rock]) -> bool {
    let far = |q: Vec2, d: f32| (p - q).length_squared() >= d * d;
    p.length() + radius <= cluster
        && far(entry, radius + ENTRY_CLEAR)
        && sites.iter().all(|&s| far(s, radius + SITE_CLEAR))
        && placed.iter().all(|r| far(r.pos, radius + r.radius + ROCK_GAP))
}

/// Rolls the rocks of `scenery` in a cluster of `radius` metres round the origin with its sites at `sites` and its
/// entry at `entry`, from `seed` (see the module docs). `Err` for an unknown rock kind.
pub fn roll(content: &Content, scenery: &Scenery, radius: f32, sites: &[Vec2], entry: Vec2, seed: u64) -> Result<Vec<FieldPlan>, String> {
    let rng = &mut Pcg32::new(splitmix64(seed ^ splitmix64(ROCK_STREAM)), ROCK_STREAM);
    let mut placed: Vec<Rock> = Vec::new();
    let mut fields = Vec::new();
    for g in &scenery.rocks {
        let kind = content.rocks.get(&g.rock).ok_or_else(|| format!("unknown rock `{}`", g.rock))?;
        let count = g.count.0 + rng.below(g.count.1.saturating_sub(g.count.0) + 1);
        for _ in 0..count {
            let n = g.rocks.0 + rng.below(g.rocks.1.saturating_sub(g.rocks.0) + 1);
            let first = placed.len();
            // Each rock: its outline, its spot (up to ROCK_TRIES), then its turn.
            let place = |rng: &mut Pcg32, placed: &mut Vec<Rock>, spot: &mut dyn FnMut(&mut Pcg32) -> Vec2| {
                let mut shape = outline(kind, rng);
                let samples = curve(&shape, OUTLINE_POINTS);
                let r = samples.iter().chain(&shape).fold(0.0f32, |m, p| m.max(p.length()));
                let mut parts = pieces(&samples, r);
                for _ in 0..ROCK_TRIES {
                    let p = spot(rng);
                    if clear(p, r, radius, sites, entry, placed) {
                        let angle = rng.range_f32(-core::f32::consts::PI, core::f32::consts::PI);
                        placed.push(Rock {
                            kind: kind.id.clone(),
                            ram: kind.ram,
                            material: kind.material.clone(),
                            ore: (kind.ore * core::f32::consts::PI * r * r).round() as u32,
                            cut: 0.0,
                            pos: p,
                            angle,
                            points: std::mem::take(&mut shape),
                            parts: std::mem::take(&mut parts),
                            radius: r,
                        });
                        return;
                    }
                }
            };
            match g.layout {
                RockLayout::Field { radius: fr } => {
                    // The field's centre: the first spot that keeps the field's middle off the sites and the entry.
                    let reach = (radius - fr * 0.5).max(0.0);
                    let mut centre = Vec2::ZERO;
                    for _ in 0..ROCK_TRIES {
                        centre = disc(rng, Vec2::ZERO, reach, true);
                        let far = |q: Vec2, d: f32| (centre - q).length_squared() >= d * d;
                        if far(entry, fr + ENTRY_CLEAR) && sites.iter().all(|&s| far(s, fr * 0.6 + SITE_CLEAR)) {
                            break;
                        }
                    }
                    for _ in 0..n {
                        place(rng, &mut placed, &mut |rng| disc(rng, centre, fr, false));
                    }
                    if placed.len() > first {
                        fields.push(field(centre, &placed[first..]));
                    }
                }
                RockLayout::Belt { width } => {
                    let arc = radius * rng.range_f32(BELT_RADIUS.0, BELT_RADIUS.1);
                    let span = (n as f32 * BELT_SPACING / arc).clamp(0.5, core::f32::consts::TAU);
                    let start = rng.range_f32(-core::f32::consts::PI, core::f32::consts::PI);
                    let mut along = Vec::with_capacity(n as usize);
                    for _ in 0..n {
                        let mut t = 0.0;
                        place(rng, &mut placed, &mut |rng| {
                            t = rng.next_f32() * span;
                            let off = (rng.next_f32() + rng.next_f32() - 1.0) * width * 0.5;
                            let a = start + t;
                            Vec2::new(cos(a), sin(a)) * (arc + off)
                        });
                        if placed.len() > first + along.len() {
                            along.push(t);
                        }
                    }
                    // Cut into pieces along the arc, each spawning on its own.
                    let pieces = ((span * arc / BELT_PIECE) as u32).max(1);
                    for k in 0..pieces {
                        let (lo, hi) = (span * k as f32 / pieces as f32, span * (k + 1) as f32 / pieces as f32);
                        let rocks: Vec<Rock> = along
                            .iter()
                            .zip(&placed[first..])
                            .filter(|&(&t, _)| t >= lo && (t < hi || k + 1 == pieces))
                            .map(|(_, r)| r.clone())
                            .collect();
                        if !rocks.is_empty() {
                            let a = start + (lo + hi) * 0.5;
                            fields.push(field(Vec2::new(cos(a), sin(a)) * arc, &rocks));
                        }
                    }
                }
                RockLayout::Scatter => {
                    for _ in 0..n {
                        let before = placed.len();
                        place(rng, &mut placed, &mut |rng| disc(rng, Vec2::ZERO, radius * SCATTER_REACH, true));
                        if placed.len() > before {
                            fields.push(field(placed[before].pos, &placed[before..]));
                        }
                    }
                }
            }
        }
    }
    Ok(fields)
}

/// A spot in the disc of `r` round `centre`: uniform (`even`) or thicker towards the middle.
fn disc(rng: &mut Pcg32, centre: Vec2, r: f32, even: bool) -> Vec2 {
    let u = rng.next_f32();
    let d = r * if even { sqrt(u) } else { u };
    let a = rng.range_f32(-core::f32::consts::PI, core::f32::consts::PI);
    centre + Vec2::new(cos(a), sin(a)) * d
}

/// The rocks round `centre` as one spawn group: its reach the farthest rock's far side.
fn field(centre: Vec2, rocks: &[Rock]) -> FieldPlan {
    let reach = rocks.iter().fold(0.0f32, |m, r| m.max((r.pos - centre).length() + r.radius));
    FieldPlan { centre, reach, rocks: rocks.to_vec() }
}

/// A ship as [`steer`] sees it: its piece's frame origin and the radius of its circle about it, its velocity now and
/// the deceleration it can plan on (`Kin::stop_accel`).
#[derive(Clone, Copy, Debug)]
pub struct Mover {
    pub pos: Vec2,
    pub radius: f32,
    pub vel: Vec2,
    pub accel: f32,
}

impl Mover {
    /// Is the ship against a rock (its circle within the rock's)? Then a pilot under a lock lets go of it, so it can
    /// push off sideways (the scripted pilot, `runner.rs`).
    pub fn against(&self, rocks: &[Rock]) -> bool {
        rocks.iter().any(|r| (r.pos - self.pos).length_squared() < (r.radius + self.radius) * (r.radius + self.radius))
    }

    /// Is the ship's own motion carrying it into a rock's gap before it could stop (its stopping distance, half a
    /// second more and [`STEER_CLEAR`])? Then [`steer`] bends that motion first, and a pilot that can only push along
    /// its nose under a lock lets go of it (the scripted pilot, `runner.rs`).
    pub fn heading_into(&self, rocks: &[Rock]) -> Option<Vec2> {
        let speed = self.vel.length();
        if rocks.is_empty() || speed < MOMENTUM_SPEED || self.accel <= 0.0 {
            return None;
        }
        let reach = speed * speed / (2.0 * self.accel) + speed * 0.5 + STEER_CLEAR;
        let dir = self.vel * (1.0 / speed);
        match first_gap(self, dir, reach, rocks)? {
            (Some(_), d, gap) => Some(tangent(dir, d, gap) * speed),
            // Inside the gap and closing on the rock: across it.
            (None, d, _) if d.dot(dir) > 0.0 => Some(tangent(dir, d, sqrt(d.length_squared())) * speed),
            (None, _, _) => None,
        }
    }
}

/// The tangent (unit) past a rock's gap of radius `gap` whose centre lies `d` away, on the side of the way `dir` the
/// rock is not.
fn tangent(dir: Vec2, d: Vec2, gap: f32) -> Vec2 {
    let dist = sqrt(d.length_squared());
    let sin_t = (gap / dist).min(1.0);
    let cos_t = sqrt((1.0 - sin_t * sin_t).max(0.0));
    let u = d * (1.0 / dist);
    if dir.cross(d) < 0.0 {
        // The rock lies to the right: pass it on the left.
        Vec2::new(u.x * cos_t - u.y * sin_t, u.x * sin_t + u.y * cos_t)
    } else {
        Vec2::new(u.x * cos_t + u.y * sin_t, -u.x * sin_t + u.y * cos_t)
    }
}

/// The rock whose gap (its radius, the ship's and [`STEER_CLEAR`]) the ship is deepest inside, the way to its centre and
/// the gap's radius. The deepest, not the first in the list: between two rocks, pushed out of the farther one's gap
/// the ship was pushed onto the nearer one (the balance harness's stuck check of 18 Oct 2026).
fn deepest_gap(m: &Mover, rocks: &[Rock]) -> Option<(Vec2, f32)> {
    let mut best: Option<(f32, Vec2, f32)> = None;
    for r in rocks {
        let reach = r.radius + m.radius + STEER_CLEAR;
        let d = r.pos - m.pos;
        let dist = sqrt(d.length_squared());
        if dist < reach && best.is_none_or(|b| dist - reach < b.0) {
            best = Some((dist - reach, d, reach));
        }
    }
    best.map(|(_, d, reach)| (d, reach))
}

/// The rock gap the straight way from `m` along `dir` enters first within `ahead` metres: how far (`None` inside
/// one, the deepest, [`deepest_gap`]), the way to the rock's centre and the gap's radius (the rock's, the ship's
/// and [`STEER_CLEAR`]).
fn first_gap(m: &Mover, dir: Vec2, ahead: f32, rocks: &[Rock]) -> Option<(Option<f32>, Vec2, f32)> {
    if let Some((d, reach)) = deepest_gap(m, rocks) {
        return Some((None, d, reach));
    }
    let mut first: Option<(f32, Vec2, f32)> = None;
    for r in rocks {
        let reach = r.radius + m.radius + STEER_CLEAR;
        let d = r.pos - m.pos;
        let along = d.dot(dir);
        if along < -reach || along > ahead + reach {
            continue;
        }
        let side = dir.cross(d);
        let half = reach * reach - side * side;
        if half <= 0.0 {
            continue;
        }
        let enter = along - sqrt(half);
        if enter > 0.0 && enter <= ahead && first.is_none_or(|f| enter < f.0) {
            first = Some((enter, d, reach));
        }
    }
    first.map(|(e, d, r)| (Some(e), d, r))
}

/// Bends the velocity goal `v` of the ship `m` round the rocks it would meet within its look-ahead (at least
/// [`STEER_AHEAD`] seconds at the faster of its speed and the goal's, and a second more than it takes to stop): for
/// the rock whose gap the straight way enters first, along the tangent past its near side at the same speed; then
/// again for the way that gives, at most [`STEER_PASSES`] times. When the way is still blocked after that (a gap too
/// narrow between two rocks), it slows to what it can stop from before the gap, so it never rams at speed. A ship
/// already inside a rock's gap is pushed out first, and one whose own motion carries it into a gap before it could
/// stop bends that motion ([`Mover::heading_into`]) whatever its goal; one at rest against a rock (a goal under
/// half a metre a second) is pushed off it at [`PUSH_OFF`] (it drifted there braking or holding and stayed; 18 Oct
/// 2026). Only `+ − × ÷` and `sqrt` (deterministic).
pub fn steer(m: &Mover, v: Vec2, rocks: &[Rock]) -> Vec2 {
    if let Some(w) = m.heading_into(rocks) {
        return w;
    }
    let speed = v.length();
    if rocks.is_empty() {
        return v;
    }
    if speed < 0.5 {
        return match deepest_gap(m, rocks) {
            Some((d, _)) if m.against(rocks) => d * (-PUSH_OFF / sqrt(d.length_squared()).max(1e-3)),
            _ => v,
        };
    }
    let fast = speed.max(m.vel.length());
    let secs = if m.accel > 0.0 { STEER_AHEAD.max(1.0 + fast / m.accel) } else { STEER_AHEAD };
    let ahead = fast * secs;
    let mut out = v;
    for _ in 0..STEER_PASSES {
        let dir = out * (1.0 / speed);
        let Some((enter, d, reach)) = first_gap(m, dir, ahead, rocks) else { return out };
        let dist = sqrt(d.length_squared());
        if enter.is_none() {
            // Inside the gap: away from the rock, keeping what of the goal does not point at it.
            let away = d * (-1.0 / dist.max(1e-3));
            let into = v.dot(away * -1.0).max(0.0);
            return v + away * (into + speed * 0.5);
        }
        // The tangent on the side the rock is not: the rock's direction turned by the angle its gap circle spans.
        out = tangent(dir, d, reach) * speed;
    }
    // Still blocked: no faster than it can stop from before the gap.
    if let Some((Some(enter), _, _)) = first_gap(m, out * (1.0 / speed), ahead, rocks)
        && m.accel > 0.0
    {
        let stop = sqrt(2.0 * m.accel * enter.max(0.0));
        if stop < speed {
            out = out * (stop / speed);
        }
    }
    out
}
