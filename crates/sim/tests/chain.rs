//! Chain reactions (GDR-0019, GDR-0084): destroyed parts with an `explosion` damage the parts of their piece in
//! reach, and what the blasts destroy explodes in turn, in one breakage resolution.

mod common;

use common::*;
use sim::{DamageTarget, PartIdx, SimEvent, TickInputs};

/// The part events of the last step or resolution: `(part, exploded)` per `PartDestroyed` and `Explosion`, in
/// order.
fn part_events(w: &sim::Sim) -> Vec<(PartIdx, bool)> {
    w.events()
        .iter()
        .filter_map(|e| match *e {
            SimEvent::PartDestroyed { part, .. } => Some((part, false)),
            SimEvent::Explosion { part, .. } => Some((part, true)),
            _ => None,
        })
        .collect()
}

#[test]
fn blast_reach_follows_the_radius() {
    let c = content();
    // A part of each kind in the middle of a 4×4 block of steel armour.
    let block = |centre: &str, size: i32| {
        let mut parts: Vec<P> = vec![(centre, 1, 1, 0, false, None)];
        for y in 0..4 {
            for x in 0..4 {
                if !(1..1 + size).contains(&x) || !(1..1 + size).contains(&y) {
                    parts.push(("armor_block", x, y, 0, false, Some("steel")));
                }
            }
        }
        custom(&c, &parts)
    };
    // Fission reactor, radius 1 m: the eight cells around it, steel taking half of 1.5.
    let d = block("fission_reactor", 1);
    let r = find(&d, "fission_reactor", 1, 1);
    let hits = d.blast.get(r as usize);
    assert_eq!(hits.len(), 8);
    assert!(hits.iter().all(|&(p, dmg)| d.parts[p as usize].hull && dmg == 0.75));
    // Capacitor, radius 0.6 m: the four beside it.
    let d = block("capacitor", 1);
    assert_eq!(d.blast.get(find(&d, "capacitor", 1, 1) as usize).len(), 4);
    // Fusion reactor (2 × 2), radius 1.5 m: the twelve cells around it, not the corners.
    let d = block("fusion_reactor", 2);
    let hits = d.blast.get(find(&d, "fusion_reactor", 1, 1) as usize);
    assert_eq!(hits.len(), 12);
    assert!(hits.iter().all(|&(_, dmg)| dmg == 1.25));
    // Parts without an explosion hit nothing.
    assert!(d.blast.get(find(&d, "armor_block", 0, 0) as usize).is_empty());
}

#[test]
fn a_reactor_shot_off_destroys_its_cannon_and_armour_survives() {
    let c = content();
    let d = custom(
        &c,
        &[
            ("armor_block", 0, 0, 0, false, Some("steel")),
            ("fission_reactor", 1, 0, 0, false, None),
            ("automaton_cannon", 2, 0, 0, false, None),
            ("armor_block", 1, 1, 0, false, Some("steel")),
            ("logic_core", 1, 2, 0, false, None),
        ],
    );
    let (mut w, s) = world_with(d.clone());
    let reactor = find(&d, "fission_reactor", 1, 0);
    let cannon = find(&d, "automaton_cannon", 2, 0);
    w.apply_damage(s, DamageTarget::Part(reactor), 1.0);
    w.resolve_breakage();
    // The reactor goes, explodes, and the cannon beside it (1 hit point) goes with it, all at once.
    assert_eq!(
        part_events(&w),
        [(reactor, false), (reactor, true), (cannon, false)]
    );
    let blast = w
        .events()
        .iter()
        .find_map(|e| match *e {
            SimEvent::Explosion { radius, damage, .. } => Some((radius, damage)),
            _ => None,
        })
        .unwrap();
    assert_eq!(blast, (1.0, 1.5));
    let ship = w.ship(s).unwrap();
    assert!(
        w.events()
            .contains(&SimEvent::PartDestroyed { ship: s, piece: ship.part(reactor).piece, part: reactor, volatile: true })
    );
    // Steel armour takes half the blast (0.75 of its 1 hit point); the core, out of reach, nothing.
    for (x, y) in [(0, 0), (1, 1)] {
        assert_eq!(ship.part(find(&d, "armor_block", x, y)).hp, 0.25);
    }
    let core = find(&d, "logic_core", 1, 2);
    assert_eq!(ship.part(core).hp, d.parts[core as usize].hp);
    assert!(ship.control_piece().is_some());
}

#[test]
fn reactors_in_a_row_go_off_one_after_another_in_one_resolution() {
    let c = content();
    let d = custom(
        &c,
        &[
            ("fission_reactor", 0, 0, 0, false, None),
            ("fission_reactor", 1, 0, 0, false, None),
            ("fission_reactor", 2, 0, 0, false, None),
            ("automaton_cannon", 3, 0, 0, false, None),
            ("armor_block", 3, 1, 0, false, Some("steel")),
            ("armor_block", 3, 2, 0, false, Some("steel")),
            ("logic_core", 3, 3, 0, false, None),
        ],
    );
    let (mut w, s) = world_with(d.clone());
    let f: Vec<PartIdx> = (0..3).map(|x| find(&d, "fission_reactor", x, 0)).collect();
    let cannon = find(&d, "automaton_cannon", 3, 0);
    // Only the first is shot; the last one's blast is the only one that reaches the cannon.
    w.apply_damage(s, DamageTarget::Part(f[0]), 1.0);
    w.step(&TickInputs::default());
    // First destroyed, first to explode: each reactor's blast takes the next, the third's takes the cannon.
    assert_eq!(
        part_events(&w),
        [
            (f[0], false),
            (f[0], true),
            (f[1], false),
            (f[1], true),
            (f[2], false),
            (f[2], true),
            (cannon, false)
        ]
    );
    let ship = w.ship(s).unwrap();
    assert_eq!(ship.part(find(&d, "armor_block", 3, 1)).hp, 0.25);
    assert_eq!(ship.part(find(&d, "armor_block", 3, 2)).hp, 1.0);
    assert_eq!(ship.pieces().len(), 1);
    assert!(ship.verify_caches().is_ok());
}

#[test]
fn a_blast_stays_on_its_piece() {
    let c = content();
    // The slope beside the reactor turns its sloped side to it: not linked, a piece of its own from the start.
    let d = custom(
        &c,
        &[
            ("fission_reactor", 0, 0, 0, false, None),
            ("slope_1x1", 1, 0, 0, true, Some("steel")),
            ("armor_block", 0, 1, 0, false, Some("steel")),
            ("logic_core", 0, 2, 0, false, None),
        ],
    );
    let reactor = find(&d, "fission_reactor", 0, 0);
    let slope = find(&d, "slope_1x1", 1, 0);
    assert!(d.neighbors.get(slope as usize).is_empty());
    assert!(d.blast.get(reactor as usize).iter().any(|b| b.0 == slope), "in reach");
    let (mut w, s) = world_with(d.clone());
    assert_ne!(
        w.ship(s).unwrap().part(slope).piece,
        w.ship(s).unwrap().part(reactor).piece
    );
    w.apply_damage(s, DamageTarget::Part(reactor), 1.0);
    w.resolve_breakage();
    let ship = w.ship(s).unwrap();
    assert_eq!(ship.part(slope).hp, d.parts[slope as usize].hp, "another piece");
    assert_eq!(ship.part(find(&d, "armor_block", 0, 1)).hp, 0.25);
}

/// The same fight twice, with reactors and drives blowing up: identical hashes every tick and identical events;
/// without chain reactions the run differs.
#[test]
fn chain_reactions_are_deterministic() {
    let run = |c: &data::Content| {
        let d = design(c, "rim_skiff");
        let (mut w, s) = world_with(d.clone());
        let mut hashes = Vec::new();
        let mut events = Vec::new();
        // One reactor shot off, then a thruster and the other reactor.
        for (t, cell) in [(0, (1, 2)), (20, (6, 3)), (40, (6, 2))] {
            while hashes.len() < t {
                w.step(&TickInputs::default());
                hashes.push(w.state_hash());
                events.extend(w.events().iter().cloned());
            }
            w.apply_damage(s, DamageTarget::Cell(cell), 5.0);
        }
        for _ in 0..30 {
            w.step(&TickInputs::default());
            hashes.push(w.state_hash());
            events.extend(w.events().iter().cloned());
        }
        let explosions = events
            .iter()
            .filter(|e| matches!(e, SimEvent::Explosion { .. }))
            .count();
        (hashes, events, explosions)
    };
    let c = content();
    let (a, ea, n) = run(&c);
    let (b, eb, _) = run(&c);
    assert!(n > 3, "{n} explosions");
    assert_eq!(a, b);
    assert_eq!(ea, eb);
    let (inert, _, none) = run(&content_without_explosions());
    assert_eq!(none, 0);
    assert_ne!(a, inert);
}
