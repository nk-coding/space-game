//! Power allocation (GDR-0014). Every piece shares one pool: each tick its supply (working reactors, with
//! bonuses) is handed to its consumers by **tier**, and by **weight** inside the first tier that cannot be
//! fully served, so parts run at partial output.
//!
//! # The rule (per piece, per tick)
//!
//! 1. Tiers are served in order: critical, high, normal, low. `Off` never draws.
//! 2. A tier whose demand fits into what is left is served in full.
//! 3. The first tier that does not fit shares what is left by weight: every running part gets the same
//!    multiple `k × weight` of its demand (capped at 1; parts that reach 1 leave the rest to the others). A
//!    part with weight 2 runs at twice the level of a weight-1 part.
//! 4. A part whose level would fall below its `min_level` switches off instead: one at a time, lowest weight
//!    first (ties: the higher part index first), and its share goes back to the others of its tier by weight.
//!    A part that reaches its minimum once others have switched off keeps running.
//! 5. If parts of that tier run at partial output, the tiers below get nothing. If the tier ends up fully
//!    served (the parts that did not fit switched off), what is left goes on to the next tier.
//!
//! A part's output, heat and power all scale with its level (`PartState::work = activity × power`): guns fire
//! slower (beams burn weaker), shields recharge slower, engines, RCS nozzles and gyros push less.
//!
//! # Settings
//!
//! Every consumer has a [`PowerSetting`] (tier and weight). The part type gives the default (content
//! `power.tier`, `power.weight`), the blueprint placement may override it (the drydock), and
//! [`Ship::set_power_setting`] or `ShipCommand::Power` change it at runtime (behaviors later). The minimum
//! level is a property of the part type (`power.min_level`).
//!
//! # Cost
//!
//! Rows are cached per piece in allocation order (tier, weight descending, part) and re-sorted only when a
//! setting changes or the piece is rebuilt. A tick is one pass over the rows, plus at most two more over the one
//! tier that is shared. Only `+ − × ÷` and comparisons in a fixed order, so results are bit-identical
//! everywhere. No allocation.

use data::{MAX_POWER_WEIGHT, PowerTier};

use crate::design::{ConsumerKind, PartIdx, Stat};
use crate::hash::StateHasher;
use crate::ship::{Piece, Ship};

/// A consumer's place in the allocation: its tier, and its weight within the tier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PowerSetting {
    pub tier: PowerTier,
    /// Above 0, at most `data::MAX_POWER_WEIGHT`.
    pub weight: f32,
}

/// A reactor row, cached per piece for the per-tick loop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Reactor {
    pub part: PartIdx,
    /// Output with bonuses.
    pub output: f32,
}

/// A power consumer row, cached per piece in allocation order (no `PartSpec` access per tick).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Consumer {
    pub part: PartIdx,
    /// What drives its demand (throttle, trigger, shields switch, always on).
    pub kind: ConsumerKind,
    /// Engine or gun slot (`design::NO_SLOT` otherwise).
    pub slot: u32,
    /// Draw at full activity, with bonuses.
    pub draw: f32,
    pub tier: PowerTier,
    pub weight: f32,
    /// Below this share of its demand the part switches off.
    pub min_level: f32,
}

/// Sorts rows into allocation order: tier, then weight descending, then part index. Part indices are unique,
/// so the order is total and deterministic.
pub(crate) fn sort_rows(rows: &mut [Consumer]) {
    rows.sort_unstable_by(|a, b| {
        a.tier
            .cmp(&b.tier)
            .then(b.weight.total_cmp(&a.weight))
            .then(a.part.cmp(&b.part))
    });
}

/// Hands `supply` to the consumer `rows` (in allocation order) that ask for `want[i]` this tick. Writes each
/// row's level into `level[i]`: the share of its demand it gets (0 = off or no power, 1 = full). A row that asks
/// for nothing gets the level its tier and weight are served at (1 in a fully served tier, 0 in a tier that
/// gets nothing or is `Off`), so a client can show it dimmed. Returns `(demand, served)`; `Off` rows count
/// for neither.
pub fn allocate(rows: &[Consumer], want: &[f32], supply: f32, level: &mut [f32]) -> (f32, f32) {
    debug_assert!(want.len() == rows.len() && level.len() == rows.len());
    let mut left = supply.max(0.0);
    let (mut demand, mut served) = (0.0f32, 0.0f32);
    let mut start = 0;
    while start < rows.len() {
        let tier = rows[start].tier;
        let mut end = start;
        let mut need = 0.0f32;
        while end < rows.len() && rows[end].tier == tier {
            need += want[end];
            end += 1;
        }
        let lv = &mut level[start..end];
        if tier == PowerTier::Off {
            lv.fill(0.0);
        } else if left > 0.0 && need <= left {
            lv.fill(1.0);
            left -= need;
            demand += need;
            served += need;
        } else {
            demand += need;
            let (used, partial) = share(&rows[start..end], &want[start..end], left, lv);
            served += used;
            left = if partial { 0.0 } else { left - used };
        }
        start = end;
    }
    (demand, served)
}

/// Level markers inside [`share`] (real levels are 0..=1).
const OPEN: f32 = -1.0;
const KEEP: f32 = -2.0;

/// Shares `budget` among the rows of one tier by weight (rule 3 and 4 above). Returns the power used and
/// whether any row runs at partial output.
fn share(rows: &[Consumer], want: &[f32], budget: f32, level: &mut [f32]) -> (f32, bool) {
    if budget <= 0.0 {
        level.fill(0.0);
        return (0.0, false);
    }
    let n = rows.len();
    // Running rows get min(1, k × weight). Rows that reach 1 form a prefix (the highest weights).
    let mut r = budget; // power for the running rows below 1
    let mut wsum = 0.0f32; // Σ weight × want over them
    let mut running = 0u32;
    for i in 0..n {
        if want[i] > 0.0 {
            level[i] = OPEN;
            wsum += rows[i].weight * want[i];
            running += 1;
        }
    }
    let k_of = |r: f32, wsum: f32, running: u32| {
        if running > 0 && wsum > 0.0 {
            r / wsum
        } else {
            f32::INFINITY
        }
    };
    let mut k = k_of(r, wsum, running);
    let mut s = 0; // next row that may reach 1
    let mut j = n; // rows [j, n) have been checked against their minimum
    loop {
        while s < j {
            if want[s] <= 0.0 {
                s += 1;
                continue;
            }
            if k * rows[s].weight < 1.0 {
                break;
            }
            level[s] = 1.0;
            r -= want[s];
            wsum -= rows[s].weight * want[s];
            running -= 1;
            k = k_of(r, wsum, running);
            s += 1;
        }
        if j <= s {
            break;
        }
        j -= 1;
        if want[j] <= 0.0 {
            continue;
        }
        if k * rows[j].weight < rows[j].min_level {
            // Below its minimum: off, and its share goes back to the others.
            level[j] = 0.0;
            wsum -= rows[j].weight * want[j];
            running -= 1;
            k = k_of(r, wsum, running);
        } else {
            // Keeps running: `k` only grows from here, so it stays above its minimum.
            level[j] = KEEP;
        }
    }
    // Kept rows may reach 1 as well (highest weights first).
    while s < n {
        if want[s] <= 0.0 || level[s] == 0.0 {
            s += 1;
            continue;
        }
        if k * rows[s].weight < 1.0 {
            break;
        }
        level[s] = 1.0;
        r -= want[s];
        wsum -= rows[s].weight * want[s];
        running -= 1;
        k = k_of(r, wsum, running);
        s += 1;
    }
    let partial = running > 0;
    let mut used = 0.0f32;
    for i in 0..n {
        let l = if want[i] > 0.0 {
            if level[i] == KEEP {
                (k * rows[i].weight).clamp(0.0, 1.0)
            } else {
                level[i]
            }
        } else if partial {
            let l = (k * rows[i].weight).min(1.0);
            if l < rows[i].min_level { 0.0 } else { l }
        } else {
            1.0
        };
        level[i] = l;
        used += want[i] * l;
    }
    (used.min(budget), partial)
}

/// Hashes the allocation settings of a piece's consumers (tier, weight; in allocation order).
pub(crate) fn hash_consumers(piece: &Piece, h: &mut StateHasher) {
    h.u32(piece.consumers.len() as u32);
    for c in &piece.consumers {
        h.u32(c.part);
        h.u8(c.tier as u8);
        h.f32(c.weight);
    }
}

impl Ship {
    /// Rebuilds a piece's power lists: reactors, consumers (in allocation order) and parts that make heat
    /// without a power role (capacitors). Parts' stats and the ship's power settings must be current.
    pub(crate) fn rebuild_power_lists(&self, piece: &mut Piece) {
        piece.reactors.clear();
        piece.consumers.clear();
        piece.heaters.clear();
        let d = &*self.design;
        for &p in &piece.parts {
            let spec = &d.parts[p as usize];
            let st = &self.stats[p as usize];
            if spec.reactor {
                piece.reactors.push(Reactor {
                    part: p,
                    output: st.get(Stat::PowerOutput),
                });
            } else if let Some(kind) = spec.consumer {
                let set = self.power_settings[p as usize];
                piece.consumers.push(Consumer {
                    part: p,
                    kind,
                    slot: d.slot_of_part[p as usize],
                    draw: st.get(Stat::PowerDraw),
                    tier: set.tier,
                    weight: set.weight,
                    min_level: spec.power_min_level,
                });
            } else if st.get(Stat::HeatOutput) > 0.0 {
                piece.heaters.push(p);
            }
        }
        sort_rows(&mut piece.consumers);
    }

    /// The tier and weight of a part that draws power (`None` for other parts and unknown indices).
    pub fn power_setting(&self, part: PartIdx) -> Option<PowerSetting> {
        let i = part as usize;
        self.design
            .parts
            .get(i)
            .filter(|s| s.consumer.is_some())
            .map(|_| self.power_settings[i])
    }

    /// Changes a consumer's tier and weight; the allocation uses it from the next systems update on. The
    /// blueprint's setting is the start value; this is the runtime override (player, later behaviors). Returns
    /// `false` and changes nothing for parts that draw no power and for weights outside `(0, MAX_POWER_WEIGHT]`.
    /// Survives splits. Prefer `ShipCommand::Power` from outside the sim, so replays see the change.
    pub fn set_power_setting(&mut self, part: PartIdx, setting: PowerSetting) -> bool {
        let w = setting.weight;
        if self.power_setting(part).is_none() || !(w > 0.0 && w <= MAX_POWER_WEIGHT) {
            return false;
        }
        self.power_settings[part as usize] = setting;
        let piece = self.parts[part as usize].piece;
        if let Some(p) = self.piece_mut(piece)
            && let Some(row) = p.consumers.iter_mut().find(|c| c.part == part)
        {
            row.tier = setting.tier;
            row.weight = w;
            sort_rows(&mut p.consumers);
            // The idle order (GDR-0044) follows the allocation order.
            p.control_dirty = true;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(part: u32, tier: PowerTier, weight: f32, min_level: f32) -> Consumer {
        Consumer {
            part,
            kind: ConsumerKind::Other,
            slot: crate::design::NO_SLOT,
            draw: 0.0,
            tier,
            weight,
            min_level,
        }
    }

    fn run(mut rows: Vec<Consumer>, want: &[f32], supply: f32) -> (Vec<(u32, f32)>, f32, f32) {
        // Pair wants with parts, sort, allocate, and report levels by part.
        let mut pairs: Vec<(Consumer, f32)> = rows.drain(..).zip(want.iter().copied()).collect();
        let mut r: Vec<Consumer> = pairs.iter().map(|p| p.0).collect();
        sort_rows(&mut r);
        pairs.sort_by_key(|p| r.iter().position(|q| q.part == p.0.part).unwrap());
        let w: Vec<f32> = pairs.iter().map(|p| p.1).collect();
        let mut level = vec![9.0; r.len()];
        let (demand, served) = allocate(&r, &w, supply, &mut level);
        let mut out: Vec<(u32, f32)> = r.iter().map(|c| c.part).zip(level).collect();
        out.sort_by_key(|x| x.0);
        (out, demand, served)
    }

    use PowerTier::*;

    #[test]
    fn tiers_in_order_partial_tier_by_weight_lower_tiers_nothing() {
        let rows = vec![
            row(0, Low, 1.0, 0.0),
            row(1, Critical, 1.0, 0.0),
            row(2, High, 1.0, 0.0),
            row(3, High, 3.0, 0.0),
            row(4, Normal, 1.0, 0.0),
            row(5, Off, 1.0, 0.0),
        ];
        // Critical 2 fits; High 4 + 4 does not fit into 6: k = 6 / (1·4 + 3·4) = 0.375 → 0.375 and 1.125 → the
        // weight-3 part saturates at 1 (4), the other gets the remaining 2 (0.5). Normal and Low get nothing.
        let (lv, demand, served) = run(rows, &[5.0, 2.0, 4.0, 4.0, 3.0, 7.0], 8.0);
        assert_eq!(
            lv,
            vec![(0, 0.0), (1, 1.0), (2, 0.5), (3, 1.0), (4, 0.0), (5, 0.0)]
        );
        assert_eq!(demand, 18.0, "Off does not count");
        assert_eq!(served, 8.0);
    }

    #[test]
    fn equal_weights_share_equal_levels() {
        let rows = vec![
            row(0, Normal, 1.0, 0.0),
            row(1, Normal, 1.0, 0.0),
            row(2, Normal, 1.0, 0.0),
        ];
        let (lv, _, served) = run(rows, &[2.0, 4.0, 6.0], 6.0);
        assert!(lv.iter().all(|x| x.1 == 0.5), "{lv:?}");
        assert_eq!(served, 6.0);
    }

    #[test]
    fn minimum_level_drops_lowest_weight_first_and_shares_its_power() {
        // Three equal-weight parts at level 0.5 each; part 2 needs 0.6: it drops (lowest weight ties → highest
        // part index first), and parts 0 and 1 share 6 → 0.75.
        let rows = vec![
            row(0, Normal, 1.0, 0.0),
            row(1, Normal, 1.0, 0.0),
            row(2, Normal, 1.0, 0.6),
        ];
        let (lv, _, served) = run(rows, &[4.0, 4.0, 4.0], 6.0);
        assert_eq!(lv, vec![(0, 0.75), (1, 0.75), (2, 0.0)]);
        assert_eq!(served, 6.0);
        // Dropping the low-weight part lifts the next one over its minimum: it keeps running.
        let rows = vec![row(0, Normal, 2.0, 0.6), row(1, Normal, 1.0, 0.6)];
        // k = 4 / (2·4 + 1·4) = 1/3 → 0.667 and 0.333: part 1 drops, part 0 gets 4/4 = 1.0.
        let (lv, _, _) = run(rows, &[4.0, 4.0], 4.0);
        assert_eq!(lv, vec![(0, 1.0), (1, 0.0)]);
    }

    #[test]
    fn a_tier_left_fully_served_passes_the_rest_on() {
        // High: a 10 MW part with minimum 0.5 cannot get 5 of the 4 left after the critical part; it drops,
        // the small high part is fully served, and the normal tier gets what is left.
        let rows = vec![
            row(0, Critical, 1.0, 0.0),
            row(1, High, 1.0, 0.5),
            row(2, High, 1.0, 0.0),
            row(3, Normal, 1.0, 0.0),
        ];
        let (lv, demand, served) = run(rows, &[2.0, 10.0, 1.0, 4.0], 6.0);
        assert_eq!(lv, vec![(0, 1.0), (1, 0.0), (2, 1.0), (3, 0.75)]);
        assert_eq!((demand, served), (17.0, 6.0));
    }

    #[test]
    fn idle_rows_show_their_tier_level() {
        let rows = vec![
            row(0, High, 1.0, 0.0),
            row(1, Normal, 1.0, 0.0),
            row(2, Normal, 2.0, 0.0),
            row(3, Low, 1.0, 0.0),
        ];
        // Normal is shared at k = 3 / 4 (only part 1 asks): an idle weight-2 part would run at 1, the idle low
        // part gets nothing.
        let (lv, _, _) = run(rows.clone(), &[0.0, 4.0, 0.0, 0.0], 3.0);
        assert_eq!(lv, vec![(0, 1.0), (1, 0.75), (2, 1.0), (3, 0.0)]);
        // No supply: nothing runs.
        let (lv, demand, served) = run(rows, &[0.0; 4], 0.0);
        assert!(lv.iter().all(|x| x.1 == 0.0), "{lv:?}");
        assert_eq!((demand, served), (0.0, 0.0));
    }
}
