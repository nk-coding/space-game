//! Sensors and scanners (GDR-0017, GDR-0080, GDR-0093; galaxy plan E3, `docs/sim.md` "Sensors and scans").
//!
//! * **Sensors**: a vessel sees as far as the longest `sensor.range` of the working parts (alive, not overheated or
//!   idle) on its controlling piece while that piece is active, and never less than [`tuning::ALWAYS_SEEN`]
//!   (GDR-0017: nearby ships are always visible). Behaviors pick targets only within it; the voyage director shows
//!   sites within it.
//! * **Scans**: `ShipCommand::Scan` (the SCAN key, the `scan` behavior action) starts one on the working scanner of
//!   the controlling piece with the longest range (the lowest part index on a tie), if the ship's scanner is ready.
//!   The scan charges while that scanner works: its power row asks for its draw only now
//!   (`ConsumerKind::Scanner`), and `charge` seconds at full power fill it (less power, slower). Then it sweeps
//!   (`SimEvent::Scanned` from the controlling piece's centre of mass, with the scanner's range and wake radius) and
//!   cools down for `cooldown` seconds. A scan whose scanner dies or leaves the controlling piece stops (no sweep, no
//!   cooldown). What a sweep reveals is the voyage director's (`voyage/mod.rs`).

use super::*;

/// Placeholder numbers of sensing.
pub mod tuning {
    /// Every vessel sees at least this far (m), with or without sensors (GDR-0017).
    pub const ALWAYS_SEEN: f32 = 150.0;
}

/// A ship's scan ([`Ship::scan`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Scan {
    /// No scan underway: one may start.
    #[default]
    Ready,
    /// Charging on scanner `part`: `charged` seconds at full power of its `charge` so far.
    Charging { part: PartIdx, charged: f32 },
    /// Swept with scanner `part`; the next may start in `left` seconds.
    Cooling { part: PartIdx, left: f32 },
}

impl Scan {
    /// The scanner a scan charges on now.
    pub fn charging(self) -> Option<PartIdx> {
        match self {
            Scan::Charging { part, .. } => Some(part),
            _ => None,
        }
    }
}

/// A sweep a scan made this tick ([`Ship::advance_scan`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Sweep {
    pub at: Vec2,
    pub range: f32,
    pub wake: f32,
}

impl Ship {
    /// The scan underway or cooling down.
    pub fn scan(&self) -> Scan {
        self.scan
    }

    /// The working part of the controlling piece the scan would use now: the scanner with the longest range (the
    /// lowest part index on a tie), alive and neither overheated nor idle.
    pub fn scanner(&self) -> Option<(PartIdx, &data::ScannerSpec)> {
        let control = self.control?;
        let mut best: Option<(PartIdx, &data::ScannerSpec)> = None;
        for &i in &self.design.scanners {
            let ps = &self.parts[i as usize];
            let Some(spec) = &self.design.parts[i as usize].scanner else { continue };
            if ps.alive && ps.piece == control && !ps.overheated && !ps.idle && best.is_none_or(|(_, b)| spec.range > b.range) {
                best = Some((i, spec));
            }
        }
        best
    }

    /// Can a scan start now: a working scanner ([`Ship::scanner`]), no scan underway or cooling down, and a vessel
    /// that runs?
    pub fn can_scan(&self) -> bool {
        self.scan == Scan::Ready && self.is_active() && self.scanner().is_some()
    }

    /// Starts a scan if [`Ship::can_scan`]; returns whether it did.
    pub(crate) fn start_scan(&mut self) -> bool {
        let Some((part, _)) = self.scanner().filter(|_| self.can_scan()) else {
            return false;
        };
        self.scan = Scan::Charging { part, charged: 0.0 };
        true
    }

    /// One tick of the scan, after `update_systems`: a charging scan fills by its scanner's work this tick and
    /// sweeps when full; a cooling one counts down.
    pub(crate) fn advance_scan(&mut self) -> Option<Sweep> {
        match self.scan {
            Scan::Ready => None,
            Scan::Charging { part, charged } => {
                let ps = &self.parts[part as usize];
                let spec = self.design.parts[part as usize].scanner.as_ref()?;
                if !ps.alive || Some(ps.piece) != self.control {
                    self.scan = Scan::Ready;
                    return None;
                }
                let charged = charged + ps.work * DT;
                if charged < spec.charge {
                    self.scan = Scan::Charging { part, charged };
                    return None;
                }
                self.scan = Scan::Cooling { part, left: spec.cooldown };
                let at = self.control_piece().and_then(|c| self.piece(c))?.com_world();
                Some(Sweep { at, range: spec.range, wake: spec.wake })
            }
            Scan::Cooling { part, left } => {
                let left = left - DT;
                self.scan = if left > 0.0 { Scan::Cooling { part, left } } else { Scan::Ready };
                None
            }
        }
    }

    /// How far the vessel sees now (see the module docs): its working sensors on the controlling piece while it
    /// is active, at least [`tuning::ALWAYS_SEEN`]; nothing while static blinds it (GDR-0125).
    pub fn sensor_range(&self) -> f32 {
        if self.blinded() {
            return 0.0;
        }
        let mut r = tuning::ALWAYS_SEEN;
        let Some(piece) = self.control_piece().and_then(|c| self.piece(c)).filter(|p| p.active) else {
            return r;
        };
        for &i in &self.design.sensors {
            let ps = &self.parts[i as usize];
            if ps.alive && ps.piece == piece.id && !ps.overheated && !ps.idle {
                r = r.max(self.design.parts[i as usize].sensor);
            }
        }
        r
    }
}

impl StateHash for Scan {
    fn hash_state(&self, h: &mut StateHasher) {
        match *self {
            Scan::Ready => h.u8(0),
            Scan::Charging { part, charged } => {
                h.u8(1);
                h.u32(part);
                h.f32(charged);
            }
            Scan::Cooling { part, left } => {
                h.u8(2);
                h.u32(part);
                h.f32(left);
            }
        }
    }
}
