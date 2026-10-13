//! The station's layout as a pure function of the drydock frame (GDR-0071): where the U, its buildings, the gate
//! and the moorings sit, in world metres (1 m = 1 cell; y up). The inside of the U *is* the frame: blueprint
//! `(x, y)` sits at world `(x, −y)`, so the building area spans `0..w × −h..0` exactly where the drydock draws its
//! grid, and the docked ship sits in it where it was built.
//!
//! The numbers are the chosen sketch's (`docs/plans/mockups/shipyard-long-u.html`, drawn at 20 px per cell for the
//! 24 × 24 frigate frame): walls, buildings and the gate keep their size, and everything is placed relative to the
//! building area, so a bigger frame makes a bigger U with the buildings moved along. Placeholders (GDR-0071).

use bevy::math::{Rect, Vec2};

/// Wall thickness of the U (62 px in the sketch).
pub const WALL: f32 = 3.1;
/// How far the arms reach past the building area towards the gate.
pub const LIP: f32 = 3.5;
/// The chamfered tip at the end of each arm.
pub const TIP: f32 = 1.3;
/// The chamfered strip along the base's outer face.
pub const BASE_STRIP: f32 = 1.1;
/// The gate's ring radius and how far its centre lies past the arm tips.
pub const GATE_RADIUS: f32 = 7.5;
const GATE_GAP: f32 = 13.0;
/// Escort moorings: how many, their inner half height, depth (base to tips) and wall.
pub const MOORINGS: usize = 4;
pub const MOORING_HALF: f32 = 2.6;
pub const MOORING_DEPTH: f32 = 5.0;
pub const MOORING_WALL: f32 = 1.2;
/// Crane fractions along the building area, and how many.
pub const CRANES: [f32; 3] = [0.18, 0.5, 0.82];
/// Docking clamps: fractions along the arms (one on each arm at each), how far they reach in.
pub const CLAMPS: [f32; 2] = [0.3, 0.7];
pub const CLAMP_REACH: f32 = 1.3;

/// A place on the station: what the camera flies to, what a click or a key opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    /// The building area inside the U: the drydock.
    Drydock,
    /// The mind behind the base: the BEHAVIOR screen.
    Behavior,
    /// The research lab with dishes on the top wall.
    Research,
    /// The gate in front of the mouth: voyages.
    Voyages,
    /// The tower and target lane under the bottom wall.
    TestRange,
    /// The moorings on the truss bridge off the left edge: FLEET.
    Fleet,
    /// Dark until it has a screen.
    Fabricator,
    /// Dark until it has a screen.
    SalvageBay,
}

impl Place {
    pub const ALL: [Place; 8] =
        [Place::Drydock, Place::Behavior, Place::Research, Place::Voyages, Place::TestRange, Place::Fleet, Place::Fabricator, Place::SalvageBay];
}

/// A building attached to the U: its body and the strut joining it to the station.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Building {
    pub centre: Vec2,
    pub size: Vec2,
    pub strut: (Vec2, Vec2),
}

impl Building {
    pub fn rect(&self) -> Rect {
        Rect::from_center_size(self.centre, self.size)
    }
}

/// How a label lines up with its anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Middle,
    End,
}

/// Everything's place for one frame size.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// The frame in cells.
    pub frame: (u32, u32),
    /// The building area: the frame, `(0, −h)..(w, 0)`.
    pub area: Rect,
    /// Where the arms' plates end (the chamfered tips go on from here).
    pub tip_x: f32,
    /// The base's outer face.
    pub base_x: f32,
    pub research: Building,
    pub fabricator: Building,
    pub mind: Building,
    /// The test range's control tower and its target lane (start and end of its centre line).
    pub range: Building,
    pub lane: (Vec2, Vec2),
    pub salvage: Building,
    pub gate: Vec2,
    /// The approach path: chevron tips from the mouth towards the gate.
    pub chevrons: Vec<Vec2>,
    /// The two gate pylons' ends at the arm tips and at the gate.
    pub pylons: [(Vec2, Vec2); 2],
    /// Mooring centres (their interior's centre), escort 1 first: the near two, then two past the edge.
    pub moorings: [Vec2; MOORINGS],
    /// The truss bridges' y and their x span (from past the left edge to the mind's back).
    pub bridges: [f32; 2],
    pub bridge_x: (f32, f32),
    /// Crane rails' x in the yard, and where they park at the mouth while the drydock is open.
    pub cranes: [f32; 3],
    pub crane_park: [f32; 3],
    /// What home shows: the station from the moorings to past the gate.
    pub overview: Rect,
}

/// 16:9 box `width` m wide centred on `c`: how close the camera comes to a place.
fn focus_box(c: Vec2, width: f32) -> Rect {
    Rect::from_center_size(c, Vec2::new(width, width * 9.0 / 16.0))
}

impl Layout {
    /// The station around a `w × h` frame.
    pub fn new(frame: (u32, u32)) -> Self {
        let (w, h) = (frame.0.max(1) as f32, frame.1.max(1) as f32);
        let mid = -h / 2.0;
        let tip_x = w + LIP;
        let base_x = -WALL;
        let top = WALL; // the top arm's outer face
        let bottom = -h - WALL; // the bottom arm's outer face
        let back = base_x - BASE_STRIP;

        let research_c = Vec2::new(0.25 * w, top + 3.9);
        let research = Building { centre: research_c, size: Vec2::new(9.5, 5.5), strut: (research_c - Vec2::Y * 2.75, Vec2::new(research_c.x, top)) };
        // Right of research however small the frame.
        let fab_c = Vec2::new((0.8 * w).max(research_c.x + 9.25), top + 3.2);
        let fabricator = Building { centre: fab_c, size: Vec2::new(7.0, 4.3), strut: (fab_c - Vec2::Y * 2.15, Vec2::new(fab_c.x, top)) };
        let mind_c = Vec2::new(back - 6.0, mid);
        let mind = Building { centre: mind_c, size: Vec2::new(7.5, 10.0), strut: (mind_c + Vec2::X * 3.75, Vec2::new(back, mid)) };
        // Right of the salvage bay however small the frame.
        let range_c = Vec2::new((0.5 * w).max(8.25), bottom - 3.9);
        let range = Building { centre: range_c, size: Vec2::new(4.5, 3.5), strut: (range_c + Vec2::Y * 1.75, Vec2::new(range_c.x, bottom)) };
        let lane = (range_c + Vec2::X * 3.0, range_c + Vec2::X * 19.0);
        let salvage_c = Vec2::new(2.0, bottom - 3.0);
        let salvage = Building { centre: salvage_c, size: Vec2::new(6.0, 3.5), strut: (salvage_c + Vec2::Y * 1.75, Vec2::new(salvage_c.x, bottom)) };

        let gate = Vec2::new(tip_x + GATE_GAP, mid);
        let chevrons = (0..7).map(|k| Vec2::new(tip_x + 2.0 + 1.3 * k as f32, mid)).collect();
        let pylons = [-1.0f32, 1.0].map(|s| (Vec2::new(tip_x + TIP, mid + s * (h + WALL) / 2.0), Vec2::new(gate.x - 3.0, mid + s * 8.0)));

        let near = mind.rect().min.x - 5.05;
        let far = near - 11.0;
        let moorings = [Vec2::new(near, mid + 7.5), Vec2::new(near, mid - 7.5), Vec2::new(far, mid + 7.5), Vec2::new(far, mid - 7.5)];
        let bridges = [mid + 7.5, mid - 7.5];
        let bridge_x = (far - MOORING_DEPTH - 8.0, mind.rect().min.x);

        let cranes = CRANES.map(|f| f * w);
        let crane_park = [0.7, 1.6, 2.5].map(|d| w + d);

        let overview = Rect::new(near - 4.5, range_c.y - 3.5, gate.x + GATE_RADIUS + 8.5, research.rect().max.y + 0.75);
        Self {
            frame,
            area: Rect::new(0.0, -h, w, 0.0),
            tip_x,
            base_x,
            research,
            fabricator,
            mind,
            range,
            lane,
            salvage,
            gate,
            chevrons,
            pylons,
            moorings,
            bridges,
            bridge_x,
            cranes,
            crane_park,
            overview,
        }
    }

    /// The U's outer bounds (tips and the base's strip included).
    pub fn station(&self) -> Rect {
        Rect::new(self.base_x - BASE_STRIP, self.area.min.y - WALL, self.tip_x + TIP, self.area.max.y + WALL)
    }

    /// The near moorings and their bridge: FLEET's place.
    fn fleet_rect(&self) -> Rect {
        let near = self.moorings[0].x;
        let reach = MOORING_HALF + MOORING_WALL;
        Rect::new(near - MOORING_DEPTH / 2.0 - MOORING_WALL - 0.6, self.bridges[1] - reach, self.mind.rect().min.x, self.bridges[0] + reach)
    }

    /// Where a click or a tap finds the place.
    pub fn hit(&self, place: Place) -> Rect {
        match place {
            Place::Drydock => self.station(),
            Place::Behavior => self.mind.rect(),
            Place::Research => self.research.rect().union(Rect::new(self.research.centre.x - 4.6, self.research.centre.y, self.research.centre.x + 4.2, self.research.centre.y + 4.4)),
            Place::Voyages => Rect::from_center_half_size(self.gate, Vec2::splat(GATE_RADIUS + 1.4)),
            Place::TestRange => self.range.rect().union(Rect::from_corners(self.lane.0 - Vec2::Y * 1.1, self.lane.1 + Vec2::new(1.1, 1.1))),
            Place::Fleet => self.fleet_rect(),
            Place::Fabricator => self.fabricator.rect(),
            Place::SalvageBay => self.salvage.rect(),
        }
    }

    /// What the camera frames when it flies to the place (the drydock's own view replaces the area's box when the
    /// drydock is laid out).
    pub fn focus(&self, place: Place) -> Rect {
        match place {
            Place::Drydock => self.area.inflate(1.5),
            Place::Behavior => focus_box(self.mind.centre, 26.0),
            Place::Research => focus_box(self.research.centre + Vec2::Y * 0.5, 28.0),
            Place::Voyages => focus_box(self.gate - Vec2::X * 2.0, 32.0),
            Place::TestRange => focus_box(self.range.centre + Vec2::X * 2.0, 30.0),
            Place::Fleet => focus_box(Vec2::new(self.moorings[0].x + 1.5, self.area.center().y), 38.0),
            Place::Fabricator => focus_box(self.fabricator.centre, 24.0),
            Place::SalvageBay => focus_box(self.salvage.centre, 23.0),
        }
    }

    /// Where the place's label goes: the point its title's top lines up with, and how.
    pub fn label(&self, place: Place) -> (Vec2, Align) {
        match place {
            Place::Drydock => (Vec2::new(self.area.center().x, self.area.min.y + 1.3), Align::Middle),
            Place::Behavior => (self.mind.centre - Vec2::Y * 5.8, Align::Middle),
            Place::Research => (self.research.centre + Vec2::new(-5.4, 2.2), Align::End),
            Place::Voyages => (self.gate - Vec2::Y * (GATE_RADIUS + 2.6), Align::Middle),
            Place::TestRange => (Vec2::new((self.lane.0.x + self.lane.1.x) / 2.0, self.range.centre.y - 1.6), Align::Middle),
            Place::Fleet => (Vec2::new(self.moorings[0].x - 1.5, self.bridges[1] - 7.0), Align::Start),
            Place::Fabricator => (self.fabricator.centre + Vec2::Y * 3.8, Align::Middle),
            Place::SalvageBay => (self.salvage.centre - Vec2::Y * 2.0, Align::Middle),
        }
    }

    /// The name plate on the base: its centre and size (the name runs up it).
    pub fn name_plate(&self) -> Rect {
        Rect::from_center_size(Vec2::new(self.base_x + 1.95, self.area.center().y), Vec2::new(0.9, 4.6))
    }
}

/// The view (the world point at the window's centre and px per metre) that shows all of `rect` in a `viewport`
/// of logical px.
pub fn fit(rect: Rect, viewport: Vec2) -> (Vec2, f32) {
    let zoom = (viewport.x / rect.width()).min(viewport.y / rect.height());
    (rect.center(), zoom)
}

/// A box of fixed screen size hung on a world point (a label over the station): where it reaches from the point,
/// in logical px, y down (`min` ≤ 0 ≤ `max` on each axis).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tag {
    pub at: Vec2,
    pub min: Vec2,
    pub max: Vec2,
}

/// The view that shows all of `rect` and every tag at least `margin` logical px inside a `viewport`, centred on
/// `rect`: the largest zoom at which nothing reaches past an edge. The world grows with the zoom while the tags keep
/// their pixel size, so each edge of each tag gives one bound on the zoom; the window's aspect decides which bound
/// wins (a wide window: the height, a tall one: the width). A tag that can't fit at any zoom (its pixels alone
/// are wider than the room) is ignored rather than shrinking the station to nothing.
pub fn fit_tagged(rect: Rect, tags: &[Tag], viewport: Vec2, margin: f32) -> (Vec2, f32) {
    let c = rect.center();
    let half = viewport / 2.0 - Vec2::splat(margin);
    let mut zoom = (half.x * 2.0 / rect.width()).min(half.y * 2.0 / rect.height());
    for t in tags {
        // Screen offset of the anchor from the centre at zoom z: d · z (y down: −dy).
        let d = Vec2::new(t.at.x - c.x, c.y - t.at.y);
        for (dist, lo, hi, room) in [(d.x, t.min.x, t.max.x, half.x), (d.y, t.min.y, t.max.y, half.y)] {
            // dist·z + hi ≤ room and dist·z + lo ≥ −room.
            let bound = if dist > 1e-6 {
                (room - hi) / dist
            } else if dist < -1e-6 {
                (room + lo) / -dist
            } else {
                f32::INFINITY
            };
            if bound > 0.0 {
                zoom = zoom.min(bound);
            }
        }
    }
    (c, zoom.max(1e-3))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The building area is the frame, exactly where the drydock draws it; the U closes around it with its walls.
    #[test]
    fn the_inside_of_the_u_is_the_drydock_frame() {
        for frame in [(24, 24), (40, 40), (64, 64), (30, 18)] {
            let l = Layout::new(frame);
            assert_eq!(l.area, Rect::new(0.0, -(frame.1 as f32), frame.0 as f32, 0.0), "{frame:?}");
            let s = l.station();
            assert_eq!(s.min.y, l.area.min.y - WALL);
            assert_eq!(s.max.y, l.area.max.y + WALL);
            assert_eq!(l.base_x, -WALL);
            assert_eq!(l.tip_x, l.area.max.x + LIP);
        }
    }

    /// A bigger frame makes a bigger U and moves the buildings along; their sizes stay.
    #[test]
    fn a_bigger_frame_makes_a_bigger_station() {
        let (small, big) = (Layout::new((24, 24)), Layout::new((40, 40)));
        let grew = |a: f32, b: f32| (b - a - 16.0).abs() < 1e-4;
        assert!(grew(small.station().width(), big.station().width()));
        assert!(grew(small.station().height(), big.station().height()));
        assert_eq!(small.research.size, big.research.size);
        assert_eq!(small.mind.size, big.mind.size);
        assert!(grew(small.gate.x, big.gate.x));
        assert!(big.overview.width() > small.overview.width() && big.overview.height() > small.overview.height());
        // The frigate's overview is the sketch's 1600 x 900 px at 20 px per cell: 80 x 45 m, 16:9.
        assert!((small.overview.width() - 80.0).abs() < 1e-3 && (small.overview.height() - 45.0).abs() < 1e-3, "{:?}", small.overview);
    }

    /// Every place is on the station and none sits on another; the overview shows all of them but the moorings past
    /// the edge, and a place's close-up is closer than the overview.
    #[test]
    fn places_are_apart_and_in_view() {
        for frame in [(24, 24), (40, 40), (64, 64), (30, 18), (12, 12)] {
            let l = Layout::new(frame);
            for (i, a) in Place::ALL.iter().enumerate() {
                let r = l.hit(*a);
                assert!(r.width() > 1.0 && r.height() > 1.0, "{a:?}");
                assert!(l.overview.contains(r.center()), "{a:?} {frame:?}");
                assert!(l.focus(*a).width() < l.overview.width(), "{a:?}");
                assert!(l.overview.contains(l.label(*a).0), "{a:?} label {frame:?}");
                for b in &Place::ALL[i + 1..] {
                    assert!(r.intersect(l.hit(*b)).is_empty(), "{a:?} overlaps {b:?} at {frame:?}");
                }
            }
            // The near moorings are on screen, the far ones past the left edge.
            assert!(l.overview.contains(l.moorings[0]) && l.overview.contains(l.moorings[1]));
            assert!(l.moorings[2].x + MOORING_DEPTH < l.overview.min.x && l.moorings[3].x + MOORING_DEPTH < l.overview.min.x);
            assert!(l.bridge_x.0 < l.overview.min.x, "the bridge runs off the edge");
            // The cranes park at the mouth, past the frame and inside the arms.
            for (x, p) in l.cranes.iter().zip(l.crane_park) {
                assert!(*x > 0.0 && *x < l.area.max.x && p > l.area.max.x && p < l.tip_x);
            }
        }
    }

    /// Labels of fixed pixel size hung on the station fit too, with the margin, at every aspect the play-test named;
    /// the station itself always does.
    #[test]
    fn the_overview_and_its_labels_fit_any_aspect() {
        let l = Layout::new((24, 24));
        let margin = 16.0;
        for (w, h) in [(1280.0, 720.0), (1600.0, 900.0), (1440.0, 1080.0), (1280.0, 1024.0), (3440.0, 1305.0), (1080.0, 1350.0)] {
            let viewport = Vec2::new(w, h);
            // Every place's label: 260 px wide, 30 high, aligned like home's.
            let tags: Vec<Tag> = Place::ALL
                .iter()
                .map(|p| {
                    let (at, align) = l.label(*p);
                    let (x0, x1) = match align {
                        Align::Start => (0.0, 260.0),
                        Align::Middle => (-130.0, 130.0),
                        Align::End => (-260.0, 0.0),
                    };
                    Tag { at, min: Vec2::new(x0, 0.0), max: Vec2::new(x1, 30.0) }
                })
                .collect();
            let (c, z) = fit_tagged(l.overview, &tags, viewport, margin);
            let screen = |p: Vec2| viewport / 2.0 + Vec2::new(p.x - c.x, c.y - p.y) * z;
            let inside = |a: Vec2| a.x >= margin - 1e-2 && a.y >= margin - 1e-2 && a.x <= w - margin + 1e-2 && a.y <= h - margin + 1e-2;
            for corner in [l.overview.min, l.overview.max, Vec2::new(l.overview.min.x, l.overview.max.y), Vec2::new(l.overview.max.x, l.overview.min.y)] {
                assert!(inside(screen(corner)), "{w}x{h}: corner {corner}");
            }
            for t in &tags {
                let p = screen(t.at);
                assert!(inside(p + t.min) && inside(p + t.max), "{w}x{h}: tag at {} reaches {:?}", t.at, (p + t.min, p + t.max));
            }
            // It is the largest such zoom: a little closer, something leaves.
            let closer = |k: f32| {
                let z = z * k;
                let s = |p: Vec2| viewport / 2.0 + Vec2::new(p.x - c.x, c.y - p.y) * z;
                let ok = |a: Vec2| a.x >= margin - 1e-2 && a.y >= margin - 1e-2 && a.x <= w - margin + 1e-2 && a.y <= h - margin + 1e-2;
                ok(s(l.overview.min)) && ok(s(l.overview.max)) && tags.iter().all(|t| ok(s(t.at) + t.min) && ok(s(t.at) + t.max))
            };
            assert!(!closer(1.02), "{w}x{h}: not the closest fit");
        }
        // Without tags it is the plain fit inside the margin.
        let (_, z) = fit_tagged(l.overview, &[], Vec2::new(1280.0, 720.0), 0.0);
        assert!((z - 16.0).abs() < 1e-3);
    }

    /// The overview fits any window: the whole station shows, centred.
    #[test]
    fn the_overview_fits_the_window() {
        let l = Layout::new((24, 24));
        let (c, z) = fit(l.overview, Vec2::new(1280.0, 720.0));
        assert_eq!(c, l.overview.center());
        assert!((z - 16.0).abs() < 1e-3);
        let (_, tall) = fit(l.overview, Vec2::new(800.0, 900.0));
        assert!((tall - 10.0).abs() < 1e-3, "a narrow window fits the width");
    }
}
