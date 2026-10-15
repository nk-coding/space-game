//! The evolution lab's content (`lab.ron`, GDR-0092): the goals are the arena's four; goal presets weigh known goals;
//! trial presets have weights, seeds and time limits in range and name ships the content has. (An id listed twice in
//! one file is the loader's error: packs merged replace each other's entries by id.)
//! Problems name the field as `lab.trials[<id>].seeds` and the like (the lab has no entry kind of its own yet).

use super::Validator;
use crate::content::model::{GOALS, LabShips, MAX_LAB_WEIGHT, MAX_PRESET_BEST, MAX_PRESET_SEEDS, MAX_PRESET_SHIPS, MAX_PRESET_WAVES, TrialPresetKind};
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
                TrialPresetKind::Encounters | TrialPresetKind::BeatYourStart | TrialPresetKind::Fleet | TrialPresetKind::Voyages => {}
            }
        }
        self.out.extend(out);
    }
}
