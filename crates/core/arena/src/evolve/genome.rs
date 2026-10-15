//! Genomes (a blueprint and a behavior text, in their start's frame), and how they change: mutation and crossover
//! through the editing models (`data::design::edit::Drydock` for designs, opened with `open_placed` so a genome keeps
//! its cells; `data::behavior::edit::BehaviorEditor` for behaviors), so every child is a legal design and a legal
//! program; the locks (GDR-0092), which no operator picks; the pre-filter (`sim::design_report`, the limits, the
//! locks) before any fight.

use std::collections::{BTreeMap, BTreeSet};

use data::behavior::{CmpOp, ListKind, MAX_RULES, Program, Rule, TargetLine, Value};
use data::behavior::edit::{BehaviorEditor, NodeRef};
use data::career::Availability;
use data::design::edit::{Drydock, Outcome, Refusal, Twin};
use data::design::geometry::{Cell, PlacedShape};
use data::{BehaviorNode, Blueprint, Content, NodeRole, ParamType, Placement, Shape, Table};
use sim::report::Issue;
use sim::{Pcg32, SimConfig};

use super::config::{Limits, Locks};

/// A design and the behavior it flies: what evolves. Both are ordinary content: the blueprint saves as a design,
/// the behavior text as a behavior (`docs/behavior-format.md`).
#[derive(Clone, Debug, PartialEq)]
pub struct Genome {
    /// Trimmed to its parts (as the drydock exports it); its `behavior` field is empty (the behavior is the
    /// text beside it).
    pub blueprint: Blueprint,
    /// The program in its canonical text form.
    pub behavior: String,
    /// The start it descends from (its index in the run's starts), whose frame and locks it keeps.
    pub root: u32,
    /// Where the blueprint's top-left cell sits in its start's frame: the genome keeps its cells from generation to
    /// generation, so the locks (frame cells) never drift.
    pub at: (i32, i32),
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

    /// The design in its start's frame `frame` (`Run::frame`), as the drydock holds and saves a design: `size` = the
    /// frame, the parts in frame coordinates.
    pub fn placed(&self, frame: (u32, u32)) -> Blueprint {
        Blueprint {
            size: frame,
            parts: self.frame_parts(),
            ..self.blueprint.clone()
        }
    }

    /// The parts in frame coordinates.
    pub(crate) fn frame_parts(&self) -> Vec<Placement> {
        let (dx, dy) = self.at;
        self.blueprint
            .parts
            .iter()
            .map(|p| Placement {
                x: p.x + dx,
                y: p.y + dy,
                ..p.clone()
            })
            .collect()
    }

    /// The design a drydock holds, flying `behavior`.
    pub(crate) fn of(dock: &Drydock, behavior: String, root: u32) -> Genome {
        let b = dock.bounds();
        Genome {
            blueprint: dock.export(),
            behavior,
            root,
            at: b.map_or((0, 0), |b| (b.min_x, b.min_y)),
        }
    }
}

/// What a genome keeps of its start (the run's [`Locks`], resolved into the start's frame).
#[derive(Clone, Debug, Default)]
pub(crate) struct Locked {
    /// Placements in frame coordinates that must stay exactly as they are.
    parts: Vec<Placement>,
    /// KEEP EMPTY frame cells.
    cells: BTreeSet<Cell>,
    rules: Vec<Rule>,
    /// `Some(t)`: the target line must stay `t`.
    target: Option<Option<TargetLine>>,
    order: bool,
}

impl Locked {
    /// The run file's locks of a start: `bp` as written and `dock`, the start opened in its frame (centred, part
    /// order kept), `program` its behavior. Under the mirror lock a locked part or cell locks its mirror image too.
    pub fn resolve(
        content: &Content,
        locks: &Locks,
        bp: &Blueprint,
        dock: &Drydock,
        program: &Program,
    ) -> Result<Locked, String> {
        let mut parts: BTreeSet<usize> = BTreeSet::new();
        for &i in &locks.parts {
            if i as usize >= dock.parts().len() {
                return Err(format!("locked part {i}: the start has {} parts", dock.parts().len()));
            }
            parts.insert(i as usize);
        }
        for c in &locks.categories {
            if !content.categories.contains(c) {
                return Err(format!("locked category `{c}`: no such part category"));
            }
            let of = |p: &Placement| content.parts.get(&p.part).is_some_and(|q| q.category == *c);
            parts.extend((0..dock.parts().len()).filter(|&i| of(&dock.parts()[i])));
        }
        let shift = match (bp.parts.first(), dock.parts().first()) {
            (Some(a), Some(b)) => (b.x - a.x, b.y - a.y),
            _ => (0, 0),
        };
        let mut cells = BTreeSet::new();
        for &(x, y) in &locks.cells {
            let cell = (x + shift.0, y + shift.1);
            let (w, h) = dock.frame();
            if cell.0 < 0 || cell.1 < 0 || cell.0 >= w as i32 || cell.1 >= h as i32 {
                return Err(format!("the empty cell ({x}, {y}) lies outside the run's frame"));
            }
            if dock.part_at(cell).is_some() {
                return Err(format!("the cell ({x}, {y}) is to stay empty, but the start has a part there"));
            }
            cells.insert(cell);
        }
        if dock.mirror {
            let twins: Vec<usize> = parts
                .iter()
                .filter_map(|&i| dock.part_at(dock.mirror_cell(dock.shapes()[i].cells()[0])))
                .collect();
            parts.extend(twins);
            let mirrored: Vec<Cell> = cells.iter().map(|&c| dock.mirror_cell(c)).collect();
            cells.extend(mirrored);
        }
        let mut rules = Vec::new();
        for &i in &locks.rules {
            let r = program
                .rules
                .get(i as usize)
                .ok_or_else(|| format!("locked rule {i}: the behavior has {} rules", program.rules.len()))?;
            rules.push(r.clone());
        }
        Ok(Locked {
            parts: parts.into_iter().map(|i| dock.parts()[i].clone()).collect(),
            cells,
            rules,
            target: locks.target.then(|| program.target.clone()),
            order: locks.order,
        })
    }

    fn part(&self, p: &Placement) -> bool {
        self.parts.contains(p)
    }

    fn rule(&self, r: &Rule) -> bool {
        self.rules.contains(r)
    }

    fn covers_empty(&self, s: &PlacedShape) -> bool {
        !self.cells.is_empty() && s.cells().iter().any(|c| self.cells.contains(c))
    }

    /// The design in `dock` keeps every locked part and leaves every KEEP EMPTY cell empty.
    fn design_holds(&self, dock: &Drydock) -> bool {
        self.parts.iter().all(|p| dock.parts().contains(p))
            && self.cells.iter().all(|&c| dock.part_at(c).is_none())
    }

    /// The program keeps every locked rule (in their order under LOCK ORDER) and a locked target line.
    fn behavior_holds(&self, p: &Program) -> bool {
        if self.target.as_ref().is_some_and(|t| *t != p.target) {
            return false;
        }
        if !self.rules.iter().all(|r| p.rules.contains(r)) {
            return false;
        }
        if self.order {
            // Each locked rule comes after the one before it.
            let mut want = self.rules.iter().peekable();
            for r in &p.rules {
                if want.peek() == Some(&r) {
                    want.next();
                }
            }
            return want.peek().is_none();
        }
        true
    }
}

/// What a genome must keep and may be, per start.
pub(crate) struct Root {
    /// The frame it is edited in: the run's, one column narrower under the mirror lock when the start's width has
    /// the other parity (a design is mirror-aligned only then, `docs/drydock.md` "Mirror mode").
    pub frame: (u32, u32),
    /// The most build points and the smallest hold of its genomes ([`super::config::Bound`] resolved).
    pub build_points: Option<f32>,
    /// The start's own build points (CHEAP's measure when the run caps none).
    pub start_bp: f32,
    pub min_hold: Option<f32>,
    pub locks: Locked,
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

/// Everything breeding needs, fixed for a run: the limits, the parts and materials to pick from, the behavior
/// vocabulary, and per start its frame, limits and locks.
pub(crate) struct Kit {
    pub mirror: bool,
    allowed_parts: Option<BTreeSet<String>>,
    allowed_materials: Option<BTreeSet<String>>,
    counts: BTreeMap<String, u32>,
    max_cores: Option<u32>,
    /// The parts to build with (allowed ones, content order).
    parts: Vec<String>,
    /// The armour materials to build with.
    materials: Vec<String>,
    nodes: Table<BehaviorNode>,
    actions: Vec<String>,
    conditions: Vec<String>,
    targets: Vec<String>,
    /// Per start, in the run's order.
    pub roots: Vec<Root>,
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
    /// The run's limits; the starts come with [`Kit::roots`].
    pub fn new(content: &Content, limits: &Limits) -> Kit {
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
            mirror: limits.mirror,
            allowed_parts: limits.parts.clone(),
            allowed_materials: limits.materials.clone(),
            counts: limits.counts.clone().unwrap_or_default(),
            max_cores: limits.max_cores,
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
            roots: Vec::new(),
        }
    }

    /// The frame a start of `size` (trimmed) is edited in, in a run whose frame is `frame`: one column narrower
    /// under the mirror lock when the widths' parity differs.
    pub fn frame_for(&self, frame: (u32, u32), size: (u32, u32)) -> (u32, u32) {
        let (w, h) = frame;
        if self.mirror && w > 1 && (w ^ size.0) & 1 == 1 {
            (w - 1, h)
        } else {
            (w, h)
        }
    }

    /// A drydock on `bp` in `frame`: the parts centred (`placed` false: a start, opened once) or kept where they are,
    /// with the mirror lock and the part counts.
    pub fn dock(&self, content: &Content, bp: &Blueprint, frame: (u32, u32), placed: bool) -> Option<Drydock> {
        let open = if placed { Drydock::open_placed } else { Drydock::open };
        let mut dock = open(content, bp, frame).ok()?;
        dock.mirror = self.mirror;
        for (part, &n) in &self.counts {
            dock.set_available(part, Availability::Count(n));
        }
        Some(dock)
    }

    /// A genome in a drydock, in its start's frame, where its parts are.
    fn open(&self, content: &Content, g: &Genome) -> Option<Drydock> {
        let frame = self.roots.get(g.root as usize)?.frame;
        self.dock(content, &g.placed(frame), frame, true)
    }

    // ---- designs ----

    /// The genome after `n` design mutations (each tried up to [`TRIES`] times); `None` if none took. An edit
    /// that breaks a lock (a locked part's mirror twin removed, a part over a KEEP EMPTY cell) is undone.
    pub fn mutate_design(&self, content: &Content, g: &Genome, rng: &mut Pcg32, n: u32) -> Option<Genome> {
        let locks = &self.roots.get(g.root as usize)?.locks;
        let mut dock = self.open(content, g)?;
        let mut any = false;
        for _ in 0..n {
            for _ in 0..TRIES {
                if self.design_op(content, &mut dock, locks, rng) {
                    if locks.design_holds(&dock) {
                        any = true;
                        break;
                    }
                    dock.undo();
                }
            }
        }
        any.then(|| Genome::of(&dock, g.behavior.clone(), g.root))
    }

    /// One random design edit on an unlocked part (or a new one); true if it took.
    fn design_op(&self, content: &Content, dock: &mut Drydock, locks: &Locked, rng: &mut Pcg32) -> bool {
        let free: Vec<usize> = (0..dock.parts().len())
            .filter(|&i| !locks.part(&dock.parts()[i]))
            .collect();
        let n = free.len() as u32;
        let op = if n == 0 { 0 } else { weighted(rng, &DESIGN_OPS) };
        let i = free.get(rng.below(n) as usize).copied().unwrap_or(0);
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

    /// Splices two designs of one frame: `a`'s parts above a random row (and its locked parts below it), `b`'s
    /// from that row down where they fit (`b`'s parts that would overlap or cover a KEEP EMPTY cell of `a`'s start
    /// are left out). The child is `a`'s: its start, its behavior. `None` if the frames differ or nothing changed.
    pub fn cross_design(&self, content: &Content, a: &Genome, b: &Genome, rng: &mut Pcg32) -> Option<Genome> {
        let root = self.roots.get(a.root as usize)?;
        if Some(root.frame) != self.roots.get(b.root as usize).map(|r| r.frame) {
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
            if da.shapes()[i].y >= cut && !root.locks.part(&da.parts()[i]) {
                da.remove_part(i).ok()?;
            }
        }
        for (p, s) in db.parts().iter().zip(db.shapes()) {
            if s.y >= cut && da.check(p).is_ok() && !root.locks.covers_empty(s) {
                da.place(p.clone()).ok()?;
            }
        }
        let child = Genome::of(&da, a.behavior.clone(), a.root);
        (child != *a).then_some(child)
    }

    /// Builds a random design for start `root` that passes [`Kit::assess`]: a logic core in the frame's middle,
    /// then a power source, an engine and a gun next to it, then a few random parts. `None` after many tries (the
    /// parts allowed cannot make a ship that flies).
    pub fn random_design(
        &self,
        content: &Content,
        header: &Blueprint,
        behavior: &str,
        root: u32,
        rng: &mut Pcg32,
    ) -> Option<Genome> {
        let frame = self.roots.get(root as usize)?.frame;
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
            let mut dock = self.dock(content, header, frame, true)?;
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
            let g = Genome::of(&dock, behavior.to_string(), root);
            if self.assess(content, &g).is_ok() {
                return Some(g);
            }
        }
        None
    }

    // ---- behaviors ----

    /// The behavior after `n` mutations (each tried up to [`TRIES`] times) of its unlocked rules and nodes, with at
    /// most `slots` rules; `None` if nothing changed.
    pub fn mutate_behavior(&self, text: &str, slots: u32, root: u32, rng: &mut Pcg32, n: u32) -> Option<String> {
        let locks = &self.roots.get(root as usize)?.locks;
        let mut ed = BehaviorEditor::from_text(&self.nodes, text).ok()?;
        let max_rules = (slots as usize).clamp(1, MAX_RULES);
        for _ in 0..n {
            for _ in 0..TRIES {
                if self.behavior_op(&mut ed, max_rules, locks, rng) {
                    break;
                }
            }
        }
        let out = ed.text();
        (out != text).then_some(out)
    }

    /// One random behavior edit that leaves the locked rules (and a locked target line) alone; true if it changed
    /// the program.
    fn behavior_op(&self, ed: &mut BehaviorEditor, max_rules: usize, locks: &Locked, rng: &mut Pcg32) -> bool {
        let free = |r: &data::behavior::edit::RuleView<'_>| !locks.rule(r.rule);
        let rules: Vec<_> = ed.rules().filter(free).map(|r| r.id).collect();
        let conds: Vec<_> = ed.rules().filter(free).flat_map(|r| r.conds.to_vec()).collect();
        let target = locks.target.is_none().then_some(NodeRef::Target);
        let nodes: Vec<NodeRef> = target
            .into_iter()
            .chain(ed.rules().filter(free).flat_map(|r| r.nodes().collect::<Vec<_>>()))
            .collect();
        let all = ed.rules().len();
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
            // Moving an unlocked rule never changes the order of the locked ones.
            2 => match rule {
                Some(id) if all > 1 => ed.move_rule(id, rng.below(all as u32) as usize),
                _ => return false,
            },
            3 => match (rule, pick(rng, &self.actions)) {
                (Some(id), Some(a)) => ed.replace_node(NodeRef::Action(id), a),
                _ => return false,
            },
            4 => match (target, pick(rng, &self.targets)) {
                (Some(at), Some(t)) => ed.replace_node(at, t),
                _ => return false,
            },
            5 => {
                if all >= max_rules {
                    return false;
                }
                let Some(a) = pick(rng, &self.actions).cloned() else {
                    return false;
                };
                let at = rng.below(all as u32 + 1) as usize;
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

    /// Splices two behaviors: `a`'s first rules (at least one), then `b`'s from a random rule on, the target line
    /// of either. The child is `a`'s, so it keeps `a`'s start's locked rules (in their place among `a`'s, the ones
    /// past the splice before `b`'s rules, which bring no second copy) and a locked target line; then it is cut to
    /// `slots` rules ([`Kit::fit_rules`]). `None` if the result is no legal program or equals `a`.
    pub fn cross_behavior(&self, a: &str, b: &str, slots: u32, root: u32, rng: &mut Pcg32) -> Option<String> {
        let locks = &self.roots.get(root as usize)?.locks;
        let pa = BehaviorEditor::from_text(&self.nodes, a).ok()?.program().clone();
        let pb = BehaviorEditor::from_text(&self.nodes, b).ok()?.program().clone();
        let i = 1 + rng.below(pa.rules.len() as u32) as usize;
        let j = rng.below(pb.rules.len() as u32) as usize;
        let mut rules = pa.rules[..i].to_vec();
        rules.extend(pa.rules[i..].iter().filter(|r| locks.rule(r)).cloned());
        rules.extend(pb.rules[j..].iter().filter(|r| !locks.rule(r)).cloned());
        let target = if locks.target.is_some() || rng.chance(0.5) {
            pa.target
        } else {
            pb.target
        };
        let text = Program { target, rules }.to_text();
        let out = self.fit_rules(&BehaviorEditor::from_text(&self.nodes, &text).ok()?.text(), slots, root)?;
        (out != a).then_some(out)
    }

    /// The behavior cut to `slots` rules (at least one) when it has more: its lowest unlocked rules go first
    /// (GDR-0092: a design mutation that removes rule slots is repaired so). `None` if its locked rules alone don't
    /// fit.
    pub fn fit_rules(&self, text: &str, slots: u32, root: u32) -> Option<String> {
        let max = (slots as usize).clamp(1, MAX_RULES);
        let ed = BehaviorEditor::from_text(&self.nodes, text).ok()?;
        if ed.program().rules.len() <= max {
            return Some(text.to_string());
        }
        let locks = &self.roots.get(root as usize)?.locks;
        let mut p = ed.program().clone();
        while p.rules.len() > max {
            let k = p.rules.iter().rposition(|r| !locks.rule(r))?;
            p.rules.remove(k);
        }
        Some(BehaviorEditor::from_text(&self.nodes, &p.to_text()).ok()?.text())
    }

    // ---- the pre-filter ----

    /// Checks a genome before any fight and measures it: the limits (parts, materials, part counts, its start's
    /// frame, symmetry under the mirror lock, build points, the hold, logic cores), its start's locks, the content's
    /// checks with its own behavior (`data::validate_ship_flying`: the rule slots of its own design), the behavior's
    /// canonical text, and the design report: it must fly (a powered logic core and thrust: no blocker), be one piece
    /// and carry a gun.
    pub fn assess(&self, content: &Content, g: &Genome) -> Result<Stats, String> {
        let bp = &g.blueprint;
        let root = self
            .roots
            .get(g.root as usize)
            .ok_or_else(|| format!("no start {}", g.root))?;
        let mut count: BTreeMap<&str, u32> = BTreeMap::new();
        for p in &bp.parts {
            if self.allowed_parts.as_ref().is_some_and(|s| !s.contains(&p.part)) {
                return Err(format!("the part `{}` is not allowed", p.part));
            }
            if let Some(m) = &p.material
                && self.allowed_materials.as_ref().is_some_and(|s| !s.contains(m))
            {
                return Err(format!("the material `{m}` is not allowed"));
            }
            *count.entry(&p.part).or_default() += 1;
        }
        for (part, n) in count {
            if let Some(&max) = self.counts.get(part)
                && n > max
            {
                return Err(format!("{n} × `{part}`, at most {max}"));
            }
        }
        let (fw, fh) = root.frame;
        if g.at.0 < 0 || g.at.1 < 0 || g.at.0 as u32 + bp.size.0 > fw || g.at.1 as u32 + bp.size.1 > fh {
            return Err(format!(
                "{} × {} cells at {:?} do not fit the {fw} × {fh} frame",
                bp.size.0, bp.size.1, g.at
            ));
        }
        let dock = self
            .open(content, g)
            .ok_or("the design does not open in its frame")?;
        if self.mirror && !self.symmetric(content, &dock) {
            return Err("not symmetric under the mirror lock".into());
        }
        if !root.locks.design_holds(&dock) {
            return Err("a locked part or KEEP EMPTY cell changed".into());
        }
        let ed = BehaviorEditor::from_text(&self.nodes, &g.behavior)
            .map_err(|e| format!("the behavior does not read: {e:?}"))?;
        if ed.text() != g.behavior {
            return Err("the behavior is not in its canonical text form".into());
        }
        if !root.locks.behavior_holds(ed.program()) {
            return Err("a locked rule changed".into());
        }
        let errors: Vec<String> = data::validate_ship_flying(content, bp, ed.program())
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
        if let Some(max) = root.build_points
            && report.build_points > max
        {
            return Err(format!("{:.1} build points, at most {max}", report.build_points));
        }
        if let Some(min) = root.min_hold {
            let hold = content.ship_hold(bp);
            if (hold as f32) < min {
                return Err(format!("a hold of {hold}, at least {min}"));
            }
        }
        if let Some(max) = self.max_cores {
            let cores = content.ship_totals(bp).logic_cores;
            if cores > max {
                return Err(format!("{cores} logic cores, the mind runs {max}"));
            }
        }
        let rules = ed.program().rules.len() as u32;
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
    fn symmetric(&self, content: &Content, dock: &Drydock) -> bool {
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

    /// A design is legal when the drydock opens it where it is in its start's frame (known parts, inside, no
    /// overlaps) and the content's checks find no error in it. (One check is about the behavior, not the design: a
    /// design with fewer rule slots than its faction's default behavior has rules. The pre-filter checks the
    /// genome's own.)
    fn legal(c: &Content, kit: &Kit, g: &Genome) {
        let frame = kit.roots[g.root as usize].frame;
        assert!(
            Drydock::open_placed(c, &g.placed(frame), frame).is_ok(),
            "the drydock refuses {g:?}"
        );
        let errors: Vec<_> = data::validate_ship(c, &g.blueprint)
            .into_iter()
            .filter(|p| p.is_error() && p.field.as_deref() != Some("faction"))
            .collect();
        assert!(errors.is_empty(), "{errors:?}");
    }

    fn new_kit(c: &Content, mirror: bool) -> Kit {
        let limits = Limits {
            mirror,
            ..Limits::default()
        };
        Kit::new(c, &limits)
    }

    fn text(c: &Content, behavior: &str) -> String {
        c.behaviors.get(behavior).unwrap().program().unwrap().to_text()
    }

    /// A start in a 24 × 24 frame with these locks: its genome.
    fn start(c: &Content, kit: &mut Kit, ship: &str, behavior: &str, locks: &Locks) -> Genome {
        let bp = c.ships.get(ship).unwrap();
        let frame = kit.frame_for((24, 24), bp.size);
        let dock = kit.dock(c, bp, frame, false).unwrap();
        let program = Program::parse(&text(c, behavior)).unwrap();
        let locks = Locked::resolve(c, locks, bp, &dock, &program).unwrap();
        kit.roots.push(Root {
            frame,
            build_points: None,
            start_bp: 0.0,
            min_hold: None,
            locks,
        });
        Genome::of(&dock, text(c, behavior), kit.roots.len() as u32 - 1)
    }

    #[test]
    fn every_mutant_and_child_is_legal() {
        let c = data::base_content();
        let mut rng = Pcg32::new(7, 1);
        for mirror in [false, true] {
            let mut kit = new_kit(&c, mirror);
            let skiff = start(&c, &mut kit, "rim_skiff", "skirmisher", &Locks::default());
            let runner = start(&c, &mut kit, "halvex_runner", "raider_basic", &Locks::default());
            let (mut a, mut b) = (skiff.clone(), runner.clone());
            let mut changed = 0;
            let slots = c.ship_rule_slots(&skiff.blueprint);
            for k in 0..400 {
                if let Some(g) = kit.mutate_design(&c, &a, &mut rng, 3) {
                    legal(&c, &kit, &g);
                    if mirror {
                        assert!(kit.symmetric(&c, &kit.open(&c, &g).unwrap()), "a mirror-locked mutant is not symmetric");
                    }
                    changed += 1;
                    // Walk on, but come back to the start now and then so the designs stay ships.
                    a = if k % 20 == 19 { skiff.clone() } else { g };
                }
                if let Some(g) = kit.cross_design(&c, &a, &b, &mut rng) {
                    legal(&c, &kit, &g);
                    assert_eq!(g.root, a.root);
                    b = g;
                }
                if let Some(t) = kit.mutate_behavior(&a.behavior, slots, a.root, &mut rng, 3) {
                    let ed = BehaviorEditor::from_text(&c.behavior_nodes, &t).expect("a legal program");
                    assert!(ed.program().rules.len() <= slots as usize);
                    assert_eq!(ed.text(), t, "the canonical text");
                    a.behavior = t;
                }
                if let Some(t) = kit.cross_behavior(&a.behavior, &runner.behavior, slots, a.root, &mut rng) {
                    BehaviorEditor::from_text(&c.behavior_nodes, &t).expect("a legal program");
                    a.behavior = t;
                }
            }
            assert!(changed > 300, "most mutations take ({changed} of 400)");
        }
    }

    /// GDR-0092: the operators never pick a locked part, cell or rule, and crossover keeps them; a design that
    /// lost rule slots drops its lowest unlocked rules.
    #[test]
    fn locks_hold_through_mutation_and_crossover() {
        let c = data::base_content();
        let mut kit = new_kit(&c, false);
        let locks = Locks {
            parts: vec![0],
            categories: vec!["weapons".into()],
            cells: vec![(-1, 0)],
            rules: vec![0],
            target: true,
            order: true,
        };
        let skiff = start(&c, &mut kit, "rim_skiff", "skirmisher", &locks);
        let other = start(&c, &mut kit, "rim_skiff", "raider_basic", &Locks::default());
        let held = &kit.roots[0].locks;
        assert!(held.parts.len() > 1 && held.cells.len() == 1 && held.rules.len() == 1);
        let mut rng = Pcg32::new(3, 1);
        let mut g = skiff.clone();
        let slots = 64;
        for k in 0..300 {
            if let Some(m) = kit.mutate_design(&c, &g, &mut rng, 3) {
                g = m;
            }
            if let Some(m) = kit.cross_design(&c, &g, &other, &mut rng) {
                g = m;
            }
            if let Some(t) = kit.mutate_behavior(&g.behavior, slots, 0, &mut rng, 3) {
                g.behavior = t;
            }
            if let Some(t) = kit.cross_behavior(&g.behavior, &other.behavior, slots, 0, &mut rng) {
                g.behavior = t;
            }
            let dock = kit.open(&c, &g).unwrap();
            assert!(held.design_holds(&dock), "generation {k}");
            let p = Program::parse(&g.behavior).unwrap();
            assert!(held.behavior_holds(&p), "generation {k}: {}", g.behavior);
            if k % 30 == 29 {
                g = Genome { behavior: g.behavior, ..skiff.clone() };
            }
        }
        // The repair: the lowest unlocked rules go; the locked ones alone must fit.
        let five = "always -> approach\nwhen hull < 30% -> flee\nalways -> orbit\nalways -> hold\nalways -> face\n";
        let mut kit = new_kit(&c, false);
        let bp = c.ships.get("rim_skiff").unwrap();
        let dock = kit.dock(&c, bp, (24, 24), false).unwrap();
        let program = Program::parse(five).unwrap();
        let locks = Locks { rules: vec![0, 4], ..Locks::default() };
        let locks = Locked::resolve(&c, &locks, bp, &dock, &program).unwrap();
        kit.roots.push(Root { frame: (24, 24), build_points: None, start_bp: 0.0, min_hold: None, locks });
        let text = BehaviorEditor::from_text(&c.behavior_nodes, five).unwrap().text();
        let three = kit.fit_rules(&text, 3, 0).unwrap();
        let rules = Program::parse(&three).unwrap().rules;
        assert_eq!(rules.len(), 3);
        assert_eq!((rules[0].clone(), rules[2].clone()), (program.rules[0].clone(), program.rules[4].clone()));
        assert_eq!(kit.fit_rules(&text, 1, 0), None);
        assert_eq!(kit.fit_rules(&text, 9, 0).as_deref(), Some(text.as_str()));
    }

    #[test]
    fn the_symmetry_check_knows_the_content_ships() {
        let c = data::base_content();
        let mut kit = new_kit(&c, true);
        for id in ["rim_skiff", "automaton_raider"] {
            let g = start(&c, &mut kit, id, "raider_basic", &Locks::default());
            assert!(kit.symmetric(&c, &kit.open(&c, &g).unwrap()), "{id}");
        }
        // One part moved off its twin's mirror cell.
        let mut g = start(&c, &mut kit, "rim_skiff", "raider_basic", &Locks::default());
        g.blueprint.parts[0].x -= 1;
        assert!(kit.open(&c, &g).is_none_or(|d| !kit.symmetric(&c, &d)));
    }

    #[test]
    fn a_random_design_flies() {
        let c = data::base_content();
        for mirror in [false, true] {
            let mut kit = new_kit(&c, mirror);
            let frame = kit.frame_for((24, 24), (0, 0));
            kit.roots.push(Root {
                frame,
                build_points: None,
                start_bp: 0.0,
                min_hold: None,
                locks: Locked::default(),
            });
            let header = Blueprint {
                parts: Vec::new(),
                size: (0, 0),
                ..c.ships.get("rim_skiff").unwrap().clone()
            };
            let mut rng = Pcg32::new(3, 1);
            let g = kit.random_design(&c, &header, "always -> approach\n", 0, &mut rng).unwrap();
            legal(&c, &kit, &g);
            assert!(kit.assess(&c, &g).is_ok());
        }
    }
}
