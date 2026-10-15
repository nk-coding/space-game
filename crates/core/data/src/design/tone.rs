//! The armour tone of the cell under a machine (GDR-0069): machines are drawn as housings on a cell in the hull's
//! tone, the same rule for the game's ship views and the wiki's ship drawings. [`HullTones`] reads a design once,
//! [`hull_tone`] is the rule itself.

use std::collections::BTreeMap;

use crate::design::geometry::{Cell, Dir, ShipGrid};
use crate::{Blueprint, Content, Placement};

/// The armour tone of a hull part without a material, and of the cells under machines in a design without hull.
pub const DEFAULT_TONE: &str = "steel";

/// The armour tone the cell under a machine takes (GDR-0069), from what touches it: the tone with the most
/// `touching` hull cell sides wins, a tie going to the tone with more hull `area` in the design (then to the first
/// in name order); a machine that touches no hull takes the design's most used tone (by area; a tie goes to the
/// first in name order). `None` when the design has no hull at all.
pub fn hull_tone<'a>(touching: &BTreeMap<&'a str, u32>, area: &BTreeMap<&'a str, f32>) -> Option<&'a str> {
    let share = |t: &str| area.get(t).copied().unwrap_or(0.0);
    let mut best: Option<(&'a str, u32, f32)> = None;
    for (&t, &n) in touching {
        if best.is_none_or(|(_, bn, ba)| n > bn || (n == bn && share(t) > ba)) {
            best = Some((t, n, share(t)));
        }
    }
    if let Some((t, ..)) = best {
        return Some(t);
    }
    let mut top: Option<(&'a str, f32)> = None;
    for (&t, &a) in area {
        if top.is_none_or(|(_, ta)| a > ta) {
            top = Some((t, a));
        }
    }
    top.map(|(t, _)| t)
}

/// The armour tones of one design's hull, to find the tone under its machines ([`hull_tone`]).
pub struct HullTones<'a> {
    /// Per part: its armour tone (hull parts) or `None` (machines).
    part: Vec<Option<&'a str>>,
    /// Hull area per tone, in cells.
    area: BTreeMap<&'a str, f32>,
}

impl<'a> HullTones<'a> {
    pub fn new(content: &'a Content, ship: &'a Blueprint, grid: &ShipGrid) -> Self {
        let mut area = BTreeMap::new();
        let part = ship
            .parts
            .iter()
            .enumerate()
            .map(|(i, pl)| {
                let tone = content.parts.get(&pl.part).filter(|d| d.hull).map(|_| armor_tone(content, pl));
                if let Some(t) = tone {
                    *area.entry(t).or_insert(0.0) += grid.shape(i).area();
                }
                tone
            })
            .collect();
        Self { part, area }
    }

    /// The tone of the cell under a machine on `cells` of `grid` (the design's grid; cells of the machine itself
    /// are skipped, so a part being placed over others works too).
    pub fn under(&self, grid: &ShipGrid, cells: &[Cell]) -> &'a str {
        let mut touching = BTreeMap::new();
        for &(x, y) in cells {
            for d in Dir::ALL {
                let (dx, dy) = d.delta();
                let n = (x + dx, y + dy);
                if cells.contains(&n) {
                    continue;
                }
                if let Some(t) = grid.occupant(n).and_then(|j| self.part.get(j).copied().flatten()) {
                    *touching.entry(t).or_insert(0) += 1;
                }
            }
        }
        hull_tone(&touching, &self.area).unwrap_or(DEFAULT_TONE)
    }
}

/// The armour tone key of a hull placement (its material's; [`DEFAULT_TONE`] without one).
pub fn armor_tone<'a>(content: &'a Content, pl: &Placement) -> &'a str {
    content.placement_material(pl).and_then(|m| m.armor.as_ref()).map_or(DEFAULT_TONE, |a| a.tone.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GDR-0069's hull tone: the armour touching the machine first, the design's main tone without any.
    #[test]
    fn hull_tone_follows_the_touching_armour_then_the_design() {
        let sides = |v: &[(&'static str, u32)]| v.iter().copied().collect::<BTreeMap<_, _>>();
        let area = |v: &[(&'static str, f32)]| v.iter().copied().collect::<BTreeMap<_, _>>();
        let design = area(&[("composite", 10.0), ("steel", 4.0)]);
        // The most touching sides win, whatever the design uses most.
        assert_eq!(hull_tone(&sides(&[("steel", 3), ("composite", 1)]), &design), Some("steel"));
        // A tie goes to the tone the design uses more, then to name order.
        assert_eq!(hull_tone(&sides(&[("steel", 2), ("composite", 2)]), &design), Some("composite"));
        assert_eq!(hull_tone(&sides(&[("steel", 1), ("reactive", 1)]), &area(&[])), Some("reactive"));
        // Nothing touching: the design's most used tone (a tie: name order); no hull at all: none.
        assert_eq!(hull_tone(&sides(&[]), &design), Some("composite"));
        assert_eq!(hull_tone(&sides(&[]), &area(&[("steel", 2.0), ("reactive", 2.0)])), Some("reactive"));
        assert_eq!(hull_tone(&sides(&[]), &area(&[])), None);
    }
}
