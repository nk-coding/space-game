//! Immediate-mode effects: every frame, systems push [`FxSprite`]s (atlas regions with position, rotation,
//! size, colour, layer) into the [`FxQueue`]; [`flush`] writes them as quads into three dynamic meshes (under
//! the ships, weapons and sparks above them, HUD markers on top), each one draw call with the shared atlas
//! material. The vertex buffers are rewritten in place, so nothing is allocated per frame on our side once
//! they have grown to their working size. (Bevy 0.20 sprites turn every distinct sprite into its own
//! material, which is the wrong tool for thousands of changing particles.)
//!
//! Drawn here: engine plumes and RCS puffs (from `piece.thrusters` × `piece.output`), dotted engine trails,
//! beams (attached to the gun at the render pose), pulse flashes, interpolated projectile tracers, loose salvage
//! fragments (`Sim::loose`: a small turning diamond with a soft glow), muzzle flashes, hit and ricochet sparks,
//! part bursts and salvage streaks. Shield hits and collapses play in the bubbles (`shield.rs`).

use std::collections::BTreeMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use sim::ShipId;

use crate::RenderToggles;
use crate::atlas::{Atlas, Region};
use crate::backdrop::Rng;
use crate::camera::CameraSet;
use crate::ship_view::{PieceView, core_tint, world_of};
use crate::sim_bridge::{SimWorld, v2};
use crate::style::{Style, vcol};

/// Z layers (world).
pub const Z_TRAILS: f32 = 5.0;
pub const Z_WEAPONS: f32 = 300.0;
pub const Z_SPARKS: f32 = 320.0;
pub const Z_HUD: f32 = 400.0;

#[derive(Clone, Copy, Debug)]
pub struct FxSprite {
    /// Normalised UV rectangle of the atlas region.
    pub rect: Rect,
    pub pos: Vec2,
    pub angle: f32,
    pub size: Vec2,
    pub anchor: Vec2,
    pub color: Color,
    pub z: f32,
}

#[derive(Resource, Default)]
pub struct FxQueue {
    items: Vec<FxSprite>,
}

impl FxQueue {
    pub fn push(&mut self, s: FxSprite) {
        self.items.push(s);
    }
    /// A region centred on `pos`.
    pub fn dot(&mut self, r: &Region, pos: Vec2, diameter: f32, color: Color, z: f32) {
        self.push(FxSprite { rect: r.uv, pos, angle: 0.0, size: Vec2::splat(diameter), anchor: Vec2::ZERO, color, z });
    }
    /// A strip from `a` to `b` (the region's x axis runs across the strip).
    pub fn line(&mut self, r: &Region, a: Vec2, b: Vec2, width: f32, color: Color, z: f32) {
        let d = b - a;
        let len = d.length();
        if len < 1e-4 {
            return;
        }
        let angle = (-d.x).atan2(d.y);
        self.push(FxSprite {
            rect: r.uv,
            pos: (a + b) / 2.0,
            angle,
            size: Vec2::new(width, len),
            anchor: Vec2::ZERO,
            color,
            z,
        });
    }
}

/// Atlas regions used every frame (looked up once).
#[derive(Resource, Clone)]
pub struct FxRegions {
    pub white: Region,
    pub soft: Region,
    pub hard: Region,
    pub halo: Region,
    pub plume_small: [Region; 2],
    pub plume_large: [Region; 2],
}

/// The three FX meshes: under the ships, weapons and sparks, HUD markers.
#[derive(Resource)]
struct FxMeshes([Handle<Mesh>; 3]);

/// Scratch buffers per layer (kept between frames).
#[derive(Resource, Default)]
struct FxScratch {
    pos: [Vec<[f32; 3]>; 3],
    uv: [Vec<[f32; 2]>; 3],
    col: [Vec<[f32; 4]>; 3],
    idx: [Vec<u32>; 3],
}

fn layer_of(z: f32) -> usize {
    if z < 50.0 {
        0
    } else if z < 350.0 {
        1
    } else {
        2
    }
}

#[derive(Clone, Copy)]
struct Spark {
    pos: Vec2,
    vel: Vec2,
    age: f32,
    life: f32,
    size: f32,
    color: Color,
    /// Drawn as a short streak along the velocity.
    streak: bool,
}

#[derive(Clone, Copy)]
struct Flash {
    pos: Vec2,
    age: f32,
    life: f32,
    size: f32,
    color: Color,
}

#[derive(Clone, Copy)]
struct Line {
    from: Vec2,
    to: Vec2,
    age: f32,
    life: f32,
    color: Color,
}

/// Short-lived effects spawned from sim events (world space, tick time).
#[derive(Resource)]
pub struct Effects {
    sparks: Vec<Spark>,
    flashes: Vec<Flash>,
    lines: Vec<Line>,
    trail: Vec<Spark>,
    rng: Rng,
    /// Weapon / glow colour per ship (filled lazily).
    ship_colors: BTreeMap<ShipId, (Color, Color)>,
    /// Where engine plumes are burning a part now (`SimEvent::PlumeHit`, refreshed every tick while it lasts).
    burns: Vec<Burn>,
}

/// A plume lane blocked by a part: the engine (ship, part) and the world point it burns.
#[derive(Clone, Copy)]
struct Burn {
    ship: ShipId,
    engine: sim::PartIdx,
    point: Vec2,
    age: f32,
}

/// A burn not refreshed for this long is over (plume hits come every tick, 33 ms).
const BURN_SECONDS: f32 = 0.12;

impl Default for Effects {
    fn default() -> Self {
        Self {
            sparks: Vec::with_capacity(512),
            flashes: Vec::with_capacity(256),
            lines: Vec::with_capacity(128),
            trail: Vec::with_capacity(1024),
            rng: Rng::new(0xF0F0_1234),
            ship_colors: BTreeMap::new(),
            burns: Vec::with_capacity(32),
        }
    }
}

impl Effects {
    pub fn clear(&mut self) {
        self.sparks.clear();
        self.flashes.clear();
        self.lines.clear();
        self.trail.clear();
        self.ship_colors.clear();
        self.burns.clear();
    }

    fn spray(&mut self, pos: Vec2, dir: Vec2, spread: f32, n: usize, speed: (f32, f32), life: (f32, f32), size: f32, color: Color) {
        let base = dir.y.atan2(dir.x);
        for _ in 0..n {
            let a = base + self.rng.range(-spread, spread);
            let v = self.rng.range(speed.0, speed.1);
            let l = self.rng.range(life.0, life.1);
            self.sparks.push(Spark {
                pos,
                vel: Vec2::new(a.cos(), a.sin()) * v,
                age: 0.0,
                life: l,
                size: size * self.rng.range(0.7, 1.3),
                color,
                streak: true,
            });
        }
    }

    pub fn flash(&mut self, pos: Vec2, color: Color, size: f32, life: f32) {
        self.flashes.push(Flash { pos, age: 0.0, life, size, color });
    }

    /// A shot leaving a gun.
    pub fn muzzle(&mut self, pos: Vec2, color: Color) {
        self.flash(pos, color, 0.9, 0.09);
    }

    /// A pulse: a bright line that fades quickly.
    pub fn pulse(&mut self, from: Vec2, to: Vec2, color: Color) {
        self.lines.push(Line { from, to, age: 0.0, life: 0.14, color });
    }

    /// Damage reached a part.
    pub fn hit(&mut self, point: Vec2, normal: Vec2, color: Color, damage: f32) {
        let s = (0.6 + damage.sqrt() * 0.25).min(1.6);
        self.flash(point, color, s, 0.16);
        let n = (3.0 + damage.sqrt() * 2.0).min(9.0) as usize;
        self.spray(point, normal, 1.1, n, (4.0, 11.0), (0.15, 0.35), 0.06, color);
    }

    pub fn ricochet(&mut self, point: Vec2, vel: Vec2) {
        let c = Color::srgb(1.0, 0.85, 0.6);
        self.flash(point, c, 0.7, 0.12);
        self.spray(point, vel.normalize_or_zero(), 0.35, 5, (8.0, 16.0), (0.12, 0.3), 0.06, c);
    }

    /// An engine's plume burns a part at `point` (one lane, this tick): the plume's real reach is drawn faintly
    /// up to it while it lasts (`draw_ships_fx`).
    pub fn burn(&mut self, ship: ShipId, engine: sim::PartIdx, point: Vec2) {
        let same = |b: &Burn| b.ship == ship && b.engine == engine && b.point.distance_squared(point) < 0.36;
        if let Some(b) = self.burns.iter_mut().find(|b| same(b)) {
            b.point = point;
            b.age = 0.0;
        } else if self.burns.len() < 64 {
            self.burns.push(Burn { ship, engine, point, age: 0.0 });
        }
    }

    /// A part or piece destroyed.
    pub fn burst(&mut self, pos: Vec2, color: Color, size: f32) {
        self.flash(pos, Color::srgb(1.0, 0.95, 0.85), size * 2.2, 0.22);
        self.flash(pos, color, size * 3.0, 0.5);
        let n = (6.0 + size * 6.0) as usize;
        self.spray(pos, Vec2::Y, std::f32::consts::PI, n, (2.0, 7.0 * size.max(0.5)), (0.35, 0.8), 0.08, color);
    }
}

pub struct FxPlugin;

/// The system set that writes the FX queue into sprites; push FX before it (in `PostUpdate`).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct FxFlush;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FxQueue>()
            .init_resource::<Effects>()
            .add_systems(Startup, setup)
            .configure_sets(PostUpdate, FxFlush.after(CameraSet).before(bevy::transform::TransformSystems::Propagate))
            .add_systems(PostUpdate, (draw_ships_fx, draw_effects).chain().after(CameraSet).before(FxFlush))
            .add_systems(PostUpdate, flush.in_set(FxFlush));
    }
}

fn setup(mut commands: Commands, atlas: Res<Atlas>, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(FxRegions {
        white: atlas.region("fx/white"),
        soft: atlas.region("fx/dot_soft"),
        hard: atlas.region("fx/dot_hard"),
        halo: atlas.region("fx/halo"),
        plume_small: [atlas.region("fx/plume_small_teal"), atlas.region("fx/plume_small_coral")],
        plume_large: [atlas.region("fx/plume_large_teal"), atlas.region("fx/plume_large_coral")],
    });
    let mut handles = Vec::new();
    for (k, z) in [Z_TRAILS, Z_WEAPONS, Z_HUD].into_iter().enumerate() {
        let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3]);
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32; 2]; 3]);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 3]);
        m.insert_indices(Indices::U32(vec![0, 1, 2]));
        let h = meshes.add(m);
        commands.spawn((
            Mesh2d(h.clone()),
            MeshMaterial2d(atlas.material.clone()),
            Transform::from_xyz(0.0, 0.0, z - k as f32 * 0.0),
            NoFrustumCulling,
        ));
        handles.push(h);
    }
    commands.insert_resource(FxMeshes([handles[0].clone(), handles[1].clone(), handles[2].clone()]));
    commands.insert_resource(FxScratch::default());
}

/// Plumes, RCS puffs, trails, beams and tracers straight from the sim state (interpolated).
#[allow(clippy::too_many_arguments)]
fn draw_ships_fx(
    world: Res<SimWorld>,
    fixed: Res<Time<Fixed>>,
    time: Res<Time>,
    toggles: Res<RenderToggles>,
    regions: Res<FxRegions>,
    style: Res<Style>,
    mut queue: ResMut<FxQueue>,
    mut fx: ResMut<Effects>,
    mut views: Query<&mut PieceView>,
) {
    if toggles.debug_shapes {
        return;
    }
    let alpha = fixed.overstep_fraction().clamp(0.0, 1.0);
    let dt = time.delta_secs().min(0.1);
    let t = time.elapsed_secs();
    let fx = &mut *fx;

    for ship in world.sim.ships() {
        for piece in ship.pieces() {
            let Some(&e) = world.views.get(&(ship.id(), piece.id)) else { continue };
            let Ok(mut view) = views.get_mut(e) else { continue };
            let (pos, angle) = piece.render_pose(alpha);
            let rot = Mat2::from_angle(angle);
            let tint = if view.enemy { 1 } else { 0 };
            if view.emit.len() != piece.thrusters.len() {
                view.emit.resize(piece.thrusters.len(), 0.0);
            }
            let vel = v2(piece.body.linvel);
            let omega = piece.body.angvel;
            for (i, th) in piece.thrusters.iter().enumerate() {
                let out = piece.output.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                // Cruise flame (GDR-0041): an engine holding a speed (stick held at or below the top speed)
                // pushes little or nothing, but keeps a calm flame scaled by how much of its top speed the
                // helm asks for. `calm` = how much of the flame is cruise rather than burn.
                let cruise = if th.rcs { 0.0 } else { piece.cruise.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0) };
                let level = out.max(cruise);
                if level < 0.02 {
                    view.emit[i] = 0.0;
                    continue;
                }
                let calm = (cruise - out).max(0.0) / level;
                let dir = v2(th.dir);
                let mut exhaust = -dir;
                if th.gimbal > 0.0 {
                    let d = piece.act.deflect.get(i).copied().unwrap_or(0.0);
                    exhaust = (exhaust - v2(th.lat) * d * th.gimbal).normalize_or_zero();
                }
                let wobble = 0.05 * (t * 23.0 + i as f32 * 1.7).sin() + 0.03 * (t * 37.0 + i as f32).sin();
                let flicker = 1.0 + wobble * (1.0 - calm * (1.0 - style.plume.cruise_flicker));
                if th.rcs {
                    // RCS: a small puff at the nozzle on the hull side.
                    let r = &regions.plume_small[tint];
                    let nozzle = v2(th.point);
                    let ew = rot * exhaust;
                    queue.push(FxSprite {
                        rect: r.uv,
                        pos: world_of(pos, angle, nozzle),
                        angle: ew.x.atan2(-ew.y),
                        size: r.cells * Vec2::new(0.45, 0.25 + 0.35 * out) * flicker,
                        anchor: r.anchor(),
                        color: Color::WHITE.with_alpha(0.5 + 0.5 * out),
                        z: Z_TRAILS,
                    });
                    continue;
                }
                let (w, h) = ship.grid().shape(th.part as usize).dims();
                let ext = if dir.x.abs() > 0.5 { w } else { h } as f32;
                let large = w.min(h) >= 2;
                let nozzle = v2(th.point) - dir * (ext / 2.0 - 0.05);
                let (idle, full) = if large { (0.75, 2.05) } else { (0.4, 1.15) };
                let shorten = 1.0 - calm * (1.0 - style.plume.cruise_length);
                let len = (idle + (full - idle) * level) * shorten * flicker;
                let r = if large { &regions.plume_large[tint] } else { &regions.plume_small[tint] };
                let ew = rot * exhaust;
                let at = world_of(pos, angle, nozzle);
                let width = 0.85 + 0.15 * level;
                queue.push(FxSprite {
                    rect: r.uv,
                    pos: at,
                    angle: ew.x.atan2(-ew.y),
                    size: r.cells * Vec2::new(width, len / full),
                    anchor: r.anchor(),
                    color: Color::WHITE.with_alpha(0.55 + 0.45 * level),
                    z: Z_TRAILS,
                });
                // The plume burns a part beyond the drawn flame (GDR-0049: the sim's reach is 3-6 m): a faint streak
                // out to it and an ember where it burns.
                for b in fx.burns.iter().filter(|b| b.ship == ship.id() && b.engine == th.part) {
                    let k = (1.0 - b.age / BURN_SECONDS).clamp(0.0, 1.0) * level;
                    queue.line(&regions.halo, at, b.point, 0.35 * width, view.glow.with_alpha(0.22 * k), Z_TRAILS);
                    queue.dot(&regions.soft, b.point, 0.9, Color::srgb(1.0, 0.72, 0.45).with_alpha(0.55 * k), Z_TRAILS + 0.1);
                }
                // Cruise: a hot, steady core down the middle of the calm flame.
                if calm > 0.0 {
                    let p = &style.plume;
                    let core_len = len * p.core_length;
                    let nominal_width = if large { 1.5 } else { 0.6 };
                    queue.push(FxSprite {
                        rect: regions.soft.uv,
                        pos: at + ew * (0.5 * core_len),
                        angle: ew.x.atan2(-ew.y),
                        size: Vec2::new(nominal_width * width * p.core_width, core_len),
                        anchor: Vec2::ZERO,
                        color: p.core[tint].with_alpha(p.core_opacity * calm * level),
                        z: Z_TRAILS,
                    });
                }

                // Trail particles from the plume tip, carried with the ship's velocity at that point (a thinner
                // trail while cruising).
                if level > 0.05 {
                    let (rate, speed, life, r0) = if large { (12.0, 7.4, 0.6, 0.095) } else { (14.0, 5.2, 0.45, 0.04) };
                    let rate = rate * (0.5 + 0.5 * level) * (1.0 - calm * (1.0 - style.plume.cruise_trail));
                    view.emit[i] += dt * rate;
                    let tip = at + ew * len;
                    let arm = tip - v2(piece.com_world());
                    let point_vel = vel + Vec2::new(-arm.y, arm.x) * omega;
                    while view.emit[i] >= 1.0 {
                        view.emit[i] -= 1.0;
                        let back = view.emit[i] / rate;
                        let v = point_vel + ew * speed * (0.6 + 0.4 * out);
                        if fx.trail.len() < 2048 {
                            fx.trail.push(Spark { pos: tip + v * back, vel: v, age: back, life, size: r0, color: view.glow, streak: false });
                        }
                    }
                } else {
                    view.emit[i] = 0.0;
                }
            }
        }
    }

    // Beams: attached to the gun at the render pose, with the tick's length.
    for b in world.sim.beams() {
        let Some(ship) = world.sim.ship(b.ship) else { continue };
        let Some(piece) = ship.piece(b.piece) else { continue };
        let Some(gun) = piece.guns.iter().find(|g| g.part == b.gun) else { continue };
        let (pos, angle) = piece.render_pose(alpha);
        let from = world_of(pos, angle, v2(gun.muzzle));
        let dir = Mat2::from_angle(angle) * v2(gun.dir);
        let len = (v2(b.to) - v2(b.from)).length();
        let to = from + dir * len;
        let c = ship_colors(fx, &world, b.ship, &style).0;
        let pulse = 0.85 + 0.15 * (t * 40.0).sin();
        queue.line(&regions.halo, from, to, 0.9, c.with_alpha(0.9 * pulse), Z_WEAPONS);
        queue.line(&regions.white, from, to, 0.08, core_tint(c).with_alpha(0.95), Z_WEAPONS + 0.1);
        if b.hit {
            queue.dot(&regions.soft, to, 1.1 * pulse, c.with_alpha(0.8), Z_WEAPONS + 0.2);
            queue.dot(&regions.hard, to, 0.18, Color::WHITE, Z_WEAPONS + 0.3);
        }
    }

    // Projectiles: interpolated, a streak behind each.
    for p in world.sim.projectiles() {
        let at = v2(p.prev_pos) + (v2(p.pos) - v2(p.prev_pos)) * alpha;
        let v = v2(p.vel);
        let speed = v.length();
        if speed < 1e-3 {
            continue;
        }
        let back = at - v / speed * (speed * 0.05).clamp(0.6, 3.0);
        let c = ship_colors(fx, &world, p.owner, &style).0;
        queue.line(&regions.halo, back, at, 0.5, c.with_alpha(0.7), Z_WEAPONS);
        queue.line(&regions.white, back, at, 0.06, core_tint(c).with_alpha(0.9), Z_WEAPONS + 0.1);
        queue.dot(&regions.soft, at, 0.55, c.with_alpha(0.85), Z_WEAPONS + 0.2);
    }

    // Loose fragments (salvage, GDR-0063): a small diamond in the logic colour, turning slowly in a soft
    // breathing glow, interpolated like the projectiles.
    let frag = style.system_color("logic");
    for l in world.sim.loose() {
        let at = v2(l.prev_pos) + (v2(l.pos) - v2(l.prev_pos)) * alpha;
        let phase = l.id.0 as f32 * 1.7;
        let glow = 0.8 + 0.2 * (t * 3.0 + phase).sin();
        queue.dot(&regions.soft, at, 1.7 * glow, frag.with_alpha(0.5), Z_WEAPONS);
        let spin = t * 0.9 + phase;
        let corner = |k: usize| {
            let a = spin + k as f32 * std::f32::consts::FRAC_PI_2;
            at + Vec2::new(a.cos(), a.sin()) * 0.45
        };
        for k in 0..4 {
            queue.line(&regions.white, corner(k), corner(k + 1), 0.08, core_tint(frag).with_alpha(0.95), Z_WEAPONS + 0.1);
        }
    }
}

/// `(weapon colour, glow colour)` of a ship.
fn ship_colors(fx: &mut Effects, world: &SimWorld, ship: ShipId, style: &Style) -> (Color, Color) {
    if let Some(c) = fx.ship_colors.get(&ship) {
        return *c;
    }
    let enemy = world
        .sim
        .ship(ship)
        .and_then(|s| world.content.factions.get(&s.design().faction))
        .is_some_and(|f| f.glow == "enemy");
    let c = if enemy {
        (style.system_color("enemy"), style.glow_enemy)
    } else {
        (style.system_color("weapons"), style.glow_player)
    };
    fx.ship_colors.insert(ship, c);
    c
}

/// Ages the event effects and draws them.
fn draw_effects(
    time: Res<Time>,
    toggles: Res<RenderToggles>,
    regions: Res<FxRegions>,
    mut queue: ResMut<FxQueue>,
    mut fx: ResMut<Effects>,
) {
    let dt = time.delta_secs().min(0.1);
    if toggles.debug_shapes {
        fx.sparks.clear();
        fx.flashes.clear();
        fx.lines.clear();
        fx.trail.clear();
        fx.burns.clear();
        return;
    }
    let fx = &mut *fx;
    fx.burns.retain_mut(|b| {
        b.age += dt;
        b.age < BURN_SECONDS
    });
    let drag = (-2.5 * dt).exp();
    fx.sparks.retain_mut(|s| {
        s.age += dt;
        s.pos += s.vel * dt;
        s.vel *= drag;
        s.age < s.life
    });
    for s in &fx.sparks {
        let k = 1.0 - s.age / s.life;
        let tail = s.pos - s.vel * 0.035;
        queue.line(&regions.white, tail, s.pos, s.size, s.color.with_alpha(k), Z_SPARKS);
    }
    let trail_drag = (-1.5 * dt).exp();
    fx.trail.retain_mut(|p| {
        p.age += dt;
        p.pos += p.vel * dt;
        p.vel *= trail_drag;
        p.age < p.life
    });
    for p in &fx.trail {
        let k = p.age / p.life;
        let d = 2.0 * p.size * (1.0 - 0.5 * k) / 0.92;
        queue.dot(&regions.hard, p.pos, d, p.color.with_alpha(0.55 * (1.0 - k)), Z_TRAILS);
    }
    fx.flashes.retain_mut(|f| {
        f.age += dt;
        f.age < f.life
    });
    for f in &fx.flashes {
        let k = f.age / f.life;
        let size = f.size * (0.6 + 0.6 * k);
        queue.dot(&regions.soft, f.pos, size, f.color.with_alpha((1.0 - k) * 0.95), Z_SPARKS - 1.0);
    }
    fx.lines.retain_mut(|l| {
        l.age += dt;
        l.age < l.life
    });
    for l in &fx.lines {
        let k = 1.0 - l.age / l.life;
        queue.line(&regions.halo, l.from, l.to, 0.7, l.color.with_alpha(0.9 * k), Z_WEAPONS);
        queue.line(&regions.white, l.from, l.to, 0.07, core_tint(l.color).with_alpha(k), Z_WEAPONS + 0.1);
    }
}

/// Writes the queue into the three layer meshes (in place).
fn flush(mut queue: ResMut<FxQueue>, fx_meshes: Res<FxMeshes>, mut scratch: ResMut<FxScratch>, mut meshes: ResMut<Assets<Mesh>>) {
    let sc = &mut *scratch;
    for k in 0..3 {
        sc.pos[k].clear();
        sc.uv[k].clear();
        sc.col[k].clear();
        sc.idx[k].clear();
    }
    for it in &queue.items {
        let k = layer_of(it.z);
        let (s, c) = it.angle.sin_cos();
        let rot = |v: Vec2| Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c);
        // Corners relative to the anchor (anchor in -0.5..0.5, y up), then rotated and placed.
        let corner = |fx: f32, fy: f32| {
            let local = Vec2::new((fx - 0.5 - it.anchor.x) * it.size.x, (0.5 - fy - it.anchor.y) * it.size.y);
            let p = it.pos + rot(local);
            [p.x, p.y, 0.0]
        };
        let base = sc.pos[k].len() as u32;
        sc.pos[k].extend_from_slice(&[corner(0.0, 0.0), corner(1.0, 0.0), corner(1.0, 1.0), corner(0.0, 1.0)]);
        let (u0, v0, u1, v1) = (it.rect.min.x, it.rect.min.y, it.rect.max.x, it.rect.max.y);
        sc.uv[k].extend_from_slice(&[[u0, v0], [u1, v0], [u1, v1], [u0, v1]]);
        let col = vcol(it.color, 1.0);
        sc.col[k].extend_from_slice(&[col; 4]);
        sc.idx[k].extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    queue.items.clear();
    for k in 0..3 {
        // An empty mesh is replaced by one degenerate triangle.
        if sc.idx[k].is_empty() {
            sc.pos[k].extend_from_slice(&[[0.0; 3]; 3]);
            sc.uv[k].extend_from_slice(&[[0.0; 2]; 3]);
            sc.col[k].extend_from_slice(&[[0.0; 4]; 3]);
            sc.idx[k].extend_from_slice(&[0, 1, 2]);
        }
        let Some(mut mesh) = meshes.get_mut(&fx_meshes.0[k]) else { continue };
        write_attr(&mut mesh, &sc.pos[k], &sc.uv[k], &sc.col[k], &sc.idx[k]);
    }
}

/// Copies the scratch buffers into the mesh, reusing its vectors when it still owns its data.
fn write_attr(mesh: &mut Mesh, pos: &[[f32; 3]], uv: &[[f32; 2]], col: &[[f32; 4]], idx: &[u32]) {
    let in_place = matches!(mesh.try_attribute_mut(Mesh::ATTRIBUTE_POSITION), Ok(VertexAttributeValues::Float32x3(_)));
    if in_place {
        if let Ok(VertexAttributeValues::Float32x3(v)) = mesh.try_attribute_mut(Mesh::ATTRIBUTE_POSITION) {
            v.clear();
            v.extend_from_slice(pos);
        }
        if let Ok(VertexAttributeValues::Float32x2(v)) = mesh.try_attribute_mut(Mesh::ATTRIBUTE_UV_0) {
            v.clear();
            v.extend_from_slice(uv);
        }
        if let Ok(VertexAttributeValues::Float32x4(v)) = mesh.try_attribute_mut(Mesh::ATTRIBUTE_COLOR) {
            v.clear();
            v.extend_from_slice(col);
        }
        if let Some(Indices::U32(v)) = mesh.indices_mut() {
            v.clear();
            v.extend_from_slice(idx);
            return;
        }
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos.to_vec());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv.to_vec());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col.to_vec());
    mesh.insert_indices(Indices::U32(idx.to_vec()));
}
