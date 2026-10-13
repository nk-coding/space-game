//! The flight HUD in the reference look (`docs/concept/boards/ui-mockups/phone-flight-hud.png`): floating text,
//! wide-tracked uppercase labels in Barlow, white numbers, no boxes.
//!
//! * top left: the home ring (what M does: home, for touch), hull % and a tiny hull map of the player's ship (in a
//!   behavior test's watch mode: the watched one), and its hold (`CARGO 3 / 8`, salvage collected / cargo units;
//!   hidden for a ship without a hold);
//! * left: speed (m/s), the top speed and the drift state;
//! * top right: target panel (name, hull %, distance; in watch mode the watched ship's behavior target);
//! * top centre: scene / wave line and whether raider waves come (C), centre: banners (wave cleared, ship lost,
//!   `PAUSED`);
//! * bottom centre: heat and power warnings ("PULSE LASER 2 · HEAT 96 %"); bottom: the controls help line;
//! * in the world: a dotted lock ring around the target, off-screen markers for enemies (also in watch mode; kept
//!   clear of the HUD's texts and widgets).
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
use crate::ui_kit::{UiClick, UiInputSet, label, number, ring_node, set_text, tracked};
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
    Warnings,
    Help,
}

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
        .add_systems(Update, go_home.after(UiInputSet).run_if(in_state(AppMode::Flight)))
        .add_systems(OnExit(AppMode::Flight), forget_ship)
        .add_systems(PostUpdate, (lock_ring, offscreen_markers).after(CameraSet).before(crate::fx::FxFlush));
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
        .spawn((HudRoot, FlightUi, Node { width: percent(100), height: percent(100), position_type: PositionType::Absolute, ..default() }))
        .with_children(|root| {
            // Top left: the home ring (as on the drydock and the BEHAVIOR screen), then the hull.
            root.spawn(abs(Some(20.0), None, Some(18.0), None)).with_children(|c| {
                c.spawn((HomeRing, HudButton, HudZone, ring_node(34.0))).with_children(|r| {
                    r.spawn((UiIcon::Glyph { glyph: Glyph::Menu, color: ui.violet }, Node { width: px(19), height: px(19), ..default() }));
                });
            });
            root.spawn(abs(Some(68.0), None, Some(18.0), None)).with_children(|c| {
                c.spawn(label(&fonts, "HULL", 13.0, grey));
                c.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(18), ..default() })
                    .with_children(|row| {
                        row.spawn((HudText::HullValue, number(&fonts, "–", 40.0, white)));
                        row.spawn((HullMap, HudZone, Node { width: px(100), height: px(60), position_type: PositionType::Relative, ..default() }));
                    });
                c.spawn((HudText::Cargo, label(&fonts, "", 12.0, grey)));
            });
            // Left: speed, ending at mid-height (on a 390 px phone it sits between the hull block and the stick).
            root.spawn(Node { bottom: percent(50), ..abs(Some(28.0), None, None, None) }).with_children(|c| {
                c.spawn(label(&fonts, "SPEED", 12.0, grey));
                c.spawn((HudText::SpeedValue, number(&fonts, "0", 34.0, white)));
                c.spawn((HudText::SpeedSub, label(&fonts, "M/S", 11.0, dim)));
            });
            // Top right: target.
            root.spawn(Node { align_items: AlignItems::FlexEnd, ..abs(None, Some(28.0), Some(18.0), None) })
                .with_children(|c| {
                    c.spawn((HudText::TargetName, label(&fonts, "NO TARGET", 13.0, grey)));
                    c.spawn((HudText::TargetValue, number(&fonts, "", 34.0, coral)));
                    c.spawn((HudText::TargetSub, label(&fonts, "", 11.0, dim)));
                });
            // Top centre: title.
            root.spawn(Node { position_type: PositionType::Absolute, top: px(16), width: percent(100), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() })
                .with_children(|c| {
                    c.spawn((HudText::Title, label(&fonts, "", 16.0, white)));
                    c.spawn((HudText::Subtitle, label(&fonts, "", 11.0, grey)));
                });
            // Centre: banner.
            root.spawn(Node { position_type: PositionType::Absolute, top: percent(28), width: percent(100), justify_content: JustifyContent::Center, ..default() })
                .with_children(|c| {
                    c.spawn((HudText::Banner, label(&fonts, "", 22.0, teal)));
                });
            // Bottom centre: warnings, then help.
            root.spawn(Node { position_type: PositionType::Absolute, bottom: px(14), width: percent(100), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(8), ..default() })
                .with_children(|c| {
                    c.spawn((HudText::Warnings, label(&fonts, "", 14.0, coral)));
                    c.spawn((HudText::Help, tracked(&fonts, "", 10.0, dim, 0.2)));
                });
        });
}

/// The home ring: home, like M.
fn go_home(mut clicks: MessageReader<UiClick>, ring: Query<(), With<HomeRing>>, mut next: ResMut<NextState<AppMode>>) {
    if clicks.read().any(|c| ring.contains(c.0)) {
        next.set(AppMode::Home);
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
    mut texts: Query<(&HudText, &mut Text)>,
    mut root: Query<&mut Visibility, With<HudRoot>>,
) {
    for mut v in &mut root {
        let want = if toggles.hud { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
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

    for (kind, mut text) in &mut texts {
        let s = &mut *scratch;
        s.clear();
        match kind {
            HudText::HullValue => match player {
                Some(ship) => {
                    let _ = write!(s, "{:.0} %", hull_fraction(ship, control.map(|p| p.id)) * 100.0);
                }
                None => s.push('–'),
            },
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
            HudText::Title => match scene.kind {
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
            },
            HudText::Subtitle => {
                let alive = world.sim.vessels(crate::scene::ENEMY).filter(|v| v.state == sim::VesselState::Running).count();
                match scene.kind {
                    SceneKind::Sandbox => {
                        // The player's vessels are in the fleet bar (`fleet_bar.rs`).
                        let _ = write!(s, "RAIDERS {} / {}  ·  KILLS {}", alive, scene.wave_ships.len(), scene.kills);
                        s.push_str(if scene.waves { "  ·  WAVES ON" } else { "  ·  WAVES OFF (C)" });
                    }
                    SceneKind::Load(_) => {
                        let active = world.sim.ships().iter().filter(|s| s.is_active()).count();
                        let pieces: usize = world.sim.ships().iter().map(|s| s.pieces().len()).sum();
                        let _ = write!(s, "ACTIVE {}  ·  PIECES {}  ·  SHOTS IN FLIGHT {}", active, pieces, world.sim.projectiles().len());
                    }
                    SceneKind::Showcase => s.push_str("DRAG TO PAN  ·  WHEEL TO ZOOM"),
                }
            }
            HudText::Banner => {
                if virtual_time.is_paused() {
                    s.push_str("PAUSED  ·  P");
                } else if let Some((b, until)) = &scene.banner
                    && world.sim.time_seconds() < *until
                {
                    s.push_str(b);
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
                _ => s.push_str("DRAG PAN  ·  WHEEL ZOOM  ·  F FRAME / FREE  ·  1 DEBUG SHAPES  ·  2 GLOW  ·  3 BLOOM  ·  F3 STATS  ·  H HIDE  ·  T DRYDOCK  ·  M HOME"),
            },
        }
        set_text(&mut text, scratch);
    }
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
    for s in world.sim.ships() {
        if !s.is_active() || sim::friendly(team, s.team()) {
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
