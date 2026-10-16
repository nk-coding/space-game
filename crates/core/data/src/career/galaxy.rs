//! The galaxy in the career (save format 3, 4 since E7 and E8; galaxy plan E4, GDR-0093, GDR-0096, GDR-0098): the
//! skeleton the game was generated with (ADR-0035), the clusters charted, the signals heard and how far they
//! narrowed, the logs to decode, the gates opened, the memory log, the jump range, and the slice's voyages on the
//! clusters the skeleton placed them on.
//!
//! * **Charting**: home is charted from the start; launching into a cluster charts it (arriving is enough: the
//!   chart is knowledge, not haul). Jumps start from home or any charted cluster (GDR-0093) and reach
//!   [`Career::jump_range`] S.
//! * **Signals** ([`SignalRecord`]): a trail's signal is heard (a beacon scanned, [`Career::beacon`]; a log
//!   decoded) with the skeleton's fresh area; a scan in another cluster inside it narrows it towards its place (the
//!   galaxy's `signal_area.narrow`, [`Career::scanned`], [`Career::narrow`]), and once no other cluster lies inside,
//!   or a scan is taken in its own cluster, it is located. Its site seen is found, and cleared (or seen, with nothing
//!   there to clear) done ([`Career::found`], galaxy plan E6, GDR-0097).
//! * **Rewards** (galaxy plan E8, GDR-0098): the last signal of a trail done gives the trail's reward: a gate opens
//!   (its guardian beaten), a lore fragment joins the memory log ([`Exploration::memories`]), a cache's research data
//!   and materials (the trail's `gives`) go into store. A find, so kept on a lost voyage.
//! * **Jumps** reach the jump range from home or a charted cluster, into the bands opened so far
//!   ([`Career::band_open`]); a blind jump to a spot goes to the cluster nearest it ([`Career::blind_target`]); the
//!   warp takes the galaxy's `warp` seconds ([`Career::warp_seconds`]).
//! * **Logs**: a data core (one hold slot, GDR-0093) brought home waits to be decoded ([`Career::decode`], free,
//!   GDR-0080); lost with the haul, the core stays in its wreck and the next launch rolls it again.
//! * **The slice** (galaxy plan E7, GDR-0098): every voyage of the content lies on the cluster the skeleton placed it
//!   on (its set pieces; a skeleton from before them keeps the pins E4 gave its career, GDR-0096); a launched voyage
//!   charts its cluster, an open one not cleared yet is a located signal, a cleared one's signal is done, and a cleared
//!   one on a gate opens the gate (the Warden's: the rim's first gate trail). That is also how a format 2 save is
//!   migrated ([`Career::begin_galaxy`]).
//!
//! Distances are integers in hundredths of S ([`crate::skeleton::UNITS_PER_S`]), so a save narrows the same way on
//! every platform.

use super::*;
use crate::content::model::SourceForm;
use crate::skeleton::{Area, Point, Skeleton, UNITS_PER_S};

/// How far from a blind jump's spot its cluster may lie, in the galaxy's spacings (GDR-0097: about the farthest any
/// spot of the disc lies from a cluster).
pub const BLIND_REACH: f32 = 1.5;

/// What the player knows of the galaxy (save format 3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exploration {
    /// The galaxy as generated for this game (ADR-0035): never regenerated on load.
    skeleton: Skeleton,
    /// Charted clusters by id: home, and every cluster a voyage was launched into.
    #[serde(default)]
    charted: BTreeMap<u32, Charted>,
    /// The signals heard, in the order heard.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    signals: Vec<SignalRecord>,
    /// Data cores brought home and not decoded yet, in the order they came home.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    logs: Vec<TrailSignal>,
    /// Gate clusters opened (their guardian beaten).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    gates: BTreeSet<u32>,
    /// The content's voyages on clusters (voyage id → cluster id): the skeleton's set pieces (E7), or for a skeleton
    /// from before them the pins E4 gave (GDR-0096).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pins: BTreeMap<String, u32>,
    /// The memory log (GDR-0080): the lore fragments (ids) found at the ends of their trails, in the order found.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    memories: Vec<String>,
    /// The cluster a voyage was launched into and has not ended yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    underway: Option<u32>,
}

/// What a career remembers about a charted cluster.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Charted {
    /// Voyages launched into it; the next launch's seed mixes it in ([`Career::cluster_seed`]).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub launched: u32,
}

/// A trail's signal: `signals[signal]` of the skeleton's `trails[trail]`, from `places[signal]` to
/// `places[signal + 1]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TrailSignal {
    pub trail: u32,
    pub signal: u32,
}

/// What a signal leads to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lead {
    /// A trail's signal of the skeleton.
    Trail(TrailSignal),
    /// A slice voyage (content id), open and not cleared: located at its cluster (GDR-0078's slice step; the slice's
    /// voyages are the rim's first gate trail, E7).
    Voyage(String),
}

/// How far a signal has come (the plan's 4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SignalState {
    /// Heard: its area is the skeleton's.
    Fresh,
    /// Its area shrank (NARROWED).
    Narrowed,
    /// Its cluster is known (LOCATED).
    Located,
    /// Its site was seen (FOUND).
    Found,
    /// Followed to its end: its site cleared (or seen, with nothing to clear), a slice voyage cleared.
    Done,
}

/// One signal the player has heard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignalRecord {
    pub lead: Lead,
    /// The cluster (id) it leads to.
    pub cluster: u32,
    /// Its area on the map now (map units): the skeleton's fresh area, halved by each narrowing; a located signal's
    /// is its cluster.
    pub area: Area,
    pub state: SignalState,
}

/// What a launch into a cluster carries from the career (`sim::VoyagePlan::roll_interior`): trail places as
/// `(trail, place)` indices of the skeleton. `signals`: the places a heard signal leads to (hidden until close,
/// bearings from a scan); `cores`: the places whose wreck holds a data core the career has neither decoded nor heard.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Leads {
    pub signals: Vec<(u32, u32)>,
    pub cores: Vec<(u32, u32)>,
}

impl Exploration {
    pub fn skeleton(&self) -> &Skeleton {
        &self.skeleton
    }

    /// Charted clusters by id (home included).
    pub fn charted(&self) -> &BTreeMap<u32, Charted> {
        &self.charted
    }

    pub fn is_charted(&self, cluster: u32) -> bool {
        self.charted.contains_key(&cluster)
    }

    /// The signals heard, in the order heard.
    pub fn signals(&self) -> &[SignalRecord] {
        &self.signals
    }

    /// The signal of a trail, if heard.
    pub fn signal(&self, s: TrailSignal) -> Option<&SignalRecord> {
        self.signals.iter().find(|r| r.lead == Lead::Trail(s))
    }

    /// The logs waiting to be decoded.
    pub fn logs(&self) -> &[TrailSignal] {
        &self.logs
    }

    /// The gates opened.
    pub fn gates(&self) -> &BTreeSet<u32> {
        &self.gates
    }

    /// The content's voyages on clusters.
    pub fn pins(&self) -> &BTreeMap<String, u32> {
        &self.pins
    }

    /// The memory log: the lore fragments (ids) found, in the order found.
    pub fn memories(&self) -> &[String] {
        &self.memories
    }

    /// The cluster a voyage is underway to.
    pub fn underway(&self) -> Option<u32> {
        self.underway
    }

    /// Is the signal `s` of the skeleton (its trail and its signal exist)?
    fn knows(&self, s: TrailSignal) -> bool {
        self.skeleton.trails.get(s.trail as usize).is_some_and(|t| (s.signal as usize) < t.signals.len())
    }

    /// The cluster the trail signal `s` leads to.
    fn target(&self, s: TrailSignal) -> Option<u32> {
        let t = self.skeleton.trails.get(s.trail as usize)?;
        t.places.get(s.signal as usize + 1).map(|p| p.cluster)
    }

    fn at(&self, cluster: u32) -> Point {
        self.skeleton.clusters[cluster as usize].at
    }
}

/// A located signal's area: the cluster's place.
fn spot(at: Point) -> Area {
    Area { centre: at, radius: 0 }
}

/// The trail signal that leads to the place `(trail, place)` of the skeleton: the trail's signal before it; `None`
/// for a trail's first place (a source only).
pub fn signal_to((trail, place): (u32, u32)) -> Option<TrailSignal> {
    place.checked_sub(1).map(|signal| TrailSignal { trail, signal })
}

impl Career {
    /// What the player knows of the galaxy (`None` before [`Career::begin_galaxy`], or for content without one).
    pub fn exploration(&self) -> Option<&Exploration> {
        self.galaxy.as_ref()
    }

    /// Gives the career its galaxy, once (a new game, or a format 2 save the game loads): `skeleton` is the game's,
    /// generated from its seed (the `galaxy` crate). Home is charted and the content's voyages are pinned and
    /// charted ([`Exploration`]'s slice rules). Returns a note when the career had voyages already (a migrated save),
    /// and refuses nothing: a career with a galaxy keeps it.
    pub fn begin_galaxy(&mut self, content: &Content, skeleton: Skeleton) -> Vec<Problem> {
        if self.galaxy.is_some() || skeleton.clusters.get(skeleton.home as usize).is_none() {
            return Vec::new();
        }
        let home = skeleton.home;
        self.galaxy = Some(Exploration {
            skeleton,
            charted: [(home, Charted::default())].into_iter().collect(),
            signals: Vec::new(),
            logs: Vec::new(),
            gates: BTreeSet::new(),
            pins: BTreeMap::new(),
            memories: Vec::new(),
            underway: None,
        });
        self.sync_slice(content);
        let g = self.galaxy.as_ref().expect("just set");
        let flown = self.voyages.values().filter(|r| r.launched > 0).count();
        if flown == 0 {
            return Vec::new();
        }
        let signals = g.signals.iter().filter(|s| s.state != SignalState::Done).count();
        let n = |k: usize, one: &str| format!("{k} {one}{}", if k == 1 { "" } else { "s" });
        vec![
            Problem::warning(format!(
                "the galaxy is charted: {} from {} flown, {} open as {}",
                n(g.charted.len() - 1, "cluster"),
                n(flown, "voyage"),
                n(signals, "voyage"),
                if signals == 1 { "a signal" } else { "signals" }
            ))
            .field("galaxy"),
        ]
    }

    /// How far a jump reaches, in S: the galaxy's `jump_range` plus every [`Unlock::JumpRange`] of the researched
    /// nodes (GDR-0093: 2.5, then 3.25 and 4 with the two propulsion nodes of the base pack). 0 without a galaxy.
    pub fn jump_range(&self, content: &Content) -> f32 {
        let Some(g) = self.galaxy.as_ref().and_then(|g| content.galaxies.get(&g.skeleton.galaxy)) else {
            return 0.0;
        };
        self.researched_unlocks(content).fold(g.jump_range, |r, u| match u {
            Unlock::JumpRange(s) => r + s,
            _ => r,
        })
    }

    /// Where a jump to `cluster` starts: the nearest of home and the charted clusters within the jump range (the
    /// lowest id on a tie), not the cluster itself unless nothing else reaches it (galaxy plan E6: the warp runs from
    /// there); `None` beyond every one of them (or without a galaxy).
    pub fn jump_from(&self, content: &Content, cluster: u32) -> Option<u32> {
        let g = self.galaxy.as_ref()?;
        let to = g.skeleton.clusters.get(cluster as usize)?.at;
        let reach = (self.jump_range(content) as f64 * UNITS_PER_S as f64) as i64;
        g.charted
            .keys()
            .map(|&k| (k == cluster, to.dist2(g.at(k)), k))
            .filter(|&(_, d2, _)| d2 <= reach * reach)
            .min()
            .map(|(_, _, k)| k)
    }

    /// Can jumps reach into `band`: the galaxy's first band, and a band past it once a gate of the band outside it is
    /// open (GDR-0078: any one gate opens the next band). False without a galaxy or for a band it doesn't have.
    pub fn band_open(&self, content: &Content, band: &str) -> bool {
        let Some(g) = &self.galaxy else { return false };
        let Some(bands) = content.galaxies.get(&g.skeleton.galaxy).map(|x| &x.bands) else { return false };
        match bands.iter().position(|b| b == band) {
            Some(0) => true,
            Some(k) => g.skeleton.entrances(bands, k).iter().any(|e| g.gates.contains(e)),
            None => false,
        }
    }

    /// How long the warp to `cluster` takes (s): the galaxy's `warp`, a base plus so much per S from where the jump
    /// starts ([`Career::jump_from`]; GDR-0093: 3 s + 2 s per S). `None` beyond reach.
    pub fn warp_seconds(&self, content: &Content, cluster: u32) -> Option<f32> {
        let g = self.galaxy.as_ref()?;
        let warp = content.galaxies.get(&g.skeleton.galaxy)?.warp;
        let from = self.jump_from(content, cluster)?;
        Some(warp.0 + warp.1 * g.at(from).distance_s(g.at(cluster)) as f32)
    }

    /// Where a blind jump to the spot `at` (map units) goes (the plan's 4.4: empty space is no destination): the
    /// cluster nearest it that a jump reaches, neither home nor a slice voyage's (those are found by their own
    /// signals, GDR-0097), within [`BLIND_REACH`] of the galaxy's spacing (`None` farther: empty space); the lowest id
    /// on a tie. Charted ones count: a jump to a spot by one goes there. One in a band not open yet is still the
    /// target: [`Career::cluster_launched`] refuses it, and the map says why.
    pub fn blind_target(&self, content: &Content, at: Point) -> Option<u32> {
        let g = self.galaxy.as_ref()?;
        let spacing = content.galaxies.get(&g.skeleton.galaxy)?.spacing;
        let near = (spacing * BLIND_REACH * UNITS_PER_S as f32) as i64;
        let pinned: BTreeSet<u32> = g.pins.values().copied().collect();
        (0u32..)
            .zip(&g.skeleton.clusters)
            .filter(|(k, c)| *k != g.skeleton.home && !pinned.contains(k) && c.at.dist2(at) <= near * near)
            .filter(|(k, _)| self.jump_from(content, *k).is_some())
            .map(|(k, c)| (c.at.dist2(at), k))
            .min()
            .map(|(_, k)| k)
    }

    /// The seed of the next launch into `cluster`: the game seed, the cluster's own seed and the launches into it
    /// so far, mixed like [`Career::voyage_seed`]: `splitmix64(seed ^ splitmix64(cluster seed ^ launched))`. Part of
    /// the save format. 0 without a galaxy or for an unknown cluster.
    pub fn cluster_seed(&self, cluster: u32) -> u64 {
        let Some(g) = &self.galaxy else { return 0 };
        let Some(c) = g.skeleton.clusters.get(cluster as usize) else { return 0 };
        let launched = g.charted.get(&cluster).map_or(0, |r| r.launched);
        splitmix64(self.seed ^ splitmix64(c.seed ^ u64::from(launched)))
    }

    /// Records a launch into `cluster` (within reach of home or a charted cluster, [`Career::jump_from`]) and returns
    /// the seed its plan rolls from ([`Career::cluster_seed`] before this launch). Arriving charts it. Underway until
    /// [`Career::cluster_ended`]; write the save now, as for a voyage.
    pub fn cluster_launched(&mut self, content: &Content, cluster: u32) -> Result<u64, Refusal> {
        let underway = self.underway.clone().or_else(|| self.galaxy.as_ref()?.underway.map(|k| k.to_string()));
        if let Some(other) = underway {
            return Err(Refusal::Underway(other));
        }
        let g = self.galaxy.as_ref().ok_or(Refusal::NoGalaxy)?;
        if g.skeleton.clusters.get(cluster as usize).is_none() {
            return Err(Refusal::UnknownCluster(cluster));
        }
        if self.jump_from(content, cluster).is_none() {
            return Err(Refusal::OutOfReach(cluster));
        }
        if !self.band_open(content, &g.skeleton.clusters[cluster as usize].band) {
            return Err(Refusal::BandClosed(cluster));
        }
        let seed = self.cluster_seed(cluster);
        let g = self.galaxy.as_mut().expect("checked");
        let record = g.charted.entry(cluster).or_default();
        record.launched = record.launched.saturating_add(1);
        g.underway = Some(cluster);
        self.log.voyages = self.log.voyages.saturating_add(1);
        Ok(seed)
    }

    /// Ends the voyage into `cluster` (refused for any other). Recalled: the haul is unloaded like a voyage's, and
    /// every data core in it becomes a log to decode (unless its signal is heard or logged already); lost: the haul,
    /// cores included, stays behind. Either way the result joins the history with the cluster's name.
    pub fn cluster_ended(
        &mut self,
        content: &Content,
        cluster: u32,
        end: VoyageEnd,
    ) -> Result<&VoyageResult, Refusal> {
        let Some(g) = self.galaxy.as_mut().filter(|g| g.underway == Some(cluster)) else {
            return Err(Refusal::NotUnderway(cluster.to_string()));
        };
        g.underway = None;
        let name = g.skeleton.clusters[cluster as usize].name.clone();
        let mut last = match end {
            VoyageEnd::Recalled { haul, ticks, .. } => {
                for item in haul.items.keys() {
                    if let &HaulItem::DataCore { trail, signal } = item {
                        let s = TrailSignal { trail, signal };
                        if g.knows(s) && g.signal(s).is_none() && !g.logs.contains(&s) {
                            g.logs.push(s);
                        }
                    }
                }
                self.recalled(content, &haul, ticks, false)
            }
            VoyageEnd::Lost { ticks } => self.lost(ticks),
        };
        last.voyage = name;
        last.cluster = Some(cluster);
        Ok(self.push_history(last))
    }

    /// What a launch into `cluster` carries from the career (see [`Leads`]): the trail places there that a heard
    /// signal not done leads to, and the log places whose core is neither decoded, logged nor heard. Empty without a
    /// galaxy.
    pub fn leads(&self, content: &Content, cluster: u32) -> Leads {
        let mut out = Leads::default();
        let Some(g) = &self.galaxy else { return out };
        for (i, t) in (0u32..).zip(&g.skeleton.trails) {
            for (j, p) in (0u32..).zip(&t.places) {
                if p.cluster != cluster {
                    continue;
                }
                let led = j.checked_sub(1).and_then(|k| g.signal(TrailSignal { trail: i, signal: k }));
                if led.is_some_and(|r| r.state != SignalState::Done) {
                    out.signals.push((i, j));
                }
                let s = TrailSignal { trail: i, signal: j };
                let log = t.signals.get(j as usize).and_then(|x| content.signal_sources.get(&x.source));
                if log.is_some_and(|x| x.form == SourceForm::Log) && g.signal(s).is_none() && !g.logs.contains(&s) {
                    out.cores.push((i, j));
                }
            }
        }
        out
    }

    /// The signal `s` was heard (a beacon scanned, a log decoded): it joins the signals, fresh, with the skeleton's
    /// area. False when it is heard already or the skeleton has no such signal.
    pub fn hear(&mut self, s: TrailSignal) -> bool {
        let Some(g) = self.galaxy.as_mut() else { return false };
        let Some(cluster) = g.target(s).filter(|_| g.knows(s) && g.signal(s).is_none()) else {
            return false;
        };
        let area = g.skeleton.trails[s.trail as usize].signals[s.signal as usize].area;
        g.logs.retain(|l| *l != s);
        g.signals.push(SignalRecord { lead: Lead::Trail(s), cluster, area, state: SignalState::Fresh });
        true
    }

    /// Decodes the log `k` of [`Exploration::logs`] (free, GDR-0080): its signal is heard. Returns it, `None` for no
    /// such log.
    pub fn decode(&mut self, k: usize) -> Option<TrailSignal> {
        let g = self.galaxy.as_mut()?;
        let s = (k < g.logs.len()).then(|| g.logs.remove(k))?;
        self.hear(s);
        Some(s)
    }

    /// A scan in `from` heard the signal `s` (a bearing): taken in the cluster it leads to, it is located; taken
    /// elsewhere, its area shrinks to the galaxy's `signal_area.narrow` of its radius and moves towards the place,
    /// which stays within the galaxy's `signal_area.offset` of the new radius from its centre (as in a fresh area),
    /// and once no other cluster lies inside, it is located. Returns the state after, `None` for a signal not heard
    /// (or done).
    pub fn narrow(&mut self, content: &Content, s: TrailSignal, from: u32) -> Option<SignalState> {
        let rules = content.galaxies.get(&self.galaxy.as_ref()?.skeleton.galaxy)?.signal_area.clone();
        let g = self.galaxy.as_mut()?;
        let k = g.signals.iter().position(|r| r.lead == Lead::Trail(s) && r.state < SignalState::Located)?;
        let target = g.signals[k].cluster;
        let at = g.at(target);
        let area = g.signals[k].area;
        let (state, area) = if from == target {
            (SignalState::Located, spot(at))
        } else {
            let radius = ((area.radius as f32 * rules.narrow) as i32).max(1);
            let (dx, dy) = ((at.x - area.centre.x) as i64, (at.y - area.centre.y) as i64);
            let d = (dx * dx + dy * dy).isqrt();
            let off = (radius as f32 * rules.offset) as i64;
            // Move the centre towards the place until the place lies within `off` of it.
            let centre = if d > off {
                let t = d - off;
                Point { x: area.centre.x + (dx * t / d) as i32, y: area.centre.y + (dy * t / d) as i32 }
            } else {
                area.centre
            };
            let r2 = i64::from(radius) * i64::from(radius);
            let alone = (0u32..)
                .zip(&g.skeleton.clusters)
                .all(|(c, x)| c == target || x.at.dist2(centre) > r2);
            if alone {
                (SignalState::Located, spot(at))
            } else {
                (SignalState::Narrowed, Area { centre, radius })
            }
        };
        let r = &mut g.signals[k];
        r.state = state;
        r.area = area;
        Some(state)
    }

    /// A scan taken in `cluster` (galaxy plan E6, GDR-0097): every heard signal not located yet that leads there, or
    /// whose area holds the cluster, narrows from it ([`Career::narrow`]), except those in `skip` (narrowed on this
    /// voyage already: one voyage's scans in one cluster tell one thing). Returns each signal narrowed and its state
    /// after, in the order heard.
    pub fn scanned(&mut self, content: &Content, cluster: u32, skip: &[TrailSignal]) -> Vec<(TrailSignal, SignalState)> {
        let Some(g) = &self.galaxy else { return Vec::new() };
        let Some(at) = g.skeleton.clusters.get(cluster as usize).map(|c| c.at) else { return Vec::new() };
        let near = |r: &SignalRecord| r.cluster == cluster || r.area.centre.dist2(at) <= i64::from(r.area.radius).pow(2);
        let heard: Vec<TrailSignal> = g
            .signals
            .iter()
            .filter(|r| r.state < SignalState::Located && near(r))
            .filter_map(|r| match r.lead {
                Lead::Trail(s) if !skip.contains(&s) => Some(s),
                _ => None,
            })
            .collect();
        heard.into_iter().filter_map(|s| self.narrow(content, s, cluster).map(|state| (s, state))).collect()
    }

    /// The site the heard signal `s` leads to was seen: it is found, or done when `done` (it was cleared, or holds
    /// nothing to clear), and its area is its cluster. The last signal of a trail done gives the trail's reward
    /// ([`Career::reward`]). False for a signal not heard, or one that far already.
    pub fn found(&mut self, content: &Content, s: TrailSignal, done: bool) -> bool {
        let Some(g) = self.galaxy.as_mut() else { return false };
        let state = if done { SignalState::Done } else { SignalState::Found };
        let Some(k) = g.signals.iter().position(|r| r.lead == Lead::Trail(s) && r.state < state) else { return false };
        let at = g.at(g.signals[k].cluster);
        let r = &mut g.signals[k];
        r.state = state;
        r.area = spot(at);
        let last = g.skeleton.trails.get(s.trail as usize).is_some_and(|t| s.signal as usize + 1 == t.signals.len());
        if done && last {
            self.reward(content, s.trail);
        }
        true
    }

    /// The reward at the end of trail `trail` of the skeleton (galaxy plan E8, GDR-0098): its gate opens (a gate's
    /// guardian beaten), its lore fragment joins the memory log, and what its template `gives` goes into store.
    fn reward(&mut self, content: &Content, trail: u32) {
        let Some(g) = self.galaxy.as_mut() else { return };
        let Some(t) = g.skeleton.trails.get(trail as usize) else { return };
        let end = t.places.last().map(|p| p.cluster);
        if let Some(k) = end.filter(|&k| g.skeleton.clusters.get(k as usize).is_some_and(|c| c.gate)) {
            g.gates.insert(k);
        }
        if let Some(l) = t.lore.clone().filter(|l| !g.memories.contains(l)) {
            g.memories.push(l);
        }
        if let Some(gives) = content.trails.get(&t.trail).map(|x| &x.gives) {
            for (family, &x) in &gives.data {
                add_data(&mut self.data, family, x);
            }
            for (material, &n) in &gives.materials {
                add_materials(&mut self.materials, material, n);
            }
        }
    }

    /// A scan swept over the trail place `place` (trail, place): when the signal it sends is a beacon's (a source of
    /// form `Beacon`), it is heard ([`Career::hear`]). Returns it, `None` when the place sends none or it was heard
    /// already.
    pub fn beacon(&mut self, content: &Content, (trail, place): (u32, u32)) -> Option<TrailSignal> {
        let g = self.galaxy.as_ref()?;
        let source = &g.skeleton.trails.get(trail as usize)?.signals.get(place as usize)?.source;
        let s = TrailSignal { trail, signal: place };
        (content.signal_sources.get(source)?.form == SourceForm::Beacon && self.hear(s)).then_some(s)
    }

    /// Opens the gate `cluster` (its guardian beaten). False when it is no gate of the skeleton or open already.
    pub fn open_gate(&mut self, cluster: u32) -> bool {
        let Some(g) = self.galaxy.as_mut() else { return false };
        g.skeleton.clusters.get(cluster as usize).is_some_and(|c| c.gate) && g.gates.insert(cluster)
    }

    /// Keeps the slice's voyages in the galaxy (galaxy plan E7): every voyage of the content lies on its set piece's
    /// cluster (a skeleton without one for it leaves it off the map), the launched ones chart their clusters, each open
    /// voyage not cleared is a located signal, the signals of cleared ones are done, and a cleared one on a gate opens
    /// it. Called whenever voyages change and on load.
    pub(super) fn sync_slice(&mut self, content: &Content) {
        let open = self.open_voyages(content);
        let Some(g) = self.galaxy.as_mut() else { return };
        for v in content.voyages.iter() {
            let Some(k) = g.pins.get(&v.id).or_else(|| g.skeleton.pieces.get(&v.id)).copied() else { continue };
            g.pins.insert(v.id.clone(), k);
            let record = self.voyages.get(&v.id);
            if record.is_some_and(|r| r.launched > 0) {
                g.charted.entry(k).or_default();
            }
            let cleared = record.is_some_and(|r| r.cleared);
            if cleared && g.skeleton.clusters[k as usize].gate {
                g.gates.insert(k);
            }
            let lead = Lead::Voyage(v.id.clone());
            match g.signals.iter_mut().find(|s| s.lead == lead) {
                Some(s) if cleared => s.state = SignalState::Done,
                None if !cleared && open.contains(&v.id.as_str()) => {
                    let area = spot(g.at(k));
                    g.signals.push(SignalRecord { lead, cluster: k, area, state: SignalState::Located });
                }
                _ => {}
            }
        }
    }

    /// Reconciles the galaxy with the content (part of [`Career::reconcile`]): pins and signals of voyages the
    /// content no longer has go, logs and signals the skeleton doesn't have go, a cluster still underway counts as
    /// lost, and the slice is brought in line ([`Career::sync_slice`]).
    pub(super) fn reconcile_galaxy(&mut self, content: &Content, notes: &mut Vec<Problem>) {
        let Some(g) = self.galaxy.as_mut() else { return };
        let n = g.skeleton.clusters.len() as u32;
        g.pins.retain(|id, k| content.voyages.contains(id) && *k < n);
        // A skeleton from before the set pieces (E4's pins): a gate's piece makes its pinned cluster a gate.
        let gated = content.galaxies.get(&g.skeleton.galaxy).map_or(&[][..], |x| &x.set_pieces).iter().filter(|p| p.gate);
        for (p, k) in gated.filter_map(|p| Some((p, *g.pins.get(&p.voyage)?))) {
            let c = &mut g.skeleton.clusters[k as usize];
            if !c.gate {
                c.gate = true;
                let name = content.voyages.get(&p.voyage).map_or(p.voyage.as_str(), |v| v.name.as_str());
                notes.push(Problem::warning(format!("{name} keeps a gate now: clearing its goal opens the next band")).field("galaxy.skeleton"));
            }
        }
        g.memories.retain(|l| content.lore.contains(l));
        let before = (g.signals.len(), g.logs.len());
        let signals = std::mem::take(&mut g.signals);
        g.signals = signals
            .into_iter()
            .filter(|r| {
                r.cluster < n
                    && match &r.lead {
                        Lead::Trail(s) => g.knows(*s),
                        Lead::Voyage(v) => content.voyages.contains(v),
                    }
            })
            .collect();
        let logs = std::mem::take(&mut g.logs);
        g.logs = logs.into_iter().filter(|s| g.knows(*s)).collect();
        g.charted.retain(|&k, _| k < n);
        g.gates.retain(|&k| g.skeleton.clusters.get(k as usize).is_some_and(|c| c.gate));
        let dropped = before.0 - g.signals.len() + before.1 - g.logs.len();
        if dropped > 0 {
            notes.push(
                Problem::warning(format!("{dropped} signals and logs lead nowhere the galaxy knows: dropped"))
                    .field("galaxy"),
            );
        }
        if let Some(k) = g.underway.filter(|k| g.charted.contains_key(k)) {
            notes.push(
                Problem::warning(format!("the game was closed during a voyage into cluster {k}: it counts as lost"))
                    .field("galaxy.underway"),
            );
            // Underway, so it ends.
            let _ = self.cluster_ended(content, k, VoyageEnd::Lost { ticks: 0 });
        } else if let Some(g) = self.galaxy.as_mut() {
            g.underway = None;
        }
        self.sync_slice(content);
    }
}
