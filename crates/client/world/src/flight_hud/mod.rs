//! The flight HUD in the reference look (the concept's phone HUD mockup): floating text,
//! wide-tracked uppercase labels in Barlow, white numbers, no boxes.
//!
//! * top left: the home ring (what M does: home, for touch), hull % and a tiny hull map of the player's ship (in a
//!   behavior test's watch mode: the watched one), and its hold (`CARGO 3 / 8`, salvage collected / cargo units;
//!   hidden for a ship without a hold);
//! * left: speed (m/s), the top speed and the drift state; under it the pilot assists of the ship flown, with their
//!   toggle keys and state (`assists.rs`, GDR-0088);
//! * top right: target panel (name, hull %, distance; in watch mode the watched ship's behavior target);
//! * top centre: scene / wave line and whether raider waves come, centre: banners (wave cleared, ship lost,
//!   `PAUSED`);
//! * bottom centre: heat and power warnings ("PULSE LASER 2 · HEAT 96 %"); bottom: the controls help line;
//! * in the world: a dotted lock ring around the target, off-screen markers for enemies (also in watch mode; kept
//!   clear of the HUD's texts and widgets).
//! * **voyages** (GDR-0065): the title is the voyage and its band, the
//!   subtitle `SITES 2 / 5 · 7:05`; under the hull the cargo block (`SALVAGE`, `5 / 8` with one pip per cargo unit,
//!   the hold's name) and right under it the RECALL pill (R; enabled only while the sim allows the recall) with
//!   what it does; markers at the screen edge for the sites off screen (name and distance: raiders coral, wrecks
//!   gold, the goal brighter, cleared ones dim); banners for sites waking and cleared and for the goal; the
//!   warp-out's countdown (`EMERGENCY WARP · 2` in gold for the emergency recall); `SHIP LOST · RECALLED HOME` with
//!   its countdown (`voyage-hud-lost.png`; `VOYAGE ABANDONED` after abandoning). No home ring: M opens the ABANDON
//!   VOYAGE? panel instead (GDR-0077), a root of its own low under the ship (the flight goes on while it is open),
//!   that slides up from the bottom: what stays behind, ABANDON VOYAGE (coral; Enter) and KEEP FLYING (Esc or M
//!   again). At a voyage's end the HUD closes with its transition, its world markers fading with it (`warp_fx.rs`).
//!
//! Text is rebuilt at 8 Hz and only re-laid out when it changes.

mod assists;
mod markers;
mod menu;
mod text;

use std::fmt::Write as _;
use std::sync::Arc;

use bevy::prelude::*;
use sim::{ShipDesign, ShipId, Stat, Target};

use bevy::ui::{ComputedNode, UiGlobalTransform};
use kit::params::RenderToggles;
use kit::style::{Style, ui_color};
use kit::ui_icons::{Glyph, UiIcon};
use kit::ui_kit::{Edge, Hot, HudFonts, Pill, PillText, ScreenFx, ScreenPart, Tip, Tone, show_screen, UiClick, UiInputSet, UiStyleSet, label, load_fonts, number, prose, ring_node, set_color, set_text, spawn_pill, tracked};

use crate::camera::{CameraRig, CameraSet};
use crate::controls::PlayerInput;
use crate::fx::{FxQueue, FxRegions, Z_HUD};
use crate::mode::{AppMode, FlightUi};
use crate::scene::{Scene, SceneKind};
use crate::settings::ActionTip;
use data::input::Action;
use crate::sim_bridge::{PilotFocus, SimWorld, piece_com, v2};
use crate::test_range::{BehaviorWatch, TestMode};
use crate::voyage::{self, Phase, VoyageRun};
use crate::warp_fx::Transit;
use crate::ship_view::PieceView;

use markers::{lock_ring, offscreen_markers, site_markers};
pub use assists::{assist_name, switch_note};
pub use menu::FlightMenu;
pub use text::hull_fraction;
use text::update_text;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum HudText {
    HullValue,
    Cargo,
    SpeedValue,
    SpeedSub,
    TargetName,
    TargetValue,
    TargetSub,
    Title,
    Subtitle,
    Banner,
    BannerSub,
    Warnings,
    Help,
    /// Voyages: the cargo block's number and the hold's name (or what became of the cargo).
    SalvageValue,
    SalvageSub,
    /// Voyages: what RECALL does now.
    RecallSub,
    /// Voyages: the lost banner (`voyage-hud-lost.png`).
    LostTitle,
    LostSub,
    LostNote,
    LostCount,
    LostHome,
}

/// The voyage's cargo block and RECALL (shown in voyages).
#[derive(Component)]
struct VoyageBlock;

/// One cargo unit's pip under the salvage number.
#[derive(Component)]
struct CargoPip(u32);

/// Pips shown at most (a bigger hold shows its share).
const PIPS: u32 = 20;

/// The RECALL pill.
#[derive(Component)]
struct RecallButton;

/// The lost banner (shown while a lost voyage counts down).
#[derive(Component)]
struct LostBlock;

/// The ABANDON VOYAGE? panel's root (GDR-0077; open while `VoyageRun::asking`).
#[derive(Component)]
struct AbandonRoot;

/// The panel's buttons.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum AbandonButton {
    Abandon,
    KeepFlying,
}

/// The panel's line of what stays behind.
#[derive(Component)]
struct AbandonCargo;

/// A site marker's label (name and distance), placed at the screen edge by `site_markers`.
#[derive(Component)]
struct SiteLabel(usize);

/// A line of a site marker's label: 0 the name, 1 the distance.
#[derive(Component)]
struct SiteLabelText(usize, u8);

/// Site labels in the pool.
const SITE_LABELS: usize = 12;

#[derive(Component)]
struct HudRoot;

#[derive(Component)]
struct HullMap;

#[derive(Component)]
struct HullCell(u32);

/// The ring top left that goes home (what M does), for touch.
#[derive(Component)]
struct HomeRing;

/// A flight widget that takes clicks and taps (the home ring): they never reach the world (`controls`).
#[derive(Component)]
pub struct HudButton;

/// A flight UI rectangle the off-screen markers keep clear of (besides the HUD's texts).
#[derive(Component)]
pub struct HudZone;

/// The laid-out [`HudButton`]s.
pub type HudButtons<'w, 's> =
    Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform, &'static InheritedVisibility), With<HudButton>>;

/// Is this logical window point on a visible [`HudButton`]?
pub fn on_hud_button(buttons: &HudButtons, p: Vec2) -> bool {
    buttons.iter().any(|(node, tf, vis)| vis.get() && logical_rect(node, tf).is_some_and(|r| r.contains(p)))
}

/// A laid-out node's rectangle in logical window pixels (`None` while it has no size).
fn logical_rect(node: &ComputedNode, tf: &UiGlobalTransform) -> Option<Rect> {
    let s = node.inverse_scale_factor;
    node.size.cmpgt(Vec2::ZERO).all().then(|| Rect::from_center_size(tf.translation * s, node.size * s))
}

/// Off-screen markers: inset from the window edge, and clearance around HUD rectangles (logical px).
const MARKER_INSET: f32 = 34.0;
const MARKER_CLEARANCE: f32 = 14.0;

#[derive(Resource)]
struct HudState {
    timer: Timer,
    /// The ship and design the hull map shows.
    map_for: BuiltFor,
    /// Display label per part of the shown ship's design ("PULSE LASER 2") and the design they were made for.
    labels: Vec<String>,
    labels_for: Option<Arc<ShipDesign>>,
    scratch: String,
}

/// What the hull map was built for: a ship and its compiled design. Every new sim numbers its ships from 0 (the
/// player's first ship is always `ShipId(0)`), so the id alone can't tell a test flight of an edited design from
/// the last flight; the design is held, so a later design can't take its address.
#[derive(Default)]
struct BuiltFor(Option<(ShipId, Arc<ShipDesign>)>);

impl BuiltFor {
    /// Was it built for this ship as it is (or for no ship, when there is none)?
    fn matches(&self, ship: Option<&sim::Ship>) -> bool {
        match (&self.0, ship) {
            (None, None) => true,
            (Some((id, design)), Some(s)) => *id == s.id() && Arc::ptr_eq(design, s.design()),
            _ => false,
        }
    }

    fn set(&mut self, ship: Option<&sim::Ship>) {
        self.0 = ship.map(|s| (s.id(), s.design().clone()));
    }
}

pub struct FlightHudPlugin;

impl Plugin for FlightHudPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(HudState {
            timer: Timer::from_seconds(0.125, TimerMode::Repeating),
            map_for: BuiltFor::default(),
            labels: Vec::new(),
            labels_for: None,
            scratch: String::with_capacity(256),
        })
        .init_resource::<FlightMenu>()
        .add_systems(Startup, (spawn_hud, spawn_abandon_panel, menu::spawn_menu).chain().after(load_fonts))
        .add_systems(Update, menu::flight_menu.after(UiInputSet).before(UiStyleSet).run_if(in_state(AppMode::Flight)))
        .add_systems(OnExit(AppMode::Flight), menu::close_menu)
        .add_systems(Update, (update_text, update_hull_map, assists::update).chain().run_if(in_state(AppMode::Flight)))
        .add_systems(Update, (go_home, recall_click).after(UiInputSet).run_if(in_state(AppMode::Flight)))
        .add_systems(Update, abandon_panel.after(UiInputSet).before(UiStyleSet).run_if(in_state(AppMode::Flight)))
        .add_systems(Update, voyage_widgets.after(UiInputSet).before(UiStyleSet).run_if(in_state(AppMode::Flight)))
        .add_systems(OnExit(AppMode::Flight), forget_ship)
        .add_systems(PostUpdate, (lock_ring, offscreen_markers, site_markers).after(CameraSet).before(crate::fx::FxFlush));
    }
}

pub fn spawn_hud(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>) {
    let ui = &style.ui;
    let grey = ui_color(ui.label, 1.0);
    let white = ui_color(ui.number, 1.0);
    let coral = ui_color(ui.coral, 1.0);
    let teal = ui_color(ui.teal, 1.0);
    let gold = ui_color(ui.gold, 1.0);
    let dim = ui_color(ui.dim, 1.0);
    let abs = |l: Option<f32>, r: Option<f32>, t: Option<f32>, b: Option<f32>| Node {
        position_type: PositionType::Absolute,
        left: l.map(px).unwrap_or(Val::Auto),
        right: r.map(px).unwrap_or(Val::Auto),
        top: t.map(px).unwrap_or(Val::Auto),
        bottom: b.map(px).unwrap_or(Val::Auto),
        flex_direction: FlexDirection::Column,
        ..default()
    };

    commands
        .spawn((
            HudRoot,
            FlightUi,
            // Slides in when flight starts (`ui_kit::ScreenFx`); H hides it the same way. Leaving flight snaps it shut.
            ScreenFx::closed(),
            Node { width: percent(100), height: percent(100), position_type: PositionType::Absolute, ..default() },
        ))
        .with_children(|root| {
            // Top left: the home ring (as on the drydock and the BEHAVIOR screen), then the hull.
            root.spawn((ScreenPart::new(Edge::Left, 0), abs(Some(20.0), None, Some(18.0), None))).with_children(|c| {
                c.spawn((HomeRing, HudButton, HudZone, ring_node(34.0), Tip::new("SHIPYARD"), ActionTip(Action::Leave))).with_children(|r| {
                    r.spawn((UiIcon::Glyph { glyph: Glyph::Menu, color: ui.violet }, Node { width: px(19), height: px(19), ..default() }));
                });
            });
            root.spawn((ScreenPart::new(Edge::Left, 0), abs(Some(68.0), None, Some(18.0), None))).with_children(|c| {
                c.spawn(label(&fonts, "HULL", 13.0, grey));
                c.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(18), ..default() })
                    .with_children(|row| {
                        row.spawn((HudText::HullValue, number(&fonts, "–", 40.0, white)));
                        row.spawn((HullMap, HudZone, Node { width: px(100), height: px(60), position_type: PositionType::Relative, ..default() }));
                    });
                c.spawn((HudText::Cargo, label(&fonts, "", 12.0, grey)));
                // Voyages: the cargo block, RECALL right under it (GDR-0065).
                c.spawn((VoyageBlock, Visibility::Hidden, Node { flex_direction: FlexDirection::Column, row_gap: px(5), margin: UiRect::top(px(2)), ..default() }))
                    .with_children(|v| {
                        v.spawn((HudText::SalvageValue, number(&fonts, "", 28.0, gold)));
                        v.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(3), ..default() }).with_children(|row| {
                            for k in 0..PIPS {
                                row.spawn((CargoPip(k), Node { width: px(11), height: px(3), ..default() }, BackgroundColor(Color::NONE)));
                            }
                        });
                        v.spawn((HudText::SalvageSub, label(&fonts, "", 10.0, dim)));
                        v.spawn(Node { height: px(16), ..default() });
                        v.spawn(Node { align_self: AlignSelf::FlexStart, ..default() }).with_children(|r| {
                            spawn_pill(r, &fonts, "RECALL", 12.0, 42.0, None, Pill::new(Tone::Teal), (RecallButton, HudButton, HudZone, Tip::new("RECALL"), ActionTip(Action::Recall)));
                        });
                        v.spawn((HudText::RecallSub, label(&fonts, "", 9.5, teal)));
                    });
            });
            // Left: speed, ending at mid-height (on a 390 px phone it sits between the hull block and the stick).
            root.spawn((ScreenPart::new(Edge::Left, 1), Node { bottom: percent(50), ..abs(Some(28.0), None, None, None) })).with_children(|c| {
                c.spawn(label(&fonts, "SPEED", 12.0, grey));
                c.spawn((HudText::SpeedValue, number(&fonts, "0", 34.0, white)));
                c.spawn((HudText::SpeedSub, label(&fonts, "M/S", 11.0, dim)));
            });
            // Left, under the speed: the pilot assists.
            assists::spawn(root, &fonts, grey, dim);
            // Top right: target.
            root.spawn((ScreenPart::new(Edge::Right, 0), Node { align_items: AlignItems::FlexEnd, ..abs(None, Some(28.0), Some(18.0), None) }))
                .with_children(|c| {
                    c.spawn((HudText::TargetName, label(&fonts, "NO TARGET", 13.0, grey)));
                    c.spawn((HudText::TargetValue, number(&fonts, "", 34.0, coral)));
                    c.spawn((HudText::TargetSub, label(&fonts, "", 11.0, dim)));
                });
            // Top centre: title.
            root.spawn((ScreenPart::new(Edge::Top, 0), Node { position_type: PositionType::Absolute, top: px(16), width: percent(100), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() }))
                .with_children(|c| {
                    c.spawn((HudText::Title, label(&fonts, "", 16.0, white)));
                    c.spawn((HudText::Subtitle, label(&fonts, "", 11.0, grey)));
                });
            // Centre: banner, and the line under it (voyages).
            root.spawn(Node { position_type: PositionType::Absolute, top: percent(25), width: percent(100), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(10), ..default() })
                .with_children(|c| {
                    c.spawn((HudText::Banner, label(&fonts, "", 22.0, teal)));
                    c.spawn((HudText::BannerSub, label(&fonts, "", 11.0, grey)));
                });
            // Voyages: the lost banner and its countdown.
            root.spawn((
                LostBlock,
                Visibility::Hidden,
                Node { position_type: PositionType::Absolute, top: percent(62), width: percent(100), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(12), ..default() },
            ))
            .with_children(|c| {
                c.spawn((HudText::LostTitle, tracked(&fonts, "", 40.0, coral, 0.3)));
                c.spawn((HudText::LostSub, label(&fonts, "", 16.0, white)));
                c.spawn((HudText::LostNote, label(&fonts, "", 10.5, grey)));
                c.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(18), margin: UiRect::top(px(10)), ..default() })
                    .with_children(|row| {
                        row.spawn((
                            Node { width: px(56), height: px(56), border: UiRect::all(px(2)), border_radius: BorderRadius::MAX, justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                            BorderColor::all(ui_color(ui.teal, 0.8)),
                        ))
                        .with_children(|ring| {
                            ring.spawn((HudText::LostCount, number(&fonts, "", 22.0, white)));
                        });
                        row.spawn((HudText::LostHome, label(&fonts, "", 11.0, teal)));
                    });
            });
            // Voyages: the labels of the site markers at the screen edge (placed every frame).
            for k in 0..SITE_LABELS {
                root.spawn((
                    SiteLabel(k),
                    Visibility::Hidden,
                    Node { position_type: PositionType::Absolute, flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(3), ..default() },
                ))
                .with_children(|l| {
                    l.spawn((SiteLabelText(k, 0), label(&fonts, "", 10.5, gold)));
                    l.spawn((SiteLabelText(k, 1), label(&fonts, "", 9.0, dim)));
                });
            }
            // Bottom centre: warnings, then help.
            root.spawn((ScreenPart::new(Edge::Bottom, 1), Node { position_type: PositionType::Absolute, bottom: px(14), width: percent(100), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(8), ..default() }))
                .with_children(|c| {
                    c.spawn((HudText::Warnings, label(&fonts, "", 14.0, coral)));
                    // Centred and kept off the window's edges when it wraps (a narrow window).
                    c.spawn((HudText::Help, tracked(&fonts, "", 10.0, dim, 0.2), TextLayout::justify(bevy::text::Justify::Center), Node { margin: UiRect::horizontal(px(20)), ..default() }));
                });
        });
}

/// The ABANDON VOYAGE? panel (GDR-0077), centred low over the help line, so the ship (at the view's centre) stays in
/// sight while the flight goes on. The whole panel is a [`HudButton`] (and [`Hot`]): a click on it never reaches
/// the world.
fn spawn_abandon_panel(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>, clear: Res<ClearColor>) {
    let ui = &style.ui;
    commands
        .spawn((
            AbandonRoot,
            FlightUi,
            ScreenFx::closed(),
            Visibility::Hidden,
            GlobalZIndex(10),
            Node { position_type: PositionType::Absolute, bottom: px(64), width: percent(100), justify_content: JustifyContent::Center, ..default() },
        ))
        .with_children(|root| {
            root.spawn((
                ScreenPart::new(Edge::Bottom, 0),
                HudButton,
                HudZone,
                Hot::default(),
                BackgroundColor(ui_color(clear.0, 0.94)),
                BorderColor::all(ui_color(ui.coral, 0.45)),
                Node {
                    width: px(460),
                    border: UiRect::all(px(1)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::new(px(28), px(28), px(22), px(22)),
                    row_gap: px(10),
                    ..default()
                },
            ))
            .with_children(|p| {
                p.spawn(tracked(&fonts, "ABANDON VOYAGE?", 24.0, ui_color(ui.coral, 1.0), 0.3));
                p.spawn((AbandonCargo, label(&fonts, "", 10.5, ui_color(ui.number, 1.0))));
                p.spawn(prose(&fonts, "The voyage counts as lost. Your ships re-form at home.", 13.0, ui_color(ui.label, 1.0)));
                p.spawn(label(&fonts, "THE FLIGHT GOES ON WHILE YOU DECIDE", 9.0, ui_color(ui.dim, 1.0)));
                p.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(12), margin: UiRect::top(px(8)), ..default() }).with_children(|row| {
                    let abandon = Pill { active: true, ..Pill::new(Tone::Coral) };
                    spawn_pill(row, &fonts, "ABANDON VOYAGE", 11.0, 36.0, None, abandon, (AbandonButton::Abandon, Tip::key("ABANDON VOYAGE", "ENTER")));
                    spawn_pill(row, &fonts, "KEEP FLYING", 11.0, 36.0, None, Pill::new(Tone::Teal), (AbandonButton::KeepFlying, Tip::key("KEEP FLYING", "ESC")));
                });
            });
        });
}

/// The ABANDON VOYAGE? panel: open while M asked (`VoyageRun::asking`), with how much stays behind; its buttons
/// do what Enter and Esc do (`voyage::voyage_keys`).
fn abandon_panel(
    mut clicks: MessageReader<UiClick>,
    buttons: Query<&AbandonButton>,
    mut run: ResMut<VoyageRun>,
    world: Res<SimWorld>,
    mut root: Single<&mut ScreenFx, With<AbandonRoot>>,
    mut cargo: Single<&mut Text, With<AbandonCargo>>,
) {
    for c in clicks.read() {
        match buttons.get(c.0) {
            // A click on the closing panel does nothing.
            Ok(AbandonButton::Abandon) if run.asking => run.abandon_now(),
            Ok(AbandonButton::KeepFlying) => run.asking = false,
            _ => {}
        }
    }
    show_screen(&mut root, run.asking);
    if run.asking {
        let n = voyage::all_cargo(&world.sim, crate::scene::PLAYER);
        let line = match n {
            0 => "THE HAUL IS LOST  ·  THE HOLDS ARE EMPTY".to_string(),
            1 => "THE HAUL IS LOST  ·  1 PART STAYS BEHIND".to_string(),
            n => format!("THE HAUL IS LOST  ·  {n} PARTS STAY BEHIND"),
        };
        // Written only when it changes (a write re-lays the text out).
        if cargo.0 != line {
            cargo.0 = line;
        }
    }
}

/// The home ring: home, like M (not in a voyage, where it is hidden).
fn go_home(mut clicks: MessageReader<UiClick>, ring: Query<(), With<HomeRing>>, scene: Res<Scene>, mut next: ResMut<NextState<AppMode>>) {
    if clicks.read().any(|c| ring.contains(c.0)) && scene.voyage().is_none() {
        next.set(AppMode::Home);
    }
}

/// The RECALL pill: like R.
fn recall_click(mut clicks: MessageReader<UiClick>, pill: Query<&Pill, With<RecallButton>>, mut run: ResMut<VoyageRun>) {
    if clicks.read().any(|c| pill.get(c.0).is_ok_and(|p| !p.disabled)) {
        run.recall = true;
    }
}

/// The voyage's widgets every frame: the cargo block and RECALL (shown in voyages; the pill enabled while the sim
/// allows the recall, its text with the phase), the pips, the lost banner, the home ring (hidden in voyages).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn voyage_widgets(
    scene: Res<Scene>,
    run: Res<VoyageRun>,
    world: Res<SimWorld>,
    focus: Res<PilotFocus>,
    style: Res<Style>,
    mut blocks: Query<(&mut Visibility, Has<VoyageBlock>, Has<LostBlock>), (Or<(With<VoyageBlock>, With<LostBlock>, With<HomeRing>)>, Without<CargoPip>)>,
    mut pips: Query<(&CargoPip, &mut Visibility, &mut BackgroundColor), Without<VoyageBlock>>,
    mut pill: Query<(Entity, &mut Pill), With<RecallButton>>,
    mut pill_text: Query<(&ChildOf, &mut Text), With<PillText>>,
) {
    let voyage = scene.voyage().is_some();
    let lost = matches!(run.phase, Phase::Lost { .. });
    for (mut v, block, lost_block) in &mut blocks {
        let want = if lost_block { lost } else if block { voyage } else { !voyage };
        let want = if want { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
    }
    if !voyage {
        return;
    }
    let (text, _, enabled) = voyage::recall_words(run.phase, run.can_recall, run.emergency, world.sim.tick());
    let warping = matches!(run.phase, Phase::Warping { .. });
    for (e, mut p) in &mut pill {
        if p.disabled == enabled || p.active != warping {
            p.disabled = !enabled;
            p.active = warping;
        }
        for (parent, mut t) in &mut pill_text {
            if parent.parent() == e {
                set_text(&mut t, &text);
            }
        }
    }
    // One pip per cargo unit of the shown ship's hold, filled for those in use.
    let ship = focus.0.and_then(|id| world.sim.ship(id));
    let (used, cap) = ship.map_or((0, 0), |s| (s.cargo().used(), s.cargo_capacity()));
    let (shown, filled) = if cap > PIPS { (PIPS, (used * PIPS).div_ceil(cap.max(1))) } else { (cap, used) };
    let ui = style.ui;
    for (pip, mut v, mut bg) in &mut pips {
        let want = if pip.0 < shown { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
        let c = if pip.0 < filled { ui_color(ui.gold, 0.95) } else { ui_color(ui.dim, 0.6) };
        if bg.0 != c {
            bg.0 = c;
        }
    }
}

/// Flight ended: the next flight's ships are new, whatever their ids.
fn forget_ship(mut commands: Commands, mut state: ResMut<HudState>, map: Query<Entity, With<HullMap>>) {
    state.map_for = BuiltFor::default();
    state.labels.clear();
    state.labels_for = None;
    for map in &map {
        commands.entity(map).despawn_children();
    }
}

/// The tiny hull map: one cell per blueprint cell of the player's ship (in a behavior test's watch mode the
/// watched one), teal by hit points, coral when lost. Rebuilt whenever the ship or its design changes.
#[allow(clippy::too_many_arguments)]
fn update_hull_map(
    mut commands: Commands,
    mut state: ResMut<HudState>,
    world: Res<SimWorld>,
    focus: Res<PilotFocus>,
    scene: Res<Scene>,
    style: Res<Style>,
    map: Query<Entity, With<HullMap>>,
    mut cells: Query<(&HullCell, &mut BackgroundColor)>,
) {
    let Ok(map) = map.single() else { return };
    let ship = focus.0.or(scene.watched()).and_then(|id| world.sim.ship(id));
    if !state.map_for.matches(ship) {
        state.map_for.set(ship);
        commands.entity(map).despawn_children();
        if let Some(s) = ship {
            let b = s.design().grid.bounds().unwrap_or(data::Bounds { min_x: 0, min_y: 0, max_x: 1, max_y: 1 });
            let cell = (100.0 / b.width() as f32).min(60.0 / b.height() as f32).floor().max(3.0);
            commands.entity(map).with_children(|m| {
                for (i, p) in s.design().parts.iter().enumerate() {
                    for (x, y) in p.shape.cells() {
                        m.spawn((
                            HullCell(i as u32),
                            Node {
                                position_type: PositionType::Absolute,
                                left: px((x - b.min_x) as f32 * cell),
                                top: px((y - b.min_y) as f32 * cell),
                                width: px(cell - 1.0),
                                height: px(cell - 1.0),
                                ..default()
                            },
                            BackgroundColor(ui_color(style.glow_player, 0.8)),
                        ));
                    }
                }
            });
        }
        return;
    }
    // Same cadence as the text (the timer was ticked by `update_text` this frame).
    if !state.timer.just_finished() {
        return;
    }
    let Some(s) = ship else { return };
    let control = s.control_piece();
    for (cell, mut bg) in &mut cells {
        let st = s.part(cell.0);
        let full = s.design().parts[cell.0 as usize].hp.max(1e-3);
        let c = if !st.alive {
            ui_color(style.glow_enemy, 0.22)
        } else if Some(st.piece) != control {
            ui_color(style.ui_label, 0.35)
        } else {
            let f = (st.hp / full).clamp(0.0, 1.0);
            if f < 0.5 { ui_color(style.system_color("power"), 0.85) } else { ui_color(style.glow_player, 0.45 + 0.45 * f) }
        };
        if bg.0 != c {
            bg.0 = c;
        }
    }
}

#[cfg(test)]
mod tests {
    use sim::Body;

    use super::*;
    use super::markers::{label_spot, marker_spot};
    use crate::scene::PLAYER;

    fn design(content: &data::Content, id: &str) -> Arc<ShipDesign> {
        Arc::new(ShipDesign::compile(content, content.ships.get(id).unwrap()).unwrap())
    }

    /// Every new sim numbers its ships from 0: a test flight of a smaller design after a flight of a bigger one
    /// has the same ship id, so the hull map (one cell per part of the old design) must be rebuilt, or it indexes
    /// past the new design's parts (it panicked).
    #[test]
    fn the_hull_map_is_rebuilt_for_a_new_sim_even_with_the_same_ship_id() {
        let content = data::base_content();
        let fly = |id: &str| {
            let mut sim = sim::Sim::new(1);
            let ship = sim.spawn_ship(design(&content, id), PLAYER, Body::at(sim::Vec2::ZERO, 0.0));
            (sim, ship)
        };
        let (first, runner) = fly("halvex_runner");
        let mut built = BuiltFor::default();
        assert!(built.matches(None));
        built.set(first.ship(runner));
        assert!(built.matches(first.ship(runner)));
        assert!(!built.matches(None));

        // The drydock's T with fewer parts: a new sim, the same id, a smaller design.
        let (second, skiff) = fly("rim_skiff");
        assert_eq!(skiff, runner);
        assert!(second.ship(skiff).unwrap().design().parts.len() < first.ship(runner).unwrap().design().parts.len());
        assert!(!built.matches(second.ship(skiff)));
        // The same design compiled again (Shift+R, another test flight) is a new design too.
        let (third, again) = fly("halvex_runner");
        assert!(!built.matches(third.ship(again)));
        built.set(third.ship(again));
        assert!(built.matches(third.ship(again)));
    }

    /// GDR-0088: the ship an auto-lock assist asks for becomes the pilot's lock at the scene's tick, and the target
    /// panel shows it like any lock; once the pilot drops it (Esc), auto-lock waits and nothing is locked again.
    #[test]
    fn a_lock_set_by_auto_lock_is_the_pilots_lock_in_the_target_panel() {
        let content = data::base_content();
        let mut sim = sim::Sim::new(1);
        sim.set_behaviors(sim::BehaviorLibrary::new(&content).unwrap());
        let me = sim.spawn_ship(design(&content, "halvex_runner"), PLAYER, Body::at(sim::Vec2::ZERO, 0.0));
        let raider = sim.spawn_ship(design(&content, "automaton_raider"), crate::scene::ENEMY, Body::at(sim::Vec2::new(0.0, 60.0), 0.0));
        let id = sim.behaviors_mut().add_assists(&data::AssistList { rules: vec!["always -> auto_lock(nearest, toggle1)".into()] }).unwrap();
        sim.command(me, sim::ShipCommand::Assists(Some(id)));
        sim.command(me, sim::ShipCommand::Focus);
        let mut world = SimWorld::new(sim, content);
        let tick = |world: &mut SimWorld, lock: Option<Target>| {
            let input = sim::PilotInput { lock, ..Default::default() };
            world.sim.step(&sim::TickInputs { commands: vec![(me, sim::ShipCommand::Pilot(input))], ..Default::default() });
        };
        let panel = |world: &SimWorld, lock: Option<Target>| {
            let mut name = String::new();
            super::text::target_text(HudText::TargetName, &mut name, world, lock, Some("TAB"), None);
            name
        };
        assert_eq!(panel(&world, None), "NO TARGET");
        tick(&mut world, None);
        let lock = crate::scene::tick_lock(&world, world.sim.ship(me).unwrap(), None);
        assert_eq!(lock, Some(Target::Ship(raider)));
        assert_eq!(panel(&world, lock), world.sim.ship(raider).unwrap().design().name.to_uppercase());
        // Held from the next tick on, it stays; a lock the pilot holds is never replaced.
        tick(&mut world, lock);
        assert_eq!(crate::scene::tick_lock(&world, world.sim.ship(me).unwrap(), lock), lock);
        // Esc: auto-lock pauses, so nothing comes back.
        tick(&mut world, None);
        assert!(world.sim.ship(me).unwrap().assists().unwrap().lock_paused());
        assert_eq!(crate::scene::tick_lock(&world, world.sim.ship(me).unwrap(), None), None);
    }

    /// A site label sits towards the view's centre, or beside its arrow when the HUD or another label is there.
    #[test]
    fn site_labels_keep_clear_of_the_hud_and_each_other() {
        let view = Vec2::new(1600.0, 900.0);
        let size = Vec2::new(120.0, 30.0);
        // An arrow at the top edge: the label goes below it.
        let top = label_spot(Vec2::new(800.0, 40.0), Vec2::new(0.0, -1.0), size, view, &[]);
        assert!(top.min.y > 40.0 && (top.center().x - 800.0).abs() < 1e-3, "{top:?}");
        // Taken: the next free place beside the arrow.
        let beside = label_spot(Vec2::new(800.0, 40.0), Vec2::new(0.0, -1.0), size, view, &[top]);
        assert!(beside.intersect(top).is_empty(), "{beside:?}");
        // Always inside the view.
        let corner = label_spot(Vec2::new(10.0, 10.0), Vec2::new(-0.7, -0.7), size, view, &[]);
        assert!(corner.min.x >= 6.0 && corner.min.y >= 6.0);
    }

    /// A marker for a ship up and to the left of a 1600 x 900 view: at the edge it sat on the hull block; it moves
    /// in along its ray, out of the block and the title, and stays in the view.
    #[test]
    fn off_screen_markers_keep_clear_of_the_hud() {
        let (centre, edge) = (Vec2::new(800.0, 450.0), Vec2::new(800.0, 450.0) - Vec2::splat(MARKER_INSET));
        let hull = Rect::new(54.0, 4.0, 330.0, 150.0);
        let title = Rect::new(560.0, 2.0, 1040.0, 70.0);
        let ray = Vec2::new(-600.0, -420.0);
        let free = marker_spot(centre, ray, edge, &[]);
        assert!((free.y - MARKER_INSET).abs() < 1e-3, "{free}");
        assert!(hull.contains(free));
        let spot = marker_spot(centre, ray, edge, &[title, hull]);
        assert!(!hull.contains(spot) && !title.contains(spot), "{spot}");
        assert!((spot - centre).perp_dot(ray).abs() < 1e-2 * ray.length(), "off the ray: {spot}");
        // Just outside the block, where the ray leaves it.
        assert!(hull.inflate(1.5).contains(spot), "{spot}");
        // Straight up through the title: just below it.
        let up = marker_spot(centre, Vec2::new(0.0, -500.0), edge, &[title, hull]);
        assert!((up.x - 800.0).abs() < 1e-3 && up.y > title.max.y && up.y <= title.max.y + 1.5, "{up}");
    }
}
