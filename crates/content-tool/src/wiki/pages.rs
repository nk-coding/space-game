//! The pages of the wiki: one per entry of every kind, one list per kind, and the index.

use super::*;

impl Wiki<'_> {
    pub(super) fn part_page(&self, p: &Part) -> String {
        let root = UP;
        let mut facts = vec![("Category".to_string(), {
            let dot = self
                .c
                .part_color(p)
                .map(|k| self.color_dot(k))
                .unwrap_or_default();
            format!("{dot}{}", self.a(root, Kind::Category, &p.category))
        })];
        let shape = if p.shape == data::Shape::Slope {
            " slope"
        } else {
            ""
        };
        facts.push(("Size".into(), format!("{} × {}{shape}", p.size.0, p.size.1)));
        if p.hull {
            facts.push(("Hull".into(), "built in any armor material".into()));
        }
        if let Some(pw) = p.power.as_ref().filter(|pw| pw.draw > 0.0) {
            let weight = if pw.weight != 1.0 {
                format!(", weight {}", format_number(pw.weight, 2))
            } else {
                String::new()
            };
            facts.push(("Power tier".into(), format!("{:?}{weight}", pw.tier)));
        }
        if let Some(w) = &p.weapon {
            let delivery = match w.delivery {
                Delivery::Beam => "beam",
                Delivery::Pulse { .. } => "pulses",
                Delivery::Projectile { .. } => "projectiles",
            };
            facts.push(("Weapon".into(), format!("{:?}, {delivery}", w.family)));
        }
        if let Some(sh) = p.shield.as_ref().filter(|s| !s.absorb.is_empty()) {
            let v: Vec<String> = sh
                .absorb
                .iter()
                .map(|(f, x)| format!("{f:?} {}%", format_number(x * 100.0, 0)))
                .collect();
            facts.push(("Absorbs".into(), v.join(", ")));
        }
        if let Some(icon) = &p.icon {
            facts.push(("Icon".into(), format!("<code>{}</code>", esc(icon))));
        }

        let lines = self.c.part_stats(p, None);
        let mut sections = Vec::new();
        let stats = if p.hull {
            sections.push((
                "Stats by material".to_string(),
                self.hull_stat_table(root, p),
            ));
            String::new()
        } else {
            self.stat_table(root, &Self::plain_lines(&lines))
        };
        let grants: Vec<String> = lines
            .iter()
            .filter(|l| l.bonus.is_some())
            .map(|l| self.bonus_text(root, l))
            .collect();
        if !grants.is_empty() {
            sections.push(("Adjacency bonuses".to_string(), ul(&grants)));
        }
        let mut receives = Vec::new();
        for q in &self.c.parts {
            for l in self
                .c
                .part_stats(q, None)
                .iter()
                .filter(|l| l.bonus.as_ref().is_some_and(|b| p.matches(&b.target)))
            {
                receives.push(format!(
                    "{}: <strong>{}</strong> {}",
                    self.a_badged(root, Kind::Part, &q.id),
                    esc(&l.text),
                    self.stat_label(root, &l.stat)
                ));
            }
        }
        if !receives.is_empty() {
            sections.push(("Gets bonuses from".to_string(), ul(&receives)));
        }
        let ships: Vec<String> = self
            .c
            .ships
            .iter()
            .filter_map(|s| {
                let n = s.parts.iter().filter(|pl| pl.part == p.id).count();
                (n > 0).then(|| format!("{} ×{n}", self.a(root, Kind::Ship, &s.id)))
            })
            .collect();
        sections.push(("Used by ships".to_string(), list_or_none(&ships)));

        let figure = if p.hull {
            let armor: Vec<String> = self
                .c
                .materials
                .iter()
                .filter(|m| m.armor.is_some())
                .map(|m| {
                    let svg =
                        self.part_svg(p, Some(m), CHIP_BOX * 1.4 / p.size.0.max(1) as f32, "part");
                    format!(
                        "<span class=\"swatch\">{svg}<span>{}{}</span></span>",
                        esc(&m.name),
                        if m.locked { LOCKED } else { "" }
                    )
                })
                .collect();
            format!("<div class=\"materials\">{}</div>", armor.join(""))
        } else {
            self.part_svg(p, None, PX_PART, "part")
        };
        self.entry(
            p,
            EntryParts {
                badges: badges(p.locked, &[]),
                figure: Some(figure),
                facts,
                stats,
                sections,
                ..Default::default()
            },
        )
    }

    /// Stats of a hull part per armour material: one column per material.
    fn hull_stat_table(&self, root: &str, p: &Part) -> String {
        let armor: Vec<&Material> = self
            .c
            .materials
            .iter()
            .filter(|m| m.armor.is_some())
            .collect();
        let cols: Vec<Lines> = armor
            .iter()
            .map(|m| Self::plain_lines(&self.c.part_stats(p, Some(m))))
            .collect();
        let mut s = String::from("<table class=\"stats by-material\">\n<tr><th></th>");
        for m in &armor {
            write!(s, "<th>{}</th>", self.a_badged(root, Kind::Material, &m.id)).unwrap();
        }
        s.push_str("</tr>\n");
        for (k, (stat, _)) in cols
            .first()
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .enumerate()
        {
            write!(s, "<tr><th>{}</th>", self.stat_label(root, stat)).unwrap();
            for col in &cols {
                write!(
                    s,
                    "<td>{}</td>",
                    esc(col.get(k).map_or("", |(_, v)| v.as_str()))
                )
                .unwrap();
            }
            s.push_str("</tr>\n");
        }
        s + "</table>\n"
    }

    pub(super) fn material_page(&self, m: &Material) -> String {
        let root = UP;
        let mut facts = Vec::new();
        if let Some(a) = &m.armor {
            facts.push((
                "Armor tone".to_string(),
                format!(
                    "<span class=\"dot tone\" style=\"--c: {}\"></span><code>{}</code>",
                    esc(&self.tone(Some(m))),
                    esc(&a.tone)
                ),
            ));
        }
        if let Some(icon) = &m.icon {
            facts.push(("Icon".into(), format!("<code>{}</code>", esc(icon))));
        }
        let stats = self.stat_table(root, &Self::plain_lines(&self.c.material_stats(m)));
        let mut sections = Vec::new();
        if m.armor.is_some() {
            let rows: Vec<(String, String, Lines)> = self
                .c
                .parts
                .iter()
                .filter(|p| p.hull)
                .map(|p| {
                    let stats = Self::plain_lines(&self.c.part_stats(p, Some(m)));
                    (
                        self.chip(p, Some(m)),
                        self.a_badged(root, Kind::Part, &p.id),
                        stats,
                    )
                })
                .collect();
            sections.push((
                "Parts made from it".to_string(),
                self.stat_columns(root, &rows),
            ));
        }
        let ships: Vec<String> = self
            .c
            .ships
            .iter()
            .filter_map(|s| {
                let n = s
                    .parts
                    .iter()
                    .filter(|pl| pl.material.as_deref() == Some(m.id.as_str()))
                    .count();
                (n > 0).then(|| format!("{} ×{n}", self.a(root, Kind::Ship, &s.id)))
            })
            .collect();
        sections.push(("Used by ships".to_string(), list_or_none(&ships)));
        let figure = m.icon.as_ref().map(|k| draw::icon(k, 1, 1, "swatch-big"));
        self.entry(
            m,
            EntryParts {
                badges: badges(m.locked, &[]),
                figure,
                facts,
                stats,
                sections,
                ..Default::default()
            },
        )
    }

    pub(super) fn category_page(&self, cat: &data::Category) -> String {
        let mut facts = Vec::new();
        if let Some(k) = &cat.color {
            facts.push((
                "Colour".to_string(),
                format!("{}<code>{}</code>", self.color_dot(k), esc(k)),
            ));
        }
        let parts: Vec<&Part> = self
            .c
            .parts
            .iter()
            .filter(|p| p.category == cat.id)
            .collect();
        facts.push(("Parts".into(), parts.len().to_string()));
        let sections = vec![("Parts".to_string(), self.parts_table(UP, &parts))];
        let figure = cat.icon.as_ref().map(|k| draw::icon(k, 1, 1, "swatch-big"));
        self.entry(
            cat,
            EntryParts {
                figure,
                facts,
                sections,
                ..Default::default()
            },
        )
    }

    pub(super) fn faction_page(&self, f: &data::Faction) -> String {
        let root = UP;
        let mut facts = vec![(
            "Hull glow".to_string(),
            format!(
                "<span class=\"dot\" style=\"--c: {}\"></span><code>{}</code>",
                esc(&self.style.glow(&f.glow)),
                esc(&f.glow)
            ),
        )];
        if let Some(b) = &f.behavior {
            facts.push(("Default behavior".into(), self.a(root, Kind::Behavior, b)));
        }
        let ships: Vec<&Blueprint> = self.c.ships.iter().filter(|s| s.faction == f.id).collect();
        let sections = vec![("Ships".to_string(), self.ship_cards(root, &ships))];
        self.entry(
            f,
            EntryParts {
                facts,
                sections,
                ..Default::default()
            },
        )
    }

    pub(super) fn frame_page(&self, fr: &data::Frame) -> String {
        let facts = vec![(
            "Grid".to_string(),
            format!("{} × {} cells", fr.size.0, fr.size.1),
        )];
        let ships: Vec<&Blueprint> = self.c.ships.iter().filter(|s| fits(s, fr)).collect();
        let sections = vec![("Ships that fit".to_string(), self.ship_cards(UP, &ships))];
        self.entry(
            fr,
            EntryParts {
                facts,
                sections,
                ..Default::default()
            },
        )
    }

    pub(super) fn ship_page(&self, k: usize, s: &Blueprint) -> String {
        let root = UP;
        let mut facts = vec![(
            "Faction".to_string(),
            self.a(root, Kind::Faction, &s.faction),
        )];
        if let Some(b) = self.c.ship_behavior(s) {
            let own = if s.behavior.is_some() {
                ""
            } else {
                " (faction default)"
            };
            facts.push((
                "Behavior".into(),
                format!("{}{own}", self.a(root, Kind::Behavior, &b.id)),
            ));
        }
        facts.push(("Grid".into(), format!("{} × {} cells", s.size.0, s.size.1)));
        facts.push(("Parts".into(), s.parts.len().to_string()));
        if let Some(fr) = self.c.frames.iter().find(|f| fits(s, f)) {
            facts.push(("Smallest frame".into(), self.a(root, Kind::Frame, &fr.id)));
        }
        if let Some(r) = &self.reports[k]
            && r.pieces.len() > 1
        {
            facts.push(("Pieces".into(), r.pieces.len().to_string()));
        }
        let stats = match &self.reports[k] {
            Some(r) => {
                let rows: Vec<(String, String)> = self
                    .ship_stats(r)
                    .into_iter()
                    .map(|(id, v)| (id.to_string(), v))
                    .collect();
                format!(
                    "<p class=\"note\">Design report at full power, as in the drydock.</p>\n{}",
                    self.stat_table(root, &rows)
                )
            }
            None => String::new(),
        };
        // Parts in order of first placement, with counts and the materials of hull parts.
        let mut order: Vec<&str> = Vec::new();
        let mut count: BTreeMap<&str, (usize, BTreeMap<usize, usize>)> = BTreeMap::new();
        for pl in &s.parts {
            if !order.contains(&pl.part.as_str()) {
                order.push(pl.part.as_str());
            }
            let e = count.entry(pl.part.as_str()).or_default();
            e.0 += 1;
            if let Some(pos) = pl
                .material
                .as_deref()
                .and_then(|m| self.c.materials.position(m))
            {
                *e.1.entry(pos).or_default() += 1;
            }
        }
        let mut t = String::from("<table class=\"list\">\n");
        for id in order {
            let Some(p) = self.c.parts.get(id) else {
                continue;
            };
            let (n, mats) = &count[id];
            let first = s.parts.iter().find(|pl| pl.part == id);
            let chip = self.chip(p, first.and_then(|pl| self.c.placement_material(pl)));
            let mats: Vec<String> = mats
                .iter()
                .map(|(&pos, n)| {
                    let m = &self.c.materials.as_slice()[pos];
                    format!("{} ×{n}", self.a_badged(root, Kind::Material, &m.id))
                })
                .collect();
            writeln!(
                t,
                "<tr><td class=\"pic\">{chip}</td><td>{}</td><td class=\"num\">×{n}</td><td>{}</td></tr>",
                self.a_badged(root, Kind::Part, &p.id),
                mats.join(", ")
            )
            .unwrap();
        }
        t.push_str("</table>\n");
        let sections = vec![("Parts".to_string(), t)];
        let hero = format!(
            "<figure class=\"hero\" style=\"--glow: {}\">{}</figure>\n",
            esc(&self.ship_glow(s)),
            self.ship_svg(s, PX_SHIP, "ship")
        );
        self.entry(
            s,
            EntryParts {
                hero: Some(hero),
                facts,
                stats,
                sections,
                ..Default::default()
            },
        )
    }

    pub(super) fn node_page(&self, n: &BehaviorNode) -> String {
        let root = UP;
        let mut extra = vec![role_badge(n.role)];
        if n.script.is_some() {
            extra.push("<span class=\"badge rhai\">Rhai</span>".into());
        }
        let mut facts = vec![("Role".to_string(), role_name(n.role).to_string())];
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
        let sig: Vec<&str> = n.params.iter().map(|p| p.id.as_str()).collect();
        let sig = if sig.is_empty() {
            n.id.clone()
        } else {
            format!("{}({})", n.id, sig.join(", "))
        };
        facts.push(("Written".into(), format!("<code>{}</code>", esc(&sig))));
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

    pub(super) fn behavior_page(&self, b: &Behavior) -> String {
        let root = UP;
        let mut facts = Vec::new();
        let mut sections = Vec::new();
        match b.program() {
            Ok(program) => {
                facts.push(("Rules".to_string(), program.rules.len().to_string()));
                facts.push(("Rule slots needed".into(), program.rules.len().to_string()));
                let mut s = String::from("<div class=\"rules\">\n");
                let target = program.target.as_ref();
                let tline = match target {
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
                s.push_str("<ol>\n");
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
                s.push_str("</ol>\n</div>\n");
                sections.push(("Rules".to_string(), s));
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

    /// A node call in a rule: the name linked to the node's page in its role's colour, then the arguments.
    fn call(&self, root: &str, call: &data::behavior::Call, role: NodeRole) -> String {
        let text = call.to_string();
        self.call_named(root, &call.name, &text[call.name.len()..], role)
    }

    fn call_named(&self, root: &str, name: &str, args: &str, role: NodeRole) -> String {
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

    pub(super) fn stat_page(&self, st: &data::StatDef) -> String {
        let root = UP;
        let better = match st.better {
            Better::Higher => "higher",
            Better::Lower => "lower",
            Better::Neutral => "neither",
        };
        let mut facts = vec![
            (
                "Unit".to_string(),
                if st.unit.is_empty() {
                    "none".into()
                } else {
                    format!("<code>{}</code>", esc(&st.unit))
                },
            ),
            ("Decimals".into(), st.decimals.to_string()),
            ("Better".into(), better.into()),
        ];
        let ship_stat = SHIP_STATS.contains(&st.id.as_str());
        if ship_stat {
            facts.push(("Scope".into(), "ship (design report)".into()));
        }
        let mut sections = Vec::new();
        let mut rows = String::new();
        for p in self.parts_by_category() {
            for l in self
                .c
                .part_stats(p, None)
                .iter()
                .filter(|l| l.bonus.is_none() && l.stat == st.id)
            {
                writeln!(
                    rows,
                    "<tr><td class=\"pic\">{}</td><td>{}</td><td class=\"num\">{}</td></tr>",
                    self.chip(p, None),
                    self.a_badged(root, Kind::Part, &p.id),
                    esc(&l.text)
                )
                .unwrap();
            }
        }
        if !rows.is_empty() {
            sections.push((
                "Parts".to_string(),
                format!("<table class=\"list\">\n{rows}</table>\n"),
            ));
        }
        let mats: Vec<String> = self
            .c
            .materials
            .iter()
            .flat_map(|m| {
                self.c
                    .material_stats(m)
                    .into_iter()
                    .filter(|l| l.stat == st.id)
                    .map(move |l| (m, l))
            })
            .map(|(m, l)| {
                format!(
                    "{}: {}",
                    self.a_badged(root, Kind::Material, &m.id),
                    esc(&l.text)
                )
            })
            .collect();
        if !mats.is_empty() {
            sections.push(("Materials".into(), ul(&mats)));
        }
        let mut grants = Vec::new();
        for p in &self.c.parts {
            for l in self
                .c
                .part_stats(p, None)
                .iter()
                .filter(|l| l.bonus.is_some() && l.stat == st.id)
            {
                grants.push(format!(
                    "{}: {}",
                    self.a_badged(root, Kind::Part, &p.id),
                    self.bonus_text(root, l)
                ));
            }
        }
        if !grants.is_empty() {
            sections.push(("Bonuses".into(), ul(&grants)));
        }
        let ships: Vec<String> = self
            .c
            .ships
            .iter()
            .zip(&self.reports)
            .filter_map(|(s, r)| {
                let v = self
                    .ship_stats(r.as_ref()?)
                    .into_iter()
                    .find(|(id, _)| *id == st.id)?
                    .1;
                Some(format!("{}: {}", self.a(root, Kind::Ship, &s.id), esc(&v)))
            })
            .collect();
        if !ships.is_empty() {
            sections.push(("Ships".into(), ul(&ships)));
        }
        if sections.is_empty() && !ship_stat {
            sections.push(("Shown on".into(), none()));
        }
        self.entry(
            st,
            EntryParts {
                facts,
                sections,
                ..Default::default()
            },
        )
    }

    pub(super) fn article(&self, id: &str) -> String {
        let Some(p) = self.c.wiki_page(Kind::Page, id) else {
            return String::new();
        };
        let title = self.c.name_of(Kind::Page, id).unwrap_or(id);
        let mut s = format!(
            "<article class=\"page\">\n<h1>{}</h1>\n<div class=\"prose\">\n",
            esc(title)
        );
        s.push_str(&self.prose(UP, &p.markdown, &p.source));
        s.push_str("</div>\n");
        s.push_str(&self.mentioned(Kind::Page, id));
        s.push_str(&self.data_details(Kind::Page, id, &p.source, &[]));
        s + "</article>\n"
    }

    pub(super) fn family_page(&self, f: &Family) -> String {
        let root = UP;
        let nodes = crate::family_research(self.c, &f.id);
        let facts = vec![
            (
                "Colour".to_string(),
                match &f.color {
                    Some(k) => format!("{}<code>{}</code>", self.color_dot(k), esc(k)),
                    None => "neutral".into(),
                },
            ),
            ("Research nodes".into(), nodes.len().to_string()),
        ];
        let mut sections = vec![("Research".to_string(), self.research_table(root, &nodes))];
        // Nodes of other trees that cost this family's data too.
        let others: Vec<&ResearchNode> = self
            .c
            .research
            .iter()
            .filter(|n| n.family != f.id && n.cost.data.contains_key(&f.id))
            .collect();
        if !others.is_empty() {
            sections.push((
                "Also pays for".to_string(),
                self.research_table(root, &others),
            ));
        }
        let (parts, materials) = crate::salvaged_from(self.c, &f.id);
        let mut rows = String::new();
        for p in parts {
            let data = p.salvage.as_ref().map_or(0.0, |s| s.data);
            writeln!(
                rows,
                "<tr><td class=\"pic\">{}</td><td class=\"name\">{}</td><td>{}</td><td class=\"num\">{} per part</td></tr>",
                self.chip(p, None),
                self.a_badged(root, Kind::Part, &p.id),
                self.text(root, &p.summary),
                format_number(data, 2)
            )
            .unwrap();
        }
        for m in materials {
            let data = m.salvage.as_ref().map_or(0.0, |s| s.data);
            writeln!(
                rows,
                "<tr><td class=\"pic\">{}</td><td class=\"name\">{}</td><td>{}</td><td class=\"num\">{} per cell of hull</td></tr>",
                m.icon
                    .as_ref()
                    .map(|k| draw::icon(k, 1, 1, "chip-icon"))
                    .unwrap_or_default(),
                self.a_badged(root, Kind::Material, &m.id),
                self.text(root, &m.summary),
                format_number(data, 2)
            )
            .unwrap();
        }
        sections.push((
            "Salvaged from".to_string(),
            if rows.is_empty() {
                none()
            } else {
                format!("<table class=\"list\">\n{rows}</table>\n")
            },
        ));
        self.entry(
            f,
            EntryParts {
                facts,
                sections,
                ..Default::default()
            },
        )
    }

    pub(super) fn research_page(&self, n: &ResearchNode) -> String {
        let root = UP;
        let cost = self.cost_items(root, &n.cost);
        let facts = vec![
            ("Family".to_string(), self.family_link(root, &n.family)),
            (
                "Kind".into(),
                format!(
                    "<span title=\"{}\">{}</span>",
                    research_kind_about(n.kind),
                    research_kind_name(n.kind)
                ),
            ),
            (
                "Cost".into(),
                if cost.is_empty() {
                    "free".into()
                } else {
                    cost.join("<br>")
                },
            ),
        ];
        let mut rows = String::new();
        for u in &n.unlocks {
            let (pic, name, about) = match u {
                Unlock::Part(id) => match self.c.parts.get(id) {
                    Some(p) => (
                        self.chip(p, None),
                        self.a_badged(root, Kind::Part, id),
                        self.text(root, &p.summary),
                    ),
                    None => continue,
                },
                Unlock::Material(id) => match self.c.materials.get(id) {
                    Some(m) => (
                        m.icon
                            .as_ref()
                            .map(|k| draw::icon(k, 1, 1, "chip-icon"))
                            .unwrap_or_default(),
                        self.a_badged(root, Kind::Material, id),
                        self.text(root, &m.summary),
                    ),
                    None => continue,
                },
                Unlock::Frame(id) => match self.c.frames.get(id) {
                    Some(fr) => (
                        String::new(),
                        self.a_badged(root, Kind::Frame, id),
                        format!(
                            "{} <span class=\"note\">{} × {} cells</span>",
                            self.text(root, &fr.summary),
                            fr.size.0,
                            fr.size.1
                        ),
                    ),
                    None => continue,
                },
                Unlock::MindCapacity(x) => (
                    String::new(),
                    format!("Mind capacity +{x}"),
                    format!(
                        "The ship-mind runs {x} more {} at once.",
                        if *x == 1 { "logic core" } else { "logic cores" }
                    ),
                ),
            };
            writeln!(
                rows,
                "<tr><td class=\"pic\">{pic}</td><td class=\"name\">{name}</td><td>{about}</td></tr>"
            )
            .unwrap();
        }
        let unlocks = if rows.is_empty() {
            none()
        } else {
            format!("<table class=\"list\">\n{rows}</table>\n")
        };
        let requires: Vec<&ResearchNode> = n
            .requires
            .iter()
            .filter_map(|id| self.c.research.get(id))
            .collect();
        let sections = vec![
            ("Unlocks".to_string(), unlocks),
            ("Requires".into(), self.research_table(root, &requires)),
            (
                "Required by".into(),
                self.research_table(root, &crate::required_by(self.c, &n.id)),
            ),
        ];
        self.entry(
            n,
            EntryParts {
                facts,
                sections,
                ..Default::default()
            },
        )
    }

    pub(super) fn start_page(&self, st: &Start) -> String {
        let root = UP;
        let facts = vec![
            ("Ship".to_string(), self.a(root, Kind::Ship, &st.ship)),
            ("Mind capacity".into(), logic_cores(st.mind_capacity)),
        ];
        // Research that adds mind capacity, in load order.
        let gains: Vec<(&ResearchNode, u32)> = self
            .c
            .research
            .iter()
            .flat_map(|n| {
                n.unlocks.iter().filter_map(move |u| match u {
                    Unlock::MindCapacity(x) => Some((n, *x)),
                    _ => None,
                })
            })
            .collect();
        let more: u32 = gains.iter().map(|(_, x)| x).sum();
        let mut mind = format!(
            "<p>The ship-mind runs <strong>{}</strong> at once at the start",
            logic_cores(st.mind_capacity)
        );
        if gains.is_empty() {
            mind.push_str("; no research adds to it yet.</p>\n");
        } else {
            writeln!(
                mind,
                "; research adds up to <strong>{more}</strong> more (<strong>{}</strong> in all).</p>",
                st.mind_capacity + more
            )
            .unwrap();
            let items: Vec<String> = gains
                .iter()
                .map(|(n, x)| format!("{}: +{x}", self.a(root, Kind::Research, &n.id)))
                .collect();
            mind.push_str(&ul(&items));
        }
        // Voyage ids are plain text: the voyage kind does not exist yet.
        let voyages: Vec<String> = st
            .voyages
            .iter()
            .map(|v| format!("<code>{}</code>", esc(v)))
            .collect();
        let voyages = if voyages.is_empty() {
            none()
        } else {
            ul(&voyages)
                + "<p class=\"note\">Voyages are not content yet, so they have no pages.</p>\n"
        };
        // A new game has every part, material and frame that is not locked.
        let open = |items: Vec<(Kind, String)>| {
            self.links(root, items)
                .unwrap_or_else(|| "none".to_string())
        };
        let parts = open(
            self.c
                .parts
                .iter()
                .filter(|p| !p.locked)
                .map(|p| (Kind::Part, p.id.clone()))
                .collect(),
        );
        let materials = open(
            self.c
                .materials
                .iter()
                .filter(|m| !m.locked)
                .map(|m| (Kind::Material, m.id.clone()))
                .collect(),
        );
        let frames = open(
            self.c
                .frames
                .iter()
                .filter(|f| !f.locked)
                .map(|f| (Kind::Frame, f.id.clone()))
                .collect(),
        );
        let kit = format!(
            "<table class=\"list\">\n<tr><th>Parts</th><td>{parts}</td></tr>\n<tr><th>Materials</th><td>{materials}</td></tr>\n<tr><th>Frames</th><td>{frames}</td></tr>\n</table>\n<p class=\"note\">Everything else is locked until <a href=\"{root}{}\">research</a> unlocks it.</p>\n",
            list_path(Kind::Research)
        );
        let sections = vec![
            ("Mind capacity".to_string(), mind),
            ("Open voyages".into(), voyages),
            ("Available at the start".into(), kit),
        ];
        let hero = self.c.ships.get(&st.ship).map(|s| {
            format!(
                "<figure class=\"hero\" style=\"--glow: {}\">{}</figure>\n",
                esc(&self.ship_glow(s)),
                self.ship_svg(s, PX_SHIP, "ship")
            )
        });
        self.entry(
            st,
            EntryParts {
                hero,
                facts,
                sections,
                ..Default::default()
            },
        )
    }

    // -----------------------------------------------------------------------------------------------------
    // Career: salvage, research costs and unlocks

    /// A family's name, linked, after its colour dot.
    fn family_link(&self, root: &str, id: &str) -> String {
        let dot = self
            .c
            .families
            .get(id)
            .and_then(|f| f.color.as_deref())
            .map(|k| self.color_dot(k))
            .unwrap_or_default();
        format!("{dot}{}", self.a(root, Kind::Family, id))
    }

    /// "8 Logic and sensors data": research data of a family.
    fn data_amount(&self, root: &str, family: &str, v: f32) -> String {
        format!(
            "{} {} data",
            format_number(v, 2),
            self.a(root, Kind::Family, family)
        )
    }

    /// Materials (id → amount) as "2 Programmable matter" items, in material order.
    fn material_items(&self, root: &str, amounts: &BTreeMap<String, u32>) -> Vec<String> {
        let mut v: Vec<(&String, &u32)> = amounts.iter().collect();
        v.sort_by_key(|(id, _)| self.c.materials.position(id).unwrap_or(usize::MAX));
        v.into_iter()
            .map(|(id, n)| format!("{n} {}", self.a_badged(root, Kind::Material, id)))
            .collect()
    }

    /// A research cost as items: data per family (in family order), then materials.
    fn cost_items(&self, root: &str, cost: &data::ResearchCost) -> Vec<String> {
        let mut data: Vec<(&String, &f32)> = cost.data.iter().collect();
        data.sort_by_key(|(id, _)| self.c.families.position(id).unwrap_or(usize::MAX));
        let mut v: Vec<String> = data
            .into_iter()
            .map(|(id, x)| self.data_amount(root, id, *x))
            .collect();
        v.extend(self.material_items(root, &cost.materials));
        v
    }

    /// What a node unlocks, in a few words: links, or "Mind capacity +1".
    fn unlock_short(&self, root: &str, u: &Unlock) -> String {
        match u {
            Unlock::Part(id) => self.a_badged(root, Kind::Part, id),
            Unlock::Material(id) => self.a_badged(root, Kind::Material, id),
            Unlock::Frame(id) => self.a_badged(root, Kind::Frame, id),
            Unlock::MindCapacity(x) => format!("Mind capacity&nbsp;+{x}"),
        }
    }

    /// Research nodes as table rows: name with its kind, summary, what it unlocks, cost.
    fn research_table(&self, root: &str, nodes: &[&ResearchNode]) -> String {
        if nodes.is_empty() {
            return none();
        }
        let mut s = String::from("<table class=\"list\">\n");
        for n in nodes {
            let unlocks: Vec<String> = n
                .unlocks
                .iter()
                .map(|u| self.unlock_short(root, u))
                .collect();
            let cost = self.cost_items(root, &n.cost);
            writeln!(
                s,
                "<tr><td class=\"name\">{}<br><span class=\"note\">{}</span></td><td>{}</td><td>{}</td><td class=\"num\">{}</td></tr>",
                self.a(root, Kind::Research, &n.id),
                research_kind_name(n.kind),
                self.text(root, &n.summary),
                unlocks.join(", "),
                if cost.is_empty() {
                    "free".to_string()
                } else {
                    cost.join("<br>")
                }
            )
            .unwrap();
        }
        s + "</table>\n"
    }

    // -----------------------------------------------------------------------------------------------------
    // Lists and the index

    /// Rows of `(picture, name, stat lines)` as a table with one column per stat (labels from `stats.ron`), in
    /// the order of the first row's lines.
    fn stat_columns(&self, root: &str, rows: &[(String, String, Lines)]) -> String {
        let Some((_, _, first)) = rows.first() else {
            return none();
        };
        let mut s = String::from("<table class=\"list\">\n<tr><th></th><th></th>");
        for (stat, _) in first {
            write!(s, "<th class=\"num\">{}</th>", self.stat_label(root, stat)).unwrap();
        }
        s.push_str("</tr>\n");
        for (pic, name, lines) in rows {
            write!(
                s,
                "<tr><td class=\"pic\">{pic}</td><td class=\"name\">{name}</td>"
            )
            .unwrap();
            for (stat, _) in first {
                let v = lines
                    .iter()
                    .find(|(st, _)| st == stat)
                    .map_or("", |(_, v)| v.as_str());
                write!(s, "<td class=\"num\">{}</td>", esc(v)).unwrap();
            }
            s.push_str("</tr>\n");
        }
        s + "</table>\n"
    }

    /// Parts as table rows: chip, name, summary, build points.
    fn parts_table(&self, root: &str, parts: &[&Part]) -> String {
        if parts.is_empty() {
            return none();
        }
        let mut s = String::from("<table class=\"list\">\n");
        for p in parts {
            writeln!(
                s,
                "<tr><td class=\"pic\">{}</td><td class=\"name\">{}</td><td>{}</td><td class=\"num\">{}</td></tr>",
                self.chip(p, None),
                self.a_badged(root, Kind::Part, &p.id),
                self.text(root, &p.summary),
                esc(&self.value("build_points", p.build_points))
            )
            .unwrap();
        }
        s + "</table>\n"
    }

    fn ship_cards(&self, root: &str, ships: &[&Blueprint]) -> String {
        let cards: Vec<Card> = ships
            .iter()
            .map(|s| {
                (
                    format!("{root}{}", item_path(Kind::Ship, &s.id)),
                    *s,
                    s.name.as_str(),
                    s.summary.as_str(),
                )
            })
            .collect();
        self.cards(&cards)
    }

    /// Cards with a ship drawing, a title and a summary.
    fn cards(&self, cards: &[Card]) -> String {
        if cards.is_empty() {
            return none();
        }
        let mut s = String::from("<div class=\"cards\">\n");
        for (href, ship, title, summary) in cards {
            writeln!(
                s,
                "<a class=\"card\" href=\"{href}\">{}<span class=\"title\">{}</span><span class=\"sub\">{}</span></a>",
                self.ship_svg(ship, PX_CARD, "thumb"),
                esc(title),
                esc(&self.c.render_plain(summary))
            )
            .unwrap();
        }
        s + "</div>\n"
    }

    /// A list table on a top-level page: picture (the column is left out when no row has one), name with badge,
    /// summary, one extra cell.
    fn rows<T: Entry>(
        &self,
        items: &[T],
        pic: impl Fn(&T) -> String,
        extra: impl Fn(&T) -> String,
    ) -> String {
        let pics: Vec<String> = items.iter().map(pic).collect();
        let with_pics = pics.iter().any(|p| !p.is_empty());
        let mut s = String::from("<table class=\"list\">\n");
        for (e, p) in items.iter().zip(pics) {
            s.push_str("<tr>");
            if with_pics {
                write!(s, "<td class=\"pic\">{p}</td>").unwrap();
            }
            writeln!(
                s,
                "<td class=\"name\">{}</td><td>{}</td><td class=\"num\">{}</td></tr>",
                self.a_badged("", T::KIND, e.id()),
                self.text("", e.summary()),
                extra(e)
            )
            .unwrap();
        }
        s + "</table>\n"
    }

    pub(super) fn list(&self, kind: Kind) -> String {
        let c = self.c;
        let mut s = format!("<h1>{}</h1>\n", kind_title(kind));
        let count = |n: usize, one: &str, many: &str| {
            if n == 1 {
                format!("1 {one}")
            } else {
                format!("{n} {many}")
            }
        };
        match kind {
            Kind::Part => {
                for cat in &c.categories {
                    let parts: Vec<&Part> =
                        c.parts.iter().filter(|p| p.category == cat.id).collect();
                    let dot = cat
                        .color
                        .as_deref()
                        .map(|k| self.color_dot(k))
                        .unwrap_or_default();
                    writeln!(
                        s,
                        "<section>\n<h2>{dot}{}</h2>\n<p class=\"lead\">{}</p>\n{}</section>",
                        self.a("", Kind::Category, &cat.id),
                        self.text("", &cat.summary),
                        self.parts_table("", &parts)
                    )
                    .unwrap();
                }
            }
            Kind::Material => {
                s += &self.rows(
                    c.materials.as_slice(),
                    |m| {
                        m.icon
                            .as_ref()
                            .map(|k| draw::icon(k, 1, 1, "chip-icon"))
                            .unwrap_or_default()
                    },
                    |m| {
                        let l: Vec<String> = c
                            .material_stats(m)
                            .iter()
                            .map(|l| format!("{} {}", esc(&l.label), esc(&l.text)))
                            .collect();
                        l.join("<br>")
                    },
                )
            }
            Kind::Category => {
                s += &self.rows(
                    c.categories.as_slice(),
                    |k| {
                        k.color
                            .as_deref()
                            .map(|key| self.color_dot(key))
                            .unwrap_or_default()
                    },
                    |k| {
                        count(
                            c.parts.iter().filter(|p| p.category == k.id).count(),
                            "part",
                            "parts",
                        )
                    },
                )
            }
            Kind::Ship => s += &self.ship_cards("", &c.ships.iter().collect::<Vec<_>>()),
            Kind::Frame => {
                s += &self.rows(
                    c.frames.as_slice(),
                    |_| String::new(),
                    |f| format!("{} × {}", f.size.0, f.size.1),
                )
            }
            Kind::Faction => {
                s += &self.rows(
                    c.factions.as_slice(),
                    |f| {
                        format!(
                            "<span class=\"dot\" style=\"--c: {}\"></span>",
                            esc(&self.style.glow(&f.glow))
                        )
                    },
                    |f| {
                        count(
                            c.ships.iter().filter(|x| x.faction == f.id).count(),
                            "ship",
                            "ships",
                        )
                    },
                )
            }
            Kind::Behavior => {
                s += &self.rows(
                    c.behaviors.as_slice(),
                    |_| String::new(),
                    |b| count(b.rules.len(), "rule", "rules"),
                )
            }
            Kind::BehaviorNode => {
                for role in ROLES {
                    let nodes: Vec<BehaviorNode> = c
                        .behavior_nodes
                        .iter()
                        .filter(|n| n.role == role)
                        .cloned()
                        .collect();
                    let title = match role {
                        NodeRole::Target => "Target selectors",
                        NodeRole::Condition => "Conditions",
                        NodeRole::Action => "Actions",
                    };
                    writeln!(
                        s,
                        "<section>\n<h2>{title}</h2>\n{}</section>",
                        self.rows(
                            &nodes,
                            |n| format!(
                                "<code class=\"{}\">{}</code>",
                                role_class(n.role),
                                esc(&n.id)
                            ),
                            |n| {
                                if n.script.is_some() {
                                    "<span class=\"badge rhai\">Rhai</span>".into()
                                } else {
                                    String::new()
                                }
                            }
                        )
                    )
                    .unwrap();
                }
            }
            Kind::Stat => {
                let (ship, part): (Vec<data::StatDef>, Vec<data::StatDef>) = c
                    .stats
                    .iter()
                    .cloned()
                    .partition(|d| SHIP_STATS.contains(&d.id.as_str()));
                let unit = |d: &data::StatDef| {
                    if d.unit.is_empty() {
                        String::new()
                    } else {
                        format!("<code>{}</code>", esc(&d.unit))
                    }
                };
                writeln!(
                    s,
                    "<section>\n<h2>Part and material stats</h2>\n{}</section>",
                    self.rows(&part, |_| String::new(), unit)
                )
                .unwrap();
                writeln!(
                    s,
                    "<section>\n<h2>Ship stats</h2>\n{}</section>",
                    self.rows(&ship, |_| String::new(), unit)
                )
                .unwrap();
            }
            Kind::Family => {
                s += &self.rows(
                    c.families.as_slice(),
                    |f| {
                        f.color
                            .as_deref()
                            .map(|k| self.color_dot(k))
                            .unwrap_or_default()
                    },
                    |f| {
                        count(
                            crate::family_research(c, &f.id).len(),
                            "research node",
                            "research nodes",
                        )
                    },
                )
            }
            Kind::Research => {
                // By family, in family order; families without nodes are left out.
                for f in &c.families {
                    let nodes = crate::family_research(c, &f.id);
                    if nodes.is_empty() {
                        continue;
                    }
                    writeln!(
                        s,
                        "<section>\n<h2>{}</h2>\n<p class=\"lead\">{}</p>\n{}</section>",
                        self.family_link("", &f.id),
                        self.text("", &f.summary),
                        self.research_table("", &nodes)
                    )
                    .unwrap();
                }
            }
            Kind::Start => {
                let cards: Vec<Card> = c
                    .starts
                    .iter()
                    .filter_map(|st| {
                        Some((
                            item_path(Kind::Start, &st.id),
                            c.ships.get(&st.ship)?,
                            st.name.as_str(),
                            st.summary.as_str(),
                        ))
                    })
                    .collect();
                s += &self.cards(&cards);
            }
            Kind::Page => {
                let pages: Vec<String> = c
                    .wiki
                    .values()
                    .filter(|p| p.kind == Kind::Page)
                    .map(|p| self.a("", Kind::Page, &p.id))
                    .collect();
                s += &list_or_none(&pages);
            }
        }
        s
    }

    pub(super) fn index(&self) -> String {
        let c = self.c;
        let mut s = format!("<h1>{}</h1>\n", esc(&self.site_name()));
        if let Some(d) = c.packs.first().and_then(|p| p.description.as_deref()) {
            writeln!(s, "<p class=\"summary\">{}</p>", esc(d)).unwrap();
        }
        let n_pages = c.wiki.values().filter(|p| p.kind == Kind::Page).count();
        s.push_str("<div class=\"kinds\">\n");
        for (kind, title) in KINDS {
            let n = match kind {
                Kind::Stat => c.stats.len(),
                Kind::Category => c.categories.len(),
                Kind::Faction => c.factions.len(),
                Kind::Material => c.materials.len(),
                Kind::Part => c.parts.len(),
                Kind::Frame => c.frames.len(),
                Kind::Ship => c.ships.len(),
                Kind::BehaviorNode => c.behavior_nodes.len(),
                Kind::Behavior => c.behaviors.len(),
                Kind::Family => c.families.len(),
                Kind::Research => c.research.len(),
                Kind::Start => c.starts.len(),
                Kind::Page => n_pages,
            };
            writeln!(s, "<a class=\"kind\" href=\"{}\"><span class=\"n\">{n}</span><span class=\"t\">{title}</span></a>", list_path(kind)).unwrap();
        }
        s.push_str("</div>\n");
        writeln!(
            s,
            "<section>\n<h2>Ships</h2>\n{}</section>",
            self.ship_cards("", &c.ships.iter().collect::<Vec<_>>())
        )
        .unwrap();
        s.push_str("<section>\n<h2>Parts</h2>\n<div class=\"palette\">\n");
        for p in self.parts_by_category() {
            writeln!(
                s,
                "<a href=\"{}\">{}<span>{}{}</span></a>",
                item_path(Kind::Part, &p.id),
                self.part_svg(p, None, PX_PALETTE, "chip"),
                esc(&p.name),
                if p.locked { LOCKED } else { "" }
            )
            .unwrap();
        }
        s.push_str("</div>\n</section>\n");
        if n_pages > 0 {
            let pages: Vec<String> = c
                .wiki
                .values()
                .filter(|p| p.kind == Kind::Page)
                .map(|p| self.a("", Kind::Page, &p.id))
                .collect();
            writeln!(s, "<section>\n<h2>Articles</h2>\n{}</section>", ul(&pages)).unwrap();
        }
        s.push_str("<section>\n<h2>Packs</h2>\n<table class=\"list\">\n");
        for p in &c.packs {
            writeln!(
                s,
                "<tr><td class=\"name\">{} <code>{}</code></td><td class=\"num\">{}</td><td>{}</td><td>{}</td></tr>",
                esc(&p.name),
                esc(&p.id),
                esc(&p.version),
                esc(p.description.as_deref().unwrap_or("")),
                esc(&p.authors.join(", "))
            )
            .unwrap();
        }
        s.push_str("</table>\n</section>\n");
        s
    }
}

fn badges(locked: bool, extra: &[String]) -> String {
    let mut s = String::new();
    for e in extra {
        s.push(' ');
        s.push_str(e);
    }
    if locked {
        s.push_str(LOCKED);
    }
    s
}

/// The kinds of research nodes (GDR-0024), as the research screen names them.
fn research_kind_name(k: ResearchKind) -> &'static str {
    match k {
        ResearchKind::NewPart => "New part",
        ResearchKind::Variant => "Variant",
        ResearchKind::SizeClass => "Size class",
        ResearchKind::Refinement => "Refinement",
        ResearchKind::Alien => "Alien tech",
    }
}

fn research_kind_about(k: ResearchKind) -> &'static str {
    match k {
        ResearchKind::NewPart => "A new part with a new silhouette and role",
        ResearchKind::Variant => "A sidegrade of an existing part, with a trade-off",
        ResearchKind::SizeClass => "A bigger footprint or frame for bigger hulls",
        ResearchKind::Refinement => {
            "A small step of a chain, such as a stat step or more mind capacity"
        }
        ResearchKind::Alien => "Needs reverse-engineered alien tech",
    }
}

/// "1 logic core", "2 logic cores": mind capacity in words.
fn logic_cores(n: u32) -> String {
    if n == 1 {
        "1 logic core".into()
    } else {
        format!("{n} logic cores")
    }
}

/// A card: link target, the ship drawn on it, title, summary (a content text).
type Card<'a> = (String, &'a Blueprint, &'a str, &'a str);

/// Node roles in the editor's order: the target line, then conditions, then actions.
const ROLES: [NodeRole; 3] = [NodeRole::Target, NodeRole::Condition, NodeRole::Action];

fn role_name(role: NodeRole) -> &'static str {
    match role {
        NodeRole::Condition => "Condition",
        NodeRole::Action => "Action",
        NodeRole::Target => "Target selector",
    }
}

fn role_class(role: NodeRole) -> &'static str {
    match role {
        NodeRole::Condition => "cond",
        NodeRole::Action => "act",
        NodeRole::Target => "tgt",
    }
}

fn role_badge(role: NodeRole) -> String {
    format!(
        "<span class=\"badge {}\">{}</span>",
        role_class(role),
        role_name(role)
    )
}

fn note(n: &Option<String>) -> String {
    n.as_ref().map_or(String::new(), |n| {
        format!(" <span class=\"note\"># {}</span>", esc(n))
    })
}

/// Every node a behavior names, in reading order: `(id, role it is used in)`; the default target counts.
fn node_uses(b: &Behavior) -> Vec<(String, NodeRole)> {
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

/// A ship's grid fits a frame.
fn fits(s: &Blueprint, f: &data::Frame) -> bool {
    s.size.0 <= f.size.0 && s.size.1 <= f.size.1
}

fn ul(items: &[String]) -> String {
    let mut s = String::from("<ul>\n");
    for i in items {
        writeln!(s, "<li>{i}</li>").unwrap();
    }
    s + "</ul>\n"
}

fn none() -> String {
    "<p class=\"none\">None.</p>\n".into()
}

fn list_or_none(items: &[String]) -> String {
    if items.is_empty() { none() } else { ul(items) }
}
