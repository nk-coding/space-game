//! A stand-in pilot (GDR-0092 "The stand-in pilot"): a scripted pilot from content (`lab.ron`'s `pilots`, a
//! [`StandIn`]) that flies a piloted ship as a player might, so the design's pilot assists act and can be scored: they
//! run only on a piloted ship and take the channels its pilot leaves idle (GDR-0088). It decides on think ticks from the
//! world as it is and keeps no state of its own: its lock is the one it holds (the ship's helm), its stick and triggers
//! follow the clock ([`Duty`]) from tick 0. So it is deterministic, and a replay flies the same.

use data::{Duty, StandIn};

use crate::behavior::tuning::THINK_PERIOD;
use crate::flight::{AimMode, FIRE_ALL, HelmMode, PilotInput, Target, next_lock};
use crate::ids::ShipId;
use crate::math::Vec2;
use crate::world::Sim;

/// `d` is in an "on" stretch at `tick` (on for `on` seconds, then off for `off`; none: never).
fn on(d: Option<&Duty>, tick: u64) -> bool {
    let Some(d) = d else {
        return false;
    };
    let hz = crate::TICK_HZ as f32;
    let on = ((d.on * hz).round() as u64).max(1);
    let off = (d.off * hz).round() as u64;
    off == 0 || tick % (on + off) < on
}

/// Where a ship's controlling piece is, while it is an active vessel.
fn at(sim: &Sim, id: ShipId) -> Option<Vec2> {
    let s = sim.ship(id).filter(|s| s.is_active())?;
    Some(s.piece(s.control_piece()?)?.com_world())
}

/// The input the stand-in gives ship `me` (whose longest gun reaches `reach` metres) on a think tick: its lock (the
/// one it holds while that ship is active, else the auto-lock's pick as a player's client takes it, else the nearest
/// enemy when it locks itself), the stick towards its lock or the nearest enemy while that is farther than its share
/// of the reach, every trigger, each while its duty is on. `None` between thinks and once `me` is no vessel (the last
/// input stays).
pub fn stand_in(rule: &StandIn, sim: &Sim, me: ShipId, reach: f32) -> Option<PilotInput> {
    let tick = sim.tick();
    if !tick.is_multiple_of(THINK_PERIOD) {
        return None;
    }
    let here = at(sim, me)?;
    let ship = sim.ship(me)?;
    let held = match ship.helm {
        HelmMode::Pilot(p) => p.lock,
        _ => None,
    }
    .filter(|t| matches!(*t, Target::Ship(s) if at(sim, s).is_some()));
    let nearest = next_lock(sim.ships(), me, None);
    let auto = ship.assists().and_then(|a| a.auto_lock()).map(Target::Ship);
    let lock = held.or(auto).or(nearest.filter(|_| rule.lock));
    let way = match lock.or(nearest) {
        Some(Target::Ship(s)) => at(sim, s).map(|p| p - here),
        _ => None,
    };
    let stick = match way {
        Some(w) if on(rule.stick.as_ref(), tick) && w.length() > rule.close_to * reach => w * (1.0 / w.length()),
        _ => Vec2::ZERO,
    };
    Some(PilotInput {
        stick,
        lock,
        drift: false,
        fire: if on(rule.fire.as_ref(), tick) { FIRE_ALL } else { 0 },
        aim: AimMode::Auto,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_duty_is_on_then_off() {
        let d = Duty { on: 3.0, off: 2.0 };
        let hz = crate::TICK_HZ as u64;
        assert!(on(Some(&d), 0) && on(Some(&d), 3 * hz - 1));
        assert!(!on(Some(&d), 3 * hz) && !on(Some(&d), 5 * hz - 1));
        assert!(on(Some(&d), 5 * hz));
        assert!(on(Some(&Duty { on: 1.0, off: 0.0 }), 12_345));
        assert!(!on(None, 0));
    }
}
