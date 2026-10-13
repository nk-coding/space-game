//! The design report of the player's design (`sim::report`, docs/sim.md "Design report (drydock)"): every
//! live number and per-part flag the drydock shows, recomputed once per design revision with the content and
//! `SimConfig` the flights use (the tuning panel's scales applied), so the drydock and a test flight agree. The
//! BEHAVIOR screen reads the same report (the design's weapon reach, whether TEST IN RANGE can fly it); [`update`]
//! runs on both screens.

use bevy::prelude::*;
use data::Content;
use sim::DesignReport;

use crate::sim_bridge::GameContent;
use crate::tuning::Tuning;

use super::PlayerDesign;

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
    /// The content the report is made with (the tuning's scales applied), made again when the tuning changes.
    scaled: Option<Content>,
    /// When it was made (real seconds).
    made_at: f64,
}

impl DrydockReport {
    /// The report, when the design compiles.
    pub fn get(&self) -> Option<&DesignReport> {
        self.report.as_ref().and_then(|r| r.as_ref().ok())
    }
}

/// Recomputes the report when the design or the tuning changes (about 25 µs for the Halvex Runner); during a paint
/// or erase drag at most every [`STROKE_SECS`].
pub(crate) fn update(
    design: Res<PlayerDesign>,
    content: Res<GameContent>,
    tuning: Res<Tuning>,
    time: Res<Time<Real>>,
    mut out: ResMut<DrydockReport>,
) {
    if tuning.is_changed() || out.scaled.is_none() {
        out.scaled = Some(tuning.scaled_content(&content.0, true));
        out.revision = None;
    }
    let now = time.elapsed_secs_f64();
    if out.revision == Some(design.revision()) || (design.dock().in_stroke() && out.revision.is_some() && now - out.made_at < STROKE_SECS) {
        return;
    }
    let out = &mut *out;
    let Some(content) = out.scaled.as_ref() else { return };
    out.revision = Some(design.revision());
    out.made_at = now;
    out.report = Some(sim::design_report(content, design.blueprint(), &tuning.config()));
}
