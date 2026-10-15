//! The TEXT view: the behavior, or on the PILOT ASSISTS tab the design's assists, as editable text
//! (`docs/behavior-format.md`), one line per rule, with live errors.
//!
//! bevy_ui has no text input, so the editor is an egui multi-line `TextEdit` (egui is already in the build for
//! the tuning panel), placed over the graph area's node and styled to the look: Barlow, no frame or box, the
//! names coloured like the graph (conditions violet, actions teal, Rhai gold, keywords grey), dim line numbers
//! in a gutter, and every problem printed in coral after its line with the column underlined. Each change is
//! parsed and checked at once ([`BehaviorDraft::set_text`]): a valid text becomes the program (the graph
//! follows), an invalid one keeps the last valid program.
//!
//! Colours: egui's shader turns its sRGB colours into linear ones, but the main pass composites in sRGB
//! (`main.rs`), so [`egui_color`] hands egui the colours pre-encoded (as `ui_color` does for bevy_ui).

use std::ops::Range;

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, egui};
use data::behavior::{KEYWORDS, ListKind, Program};
use data::{BehaviorNode, NodeRole, Table};
use kit::clipboard::Clipboard;
use kit::style::Style;
use world::sim_bridge::GameContent;

use super::draft::BehaviorDraft;
use super::graph::{always_decides, never_reached};
use super::{BehaviorUi, BehaviorView, EditorContext};

/// The egui font family of the editor (Barlow, egui's own fonts as fallback for arrows and the like).
const FAMILY: &str = "barlow";
/// Text size at UI scale 1 (logical px).
const TEXT_SIZE: f32 = 16.0;
const ERROR_SIZE: f32 = 12.5;
/// Width of the line number gutter at UI scale 1.
const GUTTER: f32 = 44.0;
/// Error messages wrap at this width (UI scale 1), or narrower after a long line, down to the minimum.
const MESSAGE_WIDTH: f32 = 420.0;
const MIN_MESSAGE_WIDTH: f32 = 200.0;
/// The editor's egui id (its state, and the undo history in it, is kept under it).
const EDITOR_ID: &str = "behavior_text";

/// The node the editor covers (the graph area's rectangle).
#[derive(Component)]
pub struct TextViewport;

/// What a piece of a line is, for colouring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Span {
    /// `when`, `and`, `not`, `->`, comparisons ...
    Keyword,
    Condition,
    /// A condition written in Rhai.
    Rhai,
    Action,
    Target,
    /// Numbers with their units, and option words inside brackets.
    Value,
    /// `# note` to the end of the line.
    Note,
    /// Spaces, brackets, commas and words the vocabulary does not know.
    Plain,
}

/// Splits one line into coloured pieces (byte ranges covering the whole line, in order). Works on any text,
/// valid or not: names are looked up in the vocabulary, words inside brackets are values.
pub fn highlight(line: &str, nodes: &Table<BehaviorNode>) -> Vec<(Range<usize>, Span)> {
    let mut out: Vec<(Range<usize>, Span)> = Vec::new();
    let mut push = |r: Range<usize>, s: Span| match out.last_mut() {
        Some((last, ls)) if *ls == s && last.end == r.start => last.end = r.end,
        _ => out.push((r, s)),
    };
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let end_of = |k: usize| chars.get(k).map_or(line.len(), |c| c.0);
    let mut depth = 0u32;
    let mut k = 0;
    while k < chars.len() {
        let (at, c) = chars[k];
        if c == '#' {
            push(at..line.len(), Span::Note);
            break;
        }
        let start = k;
        let span = if c.is_alphabetic() || c == '_' {
            while k < chars.len() && (chars[k].1.is_alphanumeric() || chars[k].1 == '_') {
                k += 1;
            }
            let word = line[at..end_of(k)].to_lowercase();
            if KEYWORDS.contains(&word.as_str()) {
                Span::Keyword
            } else if depth > 0 {
                Span::Value
            } else {
                match nodes.get(&word) {
                    Some(n) if n.role == NodeRole::Condition && n.script.is_some() => Span::Rhai,
                    Some(n) if n.role == NodeRole::Condition => Span::Condition,
                    Some(n) if n.role == NodeRole::Action => Span::Action,
                    Some(_) => Span::Target,
                    None => Span::Plain,
                }
            }
        } else if c.is_ascii_digit() || (c == '.' && chars.get(k + 1).is_some_and(|n| n.1.is_ascii_digit())) {
            while k < chars.len() && (chars[k].1.is_ascii_digit() || chars[k].1 == '.') {
                k += 1;
            }
            // The unit, if one follows directly (`45m`, `5s`, `30%`).
            if k < chars.len() && matches!(chars[k].1, 'm' | 's' | '%') && !chars.get(k + 1).is_some_and(|n| n.1.is_alphanumeric()) {
                k += 1;
            }
            Span::Value
        } else if c == '-' && chars.get(k + 1).is_some_and(|n| n.1 == '>') {
            k += 2;
            Span::Keyword
        } else if matches!(c, '<' | '>' | '≤' | '≥' | '→' | '=') {
            k += 1;
            while k < chars.len() && chars[k].1 == '=' {
                k += 1;
            }
            Span::Keyword
        } else {
            match c {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                _ => {}
            }
            k += 1;
            Span::Plain
        };
        debug_assert!(k > start);
        push(at..end_of(k), span);
    }
    out
}

/// A UI colour for egui, pre-encoded so it shows exactly like the same colour in bevy_ui (see the module doc).
pub fn egui_color(c: Color, alpha: f32) -> egui::Color32 {
    let s = c.to_srgba();
    egui::Color32::from(egui::Rgba::from_rgba_unmultiplied(s.red, s.green, s.blue, s.alpha * alpha))
}

fn span_color(s: Span, style: &Style) -> egui::Color32 {
    let ui = &style.ui;
    match s {
        Span::Keyword => egui_color(ui.label, 1.0),
        Span::Condition => egui_color(ui.violet, 1.0),
        Span::Rhai => egui_color(ui.gold, 1.0),
        Span::Action => egui_color(ui.teal, 1.0),
        Span::Target | Span::Value => egui_color(ui.number, 1.0),
        Span::Note => egui_color(ui.dim, 1.0),
        Span::Plain => egui_color(ui.number, 0.8),
    }
}

/// Barlow for the editor, once.
fn install_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(FAMILY.into(), std::sync::Arc::new(egui::FontData::from_static(kit::ui_kit::BARLOW_REGULAR)));
    let mut family = vec![FAMILY.to_string()];
    family.extend(fonts.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default());
    fonts.families.insert(egui::FontFamily::Name(FAMILY.into()), family);
    ctx.set_fonts(fonts);
}

/// egui keeps the editor's undo history across frames under its id, whatever the text is now. When the text
/// changed outside the editor (an edit in the graph or the inspector, another draft opened, tidying), that history
/// belongs to another text: Ctrl+Z would bring back another behavior's text. It is cleared then; `seen` is the text
/// the editor last showed. True when it was cleared.
fn forget_outside_history(ctx: &egui::Context, id: egui::Id, seen: &mut Option<String>, text: &str) -> bool {
    if seen.as_deref() == Some(text) {
        return false;
    }
    *seen = Some(text.to_string());
    let Some(mut state) = egui::widgets::text_edit::TextEditState::load(ctx, id) else { return false };
    state.clear_undoer();
    state.store(ctx, id);
    true
}

/// Char index of the start of each line of `text`.
fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (k, c) in text.chars().enumerate() {
        if c == '\n' {
            starts.push(k + 1);
        }
    }
    starts
}

/// The editor, while the TEXT view is open.
#[allow(clippy::too_many_arguments)]
pub(super) fn editor(
    mut contexts: EguiContexts,
    view: Res<BehaviorUi>,
    mut draft: ResMut<BehaviorDraft>,
    context: Res<EditorContext>,
    content: Res<GameContent>,
    style: Res<Style>,
    ui_scale: Res<UiScale>,
    window: Single<&Window, With<PrimaryWindow>>,
    viewport: Single<(&ComputedNode, &UiGlobalTransform), With<TextViewport>>,
    clear: Res<ClearColor>,
    mut clipboard: ResMut<Clipboard>,
    mut font_ready: Local<bool>,
    mut rule_lines: Local<(String, Vec<u32>, Vec<bool>)>,
    mut was_open: Local<bool>,
    mut seen: Local<Option<String>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    if !*font_ready {
        install_font(ctx);
        *font_ready = true;
        // The font is there from the next pass on.
        return Ok(());
    }
    let opened = view.view == BehaviorView::Text && !*was_open;
    *was_open = view.view == BehaviorView::Text;
    // Copy and paste go through the game's clipboard (`clipboard.rs`) while the editor has the keyboard.
    clipboard.offer = None;
    clipboard.accept = false;
    if view.view != BehaviorView::Text {
        return Ok(());
    }
    let (node, tf) = *viewport;
    let scale = window.scale_factor();
    let size = node.size / scale;
    if size.min_element() < 8.0 {
        return Ok(());
    }
    let centre = tf.translation / scale;
    let rect = egui::Rect::from_center_size(egui::pos2(centre.x, centre.y), egui::vec2(size.x, size.y));
    let k = ui_scale.0;
    let font = egui::FontId::new(TEXT_SIZE * k, egui::FontFamily::Name(FAMILY.into()));
    let small = egui::FontId::new(ERROR_SIZE * k, egui::FontFamily::Name(FAMILY.into()));
    let nodes = &content.0.behavior_nodes;
    let ui_c = &style.ui;

    // Which lines hold rules and which rules always decide (to mark those without a rule slot and those such an
    // `always` rule above never lets run, `graph::always_decides`), from the last valid text.
    if rule_lines.0 != draft.text {
        (rule_lines.1, rule_lines.2) = Program::parse_with_lines(&draft.text)
            .map(|(p, m)| (m.rules, always_decides(&p)))
            .unwrap_or_default();
        rule_lines.0.clone_from(&draft.text);
    }
    // Pilot assists need no slots and every one decides its own channel: nothing to mark there.
    let (no_slot, never) = if draft.editor().is_some_and(|e| e.kind() == ListKind::Assists) {
        (Vec::new(), Vec::new())
    } else {
        let no_slot: Vec<u32> = rule_lines.1.iter().skip(context.slots as usize).copied().collect();
        (no_slot, rule_lines.1.get(never_reached(&rule_lines.2, context.slots)).map_or_else(Vec::new, <[u32]>::to_vec))
    };

    let id = egui::Id::new(EDITOR_ID);
    forget_outside_history(ctx, id, &mut seen, &draft.text);
    let mut text = draft.text.clone();
    let mut changed = false;
    egui::Area::new(egui::Id::new("behavior_text_view")).fixed_pos(rect.min).order(egui::Order::Background).show(ctx, |ui| {
        ui.set_min_size(rect.size());
        ui.set_max_size(rect.size());
        let visuals = &mut ui.style_mut().visuals;
        visuals.selection.bg_fill = egui_color(ui_c.teal, 0.25);
        visuals.selection.stroke = egui::Stroke::new(1.0, egui_color(ui_c.teal, 1.0));
        visuals.text_cursor.stroke = egui::Stroke::new(2.0 * k, egui_color(ui_c.teal, 1.0));
        visuals.extreme_bg_color = egui::Color32::TRANSPARENT;
        egui::ScrollArea::both().auto_shrink([false, false]).max_width(rect.width()).max_height(rect.height()).show(ui, |ui| {
            let row_h = ui.ctx().fonts_mut(|f| f.row_height(&font));
            let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, _wrap: f32| {
                let mut job = egui::text::LayoutJob::default();
                job.wrap.max_width = f32::INFINITY;
                for line in buf.as_str().split_inclusive('\n') {
                    let body = line.strip_suffix('\n').unwrap_or(line);
                    for (r, s) in highlight(body, nodes) {
                        job.append(&body[r], 0.0, egui::TextFormat::simple(font.clone(), span_color(s, &style)));
                    }
                    if body.len() < line.len() {
                        job.append("\n", 0.0, egui::TextFormat::simple(font.clone(), egui::Color32::TRANSPARENT));
                    }
                }
                ui.ctx().fonts_mut(|f| f.layout_job(job))
            };
            let out = egui::TextEdit::multiline(&mut text)
                .id(id)
                // No box: the frame only keeps the gutter free (a frame replaces the `margin` setting).
                .frame(egui::Frame::NONE.inner_margin(egui::Margin { left: (GUTTER * k) as i8, right: 0, top: (8.0 * k) as i8, bottom: (8.0 * k) as i8 }))
                .desired_width(f32::INFINITY)
                .desired_rows(((rect.height() / row_h) as usize).saturating_sub(1).max(1))
                .lock_focus(true)
                .layouter(&mut layouter)
                .show(ui);
            changed = out.response.changed();
            // Switching to TEXT puts the caret into the text.
            if opened {
                out.response.request_focus();
            }
            // While it has the keyboard: Ctrl+C copies the selection, Ctrl+V pastes (egui inserts the text; the web
            // build needs the game's clipboard to let the keys through to the browser).
            if out.response.has_focus() {
                clipboard.accept = true;
                clipboard.offer = out.cursor_range.filter(|r| !r.is_empty()).map(|r| {
                    let range = r.as_sorted_char_range();
                    text.chars().skip(range.start.0).take(range.end.0 - range.start.0).collect()
                });
            }

            // The gutter, the errors after their lines, the rules without a slot.
            let painter = ui.painter();
            let starts = line_starts(&text);
            let dim = egui_color(ui_c.dim, 1.0);
            let coral = egui_color(ui_c.coral, 1.0);
            let mut ends = Vec::new();
            for (n, row) in out.galley.rows.iter().enumerate() {
                let r = row.rect().translate(out.galley_pos.to_vec2());
                let line = n as u32 + 1;
                let has_error = draft.errors.iter().any(|e| e.line == line);
                let color = if has_error { coral } else { dim };
                painter.text(egui::pos2(r.left() - 14.0 * k, r.center().y), egui::Align2::RIGHT_CENTER, line.to_string(), small.clone(), color);
                ends.push(r);
            }
            for e in &draft.errors {
                // The column: underline the word there (one character at least).
                if let Some(&start) = starts.get(e.line as usize - 1) {
                    let line: Vec<char> = text.chars().skip(start).take_while(|&c| c != '\n').collect();
                    let col = (e.col as usize).saturating_sub(1).min(line.len());
                    let len = line[col..].iter().take_while(|c| c.is_alphanumeric() || **c == '_').count().max(1);
                    let a = out.galley.pos_from_cursor(egui::text::CCursor::new(start + col));
                    let b = out.galley.pos_from_cursor(egui::text::CCursor::new(start + (col + len).min(line.len())));
                    let y = out.galley_pos.y + a.bottom() + 1.0 * k;
                    let (x0, x1) = (out.galley_pos.x + a.left(), (out.galley_pos.x + b.left()).max(out.galley_pos.x + a.left() + 6.0 * k));
                    painter.line_segment([egui::pos2(x0, y), egui::pos2(x1, y)], egui::Stroke::new(1.5 * k, coral));
                }
            }
            // The messages after their lines, wrapped to the room there; with too little room one moves left
            // over the line's end, on the backdrop's colour so it stays readable.
            let visible = ui.clip_rect();
            let veil = egui_color(clear.0, 0.92);
            for (n, r) in ends.iter().enumerate() {
                let line = n as u32 + 1;
                let msgs: Vec<String> = draft.errors.iter().filter(|e| e.line == line).map(|e| format!("col {}  ·  {}", e.col, e.message)).collect();
                let (msg, color) = if !msgs.is_empty() {
                    (msgs.join("   ·   "), coral)
                } else if no_slot.contains(&line) {
                    ("no rule slot: this rule does not run".to_string(), egui_color(ui_c.label, 1.0))
                } else if never.contains(&line) {
                    ("never reached: an always rule above matches first".to_string(), egui_color(ui_c.label, 1.0))
                } else {
                    continue;
                };
                let after = r.right() + 28.0 * k;
                let room = visible.right() - 8.0 * k - after;
                let wrap = if room >= MIN_MESSAGE_WIDTH * k { room.min(MESSAGE_WIDTH * k) } else { MESSAGE_WIDTH * k };
                let galley = painter.layout(msg, small.clone(), color, wrap);
                let size = galley.size();
                let x = after.min(visible.right() - size.x - 8.0 * k).max(r.left());
                let at = egui::pos2(x, r.center().y - galley.rows.first().map_or(size.y, |row| row.rect().height()) / 2.0);
                if x < after {
                    painter.rect_filled(egui::Rect::from_min_size(at, size).expand(4.0 * k), 4.0 * k, veil);
                }
                painter.galley(at, galley, color);
            }
        });
    });
    // egui gets pasted text itself; the game's copy of it is not needed (while the editor has the keyboard: else
    // it is a text field's).
    if clipboard.accept {
        clipboard.take_pasted();
    }
    if changed {
        *seen = Some(text.clone());
        draft.set_text(&text);
    }
    Ok(())
}

/// Leaving the screen: the editor stops offering its selection and taking pastes (it doesn't run elsewhere to
/// reset them, and the web clipboard would keep Ctrl+C / Ctrl+V from the next screen).
pub(super) fn leave(mut clipboard: ResMut<Clipboard>) {
    clipboard.offer = None;
    clipboard.accept = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(line: &str) -> Vec<(String, Span)> {
        let content = data::base_content();
        let out = highlight(line, &content.behavior_nodes);
        // The pieces cover the line in order.
        assert_eq!(out.iter().map(|(r, _)| &line[r.clone()]).collect::<String>(), line);
        out.into_iter().filter(|(_, s)| *s != Span::Plain).map(|(r, s)| (line[r].to_string(), s)).collect()
    }

    #[test]
    fn names_are_coloured_by_role() {
        use Span::*;
        assert_eq!(
            spans("when hull < 25% -> flee # too hurt"),
            [("when".into(), Keyword), ("hull".into(), Condition), ("<".into(), Keyword), ("25%".into(), Value), ("->".into(), Keyword), ("flee".into(), Action), ("# too hurt".into(), Note)]
        );
        assert_eq!(
            spans("when closing_in(2s) and not target_in_range(100%) -> face"),
            [
                ("when".into(), Keyword),
                ("closing_in".into(), Rhai),
                ("2s".into(), Value),
                ("and".into(), Keyword),
                ("not".into(), Keyword),
                ("target_in_range".into(), Condition),
                ("100%".into(), Value),
                ("->".into(), Keyword),
                ("face".into(), Action),
            ]
        );
        assert_eq!(
            spans("target weakest(engines)"),
            [("target".into(), Keyword), ("weakest".into(), Target), ("engines".into(), Value)]
        );
        assert_eq!(spans("always → orbit(80%, ccw)").iter().map(|s| s.1).collect::<Vec<_>>(), [Keyword, Keyword, Action, Value, Value]);
        assert_eq!(spans("If Hull≤ 30 % -> hold")[2], ("≤".into(), Keyword));
        assert_eq!(spans("when cycle(9s) >= 5s -> hold")[3], (">=".into(), Keyword));
    }

    #[test]
    fn broken_lines_still_split() {
        // Unknown words stay plain, a stray bracket does not break the rest.
        assert_eq!(spans("when sheild_down -> flee"), [("when".into(), Span::Keyword), ("->".into(), Span::Keyword), ("flee".into(), Span::Action)]);
        assert_eq!(spans("when orbit(45 -> flee").len(), 5);
        assert!(spans("").is_empty());
        assert_eq!(spans("é # ü"), [("# ü".into(), Span::Note)]);
    }

    /// Ctrl+Z in the editor never brings back the text of another behavior (or the text from before graph edits):
    /// the history is cleared when the text changed outside the editor.
    #[test]
    fn the_editors_undo_history_follows_the_text() {
        let ctx = egui::Context::default();
        let id = egui::Id::new(EDITOR_ID);
        let undo_point = |text: &str| (egui::text::CCursorRange::default(), text.to_string());
        let mut state = egui::widgets::text_edit::TextEditState::default();
        let mut undoer = state.undoer();
        undoer.add_undo(&undo_point("behavior A"));
        state.set_undoer(undoer);
        state.store(&ctx, id);
        let has_undo = |now: &str| egui::widgets::text_edit::TextEditState::load(&ctx, id).unwrap().undoer().has_undo(&undo_point(now));
        let mut seen = Some("behavior A, typed".to_string());
        // The editor shows what it typed: the history stays.
        assert!(!forget_outside_history(&ctx, id, &mut seen, "behavior A, typed"));
        assert!(has_undo("behavior A, typed"));
        // Another draft opened: the history goes.
        assert!(forget_outside_history(&ctx, id, &mut seen, "behavior B"));
        assert!(!has_undo("behavior B"));
        assert_eq!(seen.as_deref(), Some("behavior B"));
    }

    #[test]
    fn line_starts_count_chars() {
        assert_eq!(line_starts("ab\n≤c\n"), [0, 3, 6]);
        assert_eq!(line_starts(""), [0]);
    }
}
