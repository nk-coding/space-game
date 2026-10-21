//! What the voyage map shows of the galaxy (galaxy plan E5, GDR-0097), from the career: the clusters known besides
//! home and the slice's voyages (charted ones, and those a located or found signal leads to), the signals heard and
//! not followed to their end (with the content's words: the trail's hint over the source's line), the jump starts
//! (home and every charted cluster) and the reach. Pure: no Bevy systems.
//!
//! * **Picks** ([`Pick`]): a voyage, a known cluster, a signal (its area: a blind jump to its middle) or a spot in
//!   unknown space (a blind jump: `Career::blind_target` picks the nearest cluster). [`Pick::target`] is the cluster
//!   a launch goes to, [`jump_blocker`] why it can't.
//! * **The dev mode** (`?dev=1`, ADR-0046; its career has the whole galaxy charted): [`Known::with_sites`] adds what
//!   each cluster holds ([`Sites`]: its encounters with their counts, from the cluster's own seed, and its notable
//!   ones, [`Notable`]); the map marks the notable ones and the cluster's panel lists every encounter.

use std::collections::BTreeMap;

use data::career::{Lead, SignalState, TrailSignal};
use data::skeleton::{Area, Point, UNITS_PER_S};
use data::{Career, Content};
use kit::ui_kit::Tone;

use super::layout::{Dot, SiteRow};

/// A cluster on the map besides home and the slice's voyages.
#[derive(Clone, Debug, PartialEq)]
pub struct ClusterMark {
    pub cluster: u32,
    pub name: String,
    pub charted: bool,
    /// One of its band's gates, opened.
    pub gate: bool,
    /// The signals of [`Known::signals`] (indices) that lead here, located or found.
    pub signals: Vec<usize>,
}

/// A signal heard and not done.
#[derive(Clone, Debug, PartialEq)]
pub struct SignalMark {
    pub signal: TrailSignal,
    /// The cluster it leads to (shown only once located).
    pub cluster: u32,
    pub state: SignalState,
    /// Its area now (map units; a located one's is its cluster).
    pub area: Area,
    /// What it leads to, the trail's hint (`A gate`), and what found it, the source's line (`a log from a hauler's
    /// wreck`), as the content writes them.
    pub hint: String,
    pub line: String,
}

impl SignalMark {
    /// Drawn as an area (fresh or narrowed), else as a mark on its cluster.
    pub fn is_area(&self) -> bool {
        self.state < SignalState::Located
    }

    /// Its state as the map writes it: `NARROWED`, `LOCATED`, `FOUND`; nothing for a fresh one.
    pub fn state_word(&self) -> &'static str {
        match self.state {
            SignalState::Fresh => "",
            SignalState::Narrowed => "NARROWED",
            SignalState::Located => "LOCATED",
            SignalState::Found => "FOUND",
            SignalState::Done => "DONE",
        }
    }

    /// Its first line: `SIGNAL · A GATE`.
    pub fn head(&self) -> String {
        format!("SIGNAL  ·  {}", self.hint.to_uppercase())
    }
}

/// What the map shows of the galaxy.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Known {
    pub clusters: Vec<ClusterMark>,
    pub signals: Vec<SignalMark>,
    /// Where jumps start: home and every charted cluster (ids, home first).
    pub starts: Vec<u32>,
    /// The jump range (S).
    pub reach: f32,
    /// The galaxy's bands from the rim inwards (`layout::Geometry::rings`' order).
    pub bands: Vec<BandMark>,
    /// The dev mode's: what each known cluster holds ([`Known::with_sites`]); empty otherwise.
    pub sites: BTreeMap<u32, Sites>,
}

/// A notable site the dev mode's map marks on its cluster (ADR-0046): a gate's guardian (the target of a band's gate
/// trail), a boss (an encounter tagged `boss`, `layout::GUARDIAN_TAG`) or a station (an encounter with an anchored
/// ship, GDR-0110).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Notable {
    Guardian,
    Boss,
    Station,
}

impl Notable {
    /// Its word on the map, after the cluster's state: `GUARDIAN`, `BOSS`, `STATION`.
    pub fn word(self) -> &'static str {
        match self {
            Notable::Guardian => "GUARDIAN",
            Notable::Boss => "BOSS",
            Notable::Station => "STATION",
        }
    }

    /// What makes the encounter `id` notable: none, one or two (the swarm's hive is a boss and a station).
    pub fn of(content: &Content, id: &str) -> Vec<Notable> {
        let Some(e) = content.encounters.get(id) else { return Vec::new() };
        let gates = content.bands.iter().filter_map(|b| b.gates.as_ref()).filter_map(|g| content.trails.get(&g.trail));
        let guardian = gates.into_iter().any(|t| t.target == id);
        let boss = !guardian && e.tags.iter().any(|t| t == super::layout::GUARDIAN_TAG);
        let station = e.ships.iter().any(|s| content.ships.get(&s.ship).is_some_and(|b| b.anchored));
        [(guardian, Notable::Guardian), (boss, Notable::Boss), (station, Notable::Station)].into_iter().filter(|x| x.0).map(|x| x.1).collect()
    }
}

/// What a cluster holds (the dev mode, ADR-0046): its interior's encounters with their counts, in the interior's
/// order (`world::voyage::cluster_encounters`), and its notable sites, in [`Notable`]'s order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sites {
    pub encounters: Vec<(String, u32)>,
    pub notable: Vec<Notable>,
}

impl Sites {
    pub fn of(content: &Content, career: &Career, k: u32) -> Sites {
        let Some(g) = career.exploration() else { return Sites::default() };
        let encounters = world::voyage::cluster_encounters(content, g.skeleton(), k);
        let mut notable: Vec<Notable> = encounters.iter().flat_map(|(id, _)| Notable::of(content, id)).collect();
        notable.sort_unstable();
        notable.dedup();
        Sites { encounters, notable }
    }

    /// The encounters as the panel's rows: a guardian or a boss with the guardian's dot and tag.
    pub fn rows(&self, content: &Content) -> Vec<SiteRow> {
        let row = |(id, n): &(String, u32)| {
            let goal = Notable::of(content, id).iter().any(|x| matches!(x, Notable::Guardian | Notable::Boss));
            super::layout::site_row(content, &data::VoyageSite { encounter: id.clone(), count: *n, goal })
        };
        self.encounters.iter().map(row).collect()
    }
}

/// A band's state: open (jumps reach into it: the first band, or a gate of the band outside it opened,
/// `Career::band_open`) or sealed, and who keeps the gates into it (the guardians' names, a set piece's first: `The
/// Warden`, `Gate sentinel`; none for the first band).
#[derive(Clone, Debug, PartialEq)]
pub struct BandMark {
    pub open: bool,
    pub guardians: Vec<String>,
}

impl BandMark {
    /// What beats its way in, in words (uppercase): `BEAT THE WARDEN OR A GATE SENTINEL`; nothing without guardians.
    pub fn beat(&self) -> Option<String> {
        let named: Vec<String> = self.guardians.iter().map(|n| if n.starts_with("The ") { n.clone() } else { format!("a {n}") }).collect();
        (!named.is_empty()).then(|| format!("BEAT {}", named.join(" or ")).to_uppercase())
    }
}

impl Known {
    pub fn of(content: &Content, career: &Career) -> Known {
        let Some(g) = career.exploration() else { return Known::default() };
        let sk = g.skeleton();
        let pinned: Vec<u32> = g.pins().values().copied().collect();
        let mut signals = Vec::new();
        for r in g.signals() {
            let Lead::Trail(s) = r.lead else { continue };
            if r.state == SignalState::Done {
                continue;
            }
            let Some(plan) = sk.trails.get(s.trail as usize) else { continue };
            let hint = content.trails.get(&plan.trail).map_or_else(|| plan.trail.clone(), |t| t.hint.clone());
            let line = plan.signals.get(s.signal as usize).and_then(|x| content.signal_sources.get(&x.source)).map_or_else(String::new, |x| x.line.clone());
            signals.push(SignalMark { signal: s, cluster: r.cluster, state: r.state, area: r.area, hint, line });
        }
        let mut clusters: Vec<ClusterMark> = Vec::new();
        for (k, c) in (0u32..).zip(&sk.clusters) {
            if k == sk.home || pinned.contains(&k) {
                continue;
            }
            let led: Vec<usize> = (0..signals.len()).filter(|&j| signals[j].cluster == k && !signals[j].is_area()).collect();
            let gate = g.gates().contains(&k);
            if g.is_charted(k) || gate || !led.is_empty() {
                clusters.push(ClusterMark { cluster: k, name: c.name.clone(), charted: g.is_charted(k), gate, signals: led });
            }
        }
        let starts = std::iter::once(sk.home).chain(g.charted().keys().copied().filter(|&k| k != sk.home)).collect();
        let keys = content.galaxies.get(&sk.galaxy).map_or(&[][..], |x| x.bands.as_slice());
        let bands = (0..keys.len())
            .map(|b| {
                let mut guardians = Vec::new();
                if let Some(outer) = b.checked_sub(1).map(|o| keys[o].as_str()) {
                    // The set pieces on the outer band's gates (the Warden's), then its gate trail's guardian.
                    let pieces = content.voyages.iter().filter(|v| g.pins().get(&v.id).is_some_and(|&k| sk.clusters.get(k as usize).is_some_and(|c| c.gate && c.band == outer)));
                    let set = pieces.filter_map(|v| v.goal()).map(|s| s.encounter.as_str());
                    let trail = content.bands.get(outer).and_then(|x| x.gates.as_ref()).and_then(|x| content.trails.get(&x.trail)).map(|t| t.target.as_str());
                    for e in set.chain(trail).filter_map(|e| content.encounters.get(e)) {
                        if !guardians.contains(&e.name) {
                            guardians.push(e.name.clone());
                        }
                    }
                }
                BandMark { open: career.band_open(content, &keys[b]), guardians }
            })
            .collect();
        Known { clusters, signals, starts, reach: career.jump_range(content), bands, sites: BTreeMap::new() }
    }

    /// The dev mode's map (ADR-0046): what each known cluster holds.
    pub fn with_sites(mut self, content: &Content, career: &Career) -> Known {
        self.sites = self.clusters.iter().map(|m| (m.cluster, Sites::of(content, career, m.cluster))).collect();
        self
    }

    /// The mark of cluster `k`, if the map shows it.
    pub fn cluster(&self, k: u32) -> Option<&ClusterMark> {
        self.clusters.iter().find(|c| c.cluster == k)
    }

    /// The signal `s`, if the map shows it.
    pub fn signal(&self, s: TrailSignal) -> Option<&SignalMark> {
        self.signals.iter().find(|m| m.signal == s)
    }

    /// The signals whose area holds the spot `p` (indices).
    pub fn areas_at(&self, p: Point) -> Vec<usize> {
        (0..self.signals.len()).filter(|&j| self.signals[j].is_area() && holds(self.signals[j].area, p)).collect()
    }
}

/// Does `area` hold the point `p`?
pub fn holds(area: Area, p: Point) -> bool {
    area.centre.dist2(p) <= i64::from(area.radius) * i64::from(area.radius)
}

/// Does a jump reach the spot `p`: within the jump range of home or a charted cluster?
pub fn in_reach(career: &Career, content: &Content, p: Point) -> bool {
    let Some(g) = career.exploration() else { return false };
    let reach = (career.jump_range(content) * UNITS_PER_S as f32) as i64;
    let sk = g.skeleton();
    std::iter::once(sk.home).chain(g.charted().keys().copied()).any(|k| sk.clusters[k as usize].at.dist2(p) <= reach * reach)
}

/// What the map's panel shows and SET COURSE launches.
#[derive(Clone, Debug, PartialEq)]
pub enum Pick {
    /// A slice voyage (content id).
    Voyage(String),
    /// A known cluster (id).
    Cluster(u32),
    /// A signal's area: a blind jump to its middle.
    Signal(TrailSignal),
    /// A spot in unknown space (map units): a blind jump.
    Spot(Point),
}

impl Pick {
    /// The spot a blind jump aims at: a spot's, a signal's middle.
    pub fn spot(&self, known: &Known) -> Option<Point> {
        match self {
            Pick::Spot(p) => Some(*p),
            Pick::Signal(s) => known.signal(*s).map(|m| m.area.centre),
            Pick::Voyage(_) | Pick::Cluster(_) => None,
        }
    }

    /// The cluster a launch goes to: a cluster's own; for a blind jump the cluster nearest its spot that a jump
    /// reaches, when the spot is in reach (`None` else, and for a voyage).
    pub fn target(&self, content: &Content, career: &Career, known: &Known) -> Option<u32> {
        match self {
            Pick::Cluster(k) => Some(*k),
            Pick::Voyage(_) => None,
            _ => self.spot(known).filter(|p| in_reach(career, content, *p)).and_then(|p| career.blind_target(content, p)),
        }
    }
}

/// Why a jump for `pick` can't launch now, in words (uppercase), if it can't: another voyage underway, a spot out of
/// reach or with no cluster near it, a band no gate is open into, beyond the jump range, then the fleet's first
/// blocker (`voyage::fleet_blocker`).
pub fn jump_blocker(content: &Content, career: &Career, known: &Known, pick: &Pick, fleet: Option<String>) -> Option<String> {
    let name = |k: u32| career.exploration().and_then(|g| g.skeleton().clusters.get(k as usize)).map_or_else(|| k.to_string(), |c| c.name.clone());
    let underway = career.underway().map(|v| content.voyages.get(v).map_or(v, |x| x.name.as_str()).to_string()).or_else(|| career.exploration()?.underway().map(name));
    if let Some(u) = underway {
        return Some(format!("{} IS STILL UNDERWAY", u.to_uppercase()));
    }
    let Some(k) = pick.target(content, career, known) else {
        return Some(match pick.spot(known) {
            Some(p) if in_reach(career, content, p) => "EMPTY SPACE  ·  NO CLUSTER NEAR THIS SPOT".into(),
            _ => "OUT OF JUMP RANGE  ·  JUMP FROM HOME OR A CHARTED CLUSTER".into(),
        });
    };
    let band = career.exploration().and_then(|g| g.skeleton().clusters.get(k as usize)).map(|c| c.band.clone()).unwrap_or_default();
    if !career.band_open(content, &band) {
        return Some("BEYOND THE GATE  ·  NO GATE INTO THIS BAND IS OPEN".into());
    }
    if career.jump_from(content, k).is_none() {
        return Some(format!("OUT OF JUMP RANGE  ·  {:.1} S", career.jump_range(content)));
    }
    fleet.map(|b| format!("NOT VOYAGE-READY  ·  {b}"))
}

/// The panel's words for a pick other than a voyage (a voyage's are `layout`'s).
#[derive(Clone, Debug, PartialEq)]
pub struct PlaceWords {
    pub head: String,
    pub title: String,
    pub chips: Vec<(String, Tone)>,
    pub summary: String,
    /// Over the rows: `SIGNALS · 1`, `INSIDE A SIGNAL'S AREA`; the rows are the signals.
    pub rows_head: String,
    pub rows: Vec<SiteRow>,
    /// Under them: the warp (`WARP ≈ 4 S · RECALL WHEN CLEAR`, a known cluster `JUMPS AT ONCE · RECALL WHEN CLEAR`).
    pub warp: String,
    /// The status line when nothing blocks.
    pub status: String,
}

/// The band (id) whose ring holds the map point `p`, from the galaxy's edge inwards (the rim's past the edge).
pub fn band_at(content: &Content, career: &Career, p: Point) -> String {
    let Some(g) = career.exploration() else { return String::new() };
    let bands = content.galaxies.get(&g.skeleton().galaxy).map_or(&[][..], |x| x.bands.as_slice());
    let r = p.distance_s(Point::default()) as f32;
    let inside = |b: &String| content.bands.get(b).is_some_and(|x| x.edge >= r);
    bands.iter().rev().find(|b| inside(b)).or(bands.first()).cloned().unwrap_or_default()
}

/// A band's key in words: `RIM`.
fn band_words(band: &str) -> String {
    band.replace('_', " ").to_uppercase()
}

/// A signal as a panel row: its hint, its state, its source's line.
fn signal_row(m: &SignalMark) -> SiteRow {
    let state = if m.state_word().is_empty() { "HEARD" } else { m.state_word() };
    SiteRow { dot: Dot::Goal, name: m.hint.clone(), tag: (state.to_string(), Tone::Gold), gives: m.line.to_uppercase() }
}

/// A cluster's name on the map: the slice voyage's pinned to it, else its own.
fn place_name<'a>(content: &'a Content, career: &'a Career, k: u32) -> &'a str {
    let Some(g) = career.exploration() else { return "" };
    let voyage = g.pins().iter().find(|&(_, &c)| c == k).and_then(|(v, _)| content.voyages.get(v));
    voyage.map_or_else(|| g.skeleton().clusters.get(k as usize).map_or("", |c| c.name.as_str()), |v| v.name.as_str())
}

/// `2.1 S FROM HOME`: how far `p` lies from the nearest jump start (other than the cluster `not` itself, unless it is
/// the only one).
fn from_words(content: &Content, career: &Career, known: &Known, p: Point, not: Option<u32>) -> Option<String> {
    let sk = career.exploration()?.skeleton();
    let (_, d, k) = known.starts.iter().map(|&k| (Some(k) == not, sk.clusters[k as usize].at.dist2(p), k)).min()?;
    let name = if k == sk.home { "HOME".to_string() } else { place_name(content, career, k).to_uppercase() };
    Some(format!("{:.1} S FROM {name}", (d as f64).sqrt() / UNITS_PER_S as f64))
}

/// The warp's line: a blind jump's `WARP ≈ 3 S · RECALL WHEN CLEAR`, a known cluster's `JUMPS AT ONCE · RECALL WHEN
/// CLEAR` (no warp since 21 Oct 2026, GDR-0097); nothing out of reach.
fn warp_words(content: &Content, career: &Career, target: Option<u32>, blind: bool) -> String {
    match target.and_then(|k| career.warp_seconds(content, k)) {
        Some(w) if blind => format!("WARP ≈ {:.0} S  ·  RECALL WHEN CLEAR", w.round()),
        Some(_) => "JUMPS AT ONCE  ·  RECALL WHEN CLEAR".into(),
        None => String::new(),
    }
}

/// The panel's words for `pick` (`None` for a voyage, or a pick the map doesn't know).
pub fn place_words(content: &Content, career: &Career, known: &Known, pick: &Pick) -> Option<PlaceWords> {
    let sk = career.exploration()?.skeleton();
    let target = pick.target(content, career, known);
    match pick {
        Pick::Voyage(_) => None,
        Pick::Cluster(k) => {
            let m = known.cluster(*k)?;
            let c = sk.clusters.get(*k as usize)?;
            let kind = content.cluster_types.get(&c.kind);
            let signals: Vec<&SignalMark> = m.signals.iter().filter_map(|&j| known.signals.get(j)).collect();
            let what = if m.gate {
                "GATE  ·  OPEN"
            } else if !signals.is_empty() {
                "SIGNAL LOCATED"
            } else {
                "CHARTED CLUSTER"
            };
            let mut chips = vec![(band_words(&c.band), Tone::Label)];
            chips.extend(kind.map(|t| (t.name.to_uppercase(), Tone::Label)));
            chips.push(if m.charted { ("CHARTED".into(), Tone::Teal) } else { ("UNCHARTED".into(), Tone::Label) });
            if !signals.is_empty() {
                chips.push(("SIGNAL".into(), Tone::Gold));
            }
            let flown = career.exploration()?.charted().get(k).map_or(0, |r| r.launched);
            // The dev mode's: every encounter it holds, then the signals.
            let sites = known.sites.get(k);
            let mut rows_head = if signals.is_empty() { String::new() } else { format!("SIGNALS  ·  {}", signals.len()) };
            if let Some(s) = sites {
                let n: u32 = s.encounters.iter().map(|e| e.1).sum();
                let head = format!("ENCOUNTERS  ·  {n}");
                rows_head = if rows_head.is_empty() { head } else { format!("{head}  ·  {rows_head}") };
            }
            let rows = sites.map(|s| s.rows(content)).unwrap_or_default().into_iter().chain(signals.iter().copied().map(signal_row)).collect();
            let status = match (signals.is_empty(), flown) {
                (false, _) => "A SIGNAL LEADS HERE  ·  SCAN TO FIND ITS SITE".to_string(),
                (true, 0) => "CHARTED  ·  THE GATE IS OPEN".to_string(),
                (true, n) => format!("FLOWN {n} ×  ·  NEW SITES EACH TIME"),
            };
            Some(PlaceWords {
                head: format!("{what}  ·  {}", from_words(content, career, known, c.at, Some(*k))?),
                title: c.name.clone(),
                chips,
                summary: kind.map_or_else(String::new, |t| content.render_plain(&t.summary)),
                rows_head,
                rows,
                warp: warp_words(content, career, target, false),
                status,
            })
        }
        Pick::Spot(p) => {
            let inside: Vec<SignalMark> = known.areas_at(*p).into_iter().map(|j| known.signals[j].clone()).collect();
            let empty = target.is_none() && in_reach(career, content, *p);
            Some(PlaceWords {
                head: format!("BLIND JUMP  ·  {}", from_words(content, career, known, *p, None)?),
                title: if empty { "Empty space".into() } else { "Unknown space".into() },
                chips: vec![(band_words(&band_at(content, career, *p)), Tone::Label), ("UNCHARTED".into(), Tone::Label)],
                summary: if empty {
                    "No cluster lies near this spot: a jump needs one to arrive at.".into()
                } else {
                    "Nobody has charted this space. A blind jump goes to the cluster nearest this spot: ordinary finds, and now and then a new signal.".into()
                },
                rows_head: if inside.is_empty() { String::new() } else { "INSIDE A SIGNAL'S AREA".into() },
                rows: inside.iter().map(signal_row).collect(),
                warp: warp_words(content, career, target, true),
                status: "A BLIND JUMP  ·  THE GATE IS OPEN".into(),
            })
        }
        Pick::Signal(s) => {
            let m = known.signal(*s)?;
            let inside = sk.clusters.iter().filter(|c| holds(m.area, c.at)).count();
            let mut line = m.line.clone();
            if let Some(first) = line.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            Some(PlaceWords {
                head: format!("SIGNAL  ·  {}", if m.state_word().is_empty() { "HEARD" } else { m.state_word() }),
                title: m.hint.clone(),
                chips: vec![(band_words(&band_at(content, career, m.area.centre)), Tone::Label), ("SIGNAL AREA".into(), Tone::Gold)],
                summary: if inside > 1 {
                    format!("{line}. It leads to one of about {inside} clusters in its area: a scan in one of them narrows it, a blind jump goes to the cluster nearest its middle.")
                } else {
                    format!("{line}. It leads to the one cluster in its area: a blind jump goes to the cluster nearest its middle.")
                },
                rows_head: String::new(),
                rows: Vec::new(),
                warp: {
                    let across = format!("{:.1} S ACROSS", f64::from(m.area.radius) * 2.0 / UNITS_PER_S as f64);
                    let warp = warp_words(content, career, target, true);
                    if warp.is_empty() { across } else { format!("{across}  ·  {warp}") }
                },
                status: "A BLIND JUMP INTO ITS AREA".into(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::career::{Haul, HaulItem, VoyageEnd};

    /// A new game knows home and the first voyage's located signal (a slice lead, not drawn here); a cluster
    /// launched into is charted and a jump start; a log decoded is a signal area with the content's words; a spot
    /// in reach picks a cluster, one out of reach none.
    #[test]
    fn the_map_knows_what_the_career_knows() {
        let content = data::base_content();
        let mut storage = world::storage::Storage::new(world::storage::MemoryBackend::default());
        let switches = world::career::Switches::from_params(&kit::params::Params::parse("")).0;
        let mut c = world::career::load(&mut storage, &content, switches, 4207, 1).career().clone();
        let sk = c.exploration().expect("a new game has its galaxy").skeleton().clone();
        let k = Known::of(&content, &c);
        assert!(k.clusters.is_empty() && k.signals.is_empty(), "{k:?}");
        assert_eq!((k.starts.as_slice(), k.reach), (&[sk.home][..], 2.5));

        // A blind jump from home: to a spot by a free rim cluster in reach.
        let home = sk.clusters[sk.home as usize].at;
        let pins: Vec<u32> = c.exploration().unwrap().pins().values().copied().collect();
        let free = (0u32..).zip(&sk.clusters).find(|(k, x)| *k != sk.home && !pins.contains(k) && x.band == "rim" && c.jump_from(&content, *k).is_some());
        let at = free.expect("a free rim cluster near home").1.at;
        let spot = Point { x: at.x + 10, y: at.y };
        let pick = Pick::Spot(spot);
        let to = pick.target(&content, &c, &k).expect("a spot near home is in reach");
        assert_ne!(to, sk.home);
        assert_eq!(jump_blocker(&content, &c, &k, &pick, None), None);
        let far = Pick::Spot(Point { x: home.x + 2000, y: home.y });
        assert!(far.target(&content, &c, &k).is_none());
        assert!(jump_blocker(&content, &c, &k, &far, None).unwrap().starts_with("OUT OF JUMP RANGE"));

        // Launch there with a data core of a log whose place is in the cluster, if one is: charted either way.
        c.cluster_launched(&content, to).unwrap();
        assert!(jump_blocker(&content, &c, &k, &pick, None).unwrap().ends_with("IS STILL UNDERWAY"));
        let log = (0u32..).zip(&sk.trails).find_map(|(i, t)| {
            let s = t.signals.first()?;
            (content.signal_sources.get(&s.source)?.form == data::SourceForm::Log).then_some((i, 0u32))
        });
        let mut haul = Haul::default();
        if let Some((trail, signal)) = log {
            haul.add(HaulItem::DataCore { trail, signal }, 1);
        }
        c.cluster_ended(&content, to, VoyageEnd::Recalled { haul, cleared: false, ticks: 30 }).unwrap();
        let k = Known::of(&content, &c);
        assert_eq!(k.cluster(to).map(|m| m.charted), Some(true));
        assert_eq!(k.starts, [sk.home, to]);
        if log.is_some() {
            c.decode(0).unwrap();
            let k = Known::of(&content, &c);
            let m = &k.signals[0];
            assert!(m.is_area() && !m.hint.is_empty() && !m.line.is_empty(), "{m:?}");
            assert_eq!((m.head(), m.state_word()), (format!("SIGNAL  ·  {}", m.hint.to_uppercase()), ""));
            assert_eq!(k.areas_at(m.area.centre), [0]);
            assert_eq!(Pick::Signal(m.signal).spot(&k), Some(m.area.centre));
            let w = place_words(&content, &c, &k, &Pick::Signal(m.signal)).unwrap();
            assert_eq!((w.head.as_str(), w.title.as_str()), ("SIGNAL  ·  HEARD", m.hint.as_str()));
            assert!(w.summary[1..].starts_with(&m.line[1..]) && w.warp.contains(" S ACROSS"), "{w:?}");
        }
        // The panel: the charted cluster, a blind jump's spot.
        let w = place_words(&content, &c, &k, &Pick::Cluster(to)).unwrap();
        assert!(w.head.starts_with("CHARTED CLUSTER  ·  ") && w.head.ends_with(" S FROM HOME"), "{}", w.head);
        assert_eq!(w.status, "FLOWN 1 ×  ·  NEW SITES EACH TIME");
        assert!(w.warp.starts_with("JUMPS AT ONCE  ·  ") && w.chips[0].0 == "RIM", "{w:?}");
        let w = place_words(&content, &c, &k, &Pick::Spot(spot)).unwrap();
        assert_eq!((w.title.as_str(), w.chips[1].0.as_str()), ("Unknown space", "UNCHARTED"));
        assert!(w.warp.starts_with("WARP ≈ "), "{}", w.warp);
        assert_eq!(place_words(&content, &c, &k, &Pick::Voyage("yard_approach".into())), None);
    }

    /// A new game has the rim open and every band inside it sealed, the outer reach's gates kept by the Warden and
    /// the gate sentinels; a rim gate opened opens the outer reach and nothing deeper.
    #[test]
    fn the_bands_open_with_their_gates() {
        let content = data::base_content();
        let mut storage = world::storage::Storage::new(world::storage::MemoryBackend::default());
        let switches = world::career::Switches::from_params(&kit::params::Params::parse("")).0;
        let mut c = world::career::load(&mut storage, &content, switches, 4207, 1).career().clone();
        let open = |k: &Known| k.bands.iter().map(|b| b.open).collect::<Vec<_>>();
        let k = Known::of(&content, &c);
        assert_eq!(open(&k), [true, false, false, false, false]);
        assert_eq!((k.bands[0].beat(), k.bands[1].beat().as_deref()), (None, Some("BEAT THE WARDEN OR A GATE SENTINEL")));
        assert_eq!(k.bands[2].beat().as_deref(), Some("BEAT A BASTION GATE"));
        let sk = c.exploration().unwrap().skeleton().clone();
        let gate = sk.band("rim").find(|&g| sk.clusters[g as usize].gate).expect("the rim has gates");
        assert!(c.open_gate(gate));
        assert_eq!(open(&Known::of(&content, &c)), [true, true, false, false, false]);
    }

    /// What makes an encounter notable on the dev mode's map: the target of a band's gate trail is a gate's guardian,
    /// another `boss` a boss, an encounter with an anchored ship a station.
    #[test]
    fn notable_sites_come_from_the_content() {
        let content = data::base_content();
        let of = |id: &str| Notable::of(&content, id);
        assert_eq!(of("gate_sentinel"), [Notable::Guardian]);
        assert_eq!(of("the_warden"), [Notable::Boss]);
        assert_eq!(of("hive_site"), [Notable::Boss, Notable::Station]);
        assert_eq!(of("precursor_outpost"), [Notable::Station]);
        assert!(of("raider_pack").is_empty() && of("nowhere").is_empty());
    }

    /// The dev mode (ADR-0046): every cluster but home and the slice's is known and charted, every band open; each
    /// lists the encounters its interior holds with their counts, gates their guardians, and its panel lists them.
    #[test]
    fn the_dev_map_knows_every_cluster_and_what_it_holds() {
        let content = data::base_content();
        let mut storage = world::storage::Storage::new(world::storage::MemoryBackend::default());
        let switches = world::career::Switches::from_params(&kit::params::Params::parse("?dev=1")).0;
        let pc = world::career::load(&mut storage, &content, switches, 4207, 1);
        let c = pc.career();
        let g = c.exploration().unwrap();
        let sk = g.skeleton();
        let k = Known::of(&content, c);
        assert!(k.sites.is_empty(), "only with_sites adds them");
        let k = k.with_sites(&content, c);
        let pinned: std::collections::BTreeSet<u32> = g.pins().values().copied().collect();
        assert_eq!(k.clusters.len(), sk.clusters.len() - 1 - pinned.len());
        assert!(k.clusters.iter().all(|m| m.charted) && k.bands.iter().all(|b| b.open));
        assert_eq!(k.sites.len(), k.clusters.len());
        let gates: Vec<u32> = k.clusters.iter().filter(|m| m.gate).map(|m| m.cluster).collect();
        assert!(!gates.is_empty());
        assert!(gates.iter().all(|x| k.sites[x].notable.contains(&Notable::Guardian)), "every gate keeps its guardian");
        assert!(k.sites.values().any(|s| s.notable.contains(&Notable::Station)));
        for (&x, s) in &k.sites {
            assert_eq!(s.encounters, world::voyage::cluster_encounters(&content, sk, x));
            let w = place_words(&content, c, &k, &Pick::Cluster(x)).unwrap();
            let n: u32 = s.encounters.iter().map(|e| e.1).sum();
            assert!(w.rows_head.starts_with(&format!("ENCOUNTERS  ·  {n}")), "{}", w.rows_head);
            assert!(w.rows.len() >= s.encounters.len());
            assert_eq!(jump_blocker(&content, c, &k, &Pick::Cluster(x), None), None, "every cluster in reach");
        }
    }
}
