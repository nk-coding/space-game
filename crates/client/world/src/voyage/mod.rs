//! Voyages in flight (track 7H; GDR-0062, GDR-0065, GDR-0068, ADR-0033; `docs/client.md` "Voyages"): launching
//! one, what happens while it runs, and how it ends. The scene builds the voyage's world (`scene.rs`,
//! `SceneKind::Voyage`), the HUD shows it (`flight_hud/`), the debrief reports it at home (`debrief.rs`).
//!
//! * **Launch** ([`StartVoyage`] to a slice voyage or a cluster of the career's galaxy (galaxy plan E6), or
//!   `?scene=voyage&voyage=<id>` at start; `?scene=voyage&cluster=<id or signal>[&galaxy=<seed>]` flies a generated
//!   cluster of the career's galaxy, with what the career knows, as a practice run, galaxy plan E3, E4): the career
//!   records the launch (`Career::voyage_launched`, `Career::cluster_launched`: the seed, `underway`) and the save is
//!   written at once, so closing or reloading mid-voyage loses it (GDR-0062). The fleet is the drydock's working design (the flagship) and the
//!   FLEET escorts; it must be voyage-ready (`data::career::voyage_ready` against what the career grants) and every
//!   ship flight-ready (the design report). The URL launches whatever it names, for development: a voyage the
//!   career refuses (not open, a fleet that is not ready by the career's own unlocks) flies as a **practice run**,
//!   rolled from the seed the next launch would get and never written into the save.
//! * **Running** ([`VoyageRun`]): kills and ships lost are counted from the sim's `VesselLost` events, site events
//!   become banners; in a cluster of the galaxy the finds are noted ([`Find`], GDR-0097): a scan narrows the signals
//!   whose area holds the cluster (once a voyage each) and hears a beacon it sweeps over, a signal's site seen is
//!   found, cleared (or seen with nothing to clear) done; each has its banner, and the voyage's end gives them to the
//!   career, lost or not (knowledge is no haul). R or the RECALL pill asks for the recall, which starts a 3 s warp-out only when the sim allows
//!   it (`Sim::can_recall`: no armed awake enemy within 200 m, or no working gun left in the fleet: the emergency
//!   recall, GDR-0074, which the hint names). `warp_fx.rs` shows the warp-out, the arrival and the dip to dark at
//!   every end (GDR-0087).
//! * **Endings**: the warp-out ends at home with every surviving vessel's cargo as the haul
//!   (`VoyageEnd::Recalled`, the goal cleared or not); losing every vessel shows `SHIP LOST · RECALLED HOME` and
//!   ends 3 s later (`VoyageEnd::Lost`: the haul stays in the wrecks). Either way the save is written and the
//!   debrief opens at home. Leaving flight any other way while a recorded voyage runs counts as lost too.
//! * **Abandoning** (GDR-0077): M opens the ABANDON VOYAGE? panel (`flight_hud/`); its ABANDON VOYAGE (or Enter)
//!   ends the voyage as lost (`Phase::Lost { abandoned: true }`, every hold's cargo stays behind), KEEP FLYING, Esc
//!   or M again close it. The sim runs on while it is open (a real-time sim, co-op later: nothing pauses).

pub mod debrief;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::input::egui_wants_any_keyboard_input;
use data::career::{Haul, Leads, Outcome, Refusal, TrailSignal, Unlocks, VoyageEnd, VoyageResult, signal_to, voyage_ready};
use data::{Blueprint, Career, Content, SourceForm};
use data::career::SignalState;
use sim::{Owner, SimConfig, SimEvent, SiteState};
use kit::params::Params;
use kit::ui_kit::Tone;

use crate::career::{EscortDesigns, Granted, Note, PlayerCareer, readiness_words};
use crate::player::PlayerDesign;
use crate::player::design::flight_ready;
use crate::mode::AppMode;
use crate::scene::{ENEMY, Launch, PLAYER, Scene, SceneKind};
use crate::sim_bridge::{GameContent, SimSet, SimWorld};
use crate::settings::Act;
use crate::storage::Storage;
use data::input::Action;

use debrief::{Debrief, DebriefData, DebriefStats};

/// The warp-out after RECALL, and the time from losing the last vessel to home: 3 s (GDR-0062), in sim ticks.
pub const HOME_TICKS: u64 = 3 * sim::TICK_HZ as u64;

/// Launch a voyage (the voyage map's SET COURSE, after the warp for a cluster). Outside flight only; a refusal becomes
/// a note on home and a [`LaunchRefused`].
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct StartVoyage {
    pub to: Destination,
}

/// Where a launch goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Destination {
    /// A slice voyage (content id).
    Voyage(String),
    /// A cluster of the career's galaxy (id): a known one, or a blind jump's (galaxy plan E6).
    Cluster(u32),
}

/// What a voyage into a cluster of the career's galaxy found (galaxy plan E6, GDR-0097), in the order found: the
/// voyage's end gives them to the career (`Career::hear`, `narrow`, `found`), the debrief reads the difference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Find {
    /// A beacon's signal heard.
    Heard(TrailSignal),
    /// A scan in this cluster narrowed a signal.
    Narrowed(TrailSignal, u32),
    /// A signal's site seen (`true`: done, cleared or with nothing to clear).
    Found(TrailSignal, bool),
}

impl Find {
    /// Gives the find to `career`.
    pub fn apply(self, career: &mut Career, content: &Content) {
        match self {
            Find::Heard(s) => {
                career.hear(s);
            }
            Find::Narrowed(s, k) => {
                career.narrow(content, s, k);
            }
            Find::Found(s, done) => {
                career.found(content, s, done);
            }
        }
    }
}

/// A [`StartVoyage`] was refused, and why (the voyage map shows it).
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct LaunchRefused(pub String);

/// What a launch decided: the voyage, the seed it rolls from, the mind and the escorts it flies with.
#[derive(Clone, Debug, PartialEq)]
pub struct Launched {
    pub voyage: String,
    pub seed: u64,
    /// The career's mind capacity (`SimConfig::mind_capacity` of the player).
    pub mind: u32,
    /// The FLEET escorts' designs.
    pub escorts: Vec<Blueprint>,
    /// `None`: recorded in the career (its end is too). `Some(why)`: a practice run (URL only), never saved.
    pub practice: Option<String>,
    /// A generated cluster instead of a content voyage (galaxy plan E3, E4, for testing: `?scene=voyage&cluster=`);
    /// `voyage` is its name.
    pub cluster: Option<ClusterLaunch>,
}

/// A launch into a generated cluster: its id, its interior and what the launch carries from the career.
#[derive(Clone, Debug, PartialEq)]
pub struct ClusterLaunch {
    pub cluster: u32,
    pub interior: galaxy::Interior,
    pub leads: Leads,
}

/// Where a voyage is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Flying,
    /// RECALL was granted: home at this tick, with the haul.
    Warping { home_at: u64 },
    /// Every vessel is lost (or the voyage was abandoned: the ABANDON VOYAGE? panel): home at this tick, without the haul (`left`
    /// items stay behind).
    Lost { home_at: u64, left: u32, abandoned: bool },
    /// Ended (the career knows; home is next).
    Ended,
}

/// The voyage being flown (a resource; empty outside voyages).
#[derive(Resource, Default, Debug)]
pub struct VoyageRun {
    pub launched: Option<Launched>,
    pub phase: Phase,
    /// R or the RECALL pill asked for the recall; the next tick takes it.
    pub recall: bool,
    /// The ABANDON VOYAGE? panel is open (M opens it; `flight_hud/` shows it).
    pub asking: bool,
    /// The panel's ABANDON VOYAGE (or Enter): the next tick abandons the voyage (it is lost).
    pub abandon: bool,
    /// The sim allows the recall now (refreshed every tick).
    pub can_recall: bool,
    /// No vessel of the fleet has a working gun (`Sim::fleet_disarmed`): the recall is an emergency warp that armed
    /// enemies near do not block (GDR-0074; refreshed every tick).
    pub emergency: bool,
    /// Enemy vessels lost (destroyed or burned out), and the player's.
    pub kills: u32,
    pub ships_lost: u32,
    /// The last tick seen (a voyage cut short counts as lost after this long).
    pub tick: u64,
    /// A recorded voyage into a cluster: what it found so far, and the career as it would be with them (for the
    /// banners' words; the end gives the finds to the real one).
    pub finds: Vec<Find>,
    pub knows: Option<Career>,
}

impl VoyageRun {
    /// The panel's ABANDON VOYAGE: it closes, the next tick abandons the voyage.
    pub fn abandon_now(&mut self) {
        self.asking = false;
        self.abandon = true;
    }
}

pub struct VoyagePlugin;

impl Plugin for VoyagePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<StartVoyage>()
            .add_message::<LaunchRefused>()
            .init_resource::<VoyageRun>()
            .add_systems(Startup, url_launch.before(crate::scene::setup))
            .add_systems(Update, start.run_if(not(in_state(AppMode::Flight))))
            .add_systems(
                Update,
                voyage_keys.run_if(in_state(AppMode::Flight)).run_if(not(egui_wants_any_keyboard_input)),
            )
            .add_systems(FixedUpdate, voyage_tick.after(SimSet::Events).run_if(in_state(AppMode::Flight)))
            .add_systems(OnExit(AppMode::Flight), cut_short);
    }
}

/// What a launch reads and writes.
#[derive(SystemParam)]
pub(crate) struct LaunchInputs<'w> {
    career: ResMut<'w, PlayerCareer>,
    storage: ResMut<'w, Storage>,
    content: Res<'w, GameContent>,
    design: Res<'w, PlayerDesign>,
    escorts: Res<'w, EscortDesigns>,
    granted: Res<'w, Granted>,
    run: ResMut<'w, VoyageRun>,
    scene: ResMut<'w, Scene>,
}

impl LaunchInputs<'_> {
    /// Decides and records a launch (the save is written for a recorded one) and makes it the scene's next
    /// voyage; `Err`: why not, in words.
    fn launch(&mut self, voyage: Option<&str>, url: bool) -> Result<(), String> {
        let content = &self.content.0;
        let career = self.career.career();
        let Some(id) = voyage.map(str::to_string).or_else(|| default_voyage(career, content).map(str::to_string)) else {
            return Err("NO VOYAGE IS OPEN".into());
        };
        let flagship = self.design.blueprint();
        let escorts = &self.escorts.0;
        let granted = fleet_blocker(content, &self.granted.unlocks, self.granted.mind_capacity, flagship, escorts);
        // In free mode (`?unlock=all`) the drydock grants everything; the career records only fleets its own
        // unlocks allow.
        let own = if self.granted.free {
            fleet_blocker(content, &career.unlocks(content), career.mind_capacity(content), flagship, escorts)
        } else {
            granted.clone()
        };
        let mut next = career.clone();
        let (seed, practice) = decide(&mut next, content, &id, own, granted, url)?;
        if practice.is_none() {
            let ((), written) = self.career.update(&mut self.storage, |c| *c = next);
            if let Err(e) = written {
                // The launch stands (the change is in this game); the save says why not.
                warn!("voyage: the launch was not saved: {e}");
                self.career.notes.push(Note::new("NOT SAVED", e, Tone::Coral));
            }
        }
        info!("voyage: {id} launched, seed {seed}{}", practice.as_deref().map(|p| format!(" (practice: {p})")).unwrap_or_default());
        let mind = self.granted.mind_capacity;
        *self.run = VoyageRun {
            launched: Some(Launched { voyage: id.clone(), seed, mind, escorts: escorts.clone(), practice, cluster: None }),
            ..default()
        };
        self.scene.kind = SceneKind::Voyage(id);
        Ok(())
    }

    /// A launch into cluster `k` of the career's galaxy (the map's SET COURSE after the warp; galaxy plan E6): the
    /// career records it (`Career::cluster_launched`: charted, underway, the seed) and the save is written; its interior
    /// rolls with what the career knows (`Career::leads`). A fleet only `?unlock=all` allows flies it as a practice
    /// run, rolled from the seed the launch would get.
    fn launch_into(&mut self, k: u32) -> Result<(), String> {
        let content = &self.content.0;
        let flagship = self.design.blueprint();
        let escorts = &self.escorts.0;
        if let Some(b) = fleet_blocker(content, &self.granted.unlocks, self.granted.mind_capacity, flagship, escorts) {
            return Err(format!("NOT VOYAGE-READY · {b}"));
        }
        let own = self.granted.free.then(|| fleet_blocker(content, &self.career.career().unlocks(content), self.career.career().mind_capacity(content), flagship, escorts)).flatten();
        let mut next = self.career.career().clone();
        let sk = next.exploration().ok_or("THE CAREER HAS NO GALAXY")?.skeleton().clone();
        let name = sk.clusters.get(k as usize).ok_or_else(|| format!("THE GALAXY HAS {} CLUSTERS", sk.clusters.len()))?.name.clone();
        let interior = galaxy::interior(content, &sk, k).map_err(|e| e.0.to_uppercase())?;
        let leads = next.leads(content, k);
        let (seed, practice) = match own {
            Some(b) => (next.cluster_seed(k), Some(format!("NOT VOYAGE-READY · {b}"))),
            None => (next.cluster_launched(content, k).map_err(|r| refusal_words(content, &r))?, None),
        };
        if practice.is_none() {
            let ((), written) = self.career.update(&mut self.storage, |c| *c = next.clone());
            if let Err(e) = written {
                warn!("voyage: the launch was not saved: {e}");
                self.career.notes.push(Note::new("NOT SAVED", e, Tone::Coral));
            }
        }
        info!("voyage: cluster {k} ({name}) launched, seed {seed}{}", practice.as_deref().map(|p| format!(" (practice: {p})")).unwrap_or_default());
        *self.run = VoyageRun {
            knows: practice.is_none().then_some(next),
            launched: Some(Launched {
                voyage: name.clone(),
                seed,
                mind: self.granted.mind_capacity,
                escorts: escorts.clone(),
                practice,
                cluster: Some(ClusterLaunch { cluster: k, interior, leads }),
            }),
            ..default()
        };
        self.scene.kind = SceneKind::Voyage(name);
        Ok(())
    }

    /// A practice run into a generated cluster (galaxy plan E3, E4; [`url_launch`]): cluster `cluster` (`signal`: the
    /// first that holds a trail's place) of the career's galaxy, rolled from the seed its next launch would get and
    /// carrying what the career knows (`Career::leads`: what a recorded launch carries). With `galaxy`, the base
    /// galaxy generated from that seed instead, rolled from the cluster's own seed, every trail place a signal's site
    /// and every log's data core in place ([`every_lead`]). Never saved.
    fn launch_cluster(&mut self, galaxy: Option<u64>, cluster: &str) -> Result<(), String> {
        let content = &self.content.0;
        let flagship = self.design.blueprint();
        if let Some(b) = fleet_blocker(content, &self.granted.unlocks, self.granted.mind_capacity, flagship, &self.escorts.0) {
            return Err(format!("NOT VOYAGE-READY · {b}"));
        }
        let career = self.career.career();
        let own = career.exploration().filter(|_| galaxy.is_none()).map(|g| g.skeleton().clone());
        let sk = match own.clone() {
            Some(sk) => sk,
            None => {
                let id = content.galaxies.iter().next().map(|g| g.id.clone()).ok_or("THE CONTENT HAS NO GALAXY")?;
                galaxy::generate(content, &id, galaxy.unwrap_or_else(|| career.seed())).map_err(|e| e.0.to_uppercase())?
            }
        };
        let k = match cluster.parse::<u32>() {
            Ok(k) => k,
            Err(_) => sk.trails.iter().flat_map(|t| &t.places).map(|p| p.cluster).find(|&k| k != sk.home).ok_or("NO CLUSTER HOLDS A TRAIL'S PLACE")?,
        };
        let c = sk.clusters.get(k as usize).ok_or_else(|| format!("THE GALAXY HAS {} CLUSTERS", sk.clusters.len()))?;
        let interior = galaxy::interior(content, &sk, k).map_err(|e| e.0.to_uppercase())?;
        let (seed, leads) = match own {
            Some(_) => (career.cluster_seed(k), career.leads(content, k)),
            None => (c.seed, every_lead(content, &sk, &interior)),
        };
        info!("voyage: cluster {k} ({}) of galaxy seed {} launched (practice)", c.name, sk.seed);
        *self.run = VoyageRun {
            launched: Some(Launched {
                voyage: c.name.clone(),
                seed,
                mind: self.granted.mind_capacity,
                escorts: self.escorts.0.clone(),
                practice: Some("A GENERATED CLUSTER, FOR TESTING".into()),
                cluster: Some(ClusterLaunch { cluster: k, interior, leads }),
            }),
            ..default()
        };
        self.scene.kind = SceneKind::Voyage(c.name.clone());
        Ok(())
    }
}

/// Everything a cluster's interior holds, as leads (a practice run into another galaxy, [`LaunchInputs::launch_cluster`]):
/// every trail place a signal's site, every log's place with its data core.
pub fn every_lead(content: &Content, sk: &galaxy::Skeleton, interior: &galaxy::Interior) -> Leads {
    let places: Vec<(u32, u32)> = interior.sites.iter().filter_map(|s| s.place).collect();
    let log = |&(i, j): &(u32, u32)| {
        let signal = sk.trails.get(i as usize).and_then(|t| t.signals.get(j as usize));
        signal.and_then(|x| content.signal_sources.get(&x.source)).is_some_and(|x| x.form == SourceForm::Log)
    };
    Leads { cores: places.iter().copied().filter(log).collect(), signals: places }
}

/// The voyage the map selects when it opens: the first open one whose goal is not cleared, else the first open one.
pub fn default_voyage<'c>(career: &Career, content: &'c Content) -> Option<&'c str> {
    let open = career.open_voyages(content);
    open.iter().find(|v| !career.voyage(v).is_some_and(|r| r.cleared)).or(open.first()).copied()
}

/// The first reason the fleet may not fly a voyage, in words: a ship that can't fly (the design report), then
/// `data::career::voyage_ready`'s first blocker; `None`: ready (the no-cargo warning doesn't block).
pub fn fleet_blocker(content: &Content, unlocks: &Unlocks, mind: u32, flagship: &Blueprint, escorts: &[Blueprint]) -> Option<String> {
    for (k, bp) in std::iter::once(flagship).chain(escorts).enumerate() {
        let report = sim::design_report(content, bp, &SimConfig::default());
        if let Err(why) = flight_ready(&report) {
            let who = if k == 0 { String::new() } else { format!("ESCORT {k} ") };
            return Some(format!("{who}CAN'T FLY · {}", why.to_uppercase()));
        }
    }
    voyage_ready(content, unlocks, mind, flagship, escorts).iter().find(|r| r.blocks()).map(|r| readiness_words(content, r))
}

/// Decides a launch of `voyage` on `career` (changed only when it is recorded): `Ok((seed, None))` recorded,
/// `Ok((seed, Some(why)))` a practice run (only from the URL: `url`), `Err(why)` refused. `own`: the fleet's
/// blocker by the career's own unlocks; `granted`: by what the drydock grants (the same unless `?unlock=all`).
pub fn decide(
    career: &mut Career,
    content: &Content,
    voyage: &str,
    own: Option<String>,
    granted: Option<String>,
    url: bool,
) -> Result<(u64, Option<String>), String> {
    if content.voyages.get(voyage).is_none() {
        return Err(format!("THERE IS NO VOYAGE `{}`", voyage.to_uppercase()));
    }
    if !url && let Some(b) = &granted {
        return Err(format!("NOT VOYAGE-READY · {b}"));
    }
    if let Some(b) = own.or(granted) {
        return Ok((career.voyage_seed(voyage), Some(format!("NOT VOYAGE-READY · {b}"))));
    }
    match career.voyage_launched(content, voyage) {
        Ok(seed) => Ok((seed, None)),
        Err(r) if url => Ok((career.voyage_seed(voyage), Some(refusal_words(content, &r)))),
        Err(r) => Err(refusal_words(content, &r)),
    }
}

/// A refused launch in words.
fn refusal_words(content: &Content, r: &Refusal) -> String {
    let name = |id: &str| content.voyages.get(id).map_or(id, |v| v.name.as_str()).to_uppercase();
    match r {
        Refusal::NotOpen(v) => format!("{} IS NOT OPEN YET", name(v)),
        Refusal::Underway(v) => format!("{} IS STILL UNDERWAY", name(v)),
        Refusal::OutOfReach(_) => "OUT OF JUMP RANGE".into(),
        Refusal::BandClosed(_) => "BEYOND THE GATE  ·  NO GATE INTO THIS BAND IS OPEN".into(),
        other => format!("{other:?}").to_uppercase(),
    }
}

/// `?scene=voyage[&voyage=<id>]`: launch at once (a practice run when the career refuses it); without any voyage
/// to fly, home with a note. `?scene=voyage&cluster=<id or signal>[&galaxy=<seed>]`: a practice run into a generated
/// cluster ([`LaunchInputs::launch_cluster`]).
fn url_launch(params: Res<Params>, mut inputs: LaunchInputs, mut next: ResMut<NextState<AppMode>>) {
    if params.text("scene") != Some("voyage") {
        return;
    }
    let launched = match params.text("cluster") {
        Some(k) => inputs.launch_cluster(params.u64("galaxy"), k),
        None => inputs.launch(params.text("voyage"), true),
    };
    if let Err(why) = launched {
        warn!("voyage: {why}");
        inputs.career.notes.push(Note::new("NO VOYAGE", why, Tone::Coral));
        next.set(AppMode::Home);
    }
}

/// A [`StartVoyage`] (from the voyage map): launch, or say why not.
fn start(
    mut requests: MessageReader<StartVoyage>,
    mut inputs: LaunchInputs,
    mut next: ResMut<NextState<AppMode>>,
    mut refused: MessageWriter<LaunchRefused>,
) {
    let Some(request) = requests.read().last().cloned() else { return };
    let launched = match &request.to {
        Destination::Voyage(id) => inputs.launch(Some(id), false),
        Destination::Cluster(k) => inputs.launch_into(*k),
    };
    match launched {
        Ok(()) => {
            inputs.scene.launch = Some(Launch::Voyage);
            next.set(AppMode::Flight);
        }
        Err(why) => {
            info!("voyage: not launched: {why}");
            inputs.career.notes.retain(|n| n.head != "CAN'T LAUNCH");
            inputs.career.notes.insert(0, Note::new("CAN'T LAUNCH", why.clone(), Tone::Coral));
            refused.write(LaunchRefused(why));
        }
    }
}

/// R asks for the recall (the RECALL pill does the same, `flight_hud/`). M opens the ABANDON VOYAGE? panel and
/// closes it again, Esc closes it, Enter abandons the voyage (like the panel's button; it counts as lost, like
/// closing the game: the way out of a stalemate).
fn voyage_keys(keys: Res<ButtonInput<KeyCode>>, act: Act, mut run: ResMut<VoyageRun>, scene: Res<Scene>) {
    if run.launched.is_none() || scene.voyage().is_none() || !matches!(run.phase, Phase::Flying | Phase::Warping { .. }) {
        return;
    }
    if act.just_pressed(Action::Recall) {
        run.recall = true;
    }
    if act.just_pressed(Action::Leave) {
        run.asking = !run.asking;
    } else if run.asking && keys.just_pressed(KeyCode::Escape) {
        run.asking = false;
    } else if run.asking && keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
        run.abandon_now();
    }
}

/// The haul item of a cargo item (`data` doesn't depend on `sim`).
/// What `owner`'s vessels carry (dormant and dark ones too): the haul a recall brings home.
pub fn fleet_haul(sim: &sim::Sim, owner: Owner) -> Haul {
    sim.vessels(owner)
        .filter_map(|v| sim.ship(v.ship))
        .flat_map(|s| s.cargo().items().map(|(item, stack)| (item.haul(), stack.count)))
        .collect()
}

/// Items in every hold of `owner`'s: what abandoning a voyage leaves behind.
pub fn all_cargo(sim: &sim::Sim, owner: Owner) -> u32 {
    sim.ships().iter().filter(|s| s.owner() == owner).flat_map(|s| s.cargo().items()).map(|(_, st)| st.count).sum()
}

/// Items left in `owner`'s wrecks (ships of theirs that are no vessel any more): what a lost voyage leaves.
pub fn left_behind(sim: &sim::Sim, owner: Owner) -> u32 {
    sim.ships().iter().filter(|s| s.owner() == owner && !s.is_vessel()).flat_map(|s| s.cargo().items()).map(|(_, st)| st.count).sum()
}

/// Sites done and sites in all (`SITES 2 / 5`): cleared ones, and wreck-only (quiet) ones once they have spawned.
pub fn sites_done(voyage: &sim::Voyage) -> (u32, u32) {
    let done = voyage.sites().iter().filter(|s| matches!(s.state(), SiteState::Cleared | SiteState::Quiet)).count();
    (done as u32, voyage.sites().len() as u32)
}

/// Sim ticks as `m:ss` (`7:05`).
pub fn clock(ticks: u64) -> String {
    let s = ticks / sim::TICK_HZ as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The voyage's title: `SHIPYARD APPROACH · RIM`.
pub fn title(content: &Content, voyage: &str) -> String {
    match content.voyages.get(voyage) {
        Some(v) => format!("{}  ·  {}", v.name.to_uppercase(), v.band.replace('_', " ").to_uppercase()),
        None => voyage.to_uppercase(),
    }
}

/// A site's name, uppercase: its encounter's (`RAIDER PACK`), `BEACON` for a beacon alone (a trail's place with no
/// encounter).
pub fn encounter_name(content: &Content, encounter: Option<&str>) -> String {
    match encounter {
        Some(id) => content.encounters.get(id).map_or(id, |e| e.name.as_str()).to_uppercase(),
        None => "BEACON".into(),
    }
}

/// The RECALL pill and the line under it: its text, the line, and whether it can be pressed. `emergency`: the fleet
/// has no working gun, so the recall is the emergency warp (GDR-0074).
pub fn recall_words(phase: Phase, can_recall: bool, emergency: bool, tick: u64) -> (String, String, bool) {
    match phase {
        Phase::Flying if can_recall && emergency => {
            ("RECALL".into(), "EMERGENCY RECALL  ·  NO WORKING GUN  ·  WARP HOME 3 S".into(), true)
        }
        Phase::Flying if can_recall => ("RECALL".into(), "WARP HOME 3 S  ·  KEEPS THE HAUL".into(), true),
        Phase::Flying => ("RECALL".into(), "ARMED ENEMIES NEAR  ·  NO RECALL".into(), false),
        Phase::Warping { home_at } => ("WARPING".into(), format!("HOME IN {} S", seconds_left(home_at, tick)), false),
        Phase::Lost { .. } | Phase::Ended => ("RECALL".into(), String::new(), false),
    }
}

/// Whole seconds until `at` (rounded up).
pub fn seconds_left(at: u64, tick: u64) -> u64 {
    at.saturating_sub(tick).div_ceil(sim::TICK_HZ as u64)
}

/// A site event as a banner: its title, the line under it and its tone.
pub fn site_banner(content: &Content, voyage: &sim::Voyage, event: &SimEvent) -> Option<(String, String, Tone)> {
    let (site, cleared, goal) = match *event {
        SimEvent::SiteWoke { site } => (site, false, false),
        SimEvent::SiteCleared { site, goal } => (site, true, goal),
        _ => return None,
    };
    let s = voyage.sites().get(site as usize)?;
    let name = encounter_name(content, s.encounter());
    let (done, all) = sites_done(voyage);
    Some(match (cleared, goal) {
        (false, _) => (format!("{name}  ·  AWAKE"), format!("SITES {done} / {all}  ·  THEY ARE COMING"), Tone::Coral),
        (true, false) => (format!("SITE CLEARED  ·  {name}"), format!("SITES {done} / {all}  ·  THE WRECKS ARE YOURS"), Tone::Teal),
        (true, true) => (format!("GOAL CLEARED  ·  {name}"), format!("SITES {done} / {all}  ·  RECALL WHEN CLEAR"), Tone::Gold),
    })
}

/// The player's scan this tick as a banner (galaxy plan E3): how many sites it found and how many bearings it gave
/// (`SimEvent::Scanned`, `SiteRevealed`, `SiteBearing`); `None` without a sweep of `owner`'s.
pub fn scan_banner(sim: &sim::Sim, owner: Owner) -> Option<(String, String, Tone)> {
    let mut range = None;
    let (mut found, mut bearings) = (0, 0);
    for e in sim.events() {
        match *e {
            SimEvent::Scanned { ship, range: r, .. } if sim.ship(ship).is_some_and(|s| s.owner() == owner) => range = Some(r),
            SimEvent::SiteRevealed { .. } => found += 1,
            SimEvent::SiteBearing { .. } => bearings += 1,
            _ => {}
        }
    }
    let range = range?;
    let plural = |n: u32, one: &str, many: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {many}") };
    let title = if found == 0 { "SCAN  ·  NOTHING NEW".to_string() } else { format!("SCAN  ·  {} FOUND", plural(found, "SITE", "SITES")) };
    let sub = if bearings > 0 {
        format!("{} TOWARDS A SIGNAL  ·  SCAN AGAIN ELSEWHERE TO CROSS IT", plural(bearings, "BEARING", "BEARINGS"))
    } else {
        format!("SWEPT {:.1} KM AROUND", range / 1000.0)
    };
    Some((title, sub, if bearings > 0 { Tone::Gold } else { Tone::Teal }))
}

/// A signal of the career's galaxy in words: what it leads to, uppercase (`A GATE`, the trail's hint), and what found
/// it (`a log from a hauler's wreck`, the source's line), as the content writes them.
pub fn signal_words(content: &Content, career: &Career, s: TrailSignal) -> (String, String) {
    let plan = career.exploration().and_then(|g| g.skeleton().trails.get(s.trail as usize));
    let hint = plan.map_or_else(String::new, |p| content.trails.get(&p.trail).map_or_else(|| p.trail.clone(), |t| t.hint.clone()));
    let line = plan.and_then(|p| p.signals.get(s.signal as usize)).and_then(|x| content.signal_sources.get(&x.source)).map_or_else(String::new, |x| x.line.clone());
    (hint.to_uppercase(), line)
}

/// What following the signal `s` to its end gave, when it was the last of its trail (galaxy plan E8, GDR-0098), as a
/// banner's words: `GATE OPEN · THE OUTER REACH`, `CACHE OPENED` over what went into store, `A MEMORY · THE LAST
/// LAUNCH` (read at the LOG), `SIGNAL FOLLOWED · A GRAVEYARD` (salvage it).
pub fn reward_words(content: &Content, career: &Career, s: TrailSignal) -> Option<(String, String, Tone)> {
    let g = career.exploration()?;
    let plan = g.skeleton().trails.get(s.trail as usize).filter(|t| s.signal as usize + 1 == t.signals.len())?;
    let trail = content.trails.get(&plan.trail)?;
    Some(match trail.reward {
        data::Reward::Gate => {
            let bands = &content.galaxies.get(&g.skeleton().galaxy)?.bands;
            let next = bands.iter().position(|b| *b == plan.band).and_then(|k| bands.get(k + 1)).and_then(|b| content.bands.get(b));
            (format!("GATE OPEN  ·  {}", next.map_or("THE NEXT BAND".into(), |b| b.name.to_uppercase())), "ITS GUARDIAN IS BEATEN  ·  THE BAND BEYOND IS IN REACH".into(), Tone::Coral)
        }
        data::Reward::Cache => ("CACHE OPENED".into(), gives_words(content, &trail.gives), Tone::Teal),
        data::Reward::Lore => {
            let name = plan.lore.as_ref().and_then(|l| content.lore.get(l)).map_or_else(String::new, |l| l.name.to_uppercase());
            (format!("A MEMORY  ·  {name}"), "READ IT AT THE LOG".into(), Tone::Gold)
        }
        data::Reward::Salvage => (format!("SIGNAL FOLLOWED  ·  {}", trail.hint.to_uppercase()), "FAR MORE THAN USUAL TO SALVAGE HERE".into(), Tone::Gold),
    })
}

/// What a trail's `gives` puts into store, in words: `LOGIC +8  ·  PROGRAMMABLE MATTER +4`.
pub fn gives_words(content: &Content, gives: &data::ResearchCost) -> String {
    let data = gives.data.iter().map(|(f, x)| format!("{} +{}", content.families.get(f).map_or(f.as_str(), |x| x.name.as_str()).to_uppercase(), debrief::amount(*x)));
    let materials = gives.materials.iter().map(|(m, n)| format!("{} +{n}", content.materials.get(m).map_or(m.as_str(), |x| x.name.as_str()).to_uppercase()));
    data.chain(materials).collect::<Vec<_>>().join("  ·  ")
}

/// A recorded cluster voyage's finds this tick ([`Find`]), each given to the run's copy of the career at once, and the
/// banner of the last one.
pub fn note_finds(run: &mut VoyageRun, sim: &sim::Sim, content: &Content) -> Option<(String, String, Tone)> {
    let VoyageRun { launched, finds, knows, .. } = run;
    let (Some(c), Some(knows), Some(v)) = (launched.as_ref().and_then(|l| l.cluster.as_ref()), knows.as_mut(), sim.voyage()) else {
        return None;
    };
    let place = |site: u32| c.interior.sites.get(site as usize).and_then(|s| s.place);
    let mut banner = None;
    for e in sim.events() {
        match *e {
            SimEvent::Scanned { ship, at, range, .. } if sim.ship(ship).is_some_and(|s| s.owner() == PLAYER) => {
                let skip: Vec<TrailSignal> = finds.iter().filter_map(|f| if let Find::Narrowed(s, _) = f { Some(*s) } else { None }).collect();
                for (s, state) in knows.scanned(content, c.cluster, &skip) {
                    finds.push(Find::Narrowed(s, c.cluster));
                    let (hint, _) = signal_words(content, knows, s);
                    let here = knows.exploration().and_then(|g| g.signal(s)).is_some_and(|r| r.cluster == c.cluster);
                    banner = Some(match state {
                        SignalState::Narrowed => (format!("SIGNAL NARROWED  ·  {hint}"), "ITS AREA ON THE MAP SHRANK TOWARDS ITS PLACE".into(), Tone::Gold),
                        _ if here => (format!("SIGNAL LOCATED  ·  {hint}"), "IT LEADS INTO THIS CLUSTER  ·  FOLLOW ITS BEARING".into(), Tone::Gold),
                        _ => (format!("SIGNAL LOCATED  ·  {hint}"), "ITS CLUSTER IS ON THE MAP".into(), Tone::Gold),
                    });
                }
                for (k, site) in (0u32..).zip(v.sites()) {
                    if let Some(p) = place(k).filter(|_| (site.pos() - at).length() <= range)
                        && let Some(s) = knows.beacon(content, p)
                    {
                        finds.push(Find::Heard(s));
                        let (hint, line) = signal_words(content, knows, s);
                        banner = Some((format!("SIGNAL HEARD  ·  {hint}"), line.to_uppercase(), Tone::Gold));
                    }
                }
            }
            SimEvent::SiteRevealed { site } | SimEvent::SiteCleared { site, .. } => {
                let cleared = matches!(e, SimEvent::SiteCleared { .. });
                let Some(st) = v.sites().get(site as usize).filter(|x| x.signal()) else { continue };
                let Some(s) = place(site).and_then(signal_to) else { continue };
                let done = cleared || st.plan().ships.is_empty();
                if knows.found(content, s, done) {
                    finds.push(Find::Found(s, done));
                    if let Some(words) = done.then(|| reward_words(content, knows, s)).flatten() {
                        banner = Some(words);
                    } else if !cleared {
                        let (hint, _) = signal_words(content, knows, s);
                        let sub = if done { "FOLLOWED TO ITS END".to_string() } else { format!("{}  ·  CLEAR IT", encounter_name(content, st.encounter())) };
                        banner = Some((format!("SIGNAL FOUND  ·  {hint}"), sub, Tone::Gold));
                    }
                }
            }
            _ => {}
        }
    }
    banner
}

/// Every tick of a voyage: counts, banners, the finds, the recall, the endings.
#[allow(clippy::too_many_arguments)]
fn voyage_tick(
    mut run: ResMut<VoyageRun>,
    mut scene: ResMut<Scene>,
    world: Res<SimWorld>,
    mut career: ResMut<PlayerCareer>,
    mut storage: ResMut<Storage>,
    content: Res<GameContent>,
    mut debrief: ResMut<Debrief>,
    mut next: ResMut<NextState<AppMode>>,
) {
    if run.launched.is_none() || run.phase == Phase::Ended || scene.voyage().is_none() {
        return;
    }
    let sim = &world.sim;
    let (tick, now) = (sim.tick(), sim.time_seconds());
    run.tick = tick;
    for e in sim.events() {
        match e {
            SimEvent::VesselLost { owner, .. } if *owner == ENEMY => run.kills += 1,
            SimEvent::VesselLost { owner, .. } if *owner == PLAYER => run.ships_lost += 1,
            _ => {}
        }
        if let Some(v) = sim.voyage()
            && let Some((title, sub, tone)) = site_banner(&content.0, v, e)
            && matches!(run.phase, Phase::Flying)
        {
            scene.say(title, sub, tone, now + 3.5);
        }
    }
    if let Some((title, sub, tone)) = scan_banner(sim, PLAYER)
        && matches!(run.phase, Phase::Flying)
    {
        scene.say(title, sub, tone, now + 3.5);
    }
    if let Some((title, sub, tone)) = note_finds(&mut run, sim, &content.0)
        && matches!(run.phase, Phase::Flying)
    {
        scene.say(title, sub, tone, now + 4.0);
    }
    let alive = sim.vessels(PLAYER).next().is_some();
    run.can_recall = alive && sim.can_recall(PLAYER);
    run.emergency = alive && sim.fleet_disarmed(PLAYER);
    let asked = std::mem::take(&mut run.recall);
    let abandon = std::mem::take(&mut run.abandon);
    match run.phase {
        Phase::Flying | Phase::Warping { .. } if !alive => {
            // Losing every vessel wins over a warp-out underway.
            let left = left_behind(sim, PLAYER);
            run.phase = Phase::Lost { home_at: tick + HOME_TICKS, left, abandoned: false };
            run.asking = false;
            scene.banner = None;
        }
        Phase::Flying | Phase::Warping { .. } if abandon => {
            run.phase = Phase::Lost { home_at: tick + HOME_TICKS, left: all_cargo(sim, PLAYER), abandoned: true };
            scene.banner = None;
        }
        Phase::Flying if asked => {
            if run.can_recall {
                run.phase = Phase::Warping { home_at: tick + HOME_TICKS };
                let title = if run.emergency { "EMERGENCY WARP" } else { "WARPING HOME" };
                scene.say(title.into(), "THE HAUL COMES WITH YOU".into(), Tone::Teal, now + 3.0);
            } else {
                scene.say("NO RECALL".into(), "ARMED ENEMIES WITHIN 200 M".into(), Tone::Coral, now + 2.0);
            }
        }
        Phase::Warping { home_at } if tick >= home_at => {
            let haul = fleet_haul(sim, PLAYER);
            let cleared = sim.voyage().is_some_and(|v| v.goal_cleared());
            end(&mut run, &world, &mut career, &mut storage, &content.0, &mut debrief, VoyageEnd::Recalled { haul, cleared, ticks: tick });
            next.set(AppMode::Home);
        }
        Phase::Lost { home_at, .. } if tick >= home_at => {
            end(&mut run, &world, &mut career, &mut storage, &content.0, &mut debrief, VoyageEnd::Lost { ticks: tick });
            next.set(AppMode::Home);
        }
        _ => {}
    }
}

/// Ends the voyage: the career's end (written; not for a practice run) and the debrief.
fn end(
    run: &mut VoyageRun,
    world: &SimWorld,
    career: &mut PlayerCareer,
    storage: &mut Storage,
    content: &Content,
    debrief: &mut Debrief,
    how: VoyageEnd,
) {
    run.phase = Phase::Ended;
    let Some(launched) = run.launched.clone() else { return };
    let stats = DebriefStats {
        sites: world.sim.voyage().map(sites_done),
        kills: Some(run.kills),
        ships_lost: Some(run.ships_lost),
        left: left_behind(&world.sim, PLAYER),
        practice: None,
    };
    let haul = match &how {
        VoyageEnd::Recalled { haul, .. } => haul.clone(),
        VoyageEnd::Lost { .. } => Haul::default(),
    };
    let finds = run.finds.clone();
    let data = ended(career, storage, content, &launched, &finds, how, &haul, stats);
    info!("voyage: {} ended: {:?}, {} items", launched.voyage, data.outcome, data.items);
    debrief.show(data);
}

/// The career's side of an end, and the debrief it makes: a recorded voyage ends in the career (a cluster's after its
/// `finds`, lost or not) and the save is written; a practice run changes nothing (its debrief says what the haul
/// would have given).
#[allow(clippy::too_many_arguments)]
pub fn ended(
    career: &mut PlayerCareer,
    storage: &mut Storage,
    content: &Content,
    launched: &Launched,
    finds: &[Find],
    how: VoyageEnd,
    haul: &Haul,
    stats: DebriefStats,
) -> DebriefData {
    let stats = DebriefStats { practice: launched.practice.clone(), ..stats };
    let before = career.career().clone();
    let mut after = before.clone();
    let last = match recorded(&before, launched).then(|| end_in(&mut after, content, launched, finds, how.clone())).flatten() {
        Some(last) => {
            let ((), written) = career.update(storage, |c| *c = after.clone());
            if let Err(e) = written {
                warn!("voyage: the end was not saved: {e}");
                career.notes.push(Note::new("NOT SAVED", e, Tone::Coral));
            }
            last
        }
        None => {
            after = before.clone();
            unrecorded_haul(content, &launched.voyage, how)
        }
    };
    DebriefData::build(content, &before, &after, &last, haul, stats)
}

/// Is `launched` the voyage the career has underway (not a practice run)?
fn recorded(career: &Career, launched: &Launched) -> bool {
    launched.practice.is_none()
        && match &launched.cluster {
            Some(c) => career.exploration().and_then(|g| g.underway()) == Some(c.cluster),
            None => career.underway() == Some(launched.voyage.as_str()),
        }
}

/// Ends `launched` in `career`: a cluster's finds first, then its end (`Career::cluster_ended`), or a slice
/// voyage's (`Career::voyage_ended`). The result, `None` when the career refuses it.
fn end_in(career: &mut Career, content: &Content, launched: &Launched, finds: &[Find], how: VoyageEnd) -> Option<VoyageResult> {
    match &launched.cluster {
        Some(c) => {
            for f in finds {
                f.apply(career, content);
            }
            career.cluster_ended(content, c.cluster, how).ok().cloned()
        }
        None => career.voyage_ended(content, &launched.voyage, how).ok().cloned(),
    }
}

/// The result of a voyage the career doesn't record (a practice run): what it would have given.
fn unrecorded_haul(content: &Content, voyage: &str, how: VoyageEnd) -> VoyageResult {
    let (outcome, ticks, report) = match how {
        VoyageEnd::Recalled { haul, cleared, ticks } => {
            (if cleared { Outcome::Cleared } else { Outcome::Recalled }, ticks, data::career::unload(content, &haul))
        }
        VoyageEnd::Lost { ticks } => (Outcome::Lost, ticks, Default::default()),
    };
    VoyageResult { voyage: voyage.to_string(), cluster: None, outcome, ticks, report, new: false }
}

/// Flight ends while a recorded voyage still runs (it shouldn't: T does nothing in a voyage, abandoning goes
/// through the lost banner): it is lost, as closing the game would make it. The run is forgotten either way.
fn cut_short(
    mut run: ResMut<VoyageRun>,
    mut career: ResMut<PlayerCareer>,
    mut storage: ResMut<Storage>,
    content: Res<GameContent>,
) {
    if let Some(l) = &run.launched
        && run.phase != Phase::Ended
        && recorded(career.career(), l)
    {
        warn!("voyage: {} cut short: lost", l.voyage);
        let (l, finds, ticks) = (l.clone(), run.finds.clone(), run.tick);
        let (_, written) = career.update(&mut storage, |c| end_in(c, &content.0, &l, &finds, VoyageEnd::Lost { ticks }).is_some());
        if let Err(e) = written {
            career.notes.push(Note::new("NOT SAVED", e, Tone::Coral));
        }
    }
    *run = VoyageRun::default();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::career::{Switches, load};
    use crate::storage::MemoryBackend;
    use data::career::HaulItem;
    use sim::Item;

    fn fresh() -> (Content, Storage, PlayerCareer) {
        let content = data::base_content();
        let mut storage = Storage::new(MemoryBackend::default());
        let career = load(&mut storage, &content, Switches::from_params(&Params::parse("")).0, 4207, 1);
        (content, storage, career)
    }

    fn launched(voyage: &str, seed: u64, practice: Option<String>) -> Launched {
        Launched { voyage: voyage.into(), seed, mind: 2, escorts: Vec::new(), practice, cluster: None }
    }

    /// The V key's voyage, the launch's decisions, and the seed it rolls from (`docs/career.md`'s fixed value).
    #[test]
    fn a_launch_is_recorded_refused_or_practice() {
        let (content, _, pc) = fresh();
        let mut c = pc.career().clone();
        assert_eq!(default_voyage(&c, &content), Some("yard_approach"));
        // Ready and open: recorded, underway, the career's seed.
        let mut a = c.clone();
        assert_eq!(decide(&mut a, &content, "yard_approach", None, None, false), Ok((9993844787240758234, None)));
        assert_eq!(a.underway(), Some("yard_approach"));
        // Not ready: refused from home, a practice run from the URL (the career untouched).
        let mut b = c.clone();
        let why = Some("USES BEAM LASER".to_string());
        assert_eq!(decide(&mut b, &content, "yard_approach", why.clone(), why.clone(), false), Err("NOT VOYAGE-READY · USES BEAM LASER".into()));
        let (seed, practice) = decide(&mut b, &content, "yard_approach", why.clone(), why, true).unwrap();
        assert_eq!((seed, practice.as_deref()), (9993844787240758234, Some("NOT VOYAGE-READY · USES BEAM LASER")));
        assert_eq!(b, c);
        // Free mode: ready by what the drydock grants, not by the career's own unlocks: a practice run.
        assert!(decide(&mut b, &content, "yard_approach", Some("USES BEAM LASER".into()), None, false).unwrap().1.is_some());
        // Not open: refused from home, practice from the URL; an unknown voyage is refused either way.
        assert_eq!(decide(&mut c, &content, "halvex_drift", None, None, false), Err("HALVEX DRIFT IS NOT OPEN YET".into()));
        assert!(decide(&mut c, &content, "halvex_drift", None, None, true).unwrap().1.unwrap().contains("NOT OPEN"));
        assert!(decide(&mut c, &content, "nowhere", None, None, true).is_err());
        // One voyage at a time.
        assert_eq!(decide(&mut a, &content, "yard_approach", None, None, false), Err("SHIPYARD APPROACH IS STILL UNDERWAY".into()));
        // The first open voyage not cleared yet.
        let mut done = pc.career().clone();
        done.voyage_launched(&content, "yard_approach").unwrap();
        done.voyage_ended(&content, "yard_approach", VoyageEnd::Recalled { haul: Haul::default(), cleared: true, ticks: 9000 }).unwrap();
        assert_eq!(default_voyage(&done, &content), Some("halvex_drift"));
    }

    /// The starter fleet is ready; a design with a locked part or too many cores is not; a design without thrust
    /// can't fly.
    #[test]
    fn the_fleet_blocker_names_the_first_problem() {
        let (content, _, pc) = fresh();
        let (unlocks, mind) = (pc.career().unlocks(&content), pc.career().mind_capacity(&content));
        let skiff = content.ships.get("rim_skiff").unwrap().clone();
        assert_eq!(fleet_blocker(&content, &unlocks, mind, &skiff, &[]), None);
        let runner = content.ships.get("halvex_runner").unwrap().clone();
        assert!(fleet_blocker(&content, &unlocks, mind, &runner, &[]).unwrap().starts_with("USES "));
        assert_eq!(fleet_blocker(&content, &unlocks, mind, &skiff, std::slice::from_ref(&skiff)).as_deref(), Some("4 CORES, YOUR MIND RUNS 2"));
        let mut adrift = skiff.clone();
        adrift.parts.retain(|p| !p.part.contains("thruster"));
        let why = fleet_blocker(&content, &unlocks, mind, &skiff, &[adrift]).unwrap();
        assert!(why.starts_with("ESCORT 1 CAN'T FLY"), "{why}");
    }

    /// Launch, recall with a haul, the save written; a loss after a second launch; a practice run changes nothing.
    #[test]
    fn launch_and_end_flows_go_through_the_career() {
        let (content, mut storage, mut pc) = fresh();
        let mut next = pc.career().clone();
        let (seed, practice) = decide(&mut next, &content, "yard_approach", None, None, false).unwrap();
        let ((), w) = pc.update(&mut storage, |c| *c = next);
        w.unwrap();
        let stored = |s: &Storage| s.read::<crate::career::CareerDoc>("career").unwrap().unwrap().doc.0;
        assert_eq!(stored(&storage).underway(), Some("yard_approach"));
        let haul: Haul = [
            (HaulItem::Part { part: "automaton_cannon".into(), material: None }, 2),
            (HaulItem::Fragment { part: "automaton_core".into() }, 1),
        ]
        .into_iter()
        .collect();
        let stats = DebriefStats { sites: Some((4, 4)), kills: Some(5), ships_lost: Some(0), left: 0, practice: None };
        let l = launched("yard_approach", seed, practice);
        let d = ended(&mut pc, &mut storage, &content, &l, &[], VoyageEnd::Recalled { haul: haul.clone(), cleared: true, ticks: 9000 }, &haul, stats.clone());
        assert_eq!((d.outcome, d.items, d.first_clear), (Outcome::Cleared, 3, true));
        assert!(d.opened.iter().any(|v| v == "Halvex Drift"), "{:?}", d.opened);
        let saved = stored(&storage);
        assert_eq!(saved.underway(), None);
        assert!(saved.voyage("yard_approach").unwrap().cleared);
        assert!(saved.data().get("kinetic").copied().unwrap_or(0.0) > 0.0);
        assert_eq!(saved.log().salvaged, 3);

        // A second launch, lost: nothing more in the stores, the loss logged.
        let mut next = pc.career().clone();
        let (seed, _) = decide(&mut next, &content, "yard_approach", None, None, false).unwrap();
        pc.update(&mut storage, |c| *c = next).1.unwrap();
        let data_before = pc.career().data().clone();
        let d = ended(&mut pc, &mut storage, &content, &launched("yard_approach", seed, None), &[], VoyageEnd::Lost { ticks: 300 }, &Haul::default(), stats.clone());
        assert_eq!((d.outcome, d.items, d.first_clear), (Outcome::Lost, 0, false));
        let saved = stored(&storage);
        assert_eq!((saved.log().lost, saved.underway(), saved.data()), (1, None, &data_before));

        // A practice run: the save stays as it is, the debrief still says what the haul gives.
        let text = storage.read::<crate::career::CareerDoc>("career").unwrap().unwrap().doc.0.to_ron();
        let l = launched("halvex_drift", 1, Some("HALVEX DRIFT IS NOT OPEN YET".into()));
        let d = ended(&mut pc, &mut storage, &content, &l, &[], VoyageEnd::Recalled { haul: haul.clone(), cleared: false, ticks: 100 }, &haul, stats);
        assert_eq!((d.outcome, d.items), (Outcome::Recalled, 3));
        assert!(d.practice.is_some());
        assert_eq!(storage.read::<crate::career::CareerDoc>("career").unwrap().unwrap().doc.0.to_ron(), text);
    }

    /// A recorded voyage into a cluster (galaxy plan E6): charted at launch, its finds given to the career at the end
    /// even when it is lost, the debrief listing them; the save written.
    #[test]
    fn a_cluster_voyage_charts_and_keeps_its_finds() {
        let (content, mut storage, mut pc) = fresh();
        let mut next = pc.career().clone();
        let g = next.exploration().unwrap();
        let free = (0u32..).zip(&g.skeleton().clusters).find(|(k, c)| *k != g.skeleton().home && !g.pins().values().any(|p| p == k) && c.band == "rim" && next.jump_from(&content, *k).is_some());
        let k = next.blind_target(&content, free.expect("a free cluster near home").1.at).unwrap();
        let seed = next.cluster_launched(&content, k).unwrap();
        pc.update(&mut storage, |c| *c = next).1.unwrap();
        let s = TrailSignal { trail: 0, signal: 0 };
        let l = Launched {
            voyage: "x".into(),
            seed,
            mind: 2,
            escorts: Vec::new(),
            practice: None,
            cluster: Some(ClusterLaunch { cluster: k, interior: galaxy::interior(&content, pc.career().exploration().unwrap().skeleton(), k).unwrap(), leads: Leads::default() }),
        };
        let d = ended(&mut pc, &mut storage, &content, &l, &[Find::Heard(s)], VoyageEnd::Lost { ticks: 90 }, &Haul::default(), DebriefStats::default());
        let saved = storage.read::<crate::career::CareerDoc>("career").unwrap().unwrap().doc.0;
        let g = saved.exploration().unwrap();
        assert_eq!((g.underway(), g.is_charted(k), g.signal(s).map(|r| r.state)), (None, true, Some(SignalState::Fresh)));
        let lines: Vec<String> = d.signals().into_iter().map(|l| l.0).collect();
        let name = g.skeleton().clusters[k as usize].name.to_uppercase();
        assert_eq!(lines[0], format!("CHARTED  ·  {name}"));
        assert_eq!(lines[1], format!("SIGNAL HEARD  ·  {}", signal_words(&content, &saved, s).0));
        assert!(d.to_map() && d.outcome == Outcome::Lost);
    }

    /// Cargo items become haul items one to one; the haul is every vessel's cargo; a lost ship's cargo stays in
    /// its wreck (the salvage scenario loses its first collector, with cargo, at tick 540).
    #[test]
    fn cargo_becomes_the_haul() {
        let part = Item::Part { part: "steel_block".into(), material: Some("steel".into()) };
        assert_eq!(part.haul(), HaulItem::Part { part: "steel_block".into(), material: Some("steel".into()) });
        assert_eq!(Item::Fragment { part: "logic_core".into() }.haul(), HaulItem::Fragment { part: "logic_core".into() });
        assert_eq!(Item::DataCore { trail: 3, signal: 1 }.haul(), HaulItem::DataCore { trail: 3, signal: 1 });

        let content = data::base_content();
        let mut sc = sim::voyage::scenario::Scenario::with_kind(&content, 42, sim::ScenarioKind::Salvage).unwrap();
        while sc.sim.tick() < 600 {
            sc.step();
        }
        let sim = &sc.sim;
        let haul = fleet_haul(sim, PLAYER);
        let mut want = Haul::default();
        for v in sim.vessels(PLAYER) {
            for (item, stack) in sim.ship(v.ship).unwrap().cargo().items() {
                want.add(item.haul(), stack.count);
            }
        }
        assert_eq!(haul, want);
        assert!(!haul.items.is_empty(), "the collectors carry something by tick 600");
        let wrecked: u32 = sim.ships().iter().filter(|s| s.owner() == PLAYER && !s.is_vessel()).map(|s| s.cargo().items().map(|(_, st)| st.count).sum::<u32>()).sum();
        assert_eq!(left_behind(sim, PLAYER), wrecked);
    }

    /// The HUD's words: title, clock, the RECALL pill by phase and permission.
    #[test]
    fn hud_words_and_recall_gating() {
        let content = data::base_content();
        assert_eq!(title(&content, "yard_approach"), "SHIPYARD APPROACH  ·  RIM");
        assert_eq!(title(&content, "x"), "X");
        assert_eq!((clock(0), clock(30 * 425), clock(30 * 3600 + 29)), ("0:00".into(), "7:05".into(), "60:00".into()));
        assert_eq!(encounter_name(&content, Some("raider_pack")), "RAIDER PACK");
        assert_eq!(encounter_name(&content, None), "BEACON");
        assert_eq!(recall_words(Phase::Flying, true, false, 0), ("RECALL".into(), "WARP HOME 3 S  ·  KEEPS THE HAUL".into(), true));
        assert!(!recall_words(Phase::Flying, false, false, 0).2);
        // GDR-0074: a fleet without a working gun gets the emergency recall, and the hint says why.
        let (_, line, enabled) = recall_words(Phase::Flying, true, true, 0);
        assert!(enabled && line.starts_with("EMERGENCY RECALL"), "{line}");
        assert_eq!(recall_words(Phase::Warping { home_at: 90 }, true, false, 1), ("WARPING".into(), "HOME IN 3 S".into(), false));
        assert_eq!(recall_words(Phase::Warping { home_at: 90 }, true, true, 61).1, "HOME IN 1 S");
        assert!(!recall_words(Phase::Lost { home_at: 90, left: 2, abandoned: false }, true, true, 0).2);
    }

    /// A voyage rolled and started as the scene does: banners for its site events, the site count.
    #[test]
    fn site_events_make_banners() {
        let content = data::base_content();
        let voyage = content.voyages.get("yard_approach").unwrap();
        let plan = sim::VoyagePlan::roll(&content, voyage, 9).unwrap();
        let mut sim = sim::Sim::new(9);
        sim.start_voyage(plan, PLAYER, ENEMY);
        let v = sim.voyage().unwrap();
        assert_eq!(sites_done(v), (0, 4));
        let goal = v.sites().iter().position(|s| s.goal()).unwrap() as u32;
        let (t, sub, tone) = site_banner(&content, v, &SimEvent::SiteCleared { site: goal, goal: true }).unwrap();
        assert_eq!((t.as_str(), tone), ("GOAL CLEARED  ·  RAIDER PAIR", Tone::Gold));
        assert!(sub.starts_with("SITES 0 / 4"), "{sub}");
        let (t, _, tone) = site_banner(&content, v, &SimEvent::SiteWoke { site: 0 }).unwrap();
        assert_eq!((t.as_str(), tone), ("RAIDER PAIR  ·  AWAKE", Tone::Coral));
        assert!(site_banner(&content, v, &SimEvent::ShipSpawned { ship: sim::ShipId(0) }).is_none());
    }

    /// A scan from the entry of a voyage says what it found; the cluster launch finds a cluster with a trail's place
    /// and rolls it.
    #[test]
    fn a_scan_makes_a_banner_and_a_generated_cluster_rolls() {
        let content = data::base_content();
        let plan = sim::VoyagePlan::roll(&content, content.voyages.get("corran_reach").unwrap(), 3).unwrap();
        let entry = plan.entry;
        let mut sim = sim::Sim::new(3);
        sim.start_voyage(plan, PLAYER, ENEMY);
        let bp = content.ships.get("rim_skiff").unwrap();
        let me = sim.spawn_ship(std::sync::Arc::new(sim::ShipDesign::compile(&content, bp).unwrap()), PLAYER, entry);
        sim.step(&sim::TickInputs::default());
        sim.command(me, sim::ShipCommand::Scan);
        let mut banner = None;
        for _ in 0..90 {
            sim.step(&sim::TickInputs::default());
            banner = banner.or(scan_banner(&sim, PLAYER));
        }
        let (title, sub, _) = banner.expect("the sweep's banner");
        assert!(title.starts_with("SCAN  ·  "), "{title}");
        assert!(sub.contains("BEARING") || sub.starts_with("SWEPT 1.2 KM"), "{sub}");
        assert!(scan_banner(&sim, PLAYER).is_none(), "one banner per sweep");

        // Another galaxy's cluster carries everything it holds: every trail place a signal's site.
        let (_, _, pc) = fresh();
        let sk = pc.career().exploration().expect("a new game has its galaxy").skeleton();
        let k = sk.trails[0].places.iter().map(|p| p.cluster).find(|&k| k != sk.home).unwrap();
        let interior = galaxy::interior(&content, sk, k).unwrap();
        let every = every_lead(&content, sk, &interior);
        assert_eq!(every.signals.len(), interior.sites.iter().filter(|s| s.place.is_some()).count());
        let plan = sim::VoyagePlan::roll_interior(&content, "test", &interior, &every, 1).unwrap();
        assert!(plan.sites.iter().any(|s| s.signal));
        // The career's own launch carries what it knows: a new game has heard nothing yet.
        let own = pc.career().leads(&content, k);
        assert!(own.signals.is_empty());
        let plan = sim::VoyagePlan::roll_interior(&content, "test", &interior, &own, 1).unwrap();
        assert!(!plan.sites.iter().any(|s| s.signal));
    }
}
