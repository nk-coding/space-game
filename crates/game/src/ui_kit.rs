//! A small widget kit for bevy_ui screens in the reference look (style guide "UI", GDR-0008): wide-tracked
//! uppercase labels, numbers, pill buttons (plain, active teal, active gold, the glowing primary, disabled; a lock
//! on locked choices: [`PillLock`]), round ring buttons,
//! thin value bars, corner brackets and a one-line text field. Written for the drydock (12 Oct 2026); any screen
//! can use it.
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
//! * [`Tip`]: a control's tooltip (its name and key; labels carry no key letters).
//! * [`ScreenFx`] / [`ScreenPart`] (`transition`): a screen's enter / exit transition, its panels sliding in from
//!   their edges while they fade in; every screen opened from the shipyard uses it.

use bevy::input::ButtonState;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::text::{LetterSpacing, TextBackgroundColor};
use bevy::ui::{BackgroundGradient, CalculatedClip, ColorStop, ComputedNode, ComputedStackIndex, LinearGradient, UiGlobalTransform, UiScale, UiTransform, Val2};
use bevy::window::PrimaryWindow;

mod transition;

pub use transition::{
    EXIT_SECONDS, Edge, IconMaterials, ScreenFx, ScreenPart, WorldCover, show_screen, snap_screen,
};

use crate::clipboard::Clipboard;
use crate::flight_hud::HudFonts;
use crate::style::{Style, UiPalette, ui_color};
use crate::ui_icons::{Glyph, UiIcon};

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

/// The tone of a list row's subtitle (palette parts, blueprints, behaviors): dim, the label grey on the picked row
/// (dim doesn't read on its lit fill).
pub fn sub_tone(picked: bool) -> Tone {
    if picked { Tone::Label } else { Tone::Dim }
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
            .init_resource::<TipState>()
            .add_message::<UiClick>()
            .configure_sets(Update, UiStyleSet.after(UiInputSet))
            .add_systems(Startup, spawn_tip.after(crate::flight_hud::spawn_hud))
            .add_systems(Update, (hit_test, type_into_fields).chain().in_set(UiInputSet))
            .add_systems(Update, (style_widgets, show_locks, place_scroll_cues, show_tip).in_set(UiStyleSet));
        transition::plugin(app);
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
                if want {
                    f.focus();
                } else {
                    f.focused = false;
                }
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
// Tooltips

/// A control's tooltip: its name and the key that does the same, if any (Niklas, 13 Oct 2026: labels carry no key
/// letters; the tooltip names the key). Put it on a [`Hot`] node (a pill, a ring, a field, a click area over the
/// world such as home's places): resting the mouse on the node for [`TIP_DELAY`] shows a small dark plate near the
/// pointer, the name in label grey and the key in white, kept inside the window ([`tip_position`]). A press hides
/// it until the pointer leaves the node. Touch has no hover, so no tooltips.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct Tip {
    pub name: &'static str,
    pub key: Option<&'static str>,
}

impl Tip {
    /// A name only (an icon button without a key).
    pub fn new(name: &'static str) -> Self {
        Self { name, key: None }
    }

    /// A name and its key (`"B"`, `"CTRL+S"`, `"Q / E"`).
    pub fn key(name: &'static str, key: &'static str) -> Self {
        Self { name, key: Some(key) }
    }
}

/// How long the mouse rests on a control before its tooltip shows (seconds).
pub const TIP_DELAY: f32 = 0.5;
/// The tooltip's offset from the pointer (right and down, UI px) and the least room to the window's edges.
const TIP_OFFSET: Vec2 = Vec2::new(12.0, 20.0);
const TIP_MARGIN: f32 = 6.0;

/// Where a tooltip of `size` goes for the pointer at `cursor` in a window of `window` (all UI px, y down; the
/// tooltip's top-left corner): right of and below the pointer; left of it when it would leave the right edge,
/// above it when it would leave the bottom; always at least [`TIP_MARGIN`] inside the window (a tooltip larger
/// than the window keeps its top-left corner inside).
pub fn tip_position(cursor: Vec2, size: Vec2, window: Vec2) -> Vec2 {
    let mut at = cursor + TIP_OFFSET;
    if at.x + size.x > window.x - TIP_MARGIN {
        at.x = cursor.x - TIP_OFFSET.x - size.x;
    }
    if at.y + size.y > window.y - TIP_MARGIN {
        at.y = cursor.y - TIP_OFFSET.y.min(10.0) - size.y;
    }
    let max = (window - size - Vec2::splat(TIP_MARGIN)).max(Vec2::splat(TIP_MARGIN));
    at.clamp(Vec2::splat(TIP_MARGIN), max)
}

/// The tooltip plate (one for the whole game) and its texts.
#[derive(Component)]
struct TipPlate;

#[derive(Component)]
struct TipName;

#[derive(Component)]
struct TipKey;

/// Which control the mouse rests on, since when, and what the plate shows.
#[derive(Resource, Default, Debug)]
struct TipState {
    over: Option<Entity>,
    since: f32,
    /// A press on `over`: no tooltip until the pointer leaves it.
    pressed: bool,
    /// The plate shows this control's tip (its texts are set; it is placed once laid out).
    shown: Option<Entity>,
}

fn spawn_tip(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>) {
    let ui = &style.ui;
    commands
        .spawn((
            TipPlate,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(10),
                padding: UiRect::axes(px(9), px(5)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BackgroundColor(ui_color(style.background.outer(), 0.94)),
            BorderColor::all(ui_color(ui.label, 0.28)),
            GlobalZIndex(i32::MAX - 8),
            UiTransform::default(),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn((TipName, tracked(&fonts, "", 8.5, ui_color(ui.label, 1.0), 0.25), TextLayout::no_wrap()));
            p.spawn((TipKey, number(&fonts, "", 10.0, ui_color(ui.number, 1.0)), TextLayout::no_wrap()));
        });
}

/// Shows the tooltip of the control the mouse has rested on for [`TIP_DELAY`] and keeps it by the pointer (moved
/// by its transform: no layout); hides it when the pointer leaves, presses or the control goes.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn show_tip(
    time: Res<Time<Real>>,
    window: Single<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut state: ResMut<TipState>,
    tips: Query<(Entity, &Tip, &Hot, &InheritedVisibility)>,
    mut plate: Single<(&mut Visibility, &mut UiTransform, &ComputedNode), With<TipPlate>>,
    mut name: Single<(&mut Text, &mut Node), (With<TipName>, Without<TipKey>)>,
    mut key: Single<(&mut Text, &mut Node), (With<TipKey>, Without<TipName>)>,
) {
    let now = time.elapsed_secs();
    let cursor = window.cursor_position();
    let over = cursor.and_then(|_| tips.iter().find(|(_, _, h, v)| h.hovered && v.get())).map(|(e, ..)| e);
    if over != state.over {
        (state.over, state.since, state.pressed) = (over, now, false);
    }
    if buttons.any_just_pressed([MouseButton::Left, MouseButton::Right, MouseButton::Middle]) && over.is_some() {
        state.pressed = true;
    }
    let want = over.filter(|_| !state.pressed && now - state.since >= TIP_DELAY);
    let (vis, tf, node) = &mut *plate;
    let Some((e, tip)) = want.and_then(|e| tips.get(e).ok()).map(|(e, t, ..)| (e, t)) else {
        state.shown = None;
        if **vis != Visibility::Hidden {
            **vis = Visibility::Hidden;
        }
        return;
    };
    if state.shown != Some(e) {
        // New texts: laid out this frame, placed and shown from the next.
        state.shown = Some(e);
        set_text(&mut name.0, tip.name);
        set_text(&mut key.0, tip.key.unwrap_or(""));
        let d = if tip.key.is_some() { Display::Flex } else { Display::None };
        if key.1.display != d {
            key.1.display = d;
        }
        if **vis != Visibility::Hidden {
            **vis = Visibility::Hidden;
        }
        return;
    }
    let k = ui_scale.0.max(1e-3);
    let size = node.size * node.inverse_scale_factor;
    let at = tip_position(cursor.unwrap_or_default() / k, size, window.size() / k);
    let t = Val2::px(at.x, at.y);
    if tf.translation != t {
        tf.translation = t;
    }
    if **vis != Visibility::Inherited {
        **vis = Visibility::Inherited;
    }
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

/// A pill for a choice that may be locked (an armour material, a frame): its lead icon (a lock) shows only while
/// the pill is disabled.
#[derive(Component)]
pub struct PillLock;

/// A round ring button (menus, undo / redo, zoom).
#[derive(Component, Clone, Copy, Debug, PartialEq, Default)]
pub struct Ring {
    pub disabled: bool,
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

/// A scroll cue: over the bottom edge of a scrolled area (the entity: a node with a [`ScrollPosition`]) while more
/// of its content lies below, a fade into the backdrop with `MORE BELOW · WHEEL` on it. The kit places it every
/// frame ([`place_scroll_cues`]); it takes no clicks.
#[derive(Component)]
pub struct ScrollCue(pub Entity);

/// The cue's height (logical px).
const CUE_H: f32 = 24.0;

/// Spawns the [`ScrollCue`] of `area` into `parent`, the area's own parent (it must have no padding or border:
/// the cue is placed from the corners). Spawn it after the area, so it draws over it.
pub fn spawn_scroll_cue(parent: &mut ChildSpawnerCommands, fonts: &HudFonts, style: &Style, area: Entity) {
    let back = style.background.outer();
    let fade = LinearGradient::to_bottom(vec![ColorStop::percent(ui_color(back, 0.0), 0), ColorStop::percent(ui_color(back, 0.88), 80)]);
    parent
        .spawn((
            ScrollCue(area),
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                height: px(CUE_H),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::FlexEnd,
                padding: UiRect::bottom(px(1)),
                ..default()
            },
            BackgroundGradient::from(fade),
        ))
        .with_child(tracked(fonts, "MORE BELOW  ·  WHEEL", 8.0, ui_color(style.ui.label, 1.0), 0.3));
}

/// Shows each [`ScrollCue`] while its area has more below and puts it over the area's bottom edge (from last
/// frame's layout).
#[allow(clippy::type_complexity)]
fn place_scroll_cues(
    mut cues: Query<(&ScrollCue, &ChildOf, &mut Node)>,
    areas: Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility, &ScrollPosition), Without<ScrollCue>>,
    parents: Query<(&ComputedNode, &UiGlobalTransform), Without<ScrollCue>>,
) {
    for (cue, parent, mut node) in &mut cues {
        let (Ok((area, at, vis, scroll)), Ok((p, p_at))) = (areas.get(cue.0), parents.get(parent.parent())) else { continue };
        let k = area.inverse_scale_factor;
        let more = vis.get() && (area.content_size.y - area.size.y) * k - scroll.0.y > 1.0;
        let display = if more { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        if !more {
            continue;
        }
        // Corners in physical px, the offsets in logical px.
        let (a, b) = (at.translation - area.size / 2.0, p_at.translation - p.size / 2.0);
        let (left, top, width) = (px((a.x - b.x) * k), px((a.y + area.size.y - b.y) * k - CUE_H), px(area.size.x * k));
        if node.left != left || node.top != top || node.width != width {
            node.left = left;
            node.top = top;
            node.width = width;
        }
    }
}

/// The screens (the drydock, the BEHAVIOR screen, home, FLEET) are laid out for 1280 × 720 logical px; larger
/// windows scale them up to this.
const MAX_UI_SCALE: f32 = 2.0;
/// Smaller windows scale them down to this (below it they no longer fit).
const MIN_UI_SCALE: f32 = 0.5;

/// The UI scale for a window: min(w / 1280, h / 720), [`MIN_UI_SCALE`] to [`MAX_UI_SCALE`]. So the 1280 × 720
/// layout always fits the window: a window narrower or lower than that scales it down (until 13 Oct 2026 the scale
/// stopped at 1, and a centred layout such as FLEET's lost its left and top edges in a 1080 px wide or a 650 px
/// high window); a wider aspect leaves room at the sides, a taller one below and above.
pub fn ui_scale_for(window: &Window) -> f32 {
    ui_scale(window.size())
}

/// [`ui_scale_for`] for a window of `size` logical px.
pub fn ui_scale(size: Vec2) -> f32 {
    (size.x / 1280.0).min(size.y / 720.0).clamp(MIN_UI_SCALE, MAX_UI_SCALE)
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

    fn put(&mut self, at: usize, select: bool) {
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
fn byte_at(s: &str, chars: usize) -> usize {
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
fn field_parts(value: &str, caret: Caret, focused: bool) -> [&str; 5] {
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
const CARET: &str = "|";

/// The text node that shows a [`TextField`]'s value (or its placeholder), the selection and the caret.
#[derive(Component)]
pub struct TextFieldText;

/// The text spans after a [`TextFieldText`]'s own text, in [`field_parts`] order.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
struct FieldSpan(u8);

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
const CTRL_KEYS: [KeyCode; 4] = [KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight];
const SHIFT_KEYS: [KeyCode; 2] = [KeyCode::ShiftLeft, KeyCode::ShiftRight];

/// Typing into the focused field ([`edit_line`]); Esc and Enter release it. The modifiers are followed through
/// the frame's key messages in order, so a Ctrl+A pressed and let go within one frame is still a shortcut (the web
/// fills in the letter of a Ctrl key: it is never typed). Pasted text and a cut come from the [`Clipboard`], which
/// gets the field's selection to copy.
fn type_into_fields(
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

/// The lock of a [`PillLock`] pill shows while the pill is disabled.
#[allow(clippy::type_complexity)]
fn show_locks(pills: Query<(&Pill, &Children), (With<PillLock>, Changed<Pill>)>, mut icons: Query<&mut Node, With<UiIcon>>) {
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
fn style_widgets(
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

    /// A tooltip sits right of and below the pointer, flips left or up at the window's edges and never leaves it.
    #[test]
    fn a_tooltip_stays_inside_the_window() {
        let (window, size) = (Vec2::new(1280.0, 720.0), Vec2::new(120.0, 24.0));
        assert_eq!(tip_position(Vec2::new(100.0, 100.0), size, window), Vec2::new(112.0, 120.0));
        // At the right edge: left of the pointer; at the bottom: above it.
        let right = tip_position(Vec2::new(1250.0, 100.0), size, window);
        assert_eq!(right, Vec2::new(1250.0 - 12.0 - 120.0, 120.0));
        let bottom = tip_position(Vec2::new(600.0, 710.0), size, window);
        assert!(bottom.y + size.y <= 710.0 && bottom.x == 612.0, "{bottom}");
        // In a corner and at any pointer position (also off the window): fully inside, margin kept.
        for c in [Vec2::ZERO, Vec2::new(-50.0, -50.0), Vec2::new(1279.0, 719.0), Vec2::new(2000.0, 900.0), Vec2::new(5.0, 715.0), Vec2::new(1275.0, 3.0)] {
            let at = tip_position(c, size, window);
            assert!(at.x >= TIP_MARGIN && at.y >= TIP_MARGIN, "{c}: {at}");
            assert!(at.x + size.x <= window.x - TIP_MARGIN && at.y + size.y <= window.y - TIP_MARGIN, "{c}: {at}");
        }
        // A tooltip wider than the window keeps its start (its name) inside: at the left margin.
        assert_eq!(tip_position(Vec2::new(300.0, 100.0), Vec2::new(400.0, 24.0), Vec2::new(320.0, 200.0)).x, TIP_MARGIN);
    }

    #[test]
    fn the_ui_grows_with_the_window() {
        let window = |w: f32, h: f32| Window { resolution: bevy::window::WindowResolution::new(w as u32, h as u32), ..default() };
        assert_eq!(ui_scale_for(&window(1280.0, 720.0)), 1.0);
        assert_eq!(ui_scale_for(&window(1920.0, 1080.0)), 1.5);
        assert_eq!(ui_scale_for(&window(5120.0, 2880.0)), MAX_UI_SCALE);
        assert_eq!(ui_scale_for(&window(400.0, 200.0)), MIN_UI_SCALE);
    }

    /// The 1280 × 720 layout fits every window the play-test named (Niklas, 13 Oct 2026: things at the left and
    /// top were cut off at some aspects): scaled, it is never wider or higher than the window, it fills the window
    /// in one direction, and a layout centred in the window (FLEET's stage) starts inside it.
    #[test]
    fn the_layout_fits_any_aspect() {
        for (w, h) in [(1280.0, 720.0), (1600.0, 900.0), (1440.0, 1080.0), (1280.0, 1024.0), (3440.0, 1305.0), (1080.0, 1350.0), (1000.0, 700.0), (1280.0, 650.0), (2560.0, 1080.0)] {
            let k = ui_scale(Vec2::new(w, h));
            let layout = Vec2::new(1280.0, 720.0) * k;
            assert!(layout.x <= w + 1e-3 && layout.y <= h + 1e-3, "{w}x{h}: {layout}");
            assert!((layout.x - w).abs() < 1e-3 || (layout.y - h).abs() < 1e-3, "{w}x{h}: fills one direction");
            // Centred: the left and top edges are inside the window.
            let start = (Vec2::new(w, h) - layout) / 2.0;
            assert!(start.x >= -1e-3 && start.y >= -1e-3, "{w}x{h}: {start}");
        }
    }

    /// A key without text (arrows, Home, a modifier), pressed or let go.
    fn named(code: KeyCode, logical: Key, pressed: bool) -> KeyboardInput {
        let state = if pressed { ButtonState::Pressed } else { ButtonState::Released };
        KeyboardInput { key_code: code, logical_key: logical, state, text: None, repeat: false, window: Entity::PLACEHOLDER }
    }

    /// Runs `keys` through [`edit_line`] from `value` with the caret at its end.
    fn typed(value: &str, keys: &[FieldKey]) -> (String, Caret) {
        let mut v = value.to_string();
        let mut caret = Caret::end(value);
        for k in keys {
            edit_line(&mut v, &mut caret, *k, 12);
        }
        (v, caret)
    }

    #[test]
    fn the_caret_moves_and_edits_where_it_is() {
        use FieldKey::*;
        // Home, then typing: in front.
        assert_eq!(typed("Runner", &[Home { select: false }, Insert("My ")]).0, "My Runner");
        // Left twice, Backspace and Delete at the caret.
        let (v, c) = typed("Runner", &[Left { select: false }, Left { select: false }, Backspace, Delete]);
        assert_eq!((v.as_str(), c.at), ("Runr", 3));
        // The caret stays inside the text.
        let (v, c) = typed("ab", &[Right { select: false }, Home { select: false }, Left { select: false }, Backspace]);
        assert_eq!((v.as_str(), c), ("ab", Caret { at: 0, anchor: 0 }));
        assert_eq!(typed("ab", &[End { select: false }, Delete]).0, "ab");
        // Characters, not bytes.
        assert_eq!(typed("Über", &[Home { select: false }, Right { select: false }, Backspace, Insert("Ü")]).0, "Über");
    }

    #[test]
    fn a_selection_is_replaced_cut_or_collapsed() {
        use FieldKey::*;
        // Ctrl+A, then typing replaces everything (the review's "Halvex Runner copyaMy Frigate").
        assert_eq!(typed("Halvex Runner", &[SelectAll, Insert("My Frigate")]).0, "My Frigate");
        // Shift+Left selects back from the caret; Backspace removes the selection only.
        let (v, c) = typed("Runner Mk", &[Left { select: true }, Left { select: true }, Backspace]);
        assert_eq!((v.as_str(), c.at), ("Runner ", 7));
        // Shift+Home selects to the start; the selected text is what Ctrl+C copies.
        let (v, c) = typed("Runner Mk", &[Left { select: false }, Left { select: false }, Home { select: true }]);
        assert_eq!((selected_text(&v, c), c.range()), ("Runner ", 0..7));
        // Without Shift an arrow collapses the selection to its side.
        let (_, c) = typed("abcd", &[SelectAll, Left { select: false }]);
        assert_eq!(c, Caret { at: 0, anchor: 0 });
        let (_, c) = typed("abcd", &[Home { select: false }, End { select: true }, Right { select: false }]);
        assert_eq!(c, Caret { at: 4, anchor: 4 });
        // Ctrl+X: the selection goes (the clipboard has it); without one nothing happens.
        assert_eq!(typed("abcd", &[Left { select: true }, Cut]).0, "abc");
        assert_eq!(typed("abcd", &[Cut]).0, "abcd");
        // A paste replaces the selection, without control characters, up to the field's length (12).
        assert_eq!(typed("abcd", &[SelectAll, Insert("x\ty")]).0, "xy");
        assert_eq!(typed("abcdefghij", &[Home { select: false }, Insert("123456")]).0, "12abcdefghij");
        assert_eq!(typed("abcdefghijkl", &[Left { select: true }, Insert("XYZ")]).0, "abcdefghijkX");
    }

    #[test]
    fn a_field_reads_with_its_caret_and_selection_in_place() {
        let at = |at: usize, anchor: usize| Caret { at, anchor };
        assert_eq!(field_parts("Runner", at(2, 2), true), ["Ru", "", "", CARET, "nner"]);
        // The caret sits at its end of the selection.
        assert_eq!(field_parts("Runner", at(4, 1), true), ["R", "", "unn", CARET, "er"]);
        assert_eq!(field_parts("Runner", at(1, 4), true), ["R", CARET, "unn", "", "er"]);
        assert_eq!(field_parts("Runner", at(1, 4), false), ["Runner", "", "", "", ""]);
    }

    fn typing_world() -> (World, Entity, bevy::ecs::system::SystemId) {
        let mut world = World::new();
        world.init_resource::<Messages<KeyboardInput>>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<Clipboard>();
        let field = world.spawn(TextField { focused: true, max_chars: 20, ..default() }).id();
        let typing = world.register_system(type_into_fields);
        (world, field, typing)
    }

    /// The web fills in the letter of Ctrl+S / Ctrl+Z; in a one-line field that is a shortcut, not text.
    #[test]
    fn fields_take_text_but_not_shortcut_letters() {
        let (mut world, field, typing) = typing_world();
        world.write_message(key(KeyCode::KeyA, "a"));
        world.run_system(typing).unwrap();
        world.resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ControlLeft);
        world.write_message(named(KeyCode::ControlLeft, Key::Control, true));
        world.write_message(key(KeyCode::KeyS, "s"));
        world.run_system(typing).unwrap();
        // Still held in the next frame.
        world.resource_mut::<ButtonInput<KeyCode>>().clear();
        world.write_message(key(KeyCode::KeyZ, "z"));
        world.run_system(typing).unwrap();
        assert_eq!(world.get::<TextField>(field).unwrap().value, "a");
    }

    /// Ctrl+A pressed and let go within one frame (a quick press, or a headless test) selects all: the modifier is
    /// followed through the frame's keys, not read once after them. The clipboard learns the selection.
    #[test]
    fn a_quick_ctrl_a_selects_all() {
        let (mut world, field, typing) = typing_world();
        world.get_mut::<TextField>(field).unwrap().value = "Halvex Runner copy".into();
        {
            let mut held = world.resource_mut::<ButtonInput<KeyCode>>();
            held.press(KeyCode::ControlLeft);
            held.release(KeyCode::ControlLeft);
        }
        world.write_message(named(KeyCode::ControlLeft, Key::Control, true));
        world.write_message(key(KeyCode::KeyA, "a"));
        world.write_message(named(KeyCode::ControlLeft, Key::Control, false));
        world.run_system(typing).unwrap();
        assert_eq!(world.get::<TextField>(field).unwrap().value, "Halvex Runner copy");
        assert_eq!(world.resource::<Clipboard>().field.as_deref(), Some("Halvex Runner copy"));
        world.resource_mut::<ButtonInput<KeyCode>>().clear();
        world.write_message(key(KeyCode::KeyM, "M"));
        world.write_message(named(KeyCode::Home, Key::Home, true));
        world.write_message(key(KeyCode::KeyA, "a"));
        world.run_system(typing).unwrap();
        assert_eq!(world.get::<TextField>(field).unwrap().value, "aM");
        assert_eq!(world.resource::<Clipboard>().field.as_deref(), Some(""));
        // Esc lets go: the clipboard no longer serves the field.
        world.write_message(named(KeyCode::Escape, Key::Escape, true));
        world.run_system(typing).unwrap();
        assert!(!world.get::<TextField>(field).unwrap().focused);
        assert_eq!(world.resource::<Clipboard>().field, None);
    }
}
