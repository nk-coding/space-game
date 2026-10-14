//! Tooltips ([`Tip`]): a control's name and key, shown after a short hover beside the pointer, kept on screen.

use super::*;

// ---------------------------------------------------------------------------------------------------------
// Tooltips

/// A control's tooltip: its name and the key that does the same, if any (Niklas, 13 Oct 2026: labels carry no key
/// letters; the tooltip names the key). Put it on a [`Hot`] node (a pill, a ring, a field, a click area over the
/// world such as home's places): resting the mouse on the node for [`TIP_DELAY`] shows a small dark plate near the
/// pointer, the name in label grey and the key in white, kept inside the window ([`tip_position`]). A press hides
/// it until the pointer leaves the node. Touch has no hover, so no tooltips.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct Tip {
    /// The name, or what the control picks with a few key numbers (an armour swatch: `STEEL  ·  MASS 2 T`).
    pub name: Cow<'static, str>,
    pub key: Option<&'static str>,
}

impl Tip {
    /// A name only (an icon button without a key).
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self { name: name.into(), key: None }
    }

    /// A name and its key (`"B"`, `"CTRL+S"`, `"Q / E"`).
    pub fn key(name: impl Into<Cow<'static, str>>, key: &'static str) -> Self {
        Self { name: name.into(), key: Some(key) }
    }
}

/// How long the mouse rests on a control before its tooltip shows (seconds).
pub const TIP_DELAY: f32 = 0.5;
/// The tooltip's offset from the pointer (right and down, UI px) and the least room to the window's edges.
pub(super) const TIP_OFFSET: Vec2 = Vec2::new(12.0, 20.0);
pub(super) const TIP_MARGIN: f32 = 6.0;

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
pub(super) struct TipPlate;

#[derive(Component)]
pub(super) struct TipName;

#[derive(Component)]
pub(super) struct TipKey;

/// Which control the mouse rests on, since when, and what the plate shows.
#[derive(Resource, Default, Debug)]
pub(super) struct TipState {
    over: Option<Entity>,
    since: f32,
    /// A press on `over`: no tooltip until the pointer leaves it.
    pressed: bool,
    /// The plate shows this control's tip (its texts are set; it is placed once laid out).
    shown: Option<Entity>,
}

pub(super) fn spawn_tip(mut commands: Commands, fonts: Res<HudFonts>, style: Res<Style>) {
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
pub(super) fn show_tip(
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
        set_text(&mut name.0, &tip.name);
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
