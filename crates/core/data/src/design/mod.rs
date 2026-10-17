//! Ship designs: the shared blueprint geometry ([`geometry`]), the blueprint text format ([`blueprint`]), stat
//! lines and totals ([`stats`]), the drydock's editing model ([`edit`]) and the hull tone under machines
//! ([`tone`]).

pub mod blueprint;
pub mod edit;
pub mod geometry;
pub mod stats;
pub mod tone;

use crate::{Blueprint, Content};

/// Refits a player's saved design to the content (GDR-0103): every placement of a part that a content part
/// stands in for ([`crate::Part::replaces`]) becomes that part, in place. Returns how many placements changed.
pub fn refit_replaced(content: &Content, bp: &mut Blueprint) -> usize {
    let mut n = 0;
    for p in &mut bp.parts {
        if let Some(new) = content.parts.iter().find(|q| q.replaces.contains(&p.part)) {
            p.part = new.id.clone();
            n += 1;
        }
    }
    n
}
