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
    /// Applies them to the render toggles (what the URL sets for this run stays) and the UI size.
    pub fn apply(&self, params: &Params, toggles: &mut RenderToggles, size: &mut UiSize) {
        if params.flag("glow").is_none() {
            toggles.glow_sprites = self.glow;
        }
        if params.flag("bloom").is_none() {
            toggles.bloom = self.bloom;
        }
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

/// The action map's view of the keyboard and mouse: does an action fire now (see `data::input` for the rules)?
#[derive(SystemParam)]
pub struct Act<'w> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    mouse: Res<'w, ButtonInput<MouseButton>>,
    controls: Res<'w, Controls>,
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

    /// Is an input of `a` held?
    pub fn pressed(&self, a: Action) -> bool {
        self.controls.0.fires(a, |b| self.down(b, false), self.mods())
    }

    /// Was an input of `a` pressed this frame?
    pub fn just_pressed(&self, a: Action) -> bool {
        self.controls.0.fires(a, |b| self.down(b, true), self.mods())
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
        let params = app.world().resource::<Params>().clone();
        let mut size = UiSize::default();
        app.world_mut().resource_scope(|_, mut toggles: Mut<RenderToggles>| display.apply(&params, &mut toggles, &mut size));
        app.insert_resource(controls)
            .insert_resource(display)
            .insert_resource(bevy::ui::UiScale(size.0))
            .insert_resource(size)
            .add_message::<OpenSettings>()
            .init_resource::<SettingsLayer>()
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
