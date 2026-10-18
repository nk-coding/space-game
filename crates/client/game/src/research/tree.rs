//! The research tree as pure data (no Bevy systems): where every node sits ([`Tree::new`]: one lane per family,
//! columns by requirement depth, connectors from each requirement to the node that needs it), where that puts
//! the cards on the screen ([`Tree::place`]), what each card shows from the career ([`state`], [`chips`]) and the
//! words for an unlock or a refusal ([`unlocked_words`], [`refusal_words`]).
//!
//! The tree is a function of the content alone, so the screen is built once; only the states follow the career.

use bevy::math::{Rect, Vec2};
use data::career::{Refusal, Shortfall};
use data::design::stats::{format_number, with_unit};
use data::{Career, Content, ResearchKind, ResearchNode, Unlock};

/// One node's card in the tree.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    /// The research node (id).
    pub node: String,
    /// The lane (index into [`Tree::lanes`]).
    pub lane: usize,
    /// The row inside its lane (0 unless several of the lane's nodes share a column).
    pub row: usize,
    /// The column: the node's requirement depth (0: needs nothing; else one more than its deepest requirement).
    pub column: usize,
}

/// A lane: one research family with at least one node, in content order.
#[derive(Clone, Debug, PartialEq)]
pub struct Lane {
    pub family: String,
    /// Rows the lane needs (the most of its cards in one column).
    pub rows: usize,
}

/// A connector: card `from` (a requirement) leads to card `to` (the node that needs it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Link {
    pub from: usize,
    pub to: usize,
}

/// The tree: lanes, cards and connectors.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tree {
    pub lanes: Vec<Lane>,
    pub cards: Vec<Card>,
    pub links: Vec<Link>,
    /// Columns in use (the deepest requirement chain plus one).
    pub columns: usize,
}

/// Sizes the screen lays the tree out with (logical px, y down).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    /// The top left of the first column's first lane.
    pub origin: Vec2,
    /// How far a card sits below the top of its row.
    pub inset: f32,
    /// A card's size.
    pub card: Vec2,
    /// From one column's left edge to the next one's.
    pub column_pitch: f32,
    /// One lane row's height (the card, the line under it and the gap).
    pub row_pitch: f32,
}

/// The tree on the screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Placed {
    /// Each card's rectangle, in [`Tree::cards`] order.
    pub cards: Vec<Rect>,
    /// Each lane's top and height.
    pub lanes: Vec<(f32, f32)>,
    /// Each connector as a polyline of horizontal and vertical runs, in [`Tree::links`] order: from the middle of
    /// the requirement's right edge to the middle of the dependent's left edge (an elbow halfway between the
    /// columns when the two sit on different rows).
    pub links: Vec<Vec<Vec2>>,
}

impl Tree {
    /// Lays out every research node of the content: a lane per family that has nodes (families in content order;
    /// a node of a family the content doesn't know gets a lane after them), a column per requirement depth, and
    /// inside a lane the nodes sharing a column one under the other in content order. Requirements the content
    /// doesn't have are left out; a cycle (refused by validation) counts as no requirement where it closes.
    pub fn new(content: &Content) -> Self {
        let nodes: Vec<&ResearchNode> = content.research.iter().collect();
        let index = |id: &str| nodes.iter().position(|n| n.id == id);
        // Requirement depth, memoised; `None` while being computed (a cycle closes there).
        let mut depth: Vec<Option<Option<usize>>> = vec![None; nodes.len()];
        fn depth_of(k: usize, nodes: &[&ResearchNode], index: &dyn Fn(&str) -> Option<usize>, memo: &mut Vec<Option<Option<usize>>>) -> usize {
            match memo[k] {
                Some(Some(d)) => return d,
                Some(None) => return 0,
                None => {}
            }
            memo[k] = Some(None);
            let d = nodes[k].requires.iter().filter_map(|r| index(r)).map(|r| depth_of(r, nodes, index, memo) + 1).max().unwrap_or(0);
            memo[k] = Some(Some(d));
            d
        }
        let columns_of: Vec<usize> = (0..nodes.len()).map(|k| depth_of(k, &nodes, &index, &mut depth)).collect();

        let mut families: Vec<String> = content.families.iter().filter(|f| nodes.iter().any(|n| n.family == f.id)).map(|f| f.id.clone()).collect();
        for n in &nodes {
            if !content.families.contains(&n.family) && !families.contains(&n.family) {
                families.push(n.family.clone());
            }
        }
        let mut lanes: Vec<Lane> = families.into_iter().map(|family| Lane { family, rows: 1 }).collect();
        let mut cards = Vec::with_capacity(nodes.len());
        for (k, n) in nodes.iter().enumerate() {
            let lane = lanes.iter().position(|l| l.family == n.family).unwrap_or_default();
            let column = columns_of[k];
            let row = cards.iter().filter(|c: &&Card| c.lane == lane && c.column == column).count();
            lanes[lane].rows = lanes[lane].rows.max(row + 1);
            cards.push(Card { node: n.id.clone(), lane, row, column });
        }
        let mut links = Vec::new();
        for (to, n) in nodes.iter().enumerate() {
            for r in &n.requires {
                if let Some(from) = index(r)
                    && columns_of[from] < columns_of[to]
                    && !links.contains(&Link { from, to })
                {
                    links.push(Link { from, to });
                }
            }
        }
        let columns = columns_of.iter().max().map_or(0, |d| d + 1);
        Self { lanes, cards, links, columns }
    }

    /// The card of a node.
    pub fn card_of(&self, node: &str) -> Option<usize> {
        self.cards.iter().position(|c| c.node == node)
    }

    /// Rows over all lanes.
    pub fn rows(&self) -> usize {
        self.lanes.iter().map(|l| l.rows).sum()
    }

    /// The card an arrow key goes to from card `from` (`dx`, `dy`: −1, 0 or 1, one of them 0): the nearest one that
    /// way, on the tree's grid of columns and rows (all lanes' rows one under the other). Sideways it keeps the row
    /// when it can, up and down the column; `None` at the edge.
    pub fn step(&self, from: usize, dx: i32, dy: i32) -> Option<usize> {
        let row = |c: &Card| (self.lanes[..c.lane].iter().map(|l| l.rows).sum::<usize>() + c.row) as i32;
        let at = |c: &Card| (c.column as i32, row(c));
        let (x, y) = at(self.cards.get(from)?);
        self.cards
            .iter()
            .enumerate()
            .filter(|&(k, c)| {
                let (cx, cy) = at(c);
                k != from && (cx - x) * dx + (cy - y) * dy > 0
            })
            .min_by_key(|&(k, c)| {
                let (cx, cy) = at(c);
                let (along, across) = if dx != 0 { ((cx - x).abs(), (cy - y).abs()) } else { ((cy - y).abs(), (cx - x).abs()) };
                (across, along, k)
            })
            .map(|(k, _)| k)
    }

    /// Where the cards, lanes and connectors go with these sizes. A card sits at the top of its row.
    pub fn place(&self, m: &Metrics) -> Placed {
        let mut lanes = Vec::with_capacity(self.lanes.len());
        let mut top = m.origin.y;
        for l in &self.lanes {
            let h = l.rows as f32 * m.row_pitch;
            lanes.push((top, h));
            top += h;
        }
        let cards: Vec<Rect> = self
            .cards
            .iter()
            .map(|c| {
                let min = Vec2::new(m.origin.x + c.column as f32 * m.column_pitch, lanes[c.lane].0 + c.row as f32 * m.row_pitch + m.inset);
                Rect::from_corners(min, min + m.card)
            })
            .collect();
        let links = self
            .links
            .iter()
            .map(|l| {
                let (a, b) = (cards[l.from], cards[l.to]);
                let start = Vec2::new(a.max.x, a.center().y);
                let end = Vec2::new(b.min.x, b.center().y);
                if (start.y - end.y).abs() < 0.5 {
                    vec![start, end]
                } else {
                    // Halfway across the gap before the dependent's column.
                    let x = b.min.x - (m.column_pitch - m.card.x) / 2.0;
                    vec![start, Vec2::new(x, start.y), Vec2::new(x, end.y), end]
                }
            })
            .collect();
        Placed { cards, lanes, links }
    }
}

/// What a card shows, from the career (the model's `can_pay` refusals, in its order).
#[derive(Clone, Debug, PartialEq)]
pub enum CardState {
    /// Researched: what it unlocks is in the drydock.
    Done,
    /// Can be paid now: UNLOCK does it.
    Ready,
    /// These requirements (node ids) are not researched yet.
    Locked(Vec<String>),
    /// The requirements are met, the stores aren't: every cost that falls short.
    Short(Vec<Shortfall>),
}

/// The state of `node`'s card for this career.
pub fn state(content: &Content, career: &Career, node: &str) -> CardState {
    match career.can_pay(content, node) {
        Ok(()) => CardState::Ready,
        Err(Refusal::AlreadyResearched(_)) => CardState::Done,
        Err(Refusal::Requires(missing)) => CardState::Locked(missing),
        Err(Refusal::CannotPay(short)) => CardState::Short(short),
        // An unknown node (the tree comes from the same content, so never): nothing to do with it.
        Err(_) => CardState::Locked(Vec::new()),
    }
}

/// The nodes that can be researched now, in content order.
pub fn ready<'c>(content: &'c Content, career: &Career) -> Vec<&'c ResearchNode> {
    content.research.iter().filter(|n| career.can_pay(content, &n.id).is_ok()).collect()
}

/// An amount of research data as the screen shows it: rounded down to a tenth (19.96 is `19.9`, never a `20`
/// that doesn't pay), trailing zeros trimmed.
pub fn amount(x: f32) -> String {
    let x = if x.is_finite() { x.max(0.0) } else { 0.0 };
    format_number((x * 10.0 + 1e-3).floor() / 10.0, 1)
}

/// A family's short label, its id in capitals (`HULL`, `ENERGY`): the lanes' heads and the chips (the content
/// names, `Hull and armour`, are too long for them).
pub fn family_label(id: &str) -> String {
    id.replace('_', " ").to_uppercase()
}

/// One cost of a node as a chip under its card.
#[derive(Clone, Debug, PartialEq)]
pub struct Chip {
    /// A material (drawn with a diamond before the number) rather than research data.
    pub material: bool,
    /// `ENERGY 18` while the store pays it, `HULL 14 / 20` (what is held / the cost) while it doesn't; a
    /// material's number alone (`2`, `1 / 2`).
    pub text: String,
    /// The store pays this cost (gold).
    pub payable: bool,
}

/// The cost chips of `node`: data in the node's order (by family id), then materials.
pub fn chips(career: &Career, node: &ResearchNode) -> Vec<Chip> {
    let mut out = Vec::new();
    for (f, &cost) in &node.cost.data {
        let have = career.data().get(f).copied().unwrap_or(0.0);
        let payable = have + data::career::DATA_EPSILON >= cost;
        let text = if payable { format!("{} {}", family_label(f), amount(cost)) } else { format!("{} {} / {}", family_label(f), amount(have), amount(cost)) };
        out.push(Chip { material: false, text, payable });
    }
    for (m, &cost) in &node.cost.materials {
        let have = career.materials().get(m).copied().unwrap_or(0);
        let payable = have >= cost;
        let text = if payable { cost.to_string() } else { format!("{have} / {cost}") };
        out.push(Chip { material: true, text, payable });
    }
    out
}

/// A node's cost as chips, whatever the stores hold (`ENERGY 18`, a material's `2`): a researched node's detail.
pub fn cost_chips(node: &ResearchNode) -> Vec<Chip> {
    let data = node.cost.data.iter().map(|(f, x)| Chip { material: false, text: format!("{} {}", family_label(f), amount(*x)), payable: true });
    data.chain(node.cost.materials.values().map(|n| Chip { material: true, text: n.to_string(), payable: true })).collect()
}

/// A research kind's tag (GDR-0024).
pub fn kind_words(kind: ResearchKind) -> &'static str {
    match kind {
        ResearchKind::NewPart => "NEW PART",
        ResearchKind::Variant => "VARIANT",
        ResearchKind::SizeClass => "SIZE CLASS",
        ResearchKind::Refinement => "REFINEMENT",
        ResearchKind::Alien => "ALIEN TECH",
        ResearchKind::Facility => "FACILITY",
    }
}

/// What the career has now, for a node's words: the mind's capacity and the jump range (S).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Now {
    pub mind: u32,
    pub range: f32,
}

impl Now {
    pub fn of(content: &Content, career: &Career, mind: u32) -> Now {
        Now { mind, range: career.jump_range(content) }
    }
}

/// A jump range in words, with the content's unit and decimals (`stats.ron`'s `jump_range`): `3.25 S`.
pub fn range_text(content: &Content, s: f32) -> String {
    let (decimals, unit) = content.stats.get("jump_range").map_or((2, "S"), |d| (d.decimals, d.unit.as_str()));
    with_unit(&format_number(s, decimals), unit)
}

/// What a node's unlocks give, in words: `Beam laser in the drydock, unlimited`, `Composite armour in the
/// drydock`, `The cruiser frame in the drydock`, `One more logic core in the mind (runs 3)`, `Evolution opens at
/// home`, `Jumps reach 0.75 S further (3.25 S)`; joined by `; `.
pub fn gives(content: &Content, node: &ResearchNode, now: Now) -> String {
    let (mut capacity, mut range) = (now.mind, now.range);
    node.unlocks
        .iter()
        .map(|u| match u {
            Unlock::Part(p) => format!("{} in the drydock, unlimited", content.parts.get(p).map_or(p.as_str(), |p| p.name.as_str())),
            Unlock::Material(m) => format!("{} armour in the drydock", content.materials.get(m).map_or(m.as_str(), |m| m.name.as_str())),
            Unlock::Frame(f) => format!("The {} frame in the drydock", content.frames.get(f).map_or(f.as_str(), |f| f.name.as_str()).to_lowercase()),
            Unlock::MindCapacity(n) => {
                capacity += n;
                let more = if *n == 1 { "One more logic core".to_string() } else { format!("{n} more logic cores") };
                format!("{more} in the mind (runs {capacity})")
            }
            Unlock::JumpRange(s) => {
                range += s;
                format!("Jumps reach {} further ({})", range_text(content, *s), range_text(content, range))
            }
            Unlock::Facility(f) => format!("{} opens at home", content.facilities.get(f).map_or(f.as_str(), |f| f.name.as_str())),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// The line after an UNLOCK: `BEAM LASER · NOW IN THE DRYDOCK`, `MIND EXPANSION I · YOUR MIND RUNS 3 CORES`,
/// `SIMULATION VAULT · OPEN AT HOME`, `JUMP CALIBRATION I · JUMPS REACH 3.25 S` (`now`: the career after it).
pub fn unlocked_words(content: &Content, node: &ResearchNode, now: Now) -> String {
    let name = node.name.to_uppercase();
    if node.unlocks.iter().any(|u| matches!(u, Unlock::Part(_) | Unlock::Material(_) | Unlock::Frame(_))) {
        format!("{name}  ·  NOW IN THE DRYDOCK")
    } else if node.unlocks.iter().any(|u| matches!(u, Unlock::Facility(_))) {
        format!("{name}  ·  OPEN AT HOME")
    } else if node.unlocks.iter().any(|u| matches!(u, Unlock::JumpRange(_))) {
        format!("{name}  ·  JUMPS REACH {}", range_text(content, now.range).to_uppercase())
    } else {
        format!("{name}  ·  YOUR MIND RUNS {} CORES", now.mind)
    }
}

/// A refusal of the model in words (uppercase): `NEEDS TAMED AUTOMATON CORE FIRST`, `NOT ENOUGH DATA · LOGIC 4 /
/// 10 · PROGRAMMABLE MATTER 1 / 2`, `ALREADY RESEARCHED`.
pub fn refusal_words(content: &Content, r: &Refusal) -> String {
    let node = |id: &str| content.research.get(id).map_or(id.to_string(), |n| n.name.clone()).to_uppercase();
    match r {
        Refusal::AlreadyResearched(_) => "ALREADY RESEARCHED".into(),
        Refusal::Requires(missing) => format!("NEEDS {} FIRST", missing.iter().map(|m| node(m)).collect::<Vec<_>>().join(" AND ")),
        Refusal::CannotPay(short) => {
            let what = if short.iter().any(|s| matches!(s, Shortfall::Data { .. })) { "NOT ENOUGH DATA" } else { "NOT ENOUGH MATERIALS" };
            let parts: Vec<String> = short
                .iter()
                .map(|s| match s {
                    Shortfall::Data { family, have, cost } => format!("{} {} / {}", family_label(family), amount(*have), amount(*cost)),
                    Shortfall::Material { material, have, cost } => {
                        format!("{} {have} / {cost}", content.materials.get(material).map_or(material.as_str(), |m| m.name.as_str()).to_uppercase())
                    }
                })
                .collect();
            format!("{what}  ·  {}", parts.join("  ·  "))
        }
        Refusal::UnknownNode(id) => format!("NO RESEARCH `{id}`"),
        other => format!("{other:?}").to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn career(extra: &str) -> (Content, Career) {
        let content = data::base_content();
        let mut c = Career::from_ron(&format!("(start: \"rim_awakening\", seed: 1, generator: {}{extra})", data::career::GENERATOR)).unwrap();
        c.reconcile(&content);
        (content, c)
    }

    /// The arrow keys move on the grid of columns and rows across lanes: sideways in the row, up and down in the
    /// column, the nearest that way when the row or column has none; nothing past the edge.
    #[test]
    fn arrows_step_to_the_nearest_card_that_way() {
        let card = |node: &str, lane, row, column| Card { node: node.into(), lane, row, column };
        let tree = Tree {
            lanes: vec![Lane { family: "a".into(), rows: 2 }, Lane { family: "b".into(), rows: 1 }],
            cards: vec![card("a0", 0, 0, 0), card("a1", 0, 0, 1), card("a2", 0, 1, 1), card("b0", 1, 0, 0), card("b2", 1, 0, 2)],
            links: Vec::new(),
            columns: 3,
        };
        let step = |from: usize, dx, dy| tree.step(from, dx, dy).map(|k| tree.cards[k].node.as_str());
        assert_eq!(step(0, 1, 0), Some("a1"));
        assert_eq!(step(1, 0, 1), Some("a2"));
        assert_eq!(step(2, 0, 1), Some("b0"), "a tie goes to the earlier card");
        assert_eq!(step(3, 1, 0), Some("b2"));
        assert_eq!(step(4, 0, -1), Some("a2"));
        assert_eq!(step(0, -1, 0), None);
        assert_eq!(step(0, 0, -1), None);
    }

    fn at(tree: &Tree, node: &str) -> (String, usize, usize) {
        let c = &tree.cards[tree.card_of(node).unwrap()];
        (tree.lanes[c.lane].family.clone(), c.row, c.column)
    }

    /// The base pack's nodes: a lane per family with nodes (all ten, content order), three columns (the mind
    /// expansions' chain, the Grown's, the drydock's), connectors along the chains; the simulation vault under the
    /// tamed core and the first jump calibration under the torch drive (both need nothing), the sidegrades under the
    /// parts they vary.
    #[test]
    fn lanes_follow_the_families_and_columns_the_requirement_depth() {
        let content = data::base_content();
        let tree = Tree::new(&content);
        let lanes: Vec<&str> = tree.lanes.iter().map(|l| l.family.as_str()).collect();
        assert_eq!(lanes, ["hull", "power", "energy", "kinetic", "shields", "thermal", "propulsion", "logic", "utility", "grown"]);
        assert_eq!(tree.columns, 3);
        assert_eq!(tree.cards.len(), content.research.len());
        assert_eq!(at(&tree, "composite_plating"), ("hull".into(), 0, 0));
        assert_eq!(at(&tree, "cruiser_drydock"), ("hull".into(), 0, 1));
        assert_eq!(at(&tree, "tamed_core"), ("logic".into(), 0, 0));
        assert_eq!(at(&tree, "mind_expansion_1"), ("logic".into(), 0, 1));
        assert_eq!(at(&tree, "mind_expansion_2"), ("logic".into(), 0, 2));
        assert_eq!(at(&tree, "simulation_vault"), ("logic".into(), 1, 0));
        assert_eq!(at(&tree, "jump_calibration_1"), ("propulsion".into(), 1, 0));
        assert_eq!(at(&tree, "jump_calibration_2"), ("propulsion".into(), 0, 1));
        assert_eq!(at(&tree, "grown_heart"), ("grown".into(), 0, 2));
        assert_eq!(at(&tree, "spore_lining"), ("grown".into(), 1, 2));
        assert_eq!(at(&tree, "reactive_plating"), ("hull".into(), 1, 1));
        assert_eq!(at(&tree, "capital_drydock"), ("hull".into(), 0, 2));
        assert_eq!(at(&tree, "automaton_forge"), ("logic".into(), 1, 1));
        assert_eq!(at(&tree, "long_cannon"), ("kinetic".into(), 0, 1));
        assert_eq!(at(&tree, "burst_laser"), ("energy".into(), 1, 0));
        let link = |a: &str, b: &str| Link { from: tree.card_of(a).unwrap(), to: tree.card_of(b).unwrap() };
        assert_eq!(
            tree.links,
            [
                link("regrowing_tissue", "living_weapons"),
                link("living_weapons", "grown_heart"),
                link("living_weapons", "spore_lining"),
                link("jump_calibration_1", "jump_calibration_2"),
                link("tamed_core", "mind_expansion_1"),
                link("mind_expansion_1", "mind_expansion_2"),
                link("composite_plating", "cruiser_drydock"),
                link("composite_plating", "reactive_plating"),
                link("tamed_core", "automaton_forge"),
                link("cruiser_drydock", "capital_drydock"),
                link("automaton_cannon", "long_cannon")
            ]
        );
        let one_row = ["kinetic", "shields", "thermal", "utility"];
        assert!(tree.lanes.iter().all(|l| l.rows == if one_row.contains(&l.family.as_str()) { 1 } else { 2 }));
    }

    /// Nodes of one lane in one column stack in rows; a requirement in another lane still sets the column; the
    /// deepest requirement wins; unknown requirements are left out.
    #[test]
    fn rows_stack_and_cross_lane_requirements_count() {
        let mut content = data::base_content();
        let mut extra = content.research.get("beam_laser").unwrap().clone();
        extra.id = "beam_focus".into();
        extra.requires = vec!["fusion_reactor".into(), "mind_expansion_1".into(), "nowhere".into()];
        content.research.upsert(extra);
        let mut twin = content.research.get("beam_laser").unwrap().clone();
        twin.id = "beam_twin".into();
        content.research.upsert(twin);
        let tree = Tree::new(&content);
        assert_eq!(at(&tree, "beam_twin"), ("energy".into(), 2, 0), "another root of the lane goes under the others");
        assert_eq!(at(&tree, "beam_focus"), ("energy".into(), 0, 2), "one past the deepest requirement (mind expansion I)");
        let energy = tree.lanes.iter().find(|l| l.family == "energy").unwrap();
        assert_eq!(energy.rows, 3);
        let into = |to: &str| tree.links.iter().filter(|l| l.to == tree.card_of(to).unwrap()).count();
        assert_eq!(into("beam_focus"), 2);
    }

    /// Cards sit in their lane and column; connectors run from the requirement's right edge to the dependent's
    /// left edge, straight on one row, with an elbow halfway across the gap between rows.
    #[test]
    fn connectors_join_the_cards() {
        let mut content = data::base_content();
        let mut extra = content.research.get("beam_laser").unwrap().clone();
        extra.id = "beam_focus".into();
        extra.requires = vec!["fusion_reactor".into()];
        content.research.upsert(extra);
        let tree = Tree::new(&content);
        let m = Metrics { origin: Vec2::new(150.0, 100.0), inset: 0.0, card: Vec2::new(200.0, 40.0), column_pitch: 250.0, row_pitch: 90.0 };
        let placed = tree.place(&m);
        let rect = |n: &str| placed.cards[tree.card_of(n).unwrap()];
        assert_eq!(rect("composite_plating"), Rect::new(150.0, 100.0, 350.0, 140.0));
        assert_eq!(rect("cruiser_drydock").min, Vec2::new(400.0, 100.0));
        assert_eq!(placed.lanes.len(), tree.lanes.len());
        assert_eq!(placed.lanes[1], (280.0, 180.0), "the hull's lane has two rows");
        let link = |a: &str, b: &str| {
            let k = tree.links.iter().position(|l| *l == Link { from: tree.card_of(a).unwrap(), to: tree.card_of(b).unwrap() }).unwrap();
            placed.links[k].clone()
        };
        assert_eq!(link("composite_plating", "cruiser_drydock"), [Vec2::new(350.0, 120.0), Vec2::new(400.0, 120.0)]);
        // Power (lane 1) to energy (lane 2): across, down halfway in the gap, across.
        let elbow = link("fusion_reactor", "beam_focus");
        assert_eq!(elbow, [Vec2::new(350.0, 300.0), Vec2::new(375.0, 300.0), Vec2::new(375.0, 480.0), Vec2::new(400.0, 480.0)]);
    }

    /// Card states follow the model: done, ready, locked by requirements, short of data (with what is held).
    #[test]
    fn card_states_come_from_the_career() {
        let (content, c) = career(", researched: [\"tamed_core\"], data: {\"logic\": 10.0, \"hull\": 6.0, \"energy\": 19.0}, materials: {\"programmable_matter\": 1}");
        assert_eq!(state(&content, &c, "tamed_core"), CardState::Done);
        assert_eq!(state(&content, &c, "beam_laser"), CardState::Ready);
        assert_eq!(state(&content, &c, "cruiser_drydock"), CardState::Locked(vec!["composite_plating".into()]));
        assert_eq!(state(&content, &c, "mind_expansion_1"), CardState::Short(vec![Shortfall::Material { material: "programmable_matter".into(), have: 1, cost: 3 }]));
        assert_eq!(cost_chips(content.research.get("mind_expansion_1").unwrap()), [Chip { material: false, text: "LOGIC 10".into(), payable: true }, Chip { material: true, text: "3".into(), payable: true }]);
        let ready: Vec<&str> = ready(&content, &c).iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ready, ["beam_laser", "simulation_vault", "burst_laser"]);
        let chips_of = |n: &str| chips(&c, content.research.get(n).unwrap());
        assert_eq!(chips_of("composite_plating"), [Chip { material: false, text: "HULL 6 / 8".into(), payable: false }]);
        assert_eq!(chips_of("beam_laser"), [Chip { material: false, text: "ENERGY 10".into(), payable: true }]);
        assert_eq!(
            chips_of("mind_expansion_1"),
            [Chip { material: false, text: "LOGIC 10".into(), payable: true }, Chip { material: true, text: "1 / 3".into(), payable: false }]
        );
    }

    /// Unlocking pays and unlocks at once; the words say so; refusals say why and change nothing.
    #[test]
    fn the_unlock_flow_in_words() {
        let (content, mut c) = career(", data: {\"logic\": 9.5, \"kinetic\": 12.0}, materials: {\"programmable_matter\": 1}");
        let refused = c.research(&content, "mind_expansion_1").unwrap_err();
        assert_eq!(refusal_words(&content, &refused), "NEEDS TAMED AUTOMATON CORE FIRST");
        let n = c.research(&content, "automaton_cannon").unwrap();
        let now = |mind| Now { mind, range: 2.5 };
        assert_eq!(unlocked_words(&content, n, now(2)), "REVERSE-ENGINEERED CANNON  ·  NOW IN THE DRYDOCK");
        assert_eq!(state(&content, &c, "automaton_cannon"), CardState::Done);
        assert!(c.unlocks(&content).has_part("reverse_engineered_cannon"));
        assert!(c.fresh_parts(&content).contains("reverse_engineered_cannon"), "the drydock shows it NEW");
        let n = c.research(&content, "tamed_core").unwrap();
        assert_eq!(n.id, "tamed_core");
        assert!(c.data().get("logic").is_some_and(|x| (x - 6.5).abs() < 1e-4));
        let short = c.research(&content, "mind_expansion_1").unwrap_err();
        assert_eq!(refusal_words(&content, &short), "NOT ENOUGH DATA  ·  LOGIC 6.5 / 10  ·  PROGRAMMABLE MATTER 0 / 3");
        let me = content.research.get("mind_expansion_1").unwrap();
        assert_eq!(unlocked_words(&content, me, now(3)), "MIND EXPANSION I  ·  YOUR MIND RUNS 3 CORES");
        assert_eq!(gives(&content, me, now(2)), "One more logic core in the mind (runs 3)");
        assert_eq!(gives(&content, content.research.get("beam_laser").unwrap(), now(2)), "Beam laser in the drydock, unlimited");
        assert_eq!(gives(&content, content.research.get("cruiser_drydock").unwrap(), now(2)), "The cruiser frame in the drydock");
        let vault = content.research.get("simulation_vault").unwrap();
        assert_eq!((gives(&content, vault, now(2)).as_str(), unlocked_words(&content, vault, now(2)).as_str()), ("Evolution opens at home", "SIMULATION VAULT  ·  OPEN AT HOME"));
        // GDR-0093: two propulsion nodes take the jump range from 2.5 S to 4 S.
        let jump = content.research.get("jump_calibration_1").unwrap();
        assert_eq!(gives(&content, jump, now(2)), "Jumps reach 0.75 S further (3.25 S)");
        assert_eq!(unlocked_words(&content, jump, Now { mind: 2, range: 3.25 }), "JUMP CALIBRATION I  ·  JUMPS REACH 3.25 S");
        assert_eq!(refusal_words(&content, &c.research(&content, "tamed_core").unwrap_err()), "ALREADY RESEARCHED");
    }

    /// Amounts never round up to a cost they don't pay.
    #[test]
    fn amounts_round_down_to_tenths() {
        assert_eq!(amount(19.96), "19.9");
        assert_eq!(amount(20.0), "20");
        assert_eq!(amount(0.3), "0.3");
        assert_eq!(amount(-2.0), "0");
        assert_eq!(amount(f32::NAN), "0");
    }
}
