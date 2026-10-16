//! The evolution lab's page (`reference/lab.md`, GDR-0092): everything `lab.ron` holds, a table per list, every entry
//! with an anchor, and the references hand-written pages make to it (`[[lab:trial.voyages]]`, a link with its name;
//! `[[lab:size.standard.population]]`, a value). [`LAB_DOCUMENTED`] are the lists every entry of which a hand-written
//! page must name ([`super::super::hand::check`]).

use data::{Duty, Lab, LabAxis, TrialPreset, TrialPresetKind};

use super::*;

/// Where the page is, relative to the site root (without its extension).
pub(crate) const LAB_PAGE: &str = "reference/lab";

/// The lists whose every entry a hand-written page names (`[[lab:<list>.<id>]]` or one of its values): the trial
/// kinds, the trials, the goals and their presets, the genes and their presets, the pilots and the sizes.
pub(crate) const LAB_DOCUMENTED: [&str; 8] = ["kind", "trial", "goal", "goal_preset", "gene", "gene_preset", "pilot", "size"];

/// An entry of `lab.ron` as the wiki shows it: `<list>.<id>`, its name, and its values by field (HTML, links through
/// the root it was made with).
pub(crate) struct LabEntry {
    pub key: String,
    pub name: String,
    pub values: Vec<(&'static str, String)>,
}

impl LabEntry {
    /// The anchor of its row on the page.
    pub fn anchor(&self) -> String {
        self.key.replace(['.', '_'], "-")
    }
}

/// A text among the values: it wraps and reads as prose where a hand-written page puts it (`.sg-text`), not as a
/// number.
fn prose_value(html: &str) -> String {
    format!("<span class=\"sg-text\">{html}</span>")
}

fn seconds(s: f32) -> String {
    format!("{} s", format_number(s, 1))
}

fn share(x: f32) -> String {
    format!("{}%", format_number(x * 100.0, 0))
}

fn duty(d: &Option<Duty>) -> String {
    match d {
        None => "never".into(),
        Some(d) if d.off <= 0.0 => "all the time".into(),
        Some(d) => format!("{} on, {} off", seconds(d.on), seconds(d.off)),
    }
}

impl Wiki<'_> {
    /// Ships of a trial as links with their counts: `2 × Automaton raider, Automaton hunter`.
    fn lab_ships(&self, root: &str, ships: &[data::LabShips]) -> String {
        let items: Vec<String> = ships
            .iter()
            .map(|s| {
                let a = self.a(root, Kind::Ship, &s.ship);
                if s.count == 1 { a } else { format!("{} × {a}", s.count) }
            })
            .collect();
        items.join(", ")
    }

    /// A measure's name and unit (a stat's words, linked, or its own).
    fn lab_measure(&self, root: &str, id: &str) -> (String, String) {
        let lab = &self.c.lab;
        let (name, unit) = lab.measure_words(&self.c.stats, id).unwrap_or((id, ""));
        let name = match lab.measure(id).and_then(|m| m.stat.as_deref()).filter(|s| self.c.stats.contains(s)) {
            Some(stat) => self.a(root, Kind::Stat, stat),
            None => esc(name),
        };
        (name, esc(unit))
    }

    /// An axis: `Thrust/mass 0–24 m/s² in 8 cells`.
    fn lab_axis(&self, root: &str, a: &LabAxis) -> String {
        let (name, unit) = self.lab_measure(root, &a.measure);
        let unit = if unit.is_empty() { String::new() } else { format!(" {unit}") };
        format!("{name} {}–{}{unit} in {} cells", format_number(a.min, 2), format_number(a.max, 2), a.cells)
    }

    /// What a trial fights, in words with its numbers: the kind's own settings.
    fn lab_trial_values(&self, root: &str, t: &TrialPreset) -> Vec<(&'static str, String)> {
        let mut v = vec![
            ("summary", prose_value(&self.text(root, &t.summary))),
            ("kind", self.c.lab.kind(t.kind.id()).map_or_else(|| esc(t.kind.id()), |k| esc(&k.name))),
            ("weight", format_number(t.weight, 2)),
            ("seeds", t.seeds.to_string()),
        ];
        if let Some(limit) = t.time_limit {
            let words = if t.kind == TrialPresetKind::Voyages { format!("{} min", format_number(limit, 1)) } else { seconds(limit) };
            v.push(("time_limit", words));
        }
        match &t.kind {
            TrialPresetKind::EnemyShips { ships } => v.push(("ships", self.lab_ships(root, ships))),
            TrialPresetKind::Gauntlet { waves } => {
                v.push(("waves", waves.len().to_string()));
                let each: Vec<String> = waves.iter().map(|w| self.lab_ships(root, w)).collect();
                v.push(("wave_ships", each.join("; ")));
            }
            TrialPresetKind::Waves { target } => v.push(("target", target.to_string())),
            TrialPresetKind::HallOfFame { count } | TrialPresetKind::Champions { count } => v.push(("count", count.to_string())),
            TrialPresetKind::CoEvolution { size, elites, hall, anchor, check_every, champions_every, mirror } => {
                v.push(("size", share(*size)));
                v.push(("elites", elites.to_string()));
                v.push(("hall", hall.to_string()));
                v.push(("anchor", if *anchor { "yes" } else { "no" }.into()));
                v.push(("check_every", check_every.to_string()));
                v.push(("champions_every", champions_every.to_string()));
                v.push(("mirror", if *mirror { "yes" } else { "no" }.into()));
            }
            TrialPresetKind::Encounters | TrialPresetKind::BeatYourStart | TrialPresetKind::Fleet | TrialPresetKind::Voyages => {}
        }
        v.push(("default", if t.default { "on" } else { "off" }.into()));
        v
    }

    /// Every entry of `lab.ron` with its values, list by list in the page's order.
    pub(crate) fn lab_entries(&self, root: &str) -> Vec<LabEntry> {
        let lab: &Lab = &self.c.lab;
        let mut out = Vec::new();
        let mut put = |list: &str, id: &str, name: &str, values: Vec<(&'static str, String)>| {
            out.push(LabEntry { key: format!("{list}.{id}"), name: name.to_string(), values });
        };
        let goal_name = |id: &str| lab.goal(id).map_or_else(|| id.to_string(), |g| g.name.clone());
        for g in &lab.goals {
            put("goal", &g.id, &g.name, vec![("summary", prose_value(&self.text(root, &g.summary))), ("weight", format_number(g.weight, 2))]);
        }
        for p in &lab.goal_presets {
            let weights: Vec<String> = p.weights.iter().filter(|(_, w)| **w > 0.0).map(|(id, w)| format!("{} {}", esc(&goal_name(id)), format_number(*w, 2))).collect();
            put("goal_preset", &p.id, &p.name, vec![("weights", weights.join(", "))]);
        }
        for k in &lab.kinds {
            put("kind", &k.id, &k.name, vec![("summary", prose_value(&self.text(root, &k.summary)))]);
        }
        for t in &lab.trials {
            put("trial", &t.id, &t.name, self.lab_trial_values(root, t));
        }
        for g in &lab.genes {
            put("gene", &g.id, &g.name, vec![("summary", prose_value(&self.text(root, &g.summary)))]);
        }
        for p in &lab.gene_presets {
            let genes: Vec<String> = p.genes.iter().map(|g| esc(lab.gene(g).map_or(g.as_str(), |g| g.name.as_str()))).collect();
            put("gene_preset", &p.id, &p.name, vec![("genes", genes.join(", "))]);
        }
        for p in &lab.pilots {
            let f = &p.flies;
            put(
                "pilot",
                &p.id,
                &p.name,
                vec![
                    ("summary", prose_value(&self.text(root, &p.summary))),
                    ("lock", if f.lock { "yes" } else { "no" }.into()),
                    ("close_to", share(f.close_to)),
                    ("stick", duty(&f.stick)),
                    ("fire", duty(&f.fire)),
                ],
            );
        }
        for z in &lab.sizes {
            put(
                "size",
                &z.id,
                &z.name,
                vec![("summary", prose_value(&self.text(root, &z.summary))), ("population", z.population.to_string()), ("generations", z.generations.to_string()), ("children", (z.population * z.generations).to_string())],
            );
        }
        for m in &lab.measures {
            let (name, unit) = self.lab_measure(root, &m.id);
            let plain = lab.measure_words(&self.c.stats, &m.id).map_or(m.id.as_str(), |(n, _)| n);
            put("measure", &m.id, plain, vec![("summary", prose_value(&self.text(root, &m.summary))), ("unit", if unit.is_empty() { "none".into() } else { unit }), ("name", name)]);
        }
        out
    }

    /// The values that belong to no entry: the spreads' axes, the budget and the arms race's palettes.
    pub(crate) fn lab_values(&self, root: &str) -> Vec<(String, String)> {
        let lab = &self.c.lab;
        let mut out = Vec::new();
        for sp in &lab.spreads {
            let axes: Vec<String> = sp.axes.iter().map(|a| self.lab_measure(root, &a.measure).0).collect();
            out.push((format!("spread.{}", sp.id), axes.join(" × ")));
        }
        if let Some(b) = lab.budget {
            out.push(("budget.layer_open".into(), format!("{} ms", format_number(b.layer_open, 1))));
            out.push(("budget.elsewhere".into(), format!("{} ms", format_number(b.elsewhere, 1))));
        }
        for p in &lab.palettes {
            let parts: Vec<String> = p.parts.iter().map(|id| self.a(root, Kind::Part, id)).collect();
            out.push((format!("palette.{}", p.faction), parts.join(", ")));
        }
        out
    }

    /// The page: a lead, then one table per list of `lab.ron`.
    pub(crate) fn lab_page(&self) -> String {
        let lab = &self.c.lab;
        let entries = self.lab_entries(UP);
        let of = |list: &'static str| entries.iter().filter(move |e| e.key.split_once('.').is_some_and(|(l, _)| l == list));
        let value = |e: &LabEntry, field: &str| e.values.iter().find(|(f, _)| *f == field).map_or(String::new(), |(_, v)| v.clone());
        let mut s = heading(1, "Evolution lab", "evolution-lab");
        s.push_str(&wide_block(
            "sg-section",
            &format!(
                "<p class=\"lead\">The words and numbers of the {} lab: its goals, trials, genes, stand-in pilots, sizes and measures, as the content's <code>lab.ron</code> holds them. A run keeps what it was made with, so a change here changes no run.</p>\n",
                self.a(UP, Kind::Facility, "evolution")
            ),
        ));
        let table = |s: &mut String, title: &str, id: &str, head: &[&str], rows: Vec<(String, Vec<String>)>| {
            s.push_str(&heading(2, title, id));
            let mut t = String::from("<table class=\"list\">\n<tr>");
            for h in head {
                write!(t, "<th>{h}</th>").unwrap();
            }
            t.push_str("</tr>\n");
            for (anchor, cells) in rows {
                write!(t, "<tr id=\"{anchor}\">").unwrap();
                for (k, c) in cells.iter().enumerate() {
                    if k == 0 {
                        write!(t, "<td class=\"name\">{c}</td>").unwrap();
                    } else {
                        write!(t, "<td>{c}</td>").unwrap();
                    }
                }
                t.push_str("</tr>\n");
            }
            t.push_str("</table>\n");
            s.push_str(&wide_block("sg-section", &t));
        };
        let rows = |list: &'static str, fields: &[&str]| -> Vec<(String, Vec<String>)> {
            of(list).map(|e| (e.anchor(), std::iter::once(esc(&e.name)).chain(fields.iter().map(|f| value(e, f))).collect())).collect()
        };
        table(&mut s, "Goals", "goals", &["Goal", "What it scores", "Weight in a new run"], rows("goal", &["summary", "weight"]));
        table(&mut s, "Goal presets", "goal-presets", &["Preset", "Weights"], rows("goal_preset", &["weights"]));
        table(&mut s, "Trial kinds", "trial-kinds", &["Kind", "What it fights"], rows("kind", &["summary"]));
        let trials: Vec<(String, Vec<String>)> = of("trial")
            .map(|e| {
                let numbers = format!(
                    "weight&nbsp;{}<br>{}&nbsp;seeds<br>{}{}",
                    value(e, "weight"),
                    value(e, "seeds"),
                    value(e, "time_limit"),
                    if value(e, "default") == "on" { "<br><strong>on&nbsp;by&nbsp;default</strong>" } else { "" }
                );
                let fights: Vec<String> = ["ships", "wave_ships", "target", "count", "size", "elites", "hall", "check_every", "champions_every"]
                    .iter()
                    .filter_map(|f| {
                        let v = value(e, f);
                        (!v.is_empty()).then(|| format!("<span class=\"note\">{}</span> {v}", f.replace('_', " ")))
                    })
                    .collect();
                let about = if fights.is_empty() { value(e, "summary") } else { format!("{}<br>{}", value(e, "summary"), fights.join("<br>")) };
                (e.anchor(), vec![esc(&e.name), value(e, "kind"), about, numbers])
            })
            .collect();
        table(&mut s, "Trials", "trials", &["Trial", "Kind", "What it fights", "In a new run"], trials);
        table(&mut s, "Genes", "genes", &["Gene", "What it changes"], rows("gene", &["summary"]));
        table(&mut s, "What may change: presets", "gene-presets", &["Preset", "Genes"], rows("gene_preset", &["genes"]));
        table(&mut s, "Stand-in pilots", "pilots", &["Pilot", "How it flies", "Locks", "Closes to", "Stick", "Triggers"], rows("pilot", &["summary", "lock", "close_to", "stick", "fire"]));
        table(&mut s, "Sizes", "sizes", &["Size", "", "Children a generation", "Generations"], rows("size", &["summary", "population", "generations"]));
        let measures: Vec<(String, Vec<String>)> = of("measure").map(|e| (e.anchor(), vec![value(e, "name"), value(e, "unit"), value(e, "summary")])).collect();
        table(&mut s, "Measures", "measures", &["Measure", "Unit", "What it measures"], measures);
        let spreads: Vec<(String, Vec<String>)> = lab
            .spreads
            .iter()
            .map(|sp| {
                let axes: Vec<String> = sp.axes.iter().map(|a| self.lab_axis(UP, a)).collect();
                let when = if sp.id == "design" { "runs whose design may change" } else { "runs that change only the behavior or the assists" };
                (format!("spread-{}", sp.id), vec![esc(&sp.id), when.to_string(), axes.join(" × ")])
            })
            .collect();
        table(&mut s, "Spreads", "spreads", &["Spread", "For", "Axes"], spreads);
        let values = self.lab_values(UP);
        let get = |k: &str| values.iter().find(|(key, _)| key == k).map_or(String::new(), |(_, v)| v.clone());
        let mut other = vec![(
            "budget".to_string(),
            vec!["Time a frame".into(), format!("{} with the lab open, {} elsewhere at home and on the screens; none in flight", get("budget.layer_open"), get("budget.elsewhere"))],
        )];
        for p in &lab.palettes {
            other.push((format!("palette-{}", p.faction), vec![format!("Arms race: {}", self.a(UP, Kind::Faction, &p.faction)), format!("may build with {} besides its ships' parts", get(&format!("palette.{}", p.faction)))]));
        }
        table(&mut s, "Running a run", "running", &["", ""], other);
        s
    }
}
