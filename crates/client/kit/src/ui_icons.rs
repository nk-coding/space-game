//! Vector icons inside bevy_ui nodes: put a [`UiIcon`] on a node and it is drawn at the node's laid-out
//! rectangle every frame.
//!
//! How (12 Oct 2026): icons are world-space meshes of the main camera, built by the same code as the ships
//! (`ship_view::build_body`, the shared atlas material; parts and whole designs come from [`IconShapes`], which
//! `part_icons` implements), placed over the node through the camera's view ([`ViewScale`]: screen → world) and
//! scaled by 1 / zoom, at a z above every world layer. So a part icon in the palette is drawn like a
//! flight tile: the same seams and glow, composited in sRGB like the ships (no colour shift,
//! which bevy_ui's own image nodes would give, because bevy_ui does not encode for sRGB compositing), and sharp
//! at any UI size because the atlas tier follows the icons too ([`UiAtlasNeed`]). A whole design (FLEET's ships)
//! is the drydock's view of it, fitted into the node. A machine's tile shows its icon on the machine tile colour,
//! where ships show its drawing on a hull-toned cell (GDR-0069). Glyphs Barlow lacks (arrows, the play triangle, the search
//! lens, the menu dots, the bonus diamond, the lock) are small meshes of the same kind.
//!
//! An icon follows its node's laid-out place (a sliding screen part moves it) and, while its screen's transition
//! runs, fades with its part ([`IconMaterials`]: the part's own atlas material).
//!
//! The world draws before bevy_ui, so UI nodes are drawn over icons: give the nodes behind an icon at most a
//! faint background. Icons follow their node's visibility, and a parent that clips them (a list that scrolls) cuts
//! them where it cuts the rest of the row: an icon is a world mesh that bevy_ui's clipping does not reach, so it
//! draws with a material of its own cut to the clip's rectangle ([`clip_bounds`], `atlas::set_clip`; Niklas,
//! 14 Oct 2026: "it is unacceptable that the icon only shows when fully into view").

use std::collections::BTreeMap;
use std::marker::PhantomData;

use bevy::prelude::*;
use bevy::ui::{CalculatedClip, ComputedNode, UiGlobalTransform, UiSystems};
use bevy::window::PrimaryWindow;
use data::Blueprint;

use crate::params::RenderToggles;
use crate::atlas::{Atlas, AtlasMaterial, UiAtlasNeed, set_clip};
use crate::meshbuf::MeshBuf;
use crate::style::{Style, vcol};
use crate::ui_kit::{IconMaterials, ScrollClipSet};
use crate::view::{ViewScale, ViewSet};

/// Above every world layer (ships ≤ 100, effects ≤ 400), below the camera's far plane; scrims just under the icons.
const Z_UI_ICON: f32 = 600.0;
const Z_UI_SCRIM: f32 = 599.0;

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
    /// The node filled with a colour around a clear hole: a scrim behind panels that float over the world, under
    /// their icons (a UI background would draw over them, the world draws first) and over the world (the drydock's
    /// frame around its grid). `hole` is the clear rectangle's inset from the node's left, top, right and bottom
    /// (UI px, like the node's layout); the colour fades out from `out` px outside the hole to `into` px inside it.
    /// Insets that meet close the hole: `[f32::INFINITY; 4]` fills the node (a screen over the world whose own
    /// icons must show, which an opaque UI background would cover: BLUEPRINTS).
    Scrim { color: Color, hole: [f32; 4], out: f32, into: f32 },
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
    /// Three small ships in formation (the fleet).
    Formation,
    /// A padlock (locked parts, materials and frames).
    Lock,
    /// Plus and minus (zoom and stepper rings): drawn as bars centred on the node, so they sit exactly in the
    /// middle of their ring (Barlow's `+` and `−` sit on the font's math axis inside a text line box, off the
    /// box's centre and with side bearings).
    Plus,
    Minus,
    /// Four corner brackets (a view back to its default: the voyage map's reset).
    Fit,
    /// A beacon sending: a dot between two pairs of arcs (a signal found, the debrief).
    Signal,
    /// The drydock's bottom bar (GDR-0082): a floppy disk (save), four tiles (blueprints), an arrow up out of a
    /// tray (export), three dots (the ⋯ menu), two triangles across an axis (mirror), a cell with a cut corner
    /// (bevel), and a small triangle pointing down (a dropdown).
    Save,
    Blueprints,
    Export,
    More,
    Mirror,
    Bevel,
    Caret,
    /// A cog with eight teeth (SETTINGS).
    Gear,
}

/// The mesh drawing one node's icon.
struct IconMesh {
    entity: Entity,
    mesh: Handle<Mesh>,
    /// What the mesh shows: the icon, the node's size in px (glyph strokes keep their pixel width, a scrim fills the
    /// node) and the glow setting; and for parts the footprint in cells.
    built: Option<(UiIcon, UVec2, bool)>,
    dims: Vec2,
    /// Its own material while a parent clips it (made the first time one does).
    clipped: Option<Handle<AtlasMaterial>>,
}

#[derive(Resource, Default)]
struct IconMeshes(BTreeMap<Entity, IconMesh>);

#[derive(Component)]
struct UiIconMesh;

/// Builds the icons drawn like the ships: a part as one tile ([`UiIcon::Part`]) and a whole design
/// ([`UiIcon::Ship`]), centred on the origin in cells, with their extent in cells. The app adds
/// [`UiIconsPlugin`] with the resource that implements it (`part_icons`: the content).
pub trait IconShapes: Resource {
    fn shape(&self, icon: &UiIcon, style: &Style, atlas: &Atlas, glow: bool) -> Option<(Mesh, Vec2)>;
}

/// Draws every [`UiIcon`], its parts and ships through `S`.
pub struct UiIconsPlugin<S>(PhantomData<fn() -> S>);

impl<S> Default for UiIconsPlugin<S> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<S: IconShapes> Plugin for UiIconsPlugin<S> {
    fn build(&self, app: &mut App) {
        app.init_resource::<IconMeshes>().add_systems(
            PostUpdate,
            draw_icons::<S>.after(UiSystems::Layout).after(ScrollClipSet).after(ViewSet).before(bevy::transform::TransformSystems::Propagate),
        );
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw_icons<S: IconShapes>(
    mut commands: Commands,
    mut state: ResMut<IconMeshes>,
    nodes: Query<(Entity, &UiIcon, &ComputedNode, &UiGlobalTransform, &InheritedVisibility, Option<&CalculatedClip>)>,
    mut placed: Query<(&mut Transform, &mut Visibility), With<UiIconMesh>>,
    mut removed: RemovedComponents<UiIcon>,
    mut meshes: ResMut<Assets<Mesh>>,
    view: Res<ViewScale>,
    window: Single<&Window, With<PrimaryWindow>>,
    atlas: Res<Atlas>,
    style: Res<Style>,
    shapes: Res<S>,
    toggles: Res<RenderToggles>,
    mut need: ResMut<UiAtlasNeed>,
    mut faded: ResMut<IconMaterials>,
    mut mats: Query<&mut MeshMaterial2d<AtlasMaterial>, With<UiIconMesh>>,
    mut materials: ResMut<Assets<AtlasMaterial>>,
) {
    for node in removed.read() {
        if let Some(m) = state.0.remove(&node) {
            commands.entity(m.entity).despawn();
        }
        faded.0.remove(&node);
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
            IconMesh { entity, mesh, built: None, dims: Vec2::ONE, clipped: None }
        });
        let Ok((mut t, mut v)) = placed.get_mut(m.entity) else { continue };
        let size = node.size / scale;
        let bounds = clip.map(clip_bounds);
        let own = Rect::from_center_size(tf.translation, node.size);
        let show = vis.get() && size.cmpgt(Vec2::splat(1.0)).all() && bounds.is_none_or(|b| b.is_some_and(|b| !b.intersect(own).is_empty()));
        // A screen's transition fades the icon with its part (`ui_kit::ScreenPart`'s material); a clipping parent
        // cuts it on its own material that follows that fade.
        if let Ok(mut mat) = mats.get_mut(m.entity) {
            let base = faded.0.get(&node_e).unwrap_or(&atlas.material);
            let want = match bounds.flatten().filter(|_| show) {
                Some(b) => {
                    let own = m.clipped.get_or_insert_with(|| atlas.group_material(&mut materials));
                    set_clip(&mut materials, base, own, Some(b));
                    own
                }
                None => base,
            };
            if mat.0 != *want {
                mat.0 = want.clone();
            }
        }
        let want = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
        if !show {
            continue;
        }
        let side = size.min_element();
        let (px_size, glow) = (size.round().as_uvec2(), toggles.glow_sprites);
        // Compared in place: an icon (a whole ship) is cloned only when it is rebuilt.
        if m.built.as_ref().is_none_or(|(i, s, g)| i != icon || *s != px_size || *g != glow) {
            let (mesh, dims) = match icon {
                UiIcon::Part { .. } | UiIcon::Ship(_) => shapes.shape(icon, &style, &atlas, glow),
                UiIcon::Glyph { glyph, color } => Some((glyph_mesh(*glyph, *color, side, &atlas), Vec2::ONE)),
                UiIcon::Scrim { color, hole, out, into } => {
                    // Its insets are UI px, the mesh window px: larger windows scale the UI (`UiScale`), and a hole
                    // given in window px stopped halfway across the drydock's left column on a big screen.
                    let ui = 1.0 / (scale * node.inverse_scale_factor);
                    Some((scrim_mesh(*color, hole.map(|v| v * ui), out * ui, into * ui, size, &atlas), Vec2::ONE))
                }
            }
            .unwrap_or_else(|| (MeshBuf::placeholder(), Vec2::ONE));
            let _ = meshes.insert(&m.mesh, mesh);
            m.built = Some((icon.clone(), px_size, glow));
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
            UiIcon::Glyph { .. } | UiIcon::Scrim { .. } => 1.0,
        };
        let z = if matches!(icon, UiIcon::Scrim { .. }) { Z_UI_SCRIM } else { Z_UI_ICON };
        // Written only when it moves: a written transform is propagated again.
        let (at, k) = (view.screen_to_world(tf.translation / scale).extend(z), Vec3::new(px / view.zoom, px / view.zoom, 1.0));
        if t.translation != at || t.scale != k {
            t.translation = at;
            t.scale = k;
        }
    }
    if need.0 != px_per_cell {
        need.0 = px_per_cell;
    }
}

/// Where a clip lets a node draw, in framebuffer px (y down): its rectangles (axis-aligned in the UI) intersected;
/// `None` when it hides everything.
fn clip_bounds(clip: &CalculatedClip) -> Option<Rect> {
    let mut out = Rect { min: Vec2::NEG_INFINITY, max: Vec2::INFINITY };
    for c in clip.rects()? {
        // An open side is infinite: kept finite so the transform does not turn it into NaN.
        let r = Rect { min: c.rect.min.max(Vec2::splat(-1e7)), max: c.rect.max.min(Vec2::splat(1e7)) };
        let to_screen = c.world_to_clip_local.inverse();
        let a = to_screen.transform_point2(r.min);
        let b = to_screen.transform_point2(r.max);
        out = out.intersect(Rect::from_corners(a, b));
    }
    (!out.is_empty()).then_some(out)
}

/// The bars of a plus (`plus`) or a minus `side` px across, centred on the origin (the node's centre): the
/// horizontal bar, and for a plus the vertical bar's halves above and below it (no overlap, so no doubled alpha).
fn sign_bars(plus: bool, side: f32) -> Vec<Rect> {
    let (arm, t) = (0.32 * side, (side * 0.1).max(1.4) / 2.0);
    let mut bars = vec![Rect::new(-arm, -t, arm, t)];
    if plus {
        bars.push(Rect::new(-t, -arm, t, -t));
        bars.push(Rect::new(-t, t, t, arm));
    }
    bars
}

/// A scrim `size` px large, centred on the origin, in pixels (y up): solid up to `out` px outside the hole (insets
/// left, top, right, bottom), fading out to `into` px inside it, clear in the middle. One mesh, so the corners are
/// not darker where two bands would overlap.
fn scrim_mesh(color: Color, hole: [f32; 4], out: f32, into: f32, size: Vec2, atlas: &Atlas) -> Mesh {
    let uv = atlas.region("fx/white").uv_center();
    let (solid, clear) = (vcol(color, 1.0), vcol(color, 0.0));
    let h = size / 2.0;
    let [l, t, r, b] = hole;
    // The hole's edges (y up), kept inside the node and never inverted (insets that meet close it), the solid
    // region's inner edge outside them and the clear region's edge inside them.
    let (x0, y0) = ((-h.x + l).min(h.x), (-h.y + b).min(h.y));
    let (x1, y1) = ((h.x - r).max(x0), (h.y - t).max(y0));
    let s = Rect::new((x0 - out).max(-h.x), (y0 - out).max(-h.y), (x1 + out).min(h.x), (y1 + out).min(h.y));
    let c = Rect::new(x0 + into, y0 + into, (x1 - into).max(x0 + into), (y1 - into).max(y0 + into));
    let p = Vec2::new;
    let mut m = MeshBuf::default();
    // Solid: the bands above and below the fade, then left and right of it.
    m.gradient_quad([p(-h.x, s.max.y), p(h.x, s.max.y), p(h.x, h.y), p(-h.x, h.y)], uv, [solid; 4]);
    m.gradient_quad([p(-h.x, -h.y), p(h.x, -h.y), p(h.x, s.min.y), p(-h.x, s.min.y)], uv, [solid; 4]);
    m.gradient_quad([p(-h.x, s.min.y), p(s.min.x, s.min.y), p(s.min.x, s.max.y), p(-h.x, s.max.y)], uv, [solid; 4]);
    m.gradient_quad([p(s.max.x, s.min.y), p(h.x, s.min.y), p(h.x, s.max.y), p(s.max.x, s.max.y)], uv, [solid; 4]);
    // The fade: four trapezoids from the solid edge to the clear one, mitred at the corners.
    let outer = [p(s.min.x, s.min.y), p(s.max.x, s.min.y), p(s.max.x, s.max.y), p(s.min.x, s.max.y)];
    let inner = [p(c.min.x, c.min.y), p(c.max.x, c.min.y), p(c.max.x, c.max.y), p(c.min.x, c.max.y)];
    for k in 0..4 {
        let n = (k + 1) % 4;
        m.gradient_quad([outer[k], outer[n], inner[n], inner[k]], uv, [solid, solid, clear, clear]);
    }
    m.into_mesh()
}

/// A glyph `side` px across, centred on the origin, in pixels (y up). Strokes are 1.6 px wide, thicker on glyphs
/// above about 23 px.
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
        Glyph::Formation => {
            for (x, y) in [(0.0, 0.14), (-0.24, -0.2), (0.24, -0.2)] {
                let c = Vec2::new(x * s, y * s);
                let tri = [c + Vec2::new(0.0, 0.15 * s), c + Vec2::new(-0.1 * s, -0.12 * s), c + Vec2::new(0.1 * s, -0.12 * s)];
                m.strip(&tri, true, w, &white, col);
            }
        }
        Glyph::Plus | Glyph::Minus => {
            for bar in sign_bars(glyph == Glyph::Plus, s) {
                m.polygon(&[bar.min, Vec2::new(bar.max.x, bar.min.y), bar.max, Vec2::new(bar.min.x, bar.max.y)], uv, col);
            }
        }
        Glyph::Fit => {
            for (x, y) in [(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
                let p = |a: f32, b: f32| Vec2::new(x * a * s, y * b * s);
                m.strip(&[p(0.34, 0.14), p(0.34, 0.34), p(0.14, 0.34)], false, w, &white, col);
            }
        }
        Glyph::Signal => {
            m.polygon(&arc(Vec2::ZERO, 0.09 * s, 0.0, std::f32::consts::TAU, 12)[..12], uv, col);
            for side in [0.0, std::f32::consts::PI] {
                for r in [0.22, 0.4] {
                    m.strip(&arc(Vec2::ZERO, r * s, side - 0.75, side + 0.75, 10), false, w, &white, col);
                }
            }
        }
        Glyph::Save => {
            let p = |x: f32, y: f32| Vec2::new(x * s, y * s);
            m.strip(&[p(-0.32, 0.32), p(0.18, 0.32), p(0.32, 0.18), p(0.32, -0.32), p(-0.32, -0.32)], true, w, &white, col);
            m.strip(&[p(-0.17, 0.32), p(-0.17, 0.12), p(0.11, 0.12), p(0.11, 0.32)], false, w, &white, col);
            m.strip(&[p(-0.19, -0.32), p(-0.19, -0.06), p(0.19, -0.06), p(0.19, -0.32)], false, w, &white, col);
        }
        Glyph::Blueprints => {
            for (x, y) in [(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
                let p = |a: f32, b: f32| Vec2::new(x * a * s, y * b * s);
                m.strip(&[p(0.05, 0.05), p(0.33, 0.05), p(0.33, 0.33), p(0.05, 0.33)], true, w, &white, col);
            }
        }
        Glyph::Export => {
            let p = |x: f32, y: f32| Vec2::new(x * s, y * s);
            m.strip(&[p(-0.32, 0.02), p(-0.32, -0.32), p(0.32, -0.32), p(0.32, 0.02)], false, w, &white, col);
            m.strip(&[p(0.0, -0.14), p(0.0, 0.36)], false, w, &white, col);
            m.strip(&[p(-0.16, 0.2), p(0.0, 0.36), p(0.16, 0.2)], false, w, &white, col);
        }
        Glyph::Gear => {
            // Eight teeth: four steps around each, two out, two in (the corners of a tooth on the step lines).
            let n = 32;
            let teeth: Vec<Vec2> = (0..n)
                .map(|k| {
                    let a = k as f32 / n as f32 * std::f32::consts::TAU + std::f32::consts::TAU / 64.0;
                    let r = if k % 4 < 2 { 0.36 } else { 0.27 };
                    Vec2::from_angle(a) * r * s
                })
                .collect();
            m.strip(&teeth, true, w, &white, col);
            m.strip(&arc(Vec2::ZERO, 0.12 * s, 0.0, std::f32::consts::TAU, 16)[..16], true, w, &white, col);
        }
        Glyph::More => {
            for x in [-0.3, 0.0, 0.3] {
                m.polygon(&arc(Vec2::new(x * s, 0.0), (0.055 * s).max(1.1), 0.0, std::f32::consts::TAU, 10)[..10], uv, col);
            }
        }
        Glyph::Mirror => {
            let p = |x: f32, y: f32| Vec2::new(x * s, y * s);
            for (a, b) in [(0.36, 0.26), (0.18, 0.08), (-0.0, -0.1), (-0.18, -0.28)] {
                m.strip(&[p(0.0, a), p(0.0, b)], false, w, &white, col);
            }
            m.strip(&[p(-0.1, 0.24), p(-0.38, -0.24), p(-0.1, -0.24)], true, w, &white, col);
            m.polygon(&[p(0.1, 0.24), p(0.1, -0.24), p(0.38, -0.24)], uv, col);
        }
        Glyph::Bevel => {
            let p = |x: f32, y: f32| Vec2::new(x * s, y * s);
            m.strip(&[p(-0.32, -0.32), p(-0.32, 0.02), p(0.02, 0.32), p(0.32, 0.32), p(0.32, -0.32)], true, w, &white, col);
        }
        Glyph::Caret => {
            m.polygon(&[Vec2::new(-0.3 * s, 0.14 * s), Vec2::new(0.0, -0.18 * s), Vec2::new(0.3 * s, 0.14 * s)], uv, col);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Plus and minus are centred on the node: their bars' bounds are symmetric around the origin, at any size.
    #[test]
    fn plus_and_minus_are_centred() {
        for side in [8.0, 13.0, 14.0, 22.0] {
            for plus in [true, false] {
                let bars = sign_bars(plus, side);
                let all = bars.iter().fold(bars[0], |a, b| a.union(*b));
                assert!(all.center().length() < 1e-5, "{plus} {side}: {all:?}");
                assert!((all.width() - 0.64 * side).abs() < 1e-4);
                assert_eq!(all.height() > all.width() * 0.9, plus);
                // The plus's bars don't overlap (the alpha would double where they cross).
                for (i, a) in bars.iter().enumerate() {
                    for b in &bars[i + 1..] {
                        assert!(a.intersect(*b).is_empty() || a.intersect(*b).width() * a.intersect(*b).height() < 1e-6);
                    }
                }
            }
        }
    }
}
