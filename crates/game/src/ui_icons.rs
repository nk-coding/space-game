//! Vector icons inside bevy_ui nodes: put a [`UiIcon`] on a node and it is drawn at the node's laid-out
//! rectangle every frame.
//!
//! How (12 Oct 2026): icons are world-space meshes of the main camera, built by the same code as the ships
//! (`ship_view::build_body`, the shared atlas material), placed over the node through the camera rig (screen →
//! world) and scaled by 1 / zoom, at a z above every world layer. So a part icon in the palette *is* a flight
//! tile: the same tile colour, seams, icon and icon glow, composited in sRGB like the ships (no colour shift,
//! which bevy_ui's own image nodes would give, because bevy_ui does not encode for sRGB compositing), and sharp
//! at any UI size because the atlas tier follows the icons too ([`UiAtlasNeed`]). Glyphs Barlow lacks (arrows,
//! the play triangle, the search lens, the menu dots, the bonus diamond) are small meshes of the same kind.
//!
//! The world draws before bevy_ui, so UI nodes are drawn over icons: give the nodes behind an icon at most a
//! faint background. Icons follow their node's visibility.

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiSystems};
use bevy::window::PrimaryWindow;
use data::{Blueprint, Placement};

use crate::RenderToggles;
use crate::atlas::{Atlas, UiAtlasNeed};
use crate::camera::{CameraRig, CameraSet};
use crate::meshbuf::MeshBuf;
use crate::ship_model::design_visual;
use crate::ship_view::{ShipVisual, build_body};
use crate::sim_bridge::GameContent;
use crate::style::{Style, vcol};

/// Above every world layer (ships ≤ 100, effects ≤ 400), below the camera's far plane.
const Z_UI_ICON: f32 = 600.0;

/// What a UI node shows.
#[derive(Component, Clone, Debug, PartialEq)]
pub enum UiIcon {
    /// A part as one flight tile (hull parts in the material's armour tone), fitted into the node.
    Part { part: String, material: Option<String> },
    /// A vector glyph in a colour, fitted into the node.
    Glyph { glyph: Glyph, color: Color },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Undo,
    Redo,
    Play,
    Search,
    /// The ring menu: a ring of dots around a centre dot.
    Menu,
    Diamond,
}

/// The mesh drawing one node's icon.
struct IconMesh {
    entity: Entity,
    mesh: Handle<Mesh>,
    /// What the mesh shows: the icon, the glyph's size in px (glyph strokes keep their pixel width) and the
    /// glow setting; and for parts the footprint in cells.
    built: Option<(UiIcon, u32, bool)>,
    dims: Vec2,
}

#[derive(Resource, Default)]
struct IconMeshes(BTreeMap<Entity, IconMesh>);

#[derive(Component)]
struct UiIconMesh;

pub struct UiIconsPlugin;

impl Plugin for UiIconsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<IconMeshes>().add_systems(
            PostUpdate,
            draw_icons.after(UiSystems::Layout).after(CameraSet).before(bevy::transform::TransformSystems::Propagate),
        );
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw_icons(
    mut commands: Commands,
    mut state: ResMut<IconMeshes>,
    nodes: Query<(Entity, &UiIcon, &ComputedNode, &UiGlobalTransform, &InheritedVisibility)>,
    mut placed: Query<(&mut Transform, &mut Visibility), With<UiIconMesh>>,
    mut removed: RemovedComponents<UiIcon>,
    mut meshes: ResMut<Assets<Mesh>>,
    rig: Res<CameraRig>,
    window: Single<&Window, With<PrimaryWindow>>,
    atlas: Res<Atlas>,
    style: Res<Style>,
    content: Res<GameContent>,
    toggles: Res<RenderToggles>,
    mut need: ResMut<UiAtlasNeed>,
) {
    for node in removed.read() {
        if let Some(m) = state.0.remove(&node) {
            commands.entity(m.entity).despawn();
        }
    }
    let scale = window.scale_factor();
    let mut px_per_cell = 0.0f32;
    for (node_e, icon, node, tf, vis) in &nodes {
        let m = state.0.entry(node_e).or_insert_with(|| {
            let mesh = meshes.add(MeshBuf::placeholder());
            let entity = commands
                .spawn((
                    UiIconMesh,
                    Mesh2d(mesh.clone()),
                    MeshMaterial2d(atlas.material.clone()),
                    Transform::default(),
                    Visibility::Hidden,
                ))
                .id();
            IconMesh { entity, mesh, built: None, dims: Vec2::ONE }
        });
        let Ok((mut t, mut v)) = placed.get_mut(m.entity) else { continue };
        let size = node.size / scale;
        let show = vis.get() && size.cmpgt(Vec2::splat(1.0)).all();
        let want = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
        if !show {
            continue;
        }
        let side = size.min_element();
        let key = (icon.clone(), side.round() as u32, toggles.glow_sprites);
        if m.built.as_ref() != Some(&key) {
            let (mesh, dims) = match icon {
                UiIcon::Part { part, material } => match part_tile(&content.0, &style, &atlas, part, material.as_deref(), toggles.glow_sprites) {
                    Some(x) => x,
                    None => (MeshBuf::placeholder(), Vec2::ONE),
                },
                UiIcon::Glyph { glyph, color } => (glyph_mesh(*glyph, *color, side, &atlas), Vec2::ONE),
            };
            let _ = meshes.insert(&m.mesh, mesh);
            m.built = Some(key);
            m.dims = dims;
        }
        // Parts: cells fitted into the node; glyphs are built at the node's size in pixels.
        let px = match icon {
            UiIcon::Part { .. } => {
                let k = (size.x / m.dims.x).min(size.y / m.dims.y);
                px_per_cell = px_per_cell.max(k);
                k
            }
            UiIcon::Glyph { .. } => 1.0,
        };
        // Written only when it moves: a written transform is propagated again.
        let (at, k) = (rig.screen_to_world(tf.translation / scale).extend(Z_UI_ICON), Vec3::new(px / rig.zoom, px / rig.zoom, 1.0));
        if t.translation != at || t.scale != k {
            t.translation = at;
            t.scale = k;
        }
    }
    if need.0 != px_per_cell {
        need.0 = px_per_cell;
    }
}

/// One part as a single flight tile, centred on the origin in cells; and its footprint. No hull edge, aura,
/// barrel or muzzle: an icon, not a ship.
fn part_tile(
    content: &data::Content,
    style: &Style,
    atlas: &Atlas,
    part: &str,
    material: Option<&str>,
    glow: bool,
) -> Option<(Mesh, Vec2)> {
    let def = content.parts.get(part)?;
    let bp = Blueprint {
        id: String::new(),
        name: String::new(),
        summary: String::new(),
        description: None,
        lore: None,
        tags: Vec::new(),
        faction: String::new(),
        size: def.size,
        parts: vec![Placement {
            part: part.into(),
            x: 0,
            y: 0,
            rot: 0,
            flip: false,
            material: material.filter(|_| def.hull).map(str::to_string),
            power: None,
        }],
        behavior: None,
        source: Default::default(),
    };
    let dv = design_visual(content, &bp, style).ok()?;
    let mut p = dv.parts.into_iter().next()?;
    p.gun = None;
    let dims = Vec2::new(def.size.0 as f32, def.size.1 as f32);
    let v = ShipVisual {
        parts: vec![p],
        hull_loops: Vec::new(),
        pivot: dims / 2.0,
        bounds_min: Vec2::ZERO,
        bounds_max: dims,
        glow: dv.glow,
        enemy: false,
        nav_lights: Vec::new(),
        aura: false,
    };
    Some((build_body(&v, atlas, style, glow, true), dims))
}

/// A glyph `side` px across, centred on the origin, in pixels (y up). Strokes are 1.6 px wide.
fn glyph_mesh(glyph: Glyph, color: Color, side: f32, atlas: &Atlas) -> Mesh {
    let white = atlas.region("fx/white");
    let uv = white.uv_center();
    let col = vcol(color, 1.0);
    let s = side;
    let w = 0.8;
    let mut m = MeshBuf::default();
    let arc = |c: Vec2, r: f32, a0: f32, a1: f32, n: usize| -> Vec<Vec2> {
        (0..=n).map(|k| c + Vec2::from_angle(a0 + (a1 - a0) * k as f32 / n as f32) * r).collect()
    };
    match glyph {
        Glyph::Undo | Glyph::Redo => {
            // A U-turn arrow (drawn for undo, mirrored for redo): the top stroke runs to the head on the left, a
            // half turn bulges right, the bottom stroke runs back. 2D meshes are not culled, so the mirrored
            // winding draws too.
            let f = if glyph == Glyph::Undo { 1.0 } else { -1.0 };
            let p = |q: Vec2| Vec2::new(f * q.x * s, q.y * s);
            let mut stroke = vec![Vec2::new(-0.2, 0.15)];
            stroke.extend(arc(Vec2::new(0.06, -0.02), 0.17, std::f32::consts::FRAC_PI_2, -std::f32::consts::FRAC_PI_2, 12));
            stroke.push(Vec2::new(-0.14, -0.19));
            let stroke: Vec<Vec2> = stroke.into_iter().map(p).collect();
            m.strip(&stroke, false, w, &white, col);
            let head = [Vec2::new(-0.36, 0.15), Vec2::new(-0.17, 0.3), Vec2::new(-0.17, 0.0)].map(p);
            m.polygon(&head, uv, col);
        }

        Glyph::Play => {
            m.polygon(&[Vec2::new(-0.22 * s, 0.28 * s), Vec2::new(-0.22 * s, -0.28 * s), Vec2::new(0.3 * s, 0.0)], uv, col);
        }
        Glyph::Search => {
            let c = Vec2::new(-0.07 * s, 0.07 * s);
            m.strip(&arc(c, 0.26 * s, 0.0, std::f32::consts::TAU, 28), true, w, &white, col);
            let d = Vec2::new(1.0, -1.0).normalize();
            m.strip(&[c + d * 0.27 * s, c + d * 0.55 * s], false, w, &white, col);
        }
        Glyph::Menu => {
            let dot = |m: &mut MeshBuf, at: Vec2, r: f32| m.polygon(&arc(at, r, 0.0, std::f32::consts::TAU, 12)[..12], uv, col);
            for k in 0..10 {
                dot(&mut m, Vec2::from_angle(k as f32 * std::f32::consts::TAU / 10.0) * 0.36 * s, 0.045 * s);
            }
            dot(&mut m, Vec2::ZERO, 0.09 * s);
        }
        Glyph::Diamond => {
            m.polygon(&[Vec2::new(0.0, 0.42 * s), Vec2::new(-0.42 * s, 0.0), Vec2::new(0.0, -0.42 * s), Vec2::new(0.42 * s, 0.0)], uv, col);
        }
    }
    m.into_mesh()
}
