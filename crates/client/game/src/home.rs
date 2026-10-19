//! Home: the shipyard itself (GDR-0071; it replaced the ring menu of GDR-0008 and GDR-0062), where the game starts
//! (`?scene=home`, the default). The station is a world scene of its own (`shipyard`); this module is the screen
//! over it and the camera's flights between the yard and its places.
//!
//! ```text
//! THE SHIPYARD · RIM          RESEARCH  [lab]                   |* LOG
//! Rim skiff copy               1 READY  [lab]                  |  NEW · …
//!  [mooring]==.      .========================[log]
//!             [mind]=|  BUILDING AREA (ship) =|  > > > ( GATE )
//!  [mooring]=='      '========================'      VOYAGES
//! FLEET               [WORKSHOP] [tower]-o--o--o-      NOTES …
//! ```
//!
//! * **Places** (`shipyard::layout::Place`): a click or a tap on one, or its key, flies the camera onto it
//!   (`shipyard::fly`, 0.6 s, zooming around the point that keeps its place on screen) and then opens its screen:
//!   the building area is the drydock (D; the camera lands exactly on the drydock's own view of the frame,
//!   `DockCamera::opening_view`, so only its panels appear), the mind the BEHAVIOR screen (B), the tower and lane the
//!   TEST RANGE (T: the sandbox with raider waves), the moorings FLEET (F, a layer over home), the lab RESEARCH (R,
//!   `research/`), the records mast by the gate the LOG (L, a layer over home, `voyage_log.rs`; its tooltip is the
//!   last voyage in one line, and it glows while a result waits there), the compute stack above the mind the
//!   EVOLUTION lab (E, a layer over home, `evolution/`, GDR-0092; its label says how far its run is). The lab is dark
//!   and says `LOCKED` (its tooltip `LOCKED · RESEARCH SIMULATION VAULT`) until its research node is researched; a
//!   click or E on it then flies to the lab and opens RESEARCH on that node, as the drydock's LOCKED rows do. The gate opens the voyage map at once (V,
//!   `map/`; the debrief's VOYAGE MAP, `map::OpenMap`, too):
//!   its portal takes the camera through the gate and back out (`map::portal`). The workshop under the bottom wall
//!   is the WORKSHOP (W, a layer over home, `workshop.rs`, GDR-0108); its line says how many hauled parts wait to be
//!   sorted, or what the fleet is short of.
//!   Every working place sends one message when the camera arrives, [`HomeAction`]. Hover highlights a working
//!   place (corner brackets, a brighter title).
//! * **Coming back** from a screen the camera starts on its place (the drydock's last view, the mind, the test
//!   range, the lab; FLEET's moorings, the LOG's mast and the EVOLUTION lab when they close) and flies back out to
//!   the yard; from the map it is back out already. From a fight the lab's WATCH opened it lands on the lab with the
//!   layer open again; the drydock's EVOLVE THIS DESIGN flies straight from the drydock to the lab.
//! * **Labels** are small UI text placed over the world (the sketch's), shown while the camera rests on the yard.
//! * **Notes** bottom right: what the career has to say (a new game, a save kept as it is, content that changed,
//!   `?unlock=all`, an escort whose design is gone), from `career::PlayerCareer::notes`.
//! * **The layers over home** (FLEET, SETTINGS, the LOG, EVOLUTION) leave the station barely showing: home's words, labels and
//!   click areas, and the debrief's panel, step aside while one is open, so nothing reads through it. Home's words,
//!   labels and click areas step aside while the voyage's report shows, too (its backing is a world mesh).
//!
//! Entering home keeps the flight camera's zoom and leaving gives it back with the normal UI scale, so the drydock
//! and flight find the camera as they left it. UI words are chrome in code, like the drydock's; the shipyard's
//! place and name move to content once content has places.

use bevy::prelude::*;
use bevy::state::state::StateTransitionEvent;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale, UiTransform, Val2};
use bevy::window::PrimaryWindow;
use kit::style::{Style, ui_color};
use kit::ui_kit::{HudFonts, EXIT_SECONDS, Hot, ScreenFx, Tip, Tone, UiClick, show_screen, UiInputSet, UiPointer, UiSize, UiStyleSet, crumb, frame, label, prose, title, set_color, set_text, ui_scale_for};
use data::input::Action;
use world::camera::{CamMode, CameraRig, CameraSet};
use world::settings::{Act, ActionTip};
use world::career::{EscortDesigns, Granted, PlayerCareer, fleet_cores};
use world::fx::{FxFlush, FxQueue, FxRegions};
use world::mode::AppMode;
use world::player::PlayerDesign;
use world::evolution::Lab;
use world::scene::{Launch, Scene};
use world::sim_bridge::GameContent;

use crate::drydock::view::{DockCamera, GridViewport};
use crate::evolution::{self, EvolutionLayer};
use crate::fleet::{self, Fleet};
use crate::research::{self, ResearchView};
use crate::shipyard::fly::{Fly, View};
use crate::voyage_log::{self, LogRow, VoyageLog};
use crate::workshop::{self as workshop_layer, WorkshopLayer};
use crate::shipyard::draw::Evo;
use crate::shipyard::layout::{Align, Layout, Place, Tag, fit, fit_tagged};
use crate::shipyard::{ShipyardSet, Yard};

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
    /// The gate: the voyage map (`map/`).
    Voyages,
    /// The lab: the research screen.
    Research,
    /// The records mast: the LOG layer over home (`voyage_log.rs` opens it).
    Log,
    /// The compute stack: the EVOLUTION layer over home (`evolution/` opens it).
    Evolution,
    /// The workshop: the WORKSHOP layer over home (`workshop.rs` opens it).
    Workshop,
}

/// A place's action in the action map, if it has one (its tooltip names the bound keys while it works).
pub fn key_of(place: Place) -> Option<Action> {
    match place {
        Place::Drydock => Some(Action::OpenDrydock),
        Place::Behavior => Some(Action::OpenBehavior),
        Place::Research => Some(Action::OpenResearch),
        Place::Voyages => Some(Action::OpenVoyages),
        Place::TestRange => Some(Action::OpenTestRange),
        Place::Fleet => Some(Action::OpenFleet),
        Place::Log => Some(Action::OpenLog),
        Place::Evolution => Some(Action::OpenEvolution),
        Place::Workshop => Some(Action::OpenWorkshop),
    }
}

/// What a place opens; `None`: not yet (the dark buildings).
pub fn action_of(place: Place) -> Option<HomeAction> {
    match place {
        Place::Drydock => Some(HomeAction::Drydock),
        Place::Behavior => Some(HomeAction::Behavior),
        Place::TestRange => Some(HomeAction::TestRange),
        Place::Fleet => Some(HomeAction::Fleet),
        Place::Voyages => Some(HomeAction::Voyages),
        Place::Research => Some(HomeAction::Research),
        Place::Log => Some(HomeAction::Log),
        Place::Evolution => Some(HomeAction::Evolution),
        Place::Workshop => Some(HomeAction::Workshop),
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
        HomeAction::Research => "RESEARCH",
        // Until a voyage has ended (then the last one in one line, `log_tip`).
        HomeAction::Log => "LOG",
        HomeAction::Evolution => "EVOLUTION",
        HomeAction::Workshop => "WORKSHOP",
    };
    Some(Tip::new(name))
}

/// The place a screen belongs to: where the camera starts when that screen goes back home.
fn place_of(mode: AppMode) -> Option<Place> {
    match mode {
        AppMode::Drydock => Some(Place::Drydock),
        AppMode::Behavior => Some(Place::Behavior),
        AppMode::Flight => Some(Place::TestRange),
        AppMode::Research => Some(Place::Research),
        AppMode::Map => Some(Place::Voyages),
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
    /// Arrived at a place: its screen opens (FLEET and the LOG stay layers over home until they close).
    At(Place, View),
}

#[derive(Resource, Debug, Default)]
struct Visit {
    phase: Phase,
}

/// Home's UI root: the place name, the design's name and the notes (shown while the camera rests on the yard).
#[derive(Component)]
struct HomeRoot;

/// The labels over the world (shown while the camera rests on the yard).
#[derive(Component)]
struct LabelRoot;

/// The click areas over the places (while the camera rests on the yard).
#[derive(Component)]
struct SpotRoot;

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
            .add_systems(Startup, spawn.after(world::flight_hud::spawn_hud))
            .add_systems(PostStartup, remember_start_zoom.run_if(in_state(AppMode::Home)))
            .add_systems(OnEnter(AppMode::Home), enter)
            .add_systems(OnExit(AppMode::Home), exit)
            .add_systems(
                Update,
                (
                    ui_scale,
                    (clicks, keys).run_if(not(fleet::is_open).and_eager(not(crate::settings::is_open)).and_eager(not(voyage_log::is_open)).and_eager(not(evolution::is_open)).and_eager(not(workshop_layer::is_open))),
                    chip,
                    fly,
                    apply,
                    (place_labels, sync_titles, sync_name, sync_notes, log_tip, evolution_tip).run_if(not(fleet::is_open).and_eager(not(crate::settings::is_open)).and_eager(not(voyage_log::is_open)).and_eager(not(evolution::is_open)).and_eager(not(workshop_layer::is_open))),
                    measure_labels,
                    show,
                )
                    .chain()
                    .after(UiInputSet)
                    .before(UiStyleSet)
                    .before(ShipyardSet)
                    .run_if(in_state(AppMode::Home)),
            )
            .add_systems(Update, show.run_if(state_changed::<AppMode>.or_eager(resource_changed::<world::settings::SettingsLayer>).or_eager(resource_changed::<VoyageLog>).or_eager(resource_changed::<EvolutionLayer>).or_eager(resource_changed::<WorkshopLayer>)))
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
        Place::Log if yard.lights.log => Tone::Gold,
        Place::Log => Tone::Label,
        Place::Evolution => match yard.lights.evolution {
            Evo::Dark => Tone::Dim,
            Evo::Running => Tone::Teal,
            Evo::Done => Tone::Gold,
            Evo::Idle | Evo::Paused => Tone::Violet,
        },
        Place::Workshop if yard.lights.workshop => Tone::Gold,
        Place::Workshop => Tone::Teal,
        Place::Research | Place::Voyages => Tone::Dim,
    }
}

fn spawn(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let fonts = &*fonts;
    let full = Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() };
    // Home's words and the labels fade in when the camera comes to rest on the yard, and out when it leaves
    // (`ui_kit::ScreenFx`).
    commands.spawn((HomeRoot, ScreenFx::closed(), full.clone(), Visibility::Hidden)).with_children(|root| {
        // Top left (the frame's corner; the shipyard has no way back): where we are and the docked design's name.
        let f = frame();
        root.spawn(f.top_left()).with_children(|tl| {
            tl.spawn(f.heading()).with_children(|col| {
                col.spawn(crumb(fonts, "THE SHIPYARD  ·  RIM", &ui));
                col.spawn((DesignName, title(fonts, "", &ui)));
            });
        });
        // Top right: the EVOLVING chip while a run computes, SETTINGS (GDR-0090).
        root.spawn(f.top_right()).with_children(|tr| {
            evolution::spawn_chip(tr, fonts);
            crate::settings::spawn_ring(tr);
        });
        // Bottom right: the career's notes.
        root.spawn(Node {
            position_type: PositionType::Absolute,
            right: px(f.edge),
            bottom: px(f.top),
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
    // The click areas are invisible: they come and go at once (only while the camera rests on the yard).
    commands.spawn((SpotRoot, full.clone(), Visibility::Hidden)).with_children(|root| {
        for place in Place::ALL {
            if let Some(tip) = tip_of(place) {
                let spot = root.spawn((Spot(place), Hot::default(), tip, Node { position_type: PositionType::Absolute, ..default() })).id();
                if let Some(a) = key_of(place) {
                    root.commands().entity(spot).insert(ActionTip(a));
                }
            }
        }
    });
    commands.spawn((LabelRoot, ScreenFx::closed(), full, Visibility::Hidden)).with_children(|root| {
        for place in Place::ALL {
            let words = TextLayout::no_wrap();
            // Placed by their transforms (they follow the camera's flights while they fade).
            let at = Node { position_type: PositionType::Absolute, left: px(0), top: px(0), width: px(0), flex_direction: FlexDirection::Column, row_gap: px(LABEL_GAP), ..default() };
            root.spawn((PlaceLabel(place), at))
                .with_children(|l| {
                    l.spawn((LabelTitle(place), label(fonts, "", TITLE, c(Tone::Dim, 1.0)), words));
                    l.spawn((LabelSub(place), label(fonts, "", SUB, c(Tone::Label, 1.0)), words));
                });
        }
        root.spawn((
            PlateAnchor,
            Node { position_type: PositionType::Absolute, left: px(0), top: px(0), width: px(0), height: px(0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
        ))
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
/// off at some aspects). The labels hang where this layout puts them, so a view worked out for another frame than
/// the one the labels were last measured in (home entered after the drydock changed the frame) is the one the yard
/// settles on (19 Oct 2026: the flight out of the drydock ended in a jump the first time after a frame change).
fn overview(layout: &Layout, window: &Window, tags: &LabelTags) -> View {
    // Every place whole too (research's dishes reach above the sketch's overview).
    let all = Place::ALL.iter().fold(layout.overview, |r, p| r.union(layout.hit(*p)));
    let tags: Vec<Tag> = tags
        .0
        .iter()
        .map(|&(place, size)| {
            let (at, align) = layout.label(place);
            let x = match align {
                Align::Start => (0.0, size.x),
                Align::Middle => (-size.x / 2.0, size.x / 2.0),
                Align::End => (-size.x, 0.0),
            };
            Tag { at, min: Vec2::new(x.0, 0.0), max: Vec2::new(x.1, size.y) }
        })
        .collect();
    let (centre, zoom) = fit_tagged(all, &tags, window.size(), MARGIN);
    View { centre, zoom }
}

/// Each place's label size in logical px (measured from the laid-out labels; where it hangs comes from the layout).
#[derive(Resource, Debug, Default, PartialEq)]
struct LabelTags(Vec<(Place, Vec2)>);

/// Measures the labels (their texts change with the career, their size with the UI scale): written only when a
/// label's size changes by half a pixel or more.
fn measure_labels(
    window: Single<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    titles: Query<(&LabelTitle, &ComputedNode)>,
    subs: Query<(&LabelSub, &ComputedNode, &Node)>,
    mut tags: ResMut<LabelTags>,
) {
    let k = 1.0 / window.scale_factor();
    let new: Vec<(Place, Vec2)> = titles
        .iter()
        .map(|(t, title)| {
            let title = title.size * k;
            let sub = subs.iter().find(|(s, ..)| s.0 == t.0).filter(|(_, _, n)| n.display != Display::None).map(|(_, c, _)| c.size * k);
            let size = match sub {
                Some(s) => Vec2::new(title.x.max(s.x), title.y + LABEL_GAP * ui_scale.0 + s.y),
                None => title,
            };
            (t.0, size)
        })
        .collect();
    let same = tags.0.len() == new.len() && tags.0.iter().zip(&new).all(|(a, b)| a.0 == b.0 && a.1.distance(b.1) < 0.5);
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

/// Coming home: keep the flight zoom; the camera starts on the place of the screen we came from and flies out. Back
/// from a fight the lab's WATCH opened, it rests on the lab and the layer opens again; the drydock's EVOLVE THIS
/// DESIGN flies on to the lab.
#[allow(clippy::too_many_arguments)]
fn enter(
    mut transitions: MessageReader<StateTransitionEvent<AppMode>>,
    lab: Res<Lab>,
    mut out: MessageWriter<HomeAction>,
    debrief: Res<world::voyage::debrief::Debrief>,
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
    // From the map the portal has brought the camera back out to the yard.
    let from = if exited == Some(AppMode::Flight) && debrief.open { Some(Place::Voyages) } else { exited.filter(|m| *m != AppMode::Map).and_then(place_of) };
    let yard_view = overview(&yard.layout, &window, &tags);
    let lab_view = || place_view(Place::Evolution, &yard.layout, &window, &dock, None);
    if exited == Some(AppMode::Flight) && lab.watching {
        let at = lab_view();
        put_rig(&mut rig, at, window.size());
        visit.phase = Phase::At(Place::Evolution, at);
        out.write(HomeAction::Evolution);
        return;
    }
    if exited == Some(AppMode::Drydock) && lab.request.is_some() {
        let start = place_view(Place::Drydock, &yard.layout, &window, &dock, viewport.single().ok());
        put_rig(&mut rig, start, window.size());
        visit.phase = Phase::Flying { fly: Fly::new(start, lab_view()).after(EXIT_SECONDS), to: Some(Place::Evolution), from: Some(Place::Drydock) };
        return;
    }
    visit.phase = match from {
        Some(place) => {
            let start = place_view(place, &yard.layout, &window, &dock, viewport.single().ok());
            put_rig(&mut rig, start, window.size());
            // A screen closes over its place first (its transition, `ui_kit::ScreenFx`); flight's HUD snaps shut.
            let wait = if exited == Some(AppMode::Flight) { 0.0 } else { EXIT_SECONDS };
            Phase::Flying { fly: Fly::new(start, yard_view).after(wait), to: None, from: Some(place) }
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
fn exit(mut rig: ResMut<CameraRig>, cam: Res<HomeCamera>, mut ui_scale: ResMut<UiScale>, size: Res<UiSize>) {
    rig.view_offset = Vec2::ZERO;
    rig.zoom = cam.flight_zoom;
    rig.zoom_target = cam.flight_zoom;
    ui_scale.0 = size.0;
}

/// The UI scale: the labels are made for 1280 × 720 logical pixels and grow with larger windows (as the drydock's,
/// whose layout home's camera flies onto). On a phone the EVOLUTION layer, which has a phone layout, takes the
/// phone's scale while it is open (docs/client.md "Phones").
fn ui_scale(window: Single<&Window, With<PrimaryWindow>>, size: Res<UiSize>, class: Res<kit::ui_kit::LayoutClass>, lab: Res<crate::evolution::EvolutionLayer>, mut ui_scale: ResMut<UiScale>) {
    let phone = class.is_compact() && lab.open;
    let s = if phone { kit::ui_kit::compact_scale(window.size()) * size.0 } else { ui_scale_for(&window, *size) };
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
    // The gate's flight goes nowhere: the map's portal takes the camera from here.
    let to = if place == Place::Voyages { from } else { place_view(place, &yard.layout, window, dock, viewport) };
    visit.phase = Phase::Flying { fly: Fly::new(from, to), to: Some(place), from: None };
}

/// Where going to a place leads: the locked EVOLUTION lab leads to RESEARCH, on the node that opens it.
fn unless_locked(place: Place, lab: &Lab, content: &data::Content, research: &mut ResearchView) -> Place {
    if place != Place::Evolution || lab.enabled {
        return place;
    }
    research.request = evolution::unlocked_by(content).map(|n| n.id.clone());
    Place::Research
}

/// Clicks and taps on the places (the locked EVOLUTION lab's lead to RESEARCH).
#[allow(clippy::too_many_arguments)]
fn clicks(
    mut clicks: MessageReader<UiClick>,
    lab: Res<Lab>,
    content: Res<GameContent>,
    mut research: ResMut<ResearchView>,
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
            let place = unless_locked(spot.0, &lab, &content.0, &mut research);
            go(place, &mut visit, &yard, &window, &rig, &dock, viewport.single().ok());
        }
    }
}

/// The EVOLVING chip (`evolution::Chip`, at home and on the layers over it) opens the lab: SETTINGS closes (the camera
/// rests on the yard meanwhile) and the camera flies to the lab, from the yard or from FLEET's or the LOG's place as
/// their layer closes.
#[allow(clippy::too_many_arguments)]
fn chip(
    mut clicks: MessageReader<UiClick>,
    chips: Query<(), With<evolution::Chip>>,
    (mut fleet, mut log, mut settings): (ResMut<Fleet>, ResMut<VoyageLog>, ResMut<world::settings::SettingsLayer>),
    window: Single<&Window, With<PrimaryWindow>>,
    dock: Res<DockCamera>,
    viewport: Query<(&ComputedNode, &UiGlobalTransform), With<GridViewport>>,
    yard: Res<Yard>,
    rig: Res<CameraRig>,
    mut visit: ResMut<Visit>,
) {
    if !clicks.read().any(|c| chips.contains(c.0)) {
        return;
    }
    settings.open = false;
    match visit.phase {
        Phase::At(from @ (Place::Fleet | Place::Log), view) => {
            fleet.open = false;
            log.open = false;
            let to = place_view(Place::Evolution, &yard.layout, &window, &dock, None);
            visit.phase = Phase::Flying { fly: Fly::new(view, to).after(EXIT_SECONDS), to: Some(Place::Evolution), from: Some(from) };
        }
        _ => go(Place::Evolution, &mut visit, &yard, &window, &rig, &dock, viewport.single().ok()),
    }
}

/// The places' actions (D, B, T, F, R, V, L, E by default; E on the locked lab leads to RESEARCH): not while a text
/// field has the keyboard (the action map keeps them apart from Ctrl and Cmd combinations: Ctrl+D is the browser's
/// bookmark key). The debrief's VOYAGE MAP acts as the voyages' key.
#[allow(clippy::too_many_arguments)]
fn keys(
    act: Act,
    lab: Res<Lab>,
    content: Res<GameContent>,
    mut research: ResMut<ResearchView>,
    pointer: Res<UiPointer>,
    window: Single<&Window, With<PrimaryWindow>>,
    dock: Res<DockCamera>,
    viewport: Query<(&ComputedNode, &UiGlobalTransform), With<GridViewport>>,
    yard: Res<Yard>,
    rig: Res<CameraRig>,
    mut visit: ResMut<Visit>,
    mut open_map: MessageReader<crate::map::OpenMap>,
) {
    if open_map.read().count() > 0 {
        go(Place::Voyages, &mut visit, &yard, &window, &rig, &dock, viewport.single().ok());
    }
    if pointer.typing {
        return;
    }
    for place in Place::ALL {
        if key_of(place).is_some_and(|a| act.just_pressed(a)) {
            let place = unless_locked(place, &lab, &content.0, &mut research);
            go(place, &mut visit, &yard, &window, &rig, &dock, viewport.single().ok());
        }
    }
}

/// The camera: on the yard it shows the whole station (fitted to the window); in flight it follows the flight
/// (time-based), and the rigging makes room as it flies into the drydock (or comes back out); on arrival at a
/// place its screen opens. FLEET's moorings and the LOG's mast: when their layer closes, the camera flies back out.
#[allow(clippy::too_many_arguments)]
fn fly(
    time: Res<Time<Real>>,
    window: Single<&Window, With<PrimaryWindow>>,
    (fleet, log, evo, ws): (Res<Fleet>, Res<VoyageLog>, Res<EvolutionLayer>, Res<WorkshopLayer>),
    tags: Res<LabelTags>,
    mut yard: ResMut<Yard>,
    mut rig: ResMut<CameraRig>,
    mut visit: ResMut<Visit>,
    mut out: MessageWriter<HomeAction>,
) {
    let size = window.size();
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
            let closed = match *place {
                Place::Fleet => !fleet.open,
                Place::Log => !log.open,
                Place::Evolution => !evo.open,
                Place::Workshop => !ws.open,
                _ => false,
            };
            if closed {
                // The layer closed: back out to the yard.
                next = Some(Phase::Flying { fly: Fly::new(*view, overview(&yard.layout, &window, &tags)).after(EXIT_SECONDS), to: None, from: Some(*place) });
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
) {
    for a in actions.read() {
        match a {
            HomeAction::Drydock => next.set(AppMode::Drydock),
            HomeAction::Behavior => next.set(AppMode::Behavior),
            HomeAction::Research => next.set(AppMode::Research),
            HomeAction::TestRange => {
                scene.launch = Some(Launch::Range);
                next.set(AppMode::Flight);
            }
            // Layers over home, not modes: `fleet::open`, `voyage_log::open`, `evolution::open` and `workshop::open`
            // read the same message.
            HomeAction::Fleet | HomeAction::Log | HomeAction::Evolution | HomeAction::Workshop => {}
            HomeAction::Voyages => next.set(AppMode::Map),
        }
    }
}

/// Home's screen shows at home while no layer is over it (FLEET, SETTINGS, the LOG, EVOLUTION, the WORKSHOP); the labels while the
/// camera rests on the yard. While the voyage's report shows they step aside too: its backing is a world mesh (under its
/// part icons), which home's words would read through.
#[allow(clippy::type_complexity)]
fn show(
    mode: Res<State<AppMode>>,
    (fleet, log, evo, ws): (Res<Fleet>, Res<VoyageLog>, Res<EvolutionLayer>, Res<WorkshopLayer>),
    settings: Res<world::settings::SettingsLayer>,
    visit: Res<Visit>,
    debrief: Res<world::voyage::debrief::Debrief>,
    mut screens: Query<&mut ScreenFx, Or<(With<HomeRoot>, With<LabelRoot>)>>,
    mut spots: Single<&mut Visibility, With<SpotRoot>>,
) {
    let home = *mode.get() == AppMode::Home && !fleet.open && !settings.open && !log.open && !evo.open && !ws.open;
    let report = debrief.open && debrief.data.is_some();
    let resting = home && matches!(visit.phase, Phase::Yard) && !report;
    for mut fx in &mut screens {
        show_screen(&mut fx, resting);
    }
    let want = if resting { Visibility::Inherited } else { Visibility::Hidden };
    if **spots != want {
        **spots = want;
    }
}

/// Labels and the name plate follow the camera, during its flights too, by their transforms (no layout); the click
/// areas (sized) and the plate's letters only while it rests on the yard (UI px = logical px / the UI scale).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn place_labels(
    yard: Res<Yard>,
    rig: Res<CameraRig>,
    ui_scale: Res<UiScale>,
    visit: Res<Visit>,
    mut labels: Query<(&PlaceLabel, &mut Node, &mut UiTransform), Without<PlateAnchor>>,
    mut anchor: Single<&mut UiTransform, (With<PlateAnchor>, Without<PlaceLabel>)>,
    mut spots: Query<(&Spot, &mut Node), Without<PlaceLabel>>,
    mut plate: Single<&mut TextFont, With<PlateName>>,
) {
    let k = ui_scale.0.max(1e-3);
    let at = |w: Vec2| rig.world_to_screen(w) / k;
    let l = &yard.layout;
    let put = |tf: &mut UiTransform, p: Vec2, scale: f32| {
        let (t, s) = (Val2::px(p.x, p.y), Vec2::splat(scale));
        if tf.translation != t || tf.scale != s {
            tf.translation = t;
            tf.scale = s;
        }
    };
    for (label, mut node, mut tf) in &mut labels {
        let (anchor, align) = l.label(label.0);
        let items = match align {
            Align::Start => AlignItems::FlexStart,
            Align::Middle => AlignItems::Center,
            Align::End => AlignItems::FlexEnd,
        };
        if node.align_items != items {
            node.align_items = items;
        }
        put(&mut tf, at(anchor), 1.0);
    }
    // The plate's letters are painted on the station: their size follows the zoom (by the transform's scale while
    // the camera flies, by the font once it rests).
    let letters = (PLATE_LETTERS * rig.zoom / k).max(4.0);
    let resting = matches!(visit.phase, Phase::Yard);
    if resting && plate.font_size != FontSize::Px(letters) {
        plate.font_size = FontSize::Px(letters);
    }
    let set_px = match plate.font_size {
        FontSize::Px(p) => p.max(1e-3),
        _ => letters,
    };
    put(&mut anchor, at(l.name_plate().center()), letters / set_px);
    if !resting {
        return;
    }
    let set = |node: &mut Node, left: Val, top: Val, size: Vec2| {
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
        if node.width != px(size.x) || node.height != px(size.y) {
            node.width = px(size.x);
            node.height = px(size.y);
        }
    };
    for (spot, mut node) in &mut spots {
        let r = l.hit(spot.0);
        let (a, b) = (at(Vec2::new(r.min.x, r.max.y)), at(Vec2::new(r.max.x, r.min.y)));
        set(&mut node, px(a.x), px(a.y), b - a);
    }
}

/// Titles and the lines under them: the frame's name in the building area's, FLEET's cores, what research can
/// pay for now, what waits in the LOG, the lit and dark buildings; a hovered place's title brightens.
#[allow(clippy::too_many_arguments)]
fn sync_titles(
    yard: Res<Yard>,
    design: Res<PlayerDesign>,
    escorts: Res<EscortDesigns>,
    granted: Res<Granted>,
    career: Res<PlayerCareer>,
    lab: Res<Lab>,
    content: Res<GameContent>,
    style: Res<Style>,
    spots: Query<(&Spot, Ref<Hot>)>,
    mut titles: Query<(&LabelTitle, &mut Text, &mut TextColor), Without<LabelSub>>,
    mut subs: Query<(&LabelSub, &mut Text, &mut TextColor, &mut Node), Without<LabelTitle>>,
) {
    let hover = spots.iter().any(|(_, h)| h.is_changed());
    if !(hover || yard.is_changed() || design.is_changed() || escorts.is_changed() || granted.is_changed() || career.is_changed() || lab.is_changed()) {
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
            Place::Log => "LOG".into(),
            Place::Evolution => content.0.facilities.get(world::evolution::FACILITY).map_or_else(|| world::evolution::FACILITY.to_uppercase(), |f| f.name.to_uppercase()),
            Place::Workshop => content.0.workshop().map_or_else(|| "WORKSHOP".to_string(), |(f, _)| f.name.to_uppercase()),
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
            Place::Research => match research::tree::ready(&content.0, career.career()).len() {
                0 if yard.lights.research => ("NOTHING READY".into(), Tone::Label),
                0 => ("NO DATA YET".into(), Tone::Dim),
                n => (format!("{n} READY"), Tone::Gold),
            },
            Place::Voyages => {
                let open = world::voyage::default_voyage(career.career(), &content.0);
                match open.and_then(|v| content.0.voyages.get(v)) {
                    Some(v) => (format!("THE GATE  ·  {}", v.name.to_uppercase()), Tone::Teal),
                    None => ("DORMANT".into(), Tone::Dim),
                }
            }
            Place::Log => {
                let c = career.career();
                let logs = c.exploration().map_or(0, |g| g.logs().len());
                match (c.unread(), c.last_voyage()) {
                    _ if logs > 0 => (format!("{logs} {} TO DECODE", if logs == 1 { "LOG" } else { "LOGS" }), Tone::Gold),
                    (_, None) => ("NO VOYAGES YET".into(), Tone::Dim),
                    (0, Some(_)) => (plural(c.history().len(), "VOYAGE"), Tone::Label),
                    (1, Some(last)) => (format!("NEW  ·  {}", LogRow::of(&content.0, last).name.to_uppercase()), Tone::Gold),
                    (n, Some(_)) => (format!("{n} NEW"), Tone::Gold),
                }
            }
            Place::Evolution => evolution::building_line(&lab),
            Place::Workshop => {
                let c = career.career();
                let need = data::career::fleet_parts(&content.0, std::iter::once(design.blueprint()).chain(&escorts.0));
                match workshop_layer::missing(c, &need).iter().map(|(_, n, have)| n - have).sum::<u32>() {
                    0 => (format!("STOCK  ·  {} PARTS", c.stock().values().sum::<u32>()), Tone::Label),
                    n => (format!("FLEET SHORT OF {n}"), Tone::Coral),
                }
            }
        };
        set_text(&mut text, &words);
        set_color(&mut color, ui_color(tone.color(ui), 1.0));
        let display = if words.is_empty() { Display::None } else { Display::Flex };
        if node.display != display {
            node.display = display;
        }
    }
}

fn plural(n: usize, one: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {one}S") }
}

/// The LOG's tooltip: the last voyage in one line (what sat over home before the LOG, GDR-0091).
fn log_tip(career: Res<PlayerCareer>, content: Res<GameContent>, mut spots: Query<(&Spot, &mut Tip)>) {
    if !career.is_changed() {
        return;
    }
    let line = career.career().last_voyage().map_or_else(|| "LOG".to_string(), |v| LogRow::of(&content.0, v).line());
    for (spot, mut tip) in &mut spots {
        if spot.0 == Place::Log && tip.name != line {
            tip.name = line.clone().into();
        }
    }
}

/// The EVOLUTION lab's tooltip: its name, or while it is locked the research node that opens it.
fn evolution_tip(lab: Res<Lab>, content: Res<GameContent>, mut spots: Query<(&Spot, &mut Tip)>) {
    if !lab.is_changed() {
        return;
    }
    let words = match evolution::unlocked_by(&content.0).filter(|_| !lab.enabled) {
        Some(node) => format!("LOCKED  ·  RESEARCH {}", node.name.to_uppercase()),
        None => "EVOLUTION".into(),
    };
    for (spot, mut tip) in &mut spots {
        if spot.0 == Place::Evolution && tip.name != words {
            tip.name = words.clone().into();
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
    visit: Res<Visit>,
    yard: Res<Yard>,
    rig: Res<CameraRig>,
    style: Res<Style>,
    regions: Option<Res<FxRegions>>,
    mut queue: ResMut<FxQueue>,
) {
    let Some(r) = regions else { return };
    if !matches!(visit.phase, Phase::Yard) {
        return;
    }
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
    use world::scene::SceneKind;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .insert_state(AppMode::Home)
            .insert_resource(Scene::new(SceneKind::Sandbox, 1))
            .add_message::<HomeAction>()
            .add_systems(Update, apply);
        app
    }

    /// The locked lab leads to RESEARCH on the node that opens it; once open it is a place of its own.
    #[test]
    fn the_locked_lab_leads_to_its_research() {
        let content = data::base_content();
        let mut research = ResearchView::default();
        let mut lab = Lab::default();
        assert_eq!(unless_locked(Place::Evolution, &lab, &content, &mut research), Place::Research);
        assert_eq!(research.request.as_deref(), Some("simulation_vault"));
        research.request = None;
        lab.enabled = true;
        assert_eq!(unless_locked(Place::Evolution, &lab, &content, &mut research), Place::Evolution);
        assert_eq!(unless_locked(Place::Fleet, &Lab::default(), &content, &mut research), Place::Fleet);
        assert_eq!(research.request, None);
    }

    /// TEST RANGE asks for the plain sandbox (not a test flight) and flies; DRYDOCK, BEHAVIOR and the gate (the voyage map)
    /// open their screens.
    #[test]
    fn the_places_lead_to_their_modes() {
        let mut app = app();
        app.world_mut().write_message(HomeAction::TestRange);
        app.update();
        assert_eq!(app.world().resource::<Scene>().launch, Some(Launch::Range));
        app.update();
        assert_eq!(*app.world().resource::<State<AppMode>>().get(), AppMode::Flight);
        for (action, mode) in [
            (HomeAction::Drydock, AppMode::Drydock),
            (HomeAction::Behavior, AppMode::Behavior),
            (HomeAction::Research, AppMode::Research),
            (HomeAction::Voyages, AppMode::Map),
        ] {
            let mut app = self::app();
            app.world_mut().write_message(action);
            app.update();
            app.update();
            assert_eq!(*app.world().resource::<State<AppMode>>().get(), mode);
            assert_eq!(app.world().resource::<Scene>().launch, None);
        }
    }

    /// D, B, T, F, R, V, L and E lead to the drydock, the BEHAVIOR screen, the test range, FLEET, research, the voyage
    /// map, the LOG and the EVOLUTION lab; every key is one place's, every action one place's.
    #[test]
    fn keys_and_places_map_to_screens() {
        let by_key = |k: Action| Place::ALL.into_iter().find(|p| key_of(*p) == Some(k));
        let screen = |k: Action| by_key(k).and_then(action_of);
        assert_eq!(screen(Action::OpenDrydock), Some(HomeAction::Drydock));
        assert_eq!(screen(Action::OpenBehavior), Some(HomeAction::Behavior));
        assert_eq!(screen(Action::OpenTestRange), Some(HomeAction::TestRange));
        assert_eq!(screen(Action::OpenFleet), Some(HomeAction::Fleet));
        assert_eq!(screen(Action::OpenVoyages), Some(HomeAction::Voyages));
        assert_eq!(screen(Action::OpenLog), Some(HomeAction::Log));
        assert_eq!(screen(Action::OpenEvolution), Some(HomeAction::Evolution));
        assert_eq!(screen(Action::OpenWorkshop), Some(HomeAction::Workshop));
        assert_eq!((by_key(Action::OpenResearch), screen(Action::OpenResearch)), (Some(Place::Research), Some(HomeAction::Research)));
        for p in Place::ALL {
            if let Some(k) = key_of(p) {
                assert_eq!(by_key(k), Some(p), "{p:?}");
            }
            if let Some(a) = action_of(p) {
                assert_eq!(Place::ALL.iter().filter(|q| action_of(**q) == Some(a)).count(), 1, "{a:?}");
                assert!(key_of(p).is_some(), "{p:?} works but has no key");
            }
        }
        // Every screen goes back home onto its place.
        for (mode, place) in [
            (AppMode::Drydock, Place::Drydock),
            (AppMode::Behavior, Place::Behavior),
            (AppMode::Flight, Place::TestRange),
            (AppMode::Research, Place::Research),
            (AppMode::Map, Place::Voyages),
        ] {
            assert_eq!(place_of(mode), Some(place));
        }
    }

    /// A working place starts a flight there (its screen opens when the camera arrives); another one during the
    /// flight changes nothing; the inert places don't fly, the lab does.
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
        let mut rest = Visit::default();
        go(Place::Research, &mut rest, &yard, &window, &rig, &dock, None);
        assert!(matches!(rest.phase, Phase::Flying { to: Some(Place::Research), .. }));
        // The gate's flight stays where it is and arrives at once (the voyage map's portal takes the camera).
        let mut gate = Visit::default();
        go(Place::Voyages, &mut gate, &yard, &window, &rig, &dock, None);
        let Phase::Flying { mut fly, to: Some(Place::Voyages), .. } = gate.phase else { panic!("not flying: {:?}", gate.phase) };
        assert_eq!(fly.step(0.0), (View { centre: rig.pos, zoom: rig.zoom }, true));
    }

    /// The yard's view hangs the measured labels where the layout it is for puts them: entering home after the
    /// drydock changed the frame, the flight out ends on the view the yard then keeps (the labels were last measured
    /// at home in the old frame; hung on the old places, the flight ended in a jump to the new view).
    #[test]
    fn the_yard_view_hangs_the_labels_on_its_own_layout() {
        let window = Window::default();
        let tags = LabelTags(Place::ALL.iter().map(|p| (*p, Vec2::new(150.0, 30.0))).collect());
        let hung = |on: &Layout, view: &Layout| {
            let all = Place::ALL.iter().fold(view.overview, |r, p| r.union(view.hit(*p)));
            let placed: Vec<Tag> = tags
                .0
                .iter()
                .map(|&(p, size)| {
                    let (at, align) = on.label(p);
                    let x = match align {
                        Align::Start => (0.0, size.x),
                        Align::Middle => (-size.x / 2.0, size.x / 2.0),
                        Align::End => (-size.x, 0.0),
                    };
                    Tag { at, min: Vec2::new(x.0, 0.0), max: Vec2::new(x.1, size.y) }
                })
                .collect();
            fit_tagged(all, &placed, window.size(), MARGIN)
        };
        let (frigate, cruiser) = (Layout::new((24, 24)), Layout::new((40, 40)));
        let view = overview(&cruiser, &window, &tags);
        assert_eq!((view.centre, view.zoom), hung(&cruiser, &cruiser));
        assert_ne!((view.centre, view.zoom), hung(&frigate, &cruiser), "the old frame's label places give another view");
    }
}
