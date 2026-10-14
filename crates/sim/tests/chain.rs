//! Chain reactions (GDR-0019, GDR-0084, GDR-0085): destroyed parts with an `explosion` damage the parts in reach,
//! on any piece of any ship, less what armour in between and other ships' bubbles hold back, and what the blasts
//! destroy explodes in turn, in one breakage resolution.

mod common;

use std::sync::Arc;

use common::*;
use sim::{Body, DamageTarget, Owner, PartIdx, ShipDesign, ShipId, Sim, SimEvent, TickInputs, Vec2};

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
    // A part of each kind in the middle of a block of steel armour, two cells deep all round.
    let block = |centre: &str, size: i32| {
        let mut parts: Vec<P> = vec![(centre, 2, 2, 0, false, None)];
        for y in 0..4 + size {
            for x in 0..4 + size {
                if !(2..2 + size).contains(&x) || !(2..2 + size).contains(&y) {
                    parts.push(("armor_block", x, y, 0, false, Some("steel")));
                }
            }
        }
        custom(&c, &parts)
    };
    // Every target with its cover: `(x, y, damage before cover, cover cells)`, row by row.
    let reach = |d: &ShipDesign, p: PartIdx| {
        d.blast
            .get(p as usize)
            .iter()
            .map(|t| {
                let s = &d.parts[t.part as usize].shape;
                let cover: Vec<(i32, i32)> = d
                    .blast_cover
                    .get(t.cover as usize)
                    .iter()
                    .map(|&c| (d.parts[c as usize].shape.x, d.parts[c as usize].shape.y))
                    .collect();
                (s.x, s.y, t.damage, cover)
            })
            .collect::<Vec<_>>()
    };
    // Fission reactor, radius 1.5 m (GDR-0085): the eight cells around it and the next one straight out on each
    // side, behind a steel block (steel taking half of 1.5, and letting half through).
    let d = block("fission_reactor", 1);
    let hits = reach(&d, find(&d, "fission_reactor", 2, 2));
    assert_eq!(hits.len(), 12);
    assert!(hits.iter().all(|h| h.2 == 0.75));
    for h in &hits {
        let (dx, dy) = (h.0 - 2, h.1 - 2);
        if dx.abs() == 2 || dy.abs() == 2 {
            assert_eq!(dx * dy, 0, "{h:?}: straight out only");
            assert_eq!(h.3, [(2 + dx / 2, 2 + dy / 2)], "{h:?}: behind the block beside the reactor");
        } else {
            assert!(h.3.is_empty(), "{h:?}: nothing between");
        }
    }
    // Capacitor, radius 0.6 m: the four beside it.
    let d = block("capacitor", 1);
    assert_eq!(d.blast.get(find(&d, "capacitor", 2, 2) as usize).len(), 4);
    // Fusion reactor (2 × 2), radius 2 m: the twelve cells around it (corners too) and the eight two cells straight
    // out; those behind a block of the ring.
    let d = block("fusion_reactor", 2);
    let hits = reach(&d, find(&d, "fusion_reactor", 2, 2));
    assert_eq!(hits.len(), 20);
    assert!(hits.iter().all(|h| h.2 == 1.25));
    assert_eq!(hits.iter().filter(|h| h.3.len() == 1).count(), 8);
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
    assert_eq!(blast, (1.5, 1.5));
    let ship = w.ship(s).unwrap();
    assert!(
        w.events()
            .contains(&SimEvent::PartDestroyed { ship: s, piece: ship.part(reactor).piece, part: reactor, volatile: true })
    );
    // Steel armour takes half the blast (0.75 of its 1 hit point); the core two cells down, behind the block
    // below the reactor, gets only the half the steel lets through (GDR-0085) and survives.
    for (x, y) in [(0, 0), (1, 1)] {
        assert_eq!(ship.part(find(&d, "armor_block", x, y)).hp, 0.25);
    }
    let core = find(&d, "logic_core", 1, 2);
    assert_eq!(ship.part(core).hp, 0.25);
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

/// GDR-0085: a blast reaches the other pieces of its own ship too (found when it goes off, not compiled).
#[test]
fn a_blast_reaches_another_piece_of_its_own_ship() {
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
    assert!(d.blast.get(reactor as usize).iter().any(|b| b.part == slope), "in reach");
    let (mut w, s) = world_with(d.clone());
    assert_ne!(
        w.ship(s).unwrap().part(slope).piece,
        w.ship(s).unwrap().part(reactor).piece
    );
    w.apply_damage(s, DamageTarget::Part(reactor), 1.0);
    w.resolve_breakage();
    let ship = w.ship(s).unwrap();
    // The steel slope has half a cell's hit points (0.5) and takes half the blast (0.75).
    assert!(!ship.part(slope).alive, "another piece, still in reach");
    assert_eq!(ship.part(find(&d, "armor_block", 0, 1)).hp, 0.25);
    assert!(ship.verify_caches().is_ok());
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

/// The base content with `part`'s explosion set to `damage` within `radius` m (tests of the blast rules that need
/// a reach the base numbers do not have).
fn with_blast(part: &str, damage: f32, radius: f32) -> data::Content {
    let mut c = content();
    for p in c.parts.iter_mut().filter(|p| p.id == part) {
        p.explosion = Some(data::ExplosionSpec { damage, radius });
    }
    c
}

/// Moves ship `b` (one piece, unturned) so that the centre of its part `pb` sits `offset` from the centre of ship
/// `a`'s part `pa`.
fn place(w: &mut Sim, a: ShipId, pa: PartIdx, b: ShipId, pb: PartIdx, offset: Vec2) {
    let at = w.ship(a).unwrap().part_world(pa).unwrap().0 + offset;
    let ship = w.ship(b).unwrap();
    let piece = &ship.pieces()[0];
    let mut body = piece.body;
    body.pos += at - ship.part_world(pb).unwrap().0;
    let id = piece.id;
    w.teleport(b, id, body);
}

/// The explosions of the last step or resolution: `(ship, part)`.
fn explosions(w: &Sim) -> Vec<(ShipId, PartIdx)> {
    w.events()
        .iter()
        .filter_map(|e| match *e {
            SimEvent::Explosion { ship, part, .. } => Some((ship, part)),
            _ => None,
        })
        .collect()
}

/// A small exploder (`A`: a fission reactor on the right end of a row with an armour block and a core) and a
/// ship alongside (`B`: a pulse laser on its left end, then armour, a shield emitter, a reactor and a core), the
/// laser's centre 1.3 m right of the reactor's (a 0.3 m gap between the hulls). Returns the world, both ships,
/// both designs.
fn alongside(c: &data::Content, owner_a: Owner, owner_b: Owner) -> (Sim, ShipId, ShipId, Arc<ShipDesign>, Arc<ShipDesign>) {
    let da = custom(
        c,
        &[
            ("logic_core", 0, 0, 0, false, None),
            ("armor_block", 1, 0, 0, false, Some("steel")),
            ("fission_reactor", 2, 0, 0, false, None),
        ],
    );
    let db = custom(
        c,
        &[
            ("pulse_laser", 0, 0, 0, false, None),
            ("armor_block", 1, 0, 0, false, Some("steel")),
            ("shield_emitter", 2, 0, 0, false, None),
            ("fission_reactor", 3, 0, 0, false, None),
            ("logic_core", 4, 0, 0, false, None),
        ],
    );
    let mut w = Sim::new(1);
    let a = w.spawn_ship(da.clone(), owner_a, Body::default());
    let b = w.spawn_ship(db.clone(), owner_b, Body::at(Vec2::new(0.0, 50.0), 0.0));
    let (ra, lb) = (find(&da, "fission_reactor", 2, 0), find(&db, "pulse_laser", 0, 0));
    place(&mut w, a, ra, b, lb, Vec2::new(1.3, 0.0));
    (w, a, b, da, db)
}

/// GDR-0085: a blast reaches the ship alongside, whoever owns it (blasts spare no one, friendly or not).
#[test]
fn a_blast_hurts_a_ship_alongside() {
    let c = content();
    for owner_b in [Owner(1), Owner::PLAYER] {
        let (mut w, a, b, da, db) = alongside(&c, Owner::PLAYER, owner_b);
        let reactor = find(&da, "fission_reactor", 2, 0);
        let laser = find(&db, "pulse_laser", 0, 0);
        w.apply_damage(a, DamageTarget::Part(reactor), 1.0);
        w.resolve_breakage();
        assert_eq!(explosions(&w), [(a, reactor)]);
        // The laser (1 hit point) 0.3 m away goes; the armour behind it is out of reach (1.8 m).
        let ship = w.ship(b).unwrap();
        assert!(!ship.part(laser).alive, "{owner_b:?}");
        assert!(w.events().contains(&SimEvent::PartDestroyed {
            ship: b,
            piece: ship.part(laser).piece,
            part: laser,
            volatile: false
        }));
        let armour = find(&db, "armor_block", 1, 0);
        assert_eq!(ship.part(armour).hp, 1.0);
        // On its own ship the armour block beside the reactor takes half.
        assert_eq!(w.ship(a).unwrap().part(find(&da, "armor_block", 1, 0)).hp, 0.25);
        assert!(ship.verify_caches().is_ok());
    }
}

/// GDR-0085: the bubble of the ship alongside takes the blast first, like any other damage (the blast's centre
/// lies outside it). Without its shield the same blast takes the laser.
#[test]
fn the_shield_of_the_ship_alongside_absorbs_the_blast() {
    let c = content();
    for shields in [true, false] {
        let (mut w, a, b, da, db) = alongside(&c, Owner(1), Owner::PLAYER);
        let reactor = find(&da, "fission_reactor", 2, 0);
        let (laser, emitter) = (find(&db, "pulse_laser", 0, 0), find(&db, "shield_emitter", 2, 0));
        let hold = inputs(b, controls(0, 0.0, false, shields));
        for _ in 0..3 {
            w.step(&hold);
        }
        let charge = w.ship(b).unwrap().part(emitter).shield;
        let mut blast = hold.clone();
        blast.damage.push(sim::DamageCommand {
            ship: a,
            target: DamageTarget::Part(reactor),
            amount: 1.0,
        });
        w.step(&blast);
        assert_eq!(explosions(&w), [(a, reactor)]);
        let ship = w.ship(b).unwrap();
        let absorbed: f32 = w
            .events()
            .iter()
            .filter_map(|e| match *e {
                SimEvent::ShieldHit { by, ship, absorbed, .. } if by == a && ship == b => Some(absorbed),
                _ => None,
            })
            .sum();
        if shields {
            // The bubble absorbs missiles (and so blasts) whole: the laser takes nothing.
            assert!(charge >= 1.5, "{charge}");
            assert!(close(absorbed, 1.5, 1e-5), "{absorbed}");
            assert!(ship.part(laser).alive);
            assert_eq!(ship.part(laser).hp, 1.0);
        } else {
            assert_eq!(absorbed, 0.0);
            assert!(!ship.part(laser).alive);
        }
    }
}

/// GDR-0085: the exploding ship's own bubbles never help its parts (the blast goes off inside them).
#[test]
fn a_ships_own_shield_does_not_absorb_its_blast() {
    let c = content();
    let d = custom(
        &c,
        &[
            ("shield_emitter", 0, 0, 0, false, None),
            ("armor_block", 1, 0, 0, false, Some("steel")),
            ("fission_reactor", 0, 1, 0, false, None),
            ("pulse_laser", 1, 1, 0, false, None),
            ("logic_core", 0, 2, 0, false, None),
            ("armor_block", 1, 2, 0, false, Some("steel")),
        ],
    );
    let (mut w, s) = world_with(d.clone());
    let hold = inputs(s, controls(0, 0.0, false, true));
    for _ in 0..3 {
        w.step(&hold);
    }
    assert!(!w.bubbles().is_empty());
    let laser = find(&d, "pulse_laser", 1, 1);
    w.apply_damage(s, DamageTarget::Part(find(&d, "fission_reactor", 0, 1)), 1.0);
    w.resolve_breakage();
    assert!(!w.ship(s).unwrap().part(laser).alive);
    assert!(!w.events().iter().any(|e| matches!(e, SimEvent::ShieldHit { .. })));
}

/// GDR-0085: intact armour on the line from the blast's centre to a part lets only its `blast_factor` through;
/// a machine in between shields nothing. The fission reactor reaches 2 m for the test (two cells deep).
#[test]
fn armour_between_the_blast_and_a_part_shields_it() {
    for (material, through) in [("steel", 0.5), ("reactive", 0.25)] {
        let c = with_blast("fission_reactor", 1.5, 2.0);
        let d = custom(
            &c,
            &[
                ("fission_reactor", 2, 2, 0, false, None),
                // Right: armour, then a laser behind it.
                ("armor_block", 3, 2, 0, false, Some(material)),
                ("pulse_laser", 4, 2, 0, false, None),
                // Down: a cargo bay, then a laser behind it.
                ("cargo_bay", 2, 3, 0, false, None),
                ("pulse_laser", 2, 4, 0, false, None),
                // Left: armour up to the core, out of reach in the corner.
                ("armor_block", 1, 2, 0, false, Some("steel")),
                ("armor_block", 0, 2, 0, false, Some("steel")),
                ("armor_block", 0, 1, 0, false, Some("steel")),
                ("logic_core", 0, 0, 0, false, None),
            ],
        );
        let reactor = find(&d, "fission_reactor", 2, 2);
        let (shielded, open) = (find(&d, "pulse_laser", 4, 2), find(&d, "pulse_laser", 2, 4));
        let armour = find(&d, "armor_block", 3, 2);
        // The compiled cover: the armour block for the laser behind it, nothing for the one behind the bay.
        let blast = d.blast.get(reactor as usize);
        let cover = |p: PartIdx| {
            let t = blast.iter().find(|t| t.part == p).expect("in reach");
            d.blast_cover.get(t.cover as usize).to_vec()
        };
        assert_eq!(cover(shielded), [armour]);
        assert!(cover(open).is_empty());
        assert!(cover(armour).is_empty());
        let (mut w, s) = world_with(d.clone());
        w.apply_damage(s, DamageTarget::Part(reactor), 1.0);
        w.resolve_breakage();
        let ship = w.ship(s).unwrap();
        // The laser behind the armour takes only what gets through and survives; the one behind the bay goes.
        assert!(close(ship.part(shielded).hp, 1.0 - 1.5 * through, 1e-6), "{material}");
        assert!(!ship.part(open).alive, "{material}");
        let full = d.parts[armour as usize].hp;
        assert!(close(ship.part(armour).hp, full - 1.5 * through, 1e-6), "{material}");
    }
}

/// GDR-0085: armour counts while it is intact when a blast goes off: within one blast every hit is computed first
/// (armour the blast breaks still shields what lies behind it from that blast), and a later blast of the chain
/// meets only what is left. Fission reactors deal 2 within 2 m for the test.
#[test]
fn armour_shields_while_it_is_intact_when_the_blast_goes_off() {
    let c = with_blast("fission_reactor", 2.0, 2.0);
    let d = custom(
        &c,
        &[
            ("fission_reactor", 2, 0, 0, false, None),
            ("cargo_bay", 2, 1, 0, false, None),
            ("fission_reactor", 2, 2, 0, false, None),
            // Steel (1 hit point, takes half), then composite (1.4 hit points, takes half) behind it.
            ("armor_block", 3, 2, 0, false, Some("steel")),
            ("armor_block", 4, 2, 0, false, Some("composite")),
            ("armor_block", 4, 3, 0, false, Some("steel")),
            ("armor_block", 4, 4, 0, false, Some("steel")),
            ("logic_core", 4, 5, 0, false, None),
        ],
    );
    let (upper, lower) = (find(&d, "fission_reactor", 2, 0), find(&d, "fission_reactor", 2, 2));
    let (steel, behind) = (find(&d, "armor_block", 3, 2), find(&d, "armor_block", 4, 2));
    // The lower reactor alone: its blast breaks the steel block (1 of 1) and still loses half to it on the way
    // to the composite block (2 × 0.5 × 0.5). Then it takes the upper reactor, which reaches neither block.
    let (mut w, s) = world_with(d.clone());
    w.apply_damage(s, DamageTarget::Part(lower), 1.0);
    w.resolve_breakage();
    let ship = w.ship(s).unwrap();
    assert!(!ship.part(steel).alive && !ship.part(upper).alive);
    assert!(close(ship.part(behind).hp, 1.4 - 0.5, 1e-6));
    assert_eq!(explosions(&w), [(s, lower), (s, upper)]);
    // The upper reactor first: its blast breaks the steel block (nothing between) and takes the lower
    // reactor, whose blast then meets no armour on its way to the composite block (2 × 0.5).
    let (mut w, s) = world_with(d.clone());
    w.apply_damage(s, DamageTarget::Part(upper), 1.0);
    w.resolve_breakage();
    let ship = w.ship(s).unwrap();
    assert!(!ship.part(steel).alive && !ship.part(lower).alive);
    assert!(close(ship.part(behind).hp, 1.4 - 1.0, 1e-6));
    assert_eq!(explosions(&w), [(s, upper), (s, lower)]);
}

/// GDR-0085: a chain runs across ships. Ship A's reactor takes ship B's reactor alongside (0.3 m right of it),
/// whose blast takes B's laser and finishes the two armour blocks below A's reactor that A's own blast left at a
/// quarter (the one right below it behind the other: half of B's blast gets through, still enough). Fission
/// reactors deal 1.5 within 1 m (pinned for the test).
#[test]
fn a_chain_reaction_runs_across_two_ships() {
    let c = with_blast("fission_reactor", 1.5, 1.0);
    let da = custom(
        &c,
        &[
            ("logic_core", 0, 0, 0, false, None),
            ("armor_block", 1, 0, 0, false, Some("steel")),
            ("fission_reactor", 2, 0, 0, false, None),
            ("armor_block", 2, 1, 0, false, Some("steel")),
            ("armor_block", 3, 1, 0, false, Some("steel")),
        ],
    );
    let db = custom(
        &c,
        &[
            ("fission_reactor", 0, 0, 0, false, None),
            ("pulse_laser", 1, 0, 0, false, None),
            ("armor_block", 2, 0, 0, false, Some("steel")),
            ("logic_core", 3, 0, 0, false, None),
        ],
    );
    let mut w = Sim::new(1);
    let a = w.spawn_ship(da.clone(), Owner::PLAYER, Body::default());
    let b = w.spawn_ship(db.clone(), Owner(1), Body::at(Vec2::new(0.0, 50.0), 0.0));
    let (ra, rb) = (find(&da, "fission_reactor", 2, 0), find(&db, "fission_reactor", 0, 0));
    place(&mut w, a, ra, b, rb, Vec2::new(1.3, 0.0));
    w.apply_damage(a, DamageTarget::Part(ra), 1.0);
    w.resolve_breakage();
    assert_eq!(explosions(&w), [(a, ra), (b, rb)]);
    let left = find(&da, "armor_block", 1, 0);
    let (below, corner) = (find(&da, "armor_block", 2, 1), find(&da, "armor_block", 3, 1));
    let laser = find(&db, "pulse_laser", 1, 0);
    let (sa, sb) = (w.ship(a).unwrap(), w.ship(b).unwrap());
    assert!(!sa.part(below).alive && !sa.part(corner).alive);
    assert_eq!(sa.part(left).hp, 0.25, "out of B's reach");
    assert!(!sb.part(rb).alive && !sb.part(laser).alive);
    assert_eq!(sb.part(find(&db, "armor_block", 2, 0)).hp, 1.0, "out of reach behind the laser");
    // Each ship reports its own losses in the order they died: B's blast hit its own piece first, then A's.
    let order: Vec<(ShipId, PartIdx)> = w
        .events()
        .iter()
        .filter_map(|e| match *e {
            SimEvent::PartDestroyed { ship, part, .. } => Some((ship, part)),
            _ => None,
        })
        .collect();
    assert_eq!(order, [(a, ra), (a, below), (a, corner), (b, rb), (b, laser)]);
    assert!(sa.verify_caches().is_ok() && sb.verify_caches().is_ok());
}

/// GDR-0085: the automaton core, the shield emitter and the RCS pod explode too, each into a steel block beside it.
#[test]
fn cores_emitters_and_rcs_pods_explode() {
    let c = content();
    for (part, damage, radius) in [
        ("automaton_core", 1.5, 1.5),
        ("shield_emitter", 1.0, 0.6),
        ("rcs_pod", 0.5, 0.6),
    ] {
        let e = c.parts.get(part).unwrap().explosion.clone().expect(part);
        assert_eq!((e.damage, e.radius), (damage, radius), "{part}");
        let d = custom(
            &c,
            &[
                (part, 1, 1, 0, false, None),
                ("armor_block", 2, 1, 0, false, Some("steel")),
                ("armor_block", 3, 1, 0, false, Some("steel")),
                ("logic_core", 4, 1, 0, false, None),
            ],
        );
        let p = find(&d, part, 1, 1);
        let (mut w, s) = world_with(d.clone());
        w.apply_damage(s, DamageTarget::Part(p), 10.0);
        w.resolve_breakage();
        assert_eq!(explosions(&w), [(s, p)], "{part}");
        let block = w.ship(s).unwrap().part(find(&d, "armor_block", 2, 1)).hp;
        assert!(close(block, 1.0 - damage * 0.5, 1e-6), "{part}: {block}");
    }
}

/// The ships of [`alongside`] blowing up into each other, twice: the exploder's reactor into the shielded ship's
/// bubble, then the shielded ship's own reactor and emitter, then the bubble down and the exploder's core: identical
/// hashes and events every tick; without chain reactions the run differs.
#[test]
fn chain_reactions_across_ships_are_deterministic() {
    let run = |c: &data::Content| {
        let (mut w, a, b, _, _) = alongside(c, Owner(1), Owner::PLAYER);
        let mut hashes = Vec::new();
        let mut events = Vec::new();
        for (t, ship, cell, shields) in [
            (5, a, (2, 0), true),
            (15, b, (3, 0), true),
            (25, a, (1, 0), false),
            (26, a, (0, 0), false),
        ] {
            while hashes.len() < t {
                w.step(&inputs(b, controls(0, 0.0, false, shields)));
                hashes.push(w.state_hash());
                events.extend(w.events().iter().cloned());
            }
            w.apply_damage(ship, DamageTarget::Cell(cell), 5.0);
        }
        for _ in 0..30 {
            w.step(&TickInputs::default());
            hashes.push(w.state_hash());
            events.extend(w.events().iter().cloned());
        }
        let count = |f: fn(&SimEvent) -> bool| events.iter().filter(|e| f(e)).count();
        let blasts = count(|e| matches!(e, SimEvent::Explosion { .. }));
        let absorbed = count(|e| matches!(e, SimEvent::ShieldHit { .. }));
        (hashes, events, blasts, absorbed)
    };
    let c = content();
    let (a, ea, blasts, absorbed) = run(&c);
    let (b, eb, _, _) = run(&c);
    assert!(blasts >= 3 && absorbed > 0, "{blasts} blasts, {absorbed} shield hits");
    assert_eq!(a, b);
    assert_eq!(ea, eb);
    let (inert, _, none, _) = run(&content_without_explosions());
    assert_eq!(none, 0);
    assert_ne!(a, inert);
}
