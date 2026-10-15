//! What the scenarios fly besides the content's ships: the behaviors they assign, the heat-stress ship and the
//! burner (built in code, so the content can change without changing what these cover).

use super::*;

/// Player programs of the behavior scenario (text form, as typed in the editor).
pub(super) const HUNTER: &str = "target weakest(engines)
when hull < 30% -> flee
when target_in_range(110%) and cycle(8s) >= 5s -> orbit(70%, cw)
when closing_in(3s) -> face
always -> approach(60%)";
pub(super) const GUARD: &str = "target threat(weapons)
when under_attack and shield_down -> kite(120%)
when enemies_near(80m) >= 2 -> flee
when target_time > 20s and target_hull > 80% -> orbit(50%)
always -> approach(80%)";
pub(super) const ESCORT: &str = "target locked(power)
when target_in_range and not heat > 90% -> face
always -> approach(70%)";

/// Behavior scenario: raiders fly their faction default; the first ship-mind ship is piloted (random input,
/// the leader); the others cycle through the skirmisher, the hunter, the guard and an escort of the leader.
/// The content's behavior library, with problems as one message.
pub(super) fn library(content: &Content) -> Result<BehaviorLibrary, String> {
    BehaviorLibrary::new(content).map_err(|p| {
        p.iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })
}

pub(super) fn assign_behaviors(content: &Content, sim: &mut Sim) -> Result<(), String> {
    let show = |e: Vec<String>| e.join("; ");
    let mut lib = library(content)?;
    let mut add = |id: &str, text: &str| {
        lib.add_text(id, text)
            .map_err(|e| show(e.iter().map(|e| e.to_string()).collect()))
    };
    let programs = [
        None,
        Some(add("hunter", HUNTER)?),
        Some(add("guard", GUARD)?),
        Some(add("escort", ESCORT)?),
    ];
    sim.set_behaviors(lib);
    let ships: Vec<(crate::ShipId, bool, Option<crate::BehaviorId>)> = sim
        .ships()
        .iter()
        .map(|s| {
            (
                s.id(),
                s.owner() == Owner::PLAYER,
                sim.behaviors().default_for(s.design()),
            )
        })
        .collect();
    let mut leader = None;
    let mut k = 0;
    for (id, mind, default) in ships {
        if !mind {
            sim.command(id, ShipCommand::Behavior(default));
            continue;
        }
        let Some(l) = leader else {
            leader = Some(id); // piloted by the scenario's random input: the player's focus
            sim.command(id, ShipCommand::Focus);
            continue;
        };
        let program = programs[k % programs.len()].or(default);
        sim.command(id, ShipCommand::Behavior(program));
        sim.command(id, ShipCommand::Leader(Some(l)));
        k += 1;
    }
    Ok(())
}

pub(super) const HEAT_STRESS: &str = "heat_stress";

/// The player's mind capacity in the behavior scenario (logic cores): fewer than its ship-mind ships have, so
/// dormant vessels, waking and the focus are part of the determinism check.
pub const BEHAVIOR_MIND_CAPACITY: u32 = 8;

/// Three beam lasers side by side on a fusion and a fission reactor, no radiators: overheats when it fires.
/// A test fixture built from base part ids (skipped if a content pack lacks them).
pub(super) fn heat_stress_blueprint() -> data::Blueprint {
    let p = |part: &str, x: i32, y: i32| data::Placement {
        part: part.into(),
        x,
        y,
        rot: 0,
        flip: false,
        material: None,
        power: None,
        group: None,
    };
    data::Blueprint {
        id: HEAT_STRESS.into(),
        name: "Heat stress".into(),
        summary: "Determinism-scenario fixture that overheats.".into(),
        description: None,
        lore: None,
        tags: vec![],
        faction: "ship_mind".into(),
        size: (3, 4),
        parts: vec![
            p("beam_laser", 0, 0),
            p("beam_laser", 1, 0),
            p("beam_laser", 2, 0),
            p("fusion_reactor", 0, 2),
            p("fission_reactor", 2, 2),
            p("logic_core", 2, 3),
        ],
        behavior: None,
        assists: None,
        source: Default::default(),
    }
}

pub(super) const BURNER: &str = "burner";

/// A burner: two thrusters in one column facing each other (their thrust cancels, so it stays put) with reactive
/// blocks between them, framed by steel. Their plumes burn the blocks from both ends, then each other (GDR-0049).
/// A test fixture built from base part ids (skipped if a content pack lacks them).
pub(super) fn burner_blueprint() -> data::Blueprint {
    let p = |part: &str, x: i32, y: i32, rot: u8, material: Option<&str>| data::Placement {
        part: part.into(),
        x,
        y,
        rot,
        flip: false,
        material: material.map(Into::into),
        power: None,
        group: None,
    };
    let mut parts = vec![
        p("logic_core", 0, 0, 0, None),
        p("fission_reactor", 1, 0, 0, None),
        p("thruster", 1, 1, 0, None),
        p("thruster", 1, 5, 2, None),
    ];
    for y in 2..5 {
        parts.push(p("armor_block", 1, y, 0, Some("reactive")));
    }
    for y in 1..6 {
        parts.push(p("armor_block", 0, y, 0, Some("steel")));
        parts.push(p("armor_block", 2, y, 0, Some("steel")));
    }
    data::Blueprint {
        id: BURNER.into(),
        name: "Burner".into(),
        summary: "Determinism-scenario fixture that burns its own hull.".into(),
        description: None,
        lore: None,
        tags: vec![],
        faction: "ship_mind".into(),
        size: (3, 6),
        parts,
        behavior: None,
        assists: None,
        source: Default::default(),
    }
}

pub(super) fn nth_alive(ship: &Ship, n: u32) -> PartIdx {
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
