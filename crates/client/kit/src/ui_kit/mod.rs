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
//! * [`Frame`] (`frame`): the frame every screen shares, its numbers from the style tokens: the back ring and the
//!   heading at the window's top left, the top bar's pills, plates over the view, the flight HUD's type.
//! * [`HudFonts`] (`fonts`): the UI's fonts, loaded at `Startup` by [`load_fonts`].
//! * [`Tip`]: a control's tooltip (its name and key; labels carry no key letters).
//! * [`ScreenFx`] / [`ScreenPart`] (`transition`): a screen's enter / exit transition, its panels sliding in from
//!   their edges while they fade in; every screen opened from the shipyard uses it.
//! * [`Scrollbar`] (`scrollbar`): the overlay scrollbar of every area that scrolls (shown while it scrolls or is
//!   hovered, draggable), with room past its edges for the glow of a lit row and fades over the edges it scrolls
//!   past, and its smooth scrolling ([`SmoothScroll`]: the kit takes the wheel, a notch glides like Chrome's);
//!   [`ScrollRows`] for a list that shows a pool of row nodes, scrolled by pixels too.
//!
//! * [`GroupedList`] (`list`): a search field, group chips and rows in sections under headings, in an area that
//!   scrolls; the screen supplies the rows (the drydock's palette, the workshop's parts).
//!
//! Files: text helpers, the pointer and hit testing, the UI scale here; `widgets` (pills, rings, bars, brackets and
//! their styling), `field` (the text field), `tip`, `fonts`, `frame`, `transition`, `scrollbar`, `list`.

use std::borrow::Cow;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::text::{LetterSpacing, TextBackgroundColor};
use bevy::ui::{CalculatedClip, ComputedNode, ComputedStackIndex, UiGlobalTransform, UiScale, UiTransform, Val2};
use bevy::window::PrimaryWindow;

mod field;
mod fonts;
mod frame;
mod list;
mod phone;
mod scrollbar;
mod tip;
mod transition;
mod widgets;

pub use field::{Caret, FieldKey, TextField, TextFieldText, edit_line, field_text, name_in_place, selected_text};
use field::{FieldSpan, field_parts, type_into_fields};
pub use fonts::{BARLOW_REGULAR, HudFonts, load_fonts};
pub use frame::{CardLook, Frame, HudType, PanelLook, StatsLook, crumb, frame, panel_colors, panel_node, panel_title, spawn_back, spawn_close, spawn_head, title, top_pill};
pub use phone::{ByClass, CompactSize, SafeArea, TouchScreen, by_class};
pub use list::{GroupedList, ListRow, ListSpec, fill_list, head_count, lit_group, row_shown, spawn_grouped_list};
pub use scrollbar::{BAR_HIT, FLING_MIN, FlingTrack, Glide, ScrollClipSet, ScrollRows, Scrollbar, SmoothScroll};
pub use transition::{
    EXIT_SECONDS, Edge, IconMaterials, ScreenFx, ScreenPart, WorldCover, show_screen, snap_screen,
};
pub use tip::{TIP_DELAY, TIP_HOLD, TIP_SLOP, Tip, tip_position, touch_tip_position};
use tip::{TipState, show_tip, spawn_tip};
pub use widgets::{Pill, PillLock, PillText, Ring, RingGlyph, ValueBar, ring_glyph, ring_node, spawn_bar, spawn_brackets, spawn_pill};
use widgets::{show_locks, style_widgets};

use crate::clipboard::Clipboard;
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
    /// The pointer (the mouse, or a finger that is down) is over the UI, or a press that started on the UI is held:
    /// the world ignores it.
    pub blocked: bool,
    /// A text field has the keyboard: shortcuts are off.
    pub typing: bool,
    pressed: Option<(Entity, PressSource)>,
    /// The topmost [`Hot`] node under the pointer (the scrollbars' wheel looks whether it covers an area).
    over: Option<Entity>,
    /// The first finger that is down (the pointer while no mouse is over the window).
    finger: Option<u64>,
    /// Where the pointer is (physical px): the mouse, else that finger.
    at: Option<Vec2>,
}

impl UiPointer {
    /// Where the pointer is (physical px): the mouse cursor, else the first finger that is down (touch has no
    /// hover: no position between touches).
    pub fn position(&self) -> Option<Vec2> {
        self.at
    }

    /// The finger that is the pointer (none for the mouse).
    pub fn finger(&self) -> Option<u64> {
        self.finger
    }

    /// Drops the press on a node without a click (a drag that turned out to scroll its list).
    fn cancel_press(&mut self) {
        self.pressed = None;
    }
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
            .init_resource::<UiSize>()
            .init_resource::<LayoutClass>()
            .init_resource::<TipState>()
            .add_message::<UiClick>()
            .configure_sets(Update, UiStyleSet.after(UiInputSet))
            .add_systems(Startup, (load_fonts, spawn_tip.after(load_fonts)))
            .add_systems(Update, (classify, compact_text, hit_test, type_into_fields).chain().in_set(UiInputSet))
            .add_systems(Update, (style_widgets, show_locks, show_tip).in_set(UiStyleSet));
        transition::plugin(app);
        scrollbar::plugin(app);
        list::plugin(app);
        phone::plugin(app);
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
    // Under a finger a node is hit within an invisible slop that makes it at least TOUCH_TARGET square.
    let slop = TOUCH_TARGET * scale;
    let topmost = |at: Vec2, finger: bool| {
        let mut best: Option<(u32, Entity)> = None;
        let mut near: Option<(f32, u32, Entity)> = None;
        for (e, _, node, tf, vis, stack, clip) in &hots {
            // A node scrolled out of a clipping parent (a scrolled list) is not under the pointer.
            if !vis.get() || !node.size.cmpgt(Vec2::ZERO).all() || !clip.is_none_or(|c| c.contains_point(at)) {
                continue;
            }
            let z = stack.map_or(0, |s| s.0);
            if node.contains_point(*tf, at) {
                if best.is_none_or(|b| z >= b.0) {
                    best = Some((z, e));
                }
            } else if finger
                && best.is_none()
                && let Some(d) = slop_distance(node.size, tf.try_inverse().map(|i| i.transform_point2(at)), slop)
                && near.is_none_or(|n| d < n.0 || (d == n.0 && z >= n.1))
            {
                near = Some((d, z, e));
            }
        }
        best.map(|b| b.1).or(near.map(|n| n.2))
    };
    // The first finger down is the pointer until it lifts (its last place still counts for the release).
    if pointer.finger.is_none_or(|id| touches.get_pressed(id).is_none() && !touches.just_released(id) && !touches.just_canceled(id)) {
        pointer.finger = touches.iter_just_pressed().next().map(|t| t.id()).or_else(|| touches.iter().next().map(|t| t.id()));
    }
    let finger_at = |id: u64| {
        touches
            .get_pressed(id)
            .or_else(|| touches.iter_just_released().chain(touches.iter_just_canceled()).find(|t| t.id() == id))
            .map(|t| t.position() * scale)
    };
    let cursor = window.physical_cursor_position();
    let held = pointer.finger.and_then(|id| touches.get_pressed(id)).map(|t| t.position() * scale);
    pointer.at = cursor.or(held);
    let over = match cursor {
        Some(c) => topmost(c, false),
        None => held.and_then(|h| topmost(h, true)),
    };

    // Presses: the mouse, then the first new touch.
    let mut press: Option<(Option<Entity>, PressSource)> = None;
    if buttons.just_pressed(MouseButton::Left) {
        press = Some((over, PressSource::Mouse));
    } else if let Some(t) = touches.iter_just_pressed().next() {
        press = Some((topmost(t.position() * scale, true), PressSource::Touch(t.id())));
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
            PressSource::Touch(id) => (touches.just_released(id) || touches.just_canceled(id)).then(|| finger_at(id).and_then(|at| topmost(at, true))),
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
    let on_block = pointer.at.is_some_and(|c| {
        blocks.iter().any(|(node, tf, vis)| vis.get() && node.size.cmpgt(Vec2::ZERO).all() && node.contains_point(*tf, c))
    });
    pointer.blocked = over.is_some() || on_block || pressed.is_some();
    pointer.over = over;
    pointer.typing = fields.iter().any(|(_, f)| f.focused);
}

/// Under a finger every [`Hot`] node is hit as if it were at least this many logical px (Android's dp) square: a
/// tap on no node goes to the nearest one whose grown box holds it (docs/plans/android.md 4.1, Android's 48 dp
/// touch targets; the mouse hits the drawn box only).
pub const TOUCH_TARGET: f32 = 48.0;

/// How far `local` (a point relative to a node's centre, physical px) is from a node `size` big, if it is inside the
/// node's box grown to at least `slop` square; none outside it.
fn slop_distance(size: Vec2, local: Option<Vec2>, slop: f32) -> Option<f32> {
    let p = local?.abs();
    let half = size / 2.0;
    (p.cmple(half.max(Vec2::splat(slop / 2.0))).all()).then(|| (p - half).max(Vec2::ZERO).length())
}

/// The layout class (docs/plans/android.md 4.1; ADR-0045): chosen by the window's size, not by the platform, so
/// Chrome DevTools' device mode or a small desktop window shows what a phone shows. A window under
/// [`COMPACT_HEIGHT`] logical px (Android dp) high is [`LayoutClass::Compact`]: a screen with a phone layout lays it
/// out for [`COMPACT_SIZE`] and scales it by [`compact_scale`]; a screen without one keeps its 1280 × 720 layout,
/// fitted by [`ui_scale_for`] (it fits, but small). Everything else is [`LayoutClass::Wide`]: today's layouts.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LayoutClass {
    #[default]
    Wide,
    Compact,
}

/// Windows lower than this (logical px) are [`LayoutClass::Compact`] (*placeholder*).
pub const COMPACT_HEIGHT: f32 = 500.0;
/// The size the phone layouts are made for (a phone in landscape, dp: the mockups' 844 × 390).
pub const COMPACT_SIZE: Vec2 = Vec2::new(844.0, 390.0);
/// [`compact_scale`]'s range (*placeholder*).
pub const COMPACT_SCALE: (f32, f32) = (0.9, 1.3);

impl LayoutClass {
    /// The class of a window `size` logical px big.
    pub fn of(size: Vec2) -> Self {
        if size.y < COMPACT_HEIGHT { LayoutClass::Compact } else { LayoutClass::Wide }
    }

    pub fn is_compact(self) -> bool {
        self == LayoutClass::Compact
    }
}

/// The UI scale of a phone layout in a window `size` logical px big: min(w / 844, h / 390), 0.9 to 1.3.
pub fn compact_scale(size: Vec2) -> f32 {
    (size.x / COMPACT_SIZE.x).min(size.y / COMPACT_SIZE.y).clamp(COMPACT_SCALE.0, COMPACT_SCALE.1)
}

/// Type for a phone: text under a node with this is drawn at least this many px in the [`LayoutClass::Compact`]
/// class (its letter spacing grown with it), at the size it was made with in the wide class. For a screen whose
/// phone layout keeps its widgets but not its smallest type, the labels of 7 to 8 px a 1280 × 720 layout uses
/// (docs/client.md "Phones").
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct CompactText(pub f32);

/// A text's size and letter spacing as they were made, kept while [`CompactText`] grows them.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
struct MadeSize(f32, f32);

/// The size a text made `made` px big is drawn at under a [`CompactText`] `floor` in the `class`.
pub fn compact_size(made: f32, floor: f32, class: LayoutClass) -> f32 {
    if class.is_compact() { made.max(floor) } else { made }
}

/// Applies [`CompactText`]: to every text under one when the class changes, else to the texts just made.
#[allow(clippy::type_complexity)]
fn compact_text(
    class: Res<LayoutClass>,
    roots: Query<&CompactText>,
    parents: Query<&ChildOf>,
    every: Query<Entity, With<TextFont>>,
    mut texts: ParamSet<(Query<Entity, Added<TextFont>>, Query<(&mut TextFont, Option<&mut LetterSpacing>, Option<&MadeSize>)>)>,
    mut commands: Commands,
) {
    if roots.is_empty() {
        return;
    }
    let which: Vec<Entity> = if class.is_changed() { every.iter().collect() } else { texts.p0().iter().collect() };
    let mut texts = texts.p1();
    for e in which {
        let Some(floor) = std::iter::once(e).chain(parents.iter_ancestors(e)).find_map(|a| roots.get(a).ok()) else { continue };
        let Ok((mut font, spacing, made)) = texts.get_mut(e) else { continue };
        let FontSize::Px(now) = font.font_size else { continue };
        let space = spacing.as_deref().map_or(0.0, |s| if let LetterSpacing::Px(p) = s { *p } else { 0.0 });
        let made = made.copied().unwrap_or(MadeSize(now, space));
        let want = compact_size(made.0, floor.0, *class);
        if made.0 > 0.0 && want != now {
            font.font_size = FontSize::Px(want);
            if let Some(mut s) = spacing
                && matches!(*s, LetterSpacing::Px(_))
            {
                *s = LetterSpacing::Px(made.1 * want / made.0);
            }
            commands.entity(e).try_insert(made);
        }
    }
}

/// Keeps [`LayoutClass`] with the primary window (written only when it changes, so screens can react to the change).
fn classify(window: Single<&Window, With<PrimaryWindow>>, mut class: ResMut<LayoutClass>) {
    let c = LayoutClass::of(window.size());
    if *class != c {
        *class = c;
    }
}

/// The screens (the drydock, the BEHAVIOR screen, home, FLEET) are laid out for 1280 × 720 logical px; larger
/// windows scale them up to this.
const MAX_UI_SCALE: f32 = 2.0;
/// Smaller windows scale them down to this (below it they no longer fit).
const MIN_UI_SCALE: f32 = 0.5;

/// The player's UI size (GDR-0090, the display settings): a share of the scale that fits the window (1, 0.9, 0.8;
/// larger would not fit the 1280 × 720 layouts). Every screen's UI scale is [`ui_scale_for`] times it.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct UiSize(pub f32);

impl Default for UiSize {
    fn default() -> Self {
        UiSize(1.0)
    }
}

/// The UI scale for a window: min(w / 1280, h / 720), [`MIN_UI_SCALE`] to [`MAX_UI_SCALE`]. So the 1280 × 720
/// layout always fits the window (on a phone, [`LayoutClass::Compact`], about 0.54: it fits, but its type is too
/// small to read until the screen has a phone layout scaled by [`compact_scale`]): a window narrower or lower than that scales it down (until 13 Oct 2026 the scale
/// stopped at 1, and a centred layout such as FLEET's lost its left and top edges in a 1080 px wide or a 650 px
/// high window); a wider aspect leaves room at the sides, a taller one below and above.
pub fn ui_scale_for(window: &Window, size: UiSize) -> f32 {
    ui_scale(window.size()) * size.0
}

/// [`ui_scale_for`] for a window of `size` logical px.
pub fn ui_scale(size: Vec2) -> f32 {
    (size.x / 1280.0).min(size.y / 720.0).clamp(MIN_UI_SCALE, MAX_UI_SCALE)
}

/// Pixels of a touchpad's wheel that count as one line (one step of a wheel-driven control such as the drydock's
/// Shift + wheel turn; scrolling areas take the wheel through [`Scrollbar`]).
pub const WHEEL_LINE_PX: f32 = 40.0;

#[cfg(test)]
mod tests {
    use super::*;
    use super::field::CARET;
    use super::tip::TIP_MARGIN;

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

    /// Text under `CompactText` grows to the floor on a phone only, and never shrinks.
    #[test]
    fn compact_text_has_a_floor_on_a_phone() {
        assert_eq!(compact_size(7.5, 9.0, LayoutClass::Compact), 9.0);
        assert_eq!(compact_size(12.0, 9.0, LayoutClass::Compact), 12.0);
        assert_eq!(compact_size(7.5, 9.0, LayoutClass::Wide), 7.5);
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
        assert_eq!(ui_scale_for(&window(1280.0, 720.0), UiSize(1.0)), 1.0);
        assert_eq!(ui_scale_for(&window(1920.0, 1080.0), UiSize(1.0)), 1.5);
        assert_eq!(ui_scale_for(&window(5120.0, 2880.0), UiSize(1.0)), MAX_UI_SCALE);
        assert_eq!(ui_scale_for(&window(400.0, 200.0), UiSize(1.0)), MIN_UI_SCALE);
    }

    /// Phones in landscape (Chrome DevTools' Pixel 7, Pixel 5, the mockups' 844 × 390) are compact and their phone
    /// layout's scale stays near 1; a 1280 × 720 window and larger keep today's layouts (docs/plans/android.md 4.1).
    #[test]
    fn phones_are_compact() {
        for (w, h, k) in [(915.0, 412.0, 1.0564), (851.0, 393.0, 1.0077), (844.0, 390.0, 1.0), (740.0, 360.0, 0.9)] {
            let size = Vec2::new(w, h);
            assert_eq!(LayoutClass::of(size), LayoutClass::Compact, "{size}");
            assert!((compact_scale(size) - k).abs() < 1e-3, "{size}: {}", compact_scale(size));
        }
        for (w, h) in [(1280.0, 720.0), (1024.0, 600.0), (2560.0, 1440.0), (800.0, 500.0)] {
            assert_eq!(LayoutClass::of(Vec2::new(w, h)), LayoutClass::Wide);
        }
        assert_eq!(compact_scale(Vec2::new(1400.0, 600.0)), 1.3);
    }

    /// Under a finger a small node is hit within its box grown to the touch target; a large one only on its box.
    #[test]
    fn a_finger_hits_within_the_touch_target() {
        let slop = TOUCH_TARGET * 3.0; // a phone at 3 physical px per dp
        let pill = Vec2::new(200.0, 60.0);
        assert_eq!(slop_distance(pill, Some(Vec2::new(10.0, 5.0)), slop), Some(0.0));
        // Above the pill's top edge (30 px from its centre), within 72 px: hit, 30 px away from it.
        assert_eq!(slop_distance(pill, Some(Vec2::new(0.0, 60.0)), slop), Some(30.0));
        assert_eq!(slop_distance(pill, Some(Vec2::new(0.0, 73.0)), slop), None);
        // Wider than the target: no slop sideways.
        assert_eq!(slop_distance(pill, Some(Vec2::new(101.0, 0.0)), slop), None);
        assert_eq!(slop_distance(Vec2::splat(300.0), Some(Vec2::new(151.0, 0.0)), slop), None);
        assert_eq!(slop_distance(pill, None, slop), None);
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
