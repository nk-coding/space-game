//! The screen's nodes (layout v2 of GDR-0082), spawned once at start: the palette in sections, the armour swatches,
//! the stats and the selected part, the bottom bar with its menus.

use super::*;

pub(in crate::drydock) fn spawn(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>, content: Res<GameContent>) {
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

    commands
        .spawn((
            DrydockRoot,
            // Opens and closes with a transition (`view::open_close`): the panels slide in from their edges.
            ScreenFx::closed(),
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
            Visibility::Hidden,
        ))
        .with_children(|root| {
            // Top left: the ring menu, the breadcrumb and the design's name.
            root.spawn((UiBlock, ScreenPart::new(Edge::Top, 0), Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(14), ..abs(Some(22.0), None, Some(14.0), None) }))
                .with_children(|tl| {
                    tl.spawn((ring_node(40.0), ActionButton(DrydockAction::Menu), Tip::new("SHIPYARD"))).with_children(|r| {
                        r.spawn(glyph(Glyph::Menu, Tone::Violet, 22.0));
                    });
                    tl.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() }).with_children(|col| {
                        col.spawn((PanelText::Breadcrumb, label(fonts, "", 10.0, c(Tone::Label, 1.0))));
                        crate::drydock::blueprints::spawn_title(col, fonts, c(Tone::Number, 1.0), c(Tone::Dim, 1.0), c(Tone::Label, 0.3));
                    });
                });

            // Top right: the overlays.
            root.spawn((UiBlock, ScreenPart::new(Edge::Top, 0), Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(5), ..abs(None, Some(20.0), Some(19.0), None) }))
                .with_children(|tr| {
                    tr.spawn((label(fonts, "OVERLAY", 9.0, c(Tone::Dim, 1.0)), Node { margin: UiRect::right(px(5)), ..default() }));
                    for o in Overlay::ALL {
                        let accent = if o == Overlay::Bonus { Tone::Gold } else { Tone::Teal };
                        spawn_pill(tr, fonts, overlay_name(o), 10.0, 30.0, None, pill(accent), ActionButton(DrydockAction::Overlay(o)));
                    }
                });

            // Left: search, armour, category chips, every part in sections.
            root.spawn((UiBlock, ScreenPart::new(Edge::Left, 1), Node { width: px(LEFT_W), flex_direction: FlexDirection::Column, row_gap: px(10), ..abs(Some(20.0), None, Some(74.0), Some(70.0)) }))
                .with_children(|lp| {
                    lp.spawn((
                        SearchField,
                        TextField { placeholder: "Search parts".into(), max_chars: 24, ..default() },
                        Hot::default(),
                        Tip::key("SEARCH PARTS", "/"),
                        Node {
                            height: px(36),
                            flex_shrink: 0.0,
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            padding: UiRect::horizontal(px(14)),
                            column_gap: px(10),
                            border: UiRect::all(px(1.2)),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BorderColor::all(c(Tone::Label, 0.3)),
                    ))
                    .with_children(|f| {
                        f.spawn(glyph(Glyph::Search, Tone::Label, 14.0));
                        f.spawn((field_text(fonts, 12.5), Node { flex_grow: 1.0, ..default() }));
                    });
                    // ARMOR · <name>, then the swatches: every armour material research can unlock (a locked one
                    // shows a lock; the keys 1–9 count every armour material, hidden ones too).
                    lp.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(8), margin: UiRect::new(px(4), px(0), px(4), px(0)), flex_shrink: 0.0, ..default() })
                        .with_children(|r| {
                            r.spawn(label(fonts, "ARMOR  ·", 9.0, c(Tone::Dim, 1.0)));
                            r.spawn((PanelText::Armor, label(fonts, "", 10.0, c(Tone::Number, 1.0))));
                        });
                    let block = content.unit_hull_part(Shape::Rect);
                    lp.spawn((
                        SwatchStrip,
                        ScrollPosition::default(),
                        Scrollbar::horizontal().outside().room(12),
                        Node { flex_direction: FlexDirection::Row, column_gap: px(SWATCH_GAP), width: percent(100), flex_shrink: 0.0, overflow: Overflow::scroll_x(), ..default() },
                    ))
                    .with_children(|strip| {
                        for (k, m) in armor_materials(content).enumerate() {
                            let tip = Tip { name: material_tip(content, block, m).into(), key: MATERIAL_KEYS.get(k).copied() };
                            strip
                                .spawn((
                                    ActionButton(DrydockAction::Material(m.id.clone())),
                                    Pill { bare: true, ..pill(Tone::Teal) },
                                    Hot::default(),
                                    tip,
                                    Node {
                                        width: px(SWATCH),
                                        height: px(SWATCH),
                                        flex_shrink: 0.0,
                                        padding: UiRect::all(px(2)),
                                        border: UiRect::all(px(1.2)),
                                        border_radius: BorderRadius::all(px(7)),
                                        ..default()
                                    },
                                    BorderColor::all(Color::NONE),
                                    BackgroundColor(Color::NONE),
                                ))
                                .with_children(|sw| {
                                    let tile = UiIcon::Part { part: block.map_or(String::new(), |b| b.id.clone()), material: Some(m.id.clone()) };
                                    sw.spawn((
                                        SwatchTile(m.id.clone()),
                                        tile,
                                        Node { width: percent(100), height: percent(100), border: UiRect::all(px(1)), border_radius: BorderRadius::all(px(4)), ..default() },
                                        // A light outline: the near-black armour tones are told apart by it.
                                        BorderColor::all(c(Tone::Number, 0.22)),
                                    ))
                                    .with_children(|t| {
                                        if let Some(key) = MATERIAL_KEYS.get(k) {
                                            t.spawn((label(fonts, key, 7.5, c(Tone::Number, 0.8)), Node { position_type: PositionType::Absolute, right: px(1), bottom: px(-1), ..default() }));
                                        }
                                    });
                                });
                        }
                    });
                    // The category chips: one line that scrolls sideways, faded at a scrolled edge.
                    lp.spawn(Node { width: percent(100), height: px(24), flex_shrink: 0.0, ..default() }).with_children(|w| {
                        w.spawn((
                            ChipLine,
                            ScrollPosition::default(),
                            Scrollbar::horizontal().outside(),
                            Node { flex_direction: FlexDirection::Row, column_gap: px(6), width: percent(100), height: percent(100), overflow: Overflow::scroll_x(), ..default() },
                        ))
                        .with_children(|cats| {
                            for cat in content.categories.iter() {
                                spawn_pill(cats, fonts, &cat.name.to_uppercase(), 8.5, 24.0, None, pill(Tone::Teal), ActionButton(DrydockAction::Category(cat.id.clone())));
                            }
                        });
                    });
                    lp.spawn((
                        PaletteList,
                        Scrollbar::vertical(),
                        ScrollRows::default(),
                        Node {
                            flex_direction: FlexDirection::Column,
                            flex_grow: 1.0,
                            flex_basis: px(0),
                            min_height: px(0),
                            row_gap: px(ROW_GAP),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                    ))
                    .with_children(|list| {
                        for k in 0..SLOTS {
                            list.spawn((Slot(k), Node { display: Display::None, flex_direction: FlexDirection::Column, flex_shrink: 0.0, ..default() })).with_children(|slot| {
                                slot.spawn((
                                    SlotHead(k),
                                    Node {
                                        height: px(HEAD_H),
                                        flex_direction: FlexDirection::Row,
                                        align_items: AlignItems::FlexEnd,
                                        justify_content: JustifyContent::SpaceBetween,
                                        padding: UiRect::new(px(6), px(12), px(0), px(5)),
                                        ..default()
                                    },
                                ))
                                .with_children(|h| {
                                    h.spawn((PanelText::HeadName(k), label(fonts, "", 9.0, c(Tone::Label, 1.0))));
                                    h.spawn((PanelText::HeadCount(k), label(fonts, "", 8.5, c(Tone::Dim, 1.0))));
                                });
                                spawn_row(slot, fonts, &c, k);
                            });
                        }
                    });
                });

            // Right: the stats grid and the selected part.
            root.spawn((UiBlock, ScreenPart::new(Edge::Right, 1), Node { width: px(292), flex_direction: FlexDirection::Column, ..abs(None, Some(20.0), Some(78.0), Some(70.0)) }))
                .with_children(|rp| {
                    rp.spawn(Node {
                        display: Display::Grid,
                        grid_template_columns: RepeatedGridTrack::flex(2, 1.0),
                        column_gap: px(20),
                        row_gap: px(16),
                        ..default()
                    })
                    .with_children(|g| {
                        for k in 0..MAIN_CELLS {
                            g.spawn((StatCellNode(k), Node { flex_direction: FlexDirection::Column, row_gap: px(7), ..default() })).with_children(|cell| {
                                cell.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, column_gap: px(8), ..default() })
                                    .with_children(|r| {
                                        r.spawn((PanelText::StatLabel(k), label(fonts, "", 9.0, c(Tone::Label, 1.0))));
                                        r.spawn((PanelText::StatValue(k), number(fonts, "", 16.0, c(Tone::Number, 1.0))));
                                    });
                                spawn_bar(cell, ValueBar { share: 0.0, tone: Tone::Gold }, StatBar(k));
                            });
                        }
                    });
                    // The compact block under it: smaller, no bars (top speed, turn, mass, build points).
                    rp.spawn(Node {
                        display: Display::Grid,
                        grid_template_columns: RepeatedGridTrack::flex(2, 1.0),
                        column_gap: px(20),
                        row_gap: px(9),
                        margin: UiRect::top(px(18)),
                        ..default()
                    })
                    .with_children(|g| {
                        for k in MAIN_CELLS..MAIN_CELLS + DETAIL_CELLS {
                            g.spawn((StatCellNode(k), Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, column_gap: px(8), ..default() }))
                                .with_children(|r| {
                                    r.spawn((PanelText::StatLabel(k), label(fonts, "", 8.0, c(Tone::Label, 1.0))));
                                    r.spawn((PanelText::StatValue(k), number(fonts, "", 12.5, c(Tone::Number, 1.0))));
                                });
                        }
                    });
                    rp.spawn((Node { height: px(1), margin: UiRect::vertical(px(14)), flex_shrink: 0.0, ..default() }, BackgroundColor(c(Tone::Label, 0.14))));
                    rp.spawn((label(fonts, "SELECTED PART", 9.0, c(Tone::Label, 1.0)), Node { margin: UiRect::bottom(px(10)), flex_shrink: 0.0, ..default() }));
                    rp.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(16), flex_shrink: 0.0, ..default() }).with_children(|h| {
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
                            ScrollPosition::default(),
                            Scrollbar::vertical().room(0),
                            Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_basis: px(0), min_height: px(0), overflow: Overflow::scroll_y(), ..default() },
                        ))
                        .with_children(|sb| {
                            sb.spawn((PanelText::SelSummary, prose(fonts, "", 11.5, c(Tone::Label, 1.0)), Node { margin: UiRect::top(px(12)), flex_shrink: 0.0, ..default() }));
                            sb.spawn(Node {
                                display: Display::Grid,
                                grid_template_columns: RepeatedGridTrack::flex(3, 1.0),
                                column_gap: px(12),
                                row_gap: px(8),
                                margin: UiRect::top(px(14)),
                                flex_shrink: 0.0,
                                ..default()
                            })
                            .with_children(|g| {
                                for k in 0..SEL_STATS {
                                    g.spawn((SelStat(k), Node { flex_direction: FlexDirection::Column, row_gap: px(4), ..default() })).with_children(|cell| {
                                        cell.spawn((PanelText::SelStatLabel(k), label(fonts, "", 8.0, c(Tone::Label, 1.0))));
                                        cell.spawn((PanelText::SelStatValue(k), number(fonts, "", 14.0, c(Tone::Number, 1.0))));
                                    });
                                }
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
            root.spawn(Node { flex_direction: FlexDirection::Column, ..abs(Some(290.0), Some(332.0), Some(84.0), Some(66.0)) }).with_children(|ga| {
                // The info line moves, the grid viewport never (the camera centres on it).
                ga.spawn((GridViewport, Node { flex_grow: 1.0, ..default() }));
                ga.spawn((ScreenPart::new(Edge::Bottom, 2), Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, height: px(30), flex_shrink: 0.0, ..default() }))
                    .with_children(|b| {
                        b.spawn((PanelText::Info, label(fonts, "", 9.0, c(Tone::Dim, 1.0)))).with_child(crate::drydock::ghost::hint_span(fonts, &style));
                        b.spawn((UiBlock, Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(10), ..default() })).with_children(|z| {
                            z.spawn((ring_node(28.0), ActionButton(DrydockAction::ZoomOut), Tip::key("ZOOM OUT", "−"))).with_child(ring_glyph(Glyph::Minus, 13.0));
                            z.spawn((PanelText::Zoom, number(fonts, "", 13.0, c(Tone::Number, 1.0)), Node { min_width: px(46), ..default() }));
                            z.spawn((ring_node(28.0), ActionButton(DrydockAction::ZoomIn), Tip::key("ZOOM IN", "+"))).with_child(ring_glyph(Glyph::Plus, 13.0));
                        });
                    });
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
            root.spawn((UiBlock, ScreenPart::new(Edge::Bottom, 3), Node { height: px(42), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(6), ..abs(Some(22.0), Some(20.0), None, Some(14.0)) }))
                .with_children(|bb| {
                    for (g, action, tip) in [(Glyph::Undo, DrydockAction::Undo, Tip::key("UNDO", "CTRL+Z")), (Glyph::Redo, DrydockAction::Redo, Tip::key("REDO", "CTRL+SHIFT+Z"))] {
                        bb.spawn((ring_node(36.0), ActionButton(action), tip)).with_children(|r| {
                            r.spawn(glyph(g, Tone::Number, 19.0));
                        });
                    }
                    bb.spawn(sep());
                    let tools = [
                        ("PAINT", "B", DrydockAction::Tool(Tool::Paint)),
                        ("FILL", "G", DrydockAction::Tool(Tool::Fill)),
                        ("ERASE", "X", DrydockAction::Tool(Tool::Erase)),
                        ("SELECT", "V", DrydockAction::Tool(Tool::Select)),
                        ("ROTATE", "R · Q / E", DrydockAction::RotateCw),
                    ];
                    // Wide: the tool pills, mirror and bevel.
                    bb.spawn((WideTools, Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(6), flex_shrink: 0.0, ..default() })).with_children(|w| {
                        for (text, key, action) in tools.clone() {
                            spawn_pill(w, fonts, text, 9.5, 30.0, None, pill(Tone::Teal), (ActionButton(action), Tip::key(text, key)));
                        }
                        w.spawn(sep());
                        spawn_pill(w, fonts, "MIRROR", 9.5, 30.0, None, pill(Tone::Teal), (ActionButton(DrydockAction::Mirror), Tip::key("MIRROR", "M")));
                        spawn_pill(w, fonts, "BEVEL", 9.5, 30.0, None, pill(Tone::Teal), (ActionButton(DrydockAction::Bevel), Tip::key("BEVEL", "SHIFT+B")));
                    });
                    // Folded: the current tool as a dropdown (its menu opens upward), mirror and bevel as icons.
                    bb.spawn((FoldedTools, Node { display: Display::None, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(6), flex_shrink: 0.0, ..default() }))
                        .with_children(|f| {
                            f.spawn(Node::default()).with_children(|anchor| {
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
                            f.spawn((ring_node(34.0), ActionButton(DrydockAction::Mirror), Tip::key("MIRROR", "M"))).with_child(ring_glyph(Glyph::Mirror, 16.0));
                            f.spawn((ring_node(34.0), ActionButton(DrydockAction::Bevel), Tip::key("BEVEL", "SHIFT+B"))).with_child(ring_glyph(Glyph::Bevel, 15.0));
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
                    for (g, size, action, tip) in [
                        (Glyph::Save, 16.0, DrydockAction::Save, Tip::key("SAVE", "CTRL+S")),
                        (Glyph::Blueprints, 16.0, DrydockAction::Blueprints, Tip::new("BLUEPRINTS")),
                        (Glyph::Export, 16.0, DrydockAction::Export, Tip::new("EXPORT")),
                    ] {
                        bb.spawn((ring_node(34.0), ActionButton(action), tip)).with_child(ring_glyph(g, size));
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
                        });
                    });
                    bb.spawn(sep());
                    spawn_pill(bb, fonts, "BEHAVIOR", 9.5, 32.0, None, pill(Tone::Teal), ActionButton(DrydockAction::Behavior));
                    bb.spawn(Node { width: px(4), ..default() });
                    let play = UiIcon::Glyph { glyph: Glyph::Play, color: ui.teal };
                    spawn_pill(bb, fonts, "TEST-FLY", 12.0, 40.0, Some(play), Pill { primary: true, ..pill(Tone::Teal) }, (ActionButton(DrydockAction::TestFly), Tip::key("TEST-FLY", "T")));
                });
        });
}

/// One slot's part row: icon, name and subtitle, the NEW tag, how many are left.
pub(super) fn spawn_row(slot: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, k: usize) {
    slot.spawn((
        PaletteRow(k),
        ActionButton(DrydockAction::PickPart(String::new())),
        Pill { bare: true, ..Pill::new(Tone::Teal) },
        Hot::default(),
        Node {
            display: Display::None,
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
        row.spawn((RowIcon(k), UiIcon::Part { part: String::new(), material: None }, Node { width: px(32), height: px(28), flex_shrink: 0.0, ..default() }));
        // Clipped to its column: a long `LOCKED · <node>` line would otherwise run into the list's glow room.
        row.spawn(Node { flex_direction: FlexDirection::Column, row_gap: px(3), flex_grow: 1.0, flex_shrink: 1.0, min_width: px(0), overflow: Overflow::clip_x(), ..default() }).with_children(|t| {
            t.spawn((PanelText::RowName(k), prose(fonts, "", 13.5, c(Tone::Number, 1.0))));
            t.spawn((PanelText::RowSub(k), tracked(fonts, "", 8.0, c(Tone::Dim, 1.0), 0.2), TextLayout::no_wrap()));
        });
        // The NEW tag (GDR-0065): a small gold-outlined pill.
        row.spawn((
            RowNew(k),
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
        row.spawn((PanelText::RowCount(k), prose(fonts, "", 16.0, c(Tone::Label, 1.0)), Node { flex_shrink: 0.0, ..default() }));
    });
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
pub(super) fn menu_item(m: &mut ChildSpawnerCommands, fonts: &HudFonts, c: &impl Fn(Tone, f32) -> Color, text: &str, key: Option<&str>, action: DrydockAction) {
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
            r.spawn((number(fonts, key, 10.0, c(Tone::Dim, 1.0)), TextLayout::no_wrap()));
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
    }
}
