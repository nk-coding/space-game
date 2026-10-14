//! Aiming: where a moving target will be (intercept time, lead point), resolving a target to its state, and the
//! heading that points the guns at it.

use super::*;

/// The heading that brings the strongest working guns to bear on a target (GDR-0040): guns are grouped by the
/// side they face; the group with the most damage per second among working guns (clear, alive, not overheated,
/// not idle: [`gun_works`]) wins (ties: front, back, right, left), and it aims at the lead point if that group's
/// strongest gun fires projectiles. A [`HeadingGoal::Track`] with the target's bearing rate fed forward
/// (GDR-0064). `None` without working guns.
pub fn smart_aim(ship: &Ship, piece: &Piece, kin: &Kin, t: &TargetState) -> Option<HeadingGoal> {
    // [dps, best gun dps, its projectile speed (0 = hit-scan)] per facing group (forward, backward, right, left).
    let mut groups = [[0.0f32; 3]; 4];
    for g in piece.guns.iter().filter(|g| gun_works(g, ship.part(g.part))) {
        let k = if g.dir.y.abs() >= g.dir.x.abs() {
            if g.dir.y >= 0.0 { 0 } else { 1 }
        } else if g.dir.x >= 0.0 {
            2
        } else {
            3
        };
        groups[k][0] += g.dps;
        if g.dps > groups[k][1] {
            groups[k][1] = g.dps;
            groups[k][2] = match g.delivery {
                data::Delivery::Projectile { speed, .. } => speed,
                _ => 0.0,
            };
        }
    }
    let mut best = 0;
    for k in 1..4 {
        if groups[k][0] > groups[best][0] {
            best = k;
        }
    }
    if groups[best][0] <= 0.0 {
        return None;
    }
    let aim = if groups[best][2] > 0.0 {
        lead_point(kin.pos, kin.vel, t.pos, t.vel, groups[best][2])
    } else {
        t.pos
    };
    // Rotate the group's facing onto the aim point, and follow it as it moves.
    kin.track(aim, t.vel, best)
}

/// First-order intercept: the time `t > 0` at which a shot fired now at `speed` (relative to the shooter, who
/// moves with the shot's frame) meets a target at relative position `r` moving at relative velocity `v`, i.e.
/// the smallest positive root of `|r + v·t| = speed·t`. `None` if it can never catch up.
pub fn intercept_time(r: Vec2, v: Vec2, speed: f32) -> Option<f32> {
    let a = v.length_squared() - speed * speed;
    let b = 2.0 * r.dot(v);
    let c = r.length_squared();
    if a.abs() < 1e-6 {
        // Same speed: linear.
        return (b < 0.0).then(|| -c / b);
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return None;
    }
    let sq = sqrt(disc);
    let (t1, t2) = ((-b - sq) / (2.0 * a), (-b + sq) / (2.0 * a));
    let (lo, hi) = if t1 < t2 { (t1, t2) } else { (t2, t1) };
    if lo > 0.0 {
        Some(lo)
    } else if hi > 0.0 {
        Some(hi)
    } else {
        None
    }
}

/// Where to aim so that a projectile of `speed` (on top of the shooter's velocity, as the sim fires them) meets
/// a target moving at constant velocity. Falls back to the target's position if it cannot be caught.
pub fn lead_point(
    shooter: Vec2,
    shooter_vel: Vec2,
    target: Vec2,
    target_vel: Vec2,
    speed: f32,
) -> Vec2 {
    let r = target - shooter;
    let v = target_vel - shooter_vel;
    match intercept_time(r, v, speed) {
        Some(t) => shooter + r + v * t,
        None => target,
    }
}

/// Finds a target's position and velocity now.
pub fn resolve_target(ships: &[Ship], t: &Target) -> Option<TargetState> {
    let piece_state = |p: &Piece| TargetState {
        pos: p.com_world(),
        vel: p.body.linvel,
    };
    match *t {
        Target::Point(p) => Some(TargetState {
            pos: p,
            vel: Vec2::ZERO,
        }),
        Target::Ship(id) => {
            let s = find_ship(ships, id)?;
            let p = match s.control_piece() {
                Some(c) => s.piece(c)?,
                None => s
                    .pieces()
                    .iter()
                    .max_by_key(|p| (p.parts.len(), std::cmp::Reverse(p.id)))?,
            };
            Some(piece_state(p))
        }
        Target::Piece(id, piece) => find_ship(ships, id)?.piece(piece).map(piece_state),
    }
}
