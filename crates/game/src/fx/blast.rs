//! Chain-reaction blasts (`SimEvent::Explosion`, GDR-0084): a short white-hot flash, a soft glow over the
//! blast's reach, a ring that runs out to its radius (so the reach reads) and fades, and a few sparks, carried with
//! the piece. The links of a chain resolved in one tick go off one after the other. The look is [`BLAST`].

use bevy::prelude::*;

use super::{Effects, FxQueue, FxRegions, Z_SPARKS};
use crate::ship_view::core_tint;

/// A chain-reaction blast (`SimEvent::Explosion`, GDR-0084), carried with the piece it went off in.
#[derive(Clone, Copy)]
pub(super) struct Blast {
    pos: Vec2,
    vel: Vec2,
    /// Negative until it goes off: the links of a chain play one after the other ([`BlastLook::chain_step`]).
    age: f32,
    radius: f32,
    /// Strength from the blast's damage (1 at [`BlastLook::ref_damage`]).
    k: f32,
    /// The flash and glow dimmed for blasts going off together, so a chain never whites out.
    dim: f32,
    color: Color,
    ember: Color,
}

/// The look of a chain-reaction blast, in one place: a short white-hot flash, a soft glow over its reach, a ring
/// that runs out to the blast's radius (so its reach reads) and fades, and a few sparks. Times in seconds,
/// lengths in metres (1 cell = 1 m).
struct BlastLook {
    /// The damage drawn at strength 1 (a fission reactor's); strength = √(damage / this), clamped.
    ref_damage: f32,
    strength: (f32, f32),
    /// The whole blast, ring included.
    life: f32,
    /// The ring reaches the radius after this long (eased out), then holds there while it fades.
    grow: f32,
    /// The ring starts at this share of the radius.
    ring_start: f32,
    /// The ring's glow: soft dots of this diameter, `ring_spacing` diameters apart along it (dots join without the
    /// seams that strip segments show), at `ring_glow_alpha` each.
    ring_glow: f32,
    ring_spacing: f32,
    ring_glow_alpha: f32,
    /// The ring's crisp core line: width, opacity, and the segments of the circle.
    ring_core: f32,
    ring_alpha: f32,
    ring_segments: usize,
    flash_life: f32,
    /// Flash diameter per metre of radius (at least 1 m; times the strength).
    flash_size: f32,
    glow_life: f32,
    /// Glow diameter per metre of radius: the soft dot fades out near the reach.
    glow_size: f32,
    glow_alpha: f32,
    /// Sparks per blast at strength 1.
    sparks: f32,
    /// Spark speed per metre of radius (at least 1 m).
    spark_speed: (f32, f32),
    spark_life: (f32, f32),
    /// The delay between the links of a chain resolved in one tick, and the most links delayed.
    chain_step: f32,
    chain_max: usize,
    /// Blasts alive at once (more are dropped).
    max_live: usize,
}

const BLAST: BlastLook = BlastLook {
    ref_damage: 1.5,
    strength: (0.55, 1.4),
    life: 0.45,
    grow: 0.2,
    ring_start: 0.2,
    ring_glow: 0.55,
    ring_spacing: 0.3,
    ring_glow_alpha: 0.4,
    ring_core: 0.07,
    ring_alpha: 0.9,
    ring_segments: 32,
    flash_life: 0.15,
    flash_size: 2.2,
    glow_life: 0.32,
    glow_size: 3.2,
    glow_alpha: 0.45,
    sparks: 9.0,
    spark_speed: (4.0, 8.0),
    spark_life: (0.2, 0.45),
    chain_step: 0.04,
    chain_max: 8,
    max_live: 48,
};

impl Effects {
    /// A destroyed part exploded (`SimEvent::Explosion`): `radius` is its reach, `damage` sets the strength;
    /// `vel` carries it with its piece. Blasts arriving before the last one went off are the next links of a
    /// chain: each goes off [`BlastLook::chain_step`] after the one before, its flash dimmed.
    pub fn blast(&mut self, pos: Vec2, vel: Vec2, radius: f32, damage: f32, color: Color, ember: Color) {
        if self.blasts.len() >= BLAST.max_live {
            return;
        }
        let waiting = self.blasts.iter().filter(|b| b.age <= 0.0).count();
        let k = (damage / BLAST.ref_damage).max(0.0).sqrt().clamp(BLAST.strength.0, BLAST.strength.1);
        self.blasts.push(Blast {
            pos,
            vel,
            age: -(waiting.min(BLAST.chain_max) as f32) * BLAST.chain_step,
            radius,
            k,
            dim: 1.0 / (1.0 + waiting as f32).sqrt(),
            color,
            ember,
        });
    }
}

/// Ages the blasts and draws them; one going off now throws its sparks into the effects' sparks (before they
/// age, so they move with the rest).
pub(super) fn draw_blasts(fx: &mut Effects, queue: &mut FxQueue, regions: &FxRegions, dt: f32) {
    for k in 0..fx.blasts.len() {
        let b = &mut fx.blasts[k];
        let before = b.age;
        b.age += dt;
        b.pos += b.vel * dt;
        if before < 0.0 && b.age >= 0.0 {
            let b = *b;
            let first = fx.sparks.len();
            let n = (BLAST.sparks * b.k).round() as usize;
            let speed = (BLAST.spark_speed.0 * b.radius.max(1.0), BLAST.spark_speed.1 * b.radius.max(1.0));
            let half = n / 2;
            fx.spray(b.pos, Vec2::Y, std::f32::consts::PI, half, speed, BLAST.spark_life, 0.07, b.color);
            fx.spray(b.pos, Vec2::Y, std::f32::consts::PI, n - half, speed, BLAST.spark_life, 0.06, b.ember);
            for s in &mut fx.sparks[first..] {
                s.vel += b.vel;
            }
        }
    }
    fx.blasts.retain(|b| b.age < BLAST.life);
    for b in &fx.blasts {
        draw_blast(queue, regions, b);
    }
}

/// One blast at its age: the glow and the flash under the ring, the ring out to the reach.
fn draw_blast(queue: &mut FxQueue, regions: &FxRegions, b: &Blast) {
    let t = b.age;
    if t < 0.0 {
        return;
    }
    let z = Z_SPARKS - 1.5;
    // Weak blasts draw fainter and thinner, never past full.
    let weight = (0.5 + 0.5 * b.k).min(1.0);
    if t < BLAST.glow_life {
        let f = t / BLAST.glow_life;
        let a = BLAST.glow_alpha * weight * b.dim * (1.0 - f) * (1.0 - f);
        queue.dot(&regions.soft, b.pos, BLAST.glow_size * b.radius * (0.8 + 0.2 * f), b.color.with_alpha(a), z);
    }
    if t < BLAST.flash_life {
        let f = t / BLAST.flash_life;
        let d = BLAST.flash_size * b.radius.max(1.0) * b.k * (0.7 + 0.5 * f);
        queue.dot(&regions.soft, b.pos, d, core_tint(b.color).with_alpha(0.95 * b.dim * (1.0 - f)), z + 0.1);
    }
    let grow = (t / BLAST.grow).min(1.0);
    let ease = 1.0 - (1.0 - grow).powi(3);
    let r = b.radius * (BLAST.ring_start + (1.0 - BLAST.ring_start) * ease);
    let fade = if t < BLAST.grow { 1.0 } else { 1.0 - (t - BLAST.grow) / (BLAST.life - BLAST.grow) };
    let point = |i: usize, n: usize| {
        let ang = i as f32 / n as f32 * std::f32::consts::TAU;
        b.pos + Vec2::new(ang.cos(), ang.sin()) * r
    };
    let d = BLAST.ring_glow * weight;
    let dots = ((std::f32::consts::TAU * r / (d * BLAST.ring_spacing)).ceil() as usize).clamp(8, 96);
    let glow = b.color.with_alpha(BLAST.ring_glow_alpha * weight * fade);
    for i in 0..dots {
        queue.dot(&regions.soft, point(i, dots), d, glow, z + 0.2);
    }
    let core = core_tint(b.color).with_alpha(BLAST.ring_alpha * weight * fade);
    let n = BLAST.ring_segments;
    for i in 0..n {
        queue.line(&regions.white, point(i, n), point(i + 1, n), BLAST.ring_core * weight, core, z + 0.3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Blasts of one chain arrive in one tick: each goes off a step after the one before, its flash dimmed, and
    /// none is dropped below the cap.
    #[test]
    fn a_chain_goes_off_link_by_link() {
        let mut fx = Effects::default();
        for _ in 0..3 {
            fx.blast(Vec2::ZERO, Vec2::X, 1.0, 1.5, Color::WHITE, Color::WHITE);
        }
        let ages: Vec<f32> = fx.blasts.iter().map(|b| b.age).collect();
        assert_eq!(ages, [0.0, -BLAST.chain_step, -2.0 * BLAST.chain_step]);
        assert!(fx.blasts[0].dim > fx.blasts[1].dim && fx.blasts[1].dim > fx.blasts[2].dim);
        assert_eq!(fx.blasts[0].k, 1.0);
        for _ in 0..2 * BLAST.max_live {
            fx.blast(Vec2::ZERO, Vec2::ZERO, 0.6, 0.5, Color::WHITE, Color::WHITE);
        }
        assert_eq!(fx.blasts.len(), BLAST.max_live);
        assert!(fx.blasts.iter().all(|b| b.age >= -(BLAST.chain_max as f32) * BLAST.chain_step - 1e-6));
    }
}
