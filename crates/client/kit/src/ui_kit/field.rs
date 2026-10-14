//! The one-line text field ([`TextField`]): typing, the caret and selection, copy and paste, renaming in place.

use super::*;

/// A name edited in place (the drydock's design, the BEHAVIOR screen's behavior): a click on the name opens the
/// field with it; when the field lets go of the keyboard, Esc keeps the old name, else a changed, non-empty one is
/// the new name. Returns whether the field opened (`Some(true)`) or closed (`Some(false)`) this frame, and the new
/// name. The field is written only when something happens (it is change-detected).
pub fn name_in_place(
    editing: &mut bool,
    field: &mut impl std::ops::DerefMut<Target = TextField>,
    clicked: bool,
    escape: bool,
    name: &str,
) -> (Option<bool>, Option<String>) {
    let mut out = (None, None);
    if *editing && !field.focused {
        *editing = false;
        let new = field.value.trim();
        if !escape && !new.is_empty() && new != name {
            out.1 = Some(new.to_string());
        }
        out.0 = Some(false);
    }
    if clicked && !*editing {
        *editing = true;
        field.value = name.to_string();
        field.focus();
        out.0 = Some(true);
    }
    out
}

/// A one-line text field (search, names, notes). The text lives in `value`, the caret and the selection in
/// `caret`; a press on the field focuses it (the caret at the end), a press elsewhere or Esc / Enter releases it.
/// The screen spawns the field node with a [`field_text`] child.
///
/// Keys while it has the keyboard ([`edit_line`]): typing, Backspace / Delete, Left / Right / Home / End (with
/// Shift: select), Ctrl+A (all), Ctrl+C / Ctrl+X / Ctrl+V through the game's [`Clipboard`].
#[derive(Component, Clone, Debug, Default)]
pub struct TextField {
    pub value: String,
    pub placeholder: String,
    pub focused: bool,
    pub max_chars: usize,
    pub caret: Caret,
}

impl TextField {
    /// Takes the keyboard, the caret at the end.
    pub fn focus(&mut self) {
        self.focused = true;
        self.caret = Caret::end(&self.value);
    }
}

/// The caret of a [`TextField`] in characters, and the other end of the selection (`anchor == at`: nothing is
/// selected).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Caret {
    pub at: usize,
    pub anchor: usize,
}

impl Caret {
    /// At the end of `value`, nothing selected.
    pub fn end(value: &str) -> Self {
        let n = value.chars().count();
        Self { at: n, anchor: n }
    }

    /// The selection as a character range (empty: none).
    pub fn range(self) -> std::ops::Range<usize> {
        self.at.min(self.anchor)..self.at.max(self.anchor)
    }

    pub(super) fn put(&mut self, at: usize, select: bool) {
        self.at = at;
        if !select {
            self.anchor = at;
        }
    }
}

/// One key for a one-line field, the modifiers already read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKey<'a> {
    /// Typed or pasted text: it replaces the selection (control characters are dropped, the rest up to the
    /// field's length).
    Insert(&'a str),
    Backspace,
    Delete,
    /// The arrows, Home and End; `select` (Shift) moves only the caret and so grows or shrinks the selection.
    Left { select: bool },
    Right { select: bool },
    Home { select: bool },
    End { select: bool },
    SelectAll,
    /// Ctrl+X (the clipboard took the selection): the selection goes.
    Cut,
}

/// A character index of `s` as a byte offset.
pub(super) fn byte_at(s: &str, chars: usize) -> usize {
    s.char_indices().nth(chars).map_or(s.len(), |(b, _)| b)
}

/// The selected text of a field (empty: nothing selected).
pub fn selected_text(value: &str, caret: Caret) -> &str {
    let r = caret.range();
    &value[byte_at(value, r.start)..byte_at(value, r.end)]
}

/// One key applied to a field's text and caret (`max_chars`: the longest text the field takes).
pub fn edit_line(value: &mut String, caret: &mut Caret, key: FieldKey, max_chars: usize) {
    let len = value.chars().count();
    caret.at = caret.at.min(len);
    caret.anchor = caret.anchor.min(len);
    let sel = caret.range();
    // Replaces the selection (or the range `r`) with `text` and puts the caret after it.
    let replace = |value: &mut String, caret: &mut Caret, r: std::ops::Range<usize>, text: &str| {
        let (a, b) = (byte_at(value, r.start), byte_at(value, r.end));
        value.replace_range(a..b, text);
        caret.put(r.start + text.chars().count(), false);
    };
    match key {
        FieldKey::Insert(text) => {
            let room = max_chars.saturating_sub(len - sel.len());
            let text: String = text.chars().filter(|c| !c.is_control()).take(room).collect();
            if !text.is_empty() {
                replace(value, caret, sel, &text);
            }
        }
        FieldKey::Backspace if sel.is_empty() && sel.start > 0 => replace(value, caret, sel.start - 1..sel.start, ""),
        FieldKey::Delete if sel.is_empty() && sel.end < len => replace(value, caret, sel.start..sel.end + 1, ""),
        FieldKey::Backspace | FieldKey::Delete | FieldKey::Cut => {
            if !sel.is_empty() {
                replace(value, caret, sel, "");
            }
        }
        // Without Shift an arrow first collapses a selection to its side.
        FieldKey::Left { select: false } if !sel.is_empty() => caret.put(sel.start, false),
        FieldKey::Right { select: false } if !sel.is_empty() => caret.put(sel.end, false),
        FieldKey::Left { select } => caret.put(caret.at.saturating_sub(1), select),
        FieldKey::Right { select } => caret.put((caret.at + 1).min(len), select),
        FieldKey::Home { select } => caret.put(0, select),
        FieldKey::End { select } => caret.put(len, select),
        FieldKey::SelectAll => *caret = Caret { at: len, anchor: 0 },
    }
}

/// How a field reads, in the order of its text spans: the text before the selection, the caret when it is at the
/// selection's start, the selection, the caret when it is at the end (or nothing is selected), the rest.
/// Unfocused: the whole value first, no caret.
pub(super) fn field_parts(value: &str, caret: Caret, focused: bool) -> [&str; 5] {
    if !focused {
        return [value, "", "", "", ""];
    }
    let r = caret.range();
    let (a, b) = (byte_at(value, r.start), byte_at(value, r.end));
    let before = !r.is_empty() && caret.at == r.start;
    let bar = |on: bool| if on { CARET } else { "" };
    [&value[..a], bar(before), &value[a..b], bar(!before), &value[b..]]
}

/// The caret: a bar glyph in its own span, so it blinks by colour and the text beside it never moves.
pub(super) const CARET: &str = "|";

/// The text node that shows a [`TextField`]'s value (or its placeholder), the selection and the caret.
#[derive(Component)]
pub struct TextFieldText;

/// The text spans after a [`TextFieldText`]'s own text, in [`field_parts`] order.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FieldSpan(pub(super) u8);

/// The text node of a [`TextField`] (spawn it as the field's child): Barlow regular at `size`, coloured by the kit.
pub fn field_text(fonts: &HudFonts, size: f32) -> impl Bundle + use<> {
    let span = |k: u8| {
        (
            FieldSpan(k),
            TextSpan::new(""),
            TextFont { font: fonts.regular.clone().into(), font_size: FontSize::Px(size), ..default() },
            TextColor(Color::NONE),
            LetterSpacing::Px(size * 0.02),
            TextBackgroundColor(Color::NONE),
        )
    };
    (TextFieldText, prose(fonts, "", size, Color::NONE), children![span(1), span(2), span(3), span(4)])
}

/// The Ctrl and Cmd keys (shortcuts), and Shift (selecting).
pub(super) const CTRL_KEYS: [KeyCode; 4] = [KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight];
pub(super) const SHIFT_KEYS: [KeyCode; 2] = [KeyCode::ShiftLeft, KeyCode::ShiftRight];

/// Typing into the focused field ([`edit_line`]); Esc and Enter release it. The modifiers are followed through
/// the frame's key messages in order, so a Ctrl+A pressed and let go within one frame is still a shortcut (the web
/// fills in the letter of a Ctrl key: it is never typed). Pasted text and a cut come from the [`Clipboard`], which
/// gets the field's selection to copy.
pub(super) fn type_into_fields(
    mut keys: MessageReader<KeyboardInput>,
    held: Res<ButtonInput<KeyCode>>,
    mut fields: Query<&mut TextField>,
    mut clip: ResMut<Clipboard>,
) {
    let mut field = fields.iter_mut().find(|f| f.focused);
    // The modifiers held when the frame's first key came.
    let mut down: Vec<KeyCode> = CTRL_KEYS
        .iter()
        .chain(&SHIFT_KEYS)
        .copied()
        .filter(|&k| !held.just_pressed(k) && (held.pressed(k) || held.just_released(k)))
        .collect();
    for k in keys.read() {
        if CTRL_KEYS.contains(&k.key_code) || SHIFT_KEYS.contains(&k.key_code) {
            down.retain(|&d| d != k.key_code);
            if k.state == ButtonState::Pressed {
                down.push(k.key_code);
            }
            continue;
        }
        let Some(f) = field.as_mut() else { continue };
        if k.state != ButtonState::Pressed {
            continue;
        }
        let ctrl = down.iter().any(|d| CTRL_KEYS.contains(d));
        let select = down.iter().any(|d| SHIFT_KEYS.contains(d));
        let key = match &k.logical_key {
            Key::Escape | Key::Enter => {
                f.focused = false;
                continue;
            }
            Key::Backspace => FieldKey::Backspace,
            Key::Delete => FieldKey::Delete,
            Key::ArrowLeft => FieldKey::Left { select },
            Key::ArrowRight => FieldKey::Right { select },
            Key::Home => FieldKey::Home { select },
            Key::End => FieldKey::End { select },
            // Ctrl+C, Ctrl+X and Ctrl+V are the clipboard's (below); other Ctrl keys are the screens' shortcuts.
            _ if ctrl => match k.key_code {
                KeyCode::KeyA => FieldKey::SelectAll,
                _ => continue,
            },
            _ => match &k.text {
                Some(t) => FieldKey::Insert(t),
                None => continue,
            },
        };
        let f = &mut **f;
        edit_line(&mut f.value, &mut f.caret, key, f.max_chars);
    }
    let Some(mut f) = field.filter(|f| f.focused) else {
        clip.field = None;
        return;
    };
    let f = &mut *f;
    if clip.take_cut() {
        edit_line(&mut f.value, &mut f.caret, FieldKey::Cut, f.max_chars);
    }
    for text in clip.take_pasted() {
        // One line: line breaks become spaces.
        edit_line(&mut f.value, &mut f.caret, FieldKey::Insert(&text.lines().collect::<Vec<_>>().join(" ")), f.max_chars);
    }
    let selection = selected_text(&f.value, f.caret);
    if clip.field.as_deref() != Some(selection) {
        clip.field = Some(selection.to_string());
    }
}
