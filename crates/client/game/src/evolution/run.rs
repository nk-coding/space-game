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
//! * **GENERATION n**: a click on a generation in the chart lists its children in the picked design's place, best
//!   first, each with its score, whether it took a cell, its parent, its cell and what changed; a click on one picks
//!   it with its fights.
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
//!   EVOLVED tag (`world::evolution::save_as_design`); **REPLACE START** (a start that is one of YOUR DESIGNS) asks,
//!   then writes over it under its id and name. Either one TIDYs an evolved behavior (on by default): the rules that
//!   never decided in the fights it flew go.
//! * **The ARMS RACE** (GDR-0092 step 7): YOURS | THE ENEMY over the archive switches the archive, the pick and
//!   GENERATION n between the two populations; the chart adds the enemy's archive best (coral) and the PAST CHAMPIONS
//!   line (dots: the share of the other side's champions each best beat, teal yours, coral its), GENERATION n of a
//!   checked generation lists the check's fights to WATCH. An enemy's fights are seen from its side (it is side 1,
//!   coral, in the viewer as in every fight); it saves as a design of its faction tagged `enemy`, and EXPORT (TO
//!   CONTENT) copies it as a content ship with its behavior (`evolved_<faction>_<genome>`) to curate into a pack.

use std::collections::BTreeMap;

use world::arena::evolve::{Camp, ChildRecord, Elite, GenerationStats, Job, JobEnd, Measure, RunStatus, TrialSpec, VALIDATE_BEST, VALIDATE_SEEDS};
use world::arena::{DesignRef, Outcome};
use bevy::prelude::*;
use bevy::ui::UiTransform;
use data::Content;
use kit::style::{Style, ui_color};
use kit::ui_icons::UiIcon;
use kit::ui_kit::{HudFonts, Hot, Pill, Scrollbar, TextField, Tone, ValueBar, field_text, label, number, prose, set_color, set_text, spawn_bar, spawn_brackets, spawn_pill, tracked};
use world::evolution::{Lab, Loaded, SaveTo, replace_target, result_name, save_as_design};
use world::player::library::{self, DesignSlot, MAX_NAME};
use world::storage::{Storage, now_unix};
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

/// What the save panel does: SAVE AS DESIGN (a name), or REPLACE <START> (asked first).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Saving {
    New,
    Replace,
}

/// THE RUN page's state.
#[derive(Default)]
pub(crate) struct RunPage {
    /// The population the archive, the pick and GENERATION n show (an arms race's YOURS | THE ENEMY).
    camp: Camp,
    /// The picked genome (an elite, one of their ancestors, a child of a generation).
    picked: Option<u32>,
    tab: Tab,
    /// GENERATION n: the generation whose children show in the picked design's place.
    generation: Option<u32>,
    /// The save panel, if open; SAVE AS DESIGN's name (the field types into it).
    saving: Option<Saving>,
    name: String,
    /// TIDY at SAVE is off: an evolved behavior keeps the rules that never decided.
    untidy: bool,
    /// RENAME asks for the run's new name.
    renaming: bool,
    new_name: String,
    /// What the page was built for: the run, its generations done, the population, the pick, the tab, GENERATION n,
    /// the save panel and TIDY, whether a validation was underway.
    built: Option<Built>,
}

type Built = (String, u32, Camp, Option<u32>, Tab, Option<u32>, Option<Saving>, bool, bool);

impl RunPage {
    /// The page with genome `genome` of population `camp` picked (a saved design's ORIGIN).
    pub(crate) fn picking(genome: u32, camp: Camp) -> RunPage {
        RunPage { picked: Some(genome), camp, ..default() }
    }
}

/// A click on the run's page.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RunButton {
    Pause,
    Resume,
    Pick(u32),
    Tab(Tab),
    /// GENERATION n (a click on the chart, its ‹ and ›), and back to the picked design.
    Generation(u32),
    CloseGeneration,
    /// A child of GENERATION n: picked, with its fights.
    Child(u32),
    /// WATCH and COPY FIGHT CODE: generation, child, job.
    Watch(u32, usize, usize),
    Code(u32, usize, usize),
    /// WATCH a fight of generation n's PAST CHAMPIONS check (its job).
    WatchCheck(u32, usize),
    /// YOURS | THE ENEMY (an arms race).
    Camp(Camp),
    /// EXPORT TO CONTENT: an evolved enemy as a content ship, copied.
    Export,
    SaveAs,
    Save,
    Cancel,
    /// REPLACE <START>: asks, then writes over the start.
    Replace,
    ConfirmReplace,
    /// TIDY at SAVE on or off.
    Tidy,
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
    Bars,
    Last,
    ArchiveHead,
    Axes,
}

#[derive(Component)]
pub(super) struct GenerationBar;

/// The containers built again when the run or the pick change.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum Panel {
    Chart,
    Camps,
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
    p.spawn((RunText::Bars, label(fonts, "", 7.0, c(Tone::Dim, 1.0)), abs(at + Vec2::Y * (size.y + BARS_H + 12.0), None)));
    p.spawn((RunText::Last, tracked(fonts, "", 7.5, c(Tone::Label, 1.0), 0.15), abs(at + Vec2::Y * (size.y + BARS_H + 34.0), Some(Vec2::new(size.x, 60.0)))));
    // The archive.
    let (at, size) = GRID;
    p.spawn((RunText::ArchiveHead, label(fonts, "", 9.0, c(Tone::Dim, 1.0)), abs(at - Vec2::Y * 26.0, None)));
    // An arms race's YOURS | THE ENEMY, at the archive head's right end.
    p.spawn((Panel::Camps, Node { justify_content: JustifyContent::FlexEnd, ..on_row(abs(at - Vec2::Y * 32.0, Some(Vec2::new(size.x, 24.0))), 6.0) }));
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

/// The genome the page shows: the pick, else the population's archive's best.
fn shown(page: &RunPage, l: &Loaded) -> Option<u32> {
    page.picked.or_else(|| l.run.archive_of(page.camp).and_then(|a| a.best()).map(|e| e.id))
}

/// Applies a click.
pub(super) fn press(layer: &mut EvolutionLayer, ctx: &mut Ctx, b: RunButton) {
    let Some(id) = ctx.lab.viewed().map(|l| l.id.clone()) else { return };
    match b {
        RunButton::Pause => super::runs::status(layer, ctx, &id, RunStatus::Paused),
        RunButton::Resume => super::runs::status(layer, ctx, &id, RunStatus::Running),
        RunButton::Pick(g) => {
            layer.run.picked = Some(g);
            layer.run.saving = None;
        }
        RunButton::Tab(t) => layer.run.tab = t,
        RunButton::Generation(n) => {
            layer.run.generation = Some(n);
            layer.run.saving = None;
        }
        RunButton::CloseGeneration => layer.run.generation = None,
        RunButton::Child(g) => {
            layer.run.picked = Some(g);
            layer.run.generation = None;
            layer.run.tab = Tab::Fights;
        }
        RunButton::Watch(generation, child, job) => {
            let Some(code) = ctx.lab.viewed().and_then(|l| l.run.replay_code(generation, child, job)) else {
                layer.say("that fight doesn't play", Tone::Coral);
                return;
            };
            super::watch(ctx, code.encode());
        }
        RunButton::WatchCheck(generation, job) => {
            let Some(code) = ctx.lab.viewed().and_then(|l| l.run.check_fight_code(generation, job)) else {
                layer.say("that fight doesn't play", Tone::Coral);
                return;
            };
            super::watch(ctx, code.encode());
        }
        RunButton::Camp(camp) => {
            layer.run.camp = camp;
            layer.run.picked = None;
            layer.run.generation = None;
            layer.run.saving = None;
        }
        RunButton::Export => {
            let Some(l) = ctx.lab.viewed() else { return };
            let Some(g) = shown(&layer.run, l) else { return };
            match l.run.export_enemy(g, &l.id, g) {
                Some(text) => {
                    let faction = l.run.genome(g).map_or_else(String::new, |g| g.blueprint.faction);
                    ctx.clipboard.copy = Some(text);
                    layer.say(format!("copied as evolved_{faction}_{g}.ron  ·  a content ship with its behavior at its end: put it into a pack's ships/ to curate it"), Tone::Teal);
                }
                None => layer.say("only an evolved enemy exports to content", Tone::Coral),
            }
        }
        RunButton::Code(generation, child, job) => match ctx.lab.viewed().and_then(|l| l.run.replay_code(generation, child, job)) {
            Some(world::arena::Code::Voyage(code)) => {
                ctx.clipboard.copy = Some(code.encode());
                layer.say("the voyage's code is copied  ·  it plays at ?scene=fight&fight=<code>", Tone::Teal);
            }
            Some(code) => {
                ctx.clipboard.copy = Some(code.encode());
                layer.say("the fight's code is copied  ·  it plays at ?scene=fight&fight=<code>, and in determinism-check.sh --fight", Tone::Teal);
            }
            None => layer.say("that fight has no code", Tone::Coral),
        },
        RunButton::SaveAs => {
            let Some(l) = ctx.lab.viewed() else { return };
            let Some(g) = shown(&layer.run, l) else { return };
            let start = l.run.genome(g).and_then(|g| l.run.start(g.root).map(|s| s.blueprint.name.clone()));
            let start = start.map_or_else(|| l.name.clone(), |n| n.trim_end_matches(" (evolved)").to_string());
            let generation = l.run.scored(g).map_or(0, |(generation, _)| generation);
            layer.run.name = result_name(&start, generation);
            layer.run.saving = Some(Saving::New);
            layer.run.untidy = false;
        }
        RunButton::Replace => {
            layer.run.saving = Some(Saving::Replace);
            layer.run.untidy = false;
        }
        RunButton::Cancel => layer.run.saving = None,
        RunButton::Tidy => layer.run.untidy = !layer.run.untidy,
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
            layer.run.saving = None;
            let Some(l) = ctx.lab.viewed() else { return };
            let Some(g) = shown(&layer.run, l) else { return };
            let name = layer.run.name.trim().to_string();
            if name.is_empty() {
                layer.say("a design needs a name", Tone::Coral);
                return;
            }
            let content = &ctx.content.0;
            match save_as_design(&mut ctx.storage, content, &mut ctx.behaviors, l, g, SaveTo::New(&name), !layer.run.untidy, now_unix()) {
                Ok((_, problems)) => match problems.iter().find(|p| p.is_error()) {
                    Some(p) => layer.say(format!("saved as {name} in your designs  ·  {}", p.message), Tone::Gold),
                    None => layer.say(format!("saved as {name} in your designs  ·  open it in the drydock's blueprints"), Tone::Teal),
                },
                Err(e) => layer.say(format!("not saved: {e}"), Tone::Coral),
            }
        }
        RunButton::ConfirmReplace => {
            layer.run.saving = None;
            let Some(l) = ctx.lab.viewed() else { return };
            let Some(g) = shown(&layer.run, l) else { return };
            let Some((id, name)) = replace_target(&ctx.storage, l, g) else {
                layer.say("its start is none of your designs any more", Tone::Coral);
                return;
            };
            let content = &ctx.content.0;
            if let Err(e) = save_as_design(&mut ctx.storage, content, &mut ctx.behaviors, l, g, SaveTo::Replace, !layer.run.untidy, now_unix()) {
                layer.say(format!("not replaced: {e}"), Tone::Coral);
                return;
            }
            // The drydock's design follows when it is this one and has no unsaved changes.
            let docked = ctx.slot.id.as_deref() == Some(id.as_str());
            let words = match docked && !ctx.slot.dirty(&ctx.design) {
                true => match library::open_saved(&ctx.storage, content, &id) {
                    Ok(next) => {
                        ctx.design.replace(next);
                        *ctx.slot = DesignSlot::opened(Some(id.clone()), &ctx.design);
                        "the drydock has the new version"
                    }
                    Err(_) => "the drydock keeps the old version",
                },
                false if docked => "the drydock keeps your unsaved changes to it",
                false => "fleet's escorts fly the new version",
            };
            layer.say(format!("{name} replaced  ·  {words}"), Tone::Teal);
        }
    }
    layer.dirty = true;
}

/// A measure's label and unit (`lab.ron`'s: a stat's words or its own), its id for a measure the content lacks.
fn measure_words(content: &Content, m: Measure) -> (String, String) {
    content.lab.measure_words(&content.stats, m.id()).map_or_else(|| (m.id().to_uppercase(), String::new()), |(name, unit)| (name.to_uppercase(), unit.to_string()))
}

/// A trial's name: its `lab.ron` preset's, else its kind's.
fn trial_name(content: &Content, t: &TrialSpec) -> String {
    let id = t.kind.kind_id();
    let name = t.preset.as_deref().and_then(|p| content.lab.trial(p)).map(|p| &p.name).or(content.lab.kind(id).map(|k| &k.name));
    name.map_or_else(|| id.to_uppercase(), |n| n.to_uppercase())
}

/// A fight's opponent side in words (side 1, or side 0 for an enemy of the arms race, which is side 1): `2 × AUTOMATON
/// RAIDER`, a design of the player's or the run's by its name (`RIM SKIFF`, without the `(evolved)` every genome's
/// header has), a gauntlet's waves after it (`+ 4 WAVES`); a voyage by its name.
fn opponent(content: &Content, job: &Job) -> String {
    let (spec, side) = match job {
        Job::Fight { spec, side, .. } => (spec, *side as usize),
        Job::Voyage { spec, .. } => return content.voyages.get(&spec.voyage).map_or_else(|| spec.voyage.to_uppercase(), |v| v.name.to_uppercase()),
    };
    let names: Vec<String> = spec.sides[1 - side.min(1)]
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

/// How a job ended for the design (on side `side`) and when, and its tone: `WON 0:23`, `LOST 1:02`, `BOTH OUT 0:40`,
/// `DRAW 1:30`, `VOYAGE 4:10` (its score has a column of its own).
fn ending(end: &JobEnd, ticks: u64, side: usize) -> (String, Tone) {
    let at = clock(ticks);
    match end {
        JobEnd::Fight { outcome: Outcome::Won(s), .. } if *s == side => (format!("WON {at}"), Tone::Teal),
        JobEnd::Fight { outcome: Outcome::Won(_), .. } => (format!("LOST {at}"), Tone::Coral),
        JobEnd::Fight { outcome: Outcome::BothOut, .. } => (format!("BOTH OUT {at}"), Tone::Gold),
        JobEnd::Fight { outcome: Outcome::TimeLimit, .. } => (format!("DRAW {at}"), Tone::Label),
        JobEnd::Voyage { .. } => (format!("VOYAGE {at}"), Tone::Label),
        JobEnd::Failed => ("DIDN'T BUILD".into(), Tone::Coral),
    }
}

/// A fight's second line: the seed, a gauntlet's waves cleared of all, what each side lost (its build points: yours and
/// theirs, a gauntlet's waves all counted), the damage you dealt; an enemy's (side 1 of the arms race) from its side.
fn fight_detail(job: &Job, end: &JobEnd) -> String {
    let (seed, waves, stand_in, side) = match job {
        Job::Fight { spec, side, .. } => (spec.seed, spec.waves.len() as u32 + 1, spec.sides[0].entries.iter().any(|e| e.pilot.is_some()), *side),
        Job::Voyage { spec, .. } => (spec.seed, 0, false, 0),
    };
    match end {
        JobEnd::Fight { lost, dealt, .. } if side == 1 => {
            let flown = if stand_in { " · YOURS FLOWN BY THE STAND-IN" } else { "" };
            format!("SEED {seed}{flown} · IT LOST {:.0} BP · YOURS LOST {:.0} BP · IT DEALT {:.0}", lost[1], lost[0], dealt[1])
        }
        JobEnd::Fight { lost, dealt, waves: cleared, .. } => {
            // A gauntlet's waves first: its row's first line clips its opponent's waves.
            let gauntlet = if waves > 1 { format!(" · {cleared} OF {waves} WAVES CLEARED") } else { String::new() };
            let flown = if stand_in { " · STAND-IN" } else { "" };
            format!("SEED {seed}{flown}{gauntlet} · YOU LOST {:.0} BP · THEY LOST {:.0} BP · DEALT {:.0}", lost[0], lost[1], dealt[0])
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
    storage: Res<Storage>,
    fonts: Res<HudFonts>,
    style: Res<Style>,
    panels: Query<(Entity, &Panel)>,
    touch: Res<kit::ui_kit::TouchScreen>,
) {
    if !layer.open || layer.page != Page::Run {
        return;
    }
    let Some(l) = lab.viewed() else { return };
    let page = &layer.run;
    let key = (l.id.clone(), l.run.generation(), page.camp, page.picked, page.tab, page.generation, page.saving, page.untidy, lab.validating().is_some());
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
    let click = touch.says("A CLICK", "A TAP");
    for (e, panel) in &panels {
        commands.entity(e).despawn_children();
        commands.entity(e).with_children(|p| match panel {
            Panel::Chart => chart(p, &c, l, layer.run.generation, click),
            Panel::Camps => camps(p, fonts, l, layer.run.camp),
            Panel::Grid => grid(p, fonts, &c, l, picked, layer.run.camp),
            Panel::Picked => match layer.run.generation {
                Some(n) => generation(p, fonts, &c, content, l, n, layer.run.camp),
                None => picked_design(p, fonts, &c, content, &storage, &lab, l, picked, &layer.run, click),
            },
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
/// mean; 0 at the bottom, 1 at the top; an arms race's enemy archive best and PAST CHAMPIONS too) and a bar per
/// generation for the children that took a cell; a click on a generation (a column over its lines and bar, `open`
/// lit) opens GENERATION n.
fn chart(p: &mut ChildSpawnerCommands, c: &impl Fn(Tone, f32) -> Color, l: &Loaded, open: Option<u32>, click: &str) {
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
    let column_w = (size.x * every as f32 / span).clamp(4.0, size.x);
    for &g in &points {
        let h = &history[g];
        let left = (x(g) - column_w / 2.0).clamp(0.0, size.x - column_w);
        p.spawn((
            LabButton::Run(RunButton::Generation(g as u32)),
            Pill { bare: true, active: open == Some(g as u32), ..Pill::new(Tone::Teal) },
            Hot::default(),
            Node { position_type: PositionType::Absolute, left: px(left), top: px(0), width: px(column_w), height: px(size.y + 6.0 + BARS_H), border: UiRect::all(px(1.0)), border_radius: BorderRadius::all(px(3)), ..default() },
            BorderColor::all(Color::NONE),
            BackgroundColor(Color::NONE),
            kit::ui_kit::Tip::new(format!("GENERATION {g}  ·  BEST {}  ·  MEAN {}  ·  {click} LISTS ITS CHILDREN", score(Some(h.best)), score(Some(h.mean)))),
        ));
    }
    let most = history.iter().map(|g| g.evaluated).max().unwrap_or(1).max(1) as f32;
    for &g in &points {
        let h = history[g].improved as f32 / most * BARS_H;
        p.spawn((
            Node { position_type: PositionType::Absolute, left: px(x(g) - bar_w / 2.0), top: px(size.y + 6.0 + BARS_H - h), width: px(bar_w), height: px(h.max(1.0)), ..default() },
            BackgroundColor(c(Tone::Teal, 0.45)),
        ));
    }
    // An arms race: the enemy's archive best (coral) under yours, and PAST CHAMPIONS as dots, the share of the other
    // side's champions each best beat (teal yours, coral the enemy's), joined from check to check.
    let enemy: ChartLine = (|s| s.enemy.as_ref().map_or(0.0, |e| e.archive_best), Tone::Coral, 1.5);
    let race = history.iter().any(|h| h.enemy.is_some());
    let mut lines: Vec<ChartLine> = vec![(|s| s.mean, Tone::Label, 1.2), (|s| s.best, Tone::Teal, 1.5)];
    lines.extend(race.then_some(enemy));
    lines.push((|s| s.archive_best, Tone::Gold, 2.0));
    let beaten = l.run.beaten();
    for (yours, tone) in [(true, Tone::Teal), (false, Tone::Coral)] {
        let pts: Vec<Vec2> = beaten
            .iter()
            .map(|b| {
                let (n, m) = if yours { b.yours } else { b.theirs };
                Vec2::new(x(b.generation as usize), y(n as f32 / m.max(1) as f32))
            })
            .collect();
        for pair in pts.windows(2) {
            segment(p, pair[0], pair[1], 1.0, c(tone, 0.55));
        }
        for pt in pts {
            p.spawn((
                Node { position_type: PositionType::Absolute, left: px(pt.x - 3.0), top: px(pt.y - 3.0), width: px(6), height: px(6), border_radius: BorderRadius::MAX, ..default() },
                BackgroundColor(c(tone, 1.0)),
            ));
        }
    }
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

/// YOURS | THE ENEMY (an arms race only): which population the archive, the pick and GENERATION n show.
fn camps(p: &mut ChildSpawnerCommands, fonts: &HudFonts, l: &Loaded, camp: Camp) {
    if l.run.enemy_archive().is_none() {
        return;
    }
    for (k, words, tone) in [(Camp::Yours, "YOURS", Tone::Teal), (Camp::Enemy, "THE ENEMY", Tone::Coral)] {
        let tip = match k {
            Camp::Yours => "YOUR DESIGNS: THE ARCHIVE, THE PICK AND GENERATION n",
            Camp::Enemy => "THE ENEMY THAT EVOLVES AGAINST THEM: ITS ARCHIVE, ITS PICK, ITS CHILDREN",
        };
        spawn_pill(p, fonts, words, 7.5, 22.0, None, Pill { active: camp == k, ..Pill::new(tone) }, (LabButton::Run(RunButton::Camp(k)), kit::ui_kit::Tip::new(tip)));
    }
}

/// THE ARCHIVE (yours, or an arms race's enemy's in coral): a cell per bin of the first two measures (the third, if
/// any, folded in: the best of it), each occupied cell a ship card tinted by its score, the best with a gold edge, a
/// start with a corner of its colour, the pick lit.
fn grid(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, l: &Loaded, picked: Option<u32>, camp: Camp) {
    let (_, size) = GRID;
    let Some(archive) = l.run.archive_of(camp) else { return };
    // THE ENEMY's archive in its colour.
    let tone = if camp == Camp::Enemy { Tone::Coral } else { Tone::Teal };
    let axes = &archive.shape().axes;
    let cols = axes.first().map_or(1, |a| a.cells) as usize;
    let rows = axes.get(1).map_or(1, |a| a.cells) as usize;
    let cell = (size.x / cols as f32).min(size.y / rows as f32).floor();
    let best = archive.best().map(|e| e.id);
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
            let edge = if Some(e.id) == best { c(Tone::Gold, 0.95) } else { c(tone, 0.2 + 0.6 * e.score.clamp(0.0, 1.0)) };
            p.spawn((
                LabButton::Run(RunButton::Pick(e.id)),
                Pill { bare: true, active: Some(e.id) == picked, ..Pill::new(tone) },
                Hot::default(),
                node,
                BorderColor::all(edge),
                BackgroundColor(c(tone, 0.05 + 0.1 * e.score.clamp(0.0, 1.0))),
                kit::ui_kit::Tip::new(format!("G{}  ·  SCORE {}", e.generation, score(Some(e.score)))),
            ))
            .with_children(|b| {
                b.spawn((UiIcon::Ship(Box::new(e.genome.blueprint.clone())), Node { position_type: PositionType::Absolute, left: px(3), top: px(3), width: px(cell - 8.0), height: px(cell - 8.0), ..default() }));
                if l.run.table().get(e.id as usize).is_some_and(|b| b.parent.is_none()) {
                    b.spawn((Node { position_type: PositionType::Absolute, left: px(0), top: px(0), width: px(7), height: px(7), border_radius: BorderRadius::all(px(2)), ..default() }, BackgroundColor(c(tone, 1.0))));
                }
            });
        }
    }
    if archive.cells().is_empty() {
        p.spawn((label(fonts, "THE FIRST GENERATION FILLS IT", 9.0, c(Tone::Dim, 1.0)), Node { position_type: PositionType::Absolute, left: px(110), top: px(180), ..default() }));
    }
}

/// THE PICKED DESIGN: its drawing and numbers, the tabs, the tab's rows, SAVE AS DESIGN and REPLACE <START> (when
/// its start is one of YOUR DESIGNS), and while saving TIDY's line and REPLACE's question.
#[allow(clippy::too_many_arguments)]
fn picked_design(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content, storage: &Storage, lab: &Lab, l: &Loaded, picked: Option<u32>, page: &RunPage, click: &str) {
    let run = &l.run;
    p.spawn(label(fonts, "THE PICKED DESIGN", 9.0, c(Tone::Dim, 1.0)));
    let Some(g) = picked else {
        p.spawn(prose(fonts, "Nothing scored yet: the first generation is being fought.", 13.0, c(Tone::Dim, 1.0)));
        return;
    };
    let Some(genome) = run.genome(g) else { return };
    let scored = run.scored(g);
    let record = scored.and_then(|(generation, child)| run.state().records.get(generation as usize).and_then(|r| r.children.get(child)).map(|c| (generation, child, c.clone())));
    let start = run.start(genome.root);
    let enemy = run.camp(g) == Some(Camp::Enemy);
    let target = replace_target(storage, l, g);
    // The drawing and its numbers (narrow enough for SAVE AS DESIGN and REPLACE START side by side).
    p.spawn(row(12.0)).with_children(|r| {
        r.spawn(Node { width: px(140), height: px(100), flex_shrink: 0.0, ..default() }).with_children(|b| {
            spawn_brackets(b, 10.0, 1.2, c(Tone::Label, 0.4));
            b.spawn((UiIcon::Ship(Box::new(genome.blueprint.clone())), Node { position_type: PositionType::Absolute, left: px(10), top: px(8), width: px(120), height: px(84), ..default() }));
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
                    let axes = run.archive_of(run.camp_of(genome.root)).map_or(&[][..], |a| &a.shape().axes[..]);
                    for (a, v) in axes.iter().zip(&rec.measures) {
                        let (name, unit) = measure_words(content, a.measure);
                        t.spawn(tracked(fonts, &format!("{name} {v:.1} {}", unit.to_uppercase()), 7.5, c(Tone::Dim, 1.0), 0.15));
                    }
                }
                None => {
                    t.spawn(label(fonts, &format!("#{g}  ·  NOT SCORED"), 8.5, c(Tone::Label, 1.0)));
                }
            }
            match enemy {
                true => t.spawn(tracked(fonts, &format!("THE ENEMY  ·  FROM {from}"), 7.5, c(Tone::Coral, 1.0), 0.15)),
                false => t.spawn(tracked(fonts, &format!("FROM {from}"), 7.5, c(Tone::Teal, 1.0), 0.15)),
            };
            if page.saving.is_none() {
                t.spawn(Node { margin: UiRect::top(px(2)), ..row(6.0) }).with_children(|b| {
                    spawn_pill(b, fonts, "SAVE AS DESIGN", 8.0, 24.0, None, Pill { primary: true, ..Pill::new(Tone::Teal) }, LabButton::Run(RunButton::SaveAs));
                    if enemy {
                        let tip = "EXPORT TO CONTENT: COPIES IT AS A CONTENT SHIP WITH ITS BEHAVIOR, FOR A PACK'S SHIPS/ BY HAND";
                        spawn_pill(b, fonts, "EXPORT", 8.0, 24.0, None, Pill::new(Tone::Coral), (LabButton::Run(RunButton::Export), kit::ui_kit::Tip::new(tip)));
                    }
                    if let Some((_, name)) = &target {
                        let tip = format!("WRITES OVER {} IN YOUR DESIGNS UNDER ITS ID: FLEET'S ESCORTS FLY THE NEW VERSION", name.to_uppercase());
                        spawn_pill(b, fonts, "REPLACE START", 8.0, 24.0, None, Pill::new(Tone::Gold), (LabButton::Run(RunButton::Replace), kit::ui_kit::Tip::new(tip)));
                    }
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
    let rows_h = match page.saving {
        None => 340.0,
        Some(Saving::New) => 300.0,
        Some(Saving::Replace) => 290.0,
    };
    p.spawn((Scrollbar::vertical(), ScrollPosition::default(), Node { flex_direction: FlexDirection::Column, row_gap: px(3), height: px(rows_h), overflow: Overflow::scroll_y(), padding: UiRect::right(px(10)), ..default() }))
        .with_children(|rows| match page.tab {
            Tab::Scores => scores(rows, fonts, c, content, lab, l, g, record.as_ref().map(|(generation, child, rec)| (*generation, *child, rec))),
            Tab::Fights => fights(rows, fonts, c, content, l, record.as_ref().map(|(generation, child, rec)| (*generation, *child, rec))),
            Tab::Lineage => lineage(rows, fonts, c, content, l, g, click),
            Tab::Compare => compare(rows, fonts, c, content, l, &genome),
        });
    let Some(saving) = page.saving else { return };
    tidy_line(p, fonts, c, l, g, &genome, page.untidy);
    if let (Saving::Replace, Some((_, name))) = (saving, &target) {
        p.spawn(Node { flex_shrink: 0.0, overflow: Overflow::clip(), ..default() }).with_children(|t| {
            t.spawn((label(fonts, &format!("REPLACE {}? IT KEEPS ITS ID AND NAME", name.to_uppercase()), 8.0, c(Tone::Gold, 1.0)), TextLayout::no_wrap()));
        });
        p.spawn(Node { height: px(28), flex_shrink: 0.0, ..row(8.0) }).with_children(|r| {
            spawn_pill(r, fonts, "REPLACE", 8.5, 26.0, None, Pill { primary: true, ..Pill::new(Tone::Teal) }, LabButton::Run(RunButton::ConfirmReplace));
            spawn_pill(r, fonts, "KEEP", 8.5, 26.0, None, Pill::new(Tone::Teal), LabButton::Run(RunButton::Cancel));
        });
    }
}

/// TIDY at SAVE's line (GDR-0092): for an evolved behavior, how many rules never decided in the fights it flew, and
/// the pill that drops them (on by default; its tip names them); else why there is nothing to tidy.
#[allow(clippy::too_many_arguments)]
fn tidy_line(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, l: &Loaded, g: u32, genome: &world::arena::Genome, untidy: bool) {
    let start = l.run.start(genome.root).map_or("", |s| s.behavior_text());
    let rules: Vec<&str> = genome.behavior_text().lines().filter(|r| !r.trim().is_empty() && !r.trim_start().starts_with("target")).collect();
    let tidied = (genome.behavior_text() != start).then(|| l.run.tidied(g));
    p.spawn(Node { height: px(22), flex_shrink: 0.0, ..row(8.0) }).with_children(|r| {
        let (words, tone) = match &tidied {
            None => ("ITS BEHAVIOR IS ITS START'S: SAVED AS IT IS".to_string(), Tone::Dim),
            Some(None) => ("ITS BEHAVIOR FLEW NO FIGHT: NOTHING TELLS WHICH RULES RAN".to_string(), Tone::Dim),
            Some(Some((_, dropped, fights))) if dropped.is_empty() => (format!("EVERY RULE DECIDED IN ITS {fights} FIGHTS: NOTHING TO TIDY"), Tone::Dim),
            Some(Some((_, dropped, fights))) => {
                let names: Vec<String> = dropped.iter().filter_map(|&i| rules.get(i)).map(|r| r.trim().to_uppercase()).collect();
                let pill = Pill { active: !untidy, ..Pill::new(Tone::Teal) };
                spawn_pill(r, fonts, "TIDY", 8.0, 22.0, None, pill, (LabButton::Run(RunButton::Tidy), kit::ui_kit::Tip::new(format!("DROPS  {}", names.join("  ·  ")))));
                let n = dropped.len();
                let them = if n == 1 { "1 RULE".to_string() } else { format!("{n} RULES") };
                match untidy {
                    false => (format!("DROPS {them} THAT NEVER DECIDED IN ITS {fights} FIGHTS"), Tone::Label),
                    true => (format!("KEEPS ALL {} RULES  ·  {them} NEVER DECIDED", rules.len()), Tone::Label),
                }
            }
        };
        r.spawn(Node { flex_grow: 1.0, flex_shrink: 1.0, min_width: px(0), overflow: Overflow::clip(), ..default() }).with_children(|t| {
            t.spawn((tracked(fonts, &words, 6.5, c(tone, 1.0), 0.12), TextLayout::no_wrap()));
        });
    });
}

/// GENERATION n: its numbers, then its children best first, each a card with its drawing, score, whether it took a
/// cell, its parent (and a crossover's partner), its cell and what changed; a click picks it with its fights.
#[allow(clippy::too_many_arguments)]
fn generation(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content, l: &Loaded, n: u32, camp: Camp) {
    let run = &l.run;
    let state = run.state();
    let (Some(stats), Some(record)) = (state.history.get(n as usize), state.records.get(n as usize)) else { return };
    let Some(archive) = run.archive_of(camp) else { return };
    p.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..row(6.0) }).with_children(|r| {
        r.spawn(label(fonts, &format!("GENERATION {n} OF {}", state.generations), 9.0, c(Tone::Dim, 1.0)));
        r.spawn(row(6.0)).with_children(|b| {
            let last = state.history.len() as u32 - 1;
            spawn_pill(b, fonts, "‹", 9.0, 22.0, None, Pill { disabled: n == 0, ..Pill::new(Tone::Teal) }, (LabButton::Run(RunButton::Generation(n.saturating_sub(1))), kit::ui_kit::Tip::new("THE ONE BEFORE")));
            spawn_pill(b, fonts, "›", 9.0, 22.0, None, Pill { disabled: n >= last, ..Pill::new(Tone::Teal) }, (LabButton::Run(RunButton::Generation((n + 1).min(last))), kit::ui_kit::Tip::new("THE NEXT")));
            spawn_pill(b, fonts, "THE DESIGN", 8.0, 22.0, None, Pill::new(Tone::Teal), (LabButton::Run(RunButton::CloseGeneration), kit::ui_kit::Tip::new("BACK TO THE PICKED DESIGN")));
        });
    });
    let what = if stats.rescore { "  ·  A RE-SCORE" } else { "" };
    let line = match (camp, &stats.enemy) {
        (Camp::Enemy, Some(e)) => format!("THE ENEMY  ·  {} CHILDREN  ·  {} REFUSED  ·  {} TOOK A CELL  ·  BEST {}  ·  MEAN {}{what}", e.evaluated, e.rejected, e.improved, score(Some(e.best)), score(Some(e.mean))),
        _ => format!("{} CHILDREN  ·  {} REFUSED  ·  {} TOOK A CELL  ·  BEST {}  ·  MEAN {}{what}", stats.evaluated, stats.rejected, stats.improved, score(Some(stats.best)), score(Some(stats.mean))),
    };
    p.spawn((tracked(fonts, &line, 7.0, c(Tone::Label, 1.0), 0.12), TextLayout::no_wrap()));
    let mut children: Vec<&ChildRecord> = record.children[run.children_of(n, camp)].iter().collect();
    children.sort_by(|a, b| b.score.total_cmp(&a.score));
    let check = run.check(n).zip(run.check_jobs(n));
    p.spawn((Scrollbar::vertical(), ScrollPosition::default(), Node { flex_direction: FlexDirection::Column, row_gap: px(3), height: px(470), overflow: Overflow::scroll_y(), padding: UiRect::right(px(10)), ..default() }))
        .with_children(|rows| {
            if let Some((check, jobs)) = check {
                past_champions(rows, fonts, c, content, l, n, check, &jobs);
            }
            for ch in children {
                let Some(genome) = run.genome(ch.genome) else { continue };
                let bred = run.table().get(ch.genome as usize);
                let parent = bred.and_then(|b| b.parent).filter(|_| !stats.rescore);
                let changed = match parent.and_then(|id| run.genome(id)) {
                    Some(from) => changes(content, &from.blueprint, &genome.blueprint),
                    None if stats.rescore => "FOUGHT AGAIN".into(),
                    None => "A START".into(),
                };
                let changed = match bred.filter(|_| parent.is_some()) {
                    Some(b) if b.behavior.is_some() && b.assists.is_some() => format!("{changed}  ·  BEHAVIOR AND ASSISTS CHANGED"),
                    Some(b) if b.behavior.is_some() => format!("{changed}  ·  BEHAVIOR CHANGED"),
                    Some(b) if b.assists.is_some() => format!("{changed}  ·  ASSISTS CHANGED"),
                    _ => changed,
                };
                let from = match (parent, bred.and_then(|b| b.partner)) {
                    (Some(a), Some(b)) => format!("FROM #{a} × #{b}  ·  "),
                    (Some(a), None) => format!("FROM #{a}  ·  "),
                    _ => String::new(),
                };
                let cell: Vec<String> = archive.cell_of(&ch.measures).iter().map(|v| v.to_string()).collect();
                rows.spawn((
                    LabButton::Run(RunButton::Child(ch.genome)),
                    Pill { bare: true, ..Pill::new(Tone::Teal) },
                    Hot::default(),
                    Node { height: px(48), flex_shrink: 0.0, padding: UiRect::horizontal(px(8)), border: UiRect::all(px(1.0)), border_radius: BorderRadius::all(px(10)), ..row(10.0) },
                    BorderColor::all(Color::NONE),
                    BackgroundColor(Color::NONE),
                ))
                .with_children(|r| {
                    r.spawn((UiIcon::Ship(Box::new(genome.blueprint.clone())), Node { width: px(54), height: px(40), flex_shrink: 0.0, ..default() }));
                    r.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(3), flex_grow: 1.0, min_width: px(0), overflow: Overflow::clip(), ..default() }).with_children(|t| {
                        t.spawn(row(8.0)).with_children(|h| {
                            h.spawn(label(fonts, &format!("{}  ·  #{}", score(Some(ch.score)), ch.genome), 8.0, c(Tone::Number, 1.0)));
                            if ch.took {
                                h.spawn(label(fonts, "TOOK A CELL", 7.0, c(Tone::Gold, 1.0)));
                            }
                        });
                        t.spawn((tracked(fonts, &format!("{from}CELL {}  ·  {changed}", cell.join(", ")), 6.5, c(Tone::Dim, 1.0), 0.12), TextLayout::no_wrap()));
                    });
                });
            }
        });
}

/// PAST CHAMPIONS of GENERATION n (an arms race): what each best beat, then a row per champion it fought (its
/// generation, its score over its seeds, WATCH of its first fight).
#[allow(clippy::too_many_arguments)]
fn past_champions(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content, l: &Loaded, n: u32, check: &world::arena::evolve::Check, jobs: &[Job]) {
    let run = &l.run;
    let Some(b) = run.beaten().into_iter().find(|b| b.generation == n) else { return };
    let line = format!("PAST CHAMPIONS  ·  YOUR BEST #{} BEAT {} OF {}  ·  THE ENEMY'S #{} {} OF {}", check.yours, b.yours.0, b.yours.1, check.theirs, b.theirs.0, b.theirs.1);
    p.spawn((tracked(fonts, &line, 7.0, c(Tone::Gold, 1.0), 0.12), TextLayout::no_wrap(), Node { flex_shrink: 0.0, ..default() }));
    let seeds = (jobs.len() / (check.their_champions.len() + check.your_champions.len()).max(1)).max(1);
    let champions = check.their_champions.iter().map(|&id| (id, true)).chain(check.your_champions.iter().map(|&id| (id, false)));
    for (k, (id, yours)) in champions.enumerate() {
        let first = k * seeds;
        let Some(recs) = check.jobs.get(first..first + seeds) else { continue };
        let mean = recs.iter().map(|r| r.score).sum::<f32>() / seeds as f32;
        let born = run.table().get(id as usize).map_or(0, |b| b.generation);
        let who = match yours {
            true => format!("YOUR BEST VS ITS CHAMPION OF G{born} (#{id})"),
            false => format!("ITS BEST VS YOUR CHAMPION OF G{born} (#{id})"),
        };
        let against = jobs.get(first).map_or_else(String::new, |j| opponent(content, j));
        p.spawn((Node { height: px(24), flex_shrink: 0.0, border: UiRect::bottom(px(1)), ..row(8.0) }, BorderColor::all(c(Tone::Dim, 0.2)))).with_children(|r| {
            r.spawn((number(fonts, &format!("{mean:.2}"), 11.0, c(if mean > 0.5 { Tone::Teal } else { Tone::Coral }, 1.0)), Node { width: px(FIGHT_SCORE_W), flex_shrink: 0.0, ..default() }));
            r.spawn(Node { flex_grow: 1.0, flex_shrink: 1.0, min_width: px(0), overflow: Overflow::clip(), ..default() }).with_children(|t| {
                t.spawn((tracked(fonts, &format!("{who}  ·  {against}"), 6.5, c(Tone::Label, 1.0), 0.12), TextLayout::no_wrap()));
            });
            spawn_pill(r, fonts, "WATCH", 7.5, 20.0, None, Pill::new(if yours { Tone::Teal } else { Tone::Coral }), LabButton::Run(RunButton::WatchCheck(n, first)));
        });
    }
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
        let side = match job {
            Job::Fight { side, .. } => *side as usize,
            Job::Voyage { .. } => 0,
        };
        let (words, tone) = ending(&jr.end, jr.ticks, side);
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
                if !matches!(jr.end, JobEnd::Failed) {
                    let tip = if matches!(jr.end, JobEnd::Voyage { .. }) { "COPY VOYAGE CODE" } else { "COPY FIGHT CODE" };
                    spawn_pill(r, fonts, "CODE", 7.0, 20.0, None, Pill::new(Tone::Label), (LabButton::Run(RunButton::Code(generation, child, k)), kit::ui_kit::Tip::new(tip)));
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
    let how = if l.run.camp(g) == Some(Camp::Enemy) { "ITS WIN AS SIDE 1" } else { "THE TRIALS BY WEIGHT" };
    line(p, "IN ALL", how, [Some(rec.score), Some(all[0]), Some(all[1]), Some(all[2]), Some(all[3])], true);
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
        None if l.run.camp(g) == Some(Camp::Enemy) => {
            r.spawn((label(fonts, "THE ENEMY IS SCORED ONLY BY ITS FIGHTS AGAINST YOUR DESIGNS", 7.0, c(Tone::Dim, 1.0)), TextLayout::no_wrap()));
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
fn lineage(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content, l: &Loaded, g: u32, click: &str) {
    let run = &l.run;
    let line = run.lineage(g);
    let mut shown: Vec<u32> = line.clone();
    if shown.len() > LINEAGE {
        let tail = shown.split_off(shown.len() - (LINEAGE - 1));
        shown.truncate(1);
        shown.extend(tail);
    }
    p.spawn(label(fonts, &format!("{} GENERATIONS OF ANCESTORS  ·  {click} PICKS ONE", line.len().saturating_sub(1)), 8.0, c(Tone::Dim, 1.0)));
    let mut before: Option<data::Blueprint> = None;
    for (k, id) in shown.iter().enumerate() {
        let Some(genome) = run.genome(*id) else { continue };
        let scored = run.scored(*id).and_then(|(generation, child)| run.state().records.get(generation as usize).and_then(|r| r.children.get(child)).map(|c| (generation, c.score)));
        let what = match &before {
            Some(b) if k == 1 && line.len() > shown.len() => format!("… {} MORE  ·  {}", line.len() - shown.len(), changes(content, b, &genome.blueprint)),
            Some(b) => changes(content, b, &genome.blueprint),
            None => "THE START".into(),
        };
        let bred = run.table().get(*id as usize).filter(|b| b.parent.is_some());
        let (behavior, assists) = (bred.is_some_and(|b| b.behavior.is_some()), bred.is_some_and(|b| b.assists.is_some()));
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
                let what = if assists { format!("{what}  ·  ASSISTS CHANGED") } else { what };
                t.spawn((tracked(fonts, &what, 6.5, c(Tone::Dim, 1.0), 0.12), TextLayout::no_wrap()));
            });
        });
        before = Some(genome.blueprint);
    }
}

/// COMPARE: the start and the design side by side, what changed in parts, in the behavior and in the pilot assists.
fn compare(p: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content, l: &Loaded, genome: &world::arena::Genome) {
    let Some(start) = l.run.start(genome.root) else { return };
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
    let count = |g: &world::arena::Genome| g.blueprint.assists.as_ref().map_or(0, |a| a.rules.len());
    let assists = match (start.blueprint.assists == genome.blueprint.assists, count(genome)) {
        (true, 0) => "NONE, AS THE START".to_string(),
        (true, _) => "AS THE START'S".to_string(),
        (false, n) => format!("CHANGED  ·  {n} RULES (THE START {})", count(start)),
    };
    p.spawn(label(fonts, "THE PILOT ASSISTS", 8.0, c(Tone::Dim, 1.0)));
    p.spawn(prose(fonts, &assists, 12.0, c(Tone::Number, 1.0)));
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
    let naming = layer.run.saving == Some(Saving::New);
    for (open, k) in [(naming, 0), (layer.run.renaming, 1)] {
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
    if naming {
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
            RunText::Scores => {
                let enemy = l.run.enemy_archive().and_then(|a| a.best()).map_or_else(String::new, |e| format!("  ·  ENEMY {}", score(Some(e.score))));
                (format!("START {}  »  BEST {}{enemy}", score(entry.and_then(|e| e.first)), score(entry.and_then(|e| e.best))), Tone::Gold)
            }
            // An arms race's head is short: YOURS | THE ENEMY follow it.
            RunText::ArchiveHead => match l.run.enemy_archive() {
                Some(_) => ("THE ARCHIVES".into(), Tone::Dim),
                None => ("THE ARCHIVE  ·  THE BEST OF EACH KIND".into(), Tone::Dim),
            },
            RunText::Bars => match l.run.enemy_archive() {
                Some(_) => ("BARS: CHILDREN THAT TOOK A CELL  ·  CORAL: THE ENEMY'S BEST  ·  DOTS: PAST CHAMPIONS BEATEN".into(), Tone::Dim),
                None => ("BARS: CHILDREN THAT TOOK A CELL".into(), Tone::Dim),
            },
            RunText::Stale => (if l.run.stale() && !layer.run.renaming { "THE RULES HAVE CHANGED SINCE THIS RUN  ·  IT IS KEPT READ-ONLY  ·  ITS FIGHTS PLAY UNDER TODAY'S RULES  ·  RE-SCORE MAKES A NEW RUN OF IT" } else { "" }.to_string(), Tone::Coral),
            RunText::Last => match last {
                Some(g) => {
                    let mut words = format!("LAST GENERATION: {} CHILDREN  ·  {} REFUSED  ·  {} TOOK A CELL  ·  {} CELLS FILLED", g.evaluated, g.rejected, g.improved, g.filled);
                    // An arms race: the enemy's, and the last PAST CHAMPIONS check.
                    if let Some(e) = &g.enemy {
                        words += &format!("\nTHE ENEMY: {} CHILDREN  ·  {} TOOK A CELL  ·  {} CELLS  ·  ITS BEST {}", e.evaluated, e.improved, e.filled, score(Some(e.archive_best)));
                    }
                    if let Some(b) = l.run.beaten().last() {
                        words += &format!("\nPAST CHAMPIONS G{}: YOUR BEST BEAT {} OF {}, THE ENEMY'S {} OF {}", b.generation, b.yours.0, b.yours.1, b.theirs.0, b.theirs.1);
                    }
                    (words, Tone::Label)
                }
                None => ("THE FIRST GENERATION: THE STARTS AND THEIR MUTANTS".into(), Tone::Label),
            },
            RunText::Axes => {
                let axes = l.run.archive_of(layer.run.camp).map_or(&[][..], |a| &a.shape().axes[..]);
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
        assert_eq!(ending(&won, 30 * 23, 0), ("WON 0:23".into(), Tone::Teal));
        let lost = JobEnd::Fight { outcome: Outcome::Won(1), lost: [242.0, 2.4], dealt: [40.2, 0.0], left: [0, 1], hash: 0, decided: 0, waves: 0 };
        assert_eq!(ending(&lost, 30 * 62, 0).0, "LOST 1:02");
        // An enemy of the arms race is side 1: side 0 losing is its win, its losses its own.
        assert_eq!(ending(&lost, 30 * 62, 1).0, "WON 1:02");
        // Every outcome has the same columns: the score is a column of its own, the sides' losses say whose.
        let draw = JobEnd::Fight { outcome: Outcome::TimeLimit, lost: [1.0, 3.0], dealt: [5.0, 1.0], left: [1, 1], hash: 0, decided: 0, waves: 0 };
        assert_eq!(ending(&draw, 30 * 60, 0).0, "DRAW 1:00");
        let spec = world::arena::FightSpec::new(4, vec![world::arena::Entry::ship("rim_skiff")], vec![world::arena::Entry::ship("automaton_raider")]);
        let job = Job::Fight { candidate: 0, side: 0, spec: spec.clone() };
        assert_eq!(fight_detail(&job, &lost), "SEED 4 · YOU LOST 242 BP · THEY LOST 2 BP · DEALT 40");
        assert_eq!(opponent(&content, &job), "AUTOMATON RAIDER");
        let theirs = Job::Fight { candidate: 0, side: 1, spec };
        assert_eq!(fight_detail(&theirs, &lost), "SEED 4 · IT LOST 2 BP · YOURS LOST 242 BP · IT DEALT 0");
        assert_eq!(opponent(&content, &theirs), "RIM SKIFF");
        assert_eq!((duration(40.0), duration(200.0), duration(4800.0)), ("40 S".into(), "4 MIN".into(), "1 H 20 MIN".into()));
    }
}
