//! Genomes (a blueprint with its pilot assists and a behavior text, in their start's frame), and how they change:
//! mutation and crossover through the editing models (`data::design::edit::Drydock` for designs, opened with
//! `open_placed` so a genome keeps its cells; `data::behavior::edit::BehaviorEditor` for behaviors and, as an assist
//! list, for the assists), so every child is a legal design and legal programs; the genes a run may change (GDR-0092
//! "MAY CHANGE", [`MayChange`]) and the locks, which no operator picks; the pre-filter (`sim::design_report`, the
//! limits, the locks) before any fight.

use std::collections::{BTreeMap, BTreeSet};

use data::behavior::{CmpOp, ListKind, MAX_RULES, Program, Rule, TargetLine, Value};
use data::behavior::edit::{BehaviorEditor, NodeRef, RuleId};
use data::career::Availability;
use data::design::edit::{Drydock, Outcome, Refusal, Twin};
use data::design::geometry::{Cell, PlacedShape};
use data::{AssistList, BehaviorNode, Blueprint, Content, MAX_POWER_WEIGHT, NodeRole, ParamType, Placement, PowerOverride, PowerTier, Shape, Table};
use sim::report::Issue;
use sim::{Pcg32, SimConfig};

use super::config::{Gene, Limits, Locks, MayChange};

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

    /// The pilot assists in their text form (one rule a line; empty without assists).
    pub fn assists_text(&self) -> String {
        assists_text(&self.blueprint)
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

/// A blueprint's pilot assists as text: their rules, one a line (empty without assists).
pub(crate) fn assists_text(bp: &Blueprint) -> String {
    bp.assists.as_ref().and_then(|a| a.program().ok()).map_or_else(String::new, |p| p.to_text())
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
    /// Locked rules of the pilot assists.
    assists: Vec<Rule>,
    order: bool,
}

impl Locked {
    /// The run file's locks of a start: `bp` as written (with its assists) and `dock`, the start opened in its frame
    /// (centred, part order kept), `program` its behavior. Under the mirror lock a locked part or cell locks its mirror
    /// image too, and a part without a mirror image ([`unmatched`]) is locked.
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
            parts.extend(unmatched(content, dock));
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
        let assist_rules = match &bp.assists {
            Some(a) => a.program().map_err(|e| format!("the assists do not read: {e:?}"))?.rules,
            None => Vec::new(),
        };
        let mut assists = Vec::new();
        for &i in &locks.assists {
            let r = assist_rules
                .get(i as usize)
                .ok_or_else(|| format!("locked assist {i}: the assists have {} rules", assist_rules.len()))?;
            assists.push(r.clone());
        }
        Ok(Locked {
            parts: parts.into_iter().map(|i| dock.parts()[i].clone()).collect(),
            cells,
            rules,
            target: locks.target.then(|| program.target.clone()),
            assists,
            order: locks.order,
        })
    }

    fn part(&self, p: &Placement) -> bool {
        self.parts.contains(p)
    }

    fn rule(&self, r: &Rule) -> bool {
        self.rules.contains(r)
    }

    /// The locked rules of a list of this kind.
    fn rules_of(&self, kind: ListKind) -> &[Rule] {
        match kind {
            ListKind::Behavior => &self.rules,
            ListKind::Assists => &self.assists,
        }
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
        self.target.as_ref().is_none_or(|t| *t == p.target) && self.list_holds(&self.rules, p)
    }

    /// The pilot assists keep every locked assist rule (in their order under LOCK ORDER).
    fn assists_hold(&self, p: &Program) -> bool {
        self.list_holds(&self.assists, p)
    }

    fn list_holds(&self, locked: &[Rule], p: &Program) -> bool {
        if !locked.iter().all(|r| p.rules.contains(r)) {
            return false;
        }
        if self.order {
            // Each locked rule comes after the one before it.
            let mut want = locked.iter().peekable();
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

/// A design operator on a part (GDR-0092 "The mutation operators"); adding one is [`Mutation::AddPart`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DesignOp {
    Remove,
    Move,
    Rotate,
    Flip,
    Swap,
    Material,
    Power,
}

/// An operator on a list of rules: the behavior or the pilot assists (`Start` and `Target` are of one kind only).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuleOp {
    Nudge,
    Choice,
    /// An assist rule's `start`: on or off at a mission's start (nobody presses a toggle in the lab).
    Start,
    Move,
    Swap,
    Duplicate,
    Action,
    Target,
    Add,
    Remove,
    AddCond,
    RemoveCond,
    Negate,
    ReplaceCond,
}

/// What one mutation edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mutation {
    AddPart,
    Design(DesignOp),
    Behavior(RuleOp),
    Assists(RuleOp),
}

/// Every operator with its gene and weight (GDR-0092's tables; PLACEHOLDERS, `lab.ron` with step 8). One mutation
/// draws among the operators of the genes a run may change, by these weights.
const MUTATIONS: [(Gene, Mutation, u32); 31] = {
    use DesignOp as D;
    use Mutation::{AddPart, Assists as A, Behavior as B, Design};
    use RuleOp as R;
    [
        (Gene::Parts, AddPart, 30),
        (Gene::Parts, Design(D::Remove), 15),
        (Gene::Parts, Design(D::Swap), 15),
        (Gene::Layout, Design(D::Move), 15),
        (Gene::Layout, Design(D::Rotate), 10),
        (Gene::Layout, Design(D::Flip), 5),
        (Gene::Armour, Design(D::Material), 10),
        (Gene::Power, Design(D::Power), 5),
        (Gene::Numbers, B(R::Nudge), 25),
        (Gene::Numbers, B(R::Choice), 5),
        (Gene::Rules, B(R::Move), 8),
        (Gene::Rules, B(R::Swap), 4),
        (Gene::Rules, B(R::Duplicate), 4),
        (Gene::Rules, B(R::Action), 8),
        (Gene::Rules, B(R::Target), 5),
        (Gene::Rules, B(R::Add), 8),
        (Gene::Rules, B(R::Remove), 7),
        (Gene::Rules, B(R::AddCond), 7),
        (Gene::Rules, B(R::RemoveCond), 5),
        (Gene::Rules, B(R::Negate), 4),
        (Gene::Rules, B(R::ReplaceCond), 4),
        (Gene::AssistNumbers, A(R::Nudge), 25),
        (Gene::AssistNumbers, A(R::Choice), 10),
        (Gene::AssistNumbers, A(R::Start), 8),
        (Gene::AssistRules, A(R::Move), 10),
        (Gene::AssistRules, A(R::Add), 12),
        (Gene::AssistRules, A(R::Remove), 8),
        (Gene::AssistRules, A(R::AddCond), 8),
        (Gene::AssistRules, A(R::RemoveCond), 6),
        (Gene::AssistRules, A(R::Negate), 4),
        (Gene::AssistRules, A(R::ReplaceCond), 4),
    ]
};
/// The operators on one rule, for the mutation a duplicated rule gets: their weights as in [`MUTATIONS`].
const ON_A_RULE: [(RuleOp, u32); 7] = [
    (RuleOp::Nudge, 25),
    (RuleOp::Choice, 5),
    (RuleOp::Action, 8),
    (RuleOp::AddCond, 7),
    (RuleOp::RemoveCond, 5),
    (RuleOp::Negate, 4),
    (RuleOp::ReplaceCond, 4),
];
/// Tries per mutation before it is given up (an operator may be refused, e.g. a move into another part).
const TRIES: u32 = 8;
/// The power tiers a power-priority mutation steps through (`Off` would only switch a part off).
const TIERS: [PowerTier; 4] = [PowerTier::Critical, PowerTier::High, PowerTier::Normal, PowerTier::Low];

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
    /// The assist list's vocabulary: the assists, and the conditions (the pilot's own with the behaviors').
    assist_actions: Vec<String>,
    assist_conditions: Vec<String>,
    /// Per start, in the run's order: the starts numbered from `first` (the enemy's come after yours in a run's
    /// genome table, GDR-0092 "Co-evolution").
    pub roots: Vec<Root>,
    pub first: u32,
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

/// The weapon group of the gun nearest to `cell` (by its first cell), the lower part on a tie; `None` (group 1)
/// without guns.
fn nearest_group(content: &Content, dock: &Drydock, (x, y): Cell) -> Option<u8> {
    dock.parts()
        .iter()
        .zip(dock.shapes())
        .filter(|(p, _)| content.parts.get(&p.part).is_some_and(|q| q.weapon.is_some()))
        .min_by_key(|(_, s)| {
            let (a, b) = s.cells()[0];
            (a - x).abs() + (b - y).abs()
        })
        .and_then(|(p, _)| p.group)
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
        let palette = |role: NodeRole, kind: ListKind| {
            content
                .behavior_nodes
                .iter()
                .filter(|n| n.role == role && kind.allows(n))
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
            actions: palette(NodeRole::Action, ListKind::Behavior),
            conditions: palette(NodeRole::Condition, ListKind::Behavior),
            targets: palette(NodeRole::Target, ListKind::Behavior),
            assist_actions: palette(NodeRole::Action, ListKind::Assists),
            assist_conditions: palette(NodeRole::Condition, ListKind::Assists),
            roots: Vec::new(),
            first: 0,
        }
    }

    /// Start `root`'s frame, limits and locks (a start of the run's numbering: [`Kit::first`] is the kit's first).
    pub fn root(&self, root: u32) -> Option<&Root> {
        self.roots.get(root.checked_sub(self.first)? as usize)
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
        let frame = self.root(g.root)?.frame;
        self.dock(content, &g.placed(frame), frame, true)
    }

    // ---- mutation ----

    /// The genome after one mutation of a gene `may` lets change, drawn by the operators' weights (each tried up to
    /// [`TRIES`] times); `None` if none took. A design edit that breaks a lock (a locked part's mirror twin removed, a
    /// part over a KEEP EMPTY cell) is undone; no operator picks a locked part, cell or rule.
    pub fn mutate(&self, content: &Content, g: &Genome, may: &MayChange, rng: &mut Pcg32) -> Option<Genome> {
        let locks = &self.root(g.root)?.locks;
        let ops: Vec<(Mutation, u32)> = MUTATIONS.iter().filter(|(gene, ..)| may.has(*gene)).map(|&(_, m, w)| (m, w)).collect();
        let weights: Vec<u32> = ops.iter().map(|o| o.1).collect();
        if ops.is_empty() {
            return None;
        }
        // Each opened once, on the first try that needs it: a refused edit leaves it as it was.
        let mut dock = None;
        let mut behavior = None;
        let mut assists = None;
        for _ in 0..TRIES {
            match ops[weighted(rng, &weights)].0 {
                m @ (Mutation::AddPart | Mutation::Design(_)) => {
                    if dock.is_none() {
                        dock = Some(self.open(content, g)?);
                    }
                    let d = dock.as_mut()?;
                    let took = match m {
                        Mutation::Design(op) => self.design_op(content, d, locks, may, op, rng),
                        _ => self.add_any(content, d, may, rng),
                    };
                    if took {
                        if locks.design_holds(d) {
                            return Some(Genome::of(d, g.behavior.clone(), g.root));
                        }
                        d.undo();
                    }
                }
                Mutation::Behavior(op) => {
                    if behavior.is_none() {
                        behavior = Some(BehaviorEditor::from_text(&self.nodes, &g.behavior).ok()?);
                    }
                    let ed = behavior.as_mut()?;
                    let max = (content.ship_rule_slots(&g.blueprint) as usize).clamp(1, MAX_RULES);
                    // Two equal rules trade places and the text stays: no change.
                    if self.rule_op(ed, op, max, locks, None, rng) && ed.text() != g.behavior {
                        return Some(Genome { behavior: ed.text(), ..g.clone() });
                    }
                }
                Mutation::Assists(op) => {
                    if assists.is_none() {
                        assists = Some(BehaviorEditor::from_text_as(&self.nodes, &g.assists_text(), ListKind::Assists).ok()?);
                    }
                    let ed = assists.as_mut()?;
                    if self.rule_op(ed, op, MAX_RULES, locks, None, rng) && AssistList::of(ed.program()) != g.blueprint.assists {
                        let mut out = g.clone();
                        out.blueprint.assists = AssistList::of(ed.program());
                        return Some(out);
                    }
                }
            }
        }
        None
    }

    /// Adds an allowed part of a category `may` lets change ([`Kit::add_part`]); true if it took.
    fn add_any(&self, content: &Content, dock: &mut Drydock, may: &MayChange, rng: &mut Pcg32) -> bool {
        let parts: Vec<&String> = self.parts.iter().filter(|p| may.changes(content, p)).collect();
        match pick(rng, &parts) {
            Some(&part) => self.add_part(content, dock, rng, part),
            None => false,
        }
    }

    /// One design edit of an unlocked part; true if it took. Parts of a category `may` keeps are not removed or
    /// swapped.
    fn design_op(&self, content: &Content, dock: &mut Drydock, locks: &Locked, may: &MayChange, op: DesignOp, rng: &mut Pcg32) -> bool {
        let free: Vec<usize> = (0..dock.parts().len())
            .filter(|&i| !locks.part(&dock.parts()[i]))
            .filter(|&i| !matches!(op, DesignOp::Remove | DesignOp::Swap) || may.changes(content, &dock.parts()[i].part))
            .collect();
        let Some(&i) = pick(rng, &free) else {
            return false;
        };
        let r = match op {
            DesignOp::Remove => dock.remove_part(i),
            DesignOp::Move => {
                let (dx, dy) = (rng.below(5) as i32 - 2, rng.below(5) as i32 - 2);
                dock.move_part(i, dx, dy)
            }
            DesignOp::Rotate => dock.rotate(i, rng.chance(0.5)),
            DesignOp::Flip => dock.flip(i),
            DesignOp::Swap => return self.swap_part(content, dock, rng, i, may),
            DesignOp::Material => {
                let p = &dock.parts()[i];
                let Some(m) = pick(rng, &self.materials).cloned() else {
                    return false;
                };
                if p.material.is_none() || p.material.as_deref() == Some(&m) {
                    return false;
                }
                dock.set_material(i, &m)
            }
            DesignOp::Power => {
                // A part that draws power: its tier one up or down, or its weight × 0.6–1.4 (the twin follows).
                let p = &dock.parts()[i];
                let Some(spec) = content.parts.get(&p.part).and_then(|q| q.power.as_ref()).filter(|s| s.draw > 0.0) else {
                    return false;
                };
                let now = p.power.unwrap_or_default();
                let (tier, weight) = (now.tier.unwrap_or(spec.tier), now.weight.unwrap_or(spec.weight));
                let set = if rng.chance(0.5) {
                    let k = TIERS.iter().position(|t| *t == tier).unwrap_or(2) as i32 + if rng.chance(0.5) { 1 } else { -1 };
                    let Some(&t) = TIERS.get(k.clamp(0, TIERS.len() as i32 - 1) as usize).filter(|t| **t != tier) else {
                        return false;
                    };
                    PowerOverride { tier: Some(t), ..now }
                } else {
                    let w = ((weight * (0.6 + 0.8 * rng.next_f32()) * 100.0).round() / 100.0).clamp(0.01, MAX_POWER_WEIGHT);
                    PowerOverride { weight: Some(w), ..now }
                };
                dock.set_power(i, Some(set))
            }
        };
        took(r, dock, self.mirror)
    }

    /// Adds a part next to a random part (on a free cell, replacing nothing): a slope oriented to the hull, any
    /// other part in a random pose; a gun in the weapon group of the nearest gun (its mirror twin takes the same, and
    /// the first gun group 1). The first part of an empty design goes in the frame's middle.
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
            let group = if info.weapon.is_some() { nearest_group(content, dock, cell) } else { None };
            let p = Placement {
                part: part.to_string(),
                x: cell.0,
                y: cell.1,
                rot: rng.below(4) as u8,
                flip: rng.chance(0.5),
                material,
                power: None,
                group,
            };
            match dock.preview_place(&p) {
                Ok(pv) if pv.removed.is_empty() => dock.place(p),
                _ => return false,
            }
        };
        took(r, dock, self.mirror)
    }

    /// Puts another part of the same footprint (of a category `may` lets change) in part `i`'s place and pose: the
    /// drydock replaces it. A gun swapped for a gun keeps its weapon group.
    fn swap_part(&self, content: &Content, dock: &mut Drydock, rng: &mut Pcg32, i: usize, may: &MayChange) -> bool {
        let old = dock.parts()[i].clone();
        let Some(info) = content.parts.get(&old.part) else {
            return false;
        };
        let same: Vec<&String> = self
            .parts
            .iter()
            .filter(|q| **q != old.part && may.changes(content, q))
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
        let gun = content.parts.get(q).is_some_and(|p| p.weapon.is_some());
        let p = Placement {
            part: q.clone(),
            material,
            power: None,
            group: old.group.filter(|_| gun),
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
        let root = self.root(a.root)?;
        if Some(root.frame) != self.root(b.root).map(|r| r.frame) {
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
        let frame = self.root(root)?.frame;
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

    // ---- behaviors and assists ----

    /// One edit of a rule list (the behavior, or the pilot assists: `ed`'s kind) that leaves its locked rules (and a
    /// locked target line) alone, with at most `max_rules` rules; true if it changed the list. `only`: the edit is on
    /// that rule (a duplicated rule's own mutation). An assist's toggle is never changed: no stand-in presses it.
    fn rule_op(&self, ed: &mut BehaviorEditor, op: RuleOp, max_rules: usize, locks: &Locked, only: Option<RuleId>, rng: &mut Pcg32) -> bool {
        let kind = ed.kind();
        let (actions, conditions) = match kind {
            ListKind::Behavior => (&self.actions, &self.conditions),
            ListKind::Assists => (&self.assist_actions, &self.assist_conditions),
        };
        let locked = locks.rules_of(kind);
        let free = |r: &data::behavior::edit::RuleView<'_>| !locked.contains(r.rule) && only.is_none_or(|id| r.id == id);
        let rules: Vec<_> = ed.rules().filter(free).map(|r| r.id).collect();
        let conds: Vec<_> = ed.rules().filter(free).flat_map(|r| r.conds.to_vec()).collect();
        let target = (kind == ListKind::Behavior && locks.target.is_none() && only.is_none()).then_some(NodeRef::Target);
        let nodes: Vec<NodeRef> = target
            .into_iter()
            .chain(ed.rules().filter(free).flat_map(|r| r.nodes().collect::<Vec<_>>()))
            .collect();
        // The values a mutation may set: an assist's last two (its toggle and its start, as the sim compiles them)
        // are not choices of this kind.
        let settable = |at: NodeRef, v: &data::behavior::edit::NodeView<'_>, index: usize| {
            let assist = kind == ListKind::Assists && matches!(at, NodeRef::Action(_));
            !assist || index + 2 < v.params.len()
        };
        let all = ed.rules().len();
        let rule = pick(rng, &rules).copied();
        let cond = pick(rng, &conds).copied();
        let r = match op {
            RuleOp::Nudge => {
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
                        if matches!(p.value, Value::Number(..)) && settable(at, &v, p.index) {
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
            RuleOp::Choice | RuleOp::Start => {
                // A choice: another option (an orbit's direction, a target's aim point, an auto-lock's pick); or an
                // assist's start, on or off.
                let mut slots = Vec::new();
                for &at in &nodes {
                    let Some(v) = ed.node(at) else { continue };
                    let start = matches!(at, NodeRef::Action(_)) && kind == ListKind::Assists;
                    for p in &v.params {
                        let ParamType::Choice(opts) = &p.param.ty else { continue };
                        let wanted = match op {
                            RuleOp::Start => start && p.index + 1 == v.params.len(),
                            _ => settable(at, &v, p.index),
                        };
                        if wanted {
                            slots.push((at, p.index, opts.iter().map(|o| o.id.clone()).filter(|o| p.value != Value::Word(o.clone())).collect::<Vec<_>>()));
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
            RuleOp::Move => match rule {
                Some(id) if all > 1 => ed.move_rule(id, rng.below(all as u32) as usize),
                _ => return false,
            },
            RuleOp::Swap => {
                // Two unlocked rules trade places: the first moves to the second's, the second (one up now) to the
                // first's.
                let (Some(a), Some(b)) = (rule, pick(rng, &rules).copied()) else {
                    return false;
                };
                let index = |ed: &BehaviorEditor, id| ed.rules().position(|r| r.id == id);
                let (Some(i), Some(j)) = (index(ed, a), index(ed, b)) else {
                    return false;
                };
                if i == j {
                    return false;
                }
                let (lo, hi) = if i < j { (a, b) } else { (b, a) };
                let (i, j) = (i.min(j), i.max(j));
                ed.move_rule(lo, j).and_then(|_| ed.move_rule(hi, i))
            }
            RuleOp::Duplicate => {
                // A copy below it, then one more mutation on the copy (else the duplicate is undone: a copy alone
                // never decides).
                let Some(id) = rule.filter(|_| all < max_rules) else {
                    return false;
                };
                let Ok(copy) = ed.duplicate_rule(id) else {
                    return false;
                };
                let weights: Vec<u32> = ON_A_RULE.iter().map(|o| o.1).collect();
                for _ in 0..TRIES {
                    if self.rule_op(ed, ON_A_RULE[weighted(rng, &weights)].0, max_rules, locks, Some(copy), rng) {
                        return true;
                    }
                }
                ed.undo();
                return false;
            }
            RuleOp::Action => match (rule, pick(rng, actions)) {
                (Some(id), Some(a)) => ed.replace_node(NodeRef::Action(id), a),
                _ => return false,
            },
            RuleOp::Target => match (target, pick(rng, &self.targets)) {
                (Some(at), Some(t)) => ed.replace_node(at, t),
                _ => return false,
            },
            RuleOp::Add => {
                if all >= max_rules {
                    return false;
                }
                let Some(a) = pick(rng, actions).cloned() else {
                    return false;
                };
                let at = rng.below(all as u32 + 1) as usize;
                let Ok(id) = ed.add_rule(at, &a) else {
                    return false;
                };
                if rng.chance(0.8)
                    && let Some(c) = pick(rng, conditions)
                {
                    // A rule without conditions is fine too; a refused condition leaves it so.
                    let _ = ed.add_condition(id, c);
                }
                return true;
            }
            RuleOp::Remove => match rule {
                Some(id) => ed.delete_rule(id),
                None => return false,
            },
            RuleOp::AddCond => match (rule, pick(rng, conditions)) {
                (Some(id), Some(c)) => ed.add_condition(id, c).map(|_| true),
                _ => return false,
            },
            RuleOp::RemoveCond => match cond {
                Some(c) => ed.remove_condition(c),
                None => return false,
            },
            RuleOp::Negate => match cond {
                Some(c) => ed.toggle_not(c),
                None => return false,
            },
            RuleOp::ReplaceCond => match (cond, pick(rng, conditions)) {
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
        let locks = &self.root(root)?.locks;
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

    /// Splices two genomes' pilot assists as [`Kit::cross_behavior`] splices behaviors: `a`'s first rules (any
    /// number), then `b`'s from a random rule on, `a`'s locked assists past the splice before `b`'s. The child is
    /// `a`'s. `None` if the result is no legal list, too long, or equals `a`'s.
    pub fn cross_assists(&self, a: &Genome, b: &Genome, rng: &mut Pcg32) -> Option<Genome> {
        let locks = &self.root(a.root)?.locks;
        let rules = |g: &Genome| g.blueprint.assists.as_ref().map_or(Ok(Vec::new()), |l| l.program().map(|p| p.rules)).ok();
        let (ra, rb) = (rules(a)?, rules(b)?);
        let i = rng.below(ra.len() as u32 + 1) as usize;
        let j = rng.below(rb.len() as u32 + 1) as usize;
        let mut out = ra[..i].to_vec();
        out.extend(ra[i..].iter().filter(|r| locks.assists.contains(r)).cloned());
        out.extend(rb[j..].iter().filter(|r| !locks.assists.contains(r)).cloned());
        if out.len() > MAX_RULES {
            return None;
        }
        let text = Program { target: None, rules: out }.to_text();
        let ed = BehaviorEditor::from_text_as(&self.nodes, &text, ListKind::Assists).ok()?;
        let mut child = a.clone();
        child.blueprint.assists = AssistList::of(ed.program());
        (child != *a).then_some(child)
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
        let locks = &self.root(root)?.locks;
        let mut p = ed.program().clone();
        while p.rules.len() > max {
            let k = p.rules.iter().rposition(|r| !locks.rule(r))?;
            p.rules.remove(k);
        }
        Some(BehaviorEditor::from_text(&self.nodes, &p.to_text()).ok()?.text())
    }

    /// TIDY (GDR-0092): the behavior without its rules that never decided (`decided`: bit `i` for rule `i`, rules
    /// past the mask's 64 counted as deciding), its start's locked rules kept, and the top rule kept when none
    /// decided; with the dropped rules' indices. `None` if the text doesn't read.
    pub fn tidy(&self, text: &str, decided: u64, root: u32) -> Option<(String, Vec<usize>)> {
        let ed = BehaviorEditor::from_text(&self.nodes, text).ok()?;
        let locks = &self.root(root)?.locks;
        let mut p = ed.program().clone();
        let ran = |i: usize| i >= 64 || decided & (1u64 << i) != 0;
        let mut dropped: Vec<usize> = (0..p.rules.len()).filter(|&i| !ran(i) && !locks.rule(&p.rules[i])).collect();
        if dropped.len() == p.rules.len() {
            dropped.remove(0);
        }
        for &i in dropped.iter().rev() {
            p.rules.remove(i);
        }
        Some((BehaviorEditor::from_text(&self.nodes, &p.to_text()).ok()?.text(), dropped))
    }

    // ---- the pre-filter ----

    /// Checks a genome before any fight and measures it: the limits (parts, materials, part counts, its start's
    /// frame, symmetry under the mirror lock, build points, the hold, logic cores), its start's locks (its assists'
    /// too), the content's checks with its own behavior (its assists among them) (`data::validate_ship_flying`: the rule slots of its own design), the behavior's
    /// canonical text, and the design report: it must fly (a powered logic core and thrust: no blocker), be one piece
    /// and carry a gun.
    pub fn assess(&self, content: &Content, g: &Genome) -> Result<Stats, String> {
        let bp = &g.blueprint;
        let root = self.root(g.root).ok_or_else(|| format!("no start {}", g.root))?;
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
        if self.mirror && !self.symmetric(content, &dock, &root.locks) {
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
        let assists = match &bp.assists {
            Some(a) => a.program().map_err(|e| format!("the assists do not read: {e:?}"))?,
            None => Program::default(),
        };
        if !root.locks.assists_hold(&assists) {
            return Err("a locked assist changed".into());
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

    /// Every part has a mirror image across the frame's centre line ([`unmatched`]), but the parts its start keeps
    /// where they are for having none (`locked`, [`Locked::resolve`]).
    fn symmetric(&self, content: &Content, dock: &Drydock, locked: &Locked) -> bool {
        unmatched(content, dock).into_iter().all(|i| locked.part(&dock.parts()[i]))
    }
}

/// The parts (indices) of `dock` without a mirror image across the frame's centre line: no part of the same id and
/// material with the reflected outline and facing (an unknown part has none). Under the mirror lock a start keeps
/// these where they are: the starter's single scanner and the cargo bay across from it (GDR-0093).
pub fn unmatched(content: &Content, dock: &Drydock) -> Vec<usize> {
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
    let keys: BTreeSet<Key> = dock.parts().iter().filter_map(key).collect();
    (0..dock.parts().len())
        .filter(|&i| key(&dock.parts()[i]).is_none() || key(&dock.mirror_of(&dock.parts()[i])).is_none_or(|k| !keys.contains(&k)))
        .collect()
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

    /// Pilot assists a test start flies with.
    const ASSISTS: [&str; 3] = [
        "always -> auto_lock(nearest, toggle3)",
        "when target_in_range(90%) -> auto_fire(1)",
        "when idle(thrust) >= 1s -> brake",
    ];

    /// A start in a 24 × 24 frame with these locks and the test's assists: its genome.
    fn start(c: &Content, kit: &mut Kit, ship: &str, behavior: &str, locks: &Locks) -> Genome {
        let mut bp = c.ships.get(ship).unwrap().clone();
        bp.assists = Some(AssistList { rules: ASSISTS.iter().map(|r| r.to_string()).collect() });
        let bp = &bp;
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

    /// Every gene changes, and every mutant and child is a legal design with a legal behavior and legal assists
    /// (whose toggles stay as they were).
    #[test]
    fn every_mutant_and_child_is_legal() {
        let c = data::base_content();
        let mut rng = Pcg32::new(7, 1);
        let toggles = |g: &Genome| -> Vec<String> {
            let text = g.assists_text();
            ["toggle1", "toggle2", "toggle3", "toggle4"].iter().filter(|t| text.contains(*t)).map(|t| t.to_string()).collect()
        };
        for mirror in [false, true] {
            let mut kit = new_kit(&c, mirror);
            let skiff = start(&c, &mut kit, "rim_skiff", "skirmisher", &Locks::default());
            let runner = start(&c, &mut kit, "halvex_runner", "raider_basic", &Locks::default());
            let (mut a, mut b) = (skiff.clone(), runner.clone());
            let slots = c.ship_rule_slots(&skiff.blueprint);
            let mut changed = [0; 3];
            for k in 0..600 {
                let Some(g) = kit.mutate(&c, &a, &MayChange::default(), &mut rng) else { continue };
                legal(&c, &kit, &g);
                if mirror {
                    let locks = &kit.roots[g.root as usize].locks;
                    assert!(kit.symmetric(&c, &kit.open(&c, &g).unwrap(), locks), "a mirror-locked mutant is not symmetric");
                }
                let ed = BehaviorEditor::from_text(&c.behavior_nodes, &g.behavior).expect("a legal program");
                assert!(ed.program().rules.len() <= slots.max(1) as usize || g.behavior == a.behavior);
                assert_eq!(ed.text(), g.behavior, "the canonical text");
                let assists = BehaviorEditor::from_text_as(&c.behavior_nodes, &g.assists_text(), ListKind::Assists).expect("legal assists");
                assert_eq!(AssistList::of(assists.program()), g.blueprint.assists);
                assert!(toggles(&g).iter().all(|t| toggles(&skiff).contains(t)), "a toggle came from nowhere: {}", g.assists_text());
                changed[0] += (g.blueprint.parts != a.blueprint.parts) as u32;
                changed[1] += (g.behavior != a.behavior) as u32;
                changed[2] += (g.blueprint.assists != a.blueprint.assists) as u32;
                // Walk on, but come back to the start now and then so the designs stay ships.
                a = if k % 20 == 19 { skiff.clone() } else { g };
                if let Some(g) = kit.cross_design(&c, &a, &b, &mut rng) {
                    legal(&c, &kit, &g);
                    assert_eq!(g.root, a.root);
                    b = g;
                }
                if let Some(t) = kit.cross_behavior(&a.behavior, &runner.behavior, slots, a.root, &mut rng) {
                    BehaviorEditor::from_text(&c.behavior_nodes, &t).expect("a legal program");
                    a.behavior = t;
                }
                if let Some(g) = kit.cross_assists(&a, &runner, &mut rng) {
                    BehaviorEditor::from_text_as(&c.behavior_nodes, &g.assists_text(), ListKind::Assists).expect("legal assists");
                    a = g;
                }
            }
            assert!(changed.iter().sum::<u32>() > 450, "most mutations take ({changed:?} of 600)");
            assert!(changed.iter().all(|&n| n > 40), "every part of the genome changes: {changed:?}");
        }
    }

    /// MAY CHANGE: only the genes switched on change, and the categories kept get no part added, removed or swapped.
    #[test]
    fn only_the_genes_on_change() {
        let c = data::base_content();
        let mut kit = new_kit(&c, false);
        let skiff = start(&c, &mut kit, "rim_skiff", "skirmisher", &Locks::default());
        let off = MayChange { layout: false, parts: false, armour: false, power: false, numbers: false, rules: false, assist_numbers: false, assist_rules: false, fixed: Vec::new() };
        let mut rng = Pcg32::new(5, 1);
        let count = |g: &Genome, cat: &str| g.blueprint.parts.iter().filter(|p| c.parts.get(&p.part).unwrap().category == cat).map(|p| &p.part).cloned().collect::<Vec<_>>();
        for gene in Gene::ALL {
            let mut may = off.clone();
            may.set(gene, true);
            let mut any = false;
            for _ in 0..60 {
                let Some(g) = kit.mutate(&c, &skiff, &may, &mut rng) else { continue };
                any = true;
                let design = g.blueprint.parts != skiff.blueprint.parts;
                assert_eq!(design, may.design(), "{gene:?}");
                assert_eq!(g.behavior != skiff.behavior, may.behavior(), "{gene:?}");
                assert_eq!(g.blueprint.assists != skiff.blueprint.assists, may.assists(), "{gene:?}");
                if matches!(gene, Gene::Layout | Gene::Armour | Gene::Power) {
                    assert_eq!(g.blueprint.parts.len(), skiff.blueprint.parts.len(), "{gene:?} adds or removes nothing");
                }
            }
            assert!(any, "{gene:?} changes something");
        }
        // lab.ron's genes are the arena's, and its first preset is everything.
        assert_eq!(c.lab.genes.iter().map(|g| Gene::of(&g.id)).collect::<Vec<_>>(), Gene::ALL.map(Some));
        assert_eq!(MayChange::only(&c.lab.gene_presets[0].genes), MayChange::default());
        let may = MayChange { fixed: vec!["weapons".into()], ..MayChange::default() };
        let mut guns: Vec<_> = count(&skiff, "weapons");
        guns.sort();
        for _ in 0..200 {
            if let Some(g) = kit.mutate(&c, &skiff, &may, &mut rng) {
                let mut now = count(&g, "weapons");
                now.sort();
                assert_eq!(now, guns, "the guns are kept");
            }
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
            assists: vec![0, 2],
            order: true,
        };
        let skiff = start(&c, &mut kit, "rim_skiff", "skirmisher", &locks);
        let other = start(&c, &mut kit, "rim_skiff", "raider_basic", &Locks::default());
        let held = &kit.roots[0].locks;
        assert!(held.parts.len() > 1 && held.cells.len() == 1 && held.rules.len() == 1 && held.assists.len() == 2);
        let mut rng = Pcg32::new(3, 1);
        let mut g = skiff.clone();
        let slots = 64;
        for k in 0..300 {
            for _ in 0..3 {
                if let Some(m) = kit.mutate(&c, &g, &MayChange::default(), &mut rng) {
                    g = m;
                }
            }
            if let Some(m) = kit.cross_design(&c, &g, &other, &mut rng) {
                g = m;
            }
            if let Some(t) = kit.cross_behavior(&g.behavior, &other.behavior, slots, 0, &mut rng) {
                g.behavior = t;
            }
            if let Some(m) = kit.cross_assists(&g, &other, &mut rng) {
                g = m;
            }
            let dock = kit.open(&c, &g).unwrap();
            assert!(held.design_holds(&dock), "generation {k}");
            let p = Program::parse(&g.behavior).unwrap();
            assert!(held.behavior_holds(&p), "generation {k}: {}", g.behavior);
            let a = Program::parse(&g.assists_text()).unwrap();
            assert!(held.assists_hold(&a), "generation {k}: {}", g.assists_text());
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
        // TIDY: the rules that never decided go, the locked ones stay; one rule stays when none decided.
        let (tidied, dropped) = kit.tidy(&text, 0b00100, 0).unwrap();
        assert_eq!(dropped, vec![1, 3]);
        let rules = Program::parse(&tidied).unwrap().rules;
        assert_eq!(rules, vec![program.rules[0].clone(), program.rules[2].clone(), program.rules[4].clone()]);
        assert_eq!(kit.tidy(&text, u64::MAX, 0).unwrap(), (text.clone(), Vec::new()));
        kit.roots[0].locks = Locked::default();
        let (one, dropped) = kit.tidy(&text, 0, 0).unwrap();
        assert_eq!((Program::parse(&one).unwrap().rules, dropped), (vec![program.rules[0].clone()], vec![1, 2, 3, 4]));
    }

    #[test]
    fn the_symmetry_check_knows_the_content_ships() {
        let c = data::base_content();
        let mut kit = new_kit(&c, true);
        for id in ["rim_skiff", "automaton_raider"] {
            let g = start(&c, &mut kit, id, "raider_basic", &Locks::default());
            let dock = kit.open(&c, &g).unwrap();
            assert!(kit.symmetric(&c, &dock, &kit.roots[g.root as usize].locks), "{id}");
            // The skiff's scanner and the cargo bay across from it have no twin: its start keeps them in place.
            let single: Vec<&str> = unmatched(&c, &dock).into_iter().map(|i| dock.parts()[i].part.as_str()).collect();
            assert_eq!(single, if id == "rim_skiff" { vec!["cargo_bay", "scanner"] } else { vec![] }, "{id}");
        }
        // One part moved off its twin's mirror cell.
        let mut g = start(&c, &mut kit, "rim_skiff", "raider_basic", &Locks::default());
        g.blueprint.parts[0].x -= 1;
        let locks = &kit.roots[g.root as usize].locks;
        assert!(kit.open(&c, &g).is_none_or(|d| !kit.symmetric(&c, &d, locks)));
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
