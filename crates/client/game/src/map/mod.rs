//! The voyage map ([`AppMode::Map`], track 7I; GDR-0065's star map; the galaxy's map since galaxy plan E5,
//! GDR-0097): where voyages are chosen. Home's gate (V, or a click) opens it through the gate ([`portal`]: the camera
//! dives into the gate and the map shows through it); Esc or the ring button top left goes back to the shipyard (the
//! same way out); SET COURSE launches the selection (`voyage::StartVoyage`), a jump after its warp.
//!
//! ```text
//! (o) RIM · 3 CHARTED · 1 CLEARED · 2 SIGNALS    RIM :    OUTER  | BLIND JUMP · 1.6 S FROM HOME
//!     Voyages                                         :           | Unknown space
//!          SIGNAL · A GATE        ((                  :           | [RIM] [UNCHARTED]
//!       a log from a hauler's wreck  .  .  .  .     :           | Nobody has charted this space. ...
//!     .  ((o)) HALVEX DRIFT   ·  (+)  .              :           | INSIDE A SIGNAL'S AREA  ◆ A gate  HEARD
//!   .     SHIPYARD APPROACH  ●   o VARDIS          :           | WARP ≈ 6 S · RECALL WHEN CLEAR
//!              <> SHIPYARD          CHARTED        :           | FLEET   CORES 2 / 2   [ship] Rim skiff ...
//!  <> HOME  o OPEN  ● CLEARED  ((o)) SIGNAL  o CHARTED  <> GUARDIAN       | VOYAGE-READY ✓ FLIGHT-READY ...
//!  ● ENEMIES ● WRECKS ◆ GOAL SITE .:. SIGNAL AREA  o LOCATED  ) JUMP RANGE · 2.5 S | [ ▶ SET COURSE ]
//! ```
//!
//! * **Layout** (`layout`): a stage of 1280 × 720 logical px scaled to fit the window at any aspect (centred,
//!   nothing cut off); the map left of the panel: the career's galaxy at [`layout::PX_PER_S`] a S, turned so home
//!   sits left of the middle and the bands (from the content) bend away to the right; a slice voyage sits on the
//!   cluster it is pinned to. **States** come from the career: a voyage new (a signal), open, cleared (with the best
//!   time), a guardian; no route joins anything (GDR-0078: the rim is explored, not followed). Voyages not open yet
//!   and uncharted clusters are unknown space: nothing drawn, no click area.
//! * **What the career knows** (`known`, galaxy plan E5): the clusters charted, the signals heard (fresh or narrowed:
//!   an area with the trail's hint over its source's line, `SIGNAL · A GATE` over "a log from a hauler's wreck";
//!   located: a gold mark on its cluster), the gates opened, the jump range round home and every charted cluster.
//! * **Drawing** (`draw`, `galaxy`): the star map is five atlas meshes under the UI text (the world draws before
//!   bevy_ui), placed over the stage node through the camera rig like `ui_icons`, each with its own material for
//!   its fade: the galaxy and its far layer (moved and scaled with the view), the areas (the jump range and the
//!   signals), the detail (markers, the pick, the warp; both placed through the view, rebuilt when what they show or
//!   the view changes) and the chrome (the legend and the panel's backing, fixed). The panel's ships are
//!   `ui_icons::UiIcon::Ship`.
//! * **The view** (`view`): zoom (the wheel or a pinch around the cursor, + / −, the buttons bottom right) from 2×
//!   out to the whole galaxy, pan (drag), reset (0, a double click on empty space, the reset button); entering
//!   starts at the default view. The panel, the header and the legend stay fixed.
//! * **Input**: a click or a tap on a voyage, a known cluster (its marker or its label) or a signal's label selects
//!   it (zoomed far out, a click on a voyage's beacon also zooms in to it); a click on empty space picks the spot for
//!   a blind jump (by a known cluster: that cluster); ←/→ (and ↑/↓) step through the voyages and clusters known
//!   (bringing one off the view into it); Enter or SET COURSE launches; Esc or the ring button goes home (back out
//!   through the gate; not while the portal is on its way out), or ends a warp. SET COURSE is dimmed while the
//!   selection can't launch (`layout::course_blocker`, `known::jump_blocker`: not open or out of reach, another
//!   voyage underway, the fleet not ready); a click then says why, as does a launch `voyage.rs` refuses.
//! * **The warp** (galaxy plan E6, GDR-0093: 3 s + 2 s per S): SET COURSE on a cluster, a signal or a spot runs a
//!   light from where the jump starts to the cluster it goes to, the status counting down (`WARP · 5 S`); then the
//!   launch. Esc ends it on the way, nothing launched. A slice voyage launches at once, as before.
//! * **Entering** selects the first open voyage whose goal is not cleared (`voyage::default_voyage`): after a voyage
//!   that found a signal, that one. The debrief's VOYAGE MAP sends [`OpenMap`] (home opens the gate, as V).
//!
//! UI words are chrome in code, like home's; the clusters', trails' and sources' words are content.

pub mod draw;
pub mod galaxy;
pub mod known;
pub mod layout;
pub mod portal;
pub mod view;

use std::collections::BTreeMap;

use bevy::prelude::*;
use data::career::TrailSignal;
use data::input::Action;
use data::skeleton::Area;
use world::settings::ActionTip;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale, UiSystems, UiTransform};
use bevy::window::PrimaryWindow;

use bevy::state::state::StateTransitionEvent;
use kit::atlas::{Atlas, AtlasMaterial, set_alpha};
use kit::meshbuf::MeshBuf;
use kit::params::RenderToggles;
use kit::style::{Style, ui_color};
use kit::ui_icons::{Glyph, UiIcon};
use kit::ui_kit::{
    CompactText, HudFonts, Edge, Hot, LayoutClass, Pill, ScreenFx, ScreenPart, Scrollbar, Tip, Tone, show_screen, snap_screen, UiBlock, UiClick, UiInputSet, UiPointer, UiStyleSet, compact_scale, crumb, frame, label, number,
    prose, spawn_back, title, ring_glyph, ring_node, set_color, set_text, spawn_pill, tracked,
};
use world::camera::{CameraRig, CameraSet};
use world::career::{EscortDesigns, Granted, PlayerCareer, fleet_cores};
use world::mode::AppMode;
use world::player::PlayerDesign;
use world::sim_bridge::GameContent;
use world::voyage::{Destination, LaunchRefused, StartVoyage, default_voyage, fleet_blocker};
use crate::fleet::MAX_ESCORTS;

use portal::Portal;

use draw::{FADE, Hover, LEGEND, LEGEND_COLUMNS, LEGEND_GAP, LEGEND_ROWS, Look, WarpLook};
use known::{Known, Pick, jump_blocker, place_words};
use layout::{Geometry, PANEL_X, STAGE, VoyageState, chips, course_blocker, is_guardian, minutes, site_dots, site_rows, state_words};
use view::{Anchored, Layer, MapView, TOP_Y, ViewButton, ZoomText};

/// Above the world's layers (effects ≤ 400), under the UI icons (600) that draw the panel's ships; the map's meshes
/// stack a little apart above it ([`Sheet::z`]).
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
/// On a phone ([`LayoutClass::Compact`], docs/client.md "Phones"): the panel's width and padding (its rows scroll,
/// SET COURSE stays at the bottom), the least text size (`kit::ui_kit::CompactText`), and the gap kept between the
/// zoom buttons (top right of the map, beside the header) and the panel. The legend is left out.
const COMPACT_PANEL_W: f32 = 300.0;
const COMPACT_PAD: (f32, f32) = (18.0, 12.0);
const COMPACT_TEXT: f32 = 9.0;
const COMPACT_ZOOM_GAP: f32 = 14.0;
/// The legend's box (stage px): from just over its first row's symbols to the stage's bottom.
const LEGEND_TOP: f32 = 664.0;
/// A signal's label stack is about this tall (stage px).
const SIGNAL_LABEL: f32 = 42.0;

/// The voyage map's state (a resource).
#[derive(Resource, Debug, Default)]
pub struct VoyageMap {
    /// What the panel shows.
    pub selected: Option<Pick>,
    /// Why the last SET COURSE did not launch.
    say: Option<String>,
    /// Each signal's area as last drawn, and old edges fading out (the area, since when: real seconds).
    seen: BTreeMap<TrailSignal, Area>,
    fading: Vec<(Area, f32)>,
    /// A warp under way (galaxy plan E6).
    warp: Option<Warp>,
}

/// A warp on the map: to the cluster `target`, from where the jump starts (galaxy px both), since `start` (real
/// seconds) for `seconds`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Warp {
    target: u32,
    from: Vec2,
    to: Vec2,
    start: f32,
    seconds: f32,
}

/// The debrief's VOYAGE MAP: home flies to the gate and opens the map, as V does.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenMap;

/// Where everything sits (built once from the content and the career's galaxy).
#[derive(Resource)]
struct MapGeometry(Geometry);

/// What the career knows of the galaxy (`known::Known`), kept up with the career.
#[derive(Resource, Default)]
struct MapKnown(Known);

#[derive(Component)]
struct MapRoot;

/// The 1280 × 720 stage everything is laid out on.
#[derive(Component)]
struct Stage;

/// What cuts the map's labels: the map area the UI leaves free (left of the panel, run out to the window's edge,
/// between the header and the legend; measured by `fit`). `MapClip(true)` cuts the top band names instead: the
/// strip right of the header, over the area.
#[derive(Component)]
struct MapClip(bool);

/// The labels' frame inside a clip: placed so it lies on the stage (its px are stage px).
#[derive(Component)]
struct MapFrame;

/// The chrome `fit` measures and places: the header (top left), the panel, the legend and the zoom buttons.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum Chrome {
    Header,
    Panel,
    Legend,
    Zoom,
}

/// One of the star map's meshes, its material (for its fade) and what it shows.
#[derive(Component)]
struct MapMesh {
    sheet: Sheet,
    mesh: Handle<Mesh>,
    material: Handle<AtlasMaterial>,
    built: Option<Built>,
}

/// The star map's meshes, bottom to top.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sheet {
    /// The galaxy (`galaxy::build`, rebuilt when a band opens), moved and scaled with the view.
    Galaxy,
    /// Its far layer (`galaxy::build_far`), likewise.
    Far,
    /// The jump range and the signals' areas (`draw::build_areas`), placed through the view.
    Areas,
    /// The voyages' and clusters' detail (`draw::build_detail`), placed through the view.
    Detail,
    /// The legend and the panel's backing (`draw::build_chrome`), fixed.
    Chrome,
}

impl Sheet {
    const ALL: [Sheet; 5] = [Sheet::Galaxy, Sheet::Far, Sheet::Areas, Sheet::Detail, Sheet::Chrome];

    fn z(self) -> f32 {
        Z_MAP + 0.1 * self as u8 as f32
    }

    /// The layer it fades with as the view zooms.
    fn layer(self) -> Layer {
        match self {
            Sheet::Galaxy | Sheet::Chrome | Sheet::Areas => Layer::Always,
            Sheet::Far => Layer::Far,
            Sheet::Detail => Layer::Detail,
        }
    }

    /// Moved and scaled with the view (else drawn in stage px).
    fn zooms(self) -> bool {
        matches!(self, Sheet::Galaxy | Sheet::Far)
    }
}

/// What a mesh was built for (its rebuild key).
#[derive(Clone, Debug, PartialEq)]
enum Built {
    /// The panel's place and whether the legend shows (the chrome's).
    Chrome(Rect, bool),
    /// The bands open (the galaxy's).
    Bands(Vec<bool>),
    Far(Look),
    /// The look and the view (zoom, offset, the map area).
    Placed(Look, f32, Vec2, Rect),
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum MapButton {
    Back,
    Voyage(usize),
    Cluster(u32),
    /// A signal's label: `Geometry::signals[j]`.
    Signal(usize),
    SetCourse,
    View(ViewButton),
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum MapText {
    Header,
    Name(usize),
    State(usize),
    ClusterState(u32),
    SignalHead(usize),
    SignalLine(usize),
    SignalState(usize),
    /// The legend's last word: the jump range now.
    Reach,
    /// A band's state under its name (`Geometry::rings[k]`), and the next band's name and state along the top.
    BandState(usize),
    TopNext,
    TopNextState,
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
}

/// Nodes shown, hidden or recoloured with the data.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum MapNode {
    /// A voyage's click area or label: shown only once the voyage is known.
    Voyage(usize),
    /// The tag around a voyage's state words: outlined while it is a new signal.
    Tag(usize),
    /// A cluster's click area or label: shown only while the map knows it.
    Cluster(u32),
    /// A signal's label (`Geometry::signals[j]`): shown, at its area's top, while it is an area.
    Signal(usize),
    Chip(usize),
    /// Over the panel's rows (the sites, the signals); hidden without rows.
    SitesHead,
    Site(usize),
    SiteDot(usize),
    Ship(usize),
    NoEscort,
    /// A check's mark: a filled dot when it passes, a ring when it doesn't.
    Mark(usize),
}

#[derive(Component)]
struct ShipIcon(usize);

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        let world = app.world();
        let (content, career) = (&world.resource::<GameContent>().0, world.resource::<PlayerCareer>().career());
        let geometry = Geometry::new(content, career.exploration());
        let view = MapView::new(geometry.band.centre, geometry.band.outer);
        let known = Known::of(content, career);
        let map = VoyageMap { seen: areas_now(&known), ..default() };
        app.insert_resource(MapGeometry(geometry))
            .insert_resource(MapKnown(known))
            .insert_resource(view)
            .insert_resource(map)
            .add_message::<OpenMap>()
            .add_plugins(portal::plugin)
            .add_systems(Startup, spawn.after(world::flight_hud::spawn_hud))
            .add_systems(OnEnter(AppMode::Map), enter)
            .add_systems(OnExit(AppMode::Map), leave)
            .add_systems(PreUpdate, know.run_if(resource_changed::<PlayerCareer>))
            .add_systems(Update, (show, fade_mesh).chain().after(portal::PortalSet))
            .add_systems(
                Update,
                (fit, view::input, input, warp, view::ease, refused, track_areas, sync_map, sync_panel, view::place_labels)
                    .chain()
                    .after(UiInputSet)
                    .before(UiStyleSet)
                    .run_if(in_state(AppMode::Map)),
            )
            .add_systems(
                PostUpdate,
                place_mesh
                    .after(UiSystems::Layout)
                    .after(CameraSet)
                    .before(bevy::transform::TransformSystems::Propagate)
                    .run_if(shown),
            );
    }
}

/// The stage's scale for a window: the largest that fits it whole.
pub fn stage_scale(window: Vec2) -> f32 {
    (window.x / STAGE.x).min(window.y / STAGE.y).clamp(SCALE.0, SCALE.1)
}

/// Each signal's area as the map draws it: its area while it is one, its cluster's place (radius 0) once located.
fn areas_now(known: &Known) -> BTreeMap<TrailSignal, Area> {
    known.signals.iter().map(|m| (m.signal, if m.is_area() { m.area } else { Area { centre: m.area.centre, radius: 0 } })).collect()
}

/// The career changed: what the map knows follows.
fn know(career: Res<PlayerCareer>, content: Res<GameContent>, mut known: ResMut<MapKnown>) {
    let now = Known::of(&content.0, career.career());
    if known.0 != now {
        known.0 = now;
    }
}

/// Opening the map: the default view, the first open voyage not cleared yet selected (else the first known one).
fn enter(career: Res<PlayerCareer>, content: Res<GameContent>, mut map: ResMut<VoyageMap>, mut view: ResMut<MapView>) {
    view.snap_default();
    let content = &content.0;
    let career = career.career();
    let known = || content.voyages.iter().find(|v| VoyageState::of(content, career, &v.id).known()).map(|v| v.id.as_str());
    map.selected = default_voyage(career, content).or_else(known).map(|id| Pick::Voyage(id.to_string()));
    map.say = None;
    map.warp = None;
}

/// Leaving the map: the UI scale back to the window's; a warp not finished ends.
fn leave(mut s: ResMut<UiScale>, size: Res<kit::ui_kit::UiSize>, mut map: ResMut<VoyageMap>) {
    s.0 = size.0;
    map.warp = None;
}

/// The map opens and closes with its transition (`ui_kit::ScreenFx`: the panel slides in from the right, the map's
/// words fade in, the shipyard fades out under it) once the portal is far enough in, and closes first on the way
/// out; to flight (SET COURSE) it snaps shut.
fn show(mode: Res<State<AppMode>>, portal: Res<Portal>, mut transitions: MessageReader<StateTransitionEvent<AppMode>>, mut root: Single<&mut ScreenFx, With<MapRoot>>) {
    let here = *mode.get() == AppMode::Map && portal.screen_open();
    if !here && transitions.read().last().is_some_and(|t| t.entered == Some(AppMode::Flight)) {
        snap_screen(&mut root, false);
    } else {
        show_screen(&mut root, here);
    }
}

/// The map's drawing (world meshes) shows and fades with the portal and the screen, each mesh also with its layer of
/// the view; the chrome (the legend and the panel's backing) only with the screen, so it doesn't show through the
/// opening before the panel slides in.
fn fade_mesh(root: Single<&ScreenFx, With<MapRoot>>, portal: Res<Portal>, view: Res<MapView>, mut materials: ResMut<Assets<AtlasMaterial>>, mut meshes: Query<(&MapMesh, &mut Visibility)>) {
    for (m, mut vis) in &mut meshes {
        let (base, shows) = match m.sheet {
            Sheet::Chrome => (root.alpha(), root.visible()),
            _ => (root.alpha().max(portal.map_alpha()), root.visible() || portal.shows()),
        };
        let alpha = base * m.sheet.layer().alpha(view.zoom);
        set_alpha(&mut materials, &m.material, alpha);
        let want = if shows && alpha > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
}

/// Whether the map shows (open, opening or closing).
fn shown(root: Single<&ScreenFx, With<MapRoot>>, portal: Res<Portal>) -> bool {
    root.visible() || portal.shows()
}

#[allow(clippy::too_many_arguments)]
fn spawn(
    mut commands: Commands,
    fonts: Res<HudFonts>,
    style: Res<Style>,
    content: Res<GameContent>,
    career: Res<PlayerCareer>,
    geo: Res<MapGeometry>,
    atlas: Res<Atlas>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<AtlasMaterial>>,
) {
    for sheet in Sheet::ALL {
        // Each is built by `place_mesh` (the chrome once the panel's place is measured).
        let (mesh, built) = (MeshBuf::placeholder(), None);
        let mesh = meshes.add(mesh);
        let material = atlas.group_material(&mut materials);
        commands.spawn((
            MapMesh { sheet, mesh: mesh.clone(), material: material.clone(), built },
            Mesh2d(mesh),
            MeshMaterial2d(material),
            Transform::from_xyz(0.0, 0.0, sheet.z()),
            Visibility::Hidden,
        ));
    }
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let fonts = &*fonts;
    let content = &content.0;
    let career = career.career();
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
    commands.spawn((MapRoot, CompactText(COMPACT_TEXT), ScreenFx::closed().covering(), root, Visibility::Hidden)).with_children(|root| {
        root.spawn((Stage, Node { width: px(STAGE.x), height: px(STAGE.y), flex_shrink: 0.0, ..default() })).with_children(|stage| {
            // ---- The map (everything left of the panel) ----
            // It fades in place: the map's drawing is placed from the stage. Labels moved past its edges by the view
            // are cut there (and can't be clicked): the map area the UI leaves free (`fit` measures it: left of the
            // panel, below the header, above the legend, out to the window's edge, where the galaxy is drawn too in
            // a window wider or taller than the stage); the top band names in the strip right of the header.
            let clip = Node { position_type: PositionType::Absolute, left: px(0), top: px(0), width: px(PANEL_X), height: px(STAGE.y), overflow: Overflow::clip(), ..default() };
            let frame = Node { position_type: PositionType::Absolute, left: px(0), top: px(0), width: px(PANEL_X), height: px(STAGE.y), ..default() };
            let b = &geo.band;
            stage.spawn((ScreenPart::new(Edge::Fade, 0), MapClip(true), clip.clone())).with_children(|clip| {
                clip.spawn((MapFrame, frame.clone())).with_children(|m| {
                    // The band names along the top: home's band between its edges, the next one past the gate (they
                    // slide along the line with the arcs, `view::MapView::top_band_x`).
                    let top = MapView::new(b.centre, b.outer).top_band_x(b);
                    let centred = |x: f32| Node { position_type: PositionType::Absolute, left: px(x), top: px(TOP_Y - 5.0), width: px(0), justify_content: JustifyContent::Center, ..default() };
                    // The next one's name says whether it is open (`sync_map`).
                    if let Some(x) = top[0] {
                        m.spawn((Anchored::TopBand(0, x), UiTransform::default(), centred(x))).with_child((label(fonts, &b.key.replace('_', " ").to_uppercase(), 8.5, c(Tone::Label, 0.8)), words));
                    }
                    if let (Some(x), Some(next)) = (top[1], b.next_key.as_deref()) {
                        let node = Node { align_items: AlignItems::Center, ..column(2.0) };
                        m.spawn((Anchored::TopBand(1, x), UiTransform::default(), centred(x))).with_children(|l| {
                            l.spawn(node).with_children(|l| {
                                l.spawn((MapText::TopNext, label(fonts, &next.replace('_', " ").to_uppercase(), 8.5, c(Tone::Coral, 0.8)), words));
                                l.spawn((MapText::TopNextState, label(fonts, "", 6.5, c(Tone::Coral, 0.7)), words));
                            });
                        });
                    }
                });
            });
            stage.spawn((ScreenPart::new(Edge::Fade, 0), MapClip(false), clip)).with_children(|clip| { clip.spawn((MapFrame, frame)).with_children(|m| {
                // Under everything else: the galaxy's band names (seen zoomed out), halfway across each band: home's
                // and the next above the centre, the narrower deeper ones below it; under each its state (`sync_map`:
                // `CHARTED · OPEN`, `UNCHARTED · OPEN`, `SEALED · BEAT THE WARDEN …`, `SEALED`).
                let whole = MapView::new(b.centre, b.outer).whole();
                for (k, ring) in geo.rings.iter().enumerate() {
                    let up = if k < 2 { -1.0 } else { 1.0 };
                    let at = b.centre + Vec2::new(0.0, up * (ring.outer + ring.inner) / 2.0);
                    let laid = whole.to_screen(at);
                    let node = Node { position_type: PositionType::Absolute, left: px(laid.x), top: px(laid.y - 11.0), width: px(0), align_items: AlignItems::Center, ..column(3.0) };
                    let tone = if ring.charted { Tone::Teal } else { Tone::Label };
                    m.spawn((Anchored::Far(at, laid), UiTransform::default(), Visibility::Hidden, node)).with_children(|l| {
                        l.spawn((label(fonts, &ring.key.replace('_', " ").to_uppercase(), if k == 0 { 10.5 } else { 9.0 }, c(tone, 0.9)), words));
                        l.spawn((MapText::BandState(k), label(fonts, "", 7.0, c(Tone::Dim, 1.0)), words));
                    });
                }
                // The signals' labels, over their areas' tops (placed by `sync_map`): `SIGNAL · A GATE`, the source's
                // line, the state; a click area.
                for (j, _) in geo.signals.iter().enumerate() {
                    let node = Node { position_type: PositionType::Absolute, left: px(0), top: px(0), width: px(0), align_items: AlignItems::Center, display: Display::None, ..column(3.0) };
                    m.spawn((MapNode::Signal(j), Anchored::At(Vec2::ZERO, Layer::Always), UiTransform::default(), node)).with_children(|l| {
                        l.spawn((MapButton::Signal(j), Hot::default(), Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(3), padding: UiRect::axes(px(6), px(2)), ..default() }))
                            .with_children(|t| {
                                t.spawn((MapText::SignalHead(j), label(fonts, "", 9.0, c(Tone::Gold, 1.0)), words));
                                t.spawn((MapText::SignalLine(j), prose(fonts, "", 11.5, c(Tone::Number, 0.85)), words));
                                t.spawn((MapText::SignalState(j), label(fonts, "", 7.5, c(Tone::Gold, 0.85)), words));
                            });
                    });
                }
                // The shipyard's label, left of it.
                let h = geo.home;
                let node = Node { position_type: PositionType::Absolute, right: px(PANEL_X - h.x + 22.0), top: px(h.y - 10.0), align_items: AlignItems::FlexEnd, ..column(4.0) };
                m.spawn((Anchored::At(h, Layer::Always), UiTransform::default(), node))
                    .with_children(|l| {
                        l.spawn((label(fonts, "SHIPYARD", 10.5, c(Tone::Number, 0.9)), words));
                        l.spawn((label(fonts, "HOME  ·  DOCKED", 8.5, c(Tone::Label, 1.0)), words));
                    });
                // A label beside `at`: on its right, or its left near the map's right edge.
                let beside = |at: Vec2| {
                    let left = geo.label_left(at);
                    let mut node = Node { position_type: PositionType::Absolute, top: px(at.y - 10.0), align_items: if left { AlignItems::FlexEnd } else { AlignItems::FlexStart }, ..column(4.0) };
                    if left {
                        node.right = px(PANEL_X - at.x + 24.0);
                    } else {
                        node.left = px(at.x + 24.0);
                    }
                    node
                };
                let marker = |at: Vec2, r: f32| Node { position_type: PositionType::Absolute, left: px(at.x - r), top: px(at.y - r), width: px(2.0 * r), height: px(2.0 * r), ..default() };
                // The known clusters (charted, located, gates): a click area on each marker and its label, hidden until
                // the map knows it.
                for (k, &at) in (0u32..).zip(&geo.clusters) {
                    m.spawn((MapButton::Cluster(k), MapNode::Cluster(k), Hot::default(), Anchored::At(at, Layer::Always), UiTransform::default(), Node { display: Display::None, ..marker(at, 16.0) }));
                    m.spawn((MapButton::Cluster(k), MapNode::Cluster(k), Hot::default(), Anchored::At(at, Layer::Detail), UiTransform::default(), Node { display: Display::None, ..beside(at) }))
                        .with_children(|l| {
                            let name = career.exploration().and_then(|g| g.skeleton().clusters.get(k as usize)).map_or("", |c| c.name.as_str());
                            l.spawn((label(fonts, &name.to_uppercase(), 9.5, c(Tone::Number, 0.9)), words));
                            l.spawn((MapText::ClusterState(k), label(fonts, "", 7.5, c(Tone::Teal, 1.0)), words));
                        });
                }
                // The voyages: a click area on each marker (on its beacon zoomed out), and its label (a click area
                // too, faded out with the detail); both hidden until the voyage is known.
                for (k, v) in content.voyages.iter().enumerate() {
                    let at = geo.nodes[k];
                    m.spawn((MapButton::Voyage(k), MapNode::Voyage(k), Hot::default(), Anchored::At(at, Layer::Always), UiTransform::default(), marker(at, 22.0)));
                    m.spawn((MapButton::Voyage(k), MapNode::Voyage(k), Hot::default(), Anchored::At(at, Layer::Detail), UiTransform::default(), beside(at))).with_children(|l| {
                        l.spawn((MapText::Name(k), label(fonts, &v.name.to_uppercase(), 10.5, c(Tone::Number, 1.0)), words));
                        let tag = Node { border: UiRect::all(px(1)), border_radius: BorderRadius::MAX, ..default() };
                        l.spawn((MapNode::Tag(k), tag, BorderColor::all(Color::NONE)))
                            .with_child((MapText::State(k), label(fonts, "", 8.5, c(Tone::Teal, 1.0)), words));
                        l.spawn(Node { margin: UiRect::top(px(2)), ..row(4.0) }).with_children(|d| {
                            for s in site_dots(content, v).into_iter().take(MAX_DOTS) {
                                let mut n = d.spawn((dot(6.0), BackgroundColor(c(s.tone(), 1.0))));
                                if s.diamond() {
                                    n.insert((Node { border_radius: BorderRadius::ZERO, ..dot(5.0) }, UiTransform::from_rotation(Rot2::degrees(45.0))));
                                }
                            }
                        });
                    });
                }
            }); });
            // The legend (left out on a phone), under the map area.
            let legend = Node { position_type: PositionType::Absolute, left: px(0), top: px(LEGEND_TOP), width: px(PANEL_X), height: px(STAGE.y - LEGEND_TOP), ..default() };
            stage.spawn((ScreenPart::new(Edge::Fade, 0), Chrome::Legend, legend)).with_children(|m| {
                for (r, names) in LEGEND.iter().enumerate() {
                    for (k, name) in names.iter().enumerate() {
                        let mut l = m.spawn((label(fonts, name, 8.0, c(Tone::Label, 1.0)), words, abs(LEGEND_COLUMNS[k] + LEGEND_GAP, LEGEND_ROWS[r] - 5.5 - LEGEND_TOP)));
                        if (r, k) == (1, names.len() - 1) {
                            l.insert(MapText::Reach);
                        }
                    }
                }
            });
            // The view's buttons: bottom right of the map, in the legend's rows (as the drydock's zoom); on a phone top
            // right of the map, beside the header (`fit`).
            let at = Node { position_type: PositionType::Absolute, right: px(STAGE.x - PANEL_X + 24.0), top: px((LEGEND_ROWS[0] + LEGEND_ROWS[1]) / 2.0 - 14.0), ..row(10.0) };
            stage.spawn((ScreenPart::new(Edge::Fade, 0), UiBlock, Chrome::Zoom, at)).with_children(|z| {
                z.spawn((ring_node(28.0), MapButton::View(ViewButton::ZoomOut), Tip::new("ZOOM OUT"), ActionTip(Action::ZoomOut))).with_child(ring_glyph(Glyph::Minus, 13.0));
                z.spawn((ZoomText, number(fonts, "100 %", 13.0, c(Tone::Number, 1.0)), Node { min_width: px(46), justify_content: JustifyContent::Center, ..default() }));
                z.spawn((ring_node(28.0), MapButton::View(ViewButton::ZoomIn), Tip::new("ZOOM IN"), ActionTip(Action::ZoomIn))).with_child(ring_glyph(Glyph::Plus, 13.0));
                z.spawn((ring_node(28.0), MapButton::View(ViewButton::Reset), Tip::new("RESET VIEW"), ActionTip(Action::ResetView))).with_child(ring_glyph(Glyph::Fit, 13.0));
            });

            // ---- The panel: the selected voyage, the fleet, SET COURSE ----
            stage
                .spawn((ScreenPart::new(Edge::Right, 1), Chrome::Panel, Node {
                    position_type: PositionType::Absolute,
                    left: px(PANEL_X),
                    top: px(0),
                    width: px(PANEL_W),
                    height: px(STAGE.y),
                    padding: UiRect::new(px(PAD), px(PAD), px(24), px(22)),
                    ..column(8.0)
                }))
                .with_children(|panel| {
                    // Its rows (they scroll on a phone, with no glow room past their edges: they'd show under the
                    // status), then the status and SET COURSE at the bottom.
                    let body = Node { flex_grow: 1.0, flex_shrink: 1.0, min_height: px(0), overflow: Overflow::scroll_y(), ..column(8.0) };
                    panel.spawn((ScrollPosition::default(), Scrollbar::vertical().room(0), body)).with_children(|p| {
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
                    p.spawn((MapText::SitesHead, MapNode::SitesHead, label(fonts, "", 9.0, c(Tone::Label, 1.0)), Node { margin: UiRect::top(px(4)), ..default() }));
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
                                    cell.spawn((MapNode::Mark(k), BackgroundColor(Color::NONE), BorderColor::all(Color::NONE), Node { border: UiRect::all(px(1.2)), ..dot(6.0) }));
                                    cell.spawn((MapText::Check(k), tracked(fonts, "", 8.0, c(Tone::Label, 1.0), 0.18)));
                                });
                            }
                        });
                    }
                    });
                    panel.spawn((MapText::Status, label(fonts, "", 9.0, c(Tone::Teal, 1.0)), Node { margin: UiRect::bottom(px(4)), ..default() }));
                    let play = UiIcon::Glyph { glyph: Glyph::Play, color: ui.teal };
                    spawn_pill(panel, fonts, "SET COURSE", 12.0, 46.0, Some(play), Pill { primary: true, ..Pill::new(Tone::Teal) }, (MapButton::SetCourse, Tip::key("SET COURSE", "ENTER")));
                });
        });
        // Top left, at the window's corner (the frame's): the way back and where we are.
        let f = frame();
        root.spawn((Chrome::Header, f.top_left())).with_children(|tl| {
            spawn_back(tl, &ui, (MapButton::Back, Tip::key("SHIPYARD", "ESC")));
            tl.spawn(f.heading()).with_children(|col| {
                col.spawn((MapText::Header, crumb(fonts, "", &ui)));
                col.spawn(title(fonts, "Voyages", &ui));
            });
        });
    });
}

/// The stage fits the window (on a phone it is laid out at the phone's scale from the window's top left, the panel
/// at the window's right edge, the legend left out and the zoom buttons beside the header); the map area is what the
/// laid-out chrome leaves free: from the window's left edge to the panel, from the header (and on a phone the zoom
/// buttons) down to the legend (on a phone the window's bottom). The labels are cut there (their frames staying on
/// the stage), the view keeps it covered and the detail fades out at its edges (Niklas, 19 Oct 2026: on a phone the
/// clusters' labels and the shipyard ran over the back ring and the legend). The camera rig knows the window (the
/// meshes and the icons are placed through it).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn fit(
    window: Single<&Window, With<PrimaryWindow>>,
    size: Res<kit::ui_kit::UiSize>,
    class: Res<LayoutClass>,
    geo: Res<MapGeometry>,
    mut ui_scale: ResMut<UiScale>,
    mut rig: ResMut<CameraRig>,
    mut view: ResMut<MapView>,
    mut root: Single<&mut Node, (With<MapRoot>, Without<Chrome>, Without<MapClip>, Without<MapFrame>)>,
    stage: Single<(&ComputedNode, &UiGlobalTransform), With<Stage>>,
    mut chrome: Query<(&Chrome, &mut Node, &ComputedNode, &UiGlobalTransform), (Without<MapRoot>, Without<MapClip>, Without<MapFrame>)>,
    mut clips: Query<(&MapClip, &mut Node, &Children), (Without<MapRoot>, Without<Chrome>, Without<MapFrame>)>,
    mut frames: Query<&mut Node, (With<MapFrame>, Without<MapRoot>, Without<Chrome>, Without<MapClip>)>,
) {
    let compact = class.is_compact();
    let s = if compact { compact_scale(window.size()) } else { stage_scale(window.size()) } * size.0;
    if (ui_scale.0 - s).abs() > 1e-3 {
        ui_scale.0 = s;
    }
    if rig.viewport != window.size() || rig.view_offset != Vec2::ZERO {
        rig.viewport = window.size();
        rig.view_offset = Vec2::ZERO;
    }
    let align = if compact { AlignItems::FlexStart } else { AlignItems::Center };
    let justify = if compact { JustifyContent::FlexStart } else { JustifyContent::Center };
    if root.align_items != align || root.justify_content != justify {
        root.align_items = align;
        root.justify_content = justify;
    }
    // Physical px → stage px, from the stage's layout.
    let (node, tf) = *stage;
    let k = node.size.x / STAGE.x;
    if k <= 0.0 {
        return;
    }
    let corner = tf.translation - node.size / 2.0;
    let to_stage = |q: Vec2| (q - corner) / k;
    let laid = |n: &ComputedNode, t: &UiGlobalTransform| Rect::from_center_size(to_stage(t.translation), n.size / k);
    let win = Rect::from_corners(to_stage(Vec2::ZERO), to_stage(window.physical_size().as_vec2()));

    // The chrome's places.
    let f = frame();
    let mut measured = [None; 4];
    for (c, mut n, cn, t) in &mut chrome {
        let mut want = n.clone();
        match (c, compact) {
            (Chrome::Panel, false) => {
                (want.left, want.top, want.width, want.height) = (px(PANEL_X), px(0), px(PANEL_W), px(STAGE.y));
                want.padding = UiRect::new(px(PAD), px(PAD), px(24), px(22));
            }
            (Chrome::Panel, true) => {
                let x = (win.max.x - COMPACT_PANEL_W).round();
                (want.left, want.top, want.width, want.height) = (px(x), px(win.min.y.round()), px(COMPACT_PANEL_W), px(win.height().round()));
                want.padding = UiRect::axes(px(COMPACT_PAD.0), px(COMPACT_PAD.1));
            }
            (Chrome::Legend, _) => want.display = if compact { Display::None } else { Display::Flex },
            (Chrome::Zoom, false) => {
                want.right = px(STAGE.x - PANEL_X + 24.0);
                want.top = px((LEGEND_ROWS[0] + LEGEND_ROWS[1]) / 2.0 - 14.0);
            }
            (Chrome::Zoom, true) => {
                want.right = px((STAGE.x - (win.max.x - COMPACT_PANEL_W - COMPACT_ZOOM_GAP)).round());
                want.top = px((win.min.y + f.top + (f.ring - 28.0) / 2.0).round());
            }
            (Chrome::Header, _) => {}
        }
        if *n != want {
            *n = want;
        }
        let shown = n.display != Display::None && cn.size.cmpgt(Vec2::ZERO).all();
        measured[*c as usize] = shown.then(|| laid(cn, t));
    }
    let [Some(header), Some(panel), legend, zoom] = measured else { return };

    // The map area the chrome leaves free, and the top band names' strip right of the header.
    let top = if compact { zoom.map_or(header.max.y, |z| header.max.y.max(z.max.y)) } else { header.max.y };
    let bottom = legend.map_or(win.max.y, |l| l.min.y);
    let area = Rect::new(win.min.x, top, panel.min.x, bottom.max(top + 1.0));
    let strip_right = zoom.filter(|_| compact).map_or(area.max.x, |z| z.min.x);
    let strip = Rect::new(header.max.x, win.min.y, strip_right.max(header.max.x + 1.0), top);
    for (clip, mut n, children) in &mut clips {
        let r = if clip.0 { strip } else { area };
        let (l, t, w, h) = (px(r.min.x), px(r.min.y), px(r.width()), px(r.height()));
        if n.left != l || n.top != t || n.width != w || n.height != h {
            (n.left, n.top, n.width, n.height) = (l, t, w, h);
        }
        for c in children {
            if let Ok(mut fr) = frames.get_mut(*c)
                && (fr.left != px(-r.min.x) || fr.top != px(-r.min.y))
            {
                fr.left = px(-r.min.x);
                fr.top = px(-r.min.y);
            }
        }
    }
    // On a phone the default view brings home a third into the smaller area, halfway down.
    let home = if compact { (area.min + area.size() * Vec2::new(0.3, 0.5) - geo.0.home).round() } else { Vec2::ZERO };
    let top = if compact { (strip.min.x + 30.0, strip.max.x) } else { (view::HEADER_END, area.max.x) };
    view.set_frame(area, home, top);
}

/// What a launch of the selection reads.
#[derive(bevy::ecs::system::SystemParam)]
struct FleetNow<'w> {
    content: Res<'w, GameContent>,
    career: Res<'w, PlayerCareer>,
    granted: Res<'w, Granted>,
    design: Res<'w, PlayerDesign>,
    escorts: Res<'w, EscortDesigns>,
    known: Res<'w, MapKnown>,
}

impl FleetNow<'_> {
    /// Why the selection can't launch now, if it can't.
    fn blocker(&self, pick: &Pick) -> Option<String> {
        let c = &self.content.0;
        let fleet = fleet_blocker(c, &self.granted.unlocks, self.granted.mind_capacity, self.granted.launch_stock(), self.design.blueprint(), &self.escorts.0);
        let career = self.career.career();
        match pick {
            Pick::Voyage(id) => course_blocker(c, career, id, fleet),
            _ => jump_blocker(c, career, &self.known.0, pick, fleet),
        }
    }
}

/// The places the keys step through: the known voyages in content order, then the known clusters.
fn places(content: &data::Content, career: &data::Career, known: &Known) -> Vec<Pick> {
    let voyages = content.voyages.iter().filter(|v| VoyageState::of(content, career, &v.id).known()).map(|v| Pick::Voyage(v.id.clone()));
    voyages.chain(known.clusters.iter().map(|m| Pick::Cluster(m.cluster))).collect()
}

/// Where a pick sits on the map (galaxy px), if it does.
fn pick_at(geo: &Geometry, content: &data::Content, known: &Known, pick: &Pick) -> Option<Vec2> {
    match pick {
        Pick::Voyage(id) => content.voyages.position(id).map(|k| geo.nodes[k]),
        Pick::Cluster(k) => geo.clusters.get(*k as usize).copied(),
        Pick::Signal(s) => known.signal(*s).map(|m| geo.to_galaxy(m.area.centre)),
        Pick::Spot(p) => Some(geo.to_galaxy(*p)),
    }
}

/// Clicks, taps and keys: select, pick a spot, launch (a jump after its warp), go home.
#[allow(clippy::too_many_arguments)]
fn input(
    mut clicks: MessageReader<UiClick>,
    buttons: Query<&MapButton>,
    keys: Res<ButtonInput<KeyCode>>,
    act: world::settings::Act,
    pointer: Res<UiPointer>,
    time: Res<Time<Real>>,
    fleet: FleetNow,
    geo: Res<MapGeometry>,
    mut view: ResMut<MapView>,
    mut map: ResMut<VoyageMap>,
    mut portal: ResMut<Portal>,
    mut start: MessageWriter<StartVoyage>,
) {
    let tap = view.tap.take();
    if !portal.is_open() {
        clicks.clear();
        return;
    }
    let content = &fleet.content.0;
    let career = fleet.career.career();
    let known = &fleet.known.0;
    let geo = &geo.0;
    // While warping, only Esc (it ends the warp) and the way home.
    if map.warp.is_some() {
        if keys.just_pressed(KeyCode::Escape) && !pointer.typing {
            map.warp = None;
            map.say = Some("WARP ENDED  ·  NOTHING LAUNCHED".into());
        }
        for c in clicks.read() {
            if let Ok(MapButton::Back) = buttons.get(c.0) {
                map.warp = None;
                portal.leave();
            }
        }
        return;
    }
    let select = |map: &mut VoyageMap, pick: Pick| {
        if map.selected.as_ref() != Some(&pick) {
            map.selected = Some(pick);
            map.say = None;
        }
    };
    let mut launch = false;
    for c in clicks.read() {
        match buttons.get(c.0) {
            Ok(MapButton::Back) => portal.leave(),
            Ok(MapButton::Voyage(k)) => {
                if let Some(v) = content.voyages.iter().nth(*k) {
                    select(&mut map, Pick::Voyage(v.id.clone()));
                }
                // Zoomed out past the labels: a click on a beacon also zooms in to it.
                if view::detail_alpha(view.target) < 0.5 {
                    view.zoom_to(geo.nodes[*k]);
                }
            }
            Ok(MapButton::Cluster(k)) => select(&mut map, Pick::Cluster(*k)),
            Ok(MapButton::Signal(j)) => {
                if let Some(s) = geo.signals.get(*j) {
                    select(&mut map, Pick::Signal(*s));
                }
            }
            Ok(MapButton::SetCourse) => launch = true,
            Ok(MapButton::View(_)) | Err(_) => {}
        }
    }
    // A click on empty space: a blind jump's spot, or the known cluster a jump there would go to.
    if let Some(q) = tap.filter(|q| view.area().contains(*q)) {
        let p = geo.to_point(view.to_galaxy(q));
        let pick = match known::in_reach(career, content, p).then(|| career.blind_target(content, p)).flatten() {
            Some(k) if known.cluster(k).is_some() => Pick::Cluster(k),
            _ => Pick::Spot(p),
        };
        select(&mut map, pick);
    }
    let places = places(content, career, known);
    let n = places.len();
    if !pointer.typing && n > 0 {
        if keys.just_pressed(KeyCode::Escape) {
            portal.leave();
        }
        if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
            launch = true;
        }
        let step = if act.just_pressed(Action::NextVoyage) {
            1
        } else if act.just_pressed(Action::PreviousVoyage) {
            n - 1
        } else {
            0
        };
        if step > 0 {
            let at = map.selected.as_ref().and_then(|s| places.iter().position(|p| p == s));
            let pick = places[at.map_or(0, |k| (k + step) % n)].clone();
            if let Some(g) = pick_at(geo, content, known, &pick) {
                view.show(g);
            }
            select(&mut map, pick);
        }
    }
    // Only once the screen is open: SET COURSE shows from then on, and Enter mid-dive would leave the portal half way.
    if launch && portal.screen_open() && let Some(pick) = map.selected.clone() {
        match fleet.blocker(&pick) {
            Some(why) => map.say = Some(why),
            None => {
                map.say = None;
                match (&pick, pick.target(content, career, known)) {
                    (Pick::Voyage(id), _) => {
                        start.write(StartVoyage { to: Destination::Voyage(id.clone()) });
                    }
                    (_, Some(k)) => {
                        let from = career.jump_from(content, k).and_then(|f| geo.clusters.get(f as usize)).copied().unwrap_or(geo.home);
                        let seconds = career.warp_seconds(content, k).unwrap_or(0.0);
                        map.warp = Some(Warp { target: k, from, to: geo.clusters[k as usize], start: time.elapsed_secs(), seconds });
                    }
                    (_, None) => {}
                }
            }
        }
    }
}

/// A warp that has run its time launches.
fn warp(time: Res<Time<Real>>, mut map: ResMut<VoyageMap>, mut start: MessageWriter<StartVoyage>) {
    if let Some(w) = map.warp
        && time.elapsed_secs() - w.start >= w.seconds
    {
        map.warp = None;
        start.write(StartVoyage { to: Destination::Cluster(w.target) });
    }
}

/// A launch `voyage.rs` refused: the panel says why.
fn refused(mut refusals: MessageReader<LaunchRefused>, mut map: ResMut<VoyageMap>) {
    if let Some(LaunchRefused(why)) = refusals.read().last() {
        map.say = Some(why.clone());
    }
}

/// A signal's area that changed (narrowed, located) leaves its old edge fading out; edges done fading go.
fn track_areas(time: Res<Time<Real>>, known: Res<MapKnown>, mut map: ResMut<VoyageMap>) {
    let now = time.elapsed_secs();
    let fresh = map.fading.iter().all(|f| now - f.1 < FADE);
    if known.is_changed() {
        let areas = areas_now(&known.0);
        let old: Vec<Area> = map.seen.iter().filter(|(s, a)| a.radius > 0 && areas.get(s) != Some(a)).map(|(_, a)| *a).collect();
        if !old.is_empty() || areas != map.seen {
            let m = map.as_mut();
            m.fading.extend(old.into_iter().map(|a| (a, now)));
            m.seen = areas;
        }
    }
    if !fresh {
        map.fading.retain(|f| now - f.1 < FADE);
    }
}

/// The map's words: the header, each voyage's name and state (a signal's in its tag), the known clusters' states, the
/// signals' labels over their areas, the jump range's; each shows only while the map knows it.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync_map(
    map: Res<VoyageMap>,
    career: Res<PlayerCareer>,
    content: Res<GameContent>,
    known: Res<MapKnown>,
    geo: Res<MapGeometry>,
    style: Res<Style>,
    hots: Query<(&MapButton, Ref<Hot>)>,
    mut texts: Query<(&MapText, &mut Text, &mut TextColor)>,
    mut nodes: Query<(&MapNode, &mut Node, Option<&mut BorderColor>, Option<&mut Anchored>)>,
) {
    let hover = hots.iter().any(|(_, h)| h.is_changed());
    if !(hover || map.is_changed() || career.is_changed() || known.is_changed()) {
        return;
    }
    let content = &content.0;
    let (known, geo) = (&known.0, &geo.0);
    let ui = &style.ui;
    let states = VoyageState::all(content, career.career());
    let selected = |p: Pick| map.selected.as_ref() == Some(&p);
    let hovered = |b: MapButton| hots.iter().any(|(x, h)| *x == b && h.hovered);
    let mark = |j: usize| geo.signals.get(j).and_then(|s| known.signal(*s)).filter(|m| m.is_area());
    let spots = signal_labels(geo, known, content, &states);
    for (t, mut text, mut color) in &mut texts {
        let (words, tone, alpha) = match *t {
            MapText::Header => {
                let cleared = states.iter().filter(|s| matches!(s, VoyageState::Cleared { .. })).count();
                let charted = career.career().exploration().map_or(0, |g| g.charted().len().saturating_sub(1));
                let signals = known.signals.len() + states.iter().filter(|s| **s == VoyageState::Signal).count();
                let band = geo.band.key.replace('_', " ").to_uppercase();
                let mut words = format!("{band}  ·  {charted} CHARTED  ·  {cleared} CLEARED");
                if signals > 0 {
                    words.push_str(&format!("  ·  {signals} {}", if signals == 1 { "SIGNAL" } else { "SIGNALS" }));
                }
                (words, Tone::Label, 1.0)
            }
            MapText::Reach => {
                let r = format!("{:.2}", known.reach);
                (format!("JUMP RANGE  ·  {} S", r.trim_end_matches('0').trim_end_matches('.')), Tone::Label, 1.0)
            }
            MapText::Name(k) => {
                let lit = content.voyages.iter().nth(k).is_some_and(|v| selected(Pick::Voyage(v.id.clone()))) || hovered(MapButton::Voyage(k));
                (text.0.clone(), Tone::Number, if lit { 1.0 } else { 0.8 })
            }
            MapText::State(k) => {
                let v = content.voyages.iter().nth(k);
                // A voyage on a gate it opened (the Warden's, galaxy plan E7) says so, as an opened gate does.
                let g = career.career().exploration();
                if v.and_then(|v| g?.pins().get(&v.id)).is_some_and(|c| g.is_some_and(|g| g.gates().contains(c))) {
                    ("GATE  ·  OPEN".into(), Tone::Coral, 1.0)
                } else {
                    let (w, tone) = state_words(states[k], v.is_some_and(|v| is_guardian(content, v)));
                    (w, tone, 1.0)
                }
            }
            MapText::ClusterState(k) => match known.cluster(k) {
                Some(m) if m.gate => ("GATE  ·  OPEN".into(), Tone::Coral, 1.0),
                Some(m) if !m.signals.is_empty() => {
                    let s = &known.signals[m.signals[0]];
                    (format!("{}  ·  {}", s.head(), s.state_word()), Tone::Gold, 1.0)
                }
                Some(_) => ("CHARTED".into(), Tone::Teal, 1.0),
                None => continue,
            },
            MapText::SignalHead(j) => match mark(j) {
                Some(m) => (m.head(), Tone::Gold, if selected(Pick::Signal(m.signal)) || hovered(MapButton::Signal(j)) { 1.0 } else { 0.85 }),
                None => continue,
            },
            MapText::SignalLine(j) => match mark(j) {
                Some(m) => (m.line.clone(), Tone::Number, 0.8),
                None => continue,
            },
            MapText::SignalState(j) => match mark(j) {
                Some(m) => (m.state_word().to_string(), Tone::Gold, 0.85),
                None => continue,
            },
            MapText::BandState(k) => band_state(geo, known, k),
            MapText::TopNext | MapText::TopNextState => {
                let k = geo.rings.iter().position(|r| r.charted).unwrap_or(0) + 1;
                let open = known.bands.get(k).is_some_and(|m| m.open);
                let tone = if open { Tone::Teal } else { Tone::Coral };
                match *t {
                    MapText::TopNext => (text.0.clone(), tone, 0.8),
                    _ => ((if open { "OPEN" } else { "SEALED" }).into(), tone, 0.7),
                }
            }
            _ => continue,
        };
        set_text(&mut text, &words);
        set_color(&mut color, ui_color(tone.color(ui), alpha));
    }
    for (n, mut node, border, anchored) in &mut nodes {
        let show = match *n {
            MapNode::Voyage(k) => states[k].known(),
            MapNode::Tag(k) => {
                let signal = states[k] == VoyageState::Signal;
                let pad = if signal { UiRect::axes(px(6), px(2)) } else { UiRect::ZERO };
                if node.padding != pad {
                    node.padding = pad;
                }
                let want = BorderColor::all(if signal { ui_color(ui.teal, 0.7) } else { Color::NONE });
                if let Some(mut b) = border
                    && *b != want
                {
                    *b = want;
                }
                true
            }
            MapNode::Cluster(k) => known.cluster(k).is_some(),
            MapNode::Signal(j) => match mark(j).and_then(|m| spots.get(&m.signal)) {
                Some(&(g, at)) => {
                    place(&mut node, anchored, g, at);
                    true
                }
                None => false,
            },
            _ => continue,
        };
        let d = if show { Display::Flex } else { Display::None };
        if node.display != d {
            node.display = d;
        }
    }
}

/// Where each signal's label goes (galaxy px at the default view): the point of its area's edge it follows, and the
/// label's top centre. Above the area, else below, right or left of it, else inside it in its middle, the first that
/// stays on the map (below the header, above the legend) and keeps clear of the labels and markers of home, the
/// voyages, the clusters and the signals placed before it (widths estimated from the words).
fn signal_labels(geo: &Geometry, known: &Known, content: &data::Content, states: &[VoyageState]) -> BTreeMap<TrailSignal, (Vec2, Vec2)> {
    let rect = |min: Vec2, size: Vec2| Rect::from_corners(min, min + size);
    let beside = |at: Vec2, w: f32, h: f32| if geo.label_left(at) { rect(Vec2::new(at.x - 24.0 - w, at.y - 10.0), Vec2::new(w, h)) } else { rect(Vec2::new(at.x + 24.0, at.y - 10.0), Vec2::new(w, h)) };
    let mut taken: Vec<Rect> = vec![rect(Vec2::new(geo.home.x - 132.0, geo.home.y - 14.0), Vec2::new(150.0, 30.0))];
    for (k, v) in content.voyages.iter().enumerate().filter(|(k, _)| states.get(*k).is_some_and(|s| s.known())) {
        let at = geo.nodes[k];
        taken.push(rect(at - 14.0, Vec2::splat(28.0)));
        taken.push(beside(at, v.name.len() as f32 * 9.5, 44.0));
    }
    for m in &known.clusters {
        let at = geo.clusters[m.cluster as usize];
        taken.push(rect(at - 10.0, Vec2::splat(20.0)));
        taken.push(beside(at, m.name.len() as f32 * 9.0, 30.0));
    }
    let mut out = BTreeMap::new();
    for m in known.signals.iter().filter(|m| m.is_area()) {
        let (c, r) = (geo.to_galaxy(m.area.centre), geo.px(m.area.radius));
        let (w, h) = ((m.head().chars().count() as f32 * 8.3).max(m.line.chars().count() as f32 * 6.0) + 12.0, SIGNAL_LABEL);
        // (the edge point followed, the label's top centre)
        let tries = [
            (c - Vec2::Y * r, c - Vec2::new(0.0, r + h + 4.0)),
            (c + Vec2::Y * r, c + Vec2::new(0.0, r + 6.0)),
            (c + Vec2::X * r, c + Vec2::new(r + 8.0 + w / 2.0, -h / 2.0)),
            (c - Vec2::X * r, c - Vec2::new(r + 8.0 + w / 2.0, h / 2.0)),
            (c, c - Vec2::new(0.0, h / 2.0)),
        ];
        let fits = |top: Vec2| {
            let b = rect(top - Vec2::new(w / 2.0, 0.0), Vec2::new(w, h));
            let on_map = b.min.x > 0.0 && b.max.x < PANEL_X && b.min.y > view::VIEW_AREA.min.y && b.max.y < view::VIEW_AREA.max.y;
            on_map && taken.iter().all(|t| t.intersect(b).is_empty())
        };
        let (g, top) = tries.iter().copied().find(|(_, top)| fits(*top)).unwrap_or(tries[0]);
        taken.push(rect(top - Vec2::new(w / 2.0, 0.0), Vec2::new(w, h)));
        out.insert(m.signal, (g, top));
    }
    out
}

/// Lays an anchored node out at `at` (stage px of the default view) following the galaxy point `g`.
fn place(node: &mut Node, anchored: Option<Mut<Anchored>>, g: Vec2, at: Vec2) {
    if node.left != px(at.x) || node.top != px(at.y) {
        node.left = px(at.x);
        node.top = px(at.y);
    }
    if let Some(mut a) = anchored
        && *a != Anchored::At(g, Layer::Always)
    {
        *a = Anchored::At(g, Layer::Always);
    }
}

/// The panel's words for the selection: a voyage's (`layout`), or a cluster's, a signal's or a spot's (`known`).
struct Panel {
    head: String,
    title: String,
    chips: Vec<(String, Tone)>,
    summary: String,
    sites_head: String,
    sites: Vec<layout::SiteRow>,
    minutes: String,
    /// The status line when nothing blocks.
    status: String,
}

impl Panel {
    fn of(content: &data::Content, career: &data::Career, known: &Known, pick: &Pick) -> Option<Panel> {
        let Pick::Voyage(id) = pick else {
            let w = place_words(content, career, known, pick)?;
            return Some(Panel { head: w.head, title: w.title, chips: w.chips, summary: w.summary, sites_head: w.rows_head, sites: w.rows, minutes: w.warp, status: w.status });
        };
        let (k, v) = (content.voyages.position(id)?, content.voyages.get(id)?);
        let status = match VoyageState::of(content, career, &v.id) {
            VoyageState::Cleared { .. } if career.exploration().and_then(|g| Some(g.gates().contains(g.pins().get(id)?))) == Some(true) => {
                "CLEARED  ·  ITS GATE IS OPEN".into()
            }
            VoyageState::Cleared { best } => format!("CLEARED{}  ·  NEW SITES EACH TIME", best.map(|t| format!("  ·  BEST {}", world::voyage::clock(t))).unwrap_or_default()),
            VoyageState::Signal => "A NEW SIGNAL  ·  THE GATE IS OPEN".into(),
            _ => "THE GATE IS OPEN".into(),
        };
        Some(Panel {
            head: format!("DESTINATION  ·  VOYAGE {}", k + 1),
            title: v.name.clone(),
            chips: chips(content, v),
            summary: content.render_plain(&v.summary),
            sites_head: format!("SITES  ·  {}", v.site_count()),
            sites: site_rows(content, v),
            minutes: format!("{}  ·  RECALL WHEN CLEAR", minutes(v)),
            status,
        })
    }
}

/// The panel follows the selection, the career, the fleet and the warp.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync_panel(
    map: Res<VoyageMap>,
    fleet: FleetNow,
    time: Res<Time<Real>>,
    style: Res<Style>,
    mut texts: Query<(&MapText, &mut Text, &mut TextColor)>,
    mut nodes: Query<(&MapNode, &mut Node, Option<&mut BackgroundColor>, Option<&mut BorderColor>, Option<&mut UiTransform>)>,
    mut icons: Query<(&ShipIcon, &mut UiIcon)>,
    mut glyphs: Query<(&ChildOf, &mut UiIcon), Without<ShipIcon>>,
    pill: Single<(Entity, &mut Pill), With<MapButton>>,
    mut counted: Local<u32>,
) {
    let f = &fleet;
    // A warp counts down in whole seconds.
    let left = map.warp.map_or(0, |w| (w.seconds - (time.elapsed_secs() - w.start)).ceil().max(0.0) as u32);
    let ticked = left != *counted;
    *counted = left;
    if !(ticked || map.is_changed() || f.career.is_changed() || f.granted.is_changed() || f.design.is_changed() || f.escorts.is_changed() || f.known.is_changed()) {
        return;
    }
    let content = &f.content.0;
    let career = f.career.career();
    let ui = &style.ui;
    let Some(pick) = map.selected.as_ref() else { return };
    let Some(panel) = Panel::of(content, career, &f.known.0, pick) else { return };
    let flagship = f.design.blueprint();
    let escorts = &f.escorts.0;
    let ships: Vec<&data::Blueprint> = std::iter::once(flagship).chain(escorts).collect();
    let mind = f.granted.mind_capacity;
    let used = fleet_cores(content, flagship, escorts);
    let checks = layout::checklist(content, &f.granted.unlocks, mind, f.granted.launch_stock(), flagship, escorts);
    let blocker = f.blocker(pick);
    let plural = |n: u32, one: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {one}S") };
    let ship_sub = |k: usize, b: &data::Blueprint| {
        let t = content.ship_totals(b);
        let who = if k == 0 { "FLAGSHIP".to_string() } else { format!("ESCORT {k}") };
        format!("{who}  ·  {}  ·  CARGO {}", plural(t.logic_cores, "CORE"), content.ship_hold(b))
    };
    // What the line over SET COURSE says: the warp, a refusal, the blocker, a practice run (`?unlock=all` grants what
    // the career doesn't), or how the pick stands.
    let status = if map.warp.is_some() {
        (format!("WARP  ·  {left} S  ·  ESC ENDS IT"), Tone::Teal)
    } else if let Some(why) = &map.say {
        (why.clone(), Tone::Coral)
    } else if let Some(b) = &blocker {
        (b.clone(), Tone::Coral)
    } else if let Some(own) = f.granted.free.then(|| fleet_blocker(content, &career.unlocks(content), career.mind_capacity(content), Some(career.stock()), flagship, escorts)).flatten() {
        (format!("PRACTICE RUN, NOT SAVED  ·  {own}"), Tone::Gold)
    } else {
        (panel.status.clone(), Tone::Teal)
    };
    let sites = &panel.sites;
    for (t, mut text, mut color) in &mut texts {
        let (words, tone) = match *t {
            MapText::Head => (panel.head.clone(), Tone::Label),
            MapText::Title => (panel.title.clone(), Tone::Number),
            MapText::Summary => (panel.summary.clone(), Tone::Label),
            MapText::Chip(j) => panel.chips.get(j).cloned().unwrap_or((String::new(), Tone::Label)),
            MapText::SitesHead => (panel.sites_head.clone(), Tone::Label),
            MapText::SiteName(j) => (sites.get(j).map_or(String::new(), |s| s.name.clone()), Tone::Number),
            MapText::SiteTag(j) => sites.get(j).map_or((String::new(), Tone::Dim), |s| s.tag.clone()),
            MapText::SiteGives(j) => (sites.get(j).map_or(String::new(), |s| s.gives.clone()), Tone::Dim),
            MapText::Minutes => (panel.minutes.clone(), Tone::Label),
            MapText::Cores => (format!("{used} / {mind}"), if used > mind { Tone::Coral } else { Tone::Violet }),
            MapText::ShipName(j) => (ships.get(j).map_or(String::new(), |b| b.name.clone()), Tone::Number),
            MapText::ShipSub(j) => (ships.get(j).map_or(String::new(), |b| ship_sub(j, b)), Tone::Dim),
            MapText::NoEscort => (format!("NO ESCORT  ·  YOUR MIND RUNS {}", plural(mind, "CORE")), Tone::Dim),
            MapText::ReadyHead if checks.iter().all(layout::Check::ok) => ("VOYAGE-READY".into(), Tone::Teal),
            MapText::ReadyHead => ("NOT VOYAGE-READY".into(), Tone::Coral),
            MapText::Check(j) => (checks[j].words.clone(), if checks[j].ok() { Tone::Label } else { Tone::Coral }),
            MapText::Status => status.clone(),
            _ => continue,
        };
        set_text(&mut text, &words);
        set_color(&mut color, ui_color(tone.color(ui), 1.0));
    }
    let chips = &panel.chips;
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
            MapNode::SitesHead => !panel.sites_head.is_empty(),
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
            MapNode::Mark(j) => {
                let tone = ui_color(checks[j].tone.color(ui), 1.0);
                let fill = if checks[j].ok() { tone } else { Color::NONE };
                if let Some(mut bg) = bg
                    && bg.0 != fill
                {
                    bg.0 = fill;
                }
                if let Some(mut b) = border
                    && *b != BorderColor::all(tone)
                {
                    *b = BorderColor::all(tone);
                }
                true
            }
            MapNode::NoEscort => escorts.is_empty(),
            MapNode::Voyage(_) | MapNode::Tag(_) | MapNode::Cluster(_) | MapNode::Signal(_) => continue,
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
    let disabled = blocker.is_some() || map.warp.is_some();
    let (pill_entity, mut pill) = pill.into_inner();
    if pill.disabled != disabled {
        pill.disabled = disabled;
        pill.primary = !disabled;
    }
    // SET COURSE's play glyph dims with it.
    let play = UiIcon::Glyph { glyph: Glyph::Play, color: if disabled { ui.dim } else { ui.teal } };
    for (parent, mut icon) in &mut glyphs {
        if parent.parent() == pill_entity && *icon != play {
            *icon = play.clone();
        }
    }
}

/// A band's state under its name (`Geometry::rings[k]`): home's `CHARTED · OPEN`, another open one `UNCHARTED ·
/// OPEN`, the first sealed one `SEALED` and who beats its way in, a deeper one `SEALED`.
fn band_state(geo: &Geometry, known: &Known, k: usize) -> (String, Tone, f32) {
    let mark = known.bands.get(k);
    let charted = geo.rings.get(k).is_some_and(|r| r.charted);
    match mark {
        Some(m) if m.open || charted => (format!("{}  ·  OPEN", if charted { "CHARTED" } else { "UNCHARTED" }), Tone::Teal, 0.8),
        Some(m) if k > 0 && known.bands.get(k - 1).is_some_and(|o| o.open) => match m.beat() {
            Some(beat) => (format!("SEALED  ·  {beat}"), Tone::Coral, 0.85),
            None => ("SEALED".into(), Tone::Coral, 0.85),
        },
        _ => ("SEALED".into(), Tone::Dim, 1.0),
    }
}

/// The star map's meshes: each rebuilt when what it shows changes, placed over the stage (stage px → world through
/// the camera rig, as `ui_icons` places icons), the galaxy's moved and scaled with the view.
#[allow(clippy::too_many_arguments)]
fn place_mesh(
    stage: Single<(&ComputedNode, &UiGlobalTransform), With<Stage>>,
    chrome: Query<(&Chrome, &Node, &ComputedNode, &UiGlobalTransform)>,
    window: Single<&Window, With<PrimaryWindow>>,
    rig: Res<CameraRig>,
    view: Res<MapView>,
    mut sheets: Query<(&mut MapMesh, &mut Transform)>,
    (geo, known): (Res<MapGeometry>, Res<MapKnown>),
    map: Res<VoyageMap>,
    career: Res<PlayerCareer>,
    content: Res<GameContent>,
    hots: Query<(&MapButton, &Hot)>,
    toggles: Res<RenderToggles>,
    style: Res<Style>,
    atlas: Res<Atlas>,
    real: Res<Time<Real>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let content = &content.0;
    let (geo, known) = (&geo.0, &known.0);
    let career = career.career();
    let now = real.elapsed_secs();
    let far = Look {
        voyages: content.voyages.iter().map(|v| v.id.clone()).collect(),
        states: VoyageState::all(content, career),
        guardian: content.voyages.iter().map(|v| is_guardian(content, v)).collect(),
        known: known.clone(),
        selected: None,
        hovered: None,
        glow: toggles.glow_sprites,
        pulse: 0.0,
        wave: 0.0,
        fading: Vec::new(),
        course: None,
        warp: None,
    };
    let pick = map.selected.clone();
    // A jump's course: from where it starts to the cluster (or the spot) it goes to.
    let course = pick.as_ref().filter(|p| !matches!(p, Pick::Voyage(_))).and_then(|p| {
        let k = p.target(content, career, known)?;
        let from = geo.clusters.get(career.jump_from(content, k)? as usize).copied()?;
        let to = match p {
            Pick::Cluster(_) => geo.clusters.get(k as usize).copied()?,
            _ => geo.to_galaxy(p.spot(known)?),
        };
        Some((from, to))
    });
    // Real time, every frame: the pulses move on each frame (they stepped at 15–20 Hz before 18 Oct 2026).
    let seconds = real.elapsed_secs_wrapped();
    let look = Look {
        selected: pick,
        hovered: hots.iter().find_map(|(b, h)| match (b, h.hovered) {
            (MapButton::Voyage(k), true) => Some(Hover::Voyage(*k)),
            (MapButton::Cluster(k), true) => Some(Hover::Cluster(*k)),
            (MapButton::Signal(j), true) => geo.signals.get(*j).map(|s| Hover::Signal(*s)),
            _ => None,
        }),
        pulse: Look::pulse_at(&far.states, seconds),
        course: course.filter(|_| map.warp.is_none()),
        warp: map.warp.map(|w| WarpLook { from: w.from, to: w.to, t: ((now - w.start) / w.seconds.max(1e-3)).clamp(0.0, 1.0) }),
        ..far.clone()
    };
    // The areas' own look: the waves and the fading edges, not the voyages' pulse.
    let areas = Look {
        pulse: 0.0,
        wave: Look::wave_at(known, seconds),
        fading: map.fading.iter().map(|&(a, t)| (a, ((now - t) / FADE).clamp(0.0, 1.0))).collect(),
        course: None,
        warp: None,
        ..look.clone()
    };
    let (node, at) = *stage;
    let scale = window.scale_factor();
    let k = node.size.x / scale / STAGE.x;
    if k <= 0.0 {
        return;
    }
    // The panel's place (stage px) and whether the legend shows: the chrome's look.
    let corner = at.translation - node.size / 2.0;
    let panel = chrome.iter().find(|c| *c.0 == Chrome::Panel).filter(|c| c.2.size.x > 0.0).map(|(_, _, n, t)| {
        let r = Rect::from_center_size((t.translation - corner) / (k * scale), n.size / (k * scale));
        // Whole px, so a sub-px wobble of the layout doesn't rebuild it.
        (Rect::from_corners(r.min.round(), r.max.round()), chrome.iter().any(|c| *c.0 == Chrome::Legend && c.1.display != Display::None))
    });
    // World units per stage px, and where the stage's centre is.
    let unit = k / rig.zoom;
    let centre = rig.screen_to_world(at.translation / scale);
    let (white, soft) = (atlas.region("fx/white"), atlas.region("fx/dot_soft"));
    let open: Vec<bool> = known.bands.iter().map(|b| b.open).collect();
    for (mut m, mut tf) in &mut sheets {
        let want = match m.sheet {
            Sheet::Far => Some(Built::Far(far.clone())),
            Sheet::Detail => Some(Built::Placed(look.clone(), view.zoom, view.offset, view.area())),
            Sheet::Areas => Some(Built::Placed(areas.clone(), view.zoom, view.offset, view.area())),
            Sheet::Galaxy => Some(Built::Bands(open.clone())),
            Sheet::Chrome => panel.map(|(rect, legend)| Built::Chrome(rect, legend)),
        };
        if let Some(want) = want.filter(|w| m.built.as_ref() != Some(w)) {
            let mesh = match m.sheet {
                Sheet::Galaxy => {
                    let (mesh, over) = galaxy::build(geo, &style, &open, &white, &soft);
                    debug!("map: the galaxy's mesh covers its disc {over:.1} times over");
                    mesh
                }
                Sheet::Far => galaxy::build_far(geo, &far, &style, &white, &soft),
                Sheet::Areas => draw::build_areas(geo, &areas, |g| view.to_screen(g), view.zoom, view.area(), &style, &white, &soft),
                Sheet::Chrome => match panel {
                    Some((rect, legend)) => draw::build_chrome(&style, &white, &soft, rect, legend),
                    None => continue,
                },
                _ => draw::build_detail(geo, &look, |g| view.to_screen(g), view.area(), &style, &white, &soft),
            };
            let _ = meshes.insert(&m.mesh, mesh);
            m.built = Some(want);
        }
        // A zooming sheet: galaxy px → stage px on screen is `g × zoom + offset` (`view::MapView`); in the mesh's
        // frame (origin at the stage's centre, y up) that is a scale by the zoom and this shift.
        let (shift, s) = if m.sheet.zooms() {
            let z = view.zoom;
            (Vec2::new((z - 1.0) * STAGE.x / 2.0 + view.offset.x, (1.0 - z) * STAGE.y / 2.0 - view.offset.y), z)
        } else {
            (Vec2::ZERO, 1.0)
        };
        let want = Transform {
            translation: (centre + shift * unit).extend(m.sheet.z()),
            scale: Vec3::new(s * unit, s * unit, 1.0),
            ..default()
        };
        if *tf != want {
            *tf = want;
        }
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
