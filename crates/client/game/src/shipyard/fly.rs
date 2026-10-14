//! The camera's flight between two views of the yard (GDR-0071: a click eases the camera onto a place, then that
//! screen appears; coming back reverses it).
//!
//! A flight zooms around its anchor, like the rig's zoom around the cursor: the one world point that sits at the
//! same place on screen in both views stays put while the zoom changes geometrically, so flying into a building
//! looks like zooming into it, not like a pan and a zoom fighting each other. When the views have (almost) the
//! same zoom there is no such point and the camera pans. The progress follows a smootherstep of the time since
//! the flight began, so the path is the same at any frame rate. A flight to the view it starts from has arrived
//! once its wait is over.

use bevy::math::Vec2;
use world::camera::smootherstep;

/// How long a flight takes (seconds).
pub const SECONDS: f32 = 0.6;

/// What the camera shows: the world point at the window's centre and logical px per metre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub centre: Vec2,
    pub zoom: f32,
}

/// A flight in progress.
#[derive(Clone, Copy, Debug)]
pub struct Fly {
    from: View,
    to: View,
    /// The world point that keeps its place on screen, and that place (logical px from the centre, y up).
    anchor: Option<(Vec2, Vec2)>,
    seconds: f32,
}

impl Fly {
    pub fn new(from: View, to: View) -> Self {
        // `w = from.centre + d / from.zoom = to.centre + d / to.zoom` for the anchor's screen offset `d`.
        let k = 1.0 / from.zoom - 1.0 / to.zoom;
        let anchor = (k.abs() > 1e-3 / from.zoom.max(to.zoom)).then(|| {
            let d = (to.centre - from.centre) / k;
            (from.centre + d / from.zoom, d)
        });
        Self { from, to, anchor, seconds: 0.0 }
    }

    /// Waits `seconds` on `from` before it moves (a screen closing over the place first).
    pub fn after(self, seconds: f32) -> Self {
        Self { seconds: -seconds.max(0.0), ..self }
    }

    /// The view at progress `s` (0 to 1, already eased).
    pub fn at(&self, s: f32) -> View {
        let zoom = self.from.zoom * (self.to.zoom / self.from.zoom).powf(s);
        let centre = match self.anchor {
            Some((w, d)) => w - d / zoom,
            None => self.from.centre.lerp(self.to.centre, s),
        };
        View { centre, zoom }
    }

    /// Advances by `dt` seconds: the view now, and whether the flight has arrived (it ends exactly on `to`).
    pub fn step(&mut self, dt: f32) -> (View, bool) {
        self.seconds += dt.max(0.0);
        let t = if self.from == self.to && self.seconds >= 0.0 { 1.0 } else { self.seconds / SECONDS };
        // Within a hair of the end it has arrived (time sums of frame steps land a rounding error short).
        if t >= 1.0 - 1e-4 { (self.to, true) } else { (self.at(smootherstep(t)), false) }
    }

    /// How far along it is (0 to 1, eased): for things that move with the camera.
    pub fn progress(&self) -> f32 {
        smootherstep(self.seconds.max(0.0) / SECONDS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overview() -> View {
        View { centre: Vec2::new(16.5, -12.0), zoom: 16.0 }
    }

    /// Flies at `fps` and samples the view every 0.1 s.
    fn path(from: View, to: View, fps: u32) -> Vec<View> {
        let mut fly = Fly::new(from, to);
        let mut out = Vec::new();
        for frame in 1..=(fps * 6 / 10) {
            let (v, done) = fly.step(1.0 / fps as f32);
            if frame % (fps / 10) == 0 {
                out.push(v);
            }
            assert_eq!(done, frame == fps * 6 / 10, "arrives at 0.6 s, frame {frame} at {fps} fps");
        }
        out
    }

    /// The same path at 30 and 150 fps, ending exactly on the target.
    #[test]
    fn a_flight_is_framerate_independent_and_arrives() {
        let to = View { centre: Vec2::new(8.0, -2.0), zoom: 45.0 };
        let (a, b) = (path(overview(), to, 30), path(overview(), to, 150));
        assert_eq!(a.len(), 6);
        for (p, q) in a.iter().zip(&b) {
            assert!(p.centre.distance(q.centre) < 1e-3 && (p.zoom - q.zoom).abs() < 1e-3, "{p:?} vs {q:?}");
        }
        assert_eq!(*a.last().unwrap(), to);
    }

    /// Zooming in on a place keeps one world point still on screen: the anchor, inside the target.
    #[test]
    fn a_flight_zooms_around_its_anchor() {
        let to = View { centre: Vec2::new(8.0, -2.0), zoom: 45.0 };
        let fly = Fly::new(overview(), to);
        let (w, d) = fly.anchor.expect("different zooms have an anchor");
        for s in [0.0, 0.25, 0.5, 0.9, 1.0] {
            let v = fly.at(s);
            assert!((v.centre + d / v.zoom).distance(w) < 1e-3, "{s}");
        }
        // The anchor is on screen in both views (inside the 1280 x 720 window), so the place is zoomed into.
        assert!(d.x.abs() < 640.0 && d.y.abs() < 360.0, "{d}");
        // Zoom grows monotonically, smoothly: never more than 1.25x between 60 Hz frames.
        let mut fly = Fly::new(overview(), to);
        let mut last = overview().zoom;
        for _ in 0..36 {
            let (v, _) = fly.step(1.0 / 60.0);
            assert!(v.zoom >= last && v.zoom / last < 1.25);
            last = v.zoom;
        }
    }

    /// A flight that waits stays on its start, then flies the same path, arriving later by the wait.
    #[test]
    fn a_flight_can_wait_first() {
        let to = View { centre: Vec2::new(8.0, -2.0), zoom: 45.0 };
        let mut fly = Fly::new(overview(), to).after(0.3);
        for _ in 0..17 {
            let (v, done) = fly.step(1.0 / 60.0);
            assert!(v == overview() && !done && fly.progress() == 0.0);
        }
        let (mut arrived, mut frames) = (false, 17);
        while !arrived {
            arrived = fly.step(1.0 / 60.0).1;
            frames += 1;
        }
        // 0.3 s waiting, then the 0.6 s flight.
        assert!((53..=55).contains(&frames), "{frames}");
    }

    /// Without a change of zoom the camera pans; going back reverses the path.
    #[test]
    fn same_zoom_pans_and_back_reverses() {
        let to = View { centre: Vec2::new(40.0, -12.0), zoom: 16.0 };
        let fly = Fly::new(overview(), to);
        assert!(fly.anchor.is_none());
        assert_eq!(fly.at(0.5).centre, Vec2::new(28.25, -12.0));
        let a = View { centre: Vec2::new(8.0, -2.0), zoom: 45.0 };
        let (there, back) = (Fly::new(overview(), a), Fly::new(a, overview()));
        for s in [0.2, 0.5, 0.7] {
            let (p, q) = (there.at(s), back.at(1.0 - s));
            assert!(p.centre.distance(q.centre) < 1e-3 && (p.zoom - q.zoom).abs() < 1e-3);
        }
    }
}
