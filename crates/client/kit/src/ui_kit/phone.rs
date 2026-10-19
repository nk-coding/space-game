//! What the phone layouts share (docs/plans/android.md 3.7, 4.1, 4.2; `docs/client.md` "Phones"): whether the device
//! is a touch screen ([`TouchScreen`]), the window's safe area ([`SafeArea`]), and nodes and texts with a form for
//! each [`LayoutClass`] ([`ByClass`], [`CompactSize`]).

use bevy::prelude::*;
use bevy::text::LetterSpacing;

use super::LayoutClass;

/// The device's main pointer is a finger: the screens show their touch widgets (the flight's stick and FIRE, the
/// fight viewer's buttons) from the start and name taps, not clicks. True on Android, in a browser whose primary
/// pointer is coarse (a phone, or Chrome DevTools' device mode) and with `?touch=1`; `?touch=0` turns it off. Any
/// touch turns it on (a touch laptop's first touch).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TouchScreen(pub bool);

impl TouchScreen {
    pub fn on(self) -> bool {
        self.0
    }
}

/// The part of the window no cutout, rounded corner or system bar covers (logical px, Android's dp), which every
/// screen's edge widgets keep inside. *Placeholder* margins (32 dp on the long edges' ends, 16 dp top and bottom)
/// in the compact class until Android's `WindowInsets` replace them (plan 3.7); none in a wide window.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct SafeArea {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl SafeArea {
    /// The placeholder margins of a phone in landscape.
    pub const PHONE: SafeArea = SafeArea { left: 32.0, right: 32.0, top: 16.0, bottom: 16.0 };

    /// The safe area of a window in this class.
    pub fn of(class: LayoutClass) -> Self {
        if class.is_compact() { Self::PHONE } else { Self::default() }
    }
}

/// A node with a form for each layout class: `.0` in the wide class, `.1` in the compact one. Its `display` stays
/// as the screen sets it, unless a form hides it.
#[derive(Component, Clone, Debug)]
pub struct ByClass(pub Node, pub Node);

/// A node of `wide` in the wide class and `compact` in the compact one ([`ByClass`]).
pub fn by_class(wide: Node, compact: Node) -> impl Bundle {
    (wide.clone(), ByClass(wide, compact))
}

/// A text drawn this many px in the compact class (its letter spacing with it), at the size it was made with in the
/// wide one: for a phone layout whose type is smaller (or larger) than the wide one's, where [`super::CompactText`]
/// only raises a floor.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct CompactSize(pub f32);

/// A [`CompactSize`] text's size and letter spacing as it was made.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
struct WideSize(f32, f32);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<SafeArea>()
        .init_resource::<TouchScreen>()
        .add_systems(PreStartup, detect_touch)
        .add_systems(Update, (notice_touch, safe_area, apply_by_class, compact_size).chain().after(super::classify).before(super::UiStyleSet));
}

/// The device's touch screen at start.
fn detect_touch(params: Option<Res<crate::params::Params>>, mut screen: ResMut<TouchScreen>) {
    let forced = params.and_then(|p| p.flag("touch"));
    screen.0 = forced.unwrap_or_else(platform_touch);
}

#[cfg(target_arch = "wasm32")]
fn platform_touch() -> bool {
    web_sys::window().and_then(|w| w.match_media("(pointer: coarse)").ok().flatten()).is_some_and(|m| m.matches())
}

#[cfg(target_os = "android")]
fn platform_touch() -> bool {
    true
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn platform_touch() -> bool {
    false
}

/// A touch shows there is a touch screen (unless `?touch=0`).
fn notice_touch(touches: Res<Touches>, params: Option<Res<crate::params::Params>>, mut screen: ResMut<TouchScreen>) {
    if !screen.0 && touches.iter_just_pressed().next().is_some() && params.and_then(|p| p.flag("touch")) != Some(false) {
        screen.0 = true;
    }
}

fn safe_area(class: Res<LayoutClass>, mut area: ResMut<SafeArea>) {
    let want = SafeArea::of(*class);
    if *area != want {
        *area = want;
    }
}

fn apply_by_class(class: Res<LayoutClass>, mut nodes: Query<(Ref<ByClass>, &mut Node)>) {
    for (b, mut node) in &mut nodes {
        if !(class.is_changed() || b.is_added()) {
            continue;
        }
        let mut want = if class.is_compact() { b.1.clone() } else { b.0.clone() };
        if b.0.display != Display::None && b.1.display != Display::None {
            want.display = node.display;
        }
        if *node != want {
            *node = want;
        }
    }
}

#[allow(clippy::type_complexity)]
fn compact_size(
    class: Res<LayoutClass>,
    mut texts: Query<(Entity, Ref<CompactSize>, &mut TextFont, Option<&mut LetterSpacing>, Option<&WideSize>)>,
    mut commands: Commands,
) {
    for (e, size, mut font, spacing, wide) in &mut texts {
        if !(class.is_changed() || size.is_changed()) {
            continue;
        }
        let FontSize::Px(now) = font.font_size else { continue };
        let space = spacing.as_deref().map_or(0.0, |s| if let LetterSpacing::Px(p) = s { *p } else { 0.0 });
        let wide = wide.copied().unwrap_or(WideSize(now, space));
        let want = if class.is_compact() { size.0 } else { wide.0 };
        if want != now && wide.0 > 0.0 {
            font.font_size = FontSize::Px(want);
            if let Some(mut s) = spacing
                && matches!(*s, LetterSpacing::Px(_))
            {
                *s = LetterSpacing::Px(wide.1 * want / wide.0);
            }
        }
        commands.entity(e).try_insert(wide);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_safe_area_has_margins_only_on_a_phone() {
        assert_eq!(SafeArea::of(LayoutClass::Wide), SafeArea::default());
        assert_eq!(SafeArea::of(LayoutClass::Compact), SafeArea::PHONE);
    }

    /// A text with a compact size is drawn that size in the compact class and as made in the wide one, its letter
    /// spacing with it.
    #[test]
    fn a_compact_size_swaps_with_the_class() {
        let mut app = App::new();
        app.insert_resource(LayoutClass::Wide).add_systems(Update, compact_size);
        let e = app.world_mut().spawn((CompactSize(10.0), TextFont { font_size: FontSize::Px(20.0), ..default() }, LetterSpacing::Px(6.0))).id();
        let size = |app: &App| {
            let font = app.world().get::<TextFont>(e).unwrap();
            let FontSize::Px(s) = font.font_size else { panic!() };
            let LetterSpacing::Px(l) = *app.world().get::<LetterSpacing>(e).unwrap() else { panic!() };
            (s, l)
        };
        app.update();
        assert_eq!(size(&app), (20.0, 6.0));
        *app.world_mut().resource_mut::<LayoutClass>() = LayoutClass::Compact;
        app.update();
        assert_eq!(size(&app), (10.0, 3.0));
        *app.world_mut().resource_mut::<LayoutClass>() = LayoutClass::Wide;
        app.update();
        assert_eq!(size(&app), (20.0, 6.0));
    }
}
