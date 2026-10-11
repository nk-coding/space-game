//! A deterministic scenario without physics, for the determinism check (native vs wasm vs browser) and for
//! tests: ships with powered systems heating up, controls and damage rolled on the `Scenario` RNG stream,
//! parts destroyed, pieces splitting.
//!
//! It spawns two copies of every ship blueprint in the content (load order), so it needs no hard-coded ids
//! and grows with the content. Bodies move with a trivial free-flight stand-in ([`free_flight`]: engine
//! wrench → velocity → pose, no collisions) so the hash also covers rotations (libm) and the split
//! kinematics. The physics layer will replace the stand-in and plug into the same check.

use std::sync::Arc;

use data::Content;

use crate::breakage::DamageTarget;
use crate::design::{PartIdx, ShipDesign};
use crate::events::SimEvent;
use crate::math::{Rot, Vec2, cos, sin};
use crate::rng::Stream;
use crate::ship::{Body, Ship, ShipControls};
use crate::world::{DamageCommand, Sim, TickInputs};
use crate::DT;

/// Chance per tick of a random hit on a random alive part.
const HIT_CHANCE: f32 = 0.25;
/// Every this many ticks a heavy blast hits a part and all its neighbours (provokes splits).
const BLAST_EVERY: u64 = 75;
/// Controls of each ship are re-rolled every this many ticks (staggered per ship).
const REROLL_EVERY: u64 = 20;

/// Counters over the run, for the summary line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScenarioStats {
    pub parts_destroyed: u32,
    pub splits: u32,
    pub pieces_spawned: u32,
    pub pieces_destroyed: u32,
    /// Part-ticks spent overheated (summed over all parts and ticks).
    pub overheated_part_ticks: u64,
}

pub struct Scenario {
    pub sim: Sim,
    pub stats: ScenarioStats,
    inputs: TickInputs,
}

impl Scenario {
    /// Two ships of every blueprint in `content`, placed on a circle, spinning a little.
    pub fn new(content: &Content, seed: u64) -> Result<Scenario, String> {
        let mut sim = Sim::new(seed);
        let designs: Vec<Arc<ShipDesign>> = content
            .ships
            .iter()
            .map(|b| ShipDesign::compile(content, b).map(Arc::new))
            .collect::<Result<_, _>>()?;
        let n = designs.len() * 2;
        for k in 0..n {
            let design = Arc::clone(&designs[k % designs.len()]);
            let a = k as f32 * (core::f32::consts::TAU / n as f32);
            let body = Body {
                pos: Vec2::new(cos(a) * 60.0, sin(a) * 60.0),
                angle: a,
                linvel: Vec2::new(-sin(a) * 2.0, cos(a) * 2.0),
                angvel: ((k % 5) as f32 - 2.0) * 0.15,
            };
            sim.spawn_ship(design, body);
        }
        Ok(Scenario {
            sim,
            stats: ScenarioStats::default(),
            inputs: TickInputs::default(),
        })
    }

    /// One tick: roll inputs, step the sim, move bodies, count events. Returns the state hash.
    pub fn step(&mut self) -> u64 {
        self.roll_inputs();
        self.sim.step(&self.inputs);
        for e in self.sim.events() {
            match e {
                SimEvent::PartDestroyed { .. } => self.stats.parts_destroyed += 1,
                SimEvent::Split(s) => {
                    self.stats.splits += 1;
                    self.stats.pieces_spawned += s.spawned.len() as u32;
                }
                SimEvent::PieceDestroyed { .. } => self.stats.pieces_destroyed += 1,
                SimEvent::PieceChanged { .. } => {}
            }
        }
        for ship in self.sim.ships_mut() {
            free_flight(ship);
            self.stats.overheated_part_ticks +=
                ship.parts().iter().filter(|p| p.alive && p.overheated).count() as u64;
        }
        self.sim.state_hash()
    }

    fn roll_inputs(&mut self) {
        let tick = self.sim.tick();
        self.inputs.controls.clear();
        self.inputs.damage.clear();
        let ships: Vec<(crate::ShipId, usize)> = self
            .sim
            .ships()
            .iter()
            .map(|s| (s.id(), s.design().engines.len()))
            .collect();
        for (k, &(id, engines)) in ships.iter().enumerate() {
            if !(tick + k as u64 * 3).is_multiple_of(REROLL_EVERY) {
                continue;
            }
            let rng = self.sim.rng(Stream::Scenario);
            // Every second ship runs flat out, so heat builds up and parts overheat and recover.
            let stressed = k % 2 == 1;
            let throttle = (0..engines)
                .map(|_| {
                    if stressed {
                        1.0
                    } else if rng.chance(0.3) {
                        0.0
                    } else {
                        rng.next_f32()
                    }
                })
                .collect();
            let fire = stressed || rng.chance(0.6);
            let shields = rng.chance(0.85);
            self.inputs.controls.push((
                id,
                ShipControls {
                    throttle,
                    fire,
                    shields,
                },
            ));
        }

        let hit = self.sim.rng(Stream::Scenario).chance(HIT_CHANCE);
        let blast = tick > 0 && tick.is_multiple_of(BLAST_EVERY);
        if !(hit || blast) {
            return;
        }
        // A random ship that still has parts, then a random alive part of it.
        let targets: Vec<(crate::ShipId, u32)> = self
            .sim
            .ships()
            .iter()
            .map(|s| (s.id(), s.parts().iter().filter(|p| p.alive).count() as u32))
            .filter(|&(_, n)| n > 0)
            .collect();
        if targets.is_empty() {
            return;
        }
        let rng = self.sim.rng(Stream::Scenario);
        let (ship_id, alive) = targets[rng.below(targets.len() as u32) as usize];
        let pick = rng.below(alive);
        let amount = rng.range_f32(0.4, 2.2);
        let ship = self.sim.ship(ship_id).unwrap();
        let part = nth_alive(ship, pick);
        self.inputs.damage.push(DamageCommand {
            ship: ship_id,
            target: DamageTarget::Part(part),
            amount,
        });
        if blast {
            for &nb in ship.design().neighbors.get(part as usize) {
                self.inputs.damage.push(DamageCommand {
                    ship: ship_id,
                    target: DamageTarget::Part(nb),
                    amount: 3.0,
                });
            }
        }
    }
}

fn nth_alive(ship: &Ship, n: u32) -> PartIdx {
    let mut k = 0;
    for (i, p) in ship.parts().iter().enumerate() {
        if p.alive {
            if k == n {
                return i as PartIdx;
            }
            k += 1;
        }
    }
    0
}

/// Free-flight stand-in for the physics layer (NOT physics: no collisions, symplectic Euler): applies each
/// piece's engine wrench and moves its body.
pub fn free_flight(ship: &mut Ship) {
    for p in ship.pieces.iter_mut() {
        if p.mass.mass <= 0.0 {
            continue;
        }
        let rot = Rot::from_angle(p.body.angle);
        let com = p.body.pos + rot.apply(p.mass.com);
        p.body.linvel += rot.apply(p.wrench.force) * (DT / p.mass.mass);
        if p.mass.inertia > 0.0 {
            p.body.angvel += p.wrench.torque * (DT / p.mass.inertia);
        }
        // Move the centre of mass, turn about it, then place the frame origin.
        let com = com + p.body.linvel * DT;
        p.body.angle += p.body.angvel * DT;
        p.body.pos = com - Rot::from_angle(p.body.angle).apply(p.mass.com);
    }
}

/// Runs the scenario on the embedded base content and returns the state hash after every tick.
pub fn run(seed: u64, ticks: u64) -> (Vec<u64>, ScenarioStats) {
    let content = data::base_content();
    let mut sc = Scenario::new(&content, seed).expect("base content compiles");
    let hashes = (0..ticks).map(|_| sc.step()).collect();
    (hashes, sc.stats)
}
