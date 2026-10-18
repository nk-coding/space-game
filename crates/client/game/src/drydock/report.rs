//! The design report of the player's design (`sim::report`, docs/sim.md "Design report (drydock)"): every
//! live number and per-part flag the drydock shows, recomputed once per design revision with the content and
//! `SimConfig` the flights use and the career's part-stat steps (`Granted::upgrades`), so the drydock and a flight
//! agree. The
//! BEHAVIOR screen reads the same report (the design's weapon reach, whether TEST IN RANGE can fly it); [`update`]
//! runs on both screens.

use bevy::prelude::*;
use sim::{DesignReport, SimConfig};
use world::career::Granted;
use world::player::PlayerDesign;
use world::sim_bridge::GameContent;

/// While a paint or erase drag runs, the report follows at most this often (real seconds); the last one comes when
/// the stroke ends. A report of a full 64 × 64 frame takes about 5 ms (dev build), too much for every frame.
const STROKE_SECS: f64 = 0.1;

/// The report of the design at [`DrydockReport::revision`].
#[derive(Resource, Default)]
pub struct DrydockReport {
    /// `Err` when the design can't be compiled into a ship (the message says why).
    pub report: Option<Result<DesignReport, String>>,
    /// The [`PlayerDesign::revision`] the report belongs to.
    pub revision: Option<u64>,
    /// When it was made (real seconds).
    made_at: f64,
    /// The part-stat steps it was built with (the career's): what the drydock's other numbers use too.
    pub upgrades: data::Upgrades,
}

impl DrydockReport {
    /// The report, when the design compiles.
    pub fn get(&self) -> Option<&DesignReport> {
        self.report.as_ref().and_then(|r| r.as_ref().ok())
    }
}

/// Recomputes the report when the design or the content changes (about 25 µs for the Halvex Runner); during a paint
/// or erase drag at most every [`STROKE_SECS`].
pub(crate) fn update(design: Res<PlayerDesign>, content: Res<GameContent>, granted: Option<Res<Granted>>, time: Res<Time<Real>>, mut out: ResMut<DrydockReport>) {
    let upgrades = granted.map_or(data::Upgrades::NONE, |g| g.upgrades.clone());
    if content.is_changed() || out.upgrades != upgrades {
        out.revision = None;
        out.upgrades = upgrades;
    }
    let now = time.elapsed_secs_f64();
    if out.revision == Some(design.revision()) || (design.dock().in_stroke() && out.revision.is_some() && now - out.made_at < STROKE_SECS) {
        return;
    }
    out.revision = Some(design.revision());
    out.made_at = now;
    out.report = Some(sim::design_report_upgraded(&content.0, design.blueprint(), &SimConfig::default(), &out.upgrades));
}
