//! The widgets: pills (plain, active teal or gold, primary, disabled, locked), round ring buttons, value bars, corner
//! brackets; and how their state and the pointer turn into colours ([`UiStyleSet`]).

use super::*;

// ---------------------------------------------------------------------------------------------------------
// Widgets

/// A pill button. The screen sets `active` and `disabled`; the kit colours it (in [`UiStyleSet`]).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Pill {
    /// The colour of the active state (teal for selections, gold for bonuses).
    pub accent: Tone,
    pub active: bool,
    pub disabled: bool,
    /// The primary action: always glowing teal.
    pub primary: bool,
    /// No outline unless active or hovered (list rows).
    pub bare: bool,
}

impl Pill {
    pub fn new(accent: Tone) -> Self {
        Self { accent, active: false, disabled: false, primary: false, bare: false }
    }
}

/// Text inside a [`Pill`] or a [`Ring`]: coloured with its state.
#[derive(Component)]
pub struct PillText;

/// A pill for a choice that may be locked (an armour material, a frame): its lead icon (a lock) shows only while
/// the pill is disabled.
#[derive(Component)]
pub struct PillLock;

/// A round ring button (menus, undo / redo, zoom, icon buttons); `active` lights it teal like an active pill (a
/// mode that is on).
#[derive(Component, Clone, Copy, Debug, PartialEq, Default)]
pub struct Ring {
    pub disabled: bool,
    pub active: bool,
}

/// A vector glyph inside a [`Ring`] coloured with the ring's state, like a [`PillText`] (the zoom's and the
/// steppers' plus and minus): spawn it with [`ring_glyph`].
#[derive(Component)]
pub struct RingGlyph;

/// A [`RingGlyph`] `size` px across: centred in its ring by the ring's own layout, and drawn centred on its node.
pub fn ring_glyph(glyph: Glyph, size: f32) -> impl Bundle {
    (RingGlyph, UiIcon::Glyph { glyph, color: Color::NONE }, Node { width: px(size), height: px(size), flex_shrink: 0.0, ..default() })
}

/// Spawns a pill with a text in Barlow medium (the mockup's pills are semi-bold) and an optional icon before it,
/// and returns it; `extra` goes on the pill (an action, a marker).
#[allow(clippy::too_many_arguments)]
pub fn spawn_pill(
    parent: &mut ChildSpawnerCommands,
    fonts: &HudFonts,
    text: &str,
    size: f32,
    height: f32,
    lead: Option<UiIcon>,
    pill: Pill,
    extra: impl Bundle,
) -> Entity {
    let spacing = size * 0.25;
    parent
        .spawn((
            pill,
            Hot::default(),
            Node {
                height: px(height),
                padding: UiRect::new(px(height * 0.38 + spacing), px(height * 0.38), px(0), px(0)),
                border: UiRect::all(px(1.2)),
                border_radius: BorderRadius::MAX,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: px(height * 0.3),
                flex_shrink: 0.0,
                ..default()
            },
            BorderColor::all(Color::NONE),
            BackgroundColor(Color::NONE),
            extra,
        ))
        .with_children(|p| {
            if let Some(icon) = lead {
                p.spawn((icon, Node { width: px(size), height: px(size), flex_shrink: 0.0, ..default() }));
            }
            p.spawn((
                PillText,
                Text::new(text),
                TextFont { font: fonts.medium.clone().into(), font_size: FontSize::Px(size), ..default() },
                TextColor(Color::NONE),
                LetterSpacing::Px(spacing),
            ));
        })
        .id()
}

/// Spawns a ring button of `diameter` px; fill it with a glyph icon or a text child.
pub fn ring_node(diameter: f32) -> impl Bundle {
    (
        Ring::default(),
        Hot::default(),
        Node {
            width: px(diameter),
            height: px(diameter),
            border: UiRect::all(px(1.2)),
            border_radius: BorderRadius::MAX,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_shrink: 0.0,
            ..default()
        },
        BorderColor::all(Color::NONE),
        BackgroundColor(Color::NONE),
    )
}

/// A thin value bar: a 2 px track with a fill of `share` (0..1) in `tone`. A share over 1 (over a limit, e.g.
/// a power deficit) fills the whole track and a notch marks where 100 % is (at `1 / share` of the width).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ValueBar {
    pub share: f32,
    pub tone: Tone,
}

#[derive(Component)]
pub(super) struct BarFill;

/// The 100 % notch of an overfull [`ValueBar`].
#[derive(Component)]
pub(super) struct BarMark;

/// Spawns a value bar that fills its parent's width.
pub fn spawn_bar(parent: &mut ChildSpawnerCommands, bar: ValueBar, extra: impl Bundle) -> Entity {
    parent
        .spawn((bar, Node { width: percent(100), height: px(2), ..default() }, BackgroundColor(Color::NONE), extra))
        .with_children(|t| {
            t.spawn((BarFill, Node { width: percent(0), height: percent(100), ..default() }, BackgroundColor(Color::NONE)));
            let mark = Node { display: Display::None, position_type: PositionType::Absolute, top: px(-2), width: px(2), height: px(6), ..default() };
            t.spawn((BarMark, mark, BackgroundColor(Color::NONE)));
        })
        .id()
}

/// Four thin corner brackets inside `parent` (it must be positioned: they are absolute).
pub fn spawn_brackets(parent: &mut ChildSpawnerCommands, arm: f32, width: f32, color: Color) {
    let b = px(width);
    let corners = [
        (UiRect { left: b, top: b, ..default() }, Some(0.0), None, Some(0.0), None),
        (UiRect { right: b, top: b, ..default() }, None, Some(0.0), Some(0.0), None),
        (UiRect { left: b, bottom: b, ..default() }, Some(0.0), None, None, Some(0.0)),
        (UiRect { right: b, bottom: b, ..default() }, None, Some(0.0), None, Some(0.0)),
    ];
    for (border, l, r, t, bt) in corners {
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: l.map(px).unwrap_or(Val::Auto),
                right: r.map(px).unwrap_or(Val::Auto),
                top: t.map(px).unwrap_or(Val::Auto),
                bottom: bt.map(px).unwrap_or(Val::Auto),
                width: px(arm),
                height: px(arm),
                border,
                ..default()
            },
            BorderColor::all(color),
        ));
    }
}

/// The lock of a [`PillLock`] pill shows while the pill is disabled.
#[allow(clippy::type_complexity)]
pub(super) fn show_locks(pills: Query<(&Pill, &Children), (With<PillLock>, Changed<Pill>)>, mut icons: Query<&mut Node, With<UiIcon>>) {
    for (pill, children) in &pills {
        let d = if pill.disabled { Display::Flex } else { Display::None };
        for child in children {
            if let Ok(mut n) = icons.get_mut(*child)
                && n.display != d
            {
                n.display = d;
            }
        }
    }
}

/// Colours of pills, rings, value bars and text fields from their state and the pointer.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn style_widgets(
    style: Res<Style>,
    time: Res<Time<Real>>,
    mut pills: Query<
        (Ref<Pill>, Ref<Hot>, &mut BorderColor, &mut BackgroundColor, Option<&mut BoxShadow>, &Children, Entity),
        Without<Ring>,
    >,
    mut rings: Query<
        (Ref<Ring>, Ref<Hot>, &mut BorderColor, &mut BackgroundColor, &Children),
        Without<Pill>,
    >,
    mut texts: Query<&mut TextColor, With<PillText>>,
    mut glyphs: Query<&mut UiIcon, With<RingGlyph>>,
    mut bars: Query<(Ref<ValueBar>, &mut BackgroundColor, &Children), (Without<Pill>, Without<Ring>, Without<BarFill>)>,
    mut fills: Query<(&mut Node, &mut BackgroundColor, Has<BarMark>), (Or<(With<BarFill>, With<BarMark>)>, Without<ValueBar>, Without<Pill>, Without<Ring>)>,
    mut fields: Query<(Ref<TextField>, Ref<Hot>, &mut BorderColor, &Children), (Without<Pill>, Without<Ring>)>,
    mut field_texts: Query<(&mut Text, &mut TextColor, &Children), (With<TextFieldText>, Without<PillText>, Without<FieldSpan>)>,
    mut field_spans: Query<(&FieldSpan, &mut TextSpan, &mut TextColor, &mut TextBackgroundColor), (Without<TextFieldText>, Without<PillText>)>,
    mut commands: Commands,
    mut blink: Local<bool>,
) {
    let ui = &style.ui;
    let c = ui_color;
    for (pill, hot, mut border, mut bg, shadow, children, e) in &mut pills {
        if !pill.is_changed() && !hot.is_changed() {
            continue;
        }
        let accent = pill.accent.color(ui);
        let (b, f, t, glow_alpha) = if pill.disabled {
            (if pill.bare { Color::NONE } else { c(ui.dim, 0.3) }, Color::NONE, c(ui.dim, 0.75), 0.0)
        } else if pill.primary {
            let k = if hot.pressed { 0.2 } else if hot.hovered { 0.13 } else { 0.07 };
            (c(ui.teal, 0.95), c(ui.teal, k), c(ui.teal, 1.0), 0.35)
        } else if pill.active {
            let k = if hot.pressed { 0.2 } else if hot.hovered { 0.14 } else { 0.08 };
            (c(accent, 0.85), c(accent, k), c(ui.number, 0.95), 0.18)
        } else {
            let fill = if hot.pressed { c(ui.number, 0.1) } else if hot.hovered && pill.bare { c(ui.number, 0.04) } else { Color::NONE };
            let edge = match (pill.bare, hot.hovered) {
                (true, false) => Color::NONE,
                (true, true) => c(ui.label, 0.3),
                (false, false) => c(ui.label, 0.35),
                (false, true) => c(ui.label, 0.75),
            };
            (edge, fill, if hot.hovered { c(ui.number, 0.9) } else { c(ui.label, 1.0) }, 0.0)
        };
        if border.top != b {
            *border = BorderColor::all(b);
        }
        if bg.0 != f {
            bg.0 = f;
        }
        let glow_color = if pill.primary { ui.teal } else { accent };
        match (shadow, glow_alpha > 0.0) {
            (Some(mut s), true) => *s = glow(glow_color, glow_alpha),
            (Some(_), false) => {
                commands.entity(e).remove::<BoxShadow>();
            }
            (None, true) => {
                commands.entity(e).insert(glow(glow_color, glow_alpha));
            }
            (None, false) => {}
        }
        for child in children {
            if let Ok(mut tc) = texts.get_mut(*child) {
                set_color(&mut tc, t);
            }
        }
    }
    for (ring, hot, mut border, mut bg, children) in &mut rings {
        if !ring.is_changed() && !hot.is_changed() {
            continue;
        }
        // The text colour as a palette colour and its opacity (glyphs are world meshes: no `ui_color`).
        let (b, f, (ink, k)) = if ring.disabled {
            (c(ui.dim, 0.3), Color::NONE, (ui.dim, 0.75))
        } else if ring.active {
            let k = if hot.pressed { 0.2 } else if hot.hovered { 0.14 } else { 0.08 };
            (c(ui.teal, 0.85), c(ui.teal, k), (ui.teal, 1.0))
        } else if hot.pressed {
            (c(ui.teal, 0.8), c(ui.teal, 0.12), (ui.number, 1.0))
        } else if hot.hovered {
            (c(ui.label, 0.8), c(ui.number, 0.04), (ui.number, 1.0))
        } else {
            (c(ui.label, 0.4), Color::NONE, (ui.number, 0.85))
        };
        let t = c(ink, k);
        if border.top != b {
            *border = BorderColor::all(b);
        }
        if bg.0 != f {
            bg.0 = f;
        }
        for child in children {
            if let Ok(mut tc) = texts.get_mut(*child) {
                set_color(&mut tc, t);
            }
            if let Ok(mut icon) = glyphs.get_mut(*child)
                && let UiIcon::Glyph { color, .. } = &mut *icon
                && *color != ink.with_alpha(k)
            {
                *color = ink.with_alpha(k);
            }
        }
    }
    for (bar, mut track, children) in &mut bars {
        if !bar.is_changed() {
            continue;
        }
        let t = c(ui.dim, 0.35);
        if track.0 != t {
            track.0 = t;
        }
        for child in children {
            let Ok((mut node, mut fill, mark)) = fills.get_mut(*child) else { continue };
            if mark {
                let over = bar.share > 1.0;
                let (display, left) = if over { (Display::Flex, percent((100.0 / bar.share).round())) } else { (Display::None, percent(100)) };
                if node.display != display || node.left != left {
                    node.display = display;
                    node.left = left;
                }
                let f = c(ui.number, 0.9);
                if fill.0 != f {
                    fill.0 = f;
                }
                continue;
            }
            let w = percent((bar.share.clamp(0.0, 1.0) * 100.0).round());
            if node.width != w {
                node.width = w;
            }
            let f = c(bar.tone.color(ui), 0.9);
            if fill.0 != f {
                fill.0 = f;
            }
        }
    }
    // The caret (teal, like the TEXT view's) blinks at 1 Hz; the selection is lit teal; placeholders are dim.
    // Texts are rewritten only when a field changes, colours when the caret blinks.
    let caret = time.elapsed_secs().fract() < 0.55;
    let blinked = caret != *blink;
    *blink = caret;
    for (field, hot, mut border, children) in &mut fields {
        if field.is_changed() || hot.is_changed() {
            let b = if field.focused {
                c(ui.teal, 0.75)
            } else if hot.hovered {
                c(ui.label, 0.6)
            } else {
                c(ui.label, 0.3)
            };
            if border.top != b {
                *border = BorderColor::all(b);
            }
        }

        if !field.is_changed() && !(field.focused && blinked) {
            continue;
        }
        let placeholder = field.value.is_empty() && !field.focused;
        let parts = if placeholder { [field.placeholder.as_str(), "", "", "", ""] } else { field_parts(&field.value, field.caret, field.focused) };
        let ink = if placeholder { c(ui.dim, 1.0) } else { c(ui.number, 0.95) };
        for child in children {
            let Ok((mut text, mut color, spans)) = field_texts.get_mut(*child) else { continue };
            set_text(&mut text, parts[0]);
            set_color(&mut color, ink);
            for span in spans {
                let Ok((k, mut s, mut color, mut back)) = field_spans.get_mut(*span) else { continue };
                let part = parts[usize::from(k.0)];
                if s.0 != part {
                    s.0.clear();
                    s.0.push_str(part);
                }
                let (col, bg) = match k.0 {
                    1 | 3 => (c(ui.teal, if caret { 1.0 } else { 0.0 }), Color::NONE),
                    2 => (c(ui.number, 1.0), c(ui.teal, 0.3)),
                    _ => (ink, Color::NONE),
                };
                set_color(&mut color, col);
                if back.0 != bg {
                    back.0 = bg;
                }
            }
        }
    }
}
