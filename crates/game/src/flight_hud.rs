//! The flight HUD in the reference look (`docs/concept/boards/ui-mockups/phone-flight-hud.png`): floating text,
//! wide-tracked uppercase labels in Barlow, white numbers, no boxes.
//!
//! * top left: hull % and a tiny hull map of the player's ship;
//! * left: speed (m/s), the top speed and the drift state;
//! * top right: target panel (name, hull %, distance);
//! * top centre: scene / wave line, centre: banners (wave cleared, ship lost);
//! * bottom centre: heat and power warnings ("PULSE LASER 2 · HEAT 96 %"); bottom: the controls help line;
//! * in the world: a dotted lock ring around the target, off-screen markers for enemies.
//!
//! Text is rebuilt at 8 Hz and only re-laid out when it changes.

use std::fmt::Write as _;

use bevy::prelude::*;
use bevy::text::LetterSpacing;
use sim::{ShipId, Stat, Target};

use crate::camera::{CameraRig, CameraSet};
use crate::controls::PlayerInput;
use crate::fx::{FxQueue, FxRegions, Z_HUD};
use crate::scene::{PilotFocus, Scene, SceneKind};
use crate::sim_bridge::{SimWorld, piece_com, v2};
use crate::style::{Style, ui_color};
use crate::{RenderToggles, ship_view::PieceView};

static BARLOW_REGULAR: &[u8] = include_bytes!("../assets/fonts/Barlow-Regular.ttf");
static BARLOW_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Barlow-Medium.ttf");

#[derive(Resource, Clone)]
pub struct HudFonts {
    pub regular: Handle<Font>,
    pub medium: Handle<Font>,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum HudText {
    HullValue,
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

#[derive(Resource)]
struct HudState {
    timer: Timer,
    /// The ship the hull map shows.
    map_ship: Option<ShipId>,
    /// Display label per part of the player's design ("PULSE LASER 2").
    labels: Vec<String>,
    scratch: String,
}

pub struct FlightHudPlugin;

impl Plugin for FlightHudPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(HudState {
            timer: Timer::from_seconds(0.125, TimerMode::Repeating),
            map_ship: None,
            labels: Vec::new(),
            scratch: String::with_capacity(256),
        })
        .add_systems(Startup, spawn_hud)
        .add_systems(Update, (update_text, update_hull_map).chain())
        .add_systems(PostUpdate, (lock_ring, offscreen_markers).after(CameraSet).before(crate::fx::FxFlush));
    }
}

fn label(fonts: &HudFonts, text: &str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont { font: fonts.regular.clone().into(), font_size: FontSize::Px(size), ..default() },
        TextColor(color),
        LetterSpacing::Px(size * 0.3),
    )
}

fn number(fonts: &HudFonts, text: &str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont { font: fonts.medium.clone().into(), font_size: FontSize::Px(size), ..default() },
        TextColor(color),
        LetterSpacing::Px(size * 0.06),
    )
}

fn spawn_hud(mut commands: Commands, mut fonts_assets: ResMut<Assets<Font>>, style: Res<Style>) {
    let fonts = HudFonts {
        regular: fonts_assets.add(Font::from_bytes(BARLOW_REGULAR.to_vec())),
        medium: fonts_assets.add(Font::from_bytes(BARLOW_MEDIUM.to_vec())),
    };
    commands.insert_resource(fonts.clone());
    let grey = ui_color(style.ui_label, 1.0);
    let white = ui_color(Color::srgb_u8(0xeb, 0xf5, 0xff), 1.0);
    let coral = ui_color(style.glow_enemy, 1.0);
    let teal = ui_color(style.glow_player, 1.0);
    let dim = ui_color(Color::srgb_u8(0x5d, 0x6a, 0x74), 1.0);
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
        .spawn((HudRoot, Node { width: percent(100), height: percent(100), position_type: PositionType::Absolute, ..default() }))
        .with_children(|root| {
            // Top left: hull.
            root.spawn(abs(Some(28.0), None, Some(18.0), None)).with_children(|c| {
                c.spawn(label(&fonts, "HULL", 13.0, grey));
                c.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(18), ..default() })
                    .with_children(|row| {
                        row.spawn((HudText::HullValue, number(&fonts, "–", 40.0, white)));
                        row.spawn((HullMap, Node { width: px(100), height: px(60), position_type: PositionType::Relative, ..default() }));
                    });
            });
            // Left: speed.
            root.spawn(abs(Some(28.0), None, None, Some(250.0))).with_children(|c| {
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
                    c.spawn((HudText::Help, label(&fonts, "", 10.0, dim)));
                });
        });
}

fn set_text(text: &mut Text, s: &str) {
    if text.0 != s {
        text.0.clear();
        text.0.push_str(s);
    }
}

#[allow(clippy::too_many_arguments)]
fn update_text(
    time: Res<Time<Real>>,
    mut state: ResMut<HudState>,
    world: Res<SimWorld>,
    scene: Res<Scene>,
    focus: Res<PilotFocus>,
    input: Res<PlayerInput>,
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
    let HudState { scratch, labels, .. } = &mut *state;
    let alpha = fixed.overstep_fraction();
    let player = focus.0.and_then(|id| world.sim.ship(id));
    let control = player.and_then(|s| s.control_piece().and_then(|c| s.piece(c)));

    // Labels per part of the player's design.
    if let Some(s) = player
        && labels.len() != s.design().parts.len()
    {
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
                target_text(*kind, s, &world, &input, control.map(|p| piece_com(p, alpha)).or(rig.focus));
            }
            HudText::Title => match scene.kind {
                SceneKind::Sandbox => {
                    let _ = write!(s, "SANDBOX  ·  WAVE {}", scene.wave);
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
                        let _ = write!(s, "RAIDERS {} / {}  ·  KILLS {}", alive, scene.wave_ships.len(), scene.kills);
                        let vessels: Vec<ShipId> = crate::scene::player_vessels(&world);
                        if vessels.len() > 1 {
                            let k = focus.0.and_then(|f| vessels.iter().position(|v| *v == f)).map(|k| k + 1).unwrap_or(0);
                            let _ = write!(s, "  ·  VESSEL {k} / {}  (V)", vessels.len());
                        }
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
                if let Some((b, until)) = &scene.banner
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
                SceneKind::Sandbox if input.touch_ui => {
                    s.push_str("STICK FLY  ·  TAP SHIP LOCK  ·  FIRE BUTTON  ·  PINCH ZOOM");
                }
                SceneKind::Sandbox => s.push_str(
                    "A/D TURN · W/S THRUST · Q/E STRAFE · CLICK LOCK (A/D NUDGE) · MOUSE/SPACE FIRE · TAB TARGET · X DRIFT · V VESSEL · T TUNING · F3 DEBUG · H HIDE",
                ),
                _ => s.push_str("DRAG PAN  ·  WHEEL ZOOM  ·  F FRAME / FREE  ·  1 DEBUG SHAPES  ·  2 GLOW  ·  3 BLOOM  ·  F3 STATS  ·  H HIDE"),
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

fn target_text(kind: HudText, s: &mut String, world: &SimWorld, input: &PlayerInput, from: Option<Vec2>) {
    let Some(lock) = input.lock else {
        if kind == HudText::TargetName {
            s.push_str("NO TARGET");
        } else if kind == HudText::TargetSub {
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
            let name = world.content.ships.get(&ship.design().id).map(|b| b.name.as_str()).unwrap_or("UNKNOWN");
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

/// The tiny hull map: one cell per blueprint cell of the player's ship, teal by hit points, coral when lost.
#[allow(clippy::too_many_arguments)]
fn update_hull_map(
    mut commands: Commands,
    mut state: ResMut<HudState>,
    world: Res<SimWorld>,
    focus: Res<PilotFocus>,
    style: Res<Style>,
    map: Query<Entity, With<HullMap>>,
    mut cells: Query<(&HullCell, &mut BackgroundColor)>,
) {
    let Ok(map) = map.single() else { return };
    let ship = focus.0.and_then(|id| world.sim.ship(id));
    if ship.map(|s| s.id()) != state.map_ship {
        state.map_ship = ship.map(|s| s.id());
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
            if f < 0.5 { ui_color(Color::srgb_u8(0xe2, 0xb1, 0x5c), 0.85) } else { ui_color(style.glow_player, 0.45 + 0.45 * f) }
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

/// Markers at the screen edge pointing at active enemies outside the view.
#[allow(clippy::too_many_arguments)]
fn offscreen_markers(
    world: Res<SimWorld>,
    focus: Res<PilotFocus>,
    input: Res<PlayerInput>,
    rig: Res<CameraRig>,
    regions: Option<Res<FxRegions>>,
    style: Res<Style>,
    toggles: Res<RenderToggles>,
    mut queue: ResMut<FxQueue>,
) {
    let Some(regions) = regions else { return };
    let Some(me) = focus.0.and_then(|id| world.sim.ship(id)) else { return };
    if !toggles.hud {
        return;
    }
    let team = me.team();
    let px = 1.0 / rig.zoom;
    let half = rig.half_extent();
    let inset = Vec2::splat(34.0 * px);
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
        let box_half = half - inset;
        let scale = (box_half.x / d.x.abs().max(1e-3)).min(box_half.y / d.y.abs().max(1e-3));
        let pos = rig.pos + d * scale;
        let dir = d.normalize_or_zero();
        let side = Vec2::new(-dir.y, dir.x);
        let locked = input.locked_ship() == Some(s.id());
        let c = style.glow_enemy.with_alpha(if locked { 1.0 } else { 0.75 });
        let tip = pos + dir * 9.0 * px;
        queue.line(&regions.white, tip, pos - dir * 3.0 * px + side * 7.0 * px, 2.0 * px, c, Z_HUD);
        queue.line(&regions.white, tip, pos - dir * 3.0 * px - side * 7.0 * px, 2.0 * px, c, Z_HUD);
        queue.dot(&regions.soft, pos, 18.0 * px, c.with_alpha(0.25), Z_HUD - 0.1);
    }
}
