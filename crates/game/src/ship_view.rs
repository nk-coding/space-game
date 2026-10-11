//! Style 2a ship rendering: turns a render-ready [`ShipVisual`] into an entity hierarchy.
//!
//! ```text
//! ship root   Transform (from SimPose via interpolation), ShipView, SimPose, RenderPose, EngineThrottle
//! ├── body     Mesh2d: barrels, seams, tiles, icon glows, icons, hull halo + line, muzzle glows (1 draw call)
//! ├── aura     Sprite (fx/aura), faint radial glow behind the ship
//! ├── plume×n  Sprite (fx/plume_*), anchored at the nozzle, length follows EngineThrottle
//! └── shields  Mesh2d: dotted rings (toggle)
//! ```
//!
//! The body has three prebuilt meshes (style 2a, style 2a without pre-blurred glow layers, debug shapes);
//! toggles swap the handle, nothing is rebuilt per frame. All meshes sample the one atlas texture through
//! the shared atlas material.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::atlas::{Atlas, Region};
use crate::meshbuf::{ensure_ccw, inset_convex, MeshBuf};
use crate::sim_bridge::{EngineThrottle, Pose2, RenderPose, SimPose};
use crate::style::{vcol, Style};
use crate::RenderToggles;

// ---------------------------------------------------------------------------------------------------------
// Render-ready description (blueprint frame: metres, y down, forward = −y). Built from `data` content by
// `ship_model`; nothing here knows about content files.

#[derive(Clone, Debug)]
pub struct ShipVisual {
    /// One entry per blueprint placement, in blueprint order (index = part index in `Blueprint::parts`).
    pub parts: Vec<PartVisual>,
    /// Outline loops of the main piece (blueprint frame); `bool` = closed.
    pub hull_loops: Vec<(Vec<Vec2>, bool)>,
    /// The point the ship rotates about (blueprint frame). The sim will use the centre of mass.
    pub pivot: Vec2,
    pub bounds_min: Vec2,
    pub bounds_max: Vec2,
    /// Hull edge / aura / plume tint.
    pub glow: Color,
    pub enemy: bool,
    /// Navigation lights (blueprint frame position, colour).
    pub nav_lights: Vec<(Vec2, Color)>,
}

#[derive(Clone, Debug)]
pub struct PartVisual {
    /// Outline (blueprint frame).
    pub polygon: Vec<Vec2>,
    /// Flat tile colour (armour tone or the machine tile colour).
    pub fill: Color,
    /// Colour in debug-shapes mode.
    pub debug_color: Color,
    /// Atlas icon key (file stem in `art/icons`), drawn at the part centre.
    pub icon: Option<String>,
    /// Centre of the rotated footprint.
    pub center: Vec2,
    /// Rotated footprint size in cells.
    pub dims: Vec2,
    pub rot: u8,
    pub flip: bool,
    /// Unit vector the part faces (blueprint frame). Guns fire and engines push this way.
    pub facing: Vec2,
    pub engine: Option<EngineVisual>,
    pub gun: Option<GunVisual>,
    pub shield_radius: Option<f32>,
    /// Part of the main piece (salvage is drawn dimmer and without hull glow).
    pub main: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct EngineVisual {
    /// Large nozzle (torch drive) or small (thruster).
    pub large: bool,
    /// Exhaust is not blocked by the ship's own parts.
    pub clear: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct GunVisual {
    /// Line of fire is clear (barrel + muzzle glow are only drawn then).
    pub clear: bool,
    /// Barrel `(width, protrusion beyond the front edge)` in cells, if the gun has one.
    pub barrel: Option<(f32, f32)>,
    /// Muzzle glow radius in cells.
    pub muzzle_radius: f32,
    pub color: Color,
}

// ---------------------------------------------------------------------------------------------------------
// Components

/// Where an engine sits on the ship (ship-local, y up).
#[derive(Clone, Copy, Debug)]
pub struct EngineMount {
    /// Index of the engine part in the blueprint (`Blueprint::parts`), for mapping sim throttles.
    #[allow(dead_code, reason = "read by the sim bridge once the sim drives ships")]
    pub part: usize,
    pub nozzle: Vec2,
    /// Exhaust direction (unit, ship-local).
    pub exhaust: Vec2,
    pub large: bool,
    /// Plume length in cells at zero and full throttle.
    pub len_idle: f32,
    pub len_full: f32,
}

#[derive(Component)]
pub struct ShipView {
    pub body: Entity,
    pub aura: Entity,
    pub shields: Option<Entity>,
    pub plumes: Vec<Entity>,
    pub engines: Vec<EngineMount>,
    pub mesh_style: Handle<Mesh>,
    pub mesh_style_noglow: Handle<Mesh>,
    pub mesh_debug: Handle<Mesh>,
    /// Hull glow colour (trails use it too).
    pub glow: Color,
}

#[derive(Component)]
pub struct ShipBody;

#[derive(Component)]
pub struct Plume {
    pub index: usize,
    /// Full-throttle length baked into the texture (cells).
    pub art_len: f32,
}

// ---------------------------------------------------------------------------------------------------------
// Building

const Z_AURA: f32 = -0.4;
const Z_PLUME: f32 = -0.2;
const Z_SHIELD: f32 = 0.2;

/// Plume length (cells) as in `drawFlame()` of the concept board: `0.4 + 0.75 t` for thrusters,
/// `0.75 + 1.3 t` for torch drives (t = throttle).
fn plume_lengths(large: bool) -> (f32, f32) {
    if large { (0.75, 2.05) } else { (0.4, 1.15) }
}

/// The part's extent along its facing axis.
fn extent_along(p: &PartVisual) -> f32 {
    if p.facing.x.abs() > 0.5 { p.dims.x } else { p.dims.y }
}

/// Angle (CCW, y up) that turns art pointing up (local +y) to point along `d` (y up).
fn angle_up_to(d: Vec2) -> f32 {
    (-d.x).atan2(d.y)
}

pub fn spawn_ship_view(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    atlas: &Atlas,
    style: &Style,
    v: &ShipVisual,
    pose: Pose2,
    z: f32,
    toggles: &RenderToggles,
) -> Entity {
    let to_local = |p: Vec2| Vec2::new(p.x - v.pivot.x, v.pivot.y - p.y);
    let dir_local = |d: Vec2| Vec2::new(d.x, -d.y);

    let mesh_style = meshes.add(build_body(v, atlas, style, true));
    let mesh_style_noglow = meshes.add(build_body(v, atlas, style, false));
    let mesh_debug = meshes.add(build_debug(v, atlas));

    // Engines
    let mut engines = Vec::new();
    for (part, p) in v.parts.iter().enumerate() {
        let Some(e) = p.engine else { continue };
        if !e.clear || !p.main {
            continue;
        }
        // Nozzle: centre of the back side, 0.05 cells inside (as in the concept art).
        let back = p.center - p.facing * (extent_along(p) / 2.0 - 0.05);
        let (len_idle, len_full) = plume_lengths(e.large);
        engines.push(EngineMount {
            part,
            nozzle: to_local(back),
            exhaust: dir_local(-p.facing),
            large: e.large,
            len_idle,
            len_full,
        });
    }

    let root = commands
        .spawn((
            Transform::from_xyz(pose.pos.x, pose.pos.y, z).with_rotation(Quat::from_rotation_z(pose.angle)),
            Visibility::default(),
            SimPose::at(pose),
            RenderPose(pose),
            EngineThrottle(vec![0.0; engines.len()]),
        ))
        .id();

    let body_mesh = if toggles.debug_shapes {
        mesh_debug.clone()
    } else if toggles.glow_sprites {
        mesh_style.clone()
    } else {
        mesh_style_noglow.clone()
    };
    let body = commands
        .spawn((ShipBody, Mesh2d(body_mesh), MeshMaterial2d(atlas.material.clone()), Transform::default()))
        .id();

    // Aura: centred on the bounding box, radius 0.85 x the larger side (drawAura in the concept board).
    let center = to_local((v.bounds_min + v.bounds_max) / 2.0);
    let size = (v.bounds_max - v.bounds_min).max_element() * 0.85 * 2.0;
    let (aura_region, aura_alpha) = if v.enemy {
        (atlas.region("fx/aura_linear"), 0.12)
    } else {
        (atlas.region("fx/aura"), style.aura_opacity)
    };
    let aura = commands
        .spawn((
            Sprite {
                image: atlas.image.clone(),
                rect: Some(aura_region.px),
                custom_size: Some(Vec2::splat(size)),
                color: v.glow.with_alpha(aura_alpha),
                ..default()
            },
            Transform::from_xyz(center.x, center.y, Z_AURA),
            visible(!toggles.debug_shapes && toggles.glow_sprites),
        ))
        .id();

    // Plumes
    let tint = if v.enemy { "coral" } else { "teal" };
    let mut plumes = Vec::new();
    for (i, e) in engines.iter().enumerate() {
        let r = atlas.region(&format!("fx/plume_{}_{}", if e.large { "large" } else { "small" }, tint));
        let angle = (e.exhaust.x).atan2(-e.exhaust.y); // art points down (−y local)
        let id = commands
            .spawn((
                Plume { index: i, art_len: e.len_full },
                Sprite {
                    image: atlas.image.clone(),
                    rect: Some(r.px),
                    custom_size: Some(r.cells),
                    ..default()
                },
                Anchor(r.anchor()),
                Transform::from_xyz(e.nozzle.x, e.nozzle.y, Z_PLUME).with_rotation(Quat::from_rotation_z(angle)),
                visible(!toggles.debug_shapes),
            ))
            .id();
        plumes.push(id);
    }

    // Shields
    let shield_mesh = build_shields(v, atlas, style);
    let shields = shield_mesh.map(|m| {
        commands
            .spawn((
                Mesh2d(meshes.add(m)),
                MeshMaterial2d(atlas.material.clone()),
                Transform::from_xyz(0.0, 0.0, Z_SHIELD),
                visible(toggles.shields && !toggles.debug_shapes),
            ))
            .id()
    });

    let mut children = vec![body, aura];
    children.extend(&plumes);
    children.extend(shields);
    commands.entity(root).add_children(&children).insert(ShipView {
        body,
        aura,
        shields,
        plumes,
        engines,
        mesh_style,
        mesh_style_noglow,
        mesh_debug,
        glow: v.glow,
    });
    root
}

fn visible(on: bool) -> Visibility {
    if on { Visibility::Inherited } else { Visibility::Hidden }
}

/// The ship body in style 2a, bottom to top. `glow` adds the pre-blurred layers (icon glows, hull halo,
/// soft muzzle glows); without it the look is the bloom-only variant.
fn build_body(v: &ShipVisual, atlas: &Atlas, style: &Style, glow: bool) -> Mesh {
    let to_local = |p: Vec2| Vec2::new(p.x - v.pivot.x, v.pivot.y - p.y);
    let dir_local = |d: Vec2| Vec2::new(d.x, -d.y);
    let white = atlas.region("fx/white").uv_center();
    let alpha = |p: &PartVisual| if p.main { 1.0 } else { 0.45 };
    let mut m = MeshBuf::default();

    // 1. Gun barrels, under the tiles (they come out through the nose).
    let barrel = atlas.region("fx/barrel");
    for p in &v.parts {
        let Some(g) = p.gun else { continue };
        let Some((width, protrude)) = g.barrel else { continue };
        if !g.clear || !p.main {
            continue;
        }
        let front = p.center + p.facing * (extent_along(p) / 2.0);
        let tip = front + p.facing * protrude;
        // The art is a 5 x 16 unit barrel (0.25 x 0.8 cells) that pokes 0.7 cells out; scale to this gun.
        let sx = width / 0.25;
        let sy = (protrude + 0.1) / 0.8;
        let size = barrel.cells * Vec2::new(sx, sy);
        m.region_at(&barrel, to_local(tip), size, angle_up_to(dir_local(p.facing)), vcol(Color::WHITE, alpha(p)));
    }

    // 2. Tiles: the full outline in the seam colour, then the fill inset by the seam width.
    for p in &v.parts {
        let mut poly: Vec<Vec2> = p.polygon.iter().map(|&q| to_local(q)).collect();
        ensure_ccw(&mut poly);
        m.polygon(&poly, white, vcol(style.seam, alpha(p)));
        let inner = inset_convex(&poly, style.seam_width);
        m.polygon(&inner, white, vcol(p.fill, alpha(p)));
    }

    // 3. Icon glows, then the icons.
    let icon_box = |p: &PartVisual| {
        let c = to_local(p.center);
        (c - p.dims / 2.0, c + p.dims / 2.0)
    };
    if glow {
        for p in &v.parts {
            let Some(key) = &p.icon else { continue };
            let Some(r) = atlas.get(&format!("glow/{key}")) else { continue };
            let (min, max) = icon_box(p);
            m.region_box(r, min, max, p.rot, p.flip, vcol(Color::WHITE, if p.main { 1.0 } else { 0.4 }));
        }
    }
    for p in &v.parts {
        let Some(key) = &p.icon else { continue };
        let Some(r) = atlas.get(&format!("icon/{key}")) else {
            warn_once!("no atlas icon for `{key}`");
            continue;
        };
        let (min, max) = icon_box(p);
        m.region_box(r, min, max, p.rot, p.flip, vcol(Color::WHITE, if p.main { 1.0 } else { 0.4 }));
    }

    // 4. Hull edge of the main piece: soft halo, then the crisp line.
    let halo = atlas.region("fx/halo");
    let white_r = atlas.region("fx/white");
    for (pts, closed) in &v.hull_loops {
        let local: Vec<Vec2> = pts.iter().map(|&q| to_local(q)).collect();
        if glow {
            m.strip(&local, *closed, halo.cells.x / 2.0, &halo, vcol(v.glow, style.halo_opacity));
        }
        m.strip(&local, *closed, style.line_width / 2.0, &white_r, vcol(v.glow, style.line_opacity));
    }

    // 5. Muzzle glows: at the barrel tip of guns with a barrel and a clear line of fire; guns without a
    //    barrel always show theirs at the front edge of the tile (as in the reference render).
    let soft = atlas.region("fx/dot_soft");
    let hard = atlas.region("fx/dot_hard");
    for p in &v.parts {
        let Some(g) = p.gun else { continue };
        if (g.barrel.is_some() && !g.clear) || !p.main {
            continue;
        }
        let front = p.center + p.facing * (extent_along(p) / 2.0);
        let at = match g.barrel {
            Some((_, protrude)) => front + p.facing * protrude,
            None => front - p.facing * 0.05,
        };
        let at = to_local(at);
        if glow {
            // A blurred disc (radius r, blur 0.08 cells) is close to a Gaussian of sigma sqrt(0.08^2 + r^2/4).
            let sigma = (0.08f32 * 0.08 + g.muzzle_radius * g.muzzle_radius / 4.0).sqrt();
            m.region_at(&soft, at, Vec2::splat(6.0 * sigma), 0.0, vcol(g.color, 0.8));
        }
        let core = g.muzzle_radius * 0.43 * 2.0 / 0.92;
        m.region_at(&hard, at, Vec2::splat(core), 0.0, vcol(core_tint(g.color), 1.0));
    }

    // 6. Navigation lights: a blurred dot (radius 0.15 cells) with a white core (radius 0.055 cells).
    for &(at, color) in &v.nav_lights {
        let at = to_local(at);
        if glow {
            let sigma = (0.08f32 * 0.08 + 0.15 * 0.15 / 4.0).sqrt();
            m.region_at(&soft, at, Vec2::splat(6.0 * sigma), 0.0, vcol(color, 0.75));
        }
        m.region_at(&hard, at, Vec2::splat(0.11 / 0.92), 0.0, vcol(Color::WHITE, 1.0));
    }

    m.into_mesh()
}

/// The near-white centre of a glowing dot in the hue of its glow.
fn core_tint(c: Color) -> Color {
    let s = c.to_srgba();
    Color::srgb(0.85 + s.red * 0.15, 0.85 + s.green * 0.15, 0.85 + s.blue * 0.15)
}

/// Debug shapes: one flat polygon per part, no seams, icons, glow or effects.
fn build_debug(v: &ShipVisual, atlas: &Atlas) -> Mesh {
    let to_local = |p: Vec2| Vec2::new(p.x - v.pivot.x, v.pivot.y - p.y);
    let white = atlas.region("fx/white").uv_center();
    let mut m = MeshBuf::default();
    for p in &v.parts {
        let mut poly: Vec<Vec2> = p.polygon.iter().map(|&q| to_local(q)).collect();
        ensure_ccw(&mut poly);
        let inner = inset_convex(&poly, 0.05);
        m.polygon(&inner, white, vcol(p.debug_color, if p.main { 1.0 } else { 0.5 }));
    }
    m.into_mesh()
}

/// Dotted shield rings: round dots 0.08 cells across, 0.225 cells apart (style guide: 4.5 units on a
/// 20-unit cell), at the shield's radius around the emitter.
fn build_shields(v: &ShipVisual, atlas: &Atlas, style: &Style) -> Option<Mesh> {
    let to_local = |p: Vec2| Vec2::new(p.x - v.pivot.x, v.pivot.y - p.y);
    let hard: Region = atlas.region("fx/dot_hard");
    let mut m = MeshBuf::default();
    for p in &v.parts {
        let Some(r) = p.shield_radius else { continue };
        if !p.main {
            continue;
        }
        let c = to_local(p.center);
        let n = ((std::f32::consts::TAU * r) / 0.225).round().max(8.0) as usize;
        for k in 0..n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU;
            let at = c + Vec2::new(a.cos(), a.sin()) * r;
            m.region_at(&hard, at, Vec2::splat(0.08 / 0.92), 0.0, vcol(style.shield_color, style.shield_opacity));
        }
    }
    (!m.is_empty()).then(|| m.into_mesh())
}

// ---------------------------------------------------------------------------------------------------------
// Per-frame systems

pub struct ShipViewPlugin;

impl Plugin for ShipViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (update_plumes, apply_toggles));
    }
}

/// Plume length follows the throttle; a slight flicker keeps it alive.
fn update_plumes(
    time: Res<Time>,
    ships: Query<(&ShipView, &EngineThrottle)>,
    mut plumes: Query<(&Plume, &mut Transform, &mut Sprite)>,
) {
    let t = time.elapsed_secs();
    for (view, throttle) in &ships {
        for (i, &entity) in view.plumes.iter().enumerate() {
            let Ok((plume, mut tf, mut sprite)) = plumes.get_mut(entity) else { continue };
            let mount = &view.engines[plume.index];
            let th = throttle.0.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
            let flicker = 1.0 + 0.04 * (t * 23.0 + i as f32 * 1.7).sin() + 0.03 * (t * 37.0 + i as f32).sin();
            let len = (mount.len_idle + (mount.len_full - mount.len_idle) * th) * flicker;
            tf.scale.y = len / plume.art_len;
            tf.scale.x = 0.85 + 0.15 * th;
            let a = if th > 0.01 { 0.55 + 0.45 * th } else { 0.0 };
            if (sprite.color.alpha() - a).abs() > 0.005 {
                sprite.color = Color::WHITE.with_alpha(a);
            }
        }
    }
}

fn apply_toggles(
    toggles: Res<RenderToggles>,
    ships: Query<&ShipView>,
    mut bodies: Query<&mut Mesh2d, With<ShipBody>>,
    mut vis: Query<&mut Visibility>,
) {
    if !toggles.is_changed() {
        return;
    }
    for view in &ships {
        if let Ok(mut mesh) = bodies.get_mut(view.body) {
            let want = if toggles.debug_shapes {
                &view.mesh_debug
            } else if toggles.glow_sprites {
                &view.mesh_style
            } else {
                &view.mesh_style_noglow
            };
            if mesh.0 != *want {
                mesh.0 = want.clone();
            }
        }
        if let Ok(mut v) = vis.get_mut(view.aura) {
            *v = visible(!toggles.debug_shapes && toggles.glow_sprites);
        }
        for &p in &view.plumes {
            if let Ok(mut v) = vis.get_mut(p) {
                *v = visible(!toggles.debug_shapes);
            }
        }
        if let Some(s) = view.shields
            && let Ok(mut v) = vis.get_mut(s)
        {
            *v = visible(toggles.shields && !toggles.debug_shapes);
        }
    }
}
