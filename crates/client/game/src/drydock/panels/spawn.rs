//! The screen's nodes (layout v2 of GDR-0082), spawned once at start: the palette (the kit's grouped list; its rows
//! come from `fill_palette`), the armour swatches, the stats and the selected part, the bottom bar with its menus.

use super::*;
use crate::home::{COMPACT_TEXT, by_class};
use data::input::Action;
use kit::ui_kit::CompactText;
use world::settings::{ActionLabel, ActionTip};

pub(in crate::drydock) fn spawn(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>, content: Res<GameContent>, lab: Res<world::evolution::Lab>) {
    let ui = style.ui;
    let c = |tone: Tone, a: f32| ui_color(tone.color(&ui), a);
    let content = &content.0;
    let abs = |l: Option<f32>, r: Option<f32>, t: Option<f32>, b: Option<f32>| Node {
        position_type: PositionType::Absolute,
        left: l.map(px).unwrap_or(Val::Auto),
        right: r.map(px).unwrap_or(Val::Auto),
        top: t.map(px).unwrap_or(Val::Auto),
        bottom: b.map(px).unwrap_or(Val::Auto),
        ..default()
    };
    let glyph = |glyph: Glyph, tone: Tone, size: f32| {
        (UiIcon::Glyph { glyph, color: tone.color(&ui) }, Node { width: px(size), height: px(size), flex_shrink: 0.0, ..default() })
    };
    let pill = |accent: Tone| Pill::new(accent);
    let sep = || (Node { width: px(1), height: px(24), margin: UiRect::horizontal(px(4)), flex_shrink: 0.0, ..default() }, BackgroundColor(c(Tone::Label, 0.22)));
    let fonts = &*fonts;
    let back = style.background.outer();
    // Every part placed for both layout classes (`home::ByClass`): the wide layout as it was, the phone layout
    // (`C_*`, docs/plans/android.md 4.3); a part only one of them has is hidden in the other.
    let none = || Node { display: Display::None, ..default() };

    commands
        .spawn((
            DrydockRoot,
            CompactText(COMPACT_TEXT),
            // Opens and closes with a transition (`view::open_close`): the panels slide in from their edges.
            ScreenFx::closed(),
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
            Visibility::Hidden,
        ))
        .with_children(|root| {
            // A scrim around the grid, first so everything draws over it: the backdrop colour on every side, fading
            // out at the grid's edge, so the panels and bars read over a ship or the yard behind them and the frame
            // looks the same all round (Niklas, 14 Oct 2026: "with the ship in the background the side panels stuff
            // is still barely readable", then "I can still see a difference in background visibility between the
            // side and bottom thing"; the BEHAVIOR screen covers the world instead, which the drydock cannot: its
            // ship is the world). A world mesh (`UiIcon::Scrim`), so it lies under the panels' icons too.
            // Its hole comes from the grid viewport's laid-out rectangle (`sync_scrim`), not from copied numbers.
            let scrim = UiIcon::Scrim { color: back.with_alpha(SCRIM_ALPHA), hole: [0.0; 4], out: 0.0, into: SCRIM_INTO };
            root.spawn((GridScrim, ScreenPart::new(Edge::Fade, 1), Node { width: percent(100), height: percent(100), ..abs(Some(0.0), None, Some(0.0), None) }, scrim));

            // Top left: the ring menu, the breadcrumb and the design's name.
            let f = frame();
            root.spawn((UiBlock, UnderShelf, ScreenPart::new(Edge::Top, 0), f.top_left()))
                .with_children(|tl| {
                    spawn_back(tl, &ui, (ActionButton(DrydockAction::Menu), Tip::new("SHIPYARD")));
                    tl.spawn(f.heading()).with_children(|col| {
                        col.spawn((PanelText::Breadcrumb, crumb(fonts, "", &ui)));
                        crate::drydock::blueprints::spawn_title(col, fonts, c(Tone::Number, 1.0), c(Tone::Dim, 1.0), c(Tone::Label, 0.3));
                    });
                });

            // Top right: the overlays.
            root.spawn((UiBlock, UnderShelf, ScreenPart::new(Edge::Top, 0), Node { column_gap: px(5), ..f.top_right() }))
                .with_children(|tr| {
                    tr.spawn((label(fonts, "OVERLAY", 9.0, c(Tone::Dim, 1.0)), by_class(Node { margin: UiRect::right(px(5)), ..default() }, none())));
                    for o in Overlay::ALL {
                        let accent = match o {
                            Overlay::Bonus => Tone::Gold,
                            Overlay::Blast => Tone::Coral,
                            _ => Tone::Teal,
                        };
                        spawn_pill(tr, fonts, overlay_name(o), 10.0, 30.0, None, pill(accent), ActionButton(DrydockAction::Overlay(o)));
                    }
                });

            // Left: the kit's grouped list (search, armour between it and the category chips, every part in
            // sections; the rows are filled by `fill_palette`).
            let left = Node { width: px(LEFT_W), flex_direction: FlexDirection::Column, row_gap: px(10), ..abs(Some(SIDE), None, Some(74.0), Some(GRID_BOTTOM + INFO_H)) };
            let c_left = Node { width: px(C_LEFT_W), row_gap: px(8), ..abs(Some(C_SIDE), None, Some(C_TOP), Some(C_BOTTOM)) };
            root.spawn((UiBlock, UnderShelf, ScreenPart::new(Edge::Left, 1), by_class(left, Node { flex_direction: FlexDirection::Column, ..c_left })))
                .with_children(|lp| {
                    let spec = ListSpec {
                        placeholder: "Search parts".into(),
                        max_chars: 24,
                        groups: content.categories.iter().map(|c| (c.id.clone(), c.name.clone())).collect(),
                        aside: "LOCKED",
                        row_gap: ROW_GAP,
                        pad_right: 0.0,
                    };
                    let list = spawn_grouped_list(lp, fonts, &ui, spec, (SearchField, Tip::new("SEARCH PARTS"), ActionTip(Action::Search)), |lp| {
                        // ARMOR · <name>, then the swatches: every armour material research can unlock (a locked one
                        // shows a lock; the keys 1–9 count every armour material, hidden ones too).
                        lp.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(8), margin: UiRect::new(px(4), px(0), px(4), px(0)), flex_shrink: 0.0, ..default() })
                            .with_children(|r| {
                                r.spawn(label(fonts, "ARMOR  ·", 9.0, c(Tone::Dim, 1.0)));
                                r.spawn((PanelText::Armor, label(fonts, "", 10.0, c(Tone::Number, 1.0))));
                            });
                        spawn_swatches(lp, fonts, &c, content);
                    });
                    lp.commands().entity(list).insert(PaletteList);
                });

            // Right: the stats grid and the selected part.
            // On a phone the stats and the selected part take turns (`sync_column`), and a cell's label sits over its
            // value.
            let right = Node { width: px(RIGHT_W), flex_direction: FlexDirection::Column, ..abs(None, Some(SIDE), Some(78.0), Some(70.0)) };
            let c_right = Node { width: px(C_RIGHT_W), flex_direction: FlexDirection::Column, ..abs(None, Some(C_SIDE), Some(C_TOP), Some(C_BOTTOM)) };
            let grid = |gap: (f32, f32), top: f32| Node {
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::flex(2, 1.0),
                column_gap: px(gap.0),
                row_gap: px(gap.1),
                margin: UiRect::top(px(top)),
                ..default()
            };
            let pair = Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, column_gap: px(8), ..default() };
            let c_pair = Node { flex_direction: FlexDirection::Column, align_items: AlignItems::FlexStart, row_gap: px(2), ..default() };
            root.spawn((UiBlock, UnderShelf, ScreenPart::new(Edge::Right, 1), by_class(right, c_right)))
                .with_children(|rp| {
                    // The stats in a column of their own, so a phone can hide them (`sync_column`).
                    rp.spawn((ColumnPart::Stats, Node { flex_direction: FlexDirection::Column, flex_shrink: 0.0, ..default() })).with_children(|rp| {
                    rp.spawn(by_class(grid((20.0, 16.0), 0.0), grid((14.0, 8.0), 0.0)))
                    .with_children(|g| {
                        for k in 0..MAIN_CELLS {
                            g.spawn((StatCellNode(k), by_class(Node { flex_direction: FlexDirection::Column, row_gap: px(7), ..default() }, Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() }))).with_children(|cell| {
                                cell.spawn(by_class(pair.clone(), c_pair.clone()))
                                    .with_children(|r| {
                                        r.spawn((PanelText::StatLabel(k), label(fonts, "", 9.0, c(Tone::Label, 1.0))));
                                        r.spawn((PanelText::StatValue(k), number(fonts, "", 16.0, c(Tone::Number, 1.0))));
                                    });
                                spawn_bar(cell, ValueBar { share: 0.0, tone: Tone::Gold }, StatBar(k));
                                cell.spawn((PanelText::StatNote(k), label(fonts, "", 8.0, c(Tone::Label, 1.0)), Node { display: Display::None, ..default() }));
                            });
                        }
                    });
                    // The compact block under it: smaller, no bars (top speed, turn, mass, build points).
                    rp.spawn(by_class(grid((20.0, 9.0), 18.0), grid((14.0, 6.0), 10.0)))
                    .with_children(|g| {
                        for k in MAIN_CELLS..MAIN_CELLS + DETAIL_CELLS {
                            g.spawn((StatCellNode(k), by_class(pair.clone(), c_pair.clone())))
                                .with_children(|r| {
                                    r.spawn((PanelText::StatLabel(k), label(fonts, "", 8.0, c(Tone::Label, 1.0))));
                                    r.spawn((PanelText::StatValue(k), number(fonts, "", 12.5, c(Tone::Number, 1.0))));
                                });
                        }
                    });
                    });
                    rp.spawn((by_class(Node { height: px(1), margin: UiRect::vertical(px(14)), flex_shrink: 0.0, ..default() }, none()), BackgroundColor(c(Tone::Label, 0.14))));
                    // SELECTED PART; on a phone after a ring back to the stats (it deselects).
                    rp.spawn((ColumnPart::Selected, Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(10), margin: UiRect::bottom(px(10)), flex_shrink: 0.0, ..default() })).with_children(|h| {
                        h.spawn(by_class(none(), Node::default())).with_children(|b| {
                            b.spawn((ring_node(30.0), ActionButton(DrydockAction::Deselect), Tip::new("BACK TO THE STATS"))).with_child(ring_glyph(Glyph::Prev, 12.0));
                        });
                        h.spawn(label(fonts, "SELECTED PART", 9.0, c(Tone::Label, 1.0)));
                    });
                    rp.spawn((ColumnPart::Selected, Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(16), flex_shrink: 0.0, ..default() })).with_children(|h| {
                        // The icon in a bracketed box (like the fleet bar's chips).
                        h.spawn(Node { width: px(54), height: px(54), flex_shrink: 0.0, justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() })
                            .with_children(|b| {
                                spawn_brackets(b, 9.0, 1.2, c(Tone::Label, 0.45));
                                b.spawn((SelIcon, UiIcon::Part { part: String::new(), material: None }, Node { width: px(40), height: px(40), ..default() }));
                            });
                        h.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(6), ..default() }).with_children(|t| {
                            t.spawn((PanelText::SelName, prose(fonts, "", 18.0, c(Tone::Number, 1.0))));
                            t.spawn((PanelText::SelSub, label(fonts, "", 8.5, c(Tone::Dim, 1.0))));
                        });
                    });
                    // The rest scrolls when a part has more lines and controls than the column has room for.
                    rp
                        .spawn((
                            SelScroll,
                            ColumnPart::Selected,
                            ScrollPosition::default(),
                            Scrollbar::vertical().room(0),
                            Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_basis: px(0), min_height: px(0), overflow: Overflow::scroll_y(), ..default() },
                        ))
                        .with_children(|sb| {
                            sb.spawn((PanelText::SelSummary, prose(fonts, "", 11.5, c(Tone::Label, 1.0)), Node { margin: UiRect::top(px(12)), flex_shrink: 0.0, ..default() }));
                            // The shared stat block (the hover card's too): two columns, a label over its value.
                            sb.spawn(Node { flex_direction: FlexDirection::Column, margin: UiRect::top(px(14)), flex_shrink: 0.0, ..default() }).with_children(|w| {
                                spawn_stat_block(w, fonts, &style, SEL_STATS, SelStats);
                            });
                            sb.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(8), margin: UiRect::top(px(12)), flex_shrink: 0.0, ..default() }).with_children(|b| {
                                for k in 0..SEL_BONUSES {
                                    b.spawn((SelBonus(k), Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(10), ..default() }))
                                        .with_children(|r| {
                                            r.spawn(glyph(Glyph::Diamond, Tone::Gold, 9.0));
                                            r.spawn((PanelText::SelBonus(k), tracked(fonts, "", 8.5, c(Tone::Gold, 1.0), 0.2)));
                                        });
                                }
                            });
                            crate::drydock::inspector::spawn_controls(sb, fonts, &style, content);
                        });
                });

            // Centre: the grid viewport; the info line and the zoom.
            let area = |l: f32, r: f32, t: f32, b: f32| Node { flex_direction: FlexDirection::Column, ..abs(Some(l), Some(r), Some(t), Some(b)) };
            let c_grid_left = C_SIDE + C_LEFT_W + C_GAP + C_TOOLS_W + C_GAP;
            let c_grid = area(c_grid_left, C_SIDE + C_RIGHT_W + C_GAP, C_GRID_TOP, C_GRID_BOTTOM);
            root.spawn((UnderShelf, by_class(area(GRID_LEFT, GRID_RIGHT, GRID_TOP, GRID_BOTTOM), c_grid))).with_children(|ga| {
                // The info line moves, the grid viewport never (the camera centres on it).
                ga.spawn((GridViewport, Node { flex_grow: 1.0, ..default() }));
                ga.spawn((ScreenPart::new(Edge::Bottom, 2), Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, height: px(INFO_H), flex_shrink: 0.0, ..default() }))
                    .with_children(|b| {
                        // On a phone it gives way to the zoom (a long hint wraps, cut at the grid's edge).
                        let c_info = Node { flex_grow: 1.0, flex_shrink: 1.0, flex_basis: px(0), min_width: px(0), overflow: Overflow::clip(), ..default() };
                        b.spawn((PanelText::Info, label(fonts, "", 9.0, c(Tone::Dim, 1.0)), by_class(Node::default(), c_info))).with_child(crate::drydock::ghost::hint_span(fonts, &style));
                        b.spawn((UiBlock, Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(10), ..default() })).with_children(|z| {
                            z.spawn((ring_node(28.0), ActionButton(DrydockAction::ZoomOut), Tip::new("ZOOM OUT"), ActionTip(Action::ZoomOut))).with_child(ring_glyph(Glyph::Minus, 13.0));
                            // A fixed width with the value centred in it, so the rings stay put and "58 %" sits as centred as "100 %".
                            z.spawn(Node { width: px(46), justify_content: JustifyContent::Center, ..default() }).with_child((PanelText::Zoom, number(fonts, "", 13.0, c(Tone::Number, 1.0))));
                            z.spawn((ring_node(28.0), ActionButton(DrydockAction::ZoomIn), Tip::new("ZOOM IN"), ActionTip(Action::ZoomIn))).with_child(ring_glyph(Glyph::Plus, 13.0));
                        });
                    });
            });

            // A phone's tools: a strip between the palette and the grid (the bottom bar has no room for them).
            let strip = Node {
                width: px(C_TOOLS_W),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                row_gap: px(8),
                ..abs(Some(C_SIDE + C_LEFT_W + C_GAP), None, Some(C_TOP + 6.0), None)
            };
            root.spawn((UiBlock, UnderShelf, ScreenPart::new(Edge::Left, 2), by_class(none(), strip))).with_children(|st| {
                for (text, key, action) in [
                    ("PAINT", Action::ToolPaint, DrydockAction::Tool(Tool::Paint)),
                    ("FILL", Action::ToolFill, DrydockAction::Tool(Tool::Fill)),
                    ("ERASE", Action::ToolErase, DrydockAction::Tool(Tool::Erase)),
                    ("SELECT", Action::ToolSelect, DrydockAction::Tool(Tool::Select)),
                    ("ROTATE", Action::RotateRight, DrydockAction::RotateCw),
                ] {
                    spawn_pill(st, fonts, text, 9.0, 30.0, None, pill(Tone::Teal), (ActionButton(action), Tip::new(text), ActionTip(key)));
                }
            });

            // The veil under an open menu: a click on it (outside the menu) closes the menu and reaches nothing else.
            root.spawn((
                MenuVeil,
                Hot::default(),
                ActionButton(DrydockAction::CloseMenu),
                GlobalZIndex(VEIL_Z),
                Node { display: Display::None, ..abs(Some(0.0), Some(0.0), Some(0.0), Some(0.0)) },
            ));

            // Bottom: undo / redo, the tools, mirror and bevel, the status line, save / blueprints / export / ⋯,
            // the BEHAVIOR screen and the test flight.
            let bar = |l: f32, r: f32, b: f32| Node { height: px(42), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(6), ..abs(Some(l), Some(r), None, Some(b)) };
            root.spawn((UiBlock, ScreenPart::new(Edge::Bottom, 3), by_class(bar(22.0, 20.0, 14.0), bar(C_SIDE, C_SIDE, 6.0))))
                .with_children(|bb| {
                    for (g, action, tip) in [(Glyph::Undo, DrydockAction::Undo, (Tip::new("UNDO"), ActionTip(Action::Undo))), (Glyph::Redo, DrydockAction::Redo, (Tip::new("REDO"), ActionTip(Action::Redo)))] {
                        bb.spawn((ring_node(36.0), ActionButton(action), tip)).with_children(|r| {
                            r.spawn(glyph(g, Tone::Number, 19.0));
                        });
                    }
                    bb.spawn(sep());
                    let tools = [
                        ("PAINT", Action::ToolPaint, DrydockAction::Tool(Tool::Paint)),
                        ("FILL", Action::ToolFill, DrydockAction::Tool(Tool::Fill)),
                        ("ERASE", Action::ToolErase, DrydockAction::Tool(Tool::Erase)),
                        ("SELECT", Action::ToolSelect, DrydockAction::Tool(Tool::Select)),
                        ("ROTATE", Action::RotateRight, DrydockAction::RotateCw),
                    ];
                    // Wide: the tool pills, mirror and bevel.
                    bb.spawn((WideTools, Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(6), flex_shrink: 0.0, ..default() })).with_children(|w| {
                        for (text, key, action) in tools.clone() {
                            spawn_pill(w, fonts, text, 9.5, 30.0, None, pill(Tone::Teal), (ActionButton(action), Tip::new(text), ActionTip(key)));
                        }
                        w.spawn(sep());
                        spawn_pill(w, fonts, "MIRROR", 9.5, 30.0, None, pill(Tone::Teal), (ActionButton(DrydockAction::Mirror), Tip::new("MIRROR"), ActionTip(Action::Mirror)));
                        spawn_pill(w, fonts, "BEVEL", 9.5, 30.0, None, pill(Tone::Teal), (ActionButton(DrydockAction::Bevel), Tip::new("BEVEL"), ActionTip(Action::ToolBevel)));
                    });
                    // Folded: the current tool as a dropdown (its menu opens upward), mirror and bevel as icons.
                    bb.spawn((FoldedTools, Node { display: Display::None, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(6), flex_shrink: 0.0, ..default() }))
                        .with_children(|f| {
                            // On a phone the tool strip has the tools: only mirror and bevel stay here.
                            f.spawn(by_class(Node::default(), none())).with_children(|anchor| {
                                anchor
                                    .spawn((
                                        Pill { active: true, ..pill(Tone::Teal) },
                                        Hot::default(),
                                        ActionButton(DrydockAction::OpenMenu(BarMenu::Tools)),
                                        Tip::new("TOOLS"),
                                        Node {
                                            height: px(30),
                                            padding: UiRect::new(px(14), px(11), px(0), px(0)),
                                            border: UiRect::all(px(1.2)),
                                            border_radius: BorderRadius::MAX,
                                            align_items: AlignItems::Center,
                                            column_gap: px(8),
                                            ..default()
                                        },
                                        BorderColor::all(Color::NONE),
                                        BackgroundColor(Color::NONE),
                                    ))
                                    .with_children(|p| {
                                        p.spawn((PanelText::ToolName, pill_text(fonts, "PAINT", 9.5)));
                                        p.spawn(glyph(Glyph::Caret, Tone::Teal, 9.0));
                                    });
                                spawn_menu(anchor, BarMenu::Tools, ui_color(back, 0.97), c(Tone::Label, 0.3), |m| {
                                    for (text, key, action) in tools.clone() {
                                        menu_item(m, fonts, &c, text, Some(key), action);
                                    }
                                });
                            });
                            f.spawn((ring_node(34.0), ActionButton(DrydockAction::Mirror), Tip::new("MIRROR"), ActionTip(Action::Mirror))).with_child(ring_glyph(Glyph::Mirror, 16.0));
                            f.spawn((ring_node(34.0), ActionButton(DrydockAction::Bevel), Tip::new("BEVEL"), ActionTip(Action::ToolBevel))).with_child(ring_glyph(Glyph::Bevel, 15.0));
                        });
                    // The status line over the voyage-ready line, each shortened when it doesn't fit (`fit_status`).
                    bb.spawn((
                        StatusArea,
                        Hot::default(),
                        Node {
                            flex_grow: 1.0,
                            flex_shrink: 1.0,
                            min_width: px(0),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            // Start, not stretch: a line's laid-out width is its full width (`fit_status`).
                            align_items: AlignItems::FlexStart,
                            row_gap: px(4),
                            margin: UiRect::horizontal(px(12)),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                    ))
                    .with_children(|s| {
                        for t in [PanelText::Status, PanelText::Voyage] {
                            s.spawn((t, label(fonts, "", 9.5, c(Tone::Teal, 1.0)), TextLayout::no_wrap(), Node { flex_shrink: 0.0, ..default() }));
                        }
                    });
                    for (g, size, action, tip, key) in [
                        (Glyph::Save, 16.0, DrydockAction::Save, "SAVE", Some(Action::Save)),
                        (Glyph::Blueprints, 16.0, DrydockAction::Blueprints, "BLUEPRINTS", None),
                        (Glyph::Export, 16.0, DrydockAction::Export, "EXPORT", None),
                    ] {
                        let ring = bb.spawn((ring_node(34.0), ActionButton(action), Tip::new(tip))).with_child(ring_glyph(g, size)).id();
                        if let Some(a) = key {
                            bb.commands().entity(ring).insert(ActionTip(a));
                        }
                    }
                    bb.spawn(Node::default()).with_children(|anchor| {
                        anchor.spawn((ring_node(34.0), ActionButton(DrydockAction::OpenMenu(BarMenu::More)), Tip::new("MORE"))).with_child(ring_glyph(Glyph::More, 16.0));
                        spawn_menu(anchor, BarMenu::More, ui_color(back, 0.97), c(Tone::Label, 0.3), |m| {
                            menu_item(m, fonts, &c, "NEW DESIGN", None, DrydockAction::Rare(Rare::NewDesign));
                            // The content frames: the design moves to the one picked (locked ones are dimmed).
                            m.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(5), padding: UiRect::new(px(14), px(8), px(3), px(3)), ..default() })
                                .with_children(|r| {
                                    r.spawn((label(fonts, "FRAME", 9.0, c(Tone::Dim, 1.0)), Node { margin: UiRect::right(px(6)), ..default() }));
                                    for f in content.frames.iter() {
                                        spawn_pill(r, fonts, &f.name.to_uppercase(), 8.5, 24.0, None, pill(Tone::Teal), ActionButton(DrydockAction::Rare(Rare::Frame(f.size))));
                                    }
                                });
                            menu_item(m, fonts, &c, "COPY SHARE CODE", None, DrydockAction::Rare(Rare::CopyCode));
                            let file = if cfg!(target_arch = "wasm32") { "DOWNLOAD .RON FILE" } else { "WRITE .RON FILE" };
                            menu_item(m, fonts, &c, file, None, DrydockAction::Rare(Rare::Download));
                            menu_item(m, fonts, &c, "IMPORT .RON FILE", None, DrydockAction::Rare(Rare::ImportFile));
                            // The evolution lab's setup with this design as its start (GDR-0092), while the lab is open.
                            if lab.enabled {
                                menu_item(m, fonts, &c, "EVOLVE THIS DESIGN", None, DrydockAction::Rare(Rare::Evolve));
                            }
                        });
                    });
                    bb.spawn(sep());
                    spawn_pill(bb, fonts, "BEHAVIOR", 9.5, 32.0, None, pill(Tone::Teal), ActionButton(DrydockAction::Behavior));
                    bb.spawn(Node { width: px(4), ..default() });
                    let play = UiIcon::Glyph { glyph: Glyph::Play, color: ui.teal };
                    spawn_pill(bb, fonts, "TEST-FLY", 12.0, 40.0, Some(play), Pill { primary: true, ..pill(Tone::Teal) }, (ActionButton(DrydockAction::TestFly), Tip::new("TEST-FLY"), ActionTip(Action::TestFlight)));
                });
        });
}

/// The ARMOR swatches: the armour block in every armour material research can unlock (the strip scrolls sideways).
fn spawn_swatches(lp: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, content: &Content) {
    let block = content.unit_hull_part(Shape::Rect);
    lp.spawn((
        SwatchStrip,
        ScrollPosition::default(),
        Scrollbar::horizontal().outside(),
        Node { flex_direction: FlexDirection::Row, column_gap: px(SWATCH_GAP), width: percent(100), flex_shrink: 0.0, overflow: Overflow::scroll_x(), ..default() },
    ))
    .with_children(|strip| {
        for (k, m) in armor_materials(content).enumerate() {
            let tip = Tip::new(material_tip(content, block, m));
            let key = Action::armour(k + 1);
            strip
                .spawn((
                    ActionButton(DrydockAction::Material(m.id.clone())),
                    Pill { bare: true, ..Pill::new(Tone::Teal) },
                    Hot::default(),
                    tip,
                    // The tile's own size; the ring is an outline around it (`Pill` colours an `Outline` when it has
                    // one). A border plus padding around the tile came out thicker on one side at a fractional UI
                    // scale, as each edge rounds to whole pixels on its own (Niklas, 14 Oct 2026: "the border seems to
                    // be off-center"); an outline is drawn around the rounded box, the same on every side.
                    Node {
                        width: px(SWATCH_TILE),
                        height: px(SWATCH_TILE),
                        flex_shrink: 0.0,
                        margin: UiRect::all(px(SWATCH_RING)),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    Outline { width: px(SWATCH_RING_W), offset: px(SWATCH_RING - SWATCH_RING_W), color: Color::NONE },
                    BorderColor::all(Color::NONE),
                    BackgroundColor(Color::NONE),
                ))
                .insert_if(ActionTip(key.unwrap_or(Action::Armour1)), || key.is_some())
                .with_children(|sw| {
                    let tile = UiIcon::Part { part: block.map_or(String::new(), |b| b.id.clone()), material: Some(m.id.clone()) };
                    sw.spawn((
                        SwatchTile(m.id.clone()),
                        tile,
                        Node { width: percent(100), height: percent(100), border_radius: BorderRadius::all(px(4)), ..default() },
                        // A light outline: the near-black armour tones are told apart by it.
                        Outline { width: px(1), offset: px(0), color: c(Tone::Number, 0.22) },
                    ))
                    .with_children(|t| {
                        if let Some(a) = Action::armour(k + 1) {
                            t.spawn((ActionLabel(a), label(fonts, "", 7.5, c(Tone::Number, 0.8)), Node { position_type: PositionType::Absolute, right: px(1), bottom: px(-1), ..default() }));
                        }
                    });
                });
        }
    });
}

/// A part's row in the palette: icon, name and subtitle, the NEW tag, how many are left (the texts, icon and tag
/// follow the state: `sync_rows`). A locked row keeps its part: `sync_pills` disables it, so a click opens its research.
pub(super) fn spawn_row(list: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, part: &str) -> Entity {
    list.spawn((
        PaletteRow(part.to_string()),
        ActionButton(DrydockAction::PickPart(part.to_string())),
        PartHover { part: part.to_string(), material: None, held: Held::Design },
        Pill { bare: true, ..Pill::new(Tone::Teal) },
        Hot::default(),
        Node {
            height: px(ROW_H),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            padding: UiRect::new(px(10), px(14), px(0), px(0)),
            column_gap: px(12),
            border: UiRect::all(px(1.2)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BorderColor::all(Color::NONE),
        BackgroundColor(Color::NONE),
    ))
    .with_children(|row| {
        row.spawn((RowIcon(part.to_string()), UiIcon::Part { part: String::new(), material: None }, Node { width: px(32), height: px(28), flex_shrink: 0.0, ..default() }));
        // Clipped to its column: a long `LOCKED · <node>` line would otherwise run into the list's glow room.
        row.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(3), flex_grow: 1.0, flex_shrink: 1.0, min_width: px(0), overflow: Overflow::clip_x(), ..default() }).with_children(|t| {
            t.spawn((RowText::Name(part.to_string()), prose(fonts, "", 13.5, c(Tone::Number, 1.0))));
            t.spawn((RowText::Sub(part.to_string()), tracked(fonts, "", 8.0, c(Tone::Dim, 1.0), 0.2), TextLayout::no_wrap()));
        });
        // The NEW tag (GDR-0065): a small gold-outlined pill.
        row.spawn((
            RowNew(part.to_string()),
            Node {
                display: Display::None,
                height: px(18),
                padding: UiRect::horizontal(px(8)),
                align_items: AlignItems::Center,
                border: UiRect::all(px(1.0)),
                border_radius: BorderRadius::MAX,
                flex_shrink: 0.0,
                ..default()
            },
            BorderColor::all(c(Tone::Gold, 0.85)),
        ))
        .with_child(label(fonts, "NEW", 7.5, c(Tone::Gold, 1.0)));
        row.spawn((RowText::Count(part.to_string()), prose(fonts, "", 16.0, c(Tone::Label, 1.0)), Node { flex_shrink: 0.0, ..default() }));
    })
    .id()
}

/// A bottom bar menu's plate over its anchor (it opens upward; aligned left for the tools, right for ⋯), filled by
/// `items`: text only, part icons would draw under it.
pub(super) fn spawn_menu(anchor: &mut ChildSpawnerCommands, menu: BarMenu, plate: Color, edge: Color, items: impl FnOnce(&mut ChildSpawnerCommands)) {
    let (left, right) = match menu {
        BarMenu::Tools => (px(0), Val::Auto),
        BarMenu::More => (Val::Auto, px(0)),
    };
    anchor
        .spawn((
            MenuPlate(menu),
            UiBlock,
            GlobalZIndex(MENU_Z),
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                left,
                right,
                bottom: percent(100),
                margin: UiRect::bottom(px(10)),
                min_width: px(200),
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                padding: UiRect::all(px(6)),
                border: UiRect::all(px(1.2)),
                border_radius: BorderRadius::all(px(14)),
                ..default()
            },
            BackgroundColor(plate),
            BorderColor::all(edge),
        ))
        .with_children(items);
}

/// A menu row: its name (and key) on a bare pill that sends `action`.
pub(super) fn menu_item(m: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, text: &str, key: Option<Action>, action: DrydockAction) {
    m.spawn((
        ActionButton(action),
        Pill { bare: true, ..Pill::new(Tone::Teal) },
        Hot::default(),
        Node {
            height: px(30),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: px(18),
            padding: UiRect::horizontal(px(14)),
            border: UiRect::all(px(1.2)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BorderColor::all(Color::NONE),
        BackgroundColor(Color::NONE),
    ))
    .with_children(|r| {
        r.spawn((pill_text(fonts, text, 9.5), TextLayout::no_wrap()));
        if let Some(key) = key {
            r.spawn((ActionLabel(key), number(fonts, "", 10.0, c(Tone::Dim, 1.0)), TextLayout::no_wrap()));
        }
    });
}

/// A text a pill colours with its state (Barlow medium, like `spawn_pill`'s).
pub(super) fn pill_text(fonts: &HudFonts, text: &str, size: f32) -> impl Bundle + use<> {
    (
        PillText,
        Text::new(text),
        TextFont { font: fonts.medium.clone().into(), font_size: FontSize::Px(size), ..default() },
        TextColor(Color::NONE),
        LetterSpacing::Px(size * 0.25),
    )
}

/// An armour swatch's tooltip: the material's name and the armour block's numbers in it (labels from content).
pub(super) fn material_tip(content: &Content, block: Option<&Part>, m: &data::Material) -> String {
    let mut s = m.name.to_uppercase();
    if let Some(block) = block {
        for l in content.part_stats(block, Some(m)).iter().filter(|l| ["mass", "hp", "build_points"].contains(&l.stat.as_str())) {
            let _ = write!(s, "  ·  {} {}", l.label.to_uppercase(), l.text.to_uppercase());
        }
    }
    s
}

pub(super) fn overlay_name(o: Overlay) -> &'static str {
    match o {
        Overlay::Heat => "HEAT",
        Overlay::Mass => "MASS",
        Overlay::Bonus => "BONUS",
        Overlay::Shields => "SHIELDS",
        Overlay::Arcs => "ARCS",
        Overlay::Blast => "BLAST",
    }
}
