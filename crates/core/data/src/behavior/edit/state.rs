//! The behavior editor's state (part of [`BehaviorEditor`]'s impl): undo and redo, and the internals every edit goes
//! through (changes, rule ids, the revision).

use super::*;

impl BehaviorEditor {
    /// Opens a stroke: every edit until [`BehaviorEditor::end_stroke`] becomes one undo step (a slider drag
    /// in the inspector). An open stroke is closed first; undo and redo close it too.
    pub fn begin_stroke(&mut self) {
        self.end_stroke();
        self.stroke = Some(Vec::new());
    }

    /// Closes the open stroke, if any.
    pub fn end_stroke(&mut self) {
        if let Some(step) = self.stroke.take()
            && !step.is_empty()
        {
            self.push_undo(step);
        }
    }

    /// Undoes the last step (closing an open stroke first). False if there is nothing to undo. Keeps the last
    /// [`UNDO_STEPS`] steps.
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
        debug_assert!(self.is_legal());
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
        debug_assert!(self.is_legal());
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.stroke.as_ref().is_some_and(|s| !s.is_empty())
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    // ---- internals ----

    pub(super) fn view(&self, index: usize) -> RuleView<'_> {
        RuleView {
            id: self.ids[index].rule,
            index,
            rule: &self.program.rules[index],
            conds: &self.ids[index].conds,
        }
    }

    pub(super) fn entry(&self, index: usize) -> Entry {
        Entry {
            ids: self.ids[index].clone(),
            rule: self.program.rules[index].clone(),
        }
    }

    pub(super) fn index_of(&self, id: RuleId) -> Result<usize, Refusal> {
        self.ids
            .iter()
            .position(|r| r.rule == id)
            .ok_or(Refusal::NoSuchRule(id))
    }

    pub(super) fn find_cond(&self, cond: CondId) -> Result<(usize, usize), Refusal> {
        self.ids
            .iter()
            .enumerate()
            .find_map(|(i, r)| r.conds.iter().position(|&c| c == cond).map(|j| (i, j)))
            .ok_or(Refusal::NoSuchCondition(cond))
    }

    pub(super) fn place(&self, at: NodeRef) -> Result<Place, Refusal> {
        match at {
            // Pilot assists have no target line: they act on the pilot's lock.
            NodeRef::Target if self.kind == ListKind::Assists => Err(Refusal::NotInList("target".into())),
            NodeRef::Target => Ok(Place::Target),
            NodeRef::Action(r) => self.index_of(r).map(Place::Action),
            NodeRef::Cond(c) => self.find_cond(c).map(|(i, j)| Place::Cond(i, j)),
        }
    }

    /// A copy of the call at a place; the default selector's (no values) for a program without a target line.
    pub(super) fn call(&self, place: Place) -> Call {
        match place {
            Place::Target => self.program.target.as_ref().map_or_else(
                || Call {
                    name: DEFAULT_TARGET.to_string(),
                    args: Vec::new(),
                    col: Col::default(),
                },
                |t| t.call.clone(),
            ),
            Place::Action(i) => self.program.rules[i].action.clone(),
            Place::Cond(i, j) => self.program.rules[i].when[j].call.clone(),
        }
    }

    /// The vocabulary entry of a call in the program (only the default target can be missing).
    pub(super) fn node_of(&self, call: &Call) -> Result<&BehaviorNode, Refusal> {
        self.nodes
            .get(&call.name)
            .ok_or_else(|| Refusal::UnknownNode(call.name.clone()))
    }

    pub(super) fn lookup(&self, id: &str, role: NodeRole) -> Result<&BehaviorNode, Refusal> {
        let node = self
            .nodes
            .get(id)
            .ok_or_else(|| Refusal::UnknownNode(id.to_string()))?;
        if node.role != role {
            return Err(Refusal::WrongRole {
                node: id.to_string(),
                expected: role,
            });
        }
        if !self.kind.allows(node) {
            return Err(Refusal::NotInList(id.to_string()));
        }
        Ok(node)
    }

    pub(super) fn fresh(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub(super) fn fresh_ids(&mut self, rule: &Rule) -> RuleIds {
        RuleIds {
            rule: RuleId(self.fresh()),
            conds: rule.when.iter().map(|_| CondId(self.fresh())).collect(),
        }
    }

    /// Ids for the conditions `new` of a rule that had `old` with `ids`: unchanged leading and trailing
    /// conditions keep theirs, changed ones between them keep theirs by position, added ones get new ids.
    pub(super) fn cond_ids(&mut self, old: &[Cond], ids: &[CondId], new: &[Cond]) -> Vec<CondId> {
        let (pre, suf) = common_ends(old, new);
        let old_mid = old.len() - pre - suf;
        (0..new.len())
            .map(|j| {
                if j < pre || (j < pre + old_mid && j < new.len() - suf) {
                    ids[j]
                } else if j >= new.len() - suf {
                    ids[old.len() - (new.len() - j)]
                } else {
                    CondId(self.fresh())
                }
            })
            .collect()
    }

    /// Stores a call at its place as one edit (the target line keeps its note).
    pub(super) fn commit_call(&mut self, place: Place, call: Call) -> Result<bool, Refusal> {
        match place {
            // The default selector stays implicit until it changes.
            Place::Target if self.program.target.is_none() && call == self.call(place) => Ok(false),
            Place::Target => {
                let note = self.program.target.as_ref().and_then(|t| t.note.clone());
                self.commit_target(Some(TargetLine { call, note }))
            }
            Place::Action(i) => {
                let mut e = self.entry(i);
                e.rule.action = call;
                self.commit_rule(i, e)
            }
            Place::Cond(i, j) => {
                let mut e = self.entry(i);
                e.rule.when[j].call = call;
                self.commit_rule(i, e)
            }
        }
    }

    /// Replaces rule `index` by an edited copy as one edit, if it keeps the limits.
    pub(super) fn commit_rule(&mut self, index: usize, new: Entry) -> Result<bool, Refusal> {
        if new.rule.when.len() > MAX_CONDITIONS {
            return Err(Refusal::TooManyConditions);
        }
        fits_line(&new.rule)?;
        let mut log = Vec::new();
        if new.rule != self.program.rules[index] || new.ids != self.ids[index] {
            let old = self.entry(index);
            self.exec(Change::Set { index, old, new }, &mut log);
        }
        Ok(self.finish(log))
    }

    pub(super) fn commit_target(&mut self, new: Option<TargetLine>) -> Result<bool, Refusal> {
        if let Some(t) = &new {
            fits_line(t)?;
        }
        let mut log = Vec::new();
        if new != self.program.target {
            let old = self.program.target.clone();
            self.exec(Change::Target { old, new }, &mut log);
        }
        Ok(self.finish(log))
    }

    pub(super) fn exec(&mut self, change: Change, log: &mut Vec<Change>) {
        self.apply(&change);
        log.push(change);
    }

    /// Records the applied changes of one edit (into the open stroke, or as a new undo step). Returns whether
    /// anything changed.
    pub(super) fn finish(&mut self, log: Vec<Change>) -> bool {
        let changed = !log.is_empty();
        if changed {
            self.revision += 1;
            self.redo.clear();
            match &mut self.stroke {
                Some(stroke) => stroke.extend(log),
                None => self.push_undo(log),
            }
            debug_assert!(self.is_legal());
        }
        changed
    }

    pub(super) fn push_undo(&mut self, step: Vec<Change>) {
        if self.undo.len() == UNDO_STEPS {
            self.undo.pop_front();
        }
        self.undo.push_back(step);
    }

    pub(super) fn apply(&mut self, change: &Change) {
        match change {
            Change::Insert { index, entry } => {
                self.program.rules.insert(*index, entry.rule.clone());
                self.ids.insert(*index, entry.ids.clone());
            }
            Change::Remove { index, .. } => {
                self.program.rules.remove(*index);
                self.ids.remove(*index);
            }
            Change::Set { index, new, .. } => {
                self.program.rules[*index] = new.rule.clone();
                self.ids[*index] = new.ids.clone();
            }
            Change::Move { from, to } => {
                let rule = self.program.rules.remove(*from);
                self.program.rules.insert(*to, rule);
                let ids = self.ids.remove(*from);
                self.ids.insert(*to, ids);
            }
            Change::Target { new, .. } => self.program.target = new.clone(),
        }
    }

    /// The invariant (checked after every change in debug builds): the program checks against the
    /// vocabulary, survives its text form, and the ids match its shape.
    pub(super) fn is_legal(&self) -> bool {
        check_list(&self.program, &self.nodes, self.kind).is_empty()
            && Program::parse(&self.program.to_text()).is_ok_and(|p| p == self.program)
            && self.ids.len() == self.program.rules.len()
            && self
                .ids
                .iter()
                .zip(&self.program.rules)
                .all(|(ids, r)| ids.conds.len() == r.when.len())
    }
}
