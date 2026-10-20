//! The Phase walkers' mechanics (GDR-0127): the blink drive (charge, jump, cooldown, blocked landing spots, drives
//! carrying a ship's mass, a split-off section blinking on its own, the behavior words) and the lance (every part on
//! its line through the struck piece, weaker after each cell, more after armour; it stops where the piece ends);
//! determinism.

mod common;

use common::*;
use sim::{Body, DamageTarget, Owner, ShipCommand, ShipId, Sim, SimEvent, TickInputs, Vec2};

fn tick(w: &mut Sim) {
    w.step(&TickInputs::default());
}

/// The ship's controlling piece's centre of mass.
fn com(w: &Sim, s: ShipId) -> Vec2 {
    let ship = w.ship(s).unwrap();
    ship.piece(ship.control_piece().unwrap()).unwrap().com_world()
}

/// A world with one walker skirmisher (owner 1) at the origin, facing +y.
fn skirmisher() -> (Sim, ShipId) {
    let c = content();
    let mut w = Sim::new(3);
    let s = w.spawn_ship(design(&c, "walker_skirmisher"), Owner(1), Body::default());
    (w, s)
}

/// Every `Blinked` of ship `s` over `ticks` ticks: the tick, from, to.
fn blinks(w: &mut Sim, s: ShipId, ticks: usize, mut each: impl FnMut(&mut Sim)) -> Vec<(u64, Vec2, Vec2)> {
    let mut out = Vec::new();
    for _ in 0..ticks {
        each(w);
        tick(w);
        for e in w.events() {
            if let SimEvent::Blinked { ship, from, to, .. } = *e
                && ship == s
            {
                out.push((w.tick(), from, to));
            }
        }
    }
    out
}

#[test]
fn a_blink_charges_then_jumps_the_full_reach_and_cools_down() {
    let (mut w, s) = skirmisher();
    tick(&mut w);
    assert!(w.ship(s).unwrap().can_blink());
    let start = com(&w, s);
    w.command(s, ShipCommand::Blink(Vec2::new(1.0, 0.0)));
    // It charges for 0.6 s (18 ticks at full power), shimmering, and has not moved yet.
    for _ in 0..15 {
        tick(&mut w);
    }
    let ship = w.ship(s).unwrap();
    assert!(ship.blink_charge().is_some_and(|c| c > 0.7 && c < 1.0), "{:?}", ship.blink_charge());
    assert!(!ship.can_blink());
    assert!((com(&w, s) - start).length() < 0.5);
    let jumps = blinks(&mut w, s, 10, |_| {});
    assert_eq!(jumps.len(), 1, "{jumps:?}");
    let (_, from, to) = jumps[0];
    assert!((to - from - Vec2::new(150.0, 0.0)).length() < 0.5, "{from:?} -> {to:?}");
    tick(&mut w);
    assert!((com(&w, s) - to).length() < 0.5);
    // Speed and heading kept (it was standing still, facing +y).
    let p = w.ship(s).unwrap().pieces()[0].body;
    assert!(p.linvel.length() < 0.1 && p.angle.abs() < 0.01);
    // It cools down 4 s.
    let ship = w.ship(s).unwrap();
    assert!(!ship.can_blink() && ship.blink_cooldown() > 3.5);
    for _ in 0..(4 * 30) {
        tick(&mut w);
    }
    assert!(w.ship(s).unwrap().can_blink());
}

/// A salvage wall of steel blocks (no core) 6 × 6 at `at`.
fn wall(w: &mut Sim, at: Vec2) {
    let c = content();
    let mut parts = Vec::new();
    for x in 0..6 {
        for y in 0..6 {
            parts.push(("armor_block", x, y, 0u8, false, Some("steel")));
        }
    }
    w.spawn_ship(custom(&c, &parts), Owner(2), Body::at(at, 0.0));
}

#[test]
fn a_blocked_landing_spot_shortens_the_jump_and_a_full_line_fizzles() {
    let (mut w, s) = skirmisher();
    wall(&mut w, Vec2::new(150.0, 0.0));
    tick(&mut w);
    w.command(s, ShipCommand::Blink(Vec2::new(1.0, 0.0)));
    let jumps = blinks(&mut w, s, 40, |_| {});
    assert_eq!(jumps.len(), 1);
    let (_, from, to) = jumps[0];
    assert!(((to - from).x - 112.5).abs() < 0.5, "{from:?} -> {to:?}");

    let (mut w, s) = skirmisher();
    for d in [37.5, 75.0, 112.5, 150.0] {
        wall(&mut w, Vec2::new(d, 0.0));
    }
    tick(&mut w);
    let start = com(&w, s);
    w.command(s, ShipCommand::Blink(Vec2::new(1.0, 0.0)));
    let mut fizzled = false;
    let jumps = blinks(&mut w, s, 40, |w| {
        fizzled |= w.events().iter().any(|e| matches!(e, SimEvent::BlinkBlocked { ship, .. } if *ship == s));
    });
    fizzled |= w.events().iter().any(|e| matches!(e, SimEvent::BlinkBlocked { ship, .. } if *ship == s));
    assert!(jumps.is_empty() && fizzled);
    assert!((com(&w, s) - start).length() < 1.0);
    // The drive cools down half its cooldown.
    let cool = w.ship(s).unwrap().blink_cooldown();
    assert!(cool > 1.0 && cool <= 2.0, "{cool}");
}

#[test]
fn a_ship_heavier_than_its_drives_carry_cannot_blink() {
    let c = content();
    // A core, a reactor and a drive in a block of steel: about 31 t against one drive's 22.
    let mut parts = vec![
        ("phase_core", 1, 1, 0u8, false, None),
        ("blink_drive", 2, 1, 0, false, None),
        ("fission_reactor", 1, 2, 0, false, None),
    ];
    for (x, y) in [(0, 0), (1, 0), (2, 0), (3, 0), (0, 1), (3, 1), (0, 2), (3, 2), (0, 3), (1, 3), (2, 3), (3, 3)] {
        parts.push(("armor_block", x, y, 0, false, Some("steel")));
    }
    let mut w = Sim::new(1);
    let one = w.spawn_ship(custom(&c, &parts), Owner(1), Body::default());
    parts.push(("blink_drive", 2, 2, 0, false, None));
    let two = w.spawn_ship(custom(&c, &parts), Owner(1), Body::at(Vec2::new(0.0, 60.0), 0.0));
    tick(&mut w);
    assert!(!w.ship(one).unwrap().can_blink());
    assert!(w.ship(two).unwrap().can_blink());
    // A blink takes both drives: both cool down.
    w.command(two, ShipCommand::Blink(Vec2::new(-1.0, 0.0)));
    assert_eq!(blinks(&mut w, two, 40, |_| {}).len(), 1);
    let ship = w.ship(two).unwrap();
    assert!(!ship.can_blink() && ship.blink_cooldown() > 3.0);
}

#[test]
fn the_colossus_blinks_whole_only_with_all_four_drives_and_its_sections_alone_once_shot_apart() {
    let c = content();
    let d = design(&c, "walker_colossus");
    assert_eq!(d.blinks.len(), 4);
    let mut w = Sim::new(5);
    let s = w.spawn_ship(d.clone(), Owner(1), Body::default());
    tick(&mut w);
    assert!(w.ship(s).unwrap().can_blink());
    // One drive gone: three do not carry it.
    let head_drive = find(&d, "blink_drive", 6, 1);
    w.apply_damage(s, DamageTarget::Part(head_drive), 100.0);
    tick(&mut w);
    assert!(!w.ship(s).unwrap().can_blink());
    // The left wing's bridge shot through: the wing is a vessel of its own, with its own drive.
    let bridge = find(&d, "armor_block", 3, 5);
    w.apply_damage(s, DamageTarget::Part(bridge), 100.0);
    tick(&mut w);
    let wing = w
        .ships()
        .iter()
        .find(|x| x.id() != s && x.is_vessel())
        .map(|x| x.id())
        .expect("the wing became a vessel");
    tick(&mut w);
    assert!(w.ship(wing).unwrap().can_blink());
    w.command(wing, ShipCommand::Blink(Vec2::new(-1.0, 0.0)));
    assert_eq!(blinks(&mut w, wing, 40, |_| {}).len(), 1);
}

/// A target dummy (owner 0, the player's): a 6 × 6 block of reactive plate with two cores and a reactor in its outer
/// columns (off the line a lance takes from straight behind), `at`, facing +y.
fn dummy(w: &mut Sim, at: Vec2) -> ShipId {
    let c = content();
    let mut parts = vec![
        ("logic_core", 0, 1, 0u8, false, None),
        ("logic_core", 5, 1, 0, false, None),
        ("fission_reactor", 0, 4, 0, false, None),
    ];
    for x in 0..6 {
        for y in 0..6 {
            if !matches!((x, y), (0, 1) | (5, 1) | (0, 4)) {
                parts.push(("armor_block", x, y, 0, false, Some("reactive")));
            }
        }
    }
    w.spawn_ship(custom(&c, &parts), Owner::PLAYER, Body::at(at, 0.0))
}

#[test]
fn the_walker_rules_blink_in_behind_the_target_then_out_again() {
    let c = content();
    let (mut w, s) = skirmisher();
    let t = dummy(&mut w, Vec2::new(0.0, 160.0));
    fight(&mut w, &c, s);
    let jumps = blinks(&mut w, s, 30 * 20, |_| {});
    assert!(jumps.len() >= 2, "{jumps:?}");
    // In: it lands behind the target's tail (the dummy faces +y, so its tail looks at the walker), close in.
    let target = com(&w, t);
    let (_, _, to) = jumps[0];
    assert!((to - target).length() < 40.0, "landed {to:?}, target {target:?}");
    // Out: about 150 m straight away from the target, once the drive has cooled (4 s).
    let (t1, from, to) = jumps[1];
    assert!(t1 - jumps[0].0 >= 4 * 30);
    assert!((to - target).length() > (from - target).length() + 100.0);
}

/// A ship with one phase lance on a reactor and a core (a single column), facing +y, at the origin.
fn lancer() -> Vec<P<'static>> {
    vec![("phase_lance", 0, 0, 0, false, None), ("fission_reactor", 0, 1, 0, false, None), ("logic_core", 0, 2, 0, false, None)]
}

/// The struck parts of the first lance shot by `s` at a single column of `column` (listed from the far end; the
/// lance enters at the last), 20 m ahead; a second column right behind it in a ship of its own.
fn lance_hits(column: &[P<'static>]) -> (Vec<(String, f32)>, u32, bool) {
    let c = content_without_explosions();
    let mut w = Sim::new(9);
    let s = w.spawn_ship(custom(&c, &lancer()), Owner::PLAYER, Body::default());
    let len = column.len() as f32;
    let t = w.spawn_ship(custom(&c, column), Owner(1), Body::at(Vec2::new(0.0, 20.0 + len / 2.0), 0.0));
    let behind = w.spawn_ship(custom(&c, &[("armor_block", 0, 0, 0, false, Some("steel"))]), Owner(1), Body::at(Vec2::new(0.0, 22.0 + len), 0.0));
    let mut hits = Vec::new();
    let mut lanced = 0;
    let mut behind_hit = false;
    // Guns start staggered over one firing period (2 s).
    for _ in 0..90 {
        w.step(&inputs(s, controls(0, 0.0, true, true)));
        for e in w.events() {
            match e {
                SimEvent::Hit { by, ship, part, damage, .. } if *by == s && *ship == t => {
                    hits.push((w.ship(t).unwrap().design().parts[*part as usize].id.clone(), *damage));
                }
                SimEvent::Hit { ship, .. } if *ship == behind => behind_hit = true,
                SimEvent::Lanced { by, parts, .. } if *by == s => lanced = *parts,
                _ => {}
            }
        }
        if lanced > 0 {
            break;
        }
    }
    (hits, lanced, behind_hit)
}

#[test]
fn a_lance_strikes_every_part_on_its_line_weaker_after_each_cell() {
    // Far to near: a core, a reactor, two steel blocks. The lance enters the near block.
    let column = [
        ("logic_core", 0, 0, 0u8, false, None),
        ("fission_reactor", 0, 1, 0, false, None),
        ("armor_block", 0, 2, 0, false, Some("steel")),
        ("armor_block", 0, 3, 0, false, Some("steel")),
    ];
    let (hits, lanced, behind) = lance_hits(&column);
    assert_eq!(lanced, 4, "{hits:?}");
    let names: Vec<&str> = hits.iter().map(|h| h.0.as_str()).collect();
    assert_eq!(names, ["armor_block", "armor_block", "fission_reactor", "logic_core"]);
    // Shot damage 1.6 / 0.5 = 3.2; after each armour cell half of it, after the reactor 80 %.
    let d: Vec<f32> = hits.iter().map(|h| h.1).collect();
    let want = [3.2, 1.6, 0.8, 0.64];
    for (a, b) in d.iter().zip(want) {
        assert!((a - b).abs() < 1e-4, "{d:?}");
    }
    // It stops where the line leaves the piece: the block right behind (a ship of its own) takes nothing.
    assert!(!behind);
}

#[test]
fn a_lance_ends_where_the_hull_has_a_gap() {
    // Far to near: a block, a gap, a block: only the near block is struck.
    let column = [("armor_block", 0, 0, 0u8, false, Some("steel")), ("armor_block", 0, 2, 0, false, Some("steel"))];
    let (hits, lanced, _) = lance_hits(&column);
    // The two blocks are two pieces (nothing links them); the lance strikes the near one only.
    assert_eq!(lanced, 1, "{hits:?}");
}

#[test]
fn blinks_and_lances_are_deterministic() {
    let run = || {
        let c = content();
        let (mut w, s) = skirmisher();
        dummy(&mut w, Vec2::new(0.0, 160.0));
        fight(&mut w, &c, s);
        let mut hashes = Vec::new();
        for _ in 0..(30 * 20) {
            tick(&mut w);
            hashes.push(w.state_hash());
        }
        hashes
    };
    assert_eq!(run(), run());
}
