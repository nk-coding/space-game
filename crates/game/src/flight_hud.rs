//! The flight HUD in the reference look (`docs/concept/boards/ui-mockups/phone-flight-hud.png`): floating text,
//! wide-tracked uppercase labels in Barlow, white numbers, no boxes.
//!
//! * top left: the home ring (what M does: home, for touch), hull % and a tiny hull map of the player's ship (in a
//!   behavior test's watch mode: the watched one), and its hold (`CARGO 3 / 8`, salvage collected / cargo units;
//!   hidden for a ship without a hold);
//! * left: speed (m/s), the top speed and the drift state;
//! * top right: target panel (name, hull %, distance; in watch mode the watched ship's behavior target);
//! * top centre: scene / wave line and whether raider waves come, centre: banners (wave cleared, ship lost,
//!   `PAUSED`);
//! * bottom centre: heat and power warnings ("PULSE LASER 2 · HEAT 96 %"); bottom: the controls help line;
//! * in the world: a dotted lock ring around the target, off-screen markers for enemies (also in watch mode; kept
//!   clear of the HUD's texts and widgets).
//! * **voyages** (GDR-0065, `docs/plans/mockups/voyage-hud-b.png`): the title is the voyage and its band, the
//!   subtitle `SITES 2 / 5 · 7:05`; under the hull the cargo block (`SALVAGE`, `5 / 8` with one pip per cargo unit,
//!   the hold's name) and right under it the RECALL pill (R; enabled only while the sim allows the recall) with
//!   what it does; markers at the screen edge for the sites off screen (name and distance: raiders coral, wrecks
//!   gold, the goal brighter, cleared ones dim); banners for sites waking and cleared and for the goal; the
//!   warp-out's countdown; `SHIP LOST · RECALLED HOME` with its countdown (`voyage-hud-lost.png`; `VOYAGE
//!   ABANDONED` after M twice). No home ring (M asks to abandon the voyage instead).
//!
//! Text is rebuilt at 8 Hz and only re-laid out when it changes.

use std::fmt::Write as _;
use std::sync::Arc;

use bevy::prelude::*;
use sim::{ShipDesign, ShipId, Stat, Target};

use bevy::ui::{ComputedNode, UiGlobalTransform};

use crate::camera::{CameraRig, CameraSet};
use crate::controls::PlayerInput;
use crate::fx::{FxQueue, FxRegions, Z_HUD};
use crate::mode::{AppMode, FlightUi};
use crate::scene::{PilotFocus, Scene, SceneKind};
use crate::sim_bridge::{SimWorld, piece_com, v2};
use crate::style::{Style, ui_color};
use crate::test_range::{BehaviorWatch, TestMode};
use crate::ui_icons::{Glyph, UiIcon};
use crate::ui_kit::{Edge, Pill, PillText, ScreenFx, ScreenPart, Tip, Tone, show_screen, UiClick, UiInputSet, UiStyleSet, label, number, ring_node, set_color, set_text, spawn_pill, tracked};
use crate::voyage::{self, Phase, VoyageRun};
use crate::{RenderToggles, ship_view::PieceView};

pub(crate) static BARLOW_REGULAR: &[u8] = include_bytes!("../assets/fonts/Barlow-Regular.ttf");
static BARLOW_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Barlow-Medium.ttf");

#[derive(Resource, Clone)]
pub struct HudFonts {
    pub regular: Handle<Font>,
    pub medium: Handle<Font>,
}

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
        .add_systems(Startup, spawn_hud)
        .add_systems(Update, (update_text, update_hull_map).chain().run_if(in_state(AppMode::Flight)))
        .add_systems(Update, (go_home, recall_click).after(UiInputSet).run_if(in_state(AppMode::Flight)))
        .add_systems(Update, voyage_widgets.after(UiInputSet).before(UiStyleSet).run_if(in_state(AppMode::Flight)))
        .add_systems(OnExit(AppMode::Flight), forget_ship)
        .add_systems(PostUpdate, (lock_ring, offscreen_markers, site_markers).after(CameraSet).before(crate::fx::FxFlush));
    }
}

pub(crate) fn spawn_hud(mut commands: Commands, mut fonts_assets: ResMut<Assets<Font>>, style: Res<Style>) {
    let fonts = HudFonts {
        regular: fonts_assets.add(Font::from_bytes(BARLOW_REGULAR.to_vec())),
        medium: fonts_assets.add(Font::from_bytes(BARLOW_MEDIUM.to_vec())),
    };
    commands.insert_resource(fonts.clone());
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
                c.spawn((HomeRing, HudButton, HudZone, ring_node(34.0), Tip::key("SHIPYARD", "M"))).with_children(|r| {
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
                            spawn_pill(r, &fonts, "RECALL", 12.0, 42.0, None, Pill::new(Tone::Teal), (RecallButton, HudButton, HudZone, Tip::key("RECALL", "R")));
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

#[allow(clippy::too_many_arguments)]
fn update_text(
    time: Res<Time<Real>>,
    virtual_time: Res<Time<Virtual>>,
    mut state: ResMut<HudState>,
    world: Res<SimWorld>,
    scene: Res<Scene>,
    focus: Res<PilotFocus>,
    input: Res<PlayerInput>,
    watch: Res<BehaviorWatch>,
    toggles: Res<RenderToggles>,
    rig: Res<CameraRig>,
    fixed: Res<Time<Fixed>>,
    run: Res<VoyageRun>,
    style: Res<Style>,
    mut texts: Query<(&HudText, &mut Text, &mut TextColor)>,
    mut root: Query<&mut ScreenFx, With<HudRoot>>,
) {
    for mut fx in &mut root {
        show_screen(&mut fx, toggles.hud);
    }
    if !state.timer.tick(time.delta()).just_finished() {
        return;
    }
    let HudState { scratch, labels, labels_for, .. } = &mut *state;
    let alpha = fixed.overstep_fraction();
    let player = focus.0.or(scene.watched()).and_then(|id| world.sim.ship(id));
    let control = player.and_then(|s| s.control_piece().and_then(|c| s.piece(c)));
    // Watch: nobody pilots or locks; the target panel shows what the watched ship's behavior targets.
    let watching = focus.0.is_none() && scene.watched().is_some();
    let target = if watching { player.and_then(|s| s.behavior()).and_then(|b| b.target()).map(Target::Ship) } else { input.lock };

    // Labels per part of the shown ship's design (a design edited in the drydock can keep its part count).
    if let Some(s) = player
        && !labels_for.as_ref().is_some_and(|d| Arc::ptr_eq(d, s.design()))
    {
        *labels_for = Some(s.design().clone());
        labels.clear();
        let mut seen: Vec<(String, u32)> = Vec::new();
        for p in &s.design().parts {
            let name = world.content.parts.get(&p.id).map(|d| d.name.to_uppercase()).unwrap_or_else(|| p.id.to_uppercase());
            let n = match seen.iter_mut().find(|e| e.0 == name) {
                Some(e) => {
                    e.1 += 1;
                    e.1
                }
                None => {
                    seen.push((name.clone(), 1));
                    1
                }
            };
            labels.push(format!("{name} {n}"));
        }
    }

    let ui = style.ui;
    let tick = world.sim.tick();
    let voyage = world.sim.voyage().filter(|_| scene.voyage().is_some());
    let lost = matches!(run.phase, Phase::Lost { .. });
    let warping = matches!(run.phase, Phase::Warping { .. });
    for (kind, mut text, mut color) in &mut texts {
        let s = &mut *scratch;
        s.clear();
        match kind {
            HudText::HullValue => match player {
                Some(ship) => {
                    let _ = write!(s, "{:.0} %", hull_fraction(ship, control.map(|p| p.id)) * 100.0);
                }
                None => s.push('–'),
            },
            HudText::Cargo if voyage.is_some() => s.push_str("SALVAGE"),
            HudText::Cargo => {
                // Salvage in the hold (GDR-0063): cargo units in use / the hold of the controlling piece.
                if let Some(ship) = player {
                    let (used, cap) = (ship.cargo().used(), ship.cargo_capacity());
                    if cap > 0 || used > 0 {
                        let _ = write!(s, "CARGO {used} / {cap}");
                    }
                }
            }
            HudText::SpeedValue => {
                if let Some(p) = control {
                    let _ = write!(s, "{:.1}", v2(p.body.linvel).length());
                } else {
                    s.push('–');
                }
            }
            HudText::SpeedSub => {
                s.push_str("M/S");
                // The top speed of the engines pushing along the best axis (GDR-0041).
                let top = player.map_or(0.0, |ship| ship.telemetry().top_speed);
                if top > 0.0 {
                    let _ = write!(s, "  ·  TOP {top:.0}");
                }
                if input.drift {
                    s.push_str("  ·  DRIFT");
                }
            }
            HudText::TargetName | HudText::TargetValue | HudText::TargetSub => {
                let from = control.map(|p| piece_com(p, alpha)).or(rig.focus);
                target_text(*kind, s, &world, target, !watching, from);
            }
            HudText::Title => match &scene.kind {
                SceneKind::Sandbox => {
                    let name = match scene.test_mode() {
                        Some(TestMode::Watch) => "WATCH",
                        Some(TestMode::Alongside) => "FLY ALONGSIDE",
                        None if scene.test_flight => "TEST FLIGHT",
                        None => "TEST RANGE",
                    };
                    s.push_str(name);
                    if scene.test_mode().is_some() && !watch.name.is_empty() {
                        let _ = write!(s, "  ·  {}", watch.name.to_uppercase());
                    }
                    let _ = write!(s, "  ·  WAVE {}", scene.wave);
                }
                SceneKind::Showcase => s.push_str("SHOWCASE"),
                SceneKind::Load(n) => {
                    let _ = write!(s, "LOAD TEST  ·  {n} SHIPS");
                }
                SceneKind::Voyage(id) => s.push_str(&voyage::title(&world.content, id)),
            },
            HudText::Subtitle => {
                let alive = world.sim.vessels(crate::scene::ENEMY).filter(|v| v.state == sim::VesselState::Running).count();
                match scene.kind {
                    SceneKind::Sandbox => {
                        // The player's vessels are in the fleet bar (`fleet_bar.rs`).
                        let _ = write!(s, "RAIDERS {} / {}  ·  KILLS {}", alive, scene.wave_ships.len(), scene.kills);
                        s.push_str(if scene.waves { "  ·  WAVES ON" } else { "  ·  WAVES OFF" });
                    }
                    SceneKind::Load(_) => {
                        let active = world.sim.ships().iter().filter(|s| s.is_active()).count();
                        let pieces: usize = world.sim.ships().iter().map(|s| s.pieces().len()).sum();
                        let _ = write!(s, "ACTIVE {}  ·  PIECES {}  ·  SHOTS IN FLIGHT {}", active, pieces, world.sim.projectiles().len());
                    }
                    SceneKind::Showcase => s.push_str("DRAG TO PAN  ·  WHEEL TO ZOOM"),
                    SceneKind::Voyage(_) => {
                        if let Some(v) = voyage {
                            let (done, all) = voyage::sites_done(v);
                            let _ = write!(s, "SITES {done} / {all}  ·  {}", voyage::clock(tick));
                        }
                        if run.launched.as_ref().is_some_and(|l| l.practice.is_some()) {
                            s.push_str("  ·  PRACTICE, NOT SAVED");
                        }
                    }
                }
            }
            HudText::Banner => {
                if virtual_time.is_paused() {
                    s.push_str("PAUSED");
                } else if let Phase::Warping { home_at } = run.phase {
                    let _ = write!(s, "WARPING HOME  ·  {}", voyage::seconds_left(home_at, tick));
                } else if let Some((b, until)) = &scene.banner
                    && world.sim.time_seconds() < *until
                    && !lost
                {
                    s.push_str(b);
                }
                let tone = if warping { Tone::Teal } else { scene.banner_tone };
                set_color(&mut color, ui_color(tone.color(&ui), 1.0));
            }
            HudText::BannerSub => {
                if warping {
                    s.push_str("THE HAUL COMES WITH YOU");
                } else if !virtual_time.is_paused()
                    && let Some((_, until)) = &scene.banner
                    && world.sim.time_seconds() < *until
                    && !lost
                {
                    s.push_str(&scene.banner_sub);
                }
            }
            HudText::SalvageValue => {
                if let Some(ship) = player {
                    let _ = write!(s, "{} / {}", ship.cargo().used(), ship.cargo_capacity());
                } else if lost {
                    s.push('–');
                }
                set_color(&mut color, ui_color(ui.gold, if lost { 0.5 } else { 1.0 }));
            }
            HudText::SalvageSub => {
                if lost {
                    s.push_str("LEFT IN THE WRECK");
                    set_color(&mut color, ui_color(ui.coral, 1.0));
                } else {
                    match player {
                        Some(ship) if ship.cargo_capacity() > 0 => s.push_str(&hold_name(&world.content, ship)),
                        Some(_) => s.push_str("NO HOLD"),
                        None => {}
                    }
                    set_color(&mut color, ui_color(ui.dim, 1.0));
                }
            }
            HudText::RecallSub => {
                let (_, line, enabled) = voyage::recall_words(run.phase, run.can_recall, run.emergency, tick);
                s.push_str(&line);
                let tone = if enabled || warping { Tone::Teal } else { Tone::Dim };
                set_color(&mut color, ui_color(tone.color(&ui), 1.0));
            }
            HudText::LostTitle => s.push_str(if matches!(run.phase, Phase::Lost { abandoned: true, .. }) { "VOYAGE ABANDONED" } else { "SHIP LOST" }),
            HudText::LostSub => s.push_str("RECALLED HOME"),
            HudText::LostNote => {
                if let Phase::Lost { left, abandoned, .. } = run.phase {
                    if left > 0 {
                        let place = if abandoned { "BEHIND" } else { "IN THE WRECK" };
                        let _ = write!(s, "THE HAUL STAYS {place}  ·  {left} {} LOST  ·  ", if left == 1 { "PART" } else { "PARTS" });
                    }
                    s.push_str("YOUR SHIPS RE-FORM AT HOME");
                }
            }
            HudText::LostCount | HudText::LostHome => {
                if let Phase::Lost { home_at, .. } = run.phase {
                    let n = voyage::seconds_left(home_at, tick);
                    if *kind == HudText::LostCount {
                        let _ = write!(s, "{n}");
                    } else {
                        let _ = write!(s, "HOME IN {n} S");
                    }
                }
            }
            HudText::Warnings => {
                if let (Some(ship), Some(p)) = (player, control) {
                    warnings(s, ship, p, labels);
                }
            }
            HudText::Help => match scene.kind {
                SceneKind::Sandbox if scene.watched().is_some() => {
                    s.push_str("NOBODY PILOTS · DRAG PAN · F FOLLOW / FREE · WHEEL ZOOM · C WAVES · F3 RULES · T BACK · M HOME · H HIDE");
                }
                SceneKind::Sandbox if input.touch_ui => {
                    s.push_str("STICK FLY  ·  TAP SHIP LOCK  ·  FIRE BUTTON  ·  PINCH ZOOM");
                }
                SceneKind::Sandbox => s.push_str(
                    "A/D TURN · W/S THRUST · Q/E STRAFE · CLICK LOCK (A/D NUDGE) · MOUSE/SPACE FIRE · TAB TARGET · X DRIFT · V VESSEL · C WAVES · T DRYDOCK · M HOME · U TUNING · F3 DEBUG · H HIDE",
                ),
                SceneKind::Voyage(_) if input.touch_ui => s.push_str("STICK FLY  ·  TAP SHIP LOCK  ·  FIRE BUTTON  ·  PINCH ZOOM  ·  RECALL"),
                SceneKind::Voyage(_) => s.push_str(
                    "A/D TURN · W/S THRUST · Q/E STRAFE · CLICK LOCK · SPACE FIRE · TAB TARGET · X DRIFT · V VESSEL · R RECALL · M M ABANDON · U TUNING · F3 DEBUG · H HIDE",
                ),
                _ => s.push_str("DRAG PAN  ·  WHEEL ZOOM  ·  F FRAME / FREE  ·  1 DEBUG SHAPES  ·  2 GLOW  ·  3 BLOOM  ·  F3 STATS  ·  H HIDE  ·  T DRYDOCK  ·  M HOME"),
            },
        }
        set_text(&mut text, scratch);
    }
}

/// The hold's name under the salvage number: the biggest alive cargo part of the controlling piece (`CARGO HOLD`).
fn hold_name(content: &data::Content, ship: &sim::Ship) -> String {
    let control = ship.control_piece();
    let alive = |i: usize| {
        let st = ship.part(i as u32);
        st.alive && Some(st.piece) == control
    };
    let best = ship.design().parts.iter().enumerate().filter(|(i, p)| p.cargo > 0.0 && alive(*i)).max_by(|a, b| a.1.cargo.total_cmp(&b.1.cargo).then(b.0.cmp(&a.0)));
    best.map_or_else(|| "CARGO".into(), |(_, p)| content.parts.get(&p.id).map_or(p.id.to_uppercase(), |d| d.name.to_uppercase()))
}

/// Hull integrity: hit points of the controlling piece over the full design.
pub fn hull_fraction(ship: &sim::Ship, control: Option<sim::PieceId>) -> f32 {
    let mut have = 0.0;
    let mut full = 0.0;
    for (i, p) in ship.design().parts.iter().enumerate() {
        full += p.hp;
        let st = ship.part(i as u32);
        if st.alive && Some(st.piece) == control {
            have += st.hp;
        }
    }
    if full > 0.0 { have / full } else { 0.0 }
}

/// The target panel: the player's lock (with how to lock when there is none), or in watch mode the watched
/// ship's behavior target (`hint` off: nobody can lock).
fn target_text(kind: HudText, s: &mut String, world: &SimWorld, target: Option<Target>, hint: bool, from: Option<Vec2>) {
    let Some(lock) = target else {
        if kind == HudText::TargetName {
            s.push_str("NO TARGET");
        } else if kind == HudText::TargetSub && hint {
            s.push_str("CLICK OR TAB TO LOCK");
        }
        return;
    };
    let (ship, piece) = match lock {
        Target::Ship(id) => {
            let ship = world.sim.ship(id);
            (ship, ship.and_then(|s| s.control_piece().or_else(|| s.main_piece())).and_then(|p| ship.and_then(|s| s.piece(p))))
        }
        Target::Piece(id, p) => {
            let ship = world.sim.ship(id);
            (ship, ship.and_then(|s| s.piece(p)))
        }
        Target::Point(_) => (None, None),
    };
    let (Some(ship), Some(piece)) = (ship, piece) else { return };
    match kind {
        HudText::TargetName => {
            let name = &ship.design().name;
            if piece.controlled {

                for c in name.chars() {
                    s.extend(c.to_uppercase());
                }
            } else {
                s.push_str("SALVAGE");
            }
        }
        HudText::TargetValue => {
            let _ = write!(s, "{:.0} %", hull_fraction(ship, Some(piece.id)) * 100.0);
        }
        HudText::TargetSub => {
            let d = from.map(|f| v2(piece.com_world()).distance(f)).unwrap_or(0.0);
            if d >= 1000.0 {
                let _ = write!(s, "{:.1} KM", d / 1000.0);
            } else {
                let _ = write!(s, "{:.0} M", d);
            }
            if !ship.is_active() {
                s.push_str("  ·  DARK");
            }
        }
        _ => {}
    }
}

/// The two hottest parts near their limit, power shortage, shields down, no power.
fn warnings(s: &mut String, ship: &sim::Ship, piece: &sim::Piece, labels: &[String]) {
    let mut hot: [(f32, u32); 2] = [(0.0, 0); 2];
    let mut shields_down = 0;
    for &i in &piece.parts {
        let st = ship.part(i);
        let limit = ship.stats(i).get(Stat::HeatLimit);
        if limit.is_finite() && limit > 0.0 {
            let r = if st.overheated { 1.0 } else { st.heat / limit };
            if r > 0.8 {
                if r > hot[0].0 {
                    hot[1] = hot[0];
                    hot[0] = (r, i);
                } else if r > hot[1].0 {
                    hot[1] = (r, i);
                }
            }
        }
        if st.shield_down && ship.design().parts[i as usize].is_shield {
            shields_down += 1;
        }
    }
    let sep = |s: &mut String| {
        if !s.is_empty() {
            s.push_str("   ·   ");
        }
    };
    if !piece.active {
        sep(s);
        s.push_str("NO POWER");
    }
    for (r, i) in hot {
        if r > 0.0 {
            sep(s);
            let name = labels.get(i as usize).map(String::as_str).unwrap_or("PART");
            let _ = write!(s, "{name}  ·  HEAT {:.0} %", (r * 100.0).min(100.0));
        }
    }
    if piece.power.demand > 0.0 && piece.power.served < piece.power.demand * 0.97 {
        sep(s);
        let _ = write!(s, "POWER {:.0} %", piece.power.served / piece.power.demand * 100.0);
    }
    if shields_down > 0 {
        sep(s);
        let _ = write!(s, "SHIELD DOWN {shields_down}");
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

/// A dotted ring around the locked target (world space).
fn lock_ring(
    time: Res<Time>,
    world: Res<SimWorld>,
    input: Res<PlayerInput>,
    rig: Res<CameraRig>,
    fixed: Res<Time<Fixed>>,
    regions: Option<Res<FxRegions>>,
    style: Res<Style>,
    views: Query<&PieceView>,
    toggles: Res<RenderToggles>,
    mut queue: ResMut<FxQueue>,
) {
    let (Some(regions), Some(lock)) = (regions, input.lock) else { return };
    if !toggles.hud {
        return;
    }
    let alpha = fixed.overstep_fraction();
    let (ship, piece) = match lock {
        Target::Ship(id) => (id, world.sim.ship(id).and_then(|s| s.control_piece().or_else(|| s.main_piece()))),
        Target::Piece(id, p) => (id, Some(p)),
        Target::Point(_) => return,
    };
    let Some(piece_id) = piece else { return };
    let Some(p) = world.sim.ship(ship).and_then(|s| s.piece(piece_id)) else { return };
    let centre = piece_com(p, alpha);
    let r = world.views.get(&(ship, piece_id)).and_then(|e| views.get(*e).ok()).map(|v| v.radius).unwrap_or(3.0) + 1.2;
    let px = 1.0 / rig.zoom;
    let spin = time.elapsed_secs() * 0.6;
    let c = style.glow_enemy;
    let n = 36;
    for k in 0..n {
        let a = spin + k as f32 / n as f32 * std::f32::consts::TAU;
        queue.dot(&regions.hard, centre + Vec2::new(a.cos(), a.sin()) * r, 3.0 * px, c.with_alpha(0.7), Z_HUD);
    }
    for k in 0..4 {
        let a = -spin * 0.5 + k as f32 * std::f32::consts::FRAC_PI_2;
        let d = Vec2::new(a.cos(), a.sin());
        queue.line(&regions.white, centre + d * (r + 3.0 * px), centre + d * (r + 14.0 * px), 2.0 * px, c.with_alpha(0.9), Z_HUD);
    }
}

/// Markers at the screen edge pointing at active enemies (of the focus; in watch mode of the watched ship) outside
/// the view, kept clear of the HUD's texts and widgets (bevy_ui draws over the world: a marker there hid them or
/// was hidden).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn offscreen_markers(
    world: Res<SimWorld>,
    focus: Res<PilotFocus>,
    scene: Res<Scene>,
    input: Res<PlayerInput>,
    rig: Res<CameraRig>,
    regions: Option<Res<FxRegions>>,
    style: Res<Style>,
    toggles: Res<RenderToggles>,
    zones: Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility), Or<(With<HudText>, With<HudZone>)>>,
    mut rects: Local<Vec<Rect>>,
    mut queue: ResMut<FxQueue>,
) {
    let Some(regions) = regions else { return };
    // The focus, else (watch) the watched ship.
    let Some(me) = focus.0.or(scene.watched()).and_then(|id| world.sim.ship(id)) else { return };
    if !toggles.hud {
        return;
    }
    rects.clear();
    let shown = zones.iter().filter(|z| z.2.get());
    rects.extend(shown.filter_map(|(node, tf, _)| logical_rect(node, tf)).map(|r| r.inflate(MARKER_CLEARANCE)));
    let team = me.team();
    let px = 1.0 / rig.zoom;
    let half = rig.half_extent();
    let centre = rig.view_center();
    let edge = rig.viewport / 2.0 - Vec2::splat(MARKER_INSET);
    let voyage = world.sim.voyage();
    for s in world.sim.ships() {
        // A sleeping site's ships are the site's marker (`site_markers`).
        if !s.is_active() || sim::friendly(team, s.team()) || voyage.is_some_and(|v| v.asleep(s.id())) {
            continue;
        }
        let Some(p) = s.control_piece().and_then(|c| s.piece(c)) else { continue };
        let at = v2(p.com_world());
        let d = at - rig.pos;
        if d.x.abs() < half.x && d.y.abs() < half.y {
            continue;
        }
        // On screen (y down): the ray from the view centre towards the ship.
        let ray = rig.world_to_screen(at) - centre;
        let pos = rig.screen_to_world(marker_spot(centre, ray, edge, &rects));
        let dir = Vec2::new(ray.x, -ray.y).normalize_or_zero();
        let side = Vec2::new(-dir.y, dir.x);
        let locked = input.locked_ship() == Some(s.id());
        let c = style.glow_enemy.with_alpha(if locked { 1.0 } else { 0.75 });
        let tip = pos + dir * 9.0 * px;
        queue.line(&regions.white, tip, pos - dir * 3.0 * px + side * 7.0 * px, 2.0 * px, c, Z_HUD);
        queue.line(&regions.white, tip, pos - dir * 3.0 * px - side * 7.0 * px, 2.0 * px, c, Z_HUD);
        queue.dot(&regions.soft, pos, 18.0 * px, c.with_alpha(0.25), Z_HUD - 0.1);
    }
}

/// How a site marker looks: its colour and alpha (raiders coral, wrecks only gold, cleared dim; the goal brighter),
/// and the label's first line (`RAIDER PACK · GOAL`, `... · CLEARED`).
fn site_look(content: &data::Content, site: &sim::Site, ui: &crate::style::UiPalette) -> (Color, f32, String) {
    let mut name = voyage::encounter_name(content, site.encounter());
    if site.goal() {
        name.push_str("  ·  GOAL");
    }
    if site.state() == sim::SiteState::Cleared {
        name.push_str("  ·  CLEARED");
        return (ui.dim, 0.8, name);
    }
    let color = if site.plan().ships.is_empty() { ui.gold } else { ui.coral };
    (color, if site.goal() { 1.0 } else { 0.75 }, name)
}

/// Voyages: a marker at the screen edge for every site whose centre is off screen (like the enemy markers, kept
/// clear of the HUD), with its name and distance beside it.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn site_markers(
    world: Res<SimWorld>,
    focus: Res<PilotFocus>,
    scene: Res<Scene>,
    rig: Res<CameraRig>,
    regions: Option<Res<FxRegions>>,
    style: Res<Style>,
    toggles: Res<RenderToggles>,
    fixed: Res<Time<Fixed>>,
    zones: Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility), Or<(With<HudText>, With<HudZone>)>>,
    mut labels: Query<(&SiteLabel, &mut Node, &ComputedNode, &mut Visibility)>,
    mut texts: Query<(&SiteLabelText, &mut Text, &mut TextColor)>,
    mut rects: Local<Vec<Rect>>,
    mut queue: ResMut<FxQueue>,
) {
    let voyage = world.sim.voyage().filter(|_| scene.voyage().is_some() && toggles.hud);
    let mut spots: [Option<(Vec2, Vec2)>; SITE_LABELS] = [None; SITE_LABELS];
    if let (Some(v), Some(regions)) = (voyage, regions) {
        rects.clear();
        let visible = zones.iter().filter(|z| z.2.get());
        rects.extend(visible.filter_map(|(node, tf, _)| logical_rect(node, tf)).map(|r| r.inflate(MARKER_CLEARANCE)));
        let ui = style.ui;
        let alpha = fixed.overstep_fraction();
        let from = focus.0.and_then(|id| world.sim.ship(id)).and_then(|s| s.control_piece().and_then(|c| s.piece(c))).map_or(rig.pos, |p| piece_com(p, alpha));
        let (px, half, centre) = (1.0 / rig.zoom, rig.half_extent(), rig.view_center());
        let edge = rig.viewport / 2.0 - Vec2::splat(MARKER_INSET + 14.0);
        for (k, site) in v.sites().iter().enumerate().take(SITE_LABELS) {
            let at = v2(site.pos());
            let d = at - rig.pos;
            if d.x.abs() < half.x && d.y.abs() < half.y {
                continue;
            }
            let ray = rig.world_to_screen(at) - centre;
            let spot = marker_spot(centre, ray, edge, &rects);
            let (c, a, name) = site_look(&world.content, site, &ui);
            let dir = Vec2::new(ray.x, -ray.y).normalize_or_zero();
            let side = Vec2::new(-dir.y, dir.x);
            let pos = rig.screen_to_world(spot);
            let c = c.with_alpha(a);
            let size = if site.goal() { 1.25 } else { 1.0 };
            let tip = pos + dir * 9.0 * size * px;
            queue.line(&regions.white, tip, pos - dir * 3.0 * px + side * 7.0 * size * px, 2.0 * px, c, Z_HUD);
            queue.line(&regions.white, tip, pos - dir * 3.0 * px - side * 7.0 * size * px, 2.0 * px, c, Z_HUD);
            if site.goal() {
                queue.dot(&regions.soft, pos, 26.0 * px, c.with_alpha(0.3), Z_HUD - 0.1);
            }
            let dist = at.distance(from);
            for (t, mut text, mut color) in &mut texts {
                if t.0 != k {
                    continue;
                }
                if t.1 == 0 {
                    set_text(&mut text, &name);
                    set_color(&mut color, ui_color(c, if a < 0.8 { 0.95 } else { 1.0 }));
                } else if dist >= 1000.0 {
                    set_text(&mut text, &format!("{:.1} KM", dist / 1000.0));
                } else {
                    set_text(&mut text, &format!("{dist:.0} M"));
                }
            }
            spots[k] = Some((spot, ray.normalize_or_zero()));
        }
    }
    // Each label beside its arrow (towards the view's centre if there is room), kept inside the view and clear of
    // the HUD and of the labels placed before it.
    let mut order: Vec<_> = labels.iter_mut().collect();
    order.sort_by_key(|l| l.0.0);
    for (label, mut node, computed, mut vis) in order {
        let want = if spots[label.0].is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
        let Some((spot, ray)) = spots[label.0] else { continue };
        let size = computed.size * computed.inverse_scale_factor;
        let r = label_spot(spot, ray, size, rig.viewport, &rects);
        rects.push(r.inflate(4.0));
        let (left, top) = (Val::Px(r.min.x.round()), Val::Px(r.min.y.round()));
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
    }
}

/// Where a site marker's label of `size` goes (logical px, y down) for an arrow at `spot` whose ray points `ray`
/// (unit, screen): towards the view's centre, else below, above, right or left of the arrow, the first place inside
/// the `viewport` that meets none of `avoid`; the first one if none is free.
fn label_spot(spot: Vec2, ray: Vec2, size: Vec2, viewport: Vec2, avoid: &[Rect]) -> Rect {
    let gap = 18.0;
    let down = Vec2::new(0.0, gap + size.y / 2.0);
    let step = Vec2::new(0.0, size.y + 6.0);
    let candidates = [
        spot - ray * (gap + 8.0 + (size.x * ray.x.abs() + size.y * ray.y.abs()) / 2.0),
        spot + down,
        spot - down,
        spot + Vec2::new(gap + size.x / 2.0, 0.0),
        spot - Vec2::new(gap + size.x / 2.0, 0.0),
        // Stacked under (or over) the others.
        spot + down + step,
        spot - down - step,
        spot + down + step * 2.0,
        spot - down - step * 2.0,
    ];
    let max = (viewport - size - Vec2::splat(6.0)).max(Vec2::splat(6.0));
    let place = |c: Vec2| {
        let at = (c - size / 2.0).clamp(Vec2::splat(6.0), max);
        Rect::from_corners(at, at + size)
    };
    candidates.iter().map(|&c| place(c)).find(|r| avoid.iter().all(|z| z.intersect(*r).is_empty())).unwrap_or_else(|| place(candidates[0]))
}

/// Where an off-screen marker sits (logical screen px, y down): on the `ray` from the view `centre` towards its
/// ship, where it meets the view's edge inset to `edge` (half sizes), then moved back along the ray until it is
/// out of every `zone`.
fn marker_spot(centre: Vec2, ray: Vec2, edge: Vec2, zones: &[Rect]) -> Vec2 {
    let mut k = (edge.x / ray.x.abs().max(1e-3)).min(edge.y / ray.y.abs().max(1e-3));
    // Where the ray enters the slab `lo..hi` of one axis, as a multiple of the ray.
    let enter = |c: f32, d: f32, lo: f32, hi: f32| {
        if d.abs() < 1e-6 { f32::NEG_INFINITY } else { ((lo - c) / d).min((hi - c) / d) }
    };
    for _ in 0..=zones.len() {
        let p = centre + ray * k;
        let Some(r) = zones.iter().find(|r| r.contains(p)) else { break };
        let t = enter(centre.x, ray.x, r.min.x, r.max.x).max(enter(centre.y, ray.y, r.min.y, r.max.y));
        if t <= 0.0 {
            // The centre itself is in it: stay at the edge.
            break;
        }
        k = t - 1.0 / ray.length().max(1e-3);
    }
    centre + ray * k
}

#[cfg(test)]
mod tests {
    use sim::Body;

    use super::*;
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
