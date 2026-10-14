//! The voyage's transitions (Niklas, 14 Oct 2026: "the recall etc. could get a better animation"; GDR-0085,
//! `docs/client.md` "Voyages"): client visuals only, read from the sim's tick and the voyage's phase (`voyage.rs`).
//! Nothing here writes into the sim; the sim's timing (the 3 s of `voyage::HOME_TICKS`) is what they show.
//!
//! * **Warp-out** (RECALL: the 3 s of `voyage::Phase::Warping`): every vessel of the fleet charges: a soft glow
//!   under it, a ring around it that tightens and brightens while a dashed ring inside it spins up, faint streaks
//!   drawn in towards it. The camera eases in a little ([`ZOOM_IN`]) and holds. At [`JUMP_AT`] each vessel stretches
//!   along its heading and darts ahead out of sight (the escorts a moment before the flagship, [`ESCORT_LEAD`]); a
//!   flash and a ring stay where it left, a streak along its way fades. The emergency recall (no working gun,
//!   GDR-0074) charges unstable: amber and coral instead of teal, flickering, its rings and its jump shaking; the
//!   banner says `EMERGENCY WARP` in gold (`flight_hud.rs`).
//! * **Arrival** (a voyage's first [`ARRIVE_SECONDS`]): the reverse. The world comes up out of the dark while the HUD
//!   slides in; each vessel streaks in from behind, settles onto its place with a flash and a ring, and its glow
//!   fades; the camera eases back out.
//! * **The hand-over** (every end of a voyage, and a lost test flight going back to the drydock or the BEHAVIOR
//!   screen): over the last [`DIP_SECONDS`] the HUD closes with its screen transition (`ui_kit::ScreenFx`) and the
//!   window dips to the backdrop's darkest colour; the next screen comes up out of it over [`LIFT_SECONDS`]. After a
//!   recall a ring of light spreads from home's gate while home's camera flies out of it.
//! * **Lost** (`SHIP LOST · RECALLED HOME`, `VOYAGE ABANDONED`, a lost test flight): the world dims behind the
//!   banner ([`LOST_DIM`]) before the dip.
//!
//! Everything in flight follows the sim's time (the tick plus the fixed step's overstep): it pauses with P and looks
//! the same at any frame rate; the lift at the destination runs on real time. The curves are pure functions
//! ([`moment`], [`warp_out`], [`arrive`], [`frame`]) and tested. Drawn as FX sprites (`fx::FxQueue`, the atlas's
//! glow dot and halo strip); the vessels are stretched and moved through their piece views' transforms after
//! `ship_view::sync_piece_views` has posed them (so their shield bubbles follow), and the camera's zoom through
//! `CameraRig::warp_zoom`. The dark is two UI veils in the backdrop's darkest colour: the dim under the UI (the
//! HUD stays readable over it), the dip over all of it.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use crate::RenderToggles;
use crate::camera::{CameraRig, CameraSet};
use crate::fx::{FxFlush, FxQueue, FxRegions, Z_SPARKS, Z_TRAILS};
use crate::mode::AppMode;
use crate::scene::{PLAYER, PilotFocus, Scene};
use crate::ship_view::{PieceView, core_tint};
use crate::shipyard::Yard;
use crate::shipyard::fly::smootherstep;
use crate::shipyard::layout::GATE_RADIUS;
use crate::sim_bridge::SimWorld;
use crate::style::{Style, UiPalette, mix, ui_color};
use crate::ui_kit::EXIT_SECONDS;
use crate::voyage::{HOME_TICKS, Phase, VoyageRun};

// ---------------------------------------------------------------------------------------------------------
// Tuning (seconds of sim time unless said otherwise; lengths in the vessel's radius, the half diagonal of its
// bounds). Defaults for review.

const TICK_HZ: f32 = sim::TICK_HZ as f32;
/// The warp-out: the sim's 3 s ([`HOME_TICKS`]).
pub const WARP_SECONDS: f32 = HOME_TICKS as f32 / TICK_HZ;
/// The charge builds up to here; then the jump.
pub const JUMP_AT: f32 = 2.25;
/// The jump: stretched and gone in this long.
const JUMP_SECONDS: f32 = 0.3;
/// How far it stretches along its heading at the end of the jump (1: not at all), how much narrower it is then,
/// and how far ahead it has darted (radii).
const STRETCH: f32 = 5.0;
const SQUEEZE: f32 = 0.5;
const SHIFT: f32 = 16.0;
/// The flash where it leaves (or arrives): this long after the jump starts (before it has landed), fading over
/// [`FLASH_SECONDS`]; its ring spreads to [`WAVE_REACH`] radii in [`WAVE_SECONDS`].
const FLASH_AFTER: f32 = 0.15;
const FLASH_SECONDS: f32 = 0.4;
const WAVE_SECONDS: f32 = 0.6;
const WAVE_REACH: f32 = 3.5;
/// The flash's size (radii across) and brightness.
const FLASH_SIZE: f32 = 4.0;
const FLASH_ALPHA: f32 = 0.8;
/// The streak along its way fades this long after it is gone (or has landed).
const TRAIL_SECONDS: f32 = 0.3;
/// Each escort jumps this much before the flagship (and lands this much after it), at most [`ESCORT_LEAD_MAX`].
pub const ESCORT_LEAD: f32 = 0.06;
const ESCORT_LEAD_MAX: f32 = 0.24;
/// The camera's zoom-in while the fleet charges (× the zoom), held through the jump (GDR-0085).
pub const ZOOM_IN: f32 = 1.12;
/// The dip to dark at the end of a warp-out or a loss, and the HUD's close before the cut.
pub const DIP_SECONDS: f32 = 0.36;
const HUD_OUT_AT: f32 = WARP_SECONDS - EXIT_SECONDS - 0.04;
/// The next screen comes up out of the dip in this long (real time).
pub const LIFT_SECONDS: f32 = 0.5;
/// Arrival: how long it lasts, how long the world takes to come up out of the dark, when the vessels streak in
/// and how long that takes, and how bright their glow is when they land.
pub const ARRIVE_SECONDS: f32 = 1.8;
const ARRIVE_DIP: f32 = 0.45;
const IN_AT: f32 = 0.3;
const IN_SECONDS: f32 = 0.3;
const ARRIVE_GLOW: f32 = 0.75;
/// The arrival's flash goes off this long before the vessel has landed.
const ARRIVE_FLASH_EARLY: f32 = 0.05;
/// Lost: how far the world dims behind the banner, and how fast.
pub const LOST_DIM: f32 = 0.4;
const DIM_SECONDS: f32 = 0.6;
/// The charge's look: the ring from and to (radii), the glow's size (radii across) and brightness, the streaks
/// drawn in (how many, how often each comes, from and to which radius), the spinning dashes (how many; the turn is
/// `SPIN · s³` rad, so it spins up), the emergency flicker (steps per second).
const RING_FROM: f32 = 1.75;
const RING_TO: f32 = 1.2;
const GLOW_SIZE: f32 = 3.4;
const GLOW_ALPHA: f32 = 0.38;
const STREAKS: u32 = 16;
const STREAK_PERIOD: f32 = 0.65;
const STREAK_FROM: f32 = 2.8;
const STREAK_TO: f32 = 1.3;
const DASHES: u32 = 20;
const SPIN: f32 = 0.35;
const FLICKER_HZ: f32 = 22.0;
/// The ring of light at home's gate after a recall (real time), and how far it spreads (gate radii).
const GATE_SECONDS: f32 = 1.3;
const GATE_REACH: f32 = 1.6;

// The flagship has left before the dip begins, and the HUD has closed by the cut.
const _: () = assert!(JUMP_AT + JUMP_SECONDS < WARP_SECONDS - DIP_SECONDS);
const _: () = assert!(HUD_OUT_AT + EXIT_SECONDS < WARP_SECONDS);

// ---------------------------------------------------------------------------------------------------------
// The curves (pure).

/// Where the flight is in its transitions.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Moment {
    #[default]
    Calm,
    /// Seconds since the voyage began (its first [`ARRIVE_SECONDS`], and the last escort's lead).
    Arriving(f32),
    /// Seconds since the recall was granted (0..[`WARP_SECONDS`]).
    WarpingOut(f32),
    /// Seconds along a flight ending without a recall (lost, abandoned, a lost test flight), counted so that it
    /// ends at [`WARP_SECONDS`].
    Leaving(f32),
}

/// The moment at sim `tick` plus `overstep` (0..1 of a tick): from the voyage's `phase` (`voyage`: one is flown,
/// and the tick of its first frame: the arrival counts from there, so the hitch of building the world doesn't eat
/// it), else from the tick a lost test flight leaves at (`Scene::leaving_at`).
pub fn moment(phase: Phase, voyage: Option<u64>, leaving_at: Option<u64>, tick: u64, overstep: f32) -> Moment {
    let now = tick as f32 + overstep.clamp(0.0, 1.0);
    let since = |start: u64| ((now - start as f32) / TICK_HZ).clamp(0.0, WARP_SECONDS);
    let left = |end: u64| (WARP_SECONDS - (end as f32 - now) / TICK_HZ).clamp(0.0, WARP_SECONDS);
    if let Some(first) = voyage {
        let arrived = (now - first as f32).max(0.0) / TICK_HZ;
        match phase {
            Phase::Warping { home_at } => return Moment::WarpingOut(since(home_at.saturating_sub(HOME_TICKS))),
            Phase::Lost { home_at, .. } => return Moment::Leaving(left(home_at)),
            Phase::Flying if arrived < ARRIVE_SECONDS + ESCORT_LEAD_MAX => return Moment::Arriving(arrived),
            Phase::Flying | Phase::Ended => {}
        }
    }
    leaving_at.map_or(Moment::Calm, |at| Moment::Leaving(left(at)))
}

/// What the whole view does at a moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// The camera's zoom factor (1: none).
    pub zoom: f32,
    /// The veil under the UI (0..1: the world behind the HUD) and the one over all of it.
    pub dim: f32,
    pub dip: f32,
    /// The HUD closes (its screen transition) for the hand-over.
    pub hud_out: bool,
}

impl Frame {
    pub const CALM: Frame = Frame { zoom: 1.0, dim: 0.0, dip: 0.0, hud_out: false };
}

/// The dip to dark over the last [`DIP_SECONDS`] of a warp-out or a loss.
fn dip(s: f32) -> f32 {
    smootherstep((s - (WARP_SECONDS - DIP_SECONDS)) / DIP_SECONDS)
}

/// The view at a moment.
pub fn frame(m: Moment) -> Frame {
    match m {
        Moment::Calm => Frame::CALM,
        Moment::Arriving(s) => Frame {
            zoom: 1.0 + (ZOOM_IN - 1.0) * (1.0 - smootherstep(s / ARRIVE_SECONDS)),
            dim: 1.0 - smootherstep(s / ARRIVE_DIP),
            dip: 0.0,
            hud_out: false,
        },
        Moment::WarpingOut(s) => {
            Frame { zoom: 1.0 + (ZOOM_IN - 1.0) * smootherstep(s / JUMP_AT), dim: 0.0, dip: dip(s), hud_out: s >= HUD_OUT_AT }
        }
        Moment::Leaving(s) => Frame { zoom: 1.0, dim: LOST_DIM * smootherstep(s / DIM_SECONDS), dip: dip(s), hud_out: s >= HUD_OUT_AT },
    }
}

/// How one vessel looks at a moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// The build-up around it (glow, rings, streaks), 0..1.
    pub charge: f32,
    /// Its stretch along its heading (1: none) and how far ahead of its place it is drawn (radii; behind: < 0),
    /// and where the streak along its way ends behind it (radii from its place; lagging behind `shift`).
    pub stretch: f32,
    pub shift: f32,
    pub tail: f32,
    /// It is drawn.
    pub shown: bool,
    /// The flash where it left or landed (0..1, fading), the way its ring has spread (0: none, then 0..1), and the
    /// streak along its way (0..1, fading).
    pub flash: f32,
    pub wave: f32,
    pub trail: f32,
}

impl Look {
    #[cfg(test)]
    pub const CALM: Look = Look { charge: 0.0, stretch: 1.0, shift: 0.0, tail: 0.0, shown: true, flash: 0.0, wave: 0.0, trail: 0.0 };

    /// Its width (across its heading) for its stretch.
    pub fn width(&self) -> f32 {
        1.0 - SQUEEZE * ((self.stretch - 1.0) / (STRETCH - 1.0)).clamp(0.0, 1.0)
    }
}

/// A flash `s` seconds after it went off: full at once, fading out over `len`; 0 outside.
fn pop(s: f32, len: f32) -> f32 {
    if (0.0..len).contains(&s) {
        let k = 1.0 - s / len;
        k * k
    } else {
        0.0
    }
}

/// How far along `len` something started `s` seconds ago is (0 before it starts and once it is over).
fn way(s: f32, len: f32) -> f32 {
    if s > 0.0 && s < len { s / len } else { 0.0 }
}

/// A vessel `s` seconds into the warp-out (an escort: plus its lead).
pub fn warp_out(s: f32) -> Look {
    let j = ((s - JUMP_AT) / JUMP_SECONDS).clamp(0.0, 1.0);
    let gone = s - (JUMP_AT + JUMP_SECONDS);
    let flash = s - (JUMP_AT + FLASH_AFTER);
    Look {
        charge: smootherstep(s / JUMP_AT) * (1.0 - j),
        stretch: 1.0 + (STRETCH - 1.0) * j * j,
        shift: SHIFT * j * j * j,
        tail: SHIFT * j.powi(5),
        shown: j < 1.0,
        flash: pop(flash, FLASH_SECONDS),
        wave: way(flash, WAVE_SECONDS),
        trail: if j > 0.0 { 1.0 - (gone / TRAIL_SECONDS).clamp(0.0, 1.0) } else { 0.0 },
    }
}

/// A vessel `s` seconds into the voyage (an escort: less its lead): it streaks in from behind and settles.
pub fn arrive(s: f32) -> Look {
    let j = ((s - IN_AT) / IN_SECONDS).clamp(0.0, 1.0);
    let back = 1.0 - j;
    let landed = s - (IN_AT + IN_SECONDS);
    let flash = landed + ARRIVE_FLASH_EARLY;
    let charge = if landed < 0.0 {
        ARRIVE_GLOW * j
    } else {
        ARRIVE_GLOW * (1.0 - smootherstep(landed / (ARRIVE_SECONDS - IN_AT - IN_SECONDS)))
    };
    Look {
        charge,
        stretch: 1.0 + (STRETCH - 1.0) * back * back,
        shift: -SHIFT * back * back * back,
        tail: -SHIFT * back * back,
        shown: s >= IN_AT,
        flash: pop(flash, FLASH_SECONDS),
        wave: way(flash, WAVE_SECONDS),
        trail: if s >= IN_AT { 1.0 - (landed / TRAIL_SECONDS).clamp(0.0, 1.0) } else { 0.0 },
    }
}

/// How much earlier the `k`th vessel (the flagship 0) jumps, and later it lands.
pub fn lead(k: usize) -> f32 {
    (k as f32 * ESCORT_LEAD).min(ESCORT_LEAD_MAX)
}

/// A vessel's look at a moment (`k`: its place, the flagship 0); `None`: it is not in a warp.
pub fn vessel_look(m: Moment, k: usize) -> Option<Look> {
    match m {
        Moment::Arriving(s) => Some(arrive(s - lead(k))),
        Moment::WarpingOut(s) => Some(warp_out(s + lead(k))),
        Moment::Calm | Moment::Leaving(_) => None,
    }
}

/// A steady pseudo-random 0..1 for `k` (the emergency's flicker and shake).
fn noise(k: u32) -> f32 {
    let mut x = k.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65535.0
}

/// The emergency's flicker at `s` (0..1, a new value [`FLICKER_HZ`] times a second), per `seed`.
fn flicker(s: f32, seed: u32) -> f32 {
    noise((s.max(0.0) * FLICKER_HZ) as u32 ^ seed.wrapping_mul(0x01F3_5A27))
}

// ---------------------------------------------------------------------------------------------------------
// State and systems.

/// A vessel in a warp this frame, as drawn.
#[derive(Clone, Copy, Debug)]
struct Warped {
    /// Its place (the centre of its bounds at the render pose), its radius, its hull glow.
    centre: Vec2,
    radius: f32,
    glow: Color,
    /// Where it is drawn now (the stretch's shift away from its place), and where its streak ends behind it.
    at: Vec2,
    tail: Vec2,
    look: Look,
    seed: u32,
}

/// The transitions' state (a resource).
#[derive(Resource, Debug)]
pub struct Transit {
    now: Moment,
    frame: Frame,
    /// The recall is the emergency warp (taken when it starts).
    emergency: bool,
    /// The voyage's first frame (sim tick): the arrival counts from it.
    first: Option<u64>,
    warped: Vec<Warped>,
    /// Piece views stretched or hidden last frame (given back once they are not).
    touched: Vec<Entity>,
    /// The dip shown now, and the lift after a flight ended: how much of it is left (1 → 0, real time) from how dark.
    shown_dip: f32,
    lift: f32,
    lift_from: f32,
    /// The ring at home's gate after a recall: seconds since it began.
    gate: Option<f32>,
}

impl Default for Transit {
    fn default() -> Self {
        Self {
            now: Moment::Calm,
            frame: Frame::CALM,
            emergency: false,
            first: None,
            warped: Vec::new(),
            touched: Vec::new(),
            shown_dip: 0.0,
            lift: 0.0,
            lift_from: 0.0,
            gate: None,
        }
    }
}

impl Transit {
    /// The HUD closes for the hand-over.
    pub fn hud_out(&self) -> bool {
        self.frame.hud_out
    }

    /// The warp-out underway is the emergency recall.
    pub fn emergency(&self) -> bool {
        self.emergency && matches!(self.now, Moment::WarpingOut(_))
    }
}

/// The two veils (UI roots in the backdrop's darkest colour).
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Veil {
    /// Under the UI: the world behind the HUD.
    Dim,
    /// Over all of it: the dip of a hand-over.
    Dip,
}

pub struct WarpFxPlugin;

impl Plugin for WarpFxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Transit>()
            .add_systems(Startup, spawn_veils)
            .add_systems(
                PostUpdate,
                (
                    pose.after(crate::ship_view::sync_piece_views)
                        .before(crate::shield::draw_bubbles)
                        .before(CameraSet)
                        .run_if(in_state(AppMode::Flight)),
                    draw.after(pose).before(FxFlush).run_if(in_state(AppMode::Flight)),
                    gate_ring.after(CameraSet).before(FxFlush).run_if(in_state(AppMode::Home)),
                    veils.after(pose),
                ),
            )
            .add_systems(OnExit(AppMode::Flight), leave_flight)
            .add_systems(OnExit(AppMode::Home), |mut t: ResMut<Transit>| t.gate = None);
    }
}

fn spawn_veils(mut commands: Commands) {
    for (veil, z) in [(Veil::Dim, -1), (Veil::Dip, i32::MAX - 4)] {
        commands.spawn((
            veil,
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
            BackgroundColor(Color::NONE),
            GlobalZIndex(z),
            Visibility::Hidden,
        ));
    }
}

/// Every frame in flight: the moment, the view (camera zoom, veils, HUD), and the vessels in a warp stretched,
/// moved or hidden.
#[allow(clippy::too_many_arguments)]
fn pose(
    run: Res<VoyageRun>,
    scene: Res<Scene>,
    world: Res<SimWorld>,
    fixed: Res<Time<Fixed>>,
    focus: Res<PilotFocus>,
    next: Res<NextState<AppMode>>,
    mut transit: ResMut<Transit>,
    mut rig: ResMut<CameraRig>,
    mut views: Query<(&PieceView, &mut Transform, &mut Visibility)>,
) {
    // The mode changes this frame (the voyage ended, a lost test flight goes back): everything holds as it was, so
    // the dip stays dark up to the cut.
    if !matches!(*next, NextState::Unchanged) {
        return;
    }
    let t = &mut *transit;
    let voyage = run.launched.is_some() && scene.voyage().is_some();
    let tick = world.sim.tick();
    if voyage && t.first.is_none() {
        t.first = Some(tick);
    }
    let now = moment(run.phase, t.first.filter(|_| voyage), scene.leaving_at(), tick, fixed.overstep_fraction());
    if matches!(now, Moment::WarpingOut(_)) && !matches!(t.now, Moment::WarpingOut(_)) {
        t.emergency = run.emergency;
    }
    t.now = now;
    t.frame = frame(now);
    if rig.warp_zoom != t.frame.zoom {
        rig.warp_zoom = t.frame.zoom;
    }

    // The fleet, the flagship first.
    t.warped.clear();
    let before = std::mem::take(&mut t.touched);
    if !matches!(now, Moment::Arriving(_) | Moment::WarpingOut(_)) {
        give_back(&before, &mut views);
        return;
    }
    let mut fleet: Vec<sim::ShipId> = world.sim.vessels(PLAYER).map(|v| v.ship).collect();
    fleet.sort_by_key(|s| Some(*s) != focus.0);
    let shake = t.emergency && matches!(now, Moment::WarpingOut(_));
    let s = match now {
        Moment::Arriving(s) | Moment::WarpingOut(s) => s,
        _ => 0.0,
    };
    for (k, id) in fleet.iter().enumerate() {
        let Some(look) = vessel_look(now, k) else { continue };
        let Some(piece) = world.sim.ship(*id).and_then(|ship| ship.control_piece()) else { continue };
        let Some(&e) = world.views.get(&(*id, piece)) else { continue };
        let Ok((view, mut tf, mut vis)) = views.get_mut(e) else { continue };
        let rot = Mat2::from_angle(tf.rotation.to_euler(EulerRot::ZYX).0);
        let (fwd, side) = (rot * Vec2::Y, rot * Vec2::X);
        let centre = tf.translation.truncate() + rot * view.center;
        // The emergency's jump shakes sideways while it stretches.
        let jitter = if shake && look.stretch > 1.0 { (noise((s * FLICKER_HZ) as u32 ^ id.0) - 0.5) * 0.25 * view.radius } else { 0.0 };
        let scale = Vec2::new(look.width(), look.stretch);
        // Stretched about its centre, then moved along its heading.
        let moved = rot * (view.center - view.center * scale) + fwd * look.shift * view.radius + side * jitter;
        tf.translation += moved.extend(0.0);
        tf.scale = scale.extend(1.0);
        vis.set_if_neq(if look.shown { Visibility::Inherited } else { Visibility::Hidden });
        let at = centre + fwd * look.shift * view.radius + side * jitter;
        let tail = centre + fwd * look.tail * view.radius;
        t.warped.push(Warped { centre, radius: view.radius, glow: view.glow, at, tail, look, seed: id.0 });
        t.touched.push(e);
    }
    let gone: Vec<Entity> = before.into_iter().filter(|e| !t.touched.contains(e)).collect();
    give_back(&gone, &mut views);
}

/// Piece views out of a warp: their own size, shown.
fn give_back(entities: &[Entity], views: &mut Query<(&PieceView, &mut Transform, &mut Visibility)>) {
    for e in entities {
        if let Ok((_, mut tf, mut vis)) = views.get_mut(*e) {
            tf.scale = Vec3::ONE;
            vis.set_if_neq(Visibility::Inherited);
        }
    }
}

/// The warp's light around every vessel in it.
fn draw(transit: Res<Transit>, regions: Res<FxRegions>, style: Res<Style>, toggles: Res<RenderToggles>, mut queue: ResMut<FxQueue>) {
    if toggles.debug_shapes {
        return;
    }
    let (s, out) = match transit.now {
        Moment::Arriving(s) => (s, false),
        Moment::WarpingOut(s) => (s, true),
        _ => return,
    };
    let emergency = out && transit.emergency;
    for v in &transit.warped {
        draw_vessel(&mut queue, &regions, &style.ui, v, s, out, emergency);
    }
}

/// One vessel's warp: glow, rings, streaks drawn in (warp-out), the streak along its way, the flash and its ring.
fn draw_vessel(queue: &mut FxQueue, regions: &FxRegions, ui: &UiPalette, v: &Warped, s: f32, out: bool, emergency: bool) {
    let (r, look) = (v.radius, v.look);
    // Teal (the hull's glow) normally; the emergency's light amber (gold into coral), its rings gold, its streaks
    // coral, flickering.
    let (main, rim, hot) = if emergency { (mix(ui.gold, ui.coral, 0.5), ui.gold, ui.coral) } else { (v.glow, v.glow, ui.teal) };
    let fl = if emergency { 0.45 + 0.55 * flicker(s, v.seed) } else { 1.0 };
    let c = look.charge;
    if c > 0.0 {
        queue.dot(&regions.soft, v.centre, r * GLOW_SIZE * (0.8 + 0.2 * c), main.with_alpha(GLOW_ALPHA * c * fl), Z_TRAILS);
        // The ring tightens as it charges (and widens as an arrival's glow fades).
        let shake = if emergency { (flicker(s * 1.7, v.seed ^ 0x55) - 0.5) * 0.12 * r } else { 0.0 };
        let ring_r = r * (RING_FROM + (RING_TO - RING_FROM) * c) + shake;
        ring(queue, regions, v.centre, ring_r, 0.4, rim.with_alpha(0.6 * c * fl), None);
        ring(queue, regions, v.centre, ring_r, 0.05, core_tint(rim).with_alpha(0.85 * c * c * fl), None);
        if out {
            // The dashes inside it spin up; streaks are drawn in towards it.
            let dashed = Some((SPIN * s * s * s, DASHES));
            ring(queue, regions, v.centre, ring_r * 0.88, 0.16, rim.with_alpha(0.85 * c * fl), dashed);
            for i in 0..STREAKS {
                let ph = (s / STREAK_PERIOD + i as f32 * 0.618).fract();
                let a = i as f32 * TAU / STREAKS as f32 + noise(i ^ v.seed) * 0.4;
                let dir = Vec2::from_angle(a);
                let outer = r * (STREAK_FROM + (STREAK_TO - STREAK_FROM) * ph);
                let inner = (outer - r * 0.5 * (0.5 + ph)).max(r * STREAK_TO);
                let k = c * (PI * ph).sin() * fl;
                let (from, to) = (v.centre + dir * outer, v.centre + dir * inner);
                queue.line(&regions.halo, from, to, 0.26, hot.with_alpha(0.55 * k), Z_SPARKS);
                queue.line(&regions.white, from, to, 0.05, core_tint(hot).with_alpha(0.7 * k), Z_SPARKS + 0.1);
            }
        }
    }
    if look.trail > 0.0 {
        // Its way: a streak from its tail, lagging behind, up to where it is.
        let (from, to) = (v.tail, v.at);
        let k = look.trail * fl;
        queue.line(&regions.halo, from, to, r * 0.8, main.with_alpha(0.3 * k), Z_TRAILS);
        queue.line(&regions.halo, from, to, r * 0.25, core_tint(main).with_alpha(0.5 * k), Z_SPARKS);
    }
    if look.flash > 0.0 {
        let f = look.flash;
        queue.dot(&regions.soft, v.centre, r * FLASH_SIZE * (1.2 - 0.4 * f), main.with_alpha(FLASH_ALPHA * 0.6 * f), Z_SPARKS);
        queue.dot(&regions.soft, v.centre, r * FLASH_SIZE * 0.45, core_tint(main).with_alpha(FLASH_ALPHA * f * f), Z_SPARKS + 0.1);
    }
    if look.wave > 0.0 {
        let w = look.wave;
        let spread = 1.0 - (1.0 - w) * (1.0 - w);
        let k = (1.0 - w) * (1.0 - w) * fl;
        let wave_r = r * (1.0 + (WAVE_REACH - 1.0) * spread);
        ring(queue, regions, v.centre, wave_r, 0.5 * (1.0 - w) + 0.15, main.with_alpha(0.55 * k), None);
        ring(queue, regions, v.centre, wave_r, 0.05, core_tint(main).with_alpha(0.6 * k), None);
    }
}

/// A ring of halo strips around `centre` (`dashes`: its turn and how many dashes, half of it lit).
fn ring(queue: &mut FxQueue, regions: &FxRegions, centre: Vec2, radius: f32, width: f32, color: Color, dashes: Option<(f32, u32)>) {
    if color.alpha() <= 0.003 || radius <= 0.0 {
        return;
    }
    let region = if width < 0.1 { &regions.white } else { &regions.halo };
    let z = Z_SPARKS;
    match dashes {
        Some((turn, n)) => {
            for i in 0..n {
                let a = turn + i as f32 * TAU / n as f32;
                let b = a + 0.5 * TAU / n as f32;
                queue.line(region, centre + Vec2::from_angle(a) * radius, centre + Vec2::from_angle(b) * radius, width, color, z);
            }
        }
        None => {
            // Segments about a metre long, 24 at least.
            let n = ((TAU * radius).ceil() as u32).clamp(24, 96);
            for i in 0..n {
                let a = i as f32 * TAU / n as f32;
                let b = (i + 1) as f32 * TAU / n as f32;
                queue.line(region, centre + Vec2::from_angle(a) * radius, centre + Vec2::from_angle(b) * radius, width, color, z);
            }
        }
    }
}

/// After a recall, home: the fleet is back through the gate: a flash and a ring of light spreading from it.
fn gate_ring(
    time: Res<Time<Real>>,
    yard: Res<Yard>,
    regions: Res<FxRegions>,
    style: Res<Style>,
    toggles: Res<RenderToggles>,
    mut transit: ResMut<Transit>,
    mut queue: ResMut<FxQueue>,
) {
    let Some(age) = transit.gate.as_mut() else { return };
    *age += time.delta_secs().min(0.1);
    let k = *age / GATE_SECONDS;
    if k >= 1.0 {
        transit.gate = None;
        return;
    }
    if toggles.debug_shapes {
        return;
    }
    let (teal, centre) = (style.ui.teal, yard.layout.gate);
    let fade = (1.0 - k) * (1.0 - k);
    let spread = 1.0 - (1.0 - k) * (1.0 - k);
    queue.dot(&regions.soft, centre, GATE_RADIUS * 2.4, teal.with_alpha(0.35 * fade), Z_SPARKS);
    let r = GATE_RADIUS * (0.6 + (GATE_REACH - 0.6) * spread);
    ring(&mut queue, &regions, centre, r, 0.8 * (1.0 - k) + 0.2, teal.with_alpha(0.5 * fade), None);
    ring(&mut queue, &regions, centre, r, 0.06, core_tint(teal).with_alpha(0.6 * fade), None);
}

/// The veils every frame, in every mode: in flight what the moment asks for; after a flight the lift.
fn veils(
    time: Res<Time<Real>>,
    mode: Res<State<AppMode>>,
    style: Res<Style>,
    mut transit: ResMut<Transit>,
    mut nodes: Query<(&Veil, &mut BackgroundColor, &mut Visibility)>,
) {
    let t = &mut *transit;
    if t.lift > 0.0 {
        t.lift = (t.lift - time.delta_secs().min(0.1) / LIFT_SECONDS).max(0.0);
    }
    let lift = t.lift_from * smootherstep(t.lift);
    let (dim, dip) = if *mode.get() == AppMode::Flight { (t.frame.dim, t.frame.dip.max(lift)) } else { (0.0, lift) };
    t.shown_dip = dip;
    let dark = style.background.outer();
    for (veil, mut bg, mut vis) in &mut nodes {
        let a = match veil {
            Veil::Dim => dim,
            Veil::Dip => dip,
        };
        let c = ui_color(dark, a);
        if bg.0 != c {
            bg.0 = c;
        }
        vis.set_if_neq(if a > 0.0 { Visibility::Inherited } else { Visibility::Hidden });
    }
}

/// Flight ends: the next screen comes up out of the dip shown last; after a recall the gate's ring plays at home.
fn leave_flight(mut transit: ResMut<Transit>, mut rig: ResMut<CameraRig>) {
    let t = &mut *transit;
    t.lift_from = t.shown_dip;
    t.lift = if t.lift_from > 0.0 { 1.0 } else { 0.0 };
    t.gate = matches!(t.now, Moment::WarpingOut(s) if s >= JUMP_AT).then_some(0.0);
    t.now = Moment::Calm;
    t.frame = Frame::CALM;
    t.emergency = false;
    t.first = None;
    t.warped.clear();
    t.touched.clear();
    rig.warp_zoom = 1.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    /// The moment follows the voyage's phase and the sim's time (the overstep included); a lost test flight
    /// counts towards its leaving tick.
    #[test]
    fn the_moment_follows_the_phase_and_the_tick() {
        let warp = Phase::Warping { home_at: 300 + HOME_TICKS };
        let v = Some(0);
        assert_eq!(moment(warp, v, None, 300, 0.0), Moment::WarpingOut(0.0));
        assert_eq!(moment(warp, v, None, 330, 0.5), Moment::WarpingOut(30.5 / TICK_HZ));
        assert_eq!(moment(warp, v, None, 300 + HOME_TICKS + 4, 0.0), Moment::WarpingOut(WARP_SECONDS));
        // Not a voyage: the phase means nothing.
        assert_eq!(moment(warp, None, None, 330, 0.0), Moment::Calm);
        let lost = Phase::Lost { home_at: 600, left: 0, abandoned: true };
        assert_eq!(moment(lost, v, None, 600 - HOME_TICKS, 0.0), Moment::Leaving(0.0));
        assert_eq!(moment(lost, v, None, 600, 0.0), Moment::Leaving(WARP_SECONDS));
        // The first ARRIVE_SECONDS of a voyage arrive, counted from its first frame (here tick 4).
        assert_eq!(moment(Phase::Flying, Some(4), None, 7, 0.0), Moment::Arriving(0.1));
        assert_eq!(moment(Phase::Flying, Some(4), None, 2, 0.0), Moment::Arriving(0.0));
        let over = ((ARRIVE_SECONDS + ESCORT_LEAD_MAX) * TICK_HZ).ceil() as u64;
        assert_eq!(moment(Phase::Flying, v, None, over, 0.0), Moment::Calm);
        assert_eq!(moment(Phase::Ended, v, None, 9, 0.0), Moment::Calm);
        // A test flight 1 s before it goes back.
        assert_eq!(moment(Phase::Flying, None, Some(1000), 970, 0.0), Moment::Leaving(WARP_SECONDS - 1.0));
        assert_eq!(moment(Phase::Flying, None, None, 970, 0.0), Moment::Calm);
    }

    /// The warp-out charges up to the jump, the vessel stretches and darts away and is gone; the flash and its
    /// ring come after the jump starts; at the end nothing of it is left but the fading light.
    #[test]
    fn the_warp_out_charges_jumps_and_is_gone() {
        assert_eq!(warp_out(0.0), Look::CALM);
        let mut last = warp_out(0.0);
        for k in 1..=300 {
            let s = k as f32 / 100.0;
            let l = warp_out(s);
            if s <= JUMP_AT {
                assert!(l.charge >= last.charge && l.stretch == 1.0 && l.shift == 0.0 && l.shown, "{s}");
            } else {
                assert!(l.stretch >= last.stretch && l.shift >= last.shift, "{s}");
            }
            last = l;
        }
        assert!(close(warp_out(JUMP_AT).charge, 1.0));
        let gone = warp_out(JUMP_AT + JUMP_SECONDS + 1e-3);
        assert!(!gone.shown && gone.charge == 0.0 && close(gone.stretch, STRETCH) && close(gone.shift, SHIFT));
        assert!(warp_out(JUMP_AT + FLASH_AFTER).flash == 1.0 && warp_out(JUMP_AT).flash == 0.0);
        assert!(warp_out(JUMP_AT + FLASH_AFTER + 0.1).wave > 0.0);
        // Everything has left before the window is dark.
        let end = warp_out(WARP_SECONDS);
        assert!(!end.shown && end.trail == 0.0 && end.flash == 0.0);
        // The flagship last: an escort is further along at the same moment.
        let (flagship, escort) = (vessel_look(Moment::WarpingOut(2.5), 0).unwrap(), vessel_look(Moment::WarpingOut(2.5), 3).unwrap());
        assert!(escort.shift > flagship.shift);
        assert!(close(lead(100), ESCORT_LEAD_MAX));
    }

    /// The arrival is the reverse: unseen, then streaking in from behind, settled when it lands, its glow gone by the
    /// end; an escort lands after the flagship.
    #[test]
    fn the_arrival_streaks_in_and_settles() {
        assert!(!arrive(0.0).shown);
        let first = arrive(IN_AT);
        assert!(first.shown && close(first.stretch, STRETCH) && close(first.shift, -SHIFT));
        let landed = arrive(IN_AT + IN_SECONDS);
        assert!(close(landed.stretch, 1.0) && close(landed.shift, 0.0) && close(landed.charge, ARRIVE_GLOW));
        assert!(arrive(IN_AT + IN_SECONDS).flash > 0.5);
        let end = arrive(ARRIVE_SECONDS);
        assert!(end.shown && end.charge.abs() < 1e-4 && end.flash == 0.0 && end.trail == 0.0 && end.stretch == 1.0);
        let mut shift = arrive(IN_AT).shift;
        for k in 1..=60 {
            let l = arrive(IN_AT + IN_SECONDS * k as f32 / 60.0);
            assert!(l.shift >= shift && l.stretch <= STRETCH);
            shift = l.shift;
        }
        let (flagship, escort) = (vessel_look(Moment::Arriving(0.45), 0).unwrap(), vessel_look(Moment::Arriving(0.45), 2).unwrap());
        assert!(escort.shift < flagship.shift);
        assert!(vessel_look(Moment::Leaving(1.0), 0).is_none() && vessel_look(Moment::Calm, 0).is_none());
    }

    /// The view: the camera eases in during the charge and back out on arrival; the dip is dark exactly at the end
    /// of a warp-out or a loss, never before its last DIP_SECONDS; the HUD closes in time to have closed by the cut; a
    /// loss dims the world behind its banner; the arrival comes up out of the dark.
    #[test]
    fn the_view_dips_at_the_end_and_comes_up_on_arrival() {
        assert_eq!(frame(Moment::Calm), Frame::CALM);
        assert_eq!(frame(Moment::WarpingOut(0.0)).zoom, 1.0);
        assert!(close(frame(Moment::WarpingOut(JUMP_AT)).zoom, ZOOM_IN) && close(frame(Moment::WarpingOut(WARP_SECONDS)).zoom, ZOOM_IN));
        for m in [Moment::WarpingOut as fn(f32) -> Moment, Moment::Leaving] {
            assert_eq!(frame(m(WARP_SECONDS - DIP_SECONDS)).dip, 0.0);
            assert!(close(frame(m(WARP_SECONDS)).dip, 1.0));
            assert!(!frame(m(HUD_OUT_AT - 0.01)).hud_out && frame(m(HUD_OUT_AT)).hud_out);
            let mut d = 0.0;
            for k in 0..=300 {
                let f = frame(m(k as f32 / 100.0));
                assert!(f.dip >= d && f.dip <= 1.0);
                d = f.dip;
            }
        }
        assert_eq!(frame(Moment::WarpingOut(1.5)).dim, 0.0);
        assert!(close(frame(Moment::Leaving(1.5)).dim, LOST_DIM));
        let start = frame(Moment::Arriving(0.0));
        assert!(start.dim == 1.0 && close(start.zoom, ZOOM_IN) && start.dip == 0.0 && !start.hud_out);
        assert_eq!(frame(Moment::Arriving(ARRIVE_DIP)).dim, 0.0);
        assert!(frame(Moment::Arriving(IN_AT)).dim < 0.3, "the streak-in is not hidden in the dark");
        assert!(close(frame(Moment::Arriving(ARRIVE_SECONDS)).zoom, 1.0));
    }

    /// The flicker stays in 0..1 and changes FLICKER_HZ times a second.
    #[test]
    fn the_flicker_is_steady_within_a_step() {
        let a = flicker(1.0, 7);
        assert_eq!(a, flicker(1.0 + 0.2 / FLICKER_HZ, 7));
        let values: Vec<f32> = (0..200).map(|k| flicker(k as f32 / FLICKER_HZ, 3)).collect();
        assert!(values.iter().all(|v| (0.0..=1.0).contains(v)));
        assert!(values.iter().any(|v| *v < 0.3) && values.iter().any(|v| *v > 0.7));
    }
}
