//! The whole galaxy behind the voyage map (seen when the view zooms out, `view`): a stylised spiral disc in its
//! bands, rim → outer → middle → inner → core (GDR-0026; their edges from `bands.ron`), around the galaxy's centre
//! (`layout::Geometry`). Two atlas meshes in galaxy px (the default view's stage px), moved and scaled with the view:
//!
//! * **The galaxy** (always shown): haze along four spiral arms and a warm core, crisp star dust (and a few faint
//!   constellation lines) only in home's band, uncharted bands veiled, the sealed ones darker and fogged (soft
//!   smudges, no stars, a faint coral tint), an open band past home's faintly tinted, a sparse halo outside the rim,
//!   and the band edges: the galaxy's dotted, a gate edge into an open band solid, the one into the first sealed band
//!   dashed in coral, the deeper ones faint dots (GDR-0097, `map.bands.*` in the tokens). Rebuilt when a band opens. Its
//!   glows are few and large rather than many and faint: what a frame blends grows with their area, and the map
//!   must stay light on a 2560 px window (GDR-0097: [`overdraw`]).
//! * **The far layer** (fades in as the view zooms out, `view::far_alpha`): home's band lit, each voyage found and
//!   each known cluster a soft beacon (a new signal with a ring around it; unknown voyages and uncharted clusters
//!   show nothing, and no route joins them: the rim is a place to explore, not a path, GDR-0078), the shipyard
//!   a large diamond, and locks along the gate edges: open (teal) on an open gate edge, shut (coral) on the edge into
//!   the first sealed band. Rebuilt when what the career knows changes.

use bevy::prelude::*;
use kit::atlas::Region;
use kit::rng::Rng;
use kit::style::{BandStyle, Style, UiPalette, mix, vcol};

use super::draw::{Look, Pen};
use super::layout::{Geometry, VoyageState};

/// Spiral arms, and how tightly they wind (radians of turn per e-fold of radius).
const ARMS: usize = 4;
const WIND: f32 = 3.0;
/// Haze blobs per arm, fog smudges over the uncharted bands, stars in home's band and in the halo.
const HAZE: usize = 40;
const FOG: usize = 160;
const STARS: usize = 1000;
const HALO: usize = 180;
/// The halo's reach past the rim (share of the galaxy's radius).
const HALO_REACH: f32 = 0.4;
/// Where the far layer's locks sit along a gate edge (degrees from the centre's right, y down): clear of the band
/// names straight above and below the centre.
const LOCKS: [f32; 6] = [0.0, 50.0, 130.0, 180.0, 230.0, 310.0];

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

/// The galaxy's mesh (see the module doc), and how many times over it covers its disc ([`overdraw`]).
pub fn build(geo: &Geometry, style: &Style, open: &[bool], white: &Region, soft: &Region) -> (Mesh, f32) {
    let mut pen = Pen::new(white, soft);
    paint(geo, &style.ui, &style.bands, open, style.background.outer(), &mut pen);
    let over = pen.area() / disc(geo);
    (pen.into_mesh(), over)
}

/// The disc's area (galaxy px²).
fn disc(geo: &Geometry) -> f32 {
    std::f32::consts::PI * geo.band.outer.max(1.0).powi(2)
}

/// How many times over the galaxy's mesh covers its disc on average: the area of its glows, discs and dots against
/// the disc's (the pixels a frame blends grow with it; GDR-0097: from about 12 to under 5, on a disc of a fifth the
/// area, about a tenth of the pixels a frame blends).
#[cfg(test)]
fn overdraw(geo: &Geometry, open: &[bool]) -> f32 {
    let region = Region { uv: Rect::default(), cells: Vec2::ONE, origin: Vec2::ZERO };
    let mut pen = Pen::new(&region, &region);
    let w = Color::WHITE;
    let ui = UiPalette { teal: w, gold: w, coral: w, violet: w, label: w, dim: w, number: w };
    let bands = BandStyle {
        open: w,
        open_fill: 1.0,
        open_edge: 1.0,
        open_edge_width: 1.0,
        open_veil: 1.0,
        sealed: w,
        sealed_tint: 1.0,
        sealed_edge: 1.0,
        sealed_edge_width: 1.0,
        sealed_dash: 12.0,
        sealed_gap: 9.0,
        sealed_veil: 1.0,
        deep_edge: 1.0,
        lock: 1.0,
        lock_size: 26.0,
    };
    paint(geo, &ui, &bands, open, w, &mut pen);
    pen.area() / disc(geo)
}

/// `open`: each band's state, from the rim inwards (`known::Known::bands`; home's band and those outside it open
/// without one).
fn paint(geo: &Geometry, ui: &UiPalette, bands: &BandStyle, open: &[bool], veil: Color, pen: &mut Pen) {
    let c = geo.band.centre;
    let radius = geo.rings.first().map_or(geo.band.outer, |r| r.outer);
    let rings = &geo.rings;
    let core = rings.last().map_or(radius * 0.1, |r| r.outer);
    let mut rng = Rng::new(0x06A1_AC51);

    // Haze along the arms (overlapping glows, so they read as a smooth band), cooler outwards, warmer near the core.
    for arm in 0..ARMS {
        for s in 0..HAZE {
            let t = (s as f32 + rng.next_f32()) / HAZE as f32;
            let r = radius * (0.06 + 0.94 * t);
            let at = on_arm(c, radius, arm, r, rng.range(-0.16, 0.16));
            let size = radius * (0.12 + 0.16 * (1.0 - t)) * rng.range(0.8, 1.25);
            let tone = mix(mix(ui.label, ui.teal, 0.4), ui.gold, (1.0 - t * 2.0).max(0.0) * 0.5);
            pen.glow(at, size, vcol(tone, 0.09 * (1.0 - t * 0.5)));
        }
    }

    // The bands inside home's: a veil of the background dims them, darker over the sealed ones (inside the last
    // open band), where the core glows through it faintly and fog (soft smudges, more of them on the arms) drifts
    // under a faint coral tint; an open band past home's carries a faint tint of its own (GDR-0097).
    let home = rings.iter().position(|r| r.charted).unwrap_or(0);
    let gate = rings.get(home).map_or(radius, |r| r.inner);
    let is_open = |k: usize| open.get(k).copied().unwrap_or(k <= home);
    let last = (home..rings.len()).take_while(|&k| is_open(k)).last().unwrap_or(home);
    let seal = rings.get(last).map_or(gate, |r| r.inner);
    pen.disc_n(c, gate, 160, vcol(veil, bands.open_veil));
    for k in [1.0, 0.95] {
        pen.disc_n(c, seal * k, 160, vcol(veil, bands.sealed_veil / 2.0));
    }
    for ring in rings.iter().take(last + 1).skip(home + 1) {
        pen.ring(c, (ring.outer + ring.inner) / 2.0, (ring.outer - ring.inner) / 2.0, vcol(bands.open, bands.open_fill));
    }
    for (size, a, tone) in [(3.4, 0.16, mix(ui.gold, ui.number, 0.3)), (1.6, 0.3, mix(ui.gold, ui.number, 0.5)), (0.6, 0.6, ui.number)] {
        pen.glow(c, core * size, vcol(tone, a));
    }
    for k in 0..FOG {
        let at = if k % 3 == 0 {
            in_annulus(&mut rng, c, core, seal)
        } else {
            let r = rng.range(core, seal);
            on_arm(c, radius, k % ARMS, r, rng.range(-0.3, 0.3))
        };
        pen.glow(at, radius * rng.range(0.05, 0.13), vcol(mix(ui.label, ui.coral, 0.15), rng.range(0.05, 0.11)));
    }
    pen.disc_n(c, seal, 160, vcol(bands.sealed, bands.sealed_tint));

    // Star dust in home's band and a sparse halo past the rim; a few faint constellation lines.
    let charted = rings.get(home).map_or((gate, radius), |r| (r.inner, r.outer));
    let mut stars: Vec<(Vec2, f32, f32)> = (0..STARS).map(|_| (in_annulus(&mut rng, c, charted.0, charted.1), rng.range(0.6, 1.7), rng.range(0.12, 0.45))).collect();
    stars.extend((0..HALO).map(|_| (in_annulus(&mut rng, c, radius, radius * (1.0 + HALO_REACH)), rng.range(0.5, 1.2), rng.range(0.08, 0.25))));
    for (k, (a, _, _)) in stars.iter().enumerate().take(STARS).filter(|(k, _)| k % 9 == 0) {
        let near = stars.iter().enumerate().filter(|(j, _)| *j != k).map(|(_, s)| s.0).filter(|s| s.distance(*a) > 30.0).min_by(|x, y| x.distance(*a).total_cmp(&y.distance(*a)));
        if let Some(z) = near.filter(|z| z.distance(*a) < 110.0) {
            pen.dotted(*a, z, 9.0, 0.7, vcol(ui.teal, 0.1));
        }
    }
    for (at, r, a) in &stars {
        pen.disc(*at, *r, vcol(mix(ui.label, ui.teal, 0.3), *a));
    }

    // The band edges: the galaxy's edge; a gate edge into an open band solid, the one into the first sealed band
    // dashed (its locks are the far layer's), the deeper ones faint dots.
    for (k, ring) in rings.iter().enumerate() {
        if k == 0 {
            pen.dotted_circle(c, ring.outer, 7.0, 0.9, vcol(ui.label, 0.3));
        }
        if ring.inner <= 0.0 {
            continue;
        }
        if is_open(k + 1) {
            pen.ring(c, ring.inner, bands.open_edge_width / 2.0, vcol(bands.open, bands.open_edge));
        } else if k == last {
            pen.dashed_circle(c, ring.inner, bands.sealed_dash, bands.sealed_gap, bands.sealed_edge_width / 2.0, vcol(bands.sealed, bands.sealed_edge));
        } else {
            pen.dotted_circle(c, ring.inner, 7.0, 0.8, vcol(ui.label, bands.deep_edge));
        }
    }
}

/// The far layer's mesh (see the module doc): galaxy px, sized to read when the whole galaxy is in view.
pub fn build_far(geo: &Geometry, look: &Look, style: &Style, white: &Region, soft: &Region) -> Mesh {
    let ui = &style.ui;
    let (states, guardian, known, glow) = (&look.states, &look.guardian, &look.known, look.glow);
    let mut pen = Pen::new(white, soft);
    let b = &geo.band;
    // Home's band, lit.
    pen.ring(b.centre, (b.outer + b.inner) / 2.0, (b.outer - b.inner) / 2.0, vcol(ui.teal, 0.07));
    // The voyages found so far and the known clusters: beacons, no routes.
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
    for m in &known.clusters {
        let Some(&at) = geo.clusters.get(m.cluster as usize) else { continue };
        let tone = if m.charted { ui.teal } else { ui.gold };
        if glow {
            pen.glow(at, 100.0, vcol(tone, 0.3));
        }
        pen.disc(at, 8.0, vcol(tone, 0.9));
    }
    // Locks along the gate edges: open ones on each edge into an open band, shut ones on the edge into the first
    // sealed band.
    let bands = &style.bands;
    let size = bands.lock_size;
    let mut past_open = true;
    for (k, ring) in geo.rings.iter().enumerate().filter(|(_, r)| r.inner > 0.0) {
        let next = known.bands.get(k + 1).is_some_and(|m| m.open);
        if !next && !past_open {
            break;
        }
        past_open = next;
        let (tone, a) = if next { (bands.open, bands.lock * 0.6) } else { (bands.sealed, bands.lock) };
        for deg in LOCKS {
            let at = b.centre + Vec2::from_angle(deg.to_radians()) * ring.inner;
            pen.disc(at, size * 0.62, vcol(style.background.outer(), 0.9));
            lock(&mut pen, at, size, next, vcol(tone, a));
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

/// A padlock `size` across centred on `at` (galaxy px, y down): its body, and its shackle, lifted and swung open
/// when `open`.
fn lock(pen: &mut Pen, at: Vec2, size: f32, open: bool, col: [f32; 4]) {
    let q = |x: f32, y: f32| at + Vec2::new(x, y) * size;
    pen.rect(q(-0.3, -0.02), q(0.3, 0.4), col);
    let lift = if open { 0.14 } else { 0.0 };
    let end = if open { 0.15 } else { 0.0 };
    let arc: Vec<Vec2> = (0..=10)
        .map(|i| {
            let t = std::f32::consts::PI * (1.0 - i as f32 / 10.0 * (1.0 - end));
            q(0.19 * t.cos(), -0.02 - lift - 0.22 * t.sin())
        })
        .collect();
    let mut shackle = vec![q(-0.19, -0.02)];
    shackle.extend(arc);
    if !open {
        shackle.push(q(0.19, -0.02));
    }
    pen.polyline(&shackle, size * 0.06, col);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bands ring inwards from the galaxy's edge to the core, each inside the one before, home's band the
    /// charted one; the galaxy's mesh covers its disc only a few times over (about 12 before GDR-0097, on a disc 4.6 times
    /// the area).
    #[test]
    fn the_bands_ring_inwards_and_the_galaxy_is_light_to_draw() {
        let content = data::base_content();
        let career = world::career::load(
            &mut world::storage::Storage::new(world::storage::MemoryBackend::default()),
            &content,
            world::career::Switches::from_params(&kit::params::Params::parse("")).0,
            4207,
            1,
        );
        let g = Geometry::new(&content, career.career().exploration());
        let keys: Vec<&str> = g.rings.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, ["rim", "outer", "middle", "inner", "core"]);
        assert_eq!((g.rings[0].outer, g.rings[0].inner, g.rings[0].charted), (g.band.outer, g.band.inner, true));
        assert!(g.rings[1..].iter().all(|r| !r.charted));
        for w in g.rings.windows(2) {
            assert_eq!(w[0].inner, w[1].outer);
            assert!(w[1].outer > w[1].inner, "{w:?}");
        }
        assert_eq!(g.rings[4].inner, 0.0);
        let d = g.home.distance(g.band.centre);
        assert!(d > g.rings[0].inner && d < g.rings[0].outer, "home lies in the rim: {d}");
        for open in [&[true, false, false, false, false][..], &[true, true, false, false, false]] {
            let over = overdraw(&g, open);
            assert!(over < 6.0, "the galaxy covers its disc {over:.1} times over");
        }
    }
}
