//! The shipyard: home's world scene (GDR-0071). A long U-shaped
//! dock lies on its side, its inside the building area (the working design's drydock frame, the ship docked in it
//! as the drydock draws it), the gate in front of its mouth, the buildings attached: research with its dishes, the
//! dark fabricator and the LOG's records mast on the top wall, the test range's tower and lane and the dark salvage
//! bay under the bottom wall, the mind behind the base with the EVOLUTION lab's compute stack above it, the escorts'
//! moorings on two truss bridges left of the mind.
//!
//! * [`layout`]: where everything sits, a pure function of the frame size (a bigger frame, a bigger U).
//! * [`draw`]: the meshes, flat vector shapes in the style tokens with a few glows.
//! * [`fly`]: the camera's flight between the yard and a place (home flies it, `home.rs`).
//! * `gate`: the ring of light at the gate after a recall.
//! * `life`: what moves at home besides the rigging (beacons, the gate's ring and the wave up its approach, the
//!   mind, research's dishes, the LOG's glow, the lab's ring and glow, shuttles, the windows' lights).
//!
//! **Cost**: the station is three static meshes (the yard around the U, the building area's floor, the walls),
//! rebuilt only when the frame size, the career's lights or the glow setting change; the docked ship is one mesh
//! rebuilt with the design, each escort one. What moves (cranes sliding to the mouth for the drydock, carriages
//! riding their cranes, hoist lines, clamps, drones, and `life`'s parts and beacons) is small meshes moved by their
//! transforms; the approach's lit chevrons and each building's or wall's window lights are small meshes drawn again
//! only while one of them fades.
//!
//! **States** (from the career, [`lights`]): research is lit once anything has been researched or any research
//! node can be paid now; the gate is awake while any voyage is open (a new game opens its first one, so it is
//! awake from the start); the moorings hold the FLEET's escorts; the LOG's mast glows while a voyage's result waits
//! there unread or a data core waits to be decoded (galaxy plan E6); the EVOLUTION lab ([`draw::Evo`], from `world::evolution::Lab`) is dark until its research node (the
//! Simulation vault) is researched, dim without a run, busy while one computes, gold once one has finished until it
//! is opened; the fabricator and the salvage bay stay dark until they have screens.
//!
//! **In the drydock** the walls and the parked cranes stay around the frame (the drydock's veil dims them outside
//! its grid viewport); the floor, the yard and the drones are home's alone. **On the research screen** the whole
//! yard stays as at home, under the screen's veil (the camera rests on the lab). Elsewhere none of it shows.

pub mod draw;
pub mod fly;
mod gate;
pub mod layout;
mod life;

use bevy::prelude::*;
use data::{Blueprint, Content, career::Career};
use kit::atlas::{Atlas, AtlasMaterial, set_alpha, set_aperture};
use kit::meshbuf::MeshBuf;
use kit::params::RenderToggles;
use kit::style::Style;
use kit::ui_kit::{ScreenFx, WorldCover};
use world::camera::CameraSet;
use world::career::{EscortDesigns, PlayerCareer};
use world::fx::FxFlush;
use world::mode::AppMode;
use world::player::PlayerDesign;
use world::ship_model::{blueprint_visual, design_visual};
use world::ship_view::build_body;
use world::sim_bridge::GameContent;

use crate::drydock::panels::DrydockRoot;
use crate::map::portal::Portal;

use draw::{Evo, Lights, Palette, Pen};
use layout::{CLAMPS, CLAMP_REACH, Layout, MOORING_DEPTH, MOORING_HALF, MOORINGS};

/// Depths: the yard under the floor under the walls (clamps slide under them), the ships, then the cranes over
/// the docked ship and the drones over everything.
const Z_YARD: f32 = 1.0;
const Z_FLOOR: f32 = 2.0;
const Z_CLAMPS: f32 = 2.5;
const Z_WALLS: f32 = 3.0;
const Z_SHIP: f32 = 10.0;
const Z_CRANES: f32 = 12.0;
const Z_DRONES: f32 = 14.0;
/// How many drones work around the docked ship.
const DRONES: usize = 6;
/// The largest share of the drydock's scale an escort is drawn at in its mooring (the sketch's 0.45), smaller
/// when it would not fit.
const MOORED_SCALE: f32 = 0.45;

/// What lights the yard, by the career's rules and the lab's state (see the module doc).
pub fn lights(content: &Content, career: &Career, escorts: usize, evolution: Evo) -> Lights {
    Lights {
        research: !career.researched().is_empty() || content.research.iter().any(|n| career.can_pay(content, &n.id).is_ok()),
        gate: !career.open_voyages(content).is_empty(),
        moored: escorts.min(MOORINGS),
        log: career.unread() > 0 || career.exploration().is_some_and(|g| !g.logs().is_empty()),
        evolution,
    }
}

/// The station now: its layout (for the frame of the working design), what lights it, and how far the rigging has
/// made room for the drydock at home (0: working the ship, 1: cranes parked at the mouth, clamps retracted; home's
/// camera flight sets it, the drydock always has 1).
#[derive(Resource, Debug)]
pub struct Yard {
    pub layout: Layout,
    pub lights: Lights,
    pub clear: f32,
    /// The frame, lights and glow setting the meshes show.
    built: Option<((u32, u32), Lights, bool)>,
}

impl Yard {
    pub fn new(frame: (u32, u32)) -> Self {
        Self { layout: Layout::new(frame), lights: Lights::default(), clear: 0.0, built: None }
    }
}

/// Runs in `Update` at home and in the drydock: the yard follows the design and the career, then the rigging moves.
/// Home's camera flight goes before it (it sets [`Yard::clear`]).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShipyardSet;

/// A static layer of the station.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum Layer {
    /// Around the U: home only.
    Yard,
    /// The building area's floor: home only (the drydock draws its own dots).
    Floor,
    /// The U: home and the drydock.
    Walls,
}

#[derive(Component)]
struct Crane(usize);

#[derive(Component)]
struct Carriage(usize);

#[derive(Component)]
struct Hoist;

#[derive(Component)]
struct Clamp {
    at: usize,
    top: bool,
}

#[derive(Component)]
struct Drone(usize);

/// The docked ship (home's copy of the drydock's mesh, at the same place).
#[derive(Component)]
struct DockedShip {
    built: Option<(u64, bool)>,
}

/// An escort in its mooring.
#[derive(Component)]
struct Moored {
    slot: usize,
    built: Option<(Blueprint, bool, Vec2)>,
}

#[derive(Resource)]
struct Station {
    palette: Palette,
    /// The fading groups' materials: the U, its rigging and the docked ship; the yard around it, the floor, the
    /// drones and the moored escorts.
    core: Handle<AtlasMaterial>,
    outer: Handle<AtlasMaterial>,
}

pub struct ShipyardPlugin;

impl Plugin for ShipyardPlugin {
    fn build(&self, app: &mut App) {
        let frame = app.world().resource::<PlayerDesign>().frame();
        app.insert_resource(Yard::new(frame))
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (follow, rebuild, (rebuild_ship, rebuild_escorts).run_if(in_state(AppMode::Home).or_eager(in_state(AppMode::Research))), animate, life::animate, life::relight, life::evolution_ring)
                    .chain()
                    .in_set(ShipyardSet)
                    .run_if(in_state(AppMode::Home).or_eager(in_state(AppMode::Drydock)).or_eager(in_state(AppMode::Research)).or_eager(in_state(AppMode::Map))),
            )
            .add_systems(Update, show.after(ShipyardSet).after(crate::map::portal::PortalSet))
            .add_systems(PostUpdate, gate::gate_ring.after(CameraSet).before(FxFlush).run_if(in_state(AppMode::Home)))
            .add_systems(OnExit(AppMode::Home), gate::forget);
    }
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<AtlasMaterial>>, atlas: Res<Atlas>, style: Res<Style>) {
    let palette = Palette::new(&atlas, &style);
    // Two fading groups (see [`show`]): what stays around the drydock, and what is home's alone.
    let (core, outer) = (atlas.group_material(&mut materials), atlas.group_material(&mut materials));
    let mut spawn = |commands: &mut Commands, bundle: (Transform, Visibility), material: &Handle<AtlasMaterial>| {
        commands.spawn((Mesh2d(meshes.add(MeshBuf::placeholder())), MeshMaterial2d(material.clone()), bundle)).id()
    };
    let hidden = |z: f32| (Transform::from_xyz(0.0, 0.0, z), Visibility::Hidden);
    for (layer, z, m) in [(Layer::Yard, Z_YARD, &outer), (Layer::Floor, Z_FLOOR, &outer), (Layer::Walls, Z_WALLS, &core)] {
        let e = spawn(&mut commands, hidden(z), m);
        commands.entity(e).insert(layer);
    }
    let e = spawn(&mut commands, hidden(Z_SHIP), &core);
    commands.entity(e).insert(DockedShip { built: None });
    for slot in 0..MOORINGS {
        let e = spawn(&mut commands, hidden(Z_SHIP), &outer);
        commands.entity(e).insert(Moored { slot, built: None });
    }
    // The rigging: small meshes, built once (the cranes with the frame), moved by transforms.
    let mut pen = Pen::new(&atlas, true);
    draw::carriage(&mut pen, &palette);
    let carriage = meshes.add(pen.into_mesh());
    let mut pen = Pen::new(&atlas, true);
    draw::hoist(&mut pen, &palette);
    let hoist = meshes.add(pen.into_mesh());
    let mut pen = Pen::new(&atlas, true);
    draw::clamp(&mut pen, &palette);
    let clamp = meshes.add(pen.into_mesh());
    let mut pen = Pen::new(&atlas, true);
    draw::drone(&mut pen, &palette);
    let drone = meshes.add(pen.into_mesh());
    let material = MeshMaterial2d(core.clone());
    for k in 0..layout::CRANES.len() {
        commands
            .spawn((Crane(k), Mesh2d(meshes.add(MeshBuf::placeholder())), material.clone(), hidden(Z_CRANES)))
            .with_children(|c| {
                c.spawn((Carriage(k), Mesh2d(carriage.clone()), material.clone(), Transform::from_xyz(0.0, 0.0, 0.5)))
                    .with_child((Hoist, Mesh2d(hoist.clone()), material.clone(), Transform::from_xyz(0.0, 0.0, -0.1)));
            });
    }
    for at in 0..CLAMPS.len() {
        for top in [true, false] {
            commands.spawn((Clamp { at, top }, Mesh2d(clamp.clone()), material.clone(), hidden(Z_CLAMPS)));
        }
    }
    for k in 0..DRONES {
        commands.spawn((Drone(k), Mesh2d(drone.clone()), MeshMaterial2d(outer.clone()), hidden(Z_DRONES)));
    }
    life::spawn(&mut commands, &mut meshes, &outer);
    commands.insert_resource(Station { palette, core, outer });
}

/// The layout follows the design's frame, the lights the career, the fleet and the lab.
fn follow(design: Res<PlayerDesign>, career: Res<PlayerCareer>, escorts: Res<EscortDesigns>, lab: Res<world::evolution::Lab>, content: Res<GameContent>, mut yard: ResMut<Yard>) {
    if yard.layout.frame != design.frame() {
        yard.layout = Layout::new(design.frame());
    }
    if career.is_changed() || escorts.is_changed() || lab.is_changed() || yard.built.is_none() {
        let want = lights(&content.0, career.career(), escorts.0.len(), crate::evolution::light(&lab));
        if yard.lights != want {
            yard.lights = want;
        }
    }
}

/// The static meshes, the cranes, the moving parts, the beacons and the windows' lights, when the frame, the lights
/// or the glow setting changed.
#[allow(clippy::too_many_arguments)]
fn rebuild(
    mut commands: Commands,
    mut yard: ResMut<Yard>,
    atlas: Res<Atlas>,
    toggles: Res<RenderToggles>,
    res: Res<Station>,
    mut meshes: ResMut<Assets<Mesh>>,
    layers: Query<(&Layer, &Mesh2d)>,
    cranes: Query<&Mesh2d, With<Crane>>,
    parts: Query<(&life::Part, &Mesh2d)>,
    beacons: Query<Entity, With<life::Blink>>,
    windows: Query<(Entity, &life::Lit)>,
) {
    let key = (yard.layout.frame, yard.lights, toggles.glow_sprites);
    if yard.built == Some(key) {
        return;
    }
    let (l, pal, glow) = (&yard.layout, &res.palette, toggles.glow_sprites);
    for e in &beacons {
        commands.entity(e).despawn();
    }
    let mut lit = Vec::new();
    for (layer, mesh) in &layers {
        let mut pen = Pen::new(&atlas, glow);
        let z = match layer {
            Layer::Yard => {
                draw::yard(&mut pen, l, pal, yard.lights);
                Z_YARD
            }
            Layer::Floor => {
                draw::floor(&mut pen, l, pal);
                Z_FLOOR
            }
            Layer::Walls => {
                draw::walls(&mut pen, l, pal);
                Z_WALLS
            }
        };
        let core = *layer == Layer::Walls;
        life::respawn_beacons(&mut commands, &atlas, &mut meshes, if core { &res.core } else { &res.outer }, z, core, &pen.beacons);
        lit.extend(pen.windows.drain(..).map(|w| (w, core, z)));
        let _ = meshes.insert(&mesh.0, pen.into_mesh());
    }
    life::respawn_windows(&mut commands, &mut meshes, (&res.core, &res.outer), lit, &windows);
    life::rebuild(&atlas, glow, l, pal, yard.lights, &mut meshes, &parts);
    if yard.built.is_none_or(|b| b.0 != key.0) {
        let mut pen = Pen::new(&atlas, glow);
        draw::crane(&mut pen, l, pal);
        let crane = pen.into_mesh();
        for mesh in &cranes {
            let _ = meshes.insert(&mesh.0, crane.clone());
        }
    }
    yard.built = Some(key);
}

/// A design drawn as the drydock draws it, and its extent in the world (blueprint `(x, y)` at `(x, −y)`).
fn ship_mesh(content: &Content, bp: &Blueprint, atlas: &Atlas, style: &Style, glow: bool) -> Option<(Mesh, Rect)> {
    let built = design_visual(content, bp, style).and_then(|dv| content.ship_grid(bp).map(|grid| blueprint_visual(&dv, &grid)));
    match built {
        Ok(v) if !v.parts.is_empty() => {
            let bounds = Rect::from_corners(Vec2::new(v.bounds_min.x, -v.bounds_max.y), Vec2::new(v.bounds_max.x, -v.bounds_min.y));
            Some((build_body(&v, atlas, style, glow, true), bounds))
        }
        Ok(_) => None,
        Err(e) => {
            error!("shipyard: cannot draw {}: {e}", bp.name);
            None
        }
    }
}

/// The docked ship: the working design where it sits in the frame, rebuilt with its revision (or the glow setting).
fn rebuild_ship(
    design: Res<PlayerDesign>,
    content: Res<GameContent>,
    atlas: Res<Atlas>,
    style: Res<Style>,
    toggles: Res<RenderToggles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ship: Single<(&mut DockedShip, &Mesh2d)>,
) {
    let key = (design.revision(), toggles.glow_sprites);
    let (docked, mesh) = &mut *ship;
    if docked.built == Some(key) {
        return;
    }
    docked.built = Some(key);
    let new = ship_mesh(&content.0, design.blueprint(), &atlas, &style, toggles.glow_sprites).map_or_else(MeshBuf::placeholder, |m| m.0);
    let _ = meshes.insert(&mesh.0, new);
}

/// The escorts in their moorings, nose up as in the drydock, scaled down to fit.
#[allow(clippy::too_many_arguments)]
fn rebuild_escorts(
    escorts: Res<EscortDesigns>,
    yard: Res<Yard>,
    content: Res<GameContent>,
    atlas: Res<Atlas>,
    style: Res<Style>,
    toggles: Res<RenderToggles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut moored: Query<(&mut Moored, &Mesh2d, &mut Transform)>,
) {
    for (mut m, mesh, mut tf) in &mut moored {
        let Some(bp) = escorts.0.get(m.slot) else {
            m.built = None;
            continue;
        };
        let at = yard.layout.moorings[m.slot];
        if m.built.as_ref().is_some_and(|(b, g, a)| b == bp && *g == toggles.glow_sprites && *a == at) {
            continue;
        }
        m.built = Some((bp.clone(), toggles.glow_sprites, at));
        let Some((body, bounds)) = ship_mesh(&content.0, bp, &atlas, &style, toggles.glow_sprites) else {
            let _ = meshes.insert(&mesh.0, MeshBuf::placeholder());
            continue;
        };
        let room = Vec2::new(2.0 * MOORING_HALF, MOORING_DEPTH) - 0.6;
        let s = MOORED_SCALE.min(room.x / bounds.width()).min(room.y / bounds.height());
        *tf = Transform::from_translation((at - bounds.center() * s).extend(Z_SHIP)).with_scale(Vec3::new(s, s, 1.0));
        let _ = meshes.insert(&mesh.0, body);
    }
}

/// Cranes slide to the mouth as the yard clears for the drydock (and back), carriages ride their cranes (up onto
/// the arms when parked), hoist lines reach for the ship, clamps retract into the walls, drones circle the ship.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn animate(
    time: Res<Time<Real>>,
    mode: Res<State<AppMode>>,
    yard: Res<Yard>,
    mut cranes: Query<(&Crane, &mut Transform), (Without<Carriage>, Without<Hoist>, Without<Clamp>, Without<Drone>)>,
    mut carriages: Query<(&Carriage, &mut Transform, &Children), (Without<Crane>, Without<Hoist>, Without<Clamp>, Without<Drone>)>,
    mut hoists: Query<&mut Transform, (With<Hoist>, Without<Crane>, Without<Carriage>, Without<Clamp>, Without<Drone>)>,
    mut clamps: Query<(&Clamp, &mut Transform), (Without<Crane>, Without<Carriage>, Without<Hoist>, Without<Drone>)>,
    mut drones: Query<(&Drone, &mut Transform), (Without<Crane>, Without<Carriage>, Without<Hoist>, Without<Clamp>)>,
) {
    let t = time.elapsed_secs_wrapped();
    let l = &yard.layout;
    let a = l.area;
    // The drydock always has the room (also when the game starts there).
    let c = if *mode.get() == AppMode::Drydock { 1.0 } else { yard.clear.clamp(0.0, 1.0) };
    let set = |tf: &mut Transform, at: Vec2, angle: f32, scale: Vec2| {
        let want = Transform { translation: at.extend(tf.translation.z), rotation: Quat::from_rotation_z(angle), scale: scale.extend(1.0) };
        if *tf != want {
            *tf = want;
        }
    };
    let mut crane_x = [0.0; 3];
    for (crane, mut tf) in &mut cranes {
        let x = l.cranes[crane.0] + (l.crane_park[crane.0] - l.cranes[crane.0]) * c;
        crane_x[crane.0] = x;
        set(&mut tf, Vec2::new(x, 0.0), 0.0, Vec2::ONE);
    }
    let ship = a.center();
    for (carriage, mut tf, children) in &mut carriages {
        let k = carriage.0 as f32;
        // Up and down the rail, slowly, around the sketch's places (30, 50, 70 % down the area).
        let y = -a.height() * (0.3 + 0.2 * k) + (t * 0.21 + k * 2.1).sin() * a.height() * 0.06;
        // Parked, they ride up onto the arms (alternately, so neighbours don't overlap).
        let park = if carriage.0 % 2 == 0 { a.max.y + layout::WALL / 2.0 } else { a.min.y - layout::WALL / 2.0 };
        let y = y + (park - y) * c;
        set(&mut tf, Vec2::new(0.0, y), 0.0, Vec2::ONE);
        let from = Vec2::new(crane_x[carriage.0], y);
        let reach = (ship - from) * 0.35 * (1.0 - c);
        for child in children {
            if let Ok(mut h) = hoists.get_mut(*child) {
                set(&mut h, Vec2::ZERO, reach.to_angle(), Vec2::new(reach.length().max(1e-3), 1.0));
            }
        }
    }
    for (clamp, mut tf) in &mut clamps {
        let x = a.min.x + a.width() * CLAMPS[clamp.at];
        let back = (CLAMP_REACH + 0.4) * c;
        if clamp.top {
            set(&mut tf, Vec2::new(x, a.max.y + back), 0.0, Vec2::ONE);
        } else {
            set(&mut tf, Vec2::new(x, a.min.y - back), std::f32::consts::PI, Vec2::ONE);
        }
    }
    for (drone, mut tf) in &mut drones {
        let k = drone.0 as f32;
        // A slow figure around the ship, each drone on its own.
        let (w, f, p) = (0.11 + 0.023 * k, 1.0 + (drone.0 % 3) as f32, k * 1.7);
        let r = Vec2::new(a.width() * (0.18 + 0.025 * k), a.height() * (0.14 + 0.02 * k));
        let at = ship + Vec2::new((w * t + p).sin(), (f * w * t + p * 1.3).sin()) * r;
        let v = Vec2::new((w * t + p).cos() * w, (f * w * t + p * 1.3).cos() * f * w) * r;
        set(&mut tf, at, v.to_angle(), Vec2::ONE);
    }
}

/// Which parts show: everything at home (none while FLEET covers it) and behind the research screen, the walls and
/// the parked rigging in the drydock, nothing elsewhere.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn show(
    mode: Res<State<AppMode>>,
    cover: Res<WorldCover>,
    mut yard: ResMut<Yard>,
    station: Res<Station>,
    drydock: Query<&ScreenFx, With<DrydockRoot>>,
    mut materials: ResMut<Assets<AtlasMaterial>>,
    mut parts: ParamSet<(
        Query<(&Layer, &mut Visibility)>,
        Query<&mut Visibility, Or<(With<Crane>, With<Clamp>)>>,
        Query<&mut Visibility, With<Drone>>,
        Query<&mut Visibility, With<DockedShip>>,
        Query<(&Moored, &mut Visibility)>,
        Query<&mut Visibility, With<life::Part>>,
        Query<(&life::Blink, &mut Visibility)>,
        Query<(&life::Lit, &mut Visibility)>,
    )>,
    escorts: Res<EscortDesigns>,
    portal: Res<Portal>,
) {
    let mode = *mode.get();
    // The drydock has the room cleared (the BEHAVIOR screen opened from it finds it so).
    if mode == AppMode::Drydock && yard.clear != 1.0 {
        yard.clear = 1.0;
    }
    let (core, outer) = station_alpha(mode, cover.0, yard.clear);
    set_alpha(&mut materials, &station.core, core);
    set_alpha(&mut materials, &station.outer, outer);
    // The map's portal shows through the yard.
    for m in [&station.core, &station.outer] {
        set_aperture(&mut materials, m, portal.hole());
    }
    // The drydock's own design covers the docked ship's place while its screen shows (the same picture).
    let dock_shows = drydock.single().is_ok_and(ScreenFx::visible);
    let vis = |on: bool| if on { Visibility::Inherited } else { Visibility::Hidden };
    let put = |v: &mut Visibility, want: Visibility| {
        if *v != want {
            *v = want;
        }
    };
    for (layer, mut v) in &mut parts.p0() {
        put(&mut v, vis(if *layer == Layer::Walls { core > 0.0 } else { outer > 0.0 }));
    }
    for mut v in &mut parts.p1() {
        put(&mut v, vis(core > 0.0));
    }
    for mut v in &mut parts.p2() {
        put(&mut v, vis(outer > 0.0));
    }
    for mut v in &mut parts.p3() {
        put(&mut v, vis(core > 0.0 && mode != AppMode::Drydock && !dock_shows));
    }
    for (m, mut v) in &mut parts.p4() {
        put(&mut v, vis(outer > 0.0 && m.slot < escorts.0.len()));
    }
    for mut v in &mut parts.p5() {
        put(&mut v, vis(outer > 0.0));
    }
    for (b, mut v) in &mut parts.p6() {
        put(&mut v, vis(if b.core { core > 0.0 } else { outer > 0.0 }));
    }
    for (w, mut v) in &mut parts.p7() {
        put(&mut v, vis(if w.core { core > 0.0 } else { outer > 0.0 }));
    }
}

/// How much of the station shows in `mode`: the U, its rigging and the docked ship (`core`), and the yard around it,
/// the floor, the drones and the moorings (`outer`, also gone as far as the rigging has made room for the drydock,
/// `clear`). Screens that cover the world (`ui_kit::WorldCover`: FLEET, the BEHAVIOR screen) fade it out as they
/// open and back in as they close, so nothing of it vanishes at once; flight has a world of its own.
fn station_alpha(mode: AppMode, cover: f32, clear: f32) -> (f32, f32) {
    if mode == AppMode::Flight {
        return (0.0, 0.0);
    }
    let core = (1.0 - cover).clamp(0.0, 1.0);
    (core, core * (1.0 - clear.clamp(0.0, 1.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn career(extra: &str) -> (Content, Career) {
        let content = data::base_content();
        let mut c = Career::from_ron(&format!("(start: \"rim_awakening\", seed: 1, generator: {}{extra})", data::career::GENERATOR)).unwrap();
        c.reconcile(&content);
        (content, c)
    }

    /// The station shows at home, fades under a covering screen and with the room made for the drydock, and not in
    /// flight.
    #[test]
    fn the_station_fades_under_screens() {
        assert_eq!(station_alpha(AppMode::Home, 0.0, 0.0), (1.0, 1.0));
        assert_eq!(station_alpha(AppMode::Home, 0.0, 0.25), (1.0, 0.75));
        assert_eq!(station_alpha(AppMode::Drydock, 0.0, 1.0), (1.0, 0.0));
        assert_eq!(station_alpha(AppMode::Behavior, 0.5, 0.0), (0.5, 0.5));
        assert_eq!(station_alpha(AppMode::Home, 1.0, 0.0), (0.0, 0.0));
        assert_eq!(station_alpha(AppMode::Flight, 0.0, 0.0), (0.0, 0.0));
    }

    /// A new game: research dark (nothing to pay with), the gate awake (the first voyage is open), no escorts.
    #[test]
    fn a_new_game_has_research_dark_and_the_gate_awake() {
        let (content, c) = career("");
        assert_eq!(lights(&content, &c, 0, Evo::Dark), Lights { research: false, gate: true, moored: 0, log: false, evolution: Evo::Dark });
    }

    /// The LOG's mast glows from a voyage's end until the LOG has been opened.
    #[test]
    fn the_log_glows_until_read() {
        let (content, mut c) = career("");
        c.voyage_launched(&content, "yard_approach").unwrap();
        assert!(!lights(&content, &c, 0, Evo::Dark).log);
        c.voyage_ended(&content, "yard_approach", data::career::VoyageEnd::Lost { ticks: 30 }).unwrap();
        assert!(lights(&content, &c, 0, Evo::Dark).log);
        c.read_log();
        assert!(!lights(&content, &c, 0, Evo::Dark).log);
    }

    /// Research lights up as soon as a node can be paid, and stays lit once anything is researched.
    #[test]
    fn research_lights_when_it_can_run() {
        let (content, c) = career(", data: {\"kinetic\": 12.0}");
        assert!(lights(&content, &c, 0, Evo::Dark).research);
        let (content, c) = career(", researched: [\"automaton_cannon\"]");
        assert!(lights(&content, &c, 0, Evo::Dark).research);
        let (content, c) = career(", data: {\"kinetic\": 2.0}");
        assert!(!lights(&content, &c, 0, Evo::Dark).research, "not enough data for any node");
    }

    /// Without an open voyage the gate is dormant; escorts fill the moorings, at most four.
    #[test]
    fn the_gate_sleeps_without_voyages_and_moorings_fill_with_escorts() {
        let content = data::base_content();
        let c = Career::from_ron(&format!("(start: \"\", seed: 1, generator: {})", data::career::GENERATOR)).unwrap();
        assert!(!lights(&content, &c, 0, Evo::Dark).gate);
        let (content, c) = career("");
        assert_eq!(lights(&content, &c, 2, Evo::Dark).moored, 2);
        assert_eq!(lights(&content, &c, 9, Evo::Dark).moored, MOORINGS);
    }
}
