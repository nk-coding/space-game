//! The vessel model (GDR-0013, GDR-0038, GDR-0044, GDR-0045): split vessels, focus, control capacity and
//! idle parts, mind capacity and dormant vessels, friendly and shielded rams.

mod common;

use std::sync::Arc;

use common::*;
use core::f32::consts::PI;
use sim::{
    Body, DamageTarget, Owner, PartIdx, PowerSetting, PowerTier, RuleStatus, ShipCommand, ShipId,
    SimConfig, SimEvent, TickInputs, Vec2, VesselState,
};

const PLAYER: Owner = Owner::PLAYER;

fn step(w: &mut sim::Sim, n: usize) {
    for _ in 0..n {
        w.step(&TickInputs::default());
    }
}

fn with_behaviors(w: &mut sim::Sim) {
    w.set_behaviors(sim::BehaviorLibrary::new(&content()).expect("base behaviors compile"));
}

/// The runner's centre column: it falls into two halves with a logic core each.
const RUNNER_CENTRE: [(i32, i32); 6] = [(5, 0), (6, 0), (5, 2), (5, 4), (6, 4), (5, 5)];

fn cut(w: &mut sim::Sim, ship: ShipId, cells: &[(i32, i32)]) {
    for &c in cells {
        w.apply_damage(ship, DamageTarget::Cell(c), 100.0);
    }
    w.resolve_breakage();
}

/// Ten rules: more than one logic core's six slots, fewer than the runner's twelve.
const TEN_RULES: &str = "target nearest
when hull < 20% -> flee
when heat > 95% -> kite(150%)
when power < 50% -> hold
when shield_down -> kite(120%)
when target_hull < 15% -> approach(30%)
when target_in_range(130%) and cycle(10s) >= 6s -> orbit(60%)
when target_time > 60s -> orbit(80%, cw)
when not target_in_range(200%) -> approach(67%)
when enemies_near(60m) >= 3 -> flee
always -> approach(67%)";

#[test]
fn a_split_leaves_two_vessels_running_the_behavior_within_their_own_slots() {
    let c = content();
    let mut w = sim::Sim::new(1);
    with_behaviors(&mut w);
    let ten = w.behaviors_mut().add_text("ten", TEN_RULES).unwrap();
    let s = w.spawn_ship(design(&c, "halvex_runner"), PLAYER, Body::default());
    let foe = w.spawn_ship(
        design(&c, "automaton_raider"),
        Owner(1),
        Body::at(Vec2::new(0.0, 80.0), PI),
    );
    w.command(s, ShipCommand::Behavior(Some(ten)));
    w.command(s, ShipCommand::Leader(Some(foe)));
    step(&mut w, 4);
    let st = w.ship(s).unwrap().behavior().unwrap();
    assert_eq!(st.slots(), 12);
    assert!(!st.rules().contains(&RuleStatus::NoSlot));
    assert_eq!(st.target(), Some(foe));

    cut(&mut w, s, &RUNNER_CENTRE);
    let t = w.ships().last().unwrap().id();
    assert_ne!(t, foe);
    let formed: Vec<_> = w
        .events()
        .iter()
        .filter(|e| matches!(e, SimEvent::VesselFormed { .. }))
        .collect();
    assert_eq!(formed.len(), 1, "{formed:?}");
    let new = w.ship(t).unwrap();
    assert_eq!(new.team(), Some(PLAYER));
    assert_eq!(new.leader(), Some(foe), "the leader is inherited too");
    let b = new.behavior().expect("the behavior is inherited");
    assert_eq!(b.id(), ten);
    assert_eq!(b.target(), Some(foe), "same target");
    // A few thinks later each half runs the first six rules: the lowest four have no slot (GDR-0035).
    step(&mut w, 4);
    for id in [s, t] {
        let st = w.ship(id).unwrap().behavior().unwrap();
        assert_eq!(st.slots(), 6, "{id:?}");
        assert_eq!(
            &st.rules()[6..],
            &[RuleStatus::NoSlot; 4],
            "{id:?}: {:?}",
            st.rules()
        );
        assert!(!st.rules()[..6].contains(&RuleStatus::NoSlot));
    }
    // Both are vessels of the player: two entries in the fleet list, a core each.
    let fleet: Vec<_> = w.vessels(PLAYER).collect();
    assert_eq!(fleet.len(), 2);
    assert!(
        fleet
            .iter()
            .all(|v| v.cores == 1 && v.state == VesselState::Running)
    );
    assert_eq!(fleet.iter().map(|v| v.ship).collect::<Vec<_>>(), vec![s, t]);
}

#[test]
fn a_raider_shot_in_half_can_be_two_raiders() {
    // Enemies follow the same rules: a raider design with two cores in two halves (left and right column,
    // linked by the nose row only) breaks into two vessels when the nose goes.
    let c = content();
    let d = custom(
        &c,
        &[
            ("armor_block", 0, 0, 0, false, Some("automaton_plate")),
            ("automaton_cannon", 1, 0, 0, false, None),
            ("armor_block", 2, 0, 0, false, Some("automaton_plate")),
            ("automaton_core", 0, 1, 0, false, None),
            ("automaton_core", 2, 1, 0, false, None),
            ("thruster", 0, 2, 0, false, None),
            ("thruster", 2, 2, 0, false, None),
        ],
    );
    let mut w = sim::Sim::new(1);
    with_behaviors(&mut w);
    let s = w.spawn_ship(d, Owner(1), Body::default());
    let raider = w.behaviors().id("raider_basic");
    w.command(s, ShipCommand::Behavior(raider));
    cut(&mut w, s, &[(1, 0)]);
    let fleet: Vec<_> = w.vessels(Owner(1)).collect();
    assert_eq!(fleet.len(), 2, "{fleet:?}");
    for v in &fleet {
        assert_eq!(w.ship(v.ship).unwrap().behavior().map(|b| b.id()), raider);
    }
}

#[test]
fn the_focus_is_piloted_and_moves_on_when_it_loses_its_cores() {
    let c = content();
    let mut w = sim::Sim::new(1);
    with_behaviors(&mut w);
    let skirmisher = w.behaviors().id("skirmisher");
    let a = w.spawn_ship(design(&c, "halvex_runner"), PLAYER, Body::default());
    let small = w.spawn_ship(
        design(&c, "sample_early"),
        PLAYER,
        Body::at(Vec2::new(-40.0, 0.0), 0.0),
    );
    let b = w.spawn_ship(
        design(&c, "halvex_runner"),
        PLAYER,
        Body::at(Vec2::new(40.0, 0.0), 0.0),
    );
    for s in [a, small, b] {
        w.command(s, ShipCommand::Behavior(skirmisher));
    }
    assert_eq!(w.focus(PLAYER), None, "no focus until one is given");
    w.step(&TickInputs {
        commands: vec![(a, ShipCommand::Focus)],
        ..Default::default()
    });
    assert_eq!(w.focus(PLAYER), Some(a));
    assert!(w.events().contains(&SimEvent::FocusChanged {
        owner: PLAYER,
        from: None,
        to: Some(a)
    }));
    let ship_a = w.ship(a).unwrap();
    assert!(ship_a.behavior().is_none(), "the focus is piloted");
    assert_eq!(ship_a.parked_behavior().map(|b| b.id()), skirmisher);
    assert!(w.vessel(a).unwrap().focus);

    // Both cores of the focus go: it is salvage now, and the focus moves to the heaviest running vessel
    // (the other runner, not the lighter sample ship with the lower id).
    let d = w.ship(a).unwrap().design().clone();
    for (x, y) in [(3, 4), (8, 4)] {
        w.apply_damage(a, DamageTarget::Part(find(&d, "logic_core", x, y)), 10.0);
    }
    step(&mut w, 1);
    assert_eq!(w.ship(a).unwrap().team(), None);
    assert_eq!(w.focus(PLAYER), Some(b));
    let ev = w.events();
    assert!(ev.contains(&SimEvent::VesselLost {
        ship: a,
        owner: PLAYER
    }));
    assert!(ev.contains(&SimEvent::FocusChanged {
        owner: PLAYER,
        from: Some(a),
        to: Some(b)
    }));
    assert!(w.ship(b).unwrap().behavior().is_none());
    assert_eq!(
        w.ship(b).unwrap().parked_behavior().map(|b| b.id()),
        skirmisher
    );

    // A focus switch is a command: the vessel left behind runs its behavior again.
    w.step(&TickInputs {
        commands: vec![(small, ShipCommand::Focus)],
        ..Default::default()
    });
    assert_eq!(w.focus(PLAYER), Some(small));
    assert_eq!(w.ship(b).unwrap().behavior().map(|b| b.id()), skirmisher);
    // Salvage cannot take the focus; the last vessel lost leaves the owner without one.
    w.command(a, ShipCommand::Focus);
    assert_eq!(w.focus(PLAYER), Some(small));
    w.remove_ship(b);
    w.remove_ship(small);
    assert_eq!(w.focus(PLAYER), None);
    assert!(w.events().contains(&SimEvent::FocusChanged {
        owner: PLAYER,
        from: Some(small),
        to: None
    }));
}

/// A test pack: a cold core and a core that overheats quickly, each running 4 control; an engine that costs
/// control but draws no power and a reactor that costs no control.
const CORES: &str = r#"[
    (id: "free_thruster", name: "Free thruster", summary: "Test engine without a draw.", category: "propulsion",
     icon: "thruster", size: (1, 1), build_points: 3, mass: 2, hp: 1, control_cost: 1,
     heat: (output: 0.8, limit: 3.2),
     engine: (thrust: 200, rated_speed: 20, gimbal: 5, plume: (length: 3, damage: 1, heat: 1.5))),
    (id: "free_reactor", name: "Free reactor", summary: "Test reactor without a control cost.",
     category: "power", icon: "fission-reactor", size: (1, 1), build_points: 4, mass: 4, hp: 1, control_cost: 0,
     power: (output: 10), heat: (output: 3, limit: 6)),
    (id: "cold_core", name: "Cold core", summary: "Test core.", category: "logic", icon: "logic-core",
     size: (1, 1), build_points: 3, mass: 1, hp: 1, power: (draw: 1, tier: Critical),
     logic: (rule_slots: 6, control_capacity: 4)),
    (id: "hot_core", name: "Hot core", summary: "Test core that overheats.", category: "logic",
     icon: "logic-core", size: (1, 1), build_points: 3, mass: 1, hp: 1, power: (draw: 1, tier: Critical),
     heat: (output: 6, limit: 2), logic: (rule_slots: 6, control_capacity: 4)),
]"#;

fn core_content() -> data::Content {
    let m = data::Pack::from_files([
        (
            "pack.ron",
            r#"(id: "test_cores", name: "Test cores", version: "1.0.0", schema_version: 1, depends: ["base"])"#,
        ),
        ("parts/cores.ron", CORES),
    ])
    .unwrap();
    data::Content::load(&[data::base_pack().unwrap(), m]).unwrap()
}

#[test]
fn parts_over_the_control_capacity_idle_by_power_priority_and_come_back() {
    let c = core_content();
    // Load 6 (three pulse lasers, a shield, a reactor, a thruster) against 4 + 4. The hot core sits among
    // armour, open to the right.
    let mut bp = blueprint(&[
        ("pulse_laser", 0, 0, 0, false, None),
        ("pulse_laser", 1, 0, 0, false, None),
        ("pulse_laser", 2, 0, 0, false, None),
        ("armor_block", 3, 0, 0, false, Some("steel")),
        ("armor_block", 0, 1, 0, false, Some("steel")),
        ("shield_emitter", 1, 1, 0, false, None),
        ("armor_block", 2, 1, 0, false, Some("steel")),
        ("armor_block", 3, 1, 0, false, Some("steel")),
        ("fission_reactor", 0, 2, 0, false, None),
        ("cold_core", 1, 2, 0, false, None),
        ("armor_block", 2, 2, 0, false, Some("steel")),
        ("hot_core", 3, 2, 0, false, None),
        ("armor_block", 0, 3, 0, false, Some("steel")),
        ("thruster", 1, 3, 0, false, None),
        ("armor_block", 2, 3, 0, false, Some("steel")),
        ("armor_block", 3, 3, 0, false, Some("steel")),
    ]);
    bp.size = (4, 4);
    // The drydock setting: the right laser has weight 2 in the gun tier.
    bp.parts[2].power = Some(data::PowerOverride {
        tier: None,
        weight: Some(2.0),
    });
    let d = Arc::new(sim::ShipDesign::compile(&c, &bp).unwrap());
    let (l0, l1, l2) = (0 as PartIdx, 1, 2);
    let hot = find(&d, "hot_core", 3, 2);
    let mut w = sim::Sim::new(1);
    let s = w.spawn_ship(d.clone(), PLAYER, Body::default());
    // The guns fire only on the ticks that check them (they would overheat otherwise).
    let fire = inputs(s, controls(d.engines.len(), 0.0, true, true));
    let wait = inputs(s, controls(d.engines.len(), 0.0, false, true));
    w.step(&fire);
    let ship = w.ship(s).unwrap();
    assert_eq!(ship.control().unwrap().load, 6);
    assert_eq!(ship.control().unwrap().capacity, 8);
    assert!(ship.parts().iter().all(|p| !p.idle));

    // The hot core overheats: capacity 4, so 2 of the load idles, lowest power priority first: the gun
    // tier, lowest weight first, the higher part index first on a tie (l1, then l0); the weight-2 laser runs.
    let mut ticks = 0;
    while !w.ship(s).unwrap().part(hot).overheated {
        w.step(&wait);
        ticks += 1;
        assert!(ticks < 600, "the hot core never overheated");
    }
    w.step(&fire);
    let ship = w.ship(s).unwrap();
    let cs = ship.control().unwrap();
    assert_eq!((cs.load, cs.capacity, cs.idle), (6, 4, 2));
    let idle: Vec<PartIdx> = (0..d.parts.len() as PartIdx)
        .filter(|&i| ship.part(i).idle)
        .collect();
    assert_eq!(idle, vec![l0, l1]);
    // Idle parts do nothing and draw nothing; the other laser fires.
    for i in [l0, l1] {
        assert_eq!((ship.part(i).power, ship.part(i).work), (0.0, 0.0));
    }
    assert!(ship.part(l2).work > 0.0);
    assert!(ship.is_active(), "the cold core still runs the vessel");

    // Switching the weight-2 laser off (tier Off) puts it first in the idle order: now l2 and l1 idle.
    w.step(&TickInputs {
        controls: vec![(s, controls(d.engines.len(), 0.0, true, true))],
        commands: vec![(
            s,
            ShipCommand::Power {
                part: l2,
                setting: PowerSetting {
                    tier: PowerTier::Off,
                    weight: 2.0,
                },
            },
        )],
        ..Default::default()
    });
    let ship = w.ship(s).unwrap();
    assert!(ship.part(hot).overheated, "still hot two ticks later");
    let idle: Vec<PartIdx> = (0..d.parts.len() as PartIdx)
        .filter(|&i| ship.part(i).idle)
        .collect();
    assert_eq!(idle, vec![l1, l2]);
    assert!(ship.part(l0).work > 0.0);

    // The core cools down: the capacity is back and every part runs again.
    let mut ticks = 0;
    while w.ship(s).unwrap().part(hot).overheated {
        w.step(&wait);
        ticks += 1;
        assert!(ticks < 900, "the hot core never cooled down");
    }
    w.step(&fire);
    let ship = w.ship(s).unwrap();
    assert_eq!(ship.control().unwrap().capacity, 8);
    assert!(ship.parts().iter().all(|p| !p.idle));
    assert!(ship.part(l0).work > 0.0 && ship.part(l1).work > 0.0);
    assert_eq!(ship.part(l2).work, 0.0, "switched off");
    assert!(ship.verify_caches().is_ok());
}

#[test]
fn a_runner_that_loses_a_core_idles_its_last_guns() {
    // Base numbers (docs/sim.md "Vessels"): load 18 against 2 × 16; one core gone leaves 16, so 2 idle: the
    // gun tier, highest part index first: both pulse lasers (cost 1 each), the beams (cost 2) keep firing.
    let c = content();
    let d = design(&c, "halvex_runner");
    let (mut w, s) = world_with(d.clone());
    step(&mut w, 1);
    assert_eq!(w.ship(s).unwrap().control().unwrap().load, 18);
    w.apply_damage(s, DamageTarget::Part(find(&d, "logic_core", 3, 4)), 10.0);
    step(&mut w, 1);
    let ship = w.ship(s).unwrap();
    assert_eq!(ship.control().unwrap().capacity, 16);
    let idle: Vec<&str> = (0..d.parts.len())
        .filter(|&i| ship.part(i as PartIdx).idle)
        .map(|i| d.parts[i].id.as_str())
        .collect();
    assert_eq!(idle, ["pulse_laser", "pulse_laser"]);
}

#[test]
fn over_mind_capacity_the_newest_vessels_go_dormant_and_the_focus_never_does() {
    let c = content();
    let runner = design(&c, "halvex_runner");
    let mut cfg = SimConfig::default();
    cfg.mind_capacity[0] = 4; // two runners of two cores each
    let mut w = sim::Sim::with_config(1, cfg);
    let at = |x: f32| Body::at(Vec2::new(x, 0.0), 0.0);
    let a = w.spawn_ship(runner.clone(), PLAYER, at(0.0));
    let b = w.spawn_ship(runner.clone(), PLAYER, at(40.0));
    let c3 = w.spawn_ship(runner.clone(), PLAYER, at(80.0));
    assert!(w.events().contains(&SimEvent::VesselDormant {
        ship: c3,
        owner: PLAYER
    }));
    let state = |w: &sim::Sim, s: ShipId| w.vessel(s).unwrap().state;
    step(&mut w, 2);
    assert_eq!(state(&w, a), VesselState::Running);
    assert_eq!(state(&w, b), VesselState::Running);
    assert_eq!(state(&w, c3), VesselState::Dormant);
    assert_eq!(w.mind_load(PLAYER), (4, 4));
    // Dormant = dark: no thrust under full throttle, no power, no shields, no control capacity.
    w.step(&inputs(c3, controls(runner.engines.len(), 1.0, true, true)));
    let p = &w.ship(c3).unwrap().pieces()[0];
    assert!(!p.active);
    assert_eq!(p.wrench, sim::Wrench::default());
    assert_eq!(p.power.served, 0.0);
    assert_eq!(p.control.capacity, 0);
    assert!(w.bubbles().iter().all(|bb| bb.ship != c3));

    // The focus never goes dormant: focusing the dormant vessel wakes it, and the newest other running
    // vessel (b) goes dormant to make room.
    w.step(&TickInputs {
        commands: vec![(c3, ShipCommand::Focus)],
        ..Default::default()
    });
    let ev = w.events();
    assert!(ev.contains(&SimEvent::VesselWoken {
        ship: c3,
        owner: PLAYER
    }));
    assert!(ev.contains(&SimEvent::VesselDormant {
        ship: b,
        owner: PLAYER
    }));
    assert_eq!(state(&w, a), VesselState::Running);
    assert_eq!(state(&w, b), VesselState::Dormant);
    assert_ne!(state(&w, c3), VesselState::Dormant);

    // Waking b is a swap: the vessel that has not been woken (a, the oldest) goes dormant.
    w.step(&TickInputs {
        commands: vec![(b, ShipCommand::Wake)],
        ..Default::default()
    });
    assert_eq!(state(&w, a), VesselState::Dormant);
    assert_eq!(state(&w, b), VesselState::Running);
    assert_ne!(state(&w, c3), VesselState::Dormant);

    // Capacity below one vessel: only the focus runs.
    let mut tight = *w.config();
    tight.mind_capacity[0] = 1;
    w.set_config(tight);
    step(&mut w, 1);
    assert_eq!(state(&w, c3), VesselState::Running);
    assert_eq!(state(&w, a), VesselState::Dormant);
    assert_eq!(state(&w, b), VesselState::Dormant);

    // Capacity frees up: dormant vessels wake in claim order (b was woken last, then a).
    let mut roomy = *w.config();
    roomy.mind_capacity[0] = 4;
    w.set_config(roomy);
    step(&mut w, 1);
    assert_eq!(state(&w, b), VesselState::Running);
    assert_eq!(state(&w, a), VesselState::Dormant);
    w.remove_ship(b);
    step(&mut w, 1);
    assert_eq!(state(&w, a), VesselState::Running);
    // AI owners are not limited by the player's capacity.
    let r = design(&c, "automaton_raider");
    let raiders: Vec<ShipId> = (0..5)
        .map(|k| w.spawn_ship(r.clone(), Owner(1), at(200.0 + k as f32 * 20.0)))
        .collect();
    assert!(raiders.iter().all(|&s| !w.ship(s).unwrap().is_dormant()));
}

/// Two raiders head-on at `speed` m/s each.
fn ram(owner_b: Owner, speed: f32) -> (sim::Sim, ShipId, ShipId) {
    let c = content();
    let d = design(&c, "automaton_raider");
    let mut w = sim::Sim::new(1);
    let mv = |pos: Vec2, angle: f32, vy: f32| Body {
        pos,
        angle,
        linvel: Vec2::new(0.0, vy),
        angvel: 0.0,
    };
    let a = w.spawn_ship(d.clone(), Owner(1), mv(Vec2::ZERO, 0.0, speed));
    let b = w.spawn_ship(d, owner_b, mv(Vec2::new(0.0, 12.0), PI, -speed));
    (w, a, b)
}

/// Hit points a ship lost (destroyed parts count in full).
fn hp_lost(w: &sim::Sim, s: ShipId) -> f32 {
    let ship = w.ship(s).unwrap();
    ship.parts()
        .iter()
        .zip(&ship.design().parts)
        .map(|(p, d)| d.hp - if p.alive { p.hp } else { 0.0 })
        .sum()
}

fn impacts(w: &sim::Sim) -> Vec<sim::Contact> {
    w.events()
        .iter()
        .filter_map(|e| match e {
            SimEvent::Impact(c) => Some(*c),
            _ => None,
        })
        .collect()
}

#[test]
fn vessels_of_one_owner_bump_without_ram_damage() {
    let (mut w, a, b) = ram(Owner(1), 12.0);
    let mut contacts = Vec::new();
    for _ in 0..30 {
        w.step(&TickInputs::default());
        contacts.extend(impacts(&w));
    }
    assert!(
        contacts.iter().any(|c| c.impulse > 100.0),
        "a hard bump: {contacts:?}"
    );
    assert!(contacts.iter().all(|c| c.damage == 0.0), "{contacts:?}");
    for s in [a, b] {
        let ship = w.ship(s).unwrap();
        assert!(
            ship.parts()
                .iter()
                .zip(&ship.design().parts)
                .all(|(p, d)| p.alive && p.hp == d.hp)
        );
    }
    // They still bounced off each other.
    let va = w.ship(a).unwrap().pieces()[0].body.linvel;
    assert!(va.y < 0.0, "{va:?}");
}

#[test]
fn a_hard_ram_crushes_inward_past_the_parts_it_destroys() {
    // Head-on at 20 m/s each (about 390 kN·s): ~5.8 hp per side, more than the raider's centre column holds
    // (cannon 1, core 2, thruster 1 hp): the nose is crushed and the damage carries through the core to the
    // tail (GDR-0047). The side columns are untouched.
    let (mut w, a, b) = ram(Owner(2), 20.0);
    let mut hit = None;
    for _ in 0..30 {
        w.step(&TickInputs::default());
        if let Some(c) = impacts(&w).into_iter().find(|c| c.damage > 0.0) {
            hit = Some(c);
            break;
        }
    }
    let c = hit.expect("a ram");
    let t = w.config().physics;
    assert!(c.impulse > 300.0, "{c:?}");
    assert!((c.damage - (c.impulse - t.ram_threshold) * t.ram_damage).abs() < 1e-4);
    let d = w.ship(a).unwrap().design().clone();
    let column: Vec<PartIdx> = (0..3)
        .map(|y| d.grid.occupant((1, y)).unwrap() as PartIdx)
        .collect();
    for s in [a, b] {
        let ship = w.ship(s).unwrap();
        assert!(
            column.iter().all(|&p| !ship.part(p).alive),
            "{s:?}: the centre column is crushed"
        );
        assert!(
            (0..d.parts.len() as PartIdx)
                .filter(|p| !column.contains(p))
                .all(|p| ship.part(p).alive && ship.part(p).hp == d.parts[p as usize].hp),
            "{s:?}: the sides are untouched"
        );
        assert!(ship.team().is_none(), "the core went with the column");
    }
    // Below the threshold a bump is free: 4 m/s each is about 80 kN·s.
    let (mut w, a, b) = ram(Owner(2), 4.0);
    let mut impulses = Vec::new();
    for _ in 0..40 {
        w.step(&TickInputs::default());
        impulses.extend(impacts(&w).into_iter().map(|c| (c.impulse, c.damage)));
    }
    assert!(impulses.iter().any(|&(i, _)| i > 40.0), "{impulses:?}");
    assert!(impulses.iter().all(|&(_, d)| d == 0.0), "{impulses:?}");
    assert_eq!((hp_lost(&w, a), hp_lost(&w, b)), (0.0, 0.0));
}

/// A 3×3 block with a shield emitter in the middle, rammed head-on at 20 m/s by a raider of another owner.
/// Returns the hit points each side lost, the bubble's charge after the ram, the tick's events and the ram
/// damage per side.
fn shielded_ram(shields: bool) -> (f32, f32, f32, Vec<SimEvent>, f32) {
    let c = content();
    let block = custom(
        &c,
        &[
            ("armor_block", 0, 0, 0, false, Some("composite")),
            ("armor_block", 1, 0, 0, false, Some("composite")),
            ("armor_block", 2, 0, 0, false, Some("composite")),
            ("armor_block", 0, 1, 0, false, Some("composite")),
            ("shield_emitter", 1, 1, 0, false, None),
            ("armor_block", 2, 1, 0, false, Some("composite")),
            ("fission_reactor", 0, 2, 0, false, None),
            ("logic_core", 1, 2, 0, false, None),
            ("armor_block", 2, 2, 0, false, Some("composite")),
        ],
    );
    let emitter = find(&block, "shield_emitter", 1, 1);
    let mut w = sim::Sim::new(1);
    let s = w.spawn_ship(block.clone(), PLAYER, Body::default());
    let r = w.spawn_ship(
        design(&c, "automaton_raider"),
        Owner(1),
        Body {
            pos: Vec2::new(0.0, 12.0),
            angle: PI,
            linvel: Vec2::new(0.0, -20.0),
            angvel: 0.0,
        },
    );
    let hold = inputs(s, controls(0, 0.0, false, shields));
    let mut ram = None;
    let mut events = Vec::new();
    for _ in 0..40 {
        w.step(&hold);
        if let Some(c) = impacts(&w).into_iter().find(|c| c.damage > 0.0) {
            ram = Some(c);
            events = w.events().to_vec();
            break;
        }
    }
    let c = ram.expect("a ram");
    let (ks, kr) = if c.a.ship == s {
        (c.a, c.b)
    } else {
        (c.b, c.a)
    };
    assert_eq!((ks.ship, kr.ship), (s, r));
    (
        hp_lost(&w, s),
        hp_lost(&w, r),
        w.ship(s).unwrap().part(emitter).shield,
        events,
        c.damage,
    )
}

#[test]
fn a_shielded_ram_drains_the_bubble_first() {
    let (hull_off, raider_off, charge_off, _, ram_off) = shielded_ram(false);
    let (hull_on, raider_on, charge_on, events, ram_on) = shielded_ram(true);
    assert!(
        ram_off > 1.0 && (ram_on - ram_off).abs() < 0.05 * ram_off,
        "{ram_off} {ram_on}"
    );
    // Without a bubble the block takes the whole ram (crushing on past its nose armour).
    assert!((hull_off - ram_off).abs() < 1e-3, "{hull_off} vs {ram_off}");
    // The bubble covering the contact point takes the kinetic share (half) of the ram; the rest hits the
    // hull. The raider has no bubble and takes the full ram (the block's bubble does not shield the rammer).
    assert!(
        (hull_on - ram_on * 0.5).abs() < 1e-3,
        "{hull_on} vs {ram_on}"
    );
    assert!((raider_on - ram_on).abs() < 1e-3 && (raider_off - ram_off).abs() < 1e-3);
    let cap = 6.0;
    assert_eq!(charge_off, cap, "no bubble, nothing drained");
    assert!((cap - charge_on - ram_on * 0.5).abs() < 1e-3, "{charge_on}");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SimEvent::ShieldHit { absorbed, .. } if *absorbed > 0.0)),
        "{events:?}"
    );
}

/// An idle engine that draws no power (content may give it a control cost without a draw; validation only
/// warns) pushes nothing under raw throttles either (13 Oct 2026 review: only a zero power level kept idle movers
/// still, and such an engine has no level of its own).
#[test]
fn an_idle_engine_without_a_draw_pushes_nothing_under_raw_throttles() {
    let c = core_content();
    // A cold core (capacity 4), a reactor that costs nothing, five engines that cost 1 each: one idles.
    let mut parts: Vec<P> = vec![
        ("free_reactor", 0, 1, 0, false, None),
        ("cold_core", 1, 1, 0, false, None),
    ];
    for x in 0..5 {
        parts.push(("free_thruster", x, 2, 0, false, None));
    }
    let d = custom(&c, &parts);
    let (mut w, s) = world_with(d.clone());
    step(&mut w, 1);
    w.step(&inputs(s, controls(d.engines.len(), 1.0, false, true)));
    let ship = w.ship(s).unwrap();
    let idle = (0..d.parts.len() as PartIdx).filter(|&i| ship.part(i).idle).count();
    assert_eq!(idle, 1);
    let thrust = c.parts.get("free_thruster").unwrap().engine.as_ref().unwrap().thrust;
    let force = ship.pieces()[0].wrench.force.y;
    assert!((force - 4.0 * thrust).abs() < 1e-2, "{force} vs 4 × {thrust}");
}

/// A blueprint in two pieces of equal size, each with a core: the ship's controlling piece is the drydock's main
/// piece (`data::ShipGrid::main_piece`, the earlier piece in row-major order), not the one listed first (13 Oct
/// 2026 review: the sim kept the piece with the lowest part index, so the drydock's numbers could describe the
/// other vessel).
#[test]
fn the_controlling_piece_is_the_drydocks_main_piece() {
    let c = content();
    let parts: Vec<P> = vec![
        ("logic_core", 0, 2, 0, false, None),
        ("fission_reactor", 1, 2, 0, false, None),
        ("logic_core", 0, 0, 0, false, None),
        ("fission_reactor", 1, 0, 0, false, None),
    ];
    let d = custom(&c, &parts);
    let pieces = d.grid.pieces();
    let main = d.grid.main_piece(&pieces, |i| d.parts[i].logic).unwrap();
    let want: Vec<PartIdx> = pieces.parts[main].iter().map(|&p| p as PartIdx).collect();
    assert_eq!(want, vec![2, 3], "the top piece comes first in row-major order");
    let (w, s) = world_with(d.clone());
    let ship = w.ship(s).unwrap();
    assert_eq!(ship.piece(ship.control_piece().unwrap()).unwrap().parts, want);
    assert_eq!(ship.main_piece(), ship.control_piece());
    let r = sim::design_report(&c, &blueprint(&parts), &SimConfig::default()).unwrap();
    assert_eq!(r.pieces[r.main.unwrap()].parts, want);
}

/// A dormant vessel that loses its last logic core is salvage, and salvage is never dormant (13 Oct 2026 review).
#[test]
fn a_dormant_vessel_that_loses_its_cores_is_salvage_not_dormant() {
    let c = content();
    let runner = design(&c, "halvex_runner");
    let mut cfg = SimConfig::default();
    cfg.mind_capacity[0] = 2;
    let mut w = sim::Sim::with_config(1, cfg);
    w.spawn_ship(runner.clone(), PLAYER, Body::default());
    let b = w.spawn_ship(runner.clone(), PLAYER, Body::at(Vec2::new(40.0, 0.0), 0.0));
    assert!(w.ship(b).unwrap().is_dormant());
    for (i, _) in runner.parts.iter().enumerate().filter(|(_, p)| p.logic) {
        w.apply_damage(b, DamageTarget::Part(i as PartIdx), 100.0);
    }
    step(&mut w, 1);
    let ship = w.ship(b).unwrap();
    assert!(!ship.is_vessel() && !ship.is_dormant());
}
