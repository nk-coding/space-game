//! The pages of behavior nodes (the vocabulary) and behaviors, with their rules drawn as calls.

use super::*;

impl Wiki<'_> {
    pub(crate) fn node_page(&self, n: &BehaviorNode) -> String {
        let root = UP;
        let mut extra = vec![role_badge(n.role)];
        if n.script.is_some() {
            extra.push("<span class=\"badge rhai\">Rhai</span>".into());
        }
        let mut facts = vec![("Role".to_string(), role_name(n.role).to_string())];
        if n.pilot {
            facts.push(("Used in".into(), "pilot assists only (a design's PILOT ASSISTS list, GDR-0088)".into()));
        }
        if let Some(q) = n.value {
            facts.push((
                "Value".into(),
                format!(
                    "{} (compare it: <code>{} &lt; {}</code>)",
                    q.describe(),
                    esc(&n.id),
                    q.example()
                ),
            ));
        }
        facts.push(("Written".into(), format!("<code>{}</code>", esc(&signature(n)))));
        let mut sections = Vec::new();
        if !n.params.is_empty() {
            let mut t = String::from(
                "<table class=\"params\">\n<tr><th>Parameter</th><th>Type</th><th>Default</th><th>Range</th><th></th></tr>\n",
            );
            for p in &n.params {
                let ty = match &p.ty {
                    ParamType::Distance => "Distance",
                    ParamType::Percent => "Percent",
                    ParamType::Seconds => "Seconds",
                    ParamType::Count => "Count",
                    ParamType::Choice(_) => "Choice",
                };
                let range = match (p.min, p.max) {
                    (None, None) => String::new(),
                    (lo, hi) => format!(
                        "{}–{}{}",
                        lo.map_or(String::new(), |x| format_number(x, 3)),
                        hi.map_or(String::new(), |x| format_number(x, 3)),
                        p.ty.unit_suffix()
                    ),
                };
                let mut about = p
                    .summary
                    .as_deref()
                    .map_or(String::new(), |s| self.text(root, s));
                if let ParamType::Choice(opts) = &p.ty {
                    let items: Vec<String> = opts
                        .iter()
                        .map(|o| {
                            let s = o
                                .summary
                                .as_deref()
                                .map_or(String::new(), |s| format!(": {}", self.text(root, s)));
                            format!("<code>{}</code> {}{s}", esc(&o.id), esc(&o.name))
                        })
                        .collect();
                    about.push_str(&ul(&items));
                }
                writeln!(
                    t,
                    "<tr><td>{} <code>{}</code></td><td>{ty}</td><td>{}</td><td>{}</td><td>{about}</td></tr>",
                    esc(&p.name),
                    esc(&p.id),
                    p.default.as_deref().map_or("required".into(), |d| format!("<code>{}</code>", esc(d))),
                    esc(&range)
                )
                .unwrap();
            }
            t.push_str("</table>\n");
            sections.push(("Parameters".to_string(), t));
        }
        if let Some(script) = &n.script {
            sections.push((
                "Rhai script".into(),
                format!(
                    "<pre><code class=\"lang-rhai\">{}</code></pre>\n",
                    esc(script)
                ),
            ));
        }
        let used: Vec<String> = self
            .c
            .behaviors
            .iter()
            .filter_map(|b| {
                let uses = node_uses(b).iter().filter(|(id, _)| *id == n.id).count();
                (uses > 0).then(|| format!("{} ×{uses}", self.a(root, Kind::Behavior, &b.id)))
            })
            .collect();
        sections.push(("Used by behaviors".into(), list_or_none(&used)));
        self.entry(
            n,
            EntryParts {
                badges: badges(false, &extra),
                facts,
                sections,
                ..Default::default()
            },
        )
    }

    pub(crate) fn behavior_page(&self, b: &Behavior) -> String {
        let root = UP;
        let mut facts = Vec::new();
        let mut sections = Vec::new();
        match b.program() {
            Ok(program) => {
                facts.push(("Rules".to_string(), program.rules.len().to_string()));
                facts.push(("Rule slots needed".into(), program.rules.len().to_string()));
                sections.push((
                    "Rules".to_string(),
                    self.rules_figure(root, &program, data::behavior::ListKind::Behavior),
                ));
            }
            Err(errors) => {
                for e in errors {
                    self.warn(format!("behavior `{}`: {e}", b.id));
                }
            }
        }
        let mut seen = BTreeSet::new();
        let mut by_role: BTreeMap<NodeRole, Vec<(Kind, String)>> = BTreeMap::new();
        for (id, _) in node_uses(b) {
            if let Some(n) = self.c.behavior_nodes.get(&id)
                && seen.insert(id.clone())
            {
                by_role
                    .entry(n.role)
                    .or_default()
                    .push((Kind::BehaviorNode, id));
            }
        }
        let uses: Vec<String> = ROLES
            .iter()
            .filter_map(|role| {
                let ids = by_role.remove(role)?;
                Some(format!(
                    "{}: {}",
                    role_name(*role),
                    self.links(root, ids).unwrap_or_default()
                ))
            })
            .collect();
        sections.push(("Nodes it uses".to_string(), list_or_none(&uses)));
        let factions = self
            .c
            .factions
            .iter()
            .filter(|f| f.behavior.as_deref() == Some(b.id.as_str()))
            .map(|f| (Kind::Faction, f.id.clone()));
        if let Some(l) = self.links(root, factions) {
            facts.push(("Default of".into(), l));
        }
        let ships: Vec<&Blueprint> = self
            .c
            .ships
            .iter()
            .filter(|s| self.c.ship_behavior(s).is_some_and(|x| x.id == b.id))
            .collect();
        sections.push(("Flown by ships".into(), self.ship_cards(root, &ships)));
        self.entry(
            b,
            EntryParts {
                facts,
                sections,
                ..Default::default()
            },
        )
    }

    /// A rule list drawn as the GRAPH view draws it, and as its text, one line per rule with every node linked in its
    /// role's colour (what the TEXT view shows).
    pub(crate) fn rules_figure(
        &self,
        root: &str,
        program: &data::behavior::Program,
        kind: data::behavior::ListKind,
    ) -> String {
        format!(
            "<figure class=\"rule-figure\">\n{}{}</figure>\n",
            graph::draw(self.c, program, kind),
            self.rules_text(root, program, kind)
        )
    }

    /// The rule text with links: the target line (or the default one; none for pilot assists), then one line per rule.
    pub(crate) fn rules_text(
        &self,
        root: &str,
        program: &data::behavior::Program,
        kind: data::behavior::ListKind,
    ) -> String {
        let mut s = String::from("<div class=\"rules\">\n");
        // Pilot assists have no target line: they act on the pilot's lock (GDR-0088).
        if kind != data::behavior::ListKind::Assists {
            let tline = match program.target.as_ref() {
                Some(t) => format!(
                    "{}{}",
                    self.call(root, &t.call, NodeRole::Target),
                    note(&t.note)
                ),
                None => format!(
                    "{} <span class=\"note\"># default</span>",
                    self.call_named(root, "nearest", "", NodeRole::Target)
                ),
            };
            writeln!(
                s,
                "<p class=\"rule target\"><span class=\"kw\">target</span> {tline}</p>"
            )
            .unwrap();
        }
        s.push_str(if kind == data::behavior::ListKind::Assists { "<ul>\n" } else { "<ol>\n" });
        for r in &program.rules {
            let mut line = String::new();
            if r.when.is_empty() {
                line.push_str("<span class=\"kw\">always</span>");
            } else {
                line.push_str("<span class=\"kw\">when</span> ");
                for (k, cond) in r.when.iter().enumerate() {
                    if k > 0 {
                        line.push_str(" <span class=\"kw\">and</span> ");
                    }
                    if cond.not {
                        line.push_str("<span class=\"kw\">not</span> ");
                    }
                    line.push_str(&self.call(root, &cond.call, NodeRole::Condition));
                    if let Some(cmp) = &cond.cmp {
                        write!(
                            line,
                            " {} {}",
                            esc(cmp.op.symbol()),
                            esc(&cmp.rhs.to_string())
                        )
                        .unwrap();
                    }
                }
            }
            write!(
                line,
                " <span class=\"kw\">-&gt;</span> {}{}",
                self.call(root, &r.action, NodeRole::Action),
                note(&r.note)
            )
            .unwrap();
            writeln!(s, "<li class=\"rule\">{line}</li>").unwrap();
        }
        s.push_str(if kind == data::behavior::ListKind::Assists { "</ul>\n" } else { "</ol>\n" });
        s + "</div>\n"
    }

    /// A node call in a rule: the name linked to the node's page in its role's colour, then the arguments.
    pub(crate) fn call(&self, root: &str, call: &data::behavior::Call, role: NodeRole) -> String {
        let text = call.to_string();
        self.call_named(root, &call.name, &text[call.name.len()..], role)
    }

    pub(crate) fn call_named(&self, root: &str, name: &str, args: &str, role: NodeRole) -> String {
        let class = role_class(role);
        if self.c.behavior_nodes.contains(name) {
            format!(
                "<a class=\"{class}\" href=\"{root}{}\">{}</a>{}",
                item_path(Kind::BehaviorNode, name),
                esc(name),
                esc(args)
            )
        } else {
            format!("<span class=\"{class}\">{}</span>{}", esc(name), esc(args))
        }
    }
}

/// How a node is written: `orbit(radius, dir)`, or just its id without parameters.
pub(crate) fn signature(n: &BehaviorNode) -> String {
    let params: Vec<&str> = n.params.iter().map(|p| p.id.as_str()).collect();
    if params.is_empty() {
        n.id.clone()
    } else {
        format!("{}({})", n.id, params.join(", "))
    }
}

/// Node roles in the editor's order: the target line, then conditions, then actions.
pub(crate) const ROLES: [NodeRole; 3] = [NodeRole::Target, NodeRole::Condition, NodeRole::Action];

pub(crate) fn role_name(role: NodeRole) -> &'static str {
    match role {
        NodeRole::Condition => "Condition",
        NodeRole::Action => "Action",
        NodeRole::Target => "Target selector",
    }
}

/// The heading of a role's nodes in lists.
pub(crate) fn role_title(role: NodeRole) -> &'static str {
    match role {
        NodeRole::Target => "Target selectors",
        NodeRole::Condition => "Conditions",
        NodeRole::Action => "Actions",
    }
}

pub(crate) fn role_class(role: NodeRole) -> &'static str {
    match role {
        NodeRole::Condition => "cond",
        NodeRole::Action => "act",
        NodeRole::Target => "tgt",
    }
}

pub(crate) fn role_badge(role: NodeRole) -> String {
    format!(
        "<span class=\"badge {}\">{}</span>",
        role_class(role),
        role_name(role)
    )
}

pub(crate) fn note(n: &Option<String>) -> String {
    n.as_ref().map_or(String::new(), |n| {
        format!(" <span class=\"note\"># {}</span>", esc(n))
    })
}

/// Every node a behavior names, in reading order: `(id, role it is used in)`; the default target counts.
pub(crate) fn node_uses(b: &Behavior) -> Vec<(String, NodeRole)> {
    let Ok(p) = b.program() else {
        return Vec::new();
    };
    let mut v = vec![(
        p.target
            .as_ref()
            .map_or("nearest".to_string(), |t| t.call.name.clone()),
        NodeRole::Target,
    )];
    for r in &p.rules {
        v.extend(
            r.when
                .iter()
                .map(|c| (c.call.name.clone(), NodeRole::Condition)),
        );
        v.push((r.action.name.clone(), NodeRole::Action));
    }
    v
}
