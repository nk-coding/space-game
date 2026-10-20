//! The Grown's mechanics (GDR-0099, GDR-0102): hearts re-grow lost parts, tissue heals, ram prows crush harder, and
//! a hot star heats the parts open to space.

mod common;

use common::*;
use sim::ship::mass::to_piece;
use sim::{Body, BodyKey, DamageTarget, Owner, PartIdx, ShipId, Sim, SimEvent, Star, TickInputs, Vec2, VoyagePlan};

/// A heart ship: a core and a reactor, the heart (2 × 2) beside them, steel rows below.
///
/// ```text
/// y0  C R H H
/// y1  a a H H
/// y2  a a a a
/// y3  a a a a
/// ```
fn heart_ship(c: &data::Content) -> std::sync::Arc<sim::ShipDesign> {
    let mut parts: Vec<P> = vec![
        ("logic_core", 0, 0, 0, false, None),
        ("fission_reactor", 1, 0, 0, false, None),
        ("grown_heart", 2, 0, 0, false, None),
        ("armor_block", 0, 1, 0, false, Some("steel")),
        ("armor_block", 1, 1, 0, false, Some("steel")),
    ];
    for y in 2..4 {
        for x in 0..4 {
            parts.push(("armor_block", x, y, 0, false, Some("steel")));
        }
    }
    custom(c, &parts)
}

fn step(w: &mut Sim) {
    w.step(&TickInputs::default());
}

fn kill(w: &mut Sim, ship: ShipId, parts: &[PartIdx]) {
    for &p in parts {
        w.apply_damage(ship, DamageTarget::Part(p), 100.0);
    }
    w.resolve_breakage();
}

/// The parts re-grown in this step.
fn regrown(w: &Sim) -> Vec<PartIdx> {
    w.events()
        .iter()
        .filter_map(|e| match *e {
            SimEvent::PartRegrown { part, .. } => Some(part),
            _ => None,
        })
        .collect()
}

/// Steps until a part re-grows (at most `ticks`): the tick it happened and the parts.
fn until_regrown(w: &mut Sim, ticks: u32) -> Option<(u64, Vec<PartIdx>)> {
    for _ in 0..ticks {
        step(w);
        let r = regrown(w);
        if !r.is_empty() {
            return Some((w.tick(), r));
        }
    }
    None
}

#[test]
fn a_heart_regrows_lost_parts_one_an_interval_nearest_first_and_weak() {
    let c = content();
    let d = heart_ship(&c);
    let heart = d.parts[find(&d, "grown_heart", 2, 0) as usize].heart.clone().unwrap();
    let interval = (heart.interval * 30.0).round() as u64;
    let (mut w, s) = world_with(d.clone());
    let (near, mid, far) = (find(&d, "armor_block", 2, 2), find(&d, "armor_block", 3, 3), find(&d, "armor_block", 0, 3));
    kill(&mut w, s, &[far, near, mid]);
    let mass0 = w.ship(s).unwrap().pieces()[0].mass.mass;
    // One part an interval, nearest the heart first, each with the heart's share of its hit points.
    let mut last = w.tick();
    for want in [near, mid, far] {
        let (tick, parts) = until_regrown(&mut w, interval as u32 + 2).expect("a part re-grows within an interval");
        assert_eq!(parts, [want]);
        assert!(tick - last >= interval - 1 && tick - last <= interval + 1, "{} ticks", tick - last);
        last = tick;
        let ship = w.ship(s).unwrap();
        let ps = ship.part(want);
        assert!(ps.alive && ps.piece == ship.pieces()[0].id);
        assert!(close(ps.hp, d.parts[want as usize].hp * heart.hp, 1e-6), "{ps:?}");
        assert!(ship.grid().occupant((d.parts[want as usize].shape.x, d.parts[want as usize].shape.y)) == Some(want as usize));
        assert!(w.events().iter().any(|e| matches!(e, SimEvent::PieceChanged { .. })), "the piece is rebuilt in the same tick");
        ship.verify_caches().unwrap();
    }
    // The piece and its body weigh what they weighed intact.
    let ship = w.ship(s).unwrap();
    let piece = &ship.pieces()[0];
    assert!(close(piece.mass.mass, mass0 + 3.0 * d.parts[near as usize].mass, 1e-4));
    let key = BodyKey { ship: s, piece: piece.id };
    let body = w.physics().mass_props(key).unwrap();
    assert!(close(body.mass, piece.mass.mass, 1e-3), "{body:?} vs {:?}", piece.mass);
    assert_eq!(piece.parts.len(), d.parts.len());
    // Nothing left to grow: nothing happens.
    assert!(until_regrown(&mut w, 3 * interval as u32).is_none());
}

#[test]
fn a_lost_part_grows_back_only_from_a_living_neighbour() {
    let c = content();
    let d = heart_ship(&c);
    let (mut w, s) = world_with(d.clone());
    // (0, 3) has only (0, 2) and (1, 3) as neighbours: both gone, it waits for one of them.
    let (corner, up, right) = (find(&d, "armor_block", 0, 3), find(&d, "armor_block", 0, 2), find(&d, "armor_block", 1, 3));
    kill(&mut w, s, &[corner, up, right]);
    let mut order = Vec::new();
    while order.len() < 3 {
        let (_, parts) = until_regrown(&mut w, 200).expect("everything grows back");
        order.extend(parts);
    }
    assert_eq!(order.last(), Some(&corner), "{order:?}");
}

#[test]
fn killing_the_heart_stops_it_and_hearts_never_regrow() {
    let c = content();
    let d = heart_ship(&c);
    let (mut w, s) = world_with(d.clone());
    let heart = find(&d, "grown_heart", 2, 0);
    kill(&mut w, s, &[find(&d, "armor_block", 2, 2), heart]);
    assert!(until_regrown(&mut w, 600).is_none());
    assert!(!w.ship(s).unwrap().part(heart).alive);
}

#[test]
fn a_heart_on_a_wreck_or_a_broken_off_chunk_regrows_nothing() {
    let c = content();
    let d = heart_ship(&c);
    // Without its core the ship is salvage: its heart no longer works.
    let (mut w, s) = world_with(d.clone());
    kill(&mut w, s, &[find(&d, "logic_core", 0, 0), find(&d, "armor_block", 2, 2)]);
    assert!(w.ship(s).unwrap().control_piece().is_none());
    assert!(until_regrown(&mut w, 300).is_none());
}

#[test]
fn a_broken_off_chunk_is_never_grown_back_on_and_its_cells_wait() {
    let c = content();
    // A tail hangs off the hull by one block (1, 2).
    let d = custom(
        &c,
        &[
            ("logic_core", 0, 0, 0, false, None),
            ("fission_reactor", 1, 0, 0, false, None),
            ("grown_heart", 2, 0, 0, false, None),
            ("armor_block", 0, 1, 0, false, Some("steel")),
            ("armor_block", 1, 1, 0, false, Some("steel")),
            ("armor_block", 1, 2, 0, false, Some("steel")),
            ("armor_block", 1, 3, 0, false, Some("steel")),
            ("armor_block", 2, 3, 0, false, Some("steel")),
        ],
    );
    let (mut w, s) = world_with(d.clone());
    let (link, tail) = (find(&d, "armor_block", 1, 2), [find(&d, "armor_block", 1, 3), find(&d, "armor_block", 2, 3)]);
    kill(&mut w, s, &[link]);
    assert_eq!(w.ship(s).unwrap().pieces().len(), 2, "the tail broke off");
    // The link would join the tail again: it waits while the tail is there.
    assert!(until_regrown(&mut w, 300).is_none());
    // Once the tail is gone, the link grows back, then the tail from it.
    kill(&mut w, s, &tail);
    let mut grown = Vec::new();
    while grown.len() < 3 {
        grown.extend(until_regrown(&mut w, 200).expect("the tail grows back").1);
    }
    assert_eq!(grown[0], link);
    let ship = w.ship(s).unwrap();
    assert_eq!(ship.pieces().len(), 1);
    ship.verify_caches().unwrap();
}

#[test]
fn a_part_never_grows_back_inside_another_hull() {
    let c = content();
    let d = heart_ship(&c);
    let (mut w, s) = world_with(d.clone());
    let lost = find(&d, "armor_block", 3, 3);
    kill(&mut w, s, &[lost]);
    // A loose steel block sits in the lost block's place.
    let place = |w: &Sim| {
        let piece = &w.ship(s).unwrap().pieces()[0];
        Body::at(piece.body.to_world(to_piece(d.parts[lost as usize].centroid, piece.origin)), piece.body.angle)
    };
    let block = custom(&c, &[("armor_block", 0, 0, 0, false, Some("steel"))]);
    let b = w.spawn_ship(block, Owner(3), place(&w));
    let interval = (d.parts[find(&d, "grown_heart", 2, 0) as usize].heart.as_ref().unwrap().interval * 30.0) as u32;
    let bp = w.ship(b).unwrap().pieces()[0].id;
    for _ in 0..3 * interval {
        // Held in place: the test is the rule, not the push between the bodies.
        let here = place(&w);
        w.teleport(b, bp, here);
        step(&mut w);
        assert!(regrown(&w).is_empty(), "grew inside the block");
    }
    w.teleport(b, bp, Body::at(Vec2::new(500.0, 0.0), 0.0));
    let (_, parts) = until_regrown(&mut w, interval + 2).expect("it grows once the place is free");
    assert_eq!(parts, [lost]);
}

#[test]
fn tissue_heals_to_full_on_a_vessel_and_not_on_salvage() {
    let c = content();
    let d = custom(
        &c,
        &[
            ("logic_core", 0, 0, 0, false, None),
            ("fission_reactor", 1, 0, 0, false, None),
            ("armor_block", 0, 1, 0, false, Some("grown_tissue")),
            ("armor_block", 1, 1, 0, false, Some("grown_tissue")),
            ("armor_block", 1, 2, 0, false, Some("grown_tissue")),
            ("armor_block", 1, 3, 0, false, Some("grown_tissue")),
        ],
    );
    let heal = c.materials.get("grown_tissue").unwrap().armor.as_ref().unwrap().heal;
    let (mut w, s) = world_with(d.clone());
    let (a, link, chunk) = (find(&d, "armor_block", 0, 1), find(&d, "armor_block", 1, 2), find(&d, "armor_block", 1, 3));
    let full = d.parts[a as usize].hp;
    assert!(heal > 0.0 && close(d.parts[a as usize].heal, heal, 1e-6));
    w.apply_damage(s, DamageTarget::Part(a), 0.5);
    w.apply_damage(s, DamageTarget::Part(chunk), 0.5);
    kill(&mut w, s, &[link]);
    for _ in 0..30 {
        step(&mut w);
    }
    let ship = w.ship(s).unwrap();
    assert!(close(ship.part(a).hp, full - 0.5 + heal, 1e-4), "{} after a second", ship.part(a).hp);
    assert_ne!(ship.part(chunk).piece, ship.part(a).piece, "the chunk broke off");
    assert_eq!(ship.part(chunk).hp, full - 0.5, "salvage does not heal");
    for _ in 0..30 * 30 {
        step(&mut w);
    }
    assert_eq!(w.ship(s).unwrap().part(a).hp, full, "up to its full hit points, no further");
}

/// A ship with `nose` (a 2 × 1 row) in front of a core and a reactor, steel behind.
fn rammer(c: &data::Content, prow: bool) -> std::sync::Arc<sim::ShipDesign> {
    let mut parts: Vec<P> = vec![
        ("logic_core", 0, 1, 0, false, None),
        ("fission_reactor", 1, 1, 0, false, None),
        ("armor_block", 0, 2, 0, false, Some("steel")),
        ("armor_block", 1, 2, 0, false, Some("steel")),
    ];
    if prow {
        parts.push(("ram_prow", 0, 0, 0, false, None));
    } else {
        parts.push(("armor_block", 0, 0, 0, false, Some("reactive")));
        parts.push(("armor_block", 1, 0, 0, false, Some("reactive")));
    }
    custom(c, &parts)
}

#[test]
fn a_ram_prow_deals_more_and_takes_less() {
    let c = content();
    let (a, b) = (rammer(&c, true), rammer(&c, false));
    let prow = c.parts.get("ram_prow").unwrap().ram.clone().unwrap();
    let mut w = Sim::new(1);
    // Nose to nose, 8 m/s each, of two owners.
    let sa = w.spawn_ship(a.clone(), Owner(1), Body { pos: Vec2::ZERO, angle: 0.0, linvel: Vec2::new(0.0, 8.0), angvel: 0.0 });
    let sb = w.spawn_ship(
        b.clone(),
        Owner(2),
        Body { pos: Vec2::new(0.0, 8.0), angle: core::f32::consts::PI, linvel: Vec2::new(0.0, -8.0), angvel: 0.0 },
    );
    let hp = |w: &Sim, s: ShipId| w.ship(s).unwrap().parts().iter().map(|p| p.hp).collect::<Vec<f32>>();
    let (a0, b0) = (hp(&w, sa), hp(&w, sb));
    let mut hit = None;
    for _ in 0..60 {
        step(&mut w);
        if let Some(c) = w.events().iter().find_map(|e| match e {
            SimEvent::Impact(c) if c.damage > 0.0 => Some(*c),
            _ => None,
        }) {
            hit = Some(c);
            break;
        }
    }
    let hit = hit.expect("they ram");
    let (pa, pb) = if hit.a.ship == sa { (hit.part_a.unwrap(), hit.part_b.unwrap()) } else { (hit.part_b.unwrap(), hit.part_a.unwrap()) };
    assert_eq!(pa, find(&a, "ram_prow", 0, 0));
    let (a1, b1) = (hp(&w, sa), hp(&w, sb));
    assert!(close(a0[pa as usize] - a1[pa as usize], hit.damage * prow.taken, 1e-4), "prow took {} of {}", a0[pa as usize] - a1[pa as usize], hit.damage);
    let dealt: f32 = b0.iter().zip(&b1).map(|(x, y)| x - y).sum();
    assert!(hit.damage * prow.dealt < 2.0, "the test keeps to the nose block: {}", hit.damage);
    assert!(close(b0[pb as usize] - b1[pb as usize], hit.damage * prow.dealt, 1e-4), "dealt {dealt} of {}", hit.damage);
}

/// GDR-0103: a Grown rammer under its content behavior charges a ship that sits still, prow first, and its ram
/// deals damage; a ram flies at top speed straight at the target and never brakes for it.
#[test]
fn a_grown_rammer_rams_with_its_prow() {
    let c = content();
    let rammer = design(&c, "grown_rammer");
    let runner = design(&c, "halvex_runner");
    let mut w = Sim::new(1);
    let me = w.spawn_ship(runner, Owner::PLAYER, Body::at(Vec2::ZERO, 0.0));
    let r = w.spawn_ship(rammer.clone(), Owner(1), Body::at(Vec2::new(0.0, -70.0), 0.0));
    fight(&mut w, &c, r);
    assert_eq!(w.behaviors().default_for(&rammer), w.behaviors().id("grown_rammer"));
    let prow = find(&rammer, "ram_prow", 1, 0);
    let mut by_prow = 0.0;
    for _ in 0..30 * 60 {
        step(&mut w);
        for e in w.events() {
            if let SimEvent::Impact(i) = e {
                let mine = if i.a.ship == r { i.part_a } else if i.b.ship == r { i.part_b } else { None };
                if mine == Some(prow) && i.damage > 0.0 {
                    by_prow += i.damage;
                }
            }
        }
    }
    assert!(by_prow > 0.0, "the rammer never rammed with its prow");
    assert!(w.ship(me).unwrap().parts().iter().any(|p| !p.alive), "the ram broke nothing");
}

/// GDR-0109: a seedling has no gun, but its ram prow and its engine keep it armed (a site of them does not clear
/// at once); under the rammer's behavior it charges a ship that sits still; with its engine gone it is disarmed.
#[test]
fn a_seedling_is_armed_by_its_prow_and_rams() {
    let c = content();
    let seedling = design(&c, "grown_seedling");
    let runner = design(&c, "halvex_runner");
    let mut w = Sim::new(1);
    let me = w.spawn_ship(runner, Owner::PLAYER, Body::at(Vec2::ZERO, 0.0));
    let s = w.spawn_ship(seedling.clone(), Owner(1), Body::at(Vec2::new(0.0, -70.0), 0.0));
    fight(&mut w, &c, s);
    step(&mut w);
    assert!(w.ship(s).unwrap().is_armed(), "a prow and an engine arm it");
    let prow = find(&seedling, "ram_prow", 0, 0);
    let hull = |w: &Sim, s: ShipId| w.ship(s).unwrap().parts().iter().map(|p| p.hp).sum::<f32>();
    let hull0 = hull(&w, me);
    let mut by_prow = 0.0;
    for _ in 0..30 * 60 {
        step(&mut w);
        for e in w.events() {
            if let SimEvent::Impact(i) = e {
                let mine = if i.a.ship == s { i.part_a } else if i.b.ship == s { i.part_b } else { None };
                if mine == Some(prow) && i.damage > 0.0 {
                    by_prow += i.damage;
                }
            }
        }
    }
    assert!(by_prow > 0.0, "the seedling never rammed with its prow");
    assert!(hull(&w, me) < hull0, "the ram hurt nothing");
    kill(&mut w, s, &[find(&seedling, "thruster", 1, 1)]);
    step(&mut w);
    assert!(!w.ship(s).unwrap().is_armed(), "a prow without an engine is no weapon");
}

/// A shield emitter open on its top and right side beside a core and a reactor over steel; with `radiator`, a
/// radiator right of the emitter.
fn emitter_ship(c: &data::Content, radiator: bool) -> std::sync::Arc<sim::ShipDesign> {
    let mut parts: Vec<P> = vec![
        ("logic_core", 0, 0, 0, false, None),
        ("fission_reactor", 1, 0, 0, false, None),
        ("shield_emitter", 2, 0, 0, false, None),
        ("armor_block", 0, 1, 0, false, Some("steel")),
        ("armor_block", 1, 1, 0, false, Some("steel")),
        ("armor_block", 2, 1, 0, false, Some("steel")),
    ];
    if radiator {
        parts.push(("radiator", 3, 0, 0, false, None));
    }
    custom(c, &parts)
}

/// A world with a hot star 1000 m right of the origin heating `heat` there, and nothing else in the voyage.
fn hot_world(heat: f32) -> Sim {
    let mut w = Sim::new(1);
    w.start_voyage(
        VoyagePlan {
            voyage: "hot".into(),
            seed: 1,
            radius: 3000.0,
            entry: Body::at(Vec2::new(0.0, -3000.0), 0.0),
            sites: Vec::new(),
            star: Some(Star { pos: Vec2::new(1000.0, 0.0), heat, reach: 1000.0 }),
            rocks: Vec::new(),
        },
        Owner::PLAYER,
        Owner(1),
    );
    w
}

#[test]
fn a_hot_star_heats_open_parts_more_near_it_and_radiators_help() {
    let c = content();
    let star = Star { pos: Vec2::new(1000.0, 0.0), heat: 0.3, reach: 1000.0 };
    // The inverse square, capped near the star.
    assert!(close(star.heat_at(Vec2::ZERO), 0.3, 1e-6));
    assert!(close(star.heat_at(Vec2::new(500.0, 0.0)), 1.2, 1e-5));
    assert!(close(star.heat_at(Vec2::new(990.0, 0.0)), 0.3 * data::STAR_HEAT_CAP, 1e-6));
    assert!(close(star.heat_at(Vec2::new(-1000.0, 0.0)), 0.075, 1e-6));

    let mut w = hot_world(0.3);
    let at = |x: f32| Body::at(Vec2::new(x, 0.0), 0.0);
    let far = w.spawn_ship(emitter_ship(&c, false), Owner::PLAYER, at(-1000.0));
    let centre = w.spawn_ship(emitter_ship(&c, false), Owner::PLAYER, at(0.0));
    let near = w.spawn_ship(emitter_ship(&c, false), Owner::PLAYER, at(600.0));
    let cooled = w.spawn_ship(emitter_ship(&c, true), Owner::PLAYER, Body::at(Vec2::new(600.0, 40.0), 0.0));
    // A cold world: the same ship far from any star.
    let mut cold = Sim::new(1);
    let alone = cold.spawn_ship(emitter_ship(&c, false), Owner::PLAYER, at(0.0));
    for _ in 0..90 * 30 {
        step(&mut w);
        cold.step(&TickInputs::default());
    }
    let d = emitter_ship(&c, false);
    let (emitter, armour) = (find(&d, "shield_emitter", 2, 0), find(&d, "armor_block", 0, 1));
    let heat = |w: &Sim, s: ShipId, p: PartIdx| w.ship(s).unwrap().part(p).heat;
    let limit = c.parts.get("shield_emitter").unwrap().heat.as_ref().unwrap().limit.unwrap();
    // Armour (warmed by the reactor beside it) heats more in the star's light, the more the nearer.
    let (o, f, m, n) = (heat(&cold, alone, armour), heat(&w, far, armour), heat(&w, centre, armour), heat(&w, near, armour));
    assert!(o < f && f < m && m < n, "{o} {f} {m} {n}");
    // The emitter keeps cool far out and overheats near the star, unless a radiator beside it vents the heat.
    assert!(heat(&w, centre, emitter) < limit && !w.ship(centre).unwrap().part(emitter).overheated);
    assert!(w.ship(near).unwrap().part(emitter).overheated, "{}", heat(&w, near, emitter));
    let cool = find(&emitter_ship(&c, true), "shield_emitter", 2, 0);
    assert!(!w.ship(cooled).unwrap().part(cool).overheated, "{} with a radiator", heat(&w, cooled, cool));
}

#[test]
fn grown_scenario_is_deterministic_and_the_mechanics_act() {
    use sim::voyage::scenario::{self, Scenario};
    use sim::ScenarioKind;
    let (a, sa) = scenario::run_kind(ScenarioKind::Grown, 42, 900);
    let (b, sb) = scenario::run_kind(ScenarioKind::Grown, 42, 900);
    if let Some(t) = a.iter().zip(&b).position(|(x, y)| x != y) {
        panic!("diverged at tick {}", t + 1);
    }
    assert_eq!(sa, sb);
    // Hearts re-grew parts, a prow rammed, the grove woke (it was visited first) and so did the Lattice needle beside
    // the way (its territory, GDR-0126), the star overheated parts.
    assert!(sa.regrown > 0 && sa.rams > 0 && sa.sites_woken == 2 && sa.overheated_part_ticks > 0, "{sa:?}");
    assert!(sa.shots > 0, "{sa:?}");
    // Regrowth keeps the incremental caches right.
    let content = content();
    let mut sc = Scenario::with_kind(&content, 7, ScenarioKind::Grown).unwrap();
    for t in 0..600 {
        sc.step();
        if t % 30 == 0 {
            for ship in sc.sim.ships() {
                ship.verify_caches().unwrap_or_else(|e| panic!("tick {t}, {:?}: {e}", ship.id()));
            }
        }
    }
    assert!(sc.stats.regrown > 0);
}
