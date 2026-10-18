//! Part-stat steps (GDR-0024's refinement): the player's design built with a career's steps, every other one with
//! the content's numbers.

use sim::Stat;
use sim::{ShipDesign, SimConfig};

fn base_of(d: &ShipDesign, part: &str, stat: Stat) -> f32 {
    d.parts.iter().find(|p| p.id == part).unwrap().base.get(stat)
}

#[test]
fn steps_change_the_base_stats_of_the_parts_they_pick() {
    let c = data::base_content();
    let skiff = c.ships.get("rim_skiff").unwrap();
    let plain = ShipDesign::compile(&c, skiff).unwrap();
    // No steps: the same numbers as a plain compile.
    let none = ShipDesign::compile_upgraded(&c, skiff, &data::Upgrades::NONE).unwrap();
    assert!(plain.parts.iter().zip(&none.parts).all(|(a, b)| a.base == b.base));
    let all = ["shield_regen_3", "shield_regen_2", "shield_regen_1", "radiator_vent_1", "thruster_speed_1", "pulse_rate_1", "pulse_rate_2"];
    let up = data::Upgrades::of_research(&c, all);
    let d = ShipDesign::compile_upgraded(&c, skiff, &up).unwrap();
    let ratio = |part, stat| base_of(&d, part, stat) / base_of(&plain, part, stat);
    assert!((ratio("radiator", Stat::HeatVent) - 1.08).abs() < 1e-5);
    assert!((ratio("thruster", Stat::RatedSpeed) - 1.04).abs() < 1e-5);
    assert!((ratio("pulse_laser", Stat::FireRate) - 1.06).abs() < 1e-5);
    assert!((ratio("shield_emitter", Stat::ShieldRegen) - 1.24).abs() < 1e-5);
    // What no step picks stays.
    assert_eq!(base_of(&d, "pulse_laser", Stat::Range), base_of(&plain, "pulse_laser", Stat::Range));
    // The design report sees the same numbers: a faster thruster, a higher top speed; more firepower.
    let cfg = SimConfig::default();
    let (a, b) = (sim::design_report(&c, skiff, &cfg).unwrap(), sim::design_report_upgraded(&c, skiff, &cfg, &up).unwrap());
    assert!(b.flight.top_speed[b.flight.best] > a.flight.top_speed[a.flight.best]);
    assert!(b.latent_firepower() > a.latent_firepower());
}
