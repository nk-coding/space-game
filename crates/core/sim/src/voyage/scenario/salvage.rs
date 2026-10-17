//! The `salvage` scenario: wrecks spawned as wrecks, collecting, core fragments and a lost ship's cargo.

use super::*;

/// The salvage scenario's collectors pick a new target every this many ticks (staggered per collector).
pub(super) const SALVAGE_RETARGET: u64 = 30;
/// Salvage scenario script: at each of these ticks the cores of the first raider still a vessel are shot out.
pub(super) const SALVAGE_RAIDER_KILLS: [u64; 2] = [120, 240];
/// Salvage scenario script: a cargo part of the second collector is destroyed (its hold sheds).
pub(super) const SALVAGE_HOLD_HIT: u64 = 450;
/// Salvage scenario script: the first collector stops (90 ticks before) and loses its logic cores (its wreck
/// keeps its cargo); the last collector waits until then with an empty hold and then strips that wreck.
pub(super) const SALVAGE_LOST: u64 = 540;

/// The salvage scenario: a wreck of every blueprint ([`scenario_blueprints`]) in a ring (radius 30 m, drifting a
/// little, 20–50 % of their parts gone), one collector per ship-mind blueprint with a hold in a ring outside it
/// (45 m), two raiders 100 m out under their content behavior.
pub(super) fn salvage_scenario(content: &Content, seed: u64) -> Result<Scenario, String> {
    let mut sim = Sim::new(seed);
    sim.set_behaviors(library(content)?);
    let designs: Vec<Arc<ShipDesign>> = scenario_blueprints(content)
        .map(|b| ShipDesign::compile(content, b).map(Arc::new))
        .collect::<Result<_, _>>()?;
    let ring = |k: usize, n: usize, r: f32| {
        let a = k as f32 * (core::f32::consts::TAU / n.max(1) as f32);
        (Vec2::new(cos(a) * r, sin(a) * r), a)
    };
    for (k, d) in designs.iter().enumerate() {
        let (pos, a) = ring(k, designs.len(), 30.0);
        let damage = sim.rng(Stream::Scenario).range_f32(0.2, 0.5);
        let body = Body {
            pos,
            angle: a,
            linvel: Vec2::new(-sin(a) * 0.5, cos(a) * 0.5),
            angvel: 0.05,
        };
        let seed = sim.rng(Stream::Scenario).next_u64();
        sim.spawn_wreck(Arc::clone(d), scenario_owner(d), body, damage, seed);
    }
    let holds: Vec<&Arc<ShipDesign>> = designs
        .iter()
        .filter(|d| scenario_owner(d) == Owner::PLAYER && d.parts.iter().any(|p| p.cargo > 0.0))
        .collect();
    let mut collectors = Vec::new();
    for (k, d) in holds.iter().enumerate() {
        let (pos, a) = ring(k, holds.len(), 45.0);
        collectors.push(sim.spawn_ship(Arc::clone(d), Owner::PLAYER, Body::at(pos, a)));
    }
    if let Some(raider) = designs.iter().find(|d| scenario_owner(d) != Owner::PLAYER) {
        for k in 0..2 {
            let (pos, a) = ring(k, 2, 100.0);
            let body = Body::at(pos, a + core::f32::consts::PI);
            let id = sim.spawn_ship(Arc::clone(raider), Owner(1), body);
            let behavior = sim.behaviors().default_for(raider);
            sim.command(id, ShipCommand::Behavior(behavior));
        }
    }
    Ok(Scenario {
        sim,
        stats: ScenarioStats::default(),
        kind: ScenarioKind::Salvage,
        inputs: TickInputs::default(),
        collectors,
    })
}

/// Damage that destroys every alive part of `ship` that `which` picks.
pub(super) fn destroy_parts(
    ship: &Ship,
    which: impl Fn(&crate::ship::design::PartSpec) -> bool,
) -> Vec<DamageCommand> {
    ship.parts()
        .iter()
        .enumerate()
        .filter(|&(i, p)| p.alive && which(&ship.design().parts[i]))
        .map(|(i, _)| DamageCommand {
            ship: ship.id(),
            target: DamageTarget::Part(i as PartIdx),
            amount: 1000.0,
        })
        .collect()
}

impl Scenario {
    /// The salvage scenario's script for this tick: collectors fly to the nearest salvage, and the scripted
    /// losses (the `SALVAGE_*` ticks).
    pub(super) fn salvage_run(&mut self, tick: u64) {
        let last = self.collectors.len().saturating_sub(1);
        for (n, &c) in self.collectors.iter().enumerate() {
            let brake = Some(crate::flight::maneuver::Maneuver::Brake);
            let target = if n == last && n > 0 {
                match tick < SALVAGE_LOST {
                    true => brake,
                    false => salvage_target(&self.sim, c, self.collectors.first().copied()),
                }
            } else if n == 0 && tick + 90 >= SALVAGE_LOST {
                brake
            } else {
                salvage_target(&self.sim, c, None)
            };
            if (tick + n as u64 * 7).is_multiple_of(SALVAGE_RETARGET)
                && let Some(maneuver) = target
            {
                let fly = ShipCommand::Maneuver {
                    maneuver,
                    fire: false,
                };
                self.inputs.commands.push((c, fly));
            }
        }
        if SALVAGE_RAIDER_KILLS.contains(&tick)
            && let Some(raider) = self.sim.ships().iter().find(|s| s.team() == Some(Owner(1)))
        {
            self.inputs
                .damage
                .extend(destroy_parts(raider, |p| p.logic));
        }
        let collector = |n: usize| self.collectors.get(n).and_then(|&id| self.sim.ship(id));
        if tick == SALVAGE_HOLD_HIT
            && let Some(s) = collector(1)
        {
            let hold = destroy_parts(s, |p| p.cargo > 0.0);
            self.inputs.damage.extend(hold.into_iter().take(1));
        }
        if tick == SALVAGE_LOST
            && let Some(s) = collector(0)
        {
            self.inputs.damage.extend(destroy_parts(s, |p| p.logic));
        }
    }
}

/// Where collector `c` flies: to the nearest salvage piece of ship `prefer` within 150 m, else the nearest loose
/// fragment (to it), else the nearest other salvage piece (close in on it); brake when there is none. `None`
/// when `c` is no vessel.
pub(super) fn salvage_target(
    sim: &Sim,
    c: crate::ShipId,
    prefer: Option<crate::ShipId>,
) -> Option<crate::flight::maneuver::Maneuver> {
    use crate::flight::maneuver::Maneuver;
    let ship = sim.ship(c)?;
    let at = ship.piece(ship.control_piece()?)?.com_world();
    // Rank: pieces of `prefer`, then loose fragments, then other pieces; nearest first in each.
    let mut best: Option<(u8, f32, Maneuver)> = None;
    let mut consider = |rank: u8, pos: Vec2, m: Maneuver| {
        let d = (pos - at).length_squared();
        if d <= 150.0 * 150.0 && best.is_none_or(|(r, b, _)| (rank, d) < (r, b)) {
            best = Some((rank, d, m));
        }
    };
    for l in sim.loose() {
        consider(1, l.pos, Maneuver::Hold { at: l.pos });
    }
    for s in sim.ships() {
        let rank = if Some(s.id()) == prefer { 0 } else { 2 };
        for p in s.pieces().iter().filter(|p| p.salvage()) {
            let target = Target::Piece(s.id(), p.id);
            let aim = crate::flight::AimMode::Auto;
            consider(
                rank,
                p.com_world(),
                Maneuver::Approach {
                    target,
                    range: 0.0,
                    aim,
                },
            );
        }
    }
    Some(best.map_or(Maneuver::Brake, |(_, _, m)| m))
}
