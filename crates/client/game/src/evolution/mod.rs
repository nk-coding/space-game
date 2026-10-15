//! The EVOLUTION layer over home (GDR-0092 step 4): the evolution lab's screens. The runs themselves, their
//! background work and their storage are `world::evolution`; this is what the player sees of them.
//!
//! ```text
//! (o) SHIPYARD › EVOLUTION                                       [NEW RUN]  [CLOSE]
//!     Evolution
//!  RUNS: name · starts · status · generation · start → best · made   [OPEN] [PAUSE] [DELETE]
//!  SETUP (NEW RUN): START FROM · LOCKS · LIMITS · MAY CHANGE · TRIALS · SIZE · SEED · NAME      [START RUN]
//!  THE RUN: SCORES BY GENERATION | THE ARCHIVE | the picked design: FIGHTS (WATCH) · LINEAGE · COMPARE · SAVE AS DESIGN
//! ```
//!
//! * The building above the mind (E, a click, or EVOLVE THIS DESIGN in the drydock's ⋯ menu, which opens the setup
//!   with the working design as its start) opens it; the ring top left, CLOSE and Esc go back a page, and close it
//!   from RUNS. It is a layer over home, like FLEET and the LOG.
//! * **RUNS** ([`runs`]): every run, newest first, with OPEN, PAUSE / RESUME and DELETE (asked in the row first).
//! * **SETUP** ([`setup`]): one to four starts from YOUR DESIGNS and the templates, their locks (whole parts, KEEP
//!   ALL of a category, rules, the target line, LOCK ORDER), the career's limits (build points, the hold,
//!   voyage-ready, mirror; only the unlocked parts), what may change, the ENEMY SHIPS trial and the run's hall of
//!   fame, the size, the seed, the name; START RUN.
//! * **THE RUN** ([`run`]): its progress (generation n of N and the open one, the measured fights per second and
//!   the time left, start → best), PAUSE / RESUME, the scores by generation, the archive as ship cards, and the
//!   picked design: its fights with WATCH (the fight viewer; M brings the lab back), its lineage, a comparison with
//!   its start, SAVE AS DESIGN.
//! * The building's label says what the lab does ([`building_line`]); its look follows [`light`].
//!
//! UI words are chrome in code, like FLEET's and the LOG's; names come from content and the player's designs. The
//! lab's own content (`lab.ron`: size presets, measures' names, budgets) comes with GDR-0092 step 8.

mod run;
mod runs;
mod setup;

use world::arena::evolve::RunStatus;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use kit::clipboard::Clipboard;
use kit::style::{Style, ui_color};
use kit::ui_icons::{Glyph, UiIcon};
use kit::ui_kit::{Edge, HudFonts, Pill, ScreenFx, ScreenPart, TextField, Tip, Tone, UiClick, UiInputSet, UiStyleSet, label, prose, ring_node, set_color, set_text, show_screen, spawn_pill};
use world::career::{EscortDesigns, Granted};
use world::evolution::Lab;
use world::mode::AppMode;
use world::player::PlayerDesign;
use world::player::behaviors::PlayerBehaviors;
use world::replay::Replay;
use world::scene::Scene;
use world::sim_bridge::GameContent;
use world::storage::Storage;

use crate::home::HomeAction;
use crate::shipyard::draw::Evo;

pub use setup::SetupPage;

/// The layout's size (logical px), like FLEET's.
const STAGE: Vec2 = Vec2::new(1280.0, 720.0);

/// The layer's pages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Runs,
    Setup,
    Run,
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

/// How the building looks (GDR-0092 "Its states"): dark until the lab opens (`?lab=1`), busy while a run computes,
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

/// The line under the building's name and its tone, short enough for the room above the mind: `12 / 50  ·  BEST
/// 0.81` while a run computes, `PAUSED  ·  12 / 50`, `DONE  ·  BEST 0.91`, `NO RUN`, and `LATER` while it is dark.
pub fn building_line(lab: &Lab) -> (String, Tone) {
    let entry = |id: Option<&str>| id.and_then(|id| lab.entry(id));
    match light(lab) {
        Evo::Dark => ("LATER".into(), Tone::Dim),
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

#[derive(Component)]
struct LayerRoot;

/// The pages' roots.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
struct PageNode(Page);

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
    Pause,
    Resume,
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
    KeepRun,
    Setup(setup::SetupButton),
    Run(run::RunButton),
}

/// What the pages' clicks change: the lab and its storage, and what a run is made from or leads to.
#[derive(SystemParam)]
pub(crate) struct Ctx<'w> {
    lab: ResMut<'w, Lab>,
    storage: ResMut<'w, Storage>,
    content: Res<'w, GameContent>,
    design: Res<'w, PlayerDesign>,
    behaviors: ResMut<'w, PlayerBehaviors>,
    granted: Res<'w, Granted>,
    escorts: Res<'w, EscortDesigns>,
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
            .add_systems(OnExit(AppMode::Home), |mut layer: ResMut<EvolutionLayer>, mut lab: ResMut<Lab>| {
                layer.open = false;
                lab.layer_open = false;
            })
            .add_systems(Update, show.run_if(state_changed::<AppMode>.or_eager(resource_changed::<EvolutionLayer>)))
            .add_systems(
                Update,
                (open, input, (runs::fill, setup::fill, run::fill), (sync, run::sync)).chain().after(UiInputSet).before(UiStyleSet).run_if(in_state(AppMode::Home)),
            );
    }
}

/// The building (E, a click; EVOLVE THIS DESIGN and coming back from WATCH send the same): open on RUNS, on the
/// setup with the drydock's design as its start, or where it was when a fight was watched.
fn open(
    mut actions: MessageReader<HomeAction>,
    mut layer: ResMut<EvolutionLayer>,
    mut lab: ResMut<Lab>,
    storage: Res<Storage>,
    content: Res<GameContent>,
    behaviors: Res<PlayerBehaviors>,
    design: Res<PlayerDesign>,
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
        Some(start) => {
            layer.setup = Some(SetupPage::new(&content.0, &storage, &behaviors, &design, vec![start]));
            layer.go(Page::Setup);
        }
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
                Page::Setup | Page::Run => {
                    let _ = ctx.lab.view(&ctx.storage, &ctx.content.0, None);
                    layer.go(Page::Runs);
                }
            },
            LabButton::NewRun => {
                let start = world::evolution::StartPick::Working;
                layer.setup = Some(SetupPage::new(&ctx.content.0, &ctx.storage, &ctx.behaviors, &ctx.design, vec![start]));
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
            TopPill::NewRun => layer.page == Page::Runs,
            TopPill::Pause => layer.page == Page::Run && computes,
            TopPill::Resume => layer.page == Page::Run && can_resume,
        };
        let d = if on { Display::Flex } else { Display::None };
        if node.display != d {
            node.display = d;
        }
    }
}

/// What the status line says while nothing happened.
fn default_status(page: Page) -> &'static str {
    match page {
        Page::Runs => "A RUN BREEDS YOUR DESIGNS AGAINST ENEMIES AND KEEPS THE BEST OF EACH KIND  ·  ONE COMPUTES AT A TIME, AT HOME AND ON EVERY SCREEN, NEVER IN FLIGHT",
        Page::Setup => "SAME SEED, SAME SETUP, SAME RULES: THE SAME RUN",
        Page::Run => "CLICK A DESIGN IN THE ARCHIVE  ·  EVERY FIGHT CAN BE WATCHED AGAIN",
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

fn spawn(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let fonts = &*fonts;
    commands
        .spawn((
            LayerRoot,
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
                centre.spawn(Node { width: px(STAGE.x), height: px(STAGE.y), flex_shrink: 0.0, ..default() }).with_children(|stage| {
                    // Top left: the way back, where we are, the title.
                    stage.spawn((ScreenPart::new(Edge::Top, 0), on_row(abs(Vec2::new(22.0, 14.0), None), 14.0))).with_children(|tl| {
                        tl.spawn((ring_node(40.0), LabButton::Back, Tip::key("BACK", "ESC")))
                            .with_child((UiIcon::Glyph { glyph: Glyph::Menu, color: ui.violet }, Node { width: px(22), height: px(22), ..default() }));
                        tl.spawn(column(4.0)).with_children(|col| {
                            col.spawn((LayerText::Crumb, label(fonts, "SHIPYARD  ›  EVOLUTION", 10.0, c(Tone::Label, 1.0))));
                            col.spawn((LayerText::Title, prose(fonts, "Evolution", 22.0, c(Tone::Number, 1.0))));
                        });
                    });
                    // Top right: the page's pill, CLOSE.
                    stage.spawn((ScreenPart::new(Edge::Top, 0), Node { position_type: PositionType::Absolute, right: px(20), top: px(19), ..row(10.0) })).with_children(|tr| {
                        let primary = Pill { primary: true, ..Pill::new(Tone::Teal) };
                        let new_run = spawn_pill(tr, fonts, "NEW RUN", 9.5, 30.0, None, primary, (LabButton::NewRun, Tip::new("SET UP A NEW RUN")));
                        tr.commands().entity(new_run).insert(TopPill::NewRun);
                        let pause = spawn_pill(tr, fonts, "PAUSE", 9.5, 30.0, None, Pill::new(Tone::Gold), LabButton::Run(run::RunButton::Pause));
                        tr.commands().entity(pause).insert(TopPill::Pause);
                        let resume = spawn_pill(tr, fonts, "RESUME", 9.5, 30.0, None, primary, LabButton::Run(run::RunButton::Resume));
                        tr.commands().entity(resume).insert(TopPill::Resume);
                        tr.spawn(Node { width: px(6), ..default() });
                        spawn_pill(tr, fonts, "CLOSE", 9.5, 30.0, None, Pill::new(Tone::Teal), (LabButton::Back, Tip::key("CLOSE", "ESC")));
                    });
                    for page in [Page::Runs, Page::Setup, Page::Run] {
                        let node = Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() };
                        stage.spawn((PageNode(page), ScreenPart::new(Edge::Fade, 1), node)).with_children(|p| match page {
                            Page::Runs => runs::spawn(p, fonts, &c),
                            Page::Setup => setup::spawn(p, fonts, &c, &ui),
                            Page::Run => run::spawn(p, fonts, &c),
                        });
                    }
                    stage.spawn((LayerText::Status, ScreenPart::new(Edge::Bottom, 3), label(fonts, "", 9.0, c(Tone::Label, 1.0)), abs(Vec2::new(36.0, 690.0), Some(Vec2::new(1208.0, 20.0)))));
                });
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use world::evolution::RunEntry;

    /// The building's words follow the lab: dark without `?lab=1`, no run, a run's generations and best.
    #[test]
    fn the_building_says_what_the_lab_does() {
        let mut lab = Lab::default();
        assert_eq!((light(&lab), building_line(&lab).0.as_str()), (Evo::Dark, "LATER"));
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
            stale: false,
            problem: None,
        };
        for line in [format!("{} / {}  ·  BEST {}", e.generation, e.generations, score(e.best)), format!("PAUSED  ·  {} / {}", e.generation, e.generations), format!("DONE  ·  BEST {}", score(e.best))] {
            assert!(line.chars().count() <= 24, "{line}");
        }
    }
}
