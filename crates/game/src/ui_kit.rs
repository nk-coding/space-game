//! A small widget kit for bevy_ui screens in the reference look (style guide "UI", GDR-0008): wide-tracked
//! uppercase labels, numbers, pill buttons (plain, active teal, active gold, the glowing primary, disabled),
//! round ring buttons (plain, or glowing in an accent colour: [`RingAccent`]), thin value bars, corner brackets and
//! a one-line text field. Written for the drydock (12 Oct 2026); any screen can use it.
//!
//! * **Hit testing** is done here by hand from the laid-out node rectangles (bevy_picking is not compiled and
//!   `Interaction` is deprecated in Bevy 0.20): every node with [`Hot`] gets its hover and press state each
//!   frame (the topmost node under the pointer wins), and a press and release on the same node is a
//!   [`UiClick`]. Mouse and touch both work.
//! * [`UiPointer`] tells the world whether the pointer is on the UI ([`UiBlock`] panels or any [`Hot`] node)
//!   and whether a text field has the keyboard, so screens keep clicks, wheel and keys out of the world then.
//! * Widgets carry their state in components ([`Pill`], [`TextField`], [`ValueBar`]); the screen writes the
//!   state, the kit turns state + pointer into colours ([`UiStyleSet`]), writing only what changed.
//! * Colours go through [`ui_color`] (the world composites in sRGB, bevy_ui does not encode for it).
//! * Glyphs that Barlow lacks (arrows, the play triangle, the search lens) are vector icons (`ui_icons`).

use bevy::input::ButtonState;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::text::LetterSpacing;
use bevy::ui::{CalculatedClip, ComputedNode, ComputedStackIndex, UiGlobalTransform};
use bevy::window::PrimaryWindow;

use crate::flight_hud::HudFonts;
use crate::style::{Style, UiPalette, ui_color};
use crate::ui_icons::UiIcon;

/// The palette's colours by role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tone {
    Teal,
    Gold,
    Coral,
    Violet,
    /// White numbers and names.
    #[default]
    Number,
    /// Grey labels.
    Label,
    Dim,
}

impl Tone {
    pub fn color(self, ui: &UiPalette) -> Color {
        match self {
            Tone::Teal => ui.teal,
            Tone::Gold => ui.gold,
            Tone::Coral => ui.coral,
            Tone::Violet => ui.violet,
            Tone::Number => ui.number,
            Tone::Label => ui.label,
            Tone::Dim => ui.dim,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Text

/// A wide-tracked label (pass it uppercase): Barlow regular, letter spacing 0.3 em.
pub fn label(fonts: &HudFonts, text: &str, size: f32, color: Color) -> impl Bundle + use<> {
    tracked(fonts, text, size, color, 0.3)
}

/// A label with its own letter spacing (in em), for long lines in narrow places.
pub fn tracked(fonts: &HudFonts, text: &str, size: f32, color: Color, em: f32) -> impl Bundle + use<> {
    (
        Text::new(text),
        TextFont { font: fonts.regular.clone().into(), font_size: FontSize::Px(size), ..default() },
        TextColor(color),
        LetterSpacing::Px(size * em),
    )
}

/// A number: Barlow medium, slightly tracked.
pub fn number(fonts: &HudFonts, text: &str, size: f32, color: Color) -> impl Bundle + use<> {
    (
        Text::new(text),
        TextFont { font: fonts.medium.clone().into(), font_size: FontSize::Px(size), ..default() },
        TextColor(color),
        LetterSpacing::Px(size * 0.06),
    )
}

/// Names and prose: Barlow regular, plain spacing.
pub fn prose(fonts: &HudFonts, text: &str, size: f32, color: Color) -> impl Bundle + use<> {
    (
        Text::new(text),
        TextFont { font: fonts.regular.clone().into(), font_size: FontSize::Px(size), ..default() },
        TextColor(color),
        LetterSpacing::Px(size * 0.02),
    )
}

/// The soft glow behind a glowing number (bevy_ui text has no blur; a blurred box shadow does the job).
pub fn glow(color: Color, alpha: f32) -> BoxShadow {
    BoxShadow::new(ui_color(color, alpha), px(0), px(0), px(-2), px(10))
}

/// Replaces a text only when it differs (no needless re-layout).
pub fn set_text(text: &mut Text, s: &str) {
    if text.0 != s {
        text.0.clear();
        text.0.push_str(s);
    }
}

/// Sets a text colour only when it differs.
pub fn set_color(color: &mut TextColor, c: Color) {
    if color.0 != c {
        color.0 = c;
    }
}

// ---------------------------------------------------------------------------------------------------------
// Pointer

/// Pointer state of an interactive node, updated every frame (in [`UiInputSet`]).
#[derive(Component, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hot {
    pub hovered: bool,
    /// A press that started on this node is still held.
    pub pressed: bool,
}

/// A press and a release on the same [`Hot`] node (mouse or touch).
#[derive(Message, Clone, Copy, Debug)]
pub struct UiClick(pub Entity);

/// A node that keeps the pointer from the world (a panel), whether or not anything in it is interactive.
#[derive(Component)]
pub struct UiBlock;

/// Where the pointer is, for the world's input systems.
#[derive(Resource, Default, Debug)]
pub struct UiPointer {
    /// The mouse is over the UI, or a press that started on the UI is held: the world ignores it.
    pub blocked: bool,
    /// A text field has the keyboard: shortcuts are off.
    pub typing: bool,
    pressed: Option<(Entity, PressSource)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PressSource {
    Mouse,
    Touch(u64),
}

/// Runs in `Update` before any screen input: hover, press, clicks, text field focus.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct UiInputSet;

/// Runs in `Update` after the screens have written their widget state: colours.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct UiStyleSet;

pub struct UiKitPlugin;

impl Plugin for UiKitPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiPointer>()
            .add_message::<UiClick>()
            .configure_sets(Update, UiStyleSet.after(UiInputSet))
            .add_systems(Update, (hit_test, type_into_fields).chain().in_set(UiInputSet))
            .add_systems(Update, style_widgets.in_set(UiStyleSet));
    }
}

/// Hover, press and click of every visible [`Hot`] node; focus of text fields; [`UiPointer`].
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn hit_test(
    window: Single<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    mut pointer: ResMut<UiPointer>,
    mut hots: Query<(Entity, &mut Hot, &ComputedNode, &UiGlobalTransform, &InheritedVisibility, Option<&ComputedStackIndex>, Option<&CalculatedClip>)>,
    blocks: Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility), With<UiBlock>>,
    mut fields: Query<(Entity, &mut TextField)>,
    mut clicks: MessageWriter<UiClick>,
) {
    let scale = window.scale_factor();
    let topmost = |at: Vec2| {
        let mut best: Option<(u32, Entity)> = None;
        for (e, _, node, tf, vis, stack, clip) in &hots {
            // A node scrolled out of a clipping parent (a scrolled list) is not under the pointer.
            if vis.get() && node.size.cmpgt(Vec2::ZERO).all() && node.contains_point(*tf, at) && clip.is_none_or(|c| c.contains_point(at)) {
                let z = stack.map_or(0, |s| s.0);
                if best.is_none_or(|b| z >= b.0) {
                    best = Some((z, e));
                }
            }
        }
        best.map(|b| b.1)
    };
    let cursor = window.physical_cursor_position();
    let over = cursor.and_then(topmost);

    // Presses: the mouse, then the first new touch.
    let mut press: Option<(Option<Entity>, PressSource)> = None;
    if buttons.just_pressed(MouseButton::Left) {
        press = Some((over, PressSource::Mouse));
    } else if let Some(t) = touches.iter_just_pressed().next() {
        press = Some((topmost(t.position() * scale), PressSource::Touch(t.id())));
    }
    if let Some((on, source)) = press {
        pointer.pressed = on.map(|e| (e, source));
        // A press focuses the text field under it and takes the focus from every other.
        for (e, mut f) in &mut fields {
            let want = on == Some(e);
            if f.focused != want {
                f.focused = want;
            }
        }
    }
    // Releases: a click when the pointer is still on the pressed node.
    if let Some((e, source)) = pointer.pressed {
        let released = match source {
            PressSource::Mouse => buttons.just_released(MouseButton::Left).then_some(over),
            PressSource::Touch(id) => touches
                .iter_just_released()
                .chain(touches.iter_just_canceled())
                .find(|t| t.id() == id)
                .map(|t| topmost(t.position() * scale)),
        };
        if let Some(at) = released {
            if at == Some(e) {
                clicks.write(UiClick(e));
            }
            pointer.pressed = None;
        }
    }
    let pressed = pointer.pressed.map(|p| p.0);
    for (e, mut hot, ..) in &mut hots {
        let want = Hot { hovered: over == Some(e), pressed: pressed == Some(e) };
        if *hot != want {
            *hot = want;
        }
    }
    let on_block = cursor.is_some_and(|c| {
        blocks.iter().any(|(node, tf, vis)| vis.get() && node.size.cmpgt(Vec2::ZERO).all() && node.contains_point(*tf, c))
    });
    pointer.blocked = over.is_some() || on_block || pressed.is_some();
    pointer.typing = fields.iter().any(|(_, f)| f.focused);
}

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

/// A round ring button (menus, undo / redo, zoom).
#[derive(Component, Clone, Copy, Debug, PartialEq, Default)]
pub struct Ring {
    pub disabled: bool,
}

/// A glowing ring button in an accent colour (the home screen's ring menu, GDR-0008): the outline in the accent
/// with a soft glow, brighter on hover. Disabled, it is dim like any ring.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct RingAccent(pub Tone);

/// Spawns a pill with a text (and an icon before it) and returns it; `extra` goes on the pill (an action, a
/// marker).
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
                TextFont { font: fonts.regular.clone().into(), font_size: FontSize::Px(size), ..default() },
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
struct BarFill;

/// The 100 % notch of an overfull [`ValueBar`].
#[derive(Component)]
struct BarMark;

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

/// The screens (the drydock, the BEHAVIOR screen) are laid out for 1280 × 720 logical px; larger windows scale
/// them up to this.
const MAX_UI_SCALE: f32 = 2.0;

/// The UI scale for a window: min(w / 1280, h / 720), 1 to [`MAX_UI_SCALE`].
pub fn ui_scale_for(window: &Window) -> f32 {
    (window.width() / 1280.0).min(window.height() / 720.0).clamp(1.0, MAX_UI_SCALE)
}

/// Pixels of a touchpad's wheel that count as one line (one list row, 40 px of a scrolled area).
pub const WHEEL_LINE_PX: f32 = 40.0;

/// The wheel's turn this frame in list rows: whole rows, at least one per turn, signed (positive: up).
pub fn wheel_rows(scroll: &AccumulatedMouseScroll) -> f32 {
    let steps = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / WHEEL_LINE_PX,
    };
    steps.signum() * steps.abs().max(1.0).round()
}

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
        field.focused = true;
        out.0 = Some(true);
    }
    out
}

/// A one-line text field (search). The text lives in `value`; a press on the field focuses it, a press
/// elsewhere or Esc / Enter releases it. The screen spawns the field node with a child [`TextFieldText`].
#[derive(Component, Clone, Debug, Default)]
pub struct TextField {
    pub value: String,
    pub placeholder: String,
    pub focused: bool,
    pub max_chars: usize,
}

/// The text node that shows a [`TextField`]'s value (or its placeholder) and the caret.
#[derive(Component)]
pub struct TextFieldText;

/// Typing into the focused field: printable characters, Backspace; Esc and Enter release it. Characters typed with
/// Ctrl or Cmd held are shortcuts (the web fills in their letter), not text.
fn type_into_fields(mut keys: MessageReader<KeyboardInput>, held: Res<ButtonInput<KeyCode>>, mut fields: Query<&mut TextField>) {
    let mut field = fields.iter_mut().find(|f| f.focused);
    let shortcut = held.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight]);
    for k in keys.read() {
        let Some(f) = field.as_mut() else { continue };
        if k.state != ButtonState::Pressed {
            continue;
        }
        match &k.logical_key {
            Key::Backspace => {
                f.value.pop();
            }
            Key::Escape | Key::Enter => f.focused = false,
            _ if shortcut => {}
            _ => {
                if let Some(t) = &k.text {
                    for c in t.chars().filter(|c| !c.is_control()) {
                        if f.value.chars().count() < f.max_chars {
                            f.value.push(c);
                        }
                    }
                }
            }
        }
    }
}

/// Colours of pills, rings, value bars and text fields from their state and the pointer.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn style_widgets(
    style: Res<Style>,
    time: Res<Time<Real>>,
    mut pills: Query<
        (Ref<Pill>, Ref<Hot>, &mut BorderColor, &mut BackgroundColor, Option<&mut BoxShadow>, &Children, Entity),
        Without<Ring>,
    >,
    mut rings: Query<
        (Ref<Ring>, Ref<Hot>, Option<&RingAccent>, &mut BorderColor, &mut BackgroundColor, Option<&mut BoxShadow>, &Children, Entity),
        Without<Pill>,
    >,
    mut texts: Query<&mut TextColor, With<PillText>>,
    mut bars: Query<(Ref<ValueBar>, &mut BackgroundColor, &Children), (Without<Pill>, Without<Ring>, Without<BarFill>)>,
    mut fills: Query<(&mut Node, &mut BackgroundColor, Has<BarMark>), (Or<(With<BarFill>, With<BarMark>)>, Without<ValueBar>, Without<Pill>, Without<Ring>)>,
    mut fields: Query<(Ref<TextField>, Ref<Hot>, &mut BorderColor, &Children), (Without<Pill>, Without<Ring>)>,
    mut field_texts: Query<(&mut Text, &mut TextColor), (With<TextFieldText>, Without<PillText>)>,
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
    for (ring, hot, accent, mut border, mut bg, shadow, children, e) in &mut rings {
        if !ring.is_changed() && !hot.is_changed() {
            continue;
        }
        let accent = accent.map(|a| a.0.color(ui));
        let ((b, f, t), glow_alpha) = match accent {
            _ if ring.disabled => ((c(ui.dim, 0.3), Color::NONE, c(ui.dim, 0.75)), 0.0),
            Some(a) if hot.pressed => ((c(a, 1.0), c(a, 0.14), c(ui.number, 1.0)), 0.3),
            Some(a) if hot.hovered => ((c(a, 1.0), c(a, 0.05), c(ui.number, 1.0)), 0.2),
            Some(a) => ((c(a, 0.75), Color::NONE, c(ui.number, 0.85)), 0.12),
            None if hot.pressed => ((c(ui.teal, 0.8), c(ui.teal, 0.12), c(ui.number, 1.0)), 0.0),
            None if hot.hovered => ((c(ui.label, 0.8), c(ui.number, 0.04), c(ui.number, 1.0)), 0.0),
            None => ((c(ui.label, 0.4), Color::NONE, c(ui.number, 0.85)), 0.0),
        };
        if border.top != b {
            *border = BorderColor::all(b);
        }
        if bg.0 != f {
            bg.0 = f;
        }
        match (shadow, accent.filter(|_| glow_alpha > 0.0)) {
            (Some(mut s), Some(a)) => *s = glow(a, glow_alpha),
            (Some(_), None) => {
                commands.entity(e).remove::<BoxShadow>();
            }
            (None, Some(a)) => {
                commands.entity(e).insert(glow(a, glow_alpha));
            }
            (None, None) => {}
        }
        for child in children {
            if let Ok(mut tc) = texts.get_mut(*child) {
                set_color(&mut tc, t);
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
    // The caret blinks at 1 Hz; placeholders are dim. Texts are rewritten only when a field or the caret changes.
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
        for child in children {
            let Ok((mut text, mut color)) = field_texts.get_mut(*child) else { continue };
            let (s, col) = match (field.value.is_empty(), field.focused) {
                (true, false) => (field.placeholder.clone(), c(ui.dim, 1.0)),
                (_, true) => (format!("{}{}", field.value, if caret { "|" } else { " " }), c(ui.number, 0.95)),
                (false, false) => (field.value.clone(), c(ui.number, 0.95)),
            };
            set_text(&mut text, &s);
            set_color(&mut color, col);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, text: &str) -> KeyboardInput {
        KeyboardInput {
            key_code: code,
            logical_key: Key::Character(text.into()),
            state: ButtonState::Pressed,
            text: Some(text.into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        }
    }

    #[test]
    fn a_name_in_place_opens_renames_and_cancels() {
        let mut editing = false;
        let mut field = Box::new(TextField::default());
        // A click opens the field with the name.
        assert_eq!(name_in_place(&mut editing, &mut field, true, false, "Runner"), (Some(true), None));
        assert!(editing && field.focused && field.value == "Runner");
        // Typing, then the field lets go: renamed (trimmed); nothing while it has the keyboard.
        field.value = "  Runner Mk II ".into();
        assert_eq!(name_in_place(&mut editing, &mut field, false, false, "Runner"), (None, None));
        field.focused = false;
        assert_eq!(name_in_place(&mut editing, &mut field, false, false, "Runner"), (Some(false), Some("Runner Mk II".into())));
        // Esc, an empty or the same name: closed, no new name.
        for (value, escape) in [("Other", true), ("  ", false), ("Runner", false)] {
            name_in_place(&mut editing, &mut field, true, false, "Runner");
            field.value = value.into();
            field.focused = false;
            assert_eq!(name_in_place(&mut editing, &mut field, false, escape, "Runner"), (Some(false), None));
        }
    }

    #[test]
    fn the_ui_grows_with_the_window() {
        let window = |w: f32, h: f32| Window { resolution: bevy::window::WindowResolution::new(w as u32, h as u32), ..default() };
        assert_eq!(ui_scale_for(&window(1280.0, 720.0)), 1.0);
        assert_eq!(ui_scale_for(&window(1920.0, 1080.0)), 1.5);
        assert_eq!(ui_scale_for(&window(1000.0, 700.0)), 1.0);
        assert_eq!(ui_scale_for(&window(5120.0, 2880.0)), MAX_UI_SCALE);
    }

    /// The web fills in the letter of Ctrl+S / Ctrl+Z; in a one-line field that is a shortcut, not text.
    #[test]
    fn fields_take_text_but_not_shortcut_letters() {
        let mut world = World::new();
        world.init_resource::<Messages<KeyboardInput>>();
        world.init_resource::<ButtonInput<KeyCode>>();
        let field = world.spawn(TextField { focused: true, max_chars: 10, ..default() }).id();
        let typing = world.register_system(type_into_fields);
        world.write_message(key(KeyCode::KeyA, "a"));
        world.run_system(typing).unwrap();
        world.resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ControlLeft);
        world.write_message(key(KeyCode::KeyS, "s"));
        world.run_system(typing).unwrap();
        assert_eq!(world.get::<TextField>(field).unwrap().value, "a");
    }
}
