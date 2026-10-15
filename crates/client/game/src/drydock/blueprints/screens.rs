//! The BLUEPRINTS and EXPORT screens' nodes: spawned once at start (hidden), their rows, buttons and texts.

use super::*;

// ---------------------------------------------------------------------------------------------------------
// The screens

#[derive(Component)]
pub(in crate::drydock) struct ShelfRoot;

#[derive(Component)]
pub(in crate::drydock) struct PageNode(pub(in crate::drydock) Page);

/// The action bar's variants (browse pills, the rename field, the delete question).
#[derive(Component)]
pub(in crate::drydock) struct BarNode(pub(in crate::drydock) u8);

#[derive(Component)]
pub(in crate::drydock) struct RenameField;

/// A list's rows container (0 designs, 1 templates), for the wheel.
#[derive(Component)]
pub(in crate::drydock) struct ListNode(pub(in crate::drydock) usize);

#[derive(Component)]
pub(in crate::drydock) struct RowNode(pub(in crate::drydock) usize, pub(in crate::drydock) usize);

#[derive(Component)]
pub(in crate::drydock) struct EmptyHint;

#[derive(Component)]
pub(in crate::drydock) struct CodeBox;

#[derive(Component)]
pub(in crate::drydock) struct PasteBox;

/// The preview's column between the two lists (so it sits next to either), and its card: the ship of the hovered
/// row drawn with the flight tiles ([`PreviewShip`], a `UiIcon::Ship` like FLEET's) over a few numbers
/// ([`PreviewStat`]), built once per hovered entry (`shown`) and moved along with the row.
#[derive(Component)]
pub(in crate::drydock) struct PreviewColumn;

#[derive(Component, Default)]
pub(in crate::drydock) struct PreviewCard {
    pub(super) shown: Option<Pick>,
}

#[derive(Component)]
pub(in crate::drydock) struct PreviewShip;

/// The preview's values: the size class, mass, top speed, weapons.
#[derive(Component)]
pub(in crate::drydock) struct PreviewStat(pub(super) usize);

/// Every button of the screens.
#[derive(Component, Clone, Debug, PartialEq)]
pub(in crate::drydock) enum ShelfButton {
    Close,
    Row(usize, usize),
    Open,
    Duplicate,
    Rename,
    Delete,
    ConfirmDelete,
    KeepDelete,
    New((u32, u32)),
    Frame((u32, u32)),
    ImportFile,
    Download,
    Copy,
    CodeBox,
    PasteBox,
}

/// Every text the screens update.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::drydock) enum ShelfText {
    Crumb,
    Title,
    Section(usize),
    RowName(usize, usize),
    RowSub(usize, usize),
    RowTag(usize, usize),
    DeleteQuestion,
    FileNote,
    Code,
    CodeNote,
    Paste,
}

pub(in crate::drydock) fn spawn_shelf(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>, content: Res<GameContent>, clear: Res<ClearColor>) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let fonts = &*fonts;
    let content = &content.0;
    let pill = Pill::new(Tone::Teal);
    let row = |gap: f32| Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(gap), flex_shrink: 0.0, ..default() };
    let head = |text: &str, width: f32| (label(fonts, text, 9.0, c(Tone::Dim, 1.0)), Node { width: px(width), flex_shrink: 0.0, ..default() });
    let frames: Vec<(String, (u32, u32))> = content.frames.iter().map(|f| (f.name.to_uppercase(), f.size)).collect();

    commands
        .spawn((
            ShelfRoot,
            UiBlock,
            // Hot, so a click on the veil reaches no widget under it.
            Hot::default(),
            GlobalZIndex(20),
            Visibility::Hidden,
            // The backing covers the design as a world mesh, so the screen's own icons (the frames' locks, the
            // preview's ship) draw over it: under an opaque UI background they were hidden, and a lock's empty
            // room pushed its pill's label right of centre. The drydock's parts it covers hide meanwhile
            // (`UnderShelf`): they would draw over it.
            UiIcon::Scrim { color: clear.0.with_alpha(0.985), hole: [f32::INFINITY; 4], out: 0.0, into: 0.0 },
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(66),
                flex_direction: FlexDirection::Column,
                padding: UiRect::new(px(100), px(100), px(18), px(10)),
                row_gap: px(18),
                ..default()
            },
        ))
        .with_children(|root| {
            // The header: breadcrumb and title, CLOSE.
            root.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..row(14.0) }).with_children(|h| {
                h.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() }).with_children(|col| {
                    col.spawn((ShelfText::Crumb, label(fonts, "", 10.0, c(Tone::Label, 1.0))));
                    col.spawn((ShelfText::Title, number(fonts, "", 20.0, c(Tone::Number, 1.0))));
                });
                spawn_pill(h, fonts, "CLOSE", 9.5, 30.0, None, pill, (ShelfButton::Close, Tip::key("CLOSE", "ESC")));
            });

            // BLUEPRINTS: the two lists, the action bar, NEW and FRAME, IMPORT.
            root.spawn((PageNode(Page::Blueprints), Node { flex_direction: FlexDirection::Column, row_gap: px(16), flex_grow: 1.0, min_height: px(0), ..default() }))
                .with_children(|page| {
                    page.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(LISTS_GAP), flex_grow: 1.0, min_height: px(0), ..default() }).with_children(|lists| {
                        for (list, width) in [(0, DESIGNS_W), (1, TEMPLATES_W)] {
                            if list == 1 {
                                spawn_preview(lists, fonts, &c, content);
                            }
                            lists.spawn(Node { width: px(width), flex_direction: FlexDirection::Column, row_gap: px(10), min_height: px(0), ..default() }).with_children(|col| {
                                col.spawn((ShelfText::Section(list), label(fonts, "", 9.0, c(Tone::Dim, 1.0)), Node { margin: UiRect::left(px(12)), ..default() }));
                                col.spawn((
                                    ListNode(list),
                                    Scrollbar::vertical(),
                                    ScrollRows::default(),
                                    Node {
                                        flex_direction: FlexDirection::Column,
                                        row_gap: px(ROW_GAP),
                                        flex_grow: 1.0,
                                        min_height: px(0),
                                        max_height: px(LIST_H),
                                        overflow: Overflow::scroll_y(),
                                        ..default()
                                    },
                                ))
                                .with_children(|l| {
                                    if list == 0 {
                                        l.spawn((EmptyHint, label(fonts, "NOTHING SAVED YET  ·  SAVE KEEPS THE DESIGN IN THE DRYDOCK", 8.5, c(Tone::Dim, 1.0)), Node { margin: UiRect::new(px(12), px(0), px(14), px(0)), ..default() }));
                                    }
                                    for k in 0..=LIST_ROWS {
                                        spawn_row(l, fonts, &c, list, k);
                                    }
                                });
                            });
                        }
                    });
                    // The action bar: browse / rename / delete.
                    page.spawn(Node { height: px(34), ..row(10.0) }).with_children(|bar| {
                        bar.spawn((BarNode(0), row(8.0))).with_children(|b| {
                            for (text, button) in [("OPEN", ShelfButton::Open), ("DUPLICATE", ShelfButton::Duplicate), ("RENAME", ShelfButton::Rename), ("DELETE", ShelfButton::Delete)] {
                                spawn_pill(b, fonts, text, 9.5, 30.0, None, pill, button);
                            }
                        });
                        bar.spawn((BarNode(1), Node { display: Display::None, ..row(12.0) })).with_children(|b| {
                            b.spawn(head("NEW NAME", 90.0));
                            b.spawn((
                                RenameField,
                                TextField { placeholder: "Name".into(), max_chars: MAX_NAME, ..default() },
                                Hot::default(),
                                Node {
                                    width: px(320),
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
                                f.spawn(field_text(fonts, 13.0));
                            });
                            b.spawn(label(fonts, "TYPE A NEW NAME", 8.5, c(Tone::Dim, 1.0)));
                        });
                        bar.spawn((BarNode(2), Node { display: Display::None, ..row(10.0) })).with_children(|b| {
                            b.spawn((ShelfText::DeleteQuestion, label(fonts, "", 9.5, c(Tone::Coral, 1.0)), Node { margin: UiRect::right(px(8)), ..default() }));
                            spawn_pill(b, fonts, "DELETE", 9.5, 30.0, None, Pill { active: true, ..Pill::new(Tone::Coral) }, ShelfButton::ConfirmDelete);
                            spawn_pill(b, fonts, "KEEP", 9.5, 30.0, None, pill, ShelfButton::KeepDelete);
                        });
                    });
                    page.spawn((Node { height: px(1), flex_shrink: 0.0, ..default() }, BackgroundColor(c(Tone::Label, 0.14))));
                    // NEW in a frame; the frame of the design in the drydock: two rows with their own heads (on one
                    // line the two sets of frame pills looked alike).
                    let lock = || Some(UiIcon::Glyph { glyph: Glyph::Lock, color: ui.dim });
                    page.spawn(row(8.0)).with_children(|r| {
                        r.spawn(head("NEW EMPTY DESIGN", HEAD_W));
                        for (name, size) in &frames {
                            spawn_pill(r, fonts, name, 9.5, 30.0, lock(), pill, (ShelfButton::New(*size), PillLock));
                        }
                    });
                    page.spawn(row(8.0)).with_children(|r| {
                        r.spawn(head("FRAME OF THIS DESIGN", HEAD_W));
                        for (name, size) in &frames {
                            let text = format!("{name} {}×{}", size.0, size.1);
                            spawn_pill(r, fonts, &text, 9.5, 30.0, lock(), pill, (ShelfButton::Frame(*size), PillLock));
                        }
                    });
                    // IMPORT: a file, or paste a code.
                    page.spawn(row(8.0)).with_children(|r| {
                        r.spawn(head("IMPORT", HEAD_W));
                        spawn_pill(r, fonts, ".RON FILE", 9.5, 30.0, None, pill, ShelfButton::ImportFile);
                        r.spawn((
                            PasteBox,
                            ShelfButton::PasteBox,
                            Hot::default(),
                            // As tall as the pill beside it, the text inset like a pill's label.
                            Node {
                                width: px(520),
                                height: px(30),
                                align_items: AlignItems::Center,
                                padding: UiRect::horizontal(px(16)),
                                border: UiRect::all(px(1.2)),
                                border_radius: BorderRadius::MAX,
                                overflow: Overflow::clip(),
                                ..default()
                            },
                            BorderColor::all(c(Tone::Label, 0.3)),
                            BackgroundColor(Color::NONE),
                        ))
                        .with_children(|b| {
                            b.spawn((ShelfText::Paste, tracked(fonts, "", 8.5, c(Tone::Dim, 1.0), 0.2)));
                        });
                    });
                });

            // EXPORT: the file and the share code.
            root.spawn((PageNode(Page::Export), Node { display: Display::None, flex_direction: FlexDirection::Column, row_gap: px(22), ..default() }))
                .with_children(|page| {
                    page.spawn(row(12.0)).with_children(|r| {
                        r.spawn(head("FILE", 110.0));
                        let text = if cfg!(target_arch = "wasm32") { "DOWNLOAD .RON" } else { "WRITE .RON FILE" };
                        spawn_pill(r, fonts, text, 9.5, 30.0, None, pill, ShelfButton::Download);
                        r.spawn((ShelfText::FileNote, label(fonts, "", 8.5, c(Tone::Dim, 1.0))));
                    });
                    page.spawn(Node { align_items: AlignItems::FlexStart, ..row(12.0) }).with_children(|r| {
                        r.spawn((label(fonts, "SHARE CODE", 9.0, c(Tone::Dim, 1.0)), Node { width: px(110), flex_shrink: 0.0, margin: UiRect::top(px(12)), ..default() }));
                        r.spawn((
                            CodeBox,
                            ShelfButton::CodeBox,
                            Hot::default(),
                            Node { width: px(760), padding: UiRect::axes(px(16), px(12)), border: UiRect::all(px(1.2)), border_radius: BorderRadius::all(px(16)), ..default() },
                            BorderColor::all(c(Tone::Label, 0.3)),
                            BackgroundColor(Color::NONE),
                        ))
                        .with_children(|b| {
                            b.spawn((
                                ShelfText::Code,
                                prose(fonts, "", 11.0, c(Tone::Number, 0.9)),
                                TextLayout::new(Justify::Left, LineBreak::AnyCharacter),
                                Node { width: percent(100), ..default() },
                            ));
                        });
                        spawn_pill(r, fonts, "COPY", 9.5, 30.0, None, pill, ShelfButton::Copy);
                    });
                    page.spawn((ShelfText::CodeNote, label(fonts, "", 8.5, c(Tone::Dim, 1.0)), Node { margin: UiRect::left(px(122)), ..default() }));
                });
        });
}

/// The preview's column and its card (hidden until a row is hovered).
fn spawn_preview(lists: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content) {
    let names = ["CLASS".to_string(), stats::stat_label(content, "mass"), stats::stat_label(content, "top_speed"), "WEAPONS".to_string()];
    lists.spawn((PreviewColumn, Node { width: px(PREVIEW_W), flex_shrink: 0.0, ..default() })).with_children(|col| {
        col.spawn((
            PreviewCard::default(),
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                padding: UiRect::new(px(14), px(14), px(12), px(14)),
                border: UiRect::all(px(1.2)),
                border_radius: BorderRadius::all(px(16)),
                ..default()
            },
            BorderColor::all(c(Tone::Label, 0.3)),
        ))
        .with_children(|card| {
            // Inset from the card's edge: a ship's glow, plumes and beacons reach past its cells.
            card.spawn((PreviewShip, Node { height: px(PREVIEW_SHIP_H), margin: UiRect::new(px(14), px(14), px(14), px(18)), flex_shrink: 0.0, ..default() }));
            for (k, name) in names.iter().enumerate() {
                card.spawn(Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, ..default() }).with_children(|r| {
                    r.spawn(label(fonts, name, 8.5, c(Tone::Dim, 1.0)));
                    r.spawn((PreviewStat(k), number(fonts, "", 12.0, c(Tone::Number, 1.0))));
                });
            }
        });
    });
}

pub(in crate::drydock) fn spawn_row(l: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, list: usize, k: usize) {
    l.spawn((
        RowNode(list, k),
        ShelfButton::Row(list, k),
        Pill { bare: true, ..Pill::new(Tone::Teal) },
        Hot::default(),
        Node {
            display: Display::None,
            height: px(ROW_H),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::horizontal(px(16)),
            border: UiRect::all(px(1.2)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BorderColor::all(Color::NONE),
        BackgroundColor(Color::NONE),
    ))
    .with_children(|r| {
        r.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() }).with_children(|t| {
            t.spawn((ShelfText::RowName(list, k), prose(fonts, "", 14.0, c(Tone::Number, 1.0))));
            t.spawn((ShelfText::RowSub(list, k), tracked(fonts, "", 8.5, c(Tone::Dim, 1.0), 0.2)));
        });
        r.spawn((ShelfText::RowTag(list, k), label(fonts, "", 8.5, c(Tone::Teal, 1.0))));
    });
}
