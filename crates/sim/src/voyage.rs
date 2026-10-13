//! The voyage director (GDR-0062, ADR-0033; `docs/sim.md` "Voyages"): one voyage is one small cluster (one `Sim`,
//! ADR-0022) with seeded sites.
//!
//! * **Rolling** ([`VoyagePlan::roll`]): from the content's voyage and a seed, on a generator of its own (salted
//!   with [`PLAN_STREAM`], so it never shares draws with the sim's streams for the same seed): where each site lies (inside the radius,
//!   no two sites or a site and the entry closer than the spacing's min, each within its max of an earlier one or
//!   the entry; the goal last, the farthest from the entry of a few good spots), how many ships and wrecks each
//!   holds, where they lie around it, each wreck's damage seed, and the compiled designs. The fleet enters at the cluster's edge
//!   ([`VoyagePlan::entry`]).
//! * **Lazy spawning**: [`Sim::start_voyage`] hands the plan to the sim; at the end of every step a site's ships
//!   (holding where they spawned, `Maneuver::Hold`) and wrecks (`Sim::spawn_wreck`) spawn once a vessel of the
//!   player owner comes within `VoyageTuning::spawn_radius` of it.
//! * **Waking**: a sleeping site's ships fly their behavior (their own or their faction's) once a player vessel
//!   comes within the encounter's wake radius, one of them is hit (a shot or a plume on the hull, a shot its bubble
//!   took, a ram that deals damage), or one of them stopped being a running vessel (`SimEvent::SiteWoke`). Vessels
//!   that split off a site's ships belong to the site.
//! * **Clearing** (GDR-0068): an awake site is cleared when each of its ships is destroyed or disarmed: none is a
//!   vessel (running, dark or dormant) that is still armed ([`Ship::is_armed`]) (`SimEvent::SiteCleared { goal }`);
//!   clearing is final. A site that rolled no ships is quiet: nothing wakes there and it is never cleared.
//! * **Recall** ([`Sim::can_recall`], GDR-0062, GDR-0068, GDR-0074): no armed awake enemy ([`Sim::armed_enemy`])
//!   within `VoyageTuning::recall_radius` of any of the owner's vessels, or none of its vessels armed (the
//!   emergency recall, [`Sim::fleet_disarmed`]).
//!
//! Everything works by owner (the player and the enemy side passed to `start_voyage`), so co-op can add more
//! player owners later. The voyage's state is in the state hash. Cost per step: one pass over this step's events,
//! the player's vessels, and per site one distance test per player vessel (plus, for awake sites, one look-up per
//! ship); no allocation once warm, except the ships a site spawns.

use std::collections::BTreeMap;
use std::sync::Arc;

use data::Content;

use crate::design::ShipDesign;
use crate::events::SimEvent;
use crate::hash::{StateHash, StateHasher};
use crate::ids::ShipId;
use crate::maneuver::Maneuver;
use crate::mass::combine;
use crate::math::{Vec2, cos, sin, sqrt};
use crate::rng::{Pcg32, splitmix64};
use crate::ship::{Body, Ship};
use crate::vessel::{Owner, VesselState, friendly};
use crate::world::{ShipCommand, Sim};

/// Voyage settings (`SimConfig::voyage`, in the state hash). **PLACEHOLDERS** (GDR-0062, the plan's numbers, lead
/// 13 Oct 2026).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoyageTuning {
    /// A site's ships and wrecks spawn when a player vessel comes this close to it (m).
    pub spawn_radius: f32,
    /// Recall works when no armed awake enemy vessel is this close to any of the owner's vessels (m).
    pub recall_radius: f32,
}

impl Default for VoyageTuning {
    fn default() -> Self {
        VoyageTuning {
            spawn_radius: 600.0,
            recall_radius: 200.0,
        }
    }
}

impl StateHash for VoyageTuning {
    fn hash_state(&self, h: &mut StateHasher) {
        h.f32(self.spawn_radius);
        h.f32(self.recall_radius);
    }
}

/// How a site's ships and wrecks lie around it (PLACEHOLDERS): within this many metres of the site at first ...
const SITE_SPREAD: f32 = 30.0;
/// ... widened by this much after every [`SPREAD_TRIES`] spots that came too close to another ...
const SITE_SPREAD_GROW: f32 = 10.0;
const SPREAD_TRIES: u32 = 16;
/// ... and at least this far apart, hull circle to hull circle (m).
const SITE_GAP: f32 = 5.0;
/// Random spots tried per site before the best of them is taken.
const PLACE_TRIES: u32 = 64;
/// Good spots compared for the goal (the farthest from the entry wins).
const GOAL_CHOICES: u32 = 8;

/// The salt and PCG sequence of the plan's generator: `Pcg32::new(splitmix64(seed ^ splitmix64(PLAN_STREAM)),
/// PLAN_STREAM)`, derived like an RNG stream (`rng.rs`) with an id no stream has ("voyage" in ASCII). Part of the
/// save format (`data::career::GENERATOR` 2): never change it.
pub const PLAN_STREAM: u64 = 0x766f_7961_6765;

/// One ship or wreck a site spawns.
#[derive(Clone, Debug)]
pub struct Spawn {
    pub design: Arc<ShipDesign>,
    /// Pose of its main piece (at rest).
    pub body: Body,
    /// Wrecks: the share of its parts already gone (0 for ships).
    pub damage: f32,
    /// Wrecks: the seed that picks which parts are gone (`Sim::spawn_wreck`; 0 for ships). Rolled with the plan,
    /// so a wreck does not depend on when or in which order the sites spawn.
    pub seed: u64,
}

/// One site of a rolled voyage.
#[derive(Clone, Debug)]
pub struct SitePlan {
    /// The encounter (content id).
    pub encounter: String,
    /// Clearing it clears the voyage.
    pub goal: bool,
    pub pos: Vec2,
    /// From the encounter (m).
    pub wake_radius: f32,
    pub ships: Vec<Spawn>,
    pub wrecks: Vec<Spawn>,
}

/// A voyage rolled for one launch ([`VoyagePlan::roll`]): what [`Sim::start_voyage`] directs.
#[derive(Clone, Debug)]
pub struct VoyagePlan {
    /// The voyage (content id).
    pub voyage: String,
    pub seed: u64,
    /// The cluster's radius (m), centred on the world origin.
    pub radius: f32,
    /// Where the fleet enters: on the cluster's edge (world −y), nose towards the centre.
    pub entry: Body,
    /// In the content's order (an entry with a count gives that many sites in a row).
    pub sites: Vec<SitePlan>,
}

impl VoyagePlan {
    /// Rolls a launch of `voyage` from `seed`: site positions, ship and wreck counts, their poses and the compiled
    /// designs. The same content, voyage and seed give the same plan on every platform. `Err` for content that
    /// does not validate (an unknown encounter or ship).
    pub fn roll(content: &Content, voyage: &data::Voyage, seed: u64) -> Result<VoyagePlan, String> {
        let rng = &mut Pcg32::new(splitmix64(seed ^ splitmix64(PLAN_STREAM)), PLAN_STREAM);
        let radius = voyage.radius;
        let entry = Vec2::new(0.0, -radius);

        let mut list: Vec<(&data::Encounter, bool)> = Vec::new();
        for s in &voyage.sites {
            let e = content.encounters.get(&s.encounter).ok_or_else(|| {
                format!(
                    "voyage `{}`: unknown encounter `{}`",
                    voyage.id, s.encounter
                )
            })?;
            list.extend((0..s.count).map(|_| (e, s.goal)));
        }

        // Positions: every other site first, in order, then the goal.
        let mut pos = vec![Vec2::ZERO; list.len()];
        let mut placed = vec![entry];
        for goal in [false, true] {
            for (k, &(_, g)) in list.iter().enumerate() {
                if g == goal {
                    pos[k] = place_site(rng, radius, voyage.spacing, &placed, entry, goal);
                    placed.push(pos[k]);
                }
            }
        }

        let mut designs: BTreeMap<&str, Arc<ShipDesign>> = BTreeMap::new();
        let mut design = |id: &str| -> Result<Arc<ShipDesign>, String> {
            if let Some(d) = designs.get(id) {
                return Ok(Arc::clone(d));
            }
            let bp = content
                .ships
                .get(id)
                .ok_or_else(|| format!("unknown ship `{id}`"))?;
            let d = Arc::new(ShipDesign::compile(content, bp)?);
            designs.insert(&bp.id, Arc::clone(&d));
            Ok(d)
        };
        let mut sites = Vec::with_capacity(list.len());
        for (k, &(e, goal)) in list.iter().enumerate() {
            let mut ships = Vec::new();
            for s in &e.ships {
                for _ in 0..roll_count(rng, s.count) {
                    ships.push((design(&s.ship)?, 0.0));
                }
            }
            let mut wrecks = Vec::new();
            for w in &e.wrecks {
                for _ in 0..roll_count(rng, w.count) {
                    wrecks.push((design(&w.ship)?, w.damage));
                }
            }
            let bodies = layout(
                rng,
                pos[k],
                ships.iter().chain(&wrecks).map(|(d, _)| hull_radius(d)),
            );
            let mut bodies = bodies.into_iter();
            let mut spawns = |list: Vec<(Arc<ShipDesign>, f32)>, rng: Option<&mut Pcg32>| -> Vec<Spawn> {
                let mut rng = rng;
                list.into_iter()
                    .map(|(design, damage)| Spawn {
                        design,
                        body: bodies.next().unwrap(),
                        damage,
                        seed: rng.as_deref_mut().map_or(0, |r| r.next_u64()),
                    })
                    .collect()
            };
            let ships = spawns(ships, None);
            let wrecks = spawns(wrecks, Some(&mut *rng));
            sites.push(SitePlan {
                encounter: e.id.clone(),
                goal,
                pos: pos[k],
                wake_radius: e.wake_radius,
                ships,
                wrecks,
            });
        }
        Ok(VoyagePlan {
            voyage: voyage.id.clone(),
            seed,
            radius,
            entry: Body::at(entry, 0.0),
            sites,
        })
    }
}

/// A count in `lo..=hi` (`hi` below `lo` counts as `lo`).
fn roll_count(rng: &mut Pcg32, (lo, hi): (u32, u32)) -> u32 {
    lo + rng.below(hi.saturating_sub(lo).saturating_add(1))
}

/// A uniform point in the disc of `radius` about `centre`.
fn in_disc(rng: &mut Pcg32, centre: Vec2, radius: f32) -> Vec2 {
    let r = radius * sqrt(rng.next_f32());
    let a = rng.range_f32(-core::f32::consts::PI, core::f32::consts::PI);
    centre + Vec2::new(cos(a), sin(a)) * r
}

/// Where one site goes: a spot inside the radius whose nearest placed spot (sites and the entry) lies within
/// `spacing`. A plain site takes the first good spot, the goal the farthest from the entry of up to
/// [`GOAL_CHOICES`]; with no good spot in [`PLACE_TRIES`] the one that misses the spacing least.
fn place_site(
    rng: &mut Pcg32,
    radius: f32,
    (near, far): (f32, f32),
    placed: &[Vec2],
    entry: Vec2,
    goal: bool,
) -> Vec2 {
    let mut best: Option<(f32, Vec2)> = None; // (miss, spot): the least miss so far
    let mut good: Option<(f32, Vec2)> = None; // (distance from the entry, spot): the farthest good spot
    let mut found = 0;
    for _ in 0..PLACE_TRIES {
        let p = in_disc(rng, Vec2::ZERO, radius);
        let nearest = placed
            .iter()
            .map(|&q| (p - q).length())
            .fold(f32::INFINITY, f32::min);
        let miss = (near - nearest).max(nearest - far).max(0.0);
        if miss == 0.0 {
            if !goal {
                return p;
            }
            let d = (p - entry).length();
            if good.is_none_or(|(g, _)| d > g) {
                good = Some((d, p));
            }
            found += 1;
            if found == GOAL_CHOICES {
                break;
            }
        } else if best.is_none_or(|(m, _)| miss < m) {
            best = Some((miss, p));
        }
    }
    good.or(best).map_or(Vec2::ZERO, |(_, p)| p)
}

/// A ship's controlling piece's centre of mass in the world (`None` without one).
fn control_com(s: &Ship) -> Option<Vec2> {
    Some(s.piece(s.control_piece()?)?.com_world())
}

/// Radius of a design's hull circle about the point a spawn pose places (m): the farthest corner of any of its
/// parts from its main piece's centre of mass (the frame origin `Ship::new` puts at the pose; the same quantity as
/// `Piece::radius`, for the whole blueprint). 1 for a design without parts.
fn hull_radius(d: &ShipDesign) -> f32 {
    let pieces = d.grid.pieces();
    let Some(main) = d.grid.main_piece(&pieces, |i| d.parts[i].logic) else {
        return 1.0;
    };
    let com = combine(
        pieces.parts[main].iter().map(|&i| {
            let s = &d.parts[i];
            (s.mass, s.centroid, s.inertia)
        }),
        [0.0, 0.0],
    )
    .1;
    let mut r2 = 0.0f32;
    for s in &d.parts {
        for c in s.shape.polygon() {
            let [x, y] = c.to_f32();
            let (dx, dy) = (x - com[0], y - com[1]);
            r2 = r2.max(dx * dx + dy * dy);
        }
    }
    sqrt(r2)
}

/// Poses for things of these hull radii around a site: random spots and headings, hull circles at least
/// [`SITE_GAP`] apart, spreading out when the spots run short.
fn layout(rng: &mut Pcg32, centre: Vec2, radii: impl Iterator<Item = f32>) -> Vec<Body> {
    let mut placed: Vec<(Vec2, f32)> = Vec::new();
    for r in radii {
        let mut spread = SITE_SPREAD;
        let mut tries = 0;
        let p = loop {
            let p = in_disc(rng, centre, spread);
            if placed
                .iter()
                .all(|&(q, rq)| (p - q).length() >= r + rq + SITE_GAP)
            {
                break p;
            }
            tries += 1;
            if tries % SPREAD_TRIES == 0 {
                spread += SITE_SPREAD_GROW;
            }
        };
        placed.push((p, r));
    }
    placed
        .into_iter()
        .map(|(p, _)| Body::at(p, rng.range_f32(-core::f32::consts::PI, core::f32::consts::PI)))
        .collect()
}

/// What a site is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiteState {
    /// Nothing spawned yet: no player vessel came within the spawn radius.
    Unspawned,
    /// Spawned without ships (wrecks only): nothing wakes there, it is never cleared.
    Quiet,
    /// Its ships hold where they spawned.
    Asleep,
    /// Its ships fly their behavior.
    Awake,
    /// Each of its ships is destroyed or disarmed: none is a vessel that is still armed (GDR-0068; final).
    Cleared,
}

/// One site of a voyage underway.
#[derive(Clone, Debug)]
pub struct Site {
    plan: SitePlan,
    state: SiteState,
    /// Its fighting ships: those spawned and the vessels split off them, ascending id.
    ships: Vec<ShipId>,
    /// Its wrecks, ascending id.
    wrecks: Vec<ShipId>,
    /// One of its ships was hit since the last pass (scratch, always false between steps).
    hit: bool,
}

impl Site {
    /// The encounter (content id).
    pub fn encounter(&self) -> &str {
        &self.plan.encounter
    }
    pub fn goal(&self) -> bool {
        self.plan.goal
    }
    pub fn pos(&self) -> Vec2 {
        self.plan.pos
    }
    pub fn wake_radius(&self) -> f32 {
        self.plan.wake_radius
    }
    pub fn state(&self) -> SiteState {
        self.state
    }
    /// Its fighting ships (spawned, and the vessels split off them), ascending id; empty until it spawns. Some may
    /// be gone or salvage by now.
    pub fn ships(&self) -> &[ShipId] {
        &self.ships
    }
    /// Its wrecks, ascending id; empty until it spawns.
    pub fn wrecks(&self) -> &[ShipId] {
        &self.wrecks
    }
    /// What spawns (or spawned) here.
    pub fn plan(&self) -> &SitePlan {
        &self.plan
    }
}

/// A voyage underway ([`Sim::voyage`]).
#[derive(Clone, Debug)]
pub struct Voyage {
    voyage: String,
    seed: u64,
    radius: f32,
    entry: Body,
    player: Owner,
    enemy: Owner,
    sites: Vec<Site>,
    /// The site of every ship spawned for one and of the vessels split off them (derived from `sites`).
    site_of: BTreeMap<ShipId, u32>,
    /// The player's vessel positions this pass (scratch).
    players: Vec<Vec2>,
}

impl Voyage {
    /// The voyage (content id).
    pub fn id(&self) -> &str {
        &self.voyage
    }
    pub fn seed(&self) -> u64 {
        self.seed
    }
    pub fn radius(&self) -> f32 {
        self.radius
    }
    /// Where the fleet enters (the plan's).
    pub fn entry(&self) -> Body {
        self.entry
    }
    pub fn player(&self) -> Owner {
        self.player
    }
    pub fn enemy(&self) -> Owner {
        self.enemy
    }
    pub fn sites(&self) -> &[Site] {
        &self.sites
    }
    /// The site a ship belongs to (a site's ship, a vessel split off one, or a wreck), as an index into
    /// [`Voyage::sites`].
    pub fn site_of(&self, ship: ShipId) -> Option<usize> {
        self.site_of.get(&ship).map(|&k| k as usize)
    }
    /// Has the goal site been cleared?
    pub fn goal_cleared(&self) -> bool {
        self.sites
            .iter()
            .any(|s| s.plan.goal && s.state == SiteState::Cleared)
    }
    /// Is this ship one of a sleeping site's (or a vessel split off one)?
    pub fn asleep(&self, ship: ShipId) -> bool {
        self.site_of(ship)
            .is_some_and(|k| self.sites[k].state == SiteState::Asleep)
    }
    /// Is a player vessel within `r` of `p`?
    fn near(&self, p: Vec2, r: f32) -> bool {
        self.players.iter().any(|&q| (q - p).length_squared() <= r * r)
    }
}

impl StateHash for Spawn {
    fn hash_state(&self, h: &mut StateHasher) {
        h.str(&self.design.id);
        self.body.hash_state(h);
        h.f32(self.damage);
        h.u64(self.seed);
    }
}

impl StateHash for Site {
    fn hash_state(&self, h: &mut StateHasher) {
        let p = &self.plan;
        h.str(&p.encounter);
        h.bool(p.goal);
        h.vec2(p.pos);
        h.f32(p.wake_radius);
        for list in [&p.ships, &p.wrecks] {
            h.u32(list.len() as u32);
            for s in list {
                s.hash_state(h);
            }
        }
        h.u8(self.state as u8);
        for list in [&self.ships, &self.wrecks] {
            h.u32(list.len() as u32);
            for s in list {
                h.u32(s.0);
            }
        }
    }
}

impl StateHash for Voyage {
    fn hash_state(&self, h: &mut StateHasher) {
        h.str(&self.voyage);
        h.u64(self.seed);
        h.f32(self.radius);
        self.entry.hash_state(h);
        h.u8(self.player.0);
        h.u8(self.enemy.0);
        h.u32(self.sites.len() as u32);
        for s in &self.sites {
            s.hash_state(h);
        }
    }
}

impl Sim {
    /// Starts directing a voyage (replacing one underway): from the end of the next step on, sites spawn, wake and
    /// clear around the vessels of `player`; their ships and wrecks belong to `enemy`. The caller spawns the fleet
    /// (at `plan.entry`) and sets the behavior library the woken ships fly by (`Sim::set_behaviors`).
    pub fn start_voyage(&mut self, plan: VoyagePlan, player: Owner, enemy: Owner) {
        let sites = plan
            .sites
            .into_iter()
            .map(|plan| Site {
                plan,
                state: SiteState::Unspawned,
                ships: Vec::new(),
                wrecks: Vec::new(),
                hit: false,
            })
            .collect();
        self.voyage = Some(Box::new(Voyage {
            voyage: plan.voyage,
            seed: plan.seed,
            radius: plan.radius,
            entry: plan.entry,
            player,
            enemy,
            sites,
            site_of: BTreeMap::new(),
            players: Vec::new(),
        }));
    }

    /// The voyage underway, if any.
    pub fn voyage(&self) -> Option<&Voyage> {
        self.voyage.as_deref()
    }

    /// Is `ship` an awake enemy of `owner`: a running vessel of another owner that is not one of a sleeping voyage
    /// site's ships? Without a voyage every running vessel of another owner is awake (sandbox waves).
    pub fn awake_enemy(&self, owner: Owner, ship: &Ship) -> bool {
        ship.team().is_some()
            && !friendly(ship.team(), Some(owner))
            && ship.vessel_state() == Some(VesselState::Running)
            && !self.voyage().is_some_and(|v| v.asleep(ship.id()))
    }

    /// Is `ship` an armed awake enemy of `owner` ([`Sim::awake_enemy`] and [`Ship::is_armed`])? Only these block
    /// `owner`'s recall (GDR-0068); the voyage scenario and the voyage runner fight them.
    pub fn armed_enemy(&self, owner: Owner, ship: &Ship) -> bool {
        ship.is_armed() && self.awake_enemy(owner, ship)
    }

    /// May `owner` recall home now (GDR-0062, GDR-0068, GDR-0074)? It needs a vessel, and either no armed awake
    /// enemy ([`Sim::armed_enemy`]) is within `VoyageTuning::recall_radius` (centre of mass to centre of mass) of any
    /// of its vessels, or none of its vessels has a working gun ([`Sim::fleet_disarmed`]: the emergency recall, the
    /// same warp).
    pub fn can_recall(&self, owner: Owner) -> bool {
        let r = self.config().voyage.recall_radius;
        let mine = || self.own_vessels(owner).filter_map(control_com);
        if mine().next().is_none() {
            return false;
        }
        self.fleet_disarmed(owner)
            || !self
                .ships
                .iter()
                .filter(|s| self.armed_enemy(owner, s))
                .filter_map(control_com)
                .any(|q| mine().any(|p| (q - p).length_squared() <= r * r))
    }

    /// Has `owner` vessels and none of them a working gun ([`Ship::is_armed`])? Then its recall is an emergency
    /// warp that armed enemies near do not block (GDR-0074).
    pub fn fleet_disarmed(&self, owner: Owner) -> bool {
        let mut any = false;
        for s in self.own_vessels(owner) {
            if s.is_armed() {
                return false;
            }
            any = true;
        }
        any
    }

    /// `owner`'s ships with a controlling piece (its vessels, running, dark or dormant).
    fn own_vessels(&self, owner: Owner) -> impl Iterator<Item = &Ship> + '_ {
        self.ships
            .iter()
            .filter(move |s| s.team() == Some(owner) && s.control_piece().is_some())
    }

    fn running(&self, ship: ShipId) -> bool {
        self.ship(ship).and_then(|s| s.vessel_state()) == Some(VesselState::Running)
    }

    /// Does `ship` keep its site from clearing (GDR-0068): a vessel (running, dark or dormant: those states pass)
    /// that is still armed?
    fn fights(&self, ship: ShipId) -> bool {
        self.ship(ship).is_some_and(|s| s.is_vessel() && s.is_armed())
    }

    /// The director's pass at the end of a step: spawn the sites near player vessels, follow the events since the
    /// last pass (split-off vessels join their site, hits wake it), wake sites, clear them.
    pub(crate) fn direct_voyage(&mut self) {
        let Some(mut v) = self.voyage.take() else {
            return;
        };
        let spawn_radius = self.config().voyage.spawn_radius;
        v.players.clear();
        for s in &self.ships {
            if s.team() == Some(v.player)
                && !s.is_dormant()
                && let Some(p) = s.control_piece().and_then(|c| s.piece(c))
            {
                v.players.push(p.com_world());
            }
        }
        for k in 0..v.sites.len() {
            if v.sites[k].state == SiteState::Unspawned && v.near(v.sites[k].plan.pos, spawn_radius)
            {
                self.spawn_site(&mut v, k);
            }
        }
        let mark = |v: &mut Voyage, ship: ShipId| {
            if let Some(k) = v.site_of(ship)
                && v.sites[k].ships.binary_search(&ship).is_ok()
            {
                v.sites[k].hit = true;
            }
        };
        for e in &self.events {
            match e {
                &SimEvent::VesselFormed {
                    ship,
                    from: Some((parent, _)),
                    ..
                } => {
                    if let Some(k) = v.site_of(parent)
                        && v.sites[k].ships.binary_search(&parent).is_ok()
                        && let Err(i) = v.sites[k].ships.binary_search(&ship)
                    {
                        v.sites[k].ships.insert(i, ship);
                        v.site_of.insert(ship, k as u32);
                    }
                }
                // Hurt or shot at: a hit on the hull or by a plume, a shot its bubble took, a ram with damage.
                &SimEvent::Hit { ship, .. }
                | &SimEvent::PlumeHit { ship, .. }
                | &SimEvent::ShieldHit { ship, .. } => mark(&mut v, ship),
                SimEvent::Impact(c) if c.damage > 0.0 => {
                    mark(&mut v, c.a.ship);
                    mark(&mut v, c.b.ship);
                }
                _ => {}
            }
        }
        for k in 0..v.sites.len() {
            let site = &v.sites[k];
            if site.state == SiteState::Asleep
                && (site.hit
                    || v.near(site.plan.pos, site.plan.wake_radius)
                    || site.ships.iter().any(|&s| !self.running(s)))
            {
                for &id in &site.ships {
                    let behavior = self
                        .ship(id)
                        .and_then(|s| self.behaviors().default_for(s.design()));
                    if behavior.is_some() {
                        self.command(id, ShipCommand::Behavior(behavior));
                    }
                }
                v.sites[k].state = SiteState::Awake;
                self.events.push(SimEvent::SiteWoke { site: k as u32 });
            }
            let site = &mut v.sites[k];
            site.hit = false;
            if site.state == SiteState::Awake && !site.ships.iter().any(|&s| self.fights(s)) {
                site.state = SiteState::Cleared;
                self.events.push(SimEvent::SiteCleared {
                    site: k as u32,
                    goal: site.plan.goal,
                });
            }
        }
        self.voyage = Some(v);
    }

    /// Spawns site `k`: its ships for the enemy, holding where they are (with the vessels a blueprint in several
    /// cored pieces forms at spawn), then its wrecks. New ships come after every existing one, so the site's list
    /// stays ascending.
    fn spawn_site(&mut self, v: &mut Voyage, k: usize) {
        let Voyage {
            sites,
            site_of,
            enemy,
            ..
        } = v;
        let site = &mut sites[k];
        for s in &site.plan.ships {
            let first = self.ships.len();
            self.spawn_ship(Arc::clone(&s.design), *enemy, s.body);
            for j in first..self.ships.len() {
                let ship = &self.ships[j];
                let id = ship.id();
                let at = match ship.control_piece().and_then(|c| ship.piece(c)) {
                    Some(p) if j > first => p.com_world(),
                    _ => s.body.pos,
                };
                self.command(
                    id,
                    ShipCommand::Maneuver {
                        maneuver: Maneuver::Hold { at },
                        fire: false,
                    },
                );
                site.ships.push(id);
                site_of.insert(id, k as u32);
            }
        }
        for w in &site.plan.wrecks {
            let id = self.spawn_wreck(Arc::clone(&w.design), *enemy, w.body, w.damage, w.seed);
            site.wrecks.push(id);
            site_of.insert(id, k as u32);
        }
        site.state = if site.ships.is_empty() {
            SiteState::Quiet
        } else {
            SiteState::Asleep
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_cover_the_range_and_tolerate_a_reversed_one() {
        let mut rng = Pcg32::new(1, 2);
        let mut seen = [false; 3];
        for _ in 0..200 {
            let n = roll_count(&mut rng, (2, 4));
            assert!((2..=4).contains(&n));
            seen[n as usize - 2] = true;
        }
        assert_eq!(seen, [true; 3]);
        assert_eq!(roll_count(&mut rng, (3, 3)), 3);
        assert_eq!(roll_count(&mut rng, (5, 1)), 5);
    }

    /// The hull circle is about the point a spawn pose places, the main piece's centre of mass (13 Oct 2026
    /// review): every collider of a spawned ship lies inside it, also for a lopsided blueprint.
    #[test]
    fn hull_circles_hold_the_spawned_ship() {
        let content = data::base_content();
        for bp in content.ships.iter() {
            let d = Arc::new(ShipDesign::compile(&content, bp).unwrap());
            let r = hull_radius(&d);
            let mut w = Sim::new(1);
            let at = Vec2::new(30.0, -20.0);
            let id = w.spawn_ship(Arc::clone(&d), Owner(1), Body::at(at, 0.7));
            let mut far = 0.0f32;
            for s in w.ships() {
                for p in s.pieces() {
                    for c in &p.colliders {
                        for &v in c.points() {
                            far = far.max((p.body.to_world(v) - at).length());
                        }
                    }
                }
            }
            assert!(far <= r + 1e-3, "{}: {far} > {r}", bp.id);
            assert!(w.ship(id).is_some());
        }
    }

    #[test]
    fn layouts_keep_hulls_apart() {
        let mut rng = Pcg32::new(7, 9);
        let radii = [4.2, 4.2, 11.4, 4.2, 6.4, 4.2, 4.2, 4.2, 4.2];
        let bodies = layout(&mut rng, Vec2::new(100.0, -50.0), radii.into_iter());
        assert_eq!(bodies.len(), radii.len());
        for i in 0..bodies.len() {
            for j in 0..i {
                let d = (bodies[i].pos - bodies[j].pos).length();
                assert!(d >= radii[i] + radii[j] + SITE_GAP, "{i} {j}: {d}");
            }
        }
    }
}
