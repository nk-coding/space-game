//! The flight HUD on a phone (docs/plans/android.md 4.2, GDR-0120; `docs/client.md` "Phones"). In the compact class
//! the HUD is laid out for 844 × 390 and scaled by `compact_scale` (set here while in flight; SETTINGS over the
//! flight fits its own and gives this one back): the corners inside the safe area (`SafeArea::PHONE`), the hull map
//! under its number, the values and the title smaller, the title between narrower columns, so `TEST RANGE · WAVE 1`
//! keeps one line and a fight viewer's long title two. Wide windows keep the 1280 × 720 HUD at the plain UI size.
//!
//! On a touch screen (`TouchScreen`) the target panel is a button: a tap locks the next target, as the lock key does
//! (the nearest enemy first); the ring top left opens the flight menu (`go_home`). The stick, FIRE and FLARE are
//! `controls.rs`'s.

use bevy::ui::UiScale;
use bevy::window::PrimaryWindow;
use kit::ui_kit::{LayoutClass, TouchScreen, UiSize, compact_scale};

use super::*;
use crate::settings::ActionTaps;

/// A value's size on a phone (`hud.value` is 34 in a wide window).
pub(super) const VALUE: f32 = 28.0;
/// The hull map's size, wide and on a phone.
pub(super) const HULL_MAP: (Vec2, Vec2) = (Vec2::new(100.0, 60.0), Vec2::new(84.0, 44.0));
/// The room each side of the title keeps on a phone (`hud.title_side` in a wide window).
pub(super) const TITLE_SIDE: f32 = 185.0;
/// The title's and subtitle's sizes on a phone.
pub(super) const TITLE: f32 = 13.0;
pub(super) const SUBTITLE: f32 = 10.0;
/// The banner's size on a phone (22 in a wide window).
pub(super) const BANNER: f32 = 18.0;

/// The speed group: the touch stick's zone starts under it (`controls.rs`).
#[derive(Component)]
pub struct StickZone;

/// The target panel's lines: on a touch screen a button that locks the next target.
#[derive(Component)]
pub struct TargetTap;

/// The flight's UI scale: the plain UI size in a wide window, `compact_scale` times it on a phone. Not while SETTINGS
/// lies over the flight (it fits the window itself and gives this back when it closes).
pub(super) fn flight_ui_scale(
    window: Single<&Window, With<PrimaryWindow>>,
    class: Res<LayoutClass>,
    size: Res<UiSize>,
    layer: Res<crate::settings::SettingsLayer>,
    mut scale: ResMut<UiScale>,
) {
    if layer.open {
        return;
    }
    let want = flight_scale(window.size(), *class, *size);
    if (scale.0 - want).abs() > 1e-4 {
        scale.0 = want;
    }
}

/// The flight's UI scale in a window `size` logical px big.
pub fn flight_scale(size: Vec2, class: LayoutClass, ui: UiSize) -> f32 {
    if class.is_compact() { compact_scale(size) * ui.0 } else { ui.0 }
}

/// On a touch screen the target panel takes taps (a [`HudButton`]: they never reach the world); with the mouse it
/// stays text, so a click there still locks the ship under it.
pub(super) fn touch_targets(mut commands: Commands, touch: Res<TouchScreen>, panel: Query<(Entity, Has<Hot>), With<TargetTap>>) {
    for (e, hot) in &panel {
        if touch.on() && !hot {
            commands.entity(e).insert((Hot::default(), HudButton));
        } else if !touch.on() && hot {
            commands.entity(e).remove::<(Hot, HudButton)>();
        }
    }
}

/// A tap on the target panel: LOCK TARGET (the nearest enemy, then the next by distance).
pub(super) fn target_tap(mut clicks: MessageReader<UiClick>, panel: Query<(), With<TargetTap>>, mut taps: ResMut<ActionTaps>, scene: Res<Scene>) {
    if clicks.read().any(|c| panel.contains(c.0)) && scene.kind != SceneKind::Fight {
        taps.tap(Action::LockNext);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A phone flies the HUD at the phone layouts' scale; a wide window at the plain UI size, whatever its size.
    #[test]
    fn the_flight_scale_is_the_phones_in_the_compact_class() {
        let phone = Vec2::new(915.0, 412.0);
        assert!((flight_scale(phone, LayoutClass::Compact, UiSize(1.0)) - 412.0 / 390.0).abs() < 1e-4);
        assert_eq!(flight_scale(Vec2::new(844.0, 390.0), LayoutClass::Compact, UiSize(0.9)), 0.9);
        assert_eq!(flight_scale(Vec2::new(2560.0, 1440.0), LayoutClass::Wide, UiSize(0.8)), 0.8);
    }
}
