//! What the pointer would do, drawn in world space over the design: the ghost of the edit under the cursor (the
//! picked part with the flight tile look, see-through, outlined teal when it can go there and coral when it
//! can't, its mirror twin too while mirroring), the fill rectangle while dragging, the part a move would put
//! elsewhere, the part erase would take, the hovered part in the select tool and corner brackets around the
//! selected part. Why an edit can't happen goes after the grid's info line (coral), next to the last refusal.
//!
//! [`update`] works the view out from the previews of `data::edit` (`preview_paint`, `preview_slope`,
//! `preview_place`, `preview_move`: what the edit would do, planned by the same code as the edit) when its inputs
//! change (the design, the tool and picked part, the cursor cell, the gesture); the see-through tiles are one mesh (the flight tiles, `ship_view::build_body`, with a
//! translucent material), a child of the design's mesh so it shows only in the drydock, rebuilt only when the
//! tiles change; outlines, brackets and the rectangle are immediate-mode FX lines, sized in screen pixels.

use bevy::prelude::*;
use bevy::mesh::VertexAttributeValues;
use data::edit::{Preview, Refusal, Twin};
use data::{Blueprint, Cell, Content, Placement};

use crate::RenderToggles;
use crate::atlas::Atlas;
use crate::camera::CameraRig;
use crate::flight_hud::HudFonts;
use crate::fx::{FxQueue, FxRegions, FxSprite};
use crate::ship_model::design_visual;
use crate::ship_view::{ShipVisual, build_body};
use crate::sim_bridge::GameContent;
use crate::style::{Style, ui_color};
use crate::ui_kit::Tone;

use super::PlayerDesign;
use super::state::{DrydockUi, Tool};
use super::tools::{Brush, DrydockGesture, EditNote, Gesture, Selection, brush_placement, refusal_text};
use super::view::{DockShip, DrydockCursor};

/// The ghost tiles sit just above the design (its mesh is at z 10, the ghost is its child).
const Z_GHOST: f32 = 2.0;
/// Outlines and brackets: the FX layer above the ships and under the veil outside the grid viewport.
const Z_LINES: f32 = 300.0;
/// Opacity of the ghost tiles.
const GHOST_ALPHA: f32 = 0.55;

/// The see-through tiles' mesh and the tiles it shows.
#[derive(Component)]
pub(super) struct GhostMesh {
    built: Option<(Vec<Placement>, bool)>,
}

/// The coral text after the grid's info line.
#[derive(Component)]
pub(super) struct HintText;

/// What the pointer would do, in frame coordinates (cells, y down).
#[derive(Resource, Default, Debug, PartialEq)]
pub struct GhostView {
    /// See-through tiles: the edit and its mirror image.
    pub tiles: Vec<Placement>,
    /// Outlines (polygon, colour): teal = the edit can go there, coral = it can't (or erase takes it), label grey
    /// = hovered.
    pub outlines: Vec<(Vec<Vec2>, Tone)>,
    /// Fill rectangles (corner cells, inclusive): the dragged one and its mirror image.
    pub rects: Vec<(Cell, Cell)>,
    /// Why the edit under the cursor can't happen (or loses its mirror image).
    pub reason: Option<String>,
}

/// The ghost's mesh, a child of the design's mesh (so it is shown only in the drydock), on the shared atlas
/// material (it follows every atlas tier); the see-through look is in its vertex colours ([`GHOST_ALPHA`]).
pub(super) fn setup(
    mut commands: Commands,
    ship: Single<Entity, With<DockShip>>,
    atlas: Res<Atlas>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let ghost = commands
        .spawn((
            GhostMesh { built: None },
            Mesh2d(meshes.add(crate::meshbuf::MeshBuf::placeholder())),
            MeshMaterial2d(atlas.material.clone()),
            Transform::from_xyz(0.0, 0.0, Z_GHOST),
        ))
        .id();
    commands.entity(*ship).add_child(ghost);
}

/// The coral hint span for the info line (a child of its text).
pub(super) fn hint_span(fonts: &HudFonts, style: &Style) -> impl Bundle {
    let size = 9.0;
    (
        HintText,
        TextSpan::new(""),
        TextFont { font: fonts.regular.clone().into(), font_size: FontSize::Px(size), ..default() },
        TextColor(ui_color(style.ui.coral, 1.0)),
        bevy::text::LetterSpacing::Px(size * 0.3),
    )
}

/// Works out [`GhostView`] from the tool, the cursor and the gesture.
#[allow(clippy::too_many_arguments)]
pub(super) fn update(
    design: Res<PlayerDesign>,
    content: Res<GameContent>,
    ui: Res<DrydockUi>,
    cursor: Res<DrydockCursor>,
    g: Res<DrydockGesture>,
    mut view: ResMut<GhostView>,
) {
    if !(design.is_changed() || ui.is_changed() || cursor.is_changed() || g.is_changed()) {
        return;
    }
    let content = &content.0;
    let dock = design.dock();
    let mut v = GhostView::default();
    let picked = ui.picked.as_deref().and_then(|p| content.parts.get(p));
    match (ui.tool, g.gesture) {
        (_, Gesture::Rect { from, to }) => {
            let (w, h) = (design.frame().0 as i32, design.frame().1 as i32);
            let (x0, x1) = (from.0.min(to.0).max(0), from.0.max(to.0).min(w - 1));
            let (y0, y1) = (from.1.min(to.1).max(0), from.1.max(to.1).min(h - 1));
            v.rects.push(((x0, y0), (x1, y1)));
            if dock.mirror && (w - 1 - x1, w - 1 - x0) != (x0, x1) {
                v.rects.push(((w - 1 - x1, y0), (w - 1 - x0, y1)));
            }
        }
        (_, Gesture::Move { from, to }) if from != to => {
            if let Some(i) = dock.part_at(from) {
                let (dx, dy) = (to.0 - from.0, to.1 - from.1);
                let moved = Placement { x: dock.parts()[i].x + dx, y: dock.parts()[i].y + dy, ..dock.parts()[i].clone() };
                v.show(content, &design, dock.preview_move(i, dx, dy), Some(moved), None, None);
            }
        }
        (Tool::Erase, Gesture::Idle) | (_, Gesture::Stroke { erase: true, .. }) => {
            if let Some(cell) = cursor.cell {
                let mut cells = vec![cell];
                if dock.mirror {
                    cells.push(dock.mirror_cell(cell));
                }
                for k in cells.into_iter().filter_map(|c| dock.part_at(c)) {
                    v.outlines.push((outline(content, &dock.parts()[k]), Tone::Coral));
                }
            }
        }
        (Tool::Paint, Gesture::Idle | Gesture::Stroke { erase: false, .. }) => {
            if let (Some(cell), Some(part)) = (cursor.cell, picked) {
                let m = ui.material.as_str();
                match Brush::of(part) {
                    Brush::Paint => v.show(content, &design, dock.preview_paint(cell, m), None, Some(cell), None),
                    Brush::Slope => v.show(content, &design, dock.preview_slope(&part.id, cell, m), None, Some(cell), Some(&part.id)),
                    Brush::Place => {
                        let p = brush_placement(part, cell, ui.rot, m);
                        v.show(content, &design, dock.preview_place(&p), Some(p.clone()), None, Some(&part.id));
                    }
                }
            }
        }
        (Tool::Select, Gesture::Idle) => {
            if let Some(k) = cursor.cell.and_then(|c| dock.part_at(c)) {
                v.outlines.push((outline(content, &dock.parts()[k]), Tone::Label));
            }
        }
        _ => {}
    }
    if *view != v {
        *view = v;
    }
}

impl GhostView {
    /// Shows a preview: its tiles outlined teal, or the attempt (else the cell) outlined coral with the reason;
    /// a skipped mirror image as a coral outline. `new_part`: the part the edit adds, named when its mirror image
    /// would overlap it.
    #[allow(clippy::too_many_arguments)]
    fn show(
        &mut self,
        content: &Content,
        design: &PlayerDesign,
        preview: Result<Preview, Refusal>,
        attempt: Option<Placement>,
        cell: Option<Cell>,
        new_part: Option<&str>,
    ) {
        let dock = design.dock();
        match preview {
            Ok(p) => {
                self.tile(content, p.placement.clone(), Tone::Teal);
                if let Some(m) = p.mirrored {
                    self.tile(content, m, Tone::Teal);
                }
                if let Twin::Skipped(r) = &p.twin {
                    if *r != Refusal::NoTwin {
                        self.outlines.push((outline(content, &dock.mirror_of(&p.placement)), Tone::Coral));
                    }
                    self.reason = Some(format!("MIRROR SKIPPED  ·  {}", refusal_text(content, design, r, new_part)));
                }
            }
            Err(r) => {
                if let Some(a) = attempt {
                    self.tile(content, a, Tone::Coral);
                } else if let Some((x, y)) = cell {
                    let (x, y) = (x as f32, y as f32);
                    let square = vec![Vec2::new(x, y), Vec2::new(x + 1.0, y), Vec2::new(x + 1.0, y + 1.0), Vec2::new(x, y + 1.0)];
                    self.outlines.push((square, Tone::Coral));
                }
                self.reason = Some(refusal_text(content, design, &r, new_part));
            }
        }
    }

    fn tile(&mut self, content: &Content, p: Placement, tone: Tone) {
        self.outlines.push((outline(content, &p), tone));
        self.tiles.push(p);
    }
}

/// A placement's outline in frame coordinates.
fn outline(content: &Content, p: &Placement) -> Vec<Vec2> {
    content.placed_shape(p).map_or_else(Vec::new, |s| s.polygon().iter().map(|g| Vec2::new(g.x as f32, g.y as f32)).collect())
}

/// Rebuilds the see-through tiles when they change.
#[allow(clippy::too_many_arguments)]
pub(super) fn rebuild(
    view: Res<GhostView>,
    design: Res<PlayerDesign>,
    content: Res<GameContent>,
    atlas: Res<Atlas>,
    style: Res<Style>,
    toggles: Res<RenderToggles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ghost: Single<(&mut GhostMesh, &Mesh2d)>,
) {
    let (g, mesh) = &mut *ghost;
    if g.built.as_ref().is_some_and(|(tiles, glow)| *tiles == view.tiles && *glow == toggles.glow_sprites) {
        return;
    }
    g.built = Some((view.tiles.clone(), toggles.glow_sprites));
    let content = &content.0;
    let mut v = ShipVisual {
        parts: Vec::new(),
        hull_loops: Vec::new(),
        pivot: Vec2::ZERO,
        bounds_min: Vec2::ZERO,
        bounds_max: Vec2::ZERO,
        glow: style.glow_player,
        enemy: false,
        nav_lights: Vec::new(),
        aura: false,
    };
    for p in &view.tiles {
        let one = Blueprint { parts: vec![p.clone()], ..header(design.blueprint()) };
        if let Ok(dv) = design_visual(content, &one, &style) {
            v.glow = dv.glow;
            v.parts.extend(dv.parts);
        }
    }
    let mut body = build_body(&v, &atlas, &style, toggles.glow_sprites, true);
    if let Some(VertexAttributeValues::Float32x4(colors)) = body.attribute_mut(Mesh::ATTRIBUTE_COLOR) {
        for c in colors {
            c[3] *= GHOST_ALPHA;
        }
    }
    let _ = meshes.insert(&mesh.0, body);
}

/// A blueprint's header without its parts (the faction sets the glow).
fn header(bp: &Blueprint) -> Blueprint {
    Blueprint {
        id: bp.id.clone(),
        name: String::new(),
        summary: String::new(),
        description: None,
        lore: None,
        tags: Vec::new(),
        faction: bp.faction.clone(),
        size: bp.size,
        parts: Vec::new(),
        behavior: None,
        source: Default::default(),
    }
}

/// The outlines, the fill rectangles and the selection brackets (screen-pixel widths, so every frame).
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    view: Res<GhostView>,
    design: Res<PlayerDesign>,
    content: Res<GameContent>,
    sel: Res<Selection>,
    rig: Res<CameraRig>,
    style: Res<Style>,
    regions: Option<Res<FxRegions>>,
    mut queue: ResMut<FxQueue>,
) {
    let Some(r) = regions else { return };
    let px = 1.0 / rig.zoom;
    let ui = &style.ui;
    let world = |p: Vec2| Vec2::new(p.x, -p.y);
    for (poly, tone) in &view.outlines {
        let c = tone.color(ui).with_alpha(if *tone == Tone::Label { 0.6 } else { 0.95 });
        for k in 0..poly.len() {
            queue.line(&r.white, world(poly[k]), world(poly[(k + 1) % poly.len()]), 1.6 * px, c, Z_LINES);
        }
    }
    for &((x0, y0), (x1, y1)) in &view.rects {
        let (a, b) = (Vec2::new(x0 as f32, y0 as f32), Vec2::new(x1 as f32 + 1.0, y1 as f32 + 1.0));
        let centre = world((a + b) / 2.0);
        queue.push(FxSprite { rect: r.white.uv, pos: centre, angle: 0.0, size: b - a, anchor: Vec2::ZERO, color: ui.teal.with_alpha(0.12), z: Z_LINES });
        let corners = [a, Vec2::new(b.x, a.y), b, Vec2::new(a.x, b.y)];
        for k in 0..4 {
            queue.line(&r.white, world(corners[k]), world(corners[(k + 1) % 4]), 1.6 * px, ui.teal.with_alpha(0.9), Z_LINES);
        }
    }
    // Corner brackets just outside the selected part's footprint.
    if let Some(k) = sel.part(&design)
        && let Some(s) = content.0.placed_shape(&design.dock().parts()[k])
    {
        let (w, h) = s.dims();
        let off = 4.0 * px;
        let (a, b) = (Vec2::new(s.x as f32, s.y as f32) - off, Vec2::new((s.x + w as i32) as f32, (s.y + h as i32) as f32) + off);
        let arm = (10.0 * px).min((b - a).min_element() * 0.4);
        let c = ui.teal.with_alpha(0.95);
        for (corner, sx, sy) in [(a, 1.0, 1.0), (Vec2::new(b.x, a.y), -1.0, 1.0), (b, -1.0, -1.0), (Vec2::new(a.x, b.y), 1.0, -1.0)] {
            queue.line(&r.white, world(corner), world(corner + Vec2::new(sx * arm, 0.0)), 2.0 * px, c, Z_LINES);
            queue.line(&r.white, world(corner), world(corner + Vec2::new(0.0, sy * arm)), 2.0 * px, c, Z_LINES);
        }
    }
}

/// The coral text after the info line: why the edit under the cursor can't happen, else the last refusal.
pub(super) fn sync_hint(view: Res<GhostView>, note: Res<EditNote>, time: Res<Time<Real>>, mut hint: Single<&mut TextSpan, With<HintText>>) {
    const SEP: &str = "  ·  ";
    let text = view.reason.as_deref().or(note.current(time.elapsed_secs()));
    if hint.0.strip_prefix(SEP) != text {
        hint.0 = text.map_or(String::new(), |t| format!("{SEP}{t}"));
    }
}
