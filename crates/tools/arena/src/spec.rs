//! The fight spec: what fights whom, from where, for how long. Plain data with a RON text form, so a spec can be
//! written by hand, stored next to a result and later passed around as a fight code (`docs/arena.md`).

use serde::{Deserialize, Serialize};

use crate::error::ArenaError;

/// Start distance when a spec leaves it out (m, between the two sides' front rows). PLACEHOLDER.
pub const DEFAULT_START_DISTANCE: f32 = 100.0;
/// Time limit when a spec leaves it out (s; coop-arena 3.7). PLACEHOLDER.
pub const DEFAULT_TIME_LIMIT: f32 = 90.0;
/// Longest allowed time limit (s): 30 minutes, a sanity bound.
pub const MAX_TIME_LIMIT: f32 = 1800.0;
/// Largest allowed start distance (m), a sanity bound.
pub const MAX_START_DISTANCE: f32 = 5000.0;
/// Most ships one side may field (all entries together), a sanity bound.
pub const MAX_SHIPS_PER_SIDE: u32 = 64;

/// One fight: a seed, two sides, the distance between them and the time limit. Side 0 starts at the origin with
/// its noses to +y, side 1 `start_distance` ahead facing it (the same layout turned half a turn).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FightSpec {
    /// The world seed: the sim's random streams and the arena's spawn jitter.
    pub seed: u64,
    /// Written as a list of two (`sides: [(entries: [...]), (entries: [...])]`).
    #[serde(with = "two_sides")]
    pub sides: [Side; 2],
    /// Metres between the two sides' front rows (centre to centre).
    #[serde(default = "default_start_distance")]
    pub start_distance: f32,
    /// Seconds until the fight is scored as it stands.
    #[serde(default = "default_time_limit")]
    pub time_limit: f32,
}

/// One side: its entries, placed one behind the other in this order (the first entry in front).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Side {
    pub entries: Vec<Entry>,
}

/// `count` ships of one design under one behavior, in a formation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub design: DesignRef,
    /// `None`: the design's own behavior, else its faction's default.
    #[serde(default)]
    pub behavior: Option<BehaviorRef>,
    #[serde(default = "one")]
    pub count: u32,
    #[serde(default)]
    pub formation: Formation,
}

/// A design: a ship of the content by id, or a blueprint as text (a saved or exported drydock design, the format
/// of `content/base/ships/*.ron`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DesignRef {
    Ship(String),
    Blueprint(String),
}

/// A behavior: one of the content by id, or a program in the text form (`docs/behavior-format.md`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BehaviorRef {
    Content(String),
    Text(String),
}

/// How an entry's ships stand: side by side across the line of the fight, or one behind the other.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Formation {
    #[default]
    Line,
    Column,
}

/// `[Side; 2]` as a list in the text (serde would write a tuple, `((..), (..))`).
mod two_sides {
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::Side;

    pub fn serialize<S: Serializer>(sides: &[Side; 2], ser: S) -> Result<S::Ok, S::Error> {
        sides.as_slice().serialize(ser)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<[Side; 2], D::Error> {
        let v = Vec::<Side>::deserialize(de)?;
        let n = v.len();
        <[Side; 2]>::try_from(v).map_err(|_| D::Error::invalid_length(n, &"two sides"))
    }
}

fn default_start_distance() -> f32 {
    DEFAULT_START_DISTANCE
}

fn default_time_limit() -> f32 {
    DEFAULT_TIME_LIMIT
}

fn one() -> u32 {
    1
}

impl Entry {
    /// One ship of a content design under its default behavior, in a line.
    pub fn ship(id: &str) -> Entry {
        Entry {
            design: DesignRef::Ship(id.to_string()),
            behavior: None,
            count: 1,
            formation: Formation::Line,
        }
    }
}

impl FightSpec {
    /// A fight of two sides with the default distance and time limit.
    pub fn new(seed: u64, a: Vec<Entry>, b: Vec<Entry>) -> FightSpec {
        FightSpec {
            seed,
            sides: [Side { entries: a }, Side { entries: b }],
            start_distance: DEFAULT_START_DISTANCE,
            time_limit: DEFAULT_TIME_LIMIT,
        }
    }

    /// Reads a spec from its RON text (`Some(..)` may be left out, unknown fields are errors, comments are fine).
    pub fn from_ron(text: &str) -> Result<FightSpec, ArenaError> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        data::ron_options()
            .from_str(text)
            .map_err(|e| ArenaError::Parse(e.to_string()))
    }

    /// The spec as RON text; reads back with [`FightSpec::from_ron`] to an equal spec. Blueprint and behavior
    /// texts are written as multi-line strings.
    pub fn to_ron(&self) -> String {
        let pretty = ron::ser::PrettyConfig::new().escape_strings(false);
        data::ron_options()
            .to_string_pretty(self, pretty)
            .expect("a fight spec always serialises")
    }

    /// The numbers and counts are in range (references are checked when the fight is built).
    pub fn check(&self) -> Result<(), ArenaError> {
        let number = |field: &'static str, v: f32, max: f32| {
            if v.is_finite() && v > 0.0 && v <= max {
                Ok(())
            } else {
                Err(ArenaError::Number { field, value: v, max })
            }
        };
        number("start_distance", self.start_distance, MAX_START_DISTANCE)?;
        number("time_limit", self.time_limit, MAX_TIME_LIMIT)?;
        for (side, s) in self.sides.iter().enumerate() {
            if s.entries.is_empty() {
                return Err(ArenaError::EmptySide { side });
            }
            let mut ships = 0u32;
            for (entry, e) in s.entries.iter().enumerate() {
                if e.count == 0 {
                    return Err(ArenaError::ZeroCount { side, entry });
                }
                ships = ships.saturating_add(e.count);
            }
            if ships > MAX_SHIPS_PER_SIDE {
                return Err(ArenaError::TooManyShips { side, ships });
            }
        }
        Ok(())
    }

    /// The time limit in ticks (at least one).
    pub fn limit_ticks(&self) -> u64 {
        ((self.time_limit * sim::TICK_HZ as f32).round() as u64).max(1)
    }
}
