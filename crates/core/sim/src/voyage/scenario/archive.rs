//! The `archive` scenario (GDR-0130): the core band in one voyage. A radiated cluster (every part takes
//! [`ARCHIVE_RADIATION`] heat per second through each side open to space) with the Archive's warden and a sentry at its
//! goal site; the player's ship-mind ships with a hold fly the voyage scenario's script without its kills (they close
//! in, it wakes, they fight). Random hits and blasts break the warden apart, and every [`ARCHIVE_BRIDGE_EVERY`] ticks
//! the bridge to one of its sections is shot through, so its sections split off as vessels of their own and the heart's
//! hull share drops past the `archive_warden` behavior's thresholds: the phase switch (`ScenarioStats::warden_rules`
//! records each rule the warden's vessels ran).

use super::*;

/// The archive scenario's radiation: the core's at the galaxy's centre (`cluster_types/core.ron`).
pub(super) const ARCHIVE_RADIATION: f32 = 0.35;

/// Every this many ticks one of the warden's bridges is shot through (fore, port, starboard in turn).
pub(super) const ARCHIVE_BRIDGE_EVERY: u64 = 240;

/// The warden's bridge cells, in the order they are shot through (`ships/archive_warden.ron`).
const BRIDGES: [(i32, i32); 3] = [(7, 4), (2, 8), (12, 8)];

/// The archive scenario: the plan built in code (a cluster of 700 m with the warden's site 250 m in from the entry,
/// waking as the fleet comes in), the fleet at the entry, the first the focus.
pub(super) fn archive_scenario(content: &Content, seed: u64) -> Result<Scenario, String> {
    let compile = |id: &str| {
        let bp = content.ships.get(id).ok_or_else(|| format!("the archive scenario needs `{id}`"))?;
        ShipDesign::compile(content, bp).map(Arc::new)
    };
    let (warden, sentry) = (compile("archive_warden")?, compile("precursor_sentry")?);
    let at = |x: f32, y: f32, angle: f32| Body::at(Vec2::new(x, y), angle);
    let radius = 700.0;
    let plan = crate::voyage::VoyagePlan {
        voyage: "archive".into(),
        seed,
        radius,
        entry: at(0.0, -radius, 0.0),
        sites: vec![crate::voyage::SitePlan {
            encounter: None,
            goal: true,
            signal: false,
            core: None,
            pos: Vec2::new(0.0, -250.0),
            wake_radius: 400.0,
            wary: false,
            territory: None,
            ships: vec![
                crate::voyage::Spawn { design: warden, body: at(0.0, -250.0, 3.1), damage: 0.0, seed: 0 },
                crate::voyage::Spawn { design: sentry, body: at(40.0, -240.0, 3.1), damage: 0.0, seed: 0 },
            ],
            wrecks: Vec::new(),
            calls: None,
            called: false,
        }],
        star: None,
        radiation: ARCHIVE_RADIATION,
        rocks: Vec::new(),
    };
    let entry = plan.entry;
    let mut sim = Sim::new(seed);
    sim.set_behaviors(library(content)?);
    sim.start_voyage(plan, Owner::PLAYER, Owner(1));
    let fleet: Vec<Arc<ShipDesign>> = content
        .ships
        .iter()
        .map(|b| ShipDesign::compile(content, b).map(Arc::new))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|d| scenario_owner(d) == Owner::PLAYER && d.parts.iter().any(|p| p.cargo > 0.0))
        .collect();
    let mut collectors = Vec::new();
    for (k, d) in fleet.iter().enumerate() {
        let x = (k as f32 - (fleet.len() as f32 - 1.0) / 2.0) * 25.0;
        collectors.push(sim.spawn_ship(Arc::clone(d), Owner::PLAYER, Body::at(entry.pos + Vec2::new(x, 0.0), entry.angle)));
    }
    if let Some(&first) = collectors.first() {
        sim.command(first, ShipCommand::Focus);
    }
    Ok(Scenario {
        sim,
        stats: ScenarioStats::default(),
        kind: ScenarioKind::Archive,
        inputs: TickInputs::default(),
        collectors,
    })
}

impl Scenario {
    /// Every [`ARCHIVE_BRIDGE_EVERY`] ticks the next of the warden's bridges, if it is still on the ship that holds it,
    /// takes a shot that destroys it.
    pub(super) fn archive_run(&mut self, tick: u64) {
        if tick == 0 || !tick.is_multiple_of(ARCHIVE_BRIDGE_EVERY) {
            return;
        }
        let (x, y) = BRIDGES[((tick / ARCHIVE_BRIDGE_EVERY - 1) % BRIDGES.len() as u64) as usize];
        for s in self.sim.ships().iter().filter(|s| s.design().id == "archive_warden") {
            let alive = s.design().parts.iter().position(|p| p.shape.x == x && p.shape.y == y).is_some_and(|i| s.part(i as u32).alive);
            if alive {
                self.inputs.damage.push(DamageCommand { ship: s.id(), target: DamageTarget::Cell((x, y)), amount: 50.0 });
            }
        }
    }

    /// The rules the warden's vessels run this tick, as bits of `ScenarioStats::warden_rules`.
    pub(super) fn note_warden(&mut self) {
        for s in self.sim.ships().iter().filter(|s| s.design().id == "archive_warden") {
            if let Some(r) = s.behavior().and_then(|b| b.active_rule()) {
                self.stats.warden_rules |= 1 << r.min(31);
            }
        }
    }
}
