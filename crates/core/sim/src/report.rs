//! The static design report (Drydock v1): every number and per-part flag the drydock shows, computed from a
//! blueprint with the sim's own rules, without a world, physics or stepping. See `docs/sim.md`, "Design report
//! (drydock)".
//!
//! One source of rules: the report builds the ship exactly as [`crate::Sim::spawn_ship`] does (`Ship::new`:
//! exposure, stats after bonuses, pieces, mass, colliders, consumer rows, thruster and RCS tables, gun mounts),
//! then runs the sim's own passes on the main piece: the control pass (which parts idle, GDR-0044), the
//! authority ([`crate::ship::refresh_authority`], [`crate::flight::Kin`]) and one power pass at full use
//! (`ship::power_pass`, the code `Ship::update_systems` runs every tick). Only the steady-state heat solve and
//! the 90° turn estimate are the report's own; both work on the sim's constants and functions.
//!
//! # Assumptions
//!
//! * **Capabilities at full power**: thrust, torque, top speeds, turning and firepower are what the main piece's
//!   actuators and clear guns make at full power with bonuses, idle parts left out, as if the piece runs (a
//!   working, powered core). Whether the reactors can feed everything at once is the power section's answer.
//! * **Full use** (power and heat): every consumer at full activity: engines and RCS pods at full thrust, gyros
//!   at full torque, guns with a clear line of fire firing, shields on, logic cores and other consumers on.
//!   Idle parts and blocked guns draw nothing. This is the worst case; real flight rarely uses all at once.
//!   Draws that come in bursts (content `power.intermittent`: a scanner while a scan charges) are left out and
//!   summed apart ([`PowerReport::intermittent`]), so a starter that scans now and then does not read as short.
//! * **Heat** is the steady state of the dynamic model at full use, with each part's work from that power
//!   pass. The shutdown above a limit is not applied (the parts are flagged instead), and plume heat on own
//!   parts is left out (the burn is flagged instead).
//! * Only the **main piece** is analysed (mass, power, heat, flight, control); other pieces fall off when the
//!   ship spawns. Their parts report no heat and no power.
//!
//! # Cost
//!
//! One `ShipDesign::compile` and one `Ship::new` (what a spawn costs), one control and one power pass, a
//! conjugate-gradient heat solve over the links and a short 1D turn integration. It allocates freely: it runs
//! once per drydock edit, never per tick. Measured (native release): Halvex Runner about 25 µs, a 2000-part
//! slab about 1.5 ms (`sim_perf --report`).

use std::sync::Arc;

use data::{Blueprint, Content, Delivery, Dir, Effect, PowerTier, WeaponFamily};

use crate::DT;
use crate::config::SimConfig;
use crate::ship::design::{ConsumerKind, NO_SLOT, PartIdx, PartStats, ShipDesign, Stat};
use crate::flight::{Kin, TURNING, TurnAssist, turn_rate};
use crate::ids::{IdAlloc, ShipId};
use crate::ship::{
    Body, HeatK, PartState, Ship, applied_bonuses, gun_blockers, power_pass, producing,
    reactor_supply, refresh_authority, tuning as heat_tuning,
};
use crate::ship::vessel::Owner;

/// Report tuning. **PLACEHOLDERS**, like every number in the sim.
pub mod tuning {
    /// The 90° turn counts as done once the heading is within this of the target (rad; 1°, as the agility
    /// test in `tests/speed.rs` measures it).
    pub const TURN_SETTLE: f32 = 1.0 * core::f32::consts::PI / 180.0;
    /// The turn estimate gives up after this many ticks (30 s).
    pub const TURN_MAX_TICKS: u32 = 900;
    /// Heat solve: stop once the residual is below this share of the generation (relative, f64).
    pub const HEAT_TOLERANCE: f64 = 1e-7;
    /// Heat solve: at most this many conjugate-gradient iterations.
    pub const HEAT_MAX_ITERATIONS: usize = 2000;

    /// Device load model: estimated native cost of one ship for one tick in µs, `COST_SHIP` + Σ driver × its
    /// cost, per step phase: systems (power, heat over parts and links) per part, flight computer and allocation
    /// per thruster row and torque source, weapons per gun, Rapier per body and collider shape. Fitted 12 Oct
    /// 2026 with `sim_perf --calibrate` (the dev VM, release build; see `docs/sim.md`, "Design report
    /// (drydock)"). PLACEHOLDERS: one machine, ships flying apart with every gun firing.
    pub const COST_SHIP: f32 = 0.09;
    pub const COST_PART: f32 = 0.0043;
    pub const COST_ROW: f32 = 0.0045;
    pub const COST_TORQUE_SOURCE: f32 = 0.0045;
    pub const COST_GUN: f32 = 0.05;
    pub const COST_PIECE: f32 = 0.27;
    pub const COST_COLLIDER: f32 = 0.018;
    /// Device load classes: up to this many µs per tick is `Light`, `Moderate`, `Heavy`; above it `TooHeavy`.
    /// PLACEHOLDERS (a Halvex Runner is about 1.2 µs, a 2000-part slab about 10 µs).
    pub const LIGHT_MAX: f32 = 2.5;
    pub const MODERATE_MAX: f32 = 10.0;
    pub const HEAVY_MAX: f32 = 40.0;
}

/// Everything the drydock shows about a blueprint. Blueprint frame (x right, y down, metres) for positions;
/// part indices are blueprint placement indices.
#[derive(Clone, Debug, PartialEq)]
pub struct DesignReport {
    /// The connected pieces (one rigid body each), in the sim's order (row-major by their first cell, as
    /// `data::ShipGrid::pieces`).
    pub pieces: Vec<PieceReport>,
    /// Index into `pieces` of the main piece (the sim's rule: a piece with a logic core first, then the most
    /// parts, then the earlier one). `None` for an empty blueprint.
    pub main: Option<usize>,
    /// Parts that are not on the main piece: they fall off when the ship spawns.
    pub falls_off: Vec<PartIdx>,
    /// Per part, in blueprint order.
    pub parts: Vec<PartReport>,
    /// The adjacency bonuses that apply (sources alive, venting where needed, within `max_stacks`), by target
    /// part, each target's in the order they apply. For the bonus overlay.
    pub bonuses: Vec<BonusReport>,
    /// Main piece: mass (t), centre of mass (blueprint frame), moment of inertia about it (t·m²).
    pub mass: f32,
    pub centre_of_mass: [f32; 2],
    pub inertia: f32,
    pub power: PowerReport,
    pub heat: HeatReport,
    pub flight: FlightReport,
    /// Every gun of the ship (all pieces), in gun slot order.
    pub guns: Vec<GunReport>,
    /// Main piece: damage per second of its working guns (clear, not idle) at full power
    /// ([`crate::Piece::firepower`]).
    pub firepower: f32,
    /// Every engine of the ship (all pieces), in engine slot order, with its plume lanes.
    pub engines: Vec<EngineReport>,
    /// Every shield emitter of the ship (all pieces).
    pub shields: Vec<ShieldReport>,
    /// Every part that explodes when damage destroys it (all pieces), in part order: what its blast would do to
    /// the design as built.
    pub blasts: Vec<BlastReport>,
    /// Main piece: cargo capacity (content units).
    pub cargo: f32,
    /// Main piece: behavior rule slots of its logic cores.
    pub rule_slots: u32,
    pub control: ControlReport,
    /// Main piece: hit points (sum over its parts).
    pub hp: f32,
    /// Whole blueprint: build points (each part's `build_points`, times its material's `build_point_factor` for
    /// hull parts).
    pub build_points: f32,
    pub device: DeviceLoad,
    /// What keeps the design from flying (blockers) or makes it worse (warnings): blockers first.
    pub issues: Vec<Issue>,
}

/// One connected piece.
#[derive(Clone, Debug, PartialEq)]
pub struct PieceReport {
    /// Its parts, ascending.
    pub parts: Vec<PartIdx>,
    /// Has a logic core: a vessel of its own when it is not the main piece (GDR-0013), salvage otherwise.
    pub vessel: bool,
    /// Its reactors make power (a vessel without it is dark).
    pub powered: bool,
}

/// Per part.
#[derive(Clone, Debug, PartialEq)]
pub struct PartReport {
    /// Index into `DesignReport::pieces`.
    pub piece: usize,
    /// Stats after adjacency bonuses.
    pub stats: PartStats,
    /// Sides open to space (straight sides not linked to a neighbour, plus one per slope-face cell): venting.
    pub open_sides: u16,
    /// Steady-state heat at full use (main piece; 0 elsewhere).
    pub heat: f32,
    /// `heat / limit` (after bonuses); 0 for parts without a limit.
    pub heat_ratio: f32,
    /// Power level at full use (`PartState::power`: 1 = full, between = partial output, 0 = off); reactors and
    /// passive parts 1; 0 off the main piece.
    pub power: f32,
    /// Idle: over the control capacity (GDR-0044).
    pub idle: bool,
}

/// One adjacency bonus that applies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BonusReport {
    pub target: PartIdx,
    pub source: PartIdx,
    pub stat: Stat,
    pub effect: Effect,
}

/// Main piece power at full use.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PowerReport {
    /// Output of its reactors after bonuses (idle reactors left out).
    pub supply: f32,
    /// What every consumer asks for at full use (`Off` consumers left out).
    pub demand: f32,
    /// What they get: `min(supply, demand)` handed out by tier and weight.
    pub served: f32,
    /// `demand − served`.
    pub deficit: f32,
    /// What the consumers whose draw comes in bursts ask for at full activity, summed per their words (content
    /// `power.intermittent`, "while scanning"), in part order; `Off` left out. Not part of `demand`: the drydock
    /// shows it beside it.
    pub intermittent: Vec<(String, f32)>,
    /// The consumers in allocation order (tier, weight descending, part), with what they get.
    pub consumers: Vec<ConsumerReport>,
}

/// One consumer row at full use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConsumerReport {
    pub part: PartIdx,
    pub kind: ConsumerKind,
    pub tier: PowerTier,
    pub weight: f32,
    pub min_level: f32,
    /// Draw at full activity, after bonuses.
    pub draw: f32,
    /// What it asks for (0 if idle, a blocked gun or set `Off`: the rows of `demand`).
    pub want: f32,
    /// The level the allocation gives its row (0..1; a row that asks for nothing gets its tier's level, so an
    /// idle part shows it here while its `PartReport::power` is 0).
    pub level: f32,
}

/// Main piece heat at full use.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeatReport {
    /// The highest `heat / limit` of its parts (1 = at the limit).
    pub peak: f32,
    /// The part with that peak.
    pub peak_part: Option<PartIdx>,
    /// Parts above their limit: they would shut down.
    pub over: Vec<PartIdx>,
}

/// Main piece flight capability at full power (idle parts left out). Directions are the thrust groups
/// forward (the nose, +y), backward, right (+x), left.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlightReport {
    /// Thrust per direction (kN).
    pub thrust: [f32; 4],
    /// Acceleration per direction (m/s²).
    pub accel: [f32; 4],
    /// Top speed per direction (m/s, GDR-0041): the highest rated speed among the thrusters pushing that way.
    pub top_speed: [f32; 4],
    /// Direction with the most thrust (forward wins ties): the axis "fly that way" uses.
    pub best: usize,
    /// Pure torque of gyros and logic cores (kN·m, either sense).
    pub torque_pure: f32,
    /// Differential thrust torque, counter-clockwise and clockwise (kN·m): the last resort (GDR-0055), beyond
    /// what the RCS couples take.
    pub torque_ccw: f32,
    pub torque_cw: f32,
    /// The angular acceleration the flight computer plans turns with before the last resort joins in (rad/s²,
    /// `Kin::alpha` of a fresh ship): pure torque and RCS couples.
    pub alpha: f32,
    /// Estimated time for a 90° turn from rest, settled within 1° (s); `None` if it cannot turn (or takes
    /// more than 30 s).
    pub turn_90: Option<f32>,
    /// Yaw balance: the torque full forward thrust makes about the centre of mass (kN·m, counter-clockwise
    /// positive; 0 = it flies straight).
    pub yaw: f32,
    /// What the flight computer can hold against it while flying straight (kN·m): pure torque, the torque of
    /// every other thruster row that turns against it (RCS couples, then differential thrust) and the forward
    /// engines' gimbals (those two are the last resort, after a few ticks; flying straight steers to a heading).
    pub yaw_hold: f32,
    /// `|yaw| ≤ yaw_hold`: full forward thrust does not turn the ship.
    pub yaw_balanced: bool,
}

/// One gun.
#[derive(Clone, Debug, PartialEq)]
pub struct GunReport {
    pub slot: u32,
    pub part: PartIdx,
    /// Index into `DesignReport::pieces`.
    pub piece: usize,
    /// Centre of the facing side (blueprint frame).
    pub muzzle: [f32; 2],
    /// The side it fires through.
    pub facing: Dir,
    /// Line of fire clear (no part of its own piece straight ahead).
    pub clear: bool,
    /// The parts of its piece in its line of fire.
    pub blockers: Vec<PartIdx>,
    /// Reach (m), after bonuses.
    pub range: f32,
    /// Damage per second at full power, after bonuses.
    pub dps: f32,
    /// Shots per second (0 for beams).
    pub rate: f32,
    pub family: WeaponFamily,
    pub delivery: Delivery,
}

/// One engine and its exhaust plume (GDR-0039).
#[derive(Clone, Debug, PartialEq)]
pub struct EngineReport {
    pub slot: u32,
    pub part: PartIdx,
    /// Index into `DesignReport::pieces`.
    pub piece: usize,
    /// The side it pushes towards (the exhaust leaves through the opposite side).
    pub facing: Dir,
    /// Full-throttle thrust (kN), after bonuses and the thrust scale.
    pub thrust: f32,
    /// One lane per cell of its back side.
    pub lanes: Vec<LaneReport>,
}

/// One plume lane: from `origin` along `dir` for `length`, burning the first part of its own piece in the way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaneReport {
    /// Centre of the cell's back face (blueprint frame).
    pub origin: [f32; 2],
    /// Exhaust direction.
    pub dir: Dir,
    /// Plume length (m), after bonuses.
    pub length: f32,
    /// The own part the lane burns (`None`: it runs free).
    pub burns: Option<PartIdx>,
    /// How far the plume reaches: the distance to that part, else `length`.
    pub reach: f32,
}

/// One shield emitter's bubble.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShieldReport {
    pub part: PartIdx,
    /// Index into `DesignReport::pieces`.
    pub piece: usize,
    /// Centre of the emitter (blueprint frame).
    pub centre: [f32; 2],
    /// Bubble radius (m), after bonuses.
    pub radius: f32,
}

/// One part's chain reaction (GDR-0084, GDR-0085): the blast it sets off when damage destroys it, on the design
/// as built (every part alive and at full hit points), by the sim's own rules (`ShipDesign::blast`, `chain.rs`).
#[derive(Clone, Debug, PartialEq)]
pub struct BlastReport {
    pub part: PartIdx,
    /// Centre of the part (blueprint frame): the blast's centre.
    pub centre: [f32; 2],
    /// Content `explosion`: damage and radius (m).
    pub damage: f32,
    pub radius: f32,
    /// Every part it reaches, on any piece, ascending.
    pub hits: Vec<BlastHit>,
}

/// One part a blast reaches.
#[derive(Clone, Debug, PartialEq)]
pub struct BlastHit {
    pub part: PartIdx,
    /// The damage it takes: the blast's times its own `blast_factor` and the cover's.
    pub damage: f32,
    /// The armour on the line from the blast's centre that shields it (each lets its `blast_factor` through):
    /// on the exploding part's piece or the target's, ascending.
    pub cover: Vec<PartIdx>,
    /// The damage reaches its hit points: the blast destroys it.
    pub destroys: bool,
}

/// Main piece control (GDR-0038, GDR-0044).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ControlReport {
    /// Control cost of its parts.
    pub load: u32,
    /// Control capacity of its logic cores (0 without a core).
    pub capacity: u32,
    /// The parts that idle while the load exceeds the capacity (power brown-out order).
    pub idle: Vec<PartIdx>,
}

/// Estimated simulation cost of the design (GDR-0006 "device load").
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeviceLoad {
    /// Native µs per tick for this ship (all pieces), from the cost model in [`tuning`].
    pub cost_us: f32,
    pub class: LoadClass,
    pub drivers: CostDrivers,
}

/// Coarse device load class (thresholds in [`tuning`], placeholders).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LoadClass {
    Light,
    Moderate,
    Heavy,
    TooHeavy,
}

/// What a ship costs per tick: the counts the sim's per-tick loops run over (all pieces).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CostDrivers {
    /// Parts: the power and heat passes (with their links).
    pub parts: u32,
    /// Thruster rows (engines and RCS nozzles) and torque sources: the flight computer and the wrench.
    pub rows: u32,
    pub torque_sources: u32,
    /// Guns: firing and their shots.
    pub guns: u32,
    /// Rigid bodies and their collider shapes after merging: the Rapier step.
    pub pieces: u32,
    pub colliders: u32,
}

/// How bad an issue is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// The ship cannot fly.
    Blocker,
    /// It flies, but something does not work as built.
    Warning,
}

/// A problem with the design (structured; the client words it).
#[derive(Clone, Debug, PartialEq)]
pub enum Issue {
    /// The main piece has no logic core: no vessel, nothing flies it.
    NoLogicCore,
    /// Its logic cores get no power (no working reactor): the vessel is dark.
    CoresUnpowered,
    /// No thrust in any direction.
    NoThrust,
    /// At full use the reactors cannot feed every consumer: these run below full level.
    PowerDeficit { deficit: f32, short: Vec<PartIdx> },
    /// At full use these parts run above their heat limit: they would shut down.
    Overheat { parts: Vec<PartIdx> },
    /// A gun's line of fire is blocked by parts of its own piece.
    GunBlocked {
        gun: PartIdx,
        blockers: Vec<PartIdx>,
    },
    /// An engine's plume burns parts of its own piece.
    PlumeBurnsOwnPart {
        engine: PartIdx,
        parts: Vec<PartIdx>,
    },
    /// The design is not one piece: these parts fall off at spawn.
    FallsOff { parts: Vec<PartIdx> },
    /// The control load exceeds the cores' capacity: these parts idle.
    ControlOverCapacity {
        load: u32,
        capacity: u32,
        idle: Vec<PartIdx>,
    },
    /// Destroyed, this part's blast destroys these logic cores, guns and parts that explode in turn (a chain):
    /// GDR-0084's warning about a reactor beside a core or a gun.
    BlastTakes { part: PartIdx, parts: Vec<PartIdx> },
}

impl Issue {
    pub fn severity(&self) -> Severity {
        match self {
            Issue::NoLogicCore | Issue::CoresUnpowered | Issue::NoThrust => Severity::Blocker,
            _ => Severity::Warning,
        }
    }
}

impl DesignReport {
    /// No blocker: the design can fly.
    pub fn flight_ready(&self) -> bool {
        self.issues
            .iter()
            .all(|i| i.severity() != Severity::Blocker)
    }

    /// Main piece: damage per second of all its guns as if each were clear and powered (blocked and idle guns
    /// counted, at full power with bonuses): what the design fires once its line of fire is shot clear and its
    /// control load fits. [`DesignReport::firepower`] is what it fires as built (the drydock's); this one is the
    /// evolution archive's firepower measure (`docs/arena.md`), so designs that hide their guns behind their own
    /// parts sit with the other gun-heavy designs.
    pub fn latent_firepower(&self) -> f32 {
        self.guns
            .iter()
            .filter(|g| Some(g.piece) == self.main)
            .map(|g| g.dps)
            .sum()
    }
}

/// The design report of a blueprint (see the module docs). Pass the same (tuning-scaled) content and
/// `SimConfig` the game flies with. Pure and deterministic; works on any in-memory blueprint (overlaps and the
/// frame are not checked). `Err` for unknown parts or materials and for power
/// weights or minimum levels out of range (`ShipDesign::compile`).
pub fn design_report(
    content: &Content,
    blueprint: &Blueprint,
    config: &SimConfig,
) -> Result<DesignReport, String> {
    let config = &config.finite_or_default();
    let design = Arc::new(ShipDesign::compile(content, blueprint)?);
    let mut ship = Ship::new(
        ShipId(0),
        Arc::clone(&design),
        Owner::PLAYER,
        Body::default(),
        &mut IdAlloc::default(),
        config,
    );
    let d = &*design;
    let n = d.parts.len();
    let main = ship.main_piece().and_then(|id| ship.piece_index(id));

    // The main piece as it runs: idle parts (GDR-0044), capability at full power, then the power pass at full use.
    let mut flight = FlightReport::default();
    let mut power = PowerReport::default();
    let (mut mass, mut centre_of_mass, mut inertia) = (0.0, [0.0; 2], 0.0);
    let mut rule_slots = 0;
    let mut firepower = 0.0;
    let mut control = ControlReport::default();
    if let Some(k) = main {
        let Ship { pieces, parts, .. } = &mut ship;
        let piece = &mut pieces[k];
        let running = piece.controlled;
        crate::ship::control::update(piece, parts, d, running);
        piece.active = true;
        refresh_authority(piece, parts);
        let kin = Kin::of(piece, parts);
        flight = flight_report(piece, parts, &kin, config);
        firepower = piece.firepower(parts);
        mass = piece.mass.mass;
        inertia = piece.mass.inertia;
        centre_of_mass = [
            piece.origin[0] + piece.mass.com.x,
            piece.origin[1] - piece.mass.com.y,
        ];
        control = ControlReport {
            load: piece.control.load,
            capacity: piece.control.capacity,
            idle: piece
                .parts
                .iter()
                .copied()
                .filter(|&p| parts[p as usize].idle)
                .collect(),
        };

        // Full use: every mover at full activity, every gun firing, shields on; draws in bursts left out.
        let supply = reactor_supply(piece, parts);
        let mut activity = vec![1.0f32; n];
        let (mut want, mut level) = (Vec::new(), Vec::new());
        let (demand, served) = power_pass(
            piece,
            parts,
            &mut activity,
            &mut want,
            &mut level,
            supply,
            true,
            |_| true,
            true,
            |p| d.parts[p as usize].intermittent.is_none(),
        );
        let mut bursts: Vec<(PartIdx, f32)> = piece
            .consumers()
            .iter()
            .filter(|c| d.parts[c.part as usize].intermittent.is_some() && c.tier != PowerTier::Off)
            .map(|c| (c.part, c.draw))
            .collect();
        bursts.sort_by_key(|b| b.0);
        let mut intermittent: Vec<(String, f32)> = Vec::new();
        for (part, draw) in bursts {
            let words = d.parts[part as usize].intermittent.as_deref().unwrap_or_default();
            match intermittent.iter_mut().find(|(w, _)| w == words) {
                Some((_, x)) => *x += draw,
                None => intermittent.push((words.to_string(), draw)),
            }
        }
        power = PowerReport {
            supply,
            demand,
            served,
            deficit: (demand - served).max(0.0),
            intermittent,
            consumers: piece
                .consumers()
                .iter()
                .enumerate()
                .map(|(i, c)| ConsumerReport {
                    part: c.part,
                    kind: c.kind,
                    tier: c.tier,
                    weight: c.weight,
                    min_level: c.min_level,
                    draw: c.draw,
                    // A consumer the player set `Off` asks for nothing (it counts for neither demand nor served).
                    want: if c.tier == PowerTier::Off { 0.0 } else { want[i] },
                    level: level[i],
                })
                .collect(),
        };
        let id = piece.id;
        rule_slots = ship.rule_slots_of(id);
    }
    let main_id = main.map(|k| ship.pieces[k].id);
    let on_main = |p: usize| Some(ship.parts[p].piece) == main_id;

    // Steady-state heat (only the main piece works; everything else stays at 0).
    let mut heat = vec![0.0f32; n];
    steady_heat(d, &ship.heat_k, &ship.parts, &mut heat);

    // Pieces and parts.
    let piece_of = |p: usize| {
        ship.piece_index(ship.parts[p].piece)
            .expect("every part of a new ship is on a piece")
    };
    let pieces: Vec<PieceReport> = ship
        .pieces
        .iter()
        .map(|p| PieceReport {
            parts: p.parts.clone(),
            vessel: p.has_core,
            powered: reactor_supply(p, &ship.parts) > 0.0,
        })
        .collect();
    let mut heat_report = HeatReport::default();
    let mut parts = Vec::with_capacity(n);
    let mut falls_off = Vec::new();
    for (i, &t) in heat.iter().enumerate() {
        let ps = &ship.parts[i];
        let main_part = on_main(i);
        if !main_part {
            falls_off.push(i as PartIdx);
        }
        let limit = ship.heat_k[i].limit;
        let ratio = if limit.is_finite() && limit > 0.0 {
            t / limit
        } else {
            0.0
        };
        if main_part && limit.is_finite() {
            if heat_report.peak_part.is_none() || ratio > heat_report.peak {
                heat_report.peak = ratio;
                heat_report.peak_part = Some(i as PartIdx);
            }
            if t > limit {
                heat_report.over.push(i as PartIdx);
            }
        }
        parts.push(PartReport {
            piece: piece_of(i),
            stats: ship.stats[i],
            open_sides: ps.open_sides,
            heat: t,
            heat_ratio: ratio,
            power: if main_part { ps.power } else { 0.0 },
            idle: ps.idle,
        });
    }

    // Bonuses that apply, for the overlay.
    let mut bonuses = Vec::new();
    let mut groups = Vec::new();
    for t in 0..n as PartIdx {
        applied_bonuses(d, &ship.parts, t, &mut groups, |b| {
            bonuses.push(BonusReport {
                target: t,
                source: b.source,
                stat: b.stat,
                effect: b.effect,
            })
        });
    }

    // Guns, engines and plumes, shields (every piece).
    let mut guns = Vec::new();
    let mut engines = Vec::new();
    let mut shields = Vec::new();
    for (k, piece) in ship.pieces.iter().enumerate() {
        for g in &piece.guns {
            let spec = &d.guns[g.slot as usize];
            guns.push(GunReport {
                slot: g.slot,
                part: g.part,
                piece: k,
                muzzle: spec.muzzle,
                facing: spec.facing,
                clear: g.clear,
                blockers: gun_blockers(d, &ship.parts, g.slot, piece.id).collect(),
                range: g.range,
                dps: g.dps,
                rate: g.rate,
                family: g.family,
                delivery: g.delivery,
            });
        }
        for t in piece.thrusters.iter().filter(|t| t.slot != NO_SLOT) {
            let e = &d.engines[t.slot as usize];
            let dir = e.facing.opposite();
            let lanes = (e.lanes[0]..e.lanes[1])
                .map(|l| {
                    let path = d.plume_path.get(l as usize);
                    let burn = if t.burns() {
                        crate::combat::plume::own_burn(path, t.plume_length, &ship.parts, piece.id)
                    } else {
                        None
                    };
                    LaneReport {
                        origin: d.plume_lanes[l as usize].origin,
                        dir,
                        length: t.plume_length,
                        burns: burn.map(|b| b.1),
                        reach: burn.map_or(t.plume_length, |b| b.0),
                    }
                })
                .collect();
            engines.push(EngineReport {
                slot: t.slot,
                part: t.part,
                piece: k,
                facing: e.facing,
                thrust: t.max_force,
                lanes,
            });
        }
        for &e in &piece.shields {
            shields.push(ShieldReport {
                part: e,
                piece: k,
                centre: d.parts[e as usize].shape.center(),
                radius: ship.stats[e as usize].get(Stat::ShieldRadius),
            });
        }
    }
    guns.sort_by_key(|g| g.slot);
    engines.sort_by_key(|e| e.slot);

    // Blasts: the compiled targets of each exploding part, with the cover on its piece or the target's.
    let blasts = (0..n)
        .filter_map(|i| {
            let e = d.parts[i].explosion.as_ref()?;
            let hits = d
                .blast
                .get(i)
                .iter()
                .map(|t| {
                    let ends = [piece_of(i), piece_of(t.part as usize)];
                    let cover: Vec<PartIdx> = d
                        .blast_cover
                        .get(t.cover as usize)
                        .iter()
                        .copied()
                        .filter(|&c| ends.contains(&piece_of(c as usize)))
                        .collect();
                    let damage = cover.iter().fold(t.damage, |a, &c| a * d.parts[c as usize].blast_factor);
                    BlastHit {
                        part: t.part,
                        damage,
                        cover,
                        destroys: damage >= d.parts[t.part as usize].hp,
                    }
                })
                .collect();
            Some(BlastReport {
                part: i as PartIdx,
                centre: d.parts[i].shape.center(),
                damage: e.damage,
                radius: e.radius,
                hits,
            })
        })
        .collect();

    // Totals.
    let (mut cargo, mut hp) = (0.0f32, 0.0f32);
    if let Some(k) = main {
        for &p in &ship.pieces[k].parts {
            let spec = &d.parts[p as usize];
            cargo += spec.cargo;
            hp += spec.hp;
        }
    }
    let build_points = content.ship_totals(blueprint).build_points;
    let device = device_load(&ship);

    let mut report = DesignReport {
        pieces,
        main,
        falls_off,
        parts,
        bonuses,
        mass,
        centre_of_mass,
        inertia,
        power,
        heat: heat_report,
        flight,
        guns,
        firepower,
        engines,
        shields,
        blasts,
        cargo,
        rule_slots,
        control,
        hp,
        build_points,
        device,
        issues: Vec::new(),
    };
    report.issues = issues(&report, &ship);
    Ok(report)
}

/// Thrust, top speed, turning and yaw balance of the main piece (authority already at full power).
fn flight_report(
    piece: &crate::Piece,
    parts: &[PartState],
    kin: &Kin,
    config: &SimConfig,
) -> FlightReport {
    let a = &piece.authority;
    let mut accel = [0.0f32; 4];
    for (g, acc) in accel.iter_mut().enumerate() {
        *acc = kin.caps[g] / kin.mass;
    }
    // Full forward thrust: every forward row that can produce, at its power level (1 here).
    let (mut yaw, mut gimbal) = (0.0f32, 0.0f32);
    for t in &piece.thrusters {
        let ps = &parts[t.part as usize];
        if crate::ship::alloc::group_of(t.dir) != 0 || !producing(ps) {
            continue;
        }
        let f = t.max_force * ps.power;
        yaw += t.torque_per_force * f;
        gimbal += f * t.gimbal * t.torque_per_lateral.abs();
    }
    // Against the yaw, with the forward rows busy pushing: every other row whose torque has the needed sign, as
    // an RCS couple or as differential thrust (the last resort: flying straight on the stick or under a lock
    // steers to a heading, so the flight computer opens it). Each row's force turns the ship the same either
    // way, so the sum covers both.
    let mut against = 0.0f32;
    for t in &piece.thrusters {
        let ps = &parts[t.part as usize];
        if crate::ship::alloc::group_of(t.dir) != 0 && producing(ps) && t.torque_per_force * yaw < 0.0 {
            against += t.max_force * ps.power * t.torque_per_force.abs();
        }
    }
    let yaw_hold = a.torque_pure + against + gimbal;
    FlightReport {
        thrust: kin.caps,
        accel,
        top_speed: kin.speeds,
        best: kin.best,
        torque_pure: a.torque_pure,
        torque_ccw: a.torque_ccw,
        torque_cw: a.torque_cw,
        alpha: kin.alpha,
        turn_90: turn_time(kin.inertia, a, config),
        yaw,
        yaw_hold,
        yaw_balanced: yaw.abs() <= yaw_hold,
    }
}

/// Estimated time (s) of a 90° counter-clockwise turn from rest, settled within `TURN_SETTLE`: the flight
/// computer's heading loop integrated in one dimension, tick by tick. Each tick it asks
/// `τ = I · (turn_rate(e, α_plan) − ω) / DT` (the sim's [`turn_rate`] and [`Kin::plan_alpha`]) and gets at most
/// the first line (pure torque and the RCS couples of that sense); the last resort ([`TurnAssist`]: the
/// differential thrust `torque_ccw` / `torque_cw`) joins in and counts in the plan only after the first line
/// has been at its limit for `assist_after` ticks, as in the sim. It moves the way Rapier integrates a constant
/// torque over `S` substeps: `θ += ω·DT + (S + 1)/(2S) · (τ/I)·DT²`, then `ω += τ/I · DT`. Ignores what the
/// velocity hold and the gimbals add or take; `tests/report.rs` compares it with stepped turns.
fn turn_time(inertia: f32, a: &crate::ThrustAuthority, config: &SimConfig) -> Option<f32> {
    let first = [a.torque_pure + a.couple_ccw, a.torque_pure + a.couple_cw];
    if Kin::plan_alpha(a, inertia, false) <= 0.0 || first[0] <= 0.0 {
        return None;
    }
    let ft = &config.flight;
    let sub = config.physics.substeps.max(1) as f32;
    let lag = (sub + 1.0) / (2.0 * sub);
    let target = core::f32::consts::FRAC_PI_2;
    let (mut angle, mut omega, mut gate) = (0.0f32, 0.0f32, TurnAssist::default());
    for tick in 1..=tuning::TURN_MAX_TICKS {
        let e = target - angle;
        let alpha = Kin::plan_alpha(a, inertia, gate.on);
        let want = inertia * (turn_rate(e, alpha, ft) - omega) * (1.0 / DT);
        let (first, diff) = if want > 0.0 {
            (first[0], a.torque_ccw)
        } else {
            (first[1], a.torque_cw)
        };
        let saturated = want.abs() - first > crate::ship::alloc::tolerance(want);
        gate = gate.step(saturated, e.abs() > TURNING, ft.assist_after);
        let cap = if gate.on { first + diff } else { first };
        let dw = want.clamp(-cap, cap) / inertia * DT;
        angle += (omega + lag * dw) * DT;
        omega += dw;
        if (target - angle).abs() < tuning::TURN_SETTLE {
            return Some(tick as f32 * DT);
        }
    }
    None
}

/// Steady state of the heat model of `Ship::update_systems`, `loss·T = out·work − Σ K·edges·(T − T_neighbour)`
/// per alive part, i.e. the symmetric positive definite system `(loss_i + Σ_j K·e_ij)·T_i − Σ_j K·e_ij·T_j =
/// out_i·work_i` (a diagonally dominant M-matrix, `loss_i > 0`), solved with Jacobi-preconditioned conjugate
/// gradients in f64. Every sum runs in a fixed order (rows in part order, dot products in four fixed lanes), so
/// the result is the same on every platform. Parts that do not work and have no working neighbour stay at 0.
fn steady_heat(d: &ShipDesign, heat_k: &[HeatK], parts: &[PartState], out: &mut [f32]) {
    let n = parts.len();
    out.fill(0.0);
    let k = heat_tuning::HEAT_CONDUCTANCE as f64;
    let mut diag = vec![0.0f64; n];
    let mut b = vec![0.0f64; n];
    for i in 0..n {
        if parts[i].alive {
            diag[i] = heat_k[i].loss as f64;
            b[i] = heat_k[i].out as f64 * parts[i].work as f64;
        }
    }
    let bb = dot(&b, &b);
    if bb <= 0.0 {
        return;
    }
    // Rows of the matrix as neighbour lists (CSR, both directions of every live link, in link order).
    let live = |l: &&crate::ship::design::Link| parts[l.a as usize].alive && parts[l.b as usize].alive;
    let mut start = vec![0usize; n + 1];
    for l in d.links.iter().filter(live) {
        start[l.a as usize + 1] += 1;
        start[l.b as usize + 1] += 1;
    }
    for i in 0..n {
        start[i + 1] += start[i];
    }
    let mut row = vec![(0usize, 0.0f64); start[n]];
    let mut fill = start.clone();
    for l in d.links.iter().filter(live) {
        let (a, c, g) = (l.a as usize, l.b as usize, k * l.edges as f64);
        diag[a] += g;
        diag[c] += g;
        row[fill[a]] = (c, g);
        fill[a] += 1;
        row[fill[c]] = (a, g);
        fill[c] += 1;
    }
    let inv: Vec<f64> = diag
        .iter()
        .map(|&g| if g > 0.0 { 1.0 / g } else { 0.0 })
        .collect();
    let apply = |x: &[f64], y: &mut [f64]| {
        for i in 0..n {
            let mut s = diag[i] * x[i];
            for &(j, g) in &row[start[i]..start[i + 1]] {
                s -= g * x[j];
            }
            y[i] = s;
        }
    };
    let mut x = vec![0.0f64; n];
    let mut r = b;
    let mut z: Vec<f64> = r.iter().zip(&inv).map(|(r, v)| r * v).collect();
    let mut p = z.clone();
    let mut ap = vec![0.0f64; n];
    let mut rz = dot(&r, &z);
    let tol = tuning::HEAT_TOLERANCE * tuning::HEAT_TOLERANCE * bb;
    for _ in 0..tuning::HEAT_MAX_ITERATIONS {
        apply(&p, &mut ap);
        let pap = dot(&p, &ap);
        if pap <= 0.0 {
            break;
        }
        let step = rz / pap;
        for i in 0..n {
            x[i] += step * p[i];
            r[i] -= step * ap[i];
        }
        if dot(&r, &r) <= tol {
            break;
        }
        for i in 0..n {
            z[i] = r[i] * inv[i];
        }
        let rz_next = dot(&r, &z);
        let beta = rz_next / rz;
        rz = rz_next;
        for i in 0..n {
            p[i] = z[i] + beta * p[i];
        }
    }
    for (o, &t) in out.iter_mut().zip(&x) {
        *o = t.max(0.0) as f32;
    }
}

/// `Σ x_i·y_i` in four fixed lanes (`i mod 4`), added as `(l0 + l1) + (l2 + l3)`: independent chains for speed,
/// a fixed order for determinism.
fn dot(x: &[f64], y: &[f64]) -> f64 {
    let mut lane = [0.0f64; 4];
    let ((xc, xr), (yc, yr)) = (x.as_chunks::<4>(), y.as_chunks::<4>());
    for (a, b) in xc.iter().zip(yc) {
        for l in 0..4 {
            lane[l] += a[l] * b[l];
        }
    }
    for (l, (a, b)) in xr.iter().zip(yr).enumerate() {
        lane[l] += a * b;
    }
    (lane[0] + lane[1]) + (lane[2] + lane[3])
}

/// The device load of a ship: its cost drivers and the cost model of [`tuning`].
fn device_load(ship: &Ship) -> DeviceLoad {
    let mut c = CostDrivers {
        parts: ship.parts.len() as u32,
        guns: ship.design.guns.len() as u32,
        pieces: ship.pieces.len() as u32,
        ..CostDrivers::default()
    };
    for p in &ship.pieces {
        c.rows += p.thrusters.len() as u32;
        c.torque_sources += p.torque_sources.len() as u32;
        c.colliders += p.colliders.len() as u32;
    }
    let cost_us = estimate_cost(&c);
    let class = if cost_us <= tuning::LIGHT_MAX {
        LoadClass::Light
    } else if cost_us <= tuning::MODERATE_MAX {
        LoadClass::Moderate
    } else if cost_us <= tuning::HEAVY_MAX {
        LoadClass::Heavy
    } else {
        LoadClass::TooHeavy
    };
    DeviceLoad {
        cost_us,
        class,
        drivers: c,
    }
}

/// The cost model: native µs per tick from the cost drivers (coefficients in [`tuning`]).
pub fn estimate_cost(c: &CostDrivers) -> f32 {
    use tuning::*;
    COST_SHIP
        + COST_PART * c.parts as f32
        + COST_ROW * c.rows as f32
        + COST_TORQUE_SOURCE * c.torque_sources as f32
        + COST_GUN * c.guns as f32
        + COST_PIECE * c.pieces as f32
        + COST_COLLIDER * c.colliders as f32
}

/// The issues of a finished report, blockers first.
fn issues(r: &DesignReport, ship: &Ship) -> Vec<Issue> {
    let mut out = Vec::new();
    let main = r.main.map(|k| &ship.pieces[k]);
    let has_core = main.is_some_and(|p| p.has_core);
    if !has_core {
        out.push(Issue::NoLogicCore);
    } else if r.power.supply <= 0.0 {
        out.push(Issue::CoresUnpowered);
    }
    if r.flight.thrust.iter().all(|&f| f <= 0.0) {
        out.push(Issue::NoThrust);
    }
    if r.power.deficit > 0.0 {
        let short = r
            .power
            .consumers
            .iter()
            .filter(|c| c.want > 0.0 && c.level < 1.0)
            .map(|c| c.part)
            .collect();
        out.push(Issue::PowerDeficit {
            deficit: r.power.deficit,
            short,
        });
    }
    if !r.heat.over.is_empty() {
        out.push(Issue::Overheat {
            parts: r.heat.over.clone(),
        });
    }
    for g in r
        .guns
        .iter()
        .filter(|g| Some(g.piece) == r.main && !g.clear)
    {
        out.push(Issue::GunBlocked {
            gun: g.part,
            blockers: g.blockers.clone(),
        });
    }
    for e in r.engines.iter().filter(|e| Some(e.piece) == r.main) {
        let mut parts: Vec<PartIdx> = e.lanes.iter().filter_map(|l| l.burns).collect();
        parts.sort_unstable();
        parts.dedup();
        if !parts.is_empty() {
            out.push(Issue::PlumeBurnsOwnPart {
                engine: e.part,
                parts,
            });
        }
    }
    if !r.falls_off.is_empty() {
        out.push(Issue::FallsOff {
            parts: r.falls_off.clone(),
        });
    }
    if has_core && r.control.load > r.control.capacity {
        out.push(Issue::ControlOverCapacity {
            load: r.control.load,
            capacity: r.control.capacity,
            idle: r.control.idle.clone(),
        });
    }
    let d = &*ship.design;
    for b in &r.blasts {
        let parts: Vec<PartIdx> = b
            .hits
            .iter()
            .filter(|h| h.destroys)
            .map(|h| h.part)
            .filter(|&p| {
                let spec = &d.parts[p as usize];
                spec.logic || spec.weapon.is_some() || spec.explosion.is_some()
            })
            .collect();
        if !parts.is_empty() {
            out.push(Issue::BlastTakes { part: b.part, parts });
        }
    }
    out
}
