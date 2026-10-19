//! Drones (GDR-0018 "drones", GDR-0117): hangars that launch small vessels of their own.
//!
//! * **A hangar** (`ShipDesign::hangars`) holds `capacity` drones, out and in the bay together. Its bay
//!   ([`Bay`]) starts full. While the hangar works (alive, not overheated, not idle, on its vessel's controlling
//!   piece) it rebuilds lost drones one at a time, `rebuild` seconds each at full power (less power, slower).
//! * **Launch**: while its vessel is running (not dormant, not dark) and aims at something (its behavior's target,
//!   or the pilot's lock), a working hangar with a drone in the bay launches one every [`tuning::LAUNCH_TICKS`],
//!   if the cells in front of its mouth hold no part and fewer than [`tuning::MAX_DRONES`] drones live in the
//!   world. The drone spawns just beyond the mouth, facing out, with the carrier's velocity at the mouth plus
//!   [`tuning::LAUNCH_SPEED`]; it flies its design's behavior with the carrier as its leader (`locked`: what the
//!   carrier aims at) and belongs to the carrier's owner.
//! * **Leash**: the behavior condition `off_leash` is true while a drone's controlling piece is more than its
//!   hangar's `leash` from the carrier's (the drone behaviors fly back then).
//! * **Death**: a drone whose hangar stops being one (destroyed, its carrier lost or no vessel, the hangar on a
//!   piece that broke off) dies with it, and so does a drone that lost its core: it is removed at once without a
//!   wreck (`SimEvent::DroneLost`), so a flood never leaves a field of debris. A dormant or dark carrier keeps its
//!   drones.
//! * **Not the fleet**: drones take no mind capacity, never go dormant and are never their owner's focus;
//!   `Sim::vessels` leaves them out (the fleet), `Sim::drones` lists them. They have no `VesselFormed` or
//!   `VesselLost` (their own `DroneLaunched` and `DroneLost`), so kills and losses count ships, and they neither
//!   block nor allow a recall, nor spawn or reveal a voyage's sites. A voyage site's carrier takes its drones in.
//!
//! Order: the pass runs once per tick after the impacts, before the voyage director (so a site takes in its
//! carriers' drones in the same tick): lost drones are removed in ascending ship id, then the carriers' bays
//! advance and launch in ascending ship index and hangar order. No randomness.

use std::sync::Arc;

use super::*;
use crate::ship::vessel::VesselState;
use crate::events::SimEvent;
use crate::world::{ShipCommand, Sim};

/// Placeholders (lead defaults, GDR-0117).
pub mod tuning {
    /// Most drones alive in the world at once: the performance cap (a full hive flood is 24).
    pub const MAX_DRONES: usize = 48;
    /// Ticks between two launches of one hangar (0.3 s).
    pub const LAUNCH_TICKS: u32 = 9;
    /// Speed a drone leaves the mouth with, on top of the carrier's (m/s).
    pub const LAUNCH_SPEED: f32 = 8.0;
    /// Gap between the mouth and the drone's hull at launch (m).
    pub const LAUNCH_GAP: f32 = 0.3;
}

/// One hangar's bay: drones ready to launch, the rebuild under way, the launch cooldown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bay {
    /// Drones in the bay.
    pub ready: u32,
    /// Seconds of work towards the next drone.
    pub progress: f32,
    /// Ticks until it may launch again.
    pub cooldown: u32,
}

impl Bay {
    pub(crate) fn full(capacity: u32) -> Bay {
        Bay { ready: capacity, progress: 0.0, cooldown: 0 }
    }

    pub(crate) fn empty() -> Bay {
        Bay { ready: 0, progress: 0.0, cooldown: 0 }
    }

    pub(crate) fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.ready);
        h.f32(self.progress);
        h.u32(self.cooldown);
    }
}

/// What a drone belongs to: its carrier and the hangar (index into the carrier's `ShipDesign::hangars`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tie {
    pub carrier: ShipId,
    pub hangar: u32,
}

impl Ship {
    /// The carrier and hangar of a drone (`None` for every other ship).
    pub fn drone(&self) -> Option<Tie> {
        self.drone
    }

    /// Each hangar's bay, in `ShipDesign::hangars` order.
    pub fn bays(&self) -> &[Bay] {
        &self.bays
    }

    /// Does hangar `h` work now: alive, not overheated, not idle, on the controlling piece of a vessel?
    pub(crate) fn hangar_works(&self, h: usize) -> bool {
        let Some(m) = self.design.hangars.get(h) else {
            return false;
        };
        let ps = &self.parts[m.part as usize];
        ps.alive && !ps.overheated && !ps.idle && Some(ps.piece) == self.control && self.is_vessel()
    }

    /// Hangar `h` still holds its drones: alive on the controlling piece of a vessel (overheated, idle or dark
    /// it keeps them).
    fn hangar_holds(&self, h: usize) -> bool {
        self.design.hangars.get(h).is_some_and(|m| {
            let ps = &self.parts[m.part as usize];
            ps.alive && Some(ps.piece) == self.control && self.is_vessel()
        })
    }

    /// Is the mouth of hangar `h` open (no part in the cells in front of it)?
    fn mouth_open(&self, h: usize) -> bool {
        self.design.hangars[h].front.iter().all(|&c| self.grid.occupant(c).is_none())
    }
}

impl Sim {
    /// The drones in the world (ascending ship id).
    pub fn drones(&self) -> impl Iterator<Item = &Ship> + '_ {
        self.ships.iter().filter(|s| s.drone.is_some())
    }

    /// The drone pass (see the module docs).
    pub(crate) fn drones_step(&mut self) {
        let mut ties = std::mem::take(&mut self.drone_scratch);
        ties.clear();
        // Drones whose hangar is gone, or that lost their core.
        let mut lost = Vec::new();
        for s in &self.ships {
            let Some(t) = s.drone else { continue };
            let holds = self.ship(t.carrier).is_some_and(|c| c.hangar_holds(t.hangar as usize));
            if holds && s.is_vessel() {
                ties.push(t);
            } else {
                let at = s.control.and_then(|c| s.piece(c)).or(s.pieces.first()).map_or(Vec2::ZERO, |p| p.com_world());
                lost.push((s.id, t.carrier, at));
            }
        }
        for (ship, carrier, at) in lost {
            self.events.push(SimEvent::DroneLost { ship, carrier, at });
            self.remove_ship(ship);
        }
        ties.sort_unstable();
        let mut live = ties.len();
        let mut launches = Vec::new();
        for k in 0..self.ships.len() {
            let s = &self.ships[k];
            if s.design.hangars.is_empty() {
                continue;
            }
            let id = s.id;
            let engaged = s.vessel_state() == Some(VesselState::Running) && crate::behavior::aims_at(s).is_some();
            for h in 0..s.design.hangars.len() {
                let s = &self.ships[k];
                let m = &s.design.hangars[h];
                let tie = Tie { carrier: id, hangar: h as u32 };
                let out = ties.partition_point(|t| *t <= tie) - ties.partition_point(|t| *t < tie);
                let works = s.hangar_works(h);
                let work = s.parts[m.part as usize].work;
                let open = s.mouth_open(h);
                let (capacity, rebuild) = (m.capacity, m.rebuild);
                let bay = &mut self.ships[k].bays[h];
                bay.cooldown = bay.cooldown.saturating_sub(1);
                if works && out as u32 + bay.ready < capacity {
                    bay.progress += work * crate::DT;
                    if bay.progress >= rebuild {
                        bay.progress = 0.0;
                        bay.ready += 1;
                    }
                }
                if works && engaged && open && bay.ready > 0 && bay.cooldown == 0 && live < tuning::MAX_DRONES {
                    bay.ready -= 1;
                    bay.cooldown = tuning::LAUNCH_TICKS;
                    live += 1;
                    launches.push((k, h));
                }
            }
        }
        self.drone_scratch = ties;
        // Launch after the pass: spawning appends ships, so the indices above stay valid until here.
        for (k, h) in launches {
            self.launch(k, h);
        }
    }

    /// Spawns hangar `h`'s drone of ship `k` just beyond its mouth (see the module docs).
    fn launch(&mut self, k: usize, h: usize) {
        let s = &self.ships[k];
        let m = &s.design.hangars[h];
        let Some(p) = s.control.and_then(|c| s.piece(c)) else {
            return;
        };
        let (carrier, owner, part, design) = (s.id, s.owner, m.part, Arc::clone(&m.drone));
        let rot = Rot::from_angle(p.body.angle);
        let (dx, dy) = m.facing.delta();
        // Blueprint y runs down, the piece's up.
        let out = rot.apply(Vec2::new(dx as f32, -dy as f32));
        let mouth = p.body.to_world(crate::ship::mass::to_piece(m.mouth, p.origin));
        let reach = design.grid.bounds().map_or(1.0, |b| {
            let (w, h) = (b.width() as f32, b.height() as f32);
            0.5 * (w * w + h * h).sqrt()
        });
        let com = p.com_world();
        let v_mouth = p.body.linvel + Vec2::cross_scalar(p.body.angvel, mouth - com);
        // A drone's nose (+y at angle 0) points out of the mouth.
        let angle = crate::math::atan2(-out.x, out.y);
        let body = Body {
            pos: mouth + out * (reach + tuning::LAUNCH_GAP),
            angle,
            linvel: v_mouth + out * tuning::LAUNCH_SPEED,
            angvel: 0.0,
        };
        // A drone is one vessel and no member of the fleet: no `VesselFormed` (its `DroneLaunched` instead), no
        // further vessels formed from its other pieces, no mind capacity.
        let j = self.new_ship(Arc::clone(&design), owner, body);
        let claim = self.next_claim();
        let tick = self.tick();
        let s = &mut self.ships[j];
        let id = s.id;
        s.drone = Some(Tie { carrier, hangar: h as u32 });
        s.claim = claim;
        s.formed = tick;
        self.mind_dirty = true;
        self.update_minds();
        self.command(id, ShipCommand::Leader(Some(carrier)));
        if let Some(b) = self.behaviors().default_for(&design) {
            self.command(id, ShipCommand::Behavior(Some(b)));
        }
        self.events.push(SimEvent::DroneLaunched { ship: id, carrier, part });
    }
}
