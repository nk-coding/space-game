//! The `grown` scenario (GDR-0099, GDR-0102): the Grown's mechanics in one voyage. A wary grove of two thornbacks
//! (built in code: heart, ram prow, thorn guns, tissue armour) and one of their wrecks beside a hot star; the
//! player's ship-mind ships with a hold and a mender (a heart and tissue of its own) enter at the edge and fly the
//! voyage scenario's script without its kills: they visit the grove (it stays asleep), strip its wreck and scan (it
//! wakes), fight; random hits and blasts give the hearts work, a ram run every [`GROWN_RAM_EVERY`] ticks puts a
//! thornback's prow into a player ship, and the star heats every part open to space.

use super::*;

/// The grown scenario's ram run comes this often (ticks), half-way through each period.
pub(super) const GROWN_RAM_EVERY: u64 = 300;

pub(super) const THORNBACK: &str = "thornback";
pub(super) const MENDER: &str = "mender";

fn place(part: &str, x: i32, y: i32, rot: u8, material: Option<&str>) -> data::Placement {
    data::Placement {
        part: part.into(),
        x,
        y,
        rot,
        flip: false,
        material: material.map(Into::into),
        power: None,
        group: None,
    }
}

fn fixture(id: &str, faction: &str, behavior: Option<&str>, size: (u32, u32), parts: Vec<data::Placement>) -> data::Blueprint {
    data::Blueprint {
        id: id.into(),
        name: id.into(),
        summary: "Determinism-scenario fixture with the Grown's parts.".into(),
        description: None,
        lore: None,
        tags: vec![],
        faction: faction.into(),
        size,
        parts,
        behavior: behavior.map(Into::into),
        assists: None,
        origin: None,
        source: Default::default(),
    }
}

/// A thornback: a ram prow between two thorn guns, the heart behind it in tissue, an automaton core and two
/// thrusters (4 × 5). A test fixture built from base part ids, flying `raider_basic`.
pub(super) fn thornback_blueprint() -> data::Blueprint {
    let t = Some("grown_tissue");
    let parts = vec![
        place("thorn_gun", 0, 0, 0, None),
        place("ram_prow", 1, 0, 0, None),
        place("thorn_gun", 3, 0, 0, None),
        place("armor_block", 0, 1, 0, t),
        place("grown_heart", 1, 1, 0, None),
        place("armor_block", 3, 1, 0, t),
        place("armor_block", 0, 2, 0, t),
        place("armor_block", 3, 2, 0, t),
        place("armor_block", 0, 3, 0, t),
        place("automaton_core", 1, 3, 0, None),
        place("armor_block", 2, 3, 0, t),
        place("armor_block", 3, 3, 0, t),
        place("thruster", 0, 4, 0, None),
        place("armor_block", 1, 4, 0, t),
        place("armor_block", 2, 4, 0, t),
        place("thruster", 3, 4, 0, None),
    ];
    fixture(THORNBACK, "grown", Some("raider_basic"), (4, 5), parts)
}

/// The mender: a player ship with a heart in tissue, a logic core, a reactor, two pulse lasers and two thrusters
/// (4 × 4). A test fixture built from base part ids.
pub(super) fn mender_blueprint() -> data::Blueprint {
    let t = Some("grown_tissue");
    let parts = vec![
        place("pulse_laser", 0, 0, 0, None),
        place("armor_block", 1, 0, 0, t),
        place("armor_block", 2, 0, 0, t),
        place("pulse_laser", 3, 0, 0, None),
        place("logic_core", 0, 1, 0, None),
        place("grown_heart", 1, 1, 0, None),
        place("fission_reactor", 3, 1, 0, None),
        place("armor_block", 0, 2, 0, t),
        place("armor_block", 3, 2, 0, t),
        place("thruster", 0, 3, 0, None),
        place("armor_block", 1, 3, 0, t),
        place("armor_block", 2, 3, 0, t),
        place("thruster", 3, 3, 0, None),
    ];
    fixture(MENDER, "ship_mind", None, (4, 4), parts)
}

/// The grown scenario: the plan built in code (a cluster of 600 m, the grove 450 m in from the entry, so it spawns at
/// once and the fleet passes within its wake radius on the way; the star 1400 m right of the centre, heating 0.4
/// there), the fleet at the entry, the first the focus.
pub(super) fn grown_scenario(content: &Content, seed: u64) -> Result<Scenario, String> {
    let compile = |bp: &data::Blueprint| ShipDesign::compile(content, bp).map(Arc::new);
    let thornback = compile(&thornback_blueprint())?;
    let at = |x: f32, y: f32, angle: f32| Body::at(Vec2::new(x, y), angle);
    let spawn = |body: Body, damage: f32, seed: u64| crate::voyage::Spawn { design: Arc::clone(&thornback), body, damage, seed };
    let radius = 600.0;
    let plan = crate::voyage::VoyagePlan {
        voyage: "grown".into(),
        seed,
        radius,
        entry: at(0.0, -radius, 0.0),
        sites: vec![crate::voyage::SitePlan {
            encounter: None,
            goal: true,
            signal: false,
            core: None,
            pos: Vec2::new(0.0, -150.0),
            wake_radius: 250.0,
            wary: true,
            ships: vec![spawn(at(-10.0, -150.0, 0.0), 0.0, 0), spawn(at(10.0, -150.0, 0.5), 0.0, 0)],
            wrecks: vec![spawn(at(0.0, -180.0, 2.0), 0.3, seed ^ 0x6772_6f77)],
        }],
        star: Some(crate::voyage::Star { pos: Vec2::new(1400.0, 0.0), heat: 0.4, reach: 1400.0 }),
        rocks: Vec::new(),
    };
    let entry = plan.entry;
    let mut sim = Sim::new(seed);
    sim.set_behaviors(library(content)?);
    sim.start_voyage(plan, Owner::PLAYER, Owner(1));
    let mut fleet: Vec<Arc<ShipDesign>> = content
        .ships
        .iter()
        .map(|b| ShipDesign::compile(content, b).map(Arc::new))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|d| scenario_owner(d) == Owner::PLAYER && d.parts.iter().any(|p| p.cargo > 0.0))
        .collect();
    fleet.push(compile(&mender_blueprint())?);
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
        kind: ScenarioKind::Grown,
        inputs: TickInputs::default(),
        collectors,
    })
}
