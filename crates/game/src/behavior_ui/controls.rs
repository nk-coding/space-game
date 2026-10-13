//! The inspector's controls as nodes, from [`Inspection`] (`inspect.rs`): numbers with −/+ ring steppers and a
//! slider (a bar when read-only), the metres ↔ % pills and RESET; choices as pills; the `not` switch; button rows;
//! hints; note fields. Every button sends its [`BehaviorAction`]; a disabled one (no action) is dimmed.
//!
//! * **Sliders** ([`Slider`]) are dragged here ([`drag_sliders`]): a press on the track sets the value under the
//!   pointer, dragging follows it, snapped to the step there; the whole drag is one undo step (a stroke of the
//!   editing model). The inspector is not rebuilt while a slider is held ([`SliderDrag`]), so the slider keeps
//!   its node; its fill, knob and value text follow in place.
//! * **Note fields** ([`NoteField`]) are `ui_kit` text fields; the note is written when the field lets go of the
//!   keyboard (Enter, Esc or a click elsewhere), one undo step ([`commit_notes`]).
//! * [`DesignReach`]: the design's weapon reach in metres, measured when the screen opens, for the metres ↔ %
//!   switch and the `≈ 100 M` hints.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;

use crate::drydock::PlayerDesign;
use crate::sim_bridge::GameContent;
use crate::tuning::Tuning;
use crate::ui_kit::{
    Hot, Pill, PillText, Ring, TextField, TextFieldText, Tone, ValueBar, glow, number, prose, ring_node, spawn_bar, spawn_pill, tracked,
};

use super::actions::{BehaviorAction, BehaviorButton};
use super::draft::BehaviorDraft;
use super::edit::{Edit, EditState, apply_edit};
use super::inspect::{Button, ChoiceRow, Control, Inspection, NoteTarget, NumberRow, SliderSpec, SwitchRow};
use super::screen::{Kit, role_color};
use crate::style::ui_color;

/// Logical px.
const LINE_W: f32 = 1.2;
/// Space above each block of the inspector.
const GAP: f32 = 12.0;
const STEPPER: f32 = 22.0;
const SLIDER_H: f32 = 18.0;
const KNOB: f32 = 12.0;
/// The longest note the field takes (a line holds 240 characters with its rule).
const NOTE_CHARS: usize = 100;

/// The design's weapon reach in metres (its longest clear gun on the main piece), when it has one.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct DesignReach(pub Option<f32>);

/// The slider being dragged (its node), if any.
#[derive(Resource, Default, Debug)]
pub struct SliderDrag(pub Option<Entity>);

/// A slider: what it edits and shows, its fill and knob, the value text it keeps up to date.
#[derive(Component, Clone, Debug)]
pub(super) struct Slider {
    spec: SliderSpec,
    fill: Entity,
    knob: Entity,
    text: Entity,
}

/// A note field: whose note it edits, the note it started with, and whether it had the keyboard last frame.
#[derive(Component, Clone, Debug)]
pub(super) struct NoteField {
    target: NoteTarget,
    note: String,
    focused: bool,
}

/// Measures the design's weapon reach when the screen opens (with the tuning's weapon range, as flights use).
pub(super) fn measure_reach(design: Res<PlayerDesign>, content: Res<GameContent>, tuning: Res<Tuning>, mut reach: ResMut<DesignReach>) {
    let scaled = tuning.scaled_content(&content.0, true);
    let r = sim::design_report(&scaled, design.blueprint(), &tuning.config()).ok().and_then(|r| {
        let main = r.main?;
        let longest = r.guns.iter().filter(|g| g.piece == main && g.clear).map(|g| g.range).fold(0.0, f32::max);
        (longest > 0.0).then_some(longest)
    });
    if reach.0 != r {
        reach.0 = r;
    }
}

// ---------------------------------------------------------------------------------------------------------
// Spawning

/// The inspector: kicker, name, summary, the controls, a Rhai script.
pub(super) fn spawn_inspection(p: &mut ChildSpawnerCommands, kit: &Kit, i: &Inspection) {
    let name_color = match i.role {
        Some(role) => role_color(&kit.ui, role, i.rhai),
        None => kit.ui.number,
    };
    p.spawn(tracked(kit.fonts, &i.kicker, 9.0, kit.c(Tone::Label, 1.0), 0.32));
    p.spawn((prose(kit.fonts, &i.name, 25.0, ui_color(name_color, 1.0)), Node { margin: UiRect::top(px(5)), ..default() }));
    if !i.summary.is_empty() {
        p.spawn((prose(kit.fonts, &i.summary, 11.5, kit.c(Tone::Label, 1.0)), Node { margin: UiRect::top(px(6)), ..default() }));
    }
    for c in &i.controls {
        match c {
            Control::Number(n) => spawn_number(p, kit, n),
            Control::Choice(ch) => spawn_choice(p, kit, ch),
            Control::Switch(s) => spawn_switch(p, kit, s),
            Control::Buttons { label, buttons } => {
                p.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(7), margin: UiRect::top(px(GAP)), ..default() }).with_children(|col| {
                    if !label.is_empty() {
                        col.spawn(tracked(kit.fonts, label, 8.5, kit.c(Tone::Label, 1.0), 0.3));
                    }
                    col.spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: px(5), row_gap: px(5), ..default() })
                        .with_children(|row| {
                            for b in buttons {
                                spawn_button(row, kit, b, 24.0);
                            }
                        });
                });
            }
            Control::Note { target, text } => spawn_note(p, kit, *target, text),
            Control::Hint(h) => {
                p.spawn((tracked(kit.fonts, h, 7.5, kit.c(Tone::Dim, 1.0), 0.25), Node { margin: UiRect::top(px(9)), ..default() }));
            }
        }
    }
    if let Some(script) = &i.script {
        p.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(5), margin: UiRect::top(px(GAP)), ..default() }).with_children(|n| {
            n.spawn(tracked(kit.fonts, "RHAI SCRIPT", 8.5, kit.c(Tone::Gold, 1.0), 0.3));
            n.spawn(prose(kit.fonts, script, 11.0, kit.c(Tone::Gold, 0.7)));
        });
    }
}

/// A pill button: its action when it has one, else dimmed (or just marked, when it is the active one).
fn spawn_button(p: &mut ChildSpawnerCommands, kit: &Kit, b: &Button, height: f32) {
    let pill = Pill { active: b.active, disabled: b.action.is_none() && !b.active, ..Pill::new(b.tone) };
    match &b.action {
        Some(a) => spawn_pill(p, kit.fonts, &b.text, 8.0, height, None, pill, BehaviorButton(a.clone())),
        None => spawn_pill(p, kit.fonts, &b.text, 8.0, height, None, pill, ()),
    };
}

/// A small ring with `−` or `+`; dimmed without an action.
fn spawn_stepper(p: &mut ChildSpawnerCommands, kit: &Kit, text: &str, action: &Option<BehaviorAction>) {
    let mut e = p.spawn(ring_node(STEPPER));
    match action {
        Some(a) => e.insert(BehaviorButton(a.clone())),
        None => e.insert(Ring { disabled: true }),
    };
    e.with_children(|r| {
        r.spawn((PillText, number(kit.fonts, text, 13.0, kit.c(Tone::Number, 1.0))));
    });
}

/// A label with DEFAULT after it when the rule leaves the value out.
fn spawn_label(p: &mut ChildSpawnerCommands, kit: &Kit, label: &str, default: bool) {
    p.spawn(tracked(kit.fonts, label, 8.5, kit.c(Tone::Label, 1.0), 0.3));
    if default {
        p.spawn(tracked(kit.fonts, "DEFAULT", 7.0, kit.c(Tone::Dim, 1.0), 0.3));
    }
}

fn spawn_number(p: &mut ChildSpawnerCommands, kit: &Kit, n: &NumberRow) {
    p.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(6), margin: UiRect::top(px(GAP)), ..default() }).with_children(|col| {
        let mut text = Entity::PLACEHOLDER;
        col.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Baseline, justify_content: JustifyContent::SpaceBetween, column_gap: px(8), ..default() })
            .with_children(|h| {
                h.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Baseline, column_gap: px(8), ..default() }).with_children(|l| {
                    l.spawn(tracked(kit.fonts, &format!("{}  ·", n.label), 8.5, kit.c(Tone::Label, 1.0), 0.3));
                    text = l.spawn(tracked(kit.fonts, &n.value.to_uppercase(), 11.0, kit.c(Tone::Number, 1.0), 0.12)).id();
                    if let Some(hint) = &n.hint {
                        l.spawn(tracked(kit.fonts, hint, 7.5, kit.c(Tone::Dim, 1.0), 0.2));
                    }
                    if n.default {
                        l.spawn(tracked(kit.fonts, "DEFAULT", 7.0, kit.c(Tone::Dim, 1.0), 0.3));
                    }
                });
                h.spawn(tracked(kit.fonts, &n.range, 7.0, kit.c(Tone::Dim, 1.0), 0.25));
            });
        let Some(spec) = n.slider else { return };
        if spec.target.is_none() {
            // Read-only (a palette node's default): where the value sits, as a bar.
            spawn_bar(col, ValueBar { share: spec.scale.share(spec.value), tone: Tone::Teal }, ());
        } else {
            col.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(10), ..default() }).with_children(|r| {
                spawn_stepper(r, kit, "−", &n.minus);
                spawn_slider(r, kit, spec, text);
                spawn_stepper(r, kit, "+", &n.plus);
            });
        }
        if !n.units.is_empty() || n.reset.is_some() {
            col.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, ..default() })
                .with_children(|r| {
                    r.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(4), ..default() }).with_children(|u| {
                        for b in &n.units {
                            spawn_button(u, kit, b, 20.0);
                        }
                    });
                    if let Some(reset) = &n.reset {
                        spawn_pill(r, kit.fonts, "RESET", 7.5, 20.0, None, Pill::new(Tone::Teal), BehaviorButton(reset.clone()));
                    }
                });
        }
    });
}

/// The track, its fill and the knob; the whole node takes the pointer.
fn spawn_slider(p: &mut ChildSpawnerCommands, kit: &Kit, spec: SliderSpec, text: Entity) {
    let share = spec.scale.share(spec.value);
    let mut slider = p.spawn((Hot::default(), Node { flex_grow: 1.0, height: px(SLIDER_H), ..default() }));
    let (mut fill, mut knob) = (Entity::PLACEHOLDER, Entity::PLACEHOLDER);
    let mid = SLIDER_H / 2.0 - 1.0;
    slider.with_children(|s| {
        s.spawn((
            Node { position_type: PositionType::Absolute, left: px(0), right: px(0), top: px(mid), height: px(2), ..default() },
            BackgroundColor(kit.c(Tone::Dim, 0.45)),
        ));
        fill = s
            .spawn((
                Node { position_type: PositionType::Absolute, left: px(0), width: percent(share * 100.0), top: px(mid), height: px(2), ..default() },
                BackgroundColor(kit.c(Tone::Teal, 0.9)),
            ))
            .id();
        knob = s
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(share * 100.0),
                    margin: UiRect::left(px(-KNOB / 2.0)),
                    top: px((SLIDER_H - KNOB) / 2.0),
                    width: px(KNOB),
                    height: px(KNOB),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(kit.c(Tone::Teal, 1.0)),
                glow(kit.ui.teal, 0.4),
            ))
            .id();
    });
    slider.insert(Slider { spec, fill, knob, text });
}

fn spawn_choice(p: &mut ChildSpawnerCommands, kit: &Kit, ch: &ChoiceRow) {
    p.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(7), margin: UiRect::top(px(GAP)), ..default() }).with_children(|col| {
        col.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, ..default() })
            .with_children(|h| {
                h.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Baseline, column_gap: px(8), ..default() })
                    .with_children(|l| spawn_label(l, kit, &ch.label, ch.default));
                if let Some(reset) = &ch.reset {
                    spawn_pill(h, kit.fonts, "RESET", 7.5, 20.0, None, Pill::new(Tone::Teal), BehaviorButton(reset.clone()));
                }
            });
        col.spawn(Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: px(5), row_gap: px(5), ..default() }).with_children(|o| {
            for b in &ch.options {
                spawn_button(o, kit, b, 24.0);
            }
        });
    });
}

/// `NOT` with its hint and a switch (a pill track with a knob, teal when on), as in the mockup.
fn spawn_switch(p: &mut ChildSpawnerCommands, kit: &Kit, s: &SwitchRow) {
    p.spawn(Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::SpaceBetween,
        margin: UiRect::top(px(GAP)),
        ..default()
    })
    .with_children(|r| {
        r.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() }).with_children(|l| {
            l.spawn(tracked(kit.fonts, &s.label, 8.5, kit.c(if s.on { Tone::Violet } else { Tone::Label }, 1.0), 0.3));
            l.spawn(tracked(kit.fonts, &s.hint, 7.0, kit.c(Tone::Dim, 1.0), 0.25));
        });
        let (edge, fill, dot) = if s.on {
            (kit.c(Tone::Teal, 0.9), kit.c(Tone::Teal, 0.22), kit.c(Tone::Teal, 1.0))
        } else {
            (kit.c(Tone::Label, 0.45), Color::NONE, kit.c(Tone::Label, 0.9))
        };
        let mut e = r.spawn((
            Hot::default(),
            Node { width: px(38), height: px(20), border: UiRect::all(px(LINE_W)), border_radius: BorderRadius::MAX, ..default() },
            BorderColor::all(edge),
            BackgroundColor(fill),
        ));
        if let Some(a) = &s.action {
            e.insert(BehaviorButton(a.clone()));
        }
        e.with_children(|t| {
            let at = if s.on { 20.0 } else { 3.0 };
            t.spawn((
                Node { position_type: PositionType::Absolute, left: px(at), top: px(2.8), width: px(12), height: px(12), border_radius: BorderRadius::MAX, ..default() },
                BackgroundColor(dot),
            ));
        });
    });
}

/// NOTE and a one-line field.
fn spawn_note(p: &mut ChildSpawnerCommands, kit: &Kit, target: NoteTarget, text: &str) {
    p.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(6), margin: UiRect::top(px(GAP)), ..default() }).with_children(|col| {
        col.spawn(tracked(kit.fonts, if target == NoteTarget::Target { "NOTE ON THE TARGET LINE" } else { "NOTE" }, 8.5, kit.c(Tone::Label, 1.0), 0.3));
        col.spawn((
            NoteField { target, note: text.to_string(), focused: false },
            TextField { value: text.to_string(), placeholder: "Add a note".into(), max_chars: NOTE_CHARS, ..default() },
            Hot::default(),
            Node {
                min_height: px(28),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::axes(px(12), px(5)),
                border: UiRect::all(px(LINE_W)),
                border_radius: BorderRadius::all(px(14)),
                ..default()
            },
            BorderColor::all(kit.c(Tone::Label, 0.3)),
        ))
        .with_children(|f| {
            f.spawn((TextFieldText, prose(kit.fonts, "", 12.0, kit.c(Tone::Dim, 1.0)), Node { flex_grow: 1.0, ..default() }));
        });
    });
}

// ---------------------------------------------------------------------------------------------------------
// Input

/// A note field that lets go of the keyboard writes its note (if it changed) as one edit.
pub(super) fn commit_notes(mut fields: Query<(&mut NoteField, &TextField)>, mut out: MessageWriter<BehaviorAction>) {
    for (mut nf, tf) in &mut fields {
        // A written note changes the program, so the inspector is rebuilt with it.
        if nf.focused && !tf.focused && tf.value.trim() != nf.note.trim() {
            let note = tf.value.trim().to_string();
            out.write(BehaviorAction::Edit(match nf.target {
                NoteTarget::Rule(rule) => Edit::SetRuleNote { rule, note },
                NoteTarget::Target => Edit::SetTargetNote(note),
            }));
        }
        if nf.focused != tf.focused {
            nf.focused = tf.focused;
        }
    }
}

/// The held slider follows the pointer (one undo step per drag); its fill, knob and value text follow in place.
#[allow(clippy::too_many_arguments)]
pub(super) fn drag_sliders(
    window: Single<&Window, With<PrimaryWindow>>,
    touches: Res<Touches>,
    time: Res<Time<Real>>,
    mut drag: ResMut<SliderDrag>,
    mut draft: ResMut<BehaviorDraft>,
    mut state: ResMut<EditState>,
    mut sliders: Query<(Entity, &mut Slider, &Hot, &ComputedNode, &UiGlobalTransform)>,
    mut parts: Query<&mut Node, Without<Slider>>,
    mut texts: Query<&mut Text>,
) {
    let held = sliders.iter_mut().find(|(_, s, hot, ..)| hot.pressed && s.spec.target.is_some());
    let Some((e, mut slider, _, node, tf)) = held else {
        if drag.0.take().is_some() {
            draft.edit(|ed| ed.end_stroke());
        }
        return;
    };
    if drag.0 != Some(e) {
        drag.0 = Some(e);
        draft.edit(|ed| ed.begin_stroke());
    }
    let scale = window.scale_factor();
    let Some(at) = window.physical_cursor_position().or_else(|| touches.first_pressed_position().map(|p| p * scale)) else { return };
    let left = tf.translation.x - node.size.x / 2.0;
    let share = ((at.x - left) / node.size.x.max(1.0)).clamp(0.0, 1.0);
    let spec = slider.spec;
    let x = spec.scale.at(share);
    let Some(target) = spec.target.filter(|_| x != spec.value) else { return };
    match draft.edit(|ed| apply_edit(ed, &target.edit(spec.scale.kind, x))) {
        Some(Ok(_)) => {
            slider.spec.value = x;
            let w = percent(spec.scale.share(x) * 100.0);
            if let Ok(mut n) = parts.get_mut(slider.fill) {
                n.width = w;
            }
            if let Ok(mut n) = parts.get_mut(slider.knob) {
                n.left = w;
            }
            if let Ok(mut t) = texts.get_mut(slider.text) {
                t.0 = spec.scale.kind.value(x).to_string().to_uppercase();
            }
        }
        Some(Err(why)) => state.note = Some((why.to_string(), time.elapsed_secs())),
        None => {}
    }
}
