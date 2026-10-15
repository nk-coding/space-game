//! The editing model's edits (part of [`Drydock`]'s impl): place, paint, fill, erase, move, rotate, flip, material,
//! power, strokes, undo and redo, part availability. Each is one undo step and refused with a reason when illegal.

use super::*;

impl Drydock {
    /// Places any part (appended to the parts), replacing every part its footprint overlaps, as one step. A hull
    /// part needs an armour material, other parts none; a power override only goes on parts that draw power (an
    /// empty override is dropped); the footprint must lie inside the frame. Mirror mode also places the mirror
    /// image (replacing what it overlaps), unless it is the same shape (a symmetric part centred on the axis) or
    /// would overlap the placed part.
    pub fn place(&mut self, p: Placement) -> Result<Outcome, Refusal> {
        let plan = self.preview_place(&p)?;
        Ok(self.apply_new_part(plan))
    }

    /// The slope tool. On a slope of the same part it repaints it if the material differs, else turns it to its
    /// next orientation that fits. Anywhere else it places the slope, replacing what its footprint overlaps:
    /// oriented to the hull (`turn` = `None`, see `docs/drydock.md`), or in the orientation `turn` =
    /// `(rot, flip)`, centred on the cell. Mirror mode places, repaints or turns the mirror image too.
    pub fn place_slope(
        &mut self,
        part: &str,
        cell: Cell,
        material: &str,
        turn: Option<(u8, bool)>,
    ) -> Result<Outcome, Refusal> {
        match self.slope_plan(part, cell, material, turn)? {
            SlopePlan::New(p) => {
                let s = self.shape_of(&p);
                let plan = self.new_part_plan(p, &s);
                Ok(self.apply_new_part(plan))
            }
            SlopePlan::Recolor(k) => self.set_material(k, material),
            SlopePlan::Turn(k, s) => {
                let plan = self.turn_plan(k, s)?;
                Ok(self.apply_part_plan(k, plan))
            }
        }
    }

    /// The armour paint tool: an empty cell gets the 1×1 block, a hull part under the cursor takes the material,
    /// any other part is replaced by the block. Mirror mode paints the mirror cell too, decided on the design as
    /// it was before this call, so a symmetric design stays symmetric. Bevel mode cuts the outer corners among the
    /// painted cells when the stroke ends (at once outside a stroke: a click is a stroke of one cell).
    pub fn paint(&mut self, cell: Cell, material: &str) -> Result<Outcome, Refusal> {
        let plan = self.paint_plan(cell, material)?;
        let mut log = Vec::new();
        for a in [Some(&plan.first), plan.second.as_ref()].into_iter().flatten() {
            if let PaintAction::Recolor(k) = a {
                self.recolor(*k, material, &mut log);
            }
        }
        for &index in plan.removed.iter().rev() {
            let placement = self.parts[index].clone();
            self.exec(Change::Remove { index, placement }, &mut log);
        }
        let mut inserted = Vec::new();
        for a in [Some(plan.first), plan.second].into_iter().flatten() {
            if let PaintAction::Insert(placement) = a {
                inserted.push((placement.x, placement.y));
                let index = self.parts.len();
                self.exec(Change::Insert { index, placement }, &mut log);
            }
        }
        self.blocks_in(&inserted, &mut log);
        let part = self.part_at(cell);
        Ok(self.finish(log, part, plan.twin))
    }

    /// The fill tool: every empty cell of the rectangle spanned by two corner cells (clipped to the frame) gets
    /// the 1×1 block in this material, row by row, as one step. Mirror mode fills the mirrored rectangle too. In
    /// bevel mode the outer corners among the filled cells become slopes, in the same step.
    pub fn fill(&mut self, a: Cell, b: Cell, material: &str) -> Result<Outcome, Refusal> {
        self.armor(material)?;
        let block = self.catalog.block.clone().ok_or(Refusal::NoBlockPart)?;
        self.in_stock(&block, 1)?;
        let (x0, x1) = (a.0.min(b.0).max(0), a.0.max(b.0).min(self.frame.0 - 1));
        let (y0, y1) = (a.1.min(b.1).max(0), a.1.max(b.1).min(self.frame.1 - 1));
        if x0 > x1 || y0 > y1 {
            return Err(Refusal::Outside);
        }
        let mut log = Vec::new();
        let mut filled = Vec::new();
        let mut fill = |dock: &mut Self, xs| dock.fill_rect(xs, (y0, y1), &block, material, &mut log, &mut filled);
        fill(self, (x0, x1));
        let twin = if !self.mirror {
            Twin::Off
        } else {
            let (m0, m1) = (self.frame.0 - 1 - x1, self.frame.0 - 1 - x0);
            if (m0, m1) == (x0, x1) {
                Twin::Itself
            } else {
                fill(self, (m0, m1));
                Twin::Applied
            }
        };
        self.blocks_in(&filled, &mut log);
        Ok(self.finish(log, None, twin))
    }

    /// The erase tool: removes the part covering the cell (later parts move down one index). Mirror mode also
    /// removes the part covering the mirror cell.
    pub fn erase(&mut self, cell: Cell) -> Result<Outcome, Refusal> {
        if self.slot(cell).is_none() {
            return Err(Refusal::Outside);
        }
        let k = self.part_at(cell).ok_or(Refusal::Empty)?;
        let (twin, other) = match self.mirror {
            false => (Twin::Off, None),
            true => match self.part_at(self.mirror_cell(cell)) {
                None => (Twin::Skipped(Refusal::Empty), None),
                Some(j) if j == k => (Twin::Itself, None),
                Some(j) => (Twin::Applied, Some(j)),
            },
        };
        let mut log = Vec::new();
        let mut doomed = [Some(k), other];
        doomed.sort_unstable_by(|a, b| b.cmp(a));
        for index in doomed.into_iter().flatten() {
            let placement = self.parts[index].clone();
            self.exec(Change::Remove { index, placement }, &mut log);
        }
        Ok(self.finish(log, None, twin))
    }

    /// The part tool of erase (Delete on a selected part): removes part `i` (later parts move down one index).
    /// Mirror mode also removes its twin, the part that is its mirror image; whatever else covers the mirror cells
    /// stays (`Twin::Skipped(NoTwin)` when it has no twin), unlike [`Drydock::erase`] on a cell.
    pub fn remove_part(&mut self, i: usize) -> Result<Outcome, Refusal> {
        if i >= self.parts.len() {
            return Err(Refusal::NoSuchPart(i));
        }
        let (twin, other) = match self.mirror.then(|| self.twin(i)) {
            None => (Twin::Off, None),
            Some(None) => (Twin::Skipped(Refusal::NoTwin), None),
            Some(Some(j)) if j == i => (Twin::Itself, None),
            Some(Some(j)) => (Twin::Applied, Some(j)),
        };
        let mut log = Vec::new();
        let mut doomed = [Some(i), other];
        doomed.sort_unstable_by(|a, b| b.cmp(a));
        for index in doomed.into_iter().flatten() {
            let placement = self.parts[index].clone();
            self.exec(Change::Remove { index, placement }, &mut log);
        }
        Ok(self.finish(log, None, twin))
    }

    /// Moves a part by `(dx, dy)` cells if it still fits (its own old cells do not count). The mirror twin
    /// moves by `(−dx, dy)`.
    pub fn move_part(&mut self, i: usize, dx: i32, dy: i32) -> Result<Outcome, Refusal> {
        let plan = self.part_plan(i, move_by(dx, dy))?;
        Ok(self.apply_part_plan(i, plan))
    }

    /// Turns a part a quarter turn if it still fits. It turns about a fixed point of the part: its centre when
    /// the footprint's `w + h` is even, else the centre's neighbour half a cell towards the part's front (so
    /// the centre moves as little as possible, and turning back restores the placement exactly). The mirror
    /// twin turns the other way.
    pub fn rotate(&mut self, i: usize, clockwise: bool) -> Result<Outcome, Refusal> {
        let plan = self.part_plan(i, |old, s, twin| {
            Ok(with_pose(old, &turned(s, clockwise != twin)))
        })?;
        Ok(self.apply_part_plan(i, plan))
    }

    /// Mirrors a part left-right in place (same cells). The mirror twin flips too.
    pub fn flip(&mut self, i: usize) -> Result<Outcome, Refusal> {
        let plan = self.part_plan(i, |old, s, _| Ok(with_pose(old, &flipped(s))))?;
        Ok(self.apply_part_plan(i, plan))
    }

    /// Sets a hull part's armour material (and its mirror twin's).
    pub fn set_material(&mut self, i: usize, material: &str) -> Result<Outcome, Refusal> {
        let plan = self.recolor_plan(i, material)?;
        Ok(self.apply_part_plan(i, plan))
    }

    /// Sets or clears a part's power override (and its mirror twin's). Only parts that draw power take one; an
    /// empty override counts as none.
    pub fn set_power(
        &mut self,
        i: usize,
        power: Option<PowerOverride>,
    ) -> Result<Outcome, Refusal> {
        let part = self.parts.get(i).ok_or(Refusal::NoSuchPart(i))?;
        let power = nonempty(power);
        if let Some(o) = &power {
            power_ok(&self.catalog.parts[&part.part], o)?;
        }
        let plan = self.part_plan(i, |old, _, _| {
            Ok(Placement {
                power,
                ..old.clone()
            })
        })?;
        Ok(self.apply_part_plan(i, plan))
    }

    /// Puts a gun (and its mirror twin) into weapon group `group` (1 to `MAX_WEAPON_GROUP`, GDR-0088; group 1 is
    /// stored as none).
    pub fn set_group(&mut self, i: usize, group: u8) -> Result<Outcome, Refusal> {
        let part = self.parts.get(i).ok_or(Refusal::NoSuchPart(i))?;
        group_ok(&self.catalog.parts[&part.part], group)?;
        let group = group_of(Some(group));
        let plan = self.part_plan(i, |old, _, _| Ok(Placement { group, ..old.clone() }))?;
        Ok(self.apply_part_plan(i, plan))
    }

    /// Opens a stroke: every edit until [`Drydock::end_stroke`] becomes one undo step (a paint or erase drag).
    /// An open stroke is closed first; undo, redo and frame changes close it too.
    pub fn begin_stroke(&mut self) {
        self.end_stroke();
        self.stroke = Some(Stroke::default());
    }

    /// Closes the open stroke, if any. In bevel mode the outer corners among the cells it painted become slopes
    /// first (in the same undo step), decided on the finished shape, so the result does not depend on the order
    /// the cells were painted in.
    pub fn end_stroke(&mut self) {
        let Some(Stroke {
            mut changes,
            painted,
        }) = self.stroke.take()
        else {
            return;
        };
        if self.bevel {
            let mut log = Vec::new();
            self.bevel_cells(&painted, &mut log);
            if !log.is_empty() {
                self.revision += 1;
                changes.extend(log);
            }
        }
        if !changes.is_empty() {
            self.push_undo(changes);
        }
    }

    /// Undoes the last step (closing an open stroke first). False if there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        self.end_stroke();
        let Some(step) = self.undo.pop_back() else {
            return false;
        };
        for c in step.iter().rev() {
            self.apply(&c.inverse());
        }
        self.redo.push(step);
        self.revision += 1;
        true
    }

    /// Redoes the last undone step. False if there is nothing to redo (any new edit clears the redo list).
    pub fn redo(&mut self) -> bool {
        self.end_stroke();
        let Some(step) = self.redo.pop() else {
            return false;
        };
        for c in &step {
            self.apply(c);
        }
        self.push_undo(step);
        self.revision += 1;
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.stroke.as_ref().is_some_and(|s| !s.changes.is_empty())
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// A stroke is open (a paint or erase drag): the design may change again next frame.
    pub fn in_stroke(&self) -> bool {
        self.stroke.is_some()
    }

    /// How many of `part` the player has to build with (`career::Career::available`, GDR-0082): the design may
    /// hold that many, and an edit that would put down more is refused with [`Refusal::NoneLeft`] (a mirror
    /// image that would is skipped; fill fills as many cells as are left). Not an edit: no undo step, no revision
    /// change; parts already placed stay, and undo and redo always restore their step.
    pub fn set_available(&mut self, part: &str, available: Availability) {
        match available {
            Availability::Unlimited => self.limits.remove(part),
            Availability::Count(n) => self.limits.insert(part.to_string(), n),
        };
    }

    /// How many more of `part` the design may take: the count of [`Drydock::set_available`] less the parts of it
    /// placed (0 when the design holds more), or unlimited.
    pub fn left(&self, part: &str) -> Availability {
        match self.limits.get(part) {
            None => Availability::Unlimited,
            Some(&n) => {
                let used = self.parts.iter().filter(|p| p.part == part).count();
                Availability::Count(n.saturating_sub(u32::try_from(used).unwrap_or(u32::MAX)))
            }
        }
    }
}
