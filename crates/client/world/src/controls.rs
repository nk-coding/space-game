//! Player input → [`PlayerInput`] (stick, lock, fire, drift), turned into a `sim::PilotInput` each tick by the
//! scene.
//!
//! * **PC** (GDR-0040): A/D rotate (a tap nudges the nose, a hold ramps up to the key turn rate, GDR-0055), W/S
//!   thrust along the nose, Q/E strafe (arrows: rotate and thrust). Under a lock the ship aims its strongest
//!   guns and A/D nudge the aim (springs back). Left-click a ship = lock, click empty space = unlock, hold the
//!   left button on empty space = fire weapon group 1. Space fires every group, 1 / 2 / 3 (and the right button:
//!   2) fire one group each (GDR-0088); the held triggers make the pilot's fire mask. L (LOCK TARGET) = the nearest
//!   enemy, then the next by distance (`sim::flight::next_lock`), Esc = unlock (not while
//!   the ABANDON VOYAGE? panel is open: Esc closes it, `voyage.rs`), X = drift toggle, G (SCAN) = a scan with the
//!   flown ship's scanner (`ShipCommand::Scan`; the HUD's SCAN pill does the same), N (FLARES) = its flare launcher
//!   (`ShipCommand::Flare`, GDR-0115). V / Shift+V = next / previous
//!   vessel of the player (the sim's vessel list); clicking a friendly vessel switches to it. Both send
//!   `ShipCommand::Focus`; the sim moves the focus and `scene::PilotFocus` mirrors it on the next tick. F1–F4 (ASSIST
//!   TOGGLE 1–4) and Z (ALL ASSISTS) switch the flown ship's pilot assists for the mission
//!   (`ShipCommand::SwitchAssists`, GDR-0088); the HUD's banner says what changed ([`assist_switches`]). Clicks and
//!   taps on the fleet bar (`fleet_bar.rs`) and the HUD's buttons (`flight_hud::HudButton`: the home ring, RECALL,
//!   the ABANDON VOYAGE? panel) are theirs, never the world's.
//! * **Touch** (landscape; docs/plans/android.md 4.2, GDR-0120): a floating stick (where the left thumb lands in the
//!   left 45 % of the window under the speed block; push distance = speed, a dead zone of 6 % of its radius), FIRE
//!   bottom right (held: every group), FLARE beside it for a ship with a flare launcher, BLINK above it for a ship
//!   with a blink drive (GDR-0129: along the stick while it is pushed, else ahead), tap a ship = lock (2 m +
//!   24 dp of slack), tap empty space = unlock, a tap on the target panel = LOCK TARGET (`flight_hud/phone.rs`), two
//!   fingers = zoom and pan (`camera.rs`). A tap is shorter than 0.3 s and moves less than 10 dp. The touch widgets
//!   show from the start on a touch screen (`kit::ui_kit::TouchScreen`: Android, a coarse pointer in the browser,
//!   or after a first touch). A finger that lands on a HUD button (within its 48 dp touch target) or on
//!   the fleet bar is the UI's: it never steers, fires, locks or pans.
//!
//! **Timing.** The input systems run every frame before the frame's sim ticks (`RunFixedMainLoop`, before the
//! fixed loop), so a press reaches the tick of the same frame. Between ticks the keys and fire buttons are latched
//! ([`PlayerInput::for_tick`]): a tap shorter than a tick (33 ms), even one pressed and released within one frame,
//! still reaches the next tick. The sim gets the result as data, once per tick (`scene::scene_tick`).

use bevy::app::RunFixedMainLoopSystems;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::input::{egui_wants_any_keyboard_input, egui_wants_any_pointer_input};
use sim::{AssistSwitch, FIRE_ALL, ShipCommand, ShipId, Target};
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use kit::style::{Style, ui_color};
use kit::ui_kit::{HudFonts, TouchScreen, UiClick, label, ring_node};

use crate::camera::CameraRig;
use crate::fleet_bar::FleetBar;
use crate::flight_hud::{HudButton, HudButtons, HudZone, StickZone, logical_rect, on_hud_button, switch_note};
use crate::mode::{AppMode, FlightUi};
use crate::scene::{Scene, is_friendly, player_vessels};
use crate::sim_bridge::{PilotFocus, SimWorld, piece_com, sv, v2};
use crate::flight_hud::{FlightMenu, Overview};
use crate::settings::{Act, ActionTaps, SettingsLayer};
use crate::voyage::VoyageRun;
use data::input::Action;

/// Hold the left button this long on empty space to fire (shorter = a click = unlock).
const HOLD_TO_FIRE: f32 = 0.18;
/// The touch stick (docs/plans/android.md 4.2; *placeholders*): its ring and knob (dp), the dead zone (a share of
/// the radius; full speed at the rim), its ring at rest (from the window's left and bottom edges, inside the phone's
/// safe area) and its zone (the left share of the window, under the speed block).
const STICK_RING: f32 = 124.0;
const STICK_KNOB: f32 = 44.0;
const STICK_DEAD: f32 = 0.06;
const STICK_REST: Vec2 = Vec2::new(32.0, 24.0);
const STICK_ZONE: f32 = 0.45;
/// FIRE (dp): the button, its hit area and its distance from the right and bottom edges.
const FIRE_SIZE: f32 = 76.0;
const FIRE_HIT: f32 = 96.0;
const FIRE_EDGE: f32 = 32.0;
/// An ability's slot beside FIRE (FLARE left of it, BLINK above it; dp) and its gap to FIRE.
const SLOT: f32 = 44.0;
const SLOT_GAP: f32 = 14.0;
/// A tap: shorter than this (s), and moved less than [`FINGER_SLOP`].
const TAP_TIME: f32 = 0.3;
/// How far (dp) a finger moves before it drags (pans) instead of tapping (the kit's and the drydock's 10 dp).
pub const FINGER_SLOP: f32 = 10.0;
/// A pick's slack around a piece's centre beside its 2 m: a click's and a finger's (screen px, dp).
pub const CLICK_SLACK: f32 = 20.0;
pub const TAP_SLACK: f32 = 24.0;
/// The weapon group the held left button fires (GDR-0088).
const LMB_GROUP: u8 = 1;
/// How long the banner says what an assist switch did (sim seconds).
const SWITCH_BANNER: f64 = 1.5;

/// The pilot-assist switches whose action `pressed` says was pressed (GDR-0088): ASSIST TOGGLE 1–4, then ALL
/// ASSISTS, each with its action (for its key's label).
pub fn assist_switches(pressed: impl Fn(Action) -> bool) -> impl Iterator<Item = (AssistSwitch, Action)> {
    (1..=data::MAX_ASSIST_TOGGLES)
        .filter_map(|k| Action::assist_toggle(k).map(|a| (AssistSwitch::Toggle(k), a)))
        .chain([(AssistSwitch::All, Action::AllAssists)])
        .filter(move |(_, a)| pressed(*a))
}

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
    /// The weapon groups whose triggers are held (`sim::PilotInput::fire`: bit g-1 for group g).
    pub fire: u8,
    pub lock: Option<Target>,
    pub drift: bool,
    key_axes: Vec3,
    key_fire: u8,
    /// What was held since the last tick.
    latch: TickLatch,
    lmb: Option<LmbPress>,
    /// The stick's finger, where it landed (the stick's centre) and the ring's radius (logical px).
    stick_touch: Option<(u64, Vec2, f32)>,
    fire_touch: Option<u64>,
    tap: Option<(u64, Vec2, f32)>,
    /// Fingers that landed on the UI (a HUD button, the fleet bar, SETTINGS): kept until the frame after they lift.
    ui_touches: Vec<u64>,
}

#[derive(Clone, Copy)]
struct LmbPress {
    at: Vec2,
    time: f32,
    on_ship: bool,
}

impl PlayerInput {
    /// Is this touch the stick's, FIRE's or the UI's (the camera's pinch and pan and the replay's taps ignore it)?
    pub fn captures(&self, id: u64) -> bool {
        self.stick_touch.is_some_and(|(t, ..)| t == id) || self.fire_touch == Some(id) || self.ui_touches.contains(&id)
    }
    pub fn locked_ship(&self) -> Option<ShipId> {
        match self.lock {
            Some(Target::Ship(s)) | Some(Target::Piece(s, _)) => Some(s),
            _ => None,
        }
    }

    /// The keyboard axes and fire mask for one sim tick: what is held now, else what was held since the last
    /// tick (a tap between two ticks); then the latch starts over. Called once per tick by the scene.
    pub fn for_tick(&mut self) -> (Vec3, u8) {
        self.latch.take(self.axes, self.fire)
    }
}

/// The keyboard axes and fire groups held at any frame since the last sim tick (frames run faster than ticks).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct TickLatch {
    /// Per axis (turn, thrust, strafe): held towards + / towards −.
    pos: [bool; 3],
    neg: [bool; 3],
    fire: u8,
}

impl TickLatch {
    /// One frame's input.
    fn note(&mut self, axes: Vec3, fire: u8) {
        for k in 0..3 {
            self.pos[k] |= axes[k] > 0.0;
            self.neg[k] |= axes[k] < 0.0;
        }
        self.fire |= fire;
    }

    /// The tick's input: each axis as held now, or else as held since the last tick; each fire group if held at
    /// all.
    fn take(&mut self, axes: Vec3, fire: u8) -> (Vec3, u8) {
        let mut out = axes;
        for k in 0..3 {
            if out[k] == 0.0 {
                out[k] = (self.pos[k] as i32 - self.neg[k] as i32) as f32;
            }
        }
        let fire = fire | self.fire;
        *self = TickLatch::default();
        (out, fire)
    }
}

/// The input systems (keyboard, mouse, touch → [`PlayerInput`]), in flight only, every frame before the sim
/// ticks: in `RunFixedMainLoop`, [`RunFixedMainLoopSystems::BeforeFixedMainLoop`].
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ControlsSet;

/// The input systems in [`ControlsSet`], before the frame's sim ticks, in flight only.
fn add_input_systems(app: &mut App) {
    app.init_resource::<PlayerInput>()
        .configure_sets(
            RunFixedMainLoop,
            ControlsSet.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop).run_if(in_state(AppMode::Flight)),
        )
        .add_systems(
            RunFixedMainLoop,
            (
                release_keys.run_if(egui_wants_any_keyboard_input.or_eager(settings_open)),
                keyboard.run_if(not(egui_wants_any_keyboard_input).and_eager(not(settings_open))),
                mouse.run_if(not(egui_wants_any_pointer_input).and_eager(not(settings_open))),
                touch,
                combine,
            )
                .chain()
                .in_set(ControlsSet),
        );
}

#[derive(Component)]
struct StickBase;
#[derive(Component)]
struct StickKnob;
#[derive(Component)]
struct FireButton;
#[derive(Component)]
struct FlareButton;
#[derive(Component)]
struct BlinkButton;
/// The holder of an ability's slot (FLARE, BLINK): a flight UI root, hidden by `mode` when the flight ends and at a
/// start elsewhere, so `touch_ui` shows it again in flight.
#[derive(Component)]
struct SlotHolder;

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        add_input_systems(app);
        app.add_systems(Startup, spawn_touch_ui.after(kit::ui_kit::load_fonts))
            .add_systems(Update, (touch_ui, (flare_tap, blink_tap).after(kit::ui_kit::UiInputSet)).run_if(in_state(AppMode::Flight)));
    }
}

/// SETTINGS lies over the flight: its keys are the screen's (rebinding), not the ship's.
fn settings_open(layer: Res<SettingsLayer>) -> bool {
    layer.open
}

/// While egui has the keyboard (or SETTINGS is open), the ship gets no key input.
fn release_keys(mut input: ResMut<PlayerInput>) {
    input.key_axes = Vec3::ZERO;
    input.key_fire = 0;
}

#[allow(clippy::too_many_arguments)]
fn keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    act: Act,
    mut input: ResMut<PlayerInput>,
    mut world: ResMut<SimWorld>,
    focus: Res<PilotFocus>,
    run: Res<VoyageRun>,
    mut menu: ResMut<FlightMenu>,
    mut overview: ResMut<Overview>,
    mut scene: ResMut<Scene>,
) {
    // Held this frame, or pressed and released within it (a quick tap).
    let down = |a: Action| act.pressed(a) || act.just_pressed(a);
    let axis = |pos: bool, neg: bool| (pos as i32 - neg as i32) as f32;
    input.key_axes = Vec3::new(
        axis(down(Action::TurnLeft), down(Action::TurnRight)),
        axis(down(Action::ThrustForward), down(Action::ThrustBack)),
        axis(down(Action::StrafeRight), down(Action::StrafeLeft)),
    );
    // Every held trigger adds its groups.
    input.key_fire = if down(Action::Fire) { FIRE_ALL } else { 0 };
    for g in 1..=data::MAX_WEAPON_GROUP {
        if Action::fire_group(g).is_some_and(down) {
            input.key_fire |= 1 << (g - 1);
        }
    }
    if act.just_pressed(Action::Drift) {
        input.drift = !input.drift;
    }
    if let Some(me) = focus.0 {
        for (switch, action) in assist_switches(|a| act.just_pressed(a)) {
            // Said from the state before the switch (the sim flips it at the next tick).
            let assists = world.sim.ship(me).and_then(|s| s.assists());
            let (line, tone) = switch_note(&world.content, assists, switch, &act.label(action));
            let until = world.sim.time_seconds() + SWITCH_BANNER;
            scene.say(line, String::new(), tone, until);
            world.inputs.commands.push((me, ShipCommand::SwitchAssists(switch)));
        }
    }
    if act.just_pressed(Action::ControlsHelp) {
        menu.help = !menu.help;
    }
    // Esc (fixed, GDR-0090): closes the controls overlay, the menu or the pinned cluster map; else unlocks; else opens
    // the menu. The input runs before `voyage_keys` in the frame: an Esc that closes the ABANDON VOYAGE? panel does
    // neither.
    if keys.just_pressed(KeyCode::Escape) && !run.asking {
        if menu.help {
            menu.help = false;
        } else if menu.open {
            menu.open = false;
        } else if overview.is_pinned() {
            overview.unpin();
        } else if input.lock.is_some() {
            input.lock = None;
        } else {
            menu.open = true;
        }
    }
    // LOCK TARGET: the nearest enemy, then the next by distance (the sim's order, from the ship's own position).
    if act.just_pressed(Action::LockNext)
        && let Some(me) = focus.0
    {
        input.lock = sim::flight::next_lock(world.sim.ships(), me, input.locked_ship());
    }
    // SCAN: the flown ship's scanner (the sim ignores it while a scan charges or cools down, and without one).
    if act.just_pressed(Action::Scan)
        && let Some(me) = focus.0
    {
        world.inputs.commands.push((me, ShipCommand::Scan));
    }
    // FLARES: the flown ship's flare launcher (the sim ignores it while none is charged and cool, GDR-0115).
    if act.just_pressed(Action::Flare)
        && let Some(me) = focus.0
    {
        world.inputs.commands.push((me, ShipCommand::Flare));
    }
    // BLINK: the flown ship's blink drives (the sim ignores it while none can blink, GDR-0127).
    if act.just_pressed(Action::Blink)
        && let Some(me) = focus.0
        && let Some(ship) = world.sim.ship(me)
    {
        let dir = blink_dir(&input, ship);
        world.inputs.commands.push((me, ShipCommand::Blink(dir)));
    }
    // Cycle the focus through the player's vessels (a focus command for the sim).
    let step = if act.just_pressed(Action::FocusNext) {
        1
    } else if act.just_pressed(Action::FocusPrevious) {
        -1
    } else {
        0
    };
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
    hud: HudButtons,
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
        && !on_hud_button(&hud, c, false)
    {
        let alpha = fixed.overstep_fraction();
        let hit = pick(&world, focus.0, rig.screen_to_world(c), CLICK_SLACK / rig.zoom, alpha);
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
    if lmb_fire {
        input.key_fire |= 1 << (LMB_GROUP - 1);
    }
}

/// The laid-out nodes the touch input measures: FIRE and the speed block (the stick's zone starts under it).
type TouchLayout<'w, 's> = (
    Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform), With<FireButton>>,
    Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform), With<StickZone>>,
);

#[allow(clippy::too_many_arguments)]
fn touch(
    time: Res<Time<Real>>,
    touches: Res<Touches>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut world: ResMut<SimWorld>,
    focus: Res<PilotFocus>,
    rig: Res<CameraRig>,
    fixed: Res<Time<Fixed>>,
    (fleet, layer, ui_scale): (Res<FleetBar>, Res<SettingsLayer>, Res<UiScale>),
    hud: HudButtons,
    (fire, zone): TouchLayout,
    mut input: ResMut<PlayerInput>,
) {
    let size = window.size();
    let s = ui_scale.0;
    let now = time.elapsed_secs();
    input.ui_touches.retain(|&id| touches.get_pressed(id).is_some() || touches.just_released(id) || touches.just_canceled(id));
    let rect = |(node, tf): (&ComputedNode, &UiGlobalTransform)| logical_rect(node, tf);
    let fire_at = fire.single().ok().and_then(rect).map(|r| r.center());
    let zone_top = zone.single().ok().and_then(rect).map_or(size.y * 0.45, |r| r.max.y + 8.0 * s);
    let flying = focus.0.is_some();
    for t in touches.iter_just_pressed() {
        let (id, p) = (t.id(), t.position());
        if layer.open || fleet.blocks(p) || on_hud_button(&hud, p, true) {
            input.ui_touches.push(id);
            continue;
        }
        if flying && input.fire_touch.is_none() && fire_at.is_some_and(|c| c.distance(p) < FIRE_HIT / 2.0 * s) {
            input.fire_touch = Some(id);
        } else if flying && input.stick_touch.is_none() && p.x < size.x * STICK_ZONE && p.y > zone_top {
            // The stick's base appears under the thumb.
            input.stick_touch = Some((id, p, STICK_RING / 2.0 * s));
        } else if input.tap.is_some() {
            // A second finger: a pinch, not a tap.
            input.tap = None;
        } else {
            input.tap = Some((id, p, now));
        }
    }
    for t in touches.iter_just_released().chain(touches.iter_just_canceled()) {
        if input.stick_touch.is_some_and(|(id, ..)| id == t.id()) {
            input.stick_touch = None;
        }
        if input.fire_touch == Some(t.id()) {
            input.fire_touch = None;
        }
        if let Some((id, at, t0)) = input.tap
            && id == t.id()
        {
            input.tap = None;
            if now - t0 < TAP_TIME && t.position().distance(at) < FINGER_SLOP * s && flying {
                let alpha = fixed.overstep_fraction();
                let hit = pick(&world, focus.0, rig.screen_to_world(at), TAP_SLACK * s / rig.zoom, alpha);
                if hit.is_none() {
                    input.lock = None;
                }
                apply_pick(hit, &mut world, &focus, &mut input);
            }
        }
    }
    // SETTINGS over the flight takes the fingers.
    if layer.open {
        input.stick_touch = None;
        input.fire_touch = None;
        input.tap = None;
    }
}

/// The stick's push: the finger's offset from where it landed over the ring's radius, nothing inside the dead zone,
/// then rising to 1 at the rim (screen down is the world's −y).
fn stick_push(offset: Vec2, radius: f32) -> Vec2 {
    let v = Vec2::new(offset.x, -offset.y) / radius.max(1.0);
    let len = v.length();
    if len <= STICK_DEAD {
        return Vec2::ZERO;
    }
    v / len * ((len - STICK_DEAD) / (1.0 - STICK_DEAD)).min(1.0)
}

/// Stick, axes and fire from all sources: the touch stick wins while it is held, else the keyboard axes.
fn combine(touches: Res<Touches>, mut input: ResMut<PlayerInput>) {
    let mut stick = None;
    if let Some((id, centre, radius)) = input.stick_touch
        && let Some(t) = touches.get_pressed(id)
    {
        stick = Some(stick_push(t.position() - centre, radius));
    }
    input.stick_mode = stick.is_some();
    input.stick = stick.unwrap_or(Vec2::ZERO);
    input.axes = input.key_axes;
    input.fire = input.key_fire | if input.fire_touch.is_some() { FIRE_ALL } else { 0 };
    let (axes, fire) = (input.axes, input.fire);
    input.latch.note(axes, fire);
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
/// piece centre within 2 m and `slack` metres (a click's [`CLICK_SLACK`] or a finger's [`TAP_SLACK`] px at the zoom).
pub fn pick(world: &SimWorld, player: Option<ShipId>, at: Vec2, slack: f32, alpha: f32) -> Option<Target> {
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
            let slack = 2.0 + slack;
            if d < slack && best.is_none_or(|b| d < b.0) {
                best = Some((d, target));
            }
        }
    }
    best.map(|b| b.1)
}

/// The touch widgets: the stick's ring (at rest bottom left, faint), FIRE bottom right, FLARE beside it, BLINK above
/// it.
fn spawn_touch_ui(mut commands: Commands, style: Res<Style>, fonts: Res<HudFonts>) {
    let teal = style.glow_player;
    let coral = style.glow_enemy;
    let round = |size: f32, border: f32| Node {
        position_type: PositionType::Absolute,
        width: px(size),
        height: px(size),
        border: UiRect::all(px(border)),
        border_radius: BorderRadius::MAX,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    };
    commands
        .spawn((
            StickBase,
            FlightUi,
            HudZone,
            Node { left: px(STICK_REST.x), bottom: px(STICK_REST.y), ..round(STICK_RING, 2.0) },
            BorderColor::all(ui_color(teal, 0.25)),
            BackgroundColor(ui_color(teal, 0.02)),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn((StickKnob, round(STICK_KNOB, 2.0), BorderColor::all(ui_color(teal, 0.35)), BackgroundColor(ui_color(teal, 0.06))));
        });
    commands
        .spawn((
            FireButton,
            FlightUi,
            HudZone,
            Node { right: px(FIRE_EDGE), bottom: px(FIRE_EDGE), ..round(FIRE_SIZE, 3.0) },
            BorderColor::all(ui_color(coral, 0.7)),
            BackgroundColor(ui_color(coral, 0.06)),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn(label(&fonts, "FIRE", 12.0, ui_color(coral, 0.9)));
        });
    // FLARE: a ring left of FIRE, its centre at FIRE's height (a HUD button: a tap is the FLARES action).
    commands
        .spawn((
            FlightUi,
            SlotHolder,
            Node { position_type: PositionType::Absolute, right: px(FIRE_EDGE + FIRE_SIZE + SLOT_GAP), bottom: px(FIRE_EDGE + (FIRE_SIZE - SLOT) / 2.0), ..default() },
        ))
        .with_children(|p| {
            p.spawn((FlareButton, HudButton, HudZone, Visibility::Hidden, ring_node(SLOT))).with_child(label(&fonts, "FLARE", 8.0, ui_color(style.ui.label, 1.0)));
        });
    // BLINK (GDR-0129): a ring above FIRE, centred over it (a HUD button: a tap is the BLINK action, `blink_dir`).
    commands
        .spawn((
            FlightUi,
            SlotHolder,
            Node { position_type: PositionType::Absolute, right: px(FIRE_EDGE + (FIRE_SIZE - SLOT) / 2.0), bottom: px(FIRE_EDGE + FIRE_SIZE + SLOT_GAP), ..default() },
        ))
        .with_children(|p| {
            p.spawn((BlinkButton, HudButton, HudZone, Visibility::Hidden, ring_node(SLOT))).with_child(label(&fonts, "BLINK", 8.0, ui_color(style.ui.label, 1.0)));
        });
}

/// Where BLINK jumps (GDR-0127): along the touch stick while it is pushed, else along the thrust and strafe keys held
/// (turned with the ship), else straight ahead along the nose. A world direction (the sim scales it to the reach).
fn blink_dir(input: &PlayerInput, ship: &sim::Ship) -> sim::Vec2 {
    if input.stick_mode && input.stick.length() > 0.2 {
        return sv(input.stick);
    }
    let angle = ship.control_piece().and_then(|c| ship.piece(c)).map_or(0.0, |p| p.body.angle);
    let local = sim::Vec2::new(input.axes.z, input.axes.y);
    let local = if local.length_squared() > 0.01 { local } else { sim::Vec2::new(0.0, 1.0) };
    sim::Rot::from_angle(angle).apply(local)
}

/// A tap on FLARE: the flown ship's flare launcher (the FLARES action).
fn flare_tap(mut clicks: MessageReader<UiClick>, button: Query<(), With<FlareButton>>, mut taps: ResMut<ActionTaps>) {
    if clicks.read().any(|c| button.contains(c.0)) {
        taps.tap(Action::Flare);
    }
}

/// A tap on BLINK: the flown ship's blink drives (the BLINK action; along the stick while the other thumb pushes it).
fn blink_tap(mut clicks: MessageReader<UiClick>, button: Query<(), With<BlinkButton>>, mut taps: ResMut<ActionTaps>) {
    if clicks.read().any(|c| button.contains(c.0)) {
        taps.tap(Action::Blink);
    }
}

/// The widgets on a touch screen while a ship is flown: the stick's ring at rest, or under the thumb with its knob
/// where the thumb is; FIRE lit while held; FLARE for a ship with a flare launcher, BLINK for one with a blink drive.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn touch_ui(
    input: Res<PlayerInput>,
    focus: Res<PilotFocus>,
    touches: Res<Touches>,
    (screen, layer, ui_scale, world): (Res<TouchScreen>, Res<SettingsLayer>, Res<UiScale>, Res<SimWorld>),
    mut base: Query<(&mut Visibility, &mut Node, &mut BorderColor), (With<StickBase>, Without<FireButton>, Without<StickKnob>, Without<FlareButton>, Without<BlinkButton>)>,
    mut fire: Query<(&mut Visibility, &mut BackgroundColor), (With<FireButton>, Without<StickBase>, Without<FlareButton>, Without<BlinkButton>)>,
    mut flare: Query<&mut Visibility, (With<FlareButton>, Without<StickBase>, Without<FireButton>, Without<BlinkButton>)>,
    mut blink: Query<&mut Visibility, (With<BlinkButton>, Without<StickBase>, Without<FireButton>, Without<FlareButton>)>,
    mut holders: Query<&mut Visibility, (With<SlotHolder>, Without<StickBase>, Without<FireButton>, Without<FlareButton>, Without<BlinkButton>)>,
    mut knob: Query<(&mut Node, &mut BorderColor, &mut BackgroundColor), (With<StickKnob>, Without<StickBase>, Without<FireButton>)>,
    style: Res<Style>,
) {
    let show = screen.on() && focus.0.is_some() && !layer.open;
    let want = if show { Visibility::Inherited } else { Visibility::Hidden };
    // The slots' holders come back with the flight (a start elsewhere or a flight's end hid them, `mode.rs`).
    for mut v in &mut holders {
        v.set_if_neq(want);
    }
    let flown = focus.0.and_then(|id| world.sim.ship(id));
    let flares = flown.is_some_and(|s| !s.design().flares.is_empty());
    for mut v in &mut flare {
        v.set_if_neq(if show && flares { Visibility::Inherited } else { Visibility::Hidden });
    }
    let blinks = flown.is_some_and(|s| !s.design().blinks.is_empty());
    for mut v in &mut blink {
        v.set_if_neq(if show && blinks { Visibility::Inherited } else { Visibility::Hidden });
    }
    for (mut v, mut bg) in &mut fire {
        v.set_if_neq(want);
        let a = if input.fire_touch.is_some() { 0.3 } else { 0.06 };
        let c = ui_color(style.glow_enemy, a);
        if bg.0 != c {
            bg.0 = c;
        }
    }
    let s = ui_scale.0.max(1e-3);
    // Where the stick is (UI px from the window's top left) and the knob's offset, while a thumb holds it.
    let held = input.stick_touch.and_then(|(id, centre, radius)| {
        let t = touches.get_pressed(id)?;
        let d = t.position() - centre;
        Some((centre / s, d.clamp_length_max(radius) / s))
    });
    for (mut v, mut node, mut border) in &mut base {
        v.set_if_neq(want);
        let r = STICK_RING / 2.0;
        let (left, top, bottom) = match held {
            Some((c, _)) => (px(c.x - r), px(c.y - r), Val::Auto),
            None => (px(STICK_REST.x), Val::Auto, px(STICK_REST.y)),
        };
        if node.left != left || node.top != top || node.bottom != bottom {
            node.left = left;
            node.top = top;
            node.bottom = bottom;
        }
        let c = BorderColor::all(ui_color(style.glow_player, if held.is_some() { 0.5 } else { 0.25 }));
        if *border != c {
            *border = c;
        }
    }
    let off = held.map_or(Vec2::ZERO, |h| h.1);
    let k = (STICK_RING - STICK_KNOB) / 2.0 - 2.0;
    // At rest the stick is faint; under a thumb it lights up.
    let (edge, fill) = if held.is_some() { (0.8, 0.18) } else { (0.35, 0.06) };
    for (mut n, mut border, mut bg) in &mut knob {
        let (l, t) = (px(k + off.x), px(k + off.y));
        if n.left != l || n.top != t {
            n.left = l;
            n.top = t;
        }
        let c = BorderColor::all(ui_color(style.glow_player, edge));
        if *border != c {
            *border = c;
        }
        let c = ui_color(style.glow_player, fill);
        if bg.0 != c {
            bg.0 = c;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::state::app::StatesPlugin;
    use bevy::time::TimeUpdateStrategy;

    use super::*;

    #[test]
    fn the_latch_gives_a_tap_to_the_next_tick_once_and_held_keys_win() {
        let mut latch = TickLatch::default();
        // A for one frame, then group 1's trigger for one frame, no tick in between; group 2 held at the tick.
        latch.note(Vec3::new(1.0, 0.0, 0.0), 0);
        latch.note(Vec3::ZERO, 0b001);
        assert_eq!(latch.take(Vec3::ZERO, 0b010), (Vec3::new(1.0, 0.0, 0.0), 0b011));
        assert_eq!(latch.take(Vec3::ZERO, 0), (Vec3::ZERO, 0));
        // D held at the tick after an A tap and some W: D and W.
        latch.note(Vec3::new(1.0, 1.0, 0.0), 0);
        assert_eq!(latch.take(Vec3::new(-1.0, 0.0, 0.0), 0), (Vec3::new(-1.0, 1.0, 0.0), 0));
    }

    /// The touch stick: nothing inside its dead zone, full speed at the rim and past it, screen down is the
    /// world's −y.
    #[test]
    fn the_stick_has_a_dead_zone_and_is_full_at_the_rim() {
        let r = STICK_RING / 2.0;
        assert_eq!(stick_push(Vec2::new(r * STICK_DEAD * 0.9, 0.0), r), Vec2::ZERO);
        assert!((stick_push(Vec2::new(r, 0.0), r) - Vec2::X).length() < 1e-5);
        assert!((stick_push(Vec2::new(0.0, 3.0 * r), r) - Vec2::NEG_Y).length() < 1e-5);
        let half = stick_push(Vec2::new(r * 0.5, 0.0), r).x;
        assert!(half > 0.45 && half < 0.5, "{half}");
    }

    #[derive(Resource, Default)]
    struct Ticks(Vec<(Vec3, u8)>);

    /// The input systems as the plugin schedules them (no window here: the mouse and touch systems skip), a
    /// 30 Hz tick that takes the input like the scene does, and frames of `frame_ms`.
    fn app(frame_ms: f64) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(frame_ms / 1000.0)))
            .insert_resource(Time::<Fixed>::from_hz(sim::TICK_HZ as f64))
            .insert_state(AppMode::Flight)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<Touches>()
            .init_resource::<bevy_egui::input::EguiWantsInput>()
            .init_resource::<FleetBar>()
            .init_resource::<PilotFocus>()
            .init_resource::<CameraRig>()
            .init_resource::<VoyageRun>()
            .init_resource::<Ticks>()
            .init_resource::<SettingsLayer>()
            .init_resource::<FlightMenu>()
            .init_resource::<Overview>()
            .insert_resource(crate::settings::Controls::defaults(&data::base_content()))
            .insert_resource(Scene::new(crate::scene::SceneKind::Sandbox, 1))
            .insert_resource(SimWorld::new(sim::Sim::new(1), data::Content::default()));
        add_input_systems(&mut app);
        app.add_systems(FixedUpdate, |mut input: ResMut<PlayerInput>, mut ticks: ResMut<Ticks>| {
            ticks.0.push(input.for_tick())
        });
        app
    }

    /// One frame with these key changes; the ticks it ran.
    fn frame(app: &mut App, keys: impl FnOnce(&mut ButtonInput<KeyCode>)) -> Vec<(Vec3, u8)> {
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.clear();
        keys(&mut input);
        let before = app.world().resource::<Ticks>().0.len();
        app.update();
        app.world().resource::<Ticks>().0[before..].to_vec()
    }

    /// A key held in a frame reaches that frame's ticks (the input runs before them, not one frame later).
    #[test]
    fn a_held_key_reaches_the_tick_of_the_same_frame() {
        // Frames longer than a tick: every frame runs one.
        let mut app = app(40.0);
        frame(&mut app, |_| {});
        let ticks = frame(&mut app, |k| k.press(KeyCode::KeyW));
        assert!(!ticks.is_empty());
        assert!(ticks.iter().all(|t| t.0.y == 1.0), "{ticks:?}");
    }

    /// Frames at 100 Hz, ticks at 30 Hz: a tap pressed and released within one frame, between two ticks,
    /// reaches the next tick, once. Space fires every weapon group, 2 only group 2 (GDR-0088).
    #[test]
    fn a_tap_shorter_than_a_tick_reaches_one_tick() {
        let mut app = app(10.0);
        let taps = [(KeyCode::KeyA, (Vec3::X, 0)), (KeyCode::Space, (Vec3::ZERO, FIRE_ALL)), (KeyCode::Digit2, (Vec3::ZERO, 0b010))];
        for (key, check) in taps {
            // Right after a tick.
            while frame(&mut app, |_| {}).is_empty() {}
            let mut ticks = frame(&mut app, |k| {
                k.press(key);
                k.release(key);
            });
            while ticks.len() < 2 {
                ticks.extend(frame(&mut app, |_| {}));
            }
            assert_eq!(ticks[0], check, "{key:?}");
            assert_eq!(ticks[1], (Vec3::ZERO, 0), "{key:?}");
        }
    }

    /// Esc (GDR-0090): unlocks while a target is locked, else opens the flight menu, and closes it again; a
    /// rebound action works on its new key only.
    #[test]
    fn esc_unlocks_else_opens_the_menu_and_rebound_keys_work() {
        let mut app = app(40.0);
        app.world_mut().resource_mut::<PlayerInput>().lock = Some(Target::Point(sim::Vec2::ZERO));
        let esc = |k: &mut ButtonInput<KeyCode>| k.press(KeyCode::Escape);
        frame(&mut app, esc);
        assert!(app.world().resource::<PlayerInput>().lock.is_none() && !app.world().resource::<FlightMenu>().open, "unlocked, no menu");
        let up = |k: &mut ButtonInput<KeyCode>| k.release(KeyCode::Escape);
        frame(&mut app, up);
        frame(&mut app, esc);
        assert!(app.world().resource::<FlightMenu>().open, "nothing locked: the menu");
        frame(&mut app, up);
        frame(&mut app, esc);
        assert!(!app.world().resource::<FlightMenu>().open, "Esc again closes it");
        // Thrust moved from W to J: J thrusts, W doesn't.
        let chord: data::input::Chord = "KeyJ".parse().unwrap();
        app.world_mut().resource_mut::<crate::settings::Controls>().0.set(Action::ThrustForward, 0, chord);
        app.world_mut().resource_mut::<crate::settings::Controls>().0.clear(Action::ThrustForward, 1);
        let ticks = frame(&mut app, |k| k.press(KeyCode::KeyJ));
        assert!(ticks.iter().all(|t| t.0.y == 1.0), "{ticks:?}");
        let ticks = frame(&mut app, |k| {
            k.release(KeyCode::KeyJ);
            k.press(KeyCode::KeyW);
        });
        assert!(ticks.iter().all(|t| t.0.y == 0.0), "{ticks:?}");
    }

    /// LOCK TARGET (L since 18 Oct 2026, when Tab went to the cluster map; GDR-0088): the first press locks the
    /// nearest enemy, each next press the next one by distance, round to the nearest again; the lock is the one a
    /// click sets (`PlayerInput::lock`).
    #[test]
    fn lock_target_locks_the_nearest_then_cycles_by_distance() {
        let mut app = app(40.0);
        let content = data::base_content();
        let design = |id: &str| std::sync::Arc::new(sim::ShipDesign::compile(&content, content.ships.get(id).unwrap()).unwrap());
        let mut w = sim::Sim::new(1);
        let me = w.spawn_ship(design("halvex_runner"), sim::Owner::PLAYER, sim::Body::default());
        let at = |x: f32, y: f32| sim::Body::at(sim::Vec2::new(x, y), 0.0);
        let far = w.spawn_ship(design("automaton_raider"), crate::scene::ENEMY, at(0.0, 90.0));
        let near = w.spawn_ship(design("automaton_raider"), crate::scene::ENEMY, at(40.0, 0.0));
        let mid = w.spawn_ship(design("automaton_raider"), crate::scene::ENEMY, at(-60.0, 0.0));
        app.insert_resource(SimWorld::new(w, content.clone()));
        app.world_mut().resource_mut::<PilotFocus>().0 = Some(me);
        let lock = |app: &App| app.world().resource::<PlayerInput>().lock;
        let mut seen = Vec::new();
        for _ in 0..4 {
            frame(&mut app, |k| k.press(KeyCode::KeyL));
            seen.push(lock(&app));
            frame(&mut app, |k| k.release(KeyCode::KeyL));
        }
        assert_eq!(seen, [near, mid, far, near].map(|s| Some(Target::Ship(s))));
        // Esc drops it; L locks the nearest again.
        frame(&mut app, |k| k.press(KeyCode::Escape));
        assert_eq!(lock(&app), None);
        frame(&mut app, |k| {
            k.release(KeyCode::Escape);
            k.press(KeyCode::KeyL);
        });
        assert_eq!(lock(&app), Some(Target::Ship(near)));
    }

    /// The assist toggles (GDR-0088): F1–F4 switch their slot's assists and Z all of them, each a command for the
    /// flown ship, and the banner says what changed (from the state before the switch); a rebound toggle works on
    /// its new key.
    #[test]
    fn the_assist_toggles_switch_the_flown_ships_assists_and_say_so() {
        let pressed = [Action::AssistToggle2, Action::AllAssists, Action::Fire];
        let switches: Vec<_> = assist_switches(|a| pressed.contains(&a)).collect();
        assert_eq!(switches, [(AssistSwitch::Toggle(2), Action::AssistToggle2), (AssistSwitch::All, Action::AllAssists)]);

        let mut app = app(40.0);
        let content = data::base_content();
        let mut w = sim::Sim::new(1);
        w.set_behaviors(sim::BehaviorLibrary::new(&content).unwrap());
        let me = w.spawn_ship(sim::ShipDesign::compile(&content, content.ships.get("halvex_runner").unwrap()).unwrap().into(), sim::Owner::PLAYER, sim::Body::default());
        let list = data::AssistList { rules: vec!["always -> brake(toggle2, off)".into(), "always -> auto_fire(2, toggle2)".into()] };
        let id = w.behaviors_mut().add_assists(&list).unwrap();
        w.command(me, ShipCommand::Assists(Some(id)));
        app.insert_resource(SimWorld::new(w, content));
        app.world_mut().resource_mut::<PilotFocus>().0 = Some(me);
        let switched = |app: &App| -> Vec<AssistSwitch> {
            let w = app.world().resource::<SimWorld>();
            w.inputs.commands.iter().filter_map(|(s, c)| match c {
                ShipCommand::SwitchAssists(x) if *s == me => Some(*x),
                _ => None,
            }).collect()
        };
        let banner = |app: &App| app.world().resource::<Scene>().banner.clone().map(|b| b.0);
        frame(&mut app, |k| k.press(KeyCode::F2));
        assert_eq!(switched(&app), [AssistSwitch::Toggle(2)]);
        assert_eq!(banner(&app).as_deref(), Some("BRAKE ON  ·  AUTO-FIRE 2 OFF"));
        frame(&mut app, |k| {
            k.release(KeyCode::F2);
            k.press(KeyCode::KeyZ);
        });
        assert_eq!(switched(&app), [AssistSwitch::Toggle(2), AssistSwitch::All]);
        assert_eq!(banner(&app).as_deref(), Some("ALL ASSISTS OFF"));
        frame(&mut app, |k| {
            k.release(KeyCode::KeyZ);
            k.press(KeyCode::F3);
        });
        assert_eq!(banner(&app).as_deref(), Some("F3  ·  NO ASSIST ON TOGGLE 3"));
        // Toggle 2 moved to K.
        let chord: data::input::Chord = "KeyK".parse().unwrap();
        app.world_mut().resource_mut::<crate::settings::Controls>().0.set(Action::AssistToggle2, 0, chord);
        frame(&mut app, |k| {
            k.release(KeyCode::F3);
            k.press(KeyCode::F2);
        });
        frame(&mut app, |k| {
            k.release(KeyCode::F2);
            k.press(KeyCode::KeyK);
        });
        assert_eq!(switched(&app), [AssistSwitch::Toggle(2), AssistSwitch::All, AssistSwitch::Toggle(3), AssistSwitch::Toggle(2)]);
    }
}
