//! Sensors and scans (galaxy plan E3, GDR-0093, GDR-0095; `ship/scan.rs`, `voyage/mod.rs`): a scan charges on the
//! scanner's power, sweeps and cools down; behaviors see only within sensor range and scan with the `scan` action;
//! a voyage's sites stay hidden until sensors or a sweep reach them, a sweep gives far signals a bearing and wakes
//! sites close by; the cluster's edge turns ships back; generated interiors roll into plans.

mod common;

use common::*;
use sim::{Body, Owner, RuleStatus, Scan, ShipCommand, ShipId, Sim, SimEvent, SiteState, TickInputs, Vec2, VoyagePlan};

const ENEMY: Owner = Owner(1);

fn step(w: &mut Sim) -> Vec<SimEvent> {
    w.step(&TickInputs::default());
    w.events().to_vec()
}

fn scanner_of(w: &Sim, ship: ShipId) -> u32 {
    w.ship(ship).unwrap().scanner().expect("a working scanner").0
}

fn put(w: &mut Sim, ship: ShipId, pos: Vec2) {
    let piece = w.ship(ship).unwrap().control_piece().unwrap();
    w.teleport(ship, piece, Body::at(pos, 0.0));
}

#[test]
fn a_scan_charges_on_the_scanners_power_sweeps_and_cools_down() {
    let c = content();
    let spec = c.parts.get("scanner").unwrap().scanner.clone().unwrap();
    let (mut w, me) = world_with(design(&c, "rim_skiff"));
    step(&mut w);
    let part = scanner_of(&w, me);
    assert_eq!(w.ship(me).unwrap().part(part).work, 0.0, "idle: no draw");
    let demand = w.ship(me).unwrap().pieces()[0].power.demand;

    w.command(me, ShipCommand::Scan);
    assert!(matches!(w.ship(me).unwrap().scan(), Scan::Charging { .. }));
    let charge_ticks = (spec.charge * sim::TICK_HZ as f32).round() as u32;
    let mut swept = None;
    for t in 1..=charge_ticks + 2 {
        let events = step(&mut w);
        if let Some(e) = events.iter().find(|e| matches!(e, SimEvent::Scanned { .. })) {
            swept = Some((t, e.clone()));
            break;
        }
        let s = w.ship(me).unwrap();
        assert_eq!(s.part(part).work, 1.0, "charging at full power");
        assert!(s.pieces()[0].power.demand >= demand + 3.0 - 1e-4, "its draw while it charges");
    }
    let (t, e) = swept.expect("it swept");
    assert!(t.abs_diff(charge_ticks) <= 1, "after its charge: {t} ticks");
    let at = w.ship(me).unwrap().pieces()[0].com_world();
    match e {
        SimEvent::Scanned { ship, at: from, range, wake } => {
            assert_eq!((ship, range, wake), (me, spec.range, spec.wake));
            assert!((from - at).length() < 1.0);
        }
        _ => unreachable!(),
    }
    // Cooling: no draw, a second scan is ignored until the cooldown has passed.
    step(&mut w);
    assert!(matches!(w.ship(me).unwrap().scan(), Scan::Cooling { .. }));
    assert_eq!(w.ship(me).unwrap().part(part).work, 0.0);
    w.command(me, ShipCommand::Scan);
    assert!(matches!(w.ship(me).unwrap().scan(), Scan::Cooling { .. }));
    for _ in 0..(spec.cooldown * sim::TICK_HZ as f32).round() as u32 {
        step(&mut w);
    }
    assert_eq!(w.ship(me).unwrap().scan(), Scan::Ready);
    assert!(w.ship(me).unwrap().can_scan());
}

#[test]
fn a_ship_without_a_working_scanner_cannot_scan() {
    let c = content();
    let (mut w, raider) = world_with(design(&c, "automaton_raider"));
    step(&mut w);
    w.command(raider, ShipCommand::Scan);
    assert_eq!(w.ship(raider).unwrap().scan(), Scan::Ready);

    // The skiff's scanner shot away mid-charge: the scan stops, without a sweep or a cooldown.
    let (mut w, me) = world_with(design(&c, "rim_skiff"));
    step(&mut w);
    let part = scanner_of(&w, me);
    w.command(me, ShipCommand::Scan);
    step(&mut w);
    let kill = sim::DamageCommand { ship: me, target: sim::ship::breakage::DamageTarget::Part(part), amount: 100.0 };
    w.step(&TickInputs { damage: vec![kill], ..TickInputs::default() });
    for _ in 0..90 {
        assert!(!step(&mut w).iter().any(|e| matches!(e, SimEvent::Scanned { .. })));
    }
    assert_eq!(w.ship(me).unwrap().scan(), Scan::Ready);
    assert!(!w.ship(me).unwrap().can_scan());
}

/// GDR-0017, GDR-0095: a behavior picks targets only within its sensor range (the logic core's 800 m).
#[test]
fn behaviors_see_only_within_their_sensor_range() {
    let c = content();
    let mut w = Sim::new(1);
    w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    let me = w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, Body::default());
    let sensor = c.parts.get("logic_core").unwrap().sensor.clone().unwrap().range;
    let raider = w.spawn_ship(design(&c, "automaton_raider"), ENEMY, Body::at(Vec2::new(0.0, sensor + 100.0), 0.0));
    let id = w.behaviors_mut().add_text("test", "target nearest\nalways -> face").unwrap();
    w.command(me, ShipCommand::Behavior(Some(id)));
    step(&mut w);
    assert_eq!(w.ship(me).unwrap().sensor_range(), sensor);
    assert_eq!(w.ship(me).unwrap().behavior().unwrap().target(), None, "out of sight");
    put(&mut w, raider, Vec2::new(0.0, sensor - 100.0));
    for _ in 0..3 {
        step(&mut w);
    }
    assert_eq!(w.ship(me).unwrap().behavior().unwrap().target(), Some(raider));
}

/// The `scan` action starts a scan and steps aside: the next rule flies the ship.
#[test]
fn the_scan_action_scans_and_lets_the_next_rule_fly() {
    let c = content();
    let mut w = Sim::new(1);
    w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    let me = w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, Body::default());
    let id = w.behaviors_mut().add_text("test", "always -> scan\nalways -> hold").unwrap();
    w.command(me, ShipCommand::Behavior(Some(id)));
    step(&mut w);
    let st = w.ship(me).unwrap().behavior().unwrap();
    assert_eq!(st.rules()[0], RuleStatus::Scan { started: true });
    assert_eq!(st.active_rule(), Some(1));
    assert!(matches!(w.ship(me).unwrap().scan(), Scan::Charging { .. }));
    for _ in 0..3 {
        step(&mut w);
    }
    let st = w.ship(me).unwrap().behavior().unwrap();
    assert_eq!(st.rules()[0], RuleStatus::Scan { started: false }, "charging: not ready");
    assert_eq!(st.active_rule(), Some(1));
}

/// A voyage's sites stay hidden until the sensors reach them; a signal's site (here the goal, made one) only within
/// the reveal distance. A scan shows every site within its range, gives the hidden goal beyond it a bearing and wakes
/// the sleeping sites close by.
#[test]
fn sites_show_by_sensors_and_scans_and_far_signals_get_bearings() {
    let c = content();
    let mut w = Sim::new(5);
    w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    let mut plan = VoyagePlan::roll(&c, c.voyages.get("corran_reach").unwrap(), 5).unwrap();
    let goal = plan.sites.iter().position(|s| s.goal).unwrap();
    plan.sites[goal].signal = true;
    let entry = plan.entry;
    w.start_voyage(plan, Owner::PLAYER, ENEMY);
    let me = w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, entry);
    w.command(me, ShipCommand::Focus);
    let events = step(&mut w);
    let tuning = w.config().voyage;
    let sensor = w.ship(me).unwrap().sensor_range();
    let at = w.ship(me).unwrap().pieces()[0].com_world();
    let v = w.voyage().unwrap();
    for (k, s) in v.sites().iter().enumerate() {
        let d = (s.pos() - at).length();
        let sees = d <= if s.signal() { tuning.signal_reveal } else { sensor };
        assert_eq!(s.known(), sees, "site {k} at {d} m");
        assert_eq!(events.contains(&SimEvent::SiteRevealed { site: k as u32 }), sees);
    }
    assert!(v.sites()[goal].signal() && !v.sites()[goal].known());

    // Scan from the entry.
    let range = c.parts.get("scanner").unwrap().scanner.clone().unwrap().range;
    w.command(me, ShipCommand::Scan);
    let mut events = Vec::new();
    for _ in 0..90 {
        events = step(&mut w);
        if events.iter().any(|e| matches!(e, SimEvent::Scanned { .. })) {
            break;
        }
    }
    let at = w.ship(me).unwrap().pieces()[0].com_world();
    let v = w.voyage().unwrap();
    for (k, s) in v.sites().iter().enumerate() {
        let d = (s.pos() - at).length();
        if d <= range {
            assert!(s.known(), "site {k} within the sweep");
        }
    }
    let g = &v.sites()[goal];
    let d = (g.pos() - at).length();
    if d > range {
        assert!(!g.known());
        assert!(events.contains(&SimEvent::SiteBearing { site: goal as u32 }));
        let b = v.bearings().last().unwrap();
        assert_eq!(b.site, goal as u32);
        let to = (g.pos() - b.from) * (1.0 / d);
        assert!((b.dir - to).length() < 1e-3, "it points at the goal");
    } else {
        assert!(g.known() && v.bearings().is_empty());
    }

    // A sweep next to a sleeping site (outside its wake radius, within the scanner's) wakes it.
    let k = (0..v.sites().len()).find(|&k| !v.sites()[k].plan().ships.is_empty() && v.sites()[k].wake_radius() < 290.0).unwrap();
    let pos = v.sites()[k].pos();
    put(&mut w, me, pos + Vec2::new(0.0, -290.0));
    step(&mut w);
    assert_eq!(w.voyage().unwrap().sites()[k].state(), SiteState::Asleep);
    for _ in 0..(8 * sim::TICK_HZ) {
        step(&mut w);
        put(&mut w, me, pos + Vec2::new(0.0, -290.0));
    }
    w.command(me, ShipCommand::Scan);
    let mut woke = false;
    for _ in 0..90 {
        put(&mut w, me, pos + Vec2::new(0.0, -290.0));
        if step(&mut w).contains(&SimEvent::SiteWoke { site: k as u32 }) {
            woke = true;
            break;
        }
    }
    assert!(woke, "the sweep woke it");
}

/// Beyond the radius plus the margin a ship is pushed back towards the centre.
#[test]
fn the_edge_turns_ships_back() {
    let c = content();
    let mut w = Sim::new(1);
    let plan = VoyagePlan::roll(&c, c.voyages.get("yard_approach").unwrap(), 1).unwrap();
    w.start_voyage(plan, Owner::PLAYER, ENEMY);
    let edge = w.voyage().unwrap().edge(&w.config().voyage);
    assert_eq!(edge, 600.0 + w.config().voyage.edge_margin);
    let me = w.spawn_ship(
        design(&c, "rim_skiff"),
        Owner::PLAYER,
        Body { pos: Vec2::new(edge + 50.0, 0.0), linvel: Vec2::new(15.0, 0.0), ..Body::default() },
    );
    for _ in 0..(20 * sim::TICK_HZ) {
        step(&mut w);
    }
    let p = &w.ship(me).unwrap().pieces()[0];
    assert!(p.body.linvel.x < 0.0, "turned back: {:?}", p.body.linvel);
    assert!(p.com_world().x < edge + 150.0, "{:?}", p.com_world());
}

/// A generated cluster's interior rolls into a plan with what the career knows (`data::career::Leads`): the places
/// heard signals lead to are signals' sites, a log's place holds its data core in its first wreck, a beacon alone
/// spawns nothing.
#[test]
fn a_generated_interior_rolls_into_a_plan() {
    let c = content();
    let galaxy = c.galaxies.iter().next().unwrap().id.clone();
    let sk = galaxy::generate(&c, &galaxy, 1).unwrap();
    // The trail places of the skeleton that hold a log (the source of signal `j`, a data core in a wreck).
    let log = |(i, j): (u32, u32)| {
        let t = &sk.trails[i as usize];
        t.signals.get(j as usize).is_some_and(|x| c.signal_sources.get(&x.source).unwrap().form == data::SourceForm::Log)
    };
    let k = (0..sk.clusters.len() as u32)
        .find(|&k| galaxy::interior(&c, &sk, k).unwrap().sites.iter().any(|s| s.place.is_some_and(log)))
        .unwrap();
    let interior = galaxy::interior(&c, &sk, k).unwrap();
    let places: Vec<(u32, u32)> = interior.sites.iter().filter_map(|s| s.place).collect();
    let leads = data::career::Leads { signals: places[..1].to_vec(), cores: places.iter().copied().filter(|&p| log(p)).collect() };
    let a = VoyagePlan::roll_interior(&c, "cluster", &interior, &leads, 9).unwrap();
    let b = VoyagePlan::roll_interior(&c, "cluster", &interior, &leads, 9).unwrap();
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
    assert_eq!((a.radius, a.sites.len()), (interior.radius, interior.sites.len()));
    for (p, s) in a.sites.iter().zip(&interior.sites) {
        assert_eq!(p.signal, s.place == Some(places[0]), "only the heard signal's place is a signal's site");
        assert_eq!(p.core, s.place.filter(|&x| log(x)));
        assert_eq!(p.encounter, s.encounter);
        assert!(!p.goal);
        assert!(p.pos.length() <= interior.radius * 1.0001);
        if s.encounter.is_none() {
            assert!(p.ships.is_empty() && p.wrecks.is_empty());
        }
    }

    // Flying to the log's site spawns it: its first wreck carries the data core (one cargo unit, GDR-0093).
    let (site, core) = a.sites.iter().enumerate().find_map(|(n, p)| Some((n, p.core?))).unwrap();
    let at = a.sites[site].pos;
    let mut w = Sim::new(9);
    w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    let entry = a.entry;
    w.start_voyage(a, Owner::PLAYER, ENEMY);
    let me = w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, entry);
    put(&mut w, me, at + Vec2::new(0.0, -300.0));
    step(&mut w);
    let s = &w.voyage().unwrap().sites()[site];
    let wreck = w.ship(*s.wrecks().first().expect("a log lies in a wreck")).unwrap();
    let item = sim::Item::DataCore { trail: core.0, signal: core.1 };
    assert_eq!(wreck.cargo().count(&item), 1);
    assert_eq!(wreck.cargo().items().find(|(i, _)| **i == item).unwrap().1.units, 1);
}
