//! The wiki's lists (one per kind: stat tables, part tables, ship cards) and its index.

use super::*;

impl Wiki<'_> {
    // Lists and the index

    /// Rows of `(picture, name, stat lines)` as a table with one column per stat (labels from `stats.ron`), in
    /// the order of the first row's lines.
    pub(super) fn stat_columns(&self, root: &str, rows: &[(String, String, Lines)]) -> String {
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
    pub(super) fn parts_table(&self, root: &str, parts: &[&Part]) -> String {
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

    pub(super) fn ship_cards(&self, root: &str, ships: &[&Blueprint]) -> String {
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
    pub(super) fn cards(&self, cards: &[Card]) -> String {
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
    pub(super) fn rows<T: Entry>(
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

    pub(crate) fn list(&self, kind: Kind) -> String {
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
            Kind::Voyage => {
                s += &self.voyage_table("", &c.voyages.iter().collect::<Vec<_>>());
            }
            Kind::Encounter => {
                s += &self.encounter_table("", &c.encounters.iter().collect::<Vec<_>>());
            }
            Kind::Control => {
                s.push_str("<p class=\"lead\">The default inputs; players rebind them in the game's SETTINGS.</p>\n");
                s += &self.rows(c.controls.as_slice(), |_| String::new(), super::default_inputs);
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

    pub(crate) fn index(&self) -> String {
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
                Kind::Voyage => c.voyages.len(),
                Kind::Encounter => c.encounters.len(),
                Kind::Control => c.controls.len(),
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
