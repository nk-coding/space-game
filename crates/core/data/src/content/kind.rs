use serde::{Deserialize, Serialize};

/// Content kinds. `key()` is the singular token used in text keys (`part.fission_reactor.name`),
/// links (`[[part:fission_reactor]]`) and wiki paths (`wiki/part/fission_reactor.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Stat,
    Category,
    Faction,
    Material,
    Part,
    /// A drydock frame: the grid a design is built on (GDR-0006).
    Frame,
    Ship,
    /// A word of the behavior language (condition, action, target selector).
    BehaviorNode,
    /// A behavior program (priority list of rules).
    Behavior,
    /// A research family: one tech tree, and the pool its research data goes to (GDR-0024).
    Family,
    /// A research node: what it costs, needs and unlocks.
    Research,
    /// A building of the home shipyard that research opens (GDR-0071, GDR-0092): the evolution lab.
    Facility,
    /// How a new game begins.
    Start,
    /// A voyage: one small cluster with seeded sites (GDR-0062).
    Voyage,
    /// What waits at a voyage site: ships and wrecks.
    Encounter,
    /// A player action and its default inputs (the action map, GDR-0090).
    Control,
    /// Free-standing wiki article (`wiki/page/<id>.md`); has no RON data.
    Page,
}

impl Kind {
    pub const ALL: [Kind; 17] = [
        Kind::Stat,
        Kind::Category,
        Kind::Faction,
        Kind::Material,
        Kind::Part,
        Kind::Frame,
        Kind::Ship,
        Kind::BehaviorNode,
        Kind::Behavior,
        Kind::Family,
        Kind::Research,
        Kind::Facility,
        Kind::Start,
        Kind::Voyage,
        Kind::Encounter,
        Kind::Control,
        Kind::Page,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Kind::Stat => "stat",
            Kind::Category => "category",
            Kind::Faction => "faction",
            Kind::Material => "material",
            Kind::Part => "part",
            Kind::Frame => "frame",
            Kind::Ship => "ship",
            Kind::BehaviorNode => "behavior_node",
            Kind::Behavior => "behavior",
            Kind::Family => "family",
            Kind::Research => "research",
            Kind::Facility => "facility",
            Kind::Start => "start",
            Kind::Voyage => "voyage",
            Kind::Encounter => "encounter",
            Kind::Control => "control",
            Kind::Page => "page",
        }
    }

    /// Pack location of the RON data: `<dir>.ron` or `<dir>/**/*.ron`. `None` for wiki-only kinds.
    pub fn dir(self) -> Option<&'static str> {
        match self {
            Kind::Stat => Some("stats"),
            Kind::Category => Some("categories"),
            Kind::Faction => Some("factions"),
            Kind::Material => Some("materials"),
            Kind::Part => Some("parts"),
            Kind::Frame => Some("frames"),
            Kind::Ship => Some("ships"),
            Kind::BehaviorNode => Some("behavior_nodes"),
            Kind::Behavior => Some("behaviors"),
            Kind::Family => Some("families"),
            Kind::Research => Some("research"),
            Kind::Facility => Some("facilities"),
            Kind::Start => Some("starts"),
            Kind::Voyage => Some("voyages"),
            Kind::Encounter => Some("encounters"),
            Kind::Control => Some("controls"),
            Kind::Page => None,
        }
    }

    pub fn from_key(key: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.key() == key)
    }

    pub fn from_dir(dir: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.dir() == Some(dir))
    }
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.key())
    }
}
