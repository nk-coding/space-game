//! Validation of behaviors (part of [`Validator`]'s impl): the behavior nodes (the vocabulary), the behaviors written
//! in it, and references to them.

use super::*;

impl<'a> Validator<'a> {
    pub(super) fn behavior_ref<T: Entry>(&mut self, e: &T, id: &str) {
        if !self.c.behaviors.contains(id) {
            let mut msg = format!("unknown behavior `{id}`");
            if let Some(s) =
                crate::behavior::suggest(id, self.c.behaviors.iter().map(|b| b.id.as_str()))
            {
                msg += &format!("; did you mean `{s}`?");
            }
            self.err(e, "behavior", msg);
        }
    }

    pub(super) fn behavior_node(&mut self, n: &'a BehaviorNode) {
        use crate::behavior::{KEYWORDS, MAX_ARGS, check_param_value, parse_value};
        if KEYWORDS.contains(&n.id.as_str()) {
            self.err(
                n,
                "id",
                format!("`{}` is a keyword of the behavior language", n.id),
            );
        }
        if n.value.is_some() && n.role != NodeRole::Condition {
            self.err(n, "value", "only conditions yield a value");
        }
        if let Some(script) = &n.script {
            if n.role != NodeRole::Condition {
                self.err(n, "script", "only conditions can be scripted (for now)");
            } else if n.value.is_some() {
                self.err(
                    n,
                    "script",
                    "a scripted condition must be a flag (no `value`) for now",
                );
            }
            if script.trim().is_empty() {
                self.err(n, "script", "empty script");
            }
        }
        if n.params.len() > MAX_ARGS {
            self.err(n, "params", format!("at most {MAX_ARGS} parameters"));
        }
        let mut seen = BTreeSet::new();
        let mut optional = false;
        for (k, p) in n.params.iter().enumerate() {
            let f = format!("params[{k}]");
            if !is_snake_id(&p.id) || KEYWORDS.contains(&p.id.as_str()) {
                self.err(
                    n,
                    format!("{f}.id"),
                    format!(
                        "parameter id `{}` must be snake_case and not a keyword",
                        p.id
                    ),
                );
            }
            if !seen.insert(p.id.as_str()) {
                self.err(
                    n,
                    format!("{f}.id"),
                    format!("parameter `{}` is declared twice", p.id),
                );
            }
            if p.name.trim().is_empty() {
                self.err(n, format!("{f}.name"), "name must not be empty");
            }
            for (field, text) in [
                ("name", Some(p.name.as_str())),
                ("summary", p.summary.as_deref()),
            ] {
                for problem in text.map(|t| self.link_problems(t)).unwrap_or_default() {
                    self.err(n, format!("{f}.{field}"), problem);
                }
            }
            if let ParamType::Choice(opts) = &p.ty {
                if opts.is_empty() {
                    self.err(n, format!("{f}.type"), "a choice needs at least one option");
                }
                let mut ids = BTreeSet::new();
                for (j, o) in opts.iter().enumerate() {
                    if !is_snake_id(&o.id) || KEYWORDS.contains(&o.id.as_str()) {
                        self.err(
                            n,
                            format!("{f}.type[{j}].id"),
                            format!("option id `{}` must be snake_case and not a keyword", o.id),
                        );
                    }
                    if !ids.insert(o.id.as_str()) {
                        self.err(
                            n,
                            format!("{f}.type[{j}].id"),
                            format!("option `{}` is listed twice", o.id),
                        );
                    }
                    if o.name.trim().is_empty() {
                        self.err(n, format!("{f}.type[{j}].name"), "name must not be empty");
                    }
                }
                if p.min.is_some() || p.max.is_some() {
                    self.err(n, &f, "`min` and `max` are for numeric parameters");
                }
            }
            for (field, x) in [("min", p.min), ("max", p.max)] {
                if let Some(x) = x
                    && !x.is_finite()
                {
                    self.err(n, format!("{f}.{field}"), "must be a finite number");
                }
            }
            if let (Some(lo), Some(hi)) = (p.min, p.max)
                && lo > hi
            {
                self.err(n, &f, format!("min {lo} is above max {hi}"));
            }
            match &p.default {
                Some(d) => {
                    optional = true;
                    match parse_value(d).and_then(|v| check_param_value(&v, p)) {
                        Ok(_) => {}
                        Err(m) => self.err(n, format!("{f}.default"), m),
                    }
                }
                None if optional => self.err(
                    n,
                    format!("{f}.default"),
                    "parameters without a default must come before those with one",
                ),
                None => {}
            }
        }
    }

    pub(super) fn behavior(&mut self, b: &'a Behavior) {
        let program = match b.program() {
            Ok(p) => p,
            Err(errors) => {
                for e in errors {
                    self.err(
                        b,
                        e.line.field(),
                        format!("column {}: {}", e.col, e.message),
                    );
                }
                return;
            }
        };
        for e in crate::behavior::check_program(&program, &self.c.behavior_nodes) {
            let msg = match e.line {
                crate::behavior::LineRef::Whole => e.message,
                _ => format!("column {}: {}", e.col, e.message),
            };
            self.err(b, e.line.field(), msg);
        }
    }
}
