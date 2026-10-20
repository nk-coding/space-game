//! The static design report (`sim::report`, the drydock's live stats): it agrees with a spawned ship, its
//! steady-state heat with a long stepped run, its turn estimate with stepped turns, and its issues fire for
//! broken designs.

mod common;

use common::*;
use sim::flight::Kin;
use sim::report::{Issue, LoadClass, Severity};
use sim::{ConsumerKind, DesignReport, SimConfig, Stat, design_report};

const BASE_SHIPS: [&str; 4] = [
    "automaton_raider",
    "halvex_runner",
    "sample_early",
    "sample_mid",
];

fn report_of(c: &data::Content, parts: &[P]) -> DesignReport {
    design_report(c, &blueprint(parts), &SimConfig::default()).unwrap()
}

fn base_report(c: &data::Content, id: &str) -> DesignReport {
    design_report(c, c.ships.get(id).unwrap(), &SimConfig::default()).unwrap()
}

fn has(r: &DesignReport, f: impl Fn(&Issue) -> bool) -> bool {
    r.issues.iter().any(f)
}

/// Guards against rule drift: every number the report shares with a running ship is the ship's.
#[test]
fn report_agrees_with_a_spawned_ship() {
    let c = content();
    for id in BASE_SHIPS {
        let r = base_report(&c, id);
        let d = design(&c, id);
        let (mut w, s) = world_with(d.clone());
        {
            let ship = w.ship(s).unwrap();
            let piece = ship.piece(ship.control_piece().unwrap()).unwrap();
            assert_eq!(r.pieces.len(), ship.pieces().len(), "{id}");
            assert!(r.falls_off.is_empty(), "{id}");
            for (i, p) in r.parts.iter().enumerate() {
                assert_eq!(&p.stats, ship.stats(i as u32), "{id} part {i} stats");
                assert_eq!(
                    p.open_sides,
                    ship.part(i as u32).open_sides,
                    "{id} part {i}"
                );
            }
            assert_eq!((r.mass, r.inertia), (piece.mass.mass, piece.mass.inertia));
            for g in &r.guns {
                assert_eq!(g.clear, ship.part(g.part).clear, "{id} gun {}", g.part);
            }
            assert_eq!(r.firepower, piece.firepower(ship.parts()));
            assert_eq!(r.rule_slots, ship.rule_slots());
            // No base ship idles a part, so a fresh ship plans with the same authority.
            assert!(r.control.idle.is_empty());
            let kin = Kin::of(piece, ship.parts());
            assert_eq!(r.flight.thrust, kin.caps, "{id}");
            assert_eq!(r.flight.top_speed, kin.speeds, "{id}");
            assert_eq!(r.flight.alpha, kin.alpha, "{id}");
        }
        // One tick flat out with raw controls: engines at full throttle, guns firing, shields on. Raw controls
        // leave the gyros and RCS pods idle, so the report asks for their draw on top.
        w.step(&inputs(s, controls(d.engines.len(), 1.0, true, true)));
        let ship = w.ship(s).unwrap();
        let piece = ship.piece(ship.control_piece().unwrap()).unwrap();
        assert_eq!(piece.power.supply, r.power.supply, "{id}");
        let turning: f32 = r
            .power
            .consumers
            .iter()
            .filter(|c| c.kind == ConsumerKind::Engine && !d.parts[c.part as usize].is_engine)
            .map(|c| c.want)
            .sum();
        assert!(
            close(piece.power.demand + turning, r.power.demand, 1e-4),
            "{id}: demand {} + {turning} vs {}",
            piece.power.demand,
            r.power.demand
        );
        assert_eq!(
            (piece.control.load, piece.control.capacity),
            (r.control.load, r.control.capacity),
            "{id}"
        );
        assert_eq!(
            r.power.deficit, 0.0,
            "{id}: the base ships power everything"
        );
        assert!(r.power.consumers.iter().all(|c| c.level == 1.0));
    }
}

/// A design that runs everything flat out without moving: two thrusters on one row pushing against each other
/// (exhausts into space), lasers firing into space, a shield, a reactor between radiators, a capacitor.
fn heat_rig() -> data::Blueprint {
    blueprint(&[
        ("pulse_laser", 1, 0, 0, false, None),
        ("beam_laser", 2, 0, 0, false, None),
        ("pulse_laser", 3, 0, 0, false, None),
        ("capacitor", 1, 1, 0, false, None),
        ("shield_emitter", 3, 1, 0, false, None),
        ("thruster", 0, 2, 1, false, None),
        ("radiator", 1, 2, 0, false, None),
        ("fission_reactor", 2, 2, 0, false, None),
        ("logic_core", 3, 2, 0, false, None),
        ("thruster", 4, 2, 3, false, None),
        ("radiator", 1, 3, 0, false, None),
        ("radiator", 2, 3, 0, false, None),
        ("armor_block", 3, 3, 0, false, Some("steel")),
    ])
}

#[test]
fn steady_heat_matches_a_long_stepped_run() {
    let c = content();
    let bp = heat_rig();
    let r = design_report(&c, &bp, &SimConfig::default()).unwrap();
    assert!(
        r.issues
            .iter()
            .all(|i| !matches!(i, Issue::Overheat { .. })),
        "{:?}",
        r.issues
    );
    assert!(r.flight.thrust[2] > 0.0 && r.flight.thrust[2] == r.flight.thrust[3]);
    let d = std::sync::Arc::new(sim::ShipDesign::compile(&c, &bp).unwrap());
    let (mut w, s) = world_with(d.clone());
    let full = controls(d.engines.len(), 1.0, true, true);
    let ticks = 30 * 300;
    for _ in 0..ticks {
        w.step(&inputs(s, full.clone()));
    }
    let ship = w.ship(s).unwrap();
    let piece = ship.piece(ship.control_piece().unwrap()).unwrap();
    assert!(
        piece.body.linvel.length() < 1e-3,
        "it must not move: {:?}",
        piece.body.linvel
    );
    let mut worst = 0.0f32;
    for (i, p) in r.parts.iter().enumerate() {
        let stepped = ship.part(i as u32);
        assert!(!stepped.overheated);
        assert_eq!(p.power, stepped.power, "part {i}: power level at full use");
        let err = (p.heat - stepped.heat).abs();
        worst = worst.max(err / p.heat.max(0.1));
        assert!(
            err <= 0.002 * p.heat.max(0.1),
            "part {i} ({}): report {} vs stepped {}",
            d.parts[i].id,
            p.heat,
            stepped.heat
        );
    }
    println!(
        "steady heat vs {ticks} stepped ticks: peak {:.2} of the limit, worst relative error {worst:.2e}",
        r.heat.peak
    );
}

/// The 90° turn estimate against the stepped agility turn of `tests/speed.rs`, for the base ships and the
/// Halvex Runner with RCS pods (GDR-0055: couples, and differential thrust only as the last resort).
#[test]
fn turn_estimate_matches_stepped_turns() {
    let c = content();
    println!("| Ship | Report 90° | Stepped 90° | Error |");
    let mut designs: Vec<(String, data::Blueprint)> = BASE_SHIPS
        .iter()
        .map(|id| (id.to_string(), c.ships.get(id).unwrap().clone()))
        .collect();
    for pods in [2, 4] {
        designs.push((format!("runner + {pods} RCS pods"), runner_with_pods(&c, pods)));
    }
    for (id, bp) in designs {
        let r = design_report(&c, &bp, &SimConfig::default()).unwrap();
        let est = r.flight.turn_90.unwrap();
        let d = std::sync::Arc::new(sim::ShipDesign::compile(&c, &bp).unwrap());
        let (stepped, _, _) = turn_90(d);
        let stepped = stepped.unwrap();
        println!(
            "| {id} | {est:.3} s | {stepped:.3} s | {:+.1}% |",
            100.0 * (est - stepped) / stepped
        );
        assert!(
            (est - stepped).abs() <= 2.0 / 30.0 + 1e-4,
            "{id}: estimate {est} s vs stepped {stepped} s"
        );
    }
}

#[test]
fn a_design_without_a_logic_core_cannot_fly() {
    let c = content();
    let r = report_of(
        &c,
        &[
            ("fission_reactor", 0, 0, 0, false, None),
            ("thruster", 0, 1, 0, false, None),
        ],
    );
    assert!(has(&r, |i| *i == Issue::NoLogicCore));
    assert!(!r.flight_ready());
    assert_eq!(r.control.capacity, 0);
    assert!(r.control.idle.is_empty(), "nothing runs, so nothing idles");
    // The capability is still shown (as if it ran).
    assert_eq!(r.flight.thrust[0], 200.0);
    // An empty blueprint has no main piece at all.
    let r = report_of(&c, &[]);
    assert_eq!(r.main, None);
    assert!(has(&r, |i| *i == Issue::NoLogicCore) && has(&r, |i| *i == Issue::NoThrust));
}

#[test]
fn unpowered_cores_and_no_thrust_are_blockers() {
    let c = content();
    let r = report_of(
        &c,
        &[
            ("logic_core", 0, 0, 0, false, None),
            ("thruster", 0, 1, 0, false, None),
        ],
    );
    assert!(has(&r, |i| *i == Issue::CoresUnpowered));
    assert!(!r.pieces[0].powered);
    let r = report_of(
        &c,
        &[
            ("fission_reactor", 0, 0, 0, false, None),
            ("logic_core", 0, 1, 0, false, None),
        ],
    );
    // The reactor's blast would take the core beside it: a warning after the blocker.
    assert_eq!(r.issues, vec![Issue::NoThrust, Issue::BlastTakes { part: 0, parts: vec![1] }]);
    assert_eq!(Issue::NoThrust.severity(), Severity::Blocker);
    assert!(
        r.flight.turn_90.is_some(),
        "the core's reaction wheel still turns it"
    );
}

#[test]
fn a_power_deficit_names_the_parts_that_run_short() {
    let c = content();
    let r = report_of(
        &c,
        &[
            ("beam_laser", 0, 0, 0, false, None),
            ("beam_laser", 1, 0, 0, false, None),
            ("beam_laser", 2, 0, 0, false, None),
            ("fission_reactor", 0, 2, 0, false, None),
            ("logic_core", 1, 2, 0, false, None),
            ("thruster", 2, 2, 0, false, None),
        ],
    );
    // 10 supply: the core (1) and the thruster (2) first; the beams (Low, 27) would share 7 at 26% each, below
    // their 50% minimum, so two switch off (highest index first) and the first runs at 7/9.
    assert_eq!(
        (r.power.supply, r.power.demand, r.power.served),
        (10.0, 30.0, 10.0)
    );
    let Some(Issue::PowerDeficit { deficit, short }) = r
        .issues
        .iter()
        .find(|i| matches!(i, Issue::PowerDeficit { .. }))
    else {
        panic!("{:?}", r.issues)
    };
    assert_eq!(*deficit, 20.0);
    assert_eq!(short, &vec![0, 1, 2]);
    assert!(r.flight_ready(), "a deficit is a warning");
    assert!(
        close(r.parts[0].power, 7.0 / 9.0, 1e-6),
        "{}",
        r.parts[0].power
    );
    assert_eq!((r.parts[1].power, r.parts[2].power), (0.0, 0.0));
    assert_eq!(r.parts[5].power, 1.0);
}

#[test]
fn draws_in_bursts_stand_apart_from_full_use() {
    // The starter skiff's scanner draws 3 only while a scan charges (`power.intermittent`): full use leaves it out,
    // so the skiff is not short of power, and the report sums it apart under its words.
    let c = content();
    let r = base_report(&c, "rim_skiff");
    // The scanner (Normal) is served before the lasers (Low): while it charges, they get a little less.
    let scan = sim::report::Burst { words: "while scanning".to_string(), draw: 3.0, cuts: true };
    assert_eq!(r.power.intermittent, vec![scan]);
    assert!(r.power.demand <= r.power.supply, "{} / {}", r.power.demand, r.power.supply);
    assert!(r.power.demand + 3.0 > r.power.supply, "the test needs the skiff close to its supply");
    assert!(!has(&r, |i| matches!(i, Issue::PowerDeficit { .. })), "{:?}", r.issues);
}

#[test]
fn overheating_parts_are_flagged_with_the_peak() {
    let c = content();
    // The scenario's heat-stress ship (three beams on two reactors, no radiators) with a thruster.
    let r = report_of(
        &c,
        &[
            ("beam_laser", 0, 0, 0, false, None),
            ("beam_laser", 1, 0, 0, false, None),
            ("beam_laser", 2, 0, 0, false, None),
            ("fusion_reactor", 0, 2, 0, false, None),
            ("fission_reactor", 2, 2, 0, false, None),
            ("logic_core", 2, 3, 0, false, None),
            ("thruster", 2, 4, 0, false, None),
        ],
    );
    assert_eq!(r.power.deficit, 0.0);
    let Some(Issue::Overheat { parts }) = r
        .issues
        .iter()
        .find(|i| matches!(i, Issue::Overheat { .. }))
    else {
        panic!("{:?}", r.issues)
    };
    assert!(r.heat.peak > 1.0);
    let peak = r.heat.peak_part.unwrap();
    assert!(parts.contains(&peak));
    for &p in parts {
        assert!(r.parts[p as usize].heat_ratio > 1.0);
    }
    assert!(close(r.parts[peak as usize].heat_ratio, r.heat.peak, 1e-6));
}

#[test]
fn blocked_guns_and_burning_plumes_name_the_parts() {
    let c = content();
    let r = report_of(
        &c,
        &[
            ("armor_block", 0, 0, 0, false, Some("steel")),
            ("pulse_laser", 0, 1, 0, false, None),
            ("fission_reactor", 1, 1, 0, false, None),
            ("logic_core", 2, 1, 0, false, None),
            ("thruster", 1, 2, 0, false, None),
            ("armor_block", 1, 3, 0, false, Some("steel")),
        ],
    );
    assert!(has(&r, |i| *i
        == Issue::GunBlocked {
            gun: 1,
            blockers: vec![0]
        }));
    assert!(!r.guns[0].clear && r.firepower == 0.0);
    assert!(has(&r, |i| *i
        == Issue::PlumeBurnsOwnPart {
            engine: 4,
            parts: vec![5]
        }));
    let lane = r.engines[0].lanes[0];
    assert_eq!(
        (lane.burns, lane.reach, lane.dir),
        (Some(5), 0.0, data::Dir::Down)
    );
    assert_eq!(lane.origin, [1.5, 3.0]);
    // A blocked gun draws no power and makes no heat.
    let gun = r.power.consumers.iter().find(|c| c.part == 1).unwrap();
    assert_eq!(gun.want, 0.0);
}

/// The best design of the 60-point skiff evolution (`crates/core/arena/runs/skiff_design_60bp.ron`, 14 Oct 2026):
/// reactors in front of two cannons, slopes in front of two more, lasers facing sideways, two automaton cores
/// (control load 12 on a capacity of 8, so two cannons and the lasers idle).
const SHUTTERED: [P; 20] = [
    ("slope_1x1", 2, 1, 0, true, Some("steel")),
    ("slope_1x1", 5, 1, 0, false, Some("steel")),
    ("automaton_cannon", 3, 1, 0, false, None),
    ("automaton_cannon", 4, 1, 0, true, None),
    ("automaton_cannon", 2, 2, 0, true, None),
    ("automaton_cannon", 5, 2, 0, false, None),
    ("shield_emitter", 4, 2, 3, true, None),
    ("shield_emitter", 3, 2, 1, false, None),
    ("pulse_laser", 6, 2, 1, true, None),
    ("pulse_laser", 1, 2, 3, false, None),
    ("slope_1x1", 0, 3, 2, false, Some("steel")),
    ("slope_1x1", 7, 3, 2, true, Some("steel")),
    ("thruster", 1, 3, 0, false, None),
    ("thruster", 6, 3, 0, true, None),
    ("automaton_core", 4, 3, 0, true, None),
    ("automaton_core", 3, 3, 0, false, None),
    ("fission_reactor", 3, 0, 0, false, None),
    ("fission_reactor", 4, 0, 0, true, None),
    ("radiator", 2, 3, 0, false, None),
    ("radiator", 5, 3, 0, true, None),
];

/// The report counts guns as the sim fires them, also for blocked and idle guns (no base ship has either): the
/// evolved design above has firepower 0 and fires nothing, and after the sim shoots a reactor off (its chain
/// reaction takes more, GDR-0084), the report of the blueprint without the destroyed parts counts exactly the guns
/// that then fire. (The design won its fights because the enemies shoot its line of fire clear, not because
/// blocked guns fire.)
#[test]
fn report_counts_blocked_and_idle_guns_as_the_sim_fires_them() {
    let c = content();
    let fire = |s| inputs(s, controls(2, 0.0, true, true));
    let d = custom(&c, &SHUTTERED);
    // As built: every cannon blocked, the lasers (facing out sideways) idle.
    let r = report_of(&c, &SHUTTERED);
    assert_eq!(r.firepower, 0.0);
    // Latent firepower (the evolution archive's): every gun as if clear and powered, four cannons and two lasers.
    assert!((r.latent_firepower() - (4.0 * 1.2 + 2.0 * 1.0)).abs() < 1e-5, "{}", r.latent_firepower());
    assert_eq!(r.guns.iter().filter(|g| !g.clear).count(), 4);
    assert_eq!((r.control.load, r.control.capacity), (12, 8));
    let (mut w, s) = world_with(d.clone());
    w.step(&fire(s));
    let ship = w.ship(s).unwrap();
    for g in &r.guns {
        assert_eq!(g.clear, ship.part(g.part).clear, "gun {}", g.part);
        assert_eq!(r.control.idle.contains(&g.part), ship.part(g.part).idle, "gun {}", g.part);
    }
    assert!(ship.pieces()[0].guns.iter().all(|g| g.output == 0.0));
    assert!(!w.events().iter().any(|e| matches!(e, sim::SimEvent::ShotFired { .. })));

    // The left reactor shot off in the sim: its blast sets off the right one, and the two take both centre cannons
    // and both slopes with them (1 hit point each; the steel slopes take half the blast, still more than their 0.5)
    // and, two cells down behind the cannons (GDR-0085: 1.5 m), both shield emitters, whose blasts take the two
    // outer cannons beside them.
    w.apply_damage(s, sim::DamageTarget::Cell((3, 0)), 10.0);
    w.step(&fire(s));
    let ship = w.ship(s).unwrap();
    let gone: Vec<(i32, i32)> = (0..d.parts.len())
        .filter(|&i| !ship.part(i as sim::PartIdx).alive)
        .map(|i| (d.parts[i].shape.x, d.parts[i].shape.y))
        .collect();
    assert_eq!(
        gone,
        [(2, 1), (5, 1), (3, 1), (4, 1), (2, 2), (5, 2), (4, 2), (3, 2), (3, 0), (4, 0)]
    );
    let rest: Vec<P> = SHUTTERED
        .iter()
        .copied()
        .filter(|p| !gone.contains(&(p.1, p.2)))
        .collect();
    let r = report_of(&c, &rest);
    let reduced = custom(&c, &rest);
    let piece = ship.piece(ship.control_piece().unwrap()).unwrap();
    assert!(r.firepower > 0.0);
    assert_eq!(r.firepower, piece.firepower(ship.parts()));
    assert_eq!(r.latent_firepower(), r.firepower, "every gun left works");
    let mut working = [0; 2];
    for g in &r.guns {
        let shape = &reduced.parts[g.part as usize].shape;
        let part = find(&d, &reduced.parts[g.part as usize].id, shape.x, shape.y);
        let works = g.clear && !r.control.idle.contains(&g.part);
        let fires = piece.guns.iter().find(|m| m.part == part).unwrap().output > 0.0;
        assert_eq!(works, fires, "gun at {},{}", shape.x, shape.y);
        working[works as usize] += 1;
    }
    // With eight parts fewer the control load fits the capacity: the two lasers left both work.
    assert_eq!(working, [0, 2]);
}

/// The report's blasts are the sim's (GDR-0084, GDR-0085): a blast without a chain destroys in a world what the
/// report says and leaves the rest with what it says; armour on the line covers; a reactor beside a gun warns.
#[test]
fn blasts_agree_with_the_sim() {
    let c = content();
    let rig: [P; 5] = [
        ("fission_reactor", 1, 0, 0, false, None),
        ("pulse_laser", 2, 0, 0, false, None),
        ("armor_block", 1, 1, 0, false, Some("steel")),
        ("logic_core", 1, 2, 0, false, None),
        ("thruster", 0, 2, 0, false, None),
    ];
    let r = report_of(&c, &rig);
    assert_eq!(r.blasts.len(), 2, "the reactor and the thruster");
    let b = &r.blasts[0];
    assert_eq!((b.part, b.centre, b.damage, b.radius), (0, [1.5, 0.5], 1.5, 1.5));
    let hit = |p: u32| b.hits.iter().find(|h| h.part == p).unwrap();
    // The laser beside it goes; the steel block takes half, and lets half through to the core behind it.
    assert!(hit(1).destroys && hit(1).cover.is_empty());
    assert!(!hit(2).destroys && close(hit(2).damage, 0.75, 1e-6));
    assert_eq!(hit(3).cover, [2]);
    assert!(close(hit(3).damage, 0.75, 1e-6) && !hit(3).destroys);
    assert!(has(&r, |i| *i == Issue::BlastTakes { part: 0, parts: vec![1] }));

    for (name, d, r) in BASE_SHIPS
        .iter()
        .map(|id| (*id, design(&c, id), base_report(&c, id)))
        .chain([("shuttered", custom(&c, &SHUTTERED), report_of(&c, &SHUTTERED)), ("rig", custom(&c, &rig), r)])
    {
        for b in &r.blasts {
            let chain = b.hits.iter().any(|h| h.destroys && d.parts[h.part as usize].explosion.is_some());
            if chain {
                continue;
            }
            let (mut w, s) = world_with(d.clone());
            // Resolved at once: a step would let split-off pieces collide with the rest.
            w.apply_damage(s, sim::DamageTarget::Part(b.part), d.parts[b.part as usize].hp);
            w.resolve_breakage();
            let ship = w.ship(s).unwrap();
            for i in 0..d.parts.len() as u32 {
                let part = ship.part(i);
                match b.hits.iter().find(|h| h.part == i) {
                    Some(h) => {
                        assert_eq!(part.alive, !h.destroys, "{name} blast {} part {i}", b.part);
                        if part.alive {
                            assert!(close(part.hp, d.parts[i as usize].hp - h.damage, 1e-5), "{name} blast {} part {i}", b.part);
                        }
                    }
                    None => assert_eq!(part.alive, i != b.part, "{name} blast {} part {i}", b.part),
                }
            }
        }
    }
}

#[test]
fn parts_that_fall_off_and_split_vessels() {
    let c = content();
    let r = report_of(
        &c,
        &[
            ("fission_reactor", 0, 0, 0, false, None),
            ("logic_core", 0, 1, 0, false, None),
            ("thruster", 0, 2, 0, false, None),
            // Not linked: a cored piece (its own vessel) and a loose block.
            ("logic_core", 2, 0, 0, false, None),
            ("armor_block", 2, 1, 0, false, Some("steel")),
            ("armor_block", 4, 2, 0, false, Some("steel")),
        ],
    );
    assert_eq!(r.pieces.len(), 3);
    let main = r.main.unwrap();
    assert_eq!(r.pieces[main].parts, vec![0, 1, 2]);
    assert_eq!(r.falls_off, vec![3, 4, 5]);
    assert!(has(&r, |i| *i
        == Issue::FallsOff {
            parts: vec![3, 4, 5]
        }));
    let vessels: Vec<bool> = r.pieces.iter().map(|p| p.vessel).collect();
    assert_eq!(vessels, vec![true, true, false]);
    assert_eq!(r.parts[3].heat, 0.0);
    assert_eq!(r.parts[3].power, 0.0);
}

#[test]
fn control_over_capacity_idles_the_lowest_priorities() {
    let c = content();
    // One core (capacity 16), a reactor (1) and 16 pulse lasers (1 each): load 17, the last laser idles.
    let mut parts: Vec<P> = vec![
        ("fission_reactor", 0, 1, 0, false, None),
        ("logic_core", 1, 1, 0, false, None),
    ];
    for x in 0..16 {
        parts.push(("pulse_laser", x, 0, 0, false, None));
    }
    let r = report_of(&c, &parts);
    assert_eq!((r.control.load, r.control.capacity), (17, 16));
    let last = 17;
    assert_eq!(r.control.idle, vec![last]);
    assert!(r.parts[last as usize].idle && r.parts[last as usize].power == 0.0);
    assert!(has(&r, |i| *i
        == Issue::ControlOverCapacity {
            load: 17,
            capacity: 16,
            idle: vec![last]
        }));
    let row = r.power.consumers.iter().find(|c| c.part == last).unwrap();
    assert_eq!(row.want, 0.0, "idle parts ask for nothing");
    // The idle laser never fires: it counts for neither the report's firepower nor the running ship's.
    let dps = r.guns[0].dps;
    assert!(close(r.firepower, 15.0 * dps, 1e-4), "{} vs 15 × {dps}", r.firepower);
    let (mut w, s) = world_with(custom(&c, &parts));
    w.step(&sim::TickInputs::default());
    let ship = w.ship(s).unwrap();
    assert!(ship.part(last).idle);
    let piece = ship.piece(ship.control_piece().unwrap()).unwrap();
    assert!(close(piece.firepower(ship.parts()), r.firepower, 1e-4));
}

/// A part the player set `Off` asks for nothing: it is not "running short" in a deficit (13 Oct 2026 review).
#[test]
fn a_consumer_set_off_does_not_run_short() {
    let c = content();
    let mut bp = blueprint(&[
        ("beam_laser", 0, 0, 0, false, None),
        ("beam_laser", 1, 0, 0, false, None),
        ("beam_laser", 2, 0, 0, false, None),
        ("fission_reactor", 0, 2, 0, false, None),
        ("logic_core", 1, 2, 0, false, None),
        ("thruster", 2, 2, 0, false, None),
    ]);
    bp.parts[0].power = Some(data::PowerOverride {
        tier: Some(sim::PowerTier::Off),
        weight: None,
    });
    let r = design_report(&c, &bp, &SimConfig::default()).unwrap();
    // The two other beams share the 7 left over: below their minimum, the higher index switches off.
    assert_eq!((r.power.demand, r.power.served), (21.0, 10.0));
    let off = r.power.consumers.iter().find(|c| c.part == 0).unwrap();
    assert_eq!((off.want, off.level), (0.0, 0.0));
    let Some(Issue::PowerDeficit { short, .. }) =
        r.issues.iter().find(|i| matches!(i, Issue::PowerDeficit { .. }))
    else {
        panic!("{:?}", r.issues)
    };
    assert_eq!(short, &vec![1, 2]);
}

/// Rule slots come from working logic cores only (GDR-0035: alive, not overheated, powered): a core the player
/// set `Off` gets no power and gives no slots, in the report and in flight.
#[test]
fn an_unpowered_core_gives_no_rule_slots() {
    let c = content();
    let mut bp = c.ships.get("halvex_runner").unwrap().clone();
    let core = bp.parts.iter().position(|p| p.part == "logic_core").unwrap();
    bp.parts[core].power = Some(data::PowerOverride {
        tier: Some(sim::PowerTier::Off),
        weight: None,
    });
    let r = design_report(&c, &bp, &SimConfig::default()).unwrap();
    let slots = c.parts.get("logic_core").unwrap().logic.as_ref().unwrap().rule_slots;
    assert_eq!(r.rule_slots, slots);
    let (mut w, s) = world_with(std::sync::Arc::new(sim::ShipDesign::compile(&c, &bp).unwrap()));
    w.step(&sim::TickInputs::default());
    assert_eq!(w.ship(s).unwrap().rule_slots(), slots);
}

#[test]
fn bonuses_shields_and_build_points() {
    let c = content();
    let r = report_of(
        &c,
        &[
            ("fission_reactor", 0, 0, 0, false, None),
            ("radiator", 1, 0, 0, false, None),
            ("logic_core", 0, 1, 0, false, None),
            ("shield_emitter", 1, 1, 0, false, None),
            ("armor_block", 0, 2, 0, false, Some("reactive")),
            ("thruster", 1, 2, 0, false, None),
        ],
    );
    assert_eq!(r.bonuses.len(), 1);
    let b = r.bonuses[0];
    assert_eq!((b.target, b.source, b.stat), (0, 1, Stat::PowerOutput));
    assert_eq!(b.effect, data::Effect::Add(2.0));
    assert_eq!(r.parts[0].stats.get(Stat::PowerOutput), 12.0);
    assert_eq!(r.power.supply, 12.0);
    assert_eq!(r.shields.len(), 1);
    assert_eq!(
        (r.shields[0].part, r.shields[0].centre, r.shields[0].radius),
        (3, [1.5, 1.5], 2.3)
    );
    let block = c.parts.get("armor_block").unwrap().build_points;
    let reactive = c
        .materials
        .get("reactive")
        .unwrap()
        .armor
        .as_ref()
        .unwrap()
        .build_point_factor;
    let machines: f32 = [
        "fission_reactor",
        "radiator",
        "logic_core",
        "shield_emitter",
        "thruster",
    ]
    .iter()
    .map(|p| c.parts.get(p).unwrap().build_points)
    .sum();
    assert!(close(r.build_points, machines + block * reactive, 1e-4));
}

#[test]
fn yaw_balance_of_an_off_centre_engine() {
    let c = content();
    // A torch drive at the end of a long bar: its torque beats the core's reaction wheel and its own gimbal.
    let parts: Vec<P> = vec![
        ("fission_reactor", 0, 0, 0, false, None),
        ("logic_core", 1, 0, 0, false, None),
        ("armor_block", 2, 0, 0, false, Some("steel")),
        ("armor_block", 3, 0, 0, false, Some("steel")),
        ("armor_block", 4, 0, 0, false, Some("steel")),
        ("armor_block", 5, 0, 0, false, Some("steel")),
        ("torch_drive", 4, 1, 0, false, None),
    ];
    let r = report_of(&c, &parts);
    // Right of the centre of mass, pushing forward: it turns the ship counter-clockwise (to the left).
    assert!(r.flight.yaw > r.flight.yaw_hold, "{:?}", r.flight);
    assert!(!r.flight.yaw_balanced);
    // It is the torque a spawned ship's engines make at full throttle.
    let (w, s) = world_with(custom(&c, &parts));
    let ship = w.ship(s).unwrap();
    let piece = ship.piece(ship.control_piece().unwrap()).unwrap();
    assert!(close(engine_wrench(&piece.thrusters, |_| 1.0).torque, r.flight.yaw, 1e-3));
    // The base ships fly straight.
    for id in BASE_SHIPS {
        let r = base_report(&c, id);
        assert!(
            r.flight.yaw_balanced && r.flight.yaw.abs() < 1e-3,
            "{id}: {}",
            r.flight.yaw
        );
    }
}

/// Flying straight on the stick (or under a lock) steers to a heading, so once the first line is at its limit
/// the flight computer holds a yaw with differential thrust too (the last resort): every row outside the forward
/// group whose torque turns against the yaw counts in `yaw_hold` (13 Oct 2026 review: the report left them out).
/// `yaw_balanced` says whether the allocation (first line, then the last resort with differential thrust) holds
/// full forward thrust's yaw at rest.
#[test]
fn yaw_hold_counts_the_thrusters_that_turn_against_it() {
    use sim::ship::alloc::{Actuation, AllocTable, allocate, assist};
    let c = content();
    // The off-centre torch drive of the test above, two rows lower, with `n` thrusters pushing right in front of
    // the centre of mass (they turn the ship clockwise, against the drive).
    let design = |n: usize| {
        let mut parts: Vec<P> = vec![
            ("fission_reactor", 0, 1, 0, false, None),
            ("fission_reactor", 0, 2, 0, false, None),
            ("logic_core", 1, 2, 0, false, None),
            ("armor_block", 2, 2, 0, false, Some("steel")),
            ("armor_block", 3, 2, 0, false, Some("steel")),
            ("armor_block", 4, 2, 0, false, Some("steel")),
            ("armor_block", 5, 2, 0, false, Some("steel")),
            ("torch_drive", 4, 3, 0, false, None),
        ];
        parts.extend([(5, 1), (4, 1), (5, 0), (4, 0)].iter().take(n).map(|&(x, y)| ("thruster", x, y, 1, false, None)));
        parts
    };
    for (n, balanced) in [(0, false), (4, true)] {
        let parts = design(n);
        let r = report_of(&c, &parts);
        assert_eq!(r.flight.yaw_balanced, balanced, "{n} side thrusters: {:?}", r.flight);
        // One allocation at rest: full forward thrust, no turn asked; what the yaw leaves missing after the
        // last resort.
        let (w, s) = world_with(custom(&c, &parts));
        let ship = w.ship(s).unwrap();
        let p = ship.piece(ship.control_piece().unwrap()).unwrap();
        let th = &p.thrusters;
        let full: Vec<f32> = th.iter().map(|t| t.max_force).collect();
        let tmax: Vec<f32> = p.torque_sources.iter().map(|s| s.max).collect();
        let table = AllocTable::build(th);
        let mut act = Actuation::default();
        act.resize(th.len(), tmax.len());
        let force = sim::Vec2::new(0.0, 1e6);
        let missing = allocate(th, &table, &full, &full, &tmax, force, 0.0, &mut act);
        let missing = assist(th, &table, &full, &full, missing, true, &mut act);
        println!("{n} side thrusters: yaw {:.0}, hold {:.0}, missing {missing:.0}", r.flight.yaw, r.flight.yaw_hold);
        assert_eq!(missing.abs() < 1e-3 * r.flight.yaw, balanced, "missing {missing}");
    }
}

#[test]
fn device_load_grows_with_the_design() {
    let c = content();
    let raider = base_report(&c, "automaton_raider");
    let runner = base_report(&c, "halvex_runner");
    assert!(raider.device.cost_us < runner.device.cost_us);
    assert_eq!(runner.device.class, LoadClass::Light);
    assert_eq!(runner.device.drivers.parts, 56);
    assert!(runner.flight_ready());
}

#[test]
fn unknown_parts_are_an_error() {
    let c = content();
    let bp = blueprint(&[("no_such_part", 0, 0, 0, false, None)]);
    assert!(design_report(&c, &bp, &SimConfig::default()).is_err());
}
