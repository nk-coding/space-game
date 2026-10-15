//! The drydock editing model (`data::design::edit`): legality, the mirror and turning math against the geometry, bevel
//! corners, slope orientation and turning, fill, strokes, undo/redo, centring and trimming, the Halvex Runner
//! round trip, and mirrored edits keeping a symmetric design symmetric.

use std::collections::BTreeSet;

use data::design::edit::{Drydock, OpenError, Outcome, Preview, Refusal, Twin, UNDO_STEPS};
use data::{Blueprint, Content, Dir, Placement, PowerOverride, PowerTier, Shape};

fn content() -> Content {
    data::base_content()
}

fn runner(c: &Content) -> Blueprint {
    c.ships.get("halvex_runner").unwrap().clone()
}

/// An empty design in a `w × h` frame.
fn empty(c: &Content, frame: (u32, u32)) -> Drydock {
    let blank = Blueprint {
        parts: Vec::new(),
        ..runner(c)
    };
    Drydock::open(c, &blank, frame).unwrap()
}

fn pl(part: &str, x: i32, y: i32, rot: u8, flip: bool, material: Option<&str>) -> Placement {
    Placement {
        part: part.into(),
        x,
        y,
        rot,
        flip,
        material: material.map(Into::into),
        power: None,
        group: None,
    }
}

/// A placement of `part` with the material it needs (composite for hull parts).
fn any(c: &Content, part: &str, x: i32, y: i32, rot: u8, flip: bool) -> Placement {
    let hull = c.parts.get(part).unwrap().hull;
    pl(part, x, y, rot, flip, hull.then_some("composite"))
}

/// One part of every footprint: 1×1, the 1×2 beam, 2×2, and the three slopes.
const FOOTPRINTS: [&str; 6] = [
    "armor_block",
    "beam_laser",
    "fusion_reactor",
    "slope_1x1",
    "slope_1x2",
    "slope_1x3",
];

fn reflect_dir(d: Dir) -> Dir {
    match d {
        Dir::Left => Dir::Right,
        Dir::Right => Dir::Left,
        d => d,
    }
}

type Key = (
    String,
    Option<String>,
    String,
    BTreeSet<(i32, i32)>,
    Option<Dir>,
    BTreeSet<(i32, i32)>,
);

/// What makes two placements the same on the ship: part, material, power, cells, facing (rects; a rect's flip
/// only mirrors its icon) and outline (slopes). `reflect` mirrors it across the vertical centre line of a frame
/// `width` cells wide, computed from the geometry alone.
fn key(c: &Content, p: &Placement, reflect: Option<i32>) -> Key {
    let s = c.placed_shape(p).unwrap();
    let rx = |x: i32| reflect.map_or(x, |w| w - 1 - x);
    let px = |x: i32| reflect.map_or(x, |w| w - x);
    let rd = |d: Dir| if reflect.is_some() { reflect_dir(d) } else { d };
    (
        p.part.clone(),
        p.material.clone(),
        format!("{:?}", p.power),
        s.cells().into_iter().map(|(x, y)| (rx(x), y)).collect(),
        (s.footprint.shape == Shape::Rect).then(|| rd(s.facing())),
        s.polygon().into_iter().map(|q| (px(q.x), q.y)).collect(),
    )
}

/// Every part has a mirror partner (itself, for a symmetric part centred on the axis).
fn assert_symmetric(c: &Content, dock: &Drydock, what: &str) {
    let w = dock.frame().0 as i32;
    let keys: BTreeSet<Key> = dock.parts().iter().map(|p| key(c, p, None)).collect();
    for (i, p) in dock.parts().iter().enumerate() {
        assert!(
            keys.contains(&key(c, p, Some(w))),
            "after {what}: part {i} {p:?} has no mirror partner"
        );
    }
}

/// Grid occupancy agrees with the parts' shapes.
fn assert_grid(dock: &Drydock) {
    let (w, h) = dock.frame();
    let mut expect = vec![None; (w * h) as usize];
    for (i, s) in dock.shapes().iter().enumerate() {
        for (x, y) in s.cells() {
            assert!(x >= 0 && y >= 0 && x < w as i32 && y < h as i32);
            assert!(expect[(y as u32 * w + x as u32) as usize].is_none());
            expect[(y as u32 * w + x as u32) as usize] = Some(i);
        }
    }
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            assert_eq!(
                dock.part_at((x, y)),
                expect[(y as u32 * w + x as u32) as usize]
            );
        }
    }
}

#[test]
fn legality_names_the_reason() {
    let c = content();
    let mut dock = empty(&c, (10, 10));
    let block = |x, y| any(&c, "armor_block", x, y, 0, false);
    assert_eq!(dock.check(&block(-1, 0)), Err(Refusal::Outside));
    assert_eq!(dock.check(&block(10, 0)), Err(Refusal::Outside));
    assert_eq!(
        dock.check(&any(&c, "beam_laser", 0, 9, 0, false)),
        Err(Refusal::Outside)
    );
    // Turned, the beam is 2×1 and fits the bottom row.
    assert_eq!(dock.check(&any(&c, "beam_laser", 0, 9, 1, false)), Ok(()));
    assert_eq!(dock.place(block(3, 3)).unwrap().part, Some(0));
    assert_eq!(dock.place(block(5, 5)).unwrap().part, Some(1));
    assert_eq!(
        dock.check(&any(&c, "beam_laser", 5, 4, 0, false)),
        Err(Refusal::Overlaps(1))
    );
    assert_eq!(
        dock.check(&any(&c, "fusion_reactor", 2, 2, 0, false)),
        Err(Refusal::Overlaps(0))
    );
    assert_eq!(
        dock.check(&pl("nope", 0, 0, 0, false, None)),
        Err(Refusal::UnknownPart("nope".into()))
    );
    assert_eq!(
        dock.check(&pl("armor_block", 0, 0, 0, false, None)),
        Err(Refusal::NeedsMaterial)
    );
    assert_eq!(
        dock.check(&pl("armor_block", 0, 0, 0, false, Some("titanium"))),
        Err(Refusal::NotArmor("titanium".into()))
    );
    assert_eq!(
        dock.check(&pl("pulse_laser", 0, 0, 0, false, Some("steel"))),
        Err(Refusal::NotHull)
    );
    assert_eq!(
        dock.check(&any(&c, "armor_block", 0, 0, 4, false)),
        Err(Refusal::BadRotation)
    );
    let power = |tier, weight| {
        Some(PowerOverride {
            tier: Some(tier),
            weight,
        })
    };
    let reactor = Placement {
        power: power(PowerTier::High, None),
        ..any(&c, "fission_reactor", 0, 0, 0, false)
    };
    assert_eq!(dock.check(&reactor), Err(Refusal::DrawsNoPower));
    // An override that sets nothing counts as none (and is dropped when placed).
    let reactor = Placement {
        power: Some(PowerOverride::default()),
        ..reactor
    };
    assert_eq!(dock.check(&reactor), Ok(()));
    let laser = Placement {
        power: power(PowerTier::High, Some(0.0)),
        ..any(&c, "pulse_laser", 0, 0, 0, false)
    };
    assert_eq!(dock.check(&laser), Err(Refusal::BadWeight));
    let laser = Placement {
        power: power(PowerTier::High, Some(2.0)),
        ..laser
    };
    assert_eq!(dock.place(laser).unwrap().part, Some(2));
    assert_eq!(
        dock.set_power(0, power(PowerTier::Low, None)),
        Err(Refusal::DrawsNoPower)
    );
    assert_eq!(dock.set_material(2, "steel"), Err(Refusal::NotHull));
    assert_eq!(dock.set_material(9, "steel"), Err(Refusal::NoSuchPart(9)));
    // Moves and turns must stay legal; a part's own cells do not block it.
    assert_eq!(dock.move_part(0, 2, 2), Err(Refusal::Overlaps(1)));
    assert_eq!(dock.move_part(0, -4, 0), Err(Refusal::Outside));
    assert!(dock.move_part(0, 1, 0).unwrap().changed);
    assert_eq!(dock.parts()[0].x, 4);
    assert_grid(&dock);
}

#[test]
fn mirror_image_matches_the_geometry_for_every_footprint() {
    let c = content();
    for w in [9i32, 10] {
        let dock = empty(&c, (w as u32, 8));
        for part in FOOTPRINTS {
            for rot in 0..4 {
                for flip in [false, true] {
                    for x in 0..w {
                        let p = any(&c, part, x, 2, rot, flip);
                        if dock.check(&p).is_err() {
                            continue;
                        }
                        let m = dock.mirror_of(&p);
                        assert_eq!(dock.check(&m), Ok(()), "{p:?}");
                        assert_eq!(dock.mirror_of(&m), p, "mirroring twice is the identity");
                        let (s, t) = (c.placed_shape(&p).unwrap(), c.placed_shape(&m).unwrap());
                        let reflect = |(x, y): (i32, i32)| (w - 1 - x, y);
                        // Cell by cell: every local cell of the twin is the mirror image of the same local
                        // cell of the part (the twin's muzzle mirrors the part's muzzle).
                        let tc = t.cells();
                        for (k, &cell) in s.cells().iter().enumerate() {
                            assert_eq!(tc[k], reflect(cell), "{p:?}");
                        }
                        let sides = |s: &data::PlacedShape, r: bool| -> BTreeSet<_> {
                            s.connectable_sides()
                                .into_iter()
                                .map(|(c, d)| {
                                    if r {
                                        (reflect(c), reflect_dir(d))
                                    } else {
                                        (c, d)
                                    }
                                })
                                .collect()
                        };
                        assert_eq!(sides(&t, false), sides(&s, true), "{p:?}");
                        let poly = |s: &data::PlacedShape, r: bool| -> BTreeSet<_> {
                            s.polygon()
                                .into_iter()
                                .map(|q| if r { (w - q.x, q.y) } else { (q.x, q.y) })
                                .collect()
                        };
                        assert_eq!(poly(&t, false), poly(&s, true), "{p:?}");
                        if s.footprint.shape == Shape::Rect {
                            assert_eq!(t.facing(), reflect_dir(s.facing()), "{p:?}");
                        }
                        match (s.slope_normal(), t.slope_normal()) {
                            (Some(a), Some(b)) => {
                                assert!((b[0] + a[0]).abs() < 1e-6 && (b[1] - a[1]).abs() < 1e-6)
                            }
                            (None, None) => {}
                            _ => panic!("{p:?}"),
                        }
                        assert_eq!(key(&c, &m, None), key(&c, &p, Some(w)));
                    }
                }
            }
        }
    }
}

#[test]
fn turning_keeps_the_centre_and_commutes_with_the_mirror() {
    let c = content();
    for part in FOOTPRINTS {
        for rot in 0..4 {
            for flip in [false, true] {
                let mut dock = empty(&c, (15, 15));
                let p = any(&c, part, 6, 6, rot, flip);
                dock.place(p.clone()).unwrap();
                let before = dock.shapes()[0];
                dock.rotate(0, true).unwrap();
                let after = dock.shapes()[0];
                assert_eq!(after.rot, (rot + 1) % 4);
                let (cb, ca) = (before.center(), after.center());
                let moved = [(ca[0] - cb[0]).abs(), (ca[1] - cb[1]).abs()];
                let (w, h) = (before.footprint.w, before.footprint.h);
                let expect = if (w + h) % 2 == 0 {
                    [0.0, 0.0]
                } else {
                    [0.5, 0.5]
                };
                assert_eq!(moved, expect, "{p:?}");
                dock.rotate(0, false).unwrap();
                assert_eq!(dock.parts()[0], p, "turning back restores {p:?}");
                for _ in 0..4 {
                    dock.rotate(0, true).unwrap();
                }
                assert_eq!(dock.parts()[0], p, "four turns restore {p:?}");

                // The twin of a turned, flipped or moved part is the mirror image of the result.
                for width in [15u32, 16] {
                    let mut dock = empty(&c, (width, 15));
                    dock.mirror = true;
                    let o = dock.place(any(&c, part, 2, 6, rot, flip)).unwrap();
                    assert_eq!(o.twin, Twin::Applied);
                    let twin_ok = |dock: &Drydock| {
                        assert_eq!(dock.parts()[1], dock.mirror_of(&dock.parts()[0]), "{p:?}");
                    };
                    twin_ok(&dock);
                    for clockwise in [true, true, false, true] {
                        assert_eq!(dock.rotate(0, clockwise).unwrap().twin, Twin::Applied);
                        twin_ok(&dock);
                    }
                    assert_eq!(dock.flip(0).unwrap().twin, Twin::Applied);
                    twin_ok(&dock);
                    assert_eq!(dock.move_part(0, 1, 1).unwrap().twin, Twin::Applied);
                    twin_ok(&dock);
                    assert_grid(&dock);
                }
            }
        }
    }
}

#[test]
fn bevel_paints_outer_corners_as_slopes() {
    let c = content();
    let block = |x, y| any(&c, "armor_block", x, y, 0, false);
    // Every pair of adjacent sides: the slope's straight sides are exactly the two touching sides.
    for (d1, d2) in [
        (Dir::Left, Dir::Down),
        (Dir::Up, Dir::Left),
        (Dir::Right, Dir::Up),
        (Dir::Down, Dir::Right),
    ] {
        let mut dock = empty(&c, (9, 9));
        for d in [d1, d2] {
            let (dx, dy) = d.delta();
            dock.place(block(4 + dx, 4 + dy)).unwrap();
        }
        dock.bevel = true;
        let i = dock.paint((4, 4), "steel").unwrap().part.unwrap();
        assert_eq!(dock.parts()[i].part, "slope_1x1");
        assert_eq!(dock.parts()[i].material.as_deref(), Some("steel"));
        let sides: BTreeSet<Dir> = dock.shapes()[i]
            .connectable_sides()
            .into_iter()
            .map(|(_, d)| d)
            .collect();
        assert_eq!(sides, BTreeSet::from([d1, d2]));
    }
    // Not outer corners: one side, three sides, opposite sides, a side facing a slope face. And bevel off.
    let cases: [&[(i32, i32)]; 3] = [&[(3, 4)], &[(3, 4), (4, 5), (5, 4)], &[(3, 4), (5, 4)]];
    for case in cases {
        let mut dock = empty(&c, (9, 9));
        for &(x, y) in case {
            dock.place(block(x, y)).unwrap();
        }
        dock.bevel = true;
        let i = dock.paint((4, 4), "steel").unwrap().part.unwrap();
        assert_eq!(dock.parts()[i].part, "armor_block", "{case:?}");
    }
    let mut dock = empty(&c, (9, 9));
    dock.place(block(3, 4)).unwrap();
    // A slope below whose face (up-right) points at the cell: occupied, but neither touching nor open.
    dock.place(any(&c, "slope_1x1", 4, 5, 0, false)).unwrap();
    dock.bevel = true;
    let i = dock.paint((4, 4), "steel").unwrap().part.unwrap();
    assert_eq!(dock.parts()[i].part, "armor_block");
    let mut dock = empty(&c, (9, 9));
    dock.place(block(3, 4)).unwrap();
    dock.place(block(4, 5)).unwrap();
    let i = dock.paint((4, 4), "steel").unwrap().part.unwrap();
    assert_eq!(dock.parts()[i].part, "armor_block", "bevel off");
}

/// Bevel works on the finished stroke: the same shape gives the same slopes whatever order its cells were painted
/// in, in one undo step, mirror twins included; cells painted before the stroke are left alone.
#[test]
fn bevel_cuts_the_corners_a_stroke_painted() {
    let c = content();
    // On the finished shape every cell of a 2×2 square is an outer corner; painted one by one, in any order,
    // only the last cells would be corners at the moment they are painted.
    let square = [(2, 2), (3, 2), (2, 3), (3, 3)];
    let shapes: Vec<Vec<Placement>> = [square, [(3, 3), (2, 2), (3, 2), (2, 3)], [(2, 3), (3, 3), (3, 2), (2, 2)]]
        .iter()
        .map(|order| {
            let mut dock = empty(&c, (8, 8));
            dock.bevel = true;
            dock.begin_stroke();
            for &cell in order {
                dock.paint(cell, "steel").unwrap();
            }
            // While the stroke is open the cells are blocks; its end would make four slopes.
            assert!(dock.parts().iter().all(|p| p.part == "armor_block"));
            assert_eq!(dock.preview_stroke_end().len(), 4);
            dock.end_stroke();
            assert_grid(&dock);
            let mut parts = dock.parts().to_vec();
            parts.sort_by_key(|p| (p.y, p.x));
            // One undo step takes it all away.
            assert!(dock.undo());
            assert!(dock.parts().is_empty());
            assert!(!dock.undo());
            parts
        })
        .collect();
    assert!(shapes.iter().all(|s| *s == shapes[0]));
    assert!(shapes[0].iter().all(|p| p.part == "slope_1x1" && p.material.as_deref() == Some("steel")));
    // Each slope's straight sides face its two neighbours in the square (it links to both).
    let corner = |p: &Placement| c.placed_shape(p).unwrap().slope_normal().unwrap();
    let n = corner(&shapes[0][0]);
    assert!(n[0] < 0.0 && n[1] < 0.0, "top-left faces up-left: {n:?}");
    // Strokes without bevel, and cells painted before the stroke, stay blocks.
    let mut dock = empty(&c, (8, 8));
    dock.begin_stroke();
    for cell in square {
        dock.paint(cell, "steel").unwrap();
    }
    dock.end_stroke();
    dock.bevel = true;
    dock.begin_stroke();
    dock.paint((4, 2), "steel").unwrap();
    dock.paint((4, 3), "steel").unwrap();
    dock.end_stroke();
    // The new column's ends are outer corners of the 3×2 block; the old square's cells are not touched.
    let kinds: Vec<(&str, i32, i32)> = dock.parts().iter().map(|p| (p.part.as_str(), p.x, p.y)).collect();
    assert_eq!(&kinds[..4], &[("armor_block", 2, 2), ("armor_block", 3, 2), ("armor_block", 2, 3), ("armor_block", 3, 3)]);
    assert_eq!(&kinds[4..], &[("slope_1x1", 4, 2), ("slope_1x1", 4, 3)]);
    // A click is a stroke of one cell: the corner is cut at once, and the preview shows it.
    let mut dock = empty(&c, (9, 9));
    dock.place(any(&c, "armor_block", 3, 4, 0, false)).unwrap();
    dock.place(any(&c, "armor_block", 4, 5, 0, false)).unwrap();
    dock.bevel = true;
    let preview = dock.preview_paint((4, 4), "steel").unwrap();
    assert_eq!(preview.placement.part, "slope_1x1");
    let i = dock.paint((4, 4), "steel").unwrap().part.unwrap();
    assert_eq!(dock.parts()[i], preview.placement);
    // Mirror: the twins are bevelled the same way, mirror images of each other, and so is a fill.
    let mut dock = empty(&c, (10, 10));
    dock.mirror = true;
    dock.bevel = true;
    dock.begin_stroke();
    for cell in [(1, 1), (2, 1), (1, 2), (2, 2)] {
        dock.paint(cell, "steel").unwrap();
    }
    dock.end_stroke();
    assert_eq!(dock.parts().len(), 8);
    assert!(dock.parts().iter().all(|p| p.part == "slope_1x1"));
    assert_symmetric(&c, &dock, "a bevelled stroke");
    dock.undo();
    let o = dock.fill((1, 5), (3, 7), "steel").unwrap();
    assert_eq!(o.twin, Twin::Applied);
    let slopes: Vec<(i32, i32)> = dock.parts().iter().filter(|p| p.part == "slope_1x1").map(|p| (p.x, p.y)).collect();
    assert_eq!(slopes, [(1, 5), (3, 5), (1, 7), (3, 7), (6, 5), (8, 5), (6, 7), (8, 7)]);
    assert_symmetric(&c, &dock, "a bevelled fill");
    dock.undo();
    assert!(dock.parts().is_empty(), "the fill and its bevel are one step");
}

/// Placing a part replaces every part its footprint (and its mirror image's) overlaps, as one undo step.
#[test]
fn placing_replaces_what_is_under_it() {
    let c = content();
    let mut dock = empty(&c, (10, 10));
    dock.fill((0, 0), (9, 1), "steel").unwrap();
    dock.place(any(&c, "pulse_laser", 4, 4, 0, false)).unwrap();
    let n = dock.parts().len();
    // A 2×2 reactor over four blocks.
    let reactor = any(&c, "fusion_reactor", 1, 0, 0, false);
    let preview = dock.preview_place(&reactor).unwrap();
    let under: Vec<usize> = [(1, 0), (2, 0), (1, 1), (2, 1)].iter().map(|&cell| dock.part_at(cell).unwrap()).collect();
    let mut want = under.clone();
    want.sort_unstable();
    assert_eq!(preview.removed, want);
    let o = dock.place(reactor.clone()).unwrap();
    assert_eq!((o.part, dock.parts().len()), (Some(n - 4), n - 3));
    assert_eq!(dock.parts()[n - 4], reactor);
    assert_grid(&dock);
    // The same part again on itself: nothing to replace, no change, no undo step.
    assert_eq!(dock.preview_place(&reactor).unwrap().removed, []);
    let o = dock.place(reactor.clone()).unwrap();
    assert_eq!((o.changed, o.part), (false, Some(n - 4)));
    dock.undo();
    assert_eq!(dock.parts().len(), n);
    assert_grid(&dock);
    // Over a machine; the check stays strict.
    let beam = any(&c, "beam_laser", 4, 3, 0, false);
    assert!(matches!(dock.check(&beam), Err(Refusal::Overlaps(_))));
    let o = dock.place(beam).unwrap();
    assert!(dock.parts().iter().all(|p| p.part != "pulse_laser"));
    assert_eq!(dock.part_at((4, 4)), o.part);
    dock.undo();
    // Mirror: the twin replaces what is under it too; outside the frame is still refused.
    dock.mirror = true;
    let o = dock.place(any(&c, "fusion_reactor", 1, 0, 0, false)).unwrap();
    assert_eq!(o.twin, Twin::Applied);
    assert_eq!(dock.parts().len(), n - 8 + 2);
    let i = o.part.unwrap();
    assert_eq!(dock.parts()[i + 1], dock.mirror_of(&dock.parts()[i]));
    assert_grid(&dock);
    assert_eq!(dock.place(any(&c, "fusion_reactor", 9, 0, 0, false)), Err(Refusal::Outside));
    // Painting over the logic cores leaves a legal design (it just can't fly, as after erasing them).
    let mut dock = Drydock::open(&c, &runner(&c), (24, 24)).unwrap();
    dock.mirror = true;
    let core = dock.parts().iter().position(|p| p.part == "logic_core").unwrap();
    let cell = (dock.parts()[core].x, dock.parts()[core].y);
    let o = dock.paint(cell, "steel").unwrap();
    assert_eq!(o.twin, Twin::Applied);
    assert!(dock.parts().iter().all(|p| p.part != "logic_core"));
    assert_symmetric(&c, &dock, "cores painted over");
    assert_grid(&dock);
    assert!(Drydock::open(&c, &dock.export(), (24, 24)).is_ok());
}

#[test]
fn paint_fills_empty_cells_and_repaints_hull() {
    let c = content();
    let mut dock = empty(&c, (6, 6));
    let o = dock.paint((1, 1), "steel").unwrap();
    assert!(o.changed);
    assert_eq!(
        dock.parts()[0],
        pl("armor_block", 1, 1, 0, false, Some("steel"))
    );
    let rev = dock.revision();
    assert!(!dock.paint((1, 1), "steel").unwrap().changed);
    assert_eq!(dock.revision(), rev, "no change, no revision");
    assert!(dock.paint((1, 1), "reactive").unwrap().changed);
    assert_eq!(dock.parts()[0].material.as_deref(), Some("reactive"));
    dock.place(any(&c, "slope_1x2", 2, 2, 0, false)).unwrap();
    dock.paint((3, 2), "steel").unwrap();
    assert_eq!(
        dock.parts()[1].material.as_deref(),
        Some("steel"),
        "slopes are hull"
    );
    // Any other part is replaced by the block, as one step.
    dock.place(any(&c, "radiator", 4, 4, 0, false)).unwrap();
    let preview = dock.preview_paint((4, 4), "steel").unwrap();
    assert_eq!(preview.removed, [2]);
    let o = dock.paint((4, 4), "steel").unwrap();
    assert_eq!(o.part, Some(2));
    assert_eq!(dock.parts()[2], pl("armor_block", 4, 4, 0, false, Some("steel")));
    assert_eq!(dock.parts().len(), 3);
    assert_grid(&dock);
    dock.undo();
    assert_eq!(dock.parts()[2].part, "radiator");
    assert_grid(&dock);
    assert_eq!(dock.paint((6, 0), "steel"), Err(Refusal::Outside));
    assert_eq!(
        dock.paint((0, 0), "titanium"),
        Err(Refusal::NotArmor("titanium".into()))
    );
}

#[test]
fn slopes_orient_to_the_hull() {
    let c = content();
    let lean = |dock: &Drydock, i: usize| dock.shapes()[i].slope_normal().unwrap();
    let links = |dock: &Drydock, i: usize| {
        let grid = data::ShipGrid::new(dock.shapes().to_vec()).unwrap();
        dock.shapes()[i]
            .connectable_sides()
            .into_iter()
            .filter(|&(c, d)| {
                let (dx, dy) = d.delta();
                grid.occupant((c.0 + dx, c.1 + dy)).is_some() && grid.linked(c, d)
            })
            .count()
    };
    // The right end of a row on the right half: the straight left side links to the row, the face looks up
    // and out (away from the centre line), not down.
    let mut dock = empty(&c, (12, 12));
    dock.fill((2, 5), (6, 5), "steel").unwrap();
    let i = dock
        .place_slope("slope_1x1", (7, 5), "steel", None)
        .unwrap()
        .part
        .unwrap();
    assert_eq!(links(&dock, i), 1);
    let n = lean(&dock, i);
    assert!(n[0] > 0.0 && n[1] < 0.0, "{n:?}");
    // The left end on the left half faces up and left.
    let i = dock
        .place_slope("slope_1x1", (1, 5), "steel", None)
        .unwrap()
        .part
        .unwrap();
    let n = lean(&dock, i);
    assert!(n[0] < 0.0 && n[1] < 0.0, "{n:?}");
    // In a notch with hull left and below, both straight sides link.
    let mut dock = empty(&c, (12, 12));
    dock.place(any(&c, "armor_block", 3, 4, 0, false)).unwrap();
    dock.place(any(&c, "armor_block", 4, 5, 0, false)).unwrap();
    let i = dock
        .place_slope("slope_1x1", (4, 4), "steel", None)
        .unwrap()
        .part
        .unwrap();
    assert_eq!(links(&dock, i), 2);
    // A long slope on a wing edge lies along the row (all three straight bottom sides link) and faces outward,
    // like the Runner's left nose slopes (flipped on the left half).
    let mut dock = empty(&c, (12, 12));
    dock.fill((0, 6), (11, 6), "composite").unwrap();
    let i = dock
        .place_slope("slope_1x3", (2, 5), "composite", None)
        .unwrap()
        .part
        .unwrap();
    assert_eq!(
        dock.parts()[i],
        pl("slope_1x3", 1, 5, 0, true, Some("composite"))
    );
    assert_eq!(links(&dock, i), 3);
    // The preview is what the tool places.
    let mut dock = empty(&c, (12, 12));
    dock.fill((0, 6), (11, 6), "composite").unwrap();
    let preview = dock
        .preview_slope("slope_1x3", (9, 5), "composite", None)
        .unwrap()
        .placement;
    let i = dock
        .place_slope("slope_1x3", (9, 5), "composite", None)
        .unwrap()
        .part
        .unwrap();
    assert_eq!(dock.parts()[i], preview);
    assert_eq!(preview, pl("slope_1x3", 8, 5, 0, false, Some("composite")));
    assert_eq!(
        dock.place_slope("armor_block", (0, 0), "steel", None),
        Err(Refusal::NotSlope)
    );
    // On another part the slope replaces it, oriented to the hull without it: the row's left end, open above.
    let n = dock.parts().len();
    let o = dock.place_slope("slope_1x1", (0, 6), "steel", None).unwrap();
    assert_eq!((o.part, dock.parts().len()), (Some(n - 1), n));
    assert_eq!(dock.part_at((0, 6)), o.part);
    let n = lean(&dock, o.part.unwrap());
    assert!(n[0] < 0.0 && n[1] < 0.0, "{n:?}");
    // A 1×3 in a one-cell hole replaces as few parts as it can (two), and only those.
    let mut dock = empty(&c, (3, 3));
    dock.fill((0, 0), (2, 2), "steel").unwrap();
    dock.erase((1, 1)).unwrap();
    let preview = dock.preview_slope("slope_1x3", (1, 1), "steel", None).unwrap();
    assert_eq!(preview.removed.len(), 2);
    let i = dock.place_slope("slope_1x3", (1, 1), "steel", None).unwrap().part.unwrap();
    assert_eq!((dock.parts()[i].clone(), dock.parts().len()), (preview.placement, 7));
    assert_grid(&dock);
    // Larger than the frame: no room.
    let mut dock = empty(&c, (2, 2));
    assert_eq!(
        dock.place_slope("slope_1x3", (1, 1), "steel", None),
        Err(Refusal::NoRoom)
    );
}

#[test]
fn a_turned_slope_brush_keeps_its_orientation() {
    let c = content();
    let mut dock = empty(&c, (12, 12));
    dock.fill((2, 5), (6, 5), "steel").unwrap();
    // The turning order: 4 orientations of a 1×1, 8 of the longer ones; next and back are inverse.
    assert_eq!(dock.slope_orientations("slope_1x1").unwrap().len(), 4);
    let all = dock.slope_orientations("slope_1x3").unwrap();
    assert_eq!(all.len(), 8);
    for (k, &o) in all.iter().enumerate() {
        assert_eq!(dock.next_orientation("slope_1x3", o, true), Ok(all[(k + 1) % 8]));
        assert_eq!(dock.next_orientation("slope_1x3", all[(k + 1) % 8], false), Ok(o));
    }
    // Matched by shape: a 1×1 slope's flipped orientations are among its unflipped ones.
    let one = dock.slope_orientations("slope_1x1").unwrap();
    assert!(one.contains(&dock.next_orientation("slope_1x1", (0, true), true).unwrap()));
    assert_eq!(dock.slope_orientations("armor_block"), Err(Refusal::NotSlope));
    // Every orientation goes down as asked, centred on the cell, on the hull end where auto would pick another.
    let auto = dock.preview_slope("slope_1x1", (7, 5), "steel", None).unwrap().placement;
    for (rot, flip) in dock.slope_orientations("slope_1x1").unwrap() {
        let p = dock.preview_slope("slope_1x1", (7, 5), "steel", Some((rot, flip))).unwrap().placement;
        assert_eq!((p.x, p.y, p.rot, p.flip), (7, 5, rot, flip));
    }
    let other = dock.next_orientation("slope_1x1", (auto.rot, auto.flip), true).unwrap();
    let i = dock.place_slope("slope_1x1", (7, 5), "steel", Some(other)).unwrap().part.unwrap();
    assert_eq!((dock.parts()[i].rot, dock.parts()[i].flip), other);
    // A long one is pushed inside the frame like an auto-oriented one; one larger than the frame has no room.
    let p = dock.preview_slope("slope_1x3", (0, 0), "steel", Some((1, false))).unwrap().placement;
    assert_eq!((p.x, p.y), (0, 0));
    assert_eq!(dock.preview_slope("slope_1x3", (0, 0), "steel", Some((4, false))), Err(Refusal::BadRotation));
    let small = empty(&c, (2, 2));
    assert_eq!(small.preview_slope("slope_1x3", (0, 0), "steel", Some((0, false))), Err(Refusal::NoRoom));
    // Over a slope of the same part a click still turns it (or repaints it), whatever the brush says.
    let before = dock.parts()[i].clone();
    let o = dock.place_slope("slope_1x1", (7, 5), "steel", Some(other)).unwrap();
    assert_eq!(o.part, Some(i));
    assert_ne!((dock.parts()[i].rot, dock.parts()[i].flip), (before.rot, before.flip));
    // Over a block it replaces it, in the brush orientation; with mirror its twin is the exact mirror image.
    dock.mirror = true;
    let o = dock.place_slope("slope_1x1", (3, 5), "steel", Some((2, false))).unwrap();
    assert_eq!(o.twin, Twin::Applied);
    assert_eq!(dock.parts()[o.part.unwrap()].rot, 2);
    assert_eq!(dock.parts()[o.part.unwrap() + 1], dock.mirror_of(&dock.parts()[o.part.unwrap()]));
    assert_grid(&dock);
}

#[test]
fn clicking_a_slope_again_turns_or_repaints_it() {
    let c = content();
    for (part, distinct) in [("slope_1x1", 4), ("slope_1x2", 8), ("slope_1x3", 8)] {
        let mut dock = empty(&c, (9, 9));
        let i = dock
            .place_slope(part, (4, 4), "steel", None)
            .unwrap()
            .part
            .unwrap();
        let first = dock.parts()[i].clone();
        let mut seen = vec![key(&c, &first, None)];
        for _ in 1..distinct {
            let o = dock.place_slope(part, (4, 4), "steel", None).unwrap();
            assert!(o.changed);
            assert_eq!(o.part, Some(i));
            let k = key(&c, &dock.parts()[i], None);
            assert!(
                !seen.contains(&k),
                "{part}: every turn gives a new orientation"
            );
            assert!(
                dock.part_at((4, 4)) == Some(i),
                "{part}: it turns about the clicked cell"
            );
            seen.push(k);
        }
        dock.place_slope(part, (4, 4), "steel", None).unwrap();
        assert_eq!(
            key(&c, &dock.parts()[i], None),
            seen[0],
            "{part}: the cycle closes"
        );
        assert_eq!(dock.parts().len(), 1);
        // Another material repaints instead of turning.
        let before = dock.parts()[i].clone();
        dock.place_slope(part, (4, 4), "reactive", None).unwrap();
        assert_eq!(
            dock.parts()[i],
            Placement {
                material: Some("reactive".into()),
                ..before
            }
        );
    }
    // Turning skips orientations that do not fit: a 1×2 slope in a 2×1 slot can only lie flat.
    let mut dock = empty(&c, (4, 3));
    dock.fill((0, 0), (3, 2), "steel").unwrap();
    dock.erase((1, 1)).unwrap();
    dock.erase((2, 1)).unwrap();
    let i = dock
        .place_slope("slope_1x2", (1, 1), "steel", None)
        .unwrap()
        .part
        .unwrap();
    for _ in 0..8 {
        dock.place_slope("slope_1x2", (1, 1), "steel", None).unwrap();
        assert_eq!(dock.shapes()[i].dims(), (2, 1));
    }
}

#[test]
fn fill_is_one_step_and_mirrors() {
    let c = content();
    let mut dock = empty(&c, (8, 8));
    dock.place(any(&c, "pulse_laser", 2, 2, 0, false)).unwrap();
    let o = dock.fill((3, 3), (1, 1), "steel").unwrap();
    assert!(o.changed && o.part.is_none());
    assert_eq!(dock.parts().len(), 9);
    assert_eq!(dock.parts()[0].part, "pulse_laser");
    assert!(dock.parts()[1..].iter().all(|p| p.part == "armor_block"));
    assert!(dock.undo());
    assert_eq!(dock.parts().len(), 1);
    assert_eq!(
        dock.fill((1, 1), (3, 3), "nope"),
        Err(Refusal::NotArmor("nope".into()))
    );
    // Clipped to the frame; entirely outside is refused.
    dock.fill((-5, 6), (0, 20), "steel").unwrap();
    assert_eq!(dock.parts().len(), 3);
    assert_eq!(
        dock.fill((-5, -5), (-1, -1), "steel"),
        Err(Refusal::Outside)
    );
    // Mirror mode fills the mirrored rectangle too; a rectangle centred on the axis is its own image.
    let mut dock = empty(&c, (8, 8));
    dock.mirror = true;
    assert_eq!(
        dock.fill((0, 0), (1, 1), "steel").unwrap().twin,
        Twin::Applied
    );
    assert_eq!(dock.parts().len(), 8);
    assert_symmetric(&c, &dock, "fill");
    assert_eq!(
        dock.fill((3, 3), (4, 4), "steel").unwrap().twin,
        Twin::Itself
    );
    assert_eq!(dock.parts().len(), 12);
    assert!(!dock.fill((0, 0), (1, 1), "steel").unwrap().changed);
    assert_grid(&dock);
}

#[test]
fn a_stroke_is_open_between_begin_and_end() {
    let c = content();
    let mut dock = empty(&c, (8, 8));
    assert!(!dock.in_stroke());
    dock.begin_stroke();
    assert!(dock.in_stroke());
    dock.paint((1, 1), "steel").unwrap();
    dock.end_stroke();
    assert!(!dock.in_stroke());
    // Undo closes an open stroke too.
    dock.begin_stroke();
    dock.undo();
    assert!(!dock.in_stroke());
}

#[test]
fn strokes_are_one_undo_step() {
    let c = content();
    let mut dock = empty(&c, (8, 8));
    let rev = dock.revision();
    dock.begin_stroke();
    for x in 0..5 {
        assert!(dock.paint((x, 3), "steel").unwrap().changed);
    }
    assert!(!dock.paint((4, 3), "steel").unwrap().changed);
    assert!(dock.can_undo());
    dock.end_stroke();
    assert_eq!(dock.revision(), rev + 5, "every change bumps the revision");
    assert_eq!(dock.parts().len(), 5);
    assert!(dock.undo());
    assert!(dock.parts().is_empty());
    assert!(dock.redo());
    assert_eq!(dock.parts().len(), 5);
    // An erase drag; undo closes an open stroke first.
    dock.begin_stroke();
    for x in 1..4 {
        dock.erase((x, 3)).unwrap();
    }
    assert_eq!(dock.erase((2, 3)), Err(Refusal::Empty));
    assert!(dock.undo());
    assert_eq!(dock.parts().len(), 5);
    assert_grid(&dock);
    // Single edits outside strokes are one step each.
    dock.paint((0, 0), "steel").unwrap();
    dock.paint((1, 0), "steel").unwrap();
    dock.undo();
    assert_eq!(dock.parts().len(), 6);
    // An empty stroke leaves no step.
    dock.begin_stroke();
    dock.end_stroke();
    dock.undo();
    assert_eq!(dock.parts().len(), 5);
}

#[test]
fn undo_redo_keeps_enough_steps_and_restores_order() {
    let c = content();
    const { assert!(UNDO_STEPS >= 200) };
    let mut dock = empty(&c, (30, 30));
    for k in 0..300 {
        dock.paint((k % 30, k / 30), "steel").unwrap();
    }
    let rev = dock.revision();
    assert_eq!(rev, 300);
    let mut undone = 0;
    while dock.undo() {
        undone += 1;
    }
    assert_eq!(undone, UNDO_STEPS);
    assert_eq!(dock.parts().len(), 300 - UNDO_STEPS);
    assert_eq!(dock.revision(), rev + UNDO_STEPS as u64);
    assert!(dock.can_redo());
    for _ in 0..10 {
        assert!(dock.redo());
    }
    dock.paint((29, 29), "steel").unwrap();
    assert!(!dock.can_redo(), "a new edit clears redo");
    assert!(!dock.redo());

    // Undoing an erase puts the part back at its index; part_at follows every shift.
    let mut dock = empty(&c, (8, 8));
    for x in 0..5 {
        dock.place(any(&c, "armor_block", x, 0, 0, false)).unwrap();
    }
    let before = dock.parts().to_vec();
    assert_eq!(dock.part_at((3, 0)), Some(3));
    dock.erase((1, 0)).unwrap();
    assert_eq!(
        dock.part_at((3, 0)),
        Some(2),
        "erasing shifts later indices"
    );
    assert_eq!(dock.part_at((1, 0)), None);
    assert_grid(&dock);
    dock.undo();
    assert_eq!(dock.parts(), &before[..]);
    assert_eq!(dock.part_at((3, 0)), Some(3));
    assert_grid(&dock);
    dock.redo();
    assert_eq!(dock.parts().len(), 4);
    assert_grid(&dock);
}

#[test]
fn opening_centres_and_export_trims() {
    let c = content();
    let ship = c.ships.get("sample_early").unwrap();
    let grid = c.ship_grid(ship).unwrap();
    let b = grid.bounds().unwrap();
    let mut dock = Drydock::open(&c, ship, (20, 20)).unwrap();
    let d = dock.bounds().unwrap();
    assert_eq!((d.width(), d.height()), (b.width(), b.height()));
    assert_eq!(d.min_x, (20 - b.width()) / 2);
    assert_eq!(d.min_y, (20 - b.height()) / 2);
    let out = dock.export();
    assert_eq!(out.size, (b.width() as u32, b.height() as u32));
    for (p, q) in ship.parts.iter().zip(&out.parts) {
        assert_eq!((q.x, q.y), (p.x - b.min_x, p.y - b.min_y));
        assert_eq!(
            Placement {
                x: 0,
                y: 0,
                ..q.clone()
            },
            Placement {
                x: 0,
                y: 0,
                ..p.clone()
            }
        );
    }
    assert_eq!(out.id, ship.id);
    // A new frame re-centres; too small is refused and keeps the frame; frame changes undo.
    let parts = dock.parts().to_vec();
    assert!(dock.set_frame((31, 25)).unwrap().changed);
    let d = dock.bounds().unwrap();
    assert_eq!(
        (d.min_x, d.min_y),
        ((31 - b.width()) / 2, (25 - b.height()) / 2)
    );
    assert_grid(&dock);
    assert_eq!(dock.set_frame((5, 5)), Err(Refusal::Outside));
    assert_eq!(dock.set_frame((0, 5)), Err(Refusal::BadFrame));
    assert_eq!(dock.frame(), (31, 25));
    assert_eq!(dock.export(), out);
    assert!(dock.undo());
    assert_eq!(dock.frame(), (20, 20));
    assert_eq!(dock.parts(), &parts[..]);
    assert_grid(&dock);
    assert_eq!(
        Drydock::open(&c, ship, (b.width() as u32 - 1, 30)).unwrap_err(),
        OpenError::DoesNotFit {
            design: (b.width() as u32, b.height() as u32)
        }
    );
    assert_eq!(
        Drydock::open(&c, ship, (20, 0)).unwrap_err(),
        OpenError::BadFrame
    );
    // An empty design.
    let mut dock = empty(&c, (4, 4));
    assert_eq!(dock.export().size, (0, 0));
    assert!(dock.export().parts.is_empty());
    dock.paint((3, 3), "steel").unwrap();
    assert_eq!(
        dock.export().parts[0],
        pl("armor_block", 0, 0, 0, false, Some("steel"))
    );
    assert_eq!(dock.export().size, (1, 1));
}

/// GDR-0092: a design opened placed keeps every cell, so a run's locks (frame cells) never drift; one that grows on
/// one side and is opened again stays where it was, where `open` would centre it again.
#[test]
fn opening_placed_keeps_positions() {
    let c = content();
    let ship = c.ships.get("sample_early").unwrap();
    let dock = Drydock::open(&c, ship, (20, 20)).unwrap();
    let mut placed = dock.export();
    placed.parts = dock.parts().to_vec();
    placed.size = dock.frame();
    let mut again = Drydock::open_placed(&c, &placed, (20, 20)).unwrap();
    assert_eq!(again.parts(), dock.parts());
    assert_eq!(again.export(), dock.export());
    assert_grid(&again);
    // Grow it by two columns on the right, open both ways: placed keeps the old cells, centred moves them.
    let b = again.bounds().unwrap();
    again.paint((b.max_x, b.min_y), "steel").unwrap();
    again.paint((b.max_x + 1, b.min_y), "steel").unwrap();
    placed.parts = again.parts().to_vec();
    let kept = Drydock::open_placed(&c, &placed, (20, 20)).unwrap();
    assert_eq!(kept.parts(), again.parts());
    let centred = Drydock::open(&c, &placed, (20, 20)).unwrap();
    assert_ne!(centred.parts()[0], again.parts()[0]);
    // A part outside the frame: the box from the frame's corner it would need.
    let mut out = placed.clone();
    out.parts[0].x = -2;
    assert!(matches!(
        Drydock::open_placed(&c, &out, (20, 20)).unwrap_err(),
        OpenError::DoesNotFit { design: (w, 20) } if w >= 22
    ));
    out.parts[0].x = 20;
    assert!(matches!(
        Drydock::open_placed(&c, &out, (20, 20)).unwrap_err(),
        OpenError::DoesNotFit { design: (w, 20) } if w > 20
    ));
}

#[test]
fn open_refuses_broken_blueprints() {
    let c = content();
    let mut ship = runner(&c);
    ship.parts[3].part = "nope".into();
    assert_eq!(
        Drydock::open(&c, &ship, (24, 24)).unwrap_err(),
        OpenError::UnknownPart {
            index: 3,
            part: "nope".into()
        }
    );
    let mut ship = runner(&c);
    ship.parts[2].rot = 7;
    assert_eq!(
        Drydock::open(&c, &ship, (24, 24)).unwrap_err(),
        OpenError::BadRotation { index: 2 }
    );
    let mut ship = runner(&c);
    ship.parts[2].x -= 1;
    assert_eq!(
        Drydock::open(&c, &ship, (24, 24)).unwrap_err(),
        OpenError::Overlap { index: 2, other: 1 }
    );
}

#[test]
fn halvex_runner_round_trips() {
    let c = content();
    let ship = runner(&c);
    let dock = Drydock::open(&c, &ship, (24, 24)).unwrap();
    assert_eq!(dock.parts().len(), ship.parts.len());
    assert_eq!(dock.parts()[0].x, ship.parts[0].x + 6);
    assert_eq!(dock.parts()[0].y, ship.parts[0].y + 8);
    assert_grid(&dock);
    assert_symmetric(&c, &dock, "opening");
    // The design comes from no pack: its problems are not located in the template's file.
    assert_eq!(dock.export(), Blueprint { source: Default::default(), ..ship });
}

#[test]
fn mirrored_edits_keep_the_runner_symmetric() {
    let c = content();
    let ship = runner(&c);
    // The Runner sits at (6, 8) in a 24×24 frame; its axis is the frame's (x = 12).
    let mut dock = Drydock::open(&c, &ship, (24, 24)).unwrap();
    dock.mirror = true;
    let mut steps = 0;
    let mut check = |dock: &Drydock, what: &str, twin: Twin| {
        assert_symmetric(&c, dock, what);
        assert_grid(dock);
        assert!(
            matches!(twin, Twin::Applied | Twin::Itself),
            "{what}: twin {twin:?}"
        );
        steps += 1;
    };
    let o = dock.paint((5, 11), "composite").unwrap();
    assert_eq!(dock.part_at((18, 11)), Some(dock.parts().len() - 1));
    check(&dock, "paint", o.twin);
    dock.bevel = true;
    let o = dock.paint((5, 10), "composite").unwrap();
    assert_eq!(
        dock.parts()[o.part.unwrap()].part,
        "slope_1x1",
        "a bevel corner"
    );
    check(&dock, "bevel paint", o.twin);
    dock.bevel = false;
    let o = dock.erase((7, 10)).unwrap();
    check(&dock, "erase", o.twin);
    let o = dock.place(any(&c, "pulse_laser", 7, 10, 0, false)).unwrap();
    assert!(
        dock.parts()[o.part.unwrap() + 1].flip,
        "the twin is flipped"
    );
    check(&dock, "place", o.twin);
    let o = dock
        .move_part(dock.part_at((5, 11)).unwrap(), 0, 1)
        .unwrap();
    assert!(dock.part_at((18, 12)).is_some());
    check(&dock, "move", o.twin);
    let o = dock.rotate(dock.part_at((8, 13)).unwrap(), true).unwrap();
    check(&dock, "rotate", o.twin);
    let o = dock.flip(dock.part_at((6, 12)).unwrap()).unwrap();
    check(&dock, "flip", o.twin);
    let o = dock
        .set_material(dock.part_at((7, 12)).unwrap(), "steel")
        .unwrap();
    check(&dock, "material", o.twin);
    let power = Some(PowerOverride {
        tier: Some(PowerTier::High),
        weight: Some(2.0),
    });
    let o = dock
        .set_power(dock.part_at((11, 8)).unwrap(), power)
        .unwrap();
    assert_eq!(dock.parts()[dock.part_at((12, 8)).unwrap()].power, power);
    check(&dock, "power", o.twin);
    let o = dock.fill((0, 0), (3, 2), "reactive").unwrap();
    check(&dock, "fill", o.twin);
    let o = dock.place_slope("slope_1x2", (8, 7), "composite", None).unwrap();
    check(&dock, "slope", o.twin);
    let o = dock.place_slope("slope_1x2", (8, 7), "composite", None).unwrap();
    check(&dock, "slope turn", o.twin);
    let o = dock.place_slope("slope_1x2", (8, 7), "steel", None).unwrap();
    check(&dock, "slope repaint", o.twin);
    dock.begin_stroke();
    for x in 0..3 {
        dock.erase((x, 0)).unwrap();
    }
    dock.end_stroke();
    check(&dock, "erase stroke", Twin::Applied);
    // Parts on the axis are their own mirror image: edited once.
    let o = dock.rotate(dock.part_at((11, 13)).unwrap(), true).unwrap();
    assert_eq!(o.twin, Twin::Itself);
    assert!(o.changed);
    dock.undo();
    let o = dock
        .set_power(dock.part_at((11, 10)).unwrap(), None)
        .unwrap();
    assert_eq!(o.twin, Twin::Itself);
    let o = dock.paint((12, 18), "steel").unwrap();
    check(&dock, "paint below the axis", o.twin);
    // Undo everything, then redo everything: symmetric on the way, the original at the start.
    let mut undone = 0;
    while dock.undo() {
        assert_symmetric(&c, &dock, "undo");
        undone += 1;
    }
    assert_eq!(undone, steps);
    assert_eq!(dock.export(), Blueprint { source: Default::default(), ..ship.clone() });
    while dock.redo() {
        assert_symmetric(&c, &dock, "redo");
    }
    assert_grid(&dock);
}

#[test]
fn twins_are_skipped_when_they_cannot_follow() {
    let c = content();
    let mut dock = empty(&c, (10, 10));
    dock.place(any(&c, "armor_block", 7, 0, 0, false)).unwrap();
    dock.mirror = true;
    // The twin's cell is taken: the twin replaces what is there.
    let o = dock.place(any(&c, "pulse_laser", 2, 0, 0, false)).unwrap();
    assert_eq!(o.twin, Twin::Applied);
    assert_eq!(dock.parts().len(), 2);
    assert!(dock.parts().iter().all(|p| p.part == "pulse_laser"));
    // A block has no twin of its kind.
    dock.mirror = false;
    dock.place(any(&c, "armor_block", 7, 3, 0, false)).unwrap();
    dock.mirror = true;
    assert_eq!(
        dock.rotate(2, true).unwrap().twin,
        Twin::Skipped(Refusal::NoTwin)
    );
    // Erasing with an empty mirror cell.
    dock.mirror = false;
    dock.place(any(&c, "armor_block", 5, 9, 0, false)).unwrap();
    dock.mirror = true;
    assert_eq!(
        dock.erase((5, 9)).unwrap().twin,
        Twin::Skipped(Refusal::Empty)
    );
    // A twin whose move is blocked stays put.
    let o = dock.place(any(&c, "armor_block", 2, 5, 0, false)).unwrap();
    assert_eq!(o.twin, Twin::Applied);
    dock.mirror = false;
    dock.place(any(&c, "armor_block", 6, 6, 0, false)).unwrap();
    dock.mirror = true;
    let o = dock.move_part(dock.part_at((2, 5)).unwrap(), 1, 1).unwrap();
    assert_eq!(
        o.twin,
        Twin::Skipped(Refusal::Overlaps(dock.part_at((6, 6)).unwrap()))
    );
    assert!(dock.part_at((3, 6)).is_some() && dock.part_at((7, 5)).is_some());
    assert!(dock.part_at((2, 5)).is_none());
    // A slope on the axis of an odd frame overlaps its own mirror image.
    let mut dock = empty(&c, (9, 9));
    dock.mirror = true;
    let o = dock.place(any(&c, "slope_1x1", 4, 4, 0, false)).unwrap();
    assert_eq!(o.twin, Twin::Skipped(Refusal::Overlaps(0)));
    // A symmetric part centred on the axis is placed once; painting the axis column paints once.
    let o = dock.place(any(&c, "pulse_laser", 4, 0, 0, false)).unwrap();
    assert_eq!(o.twin, Twin::Itself);
    assert_eq!(dock.paint((4, 8), "steel").unwrap().twin, Twin::Itself);
    let mut dock = empty(&c, (10, 10));
    dock.mirror = true;
    let o = dock
        .place(any(&c, "fusion_reactor", 4, 4, 0, false))
        .unwrap();
    assert_eq!(o.twin, Twin::Itself);
    assert_eq!(dock.parts().len(), 1);
}

#[test]
fn removing_a_part_takes_its_twin_and_nothing_else() {
    let c = content();
    let mut dock = empty(&c, (10, 10));
    // A reactor left of the axis; an armour block covers its mirror cells (no twin of its kind).
    dock.place(any(&c, "fusion_reactor", 1, 1, 0, false))
        .unwrap();
    dock.place(any(&c, "armor_block", 8, 1, 0, false)).unwrap();
    dock.mirror = true;
    let o = dock.remove_part(0).unwrap();
    assert_eq!((o.changed, o.twin), (true, Twin::Skipped(Refusal::NoTwin)));
    assert_eq!(dock.parts().len(), 1);
    assert_eq!(dock.parts()[0].part, "armor_block");
    // (Erase on the reactor's cell would have taken the block too.)
    dock.undo();
    assert_eq!(dock.parts().len(), 2);
    assert_eq!(dock.erase((1, 1)).unwrap().twin, Twin::Applied);
    assert!(dock.parts().is_empty());
    // A mirrored pair goes together, as one undo step, whichever index is larger; a part on the axis goes once.
    let mut dock = empty(&c, (10, 10));
    dock.mirror = true;
    dock.place(any(&c, "pulse_laser", 1, 0, 0, false)).unwrap();
    dock.place(any(&c, "fusion_reactor", 4, 5, 0, false))
        .unwrap();
    assert_eq!(dock.parts().len(), 3);
    let twin = dock.part_at((8, 0)).unwrap();
    assert_eq!(dock.remove_part(twin).unwrap().twin, Twin::Applied);
    assert_eq!(dock.parts().len(), 1);
    assert_eq!(dock.remove_part(0).unwrap().twin, Twin::Itself);
    assert!(dock.parts().is_empty());
    dock.undo();
    dock.undo();
    assert_eq!(dock.parts().len(), 3);
    assert_eq!(dock.remove_part(3), Err(Refusal::NoSuchPart(3)));
    assert_grid(&dock);
}

/// The preview of an edit (the ghost) is what the edit does: the same refusal, the same twin outcome, the edited
/// part as previewed, the twin where the preview put it, and the replaced parts gone.
fn agree(
    before: &Drydock,
    preview: Result<Preview, Refusal>,
    edit: impl FnOnce(&mut Drydock) -> Result<Outcome, Refusal>,
) {
    let mut after = before.clone();
    match (preview, edit(&mut after)) {
        (Ok(p), Ok(o)) => {
            assert_eq!(p.twin, o.twin, "{p:?}");
            assert_eq!(after.parts()[o.part.unwrap()], p.placement);
            for &r in &p.removed {
                let gone = &before.parts()[r];
                assert!(!after.parts().contains(gone), "{p:?}: {gone:?} stays");
            }
            match &p.mirrored {
                Some(m) => assert!(
                    p.twin == Twin::Applied && after.parts().contains(m),
                    "{p:?}"
                ),
                None => assert_ne!(p.twin, Twin::Applied, "{p:?}"),
            }
        }
        (Err(a), Err(b)) => assert_eq!(a, b),
        (p, o) => panic!("preview {p:?}, edit {o:?}"),
    }
}

#[test]
fn previews_match_the_edits() {
    let c = content();
    for (mirror, bevel) in [(true, true), (true, false), (false, false)] {
        let mut dock = Drydock::open(&c, &runner(&c), (24, 24)).unwrap();
        dock.mirror = mirror;
        dock.bevel = bevel;
        let (w, h) = dock.frame();
        for y in -1..=h as i32 {
            for x in -1..=w as i32 {
                for m in ["steel", "composite"] {
                    agree(&dock, dock.preview_paint((x, y), m), |d| d.paint((x, y), m));
                }
                for (slope, turn) in [("slope_1x1", None), ("slope_1x2", None), ("slope_1x1", Some((1, false))), ("slope_1x2", Some((2, true)))] {
                    agree(&dock, dock.preview_slope(slope, (x, y), "steel", turn), |d| {
                        d.place_slope(slope, (x, y), "steel", turn)
                    });
                }
                for (part, rot) in [("fusion_reactor", 0), ("beam_laser", 1), ("slope_1x3", 3)] {
                    let p = any(&c, part, x, y, rot, false);
                    agree(&dock, dock.preview_place(&p), |d| d.place(p.clone()));
                }
            }
        }
        for i in 0..dock.parts().len() {
            for (dx, dy) in [(-2, 0), (1, 0), (0, -1), (1, 1), (0, 3), (-5, 2)] {
                agree(&dock, dock.preview_move(i, dx, dy), |d| {
                    d.move_part(i, dx, dy)
                });
            }
        }
    }
}

/// Times on a 64×64 frame full of blocks. Run with
/// `cargo test -p data --release --test edit -- --ignored --nocapture`.
#[test]
#[ignore]
fn edit_timings_on_a_full_64x64_frame() {
    use std::time::{Duration, Instant};
    let c = content();
    let stats = |name: &str, mut v: Vec<Duration>| {
        v.sort();
        let (med, p95, max) = (v[v.len() / 2], v[v.len() * 95 / 100], v[v.len() - 1]);
        println!("{name:<44} median {med:>10.2?}  p95 {p95:>10.2?}  max {max:>10.2?}");
    };
    let full = || {
        let mut dock = empty(&c, (64, 64));
        dock.fill((0, 0), (63, 63), "steel").unwrap();
        dock
    };
    let runs = 400;
    let mut dock = full();
    let t = |dock: &mut Drydock, f: &mut dyn FnMut(&mut Drydock)| {
        let s = Instant::now();
        f(dock);
        s.elapsed()
    };
    let mut v = Vec::new();
    for k in 0..runs {
        // The row starts as steel; every pass over it switches the material.
        let m = if (k / 64) % 2 == 0 {
            "composite"
        } else {
            "steel"
        };
        v.push(t(&mut dock, &mut |d| {
            assert!(d.paint((k % 64, 7), m).unwrap().changed)
        }));
    }
    stats("paint (repaint a block)", v);
    dock.mirror = true;
    let mut v = Vec::new();
    for k in 0..runs {
        let m = if (k / 32) % 2 == 0 {
            "reactive"
        } else {
            "steel"
        };
        v.push(t(&mut dock, &mut |d| {
            assert!(d.paint((k % 32, 9), m).unwrap().changed)
        }));
    }
    stats("paint, mirror", v);
    dock.mirror = false;
    let mut v = Vec::new();
    for _ in 0..runs {
        v.push(t(&mut dock, &mut |d| {
            assert!(d.erase((0, 0)).unwrap().changed)
        }));
        dock.undo();
    }
    stats("erase part 0 (all 4095 indices shift)", v);
    let mut v = Vec::new();
    for _ in 0..runs {
        dock.erase((0, 0)).unwrap();
        v.push(t(&mut dock, &mut |d| assert!(d.undo())));
    }
    stats("undo erase of part 0 (re-insert at 0)", v);
    dock.mirror = true;
    let mut v = Vec::new();
    for _ in 0..runs {
        v.push(t(&mut dock, &mut |d| {
            assert_eq!(d.erase((10, 10)).unwrap().twin, Twin::Applied)
        }));
        dock.undo();
    }
    stats("erase, mirror", v);
    dock.erase((10, 10)).unwrap();
    let mut v = Vec::new();
    for _ in 0..runs {
        v.push(t(&mut dock, &mut |d| {
            let i = d.part_at((11, 10)).unwrap();
            assert!(d.move_part(i, -1, 0).unwrap().changed)
        }));
        dock.undo();
    }
    stats("move into a hole, mirror", v);
    let mut v = Vec::new();
    for _ in 0..runs {
        v.push(t(&mut dock, &mut |d| {
            assert!(
                d.place(any(&c, "pulse_laser", 10, 10, 0, false))
                    .unwrap()
                    .changed
            )
        }));
        dock.undo();
    }
    stats("place into a hole, mirror", v);
    let mut v = Vec::new();
    for _ in 0..runs {
        v.push(t(&mut dock, &mut |d| {
            assert!(
                d.place_slope("slope_1x1", (10, 10), "steel", None)
                    .unwrap()
                    .changed
            )
        }));
        dock.undo();
    }
    stats("slope into a hole (auto-orient), mirror", v);
    let mut v = Vec::new();
    for _ in 0..runs {
        v.push(t(&mut dock, &mut |d| {
            let o = d.place(any(&c, "fusion_reactor", 20, 20, 0, false)).unwrap();
            assert_eq!(o.twin, Twin::Applied)
        }));
        dock.undo();
    }
    stats("place a 2×2 over 4 blocks (replace), mirror", v);
    dock.place(any(&c, "pulse_laser", 30, 30, 0, false)).unwrap();
    let mut v = Vec::new();
    for _ in 0..runs {
        v.push(t(&mut dock, &mut |d| {
            let o = d.paint((30, 30), "steel").unwrap();
            assert_eq!(o.twin, Twin::Applied)
        }));
        dock.undo();
    }
    stats("paint over a machine (replace), mirror", v);
    dock.mirror = false;
    dock.bevel = true;
    let mut v = Vec::new();
    for _ in 0..runs / 10 {
        let mut d = empty(&c, (64, 64));
        d.bevel = true;
        d.begin_stroke();
        for x in 0..64 {
            d.paint((x, 5), "steel").unwrap();
            d.paint((x, 6), "steel").unwrap();
        }
        v.push(t(&mut d, &mut |d| d.end_stroke()));
        assert_eq!(d.parts().iter().filter(|p| p.part == "slope_1x1").count(), 4);
    }
    stats("end a bevel stroke of 128 cells", v);
    dock.bevel = false;
    let mut v = Vec::new();
    for _ in 0..runs {
        v.push(t(&mut dock, &mut |d| {
            let i = d.part_at((20, 20)).unwrap();
            assert!(d.rotate(i, true).unwrap().changed)
        }));
        dock.undo();
    }
    stats("rotate a block, mirror", v);
    dock.mirror = false;
    let (mut fill, mut undo) = (Vec::new(), Vec::new());
    for _ in 0..40 {
        let mut d = empty(&c, (64, 64));
        fill.push(t(&mut d, &mut |d| {
            d.fill((0, 0), (63, 63), "steel").unwrap();
        }));
        undo.push(t(&mut d, &mut |d| assert!(d.undo())));
    }
    stats("fill an empty 64×64 (4096 blocks)", fill);
    stats("undo that fill", undo);
    let mut v = Vec::new();
    for _ in 0..20 {
        v.push(t(&mut dock, &mut |d| {
            let _ = d.export();
        }));
    }
    stats("export 4096 parts", v);
    let s = Instant::now();
    let d = full();
    println!("{:<44} {:>10.2?}", "open + fill 64×64", s.elapsed());
    assert_eq!(d.parts().len(), 4096);
}

/// GDR-0088: a gun goes into weapon group 1 to 3 (and its mirror twin with it), group 1 is stored as none, other
/// parts take no group, and the group is written into the blueprint text and read back; undo takes it back.
#[test]
fn guns_take_a_weapon_group_with_their_twin() {
    let c = content();
    let mut dock = empty(&c, (10, 8));
    dock.mirror = true;
    assert_eq!(dock.place(any(&c, "pulse_laser", 1, 2, 0, false)).unwrap().part, Some(0));
    assert_eq!(dock.parts().len(), 2, "the twin");
    assert!(dock.set_group(0, 2).unwrap().changed);
    assert!(dock.parts().iter().all(|p| p.group == Some(2)), "{:?}", dock.parts());
    assert_eq!(dock.set_group(0, 4), Err(Refusal::BadGroup));
    assert_eq!(dock.set_group(0, 0), Err(Refusal::BadGroup));
    dock.set_group(0, 1).unwrap();
    assert!(dock.parts().iter().all(|p| p.group.is_none()), "group 1 is none");
    dock.set_group(0, 3).unwrap();
    let text = data::blueprint_to_ron(&dock.export());
    assert!(text.contains("group: 3"), "{text}");
    assert_eq!(data::parse_blueprint(&text).unwrap().parts[0].group, Some(3));
    dock.undo();
    assert!(dock.parts().iter().all(|p| p.group.is_none()));
    assert_eq!(dock.place(any(&c, "armor_block", 5, 5, 0, false)).map(|o| o.part.is_some()), Ok(true));
    let block = dock.parts().iter().position(|p| p.part == "armor_block").unwrap();
    assert_eq!(dock.set_group(block, 2), Err(Refusal::NotWeapon));
    assert_eq!(dock.check(&Placement { group: Some(2), ..any(&c, "armor_block", 0, 0, 0, false) }), Err(Refusal::NotWeapon));
}
