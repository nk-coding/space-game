//! The EMP burst and overdrive (GDR-0132): a burst fills enemy vessels in reach with static (never friends, never
//! beyond the radius) and recharges; overdrive boosts fire and heat for its seconds, burns parts past their heat
//! limit, then cools down.

mod common;

use common::*;
use sim::ship::overdrive::tuning as od;
use sim::{Body, Owner, ShipCommand, SimEvent, TickInputs, Vec2};

/// An EMP emitter on a reactor and a core.
fn emp_ship() -> Vec<P<'static>> {
    vec![("emp_emitter", 0, 0, 0, false, None), ("fission_reactor", 0, 1, 0, false, None), ("logic_core", 0, 2, 0, false, None)]
}

/// A one-core vessel without shields.
fn raider() -> Vec<P<'static>> {
    vec![("logic_core", 0, 0, 0, false, None), ("fission_reactor", 0, 1, 0, false, None)]
}

fn idle(w: &mut sim::Sim, cmds: Vec<(sim::ShipId, ShipCommand)>) {
    w.step(&TickInputs { commands: cmds, ..Default::default() });
}

#[test]
fn a_burst_blinds_enemies_in_reach_only_and_recharges() {
    let c = content();
    let mut w = sim::Sim::new(3);
    let me = w.spawn_ship(custom(&c, &emp_ship()), Owner::PLAYER, Body::default());
    let near = w.spawn_ship(custom(&c, &raider()), Owner(1), Body::at(Vec2::new(0.0, 100.0), 0.0));
    let far = w.spawn_ship(custom(&c, &raider()), Owner(1), Body::at(Vec2::new(0.0, 200.0), 0.0));
    let friend = w.spawn_ship(custom(&c, &raider()), Owner::PLAYER, Body::at(Vec2::new(30.0, 0.0), 0.0));
    idle(&mut w, vec![]);
    assert!(w.ship(me).unwrap().can_emp());
    idle(&mut w, vec![(me, ShipCommand::Emp)]);
    let bursts = w.events().iter().filter(|e| matches!(e, SimEvent::EmpBurst { struck: 1, .. })).count();
    assert_eq!(bursts, 1, "{:?}", w.events());
    assert!(w.ship(near).unwrap().blinded());
    assert!(!w.ship(far).unwrap().blinded());
    assert!(!w.ship(friend).unwrap().blinded());
    assert!(w.ship(friend).unwrap().jam().is_clear());
    // Recharging: not ready again soon, ready after its cooldown at full power.
    for _ in 0..30 * 5 {
        idle(&mut w, vec![(me, ShipCommand::Emp)]);
        assert!(!w.events().iter().any(|e| matches!(e, SimEvent::EmpBurst { .. })));
    }
    for _ in 0..30 * 22 {
        idle(&mut w, vec![]);
    }
    assert!(w.ship(me).unwrap().can_emp(), "recharged after 27 s");
}

#[test]
fn a_two_core_ship_takes_static_without_going_blind() {
    let c = content();
    let mut w = sim::Sim::new(3);
    let me = w.spawn_ship(custom(&c, &emp_ship()), Owner::PLAYER, Body::default());
    let two = vec![("logic_core", 0, 0, 0, false, None), ("logic_core", 1, 0, 0, false, None), ("fission_reactor", 0, 1, 0, false, None)];
    let t = w.spawn_ship(custom(&c, &two), Owner(1), Body::at(Vec2::new(0.0, 60.0), 0.0));
    idle(&mut w, vec![]);
    idle(&mut w, vec![(me, ShipCommand::Emp)]);
    let s = w.ship(t).unwrap();
    assert!(!s.blinded());
    assert!(s.jam().charge > 2.5 && s.jam().charge < 4.0, "{:?}", s.jam());
}

/// A pulse laser on a reactor and a core, firing.
fn gunner() -> Vec<P<'static>> {
    vec![("pulse_laser", 0, 0, 0, false, None), ("fission_reactor", 0, 1, 0, false, None), ("logic_core", 0, 2, 0, false, None)]
}

/// Ticks of fire with or without an overdrive at the start: (shots fired, the gun's peak heat, events).
fn fire(overdrive: bool, ticks: usize) -> (usize, f32, Vec<SimEvent>) {
    let c = content();
    let mut w = sim::Sim::new(5);
    let me = w.spawn_ship(custom(&c, &gunner()), Owner::PLAYER, Body::default());
    let mut shots = 0;
    let mut peak = 0.0f32;
    let mut events = Vec::new();
    for i in 0..ticks {
        let cmds = if overdrive && i == 1 { vec![(me, ShipCommand::Overdrive)] } else { vec![] };
        w.step(&TickInputs { controls: vec![(me, controls(0, 0.0, true, false))], commands: cmds, ..Default::default() });
        for e in w.events() {
            if matches!(e, SimEvent::ShotFired { .. }) {
                shots += 1;
            }
            if matches!(e, SimEvent::Overdrive { .. } | SimEvent::OverdriveEnded { .. } | SimEvent::PartDestroyed { .. }) {
                events.push(e.clone());
            }
        }
        if let Some(s) = w.ship(me) {
            peak = peak.max(s.parts()[0].heat);
        }
    }
    (shots, peak, events)
}

#[test]
fn overdrive_fires_faster_runs_hotter_and_cools_down() {
    let ticks = 60;
    let (plain, plain_heat, _) = fire(false, ticks);
    let (boosted, boosted_heat, _) = fire(true, ticks);
    let (_, _, events) = fire(true, (od::SECONDS * 30.0) as usize + 10);
    assert!(boosted as f32 >= plain as f32 * 1.3, "{boosted} shots overdriven, {plain} plain");
    assert!(boosted_heat > plain_heat * 1.5, "{boosted_heat} vs {plain_heat}");
    assert!(events.iter().any(|e| matches!(e, SimEvent::Overdrive { .. })));
    assert!(events.iter().any(|e| matches!(e, SimEvent::OverdriveEnded { .. })), "{events:?}");
}

#[test]
fn overdrive_cannot_start_again_while_cooling_down() {
    let c = content();
    let mut w = sim::Sim::new(5);
    let me = w.spawn_ship(custom(&c, &gunner()), Owner::PLAYER, Body::default());
    idle(&mut w, vec![]);
    idle(&mut w, vec![(me, ShipCommand::Overdrive)]);
    assert!(w.ship(me).unwrap().overdriven());
    for _ in 0..(od::SECONDS * 30.0) as usize + 2 {
        idle(&mut w, vec![]);
    }
    let s = w.ship(me).unwrap();
    assert!(!s.overdriven() && s.overdrive().cool > 0.0 && !s.can_overdrive());
    idle(&mut w, vec![(me, ShipCommand::Overdrive)]);
    assert!(!w.ship(me).unwrap().overdriven());
}

#[test]
fn an_overdriven_part_past_its_limit_burns() {
    let c = content();
    let mut w = sim::Sim::new(5);
    let me = w.spawn_ship(custom(&c, &gunner()), Owner::PLAYER, Body::default());
    // Keep firing and overdriving: the laser (limit 3) runs past its limit and burns instead of shutting down.
    let mut lost = false;
    for i in 0..30 * 40 {
        let cmds = if i % 30 == 0 { vec![(me, ShipCommand::Overdrive)] } else { vec![] };
        w.step(&TickInputs { controls: vec![(me, controls(0, 0.0, true, false))], commands: cmds, ..Default::default() });
        let s = w.ship(me).unwrap();
        if s.overdriven() {
            assert!(!s.parts()[0].overheated || !s.parts()[0].alive);
        }
        if !s.parts()[0].alive || s.parts()[0].hp < 1.0 {
            lost = true;
            break;
        }
    }
    assert!(lost, "the overdriven laser should burn past its heat limit");
}
