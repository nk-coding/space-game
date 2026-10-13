//! The voyage map ([`AppMode::Map`], track 7I; GDR-0065's star map, after `docs/plans/mockups/map-b.png`): where
//! voyages are chosen. Home's gate (V, or a click) flies the camera to the gate and opens it; Esc or the ring button
//! top left goes back to the shipyard (the camera flies out from the gate); SET COURSE launches the selected voyage
//! (`voyage::StartVoyage`).
//!
//! ```text
//! (o) RIM · 4 VOYAGES · 1 CLEARED        RIM    :    OUTER  | DESTINATION · VOYAGE 2 OF 4
//!     Voyages                                   :            | Halvex Drift
//!                          (o) HALVEX DRIFT     :            | [RIM] [ROGUE AUTOMATONS] [WRECKS]
//!               ≈ 10–15 MIN  .  OPEN ●●●●◆       :            | A raider-haunted drift of wrecks ...
//!   SHIPYARD APPROACH  ●'           <> THE WARDEN'S GATE      | SITES · 5   ● Raider pack ×2     3 SHIPS ...
//!   CLEARED · BEST 7:42  .     o CORRAN REACH    :           | ≈ 10–15 MIN · RECALL WHEN CLEAR
//!              <> SHIPYARD      LOCKED           :           | FLEET   CORES 2 / 2   [ship] Rim skiff ...
//!  <> HOME  o OPEN  ● CLEARED  o LOCKED          :           | VOYAGE-READY ✓ FLIGHT-READY ✓ ONLY UNLOCKED PARTS
//!  ● ENEMIES ● WRECKS ◆ GOAL SITE <> GUARDIAN    :           | [ ▶ SET COURSE ]
//! ```
//!
//! * **Layout** (`layout`): a stage of 1280 × 720 logical px scaled to fit the window at any aspect (centred,
//!   nothing cut off); the map left of the panel, content's `map` positions in its rect, the shipyard bottom left,
//!   the rim band's edges as arcs placed from the positions. **States** come from the career: open, cleared (with the
//!   best time), locked, and a guardian at the chain's end.
//! * **Drawing** (`draw`): the star map is one atlas mesh under the UI text (the world draws before bevy_ui), placed
//!   over the stage node through the camera rig like `ui_icons`, rebuilt only when the states, the selection or
//!   the hover change. The panel's ships are `ui_icons::UiIcon::Ship`.
//! * **Input**: a click or a tap on a voyage (its marker or its label) selects it; ←/→ (and ↑/↓) step through the
//!   voyages; Enter or SET COURSE launches; Esc or the ring button goes home. SET COURSE is dimmed while the
//!   selected voyage can't launch (`layout::course_blocker`: locked, another one underway, the fleet not ready); a
//!   click then says why, as does a launch `voyage.rs` refuses.
//! * **Entering** selects the first open voyage whose goal is not cleared (`voyage::default_voyage`): after a voyage
//!   that opened the next one, that one. The debrief's VOYAGE MAP sends [`OpenMap`] (home flies to the gate).
//!
//! UI words are chrome in code, like home's.

pub mod draw;
pub mod layout;

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale, UiSystems, UiTransform};
use bevy::window::PrimaryWindow;

use crate::RenderToggles;
use crate::atlas::Atlas;
use crate::camera::{CameraRig, CameraSet};
use crate::career::{EscortDesigns, Granted, PlayerCareer, fleet_cores};
use crate::drydock::PlayerDesign;
use crate::fleet::MAX_ESCORTS;
use crate::flight_hud::HudFonts;
use crate::meshbuf::MeshBuf;
use crate::mode::AppMode;
use crate::sim_bridge::GameContent;
use crate::style::{Style, ui_color};
use crate::ui_icons::{Glyph, UiIcon};
use crate::ui_kit::{Hot, Pill, Tone, UiClick, UiInputSet, UiPointer, UiStyleSet, label, number, prose, ring_node, set_color, set_text, spawn_pill, tracked};
use crate::voyage::{LaunchRefused, StartVoyage, default_voyage, fleet_blocker};

use draw::{LEGEND_COLUMNS, LEGEND_GAP, LEGEND_ROWS, Look};
use layout::{Geometry, PANEL_X, STAGE, VoyageState, chips, course_blocker, is_guardian, minutes, site_dots, site_rows, state_words};

/// Above the world's layers (effects ≤ 400), under the UI icons (600) that draw the panel's ships.
const Z_MAP: f32 = 550.0;
/// The stage's scale against the window: whatever fits (a tall phone-like window shrinks it), at most 2.
const SCALE: (f32, f32) = (0.3, 2.0);
/// Rows the panel has: sites, chips, ships (the flagship and the escorts), dots under a voyage's name.
const MAX_SITES: usize = 6;
const MAX_CHIPS: usize = 5;
const SHIPS: usize = 1 + MAX_ESCORTS;
const MAX_DOTS: usize = 12;
/// The panel's width and padding (logical px).
const PANEL_W: f32 = STAGE.x - PANEL_X;
const PAD: f32 = 28.0;

/// The voyage map's state (a resource).
#[derive(Resource, Debug, Default)]
pub struct VoyageMap {
    /// The voyage the panel shows (a content id).
    pub selected: Option<String>,
    /// Why the last SET COURSE did not launch.
    say: Option<String>,
}

/// The debrief's VOYAGE MAP: home flies to the gate and opens the map, as V does.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenMap;

/// Where everything sits (built once from the content).
#[derive(Resource)]
struct MapGeometry(Geometry);

#[derive(Component)]
struct MapRoot;

/// The 1280 × 720 stage everything is laid out on.
#[derive(Component)]
struct Stage;

/// The star map's mesh and what it shows.
#[derive(Component)]
struct MapMesh {
    mesh: Handle<Mesh>,
    built: Option<Look>,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum MapButton {
    Back,
    Voyage(usize),
    SetCourse,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum MapText {
    Header,
    Name(usize),
    State(usize),
    Route(usize),
    Head,
    Title,
    Summary,
    SitesHead,
    Minutes,
    Cores,
    ReadyHead,
    Status,
    Chip(usize),
    SiteName(usize),
    SiteTag(usize),
    SiteGives(usize),
    ShipName(usize),
    ShipSub(usize),
    NoEscort,
    Check(usize),
    Mark(usize),
}

/// Nodes shown, hidden or recoloured with the data.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum MapNode {
    Route(usize),
    Dots(usize),
    Chip(usize),
    Site(usize),
    SiteDot(usize),
    Ship(usize),
    NoEscort,
}

#[derive(Component)]
struct ShipIcon(usize);

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        let geometry = Geometry::new(&app.world().resource::<GameContent>().0);
        app.insert_resource(MapGeometry(geometry))
            .init_resource::<VoyageMap>()
            .add_message::<OpenMap>()
            .add_systems(Startup, spawn.after(crate::flight_hud::spawn_hud))
            .add_systems(OnEnter(AppMode::Map), enter)
            .add_systems(OnExit(AppMode::Map), |mut s: ResMut<UiScale>| s.0 = 1.0)
            .add_systems(Update, show.run_if(state_changed::<AppMode>))
            .add_systems(
                Update,
                (fit, input, refused, sync_map, sync_panel).chain().after(UiInputSet).before(UiStyleSet).run_if(in_state(AppMode::Map)),
            )
            .add_systems(
                PostUpdate,
                place_mesh
                    .after(UiSystems::Layout)
                    .after(CameraSet)
                    .before(bevy::transform::TransformSystems::Propagate)
                    .run_if(in_state(AppMode::Map)),
            );
    }
}

/// The stage's scale for a window: the largest that fits it whole.
pub fn stage_scale(window: Vec2) -> f32 {
    (window.x / STAGE.x).min(window.y / STAGE.y).clamp(SCALE.0, SCALE.1)
}

/// Opening the map: the first open voyage not cleared yet is selected.
fn enter(career: Res<PlayerCareer>, content: Res<GameContent>, mut map: ResMut<VoyageMap>) {
    let content = &content.0;
    map.selected = default_voyage(career.career(), content).or(content.voyages.iter().next().map(|v| v.id.as_str())).map(str::to_string);
    map.say = None;
}

fn show(mode: Res<State<AppMode>>, mut root: Single<&mut Visibility, With<MapRoot>>, mut mesh: Single<&mut Visibility, (With<MapMesh>, Without<MapRoot>)>) {
    let want = if *mode.get() == AppMode::Map { Visibility::Inherited } else { Visibility::Hidden };
    if **root != want {
        **root = want;
    }
    if **mesh != want {
        **mesh = want;
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn(
    mut commands: Commands,
    fonts: Res<HudFonts>,
    style: Res<Style>,
    content: Res<GameContent>,
    geo: Res<MapGeometry>,
    atlas: Res<Atlas>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let mesh = meshes.add(MeshBuf::placeholder());
    commands.spawn((
        MapMesh { mesh: mesh.clone(), built: None },
        Mesh2d(mesh),
        MeshMaterial2d(atlas.material.clone()),
        Transform::from_xyz(0.0, 0.0, Z_MAP),
        Visibility::Hidden,
    ));
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let fonts = &*fonts;
    let content = &content.0;
    let geo = &geo.0;
    let abs = |left: f32, top: f32| Node { position_type: PositionType::Absolute, left: px(left), top: px(top), ..default() };
    let row = |gap: f32| Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(gap), ..default() };
    let column = |gap: f32| Node { flex_direction: FlexDirection::Column, row_gap: px(gap), ..default() };
    let dot = |size: f32| Node { width: px(size), height: px(size), flex_shrink: 0.0, border_radius: BorderRadius::MAX, ..default() };
    let words = TextLayout::no_wrap();

    let root = Node {
        position_type: PositionType::Absolute,
        width: percent(100),
        height: percent(100),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    };
    commands.spawn((MapRoot, root, Visibility::Hidden)).with_children(|root| {
        root.spawn((Stage, Node { width: px(STAGE.x), height: px(STAGE.y), flex_shrink: 0.0, ..default() })).with_children(|stage| {
            // ---- The map (everything left of the panel) ----
            stage.spawn(Node { position_type: PositionType::Absolute, left: px(0), top: px(0), width: px(PANEL_X), height: px(STAGE.y), ..default() }).with_children(|m| {
                // Top left: the way back and where we are.
                m.spawn(Node { position_type: PositionType::Absolute, left: px(22), top: px(14), ..row(14.0) }).with_children(|tl| {
                    tl.spawn((ring_node(40.0), MapButton::Back))
                        .with_child((UiIcon::Glyph { glyph: Glyph::Menu, color: ui.violet }, Node { width: px(22), height: px(22), ..default() }));
                    tl.spawn(column(4.0)).with_children(|col| {
                        col.spawn((MapText::Header, label(fonts, "", 10.0, c(Tone::Label, 1.0))));
                        col.spawn(prose(fonts, "Voyages", 22.0, c(Tone::Number, 1.0)));
                    });
                });
                // The band names along the top: this band between its edges, the next one past the gate.
                let b = &geo.band;
                let y = 30.0;
                let (outer, inner) = (b.x_at(b.outer, y).unwrap_or(0.0).max(0.0), b.x_at(b.inner, y).unwrap_or(PANEL_X));
                let centred = |x: f32| Node { position_type: PositionType::Absolute, left: px(x), top: px(y - 5.0), width: px(0), justify_content: JustifyContent::Center, ..default() };
                m.spawn(centred(((outer + inner) / 2.0).max(330.0))).with_child((label(fonts, &b.key.replace('_', " ").to_uppercase(), 8.5, c(Tone::Label, 0.8)), words));
                if let Some(next) = &b.next_key
                    && inner + 60.0 < PANEL_X
                {
                    m.spawn(centred(inner + 34.0)).with_child((label(fonts, &next.replace('_', " ").to_uppercase(), 8.5, c(Tone::Coral, 0.8)), words));
                }
                // The chain's times.
                for (k, &route) in geo.routes.iter().enumerate() {
                    let at = geo.route_label(route);
                    m.spawn((MapNode::Route(k), Node { position_type: PositionType::Absolute, left: px(at.x), top: px(at.y - 5.0), width: px(0), justify_content: JustifyContent::Center, ..default() }))
                        .with_child((MapText::Route(k), label(fonts, "", 8.0, c(Tone::Teal, 0.85)), words));
                }
                // The shipyard's label, left of it.
                let h = geo.home;
                m.spawn(Node { position_type: PositionType::Absolute, right: px(PANEL_X - h.x + 22.0), top: px(h.y - 10.0), align_items: AlignItems::FlexEnd, ..column(4.0) })
                    .with_children(|l| {
                        l.spawn((label(fonts, "SHIPYARD", 10.5, c(Tone::Number, 0.9)), words));
                        l.spawn((label(fonts, "HOME  ·  DOCKED", 8.5, c(Tone::Label, 1.0)), words));
                    });
                // The voyages: a click area on each marker, and its label (a click area too).
                for (k, v) in content.voyages.iter().enumerate() {
                    let at = geo.nodes[k];
                    m.spawn((MapButton::Voyage(k), Hot::default(), Node { position_type: PositionType::Absolute, left: px(at.x - 22.0), top: px(at.y - 22.0), width: px(44), height: px(44), ..default() }));
                    let left = geo.label_left(k);
                    let mut node = Node { position_type: PositionType::Absolute, top: px(at.y - 10.0), align_items: if left { AlignItems::FlexEnd } else { AlignItems::FlexStart }, ..column(4.0) };
                    if left {
                        node.right = px(PANEL_X - at.x + 24.0);
                    } else {
                        node.left = px(at.x + 24.0);
                    }
                    m.spawn((MapButton::Voyage(k), Hot::default(), node)).with_children(|l| {
                        l.spawn((MapText::Name(k), label(fonts, &v.name.to_uppercase(), 10.5, c(Tone::Number, 1.0)), words));
                        l.spawn((MapText::State(k), label(fonts, "", 8.5, c(Tone::Teal, 1.0)), words));
                        l.spawn((MapNode::Dots(k), Node { margin: UiRect::top(px(2)), ..row(4.0) })).with_children(|d| {
                            for s in site_dots(content, v).into_iter().take(MAX_DOTS) {
                                let mut n = d.spawn((dot(6.0), BackgroundColor(c(s.tone(), 1.0))));
                                if s.diamond() {
                                    n.insert((Node { border_radius: BorderRadius::ZERO, ..dot(5.0) }, UiTransform::from_rotation(Rot2::degrees(45.0))));
                                }
                            }
                        });
                    });
                }
                // The legend.
                let legend = [["HOME", "OPEN", "CLEARED", "LOCKED"], ["ENEMIES", "WRECKS", "GOAL SITE", "GUARDIAN"]];
                for (r, names) in legend.iter().enumerate() {
                    for (k, name) in names.iter().enumerate() {
                        m.spawn((label(fonts, name, 8.0, c(Tone::Label, 1.0)), words, abs(LEGEND_COLUMNS[k] + LEGEND_GAP, LEGEND_ROWS[r] - 5.5)));
                    }
                }
            });

            // ---- The panel: the selected voyage, the fleet, SET COURSE ----
            stage
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    left: px(PANEL_X),
                    top: px(0),
                    width: px(PANEL_W),
                    height: px(STAGE.y),
                    padding: UiRect::new(px(PAD), px(PAD), px(24), px(22)),
                    ..column(8.0)
                })
                .with_children(|p| {
                    p.spawn((MapText::Head, label(fonts, "", 9.0, c(Tone::Label, 1.0))));
                    p.spawn((MapText::Title, prose(fonts, "", 24.0, c(Tone::Number, 1.0))));
                    p.spawn(Node { flex_wrap: FlexWrap::Wrap, row_gap: px(6), ..row(8.0) }).with_children(|r| {
                        for k in 0..MAX_CHIPS {
                            r.spawn((
                                MapNode::Chip(k),
                                Node { padding: UiRect::axes(px(9), px(4)), border: UiRect::all(px(1)), border_radius: BorderRadius::MAX, ..default() },
                                BorderColor::all(Color::NONE),
                            ))
                            .with_child((MapText::Chip(k), label(fonts, "", 8.0, c(Tone::Label, 1.0)), words));
                        }
                    });
                    p.spawn((MapText::Summary, prose(fonts, "", 12.0, c(Tone::Label, 1.0))));
                    p.spawn((MapText::SitesHead, label(fonts, "", 9.0, c(Tone::Label, 1.0)), Node { margin: UiRect::top(px(4)), ..default() }));
                    for k in 0..MAX_SITES {
                        p.spawn((MapNode::Site(k), column(2.0))).with_children(|s| {
                            s.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..row(8.0) }).with_children(|line| {
                                line.spawn(row(9.0)).with_children(|l| {
                                    l.spawn((MapNode::SiteDot(k), dot(7.0), BackgroundColor(Color::NONE), UiTransform::default()));
                                    l.spawn((MapText::SiteName(k), prose(fonts, "", 13.5, c(Tone::Number, 1.0)), words));
                                });
                                line.spawn((MapText::SiteTag(k), label(fonts, "", 8.0, c(Tone::Coral, 1.0)), words));
                            });
                            s.spawn((MapText::SiteGives(k), tracked(fonts, "", 7.5, c(Tone::Dim, 1.0), 0.15), Node { margin: UiRect::left(px(16)), ..default() }));
                        });
                    }
                    p.spawn((MapText::Minutes, label(fonts, "", 8.5, c(Tone::Label, 1.0)), Node { margin: UiRect::top(px(2)), ..default() }));
                    p.spawn((Node { height: px(1), flex_shrink: 0.0, margin: UiRect::vertical(px(4)), ..default() }, BackgroundColor(c(Tone::Dim, 0.35))));
                    p.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..row(8.0) }).with_children(|r| {
                        r.spawn(label(fonts, "FLEET", 9.0, c(Tone::Label, 1.0)));
                        r.spawn(row(8.0)).with_children(|cores| {
                            cores.spawn(label(fonts, "CORES", 9.0, c(Tone::Label, 1.0)));
                            cores.spawn((MapText::Cores, number(fonts, "", 15.0, c(Tone::Violet, 1.0))));
                        });
                    });
                    for k in 0..SHIPS {
                        let (w, h) = if k == 0 { (64.0, 36.0) } else { (44.0, 24.0) };
                        p.spawn((MapNode::Ship(k), row(12.0))).with_children(|r| {
                            r.spawn((ShipIcon(k), UiIcon::Glyph { glyph: Glyph::Formation, color: ui.dim }, Node { width: px(w), height: px(h), flex_shrink: 0.0, ..default() }));
                            r.spawn(column(3.0)).with_children(|t| {
                                t.spawn((MapText::ShipName(k), prose(fonts, "", if k == 0 { 14.0 } else { 12.5 }, c(Tone::Number, 1.0)), words));
                                t.spawn((MapText::ShipSub(k), tracked(fonts, "", 7.5, c(Tone::Dim, 1.0), 0.15), words));
                            });
                        });
                    }
                    p.spawn((
                        MapNode::NoEscort,
                        Node { padding: UiRect::axes(px(12), px(7)), border: UiRect::all(px(1)), border_radius: BorderRadius::MAX, ..default() },
                        BorderColor::all(c(Tone::Dim, 0.45)),
                    ))
                    .with_child((MapText::NoEscort, label(fonts, "", 8.0, c(Tone::Dim, 1.0)), words));
                    p.spawn((MapText::ReadyHead, label(fonts, "", 9.0, c(Tone::Teal, 1.0)), Node { margin: UiRect::top(px(4)), ..default() }));
                    for r in 0..2 {
                        p.spawn(row(0.0)).with_children(|line| {
                            for k in [2 * r, 2 * r + 1] {
                                line.spawn(Node { width: percent(50), ..row(7.0) }).with_children(|cell| {
                                    cell.spawn((MapText::Mark(k), label(fonts, "", 9.0, c(Tone::Teal, 1.0))));
                                    cell.spawn((MapText::Check(k), tracked(fonts, "", 8.0, c(Tone::Label, 1.0), 0.18)));
                                });
                            }
                        });
                    }
                    p.spawn(Node { flex_grow: 1.0, ..default() });
                    p.spawn((MapText::Status, label(fonts, "", 9.0, c(Tone::Teal, 1.0)), Node { margin: UiRect::bottom(px(4)), ..default() }));
                    let play = UiIcon::Glyph { glyph: Glyph::Play, color: ui.teal };
                    spawn_pill(p, fonts, "SET COURSE", 12.0, 46.0, Some(play), Pill { primary: true, ..Pill::new(Tone::Teal) }, MapButton::SetCourse);
                });
        });
    });
}

/// The stage fits the window; the camera rig knows the window (the mesh and the icons are placed through it).
fn fit(window: Single<&Window, With<PrimaryWindow>>, mut ui_scale: ResMut<UiScale>, mut rig: ResMut<CameraRig>) {
    let s = stage_scale(window.size());
    if (ui_scale.0 - s).abs() > 1e-3 {
        ui_scale.0 = s;
    }
    if rig.viewport != window.size() || rig.view_offset != Vec2::ZERO {
        rig.viewport = window.size();
        rig.view_offset = Vec2::ZERO;
    }
}

/// What a launch of the selected voyage reads.
#[derive(bevy::ecs::system::SystemParam)]
struct FleetNow<'w> {
    content: Res<'w, GameContent>,
    career: Res<'w, PlayerCareer>,
    granted: Res<'w, Granted>,
    design: Res<'w, PlayerDesign>,
    escorts: Res<'w, EscortDesigns>,
}

impl FleetNow<'_> {
    /// Why the voyage can't launch now, if it can't.
    fn blocker(&self, voyage: &str) -> Option<String> {
        let c = &self.content.0;
        let fleet = fleet_blocker(c, &self.granted.unlocks, self.granted.mind_capacity, self.design.blueprint(), &self.escorts.0);
        course_blocker(c, self.career.career(), voyage, fleet)
    }
}

/// Clicks, taps and keys: select, launch, go home.
#[allow(clippy::too_many_arguments)]
fn input(
    mut clicks: MessageReader<UiClick>,
    buttons: Query<&MapButton>,
    keys: Res<ButtonInput<KeyCode>>,
    pointer: Res<UiPointer>,
    fleet: FleetNow,
    mut map: ResMut<VoyageMap>,
    mut next: ResMut<NextState<AppMode>>,
    mut start: MessageWriter<StartVoyage>,
) {
    let content = &fleet.content.0;
    let n = content.voyages.len();
    let at = map.selected.as_deref().and_then(|id| content.voyages.position(id));
    let select = |map: &mut VoyageMap, k: usize| {
        let id = content.voyages.iter().nth(k).map(|v| v.id.clone());
        if map.selected != id {
            map.selected = id;
            map.say = None;
        }
    };
    let mut launch = false;
    for c in clicks.read() {
        match buttons.get(c.0) {
            Ok(MapButton::Back) => next.set(AppMode::Home),
            Ok(MapButton::Voyage(k)) => select(&mut map, *k),
            Ok(MapButton::SetCourse) => launch = true,
            Err(_) => {}
        }
    }
    if !pointer.typing && n > 0 {
        if keys.just_pressed(KeyCode::Escape) {
            next.set(AppMode::Home);
        }
        if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
            launch = true;
        }
        let step = if keys.any_just_pressed([KeyCode::ArrowRight, KeyCode::ArrowDown]) {
            1
        } else if keys.any_just_pressed([KeyCode::ArrowLeft, KeyCode::ArrowUp]) {
            n - 1
        } else {
            0
        };
        if step > 0 {
            select(&mut map, at.map_or(0, |k| (k + step) % n));
        }
    }
    if launch && let Some(id) = map.selected.clone() {
        match fleet.blocker(&id) {
            None => {
                map.say = None;
                start.write(StartVoyage { voyage: Some(id) });
            }
            Some(why) => map.say = Some(why),
        }
    }
}

/// A launch `voyage.rs` refused: the panel says why.
fn refused(mut refusals: MessageReader<LaunchRefused>, mut map: ResMut<VoyageMap>) {
    if let Some(LaunchRefused(why)) = refusals.read().last() {
        map.say = Some(why.clone());
    }
}

/// The map's words: the header, each voyage's name and state, its dots, the chain's times.
#[allow(clippy::type_complexity)]
fn sync_map(
    map: Res<VoyageMap>,
    career: Res<PlayerCareer>,
    content: Res<GameContent>,
    geo: Res<MapGeometry>,
    style: Res<Style>,
    hots: Query<(&MapButton, Ref<Hot>)>,
    mut texts: Query<(&MapText, &mut Text, &mut TextColor)>,
    mut nodes: Query<(&MapNode, &mut Node)>,
) {
    let hover = hots.iter().any(|(_, h)| h.is_changed());
    if !(hover || map.is_changed() || career.is_changed()) {
        return;
    }
    let content = &content.0;
    let ui = &style.ui;
    let states = VoyageState::all(content, career.career());
    let selected = map.selected.as_deref().and_then(|id| content.voyages.position(id));
    let hovered = |k: usize| hots.iter().any(|(b, h)| *b == MapButton::Voyage(k) && h.hovered);
    for (t, mut text, mut color) in &mut texts {
        let (words, tone, alpha) = match *t {
            MapText::Header => {
                let cleared = states.iter().filter(|s| matches!(s, VoyageState::Cleared { .. })).count();
                let band = geo.0.band.key.replace('_', " ").to_uppercase();
                (format!("{band}  ·  {} VOYAGES  ·  {cleared} CLEARED", states.len()), Tone::Label, 1.0)
            }
            MapText::Name(k) => {
                let lit = selected == Some(k) || hovered(k);
                let tone = if !lit && states[k] == VoyageState::Locked { Tone::Label } else { Tone::Number };
                (text.0.clone(), tone, if lit { 1.0 } else { 0.8 })
            }
            MapText::State(k) => {
                let v = content.voyages.iter().nth(k);
                let (w, tone) = state_words(states[k], v.is_some_and(|v| is_guardian(content, v)));
                (w, tone, 1.0)
            }
            MapText::Route(k) => {
                let to = geo.0.routes[k].1;
                (content.voyages.iter().nth(to).map(minutes).unwrap_or_default(), Tone::Teal, 0.85)
            }
            _ => continue,
        };
        set_text(&mut text, &words);
        set_color(&mut color, ui_color(tone.color(ui), alpha));
    }
    for (n, mut node) in &mut nodes {
        let show = match *n {
            MapNode::Dots(k) => states[k] != VoyageState::Locked,
            MapNode::Route(k) => states[geo.0.routes[k].1] != VoyageState::Locked,
            _ => continue,
        };
        let d = if show { Display::Flex } else { Display::None };
        if node.display != d {
            node.display = d;
        }
    }
}

/// The panel follows the selection, the career and the fleet.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync_panel(
    map: Res<VoyageMap>,
    fleet: FleetNow,
    style: Res<Style>,
    mut texts: Query<(&MapText, &mut Text, &mut TextColor)>,
    mut nodes: Query<(&MapNode, &mut Node, Option<&mut BackgroundColor>, Option<&mut BorderColor>, Option<&mut UiTransform>)>,
    mut icons: Query<(&ShipIcon, &mut UiIcon)>,
    mut pill: Single<&mut Pill, With<MapButton>>,
) {
    let f = &fleet;
    if !(map.is_changed() || f.career.is_changed() || f.granted.is_changed() || f.design.is_changed() || f.escorts.is_changed()) {
        return;
    }
    let content = &f.content.0;
    let career = f.career.career();
    let ui = &style.ui;
    let Some(id) = map.selected.as_deref() else { return };
    let (Some(k), Some(v)) = (content.voyages.position(id), content.voyages.get(id)) else { return };
    let state = VoyageState::of(content, career, &v.id);
    let chips = chips(content, v);
    let sites = site_rows(content, v);
    let flagship = f.design.blueprint();
    let escorts = &f.escorts.0;
    let ships: Vec<&data::Blueprint> = std::iter::once(flagship).chain(escorts).collect();
    let mind = f.granted.mind_capacity;
    let used = fleet_cores(content, flagship, escorts);
    let checks = layout::checklist(content, &f.granted.unlocks, mind, flagship, escorts);
    let blocker = f.blocker(&v.id);
    let plural = |n: u32, one: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {one}S") };
    let ship_sub = |k: usize, b: &data::Blueprint| {
        let t = content.ship_totals(b);
        let who = if k == 0 { "FLAGSHIP".to_string() } else { format!("ESCORT {k}") };
        format!("{who}  ·  {}  ·  CARGO {}", plural(t.logic_cores, "CORE"), content.ship_hold(b))
    };
    // What the line over SET COURSE says: a refusal, the blocker, a practice run (`?unlock=all` grants what the
    // career doesn't), or how the voyage stands.
    let status = if let Some(why) = &map.say {
        (why.clone(), Tone::Coral)
    } else if let Some(b) = &blocker {
        (b.clone(), Tone::Coral)
    } else if let Some(own) = f.granted.free.then(|| fleet_blocker(content, &career.unlocks(content), career.mind_capacity(content), flagship, escorts)).flatten() {
        (format!("PRACTICE RUN, NOT SAVED  ·  {own}"), Tone::Gold)
    } else {
        match state {
            VoyageState::Cleared { best } => (
                format!("CLEARED{}  ·  EACH LAUNCH ROLLS NEW SITES", best.map(|t| format!("  ·  BEST {}", crate::voyage::clock(t))).unwrap_or_default()),
                Tone::Teal,
            ),
            _ => ("THE GATE IS OPEN".into(), Tone::Teal),
        }
    };
    for (t, mut text, mut color) in &mut texts {
        let (words, tone) = match *t {
            MapText::Head => (format!("DESTINATION  ·  VOYAGE {} OF {}", k + 1, content.voyages.len()), Tone::Label),
            MapText::Title => (v.name.clone(), Tone::Number),
            MapText::Summary => (content.render_plain(&v.summary), Tone::Label),
            MapText::Chip(j) => chips.get(j).cloned().unwrap_or((String::new(), Tone::Label)),
            MapText::SitesHead => (format!("SITES  ·  {}", v.site_count()), Tone::Label),
            MapText::SiteName(j) => (sites.get(j).map_or(String::new(), |s| s.name.clone()), Tone::Number),
            MapText::SiteTag(j) => sites.get(j).map_or((String::new(), Tone::Dim), |s| s.tag.clone()),
            MapText::SiteGives(j) => (sites.get(j).map_or(String::new(), |s| s.gives.clone()), Tone::Dim),
            MapText::Minutes => (format!("{}  ·  RECALL WHEN CLEAR", minutes(v)), Tone::Label),
            MapText::Cores => (format!("{used} / {mind}"), if used > mind { Tone::Coral } else { Tone::Violet }),
            MapText::ShipName(j) => (ships.get(j).map_or(String::new(), |b| b.name.clone()), Tone::Number),
            MapText::ShipSub(j) => (ships.get(j).map_or(String::new(), |b| ship_sub(j, b)), Tone::Dim),
            MapText::NoEscort => (format!("NO ESCORT  ·  YOUR MIND RUNS {}", plural(mind, "CORE")), Tone::Dim),
            MapText::ReadyHead if checks.iter().all(layout::Check::ok) => ("VOYAGE-READY".into(), Tone::Teal),
            MapText::ReadyHead => ("NOT VOYAGE-READY".into(), Tone::Coral),
            MapText::Mark(j) => (if checks[j].ok() { "✓" } else { "×" }.into(), checks[j].tone),
            MapText::Check(j) => (checks[j].words.clone(), if checks[j].ok() { Tone::Label } else { Tone::Coral }),
            MapText::Status => status.clone(),
            _ => continue,
        };
        set_text(&mut text, &words);
        set_color(&mut color, ui_color(tone.color(ui), 1.0));
    }
    for (n, mut node, bg, border, transform) in &mut nodes {
        let show = match *n {
            MapNode::Chip(j) => {
                if let (Some(mut b), Some((_, tone))) = (border, chips.get(j)) {
                    let want = BorderColor::all(ui_color(tone.color(ui), 0.7));
                    if *b != want {
                        *b = want;
                    }
                }
                j < chips.len()
            }
            MapNode::Site(j) => j < sites.len(),
            MapNode::SiteDot(j) => {
                if let Some(s) = sites.get(j) {
                    if let Some(mut bg) = bg {
                        let want = ui_color(s.dot.tone().color(ui), 1.0);
                        if bg.0 != want {
                            bg.0 = want;
                        }
                    }
                    let (radius, size, rot) = if s.dot.diamond() { (BorderRadius::ZERO, 6.0, 45.0) } else { (BorderRadius::MAX, 7.0, 0.0) };
                    if node.border_radius != radius || node.width != px(size) {
                        node.border_radius = radius;
                        node.width = px(size);
                        node.height = px(size);
                    }
                    if let Some(mut t) = transform {
                        let want = UiTransform::from_rotation(Rot2::degrees(rot));
                        if *t != want {
                            *t = want;
                        }
                    }
                }
                true
            }
            MapNode::Ship(j) => j < ships.len(),
            MapNode::NoEscort => escorts.is_empty(),
            MapNode::Route(_) | MapNode::Dots(_) => continue,
        };
        let d = if show { Display::Flex } else { Display::None };
        if node.display != d {
            node.display = d;
        }
    }
    for (i, mut icon) in &mut icons {
        let want = match ships.get(i.0) {
            Some(b) => UiIcon::Ship(Box::new((*b).clone())),
            None => UiIcon::Glyph { glyph: Glyph::Formation, color: ui.dim },
        };
        if *icon != want {
            *icon = want;
        }
    }
    let disabled = blocker.is_some();
    if pill.disabled != disabled {
        pill.disabled = disabled;
        pill.primary = !disabled;
    }
}

/// The star map's mesh: rebuilt when what it shows changes, placed over the stage (stage px → world through the
/// camera rig, as `ui_icons` places icons).
#[allow(clippy::too_many_arguments)]
fn place_mesh(
    stage: Single<(&ComputedNode, &UiGlobalTransform), With<Stage>>,
    window: Single<&Window, With<PrimaryWindow>>,
    rig: Res<CameraRig>,
    mesh: Single<(&mut MapMesh, &mut Transform)>,
    geo: Res<MapGeometry>,
    map: Res<VoyageMap>,
    career: Res<PlayerCareer>,
    content: Res<GameContent>,
    hots: Query<(&MapButton, &Hot)>,
    toggles: Res<RenderToggles>,
    style: Res<Style>,
    atlas: Res<Atlas>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let content = &content.0;
    let look = Look {
        states: VoyageState::all(content, career.career()),
        guardian: content.voyages.iter().map(|v| is_guardian(content, v)).collect(),
        selected: map.selected.as_deref().and_then(|id| content.voyages.position(id)),
        hovered: hots.iter().find_map(|(b, h)| match b {
            MapButton::Voyage(k) if h.hovered => Some(*k),
            _ => None,
        }),
        glow: toggles.glow_sprites,
    };
    let (mut m, mut tf) = mesh.into_inner();
    if m.built.as_ref() != Some(&look) {
        let built = draw::build(&geo.0, &look, &style, &atlas.region("fx/white"), &atlas.region("fx/dot_soft"));
        let _ = meshes.insert(&m.mesh, built);
        m.built = Some(look);
    }
    let (node, at) = *stage;
    let scale = window.scale_factor();
    let k = node.size.x / scale / STAGE.x;
    if k <= 0.0 {
        return;
    }
    let want = Transform {
        translation: rig.screen_to_world(at.translation / scale).extend(Z_MAP),
        scale: Vec3::new(k / rig.zoom, k / rig.zoom, 1.0),
        ..default()
    };
    if *tf != want {
        *tf = want;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stage fits any window whole: 1280 × 720 and 1600 × 900 exactly, a wide or a tall window by its short side.
    #[test]
    fn the_stage_fits_any_window() {
        assert_eq!(stage_scale(Vec2::new(1280.0, 720.0)), 1.0);
        assert_eq!(stage_scale(Vec2::new(1600.0, 900.0)), 1.25);
        for w in [Vec2::new(3440.0, 1305.0), Vec2::new(800.0, 1280.0), Vec2::new(1024.0, 768.0), Vec2::new(500.0, 900.0)] {
            let s = stage_scale(w);
            assert!(STAGE.x * s <= w.x + 0.01 && STAGE.y * s <= w.y + 0.01, "{w}: {s}");
        }
    }
}
