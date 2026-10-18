//! The voyage director (`sim::voyage`, GDR-0062): rolling a plan, lazy spawning, sleeping and waking sites,
//! clearing, split-off vessels joining their site, the recall check and the state hash. Plus the slice's enemy
//! ships, which the voyages bring, checked with the design report.

mod common;

use common::*;
use sim::ship::breakage::DamageTarget;
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
                let e = c.encounters.get(s.encounter.as_deref().unwrap()).unwrap();
                assert!(!s.signal, "a slice voyage's goal is an ordinary site (its voyage is the signal, E7)");
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
                        assert!((lo..=hi).contains(&n), "{:?}: {id} ×{n}", s.encounter);
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
        assert_eq!(s.state() != SiteState::Unspawned, near, "{:?}", s.encounter());
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
    assert!(ships.len() >= 2);
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
        damage: kill_cores(&w, ships[1]),
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

/// The voyages' new enemies fly: one piece, powered, every gun clear, no issue in the design report but what their
/// blasts would take (their cores sit beside guns, as on most ships).
#[test]
fn the_hunter_and_the_warden_are_flight_ready() {
    let c = content();
    for id in ["automaton_hunter", "automaton_warden"] {
        let r = design_report(&c, c.ships.get(id).unwrap(), &SimConfig::default()).unwrap();
        let issues: Vec<_> = r.issues.iter().filter(|i| !matches!(i, sim::report::Issue::BlastTakes { .. })).collect();
        assert!(issues.is_empty(), "{id}: {issues:?}");
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

/// GDR-0074: a fleet with no working gun may always recall (the emergency warp), even with armed enemies near; an
/// armed escort beside it takes that away again.
#[test]
fn a_disarmed_fleet_may_recall_under_fire() {
    let c = content();
    let (mut w, me) = voyage_world(&c, "yard_approach", 3);
    let (k, pos) = goal(&w);
    put(&mut w, me, pos + Vec2::new(0.0, -60.0));
    step(&mut w);
    assert_eq!(state(&w, k), SiteState::Awake);
    assert!(!w.fleet_disarmed(Owner::PLAYER));
    assert!(!w.can_recall(Owner::PLAYER), "armed raiders within 200 m");
    w.step(&TickInputs {
        damage: kill_guns(&w, me),
        ..Default::default()
    });
    assert!(!w.ship(me).unwrap().is_armed());
    assert!(w.voyage().unwrap().sites()[k].ships().iter().any(|&s| w.armed_enemy(Owner::PLAYER, w.ship(s).unwrap())));
    assert!(w.fleet_disarmed(Owner::PLAYER));
    assert!(w.can_recall(Owner::PLAYER), "the emergency recall");
    assert!(!w.fleet_disarmed(Owner(3)) && !w.can_recall(Owner(3)), "no vessels, nothing to recall");
    // An armed escort beside the disarmed flagship: the fleet is armed, the raiders block the recall again.
    let at = w.ship(me).unwrap().piece(w.ship(me).unwrap().control_piece().unwrap()).unwrap().com_world();
    w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, Body::at(at + Vec2::new(10.0, 0.0), 0.0));
    assert!(!w.fleet_disarmed(Owner::PLAYER));
    assert!(!w.can_recall(Owner::PLAYER));
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

/// A one-site plan (tests): `ships` at `pos`, nothing else.
fn one_site(ships: Vec<sim::Spawn>, wake_radius: f32) -> VoyagePlan {
    VoyagePlan {
        voyage: "test".into(),
        seed: 1,
        radius: 1000.0,
        entry: Body::at(Vec2::new(0.0, -1000.0), 0.0),
        sites: vec![sim::SitePlan {
            encounter: Some("raider_pack".into()),
            goal: true,
            signal: false,
            core: None,
            pos: Vec2::ZERO,
            wake_radius,
            wary: false,
            ships,
            wrecks: Vec::new(),
        }],
        star: None,
        rocks: Vec::new(),
    }
}

/// A wary site (GDR-0099 A3): two raiders at the origin, waking within 250 m if they were not wary, and a wreck
/// 60 m below them.
fn wary_site(c: &data::Content) -> VoyagePlan {
    let spawn = |pos: Vec2| sim::Spawn { design: design(c, "automaton_raider"), body: Body::at(pos, 0.0), damage: 0.0, seed: 0 };
    let mut plan = one_site(vec![spawn(Vec2::ZERO), spawn(Vec2::new(12.0, 0.0))], 250.0);
    plan.sites[0].wary = true;
    plan.sites[0].wrecks.push(spawn(Vec2::new(0.0, -60.0)));
    plan
}

fn wary_world(c: &data::Content, at: Vec2) -> (Sim, ShipId) {
    let mut w = Sim::new(1);
    w.set_behaviors(sim::BehaviorLibrary::new(c).unwrap());
    w.start_voyage(wary_site(c), Owner::PLAYER, ENEMY);
    let me = w.spawn_ship(design(c, "rim_skiff"), Owner::PLAYER, Body::at(at, 0.0));
    w.command(me, sim::ShipCommand::Focus);
    (w, me)
}

#[test]
fn a_wary_site_sleeps_through_a_visit_and_wakes_on_a_scan_close_by() {
    let c = content();
    let (mut w, me) = wary_world(&c, Vec2::new(0.0, -120.0));
    for _ in 0..150 {
        step(&mut w);
        put(&mut w, me, Vec2::new(0.0, -120.0));
    }
    assert_eq!(state(&w, 0), SiteState::Asleep, "within the wake radius, but wary");
    w.command(me, sim::ShipCommand::Scan);
    let mut woke = None;
    for t in 0..300 {
        if step(&mut w).iter().any(|e| matches!(e, SimEvent::SiteWoke { site: 0 })) {
            woke = Some(t);
            break;
        }
    }
    assert!(woke.is_some(), "a scan within the scanner's wake radius wakes it");
    assert_eq!(state(&w, 0), SiteState::Awake);
}

#[test]
fn a_wary_site_wakes_when_a_shot_hits_or_its_wreck_is_stripped() {
    let c = content();
    // A shot at one of its ships.
    let (mut w, me) = wary_world(&c, Vec2::new(0.0, -120.0));
    step(&mut w);
    let target = w.voyage().unwrap().sites()[0].ships()[0];
    let at = w.ship(target).unwrap().pieces()[0].com_world();
    w.spawn_projectile(me, data::WeaponFamily::Kinetic, at + Vec2::new(-4.0, 0.0), Vec2::new(90.0, 0.0), 0.1, 1, 1.0);
    for _ in 0..4 {
        step(&mut w);
    }
    assert_eq!(state(&w, 0), SiteState::Awake);
    // Stripping its wreck: a collector parked beside it (but further than the shot) takes a part.
    let (mut w, me) = wary_world(&c, Vec2::new(-8.0, -60.0));
    let mut stripped = false;
    for _ in 0..120 {
        put(&mut w, me, Vec2::new(-8.0, -60.0));
        let ev = step(&mut w);
        stripped |= ev.iter().any(|e| matches!(e, SimEvent::Collected { .. }));
        if stripped {
            break;
        }
        assert_eq!(state(&w, 0), SiteState::Asleep);
    }
    assert!(stripped, "the collector took a part");
    step(&mut w);
    assert_eq!(state(&w, 0), SiteState::Awake, "stripping its wreck wakes it");
}

#[test]
fn a_site_of_wary_ships_rolls_wary_and_a_hot_cluster_its_star() {
    let mut c = content();
    let mut bp = c.ships.get("automaton_raider").unwrap().clone();
    bp.id = "grown_test".into();
    bp.faction = "grown".into();
    c.ships.upsert(bp);
    let mut e = c.encounters.get("raider_pair").unwrap().clone();
    e.id = "grove_test".into();
    e.ships[0].ship = "grown_test".into();
    c.encounters.upsert(e);
    let interior = galaxy::Interior {
        kind: "scorched_deep".into(),
        radius: 2000.0,
        spacing: (600.0, 1200.0),
        sites: ["grove_test", "raider_pair"]
            .iter()
            .map(|e| galaxy::InteriorSite { encounter: Some((*e).into()), place: None })
            .collect(),
        star: Some(galaxy::InteriorStar { x: 3600.0, y: 0.0, heat: 0.3 }),
    };
    let plan = VoyagePlan::roll_interior(&c, "test", &interior, &Default::default(), 5).unwrap();
    assert!(plan.sites[0].wary && !plan.sites[1].wary);
    let star = plan.star.expect("its star");
    assert_eq!((star.pos, star.heat, star.reach), (Vec2::new(3600.0, 0.0), 0.3, 3600.0));
    // The same roll without the star: the sites lie where they lay.
    let cold = VoyagePlan::roll_interior(&c, "test", &galaxy::Interior { star: None, ..interior }, &Default::default(), 5).unwrap();
    assert!(cold.star.is_none());
    assert_eq!(cold.sites.iter().map(|s| s.pos).collect::<Vec<_>>(), plan.sites.iter().map(|s| s.pos).collect::<Vec<_>>());
}

/// A site of a `near_star` encounter in a hot cluster lies as near the star as the radius allows, wherever the star
/// is: nearer it than every other site, where the heat is several times the centre's. Without a star it is an
/// ordinary site.
#[test]
fn a_near_star_site_lies_by_the_hot_star() {
    let c = content();
    assert!(c.encounters.get("scorched_wrecks").unwrap().near_star);
    for (seed, angle) in [(1u64, 0.0f32), (2, 2.0), (3, 4.0), (4, 5.5)] {
        let d = 2400.0 * 1.8;
        let at = Vec2::new(d * angle.cos(), d * angle.sin());
        let interior = galaxy::Interior {
            kind: "scorched_deep".into(),
            radius: 2400.0,
            spacing: (600.0, 1400.0),
            sites: ["grove", "grove", "grown_dead", "scorched_wrecks"]
                .iter()
                .map(|e| galaxy::InteriorSite { encounter: Some((*e).into()), place: None })
                .collect(),
            star: Some(galaxy::InteriorStar { x: at.x.round(), y: at.y.round(), heat: 0.3 }),
        };
        let plan = VoyagePlan::roll_interior(&c, "test", &interior, &Default::default(), seed).unwrap();
        let star = plan.star.unwrap();
        let wrecks = &plan.sites[3];
        assert_eq!(wrecks.encounter.as_deref(), Some("scorched_wrecks"));
        assert!(wrecks.pos.length() <= 2400.0);
        let near = (wrecks.pos - star.pos).length();
        assert!(plan.sites[..3].iter().all(|s| (s.pos - star.pos).length() > near), "seed {seed}");
        assert!(star.heat_at(wrecks.pos) > 2.5 * star.heat, "seed {seed}: {} at {:?}", star.heat_at(wrecks.pos), wrecks.pos);
        for s in &plan.sites[..3] {
            assert!((s.pos - wrecks.pos).length() >= 600.0, "the spacing's min holds");
        }
    }
}

/// A blueprint in two cored pieces (13 Oct 2026 review): the second forms a vessel of its own at spawn. Its
/// vessels join the site in id order, hold where they are, and a hit on one wakes the site.
#[test]
fn a_site_ship_in_several_cored_pieces_keeps_the_list_ascending() {
    let c = content();
    let twin = custom(
        &c,
        &[
            ("automaton_cannon", 0, 0, 0, false, None),
            ("automaton_core", 0, 1, 0, false, None),
            ("automaton_cannon", 3, 0, 0, false, None),
            ("automaton_core", 3, 1, 0, false, None),
        ],
    );
    let spawn = |x: f32| sim::Spawn {
        design: twin.clone(),
        body: Body::at(Vec2::new(x, 0.0), 0.0),
        damage: 0.0,
        seed: 0,
    };
    let mut w = Sim::new(1);
    w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    w.start_voyage(one_site(vec![spawn(0.0), spawn(20.0)], 100.0), Owner::PLAYER, ENEMY);
    let me = w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, Body::at(Vec2::new(0.0, -400.0), 0.0));
    step(&mut w);
    let ships = w.voyage().unwrap().sites()[0].ships().to_vec();
    assert_eq!(ships.len(), 4, "two ships, each with a split-off vessel");
    assert!(ships.windows(2).all(|p| p[0] < p[1]), "{ships:?}");
    for &s in &ships {
        let ship = w.ship(s).unwrap();
        assert!(ship.is_vessel());
        assert!(
            matches!(ship.helm(), HelmMode::Maneuver { maneuver: Maneuver::Hold { .. }, fire: false }),
            "{s:?}: {:?}",
            ship.helm()
        );
        assert_eq!(w.voyage().unwrap().site_of(s), Some(0));
    }
    assert_eq!(state(&w, 0), SiteState::Asleep);
    // A shot at a split-off vessel wakes the site.
    let target = ships[1];
    let at = w.ship(target).unwrap().pieces()[0].com_world();
    w.spawn_projectile(me, data::WeaponFamily::Kinetic, at + Vec2::new(-4.0, 0.0), Vec2::new(90.0, 0.0), 0.1, 1, 1.0);
    for _ in 0..4 {
        step(&mut w);
    }
    assert_eq!(state(&w, 0), SiteState::Awake);
}

/// A sleeper also wakes when its bubble takes a whole shot (`ShieldHit` only) or when it is rammed hard enough to
/// take damage (`Impact` with damage), while it still runs.
#[test]
fn a_shot_into_a_bubble_or_a_ram_wakes_a_sleeping_site() {
    let c = content();
    // A small shielded ship whose bubble reaches past its hull takes an energy shot whole.
    let shielded = custom(
        &c,
        &[
            ("logic_core", 0, 0, 0, false, None),
            ("fission_reactor", 1, 0, 0, false, None),
            ("shield_emitter", 2, 0, 0, false, None),
        ],
    );
    let warden = sim::Spawn {
        design: shielded,
        body: Body::default(),
        damage: 0.0,
        seed: 0,
    };
    let mut w = Sim::new(1);
    w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    w.start_voyage(one_site(vec![warden], 100.0), Owner::PLAYER, ENEMY);
    let me = w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, Body::at(Vec2::new(0.0, -400.0), 0.0));
    for _ in 0..5 {
        step(&mut w);
    }
    let id = w.voyage().unwrap().sites()[0].ships()[0];
    let (centre, radius) = w
        .bubbles()
        .iter()
        .find(|b| b.ship == id)
        .map(|b| (b.centre, b.radius))
        .expect("the bubble is up");
    let from = centre + Vec2::new(radius + 2.0, 0.0);
    w.spawn_projectile(me, data::WeaponFamily::Energy, from, Vec2::new(-90.0, 0.0), 0.5, 1, 1.0);
    let (mut shield, mut hit, mut woke) = (false, false, false);
    for _ in 0..4 {
        for e in step(&mut w) {
            shield |= matches!(e, SimEvent::ShieldHit { ship, .. } if ship == id);
            hit |= matches!(e, SimEvent::Hit { .. });
            woke |= e == SimEvent::SiteWoke { site: 0 };
        }
    }
    assert!(shield && !hit, "the bubble took it all");
    // It woke (and, unarmed, cleared at once: GDR-0068).
    assert!(woke);

    // A drifting wreck rams a sleeping raider.
    let raider = sim::Spawn {
        design: design(&c, "automaton_raider"),
        body: Body::default(),
        damage: 0.0,
        seed: 0,
    };
    let mut w = Sim::new(1);
    w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    w.start_voyage(one_site(vec![raider], 100.0), Owner::PLAYER, ENEMY);
    w.spawn_ship(design(&c, "rim_skiff"), Owner::PLAYER, Body::at(Vec2::new(0.0, -400.0), 0.0));
    step(&mut w);
    let id = w.voyage().unwrap().sites()[0].ships()[0];
    // From behind, on its armour and thruster, slowly enough that its core outlives the crushed thruster's blast.
    let ram = Body {
        linvel: Vec2::new(0.0, 20.0),
        ..Body::at(Vec2::new(0.0, -6.0), 0.0)
    };
    w.spawn_wreck(design(&c, "automaton_raider"), Owner(2), ram, 0.0, 0);
    let (mut rammed, mut woke) = (false, false);
    for _ in 0..10 {
        for e in step(&mut w) {
            rammed |= matches!(e, SimEvent::Impact(c) if c.damage > 0.0 && (c.a.ship == id || c.b.ship == id));
            woke |= e == SimEvent::SiteWoke { site: 0 };
        }
        if rammed {
            break;
        }
    }
    assert!(rammed && woke, "the wreck rammed the raider: {rammed} {woke}");
    assert_eq!(
        w.ship(id).unwrap().vessel_state(),
        Some(sim::VesselState::Running)
    );
}

/// The voyage runner (`sim::voyage::runner`): the same seed gives the same run, stepped in slices or in one go,
/// and a run ends within its time limit.
#[test]
fn the_voyage_runner_is_deterministic_in_slices() {
    let c = content();
    let voyage = c.voyages.get("yard_approach").unwrap();
    let report = design_report(&c, c.ships.get("rim_skiff").unwrap(), &SimConfig::default()).unwrap();
    let flagship = sim::Flagship {
        design: design(&c, "rim_skiff"),
        reach: sim::closing_reach(&report),
        behavior: None,
    };
    let lib = sim::BehaviorLibrary::new(&c).unwrap();
    let limit = 3 * 60 * 30;
    let start = || {
        sim::VoyageRunner::new(&c, voyage, flagship.clone(), lib.clone(), &[], 3, limit).unwrap()
    };
    let whole = start().run();
    let mut sliced = start();
    let r = loop {
        if let Some(r) = sliced.step(97) {
            break r.clone();
        }
    };
    assert!(whole.ticks <= limit && whole.ticks > 0);
    assert_eq!(
        (r.outcome, r.ticks, r.goal, r.kills, r.sites_cleared),
        (whole.outcome, whole.ticks, whole.goal, whole.kills, whole.sites_cleared)
    );
    assert_eq!(r.hp_lost.to_bits(), whole.hp_lost.to_bits());
    assert!(whole.sites_with_ships >= 1 && whole.sites_with_ships <= whole.sites);
}
