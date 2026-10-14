//! Voyages and encounters (`voyages/*.ron`, `encounters/*.ron`): the voyage map's clusters and what their sites hold.

use super::*;

/// `voyages/*.ron`: a voyage, one small hand-made cluster with seeded sites (GDR-0062). Each launch rolls where its
/// sites lie and how many ships and wrecks each holds (`sim::voyage`); the career remembers launches and clears.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Voyage {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// The band of the galaxy it lies in (GDR-0026: `rim`, `outer`, ...), as a snake_case key; the map shows it.
    pub band: String,
    /// Place on the voyage map, `(x, y)` from 0 to 1 (x right, y down).
    pub map: (f32, f32),
    /// Voyages (ids) whose goal must have been cleared once before this one opens. Empty: only a start's list
    /// opens it.
    #[serde(default)]
    pub opens_after: Vec<String>,
    /// Radius of the cluster in metres: the sites lie inside it; the fleet enters at its edge.
    pub radius: f32,
    /// `(min, max)` metres between sites: no two sites (or a site and the entry) closer than `min`, and each site
    /// within `max` of an earlier one (or the entry).
    pub spacing: (f32, f32),
    /// Expected length `(min, max)` in minutes, for the voyage map ("≈ 8 MIN").
    pub minutes: (u32, u32),
    /// Shown the first time its goal is cleared (the debrief).
    #[serde(default)]
    pub cleared: Option<String>,
    /// The sites, in order. Exactly one is the goal: clearing it clears the voyage.
    pub sites: Vec<VoyageSite>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Voyage, Kind::Voyage; cleared);

impl Voyage {
    /// The goal site entry, if there is one.
    pub fn goal(&self) -> Option<&VoyageSite> {
        self.sites.iter().find(|s| s.goal)
    }

    /// How many sites a launch places: the sum of the entries' counts.
    pub fn site_count(&self) -> u32 {
        self.sites
            .iter()
            .fold(0, |n: u32, s| n.saturating_add(s.count))
    }
}

/// One site entry of a voyage: an encounter placed `count` times.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoyageSite {
    /// The encounter (id) waiting there.
    pub encounter: String,
    /// How many such sites (default 1).
    #[serde(default = "one_u32")]
    pub count: u32,
    /// Clearing this site clears the voyage (and opens the voyages after it).
    #[serde(default)]
    pub goal: bool,
}

pub(super) fn one_u32() -> u32 {
    1
}

/// `encounters/*.ron`: what waits at a voyage site. Its ships hold until a player vessel comes within `wake_radius`,
/// then fly their behavior; its wrecks spawn as salvage with a share of their parts already gone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Encounter {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Ships that fight (each launch rolls their counts).
    #[serde(default)]
    pub ships: Vec<EncounterShips>,
    /// Ships that lie there as wrecks (derelicts).
    #[serde(default)]
    pub wrecks: Vec<EncounterWrecks>,
    /// Metres: the ships wake when a player vessel comes this close to the site.
    pub wake_radius: f32,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Encounter, Kind::Encounter);

impl Encounter {
    /// Can it ever be cleared? Only a site with ships can (its ships destroyed or disabled): at least one ship
    /// entry with a minimum count above 0.
    pub fn always_has_ships(&self) -> bool {
        self.ships.iter().any(|s| s.count.0 > 0)
    }
}

/// Ships of one blueprint at an encounter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncounterShips {
    /// The ship (id).
    pub ship: String,
    /// `(min, max)`, both included, rolled per launch.
    pub count: (u32, u32),
}

/// Wrecks of one blueprint at an encounter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncounterWrecks {
    /// The ship (id) the wreck once was.
    pub ship: String,
    /// `(min, max)`, both included, rolled per launch.
    pub count: (u32, u32),
    /// Share of its parts already gone (0 to below 1), rolled per wreck.
    #[serde(default)]
    pub damage: f32,
}
