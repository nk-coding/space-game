//! What the pointer would do, drawn in world space over the design: the ghost of the edit under the cursor (the
//! picked part with the flight tile look, see-through, outlined teal when it can go there and coral when it
//! can't, its mirror twin too while mirroring), the parts it would replace (coral outlines), the slopes a BEVEL
//! paint stroke will cut when it ends (teal outlines), a shield part's bubble (placed or selected: the SHIELDS
//! overlay's look), the fill rectangle while dragging, the part a move would put elsewhere, the pointer's mark
//! ([`hover_mark`]: the cell, or in SELECT and ERASE the whole part under it) and corner brackets around the
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
use crate::ship_model::{HullTones, design_visual};
use crate::ship_view::{ShipVisual, build_body};
use crate::sim_bridge::GameContent;
use crate::style::{Style, ui_color};
use crate::tuning::Tuning;
use crate::ui_kit::Tone;

use super::PlayerDesign;
use super::report::DrydockReport;
use super::state::{DrydockUi, Tool};
use super::tools::{Brush, DrydockGesture, EditNote, Gesture, Selection, brush_placement, refusal_text};
use super::view::{DockShip, DrydockCursor};

/// The ghost tiles sit just above the design (its mesh is at z 10, the ghost is its child).
const Z_GHOST: f32 = 2.0;
/// Outlines and brackets: the FX layer above the ships and under the veil outside the grid viewport.
const Z_LINES: f32 = 300.0;
/// The ghost's shield bubbles sit with the overlays (relative to the design's mesh).
const Z_BUBBLES: f32 = 31.0;
/// Opacity of the ghost tiles.
const GHOST_ALPHA: f32 = 0.55;

/// The see-through tiles' mesh and the tiles it shows.
#[derive(Component)]
pub(super) struct GhostMesh {
    built: Option<(Vec<Placement>, bool)>,
}

/// The shield bubbles' mesh and the bubbles it shows.
#[derive(Component)]
pub(super) struct GhostBubbles {
    built: Option<Vec<Bubble>>,
}

/// A shield bubble: centre (frame cells, y down) and radius (m).
pub type Bubble = ([f32; 2], f32);

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
    /// What marks the pointer: the cell, or the whole part under it ([`hover_mark`]).
    pub hover: Option<HoverMark>,
    /// The bubbles of the shield part being placed (and its twin), after bonuses, or of the selected one.
    pub bubbles: Vec<Bubble>,
}

/// What marks the cell under the pointer.
#[derive(Clone, Debug, PartialEq)]
pub enum HoverMark {
    /// The cell (frame coordinates).
    Cell(Cell),
    /// The whole part covering it: its cells (filled) and its outline (one line around the footprint), in the
    /// tool's tone (teal to select, coral to erase).
    Part { cells: Vec<Cell>, outline: Vec<Vec2>, tone: Tone },
}

/// The pointer's mark for the tool and gesture at `cell`. SELECT and ERASE act on whole parts, so over a part they
/// mark its whole footprint (one outline, Niklas 13 Oct 2026: a big block's hovered 1×1 cell needs no mark of its
/// own); over an empty cell, and in the other tools, the cell. `part_at` gives the cells and the outline of the part
/// covering a cell.
pub fn hover_mark(tool: Tool, gesture: Gesture, cell: Option<Cell>, part_at: impl Fn(Cell) -> Option<(Vec<Cell>, Vec<Vec2>)>) -> Option<HoverMark> {
    let cell = cell?;
    let tone = match (tool, gesture) {
        (_, Gesture::Stroke { erase: true, .. }) | (Tool::Erase, Gesture::Idle) => Some(Tone::Coral),
        (Tool::Select, Gesture::Idle) => Some(Tone::Teal),
        _ => None,
    };
    Some(match tone.and_then(|t| part_at(cell).map(|p| (t, p))) {
        Some((tone, (cells, outline))) => HoverMark::Part { cells, outline, tone },
        None => HoverMark::Cell(cell),
    })
}

/// The ghost's mesh, a child of the design's mesh (so it is shown only in the drydock), on the shared atlas
/// material (it follows every atlas tier); the see-through look is in its vertex colours ([`GHOST_ALPHA`]).
pub(super) fn setup(
    mut commands: Commands,
    ship: Single<Entity, With<DockShip>>,
    fade: Res<super::view::DockFade>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let ghost = commands
        .spawn((
            GhostMesh { built: None },
            Mesh2d(meshes.add(crate::meshbuf::MeshBuf::placeholder())),
            // Fades with the screen (`view::DockFade`).
            MeshMaterial2d(fade.world.clone()),
            Transform::from_xyz(0.0, 0.0, Z_GHOST),
        ))
        .id();
    let bubbles = commands
        .spawn((
            GhostBubbles { built: None },
            Mesh2d(meshes.add(crate::meshbuf::MeshBuf::placeholder())),
            MeshMaterial2d(fade.world.clone()),
            Transform::from_xyz(0.0, 0.0, Z_BUBBLES),
        ))
        .id();
    commands.entity(*ship).add_children(&[ghost, bubbles]);
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
    sel: Res<Selection>,
    report: Res<DrydockReport>,
    tuning: Res<Tuning>,
    mut view: ResMut<GhostView>,
) {
    if !(design.is_changed() || ui.is_changed() || cursor.is_changed() || g.is_changed() || sel.is_changed() || report.is_changed()) {
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
            // The part under the cursor is the hover mark; its mirror twin (another part) is outlined too.
            if let Some(cell) = cursor.cell
                && dock.mirror
                && let Some(k) = dock.part_at(dock.mirror_cell(cell)).filter(|k| Some(*k) != dock.part_at(cell))
            {
                v.outlines.push((outline(content, &dock.parts()[k]), Tone::Coral));
            }
        }
        (Tool::Paint, Gesture::Idle | Gesture::Stroke { erase: false, .. }) => {
            if let (Some(cell), Some(part)) = (cursor.cell, picked) {
                let m = ui.material.as_str();
                match Brush::of(part) {
                    Brush::Paint => v.show(content, &design, dock.preview_paint(cell, m), None, Some(cell), None),
                    Brush::Slope => v.show(content, &design, dock.preview_slope(&part.id, cell, m, ui.slope_turn), None, Some(cell), Some(&part.id)),
                    Brush::Place => {
                        let p = brush_placement(part, cell, ui.rot, m);
                        let preview = dock.preview_place(&p);
                        if part.shield.is_some()
                            && let Ok(after) = &preview
                        {
                            v.bubbles = bubbles_after(report.content().unwrap_or(content), &tuning.config(), &design, after);
                        }
                        v.show(content, &design, preview, Some(p.clone()), None, Some(&part.id));
                    }
                }
            }
            // A paint stroke with BEVEL: the slopes its end will cut.
            for p in dock.preview_stroke_end() {
                v.outlines.push((outline(content, &p), Tone::Teal));
            }
        }
        _ => {}
    }
    // The selected shield part's bubble (the report's, after bonuses).
    if ui.tool == Tool::Select
        && let Some(k) = sel.part(&design)
        && report.revision == Some(design.revision())
        && let Some(r) = report.get()
    {
        v.bubbles.extend(r.shields.iter().filter(|s| s.part as usize == k).map(|s| (s.centre, s.radius)));
    }
    v.hover = hover_mark(ui.tool, g.gesture, cursor.cell, |c| {
        let p = &dock.parts()[dock.part_at(c)?];
        let cells = content.placed_shape(p)?.cells();
        Some((cells, outline(content, p)))
    });
    if *view != v {
        *view = v;
    }
}

/// The bubbles a shield part placed as `after` previews would make (and its twin), with the radius after bonuses: the
/// report of the design as the edit would leave it (one `sim::design_report`, made only when the ghost changes; tens
/// of microseconds for a frigate) with the content and config the drydock's report uses. If that design can't be
/// compiled, the part's own radius at its centre.
fn bubbles_after(content: &Content, config: &sim::SimConfig, design: &PlayerDesign, after: &Preview) -> Vec<Bubble> {
    let bp = design.blueprint();
    let mut parts: Vec<Placement> = bp.parts.iter().enumerate().filter(|(i, _)| after.removed.binary_search(i).is_err()).map(|(_, p)| p.clone()).collect();
    let first = parts.len();
    parts.push(after.placement.clone());
    parts.extend(after.mirrored.clone());
    let new = first..parts.len();
    let own: Vec<Bubble> = parts[new.clone()]
        .iter()
        .filter_map(|p| Some((content.placed_shape(p)?.center(), content.parts.get(&p.part)?.shield.as_ref()?.radius)))
        .collect();
    let what_if = Blueprint { parts, ..header(bp) };
    match sim::design_report(content, &what_if, config) {
        Ok(r) => r.shields.iter().filter(|s| new.contains(&(s.part as usize))).map(|s| (s.centre, s.radius)).collect(),
        Err(_) => own,
    }
}

impl GhostView {
    /// Shows a preview: its tiles outlined teal and the parts it replaces outlined coral, or the attempt (else the
    /// cell) outlined coral with the reason; a skipped mirror image as a coral outline. `new_part`: the part the
    /// edit adds, named when its mirror image would overlap it.
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
                for &k in &p.removed {
                    self.outlines.push((outline(content, &dock.parts()[k]), Tone::Coral));
                }
                self.tile(content, p.placement.clone(), Tone::Teal);
                if let Some(m) = p.mirrored {
                    self.tile(content, m, Tone::Teal);
                }
                if let Twin::Skipped(r) = &p.twin {
                    if *r != Refusal::NoTwin {
                        self.outlines.push((outline(content, &dock.mirror_of(&p.placement)), Tone::Coral));
                    }
                    // The new part's index after the edit (the replaced parts go first).
                    let new_part = new_part.map(|id| (dock.parts().len() - p.removed.len(), id));
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
                self.reason = Some(refusal_text(content, design, &r, new_part.map(|id| (dock.parts().len(), id))));
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
    // A machine's cell takes the hull tone of the spot it would go to in the design (GDR-0069), not of its
    // one-part blueprint.
    let grid = content.ship_grid(design.blueprint()).ok();
    let tones = grid.as_ref().map(|g| (g, HullTones::new(content, design.blueprint(), g)));
    for p in &view.tiles {
        let one = Blueprint { parts: vec![p.clone()], ..header(design.blueprint()) };
        if let Ok(dv) = design_visual(content, &one, &style) {
            v.glow = dv.glow;
            let mut parts = dv.parts;
            if let Some((grid, tones)) = &tones
                && content.parts.get(&p.part).is_some_and(|d| !d.hull)
                && let Some(shape) = content.placed_shape(p)
            {
                let tone = style.armor_tone(tones.under(grid, &shape.cells()));
                parts.iter_mut().for_each(|q| q.fill = tone);
            }
            v.parts.extend(parts);
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

/// Rebuilds the shield bubbles' mesh when they change.
pub(super) fn rebuild_bubbles(view: Res<GhostView>, atlas: Res<Atlas>, style: Res<Style>, mut meshes: ResMut<Assets<Mesh>>, mut ghost: Single<(&mut GhostBubbles, &Mesh2d)>) {
    let (g, mesh) = &mut *ghost;
    if g.built.as_ref() == Some(&view.bubbles) {
        return;
    }
    g.built = Some(view.bubbles.clone());
    let _ = meshes.insert(&mesh.0, super::overlays::bubbles_mesh(&atlas, &style, &view.bubbles));
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
    fade: Res<super::view::DockFade>,
    regions: Option<Res<FxRegions>>,
    mut queue: ResMut<FxQueue>,
) {
    let Some(r) = regions else { return };
    let px = 1.0 / rig.zoom;
    // Everything here fades with the screen.
    let fa = fade.alpha;
    let ui = &style.ui;
    let world = |p: Vec2| Vec2::new(p.x, -p.y);
    for (poly, tone) in &view.outlines {
        let c = tone.color(ui).with_alpha((if *tone == Tone::Label { 0.6 } else { 0.95 }) * fa);
        for k in 0..poly.len() {
            queue.line(&r.white, world(poly[k]), world(poly[(k + 1) % poly.len()]), 1.6 * px, c, Z_LINES);
        }
    }
    // The pointer's mark: the cell, or the whole part under it (filled, one outline).
    match &view.hover {
        Some(HoverMark::Cell((cx, cy))) => {
            let c = Vec2::new(*cx as f32 + 0.5, *cy as f32 + 0.5);
            queue.push(FxSprite { rect: r.white.uv, pos: world(c), angle: 0.0, size: Vec2::ONE, anchor: Vec2::ZERO, color: ui.teal.with_alpha(0.1 * fa), z: Z_LINES });
            let corners = [c + Vec2::new(-0.5, -0.5), c + Vec2::new(0.5, -0.5), c + Vec2::new(0.5, 0.5), c + Vec2::new(-0.5, 0.5)];
            for k in 0..4 {
                queue.line(&r.white, world(corners[k]), world(corners[(k + 1) % 4]), 1.5 * px, ui.teal.with_alpha(0.8 * fa), Z_LINES);
            }
        }
        Some(HoverMark::Part { cells, outline, tone }) => {
            let col = tone.color(ui);
            for (x, y) in cells {
                let c = Vec2::new(*x as f32 + 0.5, *y as f32 + 0.5);
                queue.push(FxSprite { rect: r.white.uv, pos: world(c), angle: 0.0, size: Vec2::ONE, anchor: Vec2::ZERO, color: col.with_alpha(0.1 * fa), z: Z_LINES });
            }
            for k in 0..outline.len() {
                queue.line(&r.white, world(outline[k]), world(outline[(k + 1) % outline.len()]), 1.6 * px, col.with_alpha(0.9 * fa), Z_LINES);
            }
        }
        None => {}
    }
    for &((x0, y0), (x1, y1)) in &view.rects {
        let (a, b) = (Vec2::new(x0 as f32, y0 as f32), Vec2::new(x1 as f32 + 1.0, y1 as f32 + 1.0));
        let centre = world((a + b) / 2.0);
        queue.push(FxSprite { rect: r.white.uv, pos: centre, angle: 0.0, size: b - a, anchor: Vec2::ZERO, color: ui.teal.with_alpha(0.12 * fa), z: Z_LINES });
        let corners = [a, Vec2::new(b.x, a.y), b, Vec2::new(a.x, b.y)];
        for k in 0..4 {
            queue.line(&r.white, world(corners[k]), world(corners[(k + 1) % 4]), 1.6 * px, ui.teal.with_alpha(0.9 * fa), Z_LINES);
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
        let c = ui.teal.with_alpha(0.95 * fa);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A shield emitter being placed shows its bubble and its twin's, centred on the parts, with the report's radius;
    /// one the report can't make (nothing else to fly with) still shows the part's own radius.
    #[test]
    fn a_shield_part_shows_the_bubble_it_would_make() {
        let content = data::base_content();
        let design = PlayerDesign::starter(&content, "halvex_runner");
        assert!(design.dock().mirror);
        let radius = content.parts.get("shield_emitter").unwrap().shield.as_ref().unwrap().radius;
        let p = Placement { part: "shield_emitter".into(), x: 2, y: 2, rot: 0, flip: false, material: None, power: None };
        let after = design.dock().preview_place(&p).unwrap();
        let bubbles = bubbles_after(&content, &sim::SimConfig::default(), &design, &after);
        assert_eq!(bubbles.len(), 2);
        let centre = content.placed_shape(&p).unwrap().center();
        assert_eq!(bubbles[0].0, centre);
        assert!(bubbles.iter().all(|b| b.1 >= radius * 0.999), "{bubbles:?}");
        let empty = PlayerDesign::open(&content, &data::Blueprint { parts: Vec::new(), ..design.blueprint().clone() }, (10, 10)).unwrap();
        let after = empty.dock().preview_place(&p).unwrap();
        let alone = bubbles_after(&content, &sim::SimConfig::default(), &empty, &after);
        assert_eq!(alone.len(), 1);
        assert!((alone[0].1 - radius).abs() < 1e-4);
    }

    /// A 2×2 block at (3, 4)..(4, 5): its cells and its outline.
    fn block(c: Cell) -> Option<(Vec<Cell>, Vec<Vec2>)> {
        ((3..=4).contains(&c.0) && (4..=5).contains(&c.1)).then(|| {
            (vec![(3, 4), (4, 4), (3, 5), (4, 5)], vec![Vec2::new(3.0, 4.0), Vec2::new(5.0, 4.0), Vec2::new(5.0, 6.0), Vec2::new(3.0, 6.0)])
        })
    }

    /// SELECT and ERASE mark the whole part under any of its cells (one outline), the empty cell itself; the other
    /// tools always mark the cell; off the frame nothing.
    #[test]
    fn select_and_erase_mark_the_whole_part() {
        let (cells, outline) = block((4, 5)).unwrap();
        for at in [(3, 4), (4, 5), (4, 4)] {
            assert_eq!(hover_mark(Tool::Select, Gesture::Idle, Some(at), block), Some(HoverMark::Part { cells: cells.clone(), outline: outline.clone(), tone: Tone::Teal }));
            assert_eq!(hover_mark(Tool::Erase, Gesture::Idle, Some(at), block), Some(HoverMark::Part { cells: cells.clone(), outline: outline.clone(), tone: Tone::Coral }));
            // A right-button erase stroke in any tool takes whole parts too.
            let stroke = Gesture::Stroke { erase: true, last: at };
            assert!(matches!(hover_mark(Tool::Paint, stroke, Some(at), block), Some(HoverMark::Part { tone: Tone::Coral, .. })));
            for tool in [Tool::Paint, Tool::Fill] {
                assert_eq!(hover_mark(tool, Gesture::Idle, Some(at), block), Some(HoverMark::Cell(at)));
            }
        }
        // An empty cell keeps its own mark; moving a part marks the cell (the move's ghost shows the part).
        for tool in [Tool::Select, Tool::Erase, Tool::Paint] {
            assert_eq!(hover_mark(tool, Gesture::Idle, Some((0, 0)), block), Some(HoverMark::Cell((0, 0))));
        }
        assert_eq!(hover_mark(Tool::Select, Gesture::Move { from: (3, 4), to: (4, 4) }, Some((4, 4)), block), Some(HoverMark::Cell((4, 4))));
        assert_eq!(hover_mark(Tool::Select, Gesture::Idle, None, block), None);
    }
}
