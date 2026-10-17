//! Asteroid rock (GDR-0104): real bodies of a voyage, placed by its scenery (`data::Scenery::rocks`, a cluster
//! type's or a voyage's) when the plan is rolled, on a generator of its own ([`ROCK_STREAM`], after the sites, so the
//! sites' draws stay as they were).
//!
//! * **Rolling** ([`roll`]): each group of the scenery, in the content's order, rolls how many fields, belts or
//!   scatterings it places, then for each how many rocks and where they lie: a field's in a disc (thicker in the
//!   middle), a belt's along an arc round the cluster's centre (thicker along its middle line), a scattering's
//!   anywhere in the cluster. A rock keeps [`ROCK_GAP`] from every other rock, [`SITE_CLEAR`] from every site's spot
//!   and [`ENTRY_CLEAR`] from the entry, and stays inside the cluster; a rock with no such spot in [`ROCK_TRIES`]
//!   tries is left out. Each rock's outline ([`outline`]) is rolled from its kind right after its spot.
//! * **Spawning**: rocks come in [`FieldPlan`]s (a field, a belt cut into pieces of about [`BELT_PIECE`] metres, one
//!   scattered rock), each spawned as anchored bodies once a player vessel comes within
//!   `VoyageTuning::spawn_radius` of its reach, as the sites spawn (E0: records cost nothing until then, and an
//!   anchored body costs the solver nothing).
//! * **What rocks do**: they block ships (a ship that hits one takes ramming damage like any ram, GDR-0047: the
//!   impulse carries its mass and speed), shots (`SimEvent::RockHit`, the hook for mining later; GDR-0031) and plumes,
//!   so they give cover. Behaviors and maneuvers bend their velocity goal round the rocks ahead ([`steer`]), and the
//!   pilot assist `avoid_collisions` sidesteps them like any piece.
//!
//! All placement math is `+ − × ÷`, `sqrt` and libm trig: the same rocks on every platform.

use data::{Content, RockKind, RockLayout, Scenery};

use crate::hash::{StateHash, StateHasher};
use crate::math::{Vec2, cos, sin, sqrt};
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
/// Behaviors look this many seconds ahead at their speed for rocks in the way ([`steer`]) ...
const STEER_AHEAD: f32 = 3.0;
/// ... and keep this gap (m) from them, besides their own and the rock's radius.
const STEER_CLEAR: f32 = 8.0;

/// One rock: its kind (content id), where it lies, how it is turned, its convex outline (counter-clockwise, about its
/// position) and the radius of the circle round it.
#[derive(Clone, Debug, PartialEq)]
pub struct Rock {
    pub kind: String,
    pub pos: Vec2,
    pub angle: f32,
    pub points: Vec<Vec2>,
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
        h.vec2(self.pos);
        h.f32(self.angle);
        h.f32(self.radius);
        h.u32(self.points.len() as u32);
        for &p in &self.points {
            h.vec2(p);
        }
    }
}

/// A rock's outline rolled from its kind: `corners` corners round its radius, each pulled in by up to `roughness`
/// of it, the whole stretched along x (and squeezed along y, keeping its area), then the convex hull.
/// Counter-clockwise, about the origin.
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
    pts
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
            // Each rock: its spot (up to ROCK_TRIES), then its outline and turn.
            let place = |rng: &mut Pcg32, placed: &mut Vec<Rock>, spot: &mut dyn FnMut(&mut Pcg32) -> Vec2| {
                let mut shape = outline(kind, rng);
                let r = shape.iter().fold(0.0f32, |m, p| m.max(p.length()));
                for _ in 0..ROCK_TRIES {
                    let p = spot(rng);
                    if clear(p, r, radius, sites, entry, placed) {
                        let angle = rng.range_f32(-core::f32::consts::PI, core::f32::consts::PI);
                        placed.push(Rock { kind: kind.id.clone(), pos: p, angle, points: std::mem::take(&mut shape), radius: r });
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

/// Bends the velocity goal `v` of a ship at `pos` (its circle `radius` metres) round the nearest rock it would meet
/// within [`STEER_AHEAD`] seconds: along the tangent past the rock's near side, at the same speed. A ship already
/// closer than the gap is pushed out first. Only `+ − × ÷` and `sqrt` (deterministic).
pub fn steer(pos: Vec2, radius: f32, v: Vec2, rocks: &[Rock]) -> Vec2 {
    let speed = v.length();
    if rocks.is_empty() || speed < 0.5 {
        return v;
    }
    let dir = v * (1.0 / speed);
    let ahead = speed * STEER_AHEAD;
    let mut first: Option<(f32, Vec2, f32)> = None;
    for r in rocks {
        let reach = r.radius + radius + STEER_CLEAR;
        let d = r.pos - pos;
        let along = d.dot(dir);
        if along < -reach || along > ahead + reach {
            continue;
        }
        let side = dir.cross(d);
        if side * side >= reach * reach || along <= 0.0 && d.length_squared() >= reach * reach {
            continue;
        }
        if first.is_none_or(|f| along < f.0) {
            first = Some((along, d, reach));
        }
    }
    let Some((_, d, reach)) = first else { return v };
    let dist2 = d.length_squared();
    let dist = sqrt(dist2);
    if dist <= reach {
        // Inside the gap: away from the rock, keeping what of the goal does not point at it.
        let out = d * (-1.0 / dist.max(1e-3));
        let into = v.dot(out * -1.0).max(0.0);
        return v + out * (into + speed * 0.5);
    }
    // The tangent on the side the rock is not: the rock's direction turned by the angle its gap circle spans.
    let sin_t = reach / dist;
    let cos_t = sqrt((1.0 - sin_t * sin_t).max(0.0));
    let u = d * (1.0 / dist);
    let left = dir.cross(d) < 0.0; // the rock lies to the right: pass it on the left
    let t = if left {
        Vec2::new(u.x * cos_t - u.y * sin_t, u.x * sin_t + u.y * cos_t)
    } else {
        Vec2::new(u.x * cos_t + u.y * sin_t, -u.x * sin_t + u.y * cos_t)
    };
    t * speed
}
