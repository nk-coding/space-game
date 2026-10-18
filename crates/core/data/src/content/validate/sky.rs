//! Validation of the sky (part of [`Validator`]'s impl, GDR-0105): planet kinds and the `sky` of cluster types and
//! bands.

use super::*;

/// Most colour sets a planet kind may have.
pub const MAX_PALETTES: usize = 16;
/// Most colours in one set.
pub const MAX_PALETTE_COLORS: usize = 24;

impl<'a> Validator<'a> {
    /// A colour field: `#rrggbb`.
    fn hex<T: Entry>(&mut self, e: &T, field: &str, s: &str) {
        if hex_rgb(s).is_none() {
            self.err(e, field, format!("a colour `#rrggbb`, got `{s}`"));
        }
    }

    /// A planet kind: at least one palette, each with the colours its form reads, all `#rrggbb`; air colour with an
    /// atmosphere; a ring chance in `0..=1`.
    pub(super) fn planet_kind(&mut self, p: &'a PlanetKind) {
        if p.palettes.is_empty() || p.palettes.len() > MAX_PALETTES {
            self.err(p, "palettes", format!("1 to {MAX_PALETTES} colour sets"));
        }
        let need = p.form.colors();
        for (k, pal) in p.palettes.iter().enumerate() {
            if pal.colors.len() < need || pal.colors.len() > MAX_PALETTE_COLORS {
                self.err(p, format!("palettes[{k}].colors"), format!("{need} to {MAX_PALETTE_COLORS} colours for form {:?}, got {}", p.form, pal.colors.len()));
            }
            for (j, c) in pal.colors.iter().enumerate() {
                self.hex(p, &format!("palettes[{k}].colors[{j}]"), c);
            }
            match (&pal.air, p.atmosphere) {
                (Some(a), _) => self.hex(p, &format!("palettes[{k}].air"), a),
                (None, Atmosphere::None) => {}
                (None, _) => self.err(p, format!("palettes[{k}].air"), "the air's colour, for a kind with an atmosphere"),
            }
        }
        if let Some(r) = &p.rings {
            if !(r.chance.is_finite() && (0.0..=1.0).contains(&r.chance)) {
                self.err(p, "rings.chance", format!("a chance from 0 to 1, got {}", r.chance));
            }
            self.hex(p, "rings.color", &r.color);
        }
    }

    /// The shipyard's sky: one to four clouds, a strength, a galaxy no bigger than a rolled one inside the view, its colour.
    pub(super) fn home_sky<T: Entry>(&mut self, e: &T, h: &HomeSky) {
        if !(1..=4).contains(&h.clouds) {
            self.err(e, "home_sky.clouds", format!("1 to 4 clouds, got {}", h.clouds));
        }
        if !(h.galaxy.size.is_finite() && h.galaxy.size > 0.0 && h.galaxy.size <= 1.0) {
            self.err(e, "home_sky.galaxy.size", format!("a share above 0 and at most 1, got {}", h.galaxy.size));
        }
        self.hex(e, "home_sky.galaxy.color", &h.galaxy.color);
        let (x, y) = h.galaxy.at;
        if !((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)) {
            self.err(e, "home_sky.galaxy.at", format!("shares of the view from 0 to 1, got ({x}, {y})"));
        }
        if !(h.strength.is_finite() && h.strength > 0.0 && h.strength <= 1.0) {
            self.err(e, "home_sky.strength", format!("a strength above 0 and at most 1, got {}", h.strength));
        }
    }

    /// The sky of a cluster type or a band: known kinds of the right body with weights above 0, at least one moon
    /// kind, colours, strengths from 0 to 2.
    pub(super) fn sky<T: Entry>(&mut self, e: &T, s: &Sky) {
        let c = self.c;
        for (list, field, body) in [(&s.planets, "sky.planets", BodyClass::Planet), (&s.moons, "sky.moons", BodyClass::Moon)] {
            for (k, w) in list.iter().enumerate() {
                match c.planets.get(&w.kind) {
                    None => self.err(e, format!("{field}[{k}].kind"), format!("unknown planet kind `{}`", w.kind)),
                    Some(p) if p.body != body => self.err(e, format!("{field}[{k}].kind"), format!("`{}` is a {:?}, not a {body:?}", w.kind, p.body)),
                    Some(_) => {}
                }
                if w.weight == 0 {
                    self.err(e, format!("{field}[{k}].weight"), "a weight above 0");
                }
                self.count(e, &format!("{field}[{k}].weight"), w.weight, MAX_COUNT);
            }
        }
        if s.moons.is_empty() {
            self.err(e, "sky.moons", "at least one moon kind");
        }
        self.hex(e, "sky.nebula.0", &s.nebula.0);
        self.hex(e, "sky.nebula.1", &s.nebula.1);
        self.hex(e, "sky.nebula_core", &s.nebula_core);
        for (field, v) in [("sky.nebula_strength", s.nebula_strength), ("sky.galaxy", s.galaxy)] {
            if !(v.is_finite() && (0.0..=2.0).contains(&v)) {
                self.err(e, field, format!("a strength from 0 to 2, got {v}"));
            }
        }
    }
}
