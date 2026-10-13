//! The voyage map's pure side: where everything sits (from the content's `map` positions), what state each voyage
//! is in (from the career), the panel's words, and whether SET COURSE may launch. No Bevy systems here.
//!
//! The map is laid out on a stage of [`STAGE`] logical px (the UI scale grows it with the window, as home's and
//! FLEET's layouts). Content's `map` coordinates (0 to 1, x right, y down) land in [`MAP_RECT`], left of the panel.
//! The galaxy's centre lies off the stage to the right ([`CENTRE`]), so the band edges are arcs across the map: the
//! rim band's outer edge (the galaxy's edge) and inner edge (the gate to the next band) are placed from the content
//! positions, [`BAND_MARGIN`] outside the outermost place and inside the innermost one; the next band's inner edge
//! lies as far again inwards, at 0.6 of the rim band's depth.

use bevy::math::{Rect, Vec2};
use data::career::{Readiness, Unlocks, voyage_ready};
use data::{Blueprint, Career, Content, Voyage};

use crate::career::{fleet_cores, readiness_words};
use crate::drydock::stats::status_line;
use crate::ui_kit::Tone;

/// The layout's size (logical px).
pub const STAGE: Vec2 = Vec2::new(1280.0, 720.0);
/// The panel's left edge; the map fills the stage left of it.
pub const PANEL_X: f32 = 904.0;
/// Where the content's map coordinates (0 to 1) land.
pub const MAP_RECT: Rect = Rect { min: Vec2::new(28.0, 64.0), max: Vec2::new(876.0, 650.0) };
/// The shipyard's place on the map in content coordinates: bottom left, as in `map-b.png`. PLACEHOLDER: content has
/// no place for home yet (a start or a band could carry one).
pub const HOME_AT: (f32, f32) = (0.12, 0.86);
/// The galaxy's centre (stage px), off the stage to the right.
pub const CENTRE: Vec2 = Vec2::new(1500.0, 400.0);
/// Room between a band edge and the nearest place inside the band (stage px).
pub const BAND_MARGIN: f32 = 34.0;
/// The next band's depth as a share of the rim band's.
const NEXT_DEPTH: f32 = 0.6;
/// The bands from the rim inwards (GDR-0026). Text keys until bands are content.
pub const BANDS: [&str; 5] = ["rim", "outer", "middle", "inner", "core"];
/// A voyage whose goal encounter has this tag ends at a guardian (the map draws it as a coral diamond).
pub const GUARDIAN_TAG: &str = "boss";
/// Nodes right of this share of the map rect put their label on their left.
const LABEL_LEFT_FROM: f32 = 0.7;

/// A content map position on the stage.
pub fn to_stage(p: (f32, f32)) -> Vec2 {
    MAP_RECT.min + Vec2::new(p.0.clamp(0.0, 1.0), p.1.clamp(0.0, 1.0)) * MAP_RECT.size()
}

/// The band the voyages lie in, as circles around [`CENTRE`] (radii in stage px).
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

/// Where everything sits: a pure function of the content.
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    /// The shipyard.
    pub home: Vec2,
    /// Each voyage, in content order (the career's and the map's order).
    pub nodes: Vec<Vec2>,
    /// The chain: `(from, to)`, `None` being the shipyard: a voyage that opens after others comes from each of them,
    /// one that opens after nothing (a start opens it) from the shipyard.
    pub routes: Vec<(Option<usize>, usize)>,
    pub band: Band,
}

impl Geometry {
    pub fn new(content: &Content) -> Self {
        let home = to_stage(HOME_AT);
        let nodes: Vec<Vec2> = content.voyages.iter().map(|v| to_stage(v.map)).collect();
        let mut routes = Vec::new();
        for (k, v) in content.voyages.iter().enumerate() {
            if v.opens_after.is_empty() {
                routes.push((None, k));
            }
            for a in &v.opens_after {
                if let Some(from) = content.voyages.position(a) {
                    routes.push((Some(from), k));
                }
            }
        }
        let dist = |p: &Vec2| p.distance(CENTRE);
        let far = nodes.iter().chain([&home]).map(dist).fold(0.0f32, f32::max);
        let near = nodes.iter().map(dist).fold(far, f32::min);
        let outer = far + BAND_MARGIN;
        // Without voyages the band is as deep as the shipyard's margin allows (a placeholder depth).
        let inner = if nodes.is_empty() { outer - 400.0 } else { near - BAND_MARGIN }.max(BAND_MARGIN);
        let next = (inner - (outer - inner) * NEXT_DEPTH).max(0.0);
        let key = content.voyages.iter().next().map_or(BANDS[0], |v| v.band.as_str()).to_string();
        let next_key = BANDS.iter().position(|b| *b == key).and_then(|k| BANDS.get(k + 1)).map(|b| b.to_string());
        Self { home, nodes, routes, band: Band { centre: CENTRE, outer, inner, next, key, next_key } }
    }

    /// Does voyage `k`'s label go on its left (near the right edge of the map)?
    pub fn label_left(&self, k: usize) -> bool {
        self.nodes.get(k).is_some_and(|p| p.x > MAP_RECT.min.x + MAP_RECT.width() * LABEL_LEFT_FROM)
    }

    /// A route's two ends.
    pub fn ends(&self, route: (Option<usize>, usize)) -> (Vec2, Vec2) {
        (route.0.map_or(self.home, |k| self.nodes[k]), self.nodes[route.1])
    }

    /// Where a route's time label starts (it runs to the right from there): beside the line's middle, on its right
    /// (above a level one).
    pub fn route_label(&self, route: (Option<usize>, usize)) -> Vec2 {
        let (a, b) = self.ends(route);
        let d = (b - a).normalize_or_zero();
        let mut n = Vec2::new(-d.y, d.x);
        if n.x < -1e-3 || (n.x.abs() <= 1e-3 && n.y > 0.0) {
            n = -n;
        }
        (a + b) / 2.0 + n * 14.0
    }
}

/// A voyage's state on the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoyageState {
    /// It may launch and its goal has not been cleared yet.
    Open,
    /// Its goal was cleared on a voyage that came home (it may launch again); the quickest such voyage in ticks.
    Cleared { best: Option<u64> },
    /// Not open yet.
    Locked,
}

impl VoyageState {
    pub fn of(content: &Content, career: &Career, voyage: &str) -> VoyageState {
        match career.voyage(voyage) {
            Some(r) if r.cleared => VoyageState::Cleared { best: r.best_ticks },
            _ if career.is_open(content, voyage) => VoyageState::Open,
            _ => VoyageState::Locked,
        }
    }

    /// Every voyage's, in content order.
    pub fn all(content: &Content, career: &Career) -> Vec<VoyageState> {
        content.voyages.iter().map(|v| VoyageState::of(content, career, &v.id)).collect()
    }
}

/// Does the voyage end at a guardian (its goal's encounter is tagged [`GUARDIAN_TAG`])?
pub fn is_guardian(content: &Content, voyage: &Voyage) -> bool {
    voyage.goal().and_then(|g| content.encounters.get(&g.encounter)).is_some_and(|e| e.tags.iter().any(|t| t == GUARDIAN_TAG))
}

/// A voyage's state in words under its name: `OPEN`, `CLEARED · BEST 7:42`, `LOCKED`, `LOCKED · GUARDIAN`.
pub fn state_words(state: VoyageState, guardian: bool) -> (String, Tone) {
    let (words, tone) = match state {
        VoyageState::Open => ("OPEN".to_string(), Tone::Teal),
        VoyageState::Cleared { best: Some(t) } => (format!("CLEARED  ·  BEST {}", crate::voyage::clock(t)), Tone::Teal),
        VoyageState::Cleared { best: None } => ("CLEARED".to_string(), Tone::Teal),
        VoyageState::Locked => ("LOCKED".to_string(), Tone::Dim),
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
        status_line(content, &report).grounded.map(|why| (k, why))
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
mod tests {
    use super::*;
    use data::career::{Haul, VoyageEnd};

    fn career(content: &Content) -> Career {
        Career::new(content, "rim_awakening", 1).unwrap()
    }

    fn clear(content: &Content, c: &mut Career, voyage: &str, ticks: u64) {
        c.voyage_launched(content, voyage).unwrap();
        c.voyage_ended(content, voyage, VoyageEnd::Recalled { haul: Haul::default(), cleared: true, ticks }).unwrap();
    }

    /// Every voyage sits at its content position inside the map rect, the shipyard bottom left; the chain follows
    /// `opens_after`; every place lies inside the rim band, whose edges are arcs left of the panel.
    #[test]
    fn the_layout_comes_from_the_content_positions() {
        let content = data::base_content();
        let g = Geometry::new(&content);
        assert_eq!(g.nodes.len(), content.voyages.len());
        for (v, p) in content.voyages.iter().zip(&g.nodes) {
            assert_eq!(*p, MAP_RECT.min + Vec2::new(v.map.0, v.map.1) * MAP_RECT.size(), "{}", v.id);
            assert!(MAP_RECT.contains(*p) && p.x < PANEL_X, "{} at {p}", v.id);
        }
        let at = |id: &str| g.nodes[content.voyages.position(id).unwrap()];
        assert_eq!(at("yard_approach"), Vec2::new(28.0 + 0.28 * 848.0, 64.0 + 0.66 * 586.0));
        assert!(g.home.x < at("yard_approach").x && g.home.y > at("yard_approach").y, "the shipyard is bottom left");
        // The chain: the shipyard opens the first voyage, each next one comes from the one before it.
        let k = |id: &str| content.voyages.position(id).unwrap();
        assert_eq!(
            g.routes,
            [(None, k("yard_approach")), (Some(k("yard_approach")), k("halvex_drift")), (Some(k("halvex_drift")), k("corran_reach")), (Some(k("corran_reach")), k("wardens_gate"))]
        );
        // The band holds every place with room to spare; the next band's edge lies further in.
        let b = &g.band;
        assert_eq!((b.key.as_str(), b.next_key.as_deref()), ("rim", Some("outer")));
        for p in g.nodes.iter().chain([&g.home]) {
            let d = p.distance(b.centre);
            assert!(d >= b.inner + BAND_MARGIN - 0.01 && d <= b.outer - BAND_MARGIN + 0.01, "{p}: {d} not in {}..{}", b.inner, b.outer);
        }
        assert!(b.next < b.inner && b.inner < b.outer);
        // The inner edge crosses the map's middle line left of the panel, the warden's gate just inside it.
        let x = b.x_at(b.inner, 360.0).unwrap();
        assert!(x < PANEL_X && x > at("wardens_gate").x, "{x}");
        // Labels go left near the right edge only.
        assert!(g.label_left(k("wardens_gate")) && !g.label_left(k("yard_approach")));
        // A route's time label sits beside it, never on it.
        let r = g.routes[1];
        let (a, z) = g.ends(r);
        assert!((g.route_label(r) - (a + z) / 2.0).length() > 10.0 && g.route_label(r).x > (a.x + z.x) / 2.0);
    }

    /// A new game: the first voyage open, the rest locked; clearing it opens the next and shows its best time;
    /// the Warden's gate is the guardian.
    #[test]
    fn each_voyage_takes_its_state_from_the_career() {
        let content = data::base_content();
        let mut c = career(&content);
        assert_eq!(VoyageState::all(&content, &c), [VoyageState::Open, VoyageState::Locked, VoyageState::Locked, VoyageState::Locked]);
        clear(&content, &mut c, "yard_approach", 30 * 462);
        assert_eq!(VoyageState::all(&content, &c), [VoyageState::Cleared { best: Some(30 * 462) }, VoyageState::Open, VoyageState::Locked, VoyageState::Locked]);
        assert_eq!(state_words(VoyageState::of(&content, &c, "yard_approach"), false), ("CLEARED  ·  BEST 7:42".into(), Tone::Teal));
        let guardian: Vec<bool> = content.voyages.iter().map(|v| is_guardian(&content, v)).collect();
        assert_eq!(guardian, [false, false, false, true]);
        assert_eq!(state_words(VoyageState::Locked, true), ("LOCKED  ·  GUARDIAN".into(), Tone::Coral));
        assert_eq!(state_words(VoyageState::Open, false), ("OPEN".into(), Tone::Teal));
        // A loss doesn't clear; a recall without the goal doesn't either.
        let mut d = career(&content);
        d.voyage_launched(&content, "yard_approach").unwrap();
        d.voyage_ended(&content, "yard_approach", VoyageEnd::Lost { ticks: 10 }).unwrap();
        assert_eq!(VoyageState::of(&content, &d, "yard_approach"), VoyageState::Open);
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
        let blocker = crate::voyage::fleet_blocker(&content, &unlocks, 2, &runner, &[]);
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
