//! Style 2a rendering of sim pieces. One entity hierarchy per live piece (one rigid body in the sim):
//!
//! ```text
//! piece root  Transform (= piece.render_pose(alpha) every frame, z = its draw layer), PieceView
//! ├── body     Mesh2d, alpha-blended: aura, barrels, [seams, tiles,] hull halo + line, machine glows, machines
//! │            (ship drawings; icons on UI tiles), muzzle glows, nav lights
//! └── tiles    Mesh2d, opaque (lit pieces only): seams and tiles, writing depth
//! ```
//!
//! Plumes, trails, beams, tracers and sparks are immediate-mode FX sprites (`fx.rs`); shield bubbles are one
//! batched mesh for all ships (`shield.rs`), placed from the piece views. The body mesh has five
//! variants (style 2a lit/dark, without pre-blurred glow lit/dark, debug shapes), built lazily on first use;
//! toggles and power changes swap the handle. A lit piece's tiles (alpha 1) are a mesh of their own in the
//! opaque pass, so they write depth: every shield bubble quad sits just above its own piece's layer
//! ([`BUBBLE_DZ`]) and is hidden under the tiles of every piece above it, although all bubbles are one draw
//! after every ship. Dark pieces (tiles at half alpha) keep their tiles in the body. Local coordinates are the
//! sim's piece frame (`blueprint (x, y) → (x − origin.x, origin.y − y)`), so the transform is the sim pose as is.

use bevy::prelude::*;
use sim::{PieceId, ShipId};

use crate::RenderToggles;
use crate::atlas::Atlas;
use crate::meshbuf::{MeshBuf, ensure_ccw, inset_convex};
use crate::scene::PilotFocus;
use crate::ship_model::DesignVisual;
use crate::sim_bridge::{SimWorld, v2};
use crate::style::{Style, vcol};

// ---------------------------------------------------------------------------------------------------------
// Render-ready description (blueprint frame: metres, y down, forward = −y).

#[derive(Clone, Debug)]
pub struct ShipVisual {
    pub parts: Vec<PartVisual>,
    /// Hull outline loops (blueprint frame); `bool` = closed.
    pub hull_loops: Vec<(Vec<Vec2>, bool)>,
    /// Blueprint point at the local origin (the sim's piece frame origin).
    pub pivot: Vec2,
    pub bounds_min: Vec2,
    pub bounds_max: Vec2,
    pub glow: Color,
    pub enemy: bool,
    /// Navigation lights (blueprint frame position, colour).
    pub nav_lights: Vec<(Vec2, Color)>,
    /// The faint aura behind lit ships in space (not in the drydock or on UI icons).
    pub aura: bool,
}

#[derive(Clone, Debug)]
pub struct PartVisual {
    /// Part index in the blueprint (`Blueprint::parts`, = sim `PartIdx`).
    pub index: u32,
    pub polygon: Vec<Vec2>,
    /// The tile's colour: a hull part's armour tone; under a machine the hull's tone (GDR-0069,
    /// `ship_model::HullTones`), on UI tiles the machine tile colour.
    pub fill: Color,
    pub debug_color: Color,
    /// The machine's ship drawing (atlas `ship/<key>`, GDR-0069); drawn instead of the icon.
    pub art: Option<String>,
    /// How far the ship drawing reaches past the part's stern, in cells (engines' nozzle bells; 0 for the rest).
    pub overhang: f32,
    /// Icon key (`content/base/icons/<key>.svg`, atlas `icon/<key>`): drawn on UI tiles, where `art` is `None`.
    pub icon: Option<String>,
    /// Centre of the rotated footprint.
    pub center: Vec2,
    /// Rotated footprint size in cells.
    pub dims: Vec2,
    pub rot: u8,
    pub flip: bool,
    /// Unit vector the part faces (blueprint frame).
    pub facing: Vec2,
    pub gun: Option<GunVisual>,
    pub shield_radius: Option<f32>,
}

#[derive(Clone, Copy, Debug)]
pub struct GunVisual {
    /// Line of fire clear (barrel and muzzle glow are only drawn then).
    pub clear: bool,
    /// Barrel `(width, protrusion beyond the front edge)` in cells.
    pub barrel: Option<(f32, f32)>,
    pub muzzle_radius: f32,
    pub color: Color,
}

impl ShipVisual {
    pub fn to_local(&self, p: Vec2) -> Vec2 {
        Vec2::new(p.x - self.pivot.x, self.pivot.y - p.y)
    }
}

// ---------------------------------------------------------------------------------------------------------
// Components

#[derive(Component)]
pub struct PieceView {
    pub ship: ShipId,
    pub piece: PieceId,
    pub visual: ShipVisual,
    meshes: [Option<Handle<Mesh>>; 5],
    variant: usize,
    pub body: Entity,
    /// The opaque tiles (spawned when the piece is first drawn lit).
    tiles: Option<Entity>,
    /// Hull glow on (the piece is active).
    pub lit: bool,
    /// The piece's own draw layer ([`draw_layer`]); the focused vessel draws at [`Z_FOCUS`] instead.
    pub z: f32,
    pub enemy: bool,
    pub glow: Color,
    pub weapon_color: Color,
    /// Trail emission accumulators per thruster row (resized when the thruster table changes).
    pub emit: Vec<f32>,
    /// Centre of the piece's bounds (piece frame) and the radius around it.
    pub center: Vec2,
    pub radius: f32,
}

#[derive(Component)]
pub struct PieceBody;

#[derive(Component)]
pub struct PieceTiles;

/// The focused vessel's piece draws above every other ship (their layers are z 10..90).
pub const Z_FOCUS: f32 = 100.0;
/// A shield bubble's depth above its own piece: under the next piece up (layers are 1/8 apart, `shield.rs`).
pub const BUBBLE_DZ: f32 = 1.0 / 16.0;
/// Aura and barrels lie this far below their piece's tiles in depth: under its own tiles, over every lower piece.
const UNDER_DZ: f32 = -1.0 / 64.0;

/// A piece's draw layer (z): its ship's layer, `10 + ship id mod 80`, plus an eighth per piece id mod 8. Fixed
/// for the ship's whole life, so a view rebuilt when a part dies keeps its place among overlapping ships, and a
/// split-off piece's layer follows from its ids (a piece that becomes a vessel takes its new ship's layer).
pub fn draw_layer(ship: ShipId, piece: PieceId) -> f32 {
    10.0 + (ship.0 % 80) as f32 + (piece.0 % 8) as f32 / 8.0
}

fn variant(toggles: &RenderToggles, lit: bool) -> usize {
    match (toggles.debug_shapes, toggles.glow_sprites, lit) {
        (true, _, _) => 4,
        (false, true, true) => 0,
        (false, true, false) => 1,
        (false, false, true) => 2,
        (false, false, false) => 3,
    }
}

/// The lit variants draw their tiles in the opaque tiles mesh.
fn opaque_tiles(k: usize) -> bool {
    k == 0 || k == 2
}

fn build_variant(v: &ShipVisual, k: usize, atlas: &Atlas, style: &Style) -> Mesh {
    match k {
        0 => body_mesh(v, atlas, style, true, true, Layers::Overlays),
        1 => body_mesh(v, atlas, style, true, false, Layers::All),
        2 => body_mesh(v, atlas, style, false, true, Layers::Overlays),
        3 => body_mesh(v, atlas, style, false, false, Layers::All),
        _ => build_debug(v, atlas),
    }
}

/// The opaque tiles of a lit piece, a child of its root.
fn spawn_tiles(commands: &mut Commands, meshes: &mut Assets<Mesh>, v: &ShipVisual, atlas: &Atlas, style: &Style) -> Entity {
    let mesh = meshes.add(body_mesh(v, atlas, style, false, true, Layers::Tiles));
    commands.spawn((PieceTiles, Mesh2d(mesh), MeshMaterial2d(atlas.opaque.clone()), Transform::default())).id()
}

/// Spawns the view of one sim piece.
#[allow(clippy::too_many_arguments)]
pub fn spawn_piece_view(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    atlas: &Atlas,
    style: &Style,
    toggles: &RenderToggles,
    dv: &DesignVisual,
    ship: &sim::Ship,
    piece: &sim::Piece,
    layer: f32,
    z: f32,
    alpha: f32,
) -> Entity {
    let visual = crate::ship_model::piece_visual(dv, ship, piece);
    let lit = piece.active;
    let k = variant(toggles, lit);
    let mut mesh_slots: [Option<Handle<Mesh>>; 5] = Default::default();
    let mesh = meshes.add(build_variant(&visual, k, atlas, style));
    mesh_slots[k] = Some(mesh.clone());

    let (pos, angle) = piece.render_pose(alpha);
    let root = commands
        .spawn((
            Transform::from_xyz(pos.x, pos.y, z).with_rotation(Quat::from_rotation_z(angle)),
            Visibility::default(),
        ))
        .id();
    let body = commands
        .spawn((PieceBody, Mesh2d(mesh), MeshMaterial2d(atlas.material.clone()), Transform::default()))
        .id();
    let tiles = opaque_tiles(k).then(|| spawn_tiles(commands, meshes, &visual, atlas, style));
    commands.entity(root).add_children(tiles.as_slice());

    let lc = visual.to_local((visual.bounds_min + visual.bounds_max) / 2.0);
    let radius = (visual.bounds_max - visual.bounds_min).length() / 2.0;
    commands.entity(root).add_child(body).insert(PieceView {
        ship: ship.id(),
        piece: piece.id,
        enemy: visual.enemy,
        glow: visual.glow,
        weapon_color: dv.weapon_color,
        visual,
        meshes: mesh_slots,
        variant: k,
        body,
        tiles,
        lit,
        z: layer,
        emit: Vec::new(),
        center: lc,
        radius,
    });
    root
}

/// Every frame: pose from the sim (interpolated), lit/dark and toggle mesh swaps (the opaque tiles show only
/// with a lit variant).
#[allow(clippy::too_many_arguments)]
pub fn sync_piece_views(
    mut commands: Commands,
    world: Res<SimWorld>,
    focus: Res<PilotFocus>,
    fixed: Res<Time<Fixed>>,
    toggles: Res<RenderToggles>,
    atlas: Res<Atlas>,
    style: Res<Style>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut views: Query<(Entity, &mut PieceView, &mut Transform)>,
    mut bodies: Query<&mut Mesh2d, With<PieceBody>>,
    mut tiles: Query<&mut Visibility, With<PieceTiles>>,
) {
    let alpha = fixed.overstep_fraction().clamp(0.0, 1.0);
    for (root, mut view, mut tf) in &mut views {
        let Some(ship) = world.sim.ship(view.ship) else { continue };
        let Some(piece) = ship.piece(view.piece) else { continue };
        let (pos, angle) = piece.render_pose(alpha);
        let z = if focus.0 == Some(view.ship) && piece.controlled { Z_FOCUS } else { view.z };
        tf.translation = Vec3::new(pos.x, pos.y, z);
        tf.rotation = Quat::from_rotation_z(angle);

        let lit = piece.active;
        let k = variant(&toggles, lit);
        if k != view.variant || lit != view.lit || toggles.is_changed() {
            if view.meshes[k].is_none() {
                let mesh = meshes.add(build_variant(&view.visual, k, &atlas, &style));
                view.meshes[k] = Some(mesh);
            }
            if let (Ok(mut m), Some(h)) = (bodies.get_mut(view.body), view.meshes[k].clone())
                && m.0 != h
            {
                m.0 = h;
            }
            match view.tiles {
                Some(e) => {
                    if let Ok(mut vis) = tiles.get_mut(e) {
                        vis.set_if_neq(if opaque_tiles(k) { Visibility::Inherited } else { Visibility::Hidden });
                    }
                }
                None if opaque_tiles(k) => {
                    let e = spawn_tiles(&mut commands, &mut meshes, &view.visual, &atlas, &style);
                    commands.entity(root).add_child(e);
                    view.tiles = Some(e);
                }
                None => {}
            }
            view.variant = k;
            view.lit = lit;
        }
    }
}

/// World position of a piece-frame point at the render pose.
pub fn world_of(pos: sim::Vec2, angle: f32, local: Vec2) -> Vec2 {
    let (s, c) = angle.sin_cos();
    v2(pos) + Vec2::new(local.x * c - local.y * s, local.x * s + local.y * c)
}

// ---------------------------------------------------------------------------------------------------------
// Meshes

/// The extent of a part along its facing axis.
fn extent_along(p: &PartVisual) -> f32 {
    if p.facing.x.abs() > 0.5 { p.dims.x } else { p.dims.y }
}

/// Angle (CCW, y up) that turns art pointing up (local +y) to point along `d` (y up).
fn angle_up_to(d: Vec2) -> f32 {
    (-d.x).atan2(d.y)
}

/// Which layers of the body a mesh holds.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Layers {
    All,
    /// Seams and tiles only (the opaque mesh of a lit piece).
    Tiles,
    /// Everything but the seams and tiles; what draws under them sits [`UNDER_DZ`] deeper.
    Overlays,
}

/// The body in style 2a, bottom to top. `glow` adds the pre-blurred layers (machine glows, hull halo, soft muzzle
/// glows); `lit` = an active piece (hull edge, full brightness). Dark pieces (salvage, unpowered) are dimmed
/// and lose the hull glow. Also draws the drydock's ship and the part icons in its UI (`ui_icons`), so every
/// tile looks the same everywhere.
pub(crate) fn build_body(v: &ShipVisual, atlas: &Atlas, style: &Style, glow: bool, lit: bool) -> Mesh {
    body_mesh(v, atlas, style, glow, lit, Layers::All)
}

fn body_mesh(v: &ShipVisual, atlas: &Atlas, style: &Style, glow: bool, lit: bool, layers: Layers) -> Mesh {
    let to_local = |p: Vec2| v.to_local(p);
    let dir_local = |d: Vec2| Vec2::new(d.x, -d.y);
    let white = atlas.region("fx/white").uv_center();
    let (tile_a, icon_a) = if lit { (1.0, 1.0) } else { (0.5, 0.4) };
    let mut m = MeshBuf::default();
    let overlays = layers != Layers::Tiles;
    if layers == Layers::Overlays {
        m.set_z(UNDER_DZ);
    }

    // 0. Aura behind lit pieces: centred on the bounding box, radius 0.85 x the larger side (drawAura in the
    //    concept board).
    if overlays && glow && lit && v.aura {
        let c = (to_local(v.bounds_min) + to_local(v.bounds_max)) / 2.0;
        let side = (v.bounds_max - v.bounds_min).max_element();
        let (r, a) = if v.enemy { (atlas.region("fx/aura_linear"), 0.12) } else { (atlas.region("fx/aura"), style.aura_opacity) };
        let d = side * 0.85 * 2.0;
        m.region_at(&r, c, Vec2::splat(d), 0.0, vcol(v.glow, a));
    }

    // 1. Gun barrels, under the tiles.
    let barrel = atlas.region("fx/barrel");
    for p in v.parts.iter().filter(|_| overlays) {
        let Some(g) = p.gun else { continue };
        let Some((width, protrude)) = g.barrel else { continue };
        if !g.clear {
            continue;
        }
        let front = p.center + p.facing * (extent_along(p) / 2.0);
        let tip = front + p.facing * protrude;
        let size = barrel.cells * Vec2::new(width / 0.25, (protrude + 0.1) / 0.8);
        m.region_at(&barrel, to_local(tip), size, angle_up_to(dir_local(p.facing)), vcol(Color::WHITE, tile_a));
    }

    // 2. Tiles: the outline in the seam colour, then the fill inset by the seam width.
    m.set_z(0.0);
    for p in v.parts.iter().filter(|_| layers != Layers::Overlays) {
        let mut poly: Vec<Vec2> = p.polygon.iter().map(|&q| to_local(q)).collect();
        ensure_ccw(&mut poly);
        m.polygon(&poly, white, vcol(style.seam, tile_a));
        let inner = inset_convex(&poly, style.seam_width);
        m.polygon(&inner, white, vcol(p.fill, tile_a));
    }
    if !overlays {
        return m.into_mesh();
    }

    // 3. Hull edge: soft halo, then the crisp line (lit only). Under the machines, so an engine's nozzle bell that
    //    reaches past the hull covers it.
    if lit {
        let halo = atlas.region("fx/halo");
        let white_r = atlas.region("fx/white");
        for (pts, closed) in &v.hull_loops {
            let local: Vec<Vec2> = pts.iter().map(|&q| to_local(q)).collect();
            if glow {
                m.strip(&local, *closed, halo.cells.x / 2.0, &halo, vcol(v.glow, style.halo_opacity));
            }
            m.strip(&local, *closed, style.line_width / 2.0, &white_r, vcol(v.glow, style.line_opacity));
        }
    }

    // 4. Machines: their glows (lit only), then the ship drawings (GDR-0069), rotated and mirrored with the part,
    //    an engine's reaching `overhang` cells past its stern; a part without a drawing shows its icon (UI tiles).
    let machines = || {
        v.parts.iter().filter_map(|p| match (&p.art, &p.icon) {
            (Some(key), _) => Some((p, "ship", key, p.overhang)),
            (None, Some(key)) => Some((p, "icon", key, 0.0)),
            _ => None,
        })
    };
    let art_box = |p: &PartVisual, overhang: f32| {
        let c = to_local(p.center);
        let (mut min, mut max) = (c - p.dims / 2.0, c + p.dims / 2.0);
        let stern = dir_local(-p.facing);
        if stern.x > 0.5 {
            max.x += overhang;
        } else if stern.x < -0.5 {
            min.x -= overhang;
        } else if stern.y > 0.5 {
            max.y += overhang;
        } else {
            min.y -= overhang;
        }
        (min, max)
    };
    if glow && lit {
        for (p, kind, key, overhang) in machines() {
            let name = if kind == "ship" { format!("ship-glow/{key}") } else { format!("glow/{key}") };
            let Some(r) = atlas.get(&name) else { continue };
            let (min, max) = art_box(p, overhang);
            m.region_box(r, min, max, p.rot, p.flip, vcol(Color::WHITE, 1.0));
        }
    }
    for (p, kind, key, overhang) in machines() {
        let Some(r) = atlas.get(&format!("{kind}/{key}")) else {
            warn_once!("no atlas region for `{kind}/{key}`");
            continue;
        };
        let (min, max) = art_box(p, overhang);
        m.region_box(r, min, max, p.rot, p.flip, vcol(Color::WHITE, icon_a));
    }

    if !lit {
        return m.into_mesh();
    }

    // 5. Muzzle glows: barrel tips of clear guns; barrel-less guns at their front edge.
    let soft = atlas.region("fx/dot_soft");
    let hard = atlas.region("fx/dot_hard");
    for p in &v.parts {
        let Some(g) = p.gun else { continue };
        if g.barrel.is_some() && !g.clear {
            continue;
        }
        let front = p.center + p.facing * (extent_along(p) / 2.0);
        let at = match g.barrel {
            Some((_, protrude)) => front + p.facing * protrude,
            None => front - p.facing * 0.05,
        };
        let at = to_local(at);
        if glow {
            let sigma = (0.08f32 * 0.08 + g.muzzle_radius * g.muzzle_radius / 4.0).sqrt();
            m.region_at(&soft, at, Vec2::splat(6.0 * sigma), 0.0, vcol(g.color, 0.8));
        }
        let core = g.muzzle_radius * 0.43 * 2.0 / 0.92;
        m.region_at(&hard, at, Vec2::splat(core), 0.0, vcol(core_tint(g.color), 1.0));
    }

    // 6. Navigation lights.
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
pub fn core_tint(c: Color) -> Color {
    let s = c.to_srgba();
    Color::srgb(0.85 + s.red * 0.15, 0.85 + s.green * 0.15, 0.85 + s.blue * 0.15)
}

/// Debug shapes: one flat polygon per part, no seams, icons, glow or effects.
fn build_debug(v: &ShipVisual, atlas: &Atlas) -> Mesh {
    let white = atlas.region("fx/white").uv_center();
    let mut m = MeshBuf::default();
    for p in &v.parts {
        let mut poly: Vec<Vec2> = p.polygon.iter().map(|&q| v.to_local(q)).collect();
        ensure_ccw(&mut poly);
        let inner = inset_convex(&poly, 0.05);
        m.polygon(&inner, white, vcol(p.debug_color, 1.0));
    }
    m.into_mesh()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Draw layers are a function of the ids alone, so a view rebuilt after a part dies keeps its place (review
    /// finding 3), and the depths line up: a bubble lies over its own piece's tiles and under every piece of a
    /// higher layer, aura and barrels included; trails stay under every ship, the focus above every layer.
    #[test]
    fn draw_layers_are_stable_and_keep_bubbles_between_ships() {
        let mut layers: Vec<f32> =
            (0..80).flat_map(|s| (0..8).map(move |p| draw_layer(ShipId(s), PieceId(p)))).collect();
        layers.sort_by(f32::total_cmp);
        layers.dedup();
        assert_eq!(layers.len(), 80 * 8, "80 ships x 8 pieces, all apart");
        for w in layers.windows(2) {
            let (below, above) = (w[0], w[1]);
            assert!(below + BUBBLE_DZ < above + UNDER_DZ, "{below} {above}");
        }
        assert!(layers[0] + UNDER_DZ > crate::fx::Z_TRAILS);
        assert!(layers[layers.len() - 1] + BUBBLE_DZ < Z_FOCUS);
        // Ids wrap after 80 ships and 8 pieces: the layer never leaves its range.
        assert_eq!(draw_layer(ShipId(85), PieceId(9)), draw_layer(ShipId(5), PieceId(1)));
    }
}
