//! The ring of light at the gate after a recall (GDR-0087): the fleet is back through it. A voyage's hand-over
//! (`warp_fx.rs`) starts it when a warp-out ends in the jump ([`Transit::gate`]); it plays at home, over the
//! station, and is gone when home is left.

use bevy::prelude::*;
use kit::params::RenderToggles;
use kit::style::Style;
use world::fx::{FxQueue, FxRegions, Z_SPARKS};
use world::ship_view::core_tint;
use world::warp_fx::{Transit, ring};

use super::Yard;
use super::layout::GATE_RADIUS;

/// The ring of light at home's gate after a recall (real time), and how far it spreads (gate radii).
const GATE_SECONDS: f32 = 1.3;
const GATE_REACH: f32 = 1.6;

/// After a recall, home: the fleet is back through the gate: a flash and a ring of light spreading from it.
pub(super) fn gate_ring(
    time: Res<Time<Real>>,
    yard: Res<Yard>,
    regions: Res<FxRegions>,
    style: Res<Style>,
    toggles: Res<RenderToggles>,
    mut transit: ResMut<Transit>,
    mut queue: ResMut<FxQueue>,
) {
    let Some(age) = transit.gate.as_mut() else { return };
    *age += time.delta_secs().min(0.1);
    let k = *age / GATE_SECONDS;
    if k >= 1.0 {
        transit.gate = None;
        return;
    }
    if toggles.debug_shapes {
        return;
    }
    let (teal, centre) = (style.ui.teal, yard.layout.gate);
    let fade = (1.0 - k) * (1.0 - k);
    let spread = 1.0 - (1.0 - k) * (1.0 - k);
    queue.dot(&regions.soft, centre, GATE_RADIUS * 2.4, teal.with_alpha(0.35 * fade), Z_SPARKS);
    let r = GATE_RADIUS * (0.6 + (GATE_REACH - 0.6) * spread);
    ring(&mut queue, &regions, centre, r, 0.8 * (1.0 - k) + 0.2, teal.with_alpha(0.5 * fade), None);
    ring(&mut queue, &regions, centre, r, 0.06, core_tint(teal).with_alpha(0.6 * fade), None);
}

/// Home is left: the ring with it.
pub(super) fn forget(mut transit: ResMut<Transit>) {
    transit.gate = None;
}
