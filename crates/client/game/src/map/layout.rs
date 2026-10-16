//! The voyage map's pure side: where everything sits (the career's galaxy skeleton, GDR-0096), what state each voyage
//! and each place is in (from the career), the panel's words, and whether SET COURSE may launch. No Bevy systems here.
//!
//! There is no route between voyages (GDR-0078): the map shows the shipyard, the clusters charted, the signals heard
//! and the jump range around home and every charted cluster (galaxy plan E5, GDR-0097). A voyage not open yet is
//! unknown space ([`VoyageState::Unknown`]), not drawn at all; one just found by clearing the voyages it opens after,
//! and not launched yet, is a new signal ([`VoyageState::Signal`]). Uncharted clusters are not drawn either: a blind
//! jump to a spot goes to the nearest one (`Career::blind_target`).
//!
//! The map is laid out on a stage of [`STAGE`] logical px (the UI scale grows it with the window, as home's and
//! FLEET's layouts). Galaxy px are the stage px of the default view: the skeleton's map units at [`PX_PER_S`] a S,
//! turned so the galaxy's centre lies right of home and home sits at [`HOME_AT`]. The bands' edges come from the
//! content (`bands.ron`), so they are arcs across the map; a slice voyage sits on the cluster it is pinned to.

use bevy::math::{Rect, Vec2};
use data::career::{Exploration, Lead, Readiness, TrailSignal, Unlocks, voyage_ready};
use data::skeleton::{Point, UNITS_PER_S};
use data::{Blueprint, Career, Content, Voyage};
use kit::ui_kit::Tone;
use world::career::{fleet_cores, readiness_words};
use world::player::design::flight_ready;

/// The layout's size (logical px).
pub const STAGE: Vec2 = Vec2::new(1280.0, 720.0);
/// The panel's left edge; the map fills the stage left of it.
pub const PANEL_X: f32 = 904.0;
/// The map's area left of the panel, between the header and the legend.
pub const MAP_RECT: Rect = Rect { min: Vec2::new(28.0, 64.0), max: Vec2::new(876.0, 650.0) };
/// Galaxy px per S at the default view (GDR-0097): the starting jump range (2.5 S) spans the map's height, a
/// neighbouring cluster (0.9 S) lies about 110 px away.
pub const PX_PER_S: f32 = 120.0;
/// Home's place on the stage at the default view: left of the map's middle, the bands bending away to the right; its
/// height is chosen so home, the slice's known voyages and the charted clusters sit in the map's middle ([`Geometry::new`]),
/// between these.
pub const HOME_AT: Vec2 = Vec2::new(300.0, 352.0);
const HOME_Y: (f32, f32) = (150.0, 560.0);
/// A voyage whose goal encounter has this tag ends at a guardian (the map draws it as a coral diamond).
pub const GUARDIAN_TAG: &str = "boss";
/// Places right of this share of the map rect put their label on their left.
const LABEL_LEFT_FROM: f32 = 0.7;

/// The band home lies in, as circles around the galaxy's centre (radii in galaxy px).
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    pub centre: Vec2,
    /// The band's outer edge (for the rim: the galaxy's edge).
    pub outer: f32,
    /// Its inner edge: the gate to the next band.
    pub inner: f32,
    /// The next band's inner edge (drawn fainter).
    pub next: f32,
    /// The band's key (`rim`) and the next one's (`outer`), if there is one.
    pub key: String,
    pub next_key: Option<String>,
}

impl Band {
    /// Where a circle of `radius` crosses the line `y` (its left crossing, the one on the map), if it does.
    pub fn x_at(&self, radius: f32, y: f32) -> Option<f32> {
        let dy = y - self.centre.y;
        (radius > dy.abs()).then(|| self.centre.x - (radius * radius - dy * dy).sqrt())
    }
}

/// One band of the galaxy as a ring around its centre (galaxy px; the core's inner edge is 0).
#[derive(Clone, Debug, PartialEq)]
pub struct Ring {
    pub key: String,
    pub outer: f32,
    pub inner: f32,
    /// The band home lies in (the others are uncharted).
    pub charted: bool,
}

/// Where everything sits: the content's bands and the career's skeleton, turned and scaled onto the stage.
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    /// Galaxy px per map unit, the turn (cos, sin) that puts home left of the galaxy's centre, and the flip (1 or −1
    /// on the turned y) that puts the slice's voyages above home.
    scale: f32,
    turn: Vec2,
    flip: f32,
    /// The shipyard (home's cluster).
    pub home: Vec2,
    /// Every cluster of the skeleton by id (galaxy px); empty without a galaxy.
    pub clusters: Vec<Vec2>,
    /// Each voyage, in content order (the career's and the map's order), known or not: the cluster it is pinned to
    /// (home without a pin).
    pub nodes: Vec<Vec2>,
    /// Each voyage's cluster (id), if pinned.
    pub pins: Vec<Option<u32>>,
    pub band: Band,
    /// The galaxy's bands from the rim inwards.
    pub rings: Vec<Ring>,
    /// Every signal of the skeleton's trails (the map has a label for each, shown once heard).
    pub signals: Vec<TrailSignal>,
}

impl Geometry {
    /// From the content's first galaxy and its bands, and the career's galaxy (`None`: everything at home).
    pub fn new(content: &Content, galaxy: Option<&Exploration>) -> Self {
        let scale = PX_PER_S / UNITS_PER_S as f32;
        let sk = galaxy.map(Exploration::skeleton);
        let home_at = sk.and_then(|s| s.clusters.get(s.home as usize)).map_or(Point::default(), |c| c.at);
        let home_v = Vec2::new(home_at.x as f32, home_at.y as f32);
        // Home's direction from the centre turned to point left (−x).
        let turn = if home_v.length() > 0.0 { Vec2::NEG_X.rotate(Vec2::new(home_v.x, -home_v.y).normalize()) } else { Vec2::X };
        let turned = |p: Point| turn.rotate(Vec2::new(p.x as f32, p.y as f32) - home_v) * scale;
        // The slice's voyages above home (flipped if they lie below), and the known ones (each has its signal) and
        // the charted clusters in the map's middle: the unknown ones are not drawn.
        let pinned: Vec<Vec2> = content.voyages.iter().filter_map(|v| galaxy?.pins().get(&v.id)).filter_map(|&k| sk?.clusters.get(k as usize)).map(|c| turned(c.at)).collect();
        let known = |v: &&Voyage| galaxy.is_some_and(|g| g.signals().iter().any(|r| r.lead == Lead::Voyage(v.id.clone())));
        let shown: Vec<Vec2> = content.voyages.iter().filter(known).filter_map(|v| galaxy?.pins().get(&v.id)).filter_map(|&k| sk?.clusters.get(k as usize)).map(|c| turned(c.at)).collect();
        let charted: Vec<Vec2> = galaxy.map_or_else(Vec::new, |g| g.charted().keys().filter_map(|&k| sk?.clusters.get(k as usize)).map(|c| turned(c.at)).collect());
        let flip = if pinned.iter().map(|q| q.y).sum::<f32>() > 0.0 { -1.0 } else { 1.0 };
        let (lo, hi) = shown.iter().chain(&charted).fold((0.0f32, 0.0f32), |(lo, hi), q| (lo.min(q.y * flip), hi.max(q.y * flip)));
        let home_y = (MAP_RECT.center().y - (lo + hi) / 2.0).clamp(HOME_Y.0, HOME_Y.1);
        let home_px = Vec2::new(HOME_AT.x, home_y);
        let centre = home_px + Vec2::X * home_v.length() * scale;
        let to = |p: Point| {
            let v = turn.rotate(Vec2::new(p.x as f32, p.y as f32)) * scale;
            centre + Vec2::new(v.x, v.y * flip)
        };
        let clusters: Vec<Vec2> = sk.map_or_else(Vec::new, |s| s.clusters.iter().map(|c| to(c.at)).collect());
        let pins: Vec<Option<u32>> = content.voyages.iter().map(|v| galaxy.and_then(|g| g.pins().get(&v.id).copied())).collect();
        let nodes = pins.iter().map(|p| p.and_then(|k| clusters.get(k as usize).copied()).unwrap_or(HOME_AT)).collect();
        // The bands: the galaxy's, from its edge inwards.
        let galaxy_content = sk.and_then(|s| content.galaxies.get(&s.galaxy)).or_else(|| content.galaxies.iter().next());
        let keys: Vec<String> = galaxy_content.map_or_else(Vec::new, |g| g.bands.clone());
        let edge = |k: &str| content.bands.get(k).map_or(0.0, |b| b.edge * PX_PER_S);
        let home_band = sk.and_then(|s| s.clusters.get(s.home as usize)).map_or_else(|| keys.first().cloned().unwrap_or_default(), |c| c.band.clone());
        let rings: Vec<Ring> = keys
            .iter()
            .enumerate()
            .map(|(k, key)| Ring { key: key.clone(), outer: edge(key), inner: keys.get(k + 1).map_or(0.0, |n| edge(n)), charted: *key == home_band })
            .collect();
        let at = rings.iter().position(|r| r.charted).unwrap_or(0);
        let ring = |k: usize| rings.get(k);
        let band = Band {
            centre,
            outer: ring(at).map_or(0.0, |r| r.outer),
            inner: ring(at).map_or(0.0, |r| r.inner),
            next: ring(at + 1).map_or(0.0, |r| r.inner),
            key: home_band,
            next_key: ring(at + 1).map(|r| r.key.clone()),
        };
        let signals = sk.map_or_else(Vec::new, |s| {
            (0u32..).zip(&s.trails).flat_map(|(trail, t)| (0u32..t.signals.len() as u32).map(move |signal| TrailSignal { trail, signal })).collect()
        });
        Self { scale, turn, flip, home: to(home_at), clusters, nodes, pins, band, rings, signals }
    }

    /// A skeleton point in galaxy px.
    pub fn to_galaxy(&self, p: Point) -> Vec2 {
        let v = self.turn.rotate(Vec2::new(p.x as f32, p.y as f32)) * self.scale;
        self.band.centre + Vec2::new(v.x, v.y * self.flip)
    }

    /// The skeleton point at galaxy px `g` (the nearest map unit).
    pub fn to_point(&self, g: Vec2) -> Point {
        let d = (g - self.band.centre) / self.scale;
        let v = Vec2::new(self.turn.x, -self.turn.y).rotate(Vec2::new(d.x, d.y * self.flip));
        Point { x: v.x.round() as i32, y: v.y.round() as i32 }
    }

    /// A length in map units as galaxy px.
    pub fn px(&self, units: i32) -> f32 {
        units as f32 * self.scale
    }

    /// Does a label at galaxy px `at` go on its left (near the right edge of the map)?
    pub fn label_left(&self, at: Vec2) -> bool {
        at.x > MAP_RECT.min.x + MAP_RECT.width() * LABEL_LEFT_FROM
    }
}

/// A voyage's state on the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoyageState {
    /// Found by clearing the voyages it opens after, and never launched: a new signal. Its first launch makes it
    /// [`VoyageState::Open`] (the career's launch count; nothing else is saved for it).
    Signal,
    /// It may launch and its goal has not been cleared yet.
    Open,
    /// Its goal was cleared on a voyage that came home (it may launch again); the quickest such voyage in ticks.
    Cleared { best: Option<u64> },
    /// Not open yet: unknown space, not on the map.
    Unknown,
}

impl VoyageState {
    pub fn of(content: &Content, career: &Career, voyage: &str) -> VoyageState {
        let record = career.voyage(voyage);
        let cleared = |id: &String| career.voyage(id).is_some_and(|r| r.cleared);
        let found = content.voyages.get(voyage).is_some_and(|v| !v.opens_after.is_empty() && v.opens_after.iter().all(cleared));
        match record {
            Some(r) if r.cleared => VoyageState::Cleared { best: r.best_ticks },
            _ if !career.is_open(content, voyage) => VoyageState::Unknown,
            _ if found && record.is_none_or(|r| r.launched == 0) => VoyageState::Signal,
            _ => VoyageState::Open,
        }
    }

    /// Every voyage's, in content order.
    pub fn all(content: &Content, career: &Career) -> Vec<VoyageState> {
        content.voyages.iter().map(|v| VoyageState::of(content, career, &v.id)).collect()
    }

    /// Is it on the map (found)?
    pub fn known(self) -> bool {
        self != VoyageState::Unknown
    }
}

/// Does the voyage end at a guardian (its goal's encounter is tagged [`GUARDIAN_TAG`])?
pub fn is_guardian(content: &Content, voyage: &Voyage) -> bool {
    voyage.goal().and_then(|g| content.encounters.get(&g.encounter)).is_some_and(|e| e.tags.iter().any(|t| t == GUARDIAN_TAG))
}

/// A voyage's state in words under its name: `SIGNAL`, `OPEN`, `CLEARED · BEST 7:42`, `OPEN · GUARDIAN`; nothing for
/// an unknown one (it isn't drawn).
pub fn state_words(state: VoyageState, guardian: bool) -> (String, Tone) {
    let (words, tone) = match state {
        VoyageState::Signal => ("SIGNAL".to_string(), Tone::Teal),
        VoyageState::Open => ("OPEN".to_string(), Tone::Teal),
        VoyageState::Cleared { best: Some(t) } => (format!("CLEARED  ·  BEST {}", world::voyage::clock(t)), Tone::Teal),
        VoyageState::Cleared { best: None } => ("CLEARED".to_string(), Tone::Teal),
        VoyageState::Unknown => return (String::new(), Tone::Dim),
    };
    match (guardian, state) {
        (true, VoyageState::Cleared { .. }) => (words, tone),
        (true, _) => (format!("{words}  ·  GUARDIAN"), Tone::Coral),
        (false, _) => (words, tone),
    }
}

/// The expected length: `≈ 8 MIN`, `≈ 10–15 MIN`.
pub fn minutes(v: &Voyage) -> String {
    let (a, b) = v.minutes;
    if a == b { format!("≈ {a} MIN") } else { format!("≈ {a}–{b} MIN") }
}

/// How SET COURSE stands for `voyage`: `None` it launches, else why not, in words (uppercase). In order: an unknown
/// voyage, one still underway, a locked one (which voyages to clear first), then the fleet's first blocker
/// (`voyage::fleet_blocker`: a ship that can't fly, then what keeps it from being voyage-ready).
pub fn course_blocker(content: &Content, career: &Career, voyage: &str, fleet: Option<String>) -> Option<String> {
    let name = |id: &str| content.voyages.get(id).map_or(id, |v| v.name.as_str()).to_uppercase();
    let Some(v) = content.voyages.get(voyage) else {
        return Some(format!("THERE IS NO VOYAGE `{}`", voyage.to_uppercase()));
    };
    if let Some(u) = career.underway() {
        return Some(format!("{} IS STILL UNDERWAY", name(u)));
    }
    if !career.is_open(content, voyage) {
        let missing: Vec<String> = v.opens_after.iter().filter(|a| !career.voyage(a).is_some_and(|r| r.cleared)).map(|a| name(a)).collect();
        return Some(if missing.is_empty() { "LOCKED".into() } else { format!("LOCKED  ·  CLEAR {} FIRST", missing.join(" AND ")) });
    }
    fleet.map(|b| format!("NOT VOYAGE-READY  ·  {b}"))
}

/// The panel's chips: the band (grey), the factions its ships fly for (coral), `WRECKS` (gold) when any site holds
/// wrecks, `GUARDIAN` (coral) when its goal is one.
pub fn chips(content: &Content, v: &Voyage) -> Vec<(String, Tone)> {
    let mut out = vec![(v.band.replace('_', " ").to_uppercase(), Tone::Label)];
    let encounters: Vec<_> = v.sites.iter().filter_map(|s| content.encounters.get(&s.encounter)).collect();
    for e in &encounters {
        for s in &e.ships {
            let faction = content.ships.get(&s.ship).map(|b| b.faction.as_str()).unwrap_or_default();
            let name = content.factions.get(faction).map_or(faction, |f| f.name.as_str()).to_uppercase();
            if !name.is_empty() && !out.iter().any(|(n, _)| *n == name) {
                out.push((name, Tone::Coral));
            }
        }
    }
    if encounters.iter().any(|e| !e.wrecks.is_empty()) {
        out.push(("WRECKS".into(), Tone::Gold));
    }
    if is_guardian(content, v) {
        out.push(("GUARDIAN".into(), Tone::Coral));
    }
    out
}

/// What a site is, for its dot: enemies (coral), wrecks (gold), the goal (a gold diamond), a guardian (coral).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dot {
    Enemies,
    Wrecks,
    Goal,
    Guardian,
}

impl Dot {
    pub fn tone(self) -> Tone {
        match self {
            Dot::Enemies | Dot::Guardian => Tone::Coral,
            Dot::Wrecks | Dot::Goal => Tone::Gold,
        }
    }

    pub fn diamond(self) -> bool {
        matches!(self, Dot::Goal | Dot::Guardian)
    }
}

/// One line of the panel's site list (one content entry, with its count).
#[derive(Clone, Debug, PartialEq)]
pub struct SiteRow {
    pub dot: Dot,
    /// `Raider pack ×2`.
    pub name: String,
    /// Right of the name: `3 SHIPS` (coral), `WRECKS` (gold), `GOAL · 4–5 SHIPS`, `GUARDIAN`.
    pub tag: (String, Tone),
    /// Under it: what it gives (the best parts in its wrecks) or who waits there (the ships' names).
    pub gives: String,
}

/// The dot of a site entry.
fn dot_of(content: &Content, site: &data::VoyageSite) -> Dot {
    let e = content.encounters.get(&site.encounter);
    match (site.goal, e) {
        (true, Some(e)) if e.tags.iter().any(|t| t == GUARDIAN_TAG) => Dot::Guardian,
        (true, _) => Dot::Goal,
        (false, Some(e)) if !e.wrecks.is_empty() => Dot::Wrecks,
        _ => Dot::Enemies,
    }
}

/// One dot per site (entries expanded by their count), in content order: the dots under a voyage's name.
pub fn site_dots(content: &Content, v: &Voyage) -> Vec<Dot> {
    v.sites.iter().flat_map(|s| std::iter::repeat_n(dot_of(content, s), s.count as usize)).collect()
}

/// `1 SHIP`, `3 SHIPS`, `2–3 SHIPS`.
fn ships_words(min: u32, max: u32) -> String {
    let n = if min == max { min.to_string() } else { format!("{min}–{max}") };
    if max == 1 { format!("{n} SHIP") } else { format!("{n} SHIPS") }
}

/// The parts worth most in an encounter's wrecks (their salvage data, materials counted three times), best first,
/// each once; hull parts left out (plate is everywhere).
fn best_parts(content: &Content, e: &data::Encounter, n: usize) -> Vec<String> {
    let mut parts: Vec<(f32, usize, String)> = Vec::new();
    for w in &e.wrecks {
        let Some(ship) = content.ships.get(&w.ship) else { continue };
        for p in &ship.parts {
            let Some(def) = content.parts.get(&p.part) else { continue };
            let Some(s) = def.salvage.as_ref().filter(|_| !def.hull) else { continue };
            if parts.iter().any(|q| q.2 == def.name) {
                continue;
            }
            let worth = s.data + 3.0 * s.materials.values().sum::<u32>() as f32;
            parts.push((worth, parts.len(), def.name.clone()));
        }
    }
    parts.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    parts.into_iter().take(n).map(|p| p.2.to_uppercase()).collect()
}

/// The panel's site list.
pub fn site_rows(content: &Content, v: &Voyage) -> Vec<SiteRow> {
    v.sites
        .iter()
        .map(|s| {
            let dot = dot_of(content, s);
            let e = content.encounters.get(&s.encounter);
            let base = e.map_or(s.encounter.as_str(), |e| e.name.as_str());
            let name = if s.count > 1 { format!("{base} ×{}", s.count) } else { base.to_string() };
            let (min, max) = e.map_or((0, 0), |e| e.ships.iter().fold((0, 0), |(a, b), s| (a + s.count.0, b + s.count.1)));
            let ships = ships_words(min, max);
            let tag = match dot {
                Dot::Guardian => ("GUARDIAN".to_string(), Tone::Coral),
                Dot::Goal => (format!("GOAL  ·  {ships}"), Tone::Gold),
                Dot::Wrecks => ("WRECKS".to_string(), Tone::Gold),
                Dot::Enemies => (ships, Tone::Coral),
            };
            let gives = match e {
                Some(e) if dot == Dot::Wrecks => best_parts(content, e, 3).join("  ·  "),
                Some(e) => {
                    let mut names: Vec<String> = Vec::new();
                    for s in &e.ships {
                        let n = content.ships.get(&s.ship).map_or(s.ship.as_str(), |b| b.name.as_str()).to_uppercase();
                        if !names.contains(&n) {
                            names.push(n);
                        }
                    }
                    names.join("  ·  ")
                }
                None => String::new(),
            };
            SiteRow { dot, name, tag, gives }
        })
        .collect()
}

/// One line of the voyage-ready checklist and its tone (teal: fine, gold: a warning, coral: it blocks).
#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub words: String,
    pub tone: Tone,
}

impl Check {
    fn new(words: impl Into<String>, tone: Tone) -> Self {
        Self { words: words.into(), tone }
    }

    pub fn ok(&self) -> bool {
        self.tone != Tone::Coral
    }
}

/// The voyage-ready checklist (`data::career::voyage_ready` and the design report), always four lines: every ship
/// flight-ready, only unlocked parts (else the first locked one), the cores against the mind, the cargo room (a
/// fleet without any is a gold warning: it may fly, but nothing comes home).
pub fn checklist(content: &Content, unlocks: &Unlocks, mind: u32, flagship: &Blueprint, escorts: &[Blueprint]) -> [Check; 4] {
    let grounded = std::iter::once(flagship).chain(escorts).enumerate().find_map(|(k, bp)| {
        let report = sim::design_report(content, bp, &sim::SimConfig::default());
        flight_ready(&report).err().map(|why| (k, why))
    });
    let flight = match grounded {
        Some((0, why)) => Check::new(format!("CAN'T FLY  ·  {}", why.to_uppercase()), Tone::Coral),
        Some((k, _)) => Check::new(format!("ESCORT {k} CAN'T FLY"), Tone::Coral),
        None => Check::new("FLIGHT-READY", Tone::Teal),
    };
    let list = voyage_ready(content, unlocks, mind, flagship, escorts);
    let locked = list.iter().find(|r| matches!(r, Readiness::LockedFrame { .. } | Readiness::LockedPart { .. } | Readiness::LockedMaterial { .. }));
    let parts = match locked {
        Some(r) => Check::new(readiness_words(content, r), Tone::Coral),
        None => Check::new("ONLY UNLOCKED PARTS", Tone::Teal),
    };
    let used = fleet_cores(content, flagship, escorts);
    let cores = Check::new(format!("CORES {used} / {mind}"), if used > mind { Tone::Coral } else { Tone::Teal });
    let hold: u32 = std::iter::once(flagship).chain(escorts).map(|b| content.ship_hold(b)).sum();
    let cargo = if list.contains(&Readiness::NoCargo) { Check::new("NO CARGO", Tone::Gold) } else { Check::new(format!("CARGO {hold}"), Tone::Teal) };
    [flight, parts, cores, cargo]
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use data::career::{Haul, VoyageEnd};

    fn career(content: &Content) -> Career {
        Career::new(content, "rim_awakening", 1).unwrap()
    }

    fn clear(content: &Content, c: &mut Career, voyage: &str, ticks: u64) {
        c.voyage_launched(content, voyage).unwrap();
        c.voyage_ended(content, voyage, VoyageEnd::Recalled { haul: Haul::default(), cleared: true, ticks }).unwrap();
    }

    /// A new game as the game makes it: its galaxy generated from the seed and charted.
    pub fn new_game(content: &Content) -> Career {
        let mut storage = world::storage::Storage::new(world::storage::MemoryBackend::default());
        let switches = world::career::Switches::from_params(&kit::params::Params::parse("")).0;
        world::career::load(&mut storage, content, switches, 4207, 1).career().clone()
    }

    /// Home sits at [`HOME_AT`], the galaxy's centre right of it; every cluster lands at its skeleton point turned and
    /// scaled (and back), every voyage on its pinned cluster; home lies in the rim, whose inner edge crosses the map's
    /// middle line left of the panel.
    #[test]
    fn the_layout_comes_from_the_skeleton() {
        let content = data::base_content();
        let c = new_game(&content);
        let g = Geometry::new(&content, c.exploration());
        let sk = c.exploration().unwrap().skeleton();
        assert!((g.home.x - HOME_AT.x).abs() < 1e-3 && (HOME_Y.0 - 1e-3..=HOME_Y.1 + 1e-3).contains(&g.home.y), "{}", g.home);
        assert!(g.nodes.iter().map(|n| n.y - g.home.y).sum::<f32>() <= 0.0, "the slice's voyages lie above home, on the whole");
        assert!(g.band.centre.x > g.home.x && (g.band.centre.y - g.home.y).abs() < 1e-3);
        let home = sk.clusters[sk.home as usize].at;
        for (cl, &at) in sk.clusters.iter().zip(&g.clusters) {
            assert_eq!(g.to_point(at), cl.at);
            let d = cl.at.distance_s(home) as f32 * PX_PER_S;
            assert!((at.distance(g.home) - d).abs() < 0.01, "{}", cl.name);
        }
        for (k, v) in content.voyages.iter().enumerate() {
            let pin = c.exploration().unwrap().pins()[&v.id];
            assert_eq!((g.pins[k], g.nodes[k]), (Some(pin), g.clusters[pin as usize]), "{}", v.id);
        }
        let b = &g.band;
        assert_eq!((b.key.as_str(), b.next_key.as_deref()), ("rim", Some("outer")));
        assert_eq!((b.outer, b.inner), (5.5 * PX_PER_S, 4.2 * PX_PER_S));
        assert!(b.next < b.inner && b.inner < b.outer);
        let d = g.home.distance(b.centre);
        assert!(d > b.inner && d < b.outer);
        let x = b.x_at(b.inner, g.home.y).unwrap();
        assert!(x > g.home.x && x < PANEL_X, "{x}");
        assert_eq!(g.signals.len(), sk.trails.iter().map(|t| t.signals.len()).sum::<usize>());
        // Labels go left near the right edge only.
        assert!(g.label_left(Vec2::new(800.0, 300.0)) && !g.label_left(g.home));
    }

    /// A new game: only the first voyage is known (the rest are unknown space, not on the map); clearing it finds the
    /// next as a signal, which its first launch makes an ordinary open voyage (a loss doesn't take that back); the
    /// cleared one shows its best time; the Warden's gate is the guardian.
    #[test]
    fn each_voyage_takes_its_state_from_the_career() {
        use VoyageState::{Open, Signal, Unknown};
        let content = data::base_content();
        let mut c = career(&content);
        assert_eq!(VoyageState::all(&content, &c), [Open, Unknown, Unknown, Unknown]);
        assert_eq!(VoyageState::all(&content, &c).iter().filter(|s| s.known()).count(), 1, "no locked voyage is on the map");
        clear(&content, &mut c, "yard_approach", 30 * 462);
        let cleared = VoyageState::Cleared { best: Some(30 * 462) };
        assert_eq!(VoyageState::all(&content, &c), [cleared, Signal, Unknown, Unknown]);
        assert_eq!(state_words(Signal, false), ("SIGNAL".into(), Tone::Teal));
        assert_eq!(state_words(VoyageState::of(&content, &c, "yard_approach"), false), ("CLEARED  ·  BEST 7:42".into(), Tone::Teal));
        c.voyage_launched(&content, "halvex_drift").unwrap();
        assert_eq!(VoyageState::of(&content, &c, "halvex_drift"), Open, "launched: no longer new");
        c.voyage_ended(&content, "halvex_drift", VoyageEnd::Lost { ticks: 10 }).unwrap();
        assert_eq!(VoyageState::all(&content, &c), [cleared, Open, Unknown, Unknown]);
        let guardian: Vec<bool> = content.voyages.iter().map(|v| is_guardian(&content, v)).collect();
        assert_eq!(guardian, [false, false, false, true]);
        assert_eq!(state_words(Signal, true), ("SIGNAL  ·  GUARDIAN".into(), Tone::Coral));
        assert_eq!(state_words(Open, false), ("OPEN".into(), Tone::Teal));
        assert_eq!(state_words(Unknown, false).0, "");
        // A loss doesn't clear; a recall without the goal doesn't either.
        let mut d = career(&content);
        d.voyage_launched(&content, "yard_approach").unwrap();
        d.voyage_ended(&content, "yard_approach", VoyageEnd::Lost { ticks: 10 }).unwrap();
        assert_eq!(VoyageState::all(&content, &d), [Open, Unknown, Unknown, Unknown]);
    }

    /// SET COURSE launches only an open voyage with a ready fleet, and one voyage at a time.
    #[test]
    fn set_course_is_gated_by_the_voyage_and_the_fleet() {
        let content = data::base_content();
        let mut c = career(&content);
        assert_eq!(course_blocker(&content, &c, "yard_approach", None), None);
        assert_eq!(course_blocker(&content, &c, "halvex_drift", None).as_deref(), Some("LOCKED  ·  CLEAR SHIPYARD APPROACH FIRST"));
        assert_eq!(
            course_blocker(&content, &c, "yard_approach", Some("USES BEAM LASER".into())).as_deref(),
            Some("NOT VOYAGE-READY  ·  USES BEAM LASER")
        );
        // The voyage's own lock comes before the fleet's.
        assert!(course_blocker(&content, &c, "corran_reach", Some("X".into())).unwrap().starts_with("LOCKED"));
        assert!(course_blocker(&content, &c, "nowhere", None).unwrap().contains("NO VOYAGE"));
        clear(&content, &mut c, "yard_approach", 9000);
        assert_eq!(course_blocker(&content, &c, "halvex_drift", None), None);
        assert_eq!(course_blocker(&content, &c, "yard_approach", None), None, "a cleared voyage flies again");
        c.voyage_launched(&content, "halvex_drift").unwrap();
        assert_eq!(course_blocker(&content, &c, "yard_approach", None).as_deref(), Some("HALVEX DRIFT IS STILL UNDERWAY"));
        // The fleet's own words: the starter fleet is ready; the runner uses locked parts.
        let unlocks = career(&content).unlocks(&content);
        let skiff = content.ships.get("rim_skiff").unwrap().clone();
        let checks = checklist(&content, &unlocks, 2, &skiff, &[]);
        assert!(checks.iter().all(Check::ok), "{checks:?}");
        assert_eq!(checks.iter().map(|c| c.words.as_str()).collect::<Vec<_>>()[..3], ["FLIGHT-READY", "ONLY UNLOCKED PARTS", "CORES 2 / 2"]);
        assert!(checks[3].words.starts_with("CARGO "));
        let runner = content.ships.get("halvex_runner").unwrap().clone();
        let checks = checklist(&content, &unlocks, 2, &runner, std::slice::from_ref(&skiff));
        assert!(checks[1].words.starts_with("USES ") && !checks[1].ok());
        assert!(!checks[2].ok(), "{:?}", checks[2]);
        let blocker = world::voyage::fleet_blocker(&content, &unlocks, 2, &runner, &[]);
        assert!(course_blocker(&content, &career(&content), "yard_approach", blocker).unwrap().starts_with("NOT VOYAGE-READY  ·  USES "));
    }

    /// The panel's words for Halvex Drift: chips, sites with what they give, the dots, the time.
    #[test]
    fn the_panel_describes_the_voyage_from_its_content() {
        let content = data::base_content();
        let v = content.voyages.get("halvex_drift").unwrap();
        let chips = chips(&content, v);
        assert_eq!(chips[0], ("RIM".into(), Tone::Label));
        assert!(chips.iter().any(|c| c.1 == Tone::Coral) && chips.contains(&("WRECKS".into(), Tone::Gold)), "{chips:?}");
        let rows = site_rows(&content, v);
        assert_eq!(rows.len(), 4);
        assert_eq!((rows[0].name.as_str(), rows[0].dot, rows[0].tag.0.as_str()), ("Raider pair", Dot::Enemies, "2 SHIPS"));
        assert_eq!((rows[1].dot, rows[1].tag.0.as_str()), (Dot::Wrecks, "WRECKS"));
        assert!(!rows[1].gives.is_empty() && !rows[1].gives.contains("STEEL"), "{}", rows[1].gives);
        assert_eq!((rows[3].dot, rows[3].tag.0.as_str()), (Dot::Goal, "GOAL  ·  2–3 SHIPS"));
        assert_eq!(site_dots(&content, v), [Dot::Enemies, Dot::Wrecks, Dot::Wrecks, Dot::Goal]);
        assert_eq!(minutes(v), "≈ 6–10 MIN");
        let warden = content.voyages.get("wardens_gate").unwrap();
        assert_eq!(site_rows(&content, warden).last().unwrap().dot, Dot::Guardian);
        assert!(super::chips(&content, warden).contains(&("GUARDIAN".into(), Tone::Coral)));
    }
}
