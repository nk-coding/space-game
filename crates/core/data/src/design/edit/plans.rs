//! How the editing model's edits are planned and carried out (part of [`Drydock`]'s impl): part plans (place, move,
//! recolour, turn), painting and filling, bevel, slopes and their orientation, mirror twins.

use super::*;

impl Drydock {
    /// Plans replacing part `i` by `make(placement, shape, false)` if it fits (its own old cells do not count),
    /// and in mirror mode its twin `j` by `make(twin placement, twin shape, true)` if that fits beside the new
    /// part `i`. Returns the plan and `j` when the twin edit applies.
    pub(super) fn part_plan(
        &self,
        i: usize,
        make: impl Fn(&Placement, &PlacedShape, bool) -> Result<Placement, Refusal>,
    ) -> Result<(Preview, Option<usize>), Refusal> {
        let old = self.parts.get(i).ok_or(Refusal::NoSuchPart(i))?;
        let new = make(old, &self.shapes[i], false)?;
        if new.rot > 3 {
            return Err(Refusal::BadRotation);
        }
        let s = self.shape_of(&new);
        self.fits(&s, &[i])?;
        let (twin, image) = match self.mirror.then(|| self.twin(i)) {
            None => (Twin::Off, None),
            Some(None) => (Twin::Skipped(Refusal::NoTwin), None),
            Some(Some(j)) if j == i => (Twin::Itself, None),
            Some(Some(j)) => {
                let new = make(&self.parts[j], &self.shapes[j], true).and_then(|n| {
                    self.fits_beside(&self.shape_of(&n), &[j, i], Some((&s, i)))
                        .map(|()| n)
                });
                match new {
                    Ok(n) => (Twin::Applied, Some((j, n))),
                    Err(r) => (Twin::Skipped(r), None),
                }
            }
        };
        let (j, mirrored) = image.unzip();
        let plan = Preview {
            placement: new,
            twin,
            mirrored,
            removed: Vec::new(),
        };
        Ok((plan, j))
    }

    /// Carries out a [`Drydock::part_plan`] of part `i`.
    pub(super) fn apply_part_plan(&mut self, i: usize, (plan, j): (Preview, Option<usize>)) -> Outcome {
        let mut log = Vec::new();
        for (k, new) in std::iter::once((i, plan.placement)).chain(j.zip(plan.mirrored)) {
            let old = self.parts[k].clone();
            if new != old {
                self.exec(Change::Set { index: k, old, new }, &mut log);
            }
        }
        self.finish(log, Some(i), plan.twin)
    }

    /// The plan of [`Drydock::set_material`].
    pub(super) fn recolor_plan(&self, i: usize, material: &str) -> Result<(Preview, Option<usize>), Refusal> {
        self.armor(material)?;
        let part = self.parts.get(i).ok_or(Refusal::NoSuchPart(i))?;
        if !self.catalog.parts[&part.part].hull {
            return Err(Refusal::NotHull);
        }
        self.part_plan(i, |old, _, _| {
            Ok(Placement {
                material: Some(material.to_string()),
                ..old.clone()
            })
        })
    }

    /// The plan of turning slope `k` to the shape `s` (its twin to the mirror image).
    pub(super) fn turn_plan(&self, k: usize, s: PlacedShape) -> Result<(Preview, Option<usize>), Refusal> {
        let m = mirrored(&s, self.frame.0);
        self.part_plan(k, |old, _, twin| {
            Ok(with_pose(old, if twin { &m } else { &s }))
        })
    }

    /// The plan of putting down a new part `p` with the shape `s` (inside the frame): it replaces every part its
    /// footprint overlaps. In mirror mode also its mirror image, unless that is the same shape, replacing what it
    /// overlaps, if it keeps clear of `p` (else `Skipped(Overlaps(k))`, `k` the index `p` gets).
    pub(super) fn new_part_plan(&self, p: Placement, s: &PlacedShape) -> Preview {
        let mut removed = self.under(s);
        let (twin, image) = if !self.mirror {
            (Twin::Off, None)
        } else {
            let m = mirrored(s, self.frame.0);
            let cells = s.cells();
            if same_shape(&m, s) {
                (Twin::Itself, None)
            } else if m.cells().iter().any(|c| cells.contains(c)) {
                let k = self.parts.len() - removed.len();
                (Twin::Skipped(Refusal::Overlaps(k)), None)
            } else {
                match self.in_stock(&p.part, 2) {
                    Ok(()) => {
                        removed.extend(self.under(&m));
                        removed.sort_unstable();
                        removed.dedup();
                        (Twin::Applied, Some(with_pose(&p, &m)))
                    }
                    Err(r) => (Twin::Skipped(r), None),
                }
            }
        };
        // The very same parts already there: nothing is replaced, nothing changes.
        let news = [Some(&p), image.as_ref()];
        let count = news.iter().flatten().count();
        if removed.len() == count && removed.iter().all(|&r| news.contains(&Some(&self.parts[r]))) {
            removed.clear();
        }
        Preview {
            placement: p,
            twin,
            mirrored: image,
            removed,
        }
    }

    /// Carries out a [`Drydock::new_part_plan`]: removes the replaced parts, then appends the part and its mirror
    /// image. A plan that replaces nothing on an occupied footprint puts down what is already there: no change.
    pub(super) fn apply_new_part(&mut self, plan: Preview) -> Outcome {
        if plan.removed.is_empty()
            && let Some(&k) = self.under(&self.shape_of(&plan.placement)).first()
        {
            return self.finish(Vec::new(), Some(k), plan.twin);
        }
        let mut log = Vec::new();
        for &index in plan.removed.iter().rev() {
            let placement = self.parts[index].clone();
            self.exec(Change::Remove { index, placement }, &mut log);
        }
        let index = self.parts.len();
        let news = std::iter::once(plan.placement).chain(plan.mirrored);
        for (k, placement) in news.enumerate() {
            let at = index + k;
            self.exec(Change::Insert { index: at, placement }, &mut log);
        }
        self.finish(log, Some(index), plan.twin)
    }

    /// The parts a shape overlaps, ascending.
    pub(super) fn under(&self, s: &PlacedShape) -> Vec<usize> {
        let mut out: Vec<usize> = s.cells().into_iter().filter_map(|c| self.part_at(c)).collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// The plan of [`Drydock::paint`]: both cells are decided on the design as it is. A non-hull part covering
    /// both cells is replaced once, by a block in each.
    pub(super) fn paint_plan(&self, cell: Cell, material: &str) -> Result<PaintPlan, Refusal> {
        self.armor(material)?;
        let (first, gone) = self.paint_action(cell, material)?;
        let first_part = match first {
            PaintAction::Nothing(k) | PaintAction::Recolor(k) => Some(k),
            PaintAction::Insert(_) => None,
        };
        let mut removed: Vec<usize> = gone.into_iter().collect();
        let mirror_cell = self.mirror_cell(cell);
        let (second, twin) = if !self.mirror {
            (None, Twin::Off)
        } else if mirror_cell == cell {
            (None, Twin::Itself)
        } else {
            match self.paint_action(mirror_cell, material) {
                Err(r) => (None, Twin::Skipped(r)),
                Ok((PaintAction::Nothing(j) | PaintAction::Recolor(j), _)) if Some(j) == first_part => {
                    (None, Twin::Itself)
                }
                Ok((a, gone)) => match self.pair_in_stock(&first, &a) {
                    Ok(()) => {
                        removed.extend(gone);
                        (Some(a), Twin::Applied)
                    }
                    Err(r) => (None, Twin::Skipped(r)),
                },
            }
        };
        removed.sort_unstable();
        removed.dedup();
        Ok(PaintPlan {
            first,
            second,
            twin,
            removed,
        })
    }

    /// What a planned paint leaves in its cell.
    pub(super) fn painted(&self, a: &PaintAction, material: &str) -> Placement {
        match a {
            PaintAction::Nothing(k) => self.parts[*k].clone(),
            PaintAction::Recolor(k) => Placement {
                material: Some(material.to_string()),
                ..self.parts[*k].clone()
            },
            PaintAction::Insert(p) => p.clone(),
        }
    }

    pub(super) fn recolor(&mut self, k: usize, material: &str, log: &mut Vec<Change>) {
        let old = self.parts[k].clone();
        let new = Placement {
            material: Some(material.to_string()),
            ..old.clone()
        };
        self.exec(Change::Set { index: k, old, new }, log);
    }

    pub(super) fn fill_rect(
        &mut self,
        (x0, x1): (i32, i32),
        (y0, y1): (i32, i32),
        block: &str,
        material: &str,
        log: &mut Vec<Change>,
        filled: &mut Vec<Cell>,
    ) {
        let mut left = self.left(block);
        for y in y0..=y1 {
            for x in x0..=x1 {
                if self.part_at((x, y)).is_none() {
                    // Fills as many cells as blocks are left.
                    if let Availability::Count(n) = &mut left {
                        let Some(k) = n.checked_sub(1) else { return };
                        *n = k;
                    }
                    let placement = unit_placement(block, (x, y), 0, material);
                    let index = self.parts.len();
                    self.exec(Change::Insert { index, placement }, log);
                    filled.push((x, y));
                }
            }
        }
    }

    /// What painting one cell does, and the non-hull part it replaces.
    pub(super) fn paint_action(
        &self,
        cell: Cell,
        material: &str,
    ) -> Result<(PaintAction, Option<usize>), Refusal> {
        if self.slot(cell).is_none() {
            return Err(Refusal::Outside);
        }
        let under = self.part_at(cell);
        if let Some(k) = under
            && self.catalog.parts[&self.parts[k].part].hull
        {
            return Ok(match self.parts[k].material.as_deref() == Some(material) {
                true => (PaintAction::Nothing(k), None),
                false => (PaintAction::Recolor(k), None),
            });
        }
        let block = self.catalog.block.as_ref().ok_or(Refusal::NoBlockPart)?;
        self.in_stock(block, 1)?;
        let placement = unit_placement(block, cell, 0, material);
        Ok((PaintAction::Insert(placement), under))
    }

    /// Blocks went into these cells: an open stroke keeps them for its end, else bevel mode cuts their corners
    /// now (a single edit is a stroke of its own).
    pub(super) fn blocks_in(&mut self, cells: &[Cell], log: &mut Vec<Change>) {
        match &mut self.stroke {
            Some(stroke) => stroke.painted.extend_from_slice(cells),
            None if self.bevel => self.bevel_cells(cells, log),
            None => {}
        }
    }

    /// Bevel: which of these cells hold a 1×1 block on an outer corner of the design as it is, and the slope each
    /// becomes (same material, oriented by [`Drydock::corner_rot_with`]). All are decided before any change.
    pub(super) fn bevel_plan(&self, cells: &[Cell]) -> Vec<(usize, Placement)> {
        let (Some(block), Some(slope)) = (&self.catalog.block, &self.catalog.corner) else {
            return Vec::new();
        };
        let mut out: Vec<(usize, Placement)> = Vec::new();
        for &cell in cells {
            let Some(k) = self.part_at(cell) else { continue };
            let p = &self.parts[k];
            if p.part != *block || out.iter().any(|(j, _)| *j == k) {
                continue;
            }
            if let Some(rot) = self.corner_rot_with(cell, &[], &[]) {
                let slope = Placement {
                    part: slope.clone(),
                    rot,
                    flip: false,
                    ..p.clone()
                };
                out.push((k, slope));
            }
        }
        out
    }

    /// Turns the blocks of [`Drydock::bevel_plan`] into their slopes.
    pub(super) fn bevel_cells(&mut self, cells: &[Cell], log: &mut Vec<Change>) {
        for (index, new) in self.bevel_plan(cells) {
            // A corner stays a block when no slope is left.
            if self.in_stock(&new.part, 1).is_err() {
                continue;
            }
            let old = self.parts[index].clone();
            self.exec(Change::Set { index, old, new }, log);
        }
    }

    /// Bevel: if `cell` is an outer corner (two adjacent sides touch a part's straight side, the other two are
    /// open), the `rot` of the 1×1 slope (unflipped) whose straight sides are the touching sides. Only the
    /// neighbours count, as the design would be with blocks in the cells `extra` and without the parts `gone`.
    pub(super) fn corner_rot_with(&self, cell: Cell, extra: &[Cell], gone: &[usize]) -> Option<u8> {
        let (mut touch, mut open) = (0u8, 0u8);
        for d in Dir::ALL {
            let n = step(cell, d);
            if extra.contains(&n) {
                touch |= d.bit();
                continue;
            }
            match self.part_at(n).filter(|k| !gone.contains(k)) {
                None => open |= d.bit(),
                Some(_) if self.straight(n, d.opposite()) => touch |= d.bit(),
                Some(_) => {}
            }
        }
        if touch.count_ones() != 2 || touch | open != 0b1111 {
            return None;
        }
        (0..4).find(|&r| {
            Dir::Left.transform(r, false).bit() | Dir::Down.transform(r, false).bit() == touch
        })
    }

    pub(super) fn slope_plan(
        &self,
        part: &str,
        cell: Cell,
        material: &str,
        turn: Option<(u8, bool)>,
    ) -> Result<SlopePlan, Refusal> {
        let info = self
            .catalog
            .parts
            .get(part)
            .ok_or_else(|| Refusal::UnknownPart(part.to_string()))?;
        if info.footprint.shape != Shape::Slope {
            return Err(Refusal::NotSlope);
        }
        let fp = info.footprint;
        let new = unit_placement(part, cell, 0, material);
        self.validate(&new)?;
        if self.slot(cell).is_none() {
            return Err(Refusal::Outside);
        }
        let orientations = orientations(fp);
        if let Some(k) = self.part_at(cell).filter(|&k| self.parts[k].part == part) {
            if self.parts[k].material.as_deref() != Some(material) {
                return Ok(SlopePlan::Recolor(k));
            }
            let current = self.shapes[k];
            let at = orientations
                .iter()
                .position(|&(rot, flip)| same_orientation(&current, rot, flip))
                .unwrap_or(0);
            let n = orientations.len();
            return (1..n)
                .map(|t| orientations[(at + t) % n])
                .filter_map(|(rot, flip)| self.anchored(fp, cell, rot, flip))
                .find(|s| self.fits(s, &[k]).is_ok())
                .map(|s| SlopePlan::Turn(k, s))
                .ok_or(Refusal::NoRoom);
        }
        if let Some((rot, flip)) = turn {
            if rot > 3 {
                return Err(Refusal::BadRotation);
            }
            let s = self.anchored(fp, cell, rot, flip).ok_or(Refusal::NoRoom)?;
            return Ok(SlopePlan::New(with_pose(&new, &s)));
        }
        // Fewest replaced parts first (none where it fits), then the hull rules on the design without them.
        // `max_by_key` keeps the last of equal maxima; reversed, ties go to the first in turning order.
        let best = orientations
            .iter()
            .filter_map(|&(rot, flip)| self.anchored(fp, cell, rot, flip))
            .rev()
            .max_by_key(|s| {
                let gone = self.under(s);
                let (links, blocked) = self.slope_contacts(s, &gone);
                let fewest = std::cmp::Reverse(gone.len());
                (fewest, links, std::cmp::Reverse(blocked), self.lean(s))
            })
            .ok_or(Refusal::NoRoom)?;
        self.in_stock(part, 1)?;
        Ok(SlopePlan::New(with_pose(&new, &best)))
    }

    /// `(links, blocked)` of a slope candidate: its straight unit sides that meet a neighbour's straight side,
    /// and the occupied cells in front of its sloped face; the parts `gone` (which it replaces) do not count.
    pub(super) fn slope_contacts(&self, s: &PlacedShape, gone: &[usize]) -> (u32, u32) {
        let cells = s.cells();
        let sides = s.connectable_sides();
        let (mut links, mut blocked) = (0, 0);
        for &c in &cells {
            for d in Dir::ALL {
                let n = step(c, d);
                if cells.contains(&n) || self.part_at(n).is_none_or(|k| gone.contains(&k)) {
                    continue;
                }
                if !sides.contains(&(c, d)) {
                    blocked += 1;
                } else if self.straight(n, d.opposite()) {
                    links += 1;
                }
            }
        }
        (links, blocked)
    }

    /// Tie-break of slope orientations: how much the face normal points away from the frame's centre line
    /// (weight 2) and forward (weight 1), the concept board's preference.
    pub(super) fn lean(&self, s: &PlacedShape) -> i32 {
        let (nx, ny) =
            transform_vector(s.footprint.h as f32, -(s.footprint.w as f32), s.rot, s.flip);
        let side = (2 * s.x + s.dims().0 as i32 - self.frame.0).signum();
        2 * nx as i32 * side - ny as i32
    }

    /// A footprint in an orientation, centred on `cell` (rounded towards the top-left) and pushed inside the
    /// frame; `None` if it is larger than the frame.
    pub(super) fn anchored(&self, fp: Footprint, (cx, cy): Cell, rot: u8, flip: bool) -> Option<PlacedShape> {
        let (w, h) = fp.dims(rot);
        let (w, h) = (w as i32, h as i32);
        if w > self.frame.0 || h > self.frame.1 {
            return None;
        }
        Some(PlacedShape {
            footprint: fp,
            x: (cx - (w - 1) / 2).clamp(0, self.frame.0 - w),
            y: (cy - (h - 1) / 2).clamp(0, self.frame.1 - h),
            rot,
            flip,
        })
    }

    /// The part that is the mirror image of part `i` (same part id, same shape as `i` mirrored); `i` itself
    /// for a part that mirrors onto itself.
    pub(super) fn twin(&self, i: usize) -> Option<usize> {
        let m = mirrored(&self.shapes[i], self.frame.0);
        let j = self.part_at((m.x, m.y))?;
        (self.parts[j].part == self.parts[i].part && same_shape(&self.shapes[j], &m)).then_some(j)
    }
}
