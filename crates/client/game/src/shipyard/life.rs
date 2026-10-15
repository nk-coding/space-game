//! What keeps the yard alive at home (Niklas, 15 Oct 2026: "the station should feel a bit more alive"): beacons
//! that breathe and flash, the gate's ring turning and a light running up its approach while it is awake, the
//! mind's spokes turning around its beating core, research's dishes sweeping and its orbit turning while research
//! runs, and two shuttles plying from the far moorings to research and to the salvage bay.
//!
//! Each is a small mesh of its own, built with the yard (`super::rebuild`) and then only moved, turned or scaled by
//! its transform, so nothing is rebuilt while it moves. The beacons' white cores stay in the static meshes; only
//! their glows blink.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;
use kit::atlas::{Atlas, AtlasMaterial};
use kit::meshbuf::MeshBuf;

use super::draw::{self, Beacon, Lights, Palette, Pen};
use super::layout::Layout;

/// Depths: the gate's ring and pulse, the mind's and research's moving parts just over the yard (under the floor and
/// the walls); the shuttles over everything but the drones.
const Z_PARTS: f32 = 1.3;
const Z_SHUTTLES: f32 = 13.0;
/// A beacon's glow just over the layer its core is drawn in.
const Z_BEACON: f32 = 0.4;

/// Shuttle speed (m/s), and how long one waits at each end of its run (s).
const SHUTTLE_SPEED: f32 = 2.4;
const SHUTTLE_DWELL: f32 = 3.0;
/// One turn of the gate's ring and of the mind's spokes, and of research's orbit (s).
const GATE_TURN: f32 = 120.0;
const MIND_TURN: f32 = 40.0;
const ORBIT_TURN: f32 = 30.0;
/// The lights up the approach: one run (s), and how many run at once, evenly apart (Niklas, 15 Oct 2026: one
/// light every 2.8 s was too sparse; now one every 0.9 s).
const PULSE_RUN: f32 = 1.8;
const PULSES: usize = 2;

/// A moving part of the yard.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    GateRing,
    GatePulse(usize),
    MindRotor,
    MindCore,
    Dish(usize),
    Orbit,
    Shuttle(usize),
}

impl Part {
    const ALL: [Part; 10] = [
        Part::GateRing,
        Part::GatePulse(0),
        Part::GatePulse(1),
        Part::MindRotor,
        Part::MindCore,
        Part::Dish(0),
        Part::Dish(1),
        Part::Orbit,
        Part::Shuttle(0),
        Part::Shuttle(1),
    ];

    fn z(self) -> f32 {
        match self {
            Part::Shuttle(_) => Z_SHUTTLES,
            _ => Z_PARTS,
        }
    }

    fn draw(self, pen: &mut Pen, l: &Layout, pal: &Palette, lights: Lights) {
        let lab = if lights.research { pal.gold } else { pal.dim };
        match self {
            Part::GateRing => draw::gate_ring(pen, pal, lights.gate),
            Part::GatePulse(_) => draw::gate_pulse(pen, pal),
            Part::MindRotor => draw::mind_rotor(pen, pal),
            Part::MindCore => draw::mind_core(pen, pal),
            Part::Dish(k) => draw::dish(pen, pal, draw::research_dishes(l)[k].1.0, lab),
            Part::Orbit => draw::orbit(pen, lab),
            Part::Shuttle(_) => draw::shuttle(pen, pal),
        }
    }
}

/// A blinking light's glow: `core` when it belongs to the U (it stays around the drydock), else to the yard.
#[derive(Component, Clone, Copy, Debug)]
pub struct Blink {
    pub beacon: Beacon,
    pub core: bool,
    phase: f32,
}

/// Spawns the moving parts (empty until the yard is built), in the yard's fading group.
pub fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, outer: &Handle<AtlasMaterial>) {
    for part in Part::ALL {
        commands.spawn((part, Mesh2d(meshes.add(MeshBuf::placeholder())), MeshMaterial2d(outer.clone()), Transform::from_xyz(0.0, 0.0, part.z()), Visibility::Hidden));
    }
}

/// The moving parts' meshes for this layout and these lights.
pub fn rebuild(atlas: &Atlas, glow: bool, l: &Layout, pal: &Palette, lights: Lights, meshes: &mut Assets<Mesh>, parts: &Query<(&Part, &Mesh2d)>) {
    for (part, mesh) in parts {
        let mut pen = Pen::new(atlas, glow);
        part.draw(&mut pen, l, pal, lights);
        let _ = meshes.insert(&mesh.0, pen.into_mesh());
    }
}

/// The beacons again, as the static layers collected them (`core`: the U's, else the yard's).
pub fn respawn_beacons(
    commands: &mut Commands,
    atlas: &Atlas,
    meshes: &mut Assets<Mesh>,
    material: &Handle<AtlasMaterial>,
    z: f32,
    core: bool,
    beacons: &[Beacon],
) {
    for b in beacons {
        let mut pen = Pen::new(atlas, true);
        pen.beacon_glow(b.size, b.color);
        // Neighbours out of step: the phase from the place.
        let phase = (b.at.x * 0.37 + b.at.y * 0.61).rem_euclid(1.0);
        commands.spawn((
            Blink { beacon: *b, core, phase },
            Mesh2d(meshes.add(pen.into_mesh())),
            MeshMaterial2d(material.clone()),
            Transform::from_translation(b.at.extend(z + Z_BEACON)),
            Visibility::Hidden,
        ));
    }
}

/// A beacon's glow size now: a navigation light breathes, a strobe flashes and fades.
fn blink(b: &Blink, t: f32) -> f32 {
    if b.beacon.strobe {
        let p = (t / 2.2 + b.phase).rem_euclid(1.0);
        let flash = (1.0 - p / 0.14).max(0.0);
        0.45 + 1.1 * flash * flash
    } else {
        0.9 + 0.12 * (t * 1.9 + b.phase * TAU).sin()
    }
}

/// Where a point `s` metres along a run is, and which way the run goes there.
fn along(run: &[Vec2], s: f32) -> (Vec2, Vec2) {
    let mut left = s;
    for (i, w) in run.windows(2).enumerate() {
        let len = w[0].distance(w[1]);
        let d = (w[1] - w[0]) / len.max(1e-6);
        if left <= len || i + 2 == run.len() {
            return (w[0] + d * left.min(len), d);
        }
        left -= len;
    }
    (run[0], Vec2::X)
}

/// A shuttle `t` seconds into its run, out and back with a wait at each end, easing in and out: its place and
/// heading.
fn shuttle_at(run: &[Vec2], t: f32) -> (Vec2, f32) {
    let len: f32 = run.windows(2).map(|w| w[0].distance(w[1])).sum();
    let trip = len / SHUTTLE_SPEED;
    let cycle = 2.0 * (trip + SHUTTLE_DWELL);
    let p = t.rem_euclid(cycle);
    let (u, back) = if p < trip {
        (p / trip, false)
    } else if p < trip + SHUTTLE_DWELL {
        (1.0, false)
    } else if p < 2.0 * trip + SHUTTLE_DWELL {
        (1.0 - (p - trip - SHUTTLE_DWELL) / trip, true)
    } else {
        (0.0, true)
    };
    let eased = u * u * (3.0 - 2.0 * u);
    let (at, d) = along(run, eased * len);
    (at, if back { (-d).to_angle() } else { d.to_angle() })
}

/// Moves, turns and scales the parts and the beacons.
pub fn animate(
    time: Res<Time<Real>>,
    yard: Res<super::Yard>,
    mut parts: Query<(&Part, &mut Transform), Without<Blink>>,
    mut beacons: Query<(&Blink, &mut Transform), Without<Part>>,
) {
    let t = time.elapsed_secs_wrapped();
    let (l, lights) = (&yard.layout, yard.lights);
    let runs = l.shuttle_runs();
    let set = |tf: &mut Transform, at: Vec2, angle: f32, scale: f32| {
        let want = Transform { translation: at.extend(tf.translation.z), rotation: Quat::from_rotation_z(angle), scale: Vec3::new(scale, scale, 1.0) };
        if *tf != want {
            *tf = want;
        }
    };
    for (part, mut tf) in &mut parts {
        match *part {
            Part::GateRing => set(&mut tf, l.gate, if lights.gate { t / GATE_TURN * TAU } else { 0.0 }, 1.0),
            Part::GatePulse(k) => {
                let (from, to) = (l.chevrons.first().copied().unwrap_or(l.gate), l.gate);
                let u = (t / PULSE_RUN + k as f32 / PULSES as f32).rem_euclid(1.0);
                let size = if lights.gate { (u * PI).sin() } else { 0.0 };
                set(&mut tf, from.lerp(to, u), 0.0, size);
            }
            Part::MindRotor => set(&mut tf, l.mind.centre, t / MIND_TURN * TAU, 1.0),
            Part::MindCore => set(&mut tf, l.mind.centre, 0.0, 1.0 + 0.06 * (t * 2.2).sin()),
            Part::Dish(k) => {
                let (at, (_, rest)) = draw::research_dishes(l)[k];
                let sweep = if lights.research { 0.35 * (t * TAU / (9.0 + 4.0 * k as f32) + k as f32).sin() } else { 0.0 };
                set(&mut tf, at, rest + sweep, 1.0);
            }
            Part::Orbit => set(&mut tf, l.research.centre, if lights.research { t / ORBIT_TURN * TAU } else { 0.0 }, 1.0),
            Part::Shuttle(k) => {
                // The two runs out of step.
                let (at, angle) = shuttle_at(&runs[k], t + 7.0 * k as f32);
                set(&mut tf, at, angle, 1.0);
            }
        }
    }
    for (b, mut tf) in &mut beacons {
        set(&mut tf, b.beacon.at, 0.0, blink(b, t));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shuttle starts at the far end, reaches the other end after its trip, waits there, and comes back.
    #[test]
    fn a_shuttle_goes_out_and_back() {
        let run = [Vec2::ZERO, Vec2::new(12.0, 0.0), Vec2::new(12.0, 12.0)];
        let trip = 24.0 / SHUTTLE_SPEED;
        assert_eq!(shuttle_at(&run, 0.0).0, Vec2::ZERO);
        assert!(shuttle_at(&run, trip).0.distance(Vec2::new(12.0, 12.0)) < 1e-3);
        assert!(shuttle_at(&run, trip + SHUTTLE_DWELL * 0.5).0.distance(Vec2::new(12.0, 12.0)) < 1e-3);
        // Halfway back it is at the corner, heading back the way it came.
        let (at, angle) = shuttle_at(&run, trip + SHUTTLE_DWELL + trip / 2.0);
        assert!(at.distance(Vec2::new(12.0, 0.0)) < 1e-2, "{at}");
        assert!((Vec2::from_angle(angle) - Vec2::NEG_X).length() < 1e-3 || (Vec2::from_angle(angle) - Vec2::NEG_Y).length() < 1e-3);
        assert!(shuttle_at(&run, 2.0 * (trip + SHUTTLE_DWELL)).0.distance(Vec2::ZERO) < 1e-3);
    }

    /// A strobe flashes once per period and rests dim; a navigation light only breathes.
    #[test]
    fn strobes_flash_and_nav_lights_breathe() {
        let beacon = |strobe| Blink { beacon: Beacon { at: Vec2::ZERO, color: Color::WHITE, size: 1.0, strobe }, core: false, phase: 0.0 };
        let s = beacon(true);
        assert!(blink(&s, 0.0) > 1.4 && blink(&s, 1.0) < 0.5);
        let n = beacon(false);
        assert!((0..100).all(|k| (0.77..=1.03).contains(&blink(&n, k as f32 * 0.1))));
    }
}
