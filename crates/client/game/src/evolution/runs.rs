//! RUNS: every run of the lab, newest first: its name and starts, its status, how far it is, its start's score and
//! its best, when it was made; OPEN, PAUSE or RESUME, and DELETE, which asks in the row first.
//!
//! ```text
//! RUNS  ·  2 OF 8  ·  ONE COMPUTES AT A TIME
//! NAME                 STARTS            STATUS     GENERATION   START → BEST   MADE
//! Evolving Rim skiff   RIM SKIFF COPY    RUNNING    12 / 50      0.42 → 0.81    5 MIN AGO   [PAUSE] [DELETE]
//! ```

use world::arena::evolve::RunStatus;
use bevy::prelude::*;
use kit::style::{Style, ui_color};
use kit::ui_kit::{HudFonts, Hot, Pill, Tone, label, number, prose, spawn_pill, tracked};
use world::evolution::{Lab, MAX_RUNS, RunEntry};
use world::player::library::ago;
use world::storage::now_unix;

use super::{Ctx, EvolutionLayer, LabButton, Page, abs, column, row, score};

/// The list's box on the stage, a row's height and the columns' widths (name, starts, status, generation, scores,
/// made; the buttons take the rest).
const LIST: (Vec2, Vec2) = (Vec2::new(60.0, 100.0), Vec2::new(1160.0, 570.0));
const ROW: f32 = 48.0;
const COLUMNS: [f32; 6] = [250.0, 210.0, 120.0, 100.0, 130.0, 100.0];

#[derive(Component)]
pub(super) struct RunsList;

pub(super) fn spawn(p: &mut ChildSpawnerCommands, _fonts: &HudFonts, _c: &impl Fn(Tone, f32) -> Color) {
    let (at, size) = LIST;
    p.spawn((RunsList, Node { flex_direction: FlexDirection::Column, ..abs(at, Some(size)) }));
}

/// A run's status in words and its tone.
fn status_words(lab: &Lab, e: &RunEntry) -> (&'static str, Tone) {
    if e.problem.is_some() {
        ("DOESN'T READ", Tone::Coral)
    } else if e.stale {
        ("RULES CHANGED", Tone::Coral)
    } else if e.done() {
        ("DONE", Tone::Gold)
    } else if lab.running().is_some_and(|l| l.id == e.id) {
        ("RUNNING", Tone::Teal)
    } else {
        ("PAUSED", Tone::Label)
    }
}

/// PAUSE and RESUME from a row or the run's page.
pub(super) fn status(layer: &mut EvolutionLayer, ctx: &mut Ctx, id: &str, status: RunStatus) {
    let done = match status {
        RunStatus::Running => ctx.lab.resume(&mut ctx.storage, &ctx.content.0, id),
        RunStatus::Paused => ctx.lab.set_status(&mut ctx.storage, id, status),
    };
    let name = ctx.lab.entry(id).map_or_else(|| id.to_string(), |e| e.name.clone());
    match (done, status) {
        (Ok(()), RunStatus::Running) => layer.say(format!("{name} computes  ·  another that did waits"), Tone::Teal),
        (Ok(()), RunStatus::Paused) => layer.say(format!("{name} waits  ·  RESUME goes on where it was"), Tone::Label),
        (Err(e), _) => layer.say(e, Tone::Coral),
    }
    layer.dirty = true;
}

/// The rows, built again when the runs or the page change.
pub(super) fn fill(mut commands: Commands, mut layer: ResMut<EvolutionLayer>, lab: Res<Lab>, fonts: Res<HudFonts>, style: Res<Style>, list: Single<Entity, With<RunsList>>) {
    if !layer.open || layer.page != Page::Runs || !(layer.dirty || lab.is_changed()) {
        return;
    }
    layer.bypass_change_detection().dirty = false;
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let fonts = &*fonts;
    let now = now_unix();
    commands.entity(*list).despawn_children();
    commands.entity(*list).with_children(|p| {
        if lab.runs().is_empty() {
            p.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(10), margin: UiRect::top(px(170)), ..default() }).with_children(|e| {
                e.spawn(label(fonts, "NO RUNS YET", 11.0, c(Tone::Label, 1.0)));
                e.spawn(prose(fonts, "A run starts from your designs, breeds variants of them and fights each against enemies: the best of every kind is kept, every fight can be watched again, and any result saves as a design.", 13.0, c(Tone::Dim, 1.0)));
                e.spawn(Node { height: px(8), ..default() });
                spawn_pill(e, fonts, "NEW RUN", 9.5, 30.0, None, Pill { primary: true, ..Pill::new(Tone::Teal) }, LabButton::NewRun);
            });
            return;
        }
        let head = format!("RUNS  ·  {} OF {MAX_RUNS}  ·  ONE COMPUTES AT A TIME", lab.runs().len());
        p.spawn((label(fonts, &head, 9.5, c(Tone::Dim, 1.0)), Node { margin: UiRect::bottom(px(10)), ..default() }));
        let cell = |w: f32| Node { width: px(w), flex_shrink: 0.0, ..default() };
        p.spawn(Node { padding: UiRect::horizontal(px(14)), margin: UiRect::bottom(px(4)), ..row(0.0) }).with_children(|h| {
            for (word, w) in ["NAME", "STARTS", "STATUS", "GENERATION", "START  »  BEST", "MADE"].into_iter().zip(COLUMNS) {
                h.spawn((label(fonts, word, 8.0, c(Tone::Dim, 1.0)), cell(w)));
            }
        });
        for e in lab.runs() {
            let (status, tone) = status_words(&lab, e);
            let asking = layer.confirm.as_deref() == Some(e.id.as_str());
            p.spawn((
                LabButton::Open(e.id.clone()),
                Pill { bare: true, ..Pill::new(Tone::Teal) },
                Hot::default(),
                Node {
                    height: px(ROW),
                    flex_shrink: 0.0,
                    padding: UiRect::horizontal(px(14)),
                    border: UiRect::all(px(1.2)),
                    border_radius: BorderRadius::all(px(12)),
                    margin: UiRect::bottom(px(4)),
                    ..row(0.0)
                },
                BorderColor::all(Color::NONE),
                BackgroundColor(Color::NONE),
            ))
            .with_children(|r| {
                r.spawn((prose(fonts, &e.name, 15.0, c(Tone::Number, 1.0)), TextLayout::no_wrap(), Node { overflow: Overflow::clip(), ..cell(COLUMNS[0]) }));
                let starts = e.starts.join(" + ").to_uppercase();
                r.spawn((tracked(fonts, &starts, 8.0, c(Tone::Label, 1.0), 0.18), TextLayout::no_wrap(), Node { overflow: Overflow::clip(), ..cell(COLUMNS[1]) }));
                r.spawn((label(fonts, status, 9.0, c(tone, 1.0)), cell(COLUMNS[2])));
                r.spawn((number(fonts, &format!("{} / {}", e.generation, e.generations), 14.0, c(Tone::Number, 1.0)), cell(COLUMNS[3])));
                r.spawn((number(fonts, &format!("{}  »  {}", score(e.first), score(e.best)), 14.0, c(Tone::Gold, 1.0)), cell(COLUMNS[4])));
                let made = e.created.map_or_else(String::new, |t| ago(now, t));
                r.spawn((label(fonts, &made, 8.0, c(Tone::Dim, 1.0)), cell(COLUMNS[5])));
                r.spawn(Node { flex_grow: 1.0, justify_content: JustifyContent::FlexEnd, ..row(8.0) }).with_children(|b| {
                    if asking {
                        b.spawn(label(fonts, "DELETE IT? THERE IS NO UNDO", 8.5, c(Tone::Coral, 1.0)));
                        spawn_pill(b, fonts, "DELETE", 8.5, 26.0, None, Pill { active: true, ..Pill::new(Tone::Coral) }, LabButton::ConfirmDelete(e.id.clone()));
                        spawn_pill(b, fonts, "KEEP", 8.5, 26.0, None, Pill::new(Tone::Teal), LabButton::KeepRun);
                        return;
                    }
                    let goes_on = !e.done() && !e.stale && e.problem.is_none();
                    if goes_on && status == "RUNNING" {
                        spawn_pill(b, fonts, "PAUSE", 8.5, 26.0, None, Pill::new(Tone::Gold), LabButton::Pause(e.id.clone()));
                    } else if goes_on {
                        spawn_pill(b, fonts, "RESUME", 8.5, 26.0, None, Pill::new(Tone::Teal), LabButton::Resume(e.id.clone()));
                    }
                    if e.problem.is_none() {
                        spawn_pill(b, fonts, "OPEN", 8.5, 26.0, None, Pill::new(Tone::Teal), LabButton::Open(e.id.clone()));
                    }
                    spawn_pill(b, fonts, "DELETE", 8.5, 26.0, None, Pill::new(Tone::Coral), LabButton::Delete(e.id.clone()));
                });
            });
            if let Some(why) = &e.problem {
                p.spawn((tracked(fonts, &why.to_uppercase(), 7.5, c(Tone::Coral, 1.0), 0.15), Node { margin: UiRect::new(px(14), px(0), px(-2), px(6)), ..column(0.0) }));
            }
        }
    });
}
