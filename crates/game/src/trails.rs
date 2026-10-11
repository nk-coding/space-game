//! Dotted engine trails: small round particles emitted at the plume tips, left behind in world space and
//! fading out. A fixed pool of sprite entities is reused, so nothing is allocated per frame.

use bevy::prelude::*;

use crate::atlas::Atlas;
use crate::ship_view::ShipView;
use crate::sim_bridge::{EngineThrottle, RenderPose, SimPose};
use crate::RenderToggles;

const MAX_PARTICLES: usize = 768;
const Z_TRAILS: f32 = 5.0;

#[derive(Clone, Copy)]
struct Particle {
    pos: Vec2,
    vel: Vec2,
    age: f32,
    life: f32,
    radius: f32,
    color: Color,
}

#[derive(Resource)]
struct TrailPool {
    particles: Vec<Particle>,
    sprites: Vec<Entity>,
    /// How many sprites were visible last frame (the rest are already hidden).
    shown: usize,
}

/// Per-ship emission accumulators, one per engine.
#[derive(Component, Default)]
pub struct TrailEmitter(Vec<f32>);

#[derive(Component)]
struct TrailSprite;

pub struct TrailsPlugin;

impl Plugin for TrailsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_pool)
            .add_systems(PostUpdate, (emit, simulate).chain().after(crate::sim_bridge::interpolate_poses));
    }
}

fn spawn_pool(mut commands: Commands, atlas: Res<Atlas>) {
    let dot = atlas.region("fx/dot_hard");
    let mut sprites = Vec::with_capacity(MAX_PARTICLES);
    for _ in 0..MAX_PARTICLES {
        sprites.push(
            commands
                .spawn((
                    TrailSprite,
                    Sprite {
                        image: atlas.image.clone(),
                        rect: Some(dot.px),
                        custom_size: Some(Vec2::splat(0.1)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.0, Z_TRAILS),
                    Visibility::Hidden,
                ))
                .id(),
        );
    }
    commands.insert_resource(TrailPool { particles: Vec::with_capacity(MAX_PARTICLES), sprites, shown: 0 });
}

/// Spacing and look follow the dotted trails in the concept art: thrusters ~0.37 cells apart, 6 dots;
/// torch drives ~0.62 cells apart, 7 dots, bigger.
fn emit(
    time: Res<Time>,
    toggles: Res<RenderToggles>,
    mut pool: ResMut<TrailPool>,
    mut ships: Query<(&ShipView, &EngineThrottle, &SimPose, &RenderPose, Option<&mut TrailEmitter>)>,
) {
    let dt = time.delta_secs().min(0.1);
    if toggles.debug_shapes || dt <= 0.0 {
        return;
    }
    for (view, throttle, sim, pose, emitter) in &mut ships {
        let Some(mut emitter) = emitter else { continue };
        if emitter.0.len() != view.engines.len() {
            emitter.0.resize(view.engines.len(), 0.0);
        }
        let rot = Mat2::from_angle(pose.0.angle);
        let ship_vel = sim.velocity();
        let omega = sim.angular_velocity();
        for (i, e) in view.engines.iter().enumerate() {
            let th = throttle.0.get(i).copied().unwrap_or(0.0);
            if th < 0.05 {
                emitter.0[i] = 0.0;
                continue;
            }
            let (rate, speed, life, r0) = if e.large { (12.0, 7.4, 0.6, 0.095) } else { (14.0, 5.2, 0.45, 0.04) };
            emitter.0[i] += dt * rate * (0.5 + 0.5 * th);
            let len = e.len_idle + (e.len_full - e.len_idle) * th;
            let tip_local = e.nozzle + e.exhaust * len;
            let tip = pose.0.pos + rot * tip_local;
            let r = rot * tip_local;
            let point_vel = ship_vel + Vec2::new(-r.y, r.x) * omega;
            let dir = rot * e.exhaust;
            while emitter.0[i] >= 1.0 {
                emitter.0[i] -= 1.0;
                if pool.particles.len() >= MAX_PARTICLES {
                    break;
                }
                // Back-date by the leftover fraction so dots stay evenly spaced at any frame rate.
                let back = emitter.0[i] / (rate * (0.5 + 0.5 * th));
                let vel = point_vel + dir * speed * (0.6 + 0.4 * th);
                pool.particles.push(Particle {
                    pos: tip + vel * back,
                    vel,
                    age: back,
                    life,
                    radius: r0,
                    color: view.glow,
                });
            }
        }
    }
}

fn simulate(
    time: Res<Time>,
    toggles: Res<RenderToggles>,
    mut pool: ResMut<TrailPool>,
    mut sprites: Query<(&mut Transform, &mut Sprite, &mut Visibility), With<TrailSprite>>,
) {
    let dt = time.delta_secs().min(0.1);
    let pool = &mut *pool;
    if toggles.debug_shapes {
        pool.particles.clear();
    }
    let drag = (-1.5 * dt).exp();
    pool.particles.retain_mut(|p| {
        p.age += dt;
        p.pos += p.vel * dt;
        p.vel *= drag;
        p.age < p.life
    });
    let n = pool.particles.len().min(pool.sprites.len());
    for (k, p) in pool.particles.iter().take(n).enumerate() {
        let Ok((mut tf, mut sprite, mut vis)) = sprites.get_mut(pool.sprites[k]) else { continue };
        let t = p.age / p.life;
        tf.translation.x = p.pos.x;
        tf.translation.y = p.pos.y;
        let d = 2.0 * p.radius * (1.0 - 0.5 * t) / 0.92;
        sprite.custom_size = Some(Vec2::splat(d));
        sprite.color = p.color.with_alpha(0.55 * (1.0 - t));
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
    }
    for k in n..pool.shown {
        if let Ok((_, _, mut vis)) = sprites.get_mut(pool.sprites[k]) {
            *vis = Visibility::Hidden;
        }
    }
    pool.shown = n;
}
