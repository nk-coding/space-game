//! Voyages in flight (track 7H; GDR-0062, GDR-0065, GDR-0068, ADR-0033; `docs/client.md` "Voyages"): launching
//! one, what happens while it runs, and how it ends. The scene builds the voyage's world (`scene.rs`,
//! `SceneKind::Voyage`), the HUD shows it (`flight_hud.rs`), the debrief reports it at home (`debrief.rs`).
//!
//! * **Launch** ([`StartVoyage`], or `?scene=voyage&voyage=<id>` at start): the career records the launch
//!   (`Career::voyage_launched`: the voyage's seed, `underway`) and the save is written at once, so closing or
//!   reloading mid-voyage loses it (GDR-0062). The fleet is the drydock's working design (the flagship) and the
//!   FLEET escorts; it must be voyage-ready (`data::career::voyage_ready` against what the career grants) and every
//!   ship flight-ready (the design report). The URL launches whatever it names, for development: a voyage the
//!   career refuses (not open, a fleet that is not ready by the career's own unlocks) flies as a **practice run**,
//!   rolled from the seed the next launch would get and never written into the save.
//! * **Running** ([`VoyageRun`]): kills and ships lost are counted from the sim's `VesselLost` events, site events
//!   become banners, R or the RECALL pill asks for the recall, which starts a 3 s warp-out only when the sim allows
//!   it (`Sim::can_recall`: no armed awake enemy within 200 m, or no working gun left in the fleet: the emergency
//!   recall, GDR-0074, which the hint names).
//! * **Endings**: the warp-out ends at home with every surviving vessel's cargo as the haul
//!   (`VoyageEnd::Recalled`, the goal cleared or not); losing every vessel shows `SHIP LOST · RECALLED HOME` and
//!   ends 3 s later (`VoyageEnd::Lost`: the haul stays in the wrecks). Either way the save is written and the
//!   debrief opens at home. Leaving flight any other way while a recorded voyage runs counts as lost too.
//! * **Abandoning** (GDR-0077): M opens the ABANDON VOYAGE? panel (`flight_hud.rs`); its ABANDON VOYAGE (or Enter)
//!   ends the voyage as lost (`Phase::Lost { abandoned: true }`, every hold's cargo stays behind), KEEP FLYING, Esc
//!   or M again close it. The sim runs on while it is open (a real-time sim, co-op later: nothing pauses).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::input::egui_wants_any_keyboard_input;
use data::career::{Haul, HaulItem, LastHaul, Outcome, Refusal, Unlocks, VoyageEnd, voyage_ready};
use data::{Blueprint, Career, Content};
use sim::{Item, Owner, SimConfig, SimEvent, SiteState};

use crate::career::{EscortDesigns, Granted, Note, PlayerCareer, readiness_words};
use crate::debrief::{Debrief, DebriefData, DebriefStats};
use crate::drydock::PlayerDesign;
use crate::drydock::stats::status_line;
use crate::mode::AppMode;
use crate::params::Params;
use crate::scene::{ENEMY, Launch, PLAYER, Scene, SceneKind};
use crate::sim_bridge::{GameContent, SimSet, SimWorld};
use crate::storage::Storage;
use crate::ui_kit::Tone;

/// The warp-out after RECALL, and the time from losing the last vessel to home: 3 s (GDR-0062), in sim ticks.
pub const HOME_TICKS: u64 = 3 * sim::TICK_HZ as u64;

/// Launch a voyage (the voyage map's SET COURSE): `None` is the first open voyage whose goal is not cleared yet, else
/// the first open one. Outside flight only; a refusal becomes a note on home and a [`LaunchRefused`].
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct StartVoyage {
    pub voyage: Option<String>,
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
    /// The ABANDON VOYAGE? panel is open (M opens it; `flight_hud.rs` shows it).
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
            launched: Some(Launched { voyage: id.clone(), seed, mind, escorts: escorts.clone(), practice }),
            ..default()
        };
        self.scene.kind = SceneKind::Voyage(id);
        Ok(())
    }
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
        if let Some(why) = status_line(content, &report).grounded {
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
        other => format!("{other:?}").to_uppercase(),
    }
}

/// `?scene=voyage[&voyage=<id>]`: launch at once (a practice run when the career refuses it); without any voyage
/// to fly, home with a note.
fn url_launch(params: Res<Params>, mut inputs: LaunchInputs, mut next: ResMut<NextState<AppMode>>) {
    if params.text("scene") != Some("voyage") {
        return;
    }
    if let Err(why) = inputs.launch(params.text("voyage"), true) {
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
    match inputs.launch(request.voyage.as_deref(), false) {
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

/// R asks for the recall (the RECALL pill does the same, `flight_hud.rs`). M opens the ABANDON VOYAGE? panel and
/// closes it again, Esc closes it, Enter abandons the voyage (like the panel's button; it counts as lost, like
/// closing the game: the way out of a stalemate).
fn voyage_keys(keys: Res<ButtonInput<KeyCode>>, mut run: ResMut<VoyageRun>, scene: Res<Scene>) {
    if run.launched.is_none() || scene.voyage().is_none() || !matches!(run.phase, Phase::Flying | Phase::Warping { .. }) {
        return;
    }
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if keys.just_pressed(KeyCode::KeyR) && !shift {
        run.recall = true;
    }
    if keys.just_pressed(KeyCode::KeyM) {
        run.asking = !run.asking;
    } else if run.asking && keys.just_pressed(KeyCode::Escape) {
        run.asking = false;
    } else if run.asking && keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
        run.abandon_now();
    }
}

/// The haul item of a cargo item (`data` doesn't depend on `sim`).
pub fn haul_item(item: &Item) -> HaulItem {
    match item {
        Item::Part { part, material } => HaulItem::Part { part: part.clone(), material: material.clone() },
        Item::Fragment { part } => HaulItem::Fragment { part: part.clone() },
    }
}

/// What `owner`'s vessels carry (dormant and dark ones too): the haul a recall brings home.
pub fn fleet_haul(sim: &sim::Sim, owner: Owner) -> Haul {
    sim.vessels(owner)
        .filter_map(|v| sim.ship(v.ship))
        .flat_map(|s| s.cargo().items().map(|(item, stack)| (haul_item(item), stack.count)))
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

/// An encounter's name, uppercase (`RAIDER PACK`).
pub fn encounter_name(content: &Content, encounter: &str) -> String {
    content.encounters.get(encounter).map_or(encounter, |e| e.name.as_str()).to_uppercase()
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

/// Every tick of a voyage: counts, banners, the recall, the endings.
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
    let data = ended(career, storage, content, &launched, how, &haul, stats);
    info!("voyage: {} ended: {:?}, {} items", launched.voyage, data.outcome, data.items);
    debrief.show(data);
}

/// The career's side of an end, and the debrief it makes: a recorded voyage ends in the career and the save is
/// written; a practice run changes nothing (its debrief says what the haul would have given).
pub fn ended(
    career: &mut PlayerCareer,
    storage: &mut Storage,
    content: &Content,
    launched: &Launched,
    how: VoyageEnd,
    haul: &Haul,
    stats: DebriefStats,
) -> DebriefData {
    let stats = DebriefStats { practice: launched.practice.clone(), ..stats };
    let before = career.career().clone();
    let mut after = before.clone();
    let recorded = launched.practice.is_none() && before.underway() == Some(launched.voyage.as_str());
    let last = match recorded.then(|| after.voyage_ended(content, &launched.voyage, how.clone()).ok().cloned()).flatten() {
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

/// The last haul of a voyage the career doesn't record (a practice run): what it would have given.
fn unrecorded_haul(content: &Content, voyage: &str, how: VoyageEnd) -> LastHaul {
    let (outcome, ticks, report) = match how {
        VoyageEnd::Recalled { haul, cleared, ticks } => {
            (if cleared { Outcome::Cleared } else { Outcome::Recalled }, ticks, data::career::unload(content, &haul))
        }
        VoyageEnd::Lost { ticks } => (Outcome::Lost, ticks, Default::default()),
    };
    LastHaul { voyage: voyage.to_string(), outcome, ticks, report }
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
        && l.practice.is_none()
        && career.career().underway() == Some(l.voyage.as_str())
    {
        warn!("voyage: {} cut short: lost", l.voyage);
        let (id, ticks) = (l.voyage.clone(), run.tick);
        let (_, written) = career.update(&mut storage, |c| c.voyage_ended(&content.0, &id, VoyageEnd::Lost { ticks }).is_ok());
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

    fn fresh() -> (Content, Storage, PlayerCareer) {
        let content = data::base_content();
        let mut storage = Storage::new(MemoryBackend::default());
        let career = load(&mut storage, &content, Switches::from_params(&Params::parse("")).0, 4207, 1);
        (content, storage, career)
    }

    fn launched(voyage: &str, seed: u64, practice: Option<String>) -> Launched {
        Launched { voyage: voyage.into(), seed, mind: 2, escorts: Vec::new(), practice }
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
        let d = ended(&mut pc, &mut storage, &content, &l, VoyageEnd::Recalled { haul: haul.clone(), cleared: true, ticks: 9000 }, &haul, stats.clone());
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
        let d = ended(&mut pc, &mut storage, &content, &launched("yard_approach", seed, None), VoyageEnd::Lost { ticks: 300 }, &Haul::default(), stats.clone());
        assert_eq!((d.outcome, d.items, d.first_clear), (Outcome::Lost, 0, false));
        let saved = stored(&storage);
        assert_eq!((saved.log().lost, saved.underway(), saved.data()), (1, None, &data_before));

        // A practice run: the save stays as it is, the debrief still says what the haul gives.
        let text = storage.read::<crate::career::CareerDoc>("career").unwrap().unwrap().doc.0.to_ron();
        let l = launched("halvex_drift", 1, Some("HALVEX DRIFT IS NOT OPEN YET".into()));
        let d = ended(&mut pc, &mut storage, &content, &l, VoyageEnd::Recalled { haul: haul.clone(), cleared: false, ticks: 100 }, &haul, stats);
        assert_eq!((d.outcome, d.items), (Outcome::Recalled, 3));
        assert!(d.practice.is_some());
        assert_eq!(storage.read::<crate::career::CareerDoc>("career").unwrap().unwrap().doc.0.to_ron(), text);
    }

    /// Cargo items become haul items one to one; the haul is every vessel's cargo; a lost ship's cargo stays in
    /// its wreck (the salvage scenario loses its first collector, with cargo, at tick 540).
    #[test]
    fn cargo_becomes_the_haul() {
        let part = Item::Part { part: "steel_block".into(), material: Some("steel".into()) };
        assert_eq!(haul_item(&part), HaulItem::Part { part: "steel_block".into(), material: Some("steel".into()) });
        assert_eq!(haul_item(&Item::Fragment { part: "logic_core".into() }), HaulItem::Fragment { part: "logic_core".into() });

        let content = data::base_content();
        let mut sc = sim::scenario::Scenario::with_kind(&content, 42, sim::ScenarioKind::Salvage).unwrap();
        while sc.sim.tick() < 600 {
            sc.step();
        }
        let sim = &sc.sim;
        let haul = fleet_haul(sim, PLAYER);
        let mut want = Haul::default();
        for v in sim.vessels(PLAYER) {
            for (item, stack) in sim.ship(v.ship).unwrap().cargo().items() {
                want.add(haul_item(item), stack.count);
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
        assert_eq!(encounter_name(&content, "raider_pack"), "RAIDER PACK");
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
}
