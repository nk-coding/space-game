//! Validation of the scenery kinds (part of [`Validator`]'s impl, GDR-0104): rock kinds, landmarks and the
//! `scenery` of cluster types and voyages.

use super::*;
use crate::content::art_keys::ROCK_TONES;

/// Largest rock radius (m).
pub const MAX_ROCK_SIZE: f32 = 120.0;
/// Most corners of a rock's outline.
pub const MAX_ROCK_CORNERS: u32 = 16;
/// Most rocks a cluster may place (every group at its largest count).
pub const MAX_CLUSTER_ROCKS: u32 = 600;
/// Most derelict shells drawn in a cluster.
pub const MAX_DERELICTS: u32 = 32;
/// Most distant fights a minute.
pub const MAX_DISTANT_FIGHTS: f32 = 20.0;

impl<'a> Validator<'a> {
    /// A rock kind: a known material, a size, an outline of at least 3 corners, a roughness below 1, a stretch of at
    /// least 1 and a known tone.
    pub(super) fn rock_kind(&mut self, r: &'a RockKind) {
        if !self.c.materials.contains(&r.material) {
            self.err(r, "material", format!("unknown material `{}`", r.material));
        }
        let (lo, hi) = r.size;
        if !(lo.is_finite() && hi.is_finite() && lo > 0.0 && lo <= hi && hi <= MAX_ROCK_SIZE) {
            self.err(r, "size", format!("`(min, max)` with 0 < min ≤ max ≤ {MAX_ROCK_SIZE} m, got ({lo}, {hi})"));
        }
        let (lo, hi) = r.corners;
        if lo < 3 || lo > hi || hi > MAX_ROCK_CORNERS {
            self.err(r, "corners", format!("`(min, max)` with 3 ≤ min ≤ max ≤ {MAX_ROCK_CORNERS}, got ({lo}, {hi})"));
        }
        if !(r.roughness.is_finite() && (0.0..0.8).contains(&r.roughness)) {
            self.err(r, "roughness", format!("a share from 0 up to (not including) 0.8, got {}", r.roughness));
        }
        let (lo, hi) = r.stretch;
        if !(lo.is_finite() && hi.is_finite() && lo >= 1.0 && lo <= hi && hi <= 4.0) {
            self.err(r, "stretch", format!("`(min, max)` with 1 ≤ min ≤ max ≤ 4, got ({lo}, {hi})"));
        }
        if !(r.ram.is_finite() && (0.0..=4.0).contains(&r.ram)) {
            self.err(r, "ram", format!("a factor from 0 to 4, got {}", r.ram));
        }
        self.key_in(r, "tone", &r.tone, ROCK_TONES, "rock tone");
    }

    /// A landmark: a size and a parallax in `0..1`.
    pub(super) fn landmark(&mut self, l: &'a Landmark) {
        let (lo, hi) = l.size;
        if !(lo.is_finite() && hi.is_finite() && lo > 0.0 && lo <= hi && hi <= MAX_VOYAGE_RADIUS) {
            self.err(l, "size", format!("`(min, max)` with 0 < min ≤ max ≤ {MAX_VOYAGE_RADIUS} m, got ({lo}, {hi})"));
        }
        if !(l.parallax.is_finite() && (0.0..1.0).contains(&l.parallax)) {
            self.err(l, "parallax", format!("a share from 0 up to (not including) 1, got {}", l.parallax));
        }
    }

    /// The scenery of a cluster type or a voyage (`star`: it has a hot star): known rock kinds and landmarks, layouts
    /// that fit into `radius` (the smallest it may roll), densities and chances in range; a corona only beside a star.
    pub(super) fn scenery<T: Entry>(&mut self, e: &T, s: &Scenery, radius: f32, star: bool) {
        let c = self.c;
        let mut most = 0u32;
        for (k, g) in s.rocks.iter().enumerate() {
            let f = format!("scenery.rocks[{k}]");
            if !c.rocks.contains(&g.rock) {
                self.err(e, format!("{f}.rock"), format!("unknown rock `{}`", g.rock));
            }
            self.count_range(e, &format!("{f}.count"), g.count, MAX_CLUSTER_ROCKS);
            self.count_range(e, &format!("{f}.rocks"), g.rocks, MAX_CLUSTER_ROCKS);
            most = most.saturating_add(g.count.1.saturating_mul(g.rocks.1));
            let wide = match g.layout {
                RockLayout::Field { radius } => Some(("layout.radius", radius)),
                RockLayout::Belt { width } => Some(("layout.width", width)),
                RockLayout::Scatter => None,
            };
            if let Some((name, x)) = wide
                && !(x.is_finite() && x > 0.0 && x <= radius)
            {
                self.err(e, format!("{f}.{name}"), format!("above 0 and at most the cluster's radius ({radius} m), got {x}"));
            }
        }
        if most > MAX_CLUSTER_ROCKS {
            self.err(e, "scenery.rocks", format!("at most {MAX_CLUSTER_ROCKS} rocks in a cluster, up to {most} here"));
        }
        for (name, x) in [("scenery.dust", s.dust), ("scenery.debris", s.debris)] {
            if !(x.is_finite() && (0.0..=4.0).contains(&x)) {
                self.err(e, name, format!("a density from 0 to 4 (1: the base amount), got {x}"));
            }
        }
        self.count_range(e, "scenery.derelicts", s.derelicts, MAX_DERELICTS);
        if !(s.fights.is_finite() && (0.0..=MAX_DISTANT_FIGHTS).contains(&s.fights)) {
            self.err(e, "scenery.fights", format!("fights a minute from 0 to {MAX_DISTANT_FIGHTS}, got {}", s.fights));
        }
        for (k, p) in s.landmarks.iter().enumerate() {
            let f = format!("scenery.landmarks[{k}]");
            match c.landmarks.get(&p.landmark) {
                None => self.err(e, format!("{f}.landmark"), format!("unknown landmark `{}`", p.landmark)),
                Some(l) if l.form == LandmarkForm::Corona && !star => {
                    self.err(e, format!("{f}.landmark"), format!("`{}` is a star's corona, and there is no hot star here", l.id))
                }
                Some(_) => {}
            }
            if !(p.chance.is_finite() && p.chance > 0.0 && p.chance <= 1.0) {
                self.err(e, format!("{f}.chance"), format!("a chance above 0 and at most 1, got {}", p.chance));
            }
        }
    }
}
