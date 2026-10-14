//! Salvage v1 (GDR-0063, `docs/sim.md` "Salvage"): cargo, collecting, fragments and wrecks spawned as wrecks.
//!
//! * **Items** ([`Item`]): a whole part by content id (hull parts carry their armour material), or the fragment
//!   a destroyed part leaves. The content's `salvage` blocks decide what counts (compiled into the design,
//!   `PartSpec::salvageable` and `PartSpec::fragment`): only parts with a block (hull parts: their material's)
//!   are collected, and only parts whose block names a `fragment` leave one. A part takes its footprint cells in cargo units
//!   (`PartSpec::cargo_units`), a fragment one.
//! * **Cargo** ([`Cargo`], `Ship::cargo`): items and counts in item order, and the units in use. A vessel's hold
//!   is the summed capacity of the alive cargo parts of its controlling piece (`Piece::cargo_capacity`); when it
//!   drops below the load, the lowest-value items go and spill as loose items at the lost hold (GDR-0076). A lost
//!   vessel's ship keeps its cargo: its wreck carries it.
//! * **Value** (lead default): build points per cargo unit (hull parts with their material's factor); a
//!   fragment is worth its part's build points. The sim does not read the career's research values.
//! * **Collecting**: every `SalvageTuning::interval` ticks each running vessel of an owner that collects takes
//!   one item that fits its free hold, from salvage within reach of its hull that moves no faster than
//!   `SalvageTuning::max_speed` relative to it (fly slowly over a wreck, GDR-0063): the nearest loose item (a
//!   fragment or spilled cargo), else from the nearest salvage piece (pieces in order of distance, the first one with
//!   something that fits): its wreck's cargo first (the best item), then its best salvageable part (ties: the
//!   lowest part index). A stripped part leaves through the destroy path without the destruction
//!   (`Ship::remove_part`: no `PartDestroyed`, no fragment); the wreck may split as in any breakage. `SimEvent::Collected` tells the client.
//! * **Fragments** ([`Loose`]): a part whose salvage block names a fragment, destroyed by damage, leaves one at its
//!   centroid with its velocity: a point the sim moves every tick (no body, no collisions) until it is collected
//!   or its `SalvageTuning::fragment_ticks` are over (by default never: it lasts as long as the sim, GDR-0079).
//!   Cargo a shrinking hold spills drifts the same way.
//! * **Wrecks spawned as wrecks** ([`Sim::spawn_wreck`]): burned out (no vessel ever forms from their cores),
//!   a share of their parts already gone (rolled from the wreck's own seed).
//!
//! Everything here is in the state hash. Cost: one bounding-circle test per salvage piece and loose item per
//! collecting vessel with free hold every `interval` ticks; for those that pass, the convex colliders are compared
//! pair by pair, skipping pairs whose bounding boxes are farther apart than the reach (or the best distance so
//! far); loose items are moved with one add per tick. No allocation per tick once the buffers are warm.

use std::collections::BTreeMap;
use std::f32::consts::TAU;
use std::sync::Arc;

use crate::DT;
use crate::ship::design::{PartIdx, ShipDesign};
use crate::events::SimEvent;
use crate::hash::{StateHash, StateHasher};
use crate::ids::{PieceId, ShipId};
use crate::math::{Rot, Vec2, sqrt};
use crate::rng::{Pcg32, Stream, splitmix64};
use crate::ship::{Body, Piece, Ship};
use crate::ship::vessel::{Owner, VesselState};
use crate::world::Sim;

/// The PCG sequence of a wreck's own generator ([`Sim::spawn_wreck`]: seeded with `splitmix64(seed)`). Part of
/// the replay format, like the stream ids.
const WRECK_SEQUENCE: u64 = Stream::Encounter as u64;

/// Something a hold carries. Ordered (cargo lists and ties follow this order).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Item {
    /// A whole part (content part id); hull parts carry their armour material.
    Part {
        part: String,
        material: Option<String>,
    },
    /// What a destroyed part whose salvage block names a `fragment` leaves (content part id of that part).
    Fragment { part: String },
}

impl Item {
    /// The content part id behind the item.
    pub fn part(&self) -> &str {
        match self {
            Item::Part { part, .. } | Item::Fragment { part } => part,
        }
    }
}

impl StateHash for Item {
    fn hash_state(&self, h: &mut StateHasher) {
        match self {
            Item::Part { part, material } => {
                h.u8(0);
                h.str(part);
                match material {
                    Some(m) => {
                        h.u8(1);
                        h.str(m);
                    }
                    None => h.u8(0),
                }
            }
            Item::Fragment { part } => {
                h.u8(1);
                h.str(part);
            }
        }
    }
}

/// How many of one item a hold carries, the cargo units each takes and its value per unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stack {
    pub count: u32,
    pub units: u32,
    /// Value per cargo unit (build points per unit, see the module docs): the best goes first when stripping a
    /// hold, the worst first when a hold shrinks.
    pub value: f32,
}

/// A ship's hold: items in item order, and the cargo units in use.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cargo {
    items: BTreeMap<Item, Stack>,
    used: u32,
}

impl Cargo {
    /// Cargo units in use.
    pub fn used(&self) -> u32 {
        self.used
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Items in item order.
    pub fn items(&self) -> impl Iterator<Item = (&Item, &Stack)> {
        self.items.iter()
    }

    /// How many of `item` the hold carries.
    pub fn count(&self, item: &Item) -> u32 {
        self.items.get(item).map_or(0, |s| s.count)
    }

    pub(crate) fn add(&mut self, item: Item, units: u32, value: f32) {
        self.items
            .entry(item)
            .or_insert(Stack {
                count: 0,
                units,
                value,
            })
            .count += 1;
        self.used += units;
    }

    /// The most valuable item that fits into `free` units (ties: the first in item order).
    fn best(&self, free: u32) -> Option<&Item> {
        let mut best: Option<(&Item, f32)> = None;
        for (item, s) in &self.items {
            if s.units <= free && best.is_none_or(|(_, v)| s.value > v) {
                best = Some((item, s.value));
            }
        }
        best.map(|(i, _)| i)
    }

    /// Takes one of `item` out: its units and value.
    fn take(&mut self, item: &Item) -> Option<(u32, f32)> {
        let s = self.items.get_mut(item)?;
        let got = (s.units, s.value);
        s.count -= 1;
        if s.count == 0 {
            self.items.remove(item);
        }
        self.used -= got.0;
        Some(got)
    }

    /// Drops the lowest-value items (ties: the last in item order), one at a time, until the load fits
    /// `capacity`. Returns what went (its stack: how many went, their units and value), in the order it went.
    pub(crate) fn shed(&mut self, capacity: u32) -> Vec<(Item, Stack)> {
        let mut gone: Vec<(Item, Stack)> = Vec::new();
        while self.used > capacity {
            let mut worst: Option<(&Item, f32)> = None;
            for (item, s) in &self.items {
                if worst.is_none_or(|(_, v)| s.value <= v) {
                    worst = Some((item, s.value));
                }
            }
            let Some((item, _)) = worst else { break };
            let item = item.clone();
            let Some((units, value)) = self.take(&item) else { break };
            match gone.iter_mut().find(|(i, _)| *i == item) {
                Some((_, s)) => s.count += 1,
                None => gone.push((item, Stack { count: 1, units, value })),
            }
        }
        gone
    }
}

impl StateHash for Cargo {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.used);
        h.u32(self.items.len() as u32);
        for (item, s) in &self.items {
            item.hash_state(h);
            h.u32(s.count);
            h.u32(s.units);
            h.f32(s.value);
        }
    }
}

/// A loose item's id: monotonic, never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LooseId(pub u32);

/// An item drifting in space (a fragment, or cargo a shrinking hold spilled): a point the sim moves every tick, no
/// body, no collisions.
#[derive(Clone, Debug, PartialEq)]
pub struct Loose {
    pub id: LooseId,
    pub item: Item,
    pub units: u32,
    /// Value per unit (as in [`Stack`]).
    pub value: f32,
    pub pos: Vec2,
    /// Position at the end of the previous tick (render at `prev_pos + (pos − prev_pos) × alpha`).
    pub prev_pos: Vec2,
    pub vel: Vec2,
    /// The tick it is gone at, if nobody collects it (`None`: it stays).
    pub expires: Option<u64>,
}

impl StateHash for Loose {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.id.0);
        self.item.hash_state(h);
        h.u32(self.units);
        h.f32(self.value);
        h.vec2(self.pos);
        h.vec2(self.prev_pos);
        h.vec2(self.vel);
        h.bool(self.expires.is_some());
        h.u64(self.expires.unwrap_or(0));
    }
}

/// Where a collected item came from (`SimEvent::Collected`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Salvaged {
    /// A part of a salvage piece; it left the piece (breakage events follow in the same tick).
    Part {
        ship: ShipId,
        piece: PieceId,
        part: PartIdx,
    },
    /// The cargo of a wreck (a ship that is no vessel).
    Cargo { ship: ShipId },
    /// A loose item.
    Loose(LooseId),
}

/// The sim's salvage state: loose items and the collect pass's buffer.
#[derive(Clone, Debug, Default)]
pub(crate) struct SalvageState {
    /// Ascending id.
    loose: Vec<Loose>,
    next_loose: u32,
    /// Salvage pieces within reach of one collector: `(hull distance, ship index, piece index)`.
    near: Vec<(f32, usize, usize)>,
    /// Pieces stripped earlier in this pass `(ship index, piece index)`: their colliders are stale until the
    /// breakage is resolved at the end of the pass, so later collectors leave them for the next pass.
    stripped: Vec<(usize, usize)>,
    /// The bounding box of each of one collector's colliders (its frame).
    boxes: Vec<Aabb>,
}

impl StateHash for SalvageState {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.next_loose);
        h.u32(self.loose.len() as u32);
        for l in &self.loose {
            l.hash_state(h);
        }
    }
}

/// What one collector takes this pass.
enum Pick {
    /// `SalvageState::loose[i]`.
    Loose(usize),
    /// One `item` of ship `j`'s cargo, reached through its piece `q`.
    Cargo { j: usize, q: usize, item: Item },
    /// Part `part` of piece `q` of ship `j`.
    Part { j: usize, q: usize, part: PartIdx },
}

impl Ship {
    /// What the ship carries (collected while it was a vessel; a wreck keeps it).
    pub fn cargo(&self) -> &Cargo {
        &self.cargo
    }

    /// Cargo units the hold takes: the alive cargo parts of the controlling piece (0 for salvage).
    pub fn cargo_capacity(&self) -> u32 {
        self.control
            .and_then(|c| self.piece(c))
            .map_or(0, |p| p.cargo_capacity)
    }

    /// Spawned as a wreck (`Sim::spawn_wreck`): its logic cores never form a vessel.
    pub fn is_burned_out(&self) -> bool {
        self.burned_out
    }

    /// Part `p` as a salvage item: the item, its cargo units and its value per unit.
    fn part_item(&self, p: PartIdx) -> (Item, u32, f32) {
        let spec = &self.design.parts[p as usize];
        let item = Item::Part {
            part: spec.id.clone(),
            material: spec.material.clone(),
        };
        (
            item,
            spec.cargo_units,
            spec.build_points / spec.cargo_units as f32,
        )
    }

    /// The best alive salvageable part of piece `q` that fits into `free` units: highest value per unit, ties the
    /// lowest part index.
    fn best_part(&self, q: usize, free: u32) -> Option<PartIdx> {
        let mut best: Option<(PartIdx, f32)> = None;
        for &p in &self.pieces[q].parts {
            let spec = &self.design.parts[p as usize];
            if !self.parts[p as usize].alive || !spec.salvageable || spec.cargo_units > free {
                continue;
            }
            let value = spec.build_points / spec.cargo_units as f32;
            if best.is_none_or(|(_, v)| value > v) {
                best = Some((p, value));
            }
        }
        best.map(|(p, _)| p)
    }
}

/// Is `v` inside (or on) the counter-clockwise convex polygon `poly`?
fn contains(poly: &[Vec2], v: Vec2) -> bool {
    let n = poly.len();
    (0..n).all(|i| (poly[(i + 1) % n] - poly[i]).cross(v - poly[i]) >= 0.0)
}

/// Squared distance from `v` to the outline of `poly`.
fn outline_d2(poly: &[Vec2], v: Vec2) -> f32 {
    let n = poly.len();
    let mut best = f32::INFINITY;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let d = b - a;
        let len2 = d.dot(d);
        let t = if len2 > 0.0 {
            ((v - a).dot(d) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        best = best.min((v - (a + d * t)).length_squared());
    }
    best
}

/// Distance between two convex polygons in one frame: 0 when a corner of one lies in the other, else the
/// nearest corner-to-edge distance (exact for polygons apart; two that cross without a corner inside each other,
/// which only overlapping pieces can, count by their nearest corners).
fn polygon_distance(p: &[Vec2], q: &[Vec2]) -> f32 {
    if q.iter().any(|&v| contains(p, v)) || p.iter().any(|&v| contains(q, v)) {
        return 0.0;
    }
    let mut d2 = f32::INFINITY;
    for &v in q {
        d2 = d2.min(outline_d2(p, v));
    }
    for &v in p {
        d2 = d2.min(outline_d2(q, v));
    }
    sqrt(d2)
}

/// An axis-aligned box.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Aabb {
    min: Vec2,
    max: Vec2,
}

impl Aabb {
    fn of(points: &[Vec2]) -> Aabb {
        let mut b = Aabb {
            min: points[0],
            max: points[0],
        };
        for &v in &points[1..] {
            b.min = Vec2::new(b.min.x.min(v.x), b.min.y.min(v.y));
            b.max = Vec2::new(b.max.x.max(v.x), b.max.y.max(v.y));
        }
        b
    }

    fn union(self, o: Aabb) -> Aabb {
        Aabb::of(&[self.min, self.max, o.min, o.max])
    }

    /// Squared gap between two boxes (0 when they touch or overlap): a lower bound of the squared distance
    /// between anything inside them.
    fn gap2(&self, o: &Aabb) -> f32 {
        let dx = (o.min.x - self.max.x).max(self.min.x - o.max.x).max(0.0);
        let dy = (o.min.y - self.max.y).max(self.min.y - o.max.y).max(0.0);
        dx * dx + dy * dy
    }
}

/// A collector's hull for one pass: its piece, rotation, and the box of each collider and of all (its frame).
struct Hull<'a> {
    piece: &'a Piece,
    rot: Rot,
    boxes: &'a [Aabb],
    all: Aabb,
}

impl Hull<'_> {
    /// World point → the collector's frame.
    fn local(&self, world: Vec2) -> Vec2 {
        self.rot.apply_inverse(world - self.piece.body.pos)
    }

    /// Distance from piece `b`'s hull to this one, if it is at most `reach` (0 when they overlap). Collider pairs
    /// whose boxes are farther apart than the best distance so far (at first `reach`) are skipped.
    fn piece_distance(&self, b: &Piece, reach: f32) -> Option<f32> {
        let rb = Rot::from_angle(b.body.angle);
        let mut found = None;
        let mut lim = reach;
        let mut q = [Vec2::ZERO; 4];
        for cb in &b.colliders {
            let pts = cb.points();
            for (i, &v) in pts.iter().enumerate() {
                q[i] = self.local(b.body.pos + rb.apply(v));
            }
            let q = &q[..pts.len()];
            let qb = Aabb::of(q);
            if self.all.gap2(&qb) > lim * lim {
                continue;
            }
            for (ca, ab) in self.piece.colliders.iter().zip(self.boxes) {
                if ab.gap2(&qb) > lim * lim {
                    continue;
                }
                let d = polygon_distance(ca.points(), q);
                if d <= lim {
                    lim = d;
                    found = Some(d);
                    if d == 0.0 {
                        return found;
                    }
                }
            }
        }
        found
    }

    /// Distance from a world point to this hull, if it is at most `reach` (0 inside).
    fn point_distance(&self, point: Vec2, reach: f32) -> Option<f32> {
        let local = self.local(point);
        let pb = Aabb::of(&[local]);
        if self.all.gap2(&pb) > reach * reach {
            return None;
        }
        let mut d2 = f32::INFINITY;
        for (c, b) in self.piece.colliders.iter().zip(self.boxes) {
            if b.gap2(&pb) > reach * reach {
                continue;
            }
            if contains(c.points(), local) {
                return Some(0.0);
            }
            d2 = d2.min(outline_d2(c.points(), local));
        }
        let d = sqrt(d2);
        (d <= reach).then_some(d)
    }
}

/// Bounding circles about the frame origins within `reach` of each other?
fn near(a: &Piece, b_pos: Vec2, b_radius: f32, reach: f32) -> bool {
    let lim = a.radius + b_radius + reach;
    (b_pos - a.body.pos).length_squared() <= lim * lim
}

/// What collector `k` (a vessel with `free` units) takes this pass, if anything: from salvage within `reach` of its
/// hull and at most `max_speed` relative to it.
fn pick(
    ships: &[Ship],
    st: &mut SalvageState,
    k: usize,
    free: u32,
    reach: f32,
    max_speed: f32,
) -> Option<Pick> {
    let c = &ships[k];
    let cp = c.piece(c.control?)?;
    if cp.colliders.is_empty() {
        return None;
    }
    let v = cp.body.linvel;
    let slow = |u: Vec2| (u - v).length_squared() <= max_speed * max_speed;
    st.boxes.clear();
    st.boxes
        .extend(cp.colliders.iter().map(|c| Aabb::of(c.points())));
    let all = st.boxes[1..].iter().fold(st.boxes[0], |a, &b| a.union(b));
    let hull = Hull {
        piece: cp,
        rot: Rot::from_angle(cp.body.angle),
        boxes: &st.boxes,
        all,
    };

    // Loose items first: the nearest that fits (ties: the lower id).
    let mut best: Option<(f32, usize)> = None;
    for (i, l) in st.loose.iter().enumerate() {
        if l.units > free || !slow(l.vel) || !near(cp, l.pos, 0.0, reach) {
            continue;
        }
        if let Some(d) = hull.point_distance(l.pos, reach)
            && best.is_none_or(|(b, _)| d < b)
        {
            best = Some((d, i));
        }
    }
    if let Some((_, i)) = best {
        return Some(Pick::Loose(i));
    }

    // Salvage pieces within reach, nearest first (ties: ship, then piece order).
    st.near.clear();
    for (j, s) in ships.iter().enumerate() {
        for (q, p) in s.pieces.iter().enumerate() {
            if p.controlled
                || !slow(p.body.linvel)
                || !near(cp, p.body.pos, p.radius, reach)
                || st.stripped.contains(&(j, q))
            {
                continue;
            }
            if let Some(d) = hull.piece_distance(p, reach) {
                st.near.push((d, j, q));
            }
        }
    }
    st.near
        .sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then((a.1, a.2).cmp(&(b.1, b.2))));
    for &(_, j, q) in &st.near {
        let s = &ships[j];
        // A wreck's own cargo goes before its parts.
        if !s.is_vessel()
            && let Some(item) = s.cargo.best(free)
        {
            return Some(Pick::Cargo {
                j,
                q,
                item: item.clone(),
            });
        }
        if let Some(part) = s.best_part(q, free) {
            return Some(Pick::Part { j, q, part });
        }
    }
    None
}

impl Sim {
    /// Loose items (fragments and spilled cargo), ascending id. Render each at `prev_pos + (pos − prev_pos) × alpha`.
    pub fn loose(&self) -> &[Loose] {
        &self.salvage.loose
    }

    /// Spawns a ship as a wreck (a derelict): burned out, so no vessel ever forms from its logic cores, with
    /// `damage` (0..1) of its parts already gone, chosen from `seed` alone (a generator of its own, so the same
    /// seed gives the same wreck whenever and wherever it spawns; the voyage plan rolls one per wreck) and removed
    /// without destruction (no `PartDestroyed`, no fragments); then breakage splits it into pieces as usual
    /// (`Split` / `PieceChanged` / `PieceDestroyed` follow `ShipSpawned`); only the wreck's breakage is resolved.
    /// `owner` is kept for later salvage rights (`Ship::owner`); the wreck counts for nobody (`Ship::team` is
    /// `None`). NaN or infinite numbers in `body` count as 0. Used by the voyage director.
    pub fn spawn_wreck(
        &mut self,
        design: Arc<ShipDesign>,
        owner: Owner,
        body: Body,
        damage: f32,
        seed: u64,
    ) -> ShipId {
        let k = self.new_ship(design, owner, body);
        let ship = &mut self.ships[k];
        ship.burned_out = true;
        ship.control = None;
        for p in &mut ship.pieces {
            p.controlled = false;
            p.active = false;
        }
        let n = ship.parts.len();
        // A NaN share casts to 0 parts.
        let count = ((damage.clamp(0.0, 1.0) * n as f32) as usize).min(n);
        // A partial Fisher–Yates shuffle picks `count` distinct parts.
        let mut order: Vec<u32> = (0..n as u32).collect();
        let mut rng = Pcg32::new(splitmix64(seed), WRECK_SEQUENCE);
        for i in 0..count {
            let j = i + rng.below((n - i) as u32) as usize;
            order.swap(i, j);
        }
        for &p in &order[..count] {
            ship.remove_part(p as usize);
        }
        self.resolve_ship(k);
        self.update_minds();
        self.ships[k].id
    }

    /// Fragments for ship `k`'s parts that leave one (`PartSpec::fragment`) destroyed by damage since its last
    /// breakage resolution (call before it): at each part's centroid, with the velocity of that point of its piece.
    pub(crate) fn spawn_fragments(&mut self, k: usize) {
        let expires = self.loose_expiry();
        let s = &self.ships[k];
        let st = &mut self.salvage;
        for &p in &s.pending_destroyed {
            let spec = &s.design.parts[p as usize];
            if !spec.fragment {
                continue;
            }
            let Some((pos, vel)) = s.part_world(p) else {
                continue;
            };
            st.loose.push(Loose {
                id: LooseId(st.next_loose),
                item: Item::Fragment {
                    part: spec.id.clone(),
                },
                units: 1,
                value: spec.build_points,
                pos,
                prev_pos: pos,
                vel,
                expires,
            });
            st.next_loose += 1;
        }
    }

    /// Ship `k`'s hold after its breakage events `from..`: a vessel whose hold shrank below its load drops its
    /// lowest-value items (`SimEvent::CargoLost` for each kind), and each one spills as a loose item (GDR-0076) at
    /// the lost hold (the first cargo part destroyed in these events, else the controlling piece's centre of mass)
    /// with the velocity of that point plus a push of `SalvageTuning::spill_speed` × [0.5, 1) in a direction
    /// rolled on `Stream::Debris` (two draws per item).
    pub(crate) fn shed_cargo(&mut self, k: usize, from: usize) {
        let s = &self.ships[k];
        let capacity = s.cargo_capacity();
        if !s.is_vessel() || s.cargo.used <= capacity {
            return;
        }
        let Some(cp) = s.control.and_then(|c| s.piece(c)) else {
            return;
        };
        let lost_hold = self.events[from..].iter().find_map(|e| match *e {
            SimEvent::PartDestroyed { ship, part, .. }
                if ship == s.id && s.design.parts[part as usize].cargo > 0.0 =>
            {
                s.part_world(part)
            }
            _ => None,
        });
        let (at, vel) = lost_hold.unwrap_or((cp.com_world(), cp.body.linvel));
        let ship = s.id;
        let gone = self.ships[k].cargo.shed(capacity);
        let t = self.config().salvage;
        let expires = self.loose_expiry();
        for (item, stack) in gone {
            for _ in 0..stack.count {
                let rng = self.rng(Stream::Debris);
                let dir = Rot::from_angle(rng.range_f32(0.0, TAU)).apply(Vec2::X);
                let push = dir * (t.spill_speed * rng.range_f32(0.5, 1.0));
                let st = &mut self.salvage;
                st.loose.push(Loose {
                    id: LooseId(st.next_loose),
                    item: item.clone(),
                    units: stack.units,
                    value: stack.value,
                    pos: at,
                    prev_pos: at,
                    vel: vel + push,
                    expires,
                });
                st.next_loose += 1;
            }
            self.events.push(SimEvent::CargoLost {
                ship,
                item,
                count: stack.count,
            });
        }
    }

    /// When a loose item made now is gone (`SalvageTuning::fragment_ticks`; `None`: never).
    fn loose_expiry(&self) -> Option<u64> {
        self.config()
            .salvage
            .fragment_ticks
            .map(|t| self.tick() + t as u64)
    }

    /// Moves the loose items one tick (after the physics step, like the bodies); expired ones are gone.
    pub(crate) fn drift_loose(&mut self) {
        let tick = self.tick();
        let loose = &mut self.salvage.loose;
        loose.retain(|l| l.expires.is_none_or(|e| e > tick));
        for l in loose.iter_mut() {
            l.prev_pos = l.pos;
            l.pos += l.vel * DT;
        }
    }

    /// The collect pass (every `SalvageTuning::interval` ticks, after the weapons): each running vessel of
    /// an owner that collects, in ship order, takes one item (see the module docs); stripped parts are
    /// resolved as breakage once at the end.
    pub(crate) fn collect(&mut self) {
        let t = self.config().salvage;
        if t.interval == 0 || !self.tick().is_multiple_of(t.interval as u64) {
            return;
        }
        let mut stripped = false;
        self.salvage.stripped.clear();
        for k in 0..self.ships.len() {
            let c = &self.ships[k];
            if !t.collects(c.owner) || c.vessel_state() != Some(VesselState::Running) {
                continue;
            }
            let free = c.cargo_capacity().saturating_sub(c.cargo.used);
            if free == 0 {
                continue;
            }
            if let Some(pick) = pick(&self.ships, &mut self.salvage, k, free, t.reach, t.max_speed) {
                stripped |= self.take(k, pick);
            }
        }
        if stripped {
            self.resolve_breakage();
        }
    }

    /// Collector `k` takes `pick`: into its hold, `SimEvent::Collected`. Returns whether a part left a piece.
    fn take(&mut self, k: usize, pick: Pick) -> bool {
        let ship = self.ships[k].id;
        let (item, units, value, from, at, stripped) = match pick {
            Pick::Loose(i) => {
                let l = self.salvage.loose.remove(i);
                (
                    l.item,
                    l.units,
                    l.value,
                    Salvaged::Loose(l.id),
                    l.pos,
                    false,
                )
            }
            Pick::Cargo { j, q, item } => {
                let s = &mut self.ships[j];
                let Some((units, value)) = s.cargo.take(&item) else {
                    return false;
                };
                let at = s.pieces[q].com_world();
                (
                    item,
                    units,
                    value,
                    Salvaged::Cargo { ship: s.id },
                    at,
                    false,
                )
            }
            Pick::Part { j, q, part } => {
                let s = &mut self.ships[j];
                let (item, units, value) = s.part_item(part);
                let at = s.part_world(part).map_or(s.pieces[q].com_world(), |m| m.0);
                let from = Salvaged::Part {
                    ship: s.id,
                    piece: s.pieces[q].id,
                    part,
                };
                s.remove_part(part as usize);
                self.salvage.stripped.push((j, q));
                (item, units, value, from, at, true)
            }
        };
        self.ships[k].cargo.add(item.clone(), units, value);
        self.events.push(SimEvent::Collected {
            ship,
            from,
            item,
            units,
            at,
        });
        stripped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(id: &str) -> Item {
        Item::Part {
            part: id.into(),
            material: None,
        }
    }

    #[test]
    fn a_hold_gives_its_best_and_sheds_its_worst() {
        let mut c = Cargo::default();
        c.add(part("gyro"), 1, 4.0);
        c.add(part("fusion_reactor"), 4, 2.5);
        c.add(part("beam_laser"), 2, 3.5);
        c.add(part("capacitor"), 1, 2.0);
        c.add(part("armor_block"), 1, 1.0);
        c.add(part("armor_block"), 1, 1.0);
        c.add(part("radiator"), 1, 2.0);
        assert_eq!(c.used(), 11);
        assert_eq!(c.best(9), Some(&part("gyro")));
        c.take(&part("gyro"));
        // Value ties go to the first in item order; only what fits counts.
        assert_eq!(c.best(1), Some(&part("capacitor")));
        assert_eq!(c.best(2), Some(&part("beam_laser")));
        assert_eq!(c.best(0), None);
        // Shrinking to 7 units drops both blocks (1.0), then the radiator (2.0, the last of the 2.0 ties).
        c.shed(7);
        assert_eq!(c.used(), 7);
        assert_eq!(c.count(&part("armor_block")), 0);
        assert_eq!(c.count(&part("radiator")), 0);
        let left: Vec<&str> = c.items().map(|(i, _)| i.part()).collect();
        assert_eq!(left, ["beam_laser", "capacitor", "fusion_reactor"]);
        c.shed(6);
        assert_eq!(c.count(&part("capacitor")), 0);
        c.shed(0);
        assert!(c.is_empty() && c.used() == 0);
    }

    #[test]
    fn cargo_loose_items_and_the_salvage_tuning_are_in_the_hash() {
        let content = data::base_content();
        let bp = content.ships.get("halvex_runner").unwrap();
        let design = Arc::new(ShipDesign::compile(&content, bp).unwrap());
        let mut w = Sim::new(1);
        w.spawn_ship(design, Owner::PLAYER, Body::default());
        let h0 = w.state_hash();

        let mut a = w.clone();
        a.ships[0].cargo.add(part("gyro"), 1, 4.0);
        assert_ne!(a.state_hash(), h0);
        let mut b = w.clone();
        b.ships[0].cargo.add(part("gyro"), 1, 4.5);
        assert_ne!(b.state_hash(), a.state_hash(), "the value counts too");
        let mut b = w.clone();
        b.ships[0].cargo.add(part("capacitor"), 1, 4.0);
        assert_ne!(b.state_hash(), a.state_hash(), "and the item");

        let mut f = w.clone();
        f.salvage.loose.push(Loose {
            id: LooseId(0),
            item: Item::Fragment {
                part: "logic_core".into(),
            },
            units: 1,
            value: 3.0,
            pos: Vec2::ZERO,
            prev_pos: Vec2::ZERO,
            vel: Vec2::X,
            expires: Some(100),
        });
        assert_ne!(f.state_hash(), h0);
        let mut g = f.clone();
        g.salvage.loose[0].expires = None;
        assert_ne!(g.state_hash(), f.state_hash(), "the lifetime counts too");

        let mut c = w.clone();
        let mut config = *c.config();
        config.salvage.reach = 5.5;
        c.set_config(config);
        assert_ne!(c.state_hash(), h0);
    }
}
