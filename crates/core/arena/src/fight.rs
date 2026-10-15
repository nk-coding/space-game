//! Building a fight from a spec and running it, in one go ([`run_fight`]) or in slices ([`Fight::step`]). A client
//! that steps the world itself (the fight viewer, GDR-0092) builds it with [`FightSetup::build`] and asks the
//! [`Referee`] after every tick; [`Fight`] is the two together. A gauntlet (a spec with `waves`) is a fight whose
//! referee sends side 1's next wave once the one before has no running vessel, as the test range does.

use std::sync::Arc;

use data::{Blueprint, Content};
use serde::{Deserialize, Serialize};
use sim::voyage::waves::wave_bodies;
use sim::{
    BehaviorId, BehaviorLibrary, Body, Owner, ShipCommand, ShipDesign, Sim, SimConfig, SimEvent,
    Stream, TickInputs, Vec2, VesselState,
};

use crate::error::ArenaError;
use crate::score::arena_score;
use crate::spec::{BehaviorRef, DesignRef, FightSpec, Formation};

/// A side is out once it has had no running vessel for this many ticks (5 s), so a short brown-out is not a
/// loss (coop-arena 3.7). PLACEHOLDER.
pub const OUT_AFTER: u64 = 5 * sim::TICK_HZ as u64;
/// Metres between neighbouring ships of a formation, and between one entry's ships and the next entry's.
/// PLACEHOLDER.
pub const FORMATION_GAP: f32 = 5.0;
/// Every ship's spawn position is moved by up to this many metres in x and y, rolled from the seed, so seeds
/// differ (the sim itself rolls nothing in a plain fight). PLACEHOLDER.
pub const JITTER: f32 = 2.0;
/// A gauntlet's next wave comes in this many ticks (4 s, the test range's pause) after the one before has no running
/// vessel left. PLACEHOLDER.
pub const WAVE_DELAY: u64 = 4 * sim::TICK_HZ as u64;
/// The sim owners of side 0 and side 1. Neither is the player's `Owner(0)`, so both sides get the same rules
/// (only the player collects salvage).
pub const OWNERS: [Owner; 2] = [Owner(1), Owner(2)];

/// How a fight ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// The other side had no running vessel for [`OUT_AFTER`] ticks while this one still had one.
    Won(usize),
    /// Neither side has a running vessel any more.
    BothOut,
    /// The time limit came first.
    TimeLimit,
}

/// One side's numbers at the end of a fight.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SideResult {
    /// Build points of the side's vessels at the start (each part's build points, with its material factor).
    pub build_points: f32,
    /// Build points no longer on one of the side's vessels: parts destroyed, and parts on pieces that broke off
    /// as salvage or lost their last logic core.
    pub build_points_lost: f32,
    /// Damage the side's shots and engine plumes did to the other side's ships (after shields).
    pub damage_dealt: f32,
    /// Vessels the side has at the end (running, dark or dormant).
    pub vessels_left: u32,
    /// The arena score, 0..1 ([`arena_score`]).
    pub score: f32,
}

impl SideResult {
    /// Share of the side's build points lost, 0..1 (0 for a side that started with none).
    pub fn lost_share(&self) -> f32 {
        if self.build_points > 0.0 {
            (self.build_points_lost / self.build_points).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FightResult {
    pub outcome: Outcome,
    /// Ticks the fight ran (30 per second).
    pub ticks: u64,
    pub sides: [SideResult; 2],
    /// `Sim::state_hash` at the end: equal specs give equal hashes on every platform.
    pub state_hash: u64,
    /// A gauntlet's waves (the first, side 1's own entries, counts) that had no running vessel left at some point:
    /// all of them is a win. 0 for a plain fight (and not written then, so plain fights read as before).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub waves: u32,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

impl FightResult {
    /// The side that won, if one did.
    pub fn winner(&self) -> Option<usize> {
        match self.outcome {
            Outcome::Won(side) => Some(side),
            Outcome::BothOut | Outcome::TimeLimit => None,
        }
    }
}

/// A fight in progress. Build it with [`Fight::new`], then call [`Fight::step`] until it returns the result: all
/// at once ([`run_fight`]) or a few ticks per frame in a client.
pub struct Fight {
    sim: Sim,
    referee: Referee,
    idle: TickInputs,
}

/// A fight built and not yet run: the world, its referee, and the blueprint and compiled design of every entry
/// (per side, in entry order; a gauntlet's waves after side 1's own), which a client needs to draw ships whose design
/// is no content ship. Step the world
/// with no inputs and call [`Referee::observe`] after every tick; that is all [`Fight`] does.
pub struct FightSetup {
    pub sim: Sim,
    pub referee: Referee,
    pub designs: [Vec<(Blueprint, Arc<ShipDesign>)>; 2],
}

/// Watches a fight tick by tick: tallies the damage each side deals and decides when and how it ends.
#[derive(Clone, Debug)]
pub struct Referee {
    limit: u64,
    build_points: [f32; 2],
    damage: [f32; 2],
    /// The first tick of the current stretch without a running vessel, per side.
    out_since: [Option<u64>; 2],
    /// Per side, a bit for every behavior rule that decided at least once (bit `i`: rule `i`).
    decided: [u64; 2],
    /// A gauntlet's waves still to come (each entry's design, behavior and count), the next one's index, the tick it
    /// comes in once the wave before is out, the waves cleared, and side 1's build points brought in so far.
    waves: Vec<Vec<(Arc<ShipDesign>, BehaviorId, u32)>>,
    next_wave: usize,
    wave_at: Option<u64>,
    cleared: u32,
    fielded: f32,
    result: Option<FightResult>,
}

/// One entry resolved against the content: its blueprint, its compiled design and its behavior.
struct Resolved {
    blueprint: Blueprint,
    design: Arc<ShipDesign>,
    behavior: BehaviorId,
}

impl FightSetup {
    /// Checks the spec, compiles its designs and behaviors and spawns the ships. Every problem with the spec
    /// comes back as an [`ArenaError`].
    pub fn build(content: &Content, spec: &FightSpec) -> Result<FightSetup, ArenaError> {
        spec.check()?;
        let mut lib = BehaviorLibrary::new(content).map_err(|p| {
            ArenaError::Content(
                p.iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        })?;
        // Sides 0 and 1, then the waves (errors name wave k as side 2 + k).
        let mut resolved: Vec<Vec<Resolved>> = Vec::new();
        for (side, s) in spec.sides.iter().chain(&spec.waves).enumerate() {
            let mut list = Vec::new();
            for (entry, e) in s.entries.iter().enumerate() {
                let (blueprint, design) = match resolve_design(content, &e.design, e.behavior.as_ref()) {
                    Ok((bp, d)) => (bp, Arc::new(d)),
                    Err(DesignProblem::UnknownShip(id)) => {
                        return Err(ArenaError::UnknownShip { side, entry, id });
                    }
                    Err(DesignProblem::Blueprint(problems)) => {
                        return Err(ArenaError::Blueprint {
                            side,
                            entry,
                            problems,
                        });
                    }
                };
                let behavior = match &e.behavior {
                    Some(BehaviorRef::Content(id)) => {
                        lib.id(id).ok_or_else(|| ArenaError::UnknownBehavior {
                            side,
                            entry,
                            id: id.clone(),
                        })?
                    }
                    Some(BehaviorRef::Text(text)) => lib
                        .add_text(&format!("arena_{side}_{entry}"), text)
                        .map_err(|errs| ArenaError::BehaviorText {
                            side,
                            entry,
                            errors: errs.iter().map(|e| e.to_string()).collect(),
                        })?,
                    None => lib
                        .default_for(&design)
                        .ok_or_else(|| ArenaError::NoBehavior {
                            side,
                            entry,
                            design: design.id.clone(),
                        })?,
                };
                list.push(Resolved {
                    blueprint,
                    design,
                    behavior,
                });
            }
            resolved.push(list);
        }
        let waves = resolved.split_off(2);
        let sides: [Vec<Resolved>; 2] = resolved.try_into().ok().expect("a spec has two sides");

        let mut config = SimConfig::default();
        for o in OWNERS {
            config.mind_capacity[o.0 as usize] = u32::MAX; // every core of a side runs
        }
        let mut sim = Sim::with_config(spec.seed, config);
        sim.set_behaviors(lib);
        for (side, resolved) in sides.iter().enumerate() {
            spawn_side(&mut sim, side, spec, resolved);
        }
        let build_points = OWNERS.map(|o| kept(&sim, o).0);
        let referee = Referee {
            limit: spec.limit_ticks(),
            build_points,
            damage: [0.0; 2],
            out_since: [None; 2],
            decided: [0; 2],
            waves: waves
                .iter()
                .zip(&spec.waves)
                .map(|(w, s)| {
                    w.iter()
                        .zip(&s.entries)
                        .map(|(r, e)| (Arc::clone(&r.design), r.behavior, e.count))
                        .collect()
                })
                .collect(),
            next_wave: 0,
            wave_at: None,
            cleared: 0,
            fielded: build_points[1],
            result: None,
        };
        let mut designs = sides.map(|side| {
            side.into_iter()
                .map(|r| (r.blueprint, r.design))
                .collect::<Vec<_>>()
        });
        designs[1].extend(waves.into_iter().flatten().map(|r| (r.blueprint, r.design)));
        Ok(FightSetup {
            sim,
            referee,
            designs,
        })
    }
}

impl Referee {
    /// Looks at the world after a tick: the damage dealt in it, a gauntlet's next wave (which it brings in), and
    /// whether the fight ended with it. `Some` from the tick it ends on; later calls look at nothing and return the
    /// same result.
    pub fn observe(&mut self, sim: &mut Sim) -> Option<FightResult> {
        if self.result.is_none() {
            self.tally(sim);
            self.note_rules(sim);
            if !self.waves.is_empty() {
                self.reinforce(sim);
            }
            self.result = self.end(sim);
        }
        self.result
    }

    /// A gauntlet's waves: `(the wave side 1 fights now, all of them)`, counting its own entries as wave 1.
    pub fn wave(&self) -> (u32, u32) {
        (self.next_wave as u32 + 1, self.waves.len() as u32 + 1)
    }

    /// A gauntlet: once side 1 has no running vessel, its wave is cleared, and the next comes in [`WAVE_DELAY`]
    /// later around side 0's first running vessel (as the test range's waves do), rolled on the `Scenario` stream.
    fn reinforce(&mut self, sim: &mut Sim) {
        let tick = sim.tick();
        if running(sim, OWNERS[1]) {
            return;
        }
        if self.wave_at.is_none() && self.cleared <= self.next_wave as u32 {
            self.cleared += 1;
            if self.next_wave < self.waves.len() {
                self.wave_at = Some(tick + WAVE_DELAY);
            }
        }
        if self.wave_at.is_some_and(|t| tick >= t) {
            self.wave_at = None;
            let centre = sim
                .ships()
                .iter()
                .filter(|s| s.owner() == OWNERS[0] && s.vessel_state() == Some(VesselState::Running))
                .find_map(|s| Some(s.piece(s.control_piece()?)?.com_world()))
                .unwrap_or(Vec2::ZERO);
            let wave = &self.waves[self.next_wave];
            let n: u32 = wave.iter().map(|w| w.2).sum();
            let rng = sim.rng(Stream::Scenario);
            let bodies = wave_bodies(centre, n as usize, |lo, hi| rng.range_f32(lo, hi));
            let before = kept(sim, OWNERS[1]).0;
            let ships = wave.iter().flat_map(|(d, b, n)| std::iter::repeat_n((d, *b), *n as usize));
            for ((design, behavior), body) in ships.zip(bodies) {
                let id = sim.spawn_ship(Arc::clone(design), OWNERS[1], body);
                sim.command(id, ShipCommand::Behavior(Some(behavior)));
            }
            self.fielded += kept(sim, OWNERS[1]).0 - before;
            self.next_wave += 1;
            self.out_since[1] = None;
        }
    }

    /// The result, once the fight has ended.
    pub fn result(&self) -> Option<FightResult> {
        self.result
    }

    /// The fight's time limit in ticks.
    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// Each side's build points at the start.
    pub fn build_points(&self) -> [f32; 2] {
        self.build_points
    }

    /// Per side, a bit for every rule of its ships' behaviors that decided at least once so far (bit `i`: rule `i`;
    /// a program has at most `data::behavior::MAX_RULES` = 64): the rules a design's fights used, so the lab can
    /// tidy away the ones that never run (GDR-0092).
    pub fn decided(&self) -> [u64; 2] {
        self.decided
    }

    /// Damage dealt by each side's shots and plumes this tick.
    fn tally(&mut self, sim: &Sim) {
        for e in sim.events() {
            let (by, ship, damage) = match *e {
                SimEvent::Hit {
                    by, ship, damage, ..
                } => (by, ship, damage),
                SimEvent::PlumeHit {
                    by, ship, damage, ..
                } => (by, ship, damage),
                _ => continue,
            };
            let owner = |id| sim.ship(id).map(|s| s.owner());
            let (Some(from), Some(to)) = (owner(by), owner(ship)) else {
                continue;
            };
            if let Some(side) = OWNERS.iter().position(|&o| o == from)
                && to != from
            {
                self.damage[side] += damage;
            }
        }
    }

    /// The rule each side's ships fly now.
    fn note_rules(&mut self, sim: &Sim) {
        for s in sim.ships() {
            let side = OWNERS.iter().position(|&o| o == s.owner());
            if let (Some(side), Some(rule)) = (side, s.behavior().and_then(|b| b.active_rule())) {
                self.decided[side] |= 1u64.checked_shl(rule as u32).unwrap_or(0);
            }
        }
    }

    /// The result, if the fight ended this tick.
    fn end(&mut self, sim: &Sim) -> Option<FightResult> {
        let tick = sim.tick();
        let mut out = [false; 2];
        // A gauntlet's side 1 is not out while a wave is still to come.
        let coming = self.next_wave < self.waves.len();
        for (side, &owner) in OWNERS.iter().enumerate() {
            if running(sim, owner) || (side == 1 && coming) {
                self.out_since[side] = None;
            } else {
                let since = *self.out_since[side].get_or_insert(tick);
                out[side] = tick - since >= OUT_AFTER;
            }
        }
        let outcome = if out[0] || out[1] {
            match self.out_since {
                [Some(_), None] => Outcome::Won(1),
                [None, Some(_)] => Outcome::Won(0),
                _ => Outcome::BothOut,
            }
        } else if tick >= self.limit {
            Outcome::TimeLimit
        } else {
            return None;
        };
        let fielded = [self.build_points[0], self.fielded];
        let mut sides = [0, 1].map(|side| {
            let (left, vessels) = kept(sim, OWNERS[side]);
            SideResult {
                build_points: self.build_points[side],
                build_points_lost: (fielded[side] - left).max(0.0),
                damage_dealt: self.damage[side],
                vessels_left: vessels,
                score: 0.0,
            }
        });
        let lost = sides.map(|s| s.lost_share());
        for (side, s) in sides.iter_mut().enumerate() {
            s.score = arena_score(outcome, side, lost);
        }
        Some(FightResult {
            outcome,
            ticks: tick,
            sides,
            state_hash: sim.state_hash(),
            waves: self.cleared,
        })
    }
}

/// Has `owner` a running vessel?
fn running(sim: &Sim, owner: Owner) -> bool {
    sim.ships()
        .iter()
        .any(|s| s.owner() == owner && s.vessel_state() == Some(VesselState::Running))
}

impl Fight {
    /// Checks the spec, compiles its designs and behaviors and spawns the ships ([`FightSetup::build`]).
    pub fn new(content: &Content, spec: &FightSpec) -> Result<Fight, ArenaError> {
        let setup = FightSetup::build(content, spec)?;
        Ok(Fight {
            sim: setup.sim,
            referee: setup.referee,
            idle: TickInputs::default(),
        })
    }

    /// Runs up to `ticks` more ticks; stops early when the fight ends. `Some` once it has ended (later calls
    /// step nothing and return the same result).
    pub fn step(&mut self, ticks: u64) -> Option<FightResult> {
        for _ in 0..ticks {
            if self.referee.result().is_some() {
                break;
            }
            self.sim.step(&self.idle);
            self.referee.observe(&mut self.sim);
        }
        self.referee.result()
    }

    /// The world, for drawing a fight or hashing it every tick.
    pub fn sim(&self) -> &Sim {
        &self.sim
    }

    pub fn referee(&self) -> &Referee {
        &self.referee
    }
}

/// Builds the fight and runs it to the end.
pub fn run_fight(content: &Content, spec: &FightSpec) -> Result<FightResult, ArenaError> {
    let mut fight = Fight::new(content, spec)?;
    Ok(fight
        .step(u64::MAX)
        .expect("a fight ends by its time limit"))
}

/// Why a design reference is not usable.
pub(crate) enum DesignProblem {
    UnknownShip(String),
    /// The blueprint text does not read, fails the content's checks or does not compile.
    Blueprint(Vec<String>),
}

/// A design reference resolved: its blueprint (a content ship, or a blueprint text checked against the content
/// first, its rule slots against `flies`, the behavior it will fly, when one is given) and the compiled design.
pub(crate) fn resolve_design(
    content: &Content,
    r: &DesignRef,
    flies: Option<&BehaviorRef>,
) -> Result<(Blueprint, ShipDesign), DesignProblem> {
    let bad = DesignProblem::Blueprint;
    let bp = match r {
        DesignRef::Ship(id) => content
            .ships
            .get(id)
            .cloned()
            .ok_or_else(|| DesignProblem::UnknownShip(id.clone()))?,
        DesignRef::Blueprint(text) => {
            let bp = data::parse_blueprint(text)
                .map_err(|p| bad(p.iter().map(|p| p.to_string()).collect()))?;
            // A program that doesn't read is the behavior's problem, reported when it is compiled.
            let program = match flies {
                Some(BehaviorRef::Text(t)) => data::behavior::Program::parse(t).ok(),
                Some(BehaviorRef::Content(id)) => content.behaviors.get(id).and_then(|b| b.program().ok()),
                None => None,
            };
            let problems = match &program {
                Some(p) => data::validate_ship_flying(content, &bp, p),
                None => data::validate_ship(content, &bp),
            };
            let errors: Vec<String> = problems
                .iter()
                .filter(|p| p.is_error())
                .map(|p| p.to_string())
                .collect();
            if !errors.is_empty() {
                return Err(bad(errors));
            }
            bp
        }
    };
    let design = ShipDesign::compile(content, &bp).map_err(|e| bad(vec![e]))?;
    Ok((bp, design))
}

/// Spawns one side's entries, one behind the other, each under its behavior. Side 0 stands at the origin with
/// its noses to +y; side 1 is the same layout turned half a turn about the middle of the field.
fn spawn_side(sim: &mut Sim, side: usize, spec: &FightSpec, resolved: &[Resolved]) {
    let owner = OWNERS[side];
    let mut depth = 0.0;
    let mut prev_half: Option<f32> = None;
    for (r, e) in resolved.iter().zip(&spec.sides[side].entries) {
        let (w, h) = r
            .design
            .grid
            .bounds()
            .map_or((1.0, 1.0), |b| (b.width() as f32, b.height() as f32));
        if let Some(p) = prev_half {
            depth += p + FORMATION_GAP + h / 2.0;
        }
        let n = e.count;
        for k in 0..n {
            let (x, d) = match e.formation {
                Formation::Line => ((k as f32 - (n as f32 - 1.0) / 2.0) * (w + FORMATION_GAP), depth),
                Formation::Column => (0.0, depth + k as f32 * (h + FORMATION_GAP)),
            };
            let rng = sim.rng(Stream::Scenario);
            let jitter = Vec2::new(
                rng.range_f32(-JITTER, JITTER),
                rng.range_f32(-JITTER, JITTER),
            );
            let body = if side == 0 {
                Body::at(Vec2::new(x, -d) + jitter, 0.0)
            } else {
                Body::at(
                    Vec2::new(-x, spec.start_distance + d) + jitter,
                    core::f32::consts::PI,
                )
            };
            let id = sim.spawn_ship(Arc::clone(&r.design), owner, body);
            sim.command(id, ShipCommand::Behavior(Some(r.behavior)));
        }
        if e.formation == Formation::Column {
            depth += (n as f32 - 1.0) * (h + FORMATION_GAP);
        }
        prev_half = Some(h / 2.0);
    }
}

/// Build points on side `side`'s vessels now, and how many vessels it has (what a client shows during a fight).
pub fn standing(sim: &Sim, side: usize) -> (f32, u32) {
    kept(sim, OWNERS[side])
}

/// Build points on the owner's vessels (alive parts of each vessel's controlling piece), and how many vessels.
fn kept(sim: &Sim, owner: Owner) -> (f32, u32) {
    let mut points = 0.0;
    let mut vessels = 0;
    for s in sim.ships().iter().filter(|s| s.owner() == owner) {
        let Some(control) = s.control_piece() else {
            continue;
        };
        vessels += 1;
        for (p, spec) in s.parts().iter().zip(&s.design().parts) {
            if p.alive && p.piece == control {
                points += spec.build_points;
            }
        }
    }
    (points, vessels)
}
