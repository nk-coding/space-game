//! The design's name top left: click to rename in place, a tag when it is new or has unsaved changes.

use super::*;

// ---------------------------------------------------------------------------------------------------------
// The name top left

#[derive(Component)]
pub(super) struct TitleButton;

#[derive(Component)]
pub(super) struct TitleField;

/// "UNSAVED" or "SAVED" after the name.
#[derive(Component)]
pub(super) struct TitleNote;

/// The design's name (the `PanelText::Name` text `panels` writes), the field that replaces it while it is
/// edited, and the saved state after it.
pub(in crate::drydock) fn spawn_title(parent: &mut ChildSpawnerCommands, fonts: &HudFonts, name: Color, dim: Color, edge: Color) {
    parent
        .spawn((TitleButton, Hot::default(), Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(12), ..default() }))
        .with_children(|t| {
            t.spawn((PanelText::Name, number(fonts, "", 20.0, name)));
            t.spawn((
                TitleField,
                TextField { placeholder: "Name".into(), max_chars: MAX_NAME, ..default() },
                Hot::default(),
                Node {
                    display: Display::None,
                    width: px(300),
                    height: px(30),
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(px(12)),
                    border: UiRect::all(px(1.2)),
                    border_radius: BorderRadius::MAX,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BorderColor::all(edge),
            ))
            .with_children(|f| {
                f.spawn(field_text(fonts, 16.0));
            });
            t.spawn((TitleNote, label(fonts, "", 8.5, dim)));
        });
}

/// A click on the name opens the field; Enter or a click elsewhere renames, Esc cancels.
pub(super) fn title(
    mut clicks: MessageReader<UiClick>,
    keys: Res<ButtonInput<KeyCode>>,
    button: Single<Entity, With<TitleButton>>,
    mut field: Single<(&mut TextField, &mut Node), With<TitleField>>,
    mut texts: Query<(&PanelText, &mut Node), Without<TitleField>>,
    mut saver: ResMut<Saver>,
    mut design: ResMut<PlayerDesign>,
) {
    let (f, node) = &mut *field;
    let clicked = clicks.read().any(|c| c.0 == *button);
    let (open, renamed) = name_in_place(&mut saver.editing_title, f, clicked, keys.just_pressed(KeyCode::Escape), &design.blueprint().name);
    if let Some(new) = renamed {
        design.edit_header(|h| h.name = new);
    }
    if let Some(open) = open {
        node.display = if open { Display::Flex } else { Display::None };
        for (t, mut n) in &mut texts {
            if *t == PanelText::Name {
                n.display = if open { Display::None } else { Display::Flex };
            }
        }
    }
}

/// "UNSAVED" while the design differs from its saved (or opened) version.
pub(super) fn sync_title(
    design: Res<PlayerDesign>,
    slot: Res<DesignSlot>,
    style: Res<Style>,
    note: Single<(&mut Text, &mut TextColor), With<TitleNote>>,
    mut done: Local<bool>,
) {
    if *done && !design.is_changed() && !slot.is_changed() {
        return;
    }
    *done = true;
    let (text, tone) = title_tag(slot.id.is_some(), slot.dirty(&design));
    let (mut t, mut color) = note.into_inner();
    set_text(&mut t, text);
    set_color(&mut color, ui_color(tone.color(&style.ui), 1.0));
}

/// The saved state after the design's name, in the BEHAVIOR screen's tones (`behavior_ui::saving::header_tag`):
/// UNSAVED gold, SAVED grey, nothing for a design never saved and unchanged.
pub(super) fn title_tag(saved_before: bool, dirty: bool) -> (&'static str, Tone) {
    match (saved_before, dirty) {
        (_, true) => ("UNSAVED", Tone::Gold),
        (true, false) => ("SAVED", Tone::Label),
        (false, false) => ("", Tone::Dim),
    }
}
