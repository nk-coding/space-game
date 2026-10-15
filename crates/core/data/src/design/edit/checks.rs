//! The editing model's checks (part of [`Drydock`]'s impl): part counts, armour, a placement's shape inside the
//! frame and clear of other parts.

use super::*;

impl Drydock {
    /// [`Refusal::NoneLeft`] unless `more` more of `part` are left.
    pub(super) fn in_stock(&self, part: &str, more: u32) -> Result<(), Refusal> {
        match self.left(part) {
            Availability::Count(n) if n < more => Err(Refusal::NoneLeft(part.to_string())),
            _ => Ok(()),
        }
    }

    /// Painting the mirror cell as well: a second new part of the same kind as the first needs two left.
    pub(super) fn pair_in_stock(&self, first: &PaintAction, second: &PaintAction) -> Result<(), Refusal> {
        match (first, second) {
            (PaintAction::Insert(a), PaintAction::Insert(b)) if a.part == b.part => self.in_stock(&b.part, 2),
            _ => Ok(()),
        }
    }

    pub(super) fn armor(&self, material: &str) -> Result<(), Refusal> {
        match self.catalog.armor.contains(material) {
            true => Ok(()),
            false => Err(Refusal::NotArmor(material.to_string())),
        }
    }

    pub(super) fn validate(&self, p: &Placement) -> Result<PlacedShape, Refusal> {
        let info = self
            .catalog
            .parts
            .get(&p.part)
            .ok_or_else(|| Refusal::UnknownPart(p.part.clone()))?;
        if p.rot > 3 {
            return Err(Refusal::BadRotation);
        }
        match (&p.material, info.hull) {
            (None, true) => return Err(Refusal::NeedsMaterial),
            (Some(m), true) => self.armor(m)?,
            (Some(_), false) => return Err(Refusal::NotHull),
            (None, false) => {}
        }
        if let Some(o) = &nonempty(p.power) {
            power_ok(info, o)?;
        }
        if let Some(g) = group_of(p.group) {
            group_ok(info, g)?;
        }
        Ok(self.shape_of(p))
    }

    /// Inside the frame.
    pub(super) fn inside(&self, s: &PlacedShape) -> Result<(), Refusal> {
        let (w, h) = s.dims();
        let (x, y) = (s.x as i64, s.y as i64);
        match x >= 0 && y >= 0 && x + w as i64 <= self.frame.0 as i64 && y + h as i64 <= self.frame.1 as i64 {
            true => Ok(()),
            false => Err(Refusal::Outside),
        }
    }

    /// Inside the frame and on free cells (parts in `ignore` do not count).
    pub(super) fn fits(&self, s: &PlacedShape, ignore: &[usize]) -> Result<(), Refusal> {
        self.fits_beside(s, ignore, None)
    }

    /// [`Drydock::fits`], and clear of `other`: the shape part `k` is about to have (the first edit of a pair,
    /// for its mirror image). The first overlap in the shape's cell order is named.
    pub(super) fn fits_beside(
        &self,
        s: &PlacedShape,
        ignore: &[usize],
        other: Option<(&PlacedShape, usize)>,
    ) -> Result<(), Refusal> {
        self.inside(s)?;
        let other = other.map(|(o, k)| (o.cells(), k));
        for c in s.cells() {
            if let Some((cells, k)) = &other
                && cells.contains(&c)
            {
                return Err(Refusal::Overlaps(*k));
            }
            if let Some(k) = self.part_at(c)
                && !ignore.contains(&k)
            {
                return Err(Refusal::Overlaps(k));
            }
        }
        Ok(())
    }

    pub(super) fn shape_of(&self, p: &Placement) -> PlacedShape {
        PlacedShape {
            footprint: self.catalog.parts[&p.part].footprint,
            x: p.x,
            y: p.y,
            rot: p.rot,
            flip: p.flip,
        }
    }
}
