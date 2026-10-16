//! The galaxy in the career (save format 3; galaxy plan E4, GDR-0093, GDR-0096): the skeleton the game was generated
//! with (ADR-0035), the clusters charted, the signals heard and how far they narrowed, the logs to decode, the gates
//! opened, the jump range, and the slice's voyages pinned to clusters until E7 pins them by content.
//!
//! * **Charting**: home is charted from the start; launching into a cluster charts it (arriving is enough: the
//!   chart is knowledge, not haul). Jumps start from home or any charted cluster (GDR-0093) and reach
//!   [`Career::jump_range`] S.
//! * **Signals** ([`SignalRecord`]): a trail's signal is heard (a beacon scanned, a log decoded) with the skeleton's
//!   fresh area; a scan in another cluster halves it towards its place ([`Career::narrow`]), and once no other
//!   cluster lies inside, or a scan is taken in its own cluster, it is located.
//! * **Logs**: a data core (one hold slot, GDR-0093) brought home waits to be decoded ([`Career::decode`], free,
//!   GDR-0080); lost with the haul, the core stays in its wreck and the next launch rolls it again.
//! * **The slice** (provisional, GDR-0096): every voyage of the content is pinned to a cluster of its band, the
//!   first near home, each later one near the voyage it opens after; a launched voyage charts its cluster, an open
//!   one not cleared yet is a located signal, a cleared one's signal is done. That is also how a format 2 save is
//!   migrated ([`Career::begin_galaxy`]).
//!
//! Distances are integers in hundredths of S ([`crate::skeleton::UNITS_PER_S`]), so a save narrows the same way on
//! every platform.

use super::*;
use crate::content::model::SourceForm;
use crate::skeleton::{Area, Point, Skeleton, UNITS_PER_S};

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
    /// The content's voyages pinned to clusters (voyage id → cluster id), provisional until E7 (GDR-0096).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pins: BTreeMap<String, u32>,
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
    /// A slice voyage (content id), open and not cleared: located at its pinned cluster (GDR-0078's slice step,
    /// until E7 makes the slice a trail).
    Voyage(String),
}

/// How far a signal has come (the plan's 4.1; found, the site seen, comes with E6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SignalState {
    /// Heard: its area is the skeleton's.
    Fresh,
    /// Its area shrank (NARROWED).
    Narrowed,
    /// Its cluster is known (LOCATED).
    Located,
    /// Followed to its end: a slice voyage cleared.
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

    /// The content's voyages pinned to clusters (provisional, GDR-0096).
    pub fn pins(&self) -> &BTreeMap<String, u32> {
        &self.pins
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
    /// lowest id on a tie), `None` beyond every one of them (or without a galaxy).
    pub fn jump_from(&self, content: &Content, cluster: u32) -> Option<u32> {
        let g = self.galaxy.as_ref()?;
        let to = g.skeleton.clusters.get(cluster as usize)?.at;
        let reach = (self.jump_range(content) as f64 * UNITS_PER_S as f64) as i64;
        g.charted
            .keys()
            .map(|&k| (to.dist2(g.at(k)), k))
            .filter(|&(d2, _)| d2 <= reach * reach)
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
    /// elsewhere, its area halves and moves towards the place, which stays inside (no closer than 40 % of the new
    /// radius to its edge, the skeleton's 60 % offset), and once no other cluster lies inside, it is located. Returns
    /// the state after, `None` for a signal not heard (or done).
    pub fn narrow(&mut self, s: TrailSignal, from: u32) -> Option<SignalState> {
        let g = self.galaxy.as_mut()?;
        let k = g.signals.iter().position(|r| r.lead == Lead::Trail(s) && r.state < SignalState::Located)?;
        let target = g.signals[k].cluster;
        let at = g.at(target);
        let area = g.signals[k].area;
        let (state, area) = if from == target {
            (SignalState::Located, spot(at))
        } else {
            let radius = (area.radius / 2).max(1);
            let (dx, dy) = ((at.x - area.centre.x) as i64, (at.y - area.centre.y) as i64);
            let d = (dx * dx + dy * dy).isqrt();
            let off = i64::from(radius) * 3 / 5;
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

    /// Opens the gate `cluster` (its guardian beaten). False when it is no gate of the skeleton or open already.
    pub fn open_gate(&mut self, cluster: u32) -> bool {
        let Some(g) = self.galaxy.as_mut() else { return false };
        g.skeleton.clusters.get(cluster as usize).is_some_and(|c| c.gate) && g.gates.insert(cluster)
    }

    /// Keeps the slice's voyages in the galaxy (provisional, GDR-0096): pins every voyage of the content that has
    /// none yet, charts the clusters of the launched ones, adds a located signal for each open voyage not cleared, and
    /// marks the signals of cleared ones done. Called whenever voyages change and on load.
    pub(super) fn sync_slice(&mut self, content: &Content) {
        let open = self.open_voyages(content);
        let Some(g) = self.galaxy.as_mut() else { return };
        for v in content.voyages.iter() {
            if !g.pins.contains_key(&v.id) {
                let anchor = v.opens_after.first().and_then(|a| g.pins.get(a)).copied().unwrap_or(g.skeleton.home);
                let from = g.at(anchor);
                let taken: BTreeSet<u32> = g.pins.values().copied().collect();
                let free = (0u32..)
                    .zip(&g.skeleton.clusters)
                    .filter(|(k, c)| c.band == v.band && *k != g.skeleton.home && !taken.contains(k))
                    .map(|(k, c)| (c.at.dist2(from), k))
                    .min();
                match free {
                    Some((_, k)) => g.pins.insert(v.id.clone(), k),
                    None => continue,
                };
            }
            let k = g.pins[&v.id];
            let record = self.voyages.get(&v.id);
            if record.is_some_and(|r| r.launched > 0) {
                g.charted.entry(k).or_default();
            }
            let cleared = record.is_some_and(|r| r.cleared);
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
        g.pins.retain(|id, _| content.voyages.contains(id));
        let n = g.skeleton.clusters.len() as u32;
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
