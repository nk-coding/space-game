//! Part-stat upgrades (GDR-0024's refinement): the small research steps on one stat of the parts a
//! selector picks ([`Unlock::PartStat`]). A career's steps ([`crate::Career::upgrades`]) are passed in wherever a
//! player's design is built (`sim::ShipDesign::compile_upgraded`, the design report, the drydock's stat lines, the
//! evolution lab's runs), never read from a global, so every place that builds a design sees the same numbers.
//! Other owners' ships (enemies, wrecks) are built without them.
//!
//! The steps on one stat add up like adjacency bonuses: `(base + adds) × (1 + percents / 100)`; the sim applies
//! the bonuses on top of the upgraded value.

use serde::{Deserialize, Serialize};

use crate::content::load::Content;
use crate::content::model::{Effect, Part, Selector, Unlock};

/// One step: `step` on `stat` of every part `target` picks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatStep {
    pub target: Selector,
    /// A stat id of [`crate::design::stats::BONUS_STATS`].
    pub stat: String,
    pub step: Effect,
}

/// The part-stat steps a player has researched, in content order (the research files' order), so two careers with
/// the same nodes sum the same numbers in the same order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Upgrades(pub Vec<StatStep>);

impl Upgrades {
    /// No steps: what other owners' ships (and a design without a career) are built with.
    pub const NONE: Upgrades = Upgrades(Vec::new());

    /// The steps of these research nodes (ids; unknown ones are skipped), in content order.
    pub fn of_research<'a>(content: &Content, researched: impl IntoIterator<Item = &'a str>) -> Upgrades {
        let done: std::collections::BTreeSet<&str> = researched.into_iter().collect();
        Upgrades(
            content
                .research
                .iter()
                .filter(|n| done.contains(n.id.as_str()))
                .flat_map(|n| &n.unlocks)
                .filter_map(|u| match u {
                    Unlock::PartStat { target, stat, step } => {
                        Some(StatStep { target: target.clone(), stat: stat.clone(), step: *step })
                    }
                    _ => None,
                })
                .collect(),
        )
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Steps from outside the content (a fight or voyage code, a run file) are usable: every stat one the sim
    /// applies, every step finite and a percentage above -100 (the content's own are validated with it).
    pub fn check(&self) -> Result<(), String> {
        for s in &self.0 {
            if !crate::design::stats::BONUS_STATS.contains(&s.stat.as_str()) {
                return Err(format!("a step on `{}`, which no step can change", s.stat));
            }
            let ok = match s.step {
                Effect::Add(x) => x.is_finite() && x.abs() <= crate::content::validate::MAX_NUMBER,
                Effect::Percent(x) => x.is_finite() && x > -100.0 && x <= crate::content::validate::MAX_BONUS_PERCENT,
            };
            if !ok {
                return Err(format!("a step on `{}` out of range: {:?}", s.stat, s.step));
            }
        }
        Ok(())
    }

    /// The summed adds and percents on `stat` of `part`; `None` when no step touches it.
    pub fn sum(&self, part: &Part, stat: &str) -> Option<(f32, f32)> {
        let mut hit = None;
        for s in self.0.iter().filter(|s| s.stat == stat && part.matches(&s.target)) {
            let (add, pct) = hit.get_or_insert((0.0f32, 0.0f32));
            match s.step {
                Effect::Add(x) => *add += x,
                Effect::Percent(x) => *pct += x,
            }
        }
        hit
    }

    /// `base` of `stat` on `part` after the steps (unchanged when none touches it).
    pub fn apply(&self, part: &Part, stat: &str, base: f32) -> f32 {
        match self.sum(part, stat) {
            Some((add, pct)) => (base + add) * (1.0 + pct / 100.0),
            None => base,
        }
    }

    /// The value of part stat line `line` (its content value `value`) after the steps: its own stat's, and for
    /// [`FIRE_RATE_LINES`] times the fire-rate multiplier; `None` when no step touches it.
    pub fn line_value(&self, part: &Part, line: &str, value: f32) -> Option<f32> {
        let own = self.sum(part, line).map(|_| self.apply(part, line, value));
        let rate = FIRE_RATE_LINES.contains(&line).then(|| self.sum(part, "fire_rate")).flatten().map(|_| self.apply(part, "fire_rate", 1.0));
        match (own, rate) {
            (None, None) => None,
            (own, rate) => Some(own.unwrap_or(value) * rate.unwrap_or(1.0)),
        }
    }
}

/// The part stat lines a step on `fire_rate` (a multiplier in the sim, no line of its own) shows on: damage per
/// second and shots per second scale with it.
pub const FIRE_RATE_LINES: &[&str] = &["damage", "shot_rate"];

/// A step on `stat` shows on the part stat line `line`: its own, and `fire_rate`'s on [`FIRE_RATE_LINES`].
pub fn shows_on(stat: &str, line: &str) -> bool {
    line == stat || (stat == "fire_rate" && FIRE_RATE_LINES.contains(&line))
}
