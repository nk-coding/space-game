//! The whole galaxy behind the voyage map (seen when the view zooms out, `view`): a stylised spiral disc in its
//! bands, rim → outer → middle → inner → core (GDR-0026), around the centre the rim band's arcs bend about
//! (`layout::CENTRE`). Two atlas meshes in galaxy px (the default view's stage px), moved and scaled with the view:
//!
//! * **The galaxy** (always shown): haze along four spiral arms, a warm core, crisp star dust (and a few faint
//!   constellation lines) only in the charted band, uncharted bands veiled and fogged (soft smudges, no stars, the
//!   faint coral tint inside the gate), a sparse halo outside the rim, and every band edge as a dotted ring. Built
//!   once.
//! * **The far layer** (fades in as the view zooms out, `view::far_alpha`): the charted band lit, each voyage found
//!   so far a soft beacon (a new signal with a ring around it; unknown voyages show nothing, and no route joins
//!   them: the rim is a place to explore, not a path, GDR-0078), and the shipyard a large diamond. Rebuilt when the
//!   voyages' states change.
//!
//! Where the bands past the next one end is a placeholder: content has no bands yet (`layout::BANDS` are keys).

use bevy::prelude::*;

use super::draw::Pen;
use super::layout::{BANDS, Band, Geometry, VoyageState};
use crate::atlas::Region;
use crate::backdrop::Rng;
use crate::style::{Style, mix, vcol};

/// Spiral arms, and how tightly they wind (radians of turn per e-fold of radius).
const ARMS: usize = 4;
const WIND: f32 = 3.0;
/// Haze blobs per arm, fog smudges over the uncharted bands, stars in the charted band and in the halo.
const HAZE: usize = 150;
const FOG: usize = 600;
const STARS: usize = 1000;
const HALO: usize = 180;
/// The halo's reach past the rim (share of the galaxy's radius).
const HALO_REACH: f32 = 0.4;

/// One band as a ring around the centre (galaxy px; the core's inner edge is 0).
#[derive(Clone, Debug, PartialEq)]
pub struct Ring {
    pub key: &'static str,
    pub outer: f32,
    pub inner: f32,
    /// The band the voyages lie in (the others are uncharted).
    pub charted: bool,
}

/// The galaxy's bands from the map's band inwards: the map's band between its edges, the next one to its next
/// edge (as the map draws them), and the rest sharing what is left equally down to the core (PLACEHOLDER
/// depths, until bands are content).
pub fn rings(band: &Band) -> Vec<Ring> {
    let first = BANDS.iter().position(|b| *b == band.key).unwrap_or(0);
    let keys = &BANDS[first..];
    let mut edges = vec![band.outer, band.inner];
    let rest = keys.len().saturating_sub(2);
    if keys.len() > 1 {
        if rest == 0 {
            edges.push(0.0);
        } else {
            edges.push(band.next);
            edges.extend((1..=rest).map(|j| band.next * (rest - j) as f32 / rest as f32));
        }
    } else {
        edges[1] = 0.0;
    }
    keys.iter().zip(edges.windows(2)).enumerate().map(|(k, (key, e))| Ring { key, outer: e[0], inner: e[1], charted: k == 0 }).collect()
}

/// A point on arm `arm` at radius `r` (galaxy px), turned by `jitter` radians.
fn on_arm(centre: Vec2, radius: f32, arm: usize, r: f32, jitter: f32) -> Vec2 {
    let a = arm as f32 * std::f32::consts::TAU / ARMS as f32 + WIND * (r / (0.08 * radius)).max(1.0).ln() + jitter;
    centre + Vec2::from_angle(a) * r
}

/// A uniformly spread point of the annulus `inner..outer`.
fn in_annulus(rng: &mut Rng, centre: Vec2, inner: f32, outer: f32) -> Vec2 {
    let r = (inner * inner + (outer * outer - inner * inner) * rng.next_f32()).sqrt();
    centre + Vec2::from_angle(rng.range(0.0, std::f32::consts::TAU)) * r
}

/// The galaxy's mesh (see the module doc).
pub fn build(geo: &Geometry, style: &Style, white: &Region, soft: &Region) -> Mesh {
    let ui = &style.ui;
    let mut pen = Pen::new(white, soft);
    let b = &geo.band;
    let (c, radius) = (b.centre, b.outer);
    let rings = rings(b);
    let core = rings.last().map_or(radius * 0.1, |r| r.outer);
    let mut rng = Rng::new(0x06A1_AC51);

    // Haze along the arms (many faint overlapping glows, so they read as a smooth band), cooler outwards, warmer
    // near the core.
    for arm in 0..ARMS {
        for s in 0..HAZE {
            let t = (s as f32 + rng.next_f32()) / HAZE as f32;
            let r = radius * (0.06 + 0.94 * t);
            let at = on_arm(c, radius, arm, r, rng.range(-0.16, 0.16));
            let size = radius * (0.12 + 0.16 * (1.0 - t)) * rng.range(0.8, 1.25);
            let tone = mix(mix(ui.label, ui.teal, 0.4), ui.gold, (1.0 - t * 2.0).max(0.0) * 0.5);
            pen.glow(at, size, vcol(tone, 0.04 * (1.0 - t * 0.5)));
        }
    }

    // Uncharted bands: a veil of the background dims them, the core glows through it faintly, fog (soft smudges,
    // more of them on the arms) drifts over it under a faint coral tint inside the gate.
    let gate = rings.first().map_or(radius, |r| r.inner);
    for k in [1.0, 0.97, 0.93] {
        pen.disc_n(c, gate * k, 160, vcol(style.background.outer(), 0.18));
    }
    for (size, a, tone) in [(3.4, 0.16, mix(ui.gold, ui.number, 0.3)), (1.6, 0.3, mix(ui.gold, ui.number, 0.5)), (0.6, 0.6, ui.number)] {
        pen.glow(c, core * size, vcol(tone, a));
    }
    for k in 0..FOG {
        let at = if k % 3 == 0 {
            in_annulus(&mut rng, c, core, gate)
        } else {
            let r = rng.range(core, gate);
            on_arm(c, radius, k % ARMS, r, rng.range(-0.3, 0.3))
        };
        pen.glow(at, rng.range(50.0, 140.0), vcol(mix(ui.label, ui.coral, 0.15), rng.range(0.02, 0.05)));
    }
    pen.disc_n(c, gate, 160, vcol(ui.coral, 0.025));

    // Star dust in the charted band and a sparse halo past the rim; a few faint constellation lines.
    let charted = rings.first().map_or((gate, radius), |r| (r.inner, r.outer));
    let mut stars: Vec<(Vec2, f32, f32)> = (0..STARS).map(|_| (in_annulus(&mut rng, c, charted.0, charted.1), rng.range(0.6, 1.7), rng.range(0.12, 0.45))).collect();
    stars.extend((0..HALO).map(|_| (in_annulus(&mut rng, c, radius, radius * (1.0 + HALO_REACH)), rng.range(0.5, 1.2), rng.range(0.08, 0.25))));
    for (k, (a, _, _)) in stars.iter().enumerate().take(STARS).filter(|(k, _)| k % 9 == 0) {
        let near = stars.iter().enumerate().filter(|(j, _)| *j != k).map(|(_, s)| s.0).filter(|s| s.distance(*a) > 40.0).min_by(|x, y| x.distance(*a).total_cmp(&y.distance(*a)));
        if let Some(z) = near.filter(|z| z.distance(*a) < 170.0) {
            pen.dotted(*a, z, 9.0, 0.7, vcol(ui.teal, 0.1));
        }
    }
    for (at, r, a) in &stars {
        pen.disc(*at, *r, vcol(mix(ui.label, ui.teal, 0.3), *a));
    }

    // The band edges: the rim's outer edge, the gate (coral), the next band's edge fainter, the deeper ones faintest.
    for (k, ring) in rings.iter().enumerate() {
        if k == 0 {
            pen.dotted_circle(c, ring.outer, 7.0, 0.9, vcol(ui.label, 0.3));
        }
        let (dot, col) = match k {
            0 => (1.0, vcol(mix(ui.label, ui.coral, 0.35), 0.45)),
            1 => (0.8, vcol(ui.label, 0.14)),
            _ => (0.8, vcol(ui.label, 0.1)),
        };
        pen.dotted_circle(c, ring.inner, 7.0, dot, col);
    }
    pen.into_mesh()
}

/// The far layer's mesh (see the module doc): galaxy px, sized to read when the whole galaxy is in view.
pub fn build_far(geo: &Geometry, states: &[VoyageState], guardian: &[bool], glow: bool, style: &Style, white: &Region, soft: &Region) -> Mesh {
    let ui = &style.ui;
    let mut pen = Pen::new(white, soft);
    let b = &geo.band;
    // The charted band, lit.
    pen.ring(b.centre, (b.outer + b.inner) / 2.0, (b.outer - b.inner) / 2.0, vcol(ui.teal, 0.07));
    // The voyages found so far: beacons, no routes.
    for (k, at) in geo.nodes.iter().copied().enumerate() {
        let state = states.get(k).copied().unwrap_or(VoyageState::Unknown);
        if !state.known() {
            continue;
        }
        let tone = if guardian.get(k).copied().unwrap_or(false) && !matches!(state, VoyageState::Cleared { .. }) { ui.coral } else { ui.teal };
        if glow {
            pen.glow(at, 130.0, vcol(tone, 0.4));
        }
        pen.disc(at, 11.0, vcol(tone, 1.0));
        if state == VoyageState::Signal {
            pen.ring(at, 26.0, 2.2, vcol(tone, 0.7));
        }
    }
    // The shipyard.
    let h = geo.home;
    if glow {
        pen.glow(h, 260.0, vcol(ui.teal, 0.3));
    }
    pen.diamond_outline(h, 42.0, 3.5, vcol(ui.number, 0.95));
    pen.diamond(h, 15.0, vcol(ui.teal, 1.0));
    pen.into_mesh()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// From the rim inwards: rim (charted) between the map's arcs, outer to the next arc, middle, inner and the
    /// core sharing the rest down to the centre; every ring inside the one before.
    #[test]
    fn the_bands_ring_inwards_to_the_core() {
        let content = data::base_content();
        let g = Geometry::new(&content);
        let rings = rings(&g.band);
        assert_eq!(rings.iter().map(|r| r.key).collect::<Vec<_>>(), BANDS);
        assert_eq!((rings[0].outer, rings[0].inner, rings[0].charted), (g.band.outer, g.band.inner, true));
        assert_eq!((rings[1].outer, rings[1].inner), (g.band.inner, g.band.next));
        assert!(rings[1..].iter().all(|r| !r.charted));
        for w in rings.windows(2) {
            assert_eq!(w[0].inner, w[1].outer);
            assert!(w[1].outer > w[1].inner, "{w:?}");
        }
        assert_eq!(rings[4].inner, 0.0);
        // Every voyage and the shipyard lie in the charted band.
        for p in g.nodes.iter().chain([&g.home]) {
            let d = p.distance(g.band.centre);
            assert!(d > rings[0].inner && d < rings[0].outer);
        }
    }
}
