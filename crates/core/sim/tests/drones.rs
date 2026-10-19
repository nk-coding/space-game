//! Drones (GDR-0117): hangars launch drones while their vessel has a target, rebuild the ones lost, keep them on a
//! leash; drones die with their hangar, take no mind capacity, stay out of the fleet, and the cap holds.

mod common;

use common::*;
use sim::ship::drones::tuning;
use sim::{Body, DamageTarget, Owner, ShipCommand, ShipId, Sim, SimEvent, TickInputs, Vec2};

/// A hive carrier (owner 1) under its behavior and a Halvex Runner (the player's) 150 m ahead of it.
fn carrier_fight(seed: u64) -> (Sim, ShipId, ShipId) {
    let c = content();
    let mut w = Sim::new(seed);
    let carrier = w.spawn_ship(design(&c, "swarm_carrier"), Owner(1), Body::default());
    let runner = w.spawn_ship(design(&c, "halvex_runner"), Owner::PLAYER, Body::at(Vec2::new(0.0, 150.0), 0.0));
    fight(&mut w, &c, carrier);
    (w, carrier, runner)
}

fn drones_of(w: &Sim, carrier: ShipId) -> Vec<ShipId> {
    w.drones().filter(|d| d.drone().is_some_and(|t| t.carrier == carrier)).map(|d| d.id()).collect()
}

#[test]
fn a_carrier_with_a_target_floods_its_drones_and_none_without() {
    let c = content();
    // Without a behavior it aims at nothing: the bays stay full.
    let mut idle = Sim::new(1);
    let carrier = idle.spawn_ship(design(&c, "swarm_carrier"), Owner(1), Body::default());
    idle.spawn_ship(design(&c, "halvex_runner"), Owner::PLAYER, Body::at(Vec2::new(0.0, 150.0), 0.0));
    for _ in 0..90 {
        idle.step(&TickInputs::default());
    }
    assert_eq!(idle.drones().count(), 0);
    assert_eq!(idle.ship(carrier).unwrap().bays().iter().map(|b| b.ready).sum::<u32>(), 12);

    let (mut w, carrier, _) = carrier_fight(1);
    let mut launched = Vec::new();
    for t in 0..120u32 {
        w.step(&TickInputs::default());
        for e in w.events() {
            if let SimEvent::DroneLaunched { ship, carrier: from, .. } = e {
                assert_eq!(*from, carrier);
                launched.push((t, *ship));
            }
        }
    }
    // Twelve drones, six from each hangar, each hangar one every LAUNCH_TICKS.
    assert_eq!(launched.len(), 12, "{launched:?}");
    assert_eq!(drones_of(&w, carrier).len(), 12);
    let last = launched.last().unwrap().0;
    assert!(last >= 5 * tuning::LAUNCH_TICKS, "launched too fast: {launched:?}");
    let s = w.ship(launched[0].1).unwrap();
    assert_eq!(s.team(), Some(Owner(1)));
    assert!(s.behavior().is_some(), "a drone flies its behavior");
    // The fleet leaves drones out, and they take no mind capacity.
    assert_eq!(w.vessels(Owner(1)).count(), 1);
    assert_eq!(w.mind_load(Owner(1)).0, 3);
}

#[test]
fn drones_die_with_their_hangar_and_leave_no_wreck() {
    let (mut w, carrier, _) = carrier_fight(2);
    for _ in 0..120 {
        w.step(&TickInputs::default());
    }
    let before = drones_of(&w, carrier);
    assert_eq!(before.len(), 12);
    let ships = w.ships().len();
    // Shoot the left hangar off: its six drones go at once, without wrecks; the right hangar's stay.
    let left = w.ship(carrier).unwrap().design().hangars[0].part;
    w.apply_damage(carrier, DamageTarget::Part(left), 100.0);
    w.step(&TickInputs::default());
    let lost = w.events().iter().filter(|e| matches!(e, SimEvent::DroneLost { carrier: c, .. } if *c == carrier)).count();
    assert_eq!(lost, 6);
    let after = drones_of(&w, carrier);
    assert_eq!(after.len(), 6);
    assert!(after.iter().all(|d| w.ship(*d).unwrap().drone().unwrap().hangar == 1));
    assert!(w.ships().len() <= ships - 6, "lost drones are removed, not left as wrecks");
    // The carrier's last core gone: every drone dies with it.
    for p in 0..w.ship(carrier).unwrap().design().parts.len() as u32 {
        if w.ship(carrier).unwrap().design().parts[p as usize].logic {
            w.apply_damage(carrier, DamageTarget::Part(p), 100.0);
        }
    }
    w.step(&TickInputs::default());
    assert_eq!(drones_of(&w, carrier).len(), 0);
}

#[test]
fn a_lost_drone_is_rebuilt_in_its_rebuild_time() {
    let (mut w, carrier, _) = carrier_fight(3);
    for _ in 0..120 {
        w.step(&TickInputs::default());
    }
    let rebuild = w.ship(carrier).unwrap().design().hangars[0].rebuild;
    let d = drones_of(&w, carrier).into_iter().find(|d| w.ship(*d).unwrap().drone().unwrap().hangar == 0).unwrap();
    // Its nerve node shot out: it is gone, and the bay starts on a new one.
    let node = w.ship(d).unwrap().design().parts.iter().position(|p| p.logic).unwrap() as u32;
    w.apply_damage(d, DamageTarget::Part(node), 100.0);
    w.step(&TickInputs::default());
    assert!(w.ship(d).is_none());
    let mut ticks = 0;
    while w.ship(carrier).unwrap().bays()[0].ready == 0 && drones_of(&w, carrier).len() < 12 {
        w.step(&TickInputs::default());
        ticks += 1;
        assert!(ticks < 2000, "never rebuilt");
    }
    let secs = ticks as f32 / sim::TICK_HZ as f32;
    assert!(secs >= rebuild * 0.95 && secs <= rebuild * 1.3, "rebuilt in {secs} s, rebuild {rebuild} s");
}

#[test]
fn a_drone_past_its_leash_comes_back() {
    let (mut w, carrier, runner) = carrier_fight(4);
    for _ in 0..40 {
        w.step(&TickInputs::default());
    }
    let d = drones_of(&w, carrier)[0];
    let leash = w.ship(carrier).unwrap().design().hangars[0].leash;
    // Take the target away, and put the drone far past the leash.
    w.remove_ship(runner);
    let piece = w.ship(d).unwrap().control_piece().unwrap();
    w.teleport(d, piece, Body::at(Vec2::new(leash * 2.0, 0.0), 0.0));
    let dist = |w: &Sim| {
        let at = |s: ShipId| {
            let s = w.ship(s).unwrap();
            s.piece(s.control_piece().unwrap()).unwrap().com_world()
        };
        (at(d) - at(carrier)).length()
    };
    let start = dist(&w);
    for _ in 0..600 {
        w.step(&TickInputs::default());
    }
    assert!(dist(&w) < start * 0.5, "{} of {start} m", dist(&w));
}

#[test]
fn drones_are_never_the_focus() {
    let c = content();
    let mut w = Sim::new(5);
    w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    // A player ship with a drone bay, piloted with a lock on an enemy.
    let bay = custom(
        &c,
        &[
            ("drone_bay", 0, 0, 0, false, None),
            ("fission_reactor", 0, 2, 0, false, None),
            ("logic_core", 1, 2, 0, false, None),
        ],
    );
    let me = w.spawn_ship(bay, Owner::PLAYER, Body::default());
    let foe = w.spawn_ship(design(&c, "automaton_raider"), Owner(1), Body::at(Vec2::new(0.0, 120.0), 0.0));
    w.command(me, ShipCommand::Focus);
    let lock = sim::PilotInput { lock: Some(sim::Target::Ship(foe)), ..Default::default() };
    for _ in 0..90 {
        w.step(&TickInputs { commands: vec![(me, ShipCommand::Pilot(lock))], ..Default::default() });
    }
    let drones = drones_of(&w, me);
    assert_eq!(drones.len(), 3, "the bay's three drones launch at the lock");
    w.command(drones[0], ShipCommand::Focus);
    assert_eq!(w.focus(Owner::PLAYER), Some(me));
    // The carrier lost: the focus does not move to a drone, and the drones die.
    w.remove_ship(me);
    w.step(&TickInputs::default());
    assert_eq!(w.focus(Owner::PLAYER), None);
    assert_eq!(w.drones().count(), 0);
}

#[test]
fn the_world_cap_holds_with_a_full_hive_flood() {
    let c = content();
    let mut w = Sim::new(6);
    // Three hives (72 drones in their bays) round one target.
    let mut hives = Vec::new();
    for k in 0..3 {
        let at = Vec2::new(400.0 * (k as f32 - 1.0), 0.0);
        hives.push(w.spawn_ship(design(&c, "swarm_hive"), Owner(1), Body::at(at, 0.0)));
    }
    w.spawn_ship(design(&c, "halvex_runner"), Owner::PLAYER, Body::at(Vec2::new(0.0, 300.0), 0.0));
    for &h in &hives {
        fight(&mut w, &c, h);
    }
    let mut most = 0;
    for _ in 0..300 {
        w.step(&TickInputs::default());
        most = most.max(w.drones().count());
    }
    assert_eq!(most, tuning::MAX_DRONES);
}

#[test]
fn a_drone_fight_is_deterministic() {
    let run = || {
        let (mut w, _, _) = carrier_fight(9);
        for _ in 0..600 {
            w.step(&TickInputs::default());
        }
        w.state_hash()
    };
    assert_eq!(run(), run());
}
