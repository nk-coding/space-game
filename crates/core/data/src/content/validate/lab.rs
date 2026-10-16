//! The evolution lab's content (`lab.ron`, GDR-0092): the goals are the arena's four; goal presets weigh known goals;
//! trial presets have weights, seeds and time limits in range and name ships the content has; genes are the arena's
//! and gene presets name genes; stand-in pilots have their numbers in range; an arms race's numbers are in range and
//! the enemy palettes name factions and parts the content has. (An id listed twice in
//! one file is the loader's error: packs merged replace each other's entries by id.)
//! Problems name the field as `lab.trials[<id>].seeds` and the like (the lab has no entry kind of its own yet).

use super::Validator;
use crate::content::model::{Duty, GENES, GOALS, LabShips, MAX_DUTY_SECONDS, MAX_LAB_WEIGHT, MAX_PRESET_BEST, MAX_PRESET_SEEDS, MAX_PRESET_SHIPS, MAX_PRESET_WAVES, TrialPresetKind};
use crate::content::problem::Problem;

/// The longest time limit a preset may give a fight (s) or a voyage (minutes): the arena's bounds.
const MAX_FIGHT_SECONDS: f32 = 1800.0;
const MAX_VOYAGE_MINUTES: f32 = 600.0;

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
        self.out.extend(out);
    }
}
