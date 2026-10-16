//! The career's pages: research families, research nodes, starts; and what salvage gives, research costs and
//! unlocks wherever they show.

use super::*;

impl Wiki<'_> {
    // Career: salvage, research costs and unlocks

    /// A family's name, linked, after its colour dot.
    pub(crate) fn family_link(&self, root: &str, id: &str) -> String {
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
    pub(crate) fn data_amount(&self, root: &str, family: &str, v: f32) -> String {
        format!(
            "{} {} data",
            format_number(v, 2),
            self.a(root, Kind::Family, family)
        )
    }

    /// Materials (id → amount) as "2 Programmable matter" items, in material order.
    pub(crate) fn material_items(&self, root: &str, amounts: &BTreeMap<String, u32>) -> Vec<String> {
        let mut v: Vec<(&String, &u32)> = amounts.iter().collect();
        v.sort_by_key(|(id, _)| self.c.materials.position(id).unwrap_or(usize::MAX));
        v.into_iter()
            .map(|(id, n)| format!("{n} {}", self.a_badged(root, Kind::Material, id)))
            .collect()
    }

    /// A research cost as items: data per family (in family order), then materials.
    pub(crate) fn cost_items(&self, root: &str, cost: &data::ResearchCost) -> Vec<String> {
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
    pub(crate) fn unlock_short(&self, root: &str, u: &Unlock) -> String {
        match u {
            Unlock::Part(id) => self.a_badged(root, Kind::Part, id),
            Unlock::Material(id) => self.a_badged(root, Kind::Material, id),
            Unlock::Frame(id) => self.a_badged(root, Kind::Frame, id),
            Unlock::Facility(id) => self.a_badged(root, Kind::Facility, id),
            Unlock::MindCapacity(x) => format!("Mind capacity&nbsp;+{x}"),
        }
    }

    /// Research nodes as table rows: name with its kind, summary, what it unlocks, cost.
    pub(crate) fn research_table(&self, root: &str, nodes: &[&ResearchNode]) -> String {
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

    /// "Unlocked by" on a part, material or frame page: the nodes that unlock it. Left out for an entry a new
    /// game has that no node names.
    pub(crate) fn unlocked_by(&self, unlock: &Unlock, locked: bool) -> Option<(String, String)> {
        let nodes = crate::unlocked_by(self.c, unlock);
        if nodes.is_empty() && !locked {
            return None;
        }
        let html = if nodes.is_empty() {
            "<p class=\"none\">No research unlocks it yet.</p>\n".into()
        } else {
            self.research_table(UP, &nodes)
        };
        Some(("Unlocked by".into(), html))
    }

    /// "Salvage" on a part page: what it gives when the haul is dismantled at home (GDR-0063). Hull parts give
    /// research data by their material, per cell of hull area.
    pub(crate) fn part_salvage(&self, root: &str, p: &Part) -> String {
        if p.hull {
            let rows: Vec<String> = self
                .c
                .materials
                .iter()
                .filter_map(|m| {
                    let s = m.salvage.as_ref()?;
                    Some(format!(
                        "<tr><th>{}</th><td>{}</td></tr>\n",
                        self.a_badged(root, Kind::Material, &m.id),
                        self.data_amount(root, &s.family, s.data * p.area())
                    ))
                })
                .collect();
            if rows.is_empty() {
                return not_salvaged();
            }
            let area = p.area();
            let cells = if area == 1.0 { "cell" } else { "cells" };
            return format!(
                "<p class=\"note\">By its material, per cell of hull area; this part is {} {cells}.</p>\n<table class=\"list\">\n{}</table>\n",
                format_number(area, 2),
                rows.concat()
            );
        }
        let Some(s) = &p.salvage else {
            return not_salvaged();
        };
        let mut intact = vec![self.data_amount(root, &s.family, s.data)];
        intact.extend(self.material_items(root, &s.materials));
        let fragment = self.material_items(root, &s.fragment);
        format!(
            "<table class=\"list\">\n<tr><th>Intact part</th><td>{}</td></tr>\n<tr><th>Destroyed part</th><td>{}</td></tr>\n</table>\n",
            intact.join(", "),
            if fragment.is_empty() {
                "leaves nothing".to_string()
            } else {
                format!("leaves a fragment of {}", fragment.join(", "))
            }
        )
    }

    /// "Salvage" on an armour material's page: research data per cell of hull area.
    pub(crate) fn material_salvage(&self, root: &str, m: &Material) -> String {
        match &m.salvage {
            Some(s) => format!(
                "<table class=\"list\">\n<tr><th>Per cell of hull area</th><td>{}</td></tr>\n</table>\n<p class=\"note\">Hull parts made from it give this per cell of their area; slopes count half.</p>\n",
                self.data_amount(root, &s.family, s.data)
            ),
            None => not_salvaged(),
        }
    }
}

impl Wiki<'_> {
    pub(crate) fn family_page(&self, f: &Family) -> String {
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

    pub(crate) fn research_page(&self, n: &ResearchNode) -> String {
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
                Unlock::Facility(id) => match self.c.facilities.get(id) {
                    Some(f) => (String::new(), self.a_badged(root, Kind::Facility, id), self.text(root, &f.summary)),
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

    /// A facility of the home shipyard: whether a new game has it, and the research that opens it.
    pub(crate) fn facility_page(&self, f: &data::Facility) -> String {
        let facts = vec![("At home".to_string(), if f.locked { "dark until researched".into() } else { "open from the start".into() })];
        let sections = self.unlocked_by(&Unlock::Facility(f.id.clone()), f.locked).into_iter().collect();
        self.entry(
            f,
            EntryParts {
                badges: badges(f.locked, &[]),
                facts,
                sections,
                ..Default::default()
            },
        )
    }

    pub(crate) fn start_page(&self, st: &Start) -> String {
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
        let voyages: Vec<&Voyage> = st
            .voyages
            .iter()
            .filter_map(|id| self.c.voyages.get(id))
            .collect();
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
            ("Open voyages".into(), self.voyage_table(root, &voyages)),
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
}
