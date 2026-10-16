//! The wiki's lists (one per kind: stat tables, part tables, ship cards) and its index.

use super::*;

impl Wiki<'_> {
    // Lists and the index

    /// Rows of `(picture, name, stat lines)` as a table with one column per stat (labels from `stats.ron`), in
    /// the order of the first row's lines.
    pub(crate) fn stat_columns(&self, root: &str, rows: &[(String, String, Lines)]) -> String {
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
    pub(crate) fn parts_table(&self, root: &str, parts: &[&Part]) -> String {
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

    pub(crate) fn ship_cards(&self, root: &str, ships: &[&Blueprint]) -> String {
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
    pub(crate) fn cards(&self, cards: &[Card]) -> String {
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
    pub(crate) fn rows<T: Entry>(
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
                self.a_badged(UP, T::KIND, e.id()),
                self.text(UP, e.summary()),
                extra(e)
            )
            .unwrap();
        }
        s + "</table>\n"
    }

    pub(crate) fn list(&self, kind: Kind) -> String {
        let c = self.c;
        let title = kind_title(kind);
        let mut s = heading(1, title, &slug(title));
        let count = |n: usize, one: &str, many: &str| {
            if n == 1 {
                format!("1 {one}")
            } else {
                format!("{n} {many}")
            }
        };
        // A titled section of the list: heading, then its block.
        let section = |s: &mut String, title_html: &str, id: &str, html: &str| {
            s.push_str(&heading(2, title_html, id));
            s.push_str(&wide_block("sg-section", html));
        };
        match kind {
            Kind::Part => {
                // The sortable table of every part (the site's component, from `ui.json`), then the categories.
                s.push_str("<PartsTable />\n\n");
                for cat in &c.categories {
                    let parts: Vec<&Part> =
                        c.parts.iter().filter(|p| p.category == cat.id).collect();
                    let dot = cat
                        .color
                        .as_deref()
                        .map(|k| self.color_dot(k))
                        .unwrap_or_default();
                    section(
                        &mut s,
                        &format!("{dot}{}", esc(&cat.name)),
                        &slug(&cat.id),
                        &format!(
                            "<p class=\"lead\">{} {}</p>\n{}",
                            self.text(UP, &cat.summary),
                            self.a(UP, Kind::Category, &cat.id),
                            self.parts_table(UP, &parts)
                        ),
                    );
                }
            }
            Kind::Material => {
                s += &wide_block(
                    "sg-list",
                    &self.rows(
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
                    ),
                )
            }
            Kind::Category => {
                s += &wide_block(
                    "sg-list",
                    &self.rows(
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
                    ),
                )
            }
            Kind::Ship => {
                s += &wide_block(
                    "sg-list",
                    &self.ship_cards(UP, &c.ships.iter().collect::<Vec<_>>()),
                )
            }
            Kind::Frame => {
                s += &wide_block(
                    "sg-list",
                    &self.rows(
                        c.frames.as_slice(),
                        |_| String::new(),
                        |f| format!("{} × {}", f.size.0, f.size.1),
                    ),
                )
            }
            Kind::Faction => {
                s += &wide_block(
                    "sg-list",
                    &self.rows(
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
                    ),
                )
            }
            Kind::Behavior => {
                s += &wide_block(
                    "sg-list",
                    &self.rows(
                        c.behaviors.as_slice(),
                        |_| String::new(),
                        |b| count(b.rules.len(), "rule", "rules"),
                    ),
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
                    let title = role_title(role);
                    section(
                        &mut s,
                        title,
                        &slug(title),
                        &self.rows(
                            &nodes,
                            |n| {
                                format!(
                                    "<code class=\"{}\">{}</code>",
                                    role_class(n.role),
                                    esc(&n.id)
                                )
                            },
                            |n| {
                                let mut b = Vec::new();
                                if n.script.is_some() {
                                    b.push(SCRIPT);
                                }
                                if n.pilot {
                                    b.push("<span class=\"badge pilot\">Pilot assist</span>");
                                }
                                b.join(" ")
                            },
                        ),
                    );
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
                section(
                    &mut s,
                    "Part and material stats",
                    "part-and-material-stats",
                    &self.rows(&part, |_| String::new(), unit),
                );
                section(
                    &mut s,
                    "Ship stats",
                    "ship-stats",
                    &self.rows(&ship, |_| String::new(), unit),
                );
            }
            Kind::Family => {
                s += &wide_block(
                    "sg-list",
                    &self.rows(
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
                    ),
                )
            }
            Kind::Research => {
                // By family, in family order; families without nodes are left out.
                for f in &c.families {
                    let nodes = crate::family_research(c, &f.id);
                    if nodes.is_empty() {
                        continue;
                    }
                    section(
                        &mut s,
                        &self.family_dot_name(f),
                        &slug(&f.id),
                        &format!(
                            "<p class=\"lead\">{} {}</p>\n{}",
                            self.text(UP, &f.summary),
                            self.a(UP, Kind::Family, &f.id),
                            self.research_table(UP, &nodes)
                        ),
                    );
                }
            }
            Kind::Facility => {
                s += &wide_block(
                    "sg-list",
                    &self.rows(c.facilities.as_slice(), |_| String::new(), |f| {
                        let by: Vec<String> = crate::unlocked_by(c, &Unlock::Facility(f.id.clone())).iter().map(|n| self.a(UP, Kind::Research, &n.id)).collect();
                        if by.is_empty() { "at home from the start".into() } else { by.join(", ") }
                    }),
                )
            }
            Kind::Start => {
                let cards: Vec<Card> = c
                    .starts
                    .iter()
                    .filter_map(|st| {
                        Some((
                            format!("{UP}{}", item_path(Kind::Start, &st.id)),
                            c.ships.get(&st.ship)?,
                            st.name.as_str(),
                            st.summary.as_str(),
                        ))
                    })
                    .collect();
                s += &wide_block("sg-list", &self.cards(&cards));
            }
            Kind::Voyage => {
                s += &wide_block(
                    "sg-list",
                    &self.voyage_table(UP, &c.voyages.iter().collect::<Vec<_>>()),
                );
            }
            Kind::Encounter => {
                s += &wide_block(
                    "sg-list",
                    &self.encounter_table(UP, &c.encounters.iter().collect::<Vec<_>>()),
                );
            }
            Kind::Galaxy => {
                s += &wide_block(
                    "sg-list",
                    &self.rows(c.galaxies.as_slice(), |_| String::new(), |g| count(g.bands.len(), "band", "bands")),
                )
            }
            Kind::Band => {
                s += &wide_block(
                    "sg-list",
                    &self.rows(c.bands.as_slice(), |_| String::new(), |b| format!("{}{}", self.value("galaxy_distance", b.edge), super::galaxy::stub(&b.tags))),
                )
            }
            Kind::ClusterType => {
                s += &wide_block(
                    "sg-list",
                    &self.rows(c.cluster_types.as_slice(), |_| String::new(), |t| format!("{}{}", super::galaxy::radius(t), super::galaxy::stub(&t.tags))),
                )
            }
            Kind::Trail => {
                s += &wide_block(
                    "sg-list",
                    &self.rows(c.trails.as_slice(), |_| String::new(), |t| format!("{}{}", super::galaxy::reward_word(t.reward), super::galaxy::stub(&t.tags))),
                )
            }
            Kind::SignalSource => {
                s += &wide_block(
                    "sg-list",
                    &self.rows(c.signal_sources.as_slice(), |_| String::new(), |x| super::galaxy::form_word(x.form).into()),
                )
            }
            Kind::Lore => {
                s += &wide_block(
                    "sg-list",
                    &self.rows(c.lore.as_slice(), |_| String::new(), |l| self.a(UP, Kind::Band, &l.band)),
                )
            }
            Kind::Control => {
                s += &wide_block(
                    "sg-list",
                    "<p class=\"lead\">The default inputs; you rebind them in the game's SETTINGS.</p>\n",
                );
                for section_id in data::input::Section::ALL {
                    let controls: Vec<data::Control> = c
                        .controls
                        .iter()
                        .filter(|e| e.section == section_id)
                        .cloned()
                        .collect();
                    if controls.is_empty() {
                        continue;
                    }
                    let title = section_id.title();
                    section(
                        &mut s,
                        title,
                        &slug(title),
                        &self.rows(&controls, |_| String::new(), super::default_inputs),
                    );
                }
            }
            Kind::Page => {
                let pages: Vec<String> = c
                    .wiki
                    .values()
                    .filter(|p| p.kind == Kind::Page)
                    .map(|p| self.a(UP, Kind::Page, &p.id))
                    .collect();
                s += &wide_block("sg-list", &list_or_none(&pages));
            }
        }
        s
    }

    /// A family's name after its colour dot (no link: a heading).
    fn family_dot_name(&self, f: &Family) -> String {
        let dot = f
            .color
            .as_deref()
            .map(|k| self.color_dot(k))
            .unwrap_or_default();
        format!("{dot}{}", esc(&f.name))
    }

    /// The reference overview: every kind with its count, and the loaded packs.
    pub(crate) fn reference(&self) -> String {
        let c = self.c;
        let mut s = heading(1, "Reference", "reference");
        let mut kinds = String::new();
        if let Some(d) = c.packs.first().and_then(|p| p.description.as_deref()) {
            writeln!(kinds, "<p class=\"summary\">{}</p>", esc(d)).unwrap();
        }
        kinds.push_str("<div class=\"kinds\">\n");
        for (kind, title) in KINDS {
            writeln!(
                kinds,
                "<a class=\"kind\" href=\"{UP}{}\"><span class=\"n\">{}</span><span class=\"t\">{title}</span></a>",
                list_path(kind),
                self.count(kind)
            )
            .unwrap();
        }
        kinds.push_str("</div>\n");
        s.push_str(&wide_block("sg-list", &kinds));
        let mut packs = String::from("<table class=\"list\">\n");
        for p in &c.packs {
            writeln!(
                packs,
                "<tr><td class=\"name\">{} <code>{}</code></td><td class=\"num\">{}</td><td>{}</td><td>{}</td></tr>",
                esc(&p.name),
                esc(&p.id),
                esc(&p.version),
                esc(p.description.as_deref().unwrap_or("")),
                esc(&p.authors.join(", "))
            )
            .unwrap();
        }
        packs.push_str("</table>\n");
        s.push_str(&heading(2, "Evolution lab", "evolution-lab"));
        s.push_str(&wide_block(
            "sg-section",
            &format!("<p>The evolution lab's goals, trials, genes, stand-in pilots, sizes and measures: <a href=\"{UP}{}.html\">Evolution lab</a>.</p>\n", pages::LAB_PAGE),
        ));
        s.push_str(&heading(2, "Packs", "packs"));
        s.push_str(&wide_block("sg-section", &packs));
        s
    }

    /// How many entries of a kind the content holds.
    pub(crate) fn count(&self, kind: Kind) -> usize {
        let c = self.c;
        match kind {
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
            Kind::Facility => c.facilities.len(),
            Kind::Start => c.starts.len(),
            Kind::Voyage => c.voyages.len(),
            Kind::Encounter => c.encounters.len(),
            Kind::Galaxy => c.galaxies.len(),
            Kind::Band => c.bands.len(),
            Kind::ClusterType => c.cluster_types.len(),
            Kind::Trail => c.trails.len(),
            Kind::SignalSource => c.signal_sources.len(),
            Kind::Lore => c.lore.len(),
            Kind::Control => c.controls.len(),
            Kind::Page => c.wiki.values().filter(|p| p.kind == Kind::Page).count(),
        }
    }
}
