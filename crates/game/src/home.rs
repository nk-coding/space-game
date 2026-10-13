//! Home: the shipyard itself (GDR-0071; it replaced the ring menu of GDR-0008 and GDR-0062), where the game starts
//! (`?scene=home`, the default). The station is a world scene of its own (`shipyard`); this module is the screen
//! over it and the camera's flights between the yard and its places.
//!
//! ```text
//! THE SHIPYARD · RIM          RESEARCH  [lab]      FABRICATOR
//! Rim skiff copy               COMING SOON  [dark]
//!  [mooring]==.      .========================.
//!             [mind]=|  BUILDING AREA (ship) =|  > > > ( GATE )
//!  [mooring]=='      '========================'      VOYAGES
//! FLEET               [SALVAGE BAY] [tower]-o--o--o-   NOTES …
//! ```
//!
//! * **Places** (`shipyard::layout::Place`): a click or a tap on one, or its key, flies the camera onto it
//!   (`shipyard::fly`, 0.6 s, zooming around the point that keeps its place on screen) and then opens its screen:
//!   the building area is the drydock (D; the camera lands exactly on the drydock's own view of the frame,
//!   `DockCamera::opening_view`, so only its panels appear), the mind the BEHAVIOR screen (B), the tower and lane the
//!   TEST RANGE (T: the sandbox with raider waves), the moorings FLEET (F, a layer over home), the gate a voyage (V:
//!   the first open one, `voyage::StartVoyage`, until the voyage map exists; a refused launch flies back out with a
//!   note). Research (R) is inert until its screen exists (track 7G); the fabricator and the salvage bay are dark.
//!   Every working place sends one message when the camera arrives, [`HomeAction`]. Hover highlights a working
//!   place (corner brackets, a brighter title).
//! * **Coming back** from a screen the camera starts on its place (the drydock's last view, the mind, the test
//!   range; FLEET's moorings when it closes) and flies back out to the yard.
//! * **Labels** are small UI text placed over the world (the sketch's), shown while the camera rests on the yard.
//! * **Notes** bottom right: what the career has to say (a new game, a save kept as it is, content that changed,
//!   `?unlock=all`, an escort whose design is gone), from `career::PlayerCareer::notes`.
//!
//! Entering home keeps the flight camera's zoom and leaving gives it back with the normal UI scale, so the drydock
//! and flight find the camera as they left it. UI words are chrome in code, like the drydock's; the shipyard's
//! place and name move to content once content has places.

use bevy::prelude::*;
use bevy::state::state::StateTransitionEvent;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale, UiTransform};
use bevy::window::PrimaryWindow;

use crate::camera::{CamMode, CameraRig, CameraSet};
use crate::career::{EscortDesigns, Granted, PlayerCareer, fleet_cores};
use crate::drydock::PlayerDesign;
use crate::drydock::view::{DockCamera, GridViewport};
use crate::fleet::{self, Fleet};
use crate::flight_hud::HudFonts;
use crate::fx::{FxFlush, FxQueue, FxRegions};
use crate::mode::AppMode;
use crate::scene::{Launch, Scene};
use crate::shipyard::fly::{Fly, View};
use crate::shipyard::layout::{Align, Layout, Place, Tag, fit, fit_tagged};
use crate::shipyard::{ShipyardSet, Yard};
use crate::sim_bridge::GameContent;
use crate::style::{Style, ui_color};
use crate::ui_kit::{Hot, Tip, Tone, UiClick, UiInputSet, UiPointer, UiStyleSet, label, prose, set_color, set_text, ui_scale_for};

/// Label sizes (logical px at the 1280 × 720 layout; the UI scale grows them with the window).
const TITLE: f32 = 11.0;
const SUB: f32 = 8.5;
/// The gap between a label's title and the line under it (logical px at the 1280 × 720 layout).
const LABEL_GAP: f32 = 4.0;
/// The least room between the station or its labels and the window's edges on the yard (logical px).
const MARGIN: f32 = 18.0;
/// The name plate's letters, in metres (they are painted on the station).
const PLATE_LETTERS: f32 = 0.42;
/// The hover brackets: arm and gap outside the place, line width (logical px).
const BRACKET: (f32, f32, f32) = (14.0, 6.0, 1.5);
/// The FX layer the hover brackets draw in (above the station and its rigging).
const Z_BRACKETS: f32 = 410.0;
/// Notes shown at most (the most serious first; the console has them all).
const NOTES: usize = 2;

/// What home's places do once the camera has arrived.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum HomeAction {
    /// The building area: the ship editor.
    Drydock,
    /// The mind: the BEHAVIOR screen for the working design.
    Behavior,
    /// The test range: the sandbox with raider waves.
    TestRange,
    /// The moorings: the fleet layer over home (`fleet.rs` opens it).
    Fleet,
    /// The gate: launch a voyage (`voyage.rs`).
    Voyages,
}

/// A place's key, if it has one (its tooltip names it while it works).
pub fn key_of(place: Place) -> Option<(KeyCode, &'static str)> {
    match place {
        Place::Drydock => Some((KeyCode::KeyD, "D")),
        Place::Behavior => Some((KeyCode::KeyB, "B")),
        Place::Research => Some((KeyCode::KeyR, "R")),
        Place::Voyages => Some((KeyCode::KeyV, "V")),
        Place::TestRange => Some((KeyCode::KeyT, "T")),
        Place::Fleet => Some((KeyCode::KeyF, "F")),
        Place::Fabricator | Place::SalvageBay => None,
    }
}

/// What a place opens; `None`: not yet (research until track 7G, the dark buildings).
pub fn action_of(place: Place) -> Option<HomeAction> {
    match place {
        Place::Drydock => Some(HomeAction::Drydock),
        Place::Behavior => Some(HomeAction::Behavior),
        Place::TestRange => Some(HomeAction::TestRange),
        Place::Fleet => Some(HomeAction::Fleet),
        Place::Voyages => Some(HomeAction::Voyages),
        Place::Research | Place::Fabricator | Place::SalvageBay => None,
    }
}

/// A working place's tooltip: what it opens and its key (labels carry no key letters, Niklas 13 Oct 2026).
fn tip_of(place: Place) -> Option<Tip> {
    let name = match action_of(place)? {
        HomeAction::Drydock => "DRYDOCK",
        HomeAction::Behavior => "BEHAVIOR",
        HomeAction::TestRange => "TEST RANGE",
        HomeAction::Fleet => "FLEET",
        HomeAction::Voyages => "VOYAGES",
    };
    Some(key_of(place).map_or(Tip::new(name), |(_, key)| Tip::key(name, key)))
}

/// The place a screen belongs to: where the camera starts when that screen goes back home.
fn place_of(mode: AppMode) -> Option<Place> {
    match mode {
        AppMode::Drydock => Some(Place::Drydock),
        AppMode::Behavior => Some(Place::Behavior),
        AppMode::Flight => Some(Place::TestRange),
        AppMode::Home => None,
    }
}

/// Where home's camera is.
#[derive(Clone, Copy, Debug, Default)]
enum Phase {
    /// On the yard: labels, hover and clicks.
    #[default]
    Yard,
    /// On the way to a place (`to`), or back to the yard from one (`from`).
    Flying { fly: Fly, to: Option<Place>, from: Option<Place> },
    /// Arrived at a place: its screen opens (FLEET stays a layer over home until it closes).
    At(Place, View),
}

#[derive(Resource, Debug, Default)]
struct Visit {
    phase: Phase,
}

/// Home's UI root: the place name, the design's name and the notes (shown while the camera rests on the yard).
#[derive(Component)]
struct HomeRoot;

/// The labels and the click areas over the world (shown while the camera rests on the yard).
#[derive(Component)]
struct LabelRoot;

#[derive(Component)]
struct DesignName;

/// A place's label: placed over its anchor.
#[derive(Component)]
struct PlaceLabel(Place);

#[derive(Component)]
struct LabelTitle(Place);

#[derive(Component)]
struct LabelSub(Place);

/// The click area over a working place.
#[derive(Component)]
struct Spot(Place);

/// The name on the base's plate, and the point it turns around.
#[derive(Component)]
struct PlateName;

#[derive(Component)]
struct PlateAnchor;

/// The career's notes: one head and its words per note.
#[derive(Component, Clone, Copy)]
enum NoteText {
    Head(usize),
    Body(usize),
}

/// The flight camera's zoom while home has the camera.
#[derive(Resource, Debug, Default)]
struct HomeCamera {
    flight_zoom: f32,
}

pub struct HomePlugin;

impl Plugin for HomePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<HomeAction>()
            .init_resource::<HomeCamera>()
            .init_resource::<Visit>()
            .init_resource::<LabelTags>()
            .add_systems(Startup, spawn.after(crate::flight_hud::spawn_hud))
            .add_systems(PostStartup, remember_start_zoom.run_if(in_state(AppMode::Home)))
            .add_systems(OnEnter(AppMode::Home), enter)
            .add_systems(OnExit(AppMode::Home), exit)
            .add_systems(
                Update,
                (
                    ui_scale,
                    (clicks, keys).run_if(not(fleet::is_open)),
                    fly,
                    apply,
                    (place_labels, sync_titles, sync_name, sync_notes).run_if(not(fleet::is_open)),
                    measure_labels,
                    show,
                )
                    .chain()
                    .after(UiInputSet)
                    .before(UiStyleSet)
                    .before(ShipyardSet)
                    .run_if(in_state(AppMode::Home)),
            )
            .add_systems(Update, show.run_if(state_changed::<AppMode>))
            .add_systems(PostUpdate, draw_hover.run_if(in_state(AppMode::Home)).after(CameraSet).before(FxFlush));
    }
}

/// The colour a place's title takes, and its line under it.
fn title_tone(place: Place, yard: &Yard) -> Tone {
    match place {
        Place::Drydock => Tone::Gold,
        Place::Behavior => Tone::Violet,
        Place::TestRange => Tone::Coral,
        Place::Fleet => Tone::Teal,
        Place::Research if yard.lights.research => Tone::Gold,
        Place::Voyages if yard.lights.gate => Tone::Teal,
        Place::Research | Place::Voyages | Place::Fabricator | Place::SalvageBay => Tone::Dim,
    }
}

fn spawn(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let fonts = &*fonts;
    let full = Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() };
    commands.spawn((HomeRoot, full.clone(), Visibility::Hidden)).with_children(|root| {
        // Top left: where we are and the docked design's name.
        root.spawn(Node { position_type: PositionType::Absolute, left: px(28), top: px(22), flex_direction: FlexDirection::Column, row_gap: px(6), ..default() })
            .with_children(|tl| {
                tl.spawn(label(fonts, "THE SHIPYARD  ·  RIM", 11.0, c(Tone::Label, 1.0)));
                tl.spawn((DesignName, prose(fonts, "", 20.0, c(Tone::Number, 1.0))));
            });
        // Bottom right: the career's notes.
        root.spawn(Node {
            position_type: PositionType::Absolute,
            right: px(28),
            bottom: px(22),
            width: px(300),
            flex_direction: FlexDirection::Column,
            row_gap: px(5),
            ..default()
        })
        .with_children(|notes| {
            for k in 0..NOTES {
                notes.spawn((NoteText::Head(k), label(fonts, "", 9.0, c(Tone::Teal, 1.0)), Node { margin: UiRect::top(px(if k == 0 { 0.0 } else { 6.0 })), ..default() }));
                notes.spawn((NoteText::Body(k), prose(fonts, "", 11.0, c(Tone::Label, 1.0))));
            }
        });
    });
    commands.spawn((LabelRoot, full, Visibility::Hidden)).with_children(|root| {
        for place in Place::ALL {
            if let Some(tip) = tip_of(place) {
                root.spawn((Spot(place), Hot::default(), tip, Node { position_type: PositionType::Absolute, ..default() }));
            }
            let words = TextLayout::no_wrap();
            root.spawn((PlaceLabel(place), Node { position_type: PositionType::Absolute, width: px(0), flex_direction: FlexDirection::Column, row_gap: px(LABEL_GAP), ..default() }))
                .with_children(|l| {
                    l.spawn((LabelTitle(place), label(fonts, "", TITLE, c(Tone::Dim, 1.0)), words));
                    l.spawn((LabelSub(place), label(fonts, "", SUB, c(Tone::Label, 1.0)), words));
                });
        }
        root.spawn((PlateAnchor, Node { position_type: PositionType::Absolute, width: px(0), height: px(0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }))
            .with_child((PlateName, label(fonts, "YARD 7  ·  RIM", 8.0, c(Tone::Gold, 0.9)), TextLayout::no_wrap(), UiTransform::from_rotation(Rot2::degrees(-90.0))));
    });
}

/// The view that shows a place: the drydock's own opening view for the building area, else the place's box.
fn place_view(place: Place, layout: &Layout, window: &Window, dock: &DockCamera, viewport: Option<(&ComputedNode, &UiGlobalTransform)>) -> View {
    let opening = (place == Place::Drydock).then(|| viewport.and_then(|v| dock.opening_view(window, v, layout.frame))).flatten();
    let (centre, zoom) = opening.unwrap_or_else(|| fit(layout.focus(place), window.size()));
    View { centre, zoom }
}

/// The yard's view: the station's overview and every place's label (their pixel size, [`LabelTags`]) at least
/// [`MARGIN`] px inside the window, whatever its aspect (Niklas, 13 Oct 2026: things at the left and top were cut
/// off at some aspects).
fn overview(layout: &Layout, window: &Window, tags: &LabelTags) -> View {
    let (centre, zoom) = fit_tagged(layout.overview, &tags.0, window.size(), MARGIN);
    View { centre, zoom }
}

/// Where each place's label reaches from its anchor, in logical px (measured from the laid-out labels).
#[derive(Resource, Debug, Default, PartialEq)]
struct LabelTags(Vec<Tag>);

/// Measures the labels (their texts change with the career, their size with the UI scale): written only when a
/// label's size changes by half a pixel or more.
#[allow(clippy::type_complexity)]
fn measure_labels(
    window: Single<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    yard: Res<Yard>,
    titles: Query<(&LabelTitle, &ComputedNode)>,
    subs: Query<(&LabelSub, &ComputedNode, &Node)>,
    mut tags: ResMut<LabelTags>,
) {
    let k = 1.0 / window.scale_factor();
    let new: Vec<Tag> = titles
        .iter()
        .map(|(t, title)| {
            let title = title.size * k;
            let sub = subs.iter().find(|(s, ..)| s.0 == t.0).filter(|(_, _, n)| n.display != Display::None).map(|(_, c, _)| c.size * k);
            let (w, h) = match sub {
                Some(s) => (title.x.max(s.x), title.y + LABEL_GAP * ui_scale.0 + s.y),
                None => (title.x, title.y),
            };
            let (at, align) = yard.layout.label(t.0);
            let x = match align {
                Align::Start => (0.0, w),
                Align::Middle => (-w / 2.0, w / 2.0),
                Align::End => (-w, 0.0),
            };
            Tag { at, min: Vec2::new(x.0, 0.0), max: Vec2::new(x.1, h) }
        })
        .collect();
    let same = tags.0.len() == new.len() && tags.0.iter().zip(&new).all(|(a, b)| a.at == b.at && a.min.distance(b.min) < 0.5 && a.max.distance(b.max) < 0.5);
    if !same {
        tags.0 = new;
    }
}

/// The rig shows `v` (the camera stands still between frames: zoom and its target agree).
fn put_rig(rig: &mut CameraRig, v: View, viewport: Vec2) {
    rig.viewport = viewport;
    rig.view_offset = Vec2::ZERO;
    rig.end_zoom_anchor();
    rig.pos = v.centre;
    rig.zoom = v.zoom;
    rig.zoom_target = v.zoom;
}

/// Coming home: keep the flight zoom; the camera starts on the place of the screen we came from and flies out.
#[allow(clippy::too_many_arguments)]
fn enter(
    mut transitions: MessageReader<StateTransitionEvent<AppMode>>,
    debrief: Res<crate::debrief::Debrief>,
    window: Single<&Window, With<PrimaryWindow>>,
    dock: Res<DockCamera>,
    viewport: Query<(&ComputedNode, &UiGlobalTransform), With<GridViewport>>,
    yard: Res<Yard>,
    tags: Res<LabelTags>,
    mut rig: ResMut<CameraRig>,
    mut cam: ResMut<HomeCamera>,
    mut visit: ResMut<Visit>,
) {
    cam.flight_zoom = rig.zoom_target;
    rig.mode = CamMode::Free;
    rig.auto_mode = CamMode::Free;
    // Home from a voyage (its debrief is open): from the gate.
    let exited = transitions.read().last().and_then(|t| t.exited);
    let from = if exited == Some(AppMode::Flight) && debrief.open { Some(Place::Voyages) } else { exited.and_then(place_of) };
    let yard_view = overview(&yard.layout, &window, &tags);
    visit.phase = match from {
        Some(place) => {
            let start = place_view(place, &yard.layout, &window, &dock, viewport.single().ok());
            put_rig(&mut rig, start, window.size());
            Phase::Flying { fly: Fly::new(start, yard_view), to: None, from: Some(place) }
        }
        None => {
            put_rig(&mut rig, yard_view, window.size());
            Phase::Yard
        }
    };
}

/// At start at home: the flight zoom of the URL (`?zoom=`, applied at Startup) is the one to give back.
fn remember_start_zoom(rig: Res<CameraRig>, mut cam: ResMut<HomeCamera>) {
    cam.flight_zoom = rig.zoom_target;
}

/// Leaving home: flight gets its zoom back, the UI its normal scale (the drydock and the BEHAVIOR screen set
/// their own).
fn exit(mut rig: ResMut<CameraRig>, cam: Res<HomeCamera>, mut ui_scale: ResMut<UiScale>) {
    rig.view_offset = Vec2::ZERO;
    rig.zoom = cam.flight_zoom;
    rig.zoom_target = cam.flight_zoom;
    ui_scale.0 = 1.0;
}

/// The UI scale: the labels are made for 1280 × 720 logical pixels and grow with larger windows (as the drydock's,
/// whose layout home's camera flies onto).
fn ui_scale(window: Single<&Window, With<PrimaryWindow>>, mut ui_scale: ResMut<UiScale>) {
    let s = ui_scale_for(&window);
    if (ui_scale.0 - s).abs() > 1e-3 {
        ui_scale.0 = s;
    }
}

/// Starts the flight to a working place (while the camera rests on the yard).
#[allow(clippy::too_many_arguments)]
fn go(place: Place, visit: &mut Visit, yard: &Yard, window: &Window, rig: &CameraRig, dock: &DockCamera, viewport: Option<(&ComputedNode, &UiGlobalTransform)>) {
    if !matches!(visit.phase, Phase::Yard) || action_of(place).is_none() {
        return;
    }
    let from = View { centre: rig.pos, zoom: rig.zoom };
    let to = place_view(place, &yard.layout, window, dock, viewport);
    visit.phase = Phase::Flying { fly: Fly::new(from, to), to: Some(place), from: None };
}

/// Clicks and taps on the places.
#[allow(clippy::too_many_arguments)]
fn clicks(
    mut clicks: MessageReader<UiClick>,
    spots: Query<&Spot>,
    window: Single<&Window, With<PrimaryWindow>>,
    dock: Res<DockCamera>,
    viewport: Query<(&ComputedNode, &UiGlobalTransform), With<GridViewport>>,
    yard: Res<Yard>,
    rig: Res<CameraRig>,
    mut visit: ResMut<Visit>,
) {
    for c in clicks.read() {
        if let Ok(spot) = spots.get(c.0) {
            go(spot.0, &mut visit, &yard, &window, &rig, &dock, viewport.single().ok());
        }
    }
}

/// D, B, T, F, V (R waits for its screen): not while a text field has the keyboard, and not with Ctrl or Cmd
/// held (Ctrl+D is the browser's bookmark key), as the drydock's shortcuts.
#[allow(clippy::too_many_arguments)]
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    pointer: Res<UiPointer>,
    window: Single<&Window, With<PrimaryWindow>>,
    dock: Res<DockCamera>,
    viewport: Query<(&ComputedNode, &UiGlobalTransform), With<GridViewport>>,
    yard: Res<Yard>,
    rig: Res<CameraRig>,
    mut visit: ResMut<Visit>,
) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight]);
    if pointer.typing || ctrl {
        return;
    }
    for place in Place::ALL {
        if key_of(place).is_some_and(|(k, _)| keys.just_pressed(k)) {
            go(place, &mut visit, &yard, &window, &rig, &dock, viewport.single().ok());
        }
    }
}

/// The camera: on the yard it shows the whole station (fitted to the window); in flight it follows the flight
/// (time-based), and the rigging makes room as it flies into the drydock (or comes back out); on arrival at a
/// place its screen opens. FLEET's moorings: when FLEET closes, the camera flies back out.
#[allow(clippy::too_many_arguments)]
fn fly(
    time: Res<Time<Real>>,
    window: Single<&Window, With<PrimaryWindow>>,
    fleet: Res<Fleet>,
    tags: Res<LabelTags>,
    mut yard: ResMut<Yard>,
    mut rig: ResMut<CameraRig>,
    mut visit: ResMut<Visit>,
    mut refused: MessageReader<crate::voyage::LaunchRefused>,
    mut out: MessageWriter<HomeAction>,
) {
    let size = window.size();
    let refused = refused.read().count() > 0;
    let mut next = None;
    let mut clear = 0.0;
    let view = match &mut visit.phase {
        Phase::Yard => overview(&yard.layout, &window, &tags),
        Phase::Flying { fly, to, from } => {
            let (view, done) = fly.step(time.delta_secs().min(0.1));
            let p = fly.progress();
            clear = match (*to, *from) {
                (Some(Place::Drydock), _) => p,
                (_, Some(Place::Drydock)) => 1.0 - p,
                _ => 0.0,
            };
            if done {
                next = Some(match *to {
                    Some(place) => {
                        if let Some(a) = action_of(place) {
                            out.write(a);
                        }
                        Phase::At(place, view)
                    }
                    None => Phase::Yard,
                });
            }
            view
        }
        Phase::At(place, view) => {
            if *place == Place::Drydock {
                clear = 1.0;
            }
            if *place == Place::Fleet && !fleet.open {
                // FLEET closed: back out to the yard.
                next = Some(Phase::Flying { fly: Fly::new(*view, overview(&yard.layout, &window, &tags)), to: None, from: Some(Place::Fleet) });
            }
            if *place == Place::Voyages && refused {
                // The launch was refused (a note says why): back out to the yard.
                next = Some(Phase::Flying { fly: Fly::new(*view, overview(&yard.layout, &window, &tags)), to: None, from: Some(Place::Voyages) });
            }
            *view
        }
    };
    if let Some(n) = next {
        visit.phase = n;
    }
    if yard.clear != clear {
        yard.clear = clear;
    }
    put_rig(&mut rig, view, size);
}

/// Where the places lead.
fn apply(
    mut actions: MessageReader<HomeAction>,
    mut scene: ResMut<Scene>,
    mut next: ResMut<NextState<AppMode>>,
    mut voyage: MessageWriter<crate::voyage::StartVoyage>,
) {
    for a in actions.read() {
        match a {
            HomeAction::Drydock => next.set(AppMode::Drydock),
            HomeAction::Behavior => next.set(AppMode::Behavior),
            HomeAction::TestRange => {
                scene.launch = Some(Launch::Range);
                next.set(AppMode::Flight);
            }
            // A layer over home, not a mode: `fleet::open` reads the same message.
            HomeAction::Fleet => {}
            // The first open voyage, until the voyage map's SET COURSE exists.
            HomeAction::Voyages => {
                voyage.write(crate::voyage::StartVoyage { voyage: None });
            }
        }
    }
}

/// Home's screen shows at home while FLEET is closed; the labels while the camera rests on the yard.
#[allow(clippy::type_complexity)]
fn show(
    mode: Res<State<AppMode>>,
    fleet: Res<Fleet>,
    visit: Res<Visit>,
    mut roots: ParamSet<(Query<&mut Visibility, With<HomeRoot>>, Query<&mut Visibility, With<LabelRoot>>)>,
) {
    let home = *mode.get() == AppMode::Home && !fleet.open;
    let resting = home && matches!(visit.phase, Phase::Yard);
    let vis = |on: bool| if on { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut roots.p0() {
        if *v != vis(resting) {
            *v = vis(resting);
        }
    }
    for mut v in &mut roots.p1() {
        if *v != vis(resting) {
            *v = vis(resting);
        }
    }
}

/// Labels, the name plate and the click areas follow the camera (UI px = logical px / the UI scale).
#[allow(clippy::type_complexity)]
fn place_labels(
    yard: Res<Yard>,
    rig: Res<CameraRig>,
    ui_scale: Res<UiScale>,
    visit: Res<Visit>,
    mut nodes: ParamSet<(
        Query<(&PlaceLabel, &mut Node)>,
        Query<(&Spot, &mut Node)>,
        Query<&mut Node, With<PlateAnchor>>,
    )>,
    mut plate: Single<&mut TextFont, With<PlateName>>,
) {
    if !matches!(visit.phase, Phase::Yard) {
        return;
    }
    let k = ui_scale.0.max(1e-3);
    let at = |w: Vec2| rig.world_to_screen(w) / k;
    let set = |node: &mut Node, left: Val, top: Val, size: Option<Vec2>| {
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
        if let Some(s) = size
            && (node.width != px(s.x) || node.height != px(s.y))
        {
            node.width = px(s.x);
            node.height = px(s.y);
        }
    };
    let l = &yard.layout;
    for (label, mut node) in &mut nodes.p0() {
        let (anchor, align) = l.label(label.0);
        let p = at(anchor);
        let items = match align {
            Align::Start => AlignItems::FlexStart,
            Align::Middle => AlignItems::Center,
            Align::End => AlignItems::FlexEnd,
        };
        if node.align_items != items {
            node.align_items = items;
        }
        set(&mut node, px(p.x), px(p.y), None);
    }
    for (spot, mut node) in &mut nodes.p1() {
        let r = l.hit(spot.0);
        let (a, b) = (at(Vec2::new(r.min.x, r.max.y)), at(Vec2::new(r.max.x, r.min.y)));
        set(&mut node, px(a.x), px(a.y), Some(b - a));
    }
    for mut node in &mut nodes.p2() {
        let p = at(l.name_plate().center());
        set(&mut node, px(p.x), px(p.y), None);
    }
    let size = FontSize::Px((PLATE_LETTERS * rig.zoom / k).max(4.0));
    if plate.font_size != size {
        plate.font_size = size;
    }
}

/// Titles and the lines under them: the frame's name in the building area's, FLEET's cores, the lit and dark
/// buildings; a hovered place's title brightens.
#[allow(clippy::too_many_arguments)]
fn sync_titles(
    yard: Res<Yard>,
    design: Res<PlayerDesign>,
    escorts: Res<EscortDesigns>,
    granted: Res<Granted>,
    career: Res<PlayerCareer>,
    content: Res<GameContent>,
    style: Res<Style>,
    spots: Query<(&Spot, Ref<Hot>)>,
    mut titles: Query<(&LabelTitle, &mut Text, &mut TextColor), Without<LabelSub>>,
    mut subs: Query<(&LabelSub, &mut Text, &mut TextColor, &mut Node), Without<LabelTitle>>,
) {
    let hover = spots.iter().any(|(_, h)| h.is_changed());
    if !(hover || yard.is_changed() || design.is_changed() || escorts.is_changed() || granted.is_changed() || career.is_changed()) {
        return;
    }
    let ui = &style.ui;
    let hovered = |p: Place| spots.iter().any(|(s, h)| s.0 == p && h.hovered);
    for (t, mut text, mut color) in &mut titles {
        let words = match t.0 {
            Place::Drydock => {
                let (w, h) = design.frame();
                let frame = content.0.frames.iter().find(|f| f.size == (w, h)).map_or("FRAME".to_string(), |f| f.name.to_uppercase());
                format!("BUILDING AREA  ·  {frame} {w} × {h}")
            }
            Place::Behavior => "BEHAVIOR".into(),
            Place::Research => "RESEARCH".into(),
            Place::Voyages => "VOYAGES".into(),
            Place::TestRange => "TEST RANGE".into(),
            Place::Fleet => "FLEET".into(),
            Place::Fabricator => "FABRICATOR".into(),
            Place::SalvageBay => "SALVAGE BAY".into(),
        };
        set_text(&mut text, &words);
        let tone = if hovered(t.0) { Tone::Number } else { title_tone(t.0, &yard) };
        set_color(&mut color, ui_color(tone.color(ui), if tone == Tone::Dim { 1.0 } else { 0.9 }));
    }
    for (s, mut text, mut color, mut node) in &mut subs {
        let (words, tone) = match s.0 {
            Place::Drydock => (String::new(), Tone::Label),
            Place::Behavior => ("THE MIND".into(), Tone::Label),
            Place::TestRange => ("NO RISK  ·  NO HAUL".into(), Tone::Label),
            Place::Fleet => {
                let cores = fleet_cores(&content.0, design.blueprint(), &escorts.0);
                (format!("MOORINGS  ·  CORES {cores} / {}", granted.mind_capacity), if cores > granted.mind_capacity { Tone::Coral } else { Tone::Violet })
            }
            Place::Research => ("COMING SOON".into(), Tone::Dim),
            Place::Voyages => {
                let open = crate::voyage::default_voyage(career.career(), &content.0);
                match open.and_then(|v| content.0.voyages.get(v)) {
                    Some(v) => (format!("THE GATE  ·  {}", v.name.to_uppercase()), Tone::Teal),
                    None => ("DORMANT".into(), Tone::Dim),
                }
            }
            Place::Fabricator | Place::SalvageBay => ("LATER".into(), Tone::Dim),
        };
        set_text(&mut text, &words);
        set_color(&mut color, ui_color(tone.color(ui), 1.0));
        let display = if words.is_empty() { Display::None } else { Display::Flex };
        if node.display != display {
            node.display = display;
        }
    }
}

fn sync_name(design: Res<PlayerDesign>, mut name: Single<&mut Text, With<DesignName>>) {
    set_text(&mut name, &design.blueprint().name);
}

/// The career's notes: the [`NOTES`] most serious (coral, then gold, then the rest, each in order), and how many
/// more the console has.
fn sync_notes(career: Res<PlayerCareer>, style: Res<Style>, mut texts: Query<(&NoteText, &mut Text, &mut TextColor, &mut Node)>) {
    if !career.is_changed() {
        return;
    }
    let rank = |t: Tone| match t {
        Tone::Coral => 0,
        Tone::Gold => 1,
        _ => 2,
    };
    let mut shown: Vec<_> = career.notes.iter().collect();
    shown.sort_by_key(|n| rank(n.tone));
    let more = shown.len().saturating_sub(NOTES);
    shown.truncate(NOTES);
    for (t, mut text, mut color, mut node) in &mut texts {
        let (k, (s, tone)) = match *t {
            NoteText::Head(k) => (k, shown.get(k).map_or((String::new(), Tone::Dim), |n| (n.head.clone(), n.tone))),
            NoteText::Body(k) => {
                let mut body = shown.get(k).map_or(String::new(), |n| n.body.clone());
                if more > 0 && k + 1 == shown.len() {
                    body.push_str(&format!(" (+{more} more in the console)"));
                }
                (k, (body, Tone::Label))
            }
        };
        set_text(&mut text, &s);
        set_color(&mut color, ui_color(tone.color(&style.ui), 1.0));
        let display = if k < shown.len() { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
}

/// Corner brackets around the hovered place, in screen pixels.
fn draw_hover(
    spots: Query<(&Spot, &Hot, &InheritedVisibility)>,
    yard: Res<Yard>,
    rig: Res<CameraRig>,
    style: Res<Style>,
    regions: Option<Res<FxRegions>>,
    mut queue: ResMut<FxQueue>,
) {
    let Some(r) = regions else { return };
    let Some((spot, ..)) = spots.iter().find(|(_, h, v)| h.hovered && v.get()) else { return };
    let px = 1.0 / rig.zoom;
    let (arm, gap, width) = (BRACKET.0 * px, BRACKET.1 * px, BRACKET.2 * px);
    let rect = yard.layout.hit(spot.0).inflate(gap);
    let c = style.ui.teal.with_alpha(0.85);
    for (corner, s) in [(rect.min, Vec2::new(1.0, 1.0)), (Vec2::new(rect.max.x, rect.min.y), Vec2::new(-1.0, 1.0)), (rect.max, Vec2::new(-1.0, -1.0)), (Vec2::new(rect.min.x, rect.max.y), Vec2::new(1.0, -1.0))] {
        queue.line(&r.white, corner, corner + Vec2::new(s.x * arm, 0.0), width, c, Z_BRACKETS);
        queue.line(&r.white, corner, corner + Vec2::new(0.0, s.y * arm), width, c, Z_BRACKETS);
    }
}

#[cfg(test)]
mod tests {
    use bevy::state::app::StatesPlugin;

    use super::*;
    use crate::scene::SceneKind;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .insert_state(AppMode::Home)
            .insert_resource(Scene::new(SceneKind::Sandbox, 1))
            .add_message::<HomeAction>()
            .add_message::<crate::voyage::StartVoyage>()
            .add_systems(Update, apply);
        app
    }

    /// TEST RANGE asks for the plain sandbox (not a test flight) and flies; DRYDOCK and BEHAVIOR open their screens.
    #[test]
    fn the_places_lead_to_their_modes() {
        let mut app = app();
        app.world_mut().write_message(HomeAction::TestRange);
        app.update();
        assert_eq!(app.world().resource::<Scene>().launch, Some(Launch::Range));
        app.update();
        assert_eq!(*app.world().resource::<State<AppMode>>().get(), AppMode::Flight);
        for (action, mode) in [(HomeAction::Drydock, AppMode::Drydock), (HomeAction::Behavior, AppMode::Behavior)] {
            let mut app = self::app();
            app.world_mut().write_message(action);
            app.update();
            app.update();
            assert_eq!(*app.world().resource::<State<AppMode>>().get(), mode);
            assert_eq!(app.world().resource::<Scene>().launch, None);
        }
        // The gate asks `voyage.rs` for the first open voyage (it launches or says why not).
        let mut app = self::app();
        app.world_mut().write_message(HomeAction::Voyages);
        app.update();
        let asked = app.world_mut().resource_mut::<Messages<crate::voyage::StartVoyage>>().drain().collect::<Vec<_>>();
        assert_eq!(asked, [crate::voyage::StartVoyage { voyage: None }]);
    }

    /// D, B, T, F and V lead to the drydock, the BEHAVIOR screen, the test range, FLEET and a voyage; R (research)
    /// is inert until its screen exists; every key is one place's, every action one place's.
    #[test]
    fn keys_and_places_map_to_screens() {
        let by_key = |k: KeyCode| Place::ALL.into_iter().find(|p| key_of(*p).is_some_and(|(key, _)| key == k));
        let screen = |k: KeyCode| by_key(k).and_then(action_of);
        assert_eq!(screen(KeyCode::KeyD), Some(HomeAction::Drydock));
        assert_eq!(screen(KeyCode::KeyB), Some(HomeAction::Behavior));
        assert_eq!(screen(KeyCode::KeyT), Some(HomeAction::TestRange));
        assert_eq!(screen(KeyCode::KeyF), Some(HomeAction::Fleet));
        assert_eq!(screen(KeyCode::KeyV), Some(HomeAction::Voyages));
        assert_eq!((by_key(KeyCode::KeyR), screen(KeyCode::KeyR)), (Some(Place::Research), None));
        assert_eq!(by_key(KeyCode::KeyV), Some(Place::Voyages));
        for p in Place::ALL {
            if let Some((k, _)) = key_of(p) {
                assert_eq!(by_key(k), Some(p), "{p:?}");
            }
            if let Some(a) = action_of(p) {
                assert_eq!(Place::ALL.iter().filter(|q| action_of(**q) == Some(a)).count(), 1, "{a:?}");
                assert!(key_of(p).is_some(), "{p:?} works but has no key");
            }
        }
        assert_eq!((key_of(Place::Fabricator), action_of(Place::SalvageBay)), (None, None));
        // Every screen goes back home onto its place.
        for (mode, place) in [(AppMode::Drydock, Place::Drydock), (AppMode::Behavior, Place::Behavior), (AppMode::Flight, Place::TestRange)] {
            assert_eq!(place_of(mode), Some(place));
        }
    }

    /// A working place starts a flight there (its screen opens when the camera arrives); another one during the
    /// flight changes nothing; the inert places don't fly.
    #[test]
    fn a_key_flies_there_then_opens_the_screen() {
        let mut visit = Visit::default();
        let yard = Yard::new((24, 24));
        let window = Window::default();
        let mut rig = CameraRig::default();
        (rig.pos, rig.zoom) = (Vec2::new(16.5, -12.0), 16.0);
        let dock = DockCamera::new(18.0);
        go(Place::Drydock, &mut visit, &yard, &window, &rig, &dock, None);
        let Phase::Flying { to, .. } = visit.phase else { panic!("not flying: {:?}", visit.phase) };
        assert_eq!(to, Some(Place::Drydock));
        go(Place::Behavior, &mut visit, &yard, &window, &rig, &dock, None);
        assert!(matches!(visit.phase, Phase::Flying { to: Some(Place::Drydock), .. }));
        // Inert places don't fly.
        let mut rest = Visit::default();
        go(Place::Research, &mut rest, &yard, &window, &rig, &dock, None);
        assert!(matches!(rest.phase, Phase::Yard));
        // The gate flies (the voyage launches on arrival).
        go(Place::Voyages, &mut rest, &yard, &window, &rig, &dock, None);
        assert!(matches!(rest.phase, Phase::Flying { to: Some(Place::Voyages), .. }));
    }
}
