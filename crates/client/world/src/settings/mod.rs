//! The player's settings (GDR-0090), saved on this device: the action map's bindings ([`Controls`],
//! `settings/controls`: only the actions that differ from the content's defaults) and the display
//! ([`DisplaySettings`], `settings/display`: glow, bloom, the UI size). The SETTINGS screen (`game::settings`)
//! edits them; every screen reads its inputs through [`Act`] (never `KeyCode`s), and tooltips that name an action
//! carry an [`ActionTip`], so they show the bound inputs.
//!
//! Both are read before the first frame. A document that doesn't read leaves the defaults (logged); URL parameters
//! (`glow=`, `bloom=`) still override the display for one run.

mod keys;

use std::collections::BTreeMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use data::input::{Action, Bindings, Button, Mods};
use kit::params::{Params, RenderToggles};
use kit::ui_kit::{Tip, UiSize};
use serde::{Deserialize, Serialize};

use crate::sim_bridge::GameContent;
use crate::storage::{Document, Storage, now_unix};

pub use keys::{button_of, key_code, key_name, mouse_button, mouse_of};

/// The UI sizes the display offers (shares of the size that fits the window: larger would not fit).
pub const UI_SIZES: [f32; 3] = [1.0, 0.9, 0.8];

/// The bindings now (a resource).
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Controls(pub Bindings);

impl Controls {
    /// The content's defaults.
    pub fn defaults(content: &data::Content) -> Self {
        Controls(Bindings::new(content.controls.as_slice(), &BTreeMap::new()).0)
    }

    /// How an action's inputs read (`W / UP`, `—` for none).
    pub fn label(&self, a: Action) -> String {
        self.0.label(a)
    }

    /// Writes the changes from the defaults (`settings/controls`).
    pub fn save(&self, storage: &mut Storage) -> Result<(), String> {
        storage.write("controls", &ControlsDoc(self.0.changed()), now_unix()).map_err(|e| e.to_string())
    }
}

/// `settings/controls`: the actions the player rebound, by id, with their inputs as text.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ControlsDoc(pub BTreeMap<String, Vec<String>>);

impl Document for ControlsDoc {
    const NAMESPACE: &'static str = "settings";
    const FORMAT: u32 = 1;

    fn to_ron(&self) -> String {
        ron::ser::to_string_pretty(&self.0, ron::ser::PrettyConfig::default()).unwrap_or_default()
    }

    fn from_ron(text: &str) -> Result<Self, String> {
        ron::from_str(text).map(ControlsDoc).map_err(|e| e.to_string())
    }
}

/// The display settings (a resource; `settings/display`).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DisplaySettings {
    /// The glow layers (`RenderToggles::glow_sprites`).
    pub glow: bool,
    /// Full-screen bloom (`RenderToggles::bloom`).
    pub bloom: bool,
    /// A share of the UI size that fits the window ([`UI_SIZES`]).
    pub ui_size: f32,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self { glow: true, bloom: false, ui_size: 1.0 }
    }
}

impl Document for DisplaySettings {
    const NAMESPACE: &'static str = "settings";
    const FORMAT: u32 = 1;

    fn to_ron(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).unwrap_or_default()
    }

    fn from_ron(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }
}

impl DisplaySettings {
    /// Applies them to the render toggles and the UI size.
    pub fn apply(&self, toggles: &mut RenderToggles, size: &mut UiSize) {
        toggles.glow_sprites = self.glow;
        toggles.bloom = self.bloom;
        let ui = UI_SIZES.into_iter().find(|s| (s - self.ui_size).abs() < 1e-3).unwrap_or(1.0);
        if size.0 != ui {
            size.0 = ui;
        }
    }

    /// Writes them (`settings/display`).
    pub fn save(&self, storage: &mut Storage) -> Result<(), String> {
        storage.write("display", self, now_unix()).map_err(|e| e.to_string())
    }
}

/// Asks the SETTINGS screen to open (the flight menu's SETTINGS; home opens it itself).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenSettings;

/// Is the SETTINGS screen open (the screen keeps it)? Flight's own keys stand aside meanwhile.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SettingsLayer {
    pub open: bool,
}

/// A tooltip naming an action: the system keeps the tooltip's key line on the action's bound inputs.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionTip(pub Action);

/// A text showing an action's first bound input (a key badge: the drydock's armour swatches, a menu row's key); empty
/// while it has none. The system keeps it on the bindings.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionLabel(pub Action);

/// The modifiers held (Cmd counts as Ctrl).
pub fn mods_of(k: &ButtonInput<KeyCode>) -> Mods {
    Mods {
        ctrl: k.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight]),
        shift: k.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
        alt: k.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]),
    }
}

/// The debug keys (ADR-0046): the debug overlay, the render toggles, the test range's waves and restart. They fire
/// only in the dev mode (`?dev=1`); elsewhere the controls overlay, the SETTINGS screen and the flight menu leave
/// them out.
pub const DEV_KEYS: [Action; 6] = [Action::DebugOverlay, Action::DebugShapes, Action::GlowToggle, Action::BloomToggle, Action::Waves, Action::ResetScene];

/// Does `a` act with these launch parameters: every action, the [`DEV_KEYS`] only in the dev mode?
pub fn acts_here(a: Action, params: Option<&Params>) -> bool {
    !DEV_KEYS.contains(&a) || params.is_some_and(Params::dev)
}

/// The action map's view of the keyboard and mouse: does an action fire now (see `data::input` for the rules)?
#[derive(SystemParam)]
pub struct Act<'w> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    mouse: Res<'w, ButtonInput<MouseButton>>,
    controls: Res<'w, Controls>,
    taps: Option<Res<'w, ActionTaps>>,
    params: Option<Res<'w, Params>>,
}

/// Actions a touch button pressed (a button for what only a key does: the flight menu's RESTART, the fight viewer's
/// PLAY): [`Act`] sees each pressed for one frame, the frame after the tap, wherever its system runs in the frame.
#[derive(Resource, Default, Debug)]
pub struct ActionTaps {
    now: Vec<Action>,
    next: Vec<Action>,
}

impl ActionTaps {
    /// Presses `a` (for the next frame).
    pub fn tap(&mut self, a: Action) {
        self.next.push(a);
    }

    fn tapped(&self, a: Action) -> bool {
        self.now.contains(&a)
    }
}

/// The taps of the last frame are this frame's.
fn turn_taps(mut taps: ResMut<ActionTaps>) {
    if !(taps.now.is_empty() && taps.next.is_empty()) {
        let next = std::mem::take(&mut taps.next);
        taps.now = next;
    }
}

impl Act<'_> {
    /// The modifiers held.
    pub fn mods(&self) -> Mods {
        mods_of(&self.keys)
    }

    fn down(&self, b: Button, just: bool) -> bool {
        match b {
            Button::Key(name) => key_code(name).is_some_and(|c| if just { self.keys.just_pressed(c) } else { self.keys.pressed(c) }),
            Button::Mouse(m) => {
                let m = mouse_button(m);
                if just { self.mouse.just_pressed(m) } else { self.mouse.pressed(m) }
            }
        }
    }

    /// Is an input of `a` held? (Never for a debug key outside the dev mode, [`DEV_KEYS`].)
    pub fn pressed(&self, a: Action) -> bool {
        acts_here(a, self.params.as_deref()) && (self.tapped(a) || self.controls.0.fires(a, |b| self.down(b, false), self.mods()))
    }

    /// Did a touch button press `a` ([`ActionTaps`])?
    fn tapped(&self, a: Action) -> bool {
        self.taps.as_ref().is_some_and(|t| t.tapped(a))
    }

    /// Was an input of `a` pressed this frame? (Never for a debug key outside the dev mode, [`DEV_KEYS`].)
    pub fn just_pressed(&self, a: Action) -> bool {
        acts_here(a, self.params.as_deref()) && (self.tapped(a) || self.controls.0.fires(a, |b| self.down(b, true), self.mods()))
    }

    /// The bound inputs' label (`W / UP`).
    pub fn label(&self, a: Action) -> String {
        self.controls.label(a)
    }

    /// Ctrl (or Cmd) held: the screens' plain shortcuts stand aside for the browser's and the editors' combinations.
    pub fn ctrl(&self) -> bool {
        self.mods().ctrl
    }
}

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        // Before the first frame: the bindings from the content and the saved changes, the display. The storage and
        // the content come from their plugins (added before this one).
        let (controls, display) = app.world_mut().resource_scope(|world, storage: Mut<Storage>| {
            let content = &world.resource::<GameContent>().0;
            let changed = match storage.read::<ControlsDoc>("controls") {
                Ok(d) => d.map(|d| d.doc.0).unwrap_or_default(),
                Err(e) => {
                    warn!("settings: the saved controls don't read ({e}): the defaults stay");
                    BTreeMap::new()
                }
            };
            let (bindings, notes) = Bindings::new(content.controls.as_slice(), &changed);
            for n in notes {
                warn!("settings: {n}");
            }
            let display = match storage.read::<DisplaySettings>("display") {
                Ok(d) => d.map(|d| d.doc).unwrap_or_default(),
                Err(e) => {
                    warn!("settings: the saved display doesn't read ({e}): the defaults stay");
                    DisplaySettings::default()
                }
            };
            (Controls(bindings), display)
        });
        let mut size = UiSize::default();
        app.world_mut().resource_scope(|_, mut toggles: Mut<RenderToggles>| display.apply(&mut toggles, &mut size));
        app.insert_resource(controls)
            .insert_resource(display)
            .insert_resource(bevy::ui::UiScale(size.0))
            .insert_resource(size)
            .add_message::<OpenSettings>()
            .init_resource::<SettingsLayer>()
            .init_resource::<ActionTaps>()
            .add_systems(First, turn_taps)
            .add_systems(Update, (action_tips, action_labels));
    }
}

/// Tooltips naming an action show its bound inputs (written when the bindings or the tooltip change).
fn action_tips(controls: Res<Controls>, mut tips: Query<(Ref<ActionTip>, &mut Tip)>) {
    for (a, mut tip) in &mut tips {
        if !(controls.is_changed() || a.is_changed()) {
            continue;
        }
        let label = controls.label(a.0);
        if tip.key.as_deref() != Some(label.as_str()) {
            tip.key = Some(label.into());
        }
    }
}

/// Key badges show the first bound input (written when the bindings or the badge change).
fn action_labels(controls: Res<Controls>, mut labels: Query<(Ref<ActionLabel>, &mut Text)>) {
    for (a, mut text) in &mut labels {
        if !(controls.is_changed() || a.is_changed()) {
            continue;
        }
        let first = controls.0.inputs(a.0).next().map(|c| c.label()).unwrap_or_default();
        if text.0 != first {
            text.0 = first;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Seen(Vec<bool>);

    /// Taps `a` after a first frame and records over three more whether `a` was pressed, with these launch
    /// parameters.
    fn tapped(a: Action, query: &str) -> Vec<bool> {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ActionTaps>()
            .init_resource::<Seen>()
            .insert_resource(Params::parse(query))
            .insert_resource(Controls::defaults(&data::base_content()))
            .add_systems(First, turn_taps)
            .add_systems(Update, move |act: Act, mut seen: ResMut<Seen>| seen.0.push(act.just_pressed(a) && act.pressed(a)));
        app.update();
        app.world_mut().resource_mut::<ActionTaps>().tap(a);
        for _ in 0..3 {
            app.update();
        }
        app.world().resource::<Seen>().0.clone()
    }

    /// A touch button's tap is its action pressed for one frame, the next one, for every system in it.
    #[test]
    fn a_tap_is_the_action_pressed_for_the_next_frame() {
        assert_eq!(tapped(Action::AllAssists, ""), [false, true, false, false]);
    }

    /// The debug keys act only in the dev mode (ADR-0046); the others always.
    #[test]
    fn the_debug_keys_act_only_in_the_dev_mode() {
        assert_eq!(tapped(Action::Waves, "?dev=1"), [false, true, false, false]);
        assert_eq!(tapped(Action::Waves, ""), [false; 4]);
        for a in DEV_KEYS {
            assert!(acts_here(a, Some(&Params::parse("?dev=1"))) && !acts_here(a, Some(&Params::parse(""))) && !acts_here(a, None));
        }
        assert!(acts_here(Action::Pause, None) && acts_here(Action::HideHud, Some(&Params::parse(""))));
    }
}
