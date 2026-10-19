//! Play-test kits (`playtest.ron`, one list per pack; not an entry kind): what a test career gets with
//! `?newgame=1&save=<slot>&bands=<band>` (`docs/client.md` "Launch parameters"), so the deep bands can be flown
//! without playing up to them. The bands up to `<band>` open as if their gates were beaten, and the kit of every band
//! from the galaxy's second up to `<band>` is given: its research as researched (unpaid) and its materials and data in
//! store. A band's kit is what a player arriving there would have; a band without a kit only opens. A later pack's kit
//! for the same band replaces an earlier one.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::Source;

/// One band's kit.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaytestKit {
    /// The band (id) it is the kit for.
    pub band: String,
    /// Research nodes (ids) researched, in this order; each node's `requires` are in this kit or an earlier band's.
    #[serde(default)]
    pub research: Vec<String>,
    /// Materials (id → amount) put in store.
    #[serde(default)]
    pub materials: BTreeMap<String, u32>,
    /// Research data (family id → amount) put in store.
    #[serde(default)]
    pub data: BTreeMap<String, f32>,
    /// Where it was read from.
    #[serde(skip_deserializing)]
    pub source: Source,
}
