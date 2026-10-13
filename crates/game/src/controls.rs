//! Player input → [`PlayerInput`] (stick, lock, fire, drift), turned into a `sim::PilotInput` each tick by the
//! scene.
//!
//! * **PC** (GDR-0040): A/D rotate (a tap nudges the nose, a hold ramps up to the key turn rate, GDR-0055), W/S
//!   thrust along the nose, Q/E strafe (arrows: rotate and thrust). Under a lock the ship aims its strongest
//!   guns and A/D nudge the aim (springs back). Left-click a ship = lock, click empty space = unlock, hold the
//!   left button on empty space (or the right button, or Space) = fire. Tab = next target, Esc = unlock, X = drift toggle. V / Shift+V = next / previous vessel of the player (the sim's
//!   vessel list); clicking a friendly vessel switches to it. Both send `ShipCommand::Focus`; the sim moves the
//!   focus and `scene::PilotFocus` mirrors it on the next tick. Clicks and taps on the fleet bar
//!   (`fleet_bar.rs`) are the bar's, never the world's.
//! * **Touch** (landscape): virtual stick bottom-left (push distance = speed), fire button bottom-right, tap a
//!   ship = lock, tap empty space = unlock, two fingers = zoom. The touch widgets appear after the first touch
//!   (or with `?touch=1`).

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::input::{egui_wants_any_keyboard_input, egui_wants_any_pointer_input};
use sim::{ShipCommand, ShipId, Target};

use crate::camera::CameraRig;
use crate::fleet_bar::FleetBar;
use crate::mode::{AppMode, FlightUi};
use crate::scene::{PilotFocus, is_friendly, player_vessels};
use crate::sim_bridge::{SimWorld, piece_com, sv, v2};
use crate::style::{Style, ui_color};

/// Hold the left button this long on empty space to fire (shorter = a click = unlock).
const HOLD_TO_FIRE: f32 = 0.18;
const STICK_RADIUS: f32 = 70.0;
const FIRE_RADIUS: f32 = 56.0;
const TAP_TIME: f32 = 0.3;

#[derive(Resource, Default)]
pub struct PlayerInput {
    /// Touch stick: world direction, length 0..1.
    pub stick: Vec2,
    /// Keyboard axes (turn +1 = A / left, thrust +1 = W, strafe +1 = E / right), used unless the touch stick is.
    pub axes: Vec3,
    /// The touch stick is in use (stick mode instead of the keyboard axes).
    pub stick_mode: bool,
    /// The A/D aim nudge under a lock (integrated once per sim tick with `sim::flight::nudge_step`).
    pub nudge: f32,
    /// The A/D rotation ramp without a lock, sent as `PilotAxes::turn` (integrated once per sim tick with
    /// `sim::flight::turn_ramp_step`, GDR-0055).
    pub turn_ramp: f32,
    pub fire: bool,
    pub lock: Option<Target>,
    pub drift: bool,
    /// Show the touch widgets.
    pub touch_ui: bool,
    key_axes: Vec3,
    key_fire: bool,
    lmb: Option<LmbPress>,
    stick_touch: Option<(u64, Vec2)>,
    fire_touch: Option<u64>,
    tap: Option<(u64, Vec2, f32)>,
}

#[derive(Clone, Copy)]
struct LmbPress {
    at: Vec2,
    time: f32,
    on_ship: bool,
}

impl PlayerInput {
    /// Is this touch used by the stick or the fire button (camera pinch ignores it)?
    pub fn captures(&self, id: u64) -> bool {
        self.stick_touch.is_some_and(|(t, _)| t == id) || self.fire_touch == Some(id)
    }
    pub fn locked_ship(&self) -> Option<ShipId> {
        match self.lock {
            Some(Target::Ship(s)) | Some(Target::Piece(s, _)) => Some(s),
            _ => None,
        }
    }
}

/// The input systems (keyboard, mouse, touch → [`PlayerInput`]), in `Update`, in flight only.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ControlsSet;

#[derive(Component)]
struct StickBase;
#[derive(Component)]
struct StickKnob;
#[derive(Component)]
struct FireButton;

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerInput>()
            .configure_sets(Update, ControlsSet.run_if(in_state(AppMode::Flight)))
            .add_systems(Startup, spawn_touch_ui)
            .add_systems(
                Update,
                (
                    release_keys.run_if(egui_wants_any_keyboard_input),
                    keyboard.run_if(not(egui_wants_any_keyboard_input)),
                    mouse.run_if(not(egui_wants_any_pointer_input)),
                    touch,
                    combine,
                    touch_ui,
                )
                    .chain()
                    .in_set(ControlsSet),
            );
    }
}

/// While egui has the keyboard, the ship gets no key input.
fn release_keys(mut input: ResMut<PlayerInput>) {
    input.key_axes = Vec3::ZERO;
    input.key_fire = false;
}

fn keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut input: ResMut<PlayerInput>,
    mut world: ResMut<SimWorld>,
    focus: Res<PilotFocus>,
    rig: Res<CameraRig>,
) {
    let k = |a: KeyCode, b: KeyCode| keys.pressed(a) || keys.pressed(b);
    let axis = |pos: bool, neg: bool| (pos as i32 - neg as i32) as f32;
    input.key_axes = Vec3::new(
        axis(k(KeyCode::KeyA, KeyCode::ArrowLeft), k(KeyCode::KeyD, KeyCode::ArrowRight)),
        axis(k(KeyCode::KeyW, KeyCode::ArrowUp), k(KeyCode::KeyS, KeyCode::ArrowDown)),
        axis(keys.pressed(KeyCode::KeyE), keys.pressed(KeyCode::KeyQ)),
    );
    input.key_fire = keys.pressed(KeyCode::Space);
    if keys.just_pressed(KeyCode::KeyX) {
        input.drift = !input.drift;
    }
    if keys.just_pressed(KeyCode::Escape) {
        input.lock = None;
    }
    if keys.just_pressed(KeyCode::Tab) {
        input.lock = next_target(&world, focus.0, input.locked_ship(), rig.focus);
    }
    // Cycle the focus through the player's vessels (a focus command for the sim).
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let step = if keys.just_pressed(KeyCode::KeyV) { if shift { -1 } else { 1 } } else { 0 };
    if step != 0 && focus.0.is_some() {
        let vessels = player_vessels(&world);
        if !vessels.is_empty() {
            let k = focus.0.and_then(|f| vessels.iter().position(|v| *v == f)).unwrap_or(0) as i32;
            let n = vessels.len() as i32;
            let next = vessels[((k + step) % n + n) as usize % vessels.len()];
            world.inputs.commands.push((next, ShipCommand::Focus));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn mouse(
    time: Res<Time<Real>>,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut world: ResMut<SimWorld>,
    focus: Res<PilotFocus>,
    rig: Res<CameraRig>,
    fixed: Res<Time<Fixed>>,
    fleet: Res<FleetBar>,
    mut input: ResMut<PlayerInput>,
) {
    if focus.0.is_none() {
        return;
    }
    let now = time.elapsed_secs();
    let cursor = window.cursor_position();
    if buttons.just_pressed(MouseButton::Left)
        && let Some(c) = cursor
        && !fleet.blocks(c)
    {
        let alpha = fixed.overstep_fraction();
        let hit = pick(&world, focus.0, rig.screen_to_world(c), 1.0 / rig.zoom, alpha);
        apply_pick(hit, &mut world, &focus, &mut input);
        input.lmb = Some(LmbPress { at: c, time: now, on_ship: hit.is_some() });
    }
    let mut lmb_fire = false;
    if let Some(p) = input.lmb {
        if buttons.pressed(MouseButton::Left) {
            lmb_fire = !p.on_ship && now - p.time >= HOLD_TO_FIRE;
        }
        if buttons.just_released(MouseButton::Left) {
            let short = now - p.time < HOLD_TO_FIRE && cursor.is_none_or(|c| c.distance(p.at) < 8.0);
            if !p.on_ship && short {
                input.lock = None;
            }
            input.lmb = None;
        }
    }
    input.key_fire = input.key_fire || lmb_fire || buttons.pressed(MouseButton::Right);
}

#[allow(clippy::too_many_arguments)]
fn touch(
    time: Res<Time<Real>>,
    touches: Res<Touches>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut world: ResMut<SimWorld>,
    focus: Res<PilotFocus>,
    rig: Res<CameraRig>,
    fixed: Res<Time<Fixed>>,
    fleet: Res<FleetBar>,
    mut input: ResMut<PlayerInput>,
) {
    let size = window.size();
    let stick_c = Vec2::new(40.0 + STICK_RADIUS + 10.0, size.y - 40.0 - STICK_RADIUS - 10.0);
    let fire_c = Vec2::new(size.x - 50.0 - FIRE_RADIUS, size.y - 50.0 - FIRE_RADIUS);
    let now = time.elapsed_secs();
    for t in touches.iter_just_pressed() {
        input.touch_ui = true;
        let p = t.position();
        if fleet.blocks(p) {
            continue;
        }
        if focus.0.is_some() && input.stick_touch.is_none() && p.distance(stick_c) < STICK_RADIUS * 1.7 {
            input.stick_touch = Some((t.id(), stick_c));
        } else if focus.0.is_some() && input.fire_touch.is_none() && p.distance(fire_c) < FIRE_RADIUS * 1.4 {
            input.fire_touch = Some(t.id());
        } else {
            input.tap = Some((t.id(), p, now));
        }
    }
    for t in touches.iter_just_released().chain(touches.iter_just_canceled()) {
        if input.stick_touch.is_some_and(|(id, _)| id == t.id()) {
            input.stick_touch = None;
        }
        if input.fire_touch == Some(t.id()) {
            input.fire_touch = None;
        }
        if let Some((id, at, t0)) = input.tap
            && id == t.id()
        {
            input.tap = None;
            if now - t0 < TAP_TIME && t.position().distance(at) < 14.0 && focus.0.is_some() {
                let alpha = fixed.overstep_fraction();
                let hit = pick(&world, focus.0, rig.screen_to_world(at), 1.0 / rig.zoom, alpha);
                if hit.is_none() {
                    input.lock = None;
                }
                apply_pick(hit, &mut world, &focus, &mut input);
            }
        }
    }
}

/// Stick, axes and fire from all sources: the touch stick wins while it is held, else the keyboard axes.
fn combine(touches: Res<Touches>, mut input: ResMut<PlayerInput>) {
    let mut stick = None;
    if let Some((id, centre)) = input.stick_touch
        && let Some(t) = touches.get_pressed(id)
    {
        let d = t.position() - centre;
        let v = Vec2::new(d.x, -d.y) / STICK_RADIUS;
        stick = Some(if v.length() > 1.0 { v.normalize() } else { v });
    }
    input.stick_mode = stick.is_some();
    input.stick = stick.unwrap_or(Vec2::ZERO);
    input.axes = input.key_axes;
    input.fire = input.key_fire || input.fire_touch.is_some();
}

/// A click or tap on a ship: the player's vessels take the focus (a command for the sim), anything else is
/// locked.
fn apply_pick(hit: Option<Target>, world: &mut SimWorld, focus: &PilotFocus, input: &mut PlayerInput) {
    match hit {
        Some(Target::Ship(id)) if is_friendly(world, id) => {
            if focus.0 != Some(id) {
                world.inputs.commands.push((id, ShipCommand::Focus));
            }
        }
        Some(t) => input.lock = Some(t),
        None => {}
    }
}

/// The ship (or salvage piece) under a world point: exact part test in each piece frame, else the nearest
/// piece centre within a few pixels' worth of slack.
pub fn pick(world: &SimWorld, player: Option<ShipId>, at: Vec2, m_per_px: f32, alpha: f32) -> Option<Target> {
    let mut best: Option<(f32, Target)> = None;
    for ship in world.sim.ships() {
        if Some(ship.id()) == player {
            continue;
        }
        for piece in ship.pieces() {
            let (pos, angle) = piece.render_pose(alpha);
            let local = Mat2::from_angle(-angle) * (at - v2(pos));
            let target = if piece.controlled { Target::Ship(ship.id()) } else { Target::Piece(ship.id(), piece.id) };
            if ship.part_at_local(piece.id, sv(local)).is_some() {
                return Some(target);
            }
            let d = piece_com(piece, alpha).distance(at);
            let slack = 2.0 + 20.0 * m_per_px;
            if d < slack && best.is_none_or(|b| d < b.0) {
                best = Some((d, target));
            }
        }
    }
    best.map(|b| b.1)
}

/// Next active enemy ship by distance from the player (wraps around).
pub fn next_target(world: &SimWorld, player: Option<ShipId>, current: Option<ShipId>, from: Option<Vec2>) -> Option<Target> {
    let player_ship = player.and_then(|p| world.sim.ship(p))?;
    let team = player_ship.team();
    let from = from.unwrap_or(Vec2::ZERO);
    let mut list: Vec<(f32, ShipId)> = world
        .sim
        .ships()
        .iter()
        .filter(|s| s.is_active() && !sim::friendly(team, s.team()))
        .filter_map(|s| s.control_piece().and_then(|c| s.piece(c)).map(|p| (v2(p.com_world()).distance(from), s.id())))
        .collect();
    list.sort_by(|a, b| a.0.total_cmp(&b.0));
    if list.is_empty() {
        return None;
    }
    let k = current.and_then(|c| list.iter().position(|e| e.1 == c)).map(|k| (k + 1) % list.len()).unwrap_or(0);
    Some(Target::Ship(list[k].1))
}

fn spawn_touch_ui(mut commands: Commands, style: Res<Style>, params: Res<crate::params::Params>, mut input: ResMut<PlayerInput>) {
    input.touch_ui = params.flag("touch").unwrap_or(false);
    let teal = style.glow_player;
    let coral = style.glow_enemy;
    let d = STICK_RADIUS * 2.0;
    commands
        .spawn((
            StickBase,
            FlightUi,
            Node {
                position_type: PositionType::Absolute,
                left: px(50),
                bottom: px(50),
                width: px(d),
                height: px(d),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::MAX,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(ui_color(teal, 0.45)),
            BackgroundColor(ui_color(teal, 0.04)),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn((
                StickKnob,
                Node {
                    position_type: PositionType::Absolute,
                    width: px(56),
                    height: px(56),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BorderColor::all(ui_color(teal, 0.8)),
                BackgroundColor(ui_color(teal, 0.18)),
            ));
        });
    let f = FIRE_RADIUS * 2.0;
    commands
        .spawn((
            FireButton,
            FlightUi,
            Node {

                position_type: PositionType::Absolute,
                right: px(50),
                bottom: px(50),
                width: px(f),
                height: px(f),
                border: UiRect::all(px(3)),
                border_radius: BorderRadius::MAX,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(ui_color(coral, 0.7)),
            BackgroundColor(ui_color(coral, 0.06)),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("FIRE"),
                TextFont { font_size: FontSize::Px(13.0), ..default() },
                TextColor(ui_color(coral, 0.9)),
            ));
        });
}

#[allow(clippy::type_complexity)]
fn touch_ui(
    input: Res<PlayerInput>,
    focus: Res<PilotFocus>,
    touches: Res<Touches>,
    mut base: Query<&mut Visibility, (With<StickBase>, Without<FireButton>)>,
    mut fire: Query<(&mut Visibility, &mut BackgroundColor), (With<FireButton>, Without<StickBase>)>,
    mut knob: Query<&mut Node, With<StickKnob>>,
    style: Res<Style>,
) {
    let show = input.touch_ui && focus.0.is_some();
    let want = if show { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut base {
        if *v != want {
            *v = want;
        }
    }
    for (mut v, mut bg) in &mut fire {
        if *v != want {
            *v = want;
        }
        let a = if input.fire_touch.is_some() { 0.3 } else { 0.06 };
        let c = ui_color(style.glow_enemy, a);
        if bg.0 != c {
            bg.0 = c;
        }
    }
    if !show {
        return;
    }
    let mut off = Vec2::ZERO;
    if let Some((id, centre)) = input.stick_touch
        && let Some(t) = touches.get_pressed(id)
    {
        let d = t.position() - centre;
        off = if d.length() > STICK_RADIUS { d.normalize() * STICK_RADIUS } else { d };
    }
    for mut n in &mut knob {
        let l = px(STICK_RADIUS - 28.0 - 2.0 + off.x);
        let t = px(STICK_RADIUS - 28.0 - 2.0 + off.y);
        if n.left != l || n.top != t {
            n.left = l;
            n.top = t;
        }
    }
}
