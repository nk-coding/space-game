//! The voyage director (`sim::voyage`, GDR-0062): rolling a plan, lazy spawning, sleeping and waking sites,
//! clearing, split-off vessels joining their site, the recall check and the state hash. Plus the slice's enemy
//! ships, which the voyages bring, checked with the design report.

mod common;

use common::*;
use sim::breakage::DamageTarget;
use sim::flight::HelmMode;
use sim::{
    Body, DamageCommand, Maneuver, Owner, PartIdx, ShipId, Sim, SimConfig, SimEvent, SiteState,
    TickInputs, Vec2, VoyagePlan, design_report,
};

const ENEMY: Owner = Owner(1);

fn plan(c: &data::Content, voyage: &str, seed: u64) -> VoyagePlan {
    VoyagePlan::roll(c, c.voyages.get(voyage).unwrap(), seed).unwrap()
}

/// A world with the content's behaviors, a voyage started and the starter ship at its entry (the player's focus).
fn voyage_world(c: &data::Content, voyage: &str, seed: u64) -> (Sim, ShipId) {
    let mut w = Sim::new(seed);
    w.set_behaviors(sim::BehaviorLibrary::new(c).unwrap());
    let p = plan(c, voyage, seed);
    let entry = p.entry;
    w.start_voyage(p, Owner::PLAYER, ENEMY);
    let me = w.spawn_ship(design(c, "rim_skiff"), Owner::PLAYER, entry);
    w.command(me, sim::ShipCommand::Focus);
    (w, me)
}

fn step(w: &mut Sim) -> Vec<SimEvent> {
    w.step(&TickInputs::default());
    w.events().to_vec()
}

/// Puts the player's ship at `pos`, at rest.
fn put(w: &mut Sim, me: ShipId, pos: Vec2) {
    let piece = w.ship(me).unwrap().control_piece().unwrap();
    w.teleport(me, piece, Body::at(pos, 0.0));
}

/// Damage that destroys every logic core of a ship.
fn kill_cores(w: &Sim, ship: ShipId) -> Vec<DamageCommand> {
    let s = w.ship(ship).unwrap();
    (0..s.parts().len())
        .filter(|&i| s.parts()[i].alive && s.design().parts[i].logic)
        .map(|i| DamageCommand {
            ship,
            target: DamageTarget::Part(i as PartIdx),
            amount: 1000.0,
        })
        .collect()
}

/// Damage that destroys every gun of a ship (its cores keep running: disarmed, GDR-0068).
fn kill_guns(w: &Sim, ship: ShipId) -> Vec<DamageCommand> {
    let s = w.ship(ship).unwrap();
    (0..s.parts().len())
        .filter(|&i| s.parts()[i].alive && s.design().parts[i].weapon.is_some())
        .map(|i| DamageCommand {
            ship,
            target: DamageTarget::Part(i as PartIdx),
            amount: 1000.0,
        })
        .collect()
}

/// The goal site of the voyage underway: its index and position.
fn goal(w: &Sim) -> (usize, Vec2) {
    let v = w.voyage().unwrap();
    let k = v.sites().iter().position(|s| s.goal()).unwrap();
    (k, v.sites()[k].pos())
}

fn state(w: &Sim, k: usize) -> SiteState {
    w.voyage().unwrap().sites()[k].state()
}

#[test]
fn plans_are_reproducible_and_follow_the_content() {
    let c = content();
    for v in c.voyages.iter() {
        let a = plan(&c, &v.id, 42);
        let b = plan(&c, &v.id, 42);
        assert_eq!(format!("{a:?}"), format!("{b:?}"), "{}", v.id);
        let other = plan(&c, &v.id, 43);
        assert_ne!(
            a.sites.iter().map(|s| s.pos).collect::<Vec<_>>(),
            other.sites.iter().map(|s| s.pos).collect::<Vec<_>>()
        );
        assert_eq!(a.sites.len() as u32, v.site_count());
        assert_eq!(a.entry.pos, Vec2::new(0.0, -v.radius));
        for seed in 1..=20 {
            let p = plan(&c, &v.id, seed);
            assert_eq!(p.sites.iter().filter(|s| s.goal).count(), 1);
            let mut spots = vec![p.entry.pos];
            for s in &p.sites {
                assert!(s.pos.length() <= v.radius * 1.0001, "{} outside", v.id);
                spots.push(s.pos);
                let e = c.encounters.get(&s.encounter).unwrap();
                assert_eq!(s.wake_radius, e.wake_radius);
                // Counts within the encounter's ranges, per blueprint.
                for (list, entries) in [
                    (&s.ships, e.ships.iter().map(|x| (&x.ship, x.count)).collect::<Vec<_>>()),
                    (&s.wrecks, e.wrecks.iter().map(|x| (&x.ship, x.count)).collect()),
                ] {
                    let total: u32 = entries
                        .iter()
                        .map(|(id, _)| list.iter().filter(|x| &x.design.id == *id).count() as u32)
                        .sum();
                    assert_eq!(total as usize, list.len());
                    for (id, (lo, hi)) in entries {
                        let n = list.iter().filter(|x| &x.design.id == id).count() as u32;
                        assert!((lo..=hi).contains(&n), "{}: {id} ×{n}", s.encounter);
                    }
                }
                for w in &s.wrecks {
                    let want = e.wrecks.iter().find(|x| x.ship == w.design.id).unwrap().damage;
                    assert_eq!(w.damage, want);
                }
                // A site's things lie around it, apart from each other.
                let all: Vec<_> = s.ships.iter().chain(&s.wrecks).collect();
                for (i, a) in all.iter().enumerate() {
                    assert!((a.body.pos - s.pos).length() < 150.0);
                    for b in &all[..i] {
                        assert!((a.body.pos - b.body.pos).length() > 5.0);
                    }
                }
            }
            // The spacing holds for the base voyages: no two spots too close, each site near an earlier one.
            let (near, far) = v.spacing;
            for (i, a) in spots.iter().enumerate() {
                for b in &spots[..i] {
                    assert!((*a - *b).length() >= near * 0.999, "{} seed {seed}", v.id);
                }
            }
            let order: Vec<Vec2> = std::iter::once(p.entry.pos)
                .chain(p.sites.iter().filter(|s| !s.goal).map(|s| s.pos))
                .chain(p.sites.iter().filter(|s| s.goal).map(|s| s.pos))
                .collect();
            for i in 1..order.len() {
                let nearest = order[..i]
                    .iter()
                    .map(|q| (order[i] - *q).length())
                    .fold(f32::INFINITY, f32::min);
                assert!(nearest <= far * 1.001, "{} seed {seed}: {nearest}", v.id);
            }
        }
    }
}

#[test]
fn sites_spawn_lazily_sleep_and_wake() {
    let c = content();
    let (mut w, me) = voyage_world(&c, "yard_approach", 7);
    let events = step(&mut w);
    let spawn = w.config().voyage.spawn_radius;
    let v = w.voyage().unwrap();
    assert_eq!(v.id(), "yard_approach");
    let at = w.ship(me).unwrap().pieces()[0].com_world();
    for s in v.sites() {
        let near = (s.pos() - at).length() <= spawn;
        assert_eq!(s.state() != SiteState::Unspawned, near, "{}", s.encounter());
        if near {
            assert_eq!(s.ships().len(), s.plan().ships.len());
            assert_eq!(s.wrecks().len(), s.plan().wrecks.len());
        }
    }
    assert!(!events.iter().any(|e| matches!(e, SimEvent::SiteWoke { .. })));

    // Fly to each site in turn: it spawns, its ships hold until we are within the wake radius, then they fight.
    for k in 0..w.voyage().unwrap().sites().len() {
        let s = &w.voyage().unwrap().sites()[k];
        let (pos, wake) = (s.pos(), s.wake_radius());
        if s.state() != SiteState::Unspawned && s.state() != SiteState::Asleep {
            continue;
        }
        put(&mut w, me, pos + Vec2::new(0.0, -(wake + 60.0)));
        step(&mut w);
        let s = &w.voyage().unwrap().sites()[k];
        assert!(matches!(s.state(), SiteState::Asleep | SiteState::Quiet));
        for &id in s.ships() {
            let ship = w.ship(id).unwrap();
            assert!(ship.behavior().is_none());
            assert!(matches!(
                ship.helm(),
                HelmMode::Maneuver { maneuver: Maneuver::Hold { .. }, fire: false }
            ));
            assert_eq!(ship.owner(), ENEMY);
        }
        for &id in s.wrecks() {
            assert!(w.ship(id).unwrap().is_burned_out());
        }
        if s.state() == SiteState::Quiet {
            continue;
        }
        put(&mut w, me, pos + Vec2::new(0.0, -(wake - 20.0)));
        let events = step(&mut w);
        assert!(events.contains(&SimEvent::SiteWoke { site: k as u32 }));
        let s = &w.voyage().unwrap().sites()[k];
        assert_eq!(s.state(), SiteState::Awake);
        for &id in s.ships() {
            assert!(w.ship(id).unwrap().behavior().is_some());
        }
    }
}

#[test]
fn a_site_wakes_when_one_of_its_ships_stops_running_and_clears_when_all_are_destroyed() {
    let c = content();
    let (mut w, me) = voyage_world(&c, "yard_approach", 3);
    // Park far from everything but within the spawn radius of the goal.
    let goal = w.voyage().unwrap().sites().iter().position(|s| s.goal()).unwrap();
    let pos = w.voyage().unwrap().sites()[goal].pos();
    put(&mut w, me, pos + Vec2::new(0.0, -500.0));
    step(&mut w);
    let ships = w.voyage().unwrap().sites()[goal].ships().to_vec();
    assert_eq!(w.voyage().unwrap().sites()[goal].state(), SiteState::Asleep);
    assert!(ships.len() >= 3);
    // Shooting the cores out of one sleeper wakes the rest.
    let inputs = TickInputs {
        damage: kill_cores(&w, ships[0]),
        ..Default::default()
    };
    w.step(&inputs);
    assert!(w.events().contains(&SimEvent::SiteWoke { site: goal as u32 }));
    assert!(w.ship(ships[1]).unwrap().behavior().is_some());
    assert!(!w.voyage().unwrap().goal_cleared());
    // Every core gone: the site (the goal) is cleared, once.
    let damage = ships.iter().flat_map(|&s| kill_cores(&w, s)).collect();
    w.step(&TickInputs {
        damage,
        ..Default::default()
    });
    let cleared: Vec<&SimEvent> = w
        .events()
        .iter()
        .filter(|e| matches!(e, SimEvent::SiteCleared { .. }))
        .collect();
    assert_eq!(cleared, [&SimEvent::SiteCleared { site: goal as u32, goal: true }]);
    assert_eq!(w.voyage().unwrap().sites()[goal].state(), SiteState::Cleared);
    assert!(w.voyage().unwrap().goal_cleared());
    let events = step(&mut w);
    assert!(!events.iter().any(|e| matches!(e, SimEvent::SiteCleared { .. })));
}

#[test]
fn vessels_split_off_a_site_ship_belong_to_its_site() {
    let c = content();
    let (mut w, me) = voyage_world(&c, "wardens_gate", 11);
    let goal = w.voyage().unwrap().sites().iter().position(|s| s.goal()).unwrap();
    let pos = w.voyage().unwrap().sites()[goal].pos();
    put(&mut w, me, pos + Vec2::new(0.0, -500.0));
    step(&mut w);
    let site = &w.voyage().unwrap().sites()[goal];
    let warden = *site
        .ships()
        .iter()
        .find(|&&s| w.ship(s).unwrap().design().id == "automaton_warden")
        .unwrap();
    let before = site.ships().len();
    // Cut both bridges: each fist flies on as a vessel of its own (GDR-0013).
    let d = w.ship(warden).unwrap().design().clone();
    let damage = [find(&d, "armor_block", 2, 1), find(&d, "armor_block", 6, 1)]
        .map(|p| DamageCommand {
            ship: warden,
            target: DamageTarget::Part(p),
            amount: 1000.0,
        })
        .to_vec();
    w.step(&TickInputs {
        damage,
        ..Default::default()
    });
    let site = &w.voyage().unwrap().sites()[goal];
    assert_eq!(site.ships().len(), before + 2, "{:?}", site.ships());
    assert!(site.ships().windows(2).all(|p| p[0] < p[1]));
    for &s in &site.ships()[before..] {
        assert!(w.ship(s).unwrap().is_vessel());
        assert_eq!(w.voyage().unwrap().site_of(s), Some(goal));
    }
    // Every ship of the site still runs, so it sleeps on until we come near; clearing waits for the fists too.
    assert_eq!(w.voyage().unwrap().sites()[goal].state(), SiteState::Asleep);
    put(&mut w, me, pos + Vec2::new(0.0, -100.0));
    step(&mut w);
    assert_eq!(w.voyage().unwrap().sites()[goal].state(), SiteState::Awake);
    let fists: Vec<ShipId> = w.voyage().unwrap().sites()[goal].ships()[before..].to_vec();
    let mut damage: Vec<DamageCommand> = w.voyage().unwrap().sites()[goal].ships()[..before]
        .iter()
        .flat_map(|&s| kill_cores(&w, s))
        .collect();
    w.step(&TickInputs {
        damage: damage.clone(),
        ..Default::default()
    });
    assert_eq!(w.voyage().unwrap().sites()[goal].state(), SiteState::Awake);
    damage = fists.iter().flat_map(|&s| kill_cores(&w, s)).collect();
    w.step(&TickInputs {
        damage,
        ..Default::default()
    });
    assert_eq!(w.voyage().unwrap().sites()[goal].state(), SiteState::Cleared);
}

#[test]
fn recall_needs_no_awake_enemy_near() {
    let c = content();
    let (mut w, me) = voyage_world(&c, "yard_approach", 5);
    assert!(w.can_recall(Owner::PLAYER), "nothing awake yet");
    assert!(!w.can_recall(Owner(3)), "an owner without vessels has nothing to recall");
    let goal = w.voyage().unwrap().sites().iter().position(|s| s.goal()).unwrap();
    let pos = w.voyage().unwrap().sites()[goal].pos();
    // Just outside the wake radius, with a sleeper moved to 100 m: sleeping ships do not stop a recall.
    let wake = w.voyage().unwrap().sites()[goal].wake_radius();
    let here = pos + Vec2::new(0.0, -(wake + 30.0));
    put(&mut w, me, here);
    step(&mut w);
    let ships = w.voyage().unwrap().sites()[goal].ships().to_vec();
    let p = w.ship(ships[0]).unwrap().control_piece().unwrap();
    w.teleport(ships[0], p, Body::at(here + Vec2::new(0.0, 100.0), 0.0));
    assert_eq!(w.voyage().unwrap().sites()[goal].state(), SiteState::Asleep);
    assert!(w.can_recall(Owner::PLAYER));
    // Awake (another of them shot), within the recall radius: no recall, for either side.
    w.step(&TickInputs {
        damage: kill_cores(&w, ships[2]),
        ..Default::default()
    });
    assert_eq!(w.voyage().unwrap().sites()[goal].state(), SiteState::Awake);
    assert!(!w.can_recall(Owner::PLAYER));
    assert!(!w.can_recall(ENEMY));
    // Far from every awake enemy again: recall.
    put(&mut w, me, here + Vec2::new(0.0, -400.0));
    assert!(w.can_recall(Owner::PLAYER));
}

#[test]
fn voyage_state_and_tuning_are_in_the_hash() {
    let c = content();
    let (mut a, _) = voyage_world(&c, "halvex_drift", 9);
    let mut b = a.clone();
    for _ in 0..40 {
        a.step(&TickInputs::default());
        b.step(&TickInputs::default());
        assert_eq!(a.state_hash(), b.state_hash());
    }
    // Without the voyage, or with another roll, the hash differs.
    let mut plain = Sim::new(9);
    plain.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    let entry = plan(&c, "halvex_drift", 9).entry;
    let me = plain.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, entry);
    plain.command(me, sim::ShipCommand::Focus);
    let (other, _) = voyage_world(&c, "halvex_drift", 9);
    let mut rolled = Sim::new(9);
    rolled.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    rolled.start_voyage(plan(&c, "halvex_drift", 10), Owner::PLAYER, ENEMY);
    let me = rolled.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, entry);
    rolled.command(me, sim::ShipCommand::Focus);
    assert_ne!(plain.state_hash(), other.state_hash());
    assert_ne!(rolled.state_hash(), other.state_hash());
    let mut config = SimConfig::default();
    config.voyage.spawn_radius = 650.0;
    let mut tuned = other.clone();
    tuned.set_config(config);
    assert_ne!(tuned.state_hash(), other.state_hash());
}

/// The voyages' new enemies fly: one piece, powered, every gun clear, no issue in the design report.
#[test]
fn the_hunter_and_the_warden_are_flight_ready() {
    let c = content();
    for id in ["automaton_hunter", "automaton_warden"] {
        let r = design_report(&c, c.ships.get(id).unwrap(), &SimConfig::default()).unwrap();
        assert!(r.issues.is_empty(), "{id}: {:?}", r.issues);
        assert_eq!(r.pieces.len(), 1);
        assert!(r.guns.iter().all(|g| g.clear));
        assert_eq!(r.power.deficit, 0.0);
        assert!(r.flight.yaw_balanced && r.flight.turn_90.is_some());
    }
}

/// GDR-0068: a site clears when its enemies are destroyed or disarmed. Shooting every gun off an awake pack clears
/// it (the goal, so the voyage too) while their cores still run, and disarmed enemies no longer block the recall.
#[test]
fn disarmed_enemies_clear_their_site_and_do_not_block_the_recall() {
    let c = content();
    let (mut w, me) = voyage_world(&c, "yard_approach", 3);
    let (k, pos) = goal(&w);
    put(&mut w, me, pos + Vec2::new(0.0, -60.0));
    step(&mut w);
    assert_eq!(state(&w, k), SiteState::Awake);
    assert!(!w.can_recall(Owner::PLAYER), "armed raiders within 200 m");
    let ships = w.voyage().unwrap().sites()[k].ships().to_vec();
    // All but one disarmed: the site stays awake, the last armed raider still blocks the recall.
    let damage = ships[1..].iter().flat_map(|&s| kill_guns(&w, s)).collect();
    w.step(&TickInputs {
        damage,
        ..Default::default()
    });
    assert_eq!(state(&w, k), SiteState::Awake);
    assert!(!w.can_recall(Owner::PLAYER));
    w.step(&TickInputs {
        damage: kill_guns(&w, ships[0]),
        ..Default::default()
    });
    assert!(w.events().contains(&SimEvent::SiteCleared {
        site: k as u32,
        goal: true
    }));
    assert!(w.voyage().unwrap().goal_cleared());
    for &s in &ships {
        let ship = w.ship(s).unwrap();
        assert_eq!(ship.vessel_state(), Some(sim::VesselState::Running));
        assert!(!ship.is_armed());
        assert!(w.awake_enemy(Owner::PLAYER, ship) && !w.armed_enemy(Owner::PLAYER, ship));
    }
    assert!(w.can_recall(Owner::PLAYER), "only armed awake enemies block the recall");
}

/// Clearing is final, so it must not fire on a passing state (13 Oct 2026 review): an awake site whose armed
/// enemies all went dormant (their owner's mind capacity cut to 0) stays awake; dormant enemies do not block the
/// recall; once they run again they do.
#[test]
fn dormant_armed_enemies_keep_their_site_open() {
    let c = content();
    let (mut w, me) = voyage_world(&c, "yard_approach", 3);
    let (k, pos) = goal(&w);
    put(&mut w, me, pos + Vec2::new(0.0, -60.0));
    step(&mut w);
    assert_eq!(state(&w, k), SiteState::Awake);
    let ships = w.voyage().unwrap().sites()[k].ships().to_vec();
    let mut config = *w.config();
    config.mind_capacity[ENEMY.0 as usize] = 0;
    w.set_config(config);
    for _ in 0..3 {
        step(&mut w);
    }
    for &s in &ships {
        assert_eq!(
            w.ship(s).unwrap().vessel_state(),
            Some(sim::VesselState::Dormant)
        );
    }
    assert_eq!(state(&w, k), SiteState::Awake, "dormant is not disarmed");
    assert!(w.can_recall(Owner::PLAYER));
    config.mind_capacity[ENEMY.0 as usize] = 1000;
    w.set_config(config);
    step(&mut w);
    assert_eq!(state(&w, k), SiteState::Awake);
    assert!(!w.can_recall(Owner::PLAYER));
}

/// The Warden's heart has a gun of its own (GDR-0068): with its fists cut off and still armed, a disarmed heart
/// keeps the goal open; disarming the fists too clears it while all three cores still run.
#[test]
fn the_warden_is_cleared_when_every_section_is_disarmed() {
    let c = content();
    let (mut w, me) = voyage_world(&c, "wardens_gate", 11);
    let (k, pos) = goal(&w);
    put(&mut w, me, pos + Vec2::new(0.0, -500.0));
    step(&mut w);
    let site = &w.voyage().unwrap().sites()[k];
    let warden = *site
        .ships()
        .iter()
        .find(|&&s| w.ship(s).unwrap().design().id == "automaton_warden")
        .unwrap();
    let raiders: Vec<ShipId> = site
        .ships()
        .iter()
        .copied()
        .filter(|&s| s != warden)
        .collect();
    assert!(w.ship(warden).unwrap().is_armed());
    let d = w.ship(warden).unwrap().design().clone();
    let bridges = [find(&d, "armor_block", 2, 1), find(&d, "armor_block", 6, 1)];
    let mut damage: Vec<DamageCommand> = bridges
        .map(|p| DamageCommand {
            ship: warden,
            target: DamageTarget::Part(p),
            amount: 1000.0,
        })
        .to_vec();
    damage.extend(raiders.iter().flat_map(|&s| kill_cores(&w, s)));
    w.step(&TickInputs {
        damage,
        ..Default::default()
    });
    assert_eq!(
        state(&w, k),
        SiteState::Awake,
        "woken by the raiders' lost cores"
    );
    let fists: Vec<ShipId> = w.voyage().unwrap().sites()[k]
        .ships()
        .iter()
        .copied()
        .filter(|&s| s != warden && !raiders.contains(&s))
        .collect();
    assert_eq!(fists.len(), 2);
    // The heart alone: its core, its cannon and its shield.
    let heart = w.ship(warden).unwrap();
    assert!(heart.is_armed() && heart.is_vessel());
    w.step(&TickInputs {
        damage: kill_guns(&w, warden),
        ..Default::default()
    });
    assert!(!w.ship(warden).unwrap().is_armed());
    assert_eq!(state(&w, k), SiteState::Awake, "the fists are still armed");
    let damage = fists.iter().flat_map(|&s| kill_guns(&w, s)).collect();
    w.step(&TickInputs {
        damage,
        ..Default::default()
    });
    assert_eq!(state(&w, k), SiteState::Cleared);
    for s in std::iter::once(warden).chain(fists) {
        assert!(w.ship(s).unwrap().is_vessel(), "the cores still run");
    }
}

/// A sleeping site wakes when one of its ships is hit (`SimEvent::Hit`): a shot from far outside the wake radius
/// that leaves the ship running.
#[test]
fn a_shot_wakes_a_sleeping_site() {
    let c = content();
    let (mut w, me) = voyage_world(&c, "yard_approach", 3);
    let (k, pos) = goal(&w);
    put(&mut w, me, pos + Vec2::new(0.0, -500.0));
    step(&mut w);
    assert_eq!(state(&w, k), SiteState::Asleep);
    let target = w.voyage().unwrap().sites()[k].ships()[0];
    let at = w.ship(target).unwrap().pieces()[0].com_world();
    let from = at + Vec2::new(-4.0, 0.0);
    let vel = Vec2::new(90.0, 0.0);
    w.spawn_projectile(me, data::WeaponFamily::Kinetic, from, vel, 0.1, 1, 1.0);
    let mut hit = false;
    for _ in 0..4 {
        let events = step(&mut w);
        hit = events
            .iter()
            .any(|e| matches!(e, SimEvent::Hit { ship, .. } if *ship == target));
        if hit {
            assert!(events.contains(&SimEvent::SiteWoke { site: k as u32 }));
            break;
        }
    }
    assert!(hit, "the shot hit");
    assert_eq!(state(&w, k), SiteState::Awake);
    assert_eq!(
        w.ship(target).unwrap().vessel_state(),
        Some(sim::VesselState::Running)
    );
}

/// Wrecks are a pure function of the plan (13 Oct 2026 review): each carries its own damage seed, so the same plan
/// gives the same wrecks whatever order the sites spawn in, and the plan never shares draws with the sim's
/// encounter stream for the same seed.
#[test]
fn wrecks_do_not_depend_on_the_order_the_sites_spawn_in() {
    let c = content();
    let p = plan(&c, "halvex_drift", 9);
    let seeds: Vec<u64> = p.sites.iter().flat_map(|s| s.wrecks.iter().map(|w| w.seed)).collect();
    assert!(seeds.len() >= 2 && seeds.iter().all(|&s| s != 0));
    assert!(p.sites.iter().all(|s| s.ships.iter().all(|x| x.seed == 0)));
    // The first draws of the sim's encounter stream are not the plan's wreck seeds.
    let mut sim = Sim::new(9);
    let first = sim.rng(sim::Stream::Encounter).next_u64();
    assert!(!seeds.contains(&first));

    let wrecks_by_site = |order: &[usize]| -> Vec<Vec<Vec<bool>>> {
        let (mut w, me) = voyage_world(&c, "halvex_drift", 9);
        let n = w.voyage().unwrap().sites().len();
        let mut out = vec![Vec::new(); n];
        for &k in order {
            let pos = w.voyage().unwrap().sites()[k].pos();
            // Other sites within the spawn radius spawn too: record each site the step it spawned.
            put(&mut w, me, pos + Vec2::new(0.0, -500.0));
            step(&mut w);
            for (j, s) in w.voyage().unwrap().sites().iter().enumerate() {
                if out[j].is_empty() && s.state() != SiteState::Unspawned {
                    out[j] = s
                        .wrecks()
                        .iter()
                        .map(|&id| w.ship(id).unwrap().parts().iter().map(|p| p.alive).collect())
                        .collect();
                    out[j].push(Vec::new()); // marks "spawned" even without wrecks
                }
            }
        }
        out
    };
    let n = p.sites.len();
    let forward: Vec<usize> = (0..n).collect();
    let backward: Vec<usize> = (0..n).rev().collect();
    let (a, b) = (wrecks_by_site(&forward), wrecks_by_site(&backward));
    assert!(a.iter().all(|s| !s.is_empty()), "every site spawned");
    assert_eq!(a, b);
}
