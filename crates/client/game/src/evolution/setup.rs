//! SETUP (NEW RUN, GDR-0092 "The run setup", what steps 4 to 6 have of it): one page, two columns.
//!
//! ```text
//! START FROM · PICK 1 TO 4                       LOCKS · RIM SKIFF COPY
//! [1 RIM SKIFF COPY] [2 HALVEX RUNNER]           KEEP ALL [HULL] [POWER] [WEAPONS] ...
//! YOUR DESIGNS            TEMPLATES              SHIP (a grid)  PARTS        [RULES] [ASSISTS] (a lock each)
//! LIMITS: BUILD POINTS, HOLD, VOYAGE-READY,       ▦▦▦▦          (a lock      [ALL] [TARGET LINE] [LOCK ORDER]
//!   MIRROR                                                       each)
//! MAY CHANGE: [EVERYTHING] [DESIGN ONLY] ...     TRIALS · WEIGHT · SEEDS         [WITH YOUR ESCORTS]
//!   [LAYOUT] [PARTS] [ARMOUR] [POWER]            [THE ENEMIES YOU MEET] (−) 2 (+)  (−) 4 (+)   16 FIGHTS ...
//!   [BEHAVIOR NUMBERS] ... [ASSIST RULES]        GOALS [WIN] [WIN UNHURT] ...; WIN (−) 1 (+) UNHURT ...
//!   PARTS OF [HULL] [POWER] [WEAPONS] ...        THE RUN: fights a child, a generation, the estimate
//!   FLOWN BY [BEHAVIOR] [STAND-IN] [BOTH] · [CASUAL] ...
//! SIZE: [QUICK] [STANDARD] [DEEP], SEED, DICE    NAME [__________]                   [START RUN]
//!   CHILDREN (−) 32 (+) GENERATIONS (−) 50 (+)
//! ```
//!
//! * **Starts**: a click on a row of YOUR DESIGNS or TEMPLATES adds it (at most four) or takes it out; a click on a
//!   start's chip shows its locks. The drydock's EVOLVE THIS DESIGN opens the page with the working design picked.
//! * **Locks** (per start): KEEP ALL of a category; the start drawn as a grid of its cells (and two around it), where
//!   a click on a part locks it (gold) or frees it and a click on an empty cell keeps it empty (coral, KEEP EMPTY: a
//!   clear line of fire) or frees it, each cell's tip naming it; the parts listed, a lock each; the behavior's rules
//!   and the pilot assists (a tab each), a lock each, ALL of the list, the target line, LOCK ORDER. Under MIRROR the
//!   arena locks a lock's mirror image too.
//! * **Limits**: build points at most the start's (the default), a number or no cap; the hold at least the start's
//!   or any; VOYAGE-READY (logic cores within the mind capacity less FLEET's escorts'); MIRROR (on when the first
//!   start looks symmetric, a single part and the one across from it aside, which the arena keeps in place); only the career's parts, always (`?unlock=all` lifts it), said in LIMITS' head.
//! * **May change** (GDR-0092 step 6): `lab.ron`'s gene presets (EVERYTHING, the default, DESIGN ONLY, BEHAVIOR ONLY,
//!   ASSISTS ONLY, NUMBERS ONLY; one lit while the genes are its), each gene on or off (`lab.ron`'s words, its summary
//!   as the tip), the part categories whose parts may be added, removed or swapped, and FLOWN BY: the behavior, a
//!   stand-in pilot or both (odd seeds and even seeds) in every fight trial, and which `lab.ron` pilot; a preset with
//!   the assists sets BOTH, one without them the behavior (only a pilot uses assists).
//! * **Trials** (GDR-0092 step 5): every `lab.ron` preset, on or off (its default selection on), its weight (0 to 3)
//!   and seeds, ENEMY SHIPS' ships, HALL OF FAME's and CHAMPIONS' count and TEST-RANGE WAVES' target; each row says
//!   how many fights a child gets of it, or why it has nothing to fight (no open voyage, no saved design, no earlier
//!   run). WITH YOUR ESCORTS flies FLEET's escorts beside the design in every trial. The ARMS RACE (step 7, off by
//!   default) has an enemy population evolve against yours: under its row the enemy ships it starts from (the first
//!   enemy faction's, its boss only when picked) and its size.
//! * **Goals**: the goals' weights (0 to 3) and `lab.ron`'s goal presets (WIN, WIN UNHURT, ...).
//! * **The estimate**: the fights a child and a generation get, and the run's time on this device with the layer open
//!   (from the measured cost of a tick and each kind's measured share of its time limit).
//! * **START RUN** builds the run (`world::evolution::Lab::create`): it computes from now on and the page of the run
//!   opens; a setup the arena refuses says why in the status line.

use world::arena::evolve::{Bound, Gene, Goals, Locks, MayChange, Sources};
use bevy::prelude::*;
use data::Content;
use data::behavior::Program;
use kit::style::{Style, UiPalette, ui_color};
use kit::ui_icons::Glyph;
use kit::ui_kit::{HudFonts, Hot, Pill, Scrollbar, TextField, Tone, field_text, label, number, prose, ring_glyph, ring_node, spawn_pill, sub_tone, tracked};
use world::career::fleet_cores;
use world::evolution::{Flier, Lab, Setup, Size, StartDesign, StartPick, sources, templates};
use world::player::PlayerDesign;
use world::player::behaviors::PlayerBehaviors;
use world::player::library::{self, MAX_NAME, fresh_name, list_designs};
use world::storage::{Storage, now_unix};

use super::{Ctx, EvolutionLayer, LabButton, Page, abs, on_row, row};

/// Most starts a run has (`world::arena::evolve::MAX_STARTS`).
const MAX_STARTS: usize = world::arena::evolve::MAX_STARTS;
/// The columns' left edges and widths, the lists' heights (the starts', the locks') and a row's.
const LEFT: (f32, f32) = (36.0, 600.0);
const RIGHT: (f32, f32) = (664.0, 580.0);
const PICK_H: f32 = 104.0;
const LIST_H: f32 = 130.0;
const LIST_ROW: f32 = 30.0;
/// The locks' grid: its square's side, a cell's largest side, and the empty cells shown around the start.
const GRID: f32 = 130.0;
const CELL_MAX: f32 = 16.0;
const MARGIN: i32 = 2;
/// The trials' list: its top and height, and a row's height; the widths of a row's columns (the trial, its weight,
/// its seeds, its count).
const TRIALS_TOP: f32 = 336.0;
const TRIALS_H: f32 = 164.0;
const TRIAL_ROW: f32 = 26.0;
const TRIAL_COLUMNS: [f32; 4] = [178.0, 84.0, 84.0, 118.0];
/// The steppers' steps and bounds (PLACEHOLDER, like the presets).
const BP_STEP: f32 = 5.0;
const POPULATION: (u32, u32, u32) = (4, 4, 256);
const GENERATIONS: (u32, u32, u32) = (5, 1, 500);
const SEEDS: (u32, u32) = (1, 12);
const WEIGHT_STEP: f32 = 0.5;
const COUNT: (u32, u32) = (1, 16);
const OPPONENTS: u32 = 6;

/// What the setup page holds: the setup and what it is picked from.
pub struct SetupPage {
    pub setup: Setup,
    /// YOUR DESIGNS as listed: id, name and the line under it.
    designs: Vec<(String, String, String)>,
    /// The templates (content ship ids).
    templates: Vec<String>,
    /// The starts resolved as picked: their parts and rules for the locks, or why they don't read.
    resolved: Vec<Result<StartDesign, String>>,
    /// The start whose locks show, and which of its lists of rules.
    selected: usize,
    list: Lists,
    /// The name field holds a name of the player's: picking starts no longer renames the run.
    named: bool,
    /// The first start's build points and hold (the limits' words).
    start_bp: Option<f32>,
    start_hold: Option<u32>,
    /// What the trials fight from outside the run: the career's open voyages, YOUR DESIGNS, earlier runs, the escorts.
    sources: Sources,
}

impl SetupPage {
    /// The setup with the defaults and these starts, the lists read fresh.
    pub fn new(content: &Content, storage: &Storage, behaviors: &PlayerBehaviors, design: &PlayerDesign, career: &data::Career, lab: &Lab, starts: Vec<StartPick>) -> SetupPage {
        let designs = list_designs(storage, content)
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.problem.is_none())
            .map(|e| {
                let mut sub = format!("{}×{}  ·  {} PARTS", e.size.0, e.size.1, e.parts);
                if let Some(t) = e.saved {
                    sub.push_str(&format!("  ·  {}", library::ago(now_unix(), t)));
                }
                (e.id, e.name, sub)
            })
            .collect();
        let mut page = SetupPage {
            setup: Setup::new(content, starts, now_unix() % 100_000),
            designs,
            templates: templates(content, &design.blueprint().faction),
            resolved: Vec::new(),
            selected: 0,
            list: Lists::Rules,
            named: false,
            start_bp: None,
            start_hold: None,
            sources: sources(storage, content, career, behaviors, design, lab),
        };
        page.resolve(content, storage, behaviors, design);
        page.setup.mirror = page.resolved.first().and_then(|r| r.as_ref().ok()).is_some_and(|s| symmetric(content, &s.blueprint));
        page
    }

    /// The starts resolved again, the run's default name, the first start's numbers.
    fn resolve(&mut self, content: &Content, storage: &Storage, behaviors: &PlayerBehaviors, design: &PlayerDesign) {
        self.resolved = self.setup.starts.iter().map(|(p, _)| p.resolve(storage, content, behaviors, design)).collect();
        self.selected = self.selected.min(self.setup.starts.len().saturating_sub(1));
        let first = self.resolved.first().and_then(|r| r.as_ref().ok());
        self.start_bp = first.and_then(|s| sim::design_report(content, &s.blueprint, &sim::SimConfig::default()).ok()).map(|r| r.build_points);
        self.start_hold = first.map(|s| content.ship_hold(&s.blueprint));
        if !self.named {
            self.setup.name = first.map_or_else(|| "New run".to_string(), |s| format!("Evolving {}", s.name));
        }
    }

    /// Adds a start, or takes it out when it is picked already (at most [`MAX_STARTS`]).
    fn toggle(&mut self, pick: StartPick) -> Result<(), String> {
        if let Some(k) = self.setup.starts.iter().position(|(p, _)| *p == pick) {
            self.setup.starts.remove(k);
            return Ok(());
        }
        if self.setup.starts.len() >= MAX_STARTS {
            return Err(format!("a run has at most {MAX_STARTS} starts"));
        }
        self.setup.starts.push((pick, Locks::default()));
        self.selected = self.setup.starts.len() - 1;
        Ok(())
    }
}

/// Every part has its mirror image across the frame's centre line (the same part, material and pose mirrored) when
/// the design opens centred in the first frame it fits, but at most one single part and the part across from it
/// (the starter's scanner, GDR-0093; the arena keeps such parts where they are, `arena::evolve::unmatched`):
/// MIRROR's default. The arena checks the run's starts again.
fn symmetric(content: &Content, bp: &data::Blueprint) -> bool {
    let Ok(d) = library::open_in_first_frame(content, bp) else { return false };
    world::arena::evolve::unmatched(content, d.dock()).len() <= 2
}

/// The locks' lists of rules: the behavior's or the pilot assists'.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lists {
    Rules,
    Assists,
}

/// A click on the setup page.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SetupButton {
    Design(String),
    Template(String),
    /// A start's chip: its locks show.
    Start(usize),
    Category(String),
    Part(u32),
    /// A cell of the locks' grid (the start's coordinates): its part's lock, or KEEP EMPTY.
    Cell(i32, i32),
    List(Lists),
    Rule(u32),
    Assist(u32),
    /// Every rule of the list shown locked, or none.
    AllRules,
    Target,
    Order,
    /// BUILD POINTS: as the start, no cap, a step up or down from what it is.
    BpStart,
    BpOff,
    BpStep(i32),
    Hold,
    VoyageReady,
    Mirror,
    /// MAY CHANGE: a gene preset (its index in `lab.ron`), a gene, a part category kept; FLOWN BY and its pilot.
    GenePreset(usize),
    Gene(Gene),
    Kept(String),
    Flier(Flier),
    Pilot(String),
    /// A trial (its index in the setup): on or off, its weight, its seeds, its count, an enemy ship's count (the ARMS
    /// RACE's: whether its enemy starts from it).
    Trial(usize),
    Weight(usize, i32),
    TrialSeeds(usize, i32),
    Count(usize, i32),
    Ship(usize, usize, i32),
    Escorts,
    /// A goal's weight (its index in `data::GOALS`), a goal preset (its index in `lab.ron`).
    Goal(usize, i32),
    GoalPreset(usize),
    Size(usize),
    Population(i32),
    Generations(i32),
    Seed(i64),
    Dice,
    StartRun,
}

/// The containers the page fills again when it changes; the lists keep their scroll.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum Section {
    Chips,
    Designs,
    Templates,
    Limits,
    Genes,
    Size,
    LocksHead,
    Grid,
    Parts,
    ListsHead,
    Rules,
    RuleLocks,
    TrialsHead,
    Trials,
    Goals,
    Estimate,
}

/// The run's name field.
#[derive(Component)]
pub(super) struct NameField;

pub(super) fn spawn(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, ui: &UiPalette) {
    let (lx, lw) = LEFT;
    let (rx, rw) = RIGHT;
    let head = |p: &mut ChildSpawnerCommands, at: Vec2, words: &str| {
        p.spawn((label(fonts, words, 9.0, c(Tone::Dim, 1.0)), abs(at, None)));
    };
    let section = |p: &mut ChildSpawnerCommands, s: Section, at: Vec2, size: Vec2| {
        p.spawn((s, Node { flex_direction: FlexDirection::Column, row_gap: px(6), ..abs(at, Some(size)) }));
    };
    let list = |p: &mut ChildSpawnerCommands, s: Section, at: Vec2, w: f32, h: f32| {
        p.spawn((
            s,
            Scrollbar::vertical(),
            ScrollPosition::default(),
            Node { flex_direction: FlexDirection::Column, row_gap: px(2), overflow: Overflow::scroll_y(), padding: UiRect::right(px(10)), ..abs(at, Some(Vec2::new(w, h))) },
        ));
    };
    let lists = 162.0 + LIST_H;
    let picks = 162.0 + PICK_H;
    // The left column: the starts, the limits, what may change, the size and the seed.
    head(p, Vec2::new(lx, 90.0), "START FROM  ·  PICK 1 TO 4  ·  A CLICK ON A START SHOWS ITS LOCKS");
    section(p, Section::Chips, Vec2::new(lx, 108.0), Vec2::new(lw, 30.0));
    head(p, Vec2::new(lx, 146.0), "YOUR DESIGNS");
    list(p, Section::Designs, Vec2::new(lx, 162.0), 300.0, PICK_H);
    head(p, Vec2::new(lx + 316.0, 146.0), "TEMPLATES");
    list(p, Section::Templates, Vec2::new(lx + 316.0, 162.0), lw - 316.0, PICK_H);
    section(p, Section::Limits, Vec2::new(lx, picks + 10.0), Vec2::new(lw, 142.0));
    head(p, Vec2::new(lx, picks + 162.0), "MAY CHANGE  ·  THE LOCKS HOLD WITHIN IT");
    section(p, Section::Genes, Vec2::new(lx, picks + 178.0), Vec2::new(lw, 154.0));
    head(p, Vec2::new(lx, picks + 340.0), "SIZE");
    section(p, Section::Size, Vec2::new(lx, picks + 356.0), Vec2::new(lw, 58.0));
    // The right column: the locks, the trials and goals, the run in numbers, the name, START RUN.
    section(p, Section::LocksHead, Vec2::new(rx, 90.0), Vec2::new(rw, 50.0));
    head(p, Vec2::new(rx, 146.0), "LOCK A PART  ·  KEEP A CELL EMPTY");
    p.spawn((Section::Grid, Node { justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..abs(Vec2::new(rx, 162.0), Some(Vec2::splat(GRID))) }));
    list(p, Section::Parts, Vec2::new(rx + GRID + 8.0, 162.0), 282.0 - GRID - 8.0, LIST_H);
    p.spawn((Section::ListsHead, on_row(abs(Vec2::new(rx + 298.0, 141.0), Some(Vec2::new(rw - 298.0, 20.0))), 6.0)));
    list(p, Section::Rules, Vec2::new(rx + 298.0, 162.0), rw - 298.0, LIST_H - 34.0);
    section(p, Section::RuleLocks, Vec2::new(rx + 298.0, 162.0 + LIST_H - 28.0), Vec2::new(rw - 298.0, 30.0));
    p.spawn((Section::TrialsHead, Node { justify_content: JustifyContent::SpaceBetween, ..on_row(abs(Vec2::new(rx, lists + 6.0), Some(Vec2::new(rw, 38.0))), 8.0) }));
    list(p, Section::Trials, Vec2::new(rx, TRIALS_TOP), rw, TRIALS_H);
    head(p, Vec2::new(rx, TRIALS_TOP + TRIALS_H + 8.0), "GOALS  ·  WHAT A SCORE WEIGHS");
    section(p, Section::Goals, Vec2::new(rx, TRIALS_TOP + TRIALS_H + 24.0), Vec2::new(rw, 58.0));
    p.spawn((Section::Estimate, Node { overflow: Overflow::clip(), ..abs(Vec2::new(rx, TRIALS_TOP + TRIALS_H + 90.0), Some(Vec2::new(rw, 18.0))) }));
    p.spawn(Node { ..abs(Vec2::new(rx, 618.0), Some(Vec2::new(rw, 58.0))) }).with_children(|n| {
        n.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(6), ..default() }).with_children(|col| {
            col.spawn(label(fonts, "NAME", 9.0, c(Tone::Dim, 1.0)));
            col.spawn((
                NameField,
                TextField { placeholder: "Name of the run".into(), max_chars: MAX_NAME, ..default() },
                Hot::default(),
                Node {
                    width: px(330),
                    height: px(32),
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(px(14)),
                    border: UiRect::all(px(1.2)),
                    border_radius: BorderRadius::MAX,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BorderColor::all(c(Tone::Label, 0.3)),
            ))
            .with_children(|f| {
                f.spawn(field_text(fonts, 14.0));
            });
        });
        n.spawn(Node { position_type: PositionType::Absolute, right: px(0), top: px(16), ..default() }).with_children(|b| {
            let play = kit::ui_icons::UiIcon::Glyph { glyph: Glyph::Play, color: ui.teal };
            spawn_pill(b, fonts, "START RUN", 12.0, 40.0, Some(play), Pill { primary: true, ..Pill::new(Tone::Teal) }, LabButton::Setup(SetupButton::StartRun));
        });
    });
}

/// Applies a click.
pub(super) fn press(layer: &mut EvolutionLayer, ctx: &mut Ctx, b: SetupButton) {
    let Some(page) = layer.setup.as_mut() else { return };
    let content = &ctx.content.0;
    let s = &mut page.setup;
    let step = |v: u32, by: i32, (lo, hi): (u32, u32)| (v as i64 + by as i64).clamp(lo as i64, hi as i64) as u32;
    let mut starts_changed = false;
    let mut say = None;
    fn locks(s: &mut Setup, k: usize) -> Option<&mut Locks> {
        s.starts.get_mut(k).map(|(_, l)| l)
    }
    match b {
        SetupButton::Design(id) => match page.toggle(StartPick::Design(id)) {
            Ok(()) => starts_changed = true,
            Err(e) => say = Some((e, Tone::Coral)),
        },
        SetupButton::Template(id) => match page.toggle(StartPick::Template(id)) {
            Ok(()) => starts_changed = true,
            Err(e) => say = Some((e, Tone::Coral)),
        },
        SetupButton::Start(k) => page.selected = k,
        SetupButton::Category(id) => {
            if let Some(l) = locks(s, page.selected) {
                flip(&mut l.categories, id);
            }
        }
        SetupButton::Part(k) => {
            if let Some(l) = locks(s, page.selected) {
                flip(&mut l.parts, k);
            }
        }
        SetupButton::Cell(x, y) => {
            // A part's cell locks or frees the part (one kept with its category stays kept); an empty one is kept
            // empty or freed.
            let start = page.resolved.get(page.selected).and_then(|r| r.as_ref().ok());
            let at = start.and_then(|st| st.blueprint.parts.iter().position(|q| content.placed_shape(q).is_some_and(|sh| sh.cells().contains(&(x, y)))));
            let by_category = |l: &Locks| at.zip(start).is_some_and(|(k, st)| content.parts.get(&st.blueprint.parts[k].part).is_some_and(|d| l.categories.contains(&d.category)));
            if let Some(l) = locks(s, page.selected) {
                match at {
                    Some(_) if by_category(l) => say = Some(("it is kept with its category".to_string(), Tone::Gold)),
                    Some(k) => flip(&mut l.parts, k as u32),
                    None => flip(&mut l.cells, (x, y)),
                }
            }
        }
        SetupButton::List(list) => page.list = list,
        SetupButton::Rule(k) => {
            if let Some(l) = locks(s, page.selected) {
                flip(&mut l.rules, k);
            }
        }
        SetupButton::Assist(k) => {
            if let Some(l) = locks(s, page.selected) {
                flip(&mut l.assists, k);
            }
        }
        SetupButton::AllRules => {
            let n = page.resolved.get(page.selected).and_then(|r| r.as_ref().ok()).map_or(0, |st| match page.list {
                Lists::Rules => Program::parse(&st.program).map_or(0, |p| p.rules.len()),
                Lists::Assists => st.blueprint.assists.as_ref().map_or(0, |a| a.rules.len()),
            });
            if let Some(l) = locks(s, page.selected) {
                let list = match page.list {
                    Lists::Rules => &mut l.rules,
                    Lists::Assists => &mut l.assists,
                };
                *list = if list.len() == n { Vec::new() } else { (0..n as u32).collect() };
            }
        }
        SetupButton::Target => {
            if let Some(l) = locks(s, page.selected) {
                l.target = !l.target;
            }
        }
        SetupButton::Order => {
            if let Some(l) = locks(s, page.selected) {
                l.order = !l.order;
            }
        }
        SetupButton::BpStart => s.build_points = Bound::Start,
        SetupButton::BpOff => s.build_points = Bound::Off,
        SetupButton::BpStep(by) => {
            let now = match s.build_points {
                Bound::At(v) => v,
                _ => page.start_bp.unwrap_or(60.0),
            };
            s.build_points = Bound::At((((now / BP_STEP).round() + by as f32) * BP_STEP).max(BP_STEP));
        }
        SetupButton::Hold => s.min_hold = if s.min_hold == Bound::Start { Bound::Off } else { Bound::Start },
        SetupButton::VoyageReady => s.voyage_ready = !s.voyage_ready,
        SetupButton::Mirror => s.mirror = !s.mirror,
        SetupButton::GenePreset(k) => {
            if let Some(p) = content.lab.gene_presets.get(k) {
                s.may_change = MayChange { fixed: s.may_change.fixed.clone(), ..MayChange::only(&p.genes) };
                s.flown_by = Flier::default_for(&s.may_change);
            }
        }
        SetupButton::Gene(g) => {
            let on = !s.may_change.has(g);
            s.may_change.set(g, on);
            if Gene::ALL.iter().all(|g| !s.may_change.has(*g)) {
                s.may_change.set(g, true);
                say = Some(("one gene at least may change".to_string(), Tone::Coral));
            }
        }
        SetupButton::Kept(id) => flip(&mut s.may_change.fixed, id),
        SetupButton::Flier(f) => s.flown_by = f,
        SetupButton::Pilot(id) => s.pilot = id,
        SetupButton::Trial(k) => {
            if let Some(t) = s.trials.get_mut(k) {
                t.on = !t.on;
            }
        }
        SetupButton::Weight(k, by) => {
            if let Some(t) = s.trials.get_mut(k) {
                t.weight = (t.weight + by as f32 * WEIGHT_STEP).clamp(0.0, data::MAX_LAB_WEIGHT);
            }
        }
        SetupButton::TrialSeeds(k, by) => {
            if let Some(t) = s.trials.get_mut(k) {
                t.seeds = step(t.seeds, by, SEEDS);
            }
        }
        SetupButton::Count(k, by) => {
            if let Some(n) = s.trials.get_mut(k).and_then(|t| t.count.as_mut()) {
                *n = step(*n, by, COUNT);
            }
        }
        SetupButton::Ship(k, ship, by) => {
            let race = s.trials.get(k).and_then(|t| ctx.content.0.lab.trial(&t.preset)).is_some_and(|p| matches!(p.kind, data::TrialPresetKind::CoEvolution { .. }));
            if let Some(o) = s.trials.get_mut(k).and_then(|t| t.ships.get_mut(ship)) {
                o.1 = step(o.1, by, (0, if race { 1 } else { OPPONENTS }));
            }
        }
        SetupButton::Escorts => s.escorts = !s.escorts,
        SetupButton::Goal(g, by) => {
            let mut w = s.goals.weights();
            if let Some(v) = w.get_mut(g) {
                *v = (*v + by as f32 * WEIGHT_STEP).clamp(0.0, data::MAX_LAB_WEIGHT);
            }
            // A score needs a goal to weigh.
            if w.iter().any(|v| *v > 0.0) {
                s.goals = Goals { win: w[0], unhurt: w[1], quick: w[2], cheap: w[3] };
            } else {
                say = Some(("one goal at least weighs something".to_string(), Tone::Coral));
            }
        }
        SetupButton::GoalPreset(k) => {
            if let Some(p) = content.lab.goal_presets.get(k) {
                s.goals = Goals::of(p.weights.clone());
            }
        }
        SetupButton::Size(k) => {
            if let Some(z) = content.lab.sizes.get(k) {
                s.size = Size::of(z);
            }
        }
        SetupButton::Population(by) => s.size.population = step(s.size.population, by * POPULATION.0 as i32, (POPULATION.1, POPULATION.2)),
        SetupButton::Generations(by) => s.size.generations = step(s.size.generations, by * GENERATIONS.0 as i32, (GENERATIONS.1, GENERATIONS.2)),
        SetupButton::Seed(by) => s.seed = s.seed.saturating_add_signed(by),
        SetupButton::Dice => s.seed = (now_unix().wrapping_mul(2_654_435_761) ^ s.seed.rotate_left(17)) % 1_000_000,
        SetupButton::StartRun => {
            start_run(layer, ctx);
            return;
        }
    }
    if starts_changed {
        page.resolve(content, &ctx.storage, &ctx.behaviors, &ctx.design);
    }
    if let Some((words, tone)) = say {
        layer.say(words, tone);
    } else {
        layer.say = None;
    }
    layer.dirty = true;
}

/// Adds `v` to the list, or takes it out.
fn flip<T: PartialEq>(list: &mut Vec<T>, v: T) {
    match list.iter().position(|x| *x == v) {
        Some(k) => {
            list.remove(k);
        }
        None => list.push(v),
    }
}

/// START RUN: the starts resolved now (the working design as it is), the run built and made; its page opens.
fn start_run(layer: &mut EvolutionLayer, ctx: &mut Ctx) {
    let Some(page) = layer.setup.as_mut() else { return };
    let content = &ctx.content.0;
    if page.setup.starts.is_empty() {
        layer.say("pick a start first", Tone::Coral);
        return;
    }
    page.resolve(content, &ctx.storage, &ctx.behaviors, &ctx.design);
    let starts: Result<Vec<StartDesign>, String> = page.resolved.iter().cloned().collect();
    let starts = match starts {
        Ok(s) => s,
        Err(e) => {
            layer.say(format!("a start doesn't read: {e}"), Tone::Coral);
            return;
        }
    };
    let escorts = fleet_cores(content, &data::Blueprint { parts: Vec::new(), ..ctx.design.blueprint().clone() }, &ctx.escorts.0);
    let free = ctx.granted.mind_capacity.saturating_sub(escorts);
    let config = match page.setup.config(content, &starts, &ctx.granted, free, &page.sources) {
        Ok(c) => c,
        Err(e) => {
            layer.say(format!("the run doesn't start: {e}"), Tone::Coral);
            return;
        }
    };
    let names: Vec<String> = ctx.lab.runs().iter().map(|e| e.name.clone()).collect();
    let name = page.setup.name.trim();
    let name = fresh_name(if name.is_empty() { "New run" } else { name }, &names);
    match ctx.lab.create(&mut ctx.storage, content, config, &name, now_unix()) {
        Ok(_) => {
            layer.run = super::run::RunPage::default();
            layer.say(format!("{name} computes  ·  at home and on every screen, never in flight"), Tone::Teal);
            layer.go(Page::Run);
        }
        Err(e) => layer.say(format!("the run doesn't start: {e}"), Tone::Coral),
    }
}

/// Builds the page's sections again when it changed; the name field follows the setup's name, and the setup the
/// field while it types.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn fill(
    mut commands: Commands,
    mut layer: ResMut<EvolutionLayer>,
    ctx: (Res<world::sim_bridge::GameContent>, Res<world::career::Granted>, Res<world::career::EscortDesigns>, Res<PlayerDesign>, Res<world::evolution::Lab>),
    fonts: Res<HudFonts>,
    style: Res<Style>,
    sections: Query<(Entity, &Section)>,
    mut field: Single<&mut TextField, With<NameField>>,
) {
    let (content, granted, escorts, design, lab) = ctx;
    if !layer.open || layer.page != Page::Setup {
        return;
    }
    // The name: typed into the field, or the default shown in it.
    if let Some(page) = layer.setup.as_ref() {
        if field.focused && field.value != page.setup.name {
            let typed = field.value.clone();
            let page = layer.bypass_change_detection().setup.as_mut().expect("the setup is open");
            page.setup.name = typed;
            page.named = true;
        } else if !field.focused && field.value != page.setup.name {
            field.value = page.setup.name.clone();
        }
    }
    if !layer.dirty {
        return;
    }
    layer.bypass_change_detection().dirty = false;
    let Some(page) = layer.setup.as_ref() else { return };
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let fonts = &*fonts;
    let content = &content.0;
    let free = granted.mind_capacity.saturating_sub(fleet_cores(content, &data::Blueprint { parts: Vec::new(), ..design.blueprint().clone() }, &escorts.0));
    for (e, s) in &sections {
        commands.entity(e).despawn_children();
        commands.entity(e).with_children(|p| match s {
            Section::Chips => chips(p, fonts, &c, page),
            Section::Designs => {
                if page.designs.is_empty() {
                    p.spawn(prose(fonts, "None saved yet: save a design in the drydock.", 12.0, c(Tone::Dim, 1.0)));
                }
                for (id, name, sub) in &page.designs {
                    let picked = page.setup.starts.iter().any(|(s, _)| *s == StartPick::Design(id.clone()));
                    pick_row(p, fonts, &c, name, sub, picked, SetupButton::Design(id.clone()));
                }
            }
            Section::Templates => {
                for id in &page.templates {
                    let Some(b) = content.ships.get(id) else { continue };
                    let picked = page.setup.starts.iter().any(|(s, _)| *s == StartPick::Template(id.clone()));
                    let (w, h) = library::parts_size(content, b);
                    pick_row(p, fonts, &c, &b.name, &format!("{w}×{h}  ·  {} PARTS", b.parts.len()), picked, SetupButton::Template(id.clone()));
                }
            }
            Section::Limits => limits(p, fonts, &c, page, &granted, free),
            Section::Genes => genes(p, fonts, &c, content, page),
            Section::Size => size(p, fonts, &c, content, page),
            Section::LocksHead => locks_head(p, fonts, &c, content, page),
            Section::Grid => grid(p, &c, content, page),
            Section::Parts => parts(p, fonts, &c, content, page),
            Section::ListsHead => {
                for (list, words) in [(Lists::Rules, "RULES"), (Lists::Assists, "ASSISTS")] {
                    spawn_pill(p, fonts, words, 7.0, 18.0, None, Pill { active: page.list == list, ..Pill::new(Tone::Teal) }, LabButton::Setup(SetupButton::List(list)));
                }
                p.spawn((label(fonts, "A LOCKED ONE STAYS", 8.0, c(Tone::Dim, 1.0)), Node { margin: UiRect::left(px(4)), ..default() }));
            }
            Section::Rules => match page.list {
                Lists::Rules => rules(p, fonts, &c, page),
                Lists::Assists => assists(p, fonts, &c, page),
            },
            Section::RuleLocks => {
                let l = page.setup.starts.get(page.selected).map(|(_, l)| l.clone()).unwrap_or_default();
                let start = page.resolved.get(page.selected).and_then(|r| r.as_ref().ok());
                let n = start.map_or(0, |st| match page.list {
                    Lists::Rules => Program::parse(&st.program).map_or(0, |p| p.rules.len()),
                    Lists::Assists => st.blueprint.assists.as_ref().map_or(0, |a| a.rules.len()),
                });
                let locked = match page.list {
                    Lists::Rules => l.rules.len(),
                    Lists::Assists => l.assists.len(),
                };
                p.spawn(row(8.0)).with_children(|r| {
                    toggle(r, fonts, "ALL", n > 0 && locked == n, SetupButton::AllRules);
                    if page.list == Lists::Rules {
                        toggle(r, fonts, "TARGET LINE", l.target, SetupButton::Target);
                    }
                    toggle(r, fonts, "LOCK ORDER", l.order, SetupButton::Order);
                });
            }
            Section::TrialsHead => trials_head(p, fonts, &c, page),
            Section::Trials => trials(p, fonts, &c, content, page),
            Section::Goals => goals(p, fonts, &c, content, page),
            Section::Estimate => estimate(p, fonts, &c, content, page, &lab),
        });
    }
}

/// A pill that is on or off.
fn toggle(p: &mut ChildSpawnerCommands, fonts: &HudFonts, words: &str, on: bool, b: SetupButton) {
    spawn_pill(p, fonts, words, 8.5, 26.0, None, Pill { active: on, ..Pill::new(Tone::Teal) }, LabButton::Setup(b));
}

/// A − / value / + stepper.
fn stepper(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, value: &str, less: SetupButton, more: SetupButton) {
    p.spawn(row(6.0)).with_children(|r| {
        r.spawn((ring_node(24.0), LabButton::Setup(less))).with_child(ring_glyph(Glyph::Minus, 10.0));
        r.spawn((number(fonts, value, 14.0, c(Tone::Number, 1.0)), Node { min_width: px(34), justify_content: JustifyContent::Center, ..default() }, TextLayout::justify(bevy::text::Justify::Center)));
        r.spawn((ring_node(24.0), LabButton::Setup(more))).with_child(ring_glyph(Glyph::Plus, 10.0));
    });
}

/// A small − / value / + stepper, for the trials' rows.
fn small_stepper(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, value: &str, less: SetupButton, more: SetupButton) {
    p.spawn(row(4.0)).with_children(|r| {
        r.spawn((ring_node(20.0), LabButton::Setup(less))).with_child(ring_glyph(Glyph::Minus, 8.0));
        r.spawn((number(fonts, value, 12.5, c(Tone::Number, 1.0)), Node { min_width: px(26), justify_content: JustifyContent::Center, ..default() }, TextLayout::justify(bevy::text::Justify::Center)));
        r.spawn((ring_node(20.0), LabButton::Setup(more))).with_child(ring_glyph(Glyph::Plus, 8.0));
    });
}

/// A row of a list one picks from: its name and the line under it, lit while picked.
fn pick_row(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, name: &str, sub: &str, picked: bool, b: SetupButton) {
    p.spawn((
        LabButton::Setup(b),
        Pill { bare: true, active: picked, ..Pill::new(Tone::Teal) },
        Hot::default(),
        Node {
            height: px(LIST_ROW),
            flex_shrink: 0.0,
            padding: UiRect::horizontal(px(10)),
            border: UiRect::all(px(1.2)),
            border_radius: BorderRadius::all(px(10)),
            justify_content: JustifyContent::SpaceBetween,
            ..row(8.0)
        },
        BorderColor::all(Color::NONE),
        BackgroundColor(Color::NONE),
    ))
    .with_children(|r| {
        r.spawn((prose(fonts, name, 12.5, c(Tone::Number, 1.0)), TextLayout::no_wrap()));
        r.spawn((tracked(fonts, sub, 7.0, c(sub_tone(picked), 1.0), 0.15), TextLayout::no_wrap()));
    });
}

fn chips(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, page: &SetupPage) {
    p.spawn(row(8.0)).with_children(|r| {
        if page.setup.starts.is_empty() {
            r.spawn(label(fonts, "NO START YET: PICK ONE BELOW", 9.0, c(Tone::Coral, 1.0)));
        }
        for (k, res) in page.resolved.iter().enumerate() {
            let (words, tone) = match res {
                Ok(s) => (format!("{}  {}", k + 1, s.name.to_uppercase()), Tone::Teal),
                Err(e) => (format!("{}  {}", k + 1, e.to_uppercase()), Tone::Coral),
            };
            spawn_pill(r, fonts, &words, 8.5, 26.0, None, Pill { active: k == page.selected, ..Pill::new(tone) }, LabButton::Setup(SetupButton::Start(k)));
        }
    });
}

fn limits(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, page: &SetupPage, granted: &world::career::Granted, free: u32) {
    let s = &page.setup;
    let parts = if granted.free { "LIMITS  ·  EVERY PART  ·  ?UNLOCK=ALL" } else { "LIMITS  ·  ONLY THE PARTS AND MATERIALS YOUR CAREER HAS" };
    p.spawn(label(fonts, parts, 9.0, c(Tone::Dim, 1.0)));
    let line = |p: &mut ChildSpawnerCommands, head: &str, body: &mut dyn FnMut(&mut ChildSpawnerCommands)| {
        p.spawn(Node { height: px(26), ..row(8.0) }).with_children(|r| {
            r.spawn((label(fonts, head, 8.5, c(Tone::Label, 1.0)), Node { width: px(120), flex_shrink: 0.0, ..default() }));
            body(r);
        });
    };
    let bp = page.start_bp.map_or_else(|| "—".into(), |v| format!("{v:.0}"));
    line(p, "BUILD POINTS", &mut |r| {
        toggle(r, fonts, &format!("AT MOST THE START'S ({bp})"), s.build_points == Bound::Start, SetupButton::BpStart);
        toggle(r, fonts, "NO CAP", s.build_points == Bound::Off, SetupButton::BpOff);
        let at = match s.build_points {
            Bound::At(v) => format!("{v:.0}"),
            _ => "—".into(),
        };
        stepper(r, fonts, c, &at, SetupButton::BpStep(-1), SetupButton::BpStep(1));
    });
    let hold = page.start_hold.map_or_else(|| "—".into(), |h| h.to_string());
    line(p, "HOLD", &mut |r| {
        toggle(r, fonts, &format!("AT LEAST THE START'S ({hold})"), s.min_hold == Bound::Start, SetupButton::Hold);
    });
    line(p, "VOYAGE-READY", &mut |r| {
        toggle(r, fonts, if s.voyage_ready { "ON" } else { "OFF" }, s.voyage_ready, SetupButton::VoyageReady);
        r.spawn(label(fonts, &format!("LOGIC CORES ≤ {free}: THE MIND'S {} LESS FLEET'S ESCORTS", granted.mind_capacity), 8.0, c(Tone::Dim, 1.0)));
    });
    line(p, "MIRROR", &mut |r| {
        toggle(r, fonts, if s.mirror { "ON" } else { "OFF" }, s.mirror, SetupButton::Mirror);
        r.spawn(label(fonts, "EVERY EDIT MIRRORED  ·  THE STARTS MUST BE SYMMETRIC", 8.0, c(Tone::Dim, 1.0)));
    });
}

/// MAY CHANGE: `lab.ron`'s gene presets (one lit while the genes are its), the genes of the design and of the behavior
/// and assists, the part categories whose parts may change (while PARTS is on), FLOWN BY and its pilot.
fn genes(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content, page: &SetupPage) {
    let s = &page.setup;
    let m = &s.may_change;
    p.spawn(row(8.0)).with_children(|r| {
        for (k, preset) in content.lab.gene_presets.iter().enumerate() {
            let on = MayChange { fixed: m.fixed.clone(), ..MayChange::only(&preset.genes) } == *m;
            toggle(r, fonts, &preset.name.to_uppercase(), on, SetupButton::GenePreset(k));
        }
    });
    let gene = |r: &mut ChildSpawnerCommands, g: Gene| {
        let (name, tip) = content.lab.gene(g.id()).map_or_else(|| (g.id().to_uppercase(), String::new()), |l| (l.name.to_uppercase(), l.summary.to_uppercase()));
        spawn_pill(r, fonts, &name, 8.0, 24.0, None, Pill { active: m.has(g), ..Pill::new(Tone::Teal) }, (LabButton::Setup(SetupButton::Gene(g)), kit::ui_kit::Tip::new(tip)));
    };
    let line = |p: &mut ChildSpawnerCommands, head: &str, body: &mut dyn FnMut(&mut ChildSpawnerCommands)| {
        p.spawn(Node { height: px(24), ..row(6.0) }).with_children(|r| {
            r.spawn((label(fonts, head, 8.0, c(Tone::Label, 1.0)), Node { width: px(76), flex_shrink: 0.0, ..default() }));
            body(r);
        });
    };
    line(p, "DESIGN", &mut |r| {
        for g in [Gene::Layout, Gene::Parts, Gene::Armour, Gene::Power] {
            gene(r, g);
        }
    });
    line(p, "MIND", &mut |r| {
        for g in [Gene::Numbers, Gene::Rules, Gene::AssistNumbers, Gene::AssistRules] {
            gene(r, g);
        }
    });
    line(p, "PARTS OF", &mut |r| {
        for cat in content.categories.iter() {
            let pill = Pill { active: m.parts && !m.fixed.contains(&cat.id), disabled: !m.parts, ..Pill::new(Tone::Teal) };
            spawn_pill(r, fonts, &cat.name.to_uppercase(), 7.0, 20.0, None, pill, LabButton::Setup(SetupButton::Kept(cat.id.clone())));
        }
    });
    line(p, "FLOWN BY", &mut |r| {
        for (f, words) in [(Flier::Behavior, "BEHAVIOR"), (Flier::StandIn, "STAND-IN"), (Flier::Both, "BOTH")] {
            toggle(r, fonts, words, s.flown_by == f, SetupButton::Flier(f));
        }
        r.spawn((label(fonts, "PILOT", 8.0, c(Tone::Label, 1.0)), Node { margin: UiRect::left(px(8)), ..default() }));
        for pilot in &content.lab.pilots {
            let pill = Pill { active: s.pilot == pilot.id, disabled: s.flown_by == Flier::Behavior, ..Pill::new(Tone::Teal) };
            spawn_pill(r, fonts, &pilot.name.to_uppercase(), 7.5, 22.0, None, pill, (LabButton::Setup(SetupButton::Pilot(pilot.id.clone())), kit::ui_kit::Tip::new(pilot.summary.to_uppercase())));
        }
    });
}

fn size(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content, page: &SetupPage) {
    let s = &page.setup;
    p.spawn(row(8.0)).with_children(|r| {
        for (k, z) in content.lab.sizes.iter().enumerate() {
            toggle(r, fonts, &z.name.to_uppercase(), s.size == Size::of(z), SetupButton::Size(k));
        }
        r.spawn(Node { width: px(14), ..default() });
        r.spawn(label(fonts, "SEED", 8.5, c(Tone::Label, 1.0)));
        stepper(r, fonts, c, &s.seed.to_string(), SetupButton::Seed(-1), SetupButton::Seed(1));
        spawn_pill(r, fonts, "DICE", 8.5, 26.0, None, Pill::new(Tone::Teal), LabButton::Setup(SetupButton::Dice));
    });
    p.spawn(row(8.0)).with_children(|r| {
        for (head, value, less, more) in [
            ("CHILDREN", s.size.population, SetupButton::Population(-1), SetupButton::Population(1)),
            ("GENERATIONS", s.size.generations, SetupButton::Generations(-1), SetupButton::Generations(1)),
        ] {
            r.spawn(label(fonts, head, 8.5, c(Tone::Label, 1.0)));
            stepper(r, fonts, c, &value.to_string(), less, more);
            r.spawn(Node { width: px(8), ..default() });
        }
    });
}

fn locks_head(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content, page: &SetupPage) {
    let name = page.resolved.get(page.selected).and_then(|r| r.as_ref().ok()).map_or_else(|| "—".into(), |s| s.name.to_uppercase());
    p.spawn(label(fonts, &format!("LOCKS  ·  {name}  ·  WHAT THE RUN KEEPS OF IT"), 9.0, c(Tone::Dim, 1.0)));
    let l = page.setup.starts.get(page.selected).map(|(_, l)| l.clone()).unwrap_or_default();
    p.spawn(row(5.0)).with_children(|r| {
        r.spawn((label(fonts, "KEEP ALL", 8.0, c(Tone::Label, 1.0)), Node { margin: UiRect::right(px(4)), ..default() }));
        for cat in content.categories.iter() {
            spawn_pill(r, fonts, &cat.name.to_uppercase(), 7.5, 22.0, None, Pill { active: l.categories.contains(&cat.id), ..Pill::new(Tone::Gold) }, LabButton::Setup(SetupButton::Category(cat.id.clone())));
        }
    });
}

fn parts(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content, page: &SetupPage) {
    let Some(Ok(start)) = page.resolved.get(page.selected) else { return };
    let l = page.setup.starts.get(page.selected).map(|(_, l)| l.clone()).unwrap_or_default();
    for (k, part) in start.blueprint.parts.iter().enumerate() {
        let def = content.parts.get(&part.part);
        let by_category = def.is_some_and(|d| l.categories.contains(&d.category));
        let locked = by_category || l.parts.contains(&(k as u32));
        let name = def.map_or(part.part.as_str(), |d| d.name.as_str()).to_uppercase();
        let tip = if by_category { format!("{name}  ·  KEPT WITH ITS CATEGORY") } else { format!("{name}  ·  AT ({}, {})", part.x, part.y) };
        let b = LabButton::Setup(SetupButton::Part(k as u32));
        p.spawn((
            b,
            kit::ui_kit::Tip::new(tip),
            Pill { bare: true, active: locked, disabled: by_category, ..Pill::new(Tone::Gold) },
            Hot::default(),
            Node { height: px(22), flex_shrink: 0.0, padding: UiRect::horizontal(px(8)), border: UiRect::all(px(1.0)), border_radius: BorderRadius::all(px(8)), overflow: Overflow::clip(), ..row(8.0) },
            BorderColor::all(Color::NONE),
            BackgroundColor(Color::NONE),
        ))
        .with_children(|r| {
            r.spawn((tracked(fonts, &name, 7.5, c(if locked { Tone::Gold } else { Tone::Number }, 1.0), 0.12), TextLayout::no_wrap()));
        });
    }
}

/// The locks' grid: the selected start's cells and [`MARGIN`] around them, a part's cells gold while it is locked (by
/// itself or with its category), else light; KEEP EMPTY cells coral; each a button with a tip that names it.
fn grid(p: &mut ChildSpawnerCommands, c: &impl Fn(Tone, f32) -> Color, content: &Content, page: &SetupPage) {
    let Some(Ok(start)) = page.resolved.get(page.selected) else { return };
    let l = page.setup.starts.get(page.selected).map(|(_, l)| l.clone()).unwrap_or_default();
    let mut owner = std::collections::BTreeMap::new();
    for (k, part) in start.blueprint.parts.iter().enumerate() {
        for cell in content.placed_shape(part).map(|s| s.cells()).unwrap_or_default() {
            owner.insert(cell, k);
        }
    }
    let (Some(x0), Some(x1), Some(y0), Some(y1)) = (owner.keys().map(|c| c.0).min(), owner.keys().map(|c| c.0).max(), owner.keys().map(|c| c.1).min(), owner.keys().map(|c| c.1).max()) else {
        return;
    };
    let (x0, x1, y0, y1) = (x0 - MARGIN, x1 + MARGIN, y0 - MARGIN, y1 + MARGIN);
    let side = (GRID / (x1 - x0 + 1).max(y1 - y0 + 1) as f32).floor().min(CELL_MAX);
    p.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|col| {
        for y in y0..=y1 {
            col.spawn(Node { flex_direction: FlexDirection::Row, ..default() }).with_children(|r| {
                for x in x0..=x1 {
                    let (tone, alpha, tip) = match owner.get(&(x, y)) {
                        Some(&k) => {
                            let part = &start.blueprint.parts[k];
                            let def = content.parts.get(&part.part);
                            let name = def.map_or(part.part.as_str(), |d| d.name.as_str()).to_uppercase();
                            let kept = def.is_some_and(|d| l.categories.contains(&d.category));
                            if kept || l.parts.contains(&(k as u32)) {
                                (Tone::Gold, 0.85, format!("{name}  ·  LOCKED{}", if kept { " WITH ITS CATEGORY" } else { "" }))
                            } else {
                                (Tone::Label, 0.45, format!("{name}  ·  A CLICK LOCKS IT"))
                            }
                        }
                        None if l.cells.contains(&(x, y)) => (Tone::Coral, 0.6, "KEPT EMPTY  ·  A CLICK FREES IT".to_string()),
                        None => (Tone::Dim, 0.12, "EMPTY  ·  A CLICK KEEPS IT EMPTY".to_string()),
                    };
                    r.spawn((
                        LabButton::Setup(SetupButton::Cell(x, y)),
                        kit::ui_kit::Tip::new(tip),
                        Hot::default(),
                        Node { width: px(side), height: px(side), border: UiRect::all(px(0.5)), ..default() },
                        BackgroundColor(c(tone, alpha)),
                        BorderColor::all(c(Tone::Dim, 0.35)),
                    ));
                }
            });
        }
    });
}

fn rules(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, page: &SetupPage) {
    let Some(Ok(start)) = page.resolved.get(page.selected) else { return };
    let l = page.setup.starts.get(page.selected).map(|(_, l)| l.clone()).unwrap_or_default();
    let Ok(program) = Program::parse(&start.program) else {
        p.spawn(label(fonts, "ITS BEHAVIOR DOESN'T READ", 8.5, c(Tone::Coral, 1.0)));
        return;
    };
    if let Some(t) = &program.target {
        p.spawn((prose(fonts, &t.to_string(), 10.5, c(if l.target { Tone::Gold } else { Tone::Label }, 1.0)), TextLayout::no_wrap(), Node { margin: UiRect::left(px(10)), flex_shrink: 0.0, ..default() }));
    }
    for (k, rule) in program.rules.iter().enumerate() {
        let locked = l.rules.contains(&(k as u32));
        p.spawn((
            LabButton::Setup(SetupButton::Rule(k as u32)),
            Pill { bare: true, active: locked, ..Pill::new(Tone::Gold) },
            Hot::default(),
            Node { height: px(24), flex_shrink: 0.0, padding: UiRect::horizontal(px(10)), border: UiRect::all(px(1.0)), border_radius: BorderRadius::all(px(8)), overflow: Overflow::clip(), ..row(8.0) },
            BorderColor::all(Color::NONE),
            BackgroundColor(Color::NONE),
        ))
        .with_children(|r| {
            r.spawn((prose(fonts, &rule.to_string(), 10.5, c(if locked { Tone::Gold } else { Tone::Number }, 1.0)), TextLayout::no_wrap()));
        });
    }
}

/// The selected start's pilot assists, a lock each (none: a line that says so; the stand-in flies it then without).
fn assists(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, page: &SetupPage) {
    let Some(Ok(start)) = page.resolved.get(page.selected) else { return };
    let l = page.setup.starts.get(page.selected).map(|(_, l)| l.clone()).unwrap_or_default();
    let rules = start.blueprint.assists.as_ref().map_or(&[][..], |a| &a.rules[..]);
    if rules.is_empty() {
        p.spawn(prose(fonts, "No pilot assists yet: the run may grow them.", 11.0, c(Tone::Dim, 1.0)));
    }
    for (k, rule) in rules.iter().enumerate() {
        let locked = l.assists.contains(&(k as u32));
        p.spawn((
            LabButton::Setup(SetupButton::Assist(k as u32)),
            Pill { bare: true, active: locked, ..Pill::new(Tone::Gold) },
            Hot::default(),
            Node { height: px(24), flex_shrink: 0.0, padding: UiRect::horizontal(px(10)), border: UiRect::all(px(1.0)), border_radius: BorderRadius::all(px(8)), overflow: Overflow::clip(), ..row(8.0) },
            BorderColor::all(Color::NONE),
            BackgroundColor(Color::NONE),
        ))
        .with_children(|r| {
            r.spawn((prose(fonts, rule, 10.5, c(if locked { Tone::Gold } else { Tone::Number }, 1.0)), TextLayout::no_wrap()));
        });
    }
}

/// A weight as the steppers show it: `2`, `1.5`.
fn weight(w: f32) -> String {
    if w.fract() == 0.0 { format!("{w:.0}") } else { format!("{w:.1}") }
}

fn trials_head(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, page: &SetupPage) {
    p.spawn(label(fonts, "TRIALS  ·  WHAT A DESIGN IS SCORED AGAINST", 9.0, c(Tone::Dim, 1.0)));
    let words = if page.sources.escorts.is_empty() { "WITH YOUR ESCORTS  ·  NONE IN FLEET".to_string() } else { format!("WITH YOUR {} ESCORTS", page.sources.escorts.len()) };
    let on = page.setup.escorts && !page.sources.escorts.is_empty();
    spawn_pill(p, fonts, &words, 8.0, 24.0, None, Pill { active: on, disabled: page.sources.escorts.is_empty(), ..Pill::new(Tone::Teal) }, LabButton::Setup(SetupButton::Escorts));
}

/// Why a trial has nothing to fight now, if it hasn't.
fn nothing_to_fight(kind: &data::TrialPresetKind, src: &Sources) -> Option<&'static str> {
    use data::TrialPresetKind as K;
    match kind {
        K::Encounters | K::Voyages if src.open_voyages.is_empty() => Some("NO VOYAGE OPEN"),
        K::Fleet if src.fleet.is_empty() => Some("NO SAVED DESIGN"),
        K::Champions { .. } if src.champions.is_empty() => Some("NO EARLIER RUN YET"),
        _ => None,
    }
}

/// The trials: a row each (on or off, weight, seeds, its count, how many fights a child gets of it), ENEMY SHIPS'
/// ships under it while it is on.
fn trials(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content, page: &SetupPage) {
    let s = &page.setup;
    let cell = |w: f32| Node { width: px(w), flex_shrink: 0.0, ..row(4.0) };
    p.spawn(Node { height: px(14), flex_shrink: 0.0, ..row(0.0) }).with_children(|h| {
        // Each word centred over its column's stepper (74 px: two rings and the value).
        for (word, w) in ["", "WEIGHT", "SEEDS", ""].into_iter().zip(TRIAL_COLUMNS) {
            h.spawn(cell(w)).with_children(|n| {
                n.spawn((tracked(fonts, word, 6.5, c(Tone::Dim, 1.0), 0.15), TextLayout::justify(bevy::text::Justify::Center), Node { width: px(74), ..default() }));
            });
        }
    });
    for (k, t) in s.trials.iter().enumerate() {
        let Some(preset) = content.lab.trial(&t.preset) else { continue };
        let blocked = nothing_to_fight(&preset.kind, &page.sources);
        let (jobs, _) = t.cost(content, s.starts.len(), &page.sources);
        p.spawn(Node { height: px(TRIAL_ROW), flex_shrink: 0.0, ..row(0.0) }).with_children(|r| {
            r.spawn(cell(TRIAL_COLUMNS[0])).with_children(|n| {
                let pill = Pill { active: t.on && blocked.is_none(), disabled: blocked.is_some(), ..Pill::new(Tone::Teal) };
                spawn_pill(n, fonts, &preset.name.to_uppercase(), 7.5, 22.0, None, pill, LabButton::Setup(SetupButton::Trial(k)));
            });
            r.spawn(cell(TRIAL_COLUMNS[1])).with_children(|n| {
                small_stepper(n, fonts, c, &weight(t.weight), SetupButton::Weight(k, -1), SetupButton::Weight(k, 1));
            });
            r.spawn(cell(TRIAL_COLUMNS[2])).with_children(|n| {
                small_stepper(n, fonts, c, &t.seeds.to_string(), SetupButton::TrialSeeds(k, -1), SetupButton::TrialSeeds(k, 1));
            });
            r.spawn(cell(TRIAL_COLUMNS[3])).with_children(|n| {
                if let Some(count) = t.count {
                    let what = match preset.kind {
                        data::TrialPresetKind::Waves { .. } => "WAVES",
                        _ => "BEST",
                    };
                    n.spawn((tracked(fonts, what, 6.5, c(Tone::Dim, 1.0), 0.15), Node { width: px(36), flex_shrink: 0.0, ..default() }));
                    small_stepper(n, fonts, c, &count.to_string(), SetupButton::Count(k, -1), SetupButton::Count(k, 1));
                }
            });
            let (note, tone) = match (blocked, t.on, t.weight > 0.0) {
                (Some(why), _, _) => (why.to_string(), Tone::Coral),
                (None, false, _) => ("OFF".to_string(), Tone::Dim),
                (None, true, false) => ("WEIGHS 0: NOT FOUGHT".to_string(), Tone::Dim),
                (None, true, true) => (format!("{jobs} {}", if jobs == 1 { "FIGHT" } else { "FIGHTS" }), Tone::Label),
            };
            // The note at the row's right end, clipped short of the count before it.
            r.spawn(Node { flex_grow: 1.0, min_width: px(0), overflow: Overflow::clip(), justify_content: JustifyContent::FlexEnd, ..default() }).with_children(|n| {
                n.spawn((tracked(fonts, &note, 7.0, c(tone, 1.0), 0.15), TextLayout::no_wrap()));
            });
        });
        let race = matches!(preset.kind, data::TrialPresetKind::CoEvolution { .. });
        if t.on && race {
            enemy(p, fonts, c, content, page, k, t);
        } else if t.on && !t.ships.is_empty() {
            for (j, (id, n)) in t.ships.iter().enumerate() {
                let name = content.ships.get(id).map_or(id.as_str(), |b| b.name.as_str()).to_uppercase();
                p.spawn(Node { height: px(TRIAL_ROW), flex_shrink: 0.0, padding: UiRect::left(px(26)), ..row(0.0) }).with_children(|r| {
                    r.spawn((tracked(fonts, &name, 7.0, c(if *n > 0 { Tone::Number } else { Tone::Dim }, 1.0), 0.15), TextLayout::no_wrap(), Node { overflow: Overflow::clip(), ..cell(TRIAL_COLUMNS[0] - 26.0) }));
                    r.spawn(cell(TRIAL_COLUMNS[1])).with_children(|c2| {
                        small_stepper(c2, fonts, c, &n.to_string(), SetupButton::Ship(k, j, -1), SetupButton::Ship(k, j, 1));
                    });
                    let note = if *n == 0 { "NOT FOUGHT".to_string() } else { format!("{n} IN A FIGHT") };
                    r.spawn((tracked(fonts, &note, 7.0, c(Tone::Dim, 1.0), 0.15), TextLayout::no_wrap()));
                });
            }
        }
    }
}

/// THE ENEMY of the ARMS RACE (GDR-0092 "Co-evolution"): a pill per enemy ship, lit when the enemy starts from it (its
/// faction's; a boss only when picked), and its size and pairing in words.
fn enemy(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content, page: &SetupPage, k: usize, t: &world::evolution::TrialPick) {
    p.spawn(Node { height: px(TRIAL_ROW), flex_shrink: 0.0, padding: UiRect::left(px(26)), ..row(6.0) }).with_children(|r| {
        r.spawn((tracked(fonts, "THE ENEMY FROM", 6.5, c(Tone::Dim, 1.0), 0.15), TextLayout::no_wrap()));
        for (j, (id, n)) in t.ships.iter().enumerate() {
            let name = content.ships.get(id).map_or(id.as_str(), |b| b.name.as_str()).to_uppercase();
            let by = if *n > 0 { -1 } else { 1 };
            spawn_pill(r, fonts, &name, 7.0, 20.0, None, Pill { active: *n > 0, ..Pill::new(Tone::Coral) }, LabButton::Setup(SetupButton::Ship(k, j, by)));
        }
    });
    let s = &page.setup;
    let words = match (t.picked().len(), s.enemy_cost(content)) {
        (0, _) => ("PICK A SHIP TO START THE ENEMY FROM".to_string(), Tone::Coral),
        (_, Some((children, jobs, _))) => (format!("{children} ENEMY CHILDREN A GENERATION  ·  {jobs} FIGHTS EACH AGAINST YOUR BEST, YOUR PAST CHAMPIONS AND YOUR STARTS"), Tone::Label),
        (_, None) => ("WEIGHS 0: NO ENEMY EVOLVES".to_string(), Tone::Dim),
    };
    p.spawn(Node { height: px(18), flex_shrink: 0.0, padding: UiRect::left(px(26)), overflow: Overflow::clip(), ..row(0.0) }).with_children(|r| {
        r.spawn((tracked(fonts, &words.0, 6.5, c(words.1, 1.0), 0.12), TextLayout::no_wrap()));
    });
}

/// GOALS: the presets (one lit while the weights are its), then each goal's weight.
fn goals(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content, page: &SetupPage) {
    let w = page.setup.goals;
    p.spawn(row(8.0)).with_children(|r| {
        for (k, preset) in content.lab.goal_presets.iter().enumerate() {
            let on = Goals::of(preset.weights.clone()) == w;
            spawn_pill(r, fonts, &preset.name.to_uppercase(), 7.5, 22.0, None, Pill { active: on, ..Pill::new(Tone::Teal) }, LabButton::Setup(SetupButton::GoalPreset(k)));
        }
    });
    p.spawn(row(6.0)).with_children(|r| {
        for (k, (id, v)) in data::GOALS.iter().zip(w.weights()).enumerate() {
            let name = content.lab.goal(id).map_or_else(|| id.to_uppercase(), |g| g.name.to_uppercase());
            r.spawn((tracked(fonts, &name, 7.0, c(if v > 0.0 { Tone::Label } else { Tone::Dim }, 1.0), 0.15), Node { margin: UiRect::left(px(if k == 0 { 0.0 } else { 8.0 })), ..default() }));
            stepper(r, fonts, c, &weight(v), SetupButton::Goal(k, -1), SetupButton::Goal(k, 1));
        }
    });
}

fn estimate(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content, page: &SetupPage, lab: &world::evolution::Lab) {
    let s = &page.setup;
    let (jobs, _) = s.per_child(content, &page.sources);
    // An arms race's enemy fights beside your children.
    let theirs = s.enemy_cost(content).map_or(0, |(n, jobs, _)| n * jobs);
    let fights = format!("{jobs} FIGHTS A CHILD, {} A GENERATION", jobs * s.size.population + theirs);
    // With the layer open the run has its larger budget: ticks per second from the measured cost per tick at 60
    // frames a second (an estimate until the first run measured this device).
    let per_second = world::evolution::budget(content, world::mode::AppMode::Home, true) * 60.0 * 1000.0 / lab.rate.us_per_tick.max(0.01);
    let minutes = (s.ticks(content, &page.sources) / per_second as f64 / 60.0).max(0.1);
    let time = match minutes {
        m if m < 1.0 => "UNDER A MINUTE".to_string(),
        m if m < 90.0 => format!("ABOUT {m:.0} MIN"),
        m => format!("ABOUT {:.1} H", m / 60.0),
    };
    let measured = if lab.rate.measured { "MEASURED" } else { "ESTIMATED" };
    let line = format!("THE RUN  ·  {fights}  ·  {time}, THE LAYER OPEN ({measured})");
    p.spawn((label(fonts, &line, 7.5, c(if jobs == 0 { Tone::Coral } else { Tone::Label }, 1.0)), TextLayout::no_wrap()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use world::storage::MemoryBackend;

    /// EVOLVE THIS DESIGN: the page opens with the working design as its one start, named after it, its limits from
    /// it; adding and removing starts keeps at most four.
    #[test]
    fn the_working_design_prefills_the_start() {
        let content = data::base_content();
        let storage = Storage::new(MemoryBackend::default());
        let design = PlayerDesign::starter(&content, "rim_skiff");
        let career = data::Career::new(&content, content.starts.iter().next().unwrap().id.as_str(), 1).unwrap();
        let page = SetupPage::new(&content, &storage, &PlayerBehaviors::default(), &design, &career, &Lab::default(), vec![StartPick::Working]);
        assert_eq!(page.setup.starts, vec![(StartPick::Working, Locks::default())]);
        let start = page.resolved[0].as_ref().unwrap();
        assert_eq!(start.name, design.blueprint().name);
        assert_eq!(page.setup.name, format!("Evolving {}", design.blueprint().name));
        assert!(page.start_bp.is_some_and(|b| b > 0.0) && page.start_hold.is_some());
        assert!(page.setup.mirror, "the starter is symmetric");
        // Everything may change, flown by both the behavior and lab.ron's first stand-in pilot.
        assert_eq!((page.setup.may_change.clone(), page.setup.flown_by, page.setup.pilot.as_str()), (MayChange::default(), Flier::Both, "casual"));
        assert!(page.templates.contains(&"rim_skiff".to_string()) && !page.templates.contains(&"automaton_raider".to_string()));
        let mut page = page;
        for t in ["rim_skiff", "halvex_runner", "sample_early"] {
            page.toggle(StartPick::Template(t.into())).unwrap();
        }
        assert!(page.toggle(StartPick::Template("sample_mid".into())).is_err(), "at most four");
        page.toggle(StartPick::Template("rim_skiff".into())).unwrap();
        assert_eq!(page.setup.starts.len(), 3);
        // The trials are lab.ron's, its default selection on; the first voyage is open, no design saved, no run made.
        let on: Vec<&str> = page.setup.trials.iter().filter(|t| t.on).map(|t| t.preset.as_str()).collect();
        assert_eq!(on, ["enemies_you_meet", "test_range_waves", "beat_your_start", "hall_of_fame"]);
        assert_eq!(page.sources.open_voyages, ["yard_approach"]);
        let kind = |id: &str| &content.lab.trial(id).unwrap().kind;
        assert_eq!(nothing_to_fight(kind("enemies_you_meet"), &page.sources), None);
        assert_eq!(nothing_to_fight(kind("your_fleet"), &page.sources), Some("NO SAVED DESIGN"));
        assert_eq!(nothing_to_fight(kind("champions"), &page.sources), Some("NO EARLIER RUN YET"));
        assert_eq!((weight(2.0), weight(1.5)), ("2".to_string(), "1.5".to_string()));
    }
}
