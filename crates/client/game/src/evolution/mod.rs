//! The EVOLUTION layer over home (GDR-0092 step 4): the evolution lab's screens. The runs themselves, their
//! background work and their storage are `world::evolution`; this is what the player sees of them.
//!
//! ```text
//! (o) SHIPYARD › EVOLUTION                                       [NEW RUN]  [CLOSE]
//!     Evolution
//!  RUNS: name · starts · status · generation · start → best · made   [OPEN] [PAUSE] [DELETE]
//!  SETUP (NEW RUN): START FROM · LOCKS · LIMITS · MAY CHANGE · TRIALS · SIZE · SEED · NAME      [START RUN]
//!  THE RUN: SCORES BY GENERATION | THE ARCHIVE | the picked design: FIGHTS (WATCH) · LINEAGE · COMPARE · SAVE AS DESIGN
//!  THE LADDER: YOUR DESIGNS with their ratings, RATE, RATE ALL | the picked design's rungs (WATCH)
//! ```
//!
//! * The building above the mind (E, a click, or EVOLVE THIS DESIGN in the drydock's ⋯ menu, which opens the setup
//!   with the working design as its start) opens it; the ring top left, CLOSE and Esc go back a page, and close it
//!   from RUNS. It is a layer over home, like FLEET and the LOG.
//! * **RUNS** ([`runs`]): every run, newest first, with OPEN, PAUSE / RESUME, DUPLICATE and DELETE (asked in the row
//!   first); IMPORT RUN reads a CLI run file (web: a file chooser; desktop: drop it onto the window).
//! * **SETUP** ([`setup`]): one to four starts from YOUR DESIGNS and the templates, their locks (whole parts, KEEP
//!   ALL of a category, rules, the target line, LOCK ORDER), the career's limits (build points, the hold,
//!   voyage-ready, mirror; only the unlocked parts), what may change, the trials (`lab.ron`'s presets with weights and
//!   seeds) and the goals, the size, the seed, the name, the estimate; START RUN.
//! * **THE RUN** ([`run`]): its progress (generation n of N and the open one, the measured fights per second and
//!   the time left, start → best), PAUSE / RESUME, the scores by generation, the archive as ship cards, and the
//!   picked design: its scores by trial and goal with VALIDATE, its fights with WATCH (the fight viewer; M brings the
//!   lab back), its lineage, a comparison with its start, SAVE AS DESIGN; RENAME, and RE-SCORE for a stale run.
//! * **THE LADDER** ([`ladder`], GDR-0092 step 9; THE LADDER on RUNS): YOUR DESIGNS rated on `lab.ron`'s ladder, a
//!   rating each and every rung's wins, RATE and RATE ALL (rated in the run's place), WATCH a rung's worst fight.
//! * The building's label says what the lab does ([`building_line`]); its look follows [`light`].
//! * **The chip** ([`Chip`]): `EVOLVING  12 / 50` beside home's gear ring and after the title of every layer over
//!   home (FLEET, the LOG, SETTINGS) while a run computes; a click opens the lab (`home::chip`).
//!
//! UI words are chrome in code, like FLEET's and the LOG's; names come from content and the player's designs, the
//! lab's own words and numbers from `lab.ron` (goals, genes, trials and their kinds, pilots, sizes, measures).

mod ladder;
mod run;
mod runs;
mod setup;

use world::arena::evolve::RunStatus;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use data::{Content, ResearchNode, Unlock};
use kit::clipboard::Clipboard;
use kit::style::{Style, ui_color};
use bevy::window::PrimaryWindow;
use kit::ui_kit::{
    CompactText, Edge, HudFonts, LayoutClass, Pill, PillText, ScreenFx, ScreenPart, Scrollbar, TextField, Tip, Tone, UiClick, UiInputSet, UiStyleSet, crumb, frame, label, set_color, set_text, show_screen, snap_screen, spawn_back, spawn_close,
    spawn_pill, title, top_pill,
};
use world::career::{EscortDesigns, Granted, PlayerCareer};
use world::evolution::{Lab, LabRequest};
use world::mode::AppMode;
use world::player::PlayerDesign;
use world::player::library::DesignSlot;
use world::player::behaviors::PlayerBehaviors;
use world::replay::Replay;
use world::scene::Scene;
use world::sim_bridge::GameContent;
use world::storage::{Storage, now_unix};

use crate::home::HomeAction;
use crate::shipyard::draw::Evo;

pub use ladder::rating_words;
pub use setup::SetupPage;

/// The layout's size (logical px), like FLEET's.
const STAGE: Vec2 = Vec2::new(1280.0, 720.0);
/// On a phone ([`LayoutClass::Compact`], docs/client.md "Phones"): the stage, where the pages start (under the
/// header and the pills) and their left edge, the status line's height, and the least text size. Each page scrolls
/// up and down there, its wide layout's columns stacked ([`Reflow`]).
const COMPACT_STAGE: Vec2 = Vec2::new(844.0, 390.0);
const COMPACT_TOP: f32 = 62.0;
const COMPACT_LEFT: f32 = 16.0;
const COMPACT_STATUS: f32 = 24.0;
const COMPACT_TEXT: f32 = 9.0;

/// The layer's pages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Runs,
    Setup,
    Run,
    Ladder,
}

/// The EVOLUTION layer's state (a resource).
#[derive(Resource, Default)]
pub struct EvolutionLayer {
    pub open: bool,
    pub page: Page,
    /// RUNS: the run whose DELETE asks first.
    confirm: Option<String>,
    /// SETUP: what is being set up.
    pub setup: Option<SetupPage>,
    /// THE RUN: the picked design and what shows of it.
    run: run::RunPage,
    /// THE LADDER: the design whose rungs show.
    ladder: ladder::LadderPage,
    /// The last message, for the status line.
    say: Option<(String, Tone)>,
    /// What the page shows is to be built again (a click changed it).
    dirty: bool,
}

impl EvolutionLayer {
    fn say(&mut self, words: impl Into<String>, tone: Tone) {
        self.say = Some((words.into().to_uppercase(), tone));
    }

    fn go(&mut self, page: Page) {
        self.page = page;
        self.confirm = None;
        self.dirty = true;
    }
}

/// Is the layer open (home's input waits then)?
pub fn is_open(layer: Res<EvolutionLayer>) -> bool {
    layer.open
}

/// How the building looks (GDR-0092 "Its states"): dark while the lab is locked, busy while a run computes,
/// gold once one has finished until it is opened, paused while a run waits for RESUME, else dim.
pub fn light(lab: &Lab) -> Evo {
    if !lab.enabled {
        Evo::Dark
    } else if lab.running().is_some() {
        Evo::Running
    } else if lab.fresh.is_some() {
        Evo::Done
    } else if lab.runs().iter().any(|e| e.status == RunStatus::Paused && !e.done() && !e.stale && e.problem.is_none()) {
        Evo::Paused
    } else {
        Evo::Idle
    }
}

/// A score as the lab shows it: two decimals, a dash for none.
pub fn score(v: Option<f32>) -> String {
    v.map_or_else(|| "—".into(), |s| format!("{s:.2}"))
}

/// The research node that opens the lab (the Simulation vault).
pub fn unlocked_by(content: &Content) -> Option<&ResearchNode> {
    world::career::unlocked_by(content, &Unlock::Facility(world::evolution::FACILITY.into()))
}

/// The line under the building's name and its tone, short enough for the room above the mind: `12 / 50  ·  BEST
/// 0.81` while a run computes, `PAUSED  ·  12 / 50`, `DONE  ·  BEST 0.91`, `NO RUN`, and `LOCKED` while it is locked
/// (its tooltip names the node that opens it, `home::evolution_tip`: a node's name doesn't fit the room).
pub fn building_line(lab: &Lab) -> (String, Tone) {
    let entry = |id: Option<&str>| id.and_then(|id| lab.entry(id));
    match light(lab) {
        Evo::Dark => ("LOCKED".into(), Tone::Dim),
        Evo::Idle => ("NO RUN".into(), Tone::Label),
        Evo::Running => match entry(lab.running().map(|l| l.id.as_str())) {
            Some(e) => (format!("{} / {}  ·  BEST {}", e.generation, e.generations, score(e.best)), Tone::Teal),
            None => ("RUNNING".into(), Tone::Teal),
        },
        Evo::Paused => match lab.runs().iter().find(|e| e.status == RunStatus::Paused && !e.done() && !e.stale) {
            Some(e) => (format!("PAUSED  ·  {} / {}", e.generation, e.generations), Tone::Label),
            None => ("PAUSED".into(), Tone::Label),
        },
        Evo::Done => match entry(lab.fresh.as_deref()) {
            Some(e) => (format!("DONE  ·  BEST {}", score(e.best)), Tone::Gold),
            None => ("DONE".into(), Tone::Gold),
        },
    }
}

/// The EVOLVING chip: on home and the layers over it while a run computes.
#[derive(Component)]
pub struct Chip;

/// Spawns the chip into a row (home's top right beside the gear ring, a layer's top left after its title).
pub fn spawn_chip(parent: &mut ChildSpawnerCommands, fonts: &HudFonts) {
    let chip = spawn_pill(parent, fonts, "EVOLVING", 8.5, 26.0, None, Pill::new(Tone::Teal), (Chip, Tip::new("THE EVOLUTION LAB")));
    parent.commands().entity(chip).entry::<Node>().and_modify(|mut n| n.display = Display::None);
}

/// The chip's words follow the run that computes (`EVOLVING  12 / 50`, the generations done of its generations); it
/// shows only while one does, and only at home (SETTINGS over flight has none).
fn sync_chip(mode: Res<State<AppMode>>, lab: Res<Lab>, mut chips: Query<(&mut Node, &Children), With<Chip>>, mut texts: Query<&mut Text, With<PillText>>) {
    if !(lab.is_changed() || mode.is_changed()) {
        return;
    }
    let shown = *mode.get() == AppMode::Home && lab.enabled;
    let words = lab.running().filter(|_| shown).and_then(|l| lab.entry(&l.id)).map(|e| format!("EVOLVING  {} / {}", e.generation, e.generations));
    for (mut node, children) in &mut chips {
        let display = if words.is_some() { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        for &child in children {
            if let (Ok(mut text), Some(w)) = (texts.get_mut(child), &words) {
                set_text(&mut text, w);
            }
        }
    }
}

/// The layer's root and whether it is laid out for a phone.
#[derive(Component)]
struct LayerRoot(bool);

/// The pages' roots.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
struct PageNode(Page);

/// What a page's widgets are placed in (on the stage as they are; on a phone moved by [`reflow`]).
#[derive(Component, Clone, Copy, PartialEq, Eq)]
struct PageBody(Page);

/// A page's wide layout on a phone: its parts in each region (wide stage px, by where a part's top left lies) move
/// by `by`, at most `max` wide and high; the page's content is `height` tall.
struct Reflow {
    regions: Vec<(Rect, Vec2, Vec2)>,
    height: f32,
}

const ANY: Vec2 = Vec2::splat(f32::MAX);
const PHONE_W: f32 = COMPACT_STAGE.x - 2.0 * COMPACT_LEFT;
const PHONE_PAGE_H: f32 = COMPACT_STAGE.y - COMPACT_TOP - COMPACT_STATUS;

impl Page {
    /// Where the page's columns go on a phone: stacked, the first at the top (one wide list keeps the page's height
    /// and scrolls itself).
    fn reflow(self) -> Reflow {
        // Each region: the wide rect its parts start in, the move, the largest size.
        match self {
            Page::Runs => Reflow { regions: vec![(Rect { min: Vec2::ZERO, max: ANY }, Vec2::new(COMPACT_LEFT - 60.0, 4.0 - 100.0), Vec2::new(PHONE_W, PHONE_PAGE_H - 8.0))], height: PHONE_PAGE_H },
            // The left column (36, 600 wide; 90 to 680), then the right one (664, 580 wide; 90 to 676), centred.
            Page::Setup => Reflow {
                regions: vec![
                    (Rect { min: Vec2::ZERO, max: Vec2::new(650.0, f32::MAX) }, Vec2::new(122.0 - 36.0, 4.0 - 90.0), ANY),
                    (Rect { min: Vec2::new(650.0, 0.0), max: ANY }, Vec2::new(132.0 - 664.0, 618.0 - 90.0), ANY),
                ],
                height: 618.0 + 586.0 + 12.0,
            },
            // The progress line, then the chart and the archive side by side, then the picked design.
            Page::Run => Reflow {
                regions: vec![
                    (Rect { min: Vec2::ZERO, max: Vec2::new(f32::MAX, 136.0) }, Vec2::new(COMPACT_LEFT - 36.0, 4.0 - 88.0), Vec2::new(PHONE_W, f32::MAX)),
                    (Rect { min: Vec2::new(0.0, 136.0), max: Vec2::new(420.0, f32::MAX) }, Vec2::new(COMPACT_LEFT - 36.0, -88.0), ANY),
                    (Rect { min: Vec2::new(420.0, 136.0), max: Vec2::new(850.0, f32::MAX) }, Vec2::new(COMPACT_LEFT + 404.0 - 444.0, -88.0), ANY),
                    (Rect { min: Vec2::new(850.0, 136.0), max: ANY }, Vec2::new(COMPACT_LEFT - 864.0, 548.0 - 140.0), ANY),
                ],
                height: 548.0 + 540.0 + 12.0,
            },
            // YOUR DESIGNS a page high (it scrolls), then the picked design's rungs under it.
            Page::Ladder => Reflow {
                regions: vec![
                    (Rect { min: Vec2::ZERO, max: Vec2::new(660.0, f32::MAX) }, Vec2::new(COMPACT_LEFT - 60.0, 4.0 - 100.0), Vec2::new(f32::MAX, PHONE_PAGE_H - 8.0)),
                    (Rect { min: Vec2::new(660.0, 0.0), max: ANY }, Vec2::new(COMPACT_LEFT - 690.0, PHONE_PAGE_H + 16.0 - 100.0), ANY),
                ],
                height: PHONE_PAGE_H + 16.0 + 570.0 + 12.0,
            },
        }
    }
}

/// On a phone each page's parts move to their place in its stacked columns ([`Page::reflow`]), once, when built.
fn reflow(roots: Query<&LayerRoot>, bodies: Query<(&PageBody, &Children), Added<PageBody>>, mut nodes: Query<&mut Node>) {
    if !roots.iter().any(|r| r.0) {
        return;
    }
    for (body, children) in &bodies {
        let flow = body.0.reflow();
        for c in children {
            let Ok(mut n) = nodes.get_mut(*c) else { continue };
            let (Val::Px(left), Val::Px(top)) = (n.left, n.top) else { continue };
            let Some(&(_, by, max)) = flow.regions.iter().find(|(r, ..)| r.contains(Vec2::new(left, top))) else { continue };
            n.left = px(left + by.x);
            n.top = px(top + by.y);
            if let Val::Px(w) = n.width
                && w > max.x
            {
                n.width = px(max.x);
                n.flex_wrap = FlexWrap::Wrap;
            }
            if let Val::Px(h) = n.height
                && h > max.y
            {
                n.height = px(max.y);
            }
        }
    }
}

/// The texts every page shares: where we are, the title, the status line.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum LayerText {
    Crumb,
    Title,
    Status,
}

/// The top right's pills, shown per page.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum TopPill {
    NewRun,
    Import,
    Pause,
    Resume,
    Rescore,
    Rename,
    Ladder,
    RateAll,
}

/// What a click on a widget of the layer does.
#[derive(Component, Clone, Debug, PartialEq)]
pub(crate) enum LabButton {
    /// The ring top left and CLOSE: back a page, or close the layer from RUNS.
    Back,
    NewRun,
    /// The run on the RUNS page.
    Open(String),
    Pause(String),
    Resume(String),
    Delete(String),
    ConfirmDelete(String),
    Duplicate(String),
    Import,
    KeepRun,
    Setup(setup::SetupButton),
    Run(run::RunButton),
    /// THE LADDER on RUNS: its page.
    OpenLadder,
    Ladder(ladder::LadderButton),
}

/// What the pages' clicks change: the lab and its storage, and what a run is made from or leads to.
#[derive(SystemParam)]
pub(crate) struct Ctx<'w> {
    lab: ResMut<'w, Lab>,
    storage: ResMut<'w, Storage>,
    content: Res<'w, GameContent>,
    design: ResMut<'w, PlayerDesign>,
    slot: ResMut<'w, DesignSlot>,
    behaviors: ResMut<'w, PlayerBehaviors>,
    granted: Res<'w, Granted>,
    escorts: Res<'w, EscortDesigns>,
    career: Res<'w, PlayerCareer>,
    scene: ResMut<'w, Scene>,
    replay: ResMut<'w, Replay>,
    next: ResMut<'w, NextState<AppMode>>,
    clipboard: ResMut<'w, Clipboard>,
}

pub struct EvolutionPlugin;

impl Plugin for EvolutionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EvolutionLayer>()
            .add_systems(Startup, spawn.after(world::flight_hud::spawn_hud))
            // A phone's layout and a wide one's are built apart: built again when the window changes class.
            .add_systems(Update, (relayout.run_if(resource_changed::<LayoutClass>), reopen, reflow).chain().after(UiInputSet).before(open))
            .add_systems(OnExit(AppMode::Home), |mut layer: ResMut<EvolutionLayer>, mut lab: ResMut<Lab>| {
                layer.open = false;
                lab.layer_open = false;
            })
            .add_systems(Update, show.run_if(state_changed::<AppMode>.or_eager(resource_changed::<EvolutionLayer>)))
            .add_systems(Update, sync_chip)
            .add_systems(
                Update,
                (open, input, runs::imported, trouble, (runs::fill, setup::fill, run::fill, ladder::fill), (sync, run::sync)).chain().after(UiInputSet).before(UiStyleSet).run_if(in_state(AppMode::Home)),
            );
    }
}

/// The building (E, a click; EVOLVE THIS DESIGN and coming back from WATCH send the same): open on RUNS, on the
/// setup with the drydock's design as its start, or where it was when a fight was watched.
#[allow(clippy::too_many_arguments)]
fn open(
    mut actions: MessageReader<HomeAction>,
    mut layer: ResMut<EvolutionLayer>,
    mut lab: ResMut<Lab>,
    storage: Res<Storage>,
    content: Res<GameContent>,
    behaviors: Res<PlayerBehaviors>,
    design: Res<PlayerDesign>,
    career: Res<PlayerCareer>,
) {
    if !actions.read().any(|a| *a == HomeAction::Evolution) {
        return;
    }
    layer.open = true;
    lab.layer_open = true;
    layer.say = None;
    if std::mem::take(&mut lab.watching) {
        layer.dirty = true;
        return;
    }
    match lab.request.take() {
        Some(LabRequest::Setup(start)) => {
            layer.setup = Some(SetupPage::new(&content.0, &storage, &behaviors, &design, career.career(), &lab, vec![start]));
            layer.go(Page::Setup);
        }
        Some(LabRequest::Origin { run, genome }) => match lab.view(&storage, &content.0, Some(&run)) {
            Ok(()) => {
                // An evolved enemy's origin opens THE ENEMY's side.
                let camp = lab.viewed().and_then(|l| l.run.camp(genome)).unwrap_or_default();
                layer.run = run::RunPage::picking(genome, camp);
                layer.go(Page::Run);
            }
            Err(e) => {
                layer.go(Page::Runs);
                layer.say(format!("{run} doesn't open: {e}"), Tone::Coral);
            }
        },
        None => layer.go(Page::Runs),
    }
}

/// The clicks of every page, and Esc (back a page; not the Esc that lets go of a text field).
#[allow(clippy::too_many_arguments)]
fn input(
    mut clicks: MessageReader<UiClick>,
    buttons: Query<(&LabButton, Option<&Pill>)>,
    keys: Res<ButtonInput<KeyCode>>,
    fields: Query<&TextField>,
    mut typed: Local<bool>,
    mut layer: ResMut<EvolutionLayer>,
    mut ctx: Ctx,
) {
    let typing = std::mem::replace(&mut *typed, fields.iter().any(|f| f.focused));
    if !layer.open {
        clicks.clear();
        return;
    }
    let mut pressed: Vec<LabButton> = clicks.read().filter_map(|c| buttons.get(c.0).ok()).filter(|(_, p)| !p.is_some_and(|p| p.disabled)).map(|(b, _)| b.clone()).collect();
    if keys.just_pressed(KeyCode::Escape) && !typing {
        pressed.push(LabButton::Back);
    }
    for b in pressed {
        match b {
            LabButton::Back => match layer.page {
                Page::Runs => {
                    layer.open = false;
                    ctx.lab.layer_open = false;
                }
                Page::Setup | Page::Run | Page::Ladder => {
                    let _ = ctx.lab.view(&ctx.storage, &ctx.content.0, None);
                    layer.go(Page::Runs);
                }
            },
            LabButton::NewRun => {
                let start = world::evolution::StartPick::Working;
                layer.setup = Some(SetupPage::new(&ctx.content.0, &ctx.storage, &ctx.behaviors, &ctx.design, ctx.career.career(), &ctx.lab, vec![start]));
                layer.say = None;
                layer.go(Page::Setup);
            }
            LabButton::Open(id) => match ctx.lab.view(&ctx.storage, &ctx.content.0, Some(&id)) {
                Ok(()) => {
                    layer.run = run::RunPage::default();
                    layer.say = None;
                    layer.go(Page::Run);
                }
                Err(e) => layer.say(format!("{id} doesn't open: {e}"), Tone::Coral),
            },
            LabButton::Pause(id) => runs::status(&mut layer, &mut ctx, &id, RunStatus::Paused),
            LabButton::Duplicate(id) => match ctx.lab.duplicate(&mut ctx.storage, &ctx.content.0, &id, now_unix()) {
                Ok(new) => {
                    let name = ctx.lab.entry(&new).map_or_else(|| new.clone(), |e| e.name.clone());
                    layer.say(format!("{name} computes: the same setup with a new seed"), Tone::Teal);
                    layer.dirty = true;
                }
                Err(e) => layer.say(format!("not duplicated: {e}"), Tone::Coral),
            },
            LabButton::Import => runs::import(&mut layer),
            LabButton::Resume(id) => runs::status(&mut layer, &mut ctx, &id, RunStatus::Running),
            LabButton::Delete(id) => {
                layer.confirm = Some(id);
                layer.dirty = true;
            }
            LabButton::KeepRun => {
                layer.confirm = None;
                layer.dirty = true;
            }
            LabButton::ConfirmDelete(id) => {
                let name = ctx.lab.entry(&id).map_or_else(|| id.clone(), |e| e.name.clone());
                match ctx.lab.delete(&mut ctx.storage, &id) {
                    Ok(()) => layer.say(format!("{name} deleted"), Tone::Teal),
                    Err(e) => layer.say(format!("not deleted: {e}"), Tone::Coral),
                }
                layer.confirm = None;
                layer.dirty = true;
            }
            LabButton::Setup(s) => setup::press(&mut layer, &mut ctx, s),
            LabButton::Run(r) => run::press(&mut layer, &mut ctx, r),
            LabButton::OpenLadder => {
                layer.say = None;
                layer.go(Page::Ladder);
            }
            LabButton::Ladder(l) => ladder::press(&mut layer, &mut ctx, l),
        }
    }
}

/// Shown at home while open, with its transition; home's words step aside meanwhile (`home::show`).
fn show(mode: Res<State<AppMode>>, layer: Res<EvolutionLayer>, mut root: Single<&mut ScreenFx, With<LayerRoot>>, mut pages: Query<(&PageNode, &mut Node)>) {
    show_screen(&mut root, *mode.get() == AppMode::Home && layer.open);
    for (p, mut node) in &mut pages {
        let d = if p.0 == layer.page { Display::Flex } else { Display::None };
        if node.display != d {
            node.display = d;
        }
    }
}

/// What went wrong while computing is news: it takes the status line from the last message (a checkpoint the web's
/// storage failed to keep comes after START RUN said the run computes).
fn trouble(mut layer: ResMut<EvolutionLayer>, lab: Res<Lab>, mut shown: Local<Option<String>>) {
    if lab.is_changed() && lab.trouble != *shown {
        shown.clone_from(&lab.trouble);
        if lab.trouble.is_some() {
            layer.say = None;
        }
    }
}

/// The shared texts and the top right's pills.
#[allow(clippy::type_complexity)]
fn sync(
    layer: Res<EvolutionLayer>,
    lab: Res<Lab>,
    style: Res<Style>,
    mut texts: Query<(&LayerText, &mut Text, &mut TextColor)>,
    mut pills: Query<(&TopPill, &mut Node)>,
) {
    if !layer.open || !(layer.is_changed() || lab.is_changed()) {
        return;
    }
    let ui = &style.ui;
    let viewed = lab.viewed();
    for (t, mut text, mut color) in &mut texts {
        let (s, tone) = match (t, layer.page) {
            (LayerText::Crumb, Page::Runs) => ("SHIPYARD  ›  EVOLUTION".to_string(), Tone::Label),
            (LayerText::Crumb, Page::Setup) => ("SHIPYARD  ›  EVOLUTION  ›  NEW RUN".to_string(), Tone::Label),
            (LayerText::Crumb, Page::Run) => ("SHIPYARD  ›  EVOLUTION  ›  RUN".to_string(), Tone::Label),
            (LayerText::Crumb, Page::Ladder) => ("SHIPYARD  ›  EVOLUTION  ›  THE LADDER".to_string(), Tone::Label),
            (LayerText::Title, Page::Ladder) => ("The ladder".to_string(), Tone::Number),
            (LayerText::Title, Page::Runs) => ("Evolution".to_string(), Tone::Number),
            (LayerText::Title, Page::Setup) => ("New run".to_string(), Tone::Number),
            (LayerText::Title, Page::Run) => (viewed.map_or_else(|| "Run".to_string(), |l| l.name.clone()), Tone::Number),
            (LayerText::Status, _) => match (&layer.say, &lab.trouble) {
                (Some((s, tone)), _) => (s.clone(), *tone),
                (None, Some(t)) => (t.to_uppercase(), Tone::Coral),
                (None, None) => (default_status(layer.page).into(), Tone::Label),
            },
        };
        set_text(&mut text, &s);
        set_color(&mut color, ui_color(tone.color(ui), 1.0));
    }
    let computes = viewed.is_some_and(|l| l.computes());
    let can_resume = viewed.is_some_and(|l| l.status == RunStatus::Paused && !l.run.finished() && !l.run.stale());
    for (p, mut node) in &mut pills {
        let on = match p {
            TopPill::NewRun | TopPill::Import | TopPill::Ladder => layer.page == Page::Runs,
            TopPill::RateAll => layer.page == Page::Ladder,
            TopPill::Pause => layer.page == Page::Run && computes,
            TopPill::Resume => layer.page == Page::Run && can_resume,
            TopPill::Rescore => layer.page == Page::Run && viewed.is_some_and(|l| l.run.stale()),
            TopPill::Rename => layer.page == Page::Run && viewed.is_some(),
        };
        let d = if on { Display::Flex } else { Display::None };
        if node.display != d {
            node.display = d;
        }
    }
}

/// WATCH: the fight viewer plays `code` (a fight's or a voyage's) and the lab waits; M brings the layer back as it
/// was.
pub(crate) fn watch(ctx: &mut Ctx, code: String) {
    ctx.replay.queue(code);
    ctx.scene.launch = Some(world::scene::Launch::Fight);
    ctx.lab.watching = true;
    ctx.next.set(AppMode::Flight);
}

/// What the status line says while nothing happened.
fn default_status(page: Page) -> &'static str {
    match page {
        Page::Runs => "A RUN BREEDS YOUR DESIGNS AGAINST ENEMIES AND KEEPS THE BEST OF EACH KIND  ·  ONE COMPUTES AT A TIME, AT HOME AND ON EVERY SCREEN, NEVER IN FLIGHT",
        Page::Setup => "SAME SEED, SAME SETUP, SAME RULES: THE SAME RUN",
        Page::Run => "CLICK A DESIGN IN THE ARCHIVE  ·  EVERY FIGHT CAN BE WATCHED AGAIN",
        Page::Ladder => "THE SAME RUNGS AND SEEDS FOR EVERY DESIGN: RATINGS COMPARE  ·  A RUN WAITS WHILE THE LAB RATES",
    }
}

/// Node helpers of the pages: absolutely placed boxes on the stage, rows and columns.
pub(crate) fn abs(at: Vec2, size: Option<Vec2>) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(at.x),
        top: px(at.y),
        width: size.map_or(Val::Auto, |s| px(s.x)),
        height: size.map_or(Val::Auto, |s| px(s.y)),
        ..default()
    }
}

pub(crate) fn row(gap: f32) -> Node {
    Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(gap), ..default() }
}

/// A placed node that lays its children out in a row.
pub(crate) fn on_row(at: Node, gap: f32) -> Node {
    Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(gap), ..at }
}

pub(crate) fn column(gap: f32) -> Node {
    Node { flex_direction: FlexDirection::Column, row_gap: px(gap), ..default() }
}

fn spawn(commands: Commands, window: Single<&Window, With<PrimaryWindow>>, fonts: Res<HudFonts>, style: Res<Style>) {
    build(commands, LayoutClass::of(window.size()).is_compact(), &fonts, &style);
}

/// The window changed its layout class: the layer is built again for the new one (open at once when it was, its
/// pages filled again).
fn relayout(mut commands: Commands, class: Res<LayoutClass>, root: Single<(Entity, &LayerRoot)>, fonts: Res<HudFonts>, style: Res<Style>, mut layer: ResMut<EvolutionLayer>) {
    if root.1.0 == class.is_compact() {
        return;
    }
    commands.entity(root.0).despawn();
    build(commands, class.is_compact(), &fonts, &style);
    layer.dirty = true;
}

/// A layer built again while open shows at once.
fn reopen(mode: Res<State<AppMode>>, layer: Res<EvolutionLayer>, mut roots: Query<&mut ScreenFx, Added<LayerRoot>>) {
    for mut fx in &mut roots {
        if *mode.get() == AppMode::Home && layer.open {
            snap_screen(&mut fx, true);
        }
    }
}

fn build(mut commands: Commands, compact: bool, fonts: &HudFonts, style: &Style) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let stage_size = if compact { COMPACT_STAGE } else { STAGE };
    commands
        .spawn((
            LayerRoot(compact),
            CompactText(COMPACT_TEXT),
            world::backdrop::VeilsBackdrop,
            ScreenFx::closed().dimming(),
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn(Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|centre| {
                centre.spawn(Node { width: px(stage_size.x), height: px(stage_size.y), flex_shrink: 0.0, ..default() }).with_children(|stage| {
                    for page in [Page::Runs, Page::Setup, Page::Run, Page::Ladder] {
                        // On a phone the page scrolls between the header and the status line.
                        let (node, body) = if compact {
                            let node = Node { position_type: PositionType::Absolute, left: px(0), top: px(COMPACT_TOP), width: percent(100), height: px(PHONE_PAGE_H), overflow: Overflow::scroll_y(), ..default() };
                            (node, Node { width: percent(100), height: px(page.reflow().height), flex_shrink: 0.0, ..default() })
                        } else {
                            let full = Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() };
                            (full.clone(), full)
                        };
                        let mut p = stage.spawn((PageNode(page), ScreenPart::new(Edge::Fade, 1), node));
                        if compact {
                            // No glow room: the page would show under the header and the status line.
                            p.insert((ScrollPosition::default(), Scrollbar::vertical().room(0)));
                        }
                        p.with_children(|p| {
                            p.spawn((PageBody(page), body)).with_children(|p| match page {
                                Page::Runs => runs::spawn(p, fonts, &c),
                                Page::Setup => setup::spawn(p, fonts, &c, &ui),
                                Page::Run => run::spawn(p, fonts, &c),
                                Page::Ladder => ladder::spawn(p, fonts, &c),
                            });
                        });
                    }
                    let line = (LayerText::Status, ScreenPart::new(Edge::Bottom, 3), label(fonts, "", 9.0, c(Tone::Label, 1.0)));
                    if compact {
                        // Within the margins, two lines when it is long, its last line at the stage's bottom (cut to one
                        // line it ran past both edges, centred on its box).
                        stage.spawn((line, Node { position_type: PositionType::Absolute, left: px(COMPACT_LEFT), width: px(PHONE_W), bottom: px(3), ..default() }));
                    } else {
                        stage.spawn((line, abs(Vec2::new(36.0, 690.0), Some(Vec2::new(1208.0, 20.0)))));
                    }
                });
            });
            // Top left: the way back, where we are, the title; top right: the page's pills, CLOSE (the frame's corners).
            let f = frame();
            root.spawn((ScreenPart::new(Edge::Top, 0), f.top_left())).with_children(|tl| {
                spawn_back(tl, &ui, (LabButton::Back, Tip::key("BACK", "ESC")));
                tl.spawn(f.heading()).with_children(|col| {
                    col.spawn((LayerText::Crumb, crumb(fonts, "SHIPYARD  ›  EVOLUTION", &ui)));
                    col.spawn((LayerText::Title, title(fonts, "Evolution", &ui)));
                });
            });
            root.spawn((ScreenPart::new(Edge::Top, 0), f.top_right())).with_children(|tr| {
                let primary = Pill { primary: true, ..Pill::new(Tone::Teal) };
                let pills = [
                    ("THE LADDER", Pill::new(Tone::Gold), LabButton::OpenLadder, Some("YOUR DESIGNS RATED ON A FIXED LADDER OF ENEMIES"), TopPill::Ladder),
                    ("RATE ALL", primary, LabButton::Ladder(ladder::LadderButton::RateAll), Some("EVERY SAVED DESIGN ON THE LADDER"), TopPill::RateAll),
                    ("IMPORT RUN", Pill::new(Tone::Teal), LabButton::Import, Some("A RUN FILE OF THE CLI: ARENA EVOLVE --SAVE"), TopPill::Import),
                    ("NEW RUN", primary, LabButton::NewRun, Some("SET UP A NEW RUN"), TopPill::NewRun),
                    ("RENAME", Pill::new(Tone::Teal), LabButton::Run(run::RunButton::Rename), None, TopPill::Rename),
                    ("RE-SCORE", primary, LabButton::Run(run::RunButton::Rescore), Some("A NEW RUN UNDER TODAY'S RULES"), TopPill::Rescore),
                    ("PAUSE", Pill::new(Tone::Gold), LabButton::Run(run::RunButton::Pause), None, TopPill::Pause),
                    ("RESUME", primary, LabButton::Run(run::RunButton::Resume), None, TopPill::Resume),
                ];
                for (text, look, button, tip, top) in pills {
                    let e = match tip {
                        Some(t) => top_pill(tr, fonts, text, look, (button, Tip::new(t))),
                        None => top_pill(tr, fonts, text, look, button),
                    };
                    tr.commands().entity(e).insert(top);
                }
                spawn_close(tr, fonts, LabButton::Back);
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use world::evolution::RunEntry;

    /// On a phone every page's columns (left, width, top) land inside the window's width and the page's content.
    #[test]
    fn the_pages_reflow_inside_a_phone() {
        let columns = [
            (Page::Runs, vec![(60.0, 1160.0, 100.0)]),
            (Page::Setup, vec![(36.0, 600.0, 90.0), (664.0, 580.0, 90.0)]),
            (Page::Run, vec![(36.0, 1208.0, 88.0), (36.0, 380.0, 144.0), (444.0, 392.0, 138.0), (864.0, 380.0, 140.0)]),
            (Page::Ladder, vec![(60.0, 600.0, 100.0), (690.0, 530.0, 100.0)]),
        ];
        for (page, cols) in columns {
            let flow = page.reflow();
            for (x, w, y) in cols {
                let (_, by, max) = flow.regions.iter().find(|(r, ..)| r.contains(Vec2::new(x, y))).unwrap();
                let w = f32::min(w, max.x);
                assert!(x + by.x >= 0.0 && x + by.x + w <= COMPACT_STAGE.x, "{page:?} at {x}");
                assert!(y + by.y >= 0.0 && y + by.y < flow.height, "{page:?} at {x}");
            }
        }
    }

    /// The building's words follow the lab: locked until its research node, no run, a run's generations and best.
    #[test]
    fn the_building_says_what_the_lab_does() {
        let mut lab = Lab::default();
        assert_eq!((light(&lab), building_line(&lab).0.as_str()), (Evo::Dark, "LOCKED"));
        assert_eq!(unlocked_by(&data::base_content()).map(|n| n.id.as_str()), Some("simulation_vault"));
        lab.enabled = true;
        assert_eq!((light(&lab), building_line(&lab)), (Evo::Idle, ("NO RUN".into(), Tone::Label)));
        assert_eq!(score(Some(0.8125)), "0.81");
        assert_eq!(score(None), "—");
        // Each line fits the room above the mind (its label is at most 160 px wide: about 24 characters).
        let e = RunEntry {
            id: "a".into(),
            name: "A".into(),
            created: None,
            status: RunStatus::Paused,
            generation: 100,
            generations: 100,
            best: Some(0.5),
            first: None,
            starts: Vec::new(),
            race: None,
            stale: false,
            problem: None,
        };
        for line in [format!("{} / {}  ·  BEST {}", e.generation, e.generations, score(e.best)), format!("PAUSED  ·  {} / {}", e.generation, e.generations), format!("DONE  ·  BEST {}", score(e.best))] {
            assert!(line.chars().count() <= 24, "{line}");
        }
    }
}
