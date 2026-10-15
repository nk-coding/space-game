//! Controls: the action map's actions with their default inputs (GDR-0090; `data::input`).

use serde::{Deserialize, Serialize};

use super::{Entry, Source};
use crate::content::kind::Kind;
use crate::input::{Context, Section};

/// `controls.ron`: one player action (its id one of `data::input::Action`'s), where it acts and its default inputs
/// (up to two: a key or mouse button with modifiers, `Ctrl+Shift+KeyZ`, `MouseRight`; keys as Bevy's `KeyCode`
/// names). The player's changes are saved apart (the client's `settings/controls`); a mod rebinds a default by
/// replacing the entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// The settings screen's group.
    pub section: Section,
    /// The screens it acts on: actions sharing one cannot share an input.
    pub contexts: Vec<Context>,
    /// Default inputs, at most two.
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Control, Kind::Control);
