//! The design report of the player's design (`sim::report`, docs/sim.md "Design report (drydock)"): every
//! live number and per-part flag the drydock shows, recomputed once per design revision with the content and
//! `SimConfig` the flights use (the tuning panel's scales applied), so the drydock and a test flight agree.

use bevy::prelude::*;
use data::Content;
use sim::DesignReport;

use crate::sim_bridge::GameContent;
use crate::tuning::Tuning;

use super::PlayerDesign;

/// The report of the design at [`DrydockReport::revision`].
#[derive(Resource, Default)]
pub struct DrydockReport {
    /// `Err` when the design can't be compiled into a ship (the message says why).
    pub report: Option<Result<DesignReport, String>>,
    /// The [`PlayerDesign::revision`] the report belongs to.
    pub revision: Option<u64>,
}

impl DrydockReport {
    /// The report, when the design compiles.
    pub fn get(&self) -> Option<&DesignReport> {
        self.report.as_ref().and_then(|r| r.as_ref().ok())
    }
}

/// Recomputes the report when the design or the tuning changes (about 25 µs for the Halvex Runner).
pub(super) fn update(
    design: Res<PlayerDesign>,
    content: Res<GameContent>,
    tuning: Res<Tuning>,
    mut scaled: Local<Option<Content>>,
    mut out: ResMut<DrydockReport>,
) {
    if tuning.is_changed() || scaled.is_none() {
        *scaled = Some(tuning.scaled_content(&content.0, true));
        out.revision = None;
    }
    if out.revision == Some(design.revision()) {
        return;
    }
    let Some(content) = scaled.as_ref() else { return };
    out.revision = Some(design.revision());
    out.report = Some(sim::design_report(content, design.blueprint(), &tuning.config()));
}
