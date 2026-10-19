//! Tooltips ([`Tip`]): a control's name and key, shown after a short hover beside the pointer, or while a finger
//! holds the control (a long press), kept on screen.

use super::*;

// ---------------------------------------------------------------------------------------------------------
// Tooltips

/// A control's tooltip: its name and the key that does the same, if any (Niklas, 13 Oct 2026: labels carry no key
/// letters; the tooltip names the key). Put it on a [`Hot`] node (a pill, a ring, a field, a click area over the
/// world such as home's places): resting the mouse on the node for [`TIP_DELAY`] shows a small dark plate near the
/// pointer, the name in label grey and the key in white, kept inside the window ([`tip_position`]). A press hides
/// it until the pointer leaves the node.
///
/// **By touch** (no hover): a finger held on the node for [`TIP_HOLD`] without moving [`TIP_SLOP`] shows the tip
/// (its name only) above the finger ([`touch_tip_position`]) until it lifts or moves on; the press then ends without a click (a long
/// press asks what a control is, as on Android, and never presses it). A press something else took first (a list
/// that scrolled under the finger) shows none. The holds the screens use (BEHAVIOR's rule rows, the drydock's grid
/// and palette rows, the flight's stick and FIRE) are on nodes without a tip, so the two never meet.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct Tip {
    /// The name, or what the control picks with a few key numbers (an armour swatch: `STEEL  ·  MASS 2 T`).
    pub name: Cow<'static, str>,
    /// The key line: fixed (`"ESC"`), or kept on an action's bound inputs by the action map (`world::settings`).
    pub key: Option<Cow<'static, str>>,
}

impl Tip {
    /// A name only (an icon button without a key).
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self { name: name.into(), key: None }
    }

    /// A name and its key (`"ESC"`, `"ENTER"`: the fixed ones; an action's inputs come from the action map).
    pub fn key(name: impl Into<Cow<'static, str>>, key: impl Into<Cow<'static, str>>) -> Self {
        Self { name: name.into(), key: Some(key.into()) }
    }
}

/// How long the mouse rests on a control before its tooltip shows (seconds).
pub const TIP_DELAY: f32 = 0.5;
/// A finger held this long (seconds) on a control shows its tip, and must not move further than [`TIP_SLOP`]
/// (logical px) meanwhile: the long press of BEHAVIOR's rule rows (*placeholders*).
pub const TIP_HOLD: f32 = 0.35;
pub const TIP_SLOP: f32 = 10.0;
/// A touch tip's bottom this far above the finger (UI px): clear of the fingertip.
pub(super) const TIP_LIFT: f32 = 40.0;

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

/// Where a tooltip of `size` goes for a finger at `finger` (UI px, y down; its top-left corner): centred over the
/// finger, [`TIP_LIFT`] above it, below it when there is no room above; kept inside the window as [`tip_position`].
pub fn touch_tip_position(finger: Vec2, size: Vec2, window: Vec2) -> Vec2 {
    let mut at = Vec2::new(finger.x - size.x / 2.0, finger.y - TIP_LIFT - size.y);
    if at.y < TIP_MARGIN {
        at.y = finger.y + TIP_LIFT;
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
    /// The finger that is the pointer, while it is down.
    touch: Option<TouchTip>,
}

/// A finger that is down: the control with a tip its press is on (none once it moved or its press went elsewhere),
/// where and when it landed (logical px, seconds), and whether its tip shows.
#[derive(Clone, Copy, Debug)]
struct TouchTip {
    finger: u64,
    on: Option<Entity>,
    from: Vec2,
    since: f32,
    shown: bool,
}

/// The control whose tip a finger shows (and the finger, logical px): held on it [`TIP_HOLD`] within [`TIP_SLOP`].
/// The first frame it shows, the press ends without a click.
fn touch_tip(state: &mut TipState, pointer: &mut UiPointer, touches: &Touches, now: f32, has_tip: impl Fn(Entity) -> bool) -> Option<(Entity, Vec2)> {
    let Some((id, at)) = pointer.finger().and_then(|id| touches.get_pressed(id).map(|t| (id, t.position()))) else {
        state.touch = None;
        return None;
    };
    let pressed = |pointer: &UiPointer| pointer.pressed.filter(|(_, s)| *s == PressSource::Touch(id)).map(|(e, _)| e);
    let t = match state.touch {
        Some(t) if t.finger == id => state.touch.insert(t),
        _ => state.touch.insert(TouchTip { finger: id, on: pressed(pointer).filter(|e| has_tip(*e)), from: at, since: now, shown: false }),
    };
    if t.on.is_some() && ((at - t.from).length() > TIP_SLOP || (!t.shown && pressed(pointer) != t.on)) {
        t.on = None;
    }
    let on = t.on?;
    if !t.shown && now - t.since >= TIP_HOLD {
        t.shown = true;
        pointer.cancel_press();
    }
    t.shown.then_some((on, at))
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
    touches: Res<Touches>,
    mut pointer: ResMut<UiPointer>,
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
    let touch = touch_tip(&mut state, &mut pointer, &touches, now, |e| tips.get(e).is_ok_and(|(_, _, _, v)| v.get()));
    let want = match touch {
        Some((e, _)) => Some(e),
        None => over.filter(|_| !state.pressed && now - state.since >= TIP_DELAY),
    };
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
        set_text(&mut key.0, tip.key.as_deref().unwrap_or(""));
        // A finger's tip names no key: a touch screen has none.
        let d = if tip.key.is_some() && touch.is_none() { Display::Flex } else { Display::None };
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
    let at = match touch {
        Some((_, finger)) => touch_tip_position(finger / k, size, window.size() / k),
        None => tip_position(cursor.unwrap_or_default() / k, size, window.size() / k),
    };
    let t = Val2::px(at.x, at.y);
    if tf.translation != t {
        tf.translation = t;
    }
    if **vis != Visibility::Inherited {
        **vis = Visibility::Inherited;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::input::touch::{TouchInput, TouchPhase, touch_screen_input_system};

    /// A finger at `at` (logical px) in `phase`, run into the world's [`Touches`].
    fn finger(world: &mut World, phase: TouchPhase, at: Vec2) {
        world.write_message(TouchInput { phase, position: at, window: Entity::PLACEHOLDER, force: None, id: 1 });
        world.run_system_once(touch_screen_input_system).unwrap();
    }

    /// A finger held still on a control with a tip shows it after the hold and drops the press (no click on
    /// release); one that moves, or whose press a scroll took, shows none.
    #[test]
    fn a_long_press_shows_the_tip_and_never_clicks() {
        let mut world = World::new();
        world.init_resource::<Messages<TouchInput>>();
        world.init_resource::<Touches>();
        let (tip, plain) = (world.spawn_empty().id(), world.spawn_empty().id());
        let run = |world: &mut World, state: &mut TipState, pointer: &mut UiPointer, now: f32| {
            touch_tip(state, pointer, world.resource::<Touches>(), now, |e| e == tip)
        };
        let press = |on: Entity| UiPointer { finger: Some(1), pressed: Some((on, PressSource::Touch(1))), ..default() };

        let (mut state, mut pointer) = (TipState::default(), press(tip));
        finger(&mut world, TouchPhase::Started, Vec2::new(100.0, 100.0));
        assert_eq!(run(&mut world, &mut state, &mut pointer, 1.0), None);
        finger(&mut world, TouchPhase::Moved, Vec2::new(104.0, 103.0));
        assert_eq!(run(&mut world, &mut state, &mut pointer, 1.0 + TIP_HOLD - 0.01), None);
        assert!(pointer.pressed.is_some());
        assert_eq!(run(&mut world, &mut state, &mut pointer, 1.0 + TIP_HOLD), Some((tip, Vec2::new(104.0, 103.0))));
        assert!(pointer.pressed.is_none(), "the press ends without a click");
        assert!(run(&mut world, &mut state, &mut pointer, 2.0).is_some());
        // Moved on: gone for this touch.
        finger(&mut world, TouchPhase::Moved, Vec2::new(100.0, 112.0));
        assert_eq!(run(&mut world, &mut state, &mut pointer, 2.1), None);
        finger(&mut world, TouchPhase::Moved, Vec2::new(100.0, 100.0));
        assert_eq!(run(&mut world, &mut state, &mut pointer, 2.2), None);
        finger(&mut world, TouchPhase::Ended, Vec2::new(100.0, 100.0));
        assert_eq!(run(&mut world, &mut state, &mut pointer, 2.3), None);
        assert!(state.touch.is_none());

        // A press a scroll took before the hold was up.
        let (mut state, mut pointer) = (TipState::default(), press(tip));
        finger(&mut world, TouchPhase::Started, Vec2::new(100.0, 100.0));
        assert_eq!(run(&mut world, &mut state, &mut pointer, 1.0), None);
        pointer.cancel_press();
        assert_eq!(run(&mut world, &mut state, &mut pointer, 1.0 + TIP_HOLD), None);
        finger(&mut world, TouchPhase::Ended, Vec2::new(100.0, 100.0));

        // A control without a tip keeps its press (a screen's own hold).
        let (mut state, mut pointer) = (TipState::default(), press(plain));
        finger(&mut world, TouchPhase::Started, Vec2::new(100.0, 100.0));
        assert_eq!(run(&mut world, &mut state, &mut pointer, 1.0), None);
        assert_eq!(run(&mut world, &mut state, &mut pointer, 2.0), None);
        assert!(pointer.pressed.is_some());
    }

    /// A touch tip sits over the finger, clear of it, or under it at the window's top; always inside the window.
    #[test]
    fn a_touch_tip_sits_above_the_finger() {
        let (size, window) = (Vec2::new(80.0, 20.0), Vec2::new(844.0, 390.0));
        assert_eq!(touch_tip_position(Vec2::new(400.0, 200.0), size, window), Vec2::new(360.0, 200.0 - TIP_LIFT - 20.0));
        assert_eq!(touch_tip_position(Vec2::new(400.0, 30.0), size, window).y, 30.0 + TIP_LIFT);
        assert_eq!(touch_tip_position(Vec2::new(10.0, 200.0), size, window).x, TIP_MARGIN);
        assert_eq!(touch_tip_position(Vec2::new(840.0, 200.0), size, window).x, window.x - size.x - TIP_MARGIN);
    }
}
