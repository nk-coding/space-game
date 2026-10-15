//! What keeps the yard alive at home (Niklas, 15 Oct 2026: "the station should feel a bit more alive"): beacons
//! that breathe and flash, the gate's ring turning and a wave of light running up its approach while it is awake,
//! the mind's spokes turning around its beating core, research's dishes sweeping and its orbit turning while
//! research runs, two shuttles roaming the station's left end, windows going on and off, a few in a minute, the
//! LOG's mast glowing while a voyage's result waits there (GDR-0091), and the EVOLUTION lab (GDR-0092): its racks'
//! windows flickering while a run computes, its ring of segments filling with the run's generations, its gold glow
//! once a run has finished.
//!
//! Each is a small mesh of its own, built with the yard (`super::rebuild`) and then only moved, turned or scaled by
//! its transform, so nothing is rebuilt while it moves. The beacons' white cores stay in the static meshes; only
//! their glows blink. The approach's lit chevrons and each building's or wall's window lights are drawn again only
//! while one of them fades ([`relight`]): the wave for about a second in five, a building's windows for a few
//! seconds about once a minute (the lab's busy racks a few times a second). The lab's ring is drawn again when a
//! segment lights ([`evolution_ring`]).

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;
use kit::atlas::{Atlas, AtlasMaterial};
use kit::meshbuf::MeshBuf;
use kit::params::RenderToggles;
use kit::rng::Rng;

use super::draw::{self, Beacon, EVO_SEGMENTS, Evo, Lights, Palette, Pen, Windows};
use super::layout::Layout;

/// Depths: the gate's ring and approach lights, the mind's and research's moving parts just over the yard (under the
/// floor and the walls); the shuttles over everything but the drones.
const Z_PARTS: f32 = 1.3;
const Z_SHUTTLES: f32 = 13.0;
/// A beacon's glow and a window's light just over the layer they belong to.
const Z_BEACON: f32 = 0.4;
const Z_WINDOWS: f32 = 0.2;

/// Shuttles (Niklas, 15 Oct 2026: they "should roam more freely, and they should not teleport"): top speed (m/s),
/// thrust and the braking they plan their arrival with (m/s²), how fast they turn (rad/s), how close a berth is
/// reached and a passing point passed (m), and how often (a share) and how long (s) they stop at a berth.
const SHUTTLE_SPEED: f32 = 2.6;
const SHUTTLE_THRUST: f32 = 1.2;
const SHUTTLE_BRAKE: f32 = 0.7;
const SHUTTLE_TURN: f32 = 1.6;
const SHUTTLE_ARRIVE: f32 = 0.25;
const SHUTTLE_PASS: f32 = 1.5;
const SHUTTLE_STOPS: f32 = 0.6;
const SHUTTLE_WAIT: (f32, f32) = (2.0, 6.0);
/// One turn of the gate's ring and of the mind's spokes, and of research's orbit (s).
const GATE_TURN: f32 = 120.0;
const MIND_TURN: f32 = 40.0;
const ORBIT_TURN: f32 = 30.0;
/// The LOG's glow while a result waits: one breath (s), and its size between breaths and at the full breath
/// (noticeable but calm).
const LOG_BREATH: f32 = 2.8;
const LOG_GLOW: (f32, f32) = (0.7, 1.15);
/// The approach's wave (Niklas, 15 Oct 2026: "highlighting the arrows after each other ... only every 5s or so"):
/// one every 5 s; each chevron lights this long after the one before it and rises and falls in this long, so a
/// wave takes about a second.
const WAVE_EVERY: f32 = 5.0;
const WAVE_STEP: f32 = 0.11;
const WAVE_RISE: f32 = 0.45;
/// Windows (Niklas, 15 Oct 2026: "very slowly change which are active ... not too many/too little per
/// component"): the share of a building's or wall's windows that is on while it is lit and while it is dark, how
/// long it waits between two switches (s, drawn evenly) and how long a window takes to light or go dark (s).
const LIT_SHARE: (f32, f32) = (0.35, 0.75);
const DARK_SHARE: (f32, f32) = (0.0, 0.12);
const SWITCH_EVERY: (f32, f32) = (20.0, 60.0);
const WINDOW_FADE: f32 = 4.0;
/// The EVOLUTION lab's racks while a run computes (GDR-0092: "the window bands flicker as fights run"): a switch
/// every so often, and a quick fade.
const BUSY_EVERY: (f32, f32) = (0.08, 0.35);
const BUSY_FADE: f32 = 0.2;
/// The lab's glow once a run has finished: one breath (s), slower than the LOG's.
const EVO_BREATH: f32 = 3.2;

/// A moving part of the yard.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    GateRing,
    /// The approach's chevrons, lit by its wave ([`relight`]).
    Approach,
    MindRotor,
    MindCore,
    Dish(usize),
    Orbit,
    /// The LOG's glow (none while nothing waits there).
    LogGlow,
    /// The EVOLUTION lab's ring of segments ([`evolution_ring`] draws it) and its glow once a run has finished.
    EvoRing,
    EvoGlow,
    Shuttle(usize),
}

impl Part {
    const ALL: [Part; 12] = [
        Part::GateRing,
        Part::Approach,
        Part::MindRotor,
        Part::MindCore,
        Part::Dish(0),
        Part::Dish(1),
        Part::Orbit,
        Part::LogGlow,
        Part::EvoRing,
        Part::EvoGlow,
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
            // Dark between waves; the wave draws it as it runs.
            Part::Approach => {}
            Part::MindRotor => draw::mind_rotor(pen, pal),
            Part::MindCore => draw::mind_core(pen, pal),
            Part::Dish(k) => draw::dish(pen, pal, draw::research_dishes(l)[k].1.0, lab),
            Part::Orbit => draw::orbit(pen, lab),
            Part::LogGlow if lights.log => draw::log_glow(pen, pal),
            Part::LogGlow => {}
            // Drawn with the run's progress.
            Part::EvoRing => {}
            Part::EvoGlow if lights.evolution == Evo::Done => draw::evolution_glow(pen, pal),
            Part::EvoGlow => {}
            Part::Shuttle(_) => draw::shuttle(pen, pal),
        }
    }
}

/// A shuttle's flight, from its first frame at home on.
#[derive(Component, Default)]
pub struct Pilot(Option<Flight>);

/// Where a shuttle is, how fast it goes and which way it faces, the stop it flies to and, at a berth, how much
/// longer it waits there.
#[derive(Clone, Copy, Debug)]
struct Flight {
    at: Vec2,
    v: Vec2,
    heading: f32,
    to: usize,
    wait: Option<f32>,
}

impl Flight {
    /// Waiting a little at a stop `rng` picks.
    fn new(stops: &[(Vec2, bool)], rng: &mut Rng) -> Self {
        let to = pick(stops.len(), rng);
        Self { at: stops[to].0, v: Vec2::ZERO, heading: rng.range(-PI, PI), to, wait: Some(rng.range(0.0, SHUTTLE_WAIT.1)) }
    }

    /// The next stop: the nearest of three picks, so mostly short hops round the moorings and now and then a long
    /// one.
    fn next(&self, stops: &[(Vec2, bool)], rng: &mut Rng) -> usize {
        (0..3)
            .map(|_| pick(stops.len(), rng))
            .filter(|k| *k != self.to)
            .min_by(|a, b| self.at.distance_squared(stops[*a].0).total_cmp(&self.at.distance_squared(stops[*b].0)))
            .unwrap_or((self.to + 1) % stops.len())
    }

    /// `dt` s on: it turns towards its stop and thrusts along its nose (the sharper the turn, the less), brakes into a
    /// berth and maybe waits there, passes through an open point at speed, then goes on to the next.
    fn step(&mut self, dt: f32, stops: &[(Vec2, bool)], rng: &mut Rng) {
        let (target, berth) = stops[self.to];
        let dist = self.at.distance(target);
        match self.wait {
            Some(w) if w > dt => self.wait = Some(w - dt),
            Some(_) => (self.wait, self.to) = (None, self.next(stops, rng)),
            None if berth && dist < SHUTTLE_ARRIVE => {
                if rng.next_f32() < SHUTTLE_STOPS {
                    self.wait = Some(rng.range(SHUTTLE_WAIT.0, SHUTTLE_WAIT.1));
                } else {
                    self.to = self.next(stops, rng);
                }
            }
            None if !berth && dist < SHUTTLE_PASS => self.to = self.next(stops, rng),
            None => {}
        }
        let (target, berth) = stops[self.to];
        let to = target - self.at;
        let dist = to.length();
        let mut want = Vec2::ZERO;
        // Close in, it holds its nose (the way to the point would swing about) and drifts onto the point.
        if dist > SHUTTLE_ARRIVE {
            let dir = to / dist;
            let turn = (dir.to_angle() - self.heading + PI).rem_euclid(TAU) - PI;
            self.heading = (self.heading + turn.clamp(-SHUTTLE_TURN * dt, SHUTTLE_TURN * dt) + PI).rem_euclid(TAU) - PI;
            let facing = Vec2::from_angle(self.heading).dot(dir).max(0.0);
            let speed = if berth { (2.0 * SHUTTLE_BRAKE * dist).sqrt().min(SHUTTLE_SPEED) } else { SHUTTLE_SPEED };
            want = dir * speed * facing;
        } else if berth {
            want = to / SHUTTLE_ARRIVE * (2.0 * SHUTTLE_BRAKE * SHUTTLE_ARRIVE).sqrt();
        }
        self.v += (want - self.v).clamp_length_max(SHUTTLE_THRUST * dt);
        self.at += self.v * dt;
    }
}

/// A random index below `n`.
fn pick(n: usize, rng: &mut Rng) -> usize {
    ((rng.next_f32() * n as f32) as usize).min(n - 1)
}

/// One building's or wall's window lights (`core`: on the U, else around it): which are on, how bright each is now,
/// and how long until the next switch.
#[derive(Component)]
pub struct Lit {
    windows: Windows,
    pub core: bool,
    on: Vec<bool>,
    glow: Vec<f32>,
    next: f32,
    drawn: bool,
}

impl Lit {
    fn new(windows: Windows, core: bool) -> Self {
        Self { windows, core, on: Vec::new(), glow: Vec::new(), next: 0.0, drawn: false }
    }

    /// How many may be on: the band the building's state allows, at least one apart.
    fn band(&self) -> (usize, usize) {
        let n = self.windows.panes.len() as f32;
        let (lo, hi) = if self.windows.lit { LIT_SHARE } else { DARK_SHARE };
        let lo = (lo * n).ceil() as usize;
        (lo, ((hi * n).floor() as usize).max(lo + 1).min(self.windows.panes.len()))
    }

    /// A random start inside the band, and a random wait until the first switch (so buildings don't switch together).
    fn start(&mut self, rng: &mut Rng) {
        let (lo, hi) = self.band();
        self.on = vec![false; self.windows.panes.len()];
        for _ in 0..lo + pick(hi - lo + 1, rng) {
            if let Some(k) = self.any(false, rng) {
                self.on[k] = true;
            }
        }
        self.glow = self.on.iter().map(|on| if *on { 1.0 } else { 0.0 }).collect();
        self.next = rng.range(0.0, self.every().1);
    }

    /// The wait between two switches (s), slow or fast while busy, and how long a window fades.
    fn every(&self) -> (f32, f32) {
        if self.windows.busy { BUSY_EVERY } else { SWITCH_EVERY }
    }

    fn fade(&self) -> f32 {
        if self.windows.busy { BUSY_FADE } else { WINDOW_FADE }
    }

    /// A random window that is on (or off).
    fn any(&self, on: bool, rng: &mut Rng) -> Option<usize> {
        let n = self.on.iter().filter(|o| **o == on).count();
        (n > 0).then(|| self.on.iter().enumerate().filter(|(_, o)| **o == on).nth(pick(n, rng)).map_or(0, |(k, _)| k))
    }

    /// One switch: a window goes on, one goes off, or (where either would leave the band) one goes off as another
    /// comes on.
    fn switch(&mut self, rng: &mut Rng) {
        let lit = self.on.iter().filter(|o| **o).count();
        let (lo, hi) = self.band();
        let (off, on) = match pick(3, rng) {
            0 if lit < hi => (None, self.any(false, rng)),
            1 if lit > lo => (self.any(true, rng), None),
            _ => (self.any(true, rng), self.any(false, rng)),
        };
        for (k, to) in [(off, false), (on, true)] {
            if let Some(k) = k {
                self.on[k] = to;
            }
        }
    }

    /// `dt` s on: maybe a switch, and the fades; whether any light changed.
    fn step(&mut self, dt: f32, rng: &mut Rng) -> bool {
        self.next -= dt;
        if self.next <= 0.0 {
            self.switch(rng);
            let (lo, hi) = self.every();
            self.next = rng.range(lo, hi);
        }
        let mut changed = false;
        let fade = self.fade();
        for (g, on) in self.glow.iter_mut().zip(&self.on) {
            let want = if *on { 1.0 } else { 0.0 };
            if *g != want {
                *g = if *on { (*g + dt / fade).min(1.0) } else { (*g - dt / fade).max(0.0) };
                changed = true;
            }
        }
        changed
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
        let mut e = commands.spawn((part, Mesh2d(meshes.add(MeshBuf::placeholder())), MeshMaterial2d(outer.clone()), Transform::from_xyz(0.0, 0.0, part.z()), Visibility::Hidden));
        if let Part::Shuttle(_) = part {
            e.insert(Pilot::default());
        }
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

/// The windows' lights again, as the static layers collected them (`groups`: their windows, whether they are the
/// U's, and their layer's depth). A building or wall whose windows didn't change keeps its lights as they are.
pub fn respawn_windows(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: (&Handle<AtlasMaterial>, &Handle<AtlasMaterial>),
    groups: Vec<(Windows, bool, f32)>,
    old: &Query<(Entity, &Lit)>,
) {
    let mut kept = Vec::new();
    for (windows, core, z) in groups {
        if let Some((e, _)) = old.iter().find(|(e, lit)| lit.windows == windows && lit.core == core && !kept.contains(e)) {
            kept.push(e);
            continue;
        }
        commands.spawn((
            Lit::new(windows, core),
            Mesh2d(meshes.add(MeshBuf::placeholder())),
            MeshMaterial2d(if core { materials.0.clone() } else { materials.1.clone() }),
            Transform::from_xyz(0.0, 0.0, z + Z_WINDOWS),
            Visibility::Hidden,
        ));
    }
    for (e, _) in old {
        if !kept.contains(&e) {
            commands.entity(e).despawn();
        }
    }
}

/// How bright chevron `k` (from the mouth) is `t` s into the approach's wave: dark but for a soft rise and fall,
/// each a step after the one before.
fn wave(k: usize, t: f32) -> f32 {
    let p = (t.rem_euclid(WAVE_EVERY) - k as f32 * WAVE_STEP) / WAVE_RISE;
    if (0.0..1.0).contains(&p) { (p * PI).sin().powi(2) } else { 0.0 }
}

/// The LOG's glow's size `t` s on: a slow, soft breath.
fn breath(t: f32) -> f32 {
    let b = 0.5 - 0.5 * (t * TAU / LOG_BREATH).cos();
    LOG_GLOW.0 + (LOG_GLOW.1 - LOG_GLOW.0) * b
}

/// Moves, turns and scales the parts and the beacons; flies the shuttles.
pub fn animate(
    time: Res<Time<Real>>,
    yard: Res<super::Yard>,
    mut rng: Local<Option<Rng>>,
    mut parts: Query<(&Part, &mut Transform, Option<&mut Pilot>), Without<Blink>>,
    mut beacons: Query<(&Blink, &mut Transform), Without<Part>>,
) {
    let t = time.elapsed_secs_wrapped();
    // A long frame (a hidden tab) must not throw a shuttle across the yard.
    let dt = time.delta_secs().min(0.05);
    let rng = rng.get_or_insert_with(|| Rng::new(time.elapsed().as_nanos() as u32));
    let (l, lights) = (&yard.layout, yard.lights);
    let stops = l.shuttle_stops();
    let set = |tf: &mut Transform, at: Vec2, angle: f32, scale: f32| {
        let want = Transform { translation: at.extend(tf.translation.z), rotation: Quat::from_rotation_z(angle), scale: Vec3::new(scale, scale, 1.0) };
        if *tf != want {
            *tf = want;
        }
    };
    for (part, mut tf, pilot) in &mut parts {
        match *part {
            Part::GateRing => set(&mut tf, l.gate, if lights.gate { t / GATE_TURN * TAU } else { 0.0 }, 1.0),
            Part::Approach => set(&mut tf, Vec2::ZERO, 0.0, 1.0),
            Part::MindRotor => set(&mut tf, l.mind.centre, t / MIND_TURN * TAU, 1.0),
            Part::MindCore => set(&mut tf, l.mind.centre, 0.0, 1.0 + 0.06 * (t * 2.2).sin()),
            Part::Dish(k) => {
                let (at, (_, rest)) = draw::research_dishes(l)[k];
                let sweep = if lights.research { 0.35 * (t * TAU / (9.0 + 4.0 * k as f32) + k as f32).sin() } else { 0.0 };
                set(&mut tf, at, rest + sweep, 1.0);
            }
            Part::Orbit => set(&mut tf, l.research.centre, if lights.research { t / ORBIT_TURN * TAU } else { 0.0 }, 1.0),
            Part::LogGlow => set(&mut tf, l.log_beacon, 0.0, breath(t)),
            Part::EvoRing => set(&mut tf, l.evolution.centre, 0.0, 1.0),
            Part::EvoGlow => set(&mut tf, l.evolution.centre, 0.0, breath(t * LOG_BREATH / EVO_BREATH)),
            Part::Shuttle(_) => {
                if let Some(mut pilot) = pilot {
                    let f = pilot.0.get_or_insert_with(|| Flight::new(&stops, rng));
                    f.step(dt, &stops, rng);
                    set(&mut tf, f.at, f.heading, 1.0);
                }
            }
        }
    }
    for (b, mut tf) in &mut beacons {
        set(&mut tf, b.beacon.at, 0.0, blink(b, t));
    }
}

/// Runs the approach's wave and switches and fades the windows, drawing a mesh again only while something in it
/// changes.
#[allow(clippy::too_many_arguments)]
pub fn relight(
    time: Res<Time<Real>>,
    yard: Res<super::Yard>,
    station: Res<super::Station>,
    atlas: Res<Atlas>,
    toggles: Res<RenderToggles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut rng: Local<Option<Rng>>,
    mut waving: Local<bool>,
    parts: Query<(&Part, &Mesh2d)>,
    mut windows: Query<(&mut Lit, &Mesh2d)>,
) {
    let (t, dt) = (time.elapsed_secs_wrapped(), time.delta_secs().min(0.05));
    let rng = rng.get_or_insert_with(|| Rng::new(time.elapsed().as_nanos() as u32 ^ 0x9e37_79b9));
    let (l, pal, glow) = (&yard.layout, &station.palette, toggles.glow_sprites);
    let lit = |k: usize| if yard.lights.gate { wave(k, t) } else { 0.0 };
    let now = (0..l.chevrons.len()).any(|k| lit(k) > 0.0);
    // Every frame of a wave, and once more to clear it.
    if now || *waving {
        for (part, mesh) in &parts {
            if *part == Part::Approach {
                let mut pen = Pen::new(&atlas, glow);
                draw::approach_lights(&mut pen, pal, &l.chevrons, (0..l.chevrons.len()).map(lit));
                let _ = meshes.insert(&mesh.0, pen.into_mesh());
            }
        }
    }
    *waving = now;
    for (mut w, mesh) in &mut windows {
        if w.on.is_empty() {
            w.start(rng);
        }
        if w.step(dt, rng) || !w.drawn {
            w.drawn = true;
            let mut pen = Pen::new(&atlas, glow);
            // A soft start and end to every fade.
            draw::window_lights(&mut pen, &w.windows, w.glow.iter().map(|g| g * g * (3.0 - 2.0 * g)));
            let _ = meshes.insert(&mesh.0, pen.into_mesh());
        }
    }
}

/// The lab's ring of segments: lit as far as the run that computes (else the newest run) has come; drawn again only
/// when a segment lights or goes, or when the yard was built again.
#[allow(clippy::too_many_arguments)]
pub fn evolution_ring(
    yard: Res<super::Yard>,
    station: Res<super::Station>,
    atlas: Res<Atlas>,
    toggles: Res<RenderToggles>,
    lab: Res<world::evolution::Lab>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut drawn: Local<Option<(usize, bool)>>,
    parts: Query<(&Part, &Mesh2d)>,
) {
    let on = yard.lights.evolution != Evo::Dark && !lab.runs().is_empty();
    let lit = (ring_share(&lab) * EVO_SEGMENTS as f32).floor() as usize;
    if *drawn == Some((lit, on)) && !yard.is_changed() {
        return;
    }
    *drawn = Some((lit, on));
    for (part, mesh) in &parts {
        if *part == Part::EvoRing {
            let mut pen = Pen::new(&atlas, toggles.glow_sprites);
            draw::evolution_ring(&mut pen, &station.palette, lit, on);
            let _ = meshes.insert(&mesh.0, pen.into_mesh());
        }
    }
}

/// How far the run the ring shows has come, 0..1: the one that computes (its open generation counted in), else the
/// newest.
fn ring_share(lab: &world::evolution::Lab) -> f32 {
    if let Some(l) = lab.running() {
        let state = l.run.state();
        return ((state.generation as f32 + lab.generation_done(l)) / state.generations.max(1) as f32).min(1.0);
    }
    lab.runs().first().map_or(0.0, |e| e.generation as f32 / e.generations.max(1) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shuttle roams: it never jumps or snaps round, keeps to its top speed, stays in view, reaches many stops and
    /// waits at some berths.
    #[test]
    fn a_shuttle_roams_smoothly() {
        let l = Layout::new((24, 24));
        let stops = l.shuttle_stops();
        let mut rng = Rng::new(3);
        let mut f = Flight::new(&stops, &mut rng);
        let dt = 1.0 / 60.0;
        let (mut seen, mut still) = (vec![f.to], 0.0f32);
        let mut longest_still = 0.0f32;
        for _ in 0..(600.0 / dt) as usize {
            let (at, heading) = (f.at, f.heading);
            f.step(dt, &stops, &mut rng);
            assert!(f.at.distance(at) <= (SHUTTLE_SPEED + 0.05) * dt, "jumped from {at} to {}", f.at);
            let turned = (f.heading - heading + PI).rem_euclid(TAU) - PI;
            assert!(turned.abs() <= SHUTTLE_TURN * dt + 1e-4, "snapped round by {turned}");
            assert!(l.overview.contains(f.at), "left the view at {}", f.at);
            if !seen.contains(&f.to) {
                seen.push(f.to);
            }
            still = if f.v.length() < 0.05 { still + dt } else { 0.0 };
            longest_still = longest_still.max(still);
        }
        assert!(seen.len() >= 8, "only {} stops in ten minutes", seen.len());
        assert!(longest_still > SHUTTLE_WAIT.0 * 0.8, "never waited at a berth");
    }

    /// The wave lights the chevrons one after another, each softly, then leaves them dark for most of its period.
    #[test]
    fn the_approach_wave_runs_up_the_chevrons_then_rests() {
        let peak = |k: usize| k as f32 * WAVE_STEP + WAVE_RISE / 2.0;
        for k in 0..7 {
            assert!(wave(k, peak(k)) > 0.99);
            assert!(wave(k, peak(k) + WAVE_EVERY) > 0.99, "it repeats");
            assert_eq!(wave(k, k as f32 * WAVE_STEP - 0.01), 0.0, "dark until its turn");
            assert!(wave(k, k as f32 * WAVE_STEP + 0.05) < 0.2, "a soft rise");
            if k > 0 {
                assert!(wave(k - 1, peak(k)) < wave(k, peak(k)), "the one before is already fading");
            }
        }
        let dark = (0..500).filter(|i| (0..7).all(|k| wave(k, *i as f32 * WAVE_EVERY / 500.0) == 0.0)).count();
        assert!(dark > 350, "dark only {dark} of 500");
    }

    /// A building's windows stay inside their band, switch about once in 20 to 60 s, and fade rather than pop.
    #[test]
    fn windows_switch_slowly_inside_their_band() {
        let mut rng = Rng::new(9);
        for (n, lit) in [(34, true), (12, true), (12, false), (29, false), (3, true)] {
            let panes = (0..n).map(|k| Rect::from_center_size(Vec2::new(k as f32, 0.0), Vec2::splat(0.2))).collect();
            let mut w = Lit::new(Windows { panes, color: Color::WHITE, lit, busy: false }, false);
            w.start(&mut rng);
            let (lo, hi) = w.band();
            let dt = 0.1;
            let (mut switches, mut before) = (0, w.on.clone());
            for _ in 0..(3600.0 / dt) as usize {
                let glow = w.glow.clone();
                w.step(dt, &mut rng);
                let on = w.on.iter().filter(|o| **o).count();
                assert!((lo..=hi).contains(&on), "{on} of {n} on, band {lo}..={hi}");
                assert!(w.glow.iter().zip(&glow).all(|(a, b)| (a - b).abs() <= dt / WINDOW_FADE + 1e-6), "a window popped");
                if w.on != before {
                    switches += 1;
                    before = w.on.clone();
                }
            }
            assert!((50..=190).contains(&switches), "{switches} switches in an hour ({n} windows)");
        }
    }

    /// The LOG's glow breathes slowly between its two sizes, never gone.
    #[test]
    fn the_log_glow_breathes() {
        let sizes: Vec<f32> = (0..280).map(|k| breath(k as f32 * 0.01)).collect();
        assert!(sizes.iter().all(|s| (LOG_GLOW.0 - 1e-4..=LOG_GLOW.1 + 1e-4).contains(s)));
        assert!((breath(0.0) - LOG_GLOW.0).abs() < 1e-4 && (breath(LOG_BREATH / 2.0) - LOG_GLOW.1).abs() < 1e-4);
        assert!(sizes.windows(2).all(|w| (w[1] - w[0]).abs() < 0.01), "it breathes, it doesn't blink");
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
