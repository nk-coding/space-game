//! Control load and capacity of a vessel (GDR-0038, GDR-0044). Every part has a control cost (content
//! `control_cost`); every running logic core runs up to its `control_capacity` of it, so a vessel's capacity
//! is the sum over its running cores (alive, not overheated, the vessel not dormant).
//!
//! When the load exceeds the capacity, parts go **idle** in the power brown-out order of GDR-0014 (the
//! reverse of the allocation order, per-placement and runtime settings included): lowest tier first (`Off`
//! before `Low`), lowest weight first inside a tier, the higher part index first on ties. Reactors, which make
//! power instead of drawing it, idle after every consumer; parts with a cost but no power role (content may
//! give them one; validation warns) idle last. Idle parts do nothing: no work, no draw, no output, no heat.
//! They run again as soon as the capacity covers them.
//!
//! # Cost
//!
//! Per piece and tick one pass over its logic cores (the capacity). The idle set is recomputed only when the
//! capacity changes, the piece was rebuilt (it lost parts) or a power setting changed: one pass over its
//! parts. No allocation.

use crate::design::ShipDesign;
use crate::ship::{PartState, Piece};

/// A piece's control load against its capacity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlState {
    /// Control cost of the piece's parts.
    pub load: u32,
    /// Control capacity of its running logic cores; 0 for salvage and for dormant vessels.
    pub capacity: u32,
    /// Control cost of the parts that are idle now (0 while the load fits).
    pub idle: u32,
}

/// Updates a piece's capacity and, when it or the piece changed, its idle parts. `running`: the piece is a
/// vessel's (its ship's controlling piece) and the vessel is not dormant. Pieces that do not run have no idle
/// parts (they are dark as a whole).
pub(crate) fn update(piece: &mut Piece, parts: &mut [PartState], d: &ShipDesign, running: bool) {
    let capacity = if running {
        piece
            .cores()
            .iter()
            .filter(|&&c| {
                let ps = &parts[c as usize];
                ps.alive && !ps.overheated
            })
            .map(|&c| d.parts[c as usize].control_capacity)
            .sum()
    } else {
        0
    };
    if capacity == piece.control.capacity && !piece.control_dirty {
        return;
    }
    piece.control_dirty = false;
    piece.control.capacity = capacity;
    piece.control.idle = 0;
    for &p in &piece.parts {
        parts[p as usize].idle = false;
    }
    if !running || piece.control.load <= capacity {
        return;
    }
    let mut excess = piece.control.load - capacity;
    let consumers = piece.consumers.iter().rev().map(|c| c.part);
    let reactors = piece.reactors.iter().rev().map(|r| r.part);
    let passive = piece.parts.iter().rev().copied().filter(|&p| {
        let s = &d.parts[p as usize];
        s.consumer.is_none() && !s.reactor
    });
    for p in consumers.chain(reactors).chain(passive) {
        if excess == 0 {
            break;
        }
        let cost = d.parts[p as usize].control_cost;
        let ps = &mut parts[p as usize];
        if cost == 0 || !ps.alive {
            continue;
        }
        ps.idle = true;
        piece.control.idle += cost;
        excess = excess.saturating_sub(cost);
    }
}
