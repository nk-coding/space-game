//! THE RUN (GDR-0092 "The run's page", what steps 4 and 5 have of it): how far the run is, its scores by generation,
//! its archive, and the picked design.
//!
//! ```text
//! RUNNING · SEED 4711   GENERATION 12 / 50 [=====-----]   ~ 140 FIGHTS/S · ABOUT 4 MIN LEFT   START 0.42 » BEST 0.81
//! SCORES BY GENERATION          THE ARCHIVE                         THE PICKED DESIGN
//!  archive best (gold),          a ship card per occupied cell,      its drawing, score, cell, build points
//!  generation best (teal),       tinted by score; the best a gold    [FIGHTS] [LINEAGE] [COMPARE]
//!  mean (grey); bars: children   edge, a start a teal corner         [SCORES] [FIGHTS] [LINEAGE] [COMPARE]
//!  that took a cell                                                  per trial and goal; VALIDATE; the fights
//!                                                                    [SAVE AS DESIGN]
//! ```
//!
//! * **Progress** is read every few frames while the page shows (the run computes in the background, `world::evolution`):
//!   the generation inside, fights per second and the time left from the measured rate.
//! * **The archive**: a grid over the run's first two measures; a click picks a design. The best design is picked
//!   when nothing is.
//! * **SCORES** (GDR-0092 step 5): the picked design's score taken apart, per trial (its weight, score and goals'
//!   means) and in all; VALIDATE THIS and VALIDATE THE BEST 8 (new seeds and the recorded fights again, computed in
//!   the run's place) and the validated score once there is one.
//! * **FIGHTS**: every fight of the picked design in its generation, each in the same columns: how it ended and when,
//!   its score, then the trial and the opponent, and under them the seed, the build points each side lost and the
//!   damage dealt; WATCH BEST and WATCH WORST; WATCH opens the fight viewer (`world::replay`) with the fight's code,
//!   and M (or the flight menu's way out) brings the lab back here.
//! * **RE-SCORE** (a stale run) makes a new run under today's rules; **RENAME** names the run anew.
//! * **LINEAGE**: its line from its start, each ancestor a small card (a click picks it); **COMPARE**: the start and
//!   it side by side, the parts it gained and lost, its behavior.
//! * **SAVE AS DESIGN**: a name (`<start> · evolved g<generation>`), then into YOUR DESIGNS with its origin and the
//!   EVOLVED tag (`world::evolution::save_as_design`).

use std::collections::BTreeMap;

use world::arena::evolve::{ChildRecord, Elite, GenerationStats, Job, JobEnd, Measure, RunStatus, TrialSpec, VALIDATE_BEST, VALIDATE_SEEDS};
use world::arena::{DesignRef, Outcome};
use bevy::prelude::*;
use bevy::ui::UiTransform;
use data::Content;
use kit::style::{Style, ui_color};
use kit::ui_icons::UiIcon;
use kit::ui_kit::{HudFonts, Hot, Pill, Scrollbar, TextField, Tone, ValueBar, field_text, label, number, prose, set_color, set_text, spawn_bar, spawn_brackets, spawn_pill, tracked};
use world::evolution::{Lab, Loaded, result_name, save_as_design};
use world::mode::AppMode;
use world::player::library::MAX_NAME;
use world::scene::Launch;
use world::storage::now_unix;
use world::voyage::clock;

use super::{Ctx, EvolutionLayer, LabButton, Page, abs, column, on_row, row, score};

/// The panels' boxes on the stage: the chart, the archive, the picked design.
const CHART: (Vec2, Vec2) = (Vec2::new(36.0, 170.0), Vec2::new(380.0, 210.0));
const BARS_H: f32 = 44.0;
const GRID: (Vec2, Vec2) = (Vec2::new(444.0, 170.0), Vec2::new(392.0, 392.0));
const PICKED: (Vec2, Vec2) = (Vec2::new(864.0, 140.0), Vec2::new(380.0, 540.0));
/// The fight rows' columns: how it ended and when, its score; the score table's: the score, a goal.
const FIGHT_END_W: f32 = 76.0;
const FIGHT_SCORE_W: f32 = 30.0;
const SCORE_W: f32 = 40.0;
const GOAL_W: f32 = 44.0;
/// The chart draws at most this many generations (a longer run is sampled).
const CHART_POINTS: usize = 120;
/// Ancestors the lineage strip shows at most (the start, then the last ones).
const LINEAGE: usize = 7;
/// The progress line is read again this often (s).
const SYNC_EVERY: f32 = 0.25;

/// The picked design's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Tab {
    #[default]
    Scores,
    Fights,
    Lineage,
    Compare,
}

/// THE RUN page's state.
#[derive(Default)]
pub(crate) struct RunPage {
    /// The picked genome (an elite, or one of their ancestors).
    picked: Option<u32>,
    tab: Tab,
    /// SAVE AS DESIGN asks for a name: this one (the field types into it).
    saving: bool,
    name: String,
    /// RENAME asks for the run's new name.
    renaming: bool,
    new_name: String,
    /// What the page was built for: the run, its generations done, the pick, the tab, the save panel, whether a
    /// validation was underway.
    built: Option<(String, u32, Option<u32>, Tab, bool, bool)>,
}

/// A click on the run's page.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RunButton {
    Pause,
    Resume,
    Pick(u32),
    Tab(Tab),
    /// WATCH and COPY FIGHT CODE: generation, child, job.
    Watch(u32, usize, usize),
    Code(u32, usize, usize),
    SaveAs,
    Save,
    Cancel,
    /// VALIDATE the picked design, or the archive's best [`VALIDATE_BEST`].
    Validate,
    ValidateBest,
    Rescore,
    Rename,
    SaveName,
    CancelName,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunText {
    Status,
    Generation,
    Rate,
    Scores,
    Stale,
    Last,
    Axes,
}

#[derive(Component)]
pub(super) struct GenerationBar;

/// The containers built again when the run or the pick change.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum Panel {
    Chart,
    Grid,
    Picked,
}

/// SAVE AS DESIGN's name field and its row.
#[derive(Component)]
pub(super) struct SaveField;

#[derive(Component)]
pub(super) struct SaveRow;

/// RENAME's field and its row (where the stale line is).
#[derive(Component)]
pub(super) struct RenameField;

#[derive(Component)]
pub(super) struct RenameRow;

pub(super) fn spawn(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color) {
    // The progress line.
    p.spawn(on_row(abs(Vec2::new(36.0, 88.0), Some(Vec2::new(1208.0, 34.0))), 18.0)).with_children(|r| {
        r.spawn((RunText::Status, label(fonts, "", 9.5, c(Tone::Teal, 1.0))));
        r.spawn(row(10.0)).with_children(|g| {
            g.spawn((RunText::Generation, number(fonts, "", 15.0, c(Tone::Number, 1.0))));
            g.spawn(Node { width: px(140), ..default() }).with_children(|b| {
                spawn_bar(b, ValueBar { share: 0.0, tone: Tone::Teal }, GenerationBar);
            });
        });
        r.spawn((RunText::Rate, label(fonts, "", 8.5, c(Tone::Label, 1.0))));
        r.spawn((RunText::Scores, number(fonts, "", 15.0, c(Tone::Gold, 1.0))));
    });
    p.spawn((RunText::Stale, label(fonts, "", 9.0, c(Tone::Coral, 1.0)), abs(Vec2::new(36.0, 122.0), None)));
    p.spawn((RenameRow, on_row(Node { display: Display::None, ..abs(Vec2::new(36.0, 116.0), Some(Vec2::new(800.0, 30.0))) }, 8.0))).with_children(|r| {
        r.spawn(label(fonts, "THE RUN'S NAME", 8.5, c(Tone::Label, 1.0)));
        r.spawn((
            RenameField,
            TextField { placeholder: "Name of the run".into(), max_chars: MAX_NAME, ..default() },
            Hot::default(),
            Node { width: px(260), height: px(28), align_items: AlignItems::Center, padding: UiRect::horizontal(px(12)), border: UiRect::all(px(1.2)), border_radius: BorderRadius::MAX, overflow: Overflow::clip(), ..default() },
            BorderColor::all(c(Tone::Label, 0.3)),
        ))
        .with_children(|f| {
            f.spawn(field_text(fonts, 13.0));
        });
        spawn_pill(r, fonts, "RENAME", 8.5, 26.0, None, Pill { primary: true, ..Pill::new(Tone::Teal) }, LabButton::Run(RunButton::SaveName));
        spawn_pill(r, fonts, "CANCEL", 8.5, 26.0, None, Pill::new(Tone::Teal), LabButton::Run(RunButton::CancelName));
    });
    // The chart.
    let (at, size) = CHART;
    p.spawn((label(fonts, "SCORES BY GENERATION", 9.0, c(Tone::Dim, 1.0)), abs(at - Vec2::Y * 26.0, None)));
    p.spawn(Node { ..abs(at - Vec2::Y * 12.0 + Vec2::X * 200.0, None) }).with_children(|l| {
        l.spawn(row(10.0)).with_children(|r| {
            for (words, tone) in [("ARCHIVE BEST", Tone::Gold), ("GENERATION BEST", Tone::Teal), ("MEAN", Tone::Label)] {
                r.spawn((tracked(fonts, words, 6.5, c(tone, 1.0), 0.15), TextLayout::no_wrap()));
            }
        });
    });
    p.spawn((Panel::Chart, abs(at, Some(size + Vec2::Y * (BARS_H + 6.0)))));
    p.spawn((label(fonts, "BARS: CHILDREN THAT TOOK A CELL", 7.0, c(Tone::Dim, 1.0)), abs(at + Vec2::Y * (size.y + BARS_H + 12.0), None)));
    p.spawn((RunText::Last, tracked(fonts, "", 7.5, c(Tone::Label, 1.0), 0.15), abs(at + Vec2::Y * (size.y + BARS_H + 34.0), Some(Vec2::new(size.x, 60.0)))));
    // The archive.
    let (at, size) = GRID;
    p.spawn((label(fonts, "THE ARCHIVE  ·  THE BEST OF EACH KIND", 9.0, c(Tone::Dim, 1.0)), abs(at - Vec2::Y * 26.0, None)));
    p.spawn((Panel::Grid, abs(at, Some(size))));
    p.spawn((RunText::Axes, tracked(fonts, "", 7.0, c(Tone::Dim, 1.0), 0.15), abs(at + Vec2::Y * (size.y + 8.0), Some(Vec2::new(size.x, 40.0)))));
    // The picked design, and SAVE AS DESIGN's name.
    let (at, size) = PICKED;
    p.spawn((Panel::Picked, Node { flex_direction: FlexDirection::Column, row_gap: px(6), ..abs(at, Some(size)) }));
    p.spawn((SaveRow, on_row(Node { display: Display::None, ..abs(at + Vec2::new(0.0, size.y - 36.0), Some(Vec2::new(size.x, 36.0))) }, 8.0))).with_children(|r| {
        r.spawn((
            SaveField,
            TextField { placeholder: "Name of the design".into(), max_chars: MAX_NAME, ..default() },
            Hot::default(),
            Node { width: px(220), height: px(30), align_items: AlignItems::Center, padding: UiRect::horizontal(px(12)), border: UiRect::all(px(1.2)), border_radius: BorderRadius::MAX, overflow: Overflow::clip(), ..default() },
            BorderColor::all(c(Tone::Label, 0.3)),
        ))
        .with_children(|f| {
            f.spawn(field_text(fonts, 13.0));
        });
        spawn_pill(r, fonts, "SAVE", 9.0, 28.0, None, Pill { primary: true, ..Pill::new(Tone::Teal) }, LabButton::Run(RunButton::Save));
        spawn_pill(r, fonts, "CANCEL", 9.0, 28.0, None, Pill::new(Tone::Teal), LabButton::Run(RunButton::Cancel));
    });
}

/// The genome the page shows: the pick, else the archive's best.
fn shown(page: &RunPage, l: &Loaded) -> Option<u32> {
    page.picked.or_else(|| l.run.archive().best().map(|e| e.id))
}

/// Applies a click.
pub(super) fn press(layer: &mut EvolutionLayer, ctx: &mut Ctx, b: RunButton) {
    let Some(id) = ctx.lab.viewed().map(|l| l.id.clone()) else { return };
    match b {
        RunButton::Pause => super::runs::status(layer, ctx, &id, RunStatus::Paused),
        RunButton::Resume => super::runs::status(layer, ctx, &id, RunStatus::Running),
        RunButton::Pick(g) => {
            layer.run.picked = Some(g);
            layer.run.saving = false;
        }
        RunButton::Tab(t) => layer.run.tab = t,
        RunButton::Watch(generation, child, job) => {
            let Some(code) = ctx.lab.viewed().and_then(|l| l.run.fight_code(generation, child, job)) else {
                layer.say("that fight doesn't play", Tone::Coral);
                return;
            };
            ctx.replay.queue(code.encode());
            ctx.scene.launch = Some(Launch::Fight);
            ctx.lab.watching = true;
            ctx.next.set(AppMode::Flight);
        }
        RunButton::Code(generation, child, job) => match ctx.lab.viewed().and_then(|l| l.run.fight_code(generation, child, job)) {
            Some(code) => {
                ctx.clipboard.copy = Some(code.encode());
                layer.say("the fight's code is copied  ·  it plays at ?scene=fight&fight=<code>, and in determinism-check.sh --fight", Tone::Teal);
            }
            None => layer.say("that fight has no code", Tone::Coral),
        },
        RunButton::SaveAs => {
            let Some(l) = ctx.lab.viewed() else { return };
            let Some(g) = shown(&layer.run, l) else { return };
            let start = l.run.genome(g).and_then(|g| l.run.starts().get(g.root as usize).map(|s| s.blueprint.name.clone()));
            let start = start.map_or_else(|| l.name.clone(), |n| n.trim_end_matches(" (evolved)").to_string());
            let generation = l.run.scored(g).map_or(0, |(generation, _)| generation);
            layer.run.name = result_name(&start, generation);
            layer.run.saving = true;
        }
        RunButton::Cancel => layer.run.saving = false,
        RunButton::Validate | RunButton::ValidateBest => {
            let Some(l) = ctx.lab.viewed() else { return };
            let genomes: Vec<u32> = match b {
                RunButton::Validate => shown(&layer.run, l).into_iter().collect(),
                _ => l.run.results().elites.iter().take(VALIDATE_BEST).map(|e| e.id).collect(),
            };
            match ctx.lab.validate(&ctx.storage, &ctx.content.0, &id, &genomes) {
                Ok(jobs) => layer.say(format!("validating {} on {VALIDATE_SEEDS} new seeds and its recorded fights again  ·  {jobs} jobs, in the run's place", if genomes.len() == 1 { "the design".to_string() } else { format!("the best {}", genomes.len()) }), Tone::Teal),
                Err(e) => layer.say(format!("not validated: {e}"), Tone::Coral),
            }
        }
        RunButton::Rescore => match ctx.lab.rescore(&mut ctx.storage, &ctx.content.0, &id, now_unix()) {
            Ok(new) => {
                let name = ctx.lab.entry(&new).map_or_else(|| new.clone(), |e| e.name.clone());
                layer.run = RunPage::default();
                layer.say(format!("{name} re-scores the archive under today's rules, then goes on  ·  the old run stays as it was"), Tone::Teal);
            }
            Err(e) => layer.say(format!("not re-scored: {e}"), Tone::Coral),
        },
        RunButton::Rename => {
            layer.run.new_name = ctx.lab.viewed().map_or_else(String::new, |l| l.name.clone());
            layer.run.renaming = true;
        }
        RunButton::CancelName => layer.run.renaming = false,
        RunButton::SaveName => {
            let name = layer.run.new_name.clone();
            match ctx.lab.rename(&mut ctx.storage, &id, &name) {
                Ok(()) => {
                    layer.run.renaming = false;
                    layer.say(format!("renamed {}", name.trim()), Tone::Teal);
                }
                Err(e) => layer.say(e, Tone::Coral),
            }
        }
        RunButton::Save => {
            layer.run.saving = false;
            let Some(l) = ctx.lab.viewed() else { return };
            let Some(g) = shown(&layer.run, l) else { return };
            let name = layer.run.name.trim().to_string();
            if name.is_empty() {
                layer.say("a design needs a name", Tone::Coral);
                return;
            }
            let content = &ctx.content.0;
            match save_as_design(&mut ctx.storage, content, &mut ctx.behaviors, l, g, &name, now_unix()) {
                Ok((_, problems)) => match problems.iter().find(|p| p.is_error()) {
                    Some(p) => layer.say(format!("saved as {name} in your designs  ·  {}", p.message), Tone::Gold),
                    None => layer.say(format!("saved as {name} in your designs  ·  open it in the drydock's blueprints"), Tone::Teal),
                },
                Err(e) => layer.say(format!("not saved: {e}"), Tone::Coral),
            }
        }
    }
    layer.dirty = true;
}

/// A measure's label and unit: the content's stat where it has one, else words of the lab's own (its `lab.ron`
/// comes with GDR-0092 step 8).
fn measure_words(content: &Content, m: Measure) -> (String, String) {
    let stat = match m {
        Measure::TopSpeed => Some("top_speed"),
        Measure::Acceleration => Some("acceleration"),
        Measure::WeaponReach => Some("range"),
        Measure::BuildPoints => Some("build_points"),
        Measure::Mass => Some("mass"),
        Measure::Firepower => Some("firepower"),
        Measure::HitPoints => Some("hp"),
        Measure::Parts | Measure::Rules | Measure::EngageDistance => None,
    };
    match stat.and_then(|id| content.stats.get(id)) {
        Some(d) => (d.name.to_uppercase(), d.unit.clone()),
        None => match m {
            Measure::Parts => ("PARTS".into(), String::new()),
            Measure::Rules => ("RULES".into(), String::new()),
            _ => ("ENGAGE DISTANCE".into(), "m".into()),
        },
    }
}

/// A trial's name: its `lab.ron` preset's, else its kind's in words.
fn trial_name(content: &Content, t: &TrialSpec) -> String {
    use world::arena::evolve::Trial as T;
    if let Some(p) = t.preset.as_deref().and_then(|id| content.lab.trial(id)) {
        return p.name.to_uppercase();
    }
    match &t.kind {
        T::EnemyShips { .. } => "ENEMY SHIPS",
        T::Encounters { .. } => "THE ENEMIES YOU MEET",
        T::Waves { .. } => "TEST-RANGE WAVES",
        T::Gauntlet { .. } => "A GAUNTLET",
        T::BeatYourStart => "BEAT YOUR START",
        T::Fleet { .. } => "YOUR FLEET",
        T::HallOfFame { .. } => "HALL OF FAME",
        T::Champions { .. } => "CHAMPIONS",
        T::Voyages { .. } => "VOYAGES",
    }
    .into()
}

/// A fight's opponent side in words: `2 × AUTOMATON RAIDER`, a design of the player's or the run's by its name
/// (`RIM SKIFF`, without the `(evolved)` every genome's header has), a gauntlet's waves after it (`+ 4 WAVES`); a
/// voyage by its name.
fn opponent(content: &Content, job: &Job) -> String {
    let spec = match job {
        Job::Fight { spec, .. } => spec,
        Job::Voyage { spec, .. } => return content.voyages.get(&spec.voyage).map_or_else(|| spec.voyage.to_uppercase(), |v| v.name.to_uppercase()),
    };
    let names: Vec<String> = spec.sides[1]
        .entries
        .iter()
        .map(|e| {
            let name = match &e.design {
                DesignRef::Ship(id) => content.ships.get(id).map_or_else(|| id.to_uppercase(), |b| b.name.to_uppercase()),
                DesignRef::Blueprint(text) => data::parse_blueprint(text).map_or_else(|_| "A DESIGN".into(), |b| b.name.trim_end_matches(" (evolved)").to_uppercase()),
            };
            if e.count > 1 { format!("{} × {name}", e.count) } else { name }
        })
        .collect();
    let waves = match spec.waves.len() {
        0 => String::new(),
        1 => "  +  1 WAVE".into(),
        n => format!("  +  {n} WAVES"),
    };
    names.join(" + ") + waves.as_str()
}

/// How a job ended for the design and when, and its tone: `WON 0:23`, `LOST 1:02`, `BOTH OUT 0:40`, `DRAW 1:30`,
/// `VOYAGE 4:10` (its score has a column of its own).
fn ending(end: &JobEnd, ticks: u64) -> (String, Tone) {
    let at = clock(ticks);
    match end {
        JobEnd::Fight { outcome: Outcome::Won(0), .. } => (format!("WON {at}"), Tone::Teal),
        JobEnd::Fight { outcome: Outcome::Won(_), .. } => (format!("LOST {at}"), Tone::Coral),
        JobEnd::Fight { outcome: Outcome::BothOut, .. } => (format!("BOTH OUT {at}"), Tone::Gold),
        JobEnd::Fight { outcome: Outcome::TimeLimit, .. } => (format!("DRAW {at}"), Tone::Label),
        JobEnd::Voyage { .. } => (format!("VOYAGE {at}"), Tone::Label),
        JobEnd::Failed => ("DIDN'T BUILD".into(), Tone::Coral),
    }
}

/// A fight's second line: the seed, a gauntlet's waves cleared of all, what each side lost (its build points: yours and
/// theirs, a gauntlet's waves all counted), the damage you dealt.
fn fight_detail(job: &Job, end: &JobEnd) -> String {
    let (seed, waves) = match job {
        Job::Fight { spec, .. } => (spec.seed, spec.waves.len() as u32 + 1),
        Job::Voyage { spec, .. } => (spec.seed, 0),
    };
    match end {
        JobEnd::Fight { lost, dealt, waves: cleared, .. } => {
            // A gauntlet's waves first: its row's first line clips its opponent's waves.
            let gauntlet = if waves > 1 { format!(" · {cleared} OF {waves} WAVES CLEARED") } else { String::new() };
            format!("SEED {seed}{gauntlet} · YOU LOST {:.0} BP · THEY LOST {:.0} BP · DEALT {:.0}", lost[0], lost[1], dealt[0])
        }
        JobEnd::Voyage { .. } => format!("SEED {seed} · FLOWN HEADLESS FROM ENTRY TO HOME"),
        JobEnd::Failed => format!("SEED {seed}"),
    }
}

/// The parts a design has, by name, and how many of each.
fn part_counts(content: &Content, bp: &data::Blueprint) -> BTreeMap<String, i32> {
    let mut out = BTreeMap::new();
    for p in &bp.parts {
        let name = content.parts.get(&p.part).map_or_else(|| p.part.to_uppercase(), |d| d.name.to_uppercase());
        *out.entry(name).or_insert(0) += 1;
    }
    out
}

/// What changed from `from` to `to`: `+2 PULSE LASER  ·  −1 CARGO BAY`, or that the parts are the same.
pub(super) fn changes(content: &Content, from: &data::Blueprint, to: &data::Blueprint) -> String {
    let (a, b) = (part_counts(content, from), part_counts(content, to));
    let mut out = Vec::new();
    for name in a.keys().chain(b.keys()).collect::<std::collections::BTreeSet<_>>() {
        let d = b.get(name).copied().unwrap_or(0) - a.get(name).copied().unwrap_or(0);
        if d != 0 {
            out.push(format!("{}{} {name}", if d > 0 { "+" } else { "−" }, d.abs()));
        }
    }
    if out.is_empty() {
        let moved = from.parts != to.parts;
        return if moved { "THE SAME PARTS, MOVED OR TURNED".into() } else { "THE SAME PARTS".into() };
    }
    out.join("  ·  ")
}

/// Builds the chart, the archive and the picked design again when the run closed a generation, the pick or the tab
/// changed, or a click asked for it.
#[allow(clippy::too_many_arguments)]
pub(super) fn fill(
    mut commands: Commands,
    mut layer: ResMut<EvolutionLayer>,
    lab: Res<Lab>,
    content: Res<world::sim_bridge::GameContent>,
    fonts: Res<HudFonts>,
    style: Res<Style>,
    panels: Query<(Entity, &Panel)>,
) {
    if !layer.open || layer.page != Page::Run {
        return;
    }
    let Some(l) = lab.viewed() else { return };
    let key = (l.id.clone(), l.run.generation(), layer.run.picked, layer.run.tab, layer.run.saving, lab.validating().is_some());
    if !layer.dirty && layer.run.built.as_ref() == Some(&key) {
        return;
    }
    let layer = layer.bypass_change_detection();
    layer.dirty = false;
    layer.run.built = Some(key);
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let (fonts, content) = (&*fonts, &content.0);
    let picked = shown(&layer.run, l);
    for (e, panel) in &panels {
        commands.entity(e).despawn_children();
        commands.entity(e).with_children(|p| match panel {
            Panel::Chart => chart(p, &c, l),
            Panel::Grid => grid(p, fonts, &c, l, picked),
            Panel::Picked => picked_design(p, fonts, &c, content, &lab, l, picked, &layer.run),
        });
    }
}

/// A line from `a` to `b` (stage px in its panel), `w` px wide: a thin node turned onto it.
fn segment(p: &mut ChildSpawnerCommands, a: Vec2, b: Vec2, w: f32, color: Color) {
    let (d, mid) = (b - a, (a + b) / 2.0);
    let len = d.length().max(0.5);
    p.spawn((
        Node { position_type: PositionType::Absolute, left: px(mid.x - len / 2.0), top: px(mid.y - w / 2.0), width: px(len), height: px(w), ..default() },
        BackgroundColor(color),
        UiTransform::from_rotation(Rot2::radians(d.y.atan2(d.x))),
    ));
}

/// A line of the chart: its value of a generation, its tone and width (px).
type ChartLine = (fn(&GenerationStats) -> f32, Tone, f32);

/// SCORES BY GENERATION: three lines over the run's generations (the archive's best, each generation's best and
/// mean; 0 at the bottom, 1 at the top) and a bar per generation for the children that took a cell.
fn chart(p: &mut ChildSpawnerCommands, c: &impl Fn(Tone, f32) -> Color, l: &Loaded) {
    let (_, size) = CHART;
    let state = l.run.state();
    // The grid lines at 0, 0.5 and 1.
    for v in [0.0, 0.5, 1.0] {
        let y = size.y * (1.0 - v);
        segment(p, Vec2::new(0.0, y), Vec2::new(size.x, y), 1.0, c(Tone::Dim, if v == 0.5 { 0.25 } else { 0.4 }));
    }
    let history = state.history;
    if history.is_empty() {
        return;
    }
    let every = history.len().div_ceil(CHART_POINTS).max(1);
    let points: Vec<usize> = (0..history.len()).step_by(every).chain(std::iter::once(history.len() - 1)).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    let span = state.generations.saturating_sub(1).max(1) as f32;
    let x = |g: usize| size.x * g as f32 / span;
    let y = |v: f32| size.y * (1.0 - v.clamp(0.0, 1.0));
    let bar_w = (size.x / state.generations.max(1) as f32).clamp(1.0, 8.0);
    let most = history.iter().map(|g| g.evaluated).max().unwrap_or(1).max(1) as f32;
    for &g in &points {
        let h = history[g].improved as f32 / most * BARS_H;
        p.spawn((
            Node { position_type: PositionType::Absolute, left: px(x(g) - bar_w / 2.0), top: px(size.y + 6.0 + BARS_H - h), width: px(bar_w), height: px(h.max(1.0)), ..default() },
            BackgroundColor(c(Tone::Teal, 0.45)),
        ));
    }
    let lines: [ChartLine; 3] = [(|s| s.mean, Tone::Label, 1.2), (|s| s.best, Tone::Teal, 1.5), (|s| s.archive_best, Tone::Gold, 2.0)];
    for (value, tone, w) in lines {
        let pts: Vec<Vec2> = points.iter().map(|&g| Vec2::new(x(g), y(value(&history[g])))).collect();
        for pair in pts.windows(2) {
            segment(p, pair[0], pair[1], w, c(tone, 0.9));
        }
        if let [only] = pts.as_slice() {
            segment(p, *only - Vec2::X * 2.0, *only + Vec2::X * 2.0, w, c(tone, 0.9));
        }
    }
}

/// THE ARCHIVE: a cell per bin of the first two measures (the third, if any, folded in: the best of it), each
/// occupied cell a ship card tinted by its score, the best with a gold edge, a start with a teal corner, the pick lit.
fn grid(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, l: &Loaded, picked: Option<u32>) {
    let (_, size) = GRID;
    let archive = l.run.archive();
    let axes = &archive.shape().axes;
    let cols = axes.first().map_or(1, |a| a.cells) as usize;
    let rows = axes.get(1).map_or(1, |a| a.cells) as usize;
    let cell = (size.x / cols as f32).min(size.y / rows as f32).floor();
    let best = archive.best().map(|e| e.id);
    let starts = l.run.starts().len() as u32;
    let mut shown: BTreeMap<(usize, usize), &Elite> = BTreeMap::new();
    for (key, e) in archive.cells() {
        let at = (key.first().copied().unwrap_or(0) as usize, key.get(1).copied().unwrap_or(0) as usize);
        if shown.get(&at).is_none_or(|o| e.score > o.score) {
            shown.insert(at, e);
        }
    }
    for gy in 0..rows {
        for gx in 0..cols {
            // The second measure grows upwards.
            let at = Vec2::new(gx as f32 * cell, (rows - 1 - gy) as f32 * cell);
            let node = Node { position_type: PositionType::Absolute, left: px(at.x + 1.0), top: px(at.y + 1.0), width: px(cell - 2.0), height: px(cell - 2.0), border: UiRect::all(px(1.0)), border_radius: BorderRadius::all(px(4)), ..default() };
            let Some(e) = shown.get(&(gx, gy)) else {
                p.spawn((node, BorderColor::all(c(Tone::Dim, 0.18)), BackgroundColor(Color::NONE)));
                continue;
            };
            let edge = if Some(e.id) == best { c(Tone::Gold, 0.95) } else { c(Tone::Teal, 0.2 + 0.6 * e.score.clamp(0.0, 1.0)) };
            p.spawn((
                LabButton::Run(RunButton::Pick(e.id)),
                Pill { bare: true, active: Some(e.id) == picked, ..Pill::new(Tone::Teal) },
                Hot::default(),
                node,
                BorderColor::all(edge),
                BackgroundColor(c(Tone::Teal, 0.05 + 0.1 * e.score.clamp(0.0, 1.0))),
                kit::ui_kit::Tip::new(format!("G{}  ·  SCORE {}", e.generation, score(Some(e.score)))),
            ))
            .with_children(|b| {
                b.spawn((UiIcon::Ship(Box::new(e.genome.blueprint.clone())), Node { position_type: PositionType::Absolute, left: px(3), top: px(3), width: px(cell - 8.0), height: px(cell - 8.0), ..default() }));
                if e.id < starts {
                    b.spawn((Node { position_type: PositionType::Absolute, left: px(0), top: px(0), width: px(7), height: px(7), border_radius: BorderRadius::all(px(2)), ..default() }, BackgroundColor(c(Tone::Teal, 1.0))));
                }
            });
        }
    }
    if archive.cells().is_empty() {
        p.spawn((label(fonts, "THE FIRST GENERATION FILLS IT", 9.0, c(Tone::Dim, 1.0)), Node { position_type: PositionType::Absolute, left: px(110), top: px(180), ..default() }));
    }
}

/// THE PICKED DESIGN: its drawing and numbers, the tabs, the tab's rows, SAVE AS DESIGN.
#[allow(clippy::too_many_arguments)]
fn picked_design(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content, lab: &Lab, l: &Loaded, picked: Option<u32>, page: &RunPage) {
    let run = &l.run;
    p.spawn(label(fonts, "THE PICKED DESIGN", 9.0, c(Tone::Dim, 1.0)));
    let Some(g) = picked else {
        p.spawn(prose(fonts, "Nothing scored yet: the first generation is being fought.", 13.0, c(Tone::Dim, 1.0)));
        return;
    };
    let Some(genome) = run.genome(g) else { return };
    let scored = run.scored(g);
    let record = scored.and_then(|(generation, child)| run.state().records.get(generation as usize).and_then(|r| r.children.get(child)).map(|c| (generation, child, c.clone())));
    let start = run.starts().get(genome.root as usize);
    // The drawing and its numbers.
    p.spawn(row(14.0)).with_children(|r| {
        r.spawn(Node { width: px(150), height: px(100), flex_shrink: 0.0, ..default() }).with_children(|b| {
            spawn_brackets(b, 10.0, 1.2, c(Tone::Label, 0.4));
            b.spawn((UiIcon::Ship(Box::new(genome.blueprint.clone())), Node { position_type: PositionType::Absolute, left: px(10), top: px(8), width: px(130), height: px(84), ..default() }));
        });
        r.spawn(column(5.0)).with_children(|t| {
            let from = start.map_or_else(String::new, |s| s.blueprint.name.trim_end_matches(" (evolved)").to_uppercase());
            match &record {
                Some((generation, _, rec)) => {
                    t.spawn(number(fonts, &score(Some(rec.score)), 22.0, c(Tone::Gold, 1.0)));
                    // Scored last by a RE-SCORE: say so, and when it was bred.
                    let rescored = run.state().history.get(*generation as usize).is_some_and(|h| h.rescore);
                    let when = match (rescored, run.table().get(g as usize)) {
                        (true, Some(b)) => format!("RE-SCORED  ·  BRED IN G{}  ·  #{g}", b.generation),
                        _ => format!("GENERATION {generation}  ·  #{g}"),
                    };
                    t.spawn(label(fonts, &when, 8.5, c(Tone::Label, 1.0)));
                    let axes = &run.archive().shape().axes;
                    for (a, v) in axes.iter().zip(&rec.measures) {
                        let (name, unit) = measure_words(content, a.measure);
                        t.spawn(tracked(fonts, &format!("{name} {v:.1} {}", unit.to_uppercase()), 7.5, c(Tone::Dim, 1.0), 0.15));
                    }
                }
                None => {
                    t.spawn(label(fonts, &format!("#{g}  ·  NOT SCORED"), 8.5, c(Tone::Label, 1.0)));
                }
            }
            t.spawn(tracked(fonts, &format!("FROM {from}"), 7.5, c(Tone::Teal, 1.0), 0.15));
            if !page.saving {
                t.spawn(Node { margin: UiRect::top(px(2)), ..row(0.0) }).with_children(|b| {
                    spawn_pill(b, fonts, "SAVE AS DESIGN", 8.0, 24.0, None, Pill { primary: true, ..Pill::new(Tone::Teal) }, LabButton::Run(RunButton::SaveAs));
                });
            }
        });
    });
    // The tabs.
    p.spawn(row(8.0)).with_children(|r| {
        for (t, words) in [(Tab::Scores, "SCORES"), (Tab::Fights, "FIGHTS"), (Tab::Lineage, "LINEAGE"), (Tab::Compare, "COMPARE")] {
            spawn_pill(r, fonts, words, 8.5, 26.0, None, Pill { active: page.tab == t, ..Pill::new(Tone::Teal) }, LabButton::Run(RunButton::Tab(t)));
        }
    });
    let rows_h = if page.saving { 300.0 } else { 340.0 };
    p.spawn((Scrollbar::vertical(), ScrollPosition::default(), Node { flex_direction: FlexDirection::Column, row_gap: px(3), height: px(rows_h), overflow: Overflow::scroll_y(), padding: UiRect::right(px(10)), ..default() }))
        .with_children(|rows| match page.tab {
            Tab::Scores => scores(rows, fonts, c, content, lab, l, g, record.as_ref().map(|(generation, child, rec)| (*generation, *child, rec))),
            Tab::Fights => fights(rows, fonts, c, content, l, record.as_ref().map(|(generation, child, rec)| (*generation, *child, rec))),
            Tab::Lineage => lineage(rows, fonts, c, content, l, g),
            Tab::Compare => compare(rows, fonts, c, content, l, &genome),
        });
}

/// FIGHTS: WATCH BEST and WATCH WORST, then a row per fight.
fn fights(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content, l: &Loaded, record: Option<(u32, usize, &ChildRecord)>) {
    let Some((generation, child, rec)) = record else {
        p.spawn(prose(fonts, "Its fights come with its generation.", 12.0, c(Tone::Dim, 1.0)));
        return;
    };
    let jobs = l.run.tagged_jobs(generation, child).unwrap_or_default();
    let by_score = |best: bool| rec.jobs.iter().enumerate().reduce(|a, b| if (b.1.score > a.1.score) == best && b.1.score != a.1.score { b } else { a }).map(|(k, _)| k);
    p.spawn(Node { margin: UiRect::bottom(px(4)), ..row(8.0) }).with_children(|r| {
        if let Some(k) = by_score(true) {
            spawn_pill(r, fonts, "WATCH BEST", 8.0, 24.0, None, Pill::new(Tone::Teal), LabButton::Run(RunButton::Watch(generation, child, k)));
        }
        if let Some(k) = by_score(false) {
            spawn_pill(r, fonts, "WATCH WORST", 8.0, 24.0, None, Pill::new(Tone::Coral), LabButton::Run(RunButton::Watch(generation, child, k)));
        }
        r.spawn(label(fonts, &format!("{} FIGHTS", rec.jobs.len()), 8.0, c(Tone::Dim, 1.0)));
    });
    let trials = l.run.trials();
    for (k, ((t, job), jr)) in jobs.iter().zip(&rec.jobs).enumerate() {
        let (words, tone) = ending(&jr.end, jr.ticks);
        let trial = trials.get(*t as usize).map_or_else(String::new, |t| trial_name(content, t));
        // The same columns for every fight: how it ended and when, its score, then what it was; under them the seed
        // and the losses, and the buttons. Each text takes the room left of what follows it and clips there.
        p.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(2), flex_shrink: 0.0, padding: UiRect::vertical(px(3)), border: UiRect::bottom(px(1)), ..default() }, BorderColor::all(c(Tone::Dim, 0.25)))).with_children(|f| {
            f.spawn(Node { height: px(18), ..row(8.0) }).with_children(|r| {
                r.spawn((label(fonts, &words, 8.0, c(tone, 1.0)), TextLayout::no_wrap(), Node { width: px(FIGHT_END_W), flex_shrink: 0.0, ..default() }));
                r.spawn((number(fonts, &format!("{:.2}", jr.score), 12.0, c(Tone::Number, 1.0)), Node { width: px(FIGHT_SCORE_W), flex_shrink: 0.0, ..default() }));
                r.spawn(Node { flex_grow: 1.0, flex_shrink: 1.0, min_width: px(0), overflow: Overflow::clip(), ..default() }).with_children(|t| {
                    t.spawn((tracked(fonts, &format!("{trial}  ·  {}", opponent(content, job)), 7.0, c(Tone::Label, 1.0), 0.12), TextLayout::no_wrap()));
                });
            });
            f.spawn(Node { height: px(22), ..row(6.0) }).with_children(|r| {
                r.spawn(Node { flex_grow: 1.0, flex_shrink: 1.0, min_width: px(0), overflow: Overflow::clip(), margin: UiRect::left(px(FIGHT_END_W + 8.0)), ..default() }).with_children(|t| {
                    t.spawn((tracked(fonts, &fight_detail(job, &jr.end), 6.5, c(Tone::Dim, 1.0), 0.12), TextLayout::no_wrap()));
                });
                if matches!(jr.end, JobEnd::Fight { .. }) {
                    spawn_pill(r, fonts, "CODE", 7.0, 20.0, None, Pill::new(Tone::Label), (LabButton::Run(RunButton::Code(generation, child, k)), kit::ui_kit::Tip::new("COPY FIGHT CODE")));
                    spawn_pill(r, fonts, "WATCH", 7.5, 20.0, None, Pill::new(Tone::Teal), LabButton::Run(RunButton::Watch(generation, child, k)));
                }
            });
        });
    }
}

/// SCORES: the design's score per trial (weight, score, each goal's mean over its fights) and in all, VALIDATE and
/// its result.
#[allow(clippy::too_many_arguments)]
fn scores(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content, lab: &Lab, l: &Loaded, g: u32, record: Option<(u32, usize, &ChildRecord)>) {
    let Some((generation, child, rec)) = record else {
        p.spawn(prose(fonts, "Its scores come with its generation.", 12.0, c(Tone::Dim, 1.0)));
        return;
    };
    let Some(b) = l.run.breakdown(generation, child) else { return };
    let trials = l.run.trials();
    let goals: Vec<String> = data::GOALS.iter().map(|id| content.lab.goal(id).map_or_else(|| id.to_uppercase(), |g| g.name.to_uppercase())).collect();
    let weights = l.run.config().goals.weights();
    let cell = |w: f32| Node { width: px(w), flex_shrink: 0.0, justify_content: JustifyContent::FlexEnd, ..default() };
    let line = |p: &mut ChildSpawnerCommands, name: &str, sub: &str, values: [Option<f32>; 5], strong: bool| {
        p.spawn((Node { height: px(30), flex_shrink: 0.0, border: UiRect::bottom(px(1)), ..row(4.0) }, BorderColor::all(c(Tone::Dim, 0.2)))).with_children(|r| {
            r.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(2), flex_grow: 1.0, min_width: px(0), overflow: Overflow::clip(), ..default() }).with_children(|t| {
                t.spawn((tracked(fonts, name, 7.0, c(if strong { Tone::Number } else { Tone::Label }, 1.0), 0.12), TextLayout::no_wrap()));
                t.spawn((tracked(fonts, sub, 6.0, c(Tone::Dim, 1.0), 0.12), TextLayout::no_wrap()));
            });
            for (k, v) in values.iter().enumerate() {
                let text = v.map_or_else(|| "—".into(), |v| format!("{v:.2}"));
                let tone = if k == 0 { Tone::Gold } else if weights[k - 1] > 0.0 { Tone::Number } else { Tone::Dim };
                r.spawn((number(fonts, &text, if k == 0 { 12.0 } else { 10.5 }, c(tone, 1.0)), cell(if k == 0 { SCORE_W } else { GOAL_W })));
            }
        });
    };
    p.spawn(Node { height: px(16), flex_shrink: 0.0, ..row(4.0) }).with_children(|h| {
        h.spawn((tracked(fonts, "TRIAL", 6.5, c(Tone::Dim, 1.0), 0.15), Node { flex_grow: 1.0, ..default() }));
        h.spawn((tracked(fonts, "SCORE", 6.5, c(Tone::Dim, 1.0), 0.15), cell(SCORE_W)));
        for (name, w) in goals.iter().zip(weights) {
            let head = if w > 0.0 { format!("{name} ×{}", short(w)) } else { name.clone() };
            h.spawn((tracked(fonts, &head, 6.0, c(if w > 0.0 { Tone::Label } else { Tone::Dim }, 1.0), 0.1), TextLayout::no_wrap(), cell(GOAL_W)));
        }
    });
    for t in &b.trials {
        let spec = trials[t.trial];
        let sub = match t.jobs {
            0 if spec.weight == 0.0 => "WEIGHS 0: NOT FOUGHT".to_string(),
            0 => "NO FIGHTS YET: IT LEAVES THE SUM".to_string(),
            n => format!("WEIGHT {}  ·  {n} {}", short(spec.weight), if n == 1 { "FIGHT" } else { "FIGHTS" }),
        };
        let gs = t.goals.all().map(|v| (t.jobs > 0).then_some(v));
        line(p, &trial_name(content, spec), &sub, [t.score, gs[0], gs[1], gs[2], gs[3]], false);
    }
    let all = b.goals.all();
    line(p, "IN ALL", "THE TRIALS BY WEIGHT", [Some(rec.score), Some(all[0]), Some(all[1]), Some(all[2]), Some(all[3])], true);
    // VALIDATE: its result, or the buttons (one validation at a time, in the run's place).
    let validated = l.run.validated(g);
    if let Some(v) = validated {
        let tone = if v.as_recorded.0 == v.as_recorded.1 { Tone::Teal } else { Tone::Coral };
        p.spawn((label(fonts, &format!("VALIDATED {:.2} ON {} NEW SEEDS", v.score, v.seeds), 8.0, c(Tone::Gold, 1.0)), TextLayout::no_wrap(), Node { margin: UiRect::top(px(8)), ..default() }));
        p.spawn((label(fonts, &format!("{} OF {} RECORDED FIGHTS END AS RECORDED", v.as_recorded.0, v.as_recorded.1), 7.0, c(tone, 1.0)), TextLayout::no_wrap()));
    }
    p.spawn(Node { margin: UiRect::top(px(8)), ..row(8.0) }).with_children(|r| match lab.validating() {
        Some((id, done, genomes)) => {
            let what = if id == l.id && genomes.contains(&g) { "VALIDATING IT" } else { "A VALIDATION IS UNDERWAY" };
            r.spawn(label(fonts, &format!("{what}  ·  {:.0}%", done * 100.0), 8.0, c(Tone::Gold, 1.0)));
        }
        None if l.run.stale() => {
            r.spawn(label(fonts, "THE RULES HAVE CHANGED: RE-SCORE THE RUN TO VALIDATE", 7.5, c(Tone::Coral, 1.0)));
        }
        None => {
            let again = if validated.is_some() { "VALIDATE AGAIN" } else { "VALIDATE THIS" };
            spawn_pill(r, fonts, again, 8.0, 24.0, None, Pill::new(Tone::Teal), (LabButton::Run(RunButton::Validate), kit::ui_kit::Tip::new("NEW SEEDS AND ITS RECORDED FIGHTS AGAIN")));
            spawn_pill(r, fonts, &format!("VALIDATE THE BEST {VALIDATE_BEST}"), 8.0, 24.0, None, Pill::new(Tone::Teal), LabButton::Run(RunButton::ValidateBest));
        }
    });
}

/// A weight in few characters: `2`, `1.5`.
fn short(w: f32) -> String {
    if w.fract() == 0.0 { format!("{w:.0}") } else { format!("{w:.1}") }
}

/// LINEAGE: its line from its start, each ancestor a small card with its generation, score and what changed.
fn lineage(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content, l: &Loaded, g: u32) {
    let run = &l.run;
    let line = run.lineage(g);
    let mut shown: Vec<u32> = line.clone();
    if shown.len() > LINEAGE {
        let tail = shown.split_off(shown.len() - (LINEAGE - 1));
        shown.truncate(1);
        shown.extend(tail);
    }
    p.spawn(label(fonts, &format!("{} GENERATIONS OF ANCESTORS  ·  A CLICK PICKS ONE", line.len().saturating_sub(1)), 8.0, c(Tone::Dim, 1.0)));
    let mut before: Option<data::Blueprint> = None;
    for (k, id) in shown.iter().enumerate() {
        let Some(genome) = run.genome(*id) else { continue };
        let scored = run.scored(*id).and_then(|(generation, child)| run.state().records.get(generation as usize).and_then(|r| r.children.get(child)).map(|c| (generation, c.score)));
        let what = match &before {
            Some(b) if k == 1 && line.len() > shown.len() => format!("… {} MORE  ·  {}", line.len() - shown.len(), changes(content, b, &genome.blueprint)),
            Some(b) => changes(content, b, &genome.blueprint),
            None => "THE START".into(),
        };
        let behavior = run.table().get(*id as usize).is_some_and(|b| b.parent.is_some() && b.behavior.is_some());
        p.spawn((
            LabButton::Run(RunButton::Pick(*id)),
            Pill { bare: true, active: *id == g, ..Pill::new(Tone::Teal) },
            Hot::default(),
            Node { height: px(48), flex_shrink: 0.0, padding: UiRect::horizontal(px(8)), border: UiRect::all(px(1.0)), border_radius: BorderRadius::all(px(10)), ..row(10.0) },
            BorderColor::all(Color::NONE),
            BackgroundColor(Color::NONE),
        ))
        .with_children(|r| {
            r.spawn((UiIcon::Ship(Box::new(genome.blueprint.clone())), Node { width: px(54), height: px(40), flex_shrink: 0.0, ..default() }));
            // The words take the rest of the row and clip there (a long list of changes).
            r.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(3), flex_grow: 1.0, min_width: px(0), overflow: Overflow::clip(), ..default() }).with_children(|t| {
                let head = scored.map_or_else(|| format!("#{id}"), |(generation, s)| format!("G{generation}  ·  {}  ·  #{id}", score(Some(s))));
                t.spawn(label(fonts, &head, 8.0, c(Tone::Number, 1.0)));
                let what = if behavior { format!("{what}  ·  BEHAVIOR CHANGED") } else { what };
                t.spawn((tracked(fonts, &what, 6.5, c(Tone::Dim, 1.0), 0.12), TextLayout::no_wrap()));
            });
        });
        before = Some(genome.blueprint);
    }
}

/// COMPARE: the start and the design side by side, what changed in parts and in the behavior.
fn compare(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content, l: &Loaded, genome: &world::arena::Genome) {
    let Some(start) = l.run.starts().get(genome.root as usize) else { return };
    p.spawn(row(12.0)).with_children(|r| {
        for (g, words) in [(start, "THE START"), (genome, "THIS DESIGN")] {
            r.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4), align_items: AlignItems::Center, ..default() }).with_children(|col| {
                col.spawn(Node { width: px(170), height: px(120), ..default() }).with_children(|b| {
                    spawn_brackets(b, 9.0, 1.0, c(Tone::Label, 0.35));
                    b.spawn((UiIcon::Ship(Box::new(g.blueprint.clone())), Node { position_type: PositionType::Absolute, left: px(8), top: px(8), width: px(154), height: px(104), ..default() }));
                });
                col.spawn(label(fonts, words, 8.0, c(Tone::Label, 1.0)));
            });
        }
    });
    p.spawn(label(fonts, "THE PARTS", 8.0, c(Tone::Dim, 1.0)));
    p.spawn(prose(fonts, &changes(content, &start.blueprint, &genome.blueprint), 12.0, c(Tone::Number, 1.0)));
    let rules = |t: &str| t.lines().filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with("target")).count();
    let behavior = if start.behavior_text() == genome.behavior_text() {
        "AS THE START'S".to_string()
    } else {
        format!("CHANGED  ·  {} RULES (THE START {})", rules(genome.behavior_text()), rules(start.behavior_text()))
    };
    p.spawn(label(fonts, "THE BEHAVIOR", 8.0, c(Tone::Dim, 1.0)));
    p.spawn(prose(fonts, &behavior, 12.0, c(Tone::Number, 1.0)));
}

/// The progress line, read every [`SYNC_EVERY`] while the page shows; the save row and its name field.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn sync(
    time: Res<Time<Real>>,
    mut since: Local<f32>,
    mut layer: ResMut<EvolutionLayer>,
    lab: Res<Lab>,
    content: Res<world::sim_bridge::GameContent>,
    style: Res<Style>,
    mut texts: Query<(&RunText, &mut Text, &mut TextColor)>,
    mut bar: Single<&mut ValueBar, With<GenerationBar>>,
    mut rows: ParamSet<(Single<&mut Node, With<SaveRow>>, Single<&mut Node, With<RenameRow>>)>,
    mut fields: ParamSet<(Single<&mut TextField, With<SaveField>>, Single<&mut TextField, With<RenameField>>)>,
) {
    if !layer.open || layer.page != Page::Run {
        return;
    }
    // The save row and its field: the default name shown, then what is typed.
    for (open, k) in [(layer.run.saving, 0), (layer.run.renaming, 1)] {
        let want = if open { Display::Flex } else { Display::None };
        let set = |n: &mut Node| {
            if n.display != want {
                n.display = want;
            }
        };
        match k {
            0 => set(&mut rows.p0()),
            _ => set(&mut rows.p1()),
        }
    }
    if layer.run.saving {
        let mut field = fields.p0();
        if field.focused && field.value != layer.run.name {
            layer.bypass_change_detection().run.name = field.value.clone();
        } else if !field.focused && field.value != layer.run.name {
            field.value = layer.run.name.clone();
        }
    }
    if layer.run.renaming {
        let mut field = fields.p1();
        if field.focused && field.value != layer.run.new_name {
            layer.bypass_change_detection().run.new_name = field.value.clone();
        } else if !field.focused && field.value != layer.run.new_name {
            field.value = layer.run.new_name.clone();
        }
    }
    *since += time.delta_secs();
    if *since < SYNC_EVERY && !lab.is_changed() && !layer.is_changed() {
        return;
    }
    *since = 0.0;
    let Some(l) = lab.viewed() else { return };
    let ui = &style.ui;
    let state = l.run.state();
    let entry = lab.entry(&l.id);
    let done = l.run.finished();
    let inside = lab.generation_done(l);
    let share = if done { 1.0 } else { inside };
    if (bar.share - share).abs() > 1e-3 {
        bar.share = share;
    }
    // Fights per second and the time left, from the measured throughput and the last generation's work.
    let last = state.history.last();
    let jobs = state.records.last().and_then(|r| r.children.first()).map_or(0, |c| c.jobs.len()).max(1);
    let per_fight = last.map(|g| g.ticks as f32 / (g.evaluated.max(1) as usize * jobs) as f32);
    let rate = lab.rate.ticks_per_second;
    for (t, mut text, mut color) in &mut texts {
        let (s, tone) = match t {
            RunText::Status => {
                let (words, tone) = if l.run.stale() {
                    ("READ-ONLY", Tone::Coral)
                } else if done {
                    ("DONE", Tone::Gold)
                } else if l.computes() {
                    ("RUNNING", Tone::Teal)
                } else {
                    ("PAUSED", Tone::Label)
                };
                (format!("{words}  ·  SEED {}", l.run.config().seed), tone)
            }
            RunText::Generation => (format!("GENERATION {} / {}", state.generation, state.generations), Tone::Number),
            RunText::Rate if lab.validating().is_some() => {
                let (id, done, genomes) = lab.validating().expect("a validation is underway");
                let whose = if id == l.id { String::new() } else { format!(" OF {}", lab.entry(id).map_or(id, |e| e.name.as_str()).to_uppercase()) };
                (format!("VALIDATING {} {}{whose}  ·  {:.0}%  ·  THE RUN WAITS", genomes.len(), if genomes.len() == 1 { "DESIGN" } else { "DESIGNS" }, done * 100.0), Tone::Gold)
            }
            RunText::Rate => match (per_fight, l.computes() && rate > 0.0) {
                (Some(f), true) => {
                    let left = (state.generations - state.generation) as f32 - inside;
                    let secs = left * last.map_or(0.0, |g| g.ticks as f32) / rate;
                    let last = lab.last_generation.map_or_else(String::new, |(took, _)| format!("  ·  THE LAST GENERATION {}", duration(took)));
                    (format!("ABOUT {:.0} FIGHTS/S  ·  {} LEFT{last}", rate / f.max(1.0), duration(secs)), Tone::Label)
                }
                _ => (String::new(), Tone::Label),
            },
            RunText::Scores => (format!("START {}  »  BEST {}", score(entry.and_then(|e| e.first)), score(entry.and_then(|e| e.best))), Tone::Gold),
            RunText::Stale => (if l.run.stale() && !layer.run.renaming { "THE RULES HAVE CHANGED SINCE THIS RUN  ·  IT IS KEPT READ-ONLY  ·  ITS FIGHTS PLAY UNDER TODAY'S RULES  ·  RE-SCORE MAKES A NEW RUN OF IT" } else { "" }.to_string(), Tone::Coral),
            RunText::Last => match last {
                Some(g) => (format!("LAST GENERATION: {} CHILDREN  ·  {} REFUSED  ·  {} TOOK A CELL  ·  {} CELLS FILLED", g.evaluated, g.rejected, g.improved, g.filled), Tone::Label),
                None => ("THE FIRST GENERATION: THE STARTS AND THEIR MUTANTS".into(), Tone::Label),
            },
            RunText::Axes => {
                let axes = &l.run.archive().shape().axes;
                let words: Vec<String> = axes
                    .iter()
                    .zip(["ACROSS", "UP", "FOLDED IN"])
                    .map(|(a, how)| {
                        let (name, unit) = measure_words(&content.0, a.measure);
                        format!("{how}: {name} {:.0} TO {:.0} {}", a.min, a.max, unit.to_uppercase())
                    })
                    .collect();
                (words.join("  ·  "), Tone::Dim)
            }
        };
        set_text(&mut text, &s);
        set_color(&mut color, ui_color(tone.color(ui), 1.0));
    }
}

/// `4 MIN`, `40 S`, `1 H 20 MIN`.
fn duration(secs: f32) -> String {
    let s = secs.max(0.0) as u64;
    match s {
        0..60 => format!("{s} S"),
        60..3600 => format!("{} MIN", s.div_ceil(60)),
        _ => format!("{} H {} MIN", s / 3600, (s % 3600) / 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What changed is said in part names and counts; a fight's end in the viewer's words.
    #[test]
    fn changes_and_endings_in_words() {
        let content = data::base_content();
        let skiff = content.ships.get("rim_skiff").unwrap().clone();
        assert_eq!(changes(&content, &skiff, &skiff), "THE SAME PARTS");
        let mut less = skiff.clone();
        let gone = less.parts.pop().unwrap();
        let name = content.parts.get(&gone.part).unwrap().name.to_uppercase();
        assert_eq!(changes(&content, &skiff, &less), format!("−1 {name}"));
        assert_eq!(changes(&content, &less, &skiff), format!("+1 {name}"));
        let won = JobEnd::Fight { outcome: Outcome::Won(0), lost: [0.0; 2], dealt: [0.0; 2], left: [1, 0], hash: 0, decided: 0, waves: 0 };
        assert_eq!(ending(&won, 30 * 23), ("WON 0:23".into(), Tone::Teal));
        let lost = JobEnd::Fight { outcome: Outcome::Won(1), lost: [242.0, 2.4], dealt: [40.2, 0.0], left: [0, 1], hash: 0, decided: 0, waves: 0 };
        assert_eq!(ending(&lost, 30 * 62).0, "LOST 1:02");
        // Every outcome has the same columns: the score is a column of its own, the sides' losses say whose.
        let draw = JobEnd::Fight { outcome: Outcome::TimeLimit, lost: [1.0, 3.0], dealt: [5.0, 1.0], left: [1, 1], hash: 0, decided: 0, waves: 0 };
        assert_eq!(ending(&draw, 30 * 60).0, "DRAW 1:00");
        let job = Job::Fight { candidate: 0, spec: world::arena::FightSpec::new(4, vec![world::arena::Entry::ship("rim_skiff")], vec![world::arena::Entry::ship("automaton_raider")]) };
        assert_eq!(fight_detail(&job, &lost), "SEED 4 · YOU LOST 242 BP · THEY LOST 2 BP · DEALT 40");
        assert_eq!(opponent(&content, &job), "AUTOMATON RAIDER");
        assert_eq!((duration(40.0), duration(200.0), duration(4800.0)), ("40 S".into(), "4 MIN".into(), "1 H 20 MIN".into()));
    }
}
