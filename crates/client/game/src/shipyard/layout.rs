//! The station's layout as a pure function of the drydock frame (GDR-0071): where the U, its buildings, the gate
//! and the moorings sit, in world metres (1 m = 1 cell; y up). The inside of the U *is* the frame: blueprint
//! `(x, y)` sits at world `(x, −y)`, so the building area spans `0..w × −h..0` exactly where the drydock draws its
//! grid, and the docked ship sits in it where it was built.
//!
//! The numbers are the chosen sketch's (GDR-0071, drawn at 20 px per cell for the
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
/// How far out of the ring the gate's four big pods sit (the pylons end on the two facing the station).
pub const GATE_POD: f32 = 0.9;
const GATE_GAP: f32 = 13.0;
/// The truss bridges' width.
pub const BRIDGE: f32 = 0.8;
/// Escort moorings: how many, their inner half width, depth (base to tips) and wall. Each hangs off a bridge,
/// its base on the bridge's outer edge and its mouth away from it, so the bridge runs past unbroken.
pub const MOORINGS: usize = 4;
pub const MOORING_HALF: f32 = 2.6;
pub const MOORING_DEPTH: f32 = 5.0;
pub const MOORING_WALL: f32 = 1.2;
/// Crane fractions along the building area, and how many.
pub const CRANES: [f32; 3] = [0.18, 0.5, 0.82];
/// Docking clamps: fractions along the arms (one on each arm at each), how far they reach in.
pub const CLAMPS: [f32; 2] = [0.3, 0.7];
pub const CLAMP_REACH: f32 = 1.3;
/// The EVOLUTION lab's label's two lines (logical px): its top stands this much over the building, and a gap.
pub const EVOLUTION_LABEL_PX: f32 = 32.0;
/// How many places the shuttles fly between ([`Layout::shuttle_stops`]).
pub const SHUTTLE_STOPS: usize = 13;

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
    /// The moorings on the truss bridges left of the mind: FLEET.
    Fleet,
    /// The records mast on the top arm's tip, by the gate: the LOG.
    Log,
    /// The compute stack above the mind, on the base's outer face: the EVOLUTION lab (GDR-0092).
    Evolution,
    /// The workshop under the bottom wall by the moorings (GDR-0108): the haul's parts, the stock, fabricating.
    Workshop,
}

impl Place {
    pub const ALL: [Place; 9] = [
        Place::Drydock,
        Place::Behavior,
        Place::Research,
        Place::Voyages,
        Place::TestRange,
        Place::Fleet,
        Place::Log,
        Place::Evolution,
        Place::Workshop,
    ];
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

/// How much a building's block is chamfered at each corner.
pub fn chamfer(r: Rect) -> f32 {
    r.width().min(r.height()) * 0.18
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
    pub mind: Building,
    /// The test range's control tower and its target lane (start and end of its centre line, a strut apart).
    pub range: Building,
    pub lane: (Vec2, Vec2),
    /// The workshop (GDR-0108), where the salvage bay stood; the fabricator's slot on the top wall is free.
    pub workshop: Building,
    /// The LOG's records house on the top arm by its tip (GDR-0091), and the top of its mast: the beacon, where it
    /// glows while a voyage's result waits.
    pub log: Building,
    pub log_beacon: Vec2,
    /// The EVOLUTION lab's compute stack (GDR-0092) above the mind, its strut onto the base's outer face, and the
    /// mind's violet conduit down to it (from its foot to the mind's top face).
    pub evolution: Building,
    pub evolution_conduit: (Vec2, Vec2),
    pub gate: Vec2,
    /// The approach path: chevron tips from the mouth towards the gate.
    pub chevrons: Vec<Vec2>,
    /// The two gate pylons' ends at the arm tips and at the gate.
    pub pylons: [(Vec2, Vec2); 2],
    /// Mooring centres (their interior's centre), escort 1 first: the near two, then the far two; the upper ones
    /// open up, the lower ones down.
    pub moorings: [Vec2; MOORINGS],
    /// The truss bridges' y and their x span (from just past the far moorings to where they bend towards the mind).
    pub bridges: [f32; 2],
    pub bridge_x: (f32, f32),
    /// Where each bridge bends, and where it then meets the middle of the mind's chamfered corner (square onto it).
    pub mind_joints: [(Vec2, Vec2); 2],
    /// Crane rails' x in the yard, and where they park at the mouth while the drydock is open.
    pub cranes: [f32; 3],
    pub crane_park: [f32; 3],
    /// What home shows: the whole station, from the far moorings to the gate, the same margin past each (Niklas,
    /// 14 Oct 2026: the whole station centred, not the U; at any window shape).
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
        // The free slot right of research however small the frame (the fabricator's until GDR-0108): the LOG keeps
        // clear of its right edge.
        let free_right = (0.8 * w).max(research_c.x + 9.25) + 3.5;
        let mind_c = Vec2::new(back - 6.0, mid);
        let mind = Building { centre: mind_c, size: Vec2::new(7.5, 10.0), strut: (mind_c + Vec2::X * 3.75, Vec2::new(back, mid)) };
        // Right of the workshop however small the frame.
        let range_c = Vec2::new((0.5 * w).max(8.25), bottom - 3.9);
        let range = Building { centre: range_c, size: Vec2::new(4.5, 3.5), strut: (range_c + Vec2::Y * 1.75, Vec2::new(range_c.x, bottom)) };
        // The lane's round end a strut's length right of the tower.
        let lane_start = range_c.x + 2.25 + 1.0 + 1.1;
        let lane = (Vec2::new(lane_start, range_c.y), Vec2::new(lane_start + 11.0, range_c.y));
        let workshop_c = Vec2::new(2.0, bottom - 3.0);
        let workshop = Building { centre: workshop_c, size: Vec2::new(6.0, 3.5), strut: (workshop_c + Vec2::Y * 1.75, Vec2::new(workshop_c.x, bottom)) };

        // The LOG's house sits on the top arm just short of its tip, the end nearest the gate, its mast rising from
        // it; right of the free slot however small the frame (the strut then leans back onto the arm).
        let log_c = Vec2::new((tip_x - 1.6).max(free_right + 3.0), top + 1.9);
        let log = Building { centre: log_c, size: Vec2::new(2.4, 1.8), strut: (log_c - Vec2::Y * 0.9, Vec2::new(log_c.x.min(tip_x), top)) };
        let log_beacon = log_c + Vec2::Y * 4.6;

        // The EVOLUTION lab sits on the base's outer face above the mind, between the upper truss bridge and the top
        // arm's corner (GDR-0092): under the arm's level, lower as a taller frame drops the mind and opens the gap
        // (its label grows in metres as the yard's view zooms out), higher when the frame is so short that the mind
        // would be in the way. Its conduit drops onto the mind's top face right of the heat sinks.
        let evo_size = Vec2::new(6.0, 4.8);
        let evo_y = top - 5.1 - (h - 24.0).max(0.0) * 0.25;
        let evo_c = Vec2::new(mind_c.x + 0.8, evo_y.max(mind.rect().max.y + 1.2 + evo_size.y / 2.0));
        let evolution = Building { centre: evo_c, size: evo_size, strut: (evo_c + Vec2::X * evo_size.x / 2.0, Vec2::new(back, evo_c.y)) };
        let conduit_x = evo_c.x + 1.2;
        let evolution_conduit = (Vec2::new(conduit_x, evo_c.y - evo_size.y / 2.0), Vec2::new(conduit_x, mind.rect().max.y));

        let gate = Vec2::new(tip_x + GATE_GAP, mid);
        let chevrons = (0..7).map(|k| Vec2::new(tip_x + 2.0 + 1.3 * k as f32, mid)).collect();
        // From the arm tips' faces to the gate's pods facing the station.
        let pylons = [-1.0f32, 1.0].map(|s| (Vec2::new(tip_x + TIP, mid + s * (h + WALL) / 2.0), gate + Vec2::new(-1.0, s).normalize() * (GATE_RADIUS + GATE_POD)));

        let bridges = [mid + 6.5, mid - 6.5];
        // Each bridge bends by 45° into the middle of the mind's corner chamfer, so it meets the block square on.
        let (m, c) = (mind.rect(), chamfer(mind.rect()));
        let mind_joints = [1.0f32, -1.0].map(|s| {
            let end = Vec2::new(m.min.x + c / 2.0, mid + s * (m.height() / 2.0 - c / 2.0));
            let drop = (mid + s * 6.5 - end.y).abs();
            (Vec2::new(end.x - drop, mid + s * 6.5), end)
        });
        let bend = mind_joints[0].0.x;
        let near = bend - MOORING_HALF - MOORING_WALL - 0.3;
        let far = near - 2.0 * (MOORING_HALF + MOORING_WALL) - 2.0;
        let off = BRIDGE / 2.0 + MOORING_WALL + MOORING_DEPTH / 2.0;
        let moorings = [Vec2::new(near, bridges[0] + off), Vec2::new(near, bridges[1] - off), Vec2::new(far, bridges[0] + off), Vec2::new(far, bridges[1] - off)];
        let bridge_x = (far - MOORING_HALF - MOORING_WALL - 1.0, bend);

        let cranes = CRANES.map(|f| f * w);
        let crane_park = [0.7, 1.6, 2.5].map(|d| w + d);

        let side = 1.4;
        let overview = Rect::new(far - MOORING_HALF - MOORING_WALL - side, range_c.y - 3.5, gate.x + GATE_RADIUS + side, research.rect().max.y + 0.75);
        Self {
            frame,
            area: Rect::new(0.0, -h, w, 0.0),
            tip_x,
            base_x,
            research,
            mind,
            range,
            lane,
            workshop,
            log,
            log_beacon,
            evolution,
            evolution_conduit,
            gate,
            chevrons,
            pylons,
            moorings,
            bridges,
            bridge_x,
            mind_joints,
            cranes,
            crane_park,
            overview,
        }
    }

    /// The U's outer bounds (tips and the base's strip included).
    pub fn station(&self) -> Rect {
        Rect::new(self.base_x - BASE_STRIP, self.area.min.y - WALL, self.tip_x + TIP, self.area.max.y + WALL)
    }

    /// All four moorings and their bridges: FLEET's place.
    fn fleet_rect(&self) -> Rect {
        let far = self.moorings[2].x;
        let reach = MOORING_DEPTH / 2.0 + MOORING_WALL;
        Rect::new(far - MOORING_HALF - MOORING_WALL - 0.6, self.moorings[1].y - reach, self.mind.rect().min.x, self.moorings[0].y + reach)
    }

    /// Where the shuttles fly around the station's left end, all clear of the hulls: berths they may stop at
    /// (`true`: off the moorings' mouths, beside the mind, research and the workshop) and points in open space
    /// they pass through (between the bridges, past the gaps between the moorings, beside the mind).
    pub fn shuttle_stops(&self) -> [(Vec2, bool); SHUTTLE_STOPS] {
        let mid = self.area.center().y;
        let m = &self.moorings;
        // Off the mouths, and past them over the gap between the near and the far moorings.
        let (mouth, past) = (MOORING_DEPTH / 2.0 + 1.0, MOORING_DEPTH / 2.0 + 2.5);
        let gap = (m[0].x + m[2].x) / 2.0;
        let (r, s) = (self.research.rect(), self.workshop.rect());
        [
            (m[0] + Vec2::Y * mouth, true),
            (m[1] - Vec2::Y * mouth, true),
            (m[2] + Vec2::Y * mouth, true),
            (m[3] - Vec2::Y * mouth, true),
            (Vec2::new(self.mind.rect().min.x - 1.8, mid), true),
            (Vec2::new(r.min.x - 1.6, self.research.centre.y), true),
            (Vec2::new(s.min.x - 1.6, self.workshop.centre.y), true),
            (Vec2::new(m[2].x, mid), false),
            (Vec2::new(m[0].x, mid), false),
            (Vec2::new(gap, m[0].y + past), false),
            (Vec2::new(gap, m[1].y - past), false),
            (Vec2::new(self.mind.centre.x, self.research.centre.y), false),
            (Vec2::new(self.mind.centre.x, self.workshop.centre.y), false),
        ]
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
            Place::Log => self.log_rect(),
            Place::Evolution => self.evolution.rect(),
            Place::Workshop => self.workshop.rect(),
        }
    }

    /// The LOG's house and its mast up to the beacon.
    fn log_rect(&self) -> Rect {
        let r = self.log.rect();
        Rect::new(r.min.x, r.min.y, r.max.x, self.log_beacon.y + 0.6)
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
            Place::Fleet => focus_box(Vec2::new(self.moorings[0].x + 1.5, self.area.center().y), 50.0),
            Place::Log => focus_box(self.log_rect().center(), 20.0),
            Place::Evolution => focus_box(self.evolution.centre - Vec2::Y * 1.5, 22.0),
            Place::Workshop => focus_box(self.workshop.centre, 23.0),
        }
    }

    /// Where the place's label goes: the point its title's top lines up with, and how. On a phone (`compact`,
    /// docs/client.md "Phones") the labels are large beside the station, so the building area's hangs inside the
    /// area's top instead of its bottom (it ran into the gate's line under the ring).
    pub fn label(&self, place: Place, compact: bool) -> (Vec2, Align) {
        match place {
            // 10 px further in than its 1.3 m: clear of EVOLUTION's two lines over the stack, left of it.
            Place::Drydock if compact => (Vec2::new(self.area.center().x, self.area.max.y - 1.3 - 10.0 * self.metres_per_px()), Align::Middle),
            Place::Drydock => (Vec2::new(self.area.center().x, self.area.min.y + 1.3), Align::Middle),
            Place::Behavior => (self.mind.centre - Vec2::Y * 5.8, Align::Middle),
            Place::Research => (self.research.centre + Vec2::new(-5.4, 2.2), Align::End),
            Place::Voyages => (self.gate - Vec2::Y * (GATE_RADIUS + 2.6), Align::Middle),
            Place::TestRange => (Vec2::new((self.lane.0.x + self.lane.1.x) / 2.0, self.range.centre.y - 1.6), Align::Middle),
            Place::Fleet => (Vec2::new(self.moorings[0].x - MOORING_HALF - MOORING_WALL, self.moorings[1].y - MOORING_DEPTH / 2.0 - 0.8), Align::Start),
            // Right of the mast's top, over the open space above the gate.
            Place::Log => (self.log_beacon + Vec2::new(2.2, 0.4), Align::Start),
            // Over the stack, its right edge short of the station's corner, as high as its two lines are at the yard's
            // view, clear of the moorings.
            Place::Evolution => {
                let up = self.evolution.rect().max.y + 0.6 + EVOLUTION_LABEL_PX * self.metres_per_px();
                (Vec2::new(self.base_x - BASE_STRIP - 0.4, up), Align::End)
            }
            Place::Workshop => (self.workshop.centre - Vec2::Y * 2.0, Align::Middle),
        }
    }

    /// About how many metres a logical pixel is at the yard's view of a 16:9 window (the overview fitted inside the
    /// labels' margin): what a label of fixed pixels takes of the station.
    pub fn metres_per_px(&self) -> f32 {
        (self.overview.width() / 1244.0).max(self.overview.height() / 684.0)
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
        // The overview is centred on the whole station: as much room past the far moorings as past the gate.
        for l in [&small, &big] {
            let left = l.moorings[2].x - MOORING_HALF - MOORING_WALL - l.overview.min.x;
            let right = l.overview.max.x - (l.gate.x + GATE_RADIUS);
            assert!(left > 0.0 && (left - right).abs() < 1e-4, "{left} {right}");
        }
    }

    /// Every place is on the station and none sits on another; the overview shows all of them and every mooring, and a place's close-up is closer than the overview.
    #[test]
    fn places_are_apart_and_in_view() {
        for frame in [(24, 24), (40, 40), (64, 64), (30, 18), (12, 12)] {
            let l = Layout::new(frame);
            for (i, a) in Place::ALL.iter().enumerate() {
                let r = l.hit(*a);
                assert!(r.width() > 1.0 && r.height() > 1.0, "{a:?}");
                assert!(l.overview.contains(r.center()), "{a:?} {frame:?}");
                assert!(l.focus(*a).width() < l.overview.width(), "{a:?}");
                assert!(l.overview.contains(l.label(*a, false).0), "{a:?} label {frame:?}");
                for b in &Place::ALL[i + 1..] {
                    assert!(r.intersect(l.hit(*b)).is_empty(), "{a:?} overlaps {b:?} at {frame:?}");
                }
            }
            // Every mooring and the whole bridge are in view; the far moorings sit apart from the near ones.
            let reach = MOORING_HALF + MOORING_WALL;
            assert!(l.moorings.iter().all(|m| l.overview.contains(*m - Vec2::X * reach)));
            assert!(l.moorings[2].x + reach < l.moorings[0].x - reach);
            // The berths sit off their bridges, clear of the bridge they hang from.
            for (k, m) in l.moorings.iter().enumerate() {
                let b = l.bridges[k % 2];
                assert!((m.y - b).abs() - MOORING_DEPTH / 2.0 - MOORING_WALL >= BRIDGE / 2.0 - 1e-4, "{k} {frame:?}");
                assert!(l.overview.min.y < m.y - MOORING_DEPTH / 2.0 && m.y + MOORING_DEPTH / 2.0 < l.overview.max.y, "{k} {frame:?}");
            }
            assert!(l.bridge_x.0 > l.overview.min.x && l.bridge_x.0 < l.moorings[2].x - reach, "the bridge ends past the far moorings");
            // The cranes park at the mouth, past the frame and inside the arms.
            for (x, p) in l.cranes.iter().zip(l.crane_park) {
                assert!(*x > 0.0 && *x < l.area.max.x && p > l.area.max.x && p < l.tip_x);
            }
        }
    }

    /// The LOG's house and mast stand on the top arm by its tip, clear of every other place, of the buildings' bodies,
    /// the gate and its pods, and of the antenna at the arm's tip; its label hangs right of the mast, above the gate.
    #[test]
    fn the_log_stands_apart_by_the_gate() {
        for frame in [(24, 24), (40, 40), (64, 64), (30, 18), (12, 12)] {
            let l = Layout::new(frame);
            let log = l.hit(Place::Log);
            let top = l.area.max.y + WALL;
            assert!(log.min.y > top && log.max.y < l.overview.max.y, "{frame:?}: {log:?}");
            for p in Place::ALL.into_iter().filter(|p| *p != Place::Log) {
                assert!(log.inflate(0.5).intersect(l.hit(p)).is_empty(), "{frame:?}: the LOG touches {p:?}");
            }
            for b in [l.research.rect(), l.mind.rect(), l.range.rect(), l.workshop.rect()] {
                assert!(log.inflate(1.0).intersect(b).is_empty(), "{frame:?}: the LOG touches {b:?}");
            }
            let gate = Rect::from_center_half_size(l.gate, Vec2::splat(GATE_RADIUS + GATE_POD + 0.5));
            assert!(log.intersect(gate).is_empty(), "{frame:?}");
            let antenna = Rect::new(l.tip_x + 0.2, top, l.tip_x + 1.4, top + 1.5);
            assert!(log.intersect(antenna).is_empty(), "{frame:?}");
            // The strut lands on the arm.
            assert!(l.log.strut.1.x <= l.tip_x && l.log.strut.1.y == top, "{frame:?}");
            let (at, align) = l.label(Place::Log, false);
            assert_eq!(align, Align::Start);
            assert!(at.x > log.max.x && at.y > l.gate.y + GATE_RADIUS + GATE_POD, "{frame:?}");
        }
    }

    /// The EVOLUTION lab (GDR-0092) stands above the mind on the base's outer face, clear of every other place, of
    /// the buildings' bodies, the station, the moorings and the bridges; its strut lands on the base and its conduit
    /// on the mind's top face. Its label sits over it inside the view, clear of the station, the moorings and the
    /// other labels at the three sizes the UI is checked at (its two lines at most 160 × 30 logical px, the
    /// research's and the mind's 130 px wide).
    #[test]
    fn the_evolution_lab_stands_apart_above_the_mind() {
        for frame in [(24, 24), (40, 40), (64, 64), (30, 18), (12, 12)] {
            let l = Layout::new(frame);
            let evo = l.hit(Place::Evolution);
            for p in Place::ALL.into_iter().filter(|p| *p != Place::Evolution) {
                assert!(evo.inflate(0.5).intersect(l.hit(p)).is_empty(), "{frame:?}: the lab touches {p:?}");
            }
            for b in [l.station(), l.research.rect(), l.mind.rect(), l.log_rect()] {
                assert!(evo.inflate(0.6).intersect(b).is_empty(), "{frame:?}: the lab touches {b:?}");
            }
            let reach = MOORING_HALF + MOORING_WALL;
            for (k, m) in l.moorings.iter().enumerate() {
                let berth = Rect::from_center_half_size(*m, Vec2::new(reach, MOORING_DEPTH / 2.0 + MOORING_WALL));
                assert!(evo.inflate(1.0).intersect(berth).is_empty(), "{frame:?}: mooring {k}");
            }
            for (bend, end) in l.mind_joints {
                assert!(!evo.inflate(0.5).contains(bend) && !evo.inflate(0.5).contains(end), "{frame:?}: the bridge");
            }
            assert_eq!(l.evolution.strut.1.x, l.base_x - BASE_STRIP, "{frame:?}: the strut lands on the base");
            let (a, b) = l.evolution_conduit;
            assert!(a.y == evo.min.y && b.y == l.mind.rect().max.y && a.x > l.mind.rect().min.x + 4.5 && a.x < l.mind.rect().max.x - chamfer(l.mind.rect()), "{frame:?}: the conduit");
            if frame.1 < 24 {
                continue;
            }
            for (w, h) in [(1280.0, 720.0), (1600.0, 900.0), (2560.0, 1440.0)] {
                let viewport = Vec2::new(w, h);
                let k = (w / 1280.0f32).min(h / 720.0);
                let (_, zoom) = fit(l.overview, viewport - Vec2::splat(36.0));
                // A label's box in metres: hung from its anchor, as wide and high as its pixels at this zoom.
                let label = |p: Place, width: f32| {
                    let (at, align) = l.label(p, false);
                    let (wm, hm) = (width * k / zoom, 30.0 * k / zoom);
                    let x = match align {
                        Align::Start => (at.x, at.x + wm),
                        Align::Middle => (at.x - wm / 2.0, at.x + wm / 2.0),
                        Align::End => (at.x - wm, at.x),
                    };
                    Rect::new(x.0, at.y - hm, x.1, at.y)
                };
                let me = label(Place::Evolution, 160.0);
                assert!(l.overview.contains(me.min) && l.overview.contains(me.max), "{frame:?} {w}x{h}: out of view");
                assert!(me.min.y > evo.max.y, "{frame:?} {w}x{h}: over the lab");
                assert!(me.intersect(l.station()).is_empty(), "{frame:?} {w}x{h}: on the station");
                for (k, m) in l.moorings.iter().enumerate() {
                    let berth = Rect::from_center_half_size(*m, Vec2::new(reach, MOORING_DEPTH / 2.0 + MOORING_WALL));
                    assert!(me.intersect(berth).is_empty(), "{frame:?} {w}x{h}: on mooring {k}");
                }
                for p in [Place::Research, Place::Behavior, Place::Fleet] {
                    assert!(me.intersect(label(p, 130.0)).is_empty(), "{frame:?} {w}x{h}: on {p:?}'s label");
                }
            }
        }
    }

    /// The shuttles' stops are in view and clear of every hull: the U, the buildings, the moorings and the bridges.
    #[test]
    fn shuttle_stops_are_in_view_and_clear_of_the_hulls() {
        for frame in [(24, 24), (40, 40), (64, 64), (30, 18), (12, 12)] {
            let l = Layout::new(frame);
            let mut hulls = vec![l.station(), l.research.rect(), l.mind.rect(), l.range.rect(), l.workshop.rect(), l.log_rect(), l.evolution.rect()];
            for (k, m) in l.moorings.iter().enumerate() {
                // From the base behind the bridge side to the tips at the mouth.
                let open = if k % 2 == 0 { 1.0 } else { -1.0 };
                let (back, tips) = (*m - Vec2::Y * open * (MOORING_DEPTH / 2.0 + MOORING_WALL), *m + Vec2::Y * open * MOORING_DEPTH / 2.0);
                hulls.push(Rect::from_corners(back - Vec2::X * (MOORING_HALF + MOORING_WALL), tips + Vec2::X * (MOORING_HALF + MOORING_WALL)));
            }
            for y in l.bridges {
                hulls.push(Rect::new(l.bridge_x.0, y - BRIDGE / 2.0, l.bridge_x.1, y + BRIDGE / 2.0));
            }
            for (at, _) in l.shuttle_stops() {
                assert!(l.overview.inflate(-0.6).contains(at), "{at} out of view at {frame:?}");
                for h in &hulls {
                    assert!(!h.inflate(0.5).contains(at), "{at} on {h:?} at {frame:?}");
                }
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
                    let (at, align) = l.label(*p, false);
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
        assert!((z - fit(l.overview, Vec2::new(1280.0, 720.0)).1).abs() < 1e-3);
    }

    /// The overview fits any window: the whole station shows, centred.
    #[test]
    fn the_overview_fits_the_window() {
        let l = Layout::new((24, 24));
        let (c, z) = fit(l.overview, Vec2::new(1280.0, 720.0));
        assert_eq!(c, l.overview.center());
        assert!((z - (1280.0 / l.overview.width()).min(720.0 / l.overview.height())).abs() < 1e-3);
        let (_, tall) = fit(l.overview, Vec2::new(800.0, 900.0));
        assert!((tall - 800.0 / l.overview.width()).abs() < 1e-3, "a narrow window fits the width");
    }
}
