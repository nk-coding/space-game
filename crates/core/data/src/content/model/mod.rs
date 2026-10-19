//! Serde types for every content kind. See `docs/content-model.md` for the meaning of each field.
//!
//! Every entry starts with the shared header (`id`, `name`, `summary`, optional `description` and `lore`, `tags`).
//! Structs deny unknown fields so typos fail loudly. Numbers are `f32`, containers are ordered.
//!
//! The kinds are split by subject: parts and materials (`parts.rs`), ships (`ships.rs`), behaviors
//! (`behaviors.rs`), progression and the station's facilities (`progression.rs`), voyages and encounters
//! (`voyages.rs`), the galaxy: its generator numbers, bands, cluster types, trails, signal sources and lore
//! (`galaxy.rs`), rock kinds, landmarks and a cluster's scenery (`scenery.rs`), planet kinds and a cluster's sky (`sky.rs`), the evolution lab's goals, trials, genes, pilots, sizes and measures (`lab.rs`, one `lab.ron` per
//! pack, not an entry kind: its texts are keyed `lab.<list>.<id>.<field>`); the shared header, packs,
//! stats, categories and factions here. Every type is reached as `content::model::*` (and `data::*`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::content::kind::Kind;

pub use controls::Control;
pub use galaxy::{Band, ClusterSite, ClusterType, Galaxy, Gates, LoreFragment, NameParts, Reward, STAR_HEAT_CAP, SetPiece, SignalArea, SignalSource, SourceForm, StarSpec, Trail, TrailCount, Weighted};
pub use lab::{Duty, EnemyPalette, GENES, GOALS, GenePreset, GoalPreset, Lab, LabAxis, LabBudget, LabGene, LabGoal, LabKind, LabMeasure, LabPilot, LabShips, LabSize, LabSpread, MAX_DUTY_SECONDS, MAX_LAB_WEIGHT, MAX_PRESET_BEST, MAX_PRESET_SEEDS, MAX_PRESET_SHIPS, MAX_PRESET_WAVES, MEASURES, SPREADS, StandIn, TRIAL_KINDS, TrialPreset, TrialPresetKind};
pub use behaviors::{Behavior, BehaviorNode, ChoiceOption, NodeParam, NodeRole, ParamType, Quantity};
pub use parts::{ArmorSpec, BarrelArt, Bonus, CargoSpec, Condition, CountermeasureSpec, DefenceSpec, Delivery, Effect, EngineSpec, ExplosionSpec, GyroSpec, HangarSpec, HeartSpec, HeatSpec, HullSalvageSpec, LogicSpec, MAX_CONTROL_COST, MAX_HANGAR, MAX_POWER_WEIGHT, Material, Part, PlumeSpec, PowerOverride, PowerSpec, PowerTier, RadiatorSpec, RamSpec, RcsSpec, Recipe, SalvageSpec, ScannerSpec, Selector, SensorSpec, Shape, ShieldSpec, WeaponFamily, WeaponSpec};
pub use progression::{Facility, Family, ResearchCost, ResearchKind, ResearchNode, Refine, Start, Unlock, WorkshopRules};
pub use scenery::{Landmark, LandmarkForm, LandmarkPlace, RockGroup, RockKind, RockLayout, RockLook, Scenery};
pub use sky::{Atmosphere, BodyClass, FarGalaxy, GalaxyKind, HomeSky, Palette, PlanetForm, PlanetKind, Rings, Sky, SkyWeight, hex_rgb};
pub use ships::{AssistList, Blueprint, Frame, Icon, MAX_ASSIST_TOGGLES, MAX_WEAPON_GROUP, Origin, Placement};
pub use voyages::{Encounter, EncounterShips, EncounterWrecks, Voyage, VoyageSite};

/// Where an entry came from: pack id and the file path inside the pack (`/`-separated).
/// Filled in by the loader, never read from RON.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Source {
    pub pack: String,
    pub file: String,
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.pack, self.file)
    }
}

/// Behaviour shared by every content entry (header fields, source, text fields).
pub trait Entry {
    const KIND: Kind;
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn summary(&self) -> &str;
    fn description(&self) -> Option<&str>;
    fn lore(&self) -> Option<&str>;
    fn tags(&self) -> &[String];
    fn source(&self) -> &Source;
    fn set_source(&mut self, source: Source);
    /// Every translatable text field as `(field, text)`; the text key is `<kind>.<id>.<field>`.
    fn text_fields(&self) -> Vec<(&'static str, &str)>;
}

/// `entry!(Type, Kind, extra_text...; optional_text...)`: the [`Entry`] impl of a kind with the common header,
/// plus further translatable text fields (`String`) and optional ones (`Option<String>`).
macro_rules! entry {
    ($ty:ty, $kind:expr $(, $extra:ident)* $(; $opt:ident)*) => {
        impl Entry for $ty {
            const KIND: Kind = $kind;
            fn id(&self) -> &str { &self.id }
            fn name(&self) -> &str { &self.name }
            fn summary(&self) -> &str { &self.summary }
            fn description(&self) -> Option<&str> { self.description.as_deref() }
            fn lore(&self) -> Option<&str> { self.lore.as_deref() }
            fn tags(&self) -> &[String] { &self.tags }
            fn source(&self) -> &Source { &self.source }
            fn set_source(&mut self, source: Source) { self.source = source; }
            fn text_fields(&self) -> Vec<(&'static str, &str)> {
                let mut v = vec![("name", self.name.as_str()), ("summary", self.summary.as_str())];
                $( v.push((stringify!($extra), self.$extra.as_str())); )*
                if let Some(d) = &self.description { v.push(("description", d.as_str())); }
                if let Some(l) = &self.lore { v.push(("lore", l.as_str())); }
                $( if let Some(t) = &self.$opt { v.push((stringify!($opt), t.as_str())); } )*
                v
            }
        }
    };
}

// After `entry!`, which they use.
mod behaviors;
mod controls;
mod galaxy;
mod lab;
mod parts;
mod progression;
mod scenery;
mod ships;
mod sky;
mod voyages;

/// `pack.ron`: the manifest of a content pack. The base game is the pack `base`; mods are packs too.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackManifest {
    /// snake_case, unique across loaded packs.
    pub id: String,
    pub name: String,
    /// Free-form content version of the pack (e.g. "0.1.0").
    pub version: String,
    /// Version of the content schema the files are written against. See [`crate::SCHEMA_VERSION`].
    pub schema_version: u32,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    /// Packs that must be loaded before this one (by id).
    #[serde(default)]
    pub depends: Vec<String>,
}

/// Whether a bigger or smaller number of a stat is better (drives green/red diffs in the drydock).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Better {
    Higher,
    Lower,
    #[default]
    Neutral,
}

/// `stats.ron`: display information for one stat. Order in the file = display order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatDef {
    pub id: String,
    /// The label shown next to the value.
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Unit suffix, e.g. "t", "kN", "/s". Empty for unitless values. Translatable (`stat.<id>.unit`).
    #[serde(default)]
    pub unit: String,
    /// Maximum decimals shown; trailing zeros are trimmed.
    #[serde(default)]
    pub decimals: u8,
    #[serde(default)]
    pub better: Better,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(StatDef, Kind::Stat, unit);

/// `categories.ron`: part categories (palette tabs, wiki sections). Order in the file = display order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// System colour key (see [`crate::content::art_keys::SYSTEM_COLORS`]) for the icons of this category's parts.
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Category, Kind::Category);

/// `factions/*.ron`: who builds and flies ships.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Faction {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Hull glow key (see [`crate::content::art_keys::GLOWS`]): "player" (teal) or "enemy" (coral).
    pub glow: String,
    /// Default behavior (id) of this faction's ships, unless a ship names its own.
    #[serde(default)]
    pub behavior: Option<String>,
    /// Calm unless provoked (GDR-0099 A3): a voyage site whose every ship is of a wary faction does not wake when a
    /// player vessel comes near, only when it is hit, rammed, scanned nearby or its wrecks are stripped.
    #[serde(default)]
    pub wary: bool,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Faction, Kind::Faction);

/// Long-form wiki prose from `wiki/<kind>/<id>.md`. Never needed by the game.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WikiPage {
    pub kind: Kind,
    pub id: String,
    pub markdown: String,
    pub source: Source,
}
