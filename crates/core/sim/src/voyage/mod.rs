//! The voyage director (GDR-0062, ADR-0033; `docs/sim.md` "Voyages"): one voyage is one small cluster (one `Sim`,
//! ADR-0022) with seeded sites.
//!
//! * **Rolling** ([`VoyagePlan::roll`]): from the content's voyage and a seed, on a generator of its own (salted
//!   with [`PLAN_STREAM`], so it never shares draws with the sim's streams for the same seed): where each site lies (inside the radius,
//!   no two sites or a site and the entry closer than the spacing's min, each within its max of an earlier one or
//!   the entry; the goal last, the farthest from the entry of a few good spots), how many ships and wrecks each
//!   holds, where they lie around it, each wreck's damage seed, and the compiled designs. The fleet enters at the cluster's edge
//!   ([`VoyagePlan::entry`]). [`VoyagePlan::roll_interior`] rolls a generated cluster's interior (`galaxy::interior`)
//!   the same way, with what the career knows (`data::career::Leads`, galaxy plan E4): the trail places a heard
//!   signal leads to are signals' sites, a log's place holds its data core in its first wreck (or loose at the site
//!   without one, `Sim::stow_data_core`), a place with no encounter is a beacon alone (a quiet site).
//! * **Hidden sites** (galaxy plan E3, GDR-0095): a site shows ([`Site::known`], `SimEvent::SiteRevealed`) once a
//!   player vessel has it within its sensor range (`Ship::sensor_range`; a signal's site only within
//!   `VoyageTuning::signal_reveal`), or a player's scan sweeps over it (`SimEvent::Scanned`, within the scanner's
//!   range). A sweep gives every hidden signal's site beyond its range a [`Bearing`] (`SimEvent::SiteBearing`) and
//!   wakes the sleeping sites within the scanner's wake radius. A slice voyage's goal is an ordinary site (its
//!   voyage is the signal, galaxy plan E7).
//! * **The edge**: beyond the radius plus `VoyageTuning::edge_margin` (at most [`MAX_EDGE`] from the centre, where
//!   `f32` still holds, E0) every piece is pushed back ([`edge_force`]).
//! * **Rocks** (GDR-0104, `rocks.rs`): the scenery's asteroid rock, rolled after the sites on a stream of its own
//!   and spawned field by field as anchored bodies when a player vessel comes near, as the sites spawn.
//! * **Lazy spawning**: [`Sim::start_voyage`] hands the plan to the sim; at the end of every step a site's ships
//!   (holding where they spawned, `Maneuver::Hold`) and wrecks (`Sim::spawn_wreck`) spawn once a vessel of the
//!   player owner comes within `VoyageTuning::spawn_radius` of it.
//! * **Waking**: a sleeping site's ships fly their behavior (their own or their faction's) once a player vessel
//!   comes within the encounter's wake radius (not at a wary site, GDR-0099 A3: every ship of a wary faction), one of
//!   them is hit (a shot or a plume on the hull, a shot its bubble took, a ram that deals damage), one of its wrecks is
//!   stripped (a part or its cargo collected, GDR-0102), or one of them stopped being a running vessel
//!   (`SimEvent::SiteWoke`). Vessels that split off a site's ships belong to the site.
//! * **Clearing** (GDR-0068): an awake site is cleared when each of its ships is destroyed or disarmed: none is a
//!   vessel (running, dark or dormant) that is still armed ([`Ship::is_armed`]) (`SimEvent::SiteCleared { goal }`);
//!   clearing is final. A site that rolled no ships is quiet: nothing wakes there and it is never cleared.
//! * **Recall** ([`Sim::can_recall`], GDR-0062, GDR-0068, GDR-0074): no armed awake enemy ([`Sim::armed_enemy`])
//!   within `VoyageTuning::recall_radius` of any of the owner's vessels, or none of its vessels armed (the
//!   emergency recall, [`Sim::fleet_disarmed`]).
//!
//! Everything works by owner (the player and the enemy side passed to `start_voyage`), so co-op can add more
//! player owners later. The voyage's state is in the state hash. Cost per step: one pass over this step's events,
//! the player's vessels, and per site up to two distance tests per player vessel (plus, for awake sites, one look-up
//! per ship, and per scan one test per site); no allocation once warm, except the ships a site spawns and a step's
//! scans.

pub mod rocks;
pub mod runner;
pub mod scenario;
pub mod waves;

use std::collections::BTreeMap;
use std::sync::Arc;

use data::Content;

use crate::combat::salvage::Salvaged;
use crate::ship::design::ShipDesign;
use crate::events::SimEvent;
use crate::hash::{StateHash, StateHasher};
use crate::ids::ShipId;
use crate::flight::maneuver::Maneuver;
use crate::flight::physics::BodyKey;
use crate::ship::mass::combine;
use crate::math::{Vec2, cos, sin, sqrt};
use crate::rng::{Pcg32, splitmix64};
use crate::ship::{Body, Ship};
use crate::ship::vessel::{Owner, VesselState, friendly};
use crate::world::{ShipCommand, Sim};

/// Voyage settings (`SimConfig::voyage`, in the state hash). **PLACEHOLDERS** (GDR-0062, the plan's numbers, lead
/// 13 Oct 2026).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoyageTuning {
    /// A site's ships and wrecks spawn when a player vessel comes this close to it (m).
    pub spawn_radius: f32,
    /// Recall works when no armed awake enemy vessel is this close to any of the owner's vessels (m).
    pub recall_radius: f32,
    /// A signal's site (a trail's place a heard signal leads to) shows when a player vessel comes this close, if
    /// its sensors reach that far, or when a scan sweeps over it (m; the plan's 4.1, 400 m).
    pub signal_reveal: f32,
    /// The cluster's edge lies this far outside its radius (m, at most [`MAX_EDGE`] from the centre): beyond it
    /// every piece is turned back (GDR-0095).
    pub edge_margin: f32,
}

impl Default for VoyageTuning {
    fn default() -> Self {
        VoyageTuning {
            spawn_radius: 600.0,
            recall_radius: 200.0,
            signal_reveal: 400.0,
            edge_margin: 500.0,
        }
    }
}

impl StateHash for VoyageTuning {
    fn hash_state(&self, h: &mut StateHasher) {
        h.f32(self.spawn_radius);
        h.f32(self.recall_radius);
        h.f32(self.signal_reveal);
        h.f32(self.edge_margin);
    }
}

/// The edge never lies further than this from the centre (m): one `f32` space holds a cluster of about 4 km
/// (ADR-0022, `docs/results/2026-10-16-bigger-clusters.md`).
pub const MAX_EDGE: f32 = 4000.0;
/// Beyond the edge a piece is pushed back towards the centre at up to this acceleration (m/s²), reached this
/// many metres past the edge, and its outward speed is taken away within [`EDGE_BRAKE`] seconds (PLACEHOLDERS).
pub const EDGE_PUSH: f32 = 3.0;
pub const EDGE_DEPTH: f32 = 100.0;
pub const EDGE_BRAKE: f32 = 2.0;

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

/// One site to roll (`VoyagePlan::roll_sites`): its encounter (none: a beacon alone), whether it is the goal,
/// whether it is a signal's site, and the data core it holds.
type SiteRoll<'c> = (Option<&'c data::Encounter>, bool, bool, Option<(u32, u32)>);

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
    /// The encounter (content id); `None` for a beacon alone (a trail's place with nothing waiting there).
    pub encounter: Option<String>,
    /// Clearing it clears the voyage.
    pub goal: bool,
    /// A signal leads here (a trail's place a heard signal leads to, GDR-0095): it shows
    /// only within `VoyageTuning::signal_reveal` or under a scan's sweep, and a scan farther away gives it a bearing.
    pub signal: bool,
    /// The data core it holds: the log of signal `.1` of trail `.0` (a log's place the career has not got yet).
    pub core: Option<(u32, u32)>,
    pub pos: Vec2,
    /// From the encounter (m).
    pub wake_radius: f32,
    /// Calm unless provoked (GDR-0099 A3): every ship of the encounter is of a wary faction, so the site does not
    /// wake when a player vessel comes within `wake_radius`, only when it is hit, rammed, scanned or stripped.
    pub wary: bool,
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
    /// A hot star beside the cluster (GDR-0102; a generated cluster whose type has one).
    pub star: Option<Star>,
    /// Asteroid rock in spawn groups (GDR-0104, [`rocks::roll`]): the voyage's or the cluster type's scenery.
    pub rocks: Vec<rocks::FieldPlan>,
}

/// A hot star beside a cluster (GDR-0102, content `ClusterType::star`): every part takes `heat` per second for each
/// of its sides open to space at `reach` metres from the star (the cluster's centre), growing with the inverse
/// square of the distance nearer it, at most [`data::STAR_HEAT_CAP`] times.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Star {
    /// World position (m).
    pub pos: Vec2,
    pub heat: f32,
    /// Its distance from the cluster's centre (m), where it heats by `heat`.
    pub reach: f32,
}

impl Star {
    /// Heat per second per open side at world point `p`. Only `+ − × ÷` and `min` (deterministic).
    pub fn heat_at(&self, p: Vec2) -> f32 {
        let d2 = (p - self.pos).length_squared().max(1.0);
        self.heat * (self.reach * self.reach / d2).min(data::STAR_HEAT_CAP)
    }
}

impl StateHash for Star {
    fn hash_state(&self, h: &mut StateHasher) {
        h.vec2(self.pos);
        h.f32(self.heat);
        h.f32(self.reach);
    }
}

impl VoyagePlan {
    /// Rolls a launch of `voyage` from `seed`: site positions, ship and wreck counts, their poses and the compiled
    /// designs. The same content, voyage and seed give the same plan on every platform. `Err` for content that
    /// does not validate (an unknown encounter or ship).
    pub fn roll(content: &Content, voyage: &data::Voyage, seed: u64) -> Result<VoyagePlan, String> {
        let mut list = Vec::new();
        for s in &voyage.sites {
            let e = content.encounters.get(&s.encounter).ok_or_else(|| {
                format!(
                    "voyage `{}`: unknown encounter `{}`",
                    voyage.id, s.encounter
                )
            })?;
            list.extend((0..s.count).map(|_| (Some(e), s.goal, false, None)));
        }
        let mut plan = Self::roll_sites(content, &voyage.id, voyage.radius, voyage.spacing, &list, seed)?;
        plan.roll_rocks(content, &voyage.scenery)?;
        Ok(plan)
    }

    /// The rocks of `scenery` round the plan's sites and entry (after the sites: their draws stay as they were).
    fn roll_rocks(&mut self, content: &Content, scenery: &data::Scenery) -> Result<(), String> {
        let sites: Vec<Vec2> = self.sites.iter().map(|s| s.pos).collect();
        self.rocks = rocks::roll(content, scenery, self.radius, &sites, self.entry.pos, self.seed)
            .map_err(|e| format!("voyage `{}`: {e}", self.voyage))?;
        Ok(())
    }

    /// Rolls a launch into a generated cluster's interior (`galaxy::interior`), named `id`: its ordinary sites and
    /// its trail places (none a goal), on the same generator as [`VoyagePlan::roll`]. `leads` (the career's,
    /// `Career::leads`) says which places are signals' sites and which hold a data core. `Err` for content that
    /// does not validate.
    pub fn roll_interior(
        content: &Content,
        id: &str,
        interior: &galaxy::Interior,
        leads: &data::career::Leads,
        seed: u64,
    ) -> Result<VoyagePlan, String> {
        let list = interior
            .sites
            .iter()
            .map(|s| {
                let e = match &s.encounter {
                    Some(e) => Some(content.encounters.get(e).ok_or_else(|| format!("cluster `{id}`: unknown encounter `{e}`"))?),
                    None => None,
                };
                let signal = s.place.is_some_and(|p| leads.signals.contains(&p));
                Ok((e, false, signal, s.place.filter(|p| leads.cores.contains(p))))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let mut plan = Self::roll_sites(content, id, interior.radius, interior.spacing, &list, seed)?;
        plan.star = interior.star.as_ref().map(|s| {
            let pos = Vec2::new(s.x, s.y);
            Star { pos, heat: s.heat, reach: pos.length() }
        });
        let kind = content.cluster_types.get(&interior.kind).ok_or_else(|| format!("cluster `{id}`: unknown cluster type `{}`", interior.kind))?;
        plan.roll_rocks(content, &kind.scenery)?;
        Ok(plan)
    }

    /// The roll of both: `list` holds each site to roll ([`SiteRoll`]).
    fn roll_sites(
        content: &Content,
        id: &str,
        radius: f32,
        spacing: (f32, f32),
        list: &[SiteRoll],
        seed: u64,
    ) -> Result<VoyagePlan, String> {
        let rng = &mut Pcg32::new(splitmix64(seed ^ splitmix64(PLAN_STREAM)), PLAN_STREAM);
        let entry = Vec2::new(0.0, -radius);

        // Positions: every other site first, in order, then the goal.
        let mut pos = vec![Vec2::ZERO; list.len()];
        let mut placed = vec![entry];
        for goal in [false, true] {
            for (k, &(_, g, _, _)) in list.iter().enumerate() {
                if g == goal {
                    pos[k] = place_site(rng, radius, spacing, &placed, entry, goal);
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
        for (k, &(e, goal, signal, core)) in list.iter().enumerate() {
            let mut ships = Vec::new();
            for s in e.map_or(&[][..], |e| &e.ships) {
                for _ in 0..roll_count(rng, s.count) {
                    ships.push((design(&s.ship)?, 0.0));
                }
            }
            let mut wrecks = Vec::new();
            for w in e.map_or(&[][..], |e| &e.wrecks) {
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
                encounter: e.map(|e| e.id.clone()),
                goal,
                signal,
                core,
                pos: pos[k],
                wake_radius: e.map_or(0.0, |e| e.wake_radius),
                wary: e.is_some_and(|e| wary(content, e)),
                ships,
                wrecks,
            });
        }
        Ok(VoyagePlan {
            voyage: id.to_string(),
            seed,
            radius,
            entry: Body::at(entry, 0.0),
            sites,
            star: None,
            rocks: Vec::new(),
        })
    }
}

/// Is an encounter calm unless provoked (GDR-0099 A3): it has ships, and every ship's faction is wary?
fn wary(content: &Content, e: &data::Encounter) -> bool {
    !e.ships.is_empty()
        && e.ships.iter().all(|s| {
            content
                .ships
                .get(&s.ship)
                .and_then(|b| content.factions.get(&b.faction))
                .is_some_and(|f| f.wary)
        })
}

/// The push that turns a piece back at a voyage's edge (GDR-0095): nothing inside `edge` (m from the centre);
/// beyond it, towards the centre, an acceleration growing to [`EDGE_PUSH`] over [`EDGE_DEPTH`] m plus what takes
/// its outward speed away within [`EDGE_BRAKE`] s. `com` and `vel`: the piece's centre of mass and velocity (world),
/// `mass` its mass. Not the piece's own force, so the speed governor never cuts it.
pub(crate) fn edge_force(com: Vec2, vel: Vec2, mass: f32, edge: f32) -> Vec2 {
    let r2 = com.length_squared();
    if r2 <= edge * edge {
        return Vec2::ZERO;
    }
    let r = sqrt(r2);
    let out = com * (1.0 / r);
    let depth = ((r - edge) / EDGE_DEPTH).min(1.0);
    let away = out.dot(vel).max(0.0);
    out * (-mass * (EDGE_PUSH * depth + away / EDGE_BRAKE))
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
    /// It shows: a player vessel had it within its sensors (a signal's site: within `signal_reveal`), or a scan
    /// swept over it. Final.
    known: bool,
    /// One of its ships was hit, or a scan swept close enough to wake it, since the last pass (scratch, always
    /// false between steps).
    hit: bool,
}

impl Site {
    /// The encounter (content id); `None` for a beacon alone.
    pub fn encounter(&self) -> Option<&str> {
        self.plan.encounter.as_deref()
    }
    pub fn goal(&self) -> bool {
        self.plan.goal
    }
    /// A signal leads here ([`SitePlan::signal`]).
    pub fn signal(&self) -> bool {
        self.plan.signal
    }
    /// Has the fleet found it ([`SimEvent::SiteRevealed`])? Until then the client shows nothing of it.
    pub fn known(&self) -> bool {
        self.known
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

/// A line a scan drew towards a hidden signal's site ([`Voyage::bearings`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bearing {
    /// The site (an index into [`Voyage::sites`]).
    pub site: u32,
    /// Where the scan was taken.
    pub from: Vec2,
    /// Unit vector from there towards the site.
    pub dir: Vec2,
    /// The tick of the scan.
    pub tick: u64,
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
    star: Option<Star>,
    /// The rocks' spawn groups, each with whether it has spawned (GDR-0104).
    fields: Vec<(rocks::FieldPlan, bool)>,
    /// The rocks spawned so far, in spawn order: rock `k` is the body `BodyKey::rock(k)`.
    rocks: Vec<rocks::Rock>,
    /// Every bearing a scan gave, oldest first.
    bearings: Vec<Bearing>,
    /// The site of every ship spawned for one and of the vessels split off them (derived from `sites`).
    site_of: BTreeMap<ShipId, u32>,
    /// The player's vessel positions and sensor ranges this pass (scratch).
    players: Vec<(Vec2, f32)>,
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
    /// The hot star beside the cluster, if any (GDR-0102).
    /// The rocks spawned so far (GDR-0104): rock `k` is the body `BodyKey::rock(k)`; more come as the fleet nears
    /// their fields.
    pub fn rocks(&self) -> &[rocks::Rock] {
        &self.rocks
    }
    pub fn star(&self) -> Option<&Star> {
        self.star.as_ref()
    }
    /// Every bearing a scan gave to a hidden signal's site, oldest first (one per site and scan).
    pub fn bearings(&self) -> &[Bearing] {
        &self.bearings
    }
    /// How far from the centre the cluster's edge lies for `tuning` (m): its radius plus the margin, at most
    /// [`MAX_EDGE`].
    pub fn edge(&self, tuning: &VoyageTuning) -> f32 {
        (self.radius + tuning.edge_margin).min(MAX_EDGE)
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
    /// The player's vessels the director looked from at its last pass (not dormant): where each is and how far it sees
    /// (`Ship::sensor_range`). What the fleet's sensors show (GDR-0095) lies within one of these.
    pub fn sensors(&self) -> &[(Vec2, f32)] {
        &self.players
    }
    /// Is a player vessel within `r` of `p`?
    fn near(&self, p: Vec2, r: f32) -> bool {
        self.players.iter().any(|&(q, _)| (q - p).length_squared() <= r * r)
    }
    /// Does a player vessel see `p`: within its sensor range, and within `cap` of it?
    fn seen(&self, p: Vec2, cap: f32) -> bool {
        self.players.iter().any(|&(q, r)| {
            let r = r.min(cap);
            (q - p).length_squared() <= r * r
        })
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
        h.str(p.encounter.as_deref().unwrap_or(""));
        h.bool(p.goal);
        h.bool(p.signal);
        // Only when there is one, so plans without data cores (the slice's) hash as before.
        if let Some((trail, signal)) = p.core {
            h.u32(trail);
            h.u32(signal);
        }
        h.vec2(p.pos);
        h.f32(p.wake_radius);
        // Only when wary, so plans without wary sites hash as before.
        if p.wary {
            h.u8(1);
        }
        for list in [&p.ships, &p.wrecks] {
            h.u32(list.len() as u32);
            for s in list {
                s.hash_state(h);
            }
        }
        h.u8(self.state as u8);
        h.bool(self.known);
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
        // Only with a star, so voyages without one hash as before.
        if let Some(star) = &self.star {
            star.hash_state(h);
        }
        // Only with rocks, so voyages without any hash as before.
        if !self.fields.is_empty() {
            h.u32(self.fields.len() as u32);
            for (f, spawned) in &self.fields {
                h.vec2(f.centre);
                h.f32(f.reach);
                h.bool(*spawned);
                h.u32(f.rocks.len() as u32);
                for r in &f.rocks {
                    r.hash_state(h);
                }
            }
            h.u32(self.rocks.len() as u32);
        }
        h.u32(self.bearings.len() as u32);
        for b in &self.bearings {
            h.u32(b.site);
            h.vec2(b.from);
            h.vec2(b.dir);
            h.u64(b.tick);
        }
    }
}

impl Sim {
    /// Starts directing a voyage (replacing one underway): from the end of the next step on, sites spawn, wake and
    /// clear around the vessels of `player`; their ships and wrecks belong to `enemy`. The caller spawns the fleet
    /// (at `plan.entry`) and sets the behavior library the woken ships fly by (`Sim::set_behaviors`).
    pub fn start_voyage(&mut self, plan: VoyagePlan, player: Owner, enemy: Owner) {
        if let Some(old) = self.voyage.take() {
            for k in 0..old.rocks.len() {
                self.physics.remove_body(BodyKey::rock(k as u32));
            }
        }
        let sites = plan
            .sites
            .into_iter()
            .map(|plan| Site {
                plan,
                state: SiteState::Unspawned,
                ships: Vec::new(),
                wrecks: Vec::new(),
                known: false,
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
            star: plan.star,
            fields: plan.rocks.into_iter().map(|f| (f, false)).collect(),
            rocks: Vec::new(),
            bearings: Vec::new(),
            site_of: BTreeMap::new(),
            players: Vec::new(),
        }));
    }

    /// Does an asteroid rock (GDR-0104) lie on the straight line from `a` to `b`? The scripted pilot holds fire
    /// through one.
    pub fn rock_between(&self, a: Vec2, b: Vec2) -> bool {
        let d = b - a;
        let len = d.length();
        len > 0.0 && self.physics.raycast_skipping(a, d * (1.0 / len), len, |k| k.as_rock().is_none()).is_some()
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
    /// last pass (split-off vessels join their site, hits wake it, the player's scans reveal sites, give bearings
    /// and wake sites), show the sites the player's sensors reach, wake sites, clear them.
    pub(crate) fn direct_voyage(&mut self) {
        let Some(mut v) = self.voyage.take() else {
            return;
        };
        let VoyageTuning { spawn_radius, signal_reveal, .. } = self.config().voyage;
        v.players.clear();
        for s in &self.ships {
            if s.team() == Some(v.player)
                && !s.is_dormant()
                && let Some(p) = s.control_piece().and_then(|c| s.piece(c))
            {
                v.players.push((p.com_world(), s.sensor_range()));
            }
        }
        for k in 0..v.sites.len() {
            if v.sites[k].state == SiteState::Unspawned && v.near(v.sites[k].plan.pos, spawn_radius)
            {
                self.spawn_site(&mut v, k);
            }
        }
        for k in 0..v.fields.len() {
            let (f, spawned) = &v.fields[k];
            if !*spawned && v.near(f.centre, spawn_radius + f.reach) {
                for r in &f.rocks {
                    self.physics.create_rock(BodyKey::rock(v.rocks.len() as u32), r);
                    v.rocks.push(r.clone());
                }
                v.fields[k].1 = true;
            }
        }
        let reveal = |v: &mut Voyage, events: &mut Vec<SimEvent>, k: usize| {
            if !v.sites[k].known {
                v.sites[k].known = true;
                events.push(SimEvent::SiteRevealed { site: k as u32 });
            }
        };
        let mut found = Vec::new();
        for e in &self.events {
            if let &SimEvent::Scanned { ship, at, range, wake } = e
                && self.ship(ship).is_some_and(|s| s.team() == Some(v.player))
            {
                found.push((at, range, wake));
            }
        }
        for (at, range, wake) in found {
            for k in 0..v.sites.len() {
                let d = v.sites[k].plan.pos - at;
                let d2 = d.length_squared();
                if d2 <= range * range {
                    reveal(&mut v, &mut self.events, k);
                } else if v.sites[k].plan.signal && !v.sites[k].known {
                    v.bearings.push(Bearing { site: k as u32, from: at, dir: d * (1.0 / sqrt(d2)), tick: self.tick() });
                    self.events.push(SimEvent::SiteBearing { site: k as u32 });
                }
                if d2 <= wake * wake {
                    v.sites[k].hit = true;
                }
            }
        }
        for k in 0..v.sites.len() {
            let cap = if v.sites[k].plan.signal { signal_reveal } else { f32::INFINITY };
            if !v.sites[k].known && v.seen(v.sites[k].plan.pos, cap) {
                reveal(&mut v, &mut self.events, k);
            }
        }
        let mark = |v: &mut Voyage, ship: ShipId| {
            if let Some(k) = v.site_of(ship)
                && v.sites[k].ships.binary_search(&ship).is_ok()
            {
                v.sites[k].hit = true;
            }
        };
        // Stripped: a player vessel took a part or cargo from one of its wrecks (GDR-0099 A3, GDR-0102).
        let stripped = |v: &mut Voyage, ship: ShipId| {
            if let Some(k) = v.site_of(ship)
                && v.sites[k].wrecks.binary_search(&ship).is_ok()
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
                SimEvent::Collected { from: Salvaged::Part { ship, .. } | Salvaged::Cargo { ship, .. }, .. } => {
                    stripped(&mut v, *ship)
                }
                _ => {}
            }
        }
        for k in 0..v.sites.len() {
            let site = &v.sites[k];
            if site.state == SiteState::Asleep
                && (site.hit
                    || (!site.plan.wary && v.near(site.plan.pos, site.plan.wake_radius))
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
        if let Some(core) = site.plan.core {
            self.stow_data_core(site.wrecks.first().copied(), site.plan.pos, core);
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
