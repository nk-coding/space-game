//! Geometry cases from the drydock rules (`docs/concept/reference/drydock-model.js`).

use data::geometry::{hull_loops, transform_cell};
use data::{Dir, Footprint, GridPoint, PlacedShape, ShipGrid};

fn at(footprint: Footprint, x: i32, y: i32, rot: u8, flip: bool) -> PlacedShape {
    PlacedShape {
        footprint,
        x,
        y,
        rot,
        flip,
    }
}
fn block(x: i32, y: i32) -> PlacedShape {
    at(Footprint::rect(1, 1), x, y, 0, false)
}
fn slope(w: u32, x: i32, y: i32, rot: u8, flip: bool) -> PlacedShape {
    at(Footprint::slope(w), x, y, rot, flip)
}
fn grid(shapes: Vec<PlacedShape>) -> ShipGrid {
    ShipGrid::new(shapes).expect("a grid of nearby parts")
}

#[test]
fn slope_links_only_through_straight_sides() {
    // slope_1x1, rot 0: right angle bottom-left, straight sides Left and Down, face up-right.
    let s = slope(1, 1, 1, 0, false);
    let mut sides: Vec<Dir> = s.connectable_sides().into_iter().map(|(_, d)| d).collect();
    sides.sort();
    assert_eq!(sides, [Dir::Down, Dir::Left]);
    for (nx, ny, d, linked) in [
        (0, 1, Dir::Left, true),
        (1, 2, Dir::Down, true),
        (2, 1, Dir::Right, false),
        (1, 0, Dir::Up, false),
    ] {
        let g = grid(vec![s, block(nx, ny)]);
        assert_eq!(g.linked((1, 1), d), linked, "neighbour on {d:?}");
        assert_eq!(g.linked((nx, ny), d.opposite()), linked);
        assert_eq!(g.pieces().len(), if linked { 1 } else { 2 });
    }
}

#[test]
fn long_slope_sides() {
    // slope_1x2 at (0,0): cells (0,0),(1,0); straight: Left of (0,0), Down of both.
    let s = slope(2, 0, 0, 0, false);
    let g = grid(vec![
        s,
        block(1, 1),
        block(2, 0),
        block(1, -1),
        block(-1, 0),
    ]);
    assert!(g.linked((1, 0), Dir::Down));
    assert!(
        !g.linked((1, 0), Dir::Right),
        "the thin end is only a point"
    );
    assert!(!g.linked((1, 0), Dir::Up), "the face");
    assert!(g.linked((0, 0), Dir::Left));
    assert!(
        g.linked((0, 0), Dir::Right),
        "cells of one part are always linked"
    );
    assert_eq!(g.neighbors(0), vec![1, 4]);
    // Rotated a quarter turn it stands 1 wide, 2 high: straight sides Up (both cells? no: the old Left) …
    let r = slope(2, 0, 0, 1, false);
    assert_eq!(r.dims(), (1, 2));
    let mut sides: Vec<_> = r.connectable_sides();
    sides.sort();
    // old Left (cell 0,0) -> Up of (0,0); old Down (cells 0,0 and 1,0) -> Left of (0,0) and (0,1)
    assert_eq!(
        sides,
        vec![((0, 0), Dir::Up), ((0, 0), Dir::Left), ((0, 1), Dir::Left)]
    );
}

#[test]
fn slopes_meeting_face_to_face_do_not_link() {
    // A "V": left slope faces up-right, right slope (mirrored) faces up-left; they touch along the
    // vertical line between them, where neither has a straight side.
    let g = grid(vec![slope(1, 0, 0, 0, false), slope(1, 1, 0, 0, true)]);
    assert!(!g.linked((0, 0), Dir::Right));
    assert_eq!(g.pieces().len(), 2);
    // Back to back (a peak): the left slope mirrored has its straight side on the right, the right one
    // unmirrored has its straight side on the left.
    let g = grid(vec![slope(1, 0, 0, 0, true), slope(1, 1, 0, 0, false)]);
    assert!(g.linked((0, 0), Dir::Right));
    assert_eq!(g.pieces().len(), 1);
}

#[test]
fn bevel_orientations_exist_for_every_corner() {
    // drydock-model `bevel`: a block with exactly two linked, adjacent sides becomes a slope_1x1 whose
    // straight sides are exactly those two. There must be such an orientation for each corner, and none
    // for opposite sides.
    let orientations: Vec<(u8, bool)> = (0..4).flat_map(|r| [(r, false), (r, true)]).collect();
    let sides_of = |rot, flip| {
        let mut v: Vec<Dir> = slope(1, 0, 0, rot, flip)
            .connectable_sides()
            .into_iter()
            .map(|(_, d)| d)
            .collect();
        v.sort();
        v
    };
    for pair in [
        [Dir::Up, Dir::Right],
        [Dir::Right, Dir::Down],
        [Dir::Up, Dir::Left],
        [Dir::Down, Dir::Left],
    ] {
        let mut want = pair.to_vec();
        want.sort();
        assert!(
            orientations.iter().any(|&(r, f)| sides_of(r, f) == want),
            "{pair:?}"
        );
    }
    for pair in [[Dir::Up, Dir::Down], [Dir::Left, Dir::Right]] {
        let mut want = pair.to_vec();
        want.sort();
        assert!(!orientations.iter().any(|&(r, f)| sides_of(r, f) == want));
    }
}

#[test]
fn exposure_counts_open_straight_sides_plus_slope_face() {
    let g = grid(vec![block(0, 0)]);
    assert_eq!(g.exposure((0, 0)), 4);
    let g = grid(vec![slope(1, 0, 0, 0, false)]);
    assert_eq!(g.exposure((0, 0)), 3);
    let mut parts = Vec::new();
    for y in 0..3 {
        for x in 0..3 {
            parts.push(block(x, y));
        }
    }
    let mut g = grid(parts);
    assert_eq!(g.exposure((1, 1)), 0);
    assert!(!g.is_exposed(4));
    assert_eq!(g.exposure((0, 0)), 2);
    // A slope beside a block covers nothing: the block's side towards the face stays open.
    g.remove(5); // (2,1)
    let g2 = {
        let mut v: Vec<PlacedShape> = g.shapes().to_vec();
        v[5] = slope(1, 2, 1, 0, true); // flipped: straight sides Right and Down, face towards (1,1)
        grid(v)
    };
    assert_eq!(g2.exposure((1, 1)), 1);
}

#[test]
fn pieces_split_when_a_connector_breaks() {
    let mut g = grid(vec![block(0, 0), block(1, 0), block(2, 0)]);
    assert_eq!(g.pieces().len(), 1);
    g.remove(1);
    let p = g.pieces();
    assert_eq!(p.parts, vec![vec![0], vec![2]]);
    assert_eq!(p.piece_of_part, vec![Some(0), None, Some(1)]);
    // A core makes a smaller piece the main one.
    assert_eq!(g.main_piece(&p, |i| i == 2), Some(1));
    assert_eq!(g.main_piece(&p, |_| false), Some(0));
}

#[test]
fn hull_edges_and_loops() {
    // One block: four unit sides, one square loop.
    let g = grid(vec![block(0, 0)]);
    let e = g.hull_edges(|_| true);
    assert_eq!(e.len(), 4);
    let l = hull_loops(&e);
    assert_eq!(l.len(), 1);
    assert_eq!(
        l[0].points,
        [
            GridPoint::new(0, 0),
            GridPoint::new(1, 0),
            GridPoint::new(1, 1),
            GridPoint::new(0, 1)
        ]
    );

    // Two linked blocks: the shared side is covered; the loop is a 2×1 rectangle.
    let g = grid(vec![block(0, 0), block(1, 0)]);
    let e = g.hull_edges(|_| true);
    assert_eq!(e.len(), 6);
    assert_eq!(hull_loops(&e)[0].points.len(), 4);
    // Keeping only one of them: the shared side is boundary again.
    assert_eq!(g.hull_edges(|i| i == 0).len(), 4);

    // Block + slope beside it, face towards the block: the block's side is boundary, plus the face.
    let g = grid(vec![block(0, 0), slope(1, 1, 0, 0, true)]);
    let e = g.hull_edges(|_| true);
    assert!(e.iter().any(|s| s.part == 0 && s.a.x == 1 && s.b.x == 1));
    assert_eq!(e.iter().filter(|s| s.slope).count(), 1);

    // Block + slope linked through its straight side: a trapezoid.
    let g = grid(vec![block(0, 0), slope(1, 1, 0, 0, false)]);
    let l = hull_loops(&g.hull_edges(|_| true));
    assert_eq!(l.len(), 1);
    assert_eq!(
        l[0].points,
        [
            GridPoint::new(0, 0),
            GridPoint::new(1, 0),
            GridPoint::new(2, 1),
            GridPoint::new(0, 1)
        ]
    );
    // Outward normals point away from the solid.
    for s in g.hull_edges(|_| true) {
        let n = s.outward_normal();
        let mid = [
            (s.a.x + s.b.x) as f32 / 2.0 + n[0] * 0.1,
            (s.a.y + s.b.y) as f32 / 2.0 + n[1] * 0.1,
        ];
        let cell = (mid[0].floor() as i32, mid[1].floor() as i32);
        if !s.slope {
            assert_eq!(g.occupant(cell), None, "{s:?}");
        }
    }
}

#[test]
fn blocks_touching_at_a_corner_give_two_loops() {
    let g = grid(vec![block(0, 0), block(1, 1)]);
    assert_eq!(g.pieces().len(), 2);
    let loops = hull_loops(&g.hull_edges(|_| true));
    assert_eq!(loops.len(), 2);
    assert!(loops.iter().all(|l| l.closed && l.points.len() == 4));
}

#[test]
fn mirroring_matches_drydock_mirror_part() {
    // drydock-model `mirrorPart`: x' = W - x - w, rot' = (4 - rot) % 4, flip' = !flip mirrors the part
    // exactly. Check cells, polygons and connectable sides for every footprint and orientation.
    const W: i32 = 16;
    let mirror_dir = |d: Dir| match d {
        Dir::Left => Dir::Right,
        Dir::Right => Dir::Left,
        d => d,
    };
    for fp in [
        Footprint::rect(1, 1),
        Footprint::rect(1, 2),
        Footprint::rect(2, 2),
        Footprint::slope(1),
        Footprint::slope(2),
        Footprint::slope(3),
    ] {
        for rot in 0..4u8 {
            for flip in [false, true] {
                let p = at(fp, 3, 4, rot, flip);
                let (w, _) = p.dims();
                let m = at(fp, W - p.x - w as i32, p.y, (4 - rot) % 4, !flip);
                let mut a: Vec<_> = p.cells().into_iter().map(|(x, y)| (W - 1 - x, y)).collect();
                let mut b = m.cells();
                a.sort();
                b.sort();
                assert_eq!(a, b, "cells {fp:?} rot {rot} flip {flip}");
                let mut pa: Vec<_> = p.polygon().into_iter().map(|q| (W - q.x, q.y)).collect();
                let mut pb: Vec<_> = m.polygon().into_iter().map(|q| (q.x, q.y)).collect();
                pa.sort();
                pb.sort();
                assert_eq!(pa, pb, "polygon {fp:?} rot {rot} flip {flip}");
                let mut sa: Vec<_> = p
                    .connectable_sides()
                    .into_iter()
                    .map(|((x, y), d)| ((W - 1 - x, y), mirror_dir(d)))
                    .collect();
                let mut sb = m.connectable_sides();
                sa.sort();
                sb.sort();
                assert_eq!(sa, sb, "sides {fp:?} rot {rot} flip {flip}");
            }
        }
    }
}

#[test]
fn rotating_a_placement_four_times_is_identity() {
    for fp in [Footprint::rect(2, 3), Footprint::slope(3)] {
        for flip in [false, true] {
            let base = at(fp, 0, 0, 0, flip);
            let mut cells = base.cells();
            cells.sort();
            let mut turned = at(fp, 0, 0, 4, flip).cells();
            turned.sort();
            assert_eq!(cells, turned);
            // Turning by one changes the shape's dims and stays inside them.
            let one = at(fp, 0, 0, 1, flip);
            assert_eq!(one.dims(), (fp.h, fp.w));
        }
    }
    // xfCell exactly: the JS loop for a 1×2 beam turned three times.
    assert_eq!(transform_cell(0, 0, 1, 2, 3, false), (0, 0));
    assert_eq!(transform_cell(0, 1, 1, 2, 3, false), (1, 0));
}

#[test]
fn guns_respect_rotation() {
    let gun = at(Footprint::rect(1, 1), 1, 1, 0, false);
    let g = grid(vec![gun, block(1, 3), block(2, 1)]);
    assert!(g.line_of_fire_clear(0));
    let g = grid(vec![gun, block(1, -2)]);
    assert!(
        !g.line_of_fire_clear(0),
        "anything ahead blocks, not just the next cell"
    );
    let turned = at(Footprint::rect(1, 1), 1, 1, 1, false);
    let g = grid(vec![turned, block(1, -2), block(4, 1)]);
    assert!(!g.line_of_fire_clear(0), "a gun turned right fires right");
}
