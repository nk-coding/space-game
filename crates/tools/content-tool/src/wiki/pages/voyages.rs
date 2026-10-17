//! The pages of voyages and encounters, and their tables.

use super::*;

impl Wiki<'_> {
    // Voyages and encounters in tables

    /// An encounter's fighting ships, one per line: "Automaton raider ×2–3".
    pub(crate) fn encounter_ships(&self, root: &str, e: &Encounter) -> String {
        let v: Vec<String> = e
            .ships
            .iter()
            .map(|x| {
                format!(
                    "{}&nbsp;×{}",
                    self.a(root, Kind::Ship, &x.ship),
                    count_range(x.count)
                )
            })
            .collect();
        if v.is_empty() {
            "—".into()
        } else {
            v.join("<br>")
        }
    }

    /// An encounter's wrecks, one per line: "Sample early ×1, 50% of parts gone".
    pub(crate) fn encounter_wrecks(&self, root: &str, e: &Encounter) -> String {
        let v: Vec<String> = e
            .wrecks
            .iter()
            .map(|w| {
                format!(
                    "{}&nbsp;×{}, {}",
                    self.a(root, Kind::Ship, &w.ship),
                    count_range(w.count),
                    parts_gone(w.damage)
                )
            })
            .collect();
        if v.is_empty() {
            "—".into()
        } else {
            v.join("<br>")
        }
    }

    /// Voyages as table rows: name with its band, summary, how many sites, expected length.
    pub(crate) fn voyage_table(&self, root: &str, voyages: &[&Voyage]) -> String {
        if voyages.is_empty() {
            return none();
        }
        let mut s = String::from("<table class=\"list\">\n");
        for v in voyages {
            writeln!(
                s,
                "<tr><td class=\"name\">{}<br><span class=\"note\">{} band</span></td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>",
                self.a(root, Kind::Voyage, &v.id),
                esc(&v.band),
                self.text(root, &v.summary),
                sites(v.site_count()),
                minutes(v.minutes)
            )
            .unwrap();
        }
        s + "</table>\n"
    }

    /// Encounters with a header row: name, summary, ships, wrecks, wake radius.
    pub(crate) fn encounter_table(&self, root: &str, encounters: &[&Encounter]) -> String {
        if encounters.is_empty() {
            return none();
        }
        let mut s = String::from(
            "<table class=\"list\">\n<tr><th></th><th></th><th>Ships</th><th>Wrecks</th><th class=\"num\">Wake radius</th></tr>\n",
        );
        for e in encounters {
            writeln!(
                s,
                "<tr><td class=\"name\">{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"num\">{}</td></tr>",
                self.a(root, Kind::Encounter, &e.id),
                self.text(root, &e.summary),
                self.encounter_ships(root, e),
                self.encounter_wrecks(root, e),
                metres(e.wake_radius)
            )
            .unwrap();
        }
        s + "</table>\n"
    }
}

impl Wiki<'_> {
    pub(crate) fn voyage_page(&self, v: &Voyage) -> String {
        let root = UP;
        let mut facts = vec![
            ("Band".to_string(), esc(&v.band)),
            ("Radius".into(), metres(v.radius)),
            (
                "Site spacing".into(),
                format!("{}–{}", format_number(v.spacing.0, 1), metres(v.spacing.1)),
            ),
            ("Expected length".into(), minutes(v.minutes)),
            ("Sites".into(), v.site_count().to_string()),
        ];
        if let Some(g) = v.goal() {
            facts.push(("Goal".into(), self.a(root, Kind::Encounter, &g.encounter)));
        }
        let starts = crate::starts_opening(self.c, &v.id);
        if let Some(l) = self.links(root, starts.iter().map(|s| (Kind::Start, s.id.clone()))) {
            facts.push(("Open at the start of".into(), l));
        }
        let mut t = format!(
            "<p>Each launch places these {} in the cluster and rolls how many ships and wrecks each holds; clearing the goal clears the voyage.</p>\n<table class=\"list\">\n<tr><th>Encounter</th><th>Ships</th><th>Wrecks</th><th class=\"num\">Sites</th></tr>\n",
            sites(v.site_count())
        );
        for site in &v.sites {
            let (ships, wrecks) = self
                .c
                .encounters
                .get(&site.encounter)
                .map(|e| {
                    (
                        self.encounter_ships(root, e),
                        self.encounter_wrecks(root, e),
                    )
                })
                .unwrap_or_default();
            writeln!(
                t,
                "<tr><td class=\"name\">{}{}</td><td>{ships}</td><td>{wrecks}</td><td class=\"num\">×{}</td></tr>",
                self.a(root, Kind::Encounter, &site.encounter),
                if site.goal { GOAL } else { "" },
                site.count
            )
            .unwrap();
        }
        t.push_str("</table>\n");
        let mut sections = vec![("Sites".to_string(), t)];
        sections.extend(self.scenery_section(root, &v.scenery));
        if let Some(text) = &v.cleared {
            sections.push((
                "When cleared".into(),
                format!(
                    "<blockquote><p>{}</p></blockquote>\n<p class=\"note\">The debrief shows this the first time its goal is cleared.</p>\n",
                    self.text(root, text)
                ),
            ));
        }
        let after: Vec<&Voyage> = v
            .opens_after
            .iter()
            .filter_map(|id| self.c.voyages.get(id))
            .collect();
        let after = if after.is_empty() && !starts.is_empty() {
            "<p class=\"none\">None: it is open at the start.</p>\n".into()
        } else if after.is_empty() {
            none()
        } else {
            format!(
                "<p class=\"note\">It opens once the goal of each of these has been cleared.</p>\n{}",
                self.voyage_table(root, &after)
            )
        };
        sections.push(("Opens after".into(), after));
        sections.push((
            "Opens".into(),
            self.voyage_table(root, &crate::opens(self.c, &v.id)),
        ));
        self.entry(
            v,
            EntryParts {
                facts,
                sections,
                ..Default::default()
            },
        )
    }

    pub(crate) fn encounter_page(&self, e: &Encounter) -> String {
        let root = UP;
        let mut facts = vec![("Wake radius".to_string(), metres(e.wake_radius))];
        if !e.ships.is_empty() {
            let n = sum_ranges(e.ships.iter().map(|x| x.count));
            facts.push(("Ships".into(), count_range(n)));
        }
        if !e.wrecks.is_empty() {
            let n = sum_ranges(e.wrecks.iter().map(|w| w.count));
            facts.push(("Wrecks".into(), count_range(n)));
        }
        // Ships and wrecks as rows: the ship drawn small, its name, a few words, how many.
        let row = |s: &Blueprint, about: String, count: (u32, u32)| {
            format!(
                "<tr><td class=\"pic\">{}</td><td class=\"name\">{}</td><td>{about}</td><td class=\"num\">×{}</td></tr>\n",
                self.ship_chip(s),
                self.a(root, Kind::Ship, &s.id),
                count_range(count)
            )
        };
        let ships: String = e
            .ships
            .iter()
            .filter_map(|x| {
                let s = self.c.ships.get(&x.ship)?;
                Some(row(s, self.text(root, &s.summary), x.count))
            })
            .collect();
        let ships = if ships.is_empty() {
            none()
        } else {
            format!(
                "<table class=\"list\">\n{ships}</table>\n<p class=\"note\">Each launch rolls how many. They hold until a ship of yours comes within {} of the site, then fly their behavior.</p>\n",
                metres(e.wake_radius)
            )
        };
        let wrecks: String = e
            .wrecks
            .iter()
            .filter_map(|w| {
                Some(row(
                    self.c.ships.get(&w.ship)?,
                    parts_gone(w.damage),
                    w.count,
                ))
            })
            .collect();
        let wrecks = if wrecks.is_empty() {
            none()
        } else {
            format!(
                "<table class=\"list\">\n{wrecks}</table>\n<p class=\"note\">They lie there as salvage. Each launch rolls how many and which of their parts are gone.</p>\n"
            )
        };
        let mut used = String::new();
        for (v, n) in crate::voyages_using(self.c, &e.id) {
            let goal = v.goal().is_some_and(|g| g.encounter == e.id);
            writeln!(
                used,
                "<tr><td class=\"name\">{}{}</td><td>{}</td><td class=\"num\">{}</td></tr>",
                self.a(root, Kind::Voyage, &v.id),
                if goal { GOAL } else { "" },
                self.text(root, &v.summary),
                sites(n)
            )
            .unwrap();
        }
        let used = if used.is_empty() {
            none()
        } else {
            format!("<table class=\"list\">\n{used}</table>\n")
        };
        let sections = vec![
            ("Ships".to_string(), ships),
            ("Wrecks".into(), wrecks),
            ("Used by voyages".into(), used),
        ];
        self.entry(
            e,
            EntryParts {
                facts,
                sections,
                ..Default::default()
            },
        )
    }
}
