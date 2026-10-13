//! The research tree as pure data (no Bevy systems): where every node sits ([`Tree::new`]: one lane per family,
//! columns by requirement depth, connectors from each requirement to the node that needs it), where that puts
//! the cards on the screen ([`Tree::place`]), what each card shows from the career ([`state`], [`chips`]) and the
//! words for an unlock or a refusal ([`unlocked_words`], [`refusal_words`]).
//!
//! The tree is a function of the content alone, so the screen is built once; only the states follow the career.

use bevy::math::{Rect, Vec2};
use data::career::{Refusal, Shortfall};
use data::stats::format_number;
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

/// A research kind's tag (GDR-0024).
pub fn kind_words(kind: ResearchKind) -> &'static str {
    match kind {
        ResearchKind::NewPart => "NEW PART",
        ResearchKind::Variant => "VARIANT",
        ResearchKind::SizeClass => "SIZE CLASS",
        ResearchKind::Refinement => "REFINEMENT",
        ResearchKind::Alien => "ALIEN TECH",
    }
}

/// What a node's unlocks give, in words: `Beam laser in the drydock, unlimited`, `Composite armour in the
/// drydock`, `The cruiser frame in the drydock`, `One more logic core in the mind (runs 3)`; joined by `; `.
/// `capacity` is the mind's capacity now.
pub fn gives(content: &Content, node: &ResearchNode, capacity: u32) -> String {
    let mut capacity = capacity;
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
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// The line after an UNLOCK: `BEAM LASER · NOW IN THE DRYDOCK`, `MIND EXPANSION I · YOUR MIND RUNS 3 CORES`
/// (`capacity` is the mind's capacity after it).
pub fn unlocked_words(node: &ResearchNode, capacity: u32) -> String {
    let name = node.name.to_uppercase();
    if node.unlocks.iter().any(|u| !matches!(u, Unlock::MindCapacity(_))) {
        format!("{name}  ·  NOW IN THE DRYDOCK")
    } else {
        format!("{name}  ·  YOUR MIND RUNS {capacity} CORES")
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

    fn at(tree: &Tree, node: &str) -> (String, usize, usize) {
        let c = &tree.cards[tree.card_of(node).unwrap()];
        (tree.lanes[c.lane].family.clone(), c.row, c.column)
    }

    /// The base pack's nine nodes: six lanes (families with nodes, content order), three columns (the mind
    /// expansions' chain), connectors along the chains.
    #[test]
    fn lanes_follow_the_families_and_columns_the_requirement_depth() {
        let content = data::base_content();
        let tree = Tree::new(&content);
        let lanes: Vec<&str> = tree.lanes.iter().map(|l| l.family.as_str()).collect();
        assert_eq!(lanes, ["hull", "power", "energy", "kinetic", "propulsion", "logic"]);
        assert_eq!(tree.columns, 3);
        assert_eq!(tree.cards.len(), content.research.len());
        assert_eq!(at(&tree, "composite_plating"), ("hull".into(), 0, 0));
        assert_eq!(at(&tree, "cruiser_drydock"), ("hull".into(), 0, 1));
        assert_eq!(at(&tree, "tamed_core"), ("logic".into(), 0, 0));
        assert_eq!(at(&tree, "mind_expansion_1"), ("logic".into(), 0, 1));
        assert_eq!(at(&tree, "mind_expansion_2"), ("logic".into(), 0, 2));
        let link = |a: &str, b: &str| Link { from: tree.card_of(a).unwrap(), to: tree.card_of(b).unwrap() };
        assert_eq!(tree.links, [link("tamed_core", "mind_expansion_1"), link("mind_expansion_1", "mind_expansion_2"), link("composite_plating", "cruiser_drydock")]);
        assert!(tree.lanes.iter().all(|l| l.rows == 1));
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
        assert_eq!(at(&tree, "beam_twin"), ("energy".into(), 1, 0), "a second root of the lane goes under the first");
        assert_eq!(at(&tree, "beam_focus"), ("energy".into(), 0, 2), "one past the deepest requirement (mind expansion I)");
        let energy = tree.lanes.iter().find(|l| l.family == "energy").unwrap();
        assert_eq!(energy.rows, 2);
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
        assert_eq!(placed.lanes[1], (190.0, 90.0));
        let link = |a: &str, b: &str| {
            let k = tree.links.iter().position(|l| *l == Link { from: tree.card_of(a).unwrap(), to: tree.card_of(b).unwrap() }).unwrap();
            placed.links[k].clone()
        };
        assert_eq!(link("composite_plating", "cruiser_drydock"), [Vec2::new(350.0, 120.0), Vec2::new(400.0, 120.0)]);
        // Power (lane 1) to energy (lane 2): across, down halfway in the gap, across.
        let elbow = link("fusion_reactor", "beam_focus");
        assert_eq!(elbow, [Vec2::new(350.0, 210.0), Vec2::new(375.0, 210.0), Vec2::new(375.0, 300.0), Vec2::new(400.0, 300.0)]);
    }

    /// Card states follow the model: done, ready, locked by requirements, short of data (with what is held).
    #[test]
    fn card_states_come_from_the_career() {
        let (content, c) = career(", researched: [\"tamed_core\"], data: {\"logic\": 10.0, \"hull\": 14.0, \"energy\": 19.0}, materials: {\"programmable_matter\": 1}");
        assert_eq!(state(&content, &c, "tamed_core"), CardState::Done);
        assert_eq!(state(&content, &c, "beam_laser"), CardState::Ready);
        assert_eq!(state(&content, &c, "cruiser_drydock"), CardState::Locked(vec!["composite_plating".into()]));
        assert_eq!(state(&content, &c, "mind_expansion_1"), CardState::Short(vec![Shortfall::Material { material: "programmable_matter".into(), have: 1, cost: 2 }]));
        let ready: Vec<&str> = ready(&content, &c).iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ready, ["beam_laser"]);
        let chips_of = |n: &str| chips(&c, content.research.get(n).unwrap());
        assert_eq!(chips_of("composite_plating"), [Chip { material: false, text: "HULL 14 / 20".into(), payable: false }]);
        assert_eq!(chips_of("beam_laser"), [Chip { material: false, text: "ENERGY 18".into(), payable: true }]);
        assert_eq!(
            chips_of("mind_expansion_1"),
            [Chip { material: false, text: "LOGIC 10".into(), payable: true }, Chip { material: true, text: "1 / 2".into(), payable: false }]
        );
    }

    /// Unlocking pays and unlocks at once; the words say so; refusals say why and change nothing.
    #[test]
    fn the_unlock_flow_in_words() {
        let (content, mut c) = career(", data: {\"logic\": 9.5, \"kinetic\": 12.0}, materials: {\"programmable_matter\": 1}");
        let refused = c.research(&content, "mind_expansion_1").unwrap_err();
        assert_eq!(refusal_words(&content, &refused), "NEEDS TAMED AUTOMATON CORE FIRST");
        let n = c.research(&content, "automaton_cannon").unwrap();
        assert_eq!(unlocked_words(n, 2), "REVERSE-ENGINEERED CANNON  ·  NOW IN THE DRYDOCK");
        assert_eq!(state(&content, &c, "automaton_cannon"), CardState::Done);
        assert!(c.unlocks(&content).has_part("automaton_cannon"));
        assert!(c.fresh_parts(&content).contains("automaton_cannon"), "the drydock shows it NEW");
        let n = c.research(&content, "tamed_core").unwrap();
        assert_eq!(n.id, "tamed_core");
        assert!(c.data().get("logic").is_some_and(|x| (x - 1.5).abs() < 1e-4));
        let short = c.research(&content, "mind_expansion_1").unwrap_err();
        assert_eq!(refusal_words(&content, &short), "NOT ENOUGH DATA  ·  LOGIC 1.5 / 10  ·  PROGRAMMABLE MATTER 0 / 2");
        let me = content.research.get("mind_expansion_1").unwrap();
        assert_eq!(unlocked_words(me, 3), "MIND EXPANSION I  ·  YOUR MIND RUNS 3 CORES");
        assert_eq!(gives(&content, me, 2), "One more logic core in the mind (runs 3)");
        assert_eq!(gives(&content, content.research.get("beam_laser").unwrap(), 2), "Beam laser in the drydock, unlimited");
        assert_eq!(gives(&content, content.research.get("cruiser_drydock").unwrap(), 2), "The cruiser frame in the drydock");
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
