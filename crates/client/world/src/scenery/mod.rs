//! What fills a cluster between its sites (GDR-0104), from the voyage's or the cluster type's `scenery`:
//!
//! * **Rocks** (`rocks.rs`): the sim's asteroid rocks (`Voyage::rocks`, real bodies), drawn as they spawn: a filled
//!   outline in the kind's tone, facet lines and a lit edge.
//! * **Filler** (`filler.rs`, drawn only, never in the way): glints and motes on a near parallax layer, bits of hull
//!   drifting about (thicker round the sites), derelict shells drifting slowly along the way a little below the play.
//! * **Far things** (`far.rs`): landmarks far off on layers of their own (a derelict station, a beacon, a junk ring),
//!   and a hot star's corona at the edge of the view towards it.
//! * **The sky** (`sky/`, GDR-0105): two or three planets, their moons, a nebula and a far galaxy behind everything,
//!   from the cluster type's or the band's `sky`, rolled from the cluster's own seed (the same sky at every visit),
//!   drawn about the same size at any zoom; at home the home band's nebula and galaxy alone, fainter.
//!
//! Everything but the rocks is seeded from the voyage's seed in the client and never touches the sim; it moves with
//! the sim's clock (paused with it). Each piece sits at an anchor in the world with a parallax `p` (GDR-0107): it is
//! drawn at `camera + (anchor − camera) · p · s` and `s` times its size, `s = camera::depth_scale(p)`, so at `p = 1` it
//! lies in the play and scales with the zoom, nearer 0 it hangs further off and keeps its size on screen.

mod far;
mod filler;
mod rocks;
mod sky;

use bevy::prelude::*;
use kit::atlas::{Atlas, AtlasMaterial};
use kit::style::Style;

use crate::camera::{CameraSet, MainCamera, depth_scale};
use crate::mode::AppMode;
use crate::sim_bridge::{GameContent, SimWorld, v2};
use crate::voyage::VoyageRun;

pub struct SceneryPlugin;

impl Plugin for SceneryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Scenery>()
            .add_systems(Update, (sync, rocks::draw_rocks).chain())
            .add_systems(
                PostUpdate,
                (place, filler::place_tiles, far::corona, far::blink, sky::place)
                    .after(CameraSet)
                    .before(bevy::transform::TransformSystems::Propagate),
            );
    }
}

/// The scenery of the voyage being drawn: which voyage (id and seed), what its content says and how many of its
/// rocks have their views.
#[derive(Resource, Default)]
struct Scenery {
    key: Option<(String, u64)>,
    spec: Option<Spec>,
    rocks_drawn: usize,
}

/// What a voyage's scenery is drawn from.
struct Spec {
    scenery: data::Scenery,
    seed: u64,
    radius: f32,
    entry: Vec2,
    sites: Vec<Vec2>,
    star: Option<Vec2>,
    /// The sky (the cluster type's, else the band's) and the seed it rolls from.
    sky: Option<data::Sky>,
    sky_seed: u64,
}

/// The shipyard's sky: its seed and how strong it shows (fainter than in flight, under the station's screens).
const HOME_SKY_SEED: u64 = 0x5eed_f00d;
const HOME_SKY_STRENGTH: f32 = 0.55;

/// Every entity of the scenery (despawned when the voyage changes or the flight ends).
#[derive(Component)]
struct SceneryPart;

/// A piece of scenery at `anchor` (plus `drift` metres a second) on a layer of parallax `parallax`, turning at `spin`
/// radians a second from `angle`.
#[derive(Component, Clone, Copy)]
struct Layered {
    anchor: Vec2,
    drift: Vec2,
    angle: f32,
    spin: f32,
    parallax: f32,
}

/// The sim's clock in seconds (the tick plus the fixed step's overstep): the scenery moves with it, so it pauses with
/// P and looks the same at any frame rate.
fn clock(world: &SimWorld, fixed: &Time<Fixed>) -> f32 {
    (world.sim.tick() as f32 + fixed.overstep_fraction().clamp(0.0, 1.0)) / sim::TICK_HZ as f32
}

/// The camera's metres a logical pixel.
fn metres_per_px(proj: &Projection) -> f32 {
    match proj {
        Projection::Orthographic(o) => o.scale,
        _ => 1.0,
    }
}

/// The view's centre and size in metres.
fn view(cam: &Camera, proj: &Projection, tf: &Transform) -> (Vec2, Vec2) {
    (tf.translation.truncate(), cam.logical_viewport_size().unwrap_or(Vec2::splat(1000.0)) * metres_per_px(proj))
}

/// A seed for the client's decoration generator from the voyage's seed and a salt (each part rolls on its own).
fn salted(seed: u64, salt: u32) -> u32 {
    let x = seed ^ (seed >> 32) ^ u64::from(salt).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    (x ^ (x >> 29)) as u32 | 1
}

/// Rebuilds the scenery when the voyage changes (a new launch, a replay, leaving flight): despawns the old one and
/// spawns the new filler and far things.
#[allow(clippy::too_many_arguments)]
fn sync(
    mut commands: Commands,
    mut state: ResMut<Scenery>,
    world: Res<SimWorld>,
    game: Res<GameContent>,
    run: Res<VoyageRun>,
    mode: Res<State<AppMode>>,
    atlas: Res<Atlas>,
    style: Res<Style>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<AtlasMaterial>>,
    params: Res<kit::params::Params>,
    parts: Query<Entity, With<SceneryPart>>,
) {
    let voyage = world.sim.voyage().filter(|_| *mode.get() == AppMode::Flight);
    // the shipyard's sky: the home band's (the galaxy's first) nebula and galaxy, fainter, without planets (Niklas,
    // 18 Oct 2026), from the packs (the running scene's content has no galaxy)
    let packs = &game.0;
    let home_sky = (*mode.get() == AppMode::Home)
        .then(|| packs.galaxies.iter().next().and_then(|g| g.bands.first()).and_then(|b| packs.bands.get(b)).and_then(|b| b.sky.as_ref()))
        .flatten();
    let key = if home_sky.is_some() { Some((String::from("home"), HOME_SKY_SEED)) } else { voyage.map(|v| (v.id().to_string(), v.seed())) };
    if key == state.key {
        return;
    }
    for e in &parts {
        commands.entity(e).despawn();
    }
    state.key = key;
    state.rocks_drawn = 0;
    if let Some(sky) = home_sky {
        let cx = &mut Build { commands: &mut commands, atlas: &atlas, style: &style, meshes: &mut meshes, materials: &mut materials };
        let s = sky::SkySpec { sky, seed: HOME_SKY_SEED, star: None, planet: None, over: None, strength: HOME_SKY_STRENGTH };
        if let Some(c) = sky::spawn(cx, &s, packs) {
            let [_, _, nebula, galaxy] = c.vertices;
            info!("sky at home: vertices nebula {nebula}, galaxy {galaxy}");
        }
        state.spec = None;
        return;
    }
    state.spec = voyage.and_then(|v| {
        // A generated cluster flies its type's scenery; a content voyage (also one replayed) its own. The sky is the
        // cluster type's or else its band's, a content voyage's its band's.
        let content = &world.content;
        let cluster = run.launched.as_ref().filter(|l| l.voyage == v.id()).and_then(|l| l.cluster.as_ref());
        let band_sky = |band: &str| content.bands.get(band).and_then(|b| b.sky.clone());
        let (scenery, sky, sky_seed) = match cluster {
            Some(c) => {
                let t = content.cluster_types.get(&c.interior.kind)?;
                (t.scenery.clone(), t.sky.clone().or_else(|| band_sky(&c.band)), c.sky_seed)
            }
            None => {
                let voyage = content.voyages.get(v.id())?;
                (voyage.scenery.clone(), band_sky(&voyage.band), data::hash::fnv1a64(v.id().as_bytes()))
            }
        };
        Some(Spec {
            scenery,
            seed: v.seed(),
            radius: v.radius(),
            entry: v2(v.entry().pos),
            sites: v.sites().iter().map(|s| v2(s.pos())).collect(),
            star: v.star().map(|s| v2(s.pos)),
            sky,
            sky_seed,
        })
    });
    let Some(spec) = &state.spec else { return };
    let cx = &mut Build { commands: &mut commands, atlas: &atlas, style: &style, meshes: &mut meshes, materials: &mut materials };
    let counts = filler::spawn(cx, spec);
    let far = far::spawn(cx, spec, &world.content);
    info!(
        "scenery: {} glint tiles of {} motes, {} debris groups ({} bits), {} derelicts, {} landmarks{}",
        counts.tiles, counts.motes, counts.groups, counts.bits, counts.derelicts, far.landmarks, if far.corona { ", a corona" } else { "" }
    );
    if let Some(sky) = &spec.sky {
        let star = spec.star.map(|s| s - spec.entry);
        let over = sky::Over { radius: spec.radius, entry: spec.entry };
        let s = sky::SkySpec { sky, seed: spec.sky_seed, star, planet: params.text("planet"), over: Some(over), strength: sky::STRENGTH };
        let start = bevy::platform::time::Instant::now();
        if let Some(c) = sky::spawn(cx, &s, &world.content) {
            let [planets, moons, nebula, galaxy] = c.vertices;
            info!(
                "sky: planets [{}] with moons [{}]; vertices: planets {planets}, moons {moons}, nebula {nebula} (at most {} blobs over a point), galaxy {galaxy}; built in {:.0} ms",
                c.planets.join(", "),
                c.moons.join(", "),
                c.nebula_peak,
                start.elapsed().as_secs_f32() * 1000.0
            );
        }
    }
}

/// What spawning the scenery needs.
struct Build<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    atlas: &'a Atlas,
    style: &'a Style,
    meshes: &'a mut Assets<Mesh>,
    materials: &'a mut Assets<AtlasMaterial>,
}

/// Puts every layered piece where its parallax shows it now, at its depth's scale (`camera::depth_scale`).
#[allow(clippy::type_complexity)]
fn place(
    world: Res<SimWorld>,
    fixed: Res<Time<Fixed>>,
    camera: Single<(&Projection, &Transform), (With<MainCamera>, Without<Layered>)>,
    mut parts: Query<(&Layered, &mut Transform)>,
) {
    let (proj, cam) = camera.into_inner();
    let (cam, k) = (cam.translation.truncate(), metres_per_px(proj));
    let t = clock(&world, &fixed);
    for (l, mut tf) in &mut parts {
        let s = depth_scale(k, l.parallax);
        let at = cam + (l.anchor + l.drift * t - cam) * l.parallax * s;
        tf.translation.x = at.x;
        tf.translation.y = at.y;
        tf.rotation = Quat::from_rotation_z(l.angle + l.spin * t);
        tf.scale = Vec3::new(s, s, 1.0);
    }
}
