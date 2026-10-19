//! The live rule panel in the flight HUD during TEST IN RANGE (GDR-0057: "the active rule is highlighted live";
//! written 13 Oct 2026): the tested behavior's name, its rules as the graph's chip labels (`Hull < 25%  ›  Flee`),
//! the rule that decides in teal, rules without a rule slot dimmed, and under each rule above it why it doesn't
//! match (the sim's `BehaviorState::describe`). Right side, under the target panel; hidden outside tests and with
//! the HUD (H).
//!
//! It follows `test_range::BehaviorWatch` (refreshed after each think of the tested ship, 10 Hz) and does nothing
//! in frames without a change: the rows are spawned when the behavior changes, and only their colours and why-lines
//! change in between.

use bevy::prelude::*;
use data::behavior::Program;
use data::behavior::edit::BehaviorEditor;
use data::{BehaviorNode, Table};
use sim::RuleStatus;
use kit::params::RenderToggles;
use kit::style::{Style, UiPalette, ui_color};
use kit::ui_kit::{HudFonts, Tone, label, prose, set_color, set_text};
use world::mode::FlightUi;
use world::sim_bridge::GameContent;
use world::test_range::BehaviorWatch;

use super::graph::{Chip, GraphView};

/// How a rule stands at the last think.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowState {
    /// It decides.
    Active,
    /// A condition is false (or its action has no target, or it is `scan`, which starts a scan and steps aside): the
    /// next rule gets its turn.
    Failed,
    /// Not evaluated: a rule above decides (or the ship hasn't thought yet).
    Waiting,
    /// No rule slot: it never runs.
    NoSlot,
}

/// One rule in the panel.
#[derive(Clone, Debug, PartialEq)]
pub struct LiveRow {
    /// The graph's chips as one line: `Shield down and Target on me  ›  Orbit 80% · ccw`.
    pub label: String,
    pub state: RowState,
    /// Why it doesn't match (rules above the one that decides).
    pub why: Option<String>,
}

/// The panel's content.
#[derive(Clone, Debug, PartialEq)]
pub struct LivePanel {
    /// `BEHAVIOR  ·  CAREFUL`.
    pub title: String,
    /// Which rule decides, or why none does.
    pub sub: String,
    pub rows: Vec<LiveRow>,
}

/// A chip as text: `not` and the name, its values, its comparison.
fn chip_text(c: &Chip) -> String {
    let mut s = String::new();
    if c.not {
        s.push_str("not ");
    }
    s.push_str(&c.name);
    for part in [&c.args, &c.compare] {
        if !part.is_empty() {
            s.push(' ');
            s.push_str(part);
        }
    }
    s
}

/// Every rule of a program as one line of the graph's chip labels.
pub fn rule_labels(program: &Program, nodes: &Table<BehaviorNode>) -> Vec<String> {
    // A flying program always opens (it passed the same checks); the editor only lends its chip labels.
    let Ok(editor) = BehaviorEditor::open(nodes, program) else { return Vec::new() };
    GraphView::of(&editor, u32::MAX)
        .rules
        .iter()
        .map(|r| {
            let when = if r.conds.is_empty() { "always".to_string() } else { r.conds.iter().map(chip_text).collect::<Vec<_>>().join(" and ") };
            format!("{when}  ›  {}", chip_text(&r.action))
        })
        .collect()
}

/// The panel for the running test, with the rules' `labels` ([`rule_labels`] of its program); `None` outside tests.
pub fn live_panel(watch: &BehaviorWatch, labels: &[String]) -> Option<LivePanel> {
    watch.mode?;
    if watch.program.rules.is_empty() {
        return None;
    }
    let rows = labels
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, label)| {
            let status = watch.rules.get(i).copied().filter(|_| watch.running).unwrap_or(RuleStatus::Skipped);
            let state = match status {
                RuleStatus::Active => RowState::Active,
                RuleStatus::Failed { .. } | RuleStatus::NoTarget | RuleStatus::Scan { .. } | RuleStatus::Flare { .. } => {
                    RowState::Failed
                }
                RuleStatus::Skipped => RowState::Waiting,
                RuleStatus::NoSlot => RowState::NoSlot,
            };
            let why = (state == RowState::Failed).then(|| watch.why.get(i).map(|w| w.replace('`', ""))).flatten();
            LiveRow { label, state, why }
        })
        .collect();
    let sub = match (watch.running, watch.active) {
        (true, Some(i)) => format!("RULE {} DECIDES", i + 1),
        (true, None) => "NO RULE MATCHES  ·  IT HOLDS".into(),
        (false, _) if watch.think.is_some() => "NOT RUNNING".into(),
        (false, _) => "STARTING".into(),
    };
    Some(LivePanel { title: format!("BEHAVIOR  ·  {}", watch.name.to_uppercase()), sub, rows })
}

// ---------------------------------------------------------------------------------------------------------
// The nodes

#[derive(Component)]
pub(super) struct LiveRoot;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum LiveText {
    Title,
    Sub,
}

/// The rows' container (rebuilt when the behavior changes).
#[derive(Component)]
pub(super) struct LiveRows;

/// Row `i`'s parts.
#[derive(Component)]
pub(super) struct RowBox(usize);
#[derive(Component)]
pub(super) struct RowDot(usize);
#[derive(Component)]
pub(super) struct RowLabel(usize);
#[derive(Component)]
pub(super) struct RowWhy(usize);

pub(super) fn spawn(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>) {
    let c = |tone: Tone, a: f32| ui_color(tone.color(&style.ui), a);
    commands
        .spawn((
            LiveRoot,
            FlightUi,
            Visibility::Hidden,
            Node {
                position_type: PositionType::Absolute,
                right: px(28),
                top: px(122),
                width: px(340),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: px(4),
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn((LiveText::Title, label(&fonts, "", 11.0, c(Tone::Label, 1.0))));
            p.spawn((LiveText::Sub, label(&fonts, "", 9.0, c(Tone::Dim, 1.0)), Node { margin: UiRect::bottom(px(6)), ..default() }));
            p.spawn((LiveRows, Node { flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd, row_gap: px(3), ..default() }));
        });
}

/// Follows [`BehaviorWatch`]: shown during a test (with the HUD on), rows spawned per behavior, colours and why-lines
/// per think.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn update(
    mut commands: Commands,
    watch: Res<BehaviorWatch>,
    toggles: Res<RenderToggles>,
    content: Res<GameContent>,
    fonts: Res<HudFonts>,
    style: Res<Style>,
    root: Single<&mut Visibility, With<LiveRoot>>,
    rows_node: Single<Entity, With<LiveRows>>,
    mut texts: Query<(&LiveText, &mut Text, &mut TextColor), (Without<RowLabel>, Without<RowWhy>)>,
    mut boxes: Query<(&RowBox, &mut BackgroundColor, &mut BorderColor), Without<RowDot>>,
    mut dots: Query<(&RowDot, &mut BackgroundColor), Without<RowBox>>,
    mut labels: Query<(&RowLabel, &mut TextColor), (Without<RowWhy>, Without<LiveText>)>,
    mut whys: Query<(&RowWhy, &mut Text, &mut TextColor, &mut Node), (Without<RowLabel>, Without<LiveText>)>,
    mut built: Local<Option<(Option<String>, Vec<String>)>>,
    mut rule_text: Local<(Program, Vec<String>)>,
    mut hud_was: Local<bool>,
) {
    let hud_changed = *hud_was != toggles.hud;
    *hud_was = toggles.hud;
    // Nothing new (the watch is reset when flight ends, so a new flight always gets here once).
    if !watch.is_changed() && !hud_changed {
        return;
    }
    // The rules' labels change only with the program (a think changes only the states).
    if rule_text.0 != watch.program {
        *rule_text = (watch.program.clone(), rule_labels(&watch.program, &content.0.behavior_nodes));
    }
    // FlightUi roots are hidden when flight ends; this one shows itself again while it runs.
    let panel = live_panel(&watch, &rule_text.1);
    let want = if panel.is_some() && toggles.hud { Visibility::Inherited } else { Visibility::Hidden };
    let mut root = root.into_inner();
    if *root != want {
        *root = want;
    }
    let Some(panel) = panel else {
        *built = None;
        return;
    };
    let ui = &style.ui;
    let key = (watch.behavior.clone(), panel.rows.iter().map(|r| r.label.clone()).collect::<Vec<_>>());
    let rebuild = built.as_ref() != Some(&key);
    if rebuild {
        *built = Some(key);
        commands.entity(*rows_node).despawn_children().with_children(|rows| {
            for (i, row) in panel.rows.iter().enumerate() {
                let (fill, edge) = box_colors(ui, row.state);
                let why = if row.why.is_some() { Display::Flex } else { Display::None };
                rows.spawn((
                    RowBox(i),
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::FlexEnd,
                        padding: UiRect::axes(px(11), px(4)),
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(12)),
                        row_gap: px(2),
                        ..default()
                    },
                    BackgroundColor(fill),
                    BorderColor::all(edge),
                ))
                .with_children(|b| {
                    b.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(8), ..default() }).with_children(|line| {
                        line.spawn((RowLabel(i), prose(&fonts, &row.label, 12.5, row_color(ui, row.state, false))));
                        let dot = Node { width: px(6), height: px(6), border_radius: BorderRadius::MAX, flex_shrink: 0.0, ..default() };
                        line.spawn((RowDot(i), dot, BackgroundColor(row_color(ui, row.state, true))));
                    });
                    let text = row.why.as_deref().unwrap_or("");
                    b.spawn((RowWhy(i), prose(&fonts, text, 11.0, ui_color(ui.label, 1.0)), Node { display: why, margin: UiRect::right(px(14)), ..default() }));
                });
            }
        });
    }
    for (t, mut text, mut color) in &mut texts {
        let (s, tone) = match t {
            LiveText::Title => (&panel.title, Tone::Label),
            LiveText::Sub => (&panel.sub, if watch.active.is_some() { Tone::Teal } else { Tone::Dim }),
        };
        set_text(&mut text, s);
        set_color(&mut color, ui_color(tone.color(ui), 1.0));
    }
    // Rows spawned this frame are not in the queries yet; they were spawned in their colours.
    for (b, mut bg, mut border) in &mut boxes {
        let Some(row) = panel.rows.get(b.0) else { continue };
        let (fill, edge) = box_colors(ui, row.state);
        if bg.0 != fill {
            bg.0 = fill;
        }
        if border.top != edge {
            *border = BorderColor::all(edge);
        }
    }
    for (d, mut bg) in &mut dots {
        let Some(row) = panel.rows.get(d.0) else { continue };
        let c = row_color(ui, row.state, true);
        if bg.0 != c {
            bg.0 = c;
        }
    }
    for (l, mut color) in &mut labels {
        if let Some(row) = panel.rows.get(l.0) {
            set_color(&mut color, row_color(ui, row.state, false));
        }
    }
    for (w, mut text, mut color, mut node) in &mut whys {
        let Some(row) = panel.rows.get(w.0) else { continue };
        let display = if row.why.is_some() { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        set_text(&mut text, row.why.as_deref().unwrap_or(""));
        set_color(&mut color, ui_color(ui.label, 1.0));
    }
}

/// A row's fill and edge: the rule that decides glows teal.
fn box_colors(ui: &UiPalette, state: RowState) -> (Color, Color) {
    match state {
        RowState::Active => (ui_color(ui.teal, 0.1), ui_color(ui.teal, 0.6)),
        _ => (Color::NONE, Color::NONE),
    }
}

/// A row's text (or dot) colour from its state.
fn row_color(ui: &UiPalette, state: RowState, dot: bool) -> Color {
    match state {
        RowState::Active => ui_color(if dot { ui.teal } else { ui.number }, 1.0),
        RowState::Failed => ui_color(if dot { ui.dim } else { ui.label }, 1.0),
        RowState::Waiting => ui_color(ui.dim, if dot { 0.6 } else { 1.0 }),
        RowState::NoSlot => ui_color(ui.dim, 0.4),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::Why;

    fn watch(content: &data::Content, id: &str) -> BehaviorWatch {
        let b = content.behaviors.get(id).unwrap();
        BehaviorWatch {
            mode: Some(world::test_range::TestMode::Watch),
            running: true,
            behavior: Some(id.into()),
            name: b.name.clone(),
            program: b.program().unwrap(),
            think: Some(30),
            slots: 12,
            ..default()
        }
    }

    #[test]
    fn rows_name_the_rules_like_the_graph_and_mark_the_one_that_decides() {
        let content = data::base_content();
        let nodes = &content.behavior_nodes;
        let mut w = watch(&content, "skirmisher");
        let labels = rule_labels(&w.program, nodes);
        assert_eq!(labels[0], "Hull < 25%  ›  Flee");
        assert_eq!(labels[1], "Shield down and Target on me and Missiles incoming < 1  ›  Orbit 80% · ccw");
        assert_eq!(labels[3], "always  ›  Approach 67%");
        // Rule 2 decides: the one above says why not, the one below waits; the last has no slot.
        let failed = RuleStatus::Failed { cond: 0, why: Why::False { value: Some(0.96) } };
        w.active = Some(1);
        w.rules = vec![failed, RuleStatus::Active, RuleStatus::Skipped, RuleStatus::NoSlot];
        w.why = vec!["`hull < 25%` is false (hull is 96%)".into(), "active: orbit(80%, ccw)".into(), String::new(), "inactive".into()];
        let p = live_panel(&w, &labels).unwrap();
        assert_eq!((p.title.as_str(), p.sub.as_str()), ("BEHAVIOR  ·  SKIRMISHER", "RULE 2 DECIDES"));
        let states: Vec<RowState> = p.rows.iter().map(|r| r.state).collect();
        assert_eq!(states, [RowState::Failed, RowState::Active, RowState::Waiting, RowState::NoSlot]);
        assert_eq!(p.rows[0].why.as_deref(), Some("hull < 25% is false (hull is 96%)"));
        assert_eq!(p.rows[1].why, None);
        // No rule matches: it holds. Not running yet: every rule waits. Outside a test: no panel.
        w.active = None;
        assert_eq!(live_panel(&w, &labels).unwrap().sub, "NO RULE MATCHES  ·  IT HOLDS");
        w.running = false;
        w.think = None;
        let p = live_panel(&w, &labels).unwrap();
        assert_eq!(p.sub, "STARTING");
        assert!(p.rows.iter().all(|r| r.state == RowState::Waiting && r.why.is_none()));
        w.mode = None;
        assert!(live_panel(&w, &labels).is_none());
    }
}
