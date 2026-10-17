//! The `voyage` scenario: a voyage directed by `sim::voyage` (sites spawning, sleeping, waking and clearing,
//! salvage, the recall check, scans with their reveals, bearings and wakes), flown by a scripted fleet.

use super::*;

/// Voyage scenario script: every this many ticks the cores of the first armed awake enemy vessel are shot out.
pub(super) const VOYAGE_KILL_EVERY: u64 = 150;
/// Voyage scenario script: a player ship fights the nearest armed awake enemy within this many metres.
pub(super) const VOYAGE_FIGHT_RANGE: f32 = 300.0;

/// What a player ship of the voyage scenario does next.
pub(super) enum Order {
    /// Fight this enemy, which lies this way (world).
    Fight(crate::ShipId, Vec2),
    /// Fly a maneuver (to salvage or to a site).
    Fly(crate::flight::maneuver::Maneuver),
}

/// The voyage scenario: the content's first voyage (load order) rolled from the seed and started for the player
/// against `Owner(1)`, one ship per ship-mind blueprint with a hold in a row at the entry, the first the focus.
pub(super) fn voyage_scenario(content: &Content, seed: u64) -> Result<Scenario, String> {
    let voyage = content
        .voyages
        .iter()
        .next()
        .ok_or("the voyage scenario needs a voyage")?;
    let plan = crate::voyage::VoyagePlan::roll(content, voyage, seed)?;
    let entry = plan.entry;
    let mut sim = Sim::new(seed);
    sim.set_behaviors(library(content)?);
    sim.start_voyage(plan, Owner::PLAYER, Owner(1));
    let holds: Vec<Arc<ShipDesign>> = content
        .ships
        .iter()
        .map(|b| ShipDesign::compile(content, b).map(Arc::new))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|d| scenario_owner(d) == Owner::PLAYER && d.parts.iter().any(|p| p.cargo > 0.0))
        .collect();
    let mut collectors = Vec::new();
    for (k, d) in holds.iter().enumerate() {
        let x = (k as f32 - (holds.len() as f32 - 1.0) / 2.0) * 25.0;
        let body = Body::at(entry.pos + Vec2::new(x, 0.0), entry.angle);
        collectors.push(sim.spawn_ship(Arc::clone(d), Owner::PLAYER, body));
    }
    if let Some(&first) = collectors.first() {
        sim.command(first, ShipCommand::Focus);
    }
    Ok(Scenario {
        sim,
        stats: ScenarioStats::default(),
        kind: ScenarioKind::Voyage,
        inputs: TickInputs::default(),
        collectors,
    })
}

impl Scenario {
    /// The voyage scenario's script for this tick: the scripted kills (not in the grown scenario, whose hearts need
    /// living ships), then each player ship fights, salvages or
    /// flies on (re-decided every `SALVAGE_RETARGET` ticks, staggered) and scans whenever it decides (the sim
    /// ignores it while its scanner charges or cools down, and for a ship without one).
    pub(super) fn voyage_run(&mut self, tick: u64) {
        let Some(v) = self.sim.voyage() else {
            return;
        };
        // The enemies that matter (GDR-0068): armed and awake.
        let awake = |s: &Ship| self.sim.armed_enemy(Owner::PLAYER, s);
        if self.kind == ScenarioKind::Voyage
            && tick > 0
            && tick.is_multiple_of(VOYAGE_KILL_EVERY)
            && let Some(enemy) = self.sim.ships().iter().find(|s| awake(s))
        {
            self.inputs.damage.extend(destroy_parts(enemy, |p| p.logic));
        }
        // Decide from the world as it is (fights: the foe and the way to it), then roll the pilot input.
        let mut decided: Vec<(crate::ShipId, Order)> = Vec::new();
        for (n, &c) in self.collectors.iter().enumerate() {
            if !(tick + n as u64 * 7).is_multiple_of(SALVAGE_RETARGET) {
                continue;
            }
            let Some(ship) = self.sim.ship(c) else {
                continue;
            };
            let Some(at) = ship
                .control_piece()
                .and_then(|p| ship.piece(p))
                .map(|p| p.com_world())
            else {
                continue;
            };
            let foe = self
                .sim
                .ships()
                .iter()
                .filter(|s| awake(s))
                .filter_map(|s| {
                    let p = s.piece(s.control_piece()?)?.com_world();
                    Some(((p - at).length(), s.id(), p - at))
                })
                .filter(|&(d, _, _)| d <= VOYAGE_FIGHT_RANGE)
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let room = ship.cargo_capacity() > ship.cargo().used();
            let salvage = salvage_target(&self.sim, c, None)
                .filter(|&m| room && m != crate::flight::maneuver::Maneuver::Brake);
            // Else each ship to a site of its own among those not cleared yet.
            let open: Vec<&crate::Site> = v
                .sites()
                .iter()
                .filter(|s| s.state() != crate::SiteState::Cleared)
                .collect();
            let fly = match open.get(n % open.len().max(1)) {
                Some(s) => crate::flight::maneuver::Maneuver::Hold { at: s.pos() },
                None => crate::flight::maneuver::Maneuver::Brake,
            };
            let plan = match (foe, salvage) {
                (Some((_, foe, way)), _) => Order::Fight(foe, way),
                (None, Some(m)) => Order::Fly(m),
                (None, None) => Order::Fly(fly),
            };
            decided.push((c, plan));
        }
        for (c, plan) in decided {
            let cmd = match plan {
                Order::Fight(foe, way) => {
                    let rng = self.sim.rng(Stream::Scenario);
                    let a = crate::math::atan2(way.y, way.x) + rng.range_f32(-0.8, 0.8);
                    ShipCommand::Pilot(PilotInput {
                        stick: Vec2::new(cos(a), sin(a)) * rng.range_f32(0.3, 1.0),
                        lock: Some(Target::Ship(foe)),
                        drift: rng.chance(0.2),
                        fire: crate::flight::FIRE_ALL,
                        aim: crate::flight::AimMode::Auto,
                        ..Default::default()
                    })
                }
                Order::Fly(maneuver) => ShipCommand::Maneuver {
                    maneuver,
                    fire: false,
                },
            };
            self.inputs.commands.push((c, cmd));
            self.inputs.commands.push((c, ShipCommand::Scan));
        }
    }
}
