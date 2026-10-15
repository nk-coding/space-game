//! Genomes (a blueprint and a behavior text), and how they change: mutation and crossover through the editing
//! models (`data::design::edit::Drydock` for designs, `data::behavior::edit::BehaviorEditor` for behaviors), so every
//! child is a legal design and a legal program; the pre-filter (`sim::design_report`) before any fight.

use std::collections::BTreeSet;

use data::behavior::{CmpOp, ListKind, MAX_RULES, Program, Value};
use data::behavior::edit::{BehaviorEditor, NodeRef};
use data::design::edit::{Drydock, Outcome, Refusal, Twin};
use data::design::geometry::PlacedShape;
use data::{BehaviorNode, Blueprint, Content, NodeRole, ParamType, Placement, Shape, Table};
use sim::report::Issue;
use sim::{Pcg32, SimConfig};

use super::config::Limits;

/// A design and the behavior it flies: what evolves. Both are ordinary content: the blueprint saves as a design,
/// the behavior text as a behavior (`docs/behavior-format.md`).
#[derive(Clone, Debug, PartialEq)]
pub struct Genome {
    /// Trimmed to its parts (as the drydock exports it); its `behavior` field is empty (the behavior is the
    /// text beside it).
    pub blueprint: Blueprint,
    /// The program in its canonical text form.
    pub behavior: String,
}

impl Genome {
    /// The blueprint in the content's RON form (`content/base/ships/*.ron`; the drydock imports it).
    pub fn blueprint_text(&self) -> String {
        data::blueprint_to_ron(&self.blueprint)
    }

    /// The behavior in its text form.
    pub fn behavior_text(&self) -> &str {
        &self.behavior
    }
}

/// A genome's numbers before any fight: the design report's and the program's.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Stats {
    pub top_speed: f32,
    pub acceleration: f32,
    pub reach: f32,
    pub build_points: f32,
    pub mass: f32,
    /// Latent: every gun of the main piece as if clear and powered (`DesignReport::latent_firepower`).
    pub firepower: f32,
    pub hit_points: f32,
    pub parts: u32,
    pub rules: u32,
    pub rule_slots: u32,
}

/// Mutation weights of the design operators: add, remove, move, rotate, flip, swap, material. PLACEHOLDERS.
const DESIGN_OPS: [u32; 7] = [30, 15, 15, 10, 5, 15, 10];
/// Mutation weights of the behavior operators: nudge a number, a choice, move a rule, swap an action, swap the
/// target, add a rule, remove a rule, add a condition, remove one, toggle `not`, replace one. PLACEHOLDERS.
const BEHAVIOR_OPS: [u32; 11] = [30, 5, 10, 10, 5, 10, 8, 8, 6, 4, 4];
/// Tries per mutation before it is given up (an operator may be refused, e.g. a move into another part).
const TRIES: u32 = 8;

/// Everything breeding needs, fixed for a run: the frame, the limits, the parts and materials to pick from and
/// the behavior vocabulary.
pub(crate) struct Kit {
    pub frame: (u32, u32),
    pub mirror: bool,
    pub build_points: Option<f32>,
    allowed_parts: Option<BTreeSet<String>>,
    allowed_materials: Option<BTreeSet<String>>,
    /// The parts to build with (allowed ones, content order).
    parts: Vec<String>,
    /// The armour materials to build with.
    materials: Vec<String>,
    nodes: Table<BehaviorNode>,
    actions: Vec<String>,
    conditions: Vec<String>,
    targets: Vec<String>,
}

fn pick<'a, T>(rng: &mut Pcg32, items: &'a [T]) -> Option<&'a T> {
    items.get(rng.below(items.len() as u32) as usize)
}

/// An index drawn with the given weights.
fn weighted(rng: &mut Pcg32, weights: &[u32]) -> usize {
    let mut r = rng.below(weights.iter().sum());
    for (k, &w) in weights.iter().enumerate() {
        if r < w {
            return k;
        }
        r -= w;
    }
    weights.len() - 1
}

/// A number changed by −40 % to +40 %, rounded to a whole number (and by one if that changes nothing), at least 0.
fn nudge(v: &Value, rng: &mut Pcg32) -> Option<Value> {
    let Value::Number(x, unit) = v else {
        return None;
    };
    let f = 0.6 + 0.8 * rng.next_f32();
    let mut y = (x * f).round();
    if y == *x {
        y = if rng.chance(0.5) { x + 1.0 } else { x - 1.0 };
    }
    Some(Value::Number(y.max(0.0), *unit))
}

/// A design edit took: it changed something and, under the mirror lock, its mirror image went with it.
fn took(r: Result<Outcome, Refusal>, dock: &mut Drydock, mirror: bool) -> bool {
    match r {
        Ok(o) if o.changed => {
            if mirror && matches!(o.twin, Twin::Skipped(_)) {
                dock.undo();
                false
            } else {
                true
            }
        }
        _ => false,
    }
}

impl Kit {
    pub fn new(content: &Content, limits: &Limits, frame: (u32, u32)) -> Kit {
        let part_ok = |id: &str| limits.parts.as_ref().is_none_or(|s| s.contains(id));
        let material_ok = |id: &str| limits.materials.as_ref().is_none_or(|s| s.contains(id));
        let palette = |role: NodeRole| {
            content
                .behavior_nodes
                .iter()
                .filter(|n| n.role == role && ListKind::Behavior.allows(n))
                .map(|n| n.id.clone())
                .collect::<Vec<_>>()
        };
        Kit {
            frame,
            mirror: limits.mirror,
            build_points: limits.build_points,
            allowed_parts: limits.parts.clone(),
            allowed_materials: limits.materials.clone(),
            parts: content
                .parts
                .iter()
                .filter(|p| part_ok(&p.id))
                .map(|p| p.id.clone())
                .collect(),
            materials: content
                .materials
                .iter()
                .filter(|m| m.armor.is_some() && material_ok(&m.id))
                .map(|m| m.id.clone())
                .collect(),
            nodes: content.behavior_nodes.clone(),
            actions: palette(NodeRole::Action),
            conditions: palette(NodeRole::Condition),
            targets: palette(NodeRole::Target),
        }
    }

    /// The frame a design is edited in: the run's, one column narrower under the mirror lock when the design's
    /// width has the other parity (a design is mirror-aligned only then, `docs/drydock.md` "Mirror mode").
    fn frame_for(&self, bp: &Blueprint) -> (u32, u32) {
        let (w, h) = self.frame;
        if self.mirror && w > 1 && (w ^ bp.size.0) & 1 == 1 {
            (w - 1, h)
        } else {
            (w, h)
        }
    }

    fn open(&self, content: &Content, bp: &Blueprint) -> Option<Drydock> {
        let mut dock = Drydock::open(content, bp, self.frame_for(bp)).ok()?;
        dock.mirror = self.mirror;
        Some(dock)
    }

    // ---- designs ----

    /// The blueprint after `n` mutations (each tried up to [`TRIES`] times); `None` if none took.
    pub fn mutate_design(
        &self,
        content: &Content,
        bp: &Blueprint,
        rng: &mut Pcg32,
        n: u32,
    ) -> Option<Blueprint> {
        let mut dock = self.open(content, bp)?;
        let mut any = false;
        for _ in 0..n {
            for _ in 0..TRIES {
                if self.design_op(content, &mut dock, rng) {
                    any = true;
                    break;
                }
            }
        }
        any.then(|| dock.export())
    }

    /// One random design edit; true if it took.
    fn design_op(&self, content: &Content, dock: &mut Drydock, rng: &mut Pcg32) -> bool {
        let n = dock.parts().len() as u32;
        let op = if n == 0 { 0 } else { weighted(rng, &DESIGN_OPS) };
        let i = rng.below(n) as usize;
        let r = match op {
            0 => {
                let Some(part) = pick(rng, &self.parts).cloned() else {
                    return false;
                };
                return self.add_part(content, dock, rng, &part);
            }
            1 => dock.remove_part(i),
            2 => {
                let (dx, dy) = (rng.below(5) as i32 - 2, rng.below(5) as i32 - 2);
                dock.move_part(i, dx, dy)
            }
            3 => dock.rotate(i, rng.chance(0.5)),
            4 => dock.flip(i),
            5 => return self.swap_part(content, dock, rng, i),
            _ => {
                let p = &dock.parts()[i];
                let Some(m) = pick(rng, &self.materials).cloned() else {
                    return false;
                };
                if p.material.is_none() || p.material.as_deref() == Some(&m) {
                    return false;
                }
                dock.set_material(i, &m)
            }
        };
        took(r, dock, self.mirror)
    }

    /// Adds a part next to a random part (on a free cell, replacing nothing): a slope oriented to the hull, any
    /// other part in a random pose. The first part of an empty design goes in the frame's middle.
    fn add_part(&self, content: &Content, dock: &mut Drydock, rng: &mut Pcg32, part: &str) -> bool {
        let Some(info) = content.parts.get(part) else {
            return false;
        };
        let cell = if dock.parts().is_empty() {
            let (w, h) = dock.frame();
            ((w / 2) as i32, (h / 2) as i32)
        } else {
            let a = rng.below(dock.parts().len() as u32) as usize;
            let cells = dock.shapes()[a].cells();
            let Some(&(x, y)) = pick(rng, &cells) else {
                return false;
            };
            let (dx, dy) = [(0, -1), (1, 0), (0, 1), (-1, 0)][rng.below(4) as usize];
            (x + dx, y + dy)
        };
        if dock.part_at(cell).is_some() {
            return false;
        }
        let material = if info.hull {
            match pick(rng, &self.materials) {
                Some(m) => Some(m.clone()),
                None => return false,
            }
        } else {
            None
        };
        let r = if info.shape == Shape::Slope {
            let m = material.as_deref().unwrap_or_default();
            match dock.preview_slope(part, cell, m, None) {
                Ok(p) if p.removed.is_empty() => dock.place_slope(part, cell, m, None),
                _ => return false,
            }
        } else {
            let p = Placement {
                part: part.to_string(),
                x: cell.0,
                y: cell.1,
                rot: rng.below(4) as u8,
                flip: rng.chance(0.5),
                material,
                power: None,
                group: None,
            };
            match dock.preview_place(&p) {
                Ok(pv) if pv.removed.is_empty() => dock.place(p),
                _ => return false,
            }
        };
        took(r, dock, self.mirror)
    }

    /// Puts another part of the same footprint in part `i`'s place (and pose): the drydock replaces it.
    fn swap_part(&self, content: &Content, dock: &mut Drydock, rng: &mut Pcg32, i: usize) -> bool {
        let old = dock.parts()[i].clone();
        let Some(info) = content.parts.get(&old.part) else {
            return false;
        };
        let same: Vec<&String> = self
            .parts
            .iter()
            .filter(|q| **q != old.part)
            .filter(|q| {
                content
                    .parts
                    .get(q)
                    .is_some_and(|p| p.size == info.size && p.shape == info.shape)
            })
            .collect();
        let Some(&q) = pick(rng, &same) else {
            return false;
        };
        let hull = content.parts.get(q).is_some_and(|p| p.hull);
        let material = match (hull, &old.material) {
            (false, _) => None,
            (true, Some(m)) => Some(m.clone()),
            (true, None) => pick(rng, &self.materials).cloned(),
        };
        if hull && material.is_none() {
            return false;
        }
        let p = Placement {
            part: q.clone(),
            material,
            power: None,
            ..old
        };
        match dock.preview_place(&p) {
            Ok(pv) if pv.removed.contains(&i) && pv.removed.len() <= 2 => {
                let r = dock.place(p);
                took(r, dock, self.mirror)
            }
            _ => false,
        }
    }

    /// Splices two designs: `a`'s parts above a random row, `b`'s from that row down (in `a`'s frame; `b`'s
    /// parts that would overlap are left out). `None` if the frames do not line up or nothing changed.
    pub fn cross_design(
        &self,
        content: &Content,
        a: &Blueprint,
        b: &Blueprint,
        rng: &mut Pcg32,
    ) -> Option<Blueprint> {
        if self.frame_for(a) != self.frame_for(b) {
            return None;
        }
        let mut da = self.open(content, a)?;
        let db = self.open(content, b)?;
        let bounds = da.bounds()?;
        if bounds.height() < 2 {
            return None;
        }
        let cut = bounds.min_y + 1 + rng.below(bounds.height() as u32 - 1) as i32;
        da.mirror = false; // both halves come whole: a symmetric pair stays symmetric
        for i in (0..da.parts().len()).rev() {
            if da.shapes()[i].y >= cut {
                da.remove_part(i).ok()?;
            }
        }
        for (p, s) in db.parts().iter().zip(db.shapes()) {
            if s.y >= cut && da.check(p).is_ok() {
                da.place(p.clone()).ok()?;
            }
        }
        let child = da.export();
        (child.parts != a.parts).then_some(child)
    }

    /// Builds a random design that passes [`Kit::assess`]: a logic core in the frame's middle, then a power
    /// source, an engine and a gun next to it, then a few random parts. `None` after many tries (the parts allowed
    /// cannot make a ship that flies).
    pub fn random_design(
        &self,
        content: &Content,
        header: &Blueprint,
        behavior: &str,
        rng: &mut Pcg32,
    ) -> Option<Genome> {
        let of = |f: &dyn Fn(&data::Part) -> bool| -> Vec<String> {
            self.parts
                .iter()
                .filter(|id| content.parts.get(id).is_some_and(f))
                .cloned()
                .collect()
        };
        let kinds = [
            of(&|p| p.logic.is_some()),
            of(&|p| p.power.as_ref().is_some_and(|s| s.output > 0.0)),
            of(&|p| p.engine.is_some()),
            of(&|p| p.weapon.is_some()),
        ];
        if kinds.iter().any(|k| k.is_empty()) {
            return None;
        }
        for _ in 0..256 {
            let mut dock = self.open(content, header)?;
            let mut ok = true;
            for kind in &kinds {
                let part = pick(rng, kind)?.clone();
                ok &= (0..TRIES).any(|_| self.add_part(content, &mut dock, rng, &part));
            }
            if !ok {
                continue;
            }
            for _ in 0..3 + rng.below(8) {
                if let Some(part) = pick(rng, &self.parts).cloned() {
                    self.add_part(content, &mut dock, rng, &part);
                }
            }
            let g = Genome {
                blueprint: dock.export(),
                behavior: behavior.to_string(),
            };
            if self.assess(content, &g).is_ok() {
                return Some(g);
            }
        }
        None
    }

    // ---- behaviors ----

    /// The behavior after `n` mutations (each tried up to [`TRIES`] times), with at most `slots` rules; `None`
    /// if nothing changed.
    pub fn mutate_behavior(&self, text: &str, slots: u32, rng: &mut Pcg32, n: u32) -> Option<String> {
        let mut ed = BehaviorEditor::from_text(&self.nodes, text).ok()?;
        let max_rules = (slots as usize).clamp(1, MAX_RULES);
        for _ in 0..n {
            for _ in 0..TRIES {
                if self.behavior_op(&mut ed, max_rules, rng) {
                    break;
                }
            }
        }
        let out = ed.text();
        (out != text).then_some(out)
    }

    /// One random behavior edit; true if it changed the program.
    fn behavior_op(&self, ed: &mut BehaviorEditor, max_rules: usize, rng: &mut Pcg32) -> bool {
        let rules: Vec<_> = ed.rules().map(|r| r.id).collect();
        let conds: Vec<_> = ed.rules().flat_map(|r| r.conds.to_vec()).collect();
        let nodes: Vec<NodeRef> = std::iter::once(NodeRef::Target)
            .chain(ed.rules().flat_map(|r| r.nodes().collect::<Vec<_>>()))
            .collect();
        let rule = pick(rng, &rules).copied();
        let cond = pick(rng, &conds).copied();
        let r = match weighted(rng, &BEHAVIOR_OPS) {
            0 => {
                // A number: a comparison's threshold (sometimes its comparator too) or a parameter.
                let mut slots = Vec::new();
                for &at in &nodes {
                    let Some(v) = ed.node(at) else { continue };
                    if let Some(c) = v.compare
                        && matches!(c.rhs.value, Value::Number(..))
                    {
                        slots.push((at, None));
                    }
                    for p in &v.params {
                        if matches!(p.value, Value::Number(..)) {
                            slots.push((at, Some(p.index)));
                        }
                    }
                }
                let Some(&(at, k)) = pick(rng, &slots) else {
                    return false;
                };
                match (at, k) {
                    (NodeRef::Cond(c), None) => {
                        let Some(cmp) = ed.node(at).and_then(|v| v.compare.cloned()) else {
                            return false;
                        };
                        let op = if rng.chance(0.1) {
                            [CmpOp::Lt, CmpOp::Le, CmpOp::Gt, CmpOp::Ge][rng.below(4) as usize]
                        } else {
                            cmp.op
                        };
                        let Some(v) = nudge(&cmp.rhs.value, rng) else {
                            return false;
                        };
                        ed.set_compare(c, op, v)
                    }
                    (at, Some(k)) => {
                        let Some(v) = ed
                            .node(at)
                            .and_then(|n| n.params.get(k).map(|p| p.value.clone()))
                        else {
                            return false;
                        };
                        let Some(v) = nudge(&v, rng) else {
                            return false;
                        };
                        ed.set_param(at, k, v)
                    }
                    _ => return false,
                }
            }
            1 => {
                // A choice: another option (an orbit's direction, a target's aim point).
                let mut slots = Vec::new();
                for &at in &nodes {
                    let Some(v) = ed.node(at) else { continue };
                    for p in &v.params {
                        if let ParamType::Choice(opts) = &p.param.ty {
                            slots.push((at, p.index, opts.iter().map(|o| o.id.clone()).collect::<Vec<_>>()));
                        }
                    }
                }
                let Some((at, k, opts)) = pick(rng, &slots).cloned() else {
                    return false;
                };
                let Some(o) = pick(rng, &opts).cloned() else {
                    return false;
                };
                ed.set_param(at, k, Value::Word(o))
            }
            2 => match rule {
                Some(id) if rules.len() > 1 => ed.move_rule(id, rng.below(rules.len() as u32) as usize),
                _ => return false,
            },
            3 => match (rule, pick(rng, &self.actions)) {
                (Some(id), Some(a)) => ed.replace_node(NodeRef::Action(id), a),
                _ => return false,
            },
            4 => match pick(rng, &self.targets) {
                Some(t) => ed.replace_node(NodeRef::Target, t),
                None => return false,
            },
            5 => {
                if rules.len() >= max_rules {
                    return false;
                }
                let Some(a) = pick(rng, &self.actions).cloned() else {
                    return false;
                };
                let at = rng.below(rules.len() as u32 + 1) as usize;
                let Ok(id) = ed.add_rule(at, &a) else {
                    return false;
                };
                if rng.chance(0.8)
                    && let Some(c) = pick(rng, &self.conditions)
                {
                    // A rule without conditions is fine too; a refused condition leaves it so.
                    let _ = ed.add_condition(id, c);
                }
                return true;
            }
            6 => match rule {
                Some(id) => ed.delete_rule(id),
                None => return false,
            },
            7 => match (rule, pick(rng, &self.conditions)) {
                (Some(id), Some(c)) => ed.add_condition(id, c).map(|_| true),
                _ => return false,
            },
            8 => match cond {
                Some(c) => ed.remove_condition(c),
                None => return false,
            },
            9 => match cond {
                Some(c) => ed.toggle_not(c),
                None => return false,
            },
            _ => match (cond, pick(rng, &self.conditions)) {
                (Some(c), Some(n)) => ed.replace_node(NodeRef::Cond(c), n),
                _ => return false,
            },
        };
        r == Ok(true)
    }

    /// Splices two behaviors: `a`'s first rules (at least one), then `b`'s from a random rule on, at most
    /// `slots` rules, the target line of either. `None` if the result is no legal program or equals `a`.
    pub fn cross_behavior(&self, a: &str, b: &str, slots: u32, rng: &mut Pcg32) -> Option<String> {
        let pa = BehaviorEditor::from_text(&self.nodes, a).ok()?.program().clone();
        let pb = BehaviorEditor::from_text(&self.nodes, b).ok()?.program().clone();
        let i = 1 + rng.below(pa.rules.len() as u32) as usize;
        let j = rng.below(pb.rules.len() as u32) as usize;
        let mut rules = pa.rules[..i].to_vec();
        rules.extend_from_slice(&pb.rules[j..]);
        rules.truncate((slots as usize).clamp(1, MAX_RULES));
        let target = if rng.chance(0.5) { pa.target } else { pb.target };
        let text = Program { target, rules }.to_text();
        let out = BehaviorEditor::from_text(&self.nodes, &text).ok()?.text();
        (out != a).then_some(out)
    }

    // ---- the pre-filter ----

    /// Checks a genome before any fight and measures it: the limits (parts, materials, frame, build points,
    /// symmetry under the mirror lock), the content's checks (`data::validate_ship`), and the design report: it
    /// must fly (a powered logic core and thrust: no blocker), be one piece and carry a gun.
    pub fn assess(&self, content: &Content, g: &Genome) -> Result<Stats, String> {
        let bp = &g.blueprint;
        for p in &bp.parts {
            if self.allowed_parts.as_ref().is_some_and(|s| !s.contains(&p.part)) {
                return Err(format!("the part `{}` is not allowed", p.part));
            }
            if let Some(m) = &p.material
                && self.allowed_materials.as_ref().is_some_and(|s| !s.contains(m))
            {
                return Err(format!("the material `{m}` is not allowed"));
            }
        }
        let frame = self.frame_for(bp);
        if bp.size.0 > frame.0 || bp.size.1 > frame.1 {
            return Err(format!(
                "{} × {} cells do not fit the {} × {} frame",
                bp.size.0, bp.size.1, frame.0, frame.1
            ));
        }
        if self.mirror && !self.symmetric(content, bp) {
            return Err("not symmetric under the mirror lock".into());
        }
        let errors: Vec<String> = data::validate_ship(content, bp)
            .iter()
            .filter(|p| p.is_error())
            .map(|p| p.to_string())
            .collect();
        if !errors.is_empty() {
            return Err(errors.join("; "));
        }
        let report = sim::design_report(content, bp, &SimConfig::default())?;
        if !report.flight_ready() {
            return Err("it cannot fly (no powered logic core, or no thrust)".into());
        }
        if report.issues.iter().any(|i| matches!(i, Issue::FallsOff { .. })) {
            return Err("parts fall off".into());
        }
        if report.guns.is_empty() {
            return Err("it has no gun".into());
        }
        if let Some(max) = self.build_points
            && report.build_points > max
        {
            return Err(format!(
                "{:.1} build points, at most {max}",
                report.build_points
            ));
        }
        let rules = BehaviorEditor::from_text(&self.nodes, &g.behavior)
            .map_err(|e| format!("the behavior does not read: {e:?}"))?
            .program()
            .rules
            .len() as u32;
        Ok(Stats {
            top_speed: report.flight.top_speed[report.flight.best],
            acceleration: report.flight.accel[report.flight.best],
            reach: report
                .guns
                .iter()
                .filter(|g| g.clear)
                .map(|g| g.range)
                .fold(0.0, f32::max),
            build_points: report.build_points,
            mass: report.mass,
            firepower: report.latent_firepower(),
            hit_points: report.hp,
            parts: bp.parts.len() as u32,
            rules: rules.min(report.rule_slots),
            rule_slots: report.rule_slots,
        })
    }

    /// Every part has a mirror image across the frame's centre line: a part of the same id and material with
    /// the reflected outline and facing.
    fn symmetric(&self, content: &Content, bp: &Blueprint) -> bool {
        let Some(dock) = self.open(content, bp) else {
            return false;
        };
        type Key = (String, Option<String>, Vec<(i32, i32)>, u8);
        let key = |p: &Placement| -> Option<Key> {
            let part = content.parts.get(&p.part)?;
            let s = PlacedShape {
                footprint: part.footprint(),
                x: p.x,
                y: p.y,
                rot: p.rot,
                flip: p.flip,
            };
            let mut outline: Vec<(i32, i32)> = s.polygon().iter().map(|q| (q.x, q.y)).collect();
            outline.sort_unstable();
            Some((p.part.clone(), p.material.clone(), outline, s.facing().index()))
        };
        let keys: Option<BTreeSet<Key>> = dock.parts().iter().map(key).collect();
        let Some(keys) = keys else {
            return false;
        };
        dock.parts()
            .iter()
            .all(|p| key(&dock.mirror_of(p)).is_some_and(|k| keys.contains(&k)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A design is legal when the drydock opens it in the run's frame (known parts, inside, no overlaps) and the
    /// content's checks find no error in it. (One check is about the behavior, not the design: a design with
    /// fewer rule slots than its faction's default behavior has rules. The pre-filter refuses those.)
    fn legal(c: &Content, kit: &Kit, bp: &Blueprint) {
        assert!(
            Drydock::open(c, bp, kit.frame_for(bp)).is_ok(),
            "the drydock refuses {bp:?}"
        );
        let errors: Vec<_> = data::validate_ship(c, bp)
            .into_iter()
            .filter(|p| p.is_error() && p.field.as_deref() != Some("faction"))
            .collect();
        assert!(errors.is_empty(), "{errors:?}");
    }

    fn kit(c: &Content, mirror: bool) -> Kit {
        let limits = Limits {
            mirror,
            ..Limits::default()
        };
        Kit::new(c, &limits, (24, 24))
    }

    #[test]
    fn every_mutant_and_child_is_legal() {
        let c = data::base_content();
        let skiff = c.ships.get("rim_skiff").unwrap().clone();
        let runner = c.ships.get("halvex_runner").unwrap().clone();
        let text = c.behaviors.get("skirmisher").unwrap().program().unwrap().to_text();
        let raider = c.behaviors.get("raider_basic").unwrap().program().unwrap().to_text();
        let mut rng = Pcg32::new(7, 1);
        for mirror in [false, true] {
            let kit = kit(&c, mirror);
            let (mut a, mut b) = (skiff.clone(), runner.clone());
            let (mut ta, mut tb) = (text.clone(), raider.clone());
            let mut changed = 0;
            for k in 0..400 {
                if let Some(bp) = kit.mutate_design(&c, &a, &mut rng, 3) {
                    legal(&c, &kit, &bp);
                    if mirror {
                        assert!(kit.symmetric(&c, &bp), "a mirror-locked mutant is not symmetric");
                    }
                    changed += 1;
                    // Walk on, but come back to the start now and then so the designs stay ships.
                    a = if k % 20 == 19 { skiff.clone() } else { bp };
                }
                if let Some(bp) = kit.cross_design(&c, &a, &b, &mut rng) {
                    legal(&c, &kit, &bp);
                    b = bp;
                }
                let slots = c.ship_rule_slots(&skiff);
                if let Some(t) = kit.mutate_behavior(&ta, slots, &mut rng, 3) {
                    let ed = BehaviorEditor::from_text(&c.behavior_nodes, &t).expect("a legal program");
                    assert!(ed.program().rules.len() <= slots as usize);
                    assert_eq!(ed.text(), t, "the canonical text");
                    ta = t;
                }
                if let Some(t) = kit.cross_behavior(&ta, &tb, slots, &mut rng) {
                    BehaviorEditor::from_text(&c.behavior_nodes, &t).expect("a legal program");
                    tb = t;
                }
            }
            assert!(changed > 300, "most mutations take ({changed} of 400)");
        }
    }

    #[test]
    fn the_symmetry_check_knows_the_content_ships() {
        let c = data::base_content();
        let kit = kit(&c, true);
        for id in ["rim_skiff", "automaton_raider"] {
            assert!(kit.symmetric(&c, c.ships.get(id).unwrap()), "{id}");
        }
        // One part moved off its twin's mirror cell.
        let mut bp = c.ships.get("rim_skiff").unwrap().clone();
        bp.parts[0].x -= 1;
        assert!(!kit.symmetric(&c, &bp));
    }

    #[test]
    fn a_random_design_flies() {
        let c = data::base_content();
        for mirror in [false, true] {
            let kit = kit(&c, mirror);
            let header = Blueprint {
                parts: Vec::new(),
                size: (0, 0),
                ..c.ships.get("rim_skiff").unwrap().clone()
            };
            let mut rng = Pcg32::new(3, 1);
            let g = kit.random_design(&c, &header, "always -> approach", &mut rng).unwrap();
            legal(&c, &kit, &g.blueprint);
            assert!(kit.assess(&c, &g).is_ok());
        }
    }
}
