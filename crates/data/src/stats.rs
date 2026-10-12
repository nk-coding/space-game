//! Displayable stat lines. The drydock inspector, tooltips and the wiki all call [`Content::part_stats`],
//! so they show identical numbers, labels and units. Labels, units, precision and order come from
//! `stats.ron`; which component field feeds which stat id is fixed here. [`Content::ship_totals`] sums the
//! per-part values over a blueprint.

use serde::{Deserialize, Serialize};

use crate::load::Content;
use crate::model::{Better, Blueprint, Condition, Effect, Material, Part, Selector};

/// Stat ids the code emits. Each must be defined in `stats.ron` (checked by validation).
pub const BUILTIN_STATS: &[&str] = &[
    "build_points",
    "control_cost",
    "mass",
    "hp",
    "power_output",
    "power_draw",
    "power_min_level",
    "heat_output",
    "heat_limit",
    "heat_vent",
    "damage",
    "range",
    "shot_rate",
    "shot_speed",
    "penetration",
    "recoil",
    "thrust",
    "rated_speed",
    "gimbal",
    "plume_length",
    "plume_damage",
    "plume_heat",
    "rcs_thrust",
    "rcs_rated_speed",
    "torque",
    "shield_radius",
    "shield_capacity",
    "shield_regen",
    "cargo_capacity",
    "rule_slots",
    "control_capacity",
    "mass_per_cell",
    "hp_per_cell",
    "build_point_factor",
];

/// One displayable stat.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatLine {
    pub stat: String,
    pub label: String,
    pub unit: String,
    pub value: f32,
    /// Formatted value with unit, e.g. `"10 MW"`, `"1.5 t"`, `"+15%"`.
    pub text: String,
    pub better: Better,
    /// Set for adjacency bonuses this part grants to its neighbours.
    pub bonus: Option<BonusInfo>,
}

/// Sums over every placement of a blueprint ([`Content::ship_totals`]): the per-part stat values that add up,
/// in the units of their stats.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ShipTotals {
    /// Hull parts scaled by their material's `build_point_factor`.
    pub build_points: f32,
    pub mass: f32,
    pub hp: f32,
    /// Power output and draw at full load, before bonuses.
    pub power_output: f32,
    pub power_draw: f32,
    /// Engine thrust in every direction together, before bonuses.
    pub thrust: f32,
    pub cargo_capacity: f32,
    /// Placements with a `logic` block.
    pub logic_cores: u32,
    pub rule_slots: u32,
    /// Summed [`Part::control_cost`] (defaults included).
    pub control_cost: u32,
    pub control_capacity: u32,
}

/// Who receives a bonus line's effect.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BonusInfo {
    pub target: Selector,
    /// Part or category name for `Part`/`Category` targets, the tag itself for `Tag` targets.
    pub target_label: String,
    pub max_stacks: Option<u32>,
    pub when: Vec<Condition>,
}

/// Formats a number with at most `decimals` decimals, trailing zeros trimmed.
pub fn format_number(value: f32, decimals: u8) -> String {
    let mut s = format!("{:.*}", decimals as usize, value);
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if s == "-0" {
        s = "0".into();
    }
    s
}

/// `"10 MW"`, `"2.6/s"`, `"1.5×"`: a space only before units that start with a letter.
pub fn with_unit(number: &str, unit: &str) -> String {
    match unit.chars().next() {
        None => number.to_string(),
        Some(c) if c.is_alphanumeric() => format!("{number} {unit}"),
        Some(_) => format!("{number}{unit}"),
    }
}

impl Content {
    fn line(&self, stat: &str, value: f32) -> Option<StatLine> {
        let def = self.stats.get(stat)?;
        let text = with_unit(&format_number(value, def.decimals), &def.unit);
        Some(StatLine {
            stat: stat.into(),
            label: def.name.clone(),
            unit: def.unit.clone(),
            value,
            text,
            better: def.better,
            bonus: None,
        })
    }

    fn sorted(&self, mut lines: Vec<StatLine>) -> Vec<StatLine> {
        lines.sort_by_key(|l| self.stats.position(&l.stat).unwrap_or(usize::MAX));
        lines
    }

    /// Stat lines of a part, in `stats.ron` order, followed by the bonuses it grants (in declaration order).
    ///
    /// For hull parts pass the placement's material to get mass, hit points and scaled build points; without
    /// one only the base build points are shown. Unknown stat ids are skipped (validation reports them).
    pub fn part_stats(&self, part: &Part, material: Option<&Material>) -> Vec<StatLine> {
        let mut v: Vec<(&str, f32)> = Vec::new();
        let armor = material.and_then(|m| m.armor.as_ref());
        v.push(("build_points", part.build_points_in(armor)));
        let control = part.control_cost();
        if control > 0 {
            v.push(("control_cost", control as f32));
        }
        if let Some(m) = part.mass_in(armor) {
            v.push(("mass", m));
        }
        if let Some(h) = part.hp_in(armor) {
            v.push(("hp", h));
        }
        if let Some(p) = &part.power {
            if p.output != 0.0 {
                v.push(("power_output", p.output));
            }
            if p.draw != 0.0 {
                v.push(("power_draw", p.draw));
                if p.min_level > 0.0 {
                    v.push(("power_min_level", p.min_level * 100.0));
                }
            }
        }
        if let Some(h) = &part.heat {
            v.push(("heat_output", h.output));
            if let Some(l) = h.limit {
                v.push(("heat_limit", l));
            }
        }
        if let Some(r) = &part.radiator {
            v.push(("heat_vent", r.vent));
        }
        if let Some(w) = &part.weapon {
            v.push(("damage", w.damage));
            v.push(("range", w.range));
            match w.delivery {
                crate::model::Delivery::Beam => {}
                crate::model::Delivery::Pulse { rate } => v.push(("shot_rate", rate)),
                crate::model::Delivery::Projectile {
                    rate,
                    speed,
                    penetration,
                    recoil,
                } => {
                    v.push(("shot_rate", rate));
                    v.push(("shot_speed", speed));
                    v.push(("penetration", penetration as f32));
                    if recoil != 0.0 {
                        v.push(("recoil", recoil));
                    }
                }
            }
        }
        if let Some(e) = &part.engine {
            v.push(("thrust", e.thrust));
            v.push(("rated_speed", e.rated_speed));
            if e.gimbal != 0.0 {
                v.push(("gimbal", e.gimbal));
            }
            v.push(("plume_length", e.plume.length));
            v.push(("plume_damage", e.plume.damage));
            v.push(("plume_heat", e.plume.heat));
        }
        if let Some(r) = &part.rcs {
            v.push(("rcs_thrust", r.thrust));
            v.push(("rcs_rated_speed", r.rated_speed));
        }
        if let Some(g) = &part.gyro {
            v.push(("torque", g.torque));
        }
        if let Some(s) = &part.shield {
            v.push(("shield_radius", s.radius));
            v.push(("shield_capacity", s.capacity));
            v.push(("shield_regen", s.regen));
        }
        if let Some(c) = &part.cargo {
            v.push(("cargo_capacity", c.capacity));
        }
        if let Some(l) = &part.logic {
            v.push(("rule_slots", l.rule_slots as f32));
            v.push(("control_capacity", l.control_capacity as f32));
        }
        let mut lines = self.sorted(v.into_iter().filter_map(|(s, x)| self.line(s, x)).collect());

        for b in &part.bonuses {
            let Some(def) = self.stats.get(&b.stat) else {
                continue;
            };
            let (value, text) = match b.effect {
                Effect::Add(x) => (
                    x,
                    with_unit(&format!("+{}", format_number(x, def.decimals)), &def.unit),
                ),
                Effect::Percent(x) => (x, format!("+{}%", format_number(x, 1))),
            };
            let target_label = match &b.target {
                Selector::Part(id) => self.parts.get(id).map_or(id.clone(), |p| p.name.clone()),
                Selector::Category(id) => self
                    .categories
                    .get(id)
                    .map_or(id.clone(), |c| c.name.clone()),
                Selector::Tag(t) => t.clone(),
            };
            lines.push(StatLine {
                stat: b.stat.clone(),
                label: def.name.clone(),
                unit: def.unit.clone(),
                value,
                text,
                better: def.better,
                bonus: Some(BonusInfo {
                    target: b.target.clone(),
                    target_label,
                    max_stacks: b.max_stacks,
                    when: b.when.clone(),
                }),
            });
        }
        lines
    }

    /// Sums over every placement of a blueprint: the values of the per-part stat lines that add up (as
    /// [`Content::part_stats`] gives them with the placement's material), plus the number of logic cores.
    ///
    /// Every placement counts, connected or not; the rule slots and control of the intact ship's main piece are
    /// [`Content::ship_rule_slots`] and [`Content::ship_control`]. Bonuses and what the sim works out (net power
    /// under load, heat, thrust per direction) are not in here. Works on any blueprint, valid or not: unknown
    /// parts are skipped, a hull part without an armour material adds only its base build points.
    pub fn ship_totals(&self, ship: &Blueprint) -> ShipTotals {
        let mut t = ShipTotals::default();
        for p in &ship.parts {
            let Some(part) = self.parts.get(&p.part) else {
                continue;
            };
            let armor = self.placement_armor(p);
            t.build_points += part.build_points_in(armor);
            t.mass += part.mass_in(armor).unwrap_or(0.0);
            t.hp += part.hp_in(armor).unwrap_or(0.0);
            if let Some(x) = &part.power {
                t.power_output += x.output;
                t.power_draw += x.draw;
            }
            if let Some(e) = &part.engine {
                t.thrust += e.thrust;
            }
            if let Some(c) = &part.cargo {
                t.cargo_capacity += c.capacity;
            }
            if let Some(l) = &part.logic {
                t.logic_cores += 1;
                t.rule_slots = t.rule_slots.saturating_add(l.rule_slots);
                t.control_capacity = t.control_capacity.saturating_add(l.control_capacity);
            }
            t.control_cost = t.control_cost.saturating_add(part.control_cost());
        }
        t
    }

    /// Stat lines of a material (armour values per cell).
    pub fn material_stats(&self, material: &Material) -> Vec<StatLine> {
        let Some(a) = &material.armor else {
            return Vec::new();
        };
        let v = [
            ("mass_per_cell", a.mass_per_cell),
            ("hp_per_cell", a.hp_per_cell),
            ("build_point_factor", a.build_point_factor),
        ];
        self.sorted(v.into_iter().filter_map(|(s, x)| self.line(s, x)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_format() {
        assert_eq!(format_number(10.0, 1), "10");
        assert_eq!(format_number(1.5, 2), "1.5");
        assert_eq!(format_number(0.15, 2), "0.15");
        assert_eq!(format_number(2.6, 0), "3");
        assert_eq!(format_number(-0.0001, 1), "0");
        assert_eq!(with_unit("10", "MW"), "10 MW");
        assert_eq!(with_unit("2.6", "/s"), "2.6/s");
        assert_eq!(with_unit("4", ""), "4");
    }
}
