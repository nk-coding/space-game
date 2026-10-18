//! What each planet form rolls (GDR-0105): the port of the design study's kinds, with more on the surface (fine
//! ripples along the bands, rough coastlines, wispy cloud streaks and storm swirls). Colours come from the kind's
//! palette, in the order its form documents (`data::PlanetForm`).

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;
use data::{Atmosphere, PlanetForm, PlanetKind};

use super::paint::{Air, Bands, Cap, Clouds, Crater, Look, Plume, Poles, Ring, Storm, Streak, Swirl, Walk, Wave};
use super::roll::{Noise, Rgb, Stream, WHITE, Wob, hex, jit, mix};

/// The palette's colours.
struct Pal(Vec<Rgb>);

impl Pal {
    fn at(&self, i: usize) -> Rgb {
        self.0[i.min(self.0.len() - 1)]
    }
}

fn air_level(a: Atmosphere) -> f32 {
    match a {
        Atmosphere::None => 0.0,
        Atmosphere::Thin => 0.3,
        Atmosphere::Moderate => 0.6,
        Atmosphere::Thick => 0.95,
    }
}

fn empty(r: &mut Stream, base: Rgb, air: Air) -> Look {
    Look {
        base,
        bands: None,
        streaks: Vec::new(),
        storms: Vec::new(),
        caps: Vec::new(),
        poles: None,
        clouds: None,
        craters: Vec::new(),
        cracks: Vec::new(),
        rifts: Vec::new(),
        rift_cols: (Vec3::ZERO, Vec3::ZERO),
        vents: Vec::new(),
        biolum: None,
        night_glow: None,
        plumes: Vec::new(),
        shape: None,
        mottle: 0.06,
        air,
        ring: None,
        noise: Noise::new(r, 4, 3.0, 0.55),
    }
}

/// A body of kind `k` (`tilt`: the surface's tilt, for its rings).
pub(super) fn look(r: &mut Stream, k: &PlanetKind, tilt: f32) -> Look {
    let pal = r.pick(&k.palettes);
    let air_col = pal.air.as_deref().map_or(WHITE, hex);
    let pal = Pal(pal.colors.iter().map(|c| hex(c)).collect());
    let air = Air { lv: air_level(k.atmosphere), col: air_col, haze: 0.0 };
    let mut l = empty(r, pal.at(0), air);
    match k.form {
        PlanetForm::Jovian => {
            bands(r, &mut l, &pal.0[2..], (12, 17), 0.75, 14);
            let (c, size) = (jit(r, pal.at(0), 8.0, 0.08, 0.05), r.range(0.13, 0.2));
            storm(r, &mut l, c, size, true);
            for _ in 0..r.int(2, 4) {
                let (c, size) = (jit(r, pal.at(1), 8.0, 0.08, 0.05), r.range(0.03, 0.06));
                storm(r, &mut l, c, size, false);
            }
        }
        PlanetForm::Ringed => {
            bands(r, &mut l, &pal.0[1..], (10, 15), 0.45, 8);
            if r.chance(0.4) {
                storm(r, &mut l, pal.at(0), 0.05, false);
            }
        }
        PlanetForm::IceGiant => {
            bands(r, &mut l, &pal.0[1..], (5, 8), 0.35, 5);
            let base = l.bands.as_ref().map_or(l.base, |b| b.cols[0]);
            for s in &mut l.streaks {
                s.col = mix(base, WHITE, 0.45);
                s.a *= 0.7;
                s.h *= 1.6;
            }
            if r.chance(0.6) {
                let (c, size) = (jit(r, pal.at(0), 8.0, 0.08, 0.05), r.range(0.08, 0.13));
                storm(r, &mut l, c, size, false);
            }
            l.air.haze = 0.25;
        }
        PlanetForm::HotGiant => {
            bands(r, &mut l, &pal.0[2..], (9, 14), 0.7, 10);
            if r.chance(0.6) {
                storm(r, &mut l, pal.at(0), 0.1, false);
            }
            l.air.haze = 0.15;
            l.night_glow = Some(pal.at(1));
        }
        PlanetForm::Toxic => {
            bands(r, &mut l, &pal.0, (4, 7), 0.4, 4);
            l.air.haze = 0.45;
            let col = mix(pal.at(1), WHITE, 0.3);
            let mut cl = clouds(r, col, [0, 0, 0], 1.0);
            if r.chance(0.6) {
                cl.swirls.push(Swirl { c: r.facing(0.5, 1.2), size: r.range(0.2, 0.32), turns: 1.6, dir: 1.0, a: 0.3 });
            }
            let n = r.int(3, 6);
            cl.streaks.extend(streaks(r, n, 0.35));
            l.clouds = Some(cl);
        }
        PlanetForm::Living => {
            l.base = jit(r, pal.at(0), 8.0, 0.08, 0.05);
            let shallow = jit(r, pal.at(1), 8.0, 0.08, 0.05);
            for _ in 0..r.int(3, 6) {
                let cols = [jit(r, pal.at(2), 8.0, 0.08, 0.05), jit(r, pal.at(3), 8.0, 0.08, 0.05), jit(r, pal.at(4), 8.0, 0.08, 0.05)];
                let rho = r.range(0.25, 0.55);
                land(r, &mut l, rho, shallow, cols);
            }
            l.poles = Some(Poles { n: r.range(0.8, 0.9), s: r.range(0.84, 0.93), col: Vec3::new(0.94, 0.96, 1.0), f: Wob::new(r, 0.25, 6, 1.1) });
            let n = [r.int(10, 16), r.int(3, 6), r.int(1, 2)];
            l.clouds = Some(clouds(r, Vec3::new(0.98, 0.99, 1.0), n, 1.0));
        }
        PlanetForm::Garden => {
            l.base = jit(r, pal.at(0), 8.0, 0.08, 0.05);
            let shallow = mix(l.base, pal.at(3), 0.3);
            let mut dots = Vec::new();
            for _ in 0..r.int(4, 7) {
                let flora = jit(r, pal.at(2), 8.0, 0.08, 0.05);
                let cols = [jit(r, pal.at(1), 8.0, 0.08, 0.05), flora, mix(flora, Vec3::new(0.08, 0.06, 0.12), 0.35)];
                let rho = r.range(0.2, 0.5);
                let c = land(r, &mut l, rho, shallow, cols);
                let (u, v) = super::roll::basis(c);
                for _ in 0..10 {
                    let a = r.range(0.0, TAU);
                    let d = r.next().sqrt() * rho * 0.8;
                    dots.push((c + (u * a.cos() + v * a.sin()) * d.tan()).normalize());
                }
            }
            l.biolum = Some((pal.at(3), dots));
            if r.chance(0.5) {
                l.poles = Some(Poles { n: r.range(0.85, 0.93), s: r.range(0.88, 0.95), col: Vec3::new(0.92, 0.94, 0.98), f: Wob::new(r, 0.25, 6, 1.1) });
            }
            let n = [r.int(8, 13), r.int(2, 5), r.int(0, 1)];
            l.clouds = Some(clouds(r, Vec3::new(0.98, 0.99, 1.0), n, 0.85));
        }
        PlanetForm::Ocean => {
            l.base = jit(r, pal.at(0), 8.0, 0.08, 0.05);
            let shallow = jit(r, pal.at(1), 8.0, 0.08, 0.05);
            for _ in 0..r.int(3, 8) {
                let cols = [jit(r, pal.at(2), 8.0, 0.08, 0.05), jit(r, pal.at(3), 8.0, 0.08, 0.05), jit(r, pal.at(3), 8.0, 0.08, 0.05)];
                let rho = r.range(0.03, 0.08);
                land(r, &mut l, rho, shallow, cols);
            }
            // currents: faint lighter streaks in the sea
            let n = r.int(3, 6);
            l.streaks.extend((0..n).map(|_| Streak { wave: wave(r, (-0.8, 0.8), 0.02), h: r.range(0.01, 0.025), col: mix(l.base, shallow, 0.6), a: r.range(0.15, 0.3) }));
            let n = [r.int(14, 22), r.int(4, 7), r.int(2, 4)];
            l.clouds = Some(clouds(r, Vec3::new(0.98, 0.99, 1.0), n, 1.1));
        }
        PlanetForm::Eyeball => {
            l.base = jit(r, pal.at(0), 6.0, 0.05, 0.03);
            // the rings round the point under the light are placed by `eye` once the light is known
            let f = Wob::new(r, 0.12, 6, 1.1);
            let layers = vec![
                (r.range(0.8, 0.95), jit(r, pal.at(1), 8.0, 0.08, 0.05)),
                (r.range(0.66, 0.76), jit(r, pal.at(2), 8.0, 0.08, 0.05)),
                (r.range(0.52, 0.6), jit(r, pal.at(3), 8.0, 0.08, 0.05)),
                (r.range(0.36, 0.46), jit(r, pal.at(4), 8.0, 0.08, 0.05)),
            ];
            for (rho, col) in layers {
                l.caps.push(Cap { c: Vec3::Z, rho, f: f.clone(), layers: vec![(1.05, col, 0.4), (1.0, col, 0.95)], soft: 0.03, rough: 0.03 });
            }
        }
        PlanetForm::Desert => {
            l.base = jit(r, pal.at(0), 8.0, 0.08, 0.05);
            l.mottle = 0.1;
            for _ in 0..r.int(6, 10) {
                let pick = *r.pick(&pal.0);
                let c = jit(r, pick, 8.0, 0.08, 0.05);
                l.streaks.push(Streak { wave: wave(r, (-0.93, 0.93), 0.03), h: r.range(0.01, 0.03), col: c, a: r.range(0.25, 0.5) });
            }
            let n = r.int(4, 7);
            patches(r, &mut l, n, (0.12, 0.3), pal.at(1), (0.5, 0.8));
            let crack = mix(jit(r, pal.at(1), 8.0, 0.08, 0.05), Vec3::new(0.08, 0.04, 0.02), 0.4);
            let n = r.int(2, 4);
            for w in walks(r, n, (10, 22), 0.05, 0.25) {
                l.cracks.push((w, crack, 0.006));
            }
            let n = r.int(5, 12);
            l.craters = craters(r, n, 0.05);
            l.poles = Some(Poles { n: r.range(0.9, 0.96), s: r.range(0.92, 0.97), col: Vec3::new(0.94, 0.93, 0.89), f: Wob::new(r, 0.3, 6, 1.1) });
            if r.chance(0.35) {
                l.clouds = Some(clouds(r, mix(l.base, WHITE, 0.35), [2, 2, 0], 0.6));
            }
            ripples_on(r, &mut l, 0.04);
        }
        PlanetForm::Ice => {
            l.base = jit(r, pal.at(0), 5.0, 0.04, 0.03);
            l.mottle = 0.07;
            let n = r.int(3, 6);
            patches(r, &mut l, n, (0.15, 0.35), pal.at(1), (0.35, 0.6));
            // lineae: long gently curving streaks tinted reddish brown, close to the ground's colour
            let crack = mix(mix(jit(r, pal.at(2), 8.0, 0.08, 0.05), Vec3::new(0.55, 0.33, 0.24), 0.45), l.base, 0.25);
            let n = r.int(8, 14);
            for w in walks(r, n, (18, 34), 0.045, 0.08) {
                l.cracks.push((w, crack, 0.006));
            }
            let n = r.int(3, 9);
            l.craters = craters(r, n, 0.04);
        }
        PlanetForm::Lava => {
            l.base = jit(r, pal.at(0), 6.0, 0.05, 0.03);
            l.mottle = 0.16;
            for _ in 0..r.int(4, 7) {
                let c = *r.pick(&pal.0[3.min(pal.0.len() - 1)..]);
                patches(r, &mut l, 1, (0.1, 0.28), c, (0.8, 0.8));
            }
            let n = r.int(14, 22);
            l.rifts = walks(r, n, (6, 20), 0.06, 0.5);
            l.rift_cols = (pal.at(1), pal.at(2));
            l.vents = (0..r.int(4, 9)).map(|_| (r.facing(0.9, 1.2), r.range(0.02, 0.05))).collect();
            let n = r.int(5, 12);
            l.craters = craters(r, n, 0.05);
        }
        PlanetForm::Rock | PlanetForm::Rubble => {
            l.base = jit(r, pal.at(0), 5.0, 0.04, 0.04);
            l.mottle = 0.14;
            let n = r.int(3, 6);
            patches(r, &mut l, n, (0.12, 0.32), pal.at(1), (0.75, 0.75));
            let n = if k.form == PlanetForm::Rubble { 14 } else if k.body == data::BodyClass::Moon { r.int(14, 26) } else { r.int(40, 80) };
            l.craters = craters(r, n, if k.form == PlanetForm::Rubble { 0.1 } else { 0.08 });
            // bright ray dust round a few young craters
            for c in l.craters.iter().filter(|c| c.s > 0.05).take(3) {
                l.caps.push(Cap { c: c.at, rho: c.s * 2.5, f: Wob::new(r, 0.3, 5, 1.0), layers: vec![(1.0, pal.at(2), 0.25)], soft: c.s, rough: 0.0 });
            }
            if k.form == PlanetForm::Rubble {
                l.shape = Some(Wob::new(r, 0.3, 5, 0.9));
            }
        }
        PlanetForm::Geyser => {
            l.base = jit(r, pal.at(0), 4.0, 0.03, 0.02);
            let crack = jit(r, pal.at(1), 8.0, 0.08, 0.05);
            for w in walks(r, 4, (8, 16), 0.06, 0.1) {
                l.cracks.push((w, crack, 0.012));
            }
            l.plumes = (0..r.int(2, 4)).map(|_| Plume { a: -PI / 2.0 + r.range(-0.5, 0.5), len: r.range(0.5, 1.1), w: r.range(0.12, 0.22) }).collect();
        }
        PlanetForm::Sulfur => {
            l.base = jit(r, pal.at(0), 6.0, 0.05, 0.04);
            l.mottle = 0.12;
            for _ in 0..r.int(6, 10) {
                let c = *r.pick(&pal.0[1..]);
                patches(r, &mut l, 1, (0.08, 0.2), c, (0.8, 0.8));
            }
            l.vents = (0..r.int(2, 4)).map(|_| (r.facing(0.9, 1.2), r.range(0.02, 0.04))).collect();
            l.rift_cols = (Vec3::new(0.93, 0.56, 0.49), Vec3::new(1.0, 0.88, 0.63));
        }
    }
    if let Some(rings) = &k.rings
        && r.chance(rings.chance)
    {
        l.ring = Some(ring(r, hex(&rings.color), tilt, rings.bright));
    }
    l
}

/// Puts an eyeball world's rings round the point under its light (feature frame).
pub(super) fn aim_eye(l: &mut Look, sub: Vec3) {
    for c in &mut l.caps {
        c.c = sub;
    }
}

fn wave(r: &mut Stream, lat: (f32, f32), amp: f32) -> Wave {
    Wave { y: r.range(lat.0, lat.1).sin(), amp: r.range(amp * 0.3, amp), fr: r.range(2.0, 7.0), ph: r.range(0.0, TAU), ph2: r.range(0.0, TAU) }
}

/// `k` bands in colours from `pal` (the first the base), `contrast` how far they stray from it, `turb` streaks
/// along their boundaries; fine ripples over all.
fn bands(r: &mut Stream, l: &mut Look, pal: &[Rgb], n: (i32, i32), contrast: f32, turb: i32) {
    let k = r.int(n.0, n.1);
    let mut lats: Vec<f32> = (0..k).map(|_| r.range(-1.35, 1.35)).collect();
    lats.sort_by(f32::total_cmp);
    let base = jit(r, pal[0], 4.0, 0.04, 0.03);
    let bounds: Vec<Wave> = lats.iter().map(|lat| Wave { y: lat.sin(), amp: r.range(0.006, 0.02), fr: r.range(2.0, 7.0), ph: r.range(0.0, TAU), ph2: r.range(0.0, TAU) }).collect();
    let cols: Vec<Rgb> = (0..=k)
        .map(|_| {
            let pick = *r.pick(pal);
            let c = jit(r, pick, 6.0, 0.06, 0.05);
            mix(base, c, contrast * r.range(0.5, 1.0))
        })
        .collect();
    for _ in 0..turb {
        let b = *r.pick(&bounds);
        let col = if r.chance(0.5) { mix(base, WHITE, 0.3) } else { mix(base, Vec3::ZERO, 0.25) };
        let wave = Wave { y: b.y + r.range(-0.012, 0.012), amp: r.range(0.006, 0.018), fr: r.range(8.0, 16.0), ph: r.range(0.0, TAU), ph2: r.range(0.0, TAU) };
        l.streaks.push(Streak { wave, h: r.range(0.006, 0.018), col, a: r.range(0.2, 0.45) });
    }
    l.base = base;
    l.mottle = 0.0;
    l.bands = Some(Bands { bounds, cols, soft: 0.012, ripples: Vec::new() });
    ripples_on(r, l, 0.05 * contrast.max(0.4));
}

/// Fine ripples along the latitudes, `amount` the brightness they swing by.
fn ripples_on(r: &mut Stream, l: &mut Look, amount: f32) {
    let ripples: Vec<[f32; 6]> = (0..6).map(|i| [r.range(18.0, 26.0) * 1.45f32.powi(i), r.range(0.0, TAU), amount * 0.8f32.powi(i), r.range(2.0, 6.0), r.range(0.01, 0.04), r.range(0.0, TAU)]).collect();
    match &mut l.bands {
        Some(b) => b.ripples = ripples,
        None => l.bands = Some(Bands { bounds: Vec::new(), cols: vec![l.base], soft: 0.01, ripples }),
    }
}

/// A storm on a band (between two boundaries), `size` its width (share of the radius).
fn storm(r: &mut Stream, l: &mut Look, col: Rgb, size: f32, halo: bool) {
    let y = match &l.bands {
        Some(b) if b.bounds.len() > 1 => {
            let i = r.int(1, b.bounds.len() as i32 - 1) as usize;
            (b.bounds[i - 1].y + b.bounds[i].y) / 2.0
        }
        _ => r.range(-0.4, 0.4),
    };
    let at = Vec2::new(r.range(-0.45, 0.45), y);
    l.storms.push(Storm { at, r: Vec2::new(size * r.range(0.8, 1.2), size * r.range(0.42, 0.55)), col, halo });
}

/// A landmass round a random point: shallows round it, sand at its edge, lowland, highland in its middle.
fn land(r: &mut Stream, l: &mut Look, rho: f32, shallow: Rgb, cols: [Rgb; 3]) -> Vec3 {
    let c = r.facing(0.9, 1.2);
    let f = Wob::new(r, 0.32, 7, 0.9);
    l.caps.push(Cap { c, rho, f, layers: vec![(1.22, shallow, 0.4), (1.0, cols[0], 0.9), (0.95, cols[1], 1.0), (0.5, cols[2], 0.55)], soft: 0.012 + 0.03 * rho, rough: 0.035 + 0.06 * rho });
    c
}

/// `n` soft patches of colour `col` (jittered), radius and alpha ranges.
fn patches(r: &mut Stream, l: &mut Look, n: i32, rho: (f32, f32), col: Rgb, a: (f32, f32)) {
    for _ in 0..n {
        let c = r.facing(0.9, 1.2);
        let rho = r.range(rho.0, rho.1);
        let col = jit(r, col, 8.0, 0.08, 0.05);
        let a = r.range(a.0, a.1);
        l.caps.push(Cap { c, rho, f: Wob::new(r, 0.32, 6, 1.0), layers: vec![(1.08, col, a * 0.35), (1.0, col, a)], soft: 0.02 + 0.05 * rho, rough: 0.03 });
    }
}

fn craters(r: &mut Stream, n: i32, big: f32) -> Vec<Crater> {
    (0..n)
        .map(|_| {
            let lat = r.range(-0.97, 0.97).asin();
            let lon = r.range(-1.5, 1.5);
            let s = 0.012 + big * r.next().powi(3);
            Crater { at: Vec3::new(lat.cos() * lon.sin(), lat.sin(), lat.cos() * lon.cos()), s }
        })
        .collect()
}

/// `n` random walks over the sphere, `steps` long, of `len` radians a step, turning by up to `turn`.
fn walks(r: &mut Stream, n: i32, steps: (i32, i32), len: f32, turn: f32) -> Vec<Walk> {
    (0..n)
        .map(|_| {
            let (mut lat, mut lon, mut h) = (r.range(-0.9, 0.9).asin(), r.range(-1.4, 1.4), r.range(0.0, TAU));
            (0..r.int(steps.0, steps.1))
                .map(|_| {
                    let p = Vec3::new(lat.cos() * lon.sin(), lat.sin(), lat.cos() * lon.cos());
                    lat = (lat + h.cos() * len).clamp(-1.45, 1.45);
                    lon += h.sin() * len / lat.cos().max(0.2);
                    h += r.gauss() * turn;
                    p
                })
                .collect()
        })
        .collect()
}

/// Wispy cloud streaks along latitudes: `n` of them, alpha up to `a`.
fn streaks(r: &mut Stream, n: i32, a: f32) -> Vec<(Wave, f32, f32)> {
    (0..n).map(|_| (wave(r, (-1.2, 1.2), 0.04), r.range(0.02, 0.06), r.range(0.4, 1.0) * a)).collect()
}

/// Clouds in `col`: soft blobs, wispy streaks and storm swirls, `a` how thick.
fn clouds(r: &mut Stream, col: Rgb, [blobs, n_streaks, swirls]: [i32; 3], a: f32) -> Clouds {
    let blobs = (0..blobs).map(|_| (r.facing(0.9, 1.2), r.range(0.08, 0.26), Wob::new(r, 0.4, 6, 0.9), r.range(0.3, 0.6) * a)).collect();
    let streaks = streaks(r, n_streaks, 0.4 * a);
    let swirls = (0..swirls)
        .map(|_| Swirl { c: r.facing(0.7, 1.0), size: r.range(0.14, 0.26), turns: r.range(2.0, 3.0), dir: if r.chance(0.5) { 1.0 } else { -1.0 }, a: r.range(0.6, 0.85) * a.min(1.0) })
        .collect();
    Clouds { col, blobs, streaks, swirls, wisp: Noise::new(r, 5, 4.0, 0.6) }
}

/// Rings: bands of sub-bands from just outside the body outwards; `bright`: wide and many.
fn ring(r: &mut Stream, col: Rgb, tilt: f32, bright: bool) -> Ring {
    let col = jit(r, col, 6.0, 0.05, 0.05);
    let rot = tilt + r.range(-0.1, 0.1);
    let k = r.range(0.14, 0.32);
    let (n, max, a) = if bright { (r.int(3, 4), 2.5, 0.42) } else { (r.int(1, 2), 2.3, 0.25) };
    let mut subs = Vec::new();
    let mut rad = r.range(1.25, 1.45);
    for _ in 0..n {
        if rad >= max {
            break;
        }
        let r1 = (rad + r.range(0.08, if bright { 0.45 } else { 0.25 })).min(max);
        let ns = r.int(3, 7);
        let mut s0 = rad;
        for j in 0..ns {
            let s1 = if j == ns - 1 { r1 } else { s0 + (r1 - s0) * r.range(0.15, 0.5) };
            let tone = if r.chance(0.5) { WHITE } else { Vec3::new(0.24, 0.2, 0.16) };
            let c = mix(col, tone, r.range(0.0, 0.25));
            subs.push((s0, s1, c, a * r.range(0.35, 1.2)));
            s0 = s1 + r.range(0.002, 0.012);
        }
        rad = r1 + r.range(0.015, 0.08);
    }
    Ring { rot, k, subs }
}
