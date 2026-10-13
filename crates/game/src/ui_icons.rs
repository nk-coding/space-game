//! Vector icons inside bevy_ui nodes: put a [`UiIcon`] on a node and it is drawn at the node's laid-out
//! rectangle every frame.
//!
//! How (12 Oct 2026): icons are world-space meshes of the main camera, built by the same code as the ships
//! (`ship_view::build_body`, the shared atlas material), placed over the node through the camera rig (screen →
//! world) and scaled by 1 / zoom, at a z above every world layer. So a part icon in the palette is drawn like a
//! flight tile: the same seams and glow, composited in sRGB like the ships (no colour shift,
//! which bevy_ui's own image nodes would give, because bevy_ui does not encode for sRGB compositing), and sharp
//! at any UI size because the atlas tier follows the icons too ([`UiAtlasNeed`]). A whole design (FLEET's ships)
//! is the drydock's view of it, fitted into the node. A machine's tile shows its icon on the machine tile colour,
//! where ships show its drawing on a hull-toned cell (GDR-0069). Glyphs Barlow lacks (arrows, the play triangle, the search
//! lens, the menu dots, the bonus diamond, the lock) are small meshes of the same kind.
//!
//! The world draws before bevy_ui, so UI nodes are drawn over icons: give the nodes behind an icon at most a
//! faint background. Icons follow their node's visibility, and hide when a scrolled parent clips them (an icon
//! is a world mesh: bevy_ui's clipping does not reach it).

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy::ui::{CalculatedClip, ComputedNode, UiGlobalTransform, UiSystems};
use bevy::window::PrimaryWindow;
use data::{Blueprint, Placement};

use crate::RenderToggles;
use crate::atlas::{Atlas, UiAtlasNeed};
use crate::camera::{CameraRig, CameraSet};
use crate::meshbuf::MeshBuf;
use crate::ship_model::{blueprint_visual, design_visual};
use crate::ship_view::{ShipVisual, build_body};
use crate::sim_bridge::GameContent;
use crate::style::{Style, vcol};

/// Above every world layer (ships ≤ 100, effects ≤ 400), below the camera's far plane.
const Z_UI_ICON: f32 = 600.0;

/// What a UI node shows.
#[derive(Component, Clone, Debug, PartialEq)]
pub enum UiIcon {
    /// A part as one tile (hull parts in the material's armour tone, machines as their icon on the machine tile
    /// colour), fitted into the node.
    Part { part: String, material: Option<String> },
    /// A whole design drawn with the flight tiles (hull edge, nav lights, no aura), fitted into the node: FLEET's
    /// flagship, escorts and saved designs.
    Ship(Box<Blueprint>),
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
    // The home screen's ring (`home.rs`, after `phone-home-ring-menu.png`).
    /// A square split into four cells (the drydock).
    Grid,
    /// A node forking into two (behaviors).
    Tree,
    /// A tilted orbit around a small ring (research).
    Orbit,
    /// A navigation arrowhead (voyages).
    Arrow,
    /// Two rings around a dot (the test range).
    Target,
    /// Three small ships in formation (the fleet).
    Formation,
    /// A padlock (locked parts, materials and frames).
    Lock,
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
    nodes: Query<(Entity, &UiIcon, &ComputedNode, &UiGlobalTransform, &InheritedVisibility, Option<&CalculatedClip>)>,
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
    for (node_e, icon, node, tf, vis, clip) in &nodes {
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
        // Shown only whole: an icon a scrolled parent cuts would draw over its neighbours.
        let inside = |c: &CalculatedClip| {
            let half = node.size / 2.0 - 1.0;
            [tf.translation - half, tf.translation + half].iter().all(|p| c.contains_point(*p))
        };
        let show = vis.get() && size.cmpgt(Vec2::splat(1.0)).all() && clip.is_none_or(inside);
        let want = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
        if !show {
            continue;
        }
        let side = size.min_element();
        let (px_side, glow) = (side.round() as u32, toggles.glow_sprites);
        // Compared in place: an icon (a whole ship) is cloned only when it is rebuilt.
        if m.built.as_ref().is_none_or(|(i, s, g)| i != icon || *s != px_side || *g != glow) {
            let (mesh, dims) = match icon {
                UiIcon::Part { part, material } => part_tile(&content.0, &style, &atlas, part, material.as_deref(), glow),
                UiIcon::Ship(bp) => ship_icon(&content.0, &style, &atlas, bp, glow),
                UiIcon::Glyph { glyph, color } => Some((glyph_mesh(*glyph, *color, side, &atlas), Vec2::ONE)),
            }
            .unwrap_or_else(|| (MeshBuf::placeholder(), Vec2::ONE));
            let _ = meshes.insert(&m.mesh, mesh);
            m.built = Some((icon.clone(), px_side, glow));
            m.dims = dims;
        }
        // Parts and ships: cells fitted into the node; glyphs are built at the node's size in pixels. Part icons are
        // stored at `art::ICON_DENSITY` in the atlas, so they need that many more pixels per cell from it.
        let px = match icon {
            UiIcon::Part { .. } | UiIcon::Ship(_) => {
                let k = (size.x / m.dims.x).min(size.y / m.dims.y);
                let density = if matches!(icon, UiIcon::Part { .. }) { art::ICON_DENSITY } else { 1.0 };
                px_per_cell = px_per_cell.max(k / density);
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

/// One part as a single tile, centred on the origin in cells; and its footprint. No hull edge, aura, barrel or
/// muzzle: an icon, not a ship. A machine shows its icon on the machine tile colour (on ships it is its drawing on
/// a hull-toned cell, GDR-0069).
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
    if !def.hull {
        (p.fill, p.art, p.overhang) = (style.machine_tile, None, 0.0);
    }
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

/// A whole design as the drydock draws it (`blueprint_visual`), centred on the origin in cells; and its extent.
fn ship_icon(content: &data::Content, style: &Style, atlas: &Atlas, bp: &Blueprint, glow: bool) -> Option<(Mesh, Vec2)> {
    let dv = design_visual(content, bp, style).ok()?;
    let grid = content.ship_grid(bp).ok()?;
    let mut v = blueprint_visual(&dv, &grid);
    if v.parts.is_empty() {
        return None;
    }
    v.pivot = (v.bounds_min + v.bounds_max) / 2.0;
    Some((build_body(&v, atlas, style, glow, true), v.bounds_max - v.bounds_min))
}

/// A glyph `side` px across, centred on the origin, in pixels (y up). Strokes are 1.6 px wide, thicker on glyphs
/// above about 23 px (the home ring's).
fn glyph_mesh(glyph: Glyph, color: Color, side: f32, atlas: &Atlas) -> Mesh {
    let white = atlas.region("fx/white");
    let uv = white.uv_center();
    let col = vcol(color, 1.0);
    let s = side;
    let w = (s * 0.035).max(0.8);
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
        Glyph::Grid => {
            let r = 0.3 * s;
            m.strip(&[Vec2::new(-r, r), Vec2::new(r, r), Vec2::new(r, -r), Vec2::new(-r, -r)], true, w, &white, col);
            m.strip(&[Vec2::new(0.0, r), Vec2::new(0.0, -r)], false, w, &white, col);
            m.strip(&[Vec2::new(-r, 0.0), Vec2::new(r, 0.0)], false, w, &white, col);
        }
        Glyph::Tree => {
            let p = |x: f32, y: f32| Vec2::new(x * s, y * s);
            m.polygon(&arc(p(0.0, 0.26), 0.07 * s, 0.0, std::f32::consts::TAU, 12)[..12], uv, col);
            m.strip(&[p(0.0, 0.2), p(0.0, 0.04)], false, w, &white, col);
            m.strip(&[p(-0.24, -0.16), p(-0.24, 0.04), p(0.24, 0.04), p(0.24, -0.16)], false, w, &white, col);
            for x in [-0.24, 0.24] {
                m.polygon(&[p(x - 0.08, -0.14), p(x + 0.08, -0.14), p(x + 0.08, -0.3), p(x - 0.08, -0.3)], uv, col);
            }
        }
        Glyph::Orbit => {
            let tilt = Vec2::from_angle(0.45);
            let ring: Vec<Vec2> = (0..36)
                .map(|k| {
                    let a = k as f32 * std::f32::consts::TAU / 36.0;
                    tilt.rotate(Vec2::new(a.cos() * 0.38, a.sin() * 0.16) * s)
                })
                .collect();
            m.strip(&ring, true, w, &white, col);
            m.strip(&arc(Vec2::ZERO, 0.11 * s, 0.0, std::f32::consts::TAU, 20)[..20], true, w, &white, col);
        }
        Glyph::Arrow => {
            let p = |x: f32, y: f32| Vec2::new(x * s, y * s);
            m.strip(&[p(0.0, 0.36), p(0.27, -0.34), p(0.0, -0.18), p(-0.27, -0.34)], true, w, &white, col);
        }
        Glyph::Target => {
            m.strip(&arc(Vec2::ZERO, 0.32 * s, 0.0, std::f32::consts::TAU, 36)[..36], true, w, &white, col);
            m.strip(&arc(Vec2::ZERO, 0.17 * s, 0.0, std::f32::consts::TAU, 24)[..24], true, w, &white, col);
            m.polygon(&arc(Vec2::ZERO, 0.06 * s, 0.0, std::f32::consts::TAU, 12)[..12], uv, col);
        }
        Glyph::Formation => {
            for (x, y) in [(0.0, 0.14), (-0.24, -0.2), (0.24, -0.2)] {
                let c = Vec2::new(x * s, y * s);
                let tri = [c + Vec2::new(0.0, 0.15 * s), c + Vec2::new(-0.1 * s, -0.12 * s), c + Vec2::new(0.1 * s, -0.12 * s)];
                m.strip(&tri, true, w, &white, col);
            }
        }
        Glyph::Lock => {
            let p = |x: f32, y: f32| Vec2::new(x * s, y * s);
            m.strip(&[p(-0.26, 0.04), p(0.26, 0.04), p(0.26, -0.34), p(-0.26, -0.34)], true, w, &white, col);
            let mut shackle = vec![p(-0.16, 0.04)];
            shackle.extend(arc(p(0.0, 0.16), 0.16 * s, std::f32::consts::PI, 0.0, 12));
            shackle.push(p(0.16, 0.04));
            m.strip(&shackle, false, w, &white, col);
            m.polygon(&arc(p(0.0, -0.13), 0.05 * s, 0.0, std::f32::consts::TAU, 10)[..10], uv, col);
        }
    }
    m.into_mesh()
}
