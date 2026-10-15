//! The action map (GDR-0088, GDR-0090): the named things a player does ([`Action`]), the inputs that do them
//! ([`Chord`]: a key or a mouse button, with Ctrl, Shift or Alt), and the bindings now ([`Bindings`]: the content
//! pack's defaults, `controls.ron`, with the player's changes). Engine-free: keys are named as Bevy's `KeyCode`
//! variants (`KeyW`, `Digit1`, `ArrowUp`, …, see [`KEYS`]) and the client maps the names; the settings screen, the
//! tooltips and the wiki show [`Chord::label`].
//!
//! **Matching** ([`Bindings::fires`]): an input without Ctrl or Alt never fires while Ctrl or Alt is held (the
//! browser's and the editors' Ctrl combinations stay apart); Shift is ignored unless the same key is also bound
//! with Shift to an action sharing a context, so W thrusts with Shift held while the drydock's R and Shift+R stay
//! apart. A chord whose key is itself a modifier (ShiftLeft …) ignores the modifiers.
//!
//! **Not in the map** (fixed, GDR-0090): Esc (back, close; in flight: unlock, else the menu) and Enter (confirm),
//! text fields and the clipboard, the left mouse as the pointer, middle-drag panning, the wheel, touch.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Where an action acts: the screens that read it. Two actions sharing one cannot share an input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Context {
    Flight,
    Home,
    Drydock,
    Behavior,
    Research,
    Map,
    /// Every screen.
    Any,
}

/// Do two context lists share a screen?
pub fn overlap(a: &[Context], b: &[Context]) -> bool {
    a.iter().any(|x| b.iter().any(|y| x == y || *x == Context::Any || *y == Context::Any))
}

/// The settings screen's groups, in its order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    Flight,
    Mission,
    Camera,
    Shipyard,
    Drydock,
    Map,
    Debug,
}

impl Section {
    pub const ALL: [Section; 7] = [Section::Flight, Section::Mission, Section::Camera, Section::Shipyard, Section::Drydock, Section::Map, Section::Debug];
}

macro_rules! actions {
    ($($variant:ident = $id:literal),* $(,)?) => {
        /// A named thing the player does (its name, summary, section, contexts and default inputs: the content
        /// pack's `controls.ron`, one entry per action, checked both ways by validation).
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Action { $($variant),* }

        impl Action {
            pub const ALL: &'static [Action] = &[$(Action::$variant),*];

            /// The content id (`controls.ron`'s `id`, the saved settings' key).
            pub fn id(self) -> &'static str {
                match self { $(Action::$variant => $id),* }
            }
        }
    };
}

actions! {
    TurnLeft = "turn_left",
    TurnRight = "turn_right",
    ThrustForward = "thrust_forward",
    ThrustBack = "thrust_back",
    StrafeLeft = "strafe_left",
    StrafeRight = "strafe_right",
    Fire = "fire",
    FireGroup1 = "fire_group_1",
    FireGroup2 = "fire_group_2",
    FireGroup3 = "fire_group_3",
    LockNext = "lock_next",
    Drift = "drift",
    FocusNext = "focus_next",
    FocusPrevious = "focus_previous",
    Recall = "recall",
    Leave = "leave",
    TestReturn = "test_return",
    Waves = "waves",
    ResetScene = "reset_scene",
    ZoomIn = "zoom_in",
    ZoomOut = "zoom_out",
    CameraFollow = "camera_follow",
    ResetView = "reset_view",
    OpenDrydock = "open_drydock",
    OpenBehavior = "open_behavior",
    OpenResearch = "open_research",
    OpenVoyages = "open_voyages",
    OpenTestRange = "open_test_range",
    OpenFleet = "open_fleet",
    OpenLog = "open_log",
    OpenSettings = "open_settings",
    ToolPaint = "tool_paint",
    ToolBevel = "tool_bevel",
    ToolFill = "tool_fill",
    ToolErase = "tool_erase",
    ToolSelect = "tool_select",
    RotateLeft = "rotate_left",
    RotateRight = "rotate_right",
    Mirror = "mirror",
    Flip = "flip",
    Delete = "delete",
    TestFlight = "test_flight",
    Search = "search",
    Undo = "undo",
    Redo = "redo",
    Save = "save",
    Armour1 = "armour_1",
    Armour2 = "armour_2",
    Armour3 = "armour_3",
    Armour4 = "armour_4",
    Armour5 = "armour_5",
    Armour6 = "armour_6",
    Armour7 = "armour_7",
    Armour8 = "armour_8",
    Armour9 = "armour_9",
    NextVoyage = "next_voyage",
    PreviousVoyage = "previous_voyage",
    HideHud = "hide_hud",
    Pause = "pause",
    Tuning = "tuning",
    DebugOverlay = "debug_overlay",
    DebugShapes = "debug_shapes",
    GlowToggle = "glow_toggle",
    BloomToggle = "bloom_toggle",
}

impl Action {
    pub fn from_id(id: &str) -> Option<Action> {
        Action::ALL.iter().copied().find(|a| a.id() == id)
    }

    /// The trigger of weapon group `g` (1-based, GDR-0088), if there is one.
    pub fn fire_group(g: u8) -> Option<Action> {
        const GROUPS: [Action; crate::MAX_WEAPON_GROUP as usize] = [Action::FireGroup1, Action::FireGroup2, Action::FireGroup3];
        g.checked_sub(1).and_then(|k| GROUPS.get(k as usize).copied())
    }

    /// The drydock's armour shortcut for material `n` (1-based), if there is one.
    pub fn armour(n: usize) -> Option<Action> {
        const ARMOUR: [Action; 9] =
            [Action::Armour1, Action::Armour2, Action::Armour3, Action::Armour4, Action::Armour5, Action::Armour6, Action::Armour7, Action::Armour8, Action::Armour9];
        n.checked_sub(1).and_then(|k| ARMOUR.get(k).copied())
    }
}

/// Every key a binding may name: Bevy's `KeyCode` variant names (the client maps them both ways).
pub const KEYS: &[&str] = &[
    "KeyA", "KeyB", "KeyC", "KeyD", "KeyE", "KeyF", "KeyG", "KeyH", "KeyI", "KeyJ", "KeyK", "KeyL", "KeyM", "KeyN", "KeyO", "KeyP", "KeyQ",
    "KeyR", "KeyS", "KeyT", "KeyU", "KeyV", "KeyW", "KeyX", "KeyY", "KeyZ", "Digit0", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5",
    "Digit6", "Digit7", "Digit8", "Digit9", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "Numpad0",
    "Numpad1", "Numpad2", "Numpad3", "Numpad4", "Numpad5", "Numpad6", "Numpad7", "Numpad8", "Numpad9", "NumpadAdd", "NumpadSubtract",
    "NumpadMultiply", "NumpadDivide", "NumpadDecimal", "NumpadEnter", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Space", "Tab",
    "Enter", "Backspace", "Delete", "Insert", "Home", "End", "PageUp", "PageDown", "Minus", "Equal", "BracketLeft", "BracketRight",
    "Backslash", "Semicolon", "Quote", "Backquote", "Comma", "Period", "Slash", "CapsLock", "ShiftLeft", "ShiftRight", "ControlLeft",
    "ControlRight", "AltLeft", "AltRight",
];

/// Keys that are modifiers themselves (a chord on one ignores the modifiers).
const MODIFIER_KEYS: [&str; 6] = ["ShiftLeft", "ShiftRight", "ControlLeft", "ControlRight", "AltLeft", "AltRight"];

/// A mouse button a binding may name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mouse {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

impl Mouse {
    pub const ALL: [Mouse; 5] = [Mouse::Left, Mouse::Right, Mouse::Middle, Mouse::Back, Mouse::Forward];

    fn name(self) -> &'static str {
        match self {
            Mouse::Left => "MouseLeft",
            Mouse::Right => "MouseRight",
            Mouse::Middle => "MouseMiddle",
            Mouse::Back => "MouseBack",
            Mouse::Forward => "MouseForward",
        }
    }
}

/// A key (by its name in [`KEYS`]) or a mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Button {
    Key(&'static str),
    Mouse(Mouse),
}

impl Button {
    /// The button named `name` (a [`KEYS`] entry or `Mouse…`).
    pub fn named(name: &str) -> Option<Button> {
        KEYS.iter().find(|k| **k == name).map(|k| Button::Key(k)).or_else(|| Mouse::ALL.into_iter().find(|m| m.name() == name).map(Button::Mouse))
    }

    pub fn name(self) -> &'static str {
        match self {
            Button::Key(k) => k,
            Button::Mouse(m) => m.name(),
        }
    }

    fn is_modifier(self) -> bool {
        matches!(self, Button::Key(k) if MODIFIER_KEYS.contains(&k))
    }

    /// How the button reads on screen (`W`, `UP`, `SPACE`, `RMB`, `NUM +`).
    pub fn label(self) -> String {
        let k = match self {
            Button::Mouse(Mouse::Left) => return "LMB".into(),
            Button::Mouse(Mouse::Right) => return "RMB".into(),
            Button::Mouse(Mouse::Middle) => return "MMB".into(),
            Button::Mouse(Mouse::Back) => return "MOUSE 4".into(),
            Button::Mouse(Mouse::Forward) => return "MOUSE 5".into(),
            Button::Key(k) => k,
        };
        if let Some(c) = k.strip_prefix("Key") {
            return c.into();
        }
        if let Some(d) = k.strip_prefix("Digit") {
            return d.into();
        }
        if let Some(n) = k.strip_prefix("Numpad") {
            let n = match n {
                "Add" => "+",
                "Subtract" => "−",
                "Multiply" => "×",
                "Divide" => "/",
                "Decimal" => ".",
                "Enter" => "ENTER",
                n => n,
            };
            return format!("NUM {n}");
        }
        match k {
            // Words: the UI font has no arrows.
            "ArrowUp" => "UP",
            "ArrowDown" => "DOWN",
            "ArrowLeft" => "LEFT",
            "ArrowRight" => "RIGHT",
            "Space" => "SPACE",
            "Tab" => "TAB",
            "Enter" => "ENTER",
            "Backspace" => "BKSP",
            "Delete" => "DEL",
            "Insert" => "INS",
            "Home" => "HOME",
            "End" => "END",
            "PageUp" => "PG UP",
            "PageDown" => "PG DN",
            "Minus" => "−",
            "Equal" => "=",
            "BracketLeft" => "[",
            "BracketRight" => "]",
            "Backslash" => "\\",
            "Semicolon" => ";",
            "Quote" => "'",
            "Backquote" => "`",
            "Comma" => ",",
            "Period" => ".",
            "Slash" => "/",
            "CapsLock" => "CAPS",
            "ShiftLeft" | "ShiftRight" => "SHIFT",
            "ControlLeft" | "ControlRight" => "CTRL",
            "AltLeft" | "AltRight" => "ALT",
            k => return k.to_uppercase(),
        }
        .into()
    }
}

/// The modifiers held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

/// An input: a button with the modifiers it needs. Text form `Ctrl+Shift+KeyZ`, `MouseRight`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Chord {
    pub button: Button,
    pub mods: Mods,
}

impl Chord {
    pub fn new(button: Button, mods: Mods) -> Self {
        Chord { button, mods: if button.is_modifier() { Mods::default() } else { mods } }
    }

    /// How it reads on screen: `SHIFT+R`, `CTRL+Z`, `RMB`.
    pub fn label(&self) -> String {
        let mut s = String::new();
        for (on, word) in [(self.mods.ctrl, "CTRL+"), (self.mods.alt, "ALT+"), (self.mods.shift, "SHIFT+")] {
            if on {
                s.push_str(word);
            }
        }
        s + &self.button.label()
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (on, word) in [(self.mods.ctrl, "Ctrl+"), (self.mods.alt, "Alt+"), (self.mods.shift, "Shift+")] {
            if on {
                f.write_str(word)?;
            }
        }
        f.write_str(self.button.name())
    }
}

impl FromStr for Chord {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let mut mods = Mods::default();
        let mut parts: Vec<&str> = s.split('+').map(str::trim).collect();
        let last = parts.pop().filter(|p| !p.is_empty()).ok_or_else(|| format!("`{s}` names no key or button"))?;
        for p in parts {
            let flag = match p {
                "Ctrl" => &mut mods.ctrl,
                "Shift" => &mut mods.shift,
                "Alt" => &mut mods.alt,
                _ => return Err(format!("`{p}` in `{s}` is not a modifier (Ctrl, Shift, Alt)")),
            };
            if *flag {
                return Err(format!("`{p}` twice in `{s}`"));
            }
            *flag = true;
        }
        let button = Button::named(last).ok_or_else(|| format!("`{last}` is not a key or mouse button the game knows (see `data::input::KEYS`)"))?;
        Ok(Chord::new(button, mods))
    }
}

/// At most this many inputs per action.
pub const MAX_INPUTS: usize = 2;

/// What an action is bound to and where it acts (`controls.ron` as resolved).
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    pub contexts: Vec<Context>,
    pub section: Section,
    /// Up to [`MAX_INPUTS`], in slot order (`None`: an empty slot).
    pub inputs: [Option<Chord>; MAX_INPUTS],
    pub defaults: [Option<Chord>; MAX_INPUTS],
}

/// Every action's binding now: the defaults with the player's changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bindings {
    map: BTreeMap<Action, Binding>,
}

/// Up to two chords from text, as slots.
fn slots(inputs: &[String]) -> Result<[Option<Chord>; MAX_INPUTS], String> {
    if inputs.len() > MAX_INPUTS {
        return Err(format!("{} inputs: at most {MAX_INPUTS}", inputs.len()));
    }
    let mut out = [None; MAX_INPUTS];
    for (k, s) in inputs.iter().enumerate() {
        out[k] = Some(s.parse::<Chord>()?);
    }
    if out[0].is_some() && out[0] == out[1] {
        return Err(format!("`{}` twice", inputs[0]));
    }
    Ok(out)
}

impl Bindings {
    /// The defaults of `controls` (the content's entries; ones that don't read are left out: validation reports
    /// them) with the player's `changed` inputs (by action id; an id the game doesn't know, or inputs that don't
    /// read, are skipped and named in the returned notes).
    pub fn new(controls: &[crate::Control], changed: &BTreeMap<String, Vec<String>>) -> (Bindings, Vec<String>) {
        let mut map = BTreeMap::new();
        for c in controls {
            let (Some(action), Ok(defaults)) = (Action::from_id(&c.id), slots(&c.inputs)) else { continue };
            map.insert(action, Binding { contexts: c.contexts.clone(), section: c.section, inputs: defaults, defaults });
        }
        let mut b = Bindings { map };
        let mut notes = Vec::new();
        for (id, inputs) in changed {
            let Some(action) = Action::from_id(id).filter(|a| b.map.contains_key(a)) else {
                notes.push(format!("a saved control `{id}` is not one the game has: left out"));
                continue;
            };
            match slots(inputs) {
                Ok(s) => {
                    if let Some(binding) = b.map.get_mut(&action) {
                        binding.inputs = s;
                    }
                }
                Err(e) => notes.push(format!("the saved control `{id}` does not read ({e}): the default stays")),
            }
        }
        (b, notes)
    }

    pub fn get(&self, action: Action) -> Option<&Binding> {
        self.map.get(&action)
    }

    /// The bound inputs of `action`, in slot order.
    pub fn inputs(&self, action: Action) -> impl Iterator<Item = Chord> + '_ {
        self.map.get(&action).into_iter().flat_map(|b| b.inputs.iter().flatten().copied())
    }

    /// Every action with its binding, in [`Action::ALL`] order.
    pub fn iter(&self) -> impl Iterator<Item = (Action, &Binding)> {
        self.map.iter().map(|(a, b)| (*a, b))
    }

    /// How the action's inputs read (`W / UP`), or `—` when it has none.
    pub fn label(&self, action: Action) -> String {
        let all: Vec<String> = self.inputs(action).map(|c| c.label()).collect();
        if all.is_empty() { "—".into() } else { all.join(" / ") }
    }

    /// The actions that differ from their defaults, as saved (`id → inputs`).
    pub fn changed(&self) -> BTreeMap<String, Vec<String>> {
        self.map
            .iter()
            .filter(|(_, b)| b.inputs != b.defaults)
            .map(|(a, b)| (a.id().to_string(), b.inputs.iter().flatten().map(|c| c.to_string()).collect()))
            .collect()
    }

    /// Binds `chord` to slot `slot` of `action`. Another action sharing a context with it that had the chord loses
    /// it: returned, with its slot. The same chord in the action's other slot moves here.
    pub fn set(&mut self, action: Action, slot: usize, chord: Chord) -> Option<(Action, usize)> {
        let contexts = self.map.get(&action)?.contexts.clone();
        let mut taken = None;
        for (other, b) in self.map.iter_mut() {
            if !overlap(&contexts, &b.contexts) {
                continue;
            }
            for (k, input) in b.inputs.iter_mut().enumerate() {
                if *input == Some(chord) && !(*other == action && k == slot) {
                    *input = None;
                    if *other != action {
                        taken = Some((*other, k));
                    }
                }
            }
        }
        if let Some(b) = self.map.get_mut(&action)
            && let Some(s) = b.inputs.get_mut(slot)
        {
            *s = Some(chord);
        }
        taken
    }

    /// Empties slot `slot` of `action`.
    pub fn clear(&mut self, action: Action, slot: usize) {
        if let Some(s) = self.map.get_mut(&action).and_then(|b| b.inputs.get_mut(slot)) {
            *s = None;
        }
    }

    /// Every action back to its defaults.
    pub fn reset(&mut self) {
        for b in self.map.values_mut() {
            b.inputs = b.defaults;
        }
    }

    /// Does `action` fire, given which buttons count (`down`: held for `pressed`, or just pressed) and the
    /// modifiers held? See the module docs for the rules.
    pub fn fires(&self, action: Action, down: impl Fn(Button) -> bool, held: Mods) -> bool {
        let Some(b) = self.map.get(&action) else { return false };
        b.inputs.iter().flatten().any(|c| down(c.button) && self.mods_match(&b.contexts, c, held))
    }

    fn mods_match(&self, contexts: &[Context], c: &Chord, held: Mods) -> bool {
        if c.button.is_modifier() {
            return true;
        }
        let m = c.mods;
        if m.ctrl != held.ctrl || m.alt != held.alt {
            return false;
        }
        if m.shift {
            return held.shift;
        }
        // Without Shift: Shift held only matters if a sibling binds the key with Shift.
        !held.shift || !self.map.values().any(|b| overlap(contexts, &b.contexts) && b.inputs.iter().flatten().any(|o| o.button == c.button && o.mods == Mods { shift: true, ..m }))
    }
}

/// Pairs of actions sharing a context and an input (content validation refuses them), as `(a, b, chord)`.
pub fn clashes(controls: &[crate::Control]) -> Vec<(String, String, String)> {
    let parsed: Vec<(&crate::Control, Vec<Chord>)> = controls.iter().map(|c| (c, c.inputs.iter().filter_map(|s| s.parse().ok()).collect())).collect();
    let mut out = Vec::new();
    for (i, (a, ca)) in parsed.iter().enumerate() {
        for (b, cb) in &parsed[i + 1..] {
            if !overlap(&a.contexts, &b.contexts) {
                continue;
            }
            for chord in ca.iter().filter(|c| cb.contains(c)) {
                out.push((a.id.clone(), b.id.clone(), chord.to_string()));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
