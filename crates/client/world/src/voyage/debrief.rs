//! A voyage's debrief as data (GDR-0065): what came home and what it gave. [`DebriefData`] is built from the
//! career before and after the voyage's end (`voyage::end`), the voyage's haul and what the flight counted
//! ([`DebriefStats`]: sites, kills, ships lost); [`Debrief`] holds the last one for home's panel (`debrief.rs`).
//! Kills and ships lost are not in the save; the career's history keeps the rest (home's LOG, GDR-0091).

use std::collections::BTreeMap;

use bevy::prelude::*;
use data::career::{Haul, HaulItem, Lead, Outcome, SignalState, VoyageResult, unload};
use data::{Career, Content, Reward};
use kit::ui_kit::Tone;

/// How a part came home: intact (to the workshop, or hull taken apart at once), broken, or as a fragment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kept {
    Workshop,
    Whole,
    Broken,
    Fragment,
}

/// What one part's items gave: how many, research data per family, materials per id.
type Gave = (u32, BTreeMap<String, f32>, BTreeMap<String, u32>);

/// What the flight counted, beside the career's last haul.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DebriefStats {
    /// Sites done and all (`SITES 4 / 4`).
    pub sites: Option<(u32, u32)>,
    pub kills: Option<u32>,
    pub ships_lost: Option<u32>,
    /// Items left in the wrecks (a lost voyage).
    pub left: u32,
    /// Why the voyage was a practice run (not saved).
    pub practice: Option<String>,
}

/// One kind of item that came home.
#[derive(Clone, Debug, PartialEq)]
pub struct PartRow {
    pub name: String,
    pub count: u32,
    /// What it gave: `KINETIC +18  ·  PROGRAMMABLE MATTER +1`.
    pub gives: String,
    /// The family colour key of its research data (none: neutral).
    pub color: Option<String>,
}

/// One family's research data.
#[derive(Clone, Debug, PartialEq)]
pub struct FamilyLine {
    pub name: String,
    pub gained: f32,
    pub store: f32,
    pub color: Option<String>,
}

/// A voyage's debrief.
#[derive(Clone, Debug, PartialEq)]
pub struct DebriefData {
    pub voyage: String,
    pub name: String,
    pub band: String,
    pub outcome: Outcome,
    pub ticks: u64,
    pub stats: DebriefStats,
    /// Items dismantled (parts and fragments).
    pub items: u32,
    /// The goal was cleared for the first time, and the voyage's words for it.
    pub first_clear: bool,
    pub cleared_text: Option<String>,
    pub rows: Vec<PartRow>,
    pub families: Vec<FamilyLine>,
    /// Materials gained: name, amount.
    pub materials: Vec<(String, u32)>,
    /// Research nodes that can be paid now (names, content order).
    pub ready: Vec<String>,
    /// Voyages this one opened: the signals it found (names).
    pub opened: Vec<String>,
    /// What it found in the galaxy ([`galaxy_finds`]): clusters charted (teal), signals heard and how far each came
    /// (gold).
    pub finds: Vec<(String, Tone)>,
    /// Data cores it brought home: logs to decode at the LOG.
    pub logs: usize,
    pub practice: Option<String>,
}

/// How a voyage ended, in words and their tone (the debrief's chip, the LOG's rows).
pub fn outcome_words(outcome: Outcome) -> (&'static str, Tone) {
    match outcome {
        Outcome::Cleared => ("GOAL CLEARED", Tone::Teal),
        Outcome::Recalled => ("RECALLED", Tone::Label),
        Outcome::Lost => ("LOST", Tone::Coral),
    }
}

/// Research data as the debrief writes it: whole numbers plain, else one decimal.
pub fn amount(x: f32) -> String {
    if (x - x.round()).abs() < 0.05 { format!("{:.0}", x.round()) } else { format!("{x:.1}") }
}

impl DebriefData {
    /// The debrief of `last` (the voyage's result), from the career `before` and `after` the end, the `haul` that
    /// came home (its items, for the rows) and what the flight counted.
    pub fn build(content: &Content, before: &Career, after: &Career, last: &VoyageResult, haul: &Haul, stats: DebriefStats) -> DebriefData {
        let voyage = content.voyages.get(&last.voyage);
        let was_cleared = before.voyage(&last.voyage).is_some_and(|r| r.cleared);
        let first_clear = !was_cleared && after.voyage(&last.voyage).is_some_and(|r| r.cleared);
        let family = |id: &str| content.families.get(id);
        let upper = |s: &str| s.to_uppercase();

        // Rows: per part (intact, broken and fragments apart), what its items gave, most first; intact parts that are
        // not hull went to the workshop (GDR-0108). Data cores are no part (the finds, E6).
        let mut rows: BTreeMap<(Kept, String), Gave> = BTreeMap::new();
        for (item, &n) in &haul.items {
            let one = unload(content, &[(item.clone(), n)].into_iter().collect());
            let key = match item {
                HaulItem::Part { part, .. } if content.parts.get(part).is_some_and(|p| !p.hull) => (Kept::Workshop, part.clone()),
                HaulItem::Part { part, .. } => (Kept::Whole, part.clone()),
                HaulItem::Broken { part, .. } => (Kept::Broken, part.clone()),
                HaulItem::Fragment { part } => (Kept::Fragment, part.clone()),
                HaulItem::DataCore { .. } => continue,
            };
            let row = rows.entry(key).or_default();
            row.0 = row.0.saturating_add(n);
            for (f, x) in one.data {
                *row.1.entry(f).or_default() += x;
            }
            for (m, x) in one.materials {
                *row.2.entry(m).or_default() += x;
            }
        }
        let mut rows: Vec<PartRow> = rows
            .into_iter()
            .map(|((kept, part), (count, data, materials))| {
                let base = content.parts.get(&part).map_or(part.clone(), |p| p.name.clone());
                let name = match kept {
                    Kept::Fragment => format!("{base} fragment"),
                    Kept::Broken => format!("{base}, broken"),
                    Kept::Workshop => format!("{base}, intact"),
                    Kept::Whole => base,
                };
                let mut gives: Vec<String> = data.iter().map(|(f, x)| format!("{} +{}", upper(family(f).map_or(f, |f| &f.name)), amount(*x))).collect();
                if kept == Kept::Workshop {
                    gives.push("SORT BELOW".into());
                }
                gives.extend(materials.iter().map(|(m, x)| format!("{} +{x}", upper(content.materials.get(m).map_or(m, |m| &m.name)))));
                let color = data.keys().next().and_then(|f| family(f)).and_then(|f| f.color.clone());
                PartRow { name, count, gives: gives.join("  ·  "), color }
            })
            .collect();
        rows.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));

        let families = last
            .report
            .data
            .iter()
            .map(|(f, &gained)| FamilyLine {
                name: upper(family(f).map_or(f, |f| &f.name)),
                gained,
                store: after.data().get(f).copied().unwrap_or(0.0),
                color: family(f).and_then(|f| f.color.clone()),
            })
            .collect();
        let materials = last.report.materials.iter().map(|(m, &n)| (content.materials.get(m).map_or(m.clone(), |m| m.name.clone()), n)).collect();
        let ready = content.research.iter().filter(|n| after.can_pay(content, &n.id).is_ok()).map(|n| n.name.clone()).collect();
        let open_before = before.open_voyages(content);
        let opened = after
            .open_voyages(content)
            .into_iter()
            .filter(|v| !open_before.contains(v))
            .map(|v| content.voyages.get(v).map_or(v.to_string(), |v| v.name.clone()))
            .collect();
        let cluster = last.cluster.and_then(|k| after.exploration()?.skeleton().clusters.get(k as usize));
        let band = voyage.map(|v| v.band.as_str()).or(cluster.map(|c| c.band.as_str())).unwrap_or_default();
        DebriefData {
            voyage: last.voyage.clone(),
            name: voyage.map_or(last.voyage.clone(), |v| v.name.clone()),
            band: upper(&band.replace('_', " ")),
            finds: galaxy_finds(content, before, after, last.cluster),
            logs: after.exploration().map_or(0, |g| g.logs().len()).saturating_sub(before.exploration().map_or(0, |g| g.logs().len())),
            outcome: last.outcome,
            ticks: last.ticks,
            practice: stats.practice.clone(),
            stats,
            items: last.report.items(),
            first_clear,
            cleared_text: first_clear.then(|| voyage.and_then(|v| v.cleared.as_deref()).map(|t| content.render_plain(t))).flatten(),
            rows,
            families,
            materials,
            ready,
            opened,
        }
    }

    /// The panel's head line.
    pub fn head(&self) -> (&'static str, Tone) {
        match (&self.practice, self.outcome) {
            (Some(_), _) => ("PRACTICE RUN  ·  NOT SAVED", Tone::Gold),
            (None, Outcome::Lost) => ("SHIP LOST  ·  RECALLED HOME", Tone::Coral),
            (None, _) => ("VOYAGE HOME  ·  HAUL UNLOADED", Tone::Label),
        }
    }

    /// The outcome chip.
    pub fn outcome_chip(&self) -> (&'static str, Tone) {
        outcome_words(self.outcome)
    }

    /// One line per signal found and per find in the galaxy, with its tone: `SIGNAL FOUND · HALVEX DRIFT` (teal),
    /// `CHARTED · VARDIS` (teal), `SIGNAL NARROWED · A GATE` (gold), `1 LOG TO DECODE · AT THE LOG` (gold).
    pub fn signals(&self) -> Vec<(String, Tone)> {
        let opened = self.opened.iter().map(|v| (format!("SIGNAL FOUND  ·  {}", v.to_uppercase()), Tone::Teal));
        let logs = (self.logs > 0).then(|| (format!("{} {} TO DECODE  ·  AT THE LOG", self.logs, if self.logs == 1 { "LOG" } else { "LOGS" }), Tone::Gold));
        opened.chain(self.finds.iter().cloned()).chain(logs).collect()
    }

    /// Something to see on the map: a voyage opened, a cluster charted or a signal that came on.
    pub fn to_map(&self) -> bool {
        !self.opened.is_empty() || !self.finds.is_empty()
    }
}

/// What a voyage found in the galaxy, from the career before and after it (galaxy plan E6, GDR-0097), as the
/// debrief's lines: the voyage's `cluster` on its first launch (the launch charted it) and any other cluster charted
/// meanwhile (not a slice voyage's), each signal heard or come further (`SIGNAL HEARD · A GATE`, then `NARROWED`,
/// `LOCATED`, `FOUND`, `FOLLOWED` for one done), what a trail's end gave (`CACHE OPENED · LOGIC +8`, `A MEMORY · THE
/// LAST LAUNCH · READ IT AT THE LOG`) and each gate opened (`GATE OPEN · THE OUTER REACH`, coral).
pub fn galaxy_finds(content: &Content, before: &Career, after: &Career, cluster: Option<u32>) -> Vec<(String, Tone)> {
    let (Some(a), Some(b)) = (before.exploration(), after.exploration()) else { return Vec::new() };
    let pinned: Vec<u32> = b.pins().values().copied().collect();
    let first = |k: &u32| Some(*k) == cluster && b.charted().get(k).is_some_and(|r| r.launched == 1);
    let mut out: Vec<(String, Tone)> = b
        .charted()
        .keys()
        .filter(|k| (first(k) || !a.is_charted(**k)) && !pinned.contains(k))
        .filter_map(|k| b.skeleton().clusters.get(*k as usize))
        .map(|c| (format!("CHARTED  ·  {}", c.name.to_uppercase()), Tone::Teal))
        .collect();
    for r in b.signals() {
        let Lead::Trail(s) = r.lead else { continue };
        let word = match a.signal(s).map(|x| x.state) {
            None => "HEARD",
            Some(was) if was == r.state => continue,
            Some(_) => match r.state {
                SignalState::Fresh => continue,
                SignalState::Narrowed => "NARROWED",
                SignalState::Located => "LOCATED",
                SignalState::Found => "FOUND",
                SignalState::Done => "FOLLOWED",
            },
        };
        out.push((format!("SIGNAL {word}  ·  {}", super::signal_words(content, after, s).0), Tone::Gold));
        // A cache's or a memory's reward (a gate's is the gate below, a graveyard's the haul).
        let reward = b.skeleton().trails.get(s.trail as usize).and_then(|t| content.trails.get(&t.trail)).map(|t| t.reward);
        if r.state == SignalState::Done
            && matches!(reward, Some(Reward::Cache | Reward::Lore))
            && let Some((head, sub, tone)) = super::reward_words(content, after, s)
        {
            out.push((format!("{head}  ·  {sub}"), tone));
        }
    }
    // Gates opened: a trail's guardian beaten, or the Warden (the slice's voyages are the rim's first gate trail).
    let bands = content.galaxies.get(&b.skeleton().galaxy).map_or(&[][..], |g| g.bands.as_slice());
    for k in b.gates().iter().filter(|k| !a.gates().contains(k)) {
        let Some(c) = b.skeleton().clusters.get(*k as usize) else { continue };
        let next = bands.iter().position(|x| *x == c.band).and_then(|j| bands.get(j + 1)).and_then(|x| content.bands.get(x));
        out.push((format!("GATE OPEN  ·  {}", next.map_or_else(|| "THE NEXT BAND".to_string(), |x| x.name.to_uppercase())), Tone::Coral));
    }
    out
}

/// The last debrief and whether its panel is open (a resource).
#[derive(Resource, Default, Debug)]
pub struct Debrief {
    pub data: Option<DebriefData>,
    pub open: bool,
}

impl Debrief {
    /// A voyage ended: its panel opens at home.
    pub fn show(&mut self, data: DebriefData) {
        self.data = Some(data);
        self.open = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::career::VoyageEnd;

    fn career(content: &Content) -> Career {
        Career::new(content, "rim_awakening", 4207).unwrap()
    }

    /// A first clear: rows by part with what they gave, data per family with the store, research ready, the next
    /// voyage open, the voyage's words.
    #[test]
    fn a_cleared_voyage_debriefs_its_haul() {
        let content = data::base_content();
        let before = {
            let mut c = career(&content);
            c.voyage_launched(&content, "yard_approach").unwrap();
            c
        };
        let haul: Haul = [
            (HaulItem::Broken { part: "automaton_cannon".into(), material: None }, 6),
            (HaulItem::Part { part: "thruster".into(), material: None }, 3),
            (HaulItem::Fragment { part: "automaton_core".into() }, 1),
        ]
        .into_iter()
        .collect();
        let mut after = before.clone();
        let last = after.voyage_ended(&content, "yard_approach", VoyageEnd::Recalled { haul: haul.clone(), cleared: true, ticks: 30 * 462 }).unwrap().clone();
        let stats = DebriefStats { sites: Some((4, 4)), kills: Some(9), ships_lost: Some(0), ..default() };
        let d = DebriefData::build(&content, &before, &after, &last, &haul, stats);
        assert_eq!((d.name.as_str(), d.band.as_str(), d.items, d.first_clear), ("Shipyard approach", "RIM", 10, true));
        assert_eq!(d.head(), ("VOYAGE HOME  ·  HAUL UNLOADED", Tone::Label));
        assert_eq!(d.outcome_chip().0, "GOAL CLEARED");
        let cannon = &d.rows[0];
        assert_eq!((cannon.name.as_str(), cannon.count), ("Automaton cannon, broken", 6));
        assert!(cannon.gives.starts_with("KINETIC WEAPONS +"), "{}", cannon.gives);
        // Intact parts go to the workshop to be sorted (GDR-0108).
        let thruster = d.rows.iter().find(|r| r.name == "Thruster, intact").unwrap();
        assert_eq!((thruster.count, thruster.gives.as_str()), (3, "SORT BELOW"));
        let fragment = d.rows.iter().find(|r| r.name.ends_with("fragment")).unwrap();
        assert!(fragment.gives.contains("PROGRAMMABLE MATTER +1"), "{}", fragment.gives);
        let kinetic = d.families.iter().find(|f| f.name == "KINETIC WEAPONS").unwrap();
        assert_eq!(kinetic.gained, kinetic.store);
        assert_eq!(d.opened, ["Halvex Drift"]);
        assert_eq!(d.signals(), [("SIGNAL FOUND  ·  HALVEX DRIFT".to_string(), Tone::Teal)]);
        assert!(d.cleared_text.as_deref().is_some_and(|t| t.contains("Halvex Drift") && !t.contains("[[")), "{:?}", d.cleared_text);
        assert!(!d.ready.is_empty(), "the cannon data pays for the cannon node");
    }

    #[test]
    fn a_lost_voyage_debriefs_what_stayed_behind() {
        let content = data::base_content();
        let mut before = career(&content);
        before.voyage_launched(&content, "yard_approach").unwrap();
        let mut after = before.clone();
        let last = after.voyage_ended(&content, "yard_approach", VoyageEnd::Lost { ticks: 30 * 61 }).unwrap().clone();
        let d = DebriefData::build(&content, &before, &after, &last, &Haul::default(), DebriefStats { left: 5, ..default() });
        assert_eq!(d.head(), ("SHIP LOST  ·  RECALLED HOME", Tone::Coral));
        assert!(d.opened.is_empty() && d.signals().is_empty() && !d.first_clear);
        assert_eq!((amount(18.0), amount(4.5), amount(0.04)), ("18".into(), "4.5".into(), "0".into()));
    }
}
