//! The behavior editor's edits (part of [`BehaviorEditor`]'s impl): rules, conditions, actions, the target, notes and
//! the text; each one undo step, refused with a reason when illegal.

use super::*;

impl BehaviorEditor {
    /// Replaces the program with a text (the text view). Refused with every syntax and vocabulary error.
    /// Unchanged rules and conditions keep their ids: the leading and trailing rules equal to the old ones,
    /// and rules edited in place between them, by position (a condition likewise within its rule); added
    /// ones get new ids. One undo step; `Ok(false)` if the text gives the same program.
    pub fn set_text(&mut self, text: &str) -> Result<bool, Vec<TextError>> {
        let new = self.check_text(text)?;
        let mut log = Vec::new();
        if new.target != self.program.target {
            let old = self.program.target.clone();
            self.exec(
                Change::Target {
                    old,
                    new: new.target,
                },
                &mut log,
            );
        }
        let (pre, suf) = common_ends(&self.program.rules, &new.rules);
        let old_mid = self.program.rules.len() - pre - suf;
        let new_mid = new.rules.len() - pre - suf;
        let paired = old_mid.min(new_mid);
        let mut incoming = new.rules.into_iter().skip(pre);
        for (k, rule) in (0..paired).zip(incoming.by_ref()) {
            let index = pre + k;
            if self.program.rules[index] != rule {
                let old = self.entry(index);
                let conds = self.cond_ids(&old.rule.when, &old.ids.conds, &rule.when);
                let ids = RuleIds {
                    rule: old.ids.rule,
                    conds,
                };
                let new = Entry { ids, rule };
                self.exec(Change::Set { index, old, new }, &mut log);
            }
        }
        for _ in paired..old_mid {
            let index = pre + paired;
            let entry = self.entry(index);
            self.exec(Change::Remove { index, entry }, &mut log);
        }
        for (k, rule) in (paired..new_mid).zip(incoming) {
            let ids = self.fresh_ids(&rule);
            let entry = Entry { ids, rule };
            self.exec(
                Change::Insert {
                    index: pre + k,
                    entry,
                },
                &mut log,
            );
        }
        Ok(self.finish(log))
    }

    /// Adds `always -> action` at a priority position (0 = top, the number of rules = bottom). Required
    /// parameters of the action get starting values (see [`BehaviorEditor::add_condition`]).
    pub fn add_rule(&mut self, index: usize, action: &str) -> Result<RuleId, Refusal> {
        if index > self.program.rules.len() {
            return Err(Refusal::BadIndex(index));
        }
        if self.program.rules.len() >= MAX_RULES {
            return Err(Refusal::TooManyRules);
        }
        let action = new_call(self.lookup(action, NodeRole::Action)?)?;
        let rule = Rule {
            when: Vec::new(),
            action,
            note: None,
        };
        fits_line(&rule)?;
        let ids = self.fresh_ids(&rule);
        let id = ids.rule;
        let mut log = Vec::new();
        self.exec(
            Change::Insert {
                index,
                entry: Entry { ids, rule },
            },
            &mut log,
        );
        self.finish(log);
        Ok(id)
    }

    /// Deletes a rule; refused for the last one.
    pub fn delete_rule(&mut self, id: RuleId) -> Result<bool, Refusal> {
        let index = self.index_of(id)?;
        if self.program.rules.len() == 1 {
            return Err(Refusal::LastRule);
        }
        let entry = self.entry(index);
        let mut log = Vec::new();
        self.exec(Change::Remove { index, entry }, &mut log);
        Ok(self.finish(log))
    }

    /// Copies a rule (with its note) right below itself, with new ids. Returns the copy's id.
    pub fn duplicate_rule(&mut self, id: RuleId) -> Result<RuleId, Refusal> {
        let index = self.index_of(id)?;
        if self.program.rules.len() >= MAX_RULES {
            return Err(Refusal::TooManyRules);
        }
        let rule = self.program.rules[index].clone();
        let ids = self.fresh_ids(&rule);
        let copy = ids.rule;
        let mut log = Vec::new();
        self.exec(
            Change::Insert {
                index: index + 1,
                entry: Entry { ids, rule },
            },
            &mut log,
        );
        self.finish(log);
        Ok(copy)
    }

    /// Moves a rule to a priority position (the others close up): up is `index - 1`, down `index + 1`.
    pub fn move_rule(&mut self, id: RuleId, to: usize) -> Result<bool, Refusal> {
        let from = self.index_of(id)?;
        if to >= self.program.rules.len() {
            return Err(Refusal::BadIndex(to));
        }
        let mut log = Vec::new();
        if from != to {
            self.exec(Change::Move { from, to }, &mut log);
        }
        Ok(self.finish(log))
    }

    /// Sets or clears a rule's `# note`. The note is trimmed; an empty one clears it.
    pub fn set_rule_note(&mut self, id: RuleId, note: Option<&str>) -> Result<bool, Refusal> {
        let index = self.index_of(id)?;
        let mut e = self.entry(index);
        e.rule.note = clean_note(note)?;
        self.commit_rule(index, e)
    }

    /// Adds a condition at the end of a rule's conditions (`always -> x` becomes `when c -> x`). Its parameters
    /// are left to their defaults; a required one (no default) gets a starting value: the first option of a
    /// choice, else the parameter's `min` (or 0) in its unit. A value condition is compared with `<` and the
    /// example of its quantity (`hull < 30%`, `enemies_near < 2`, `cycle < 5s`).
    pub fn add_condition(&mut self, rule: RuleId, node: &str) -> Result<CondId, Refusal> {
        let index = self.index_of(rule)?;
        let node = self.lookup(node, NodeRole::Condition)?;
        let cond = Cond {
            not: false,
            call: new_call(node)?,
            cmp: node.value.map(|q| default_compare(q, CmpOp::Lt)),
        };
        let mut e = self.entry(index);
        e.rule.when.push(cond);
        let id = CondId(self.fresh());
        e.ids.conds.push(id);
        self.commit_rule(index, e)?;
        Ok(id)
    }

    /// Removes a condition (a rule without conditions is `always -> x`).
    pub fn remove_condition(&mut self, cond: CondId) -> Result<bool, Refusal> {
        let (i, j) = self.find_cond(cond)?;
        let mut e = self.entry(i);
        e.rule.when.remove(j);
        e.ids.conds.remove(j);
        self.commit_rule(i, e)
    }

    /// Turns a condition around: `not` in front, or not.
    pub fn toggle_not(&mut self, cond: CondId) -> Result<bool, Refusal> {
        let (i, j) = self.find_cond(cond)?;
        let mut e = self.entry(i);
        e.rule.when[j].not ^= true;
        self.commit_rule(i, e)
    }

    /// Sets the comparison of a value condition (`hull < 30%`): the comparator and a threshold of the
    /// condition's quantity, at least 0 (a percentage needs `%`, a count no unit, see
    /// [`crate::behavior::quantity_amount`]).
    pub fn set_compare(&mut self, cond: CondId, op: CmpOp, rhs: Value) -> Result<bool, Refusal> {
        let (i, j) = self.find_cond(cond)?;
        let name = &self.program.rules[i].when[j].call.name;
        let q = self
            .nodes
            .get(name)
            .and_then(|n| n.value)
            .ok_or_else(|| Refusal::NotAValue(name.clone()))?;
        check_threshold(&rhs, q)?;
        let mut e = self.entry(i);
        e.rule.when[j].cmp = Some(Compare { op, rhs: arg(rhs) });
        self.commit_rule(i, e)
    }

    /// Replaces a node by another of the same role (a condition, an action or the target selector). The
    /// leading values that fit the new node's parameters are kept, the rest are dropped (defaults apply);
    /// required parameters past them get starting values. A condition keeps its `not`; a value condition keeps
    /// a comparison whose threshold fits its quantity, else gets the default threshold (the comparator stays);
    /// a flag drops the comparison. The target line keeps its note.
    pub fn replace_node(&mut self, at: NodeRef, node: &str) -> Result<bool, Refusal> {
        let place = self.place(at)?;
        let node = self.lookup(node, place.role())?;
        let old = self.call(place);
        let kept: Vec<Arg> = old
            .args
            .iter()
            .zip(&node.params)
            .take_while(|(a, p)| check_param_value(&a.value, p).is_ok())
            .map(|(a, _)| a.clone())
            .collect();
        let call = complete_call(node, kept)?;
        let Place::Cond(i, j) = place else {
            return self.commit_call(place, call);
        };
        let value = node.value;
        let mut e = self.entry(i);
        let c = &mut e.rule.when[j];
        c.cmp = match (value, c.cmp.take()) {
            (None, _) => None,
            (Some(q), Some(cmp)) if check_threshold(&cmp.rhs.value, q).is_ok() => Some(cmp),
            (Some(q), Some(cmp)) => Some(default_compare(q, cmp.op)),
            (Some(q), None) => Some(default_compare(q, CmpOp::Lt)),
        };
        c.call = call;
        self.commit_rule(i, e)
    }

    /// Sets parameter `index` of a node, checked against its type, unit and range
    /// ([`check_param_value`]). Values before it that were left to their defaults are written out (values
    /// are positional). On the default target selector this writes the `target` line.
    pub fn set_param(&mut self, at: NodeRef, index: usize, value: Value) -> Result<bool, Refusal> {
        let place = self.place(at)?;
        let mut call = self.call(place);
        let node = self.node_of(&call)?;
        let param = param_at(node, index)?;
        check_value(&value, param)?;
        for p in &node.params[call.args.len().min(index)..index] {
            let default = default_of(p).ok_or_else(|| no_default(node, p))?;
            call.args.push(arg(default));
        }
        match call.args.get_mut(index) {
            Some(a) => a.value = value,
            None => call.args.push(arg(value)),
        }
        self.commit_call(place, call)
    }

    /// Resets parameter `index` of a node to its default: the default is written in its place, then the
    /// trailing values that equal their defaults are dropped (the shortest text with the same meaning), so
    /// resetting the last written value removes it. `Ok(false)` if the value was not written. Refused for a
    /// required parameter (no default).
    pub fn reset_param(&mut self, at: NodeRef, index: usize) -> Result<bool, Refusal> {
        let place = self.place(at)?;
        let mut call = self.call(place);
        let node = self.node_of(&call)?;
        let param = param_at(node, index)?;
        let default = default_of(param).ok_or_else(|| no_default(node, param))?;
        let Some(a) = call.args.get_mut(index) else {
            return Ok(false);
        };
        a.value = default;
        while let Some(last) = call.args.last() {
            let p = &node.params[call.args.len() - 1];
            if !default_of(p).is_some_and(|d| same_amount(&last.value, &d, p)) {
                break;
            }
            call.args.pop();
        }
        self.commit_call(place, call)
    }

    /// Sets or clears the target line's `# note` (writing the `target` line if the program has none). The
    /// note is trimmed; an empty one clears it.
    pub fn set_target_note(&mut self, note: Option<&str>) -> Result<bool, Refusal> {
        let note = clean_note(note)?;
        if self.program.target.is_none() && note.is_none() {
            return Ok(false);
        }
        let call = self.call(Place::Target);
        self.node_of(&call)?;
        self.commit_target(Some(TargetLine { call, note }))
    }

    /// Removes the `target` line (and its note): the rules are about the default target, `nearest`.
    pub fn clear_target(&mut self) -> bool {
        let mut log = Vec::new();
        if let Some(old) = self.program.target.clone() {
            self.exec(
                Change::Target {
                    old: Some(old),
                    new: None,
                },
                &mut log,
            );
        }
        self.finish(log)
    }
}
