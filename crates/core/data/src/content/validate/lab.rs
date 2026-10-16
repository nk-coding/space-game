//! The evolution lab's content (`lab.ron`, GDR-0092): the goals are the arena's four; goal presets weigh known goals;
//! trial presets have weights, seeds and time limits in range and name ships the content has; genes are the arena's
//! and gene presets name genes; stand-in pilots have their numbers in range; an arms race's numbers are in range and
//! the enemy palettes name factions and parts the content has; the trial kinds, measures and spreads are the arena's,
//! every trial kind has its words, a measure takes a stat's words or has its own, a spread's axes are in range and
//! measure what the file names; a lab with trials has sizes (one default, in the arena's bounds), both spreads and a
//! budget. (An id listed twice in one file is the loader's error: packs merged replace each other's entries by id.)
//! Problems name the field as `lab.trials[<id>].seeds` and the like (the lab is not an entry kind).

use super::Validator;
use crate::content::model::{Duty, GENES, GOALS, LabShips, MAX_DUTY_SECONDS, MAX_LAB_WEIGHT, MAX_PRESET_BEST, MAX_PRESET_SEEDS, MAX_PRESET_SHIPS, MAX_PRESET_WAVES, MEASURES, SPREADS, TRIAL_KINDS, TrialPresetKind};
use crate::content::problem::Problem;

/// The longest time limit a preset may give a fight (s) or a voyage (minutes): the arena's bounds.
const MAX_FIGHT_SECONDS: f32 = 1800.0;
const MAX_VOYAGE_MINUTES: f32 = 600.0;
/// A size preset's bounds: the arena's (`arena::evolve::MAX_POPULATION`, `MAX_GENERATIONS`).
const MAX_SIZE_POPULATION: u32 = 4096;
const MAX_SIZE_GENERATIONS: u32 = 100_000;
/// Most cells an archive may have (`arena::evolve::MAX_CELLS`).
const MAX_SPREAD_CELLS: u64 = 65_536;
/// The longest a frame may give a run (ms).
const MAX_BUDGET_MS: f32 = 100.0;

impl<'a> Validator<'a> {
    pub(super) fn lab(&mut self) {
        let lab = &self.c.lab;
        let src = lab.source.clone();
        let mut out = Vec::new();
        let mut bad = |field: String, msg: String| out.push(Problem::error(msg).at(&src).field(field));
        let weight = |w: f32| w.is_finite() && (0.0..=MAX_LAB_WEIGHT).contains(&w);
        for g in &lab.goals {
            let at = format!("lab.goals[{}]", g.id);
            if !GOALS.contains(&g.id.as_str()) {
                bad(at.clone(), format!("`{}` is not a goal the arena scores (one of {})", g.id, GOALS.join(", ")));
            }
            if !weight(g.weight) {
                bad(format!("{at}.weight"), format!("{}: a weight is 0 to {MAX_LAB_WEIGHT}", g.weight));
            }
        }
        if !lab.goals.is_empty() && lab.goals.iter().all(|g| g.weight == 0.0) {
            bad("lab.goals".into(), "every goal weighs 0: a new run would score nothing".into());
        }
        for p in &lab.goal_presets {
            let at = format!("lab.goal_presets[{}]", p.id);
            for (id, w) in &p.weights {
                if !GOALS.contains(&id.as_str()) {
                    bad(format!("{at}.weights"), format!("`{id}` is not a goal (one of {})", GOALS.join(", ")));
                }
                if !weight(*w) {
                    bad(format!("{at}.weights"), format!("{w}: a weight is 0 to {MAX_LAB_WEIGHT}"));
                }
            }
            if p.weights.values().all(|w| *w == 0.0) {
                bad(format!("{at}.weights"), "every goal weighs 0".into());
            }
        }
        let ships = |out: &mut Vec<Problem>, at: &str, list: &[LabShips]| {
            if list.is_empty() {
                out.push(Problem::error("no ships").at(&src).field(at.to_string()));
            }
            for (k, s) in list.iter().enumerate() {
                if !self.c.ships.contains(&s.ship) {
                    out.push(Problem::error(format!("no ship `{}` in the content", s.ship)).at(&src).field(format!("{at}[{k}].ship")));
                }
                if !(1..=MAX_PRESET_SHIPS).contains(&s.count) {
                    out.push(Problem::error(format!("{} ships: 1 to {MAX_PRESET_SHIPS}", s.count)).at(&src).field(format!("{at}[{k}].count")));
                }
            }
        };
        for t in &lab.trials {
            let at = format!("lab.trials[{}]", t.id);
            if !weight(t.weight) {
                out.push(Problem::error(format!("{}: a weight is 0 to {MAX_LAB_WEIGHT}", t.weight)).at(&src).field(format!("{at}.weight")));
            }
            if !(1..=MAX_PRESET_SEEDS).contains(&t.seeds) {
                out.push(Problem::error(format!("{} seeds: 1 to {MAX_PRESET_SEEDS}", t.seeds)).at(&src).field(format!("{at}.seeds")));
            }
            let most = if t.kind == TrialPresetKind::Voyages { MAX_VOYAGE_MINUTES } else { MAX_FIGHT_SECONDS };
            if let Some(v) = t.time_limit
                && !(v.is_finite() && v > 0.0 && v <= most)
            {
                out.push(Problem::error(format!("{v}: above 0 and at most {most}")).at(&src).field(format!("{at}.time_limit")));
            }
            let count = |out: &mut Vec<Problem>, n: u32, max: u32, what: &str| {
                if !(1..=max).contains(&n) {
                    out.push(Problem::error(format!("{n} {what}: 1 to {max}")).at(&src).field(format!("{at}.kind")));
                }
            };
            match &t.kind {
                TrialPresetKind::EnemyShips { ships: list } => ships(&mut out, &format!("{at}.kind.ships"), list),
                TrialPresetKind::Gauntlet { waves } => {
                    if waves.is_empty() {
                        out.push(Problem::error("a gauntlet needs at least one wave").at(&src).field(format!("{at}.kind.waves")));
                    }
                    for (k, w) in waves.iter().enumerate() {
                        ships(&mut out, &format!("{at}.kind.waves[{k}]"), w);
                    }
                }
                TrialPresetKind::Waves { target } => count(&mut out, *target, MAX_PRESET_WAVES, "waves"),
                TrialPresetKind::HallOfFame { count: n } | TrialPresetKind::Champions { count: n } => {
                    count(&mut out, *n, MAX_PRESET_BEST, "designs")
                }
                TrialPresetKind::CoEvolution { size, elites, hall, champions_every, .. } => {
                    if !(size.is_finite() && *size > 0.0 && *size <= 2.0) {
                        out.push(Problem::error(format!("{size}: a share of your children above 0 and at most 2")).at(&src).field(format!("{at}.kind.size")));
                    }
                    count(&mut out, *elites, MAX_PRESET_BEST, "elites");
                    if *hall > MAX_PRESET_BEST {
                        out.push(Problem::error(format!("{hall} past champions: 0 to {MAX_PRESET_BEST}")).at(&src).field(format!("{at}.kind.hall")));
                    }
                    count(&mut out, *champions_every, MAX_PRESET_WAVES, "generations between champions");
                }
                TrialPresetKind::Encounters | TrialPresetKind::BeatYourStart | TrialPresetKind::Fleet | TrialPresetKind::Voyages => {}
            }
        }
        for pal in &lab.palettes {
            let at = format!("lab.palettes[{}]", pal.faction);
            if !self.c.factions.contains(&pal.faction) {
                out.push(Problem::error(format!("no faction `{}` in the content", pal.faction)).at(&src).field(at.clone()));
            }
            for part in pal.parts.iter().filter(|p| !self.c.parts.contains(p)) {
                out.push(Problem::error(format!("no part `{part}` in the content")).at(&src).field(format!("{at}.parts")));
            }
        }
        for g in &lab.genes {
            if !GENES.contains(&g.id.as_str()) {
                out.push(Problem::error(format!("`{}` is not a gene the arena changes (one of {})", g.id, GENES.join(", "))).at(&src).field(format!("lab.genes[{}]", g.id)));
            }
        }
        for p in &lab.gene_presets {
            let at = format!("lab.gene_presets[{}].genes", p.id);
            if p.genes.is_empty() {
                out.push(Problem::error("no gene: nothing would change").at(&src).field(at.clone()));
            }
            for g in p.genes.iter().filter(|g| !GENES.contains(&g.as_str())) {
                out.push(Problem::error(format!("`{g}` is not a gene (one of {})", GENES.join(", "))).at(&src).field(at.clone()));
            }
        }
        for p in &lab.pilots {
            let at = format!("lab.pilots[{}].flies", p.id);
            let f = &p.flies;
            if !(f.close_to.is_finite() && f.close_to > 0.0 && f.close_to <= 2.0) {
                out.push(Problem::error(format!("{}: a share of the reach above 0 and at most 2", f.close_to)).at(&src).field(format!("{at}.close_to")));
            }
            let duty = |d: &Duty| [d.on, d.off].iter().all(|v| v.is_finite() && (0.0..=MAX_DUTY_SECONDS).contains(v)) && d.on > 0.0;
            for (what, d) in [("stick", &f.stick), ("fire", &f.fire)] {
                if d.as_ref().is_some_and(|d| !duty(d)) {
                    out.push(Problem::error(format!("on above 0, on and off at most {MAX_DUTY_SECONDS} s")).at(&src).field(format!("{at}.{what}")));
                }
            }
        }
        for k in &lab.kinds {
            if !TRIAL_KINDS.contains(&k.id.as_str()) {
                out.push(Problem::error(format!("`{}` is not a trial kind the arena fights (one of {})", k.id, TRIAL_KINDS.join(", "))).at(&src).field(format!("lab.kinds[{}]", k.id)));
            }
        }
        for t in lab.trials.iter().filter(|t| lab.kind(t.kind.id()).is_none()) {
            out.push(Problem::error(format!("the kind `{}` has no words in `kinds`", t.kind.id())).at(&src).field(format!("lab.trials[{}].kind", t.id)));
        }
        for z in &lab.sizes {
            let at = format!("lab.sizes[{}]", z.id);
            if !(1..=MAX_SIZE_POPULATION).contains(&z.population) {
                out.push(Problem::error(format!("{} children: 1 to {MAX_SIZE_POPULATION}", z.population)).at(&src).field(format!("{at}.population")));
            }
            if !(1..=MAX_SIZE_GENERATIONS).contains(&z.generations) {
                out.push(Problem::error(format!("{} generations: 1 to {MAX_SIZE_GENERATIONS}", z.generations)).at(&src).field(format!("{at}.generations")));
            }
        }
        if lab.sizes.iter().filter(|z| z.default).count() > 1 {
            out.push(Problem::error("more than one size is the default").at(&src).field("lab.sizes"));
        }
        for m in &lab.measures {
            let at = format!("lab.measures[{}]", m.id);
            if !MEASURES.contains(&m.id.as_str()) {
                out.push(Problem::error(format!("`{}` is not a measure the arena takes (one of {})", m.id, MEASURES.join(", "))).at(&src).field(at.clone()));
            }
            match (&m.stat, &m.name) {
                (Some(st), None) if !self.c.stats.contains(st) => {
                    out.push(Problem::error(format!("no stat `{st}` in the content")).at(&src).field(format!("{at}.stat")));
                }
                (Some(_), None) if m.unit.is_some() => {
                    out.push(Problem::error("a measure with a stat takes the stat's unit").at(&src).field(format!("{at}.unit")));
                }
                (Some(_), Some(_)) => out.push(Problem::error("a stat's words or a name of its own, not both").at(&src).field(at.clone())),
                (None, None) => out.push(Problem::error("a measure needs a stat or a name").at(&src).field(at.clone())),
                _ => {}
            }
        }
        for sp in &lab.spreads {
            let at = format!("lab.spreads[{}]", sp.id);
            if !SPREADS.contains(&sp.id.as_str()) {
                out.push(Problem::error(format!("`{}` is not a spread (one of {})", sp.id, SPREADS.join(", "))).at(&src).field(at.clone()));
            }
            if !(1..=3).contains(&sp.axes.len()) {
                out.push(Problem::error("one to three axes").at(&src).field(format!("{at}.axes")));
            }
            let mut cells = 1u64;
            for (k, a) in sp.axes.iter().enumerate() {
                if lab.measure(&a.measure).is_none() {
                    out.push(Problem::error(format!("no measure `{}` in `measures`", a.measure)).at(&src).field(format!("{at}.axes[{k}].measure")));
                }
                if !(a.min.is_finite() && a.max.is_finite() && a.min < a.max) || a.cells == 0 {
                    out.push(Problem::error("min below max and at least one cell").at(&src).field(format!("{at}.axes[{k}]")));
                }
                cells = cells.saturating_mul(a.cells as u64);
            }
            if cells > MAX_SPREAD_CELLS {
                out.push(Problem::error(format!("{cells} cells: at most {MAX_SPREAD_CELLS}")).at(&src).field(format!("{at}.axes")));
            }
        }
        if let Some(b) = lab.budget {
            for (what, ms) in [("layer_open", b.layer_open), ("elsewhere", b.elsewhere)] {
                if !(ms.is_finite() && ms > 0.0 && ms <= MAX_BUDGET_MS) {
                    out.push(Problem::error(format!("{ms} ms: above 0 and at most {MAX_BUDGET_MS}")).at(&src).field(format!("lab.budget.{what}")));
                }
            }
        }
        // What the setup needs to make a run.
        if !lab.trials.is_empty() {
            if lab.sizes.is_empty() {
                out.push(Problem::error("a lab with trials needs a size preset").at(&src).field("lab.sizes"));
            }
            for id in SPREADS.iter().filter(|id| lab.spread(id).is_none()) {
                out.push(Problem::error(format!("a lab with trials needs the spread `{id}`")).at(&src).field("lab.spreads"));
            }
            if lab.budget.is_none() {
                out.push(Problem::error("a lab with trials needs a budget").at(&src).field("lab.budget"));
            }
        }
        self.out.extend(out);
    }
}
