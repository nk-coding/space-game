//! The pages of the wiki: one per entry of every kind, one list per kind, and the index.
//!
//! Parts, materials, categories, factions, frames, ships, stats and articles here; behaviors (`behaviors.rs`), the
//! career's research, facilities and starts (`career.rs`), voyages and encounters (`voyages.rs`), the evolution lab's
//! page (`lab.rs`), the lists and the index (`lists.rs`).

mod behaviors;
mod career;
mod lab;
mod lists;
mod voyages;

use super::*;

pub(crate) use behaviors::{ROLES, role_title, signature};
pub(crate) use behaviors::role_class;
pub(crate) use lab::{LAB_DOCUMENTED, LAB_PAGE};

impl Wiki<'_> {
    pub(crate) fn part_page(&self, p: &Part) -> String {
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
        sections.extend(self.unlocked_by(&Unlock::Part(p.id.clone()), p.locked));
        sections.push(("Salvage".to_string(), self.part_salvage(root, p)));

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
            self.part_figure(p)
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

    /// A part's technical drawing (`draw::blueprint`): its tile with what it does marked around it.
    pub(crate) fn part_figure(&self, p: &Part) -> String {
        let shape = self.part_tile(p, None).shape;
        let upper = |stat: &str, v: f32| format!("{} {}", self.c.stats.get(stat).map_or(stat.to_string(), |d| d.name.to_uppercase()), self.value(stat, v));
        let marks = draw::Marks {
            sides: shape.connectable_sides(),
            thrust: p.engine.as_ref().map(|e| e.gimbal),
            plume: p
                .engine
                .as_ref()
                .filter(|e| e.plume.length > 0.0)
                .map(|e| (e.plume.length, upper("plume_length", e.plume.length))),
            fires: p.weapon.is_some(),
            nozzles: p.rcs.is_some(),
            vents: p.radiator.is_some(),
            torque: p.gyro.is_some(),
            rings: p
                .shield
                .iter()
                .map(|sh| (sh.radius, "bp-shield", upper("shield_radius", sh.radius)))
                .collect(),
            blast: p
                .explosion
                .as_ref()
                .map(|x| (x.radius, upper("blast_radius", x.radius))),
        };
        draw::blueprint(&self.style, &self.part_tile(p, None), &marks, 260.0, &p.name)
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

    pub(crate) fn material_page(&self, m: &Material) -> String {
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
        sections.extend(self.unlocked_by(&Unlock::Material(m.id.clone()), m.locked));
        if m.armor.is_some() {
            sections.push(("Salvage".to_string(), self.material_salvage(root, m)));
        }
        // Parts whose salvage gives this material, intact or as the fragment a destroyed one leaves.
        let mut rows = String::new();
        for p in &self.c.parts {
            let Some(s) = &p.salvage else { continue };
            let mut from = Vec::new();
            if let Some(n) = s.materials.get(&m.id) {
                from.push(format!("{n} per intact part"));
            }
            if let Some(n) = s.fragment.get(&m.id) {
                from.push(format!("{n} per fragment of a destroyed one"));
            }
            if !from.is_empty() {
                writeln!(
                    rows,
                    "<tr><td class=\"pic\">{}</td><td class=\"name\">{}</td><td>{}</td></tr>",
                    self.chip(p, None),
                    self.a_badged(root, Kind::Part, &p.id),
                    from.join(", ")
                )
                .unwrap();
            }
        }
        if !rows.is_empty() {
            sections.push((
                "From salvage".to_string(),
                format!("<table class=\"list\">\n{rows}</table>\n"),
            ));
        }
        let costs: Vec<&ResearchNode> = self
            .c
            .research
            .iter()
            .filter(|n| n.cost.materials.contains_key(&m.id))
            .collect();
        if !costs.is_empty() {
            sections.push((
                "Research that costs it".to_string(),
                self.research_table(root, &costs),
            ));
        }
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

    pub(crate) fn category_page(&self, cat: &data::Category) -> String {
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

    /// A control: its section, the screens it acts on and its default inputs.
    pub(crate) fn control_page(&self, e: &data::Control) -> String {
        let contexts: Vec<&str> = e.contexts.iter().map(|&c| context_name(c)).collect();
        let facts = vec![
            ("Section".to_string(), esc(e.section.title())),
            ("Acts on".into(), esc(&contexts.join(", "))),
            ("Default".into(), default_inputs(e)),
        ];
        // The other actions of its section: what it sits beside in SETTINGS.
        let others: Vec<String> = self
            .c
            .controls
            .iter()
            .filter(|o| o.section == e.section && o.id != e.id)
            .map(|o| format!("{}: {}", self.a(UP, Kind::Control, &o.id), default_inputs(o)))
            .collect();
        let sections = vec![(format!("More {}", e.section.title().to_lowercase()), list_or_none(&others))];
        self.entry(
            e,
            EntryParts {
                facts,
                sections,
                ..Default::default()
            },
        )
    }

    pub(crate) fn faction_page(&self, f: &data::Faction) -> String {
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

    pub(crate) fn frame_page(&self, fr: &data::Frame) -> String {
        let facts = vec![(
            "Grid".to_string(),
            format!("{} × {} cells", fr.size.0, fr.size.1),
        )];
        let ships: Vec<&Blueprint> = self.c.ships.iter().filter(|s| fits(s, fr)).collect();
        let mut sections: Vec<(String, String)> = self
            .unlocked_by(&Unlock::Frame(fr.id.clone()), fr.locked)
            .into_iter()
            .collect();
        sections.push(("Ships that fit".to_string(), self.ship_cards(UP, &ships)));
        self.entry(
            fr,
            EntryParts {
                badges: badges(fr.locked, &[]),
                facts,
                sections,
                ..Default::default()
            },
        )
    }

    pub(crate) fn ship_page(&self, k: usize, s: &Blueprint) -> String {
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
        // The smallest by area (ties: load order), wherever a mod put it.
        let smallest = self
            .c
            .frames
            .iter()
            .filter(|f| fits(s, f))
            .min_by_key(|f| u64::from(f.size.0) * u64::from(f.size.1));
        if let Some(fr) = smallest {
            facts.push((
                "Smallest frame".into(),
                self.a_badged(root, Kind::Frame, &fr.id),
            ));
        }
        let starts = self
            .c
            .starts
            .iter()
            .filter(|st| st.ship == s.id)
            .map(|st| (Kind::Start, st.id.clone()));
        if let Some(l) = self.links(root, starts) {
            facts.push(("Start ship of".into(), l));
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
        let mut sections = vec![("Parts".to_string(), t)];
        // Its pilot assists (GDR-0088): the rules that help the player fly it.
        let assists = match s.assists.as_ref().map(|a| a.program()) {
            Some(Ok(p)) => self.rules_figure(root, &p, data::behavior::ListKind::Assists),
            Some(Err(errors)) => {
                for e in errors {
                    self.warn(format!("ship `{}`: assists: {e}", s.id));
                }
                none()
            }
            None => "<p class=\"none\">None: the ship coasts when you let go.</p>\n".into(),
        };
        sections.push(("Pilot assists".to_string(), assists));
        // Where it waits on voyages: fighting, or lying there as a wreck.
        let found: Vec<String> = crate::encounters_with(self.c, &s.id)
            .into_iter()
            .map(|e| {
                let mut how: Vec<String> = e
                    .ships
                    .iter()
                    .filter(|x| x.ship == s.id)
                    .map(|x| format!("×{} fighting", count_range(x.count)))
                    .collect();
                how.extend(e.wrecks.iter().filter(|w| w.ship == s.id).map(|w| {
                    format!(
                        "×{} wrecked, {}",
                        count_range(w.count),
                        parts_gone(w.damage)
                    )
                }));
                format!(
                    "{}: {}",
                    self.a(root, Kind::Encounter, &e.id),
                    how.join("; ")
                )
            })
            .collect();
        if !found.is_empty() {
            sections.push(("In encounters".to_string(), ul(&found)));
        }
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

    pub(crate) fn stat_page(&self, st: &data::StatDef) -> String {
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

    pub(crate) fn article(&self, id: &str) -> String {
        let Some(p) = self.c.wiki_page(Kind::Page, id) else {
            return String::new();
        };
        let title = self.c.name_of(Kind::Page, id).unwrap_or(id);
        let mut s = heading(1, &esc(title), &slug(id));
        s.push_str(&block(
            "sg-article prose",
            &self.prose(UP, &p.markdown, &p.source),
        ));
        s.push_str(&self.mentioned(Kind::Page, id));
        s.push_str(&block(
            "sg-data",
            &self.data_details(Kind::Page, id, &p.source, &[]),
        ));
        s
    }
}

/// Where a control acts, in words.
fn context_name(c: data::input::Context) -> &'static str {
    use data::input::Context;
    match c {
        Context::Flight => "flight",
        Context::Home => "the shipyard",
        Context::Drydock => "the drydock",
        Context::Behavior => "the BEHAVIOR screen",
        Context::Research => "research",
        Context::Map => "the voyage map",
        Context::Replay => "the fight viewer",
        Context::Any => "every screen",
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
        ResearchKind::Facility => "Facility",
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
        ResearchKind::Facility => "Opens a building of the home shipyard",
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

/// "2" or "2–3": a count rolled per launch, `(min, max)`.
fn count_range((lo, hi): (u32, u32)) -> String {
    if lo == hi {
        lo.to_string()
    } else {
        format!("{lo}–{hi}")
    }
}

pub(crate) fn metres(x: f32) -> String {
    format!("{} m", format_number(x, 1))
}

/// The sum of several `(min, max)` counts.
fn sum_ranges(counts: impl Iterator<Item = (u32, u32)>) -> (u32, u32) {
    counts.fold((0, 0), |(lo, hi), (a, b)| {
        (lo.saturating_add(a), hi.saturating_add(b))
    })
}

/// An expected length, `(min, max)` minutes: "6–8 min".
pub(crate) fn minutes(range: (u32, u32)) -> String {
    format!("{} min", count_range(range))
}

fn sites(n: u32) -> String {
    if n == 1 {
        "1 site".into()
    } else {
        format!("{n} sites")
    }
}

/// A wreck's damage: "50% of parts gone".
fn parts_gone(damage: f32) -> String {
    format!("{}% of parts gone", format_number(damage * 100.0, 0))
}

fn not_salvaged() -> String {
    "<p class=\"none\">Not collected as salvage.</p>\n".into()
}

/// A card: link target, the ship drawn on it, title, summary (a content text).
type Card<'a> = (String, &'a Blueprint, &'a str, &'a str);

/// The badge of a voyage's goal site.
const GOAL: &str =
    " <span class=\"badge goal\" title=\"Clearing this site clears the voyage\">Goal</span>";

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

/// A control's default inputs as keys (`<kbd>W</kbd> or <kbd>↑</kbd>`), or "none".
pub(crate) fn default_inputs(e: &data::Control) -> String {
    let keys: Vec<String> =
        e.inputs.iter().map(|s| s.parse::<data::input::Chord>().map_or_else(|_| esc(s), |c| format!("<kbd>{}</kbd>", esc(&c.label())))).collect();
    if keys.is_empty() { "none".into() } else { keys.join(" or ") }
}
