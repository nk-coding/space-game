//! The editing model's state (part of [`Drydock`]'s impl): executing and undoing changes, the undo stack, the
//! cell grid of placed parts, centring in a new frame.

use super::*;

impl Drydock {
    pub(super) fn exec(&mut self, change: Change, log: &mut Vec<Change>) {
        self.apply(&change);
        log.push(change);
    }

    /// Records the applied changes of one edit (into the open stroke, or as a new undo step).
    pub(super) fn finish(&mut self, log: Vec<Change>, part: Option<usize>, twin: Twin) -> Outcome {
        let changed = !log.is_empty();
        if changed {
            self.revision += 1;
            self.redo.clear();
            match &mut self.stroke {
                Some(stroke) => stroke.changes.extend(log),
                None => self.push_undo(log),
            }
        }
        Outcome {
            changed,
            part,
            twin,
        }
    }

    pub(super) fn push_undo(&mut self, step: Vec<Change>) {
        if self.undo.len() == UNDO_STEPS {
            self.undo.pop_front();
        }
        self.undo.push_back(step);
    }

    pub(super) fn apply(&mut self, change: &Change) {
        match change {
            Change::Insert { index, placement } => {
                let i = *index;
                if i < self.parts.len() {
                    self.renumber(i, true);
                }
                self.shapes.insert(i, self.shape_of(placement));
                self.parts.insert(i, placement.clone());
                self.stamp(i);
            }
            Change::Remove { index, .. } => {
                let i = *index;
                self.unstamp(i);
                self.parts.remove(i);
                self.shapes.remove(i);
                if i < self.parts.len() {
                    self.renumber(i + 1, false);
                }
            }
            Change::Set { index, new, .. } => {
                let i = *index;
                self.unstamp(i);
                self.shapes[i] = self.shape_of(new);
                self.parts[i] = new.clone();
                self.stamp(i);
            }
            Change::Frame { new, shift, .. } => {
                self.frame = *new;
                self.shift_parts(*shift);
                self.rebuild()
                    .expect("a frame change keeps the parts apart");
            }
        }
    }

    /// The shift that centres the design's bounding box in `frame`; `None` if it does not fit. In `i64`: a
    /// blueprint being opened may lie anywhere in the `i32` range.
    pub(super) fn centring(&self, (fw, fh): (i32, i32)) -> Option<(i64, i64)> {
        let Some([x0, y0, x1, y1]) = span(&self.shapes) else {
            return Some((0, 0));
        };
        let (w, h, fw, fh) = (x1 - x0, y1 - y0, fw as i64, fh as i64);
        if w > fw || h > fh {
            return None;
        }
        Some(((fw - w) / 2 - x0, (fh - h) / 2 - y0))
    }

    /// Moves every part by a [`Drydock::centring`] shift, which puts them all inside the frame.
    pub(super) fn shift_parts(&mut self, (dx, dy): (i64, i64)) {
        let to = |v: i32, d: i64| (v as i64 + d) as i32;
        for (p, s) in self.parts.iter_mut().zip(&mut self.shapes) {
            (p.x, p.y) = (to(p.x, dx), to(p.y, dy));
            (s.x, s.y) = (to(s.x, dx), to(s.y, dy));
        }
    }

    /// Rebuilds the occupancy grid; `Err((part, other))` if two parts share a cell.
    pub(super) fn rebuild(&mut self) -> Result<(), (usize, usize)> {
        let n = (self.frame.0 * self.frame.1) as usize;
        self.occ = vec![NONE; n];
        self.sides = vec![0; n];
        for i in 0..self.shapes.len() {
            if let Some(k) = self.shapes[i]
                .cells()
                .into_iter()
                .find_map(|c| self.part_at(c))
            {
                return Err((i, k));
            }
            self.stamp(i);
        }
        Ok(())
    }

    pub(super) fn slot(&self, (x, y): Cell) -> Option<usize> {
        let (w, h) = self.frame;
        (x >= 0 && y >= 0 && x < w && y < h).then(|| (y * w + x) as usize)
    }

    /// Does the part in `cell` have a straight side on `dir`?
    pub(super) fn straight(&self, cell: Cell, dir: Dir) -> bool {
        self.slot(cell)
            .is_some_and(|k| self.occ[k] != NONE && self.sides[k] & dir.bit() != 0)
    }

    pub(super) fn stamp(&mut self, i: usize) {
        let s = self.shapes[i];
        for c in s.cells() {
            let k = self.slot(c).expect("parts stay inside the frame");
            self.occ[k] = i as u32;
            self.sides[k] = 0;
        }
        for (c, d) in s.connectable_sides() {
            let k = self.slot(c).expect("parts stay inside the frame");
            self.sides[k] |= d.bit();
        }
    }

    pub(super) fn unstamp(&mut self, i: usize) {
        for c in self.shapes[i].cells() {
            let k = self.slot(c).expect("parts stay inside the frame");
            self.occ[k] = NONE;
            self.sides[k] = 0;
        }
    }

    /// Moves every grid index `>= from` up or down by one (a part inserted or removed before them).
    pub(super) fn renumber(&mut self, from: usize, up: bool) {
        let from = from as u32;
        for o in &mut self.occ {
            if *o != NONE && *o >= from {
                *o = if up { *o + 1 } else { *o - 1 };
            }
        }
    }
}
