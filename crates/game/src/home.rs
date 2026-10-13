//! Home: the shipyard's ring menu around the docked ship (GDR-0008, GDR-0062), where the game starts
//! (`?scene=home`, the default). The look follows `docs/concept/boards/ui-mockups/phone-home-ring-menu.png`, laid
//! out for the desktop at 1280 × 720 logical pixels; larger windows scale it like the drydock (`UiScale` =
//! min(w / 1280, h / 720), 1 to 2).
//!
//! ```text
//! THE SHIPYARD · RIM                         (#) DRYDOCK  D
//! Halvex Runner
//!               BEHAVIOR  B (⑂)       · · ·        (o) RESEARCH
//!                             ·  .----------.  ·       COMING SOON
//!                            ·   |  the ship |   ·
//!                             ·  '----------'  ·
//!                    FLEET (⁂)        · · ·        (◎) TEST RANGE  T
//!              COMING SOON          (A) VOYAGES
//!                                       COMING SOON
//! ```
//!
//! * **The docked ship** is the drydock's working design ([`PlayerDesign`]), drawn with the flight tiles exactly as
//!   the drydock draws it (`ship_model::design_visual` → `blueprint_visual` → `ship_view::build_body`, one mesh,
//!   rebuilt when the design's revision changes) and framed in the dock rectangle by the shared camera rig. A
//!   dotted ring runs through the buttons and a dotted dock rectangle with gold corners surrounds the ship
//!   (immediate-mode FX, like the drydock's frame).
//! * **The ring** ([`SLOTS`]): DRYDOCK (D), BEHAVIOR (B: the BEHAVIOR screen for the working design) and TEST
//!   RANGE (T: the sandbox with raider waves, `scene::Launch::Range`) work. RESEARCH, VOYAGES and FLEET are
//!   disabled rings ("COMING SOON") until their tracks give them an action. Every working button and its key
//!   send one message, [`HomeAction`].
//! * **Coming back**: the ring button top left of the drydock and the BEHAVIOR screen, and M in flight.
//!
//! The screen and the ship's mesh are spawned once at start and hidden outside home (like the drydock's). Entering
//! home keeps the flight camera's zoom and leaving gives it back with the normal UI scale, so the drydock and
//! flight find the camera as they left it. UI words are chrome in code, like the drydock's; the shipyard's place
//! name moves to content once content has places.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use bevy::window::PrimaryWindow;

use crate::RenderToggles;
use crate::atlas::Atlas;
use crate::camera::{CamMode, CameraRig, CameraSet, MIN_PX_PER_M};
use crate::drydock::PlayerDesign;
use crate::flight_hud::HudFonts;
use crate::fx::{FxFlush, FxQueue, FxRegions};
use crate::meshbuf::MeshBuf;
use crate::mode::AppMode;
use crate::scene::{Launch, Scene};
use crate::ship_model::{blueprint_visual, design_visual};
use crate::ship_view::build_body;
use crate::sim_bridge::GameContent;
use crate::style::{Style, ui_color};
use crate::ui_icons::{Glyph, UiIcon};
use crate::ui_kit::{Ring, RingAccent, Tone, UiClick, UiInputSet, UiStyleSet, label, prose, ring_node, set_text};

/// The layout's size in logical px (the stage, centred in the window).
const STAGE: Vec2 = Vec2::new(1280.0, 720.0);
/// The ring's centre on the stage and its half axes.
const CENTRE: Vec2 = Vec2::new(640.0, 372.0);
const RADII: Vec2 = Vec2::new(400.0, 252.0);
/// The dock rectangle around the ship (centred on the ring).
const DOCK: Vec2 = Vec2::new(440.0, 290.0);
/// Ring button diameters; the voyage button is the big one, as LAUNCH VOYAGE in the mockup.
const RING: f32 = 84.0;
const BIG_RING: f32 = 104.0;
/// Empty cells kept around the ship in the dock, and the largest cell (px on the 1280 × 720 layout), so a tiny
/// design doesn't fill the dock with one tile.
const FIT_MARGIN: f32 = 1.0;
const MAX_CELL: f32 = 48.0;
/// The ship's mesh sits where the drydock's does; the dotted ring and dock under it.
const Z_SHIP: f32 = 10.0;
const Z_DOTS: f32 = 4.0;
/// Dots: spacing and size (logical px).
const DOT_STEP: f32 = 10.0;
const DOT: f32 = 2.2;
const CORNER_DOT: f32 = 4.5;
/// cos 45°: the diagonal slots.
const DIAG: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// What home's buttons and keys do.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum HomeAction {
    /// DRYDOCK (D): the ship editor.
    Drydock,
    /// BEHAVIOR (B): the BEHAVIOR screen for the working design.
    Behavior,
    /// TEST RANGE (T): the sandbox with raider waves.
    TestRange,
}

impl HomeAction {
    const ALL: [HomeAction; 3] = [HomeAction::Drydock, HomeAction::Behavior, HomeAction::TestRange];

    /// The key and its name on the button.
    fn key(self) -> (KeyCode, &'static str) {
        match self {
            HomeAction::Drydock => (KeyCode::KeyD, "D"),
            HomeAction::Behavior => (KeyCode::KeyB, "B"),
            HomeAction::TestRange => (KeyCode::KeyT, "T"),
        }
    }
}

/// One button of the ring.
struct Slot {
    label: &'static str,
    glyph: Glyph,
    accent: Tone,
    /// Where on the ring: a direction (x right, y down) scaled by [`RADII`].
    dir: Vec2,
    diameter: f32,
    /// What it does; `None`: not yet (a disabled ring, "COMING SOON").
    action: Option<HomeAction>,
}

/// The ring, clockwise from the top: the mockup's places for the six buttons of the slice (its galaxy map,
/// fabricator and storage wait for their phases).
const SLOTS: [Slot; 6] = [
    Slot { label: "DRYDOCK", glyph: Glyph::Grid, accent: Tone::Number, dir: Vec2::new(0.0, -1.0), diameter: RING, action: Some(HomeAction::Drydock) },
    Slot { label: "RESEARCH", glyph: Glyph::Orbit, accent: Tone::Teal, dir: Vec2::new(DIAG, -DIAG), diameter: RING, action: None },
    Slot { label: "TEST RANGE", glyph: Glyph::Target, accent: Tone::Coral, dir: Vec2::new(DIAG, DIAG), diameter: RING, action: Some(HomeAction::TestRange) },
    Slot { label: "VOYAGES", glyph: Glyph::Arrow, accent: Tone::Teal, dir: Vec2::new(0.0, 1.0), diameter: BIG_RING, action: None },
    Slot { label: "FLEET", glyph: Glyph::Formation, accent: Tone::Gold, dir: Vec2::new(-DIAG, DIAG), diameter: RING, action: None },
    Slot { label: "BEHAVIOR", glyph: Glyph::Tree, accent: Tone::Violet, dir: Vec2::new(-DIAG, -DIAG), diameter: RING, action: Some(HomeAction::Behavior) },
];

/// Home's UI root (shown only at home).
#[derive(Component)]
struct HomeRoot;

/// The design's name top left.
#[derive(Component)]
struct DesignName;

/// A ring of [`SLOTS`] (the dotted ring skips it); the working ones carry a [`HomeButton`].
#[derive(Component)]
struct SlotRing;

/// A working button.
#[derive(Component, Clone, Copy)]
struct HomeButton(HomeAction);

/// The node the dotted ring is drawn through (its size is the ring's).
#[derive(Component)]
struct RingPath;

/// The dock rectangle: the camera frames the ship in it.
#[derive(Component)]
struct Dock;

/// The docked ship's mesh.
#[derive(Component)]
struct HomeShip {
    /// The revision and glow setting the mesh shows.
    built: Option<(u64, bool)>,
    /// The ship's extent in the world (blueprint `(x, y)` sits at `(x, −y)`).
    bounds: Rect,
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
            .add_systems(Startup, (setup, spawn.after(crate::flight_hud::spawn_hud)))
            .add_systems(PostStartup, (show, remember_start_zoom.run_if(in_state(AppMode::Home))))
            .add_systems(OnEnter(AppMode::Home), enter)
            .add_systems(OnExit(AppMode::Home), exit)
            .add_systems(Update, show.run_if(state_changed::<AppMode>))
            .add_systems(
                Update,
                (ui_scale, (clicks, keys), apply, rebuild_ship, frame, sync_name)
                    .chain()
                    .after(UiInputSet)
                    .before(UiStyleSet)
                    .run_if(in_state(AppMode::Home)),
            )
            .add_systems(PostUpdate, draw_dots.run_if(in_state(AppMode::Home)).after(CameraSet).before(FxFlush));
    }
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, atlas: Res<Atlas>) {
    commands.spawn((
        HomeShip { built: None, bounds: Rect::default() },
        Mesh2d(meshes.add(MeshBuf::placeholder())),
        MeshMaterial2d(atlas.material.clone()),
        Transform::from_xyz(0.0, 0.0, Z_SHIP),
        Visibility::Hidden,
    ));
}

fn spawn(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let fonts = &*fonts;
    let abs = |at: Vec2, size: Vec2| Node {
        position_type: PositionType::Absolute,
        left: px(at.x),
        top: px(at.y),
        width: px(size.x),
        height: px(size.y),
        ..default()
    };
    commands
        .spawn((HomeRoot, Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() }, Visibility::Hidden))
        .with_children(|root| {
            // Top left: where we are and the docked design's name.
            root.spawn(Node {
                position_type: PositionType::Absolute,
                left: px(28),
                top: px(22),
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                ..default()
            })
            .with_children(|tl| {
                tl.spawn(label(fonts, "THE SHIPYARD  ·  RIM", 11.0, c(Tone::Label, 1.0)));
                tl.spawn((DesignName, prose(fonts, "", 28.0, c(Tone::Number, 1.0))));
            });
            // The stage, centred in the window: the ring, its buttons and the dock.
            root.spawn(Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|centre| {
                centre.spawn(Node { width: px(STAGE.x), height: px(STAGE.y), flex_shrink: 0.0, ..default() }).with_children(|stage| {
                    stage.spawn((RingPath, abs(CENTRE - RADII, 2.0 * RADII)));
                    stage.spawn((Dock, abs(CENTRE - DOCK / 2.0, DOCK)));
                    for slot in &SLOTS {
                        spawn_slot(stage, fonts, &style, slot);
                    }
                });
            });
        });
}

/// A ring button with its label beside it: left of the ring on the left half, right of it elsewhere.
fn spawn_slot(stage: &mut ChildSpawnerCommands, fonts: &HudFonts, style: &Style, slot: &Slot) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let d = slot.diameter;
    let at = CENTRE + slot.dir * RADII;
    let left = slot.dir.x < 0.0;
    let row = Node {
        position_type: PositionType::Absolute,
        top: px(at.y - d / 2.0),
        height: px(d),
        left: if left { Val::Auto } else { px(at.x - d / 2.0) },
        right: if left { px(STAGE.x - at.x - d / 2.0) } else { Val::Auto },
        flex_direction: if left { FlexDirection::RowReverse } else { FlexDirection::Row },
        align_items: AlignItems::Center,
        column_gap: px(18),
        ..default()
    };
    let glyph_color = if slot.action.is_some() { slot.accent.color(&ui) } else { ui.dim };
    stage.spawn(row).with_children(|r| {
        let mut ring = r.spawn((ring_node(d), RingAccent(slot.accent), SlotRing));
        match slot.action {
            Some(action) => ring.insert(HomeButton(action)),
            None => ring.insert(Ring { disabled: true }),
        };
        ring.with_child((UiIcon::Glyph { glyph: slot.glyph, color: glyph_color }, Node { width: px(d * 0.4), height: px(d * 0.4), ..default() }));
        let align = if left { AlignItems::FlexEnd } else { AlignItems::FlexStart };
        r.spawn(Node { flex_direction: FlexDirection::Column, align_items: align, row_gap: px(7), ..default() }).with_children(|col| {
            match slot.action {
                Some(action) => {
                    col.spawn(label(fonts, slot.label, 15.0, c(Tone::Number, 0.88))).with_child((
                        TextSpan::new(format!("  {}", action.key().1)),
                        TextFont { font: fonts.regular.clone().into(), font_size: FontSize::Px(15.0), ..default() },
                        TextColor(c(Tone::Dim, 1.0)),
                        bevy::text::LetterSpacing::Px(15.0 * 0.3),
                    ));
                }
                None => {
                    col.spawn(label(fonts, slot.label, 15.0, c(Tone::Dim, 1.0)));
                    col.spawn(label(fonts, "COMING SOON", 10.5, c(Tone::Dim, 0.8)));
                }
            }
        });
    });
}

/// Home's screen and ship are shown only at home.
fn show(mode: Res<State<AppMode>>, mut nodes: Query<&mut Visibility, Or<(With<HomeRoot>, With<HomeShip>)>>) {
    let want = if *mode.get() == AppMode::Home { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut nodes {
        if *v != want {
            *v = want;
        }
    }
}

/// Coming home: keep the flight zoom; the camera stops following (`frame` places it).
fn enter(mut rig: ResMut<CameraRig>, mut cam: ResMut<HomeCamera>) {
    cam.flight_zoom = rig.zoom_target;
    rig.mode = CamMode::Free;
    rig.auto_mode = CamMode::Free;
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

/// The UI scale: the layout is made for 1280 × 720 logical pixels and grows with larger windows (as the drydock).
fn ui_scale(window: Single<&Window, With<PrimaryWindow>>, mut ui_scale: ResMut<UiScale>) {
    let s = (window.width() / STAGE.x).min(window.height() / STAGE.y).clamp(1.0, 2.0);
    if (ui_scale.0 - s).abs() > 1e-3 {
        ui_scale.0 = s;
    }
}

/// Clicks on the working buttons (the disabled rings have no action).
fn clicks(mut clicks: MessageReader<UiClick>, buttons: Query<&HomeButton>, mut out: MessageWriter<HomeAction>) {
    for c in clicks.read() {
        if let Ok(b) = buttons.get(c.0) {
            out.write(b.0);
        }
    }
}

/// D, B, T.
fn keys(keys: Res<ButtonInput<KeyCode>>, mut out: MessageWriter<HomeAction>) {
    for a in HomeAction::ALL {
        if keys.just_pressed(a.key().0) {
            out.write(a);
        }
    }
}

/// Where the buttons go.
fn apply(mut actions: MessageReader<HomeAction>, mut scene: ResMut<Scene>, mut next: ResMut<NextState<AppMode>>) {
    for a in actions.read() {
        match a {
            HomeAction::Drydock => next.set(AppMode::Drydock),
            HomeAction::Behavior => next.set(AppMode::Behavior),
            HomeAction::TestRange => {
                scene.launch = Some(Launch::Range);
                next.set(AppMode::Flight);
            }
        }
    }
}

/// Rebuilds the ship's mesh when the design's revision (or the glow setting) changes: the drydock's tiles.
fn rebuild_ship(
    design: Res<PlayerDesign>,
    content: Res<GameContent>,
    atlas: Res<Atlas>,
    style: Res<Style>,
    toggles: Res<RenderToggles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ship: Single<(&mut HomeShip, &Mesh2d)>,
) {
    let key = (design.revision(), toggles.glow_sprites);
    let (home, mesh) = &mut *ship;
    if home.built == Some(key) {
        return;
    }
    home.built = Some(key);
    let (content, bp) = (&content.0, design.blueprint());
    let built = design_visual(content, bp, &style).and_then(|dv| content.ship_grid(bp).map(|grid| blueprint_visual(&dv, &grid)));
    let new = match built {
        Ok(v) => {
            home.bounds = Rect::from_corners(Vec2::new(v.bounds_min.x, -v.bounds_max.y), Vec2::new(v.bounds_max.x, -v.bounds_min.y));
            build_body(&v, &atlas, &style, toggles.glow_sprites, true)
        }
        Err(e) => {
            error!("home: cannot draw the design: {e}");
            MeshBuf::placeholder()
        }
    };
    let _ = meshes.insert(&mesh.0, new);
}

/// The camera frames the ship in the dock rectangle (once the layout is there).
fn frame(
    window: Single<&Window, With<PrimaryWindow>>,
    dock: Single<(&ComputedNode, &UiGlobalTransform), With<Dock>>,
    ship: Single<&HomeShip>,
    mut rig: ResMut<CameraRig>,
) {
    let (node, tf) = *dock;
    let scale = window.scale_factor();
    let size = node.size / scale;
    rig.viewport = window.size();
    if size.min_element() < 8.0 {
        return;
    }
    rig.view_offset = tf.translation / scale - rig.viewport / 2.0;
    let b = ship.bounds;
    let fit = (size.x / (b.width() + 2.0 * FIT_MARGIN)).min(size.y / (b.height() + 2.0 * FIT_MARGIN));
    let zoom = fit.min(MAX_CELL * size.x / DOCK.x).max(MIN_PX_PER_M);
    rig.pos = b.center();
    rig.zoom = zoom;
    rig.zoom_target = zoom;
}

fn sync_name(design: Res<PlayerDesign>, mut name: Single<&mut Text, With<DesignName>>) {
    set_text(&mut name, &design.blueprint().name);
}

/// The dotted ring through the buttons (not under them) and the dotted dock with gold corners, in screen pixels.
#[allow(clippy::type_complexity)]
fn draw_dots(
    window: Single<&Window, With<PrimaryWindow>>,
    path: Single<(&ComputedNode, &UiGlobalTransform), With<RingPath>>,
    dock: Single<(&ComputedNode, &UiGlobalTransform), With<Dock>>,
    rings: Query<(&ComputedNode, &UiGlobalTransform), With<SlotRing>>,
    rig: Res<CameraRig>,
    style: Res<Style>,
    regions: Option<Res<FxRegions>>,
    mut queue: ResMut<FxQueue>,
) {
    let Some(r) = regions else { return };
    let scale = window.scale_factor();
    let rect = |(node, tf): (&ComputedNode, &UiGlobalTransform)| Rect::from_center_size(tf.translation / scale, node.size / scale);
    let (path, dock) = (rect(*path), rect(*dock));
    // Logical px per layout px (the UI scale), for dot sizes and spacing.
    let k = path.width() / (2.0 * RADII.x);
    if k <= 0.0 {
        return;
    }
    let holes: Vec<(Vec2, f32)> = rings.iter().map(|(n, tf)| (tf.translation / scale, n.size.x / scale / 2.0 + 8.0 * k)).collect();
    let px = 1.0 / rig.zoom;
    let dot_c = style.dotted_ring.with_alpha(0.9);
    let mut dot = |at: Vec2, size: f32, color: Color| queue.dot(&r.hard, rig.screen_to_world(at), size * k * px, color, Z_DOTS);

    // The ring: an ellipse through the button centres.
    let half = path.size() / 2.0;
    let around = std::f32::consts::TAU * ((half.x * half.x + half.y * half.y) / 2.0).sqrt();
    let n = (around / (DOT_STEP * k)).round().max(8.0) as usize;
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        let at = path.center() + Vec2::new(a.cos(), a.sin()) * half;
        if holes.iter().all(|(c, radius)| at.distance(*c) > *radius) {
            dot(at, DOT, dot_c);
        }
    }
    // The dock: dotted sides, gold corners.
    let corners = [dock.min, Vec2::new(dock.max.x, dock.min.y), dock.max, Vec2::new(dock.min.x, dock.max.y)];
    for (s, &from) in corners.iter().enumerate() {
        let to = corners[(s + 1) % 4];
        let steps = (from.distance(to) / (DOT_STEP * k)).round().max(1.0) as usize;
        for j in 1..steps {
            dot(from.lerp(to, j as f32 / steps as f32), DOT * 0.8, dot_c.with_alpha(0.6));
        }
        dot(from, CORNER_DOT, style.ui.gold.with_alpha(0.85));
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
            .add_systems(Update, apply);
        app
    }

    /// TEST RANGE asks for the plain sandbox (not a test flight) and flies; DRYDOCK and BEHAVIOR open their screens.
    #[test]
    fn the_ring_buttons_lead_to_their_modes() {
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
    }

    /// Every working action has one button and its own key; the buttons sit on the stage, apart from each other and
    /// outside the dock.
    #[test]
    fn the_ring_is_complete_and_its_buttons_do_not_overlap() {
        for a in HomeAction::ALL {
            assert_eq!(SLOTS.iter().filter(|s| s.action == Some(a)).count(), 1, "{a:?}");
            assert_eq!(HomeAction::ALL.iter().filter(|b| b.key() == a.key()).count(), 1, "{a:?}");
        }
        let dock = Rect::from_center_size(CENTRE, DOCK);
        for (i, s) in SLOTS.iter().enumerate() {
            let at = CENTRE + s.dir * RADII;
            let r = s.diameter / 2.0;
            assert!(at.x - r >= 0.0 && at.y - r >= 0.0 && at.x + r <= STAGE.x && at.y + r <= STAGE.y, "{}", s.label);
            let near = Vec2::new(at.x.clamp(dock.min.x, dock.max.x), at.y.clamp(dock.min.y, dock.max.y));
            assert!(near.distance(at) > r, "{} overlaps the dock", s.label);
            for t in &SLOTS[i + 1..] {
                assert!(at.distance(CENTRE + t.dir * RADII) > r + t.diameter / 2.0, "{} and {}", s.label, t.label);
            }
        }
    }
}
