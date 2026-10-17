//! THE LADDER (GDR-0092 step 9; GDR-0083's benchmark rating): YOUR DESIGNS rated on `lab.ron`'s ladder, fixed rungs
//! of enemies from the weakest up, each fought on its seeds. A design's rating is the rungs it wins, added up
//! (`4.5 / 7`); each rung says how often it won there, and WATCH plays its worst fight on it (what beat it).
//!
//! ```text
//! THE LADDER  ·  7 RUNGS  ·  3 SEEDS EACH  ·  THE SAME FOR EVERY DESIGN          RIM SKIFF COPY  ·  LADDER 4.5 / 7
//! NAME              LADDER     RATED                                [RATE]      1  AUTOMATON RAIDER       3 OF 3  [WATCH]
//! Rim skiff copy    4.5 / 7    5 MIN AGO                            [RATE]      2  2 × AUTOMATON RAIDER   2 OF 3  [WATCH]
//! Runner mk2        —          NOT RATED                            [RATE]      ...
//! ```
//!
//! * RATE (a row) and RATE ALL (top right) queue designs; the lab rates one after the other in the run's place, as
//!   VALIDATE does (the run waits), on the web in its workers (`world::evolution`, ADR-0043). A rating is kept in
//!   `ladder/<design id>`; one made before the design, its behavior, the ladder or the rules changed is shown in
//!   coral with `BEFORE ITS LAST CHANGE`.
//! * A click on a row shows its rungs on the right.

use bevy::prelude::*;
use data::Content;
use kit::style::{Style, ui_color};
use kit::ui_kit::{BAR_HIT, HudFonts, Hot, Pill, Scrollbar, Tip, Tone, label, number, prose, spawn_pill, tracked};
use world::arena::evolve::JobEnd;
use world::arena::evolve::ladder::Ladder;
use world::arena::{DesignRef, Entry, Outcome};
use world::evolution::Lab;
use world::evolution::ladder::{RatingDoc, Standing};
use world::player::library::{self, ago};
use world::storage::now_unix;

use super::{Ctx, EvolutionLayer, LabButton, Page, abs, row, watch};

/// The designs' list and the picked design's rungs on the stage, a row's height and the list's columns (name, rating,
/// when; the button takes the rest).
const LIST: (Vec2, Vec2) = (Vec2::new(60.0, 100.0), Vec2::new(600.0, 570.0));
const RUNGS: (Vec2, Vec2) = (Vec2::new(690.0, 100.0), Vec2::new(530.0, 570.0));
const ROW: f32 = 44.0;
const COLUMNS: [f32; 3] = [210.0, 90.0, 190.0];

/// THE LADDER's state: the design whose rungs show.
#[derive(Default)]
pub(super) struct LadderPage {
    picked: Option<String>,
}

/// What a click on the page does.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum LadderButton {
    Rate(String),
    RateAll,
    Pick(String),
    /// Fight `k` (job order) of a design's rating.
    Watch(String, usize),
}

#[derive(Component)]
pub(super) struct LadderList;

#[derive(Component)]
pub(super) struct LadderRungs;

pub(super) fn spawn(p: &mut ChildSpawnerCommands, _fonts: &HudFonts, _c: &impl Fn(Tone, f32) -> Color) {
    let (at, size) = LIST;
    p.spawn((
        LadderList,
        Scrollbar::vertical(),
        ScrollPosition::default(),
        Node { flex_direction: FlexDirection::Column, overflow: Overflow::scroll_y(), padding: UiRect::right(px(BAR_HIT)), ..abs(at, Some(size)) },
    ));
    let (at, size) = RUNGS;
    p.spawn((LadderRungs, Node { flex_direction: FlexDirection::Column, row_gap: px(6), ..abs(at, Some(size)) }));
}

/// The clicks of the page.
pub(super) fn press(layer: &mut EvolutionLayer, ctx: &mut Ctx, b: LadderButton) {
    match b {
        LadderButton::Rate(id) => rate(layer, ctx, &[id]),
        LadderButton::RateAll => {
            let ids: Vec<String> = library::list_designs(&ctx.storage, &ctx.content.0).unwrap_or_default().into_iter().filter(|e| e.problem.is_none()).map(|e| e.id).collect();
            rate(layer, ctx, &ids);
        }
        LadderButton::Pick(id) => {
            layer.ladder.picked = Some(id);
            layer.dirty = true;
        }
        LadderButton::Watch(id, k) => match ctx.storage.read::<RatingDoc>(&id).ok().flatten().and_then(|d| d.doc.code(&ctx.content.0, k)) {
            Some(code) => watch(ctx, code.encode()),
            None => layer.say("that fight doesn't play", Tone::Coral),
        },
    }
}

fn rate(layer: &mut EvolutionLayer, ctx: &mut Ctx, ids: &[String]) {
    match ctx.lab.rate(&ctx.storage, &ctx.content.0, &ctx.behaviors, &ctx.design, ids) {
        Ok(0) => layer.say("those are being rated already", Tone::Label),
        Ok(n) => layer.say(format!("{n} {} on the ladder  ·  a run waits meanwhile", if n == 1 { "design goes" } else { "designs go" }), Tone::Teal),
        Err(e) => layer.say(format!("not rated: {e}"), Tone::Coral),
    }
    layer.dirty = true;
}

/// A rating in numbers: `4.5 / 7`.
pub fn rating_words(doc: &RatingDoc) -> String {
    format!("{:.1} / {}", doc.rating.total(), doc.rating.of())
}

/// A rung's opponents in words: `2 × AUTOMATON RAIDER + AUTOMATON HUNTER`.
fn rung_name(content: &Content, rung: &[Entry]) -> String {
    let names: Vec<String> = rung
        .iter()
        .map(|e| {
            let name = match &e.design {
                DesignRef::Ship(id) => content.ships.get(id).map_or_else(|| id.to_uppercase(), |b| b.name.to_uppercase()),
                DesignRef::Blueprint(_) => "A DESIGN".into(),
            };
            if e.count > 1 { format!("{} × {name}", e.count) } else { name }
        })
        .collect();
    names.join(" + ")
}

/// The list and the rungs, built again when the page, the lab or the rating's progress (in tenths) changes.
#[allow(clippy::too_many_arguments)]
pub(super) fn fill(
    mut commands: Commands,
    mut layer: ResMut<EvolutionLayer>,
    mut lab: ResMut<Lab>,
    ctx_storage: Res<world::storage::Storage>,
    content: Res<world::sim_bridge::GameContent>,
    behaviors: Res<world::player::behaviors::PlayerBehaviors>,
    working: Res<world::player::PlayerDesign>,
    mut shown: Local<Option<(String, u32)>>,
    fonts: Res<HudFonts>,
    style: Res<Style>,
    list: Single<Entity, With<LadderList>>,
    rungs: Single<Entity, With<LadderRungs>>,
) {
    let progress = lab.rating().map(|(id, _, done, _)| (id.to_string(), (done * 10.0) as u32));
    if !layer.open || layer.page != Page::Ladder || !(layer.dirty || lab.is_changed() || progress != *shown) {
        return;
    }
    *shown = progress;
    layer.bypass_change_detection().dirty = false;
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let fonts = &*fonts;
    let content = &content.0;
    let now = now_unix();
    let rules = lab.bypass_change_detection().rules(content);
    let rating = lab.rating().map(|(id, _, done, waiting)| (id.to_string(), done, waiting));
    let designs: Vec<_> = library::list_designs(&ctx_storage, content).unwrap_or_default().into_iter().filter(|e| e.problem.is_none()).collect();
    let standings: Vec<Standing> = designs.iter().map(|e| Standing::of(&ctx_storage, content, &behaviors, &working, rules, &e.id)).collect();
    let picked = layer.ladder.picked.clone().filter(|p| designs.iter().any(|e| e.id == *p)).or_else(|| designs.first().map(|e| e.id.clone()));
    let Some(ladder) = Ladder::of(content) else { return };
    commands.entity(*list).despawn_children();
    commands.entity(*rungs).despawn_children();
    let cell = |w: f32| Node { width: px(w), flex_shrink: 0.0, ..default() };
    commands.entity(*list).with_children(|p| {
        let head = format!("THE LADDER  ·  {} RUNGS  ·  {} SEEDS EACH  ·  THE SAME FOR EVERY DESIGN", ladder.rungs().len(), ladder.trial.seeds);
        p.spawn((label(fonts, &head, 9.5, c(Tone::Dim, 1.0)), Node { margin: UiRect::bottom(px(10)), ..default() }));
        if designs.is_empty() {
            p.spawn((prose(fonts, "No saved designs yet: the drydock's SAVE puts one on this list.", 13.0, c(Tone::Dim, 1.0)), Node { margin: UiRect::top(px(40)), ..default() }));
            return;
        }
        p.spawn(Node { padding: UiRect::horizontal(px(14)), margin: UiRect::bottom(px(4)), ..row(0.0) }).with_children(|h| {
            for (word, w) in ["NAME", "LADDER", "RATED"].into_iter().zip(COLUMNS) {
                h.spawn((label(fonts, word, 8.0, c(Tone::Dim, 1.0)), cell(w)));
            }
        });
        for (e, st) in designs.iter().zip(&standings) {
            let on = picked.as_deref() == Some(e.id.as_str());
            let (when, tone) = match (&rating, st) {
                (Some((id, done, _)), _) if *id == e.id => (format!("RATING  ·  {:.0} %", done * 100.0), Tone::Teal),
                _ if lab.rating().is_some() && lab.queued(&e.id) => ("WAITING TO BE RATED".to_string(), Tone::Label),
                (_, Standing::Unrated) => ("NOT RATED".to_string(), Tone::Dim),
                (_, Standing::Rated(_)) => (ctx_storage.read::<RatingDoc>(&e.id).ok().flatten().and_then(|d| d.saved).map_or_else(|| "RATED".into(), |t| ago(now, t)), Tone::Label),
                (_, Standing::Changed(_)) => ("BEFORE ITS LAST CHANGE".to_string(), Tone::Coral),
            };
            p.spawn((
                LabButton::Ladder(LadderButton::Pick(e.id.clone())),
                Pill { bare: true, active: on, ..Pill::new(Tone::Teal) },
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
                r.spawn((prose(fonts, &e.name, 14.0, c(Tone::Number, 1.0)), TextLayout::no_wrap(), Node { overflow: Overflow::clip(), ..cell(COLUMNS[0]) }));
                let (words, rated) = st.doc().map_or(("—".to_string(), Tone::Dim), |d| (rating_words(d), if matches!(st, Standing::Rated(_)) { Tone::Gold } else { Tone::Coral }));
                r.spawn((number(fonts, &words, 14.0, c(rated, 1.0)), cell(COLUMNS[1])));
                r.spawn((label(fonts, &when, 8.0, c(tone, 1.0)), cell(COLUMNS[2])));
                r.spawn(Node { flex_grow: 1.0, justify_content: JustifyContent::FlexEnd, ..row(8.0) }).with_children(|b| {
                    spawn_pill(b, fonts, "RATE", 8.5, 26.0, None, Pill::new(Tone::Teal), (LabButton::Ladder(LadderButton::Rate(e.id.clone())), Tip::new("RATE IT ON THE LADDER")));
                });
            });
        }
    });
    let Some((k, e)) = picked.as_ref().and_then(|p| designs.iter().enumerate().find(|(_, e)| e.id == *p)) else { return };
    let doc = standings[k].doc().cloned();
    commands.entity(*rungs).with_children(|p| {
        let head = match &doc {
            Some(d) => format!("{}  ·  LADDER {}", e.name.to_uppercase(), rating_words(d)),
            None => format!("{}  ·  NOT RATED YET", e.name.to_uppercase()),
        };
        p.spawn(label(fonts, &head, 9.5, c(Tone::Label, 1.0)));
        if let Some(summary) = ladder.trial.preset.as_deref().and_then(|id| content.lab.trial(id)) {
            p.spawn((prose(fonts, &summary.summary, 11.5, c(Tone::Dim, 1.0)), Node { margin: UiRect::bottom(px(6)), ..default() }));
        }
        let seeds = ladder.trial.seeds.max(1) as usize;
        for (r, rung) in ladder.rungs().iter().enumerate() {
            let fights = doc.as_ref().map(|d| &d.records[(r * seeds).min(d.records.len())..((r + 1) * seeds).min(d.records.len())]);
            let won = fights.map(|f| f.iter().filter(|j| matches!(j.end, JobEnd::Fight { outcome: Outcome::Won(0), .. })).count());
            let mean = doc.as_ref().and_then(|d| d.rating.rungs.get(r).copied());
            // Its worst fight: what beat it on this rung.
            let worst = fights.and_then(|f| f.iter().enumerate().min_by(|a, b| a.1.score.total_cmp(&b.1.score)).map(|(j, _)| r * seeds + j));
            p.spawn((Node { height: px(34), flex_shrink: 0.0, border: UiRect::bottom(px(1)), ..row(10.0) }, BorderColor::all(c(Tone::Dim, 0.25)))).with_children(|row| {
                row.spawn((number(fonts, &format!("{}", r + 1), 13.0, c(Tone::Dim, 1.0)), Node { width: px(18), flex_shrink: 0.0, ..default() }));
                row.spawn((tracked(fonts, &rung_name(content, rung), 7.5, c(Tone::Label, 1.0), 0.12), TextLayout::no_wrap(), Node { width: px(220), flex_shrink: 0.0, overflow: Overflow::clip(), ..default() }));
                row.spawn((Node { width: px(90), height: px(4), flex_shrink: 0.0, ..default() }, BackgroundColor(c(Tone::Dim, 0.35)))).with_child((
                    Node { width: percent(mean.unwrap_or(0.0) * 100.0), height: percent(100), ..default() },
                    BackgroundColor(c(Tone::Gold, 0.95)),
                ));
                let words = won.map_or_else(|| "—".to_string(), |w| format!("WON {w} OF {seeds}"));
                row.spawn((label(fonts, &words, 8.0, c(if won == Some(seeds) { Tone::Teal } else { Tone::Label }, 1.0)), Node { width: px(80), flex_shrink: 0.0, ..default() }));
                if let Some(j) = worst {
                    spawn_pill(row, fonts, "WATCH", 7.5, 20.0, None, Pill::new(Tone::Teal), (LabButton::Ladder(LadderButton::Watch(e.id.clone(), j)), Tip::new("ITS WORST FIGHT ON THIS RUNG")));
                }
            });
        }
    });
}
