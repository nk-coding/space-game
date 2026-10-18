//! A career's part-stat steps (GDR-0024's refinement) in fights and voyages: carried in the spec, so the same code
//! builds the same ships anywhere; left out of the text when there are none.

use arena::{DesignRef, Entry, FightSpec, VoyageSetup, VoyageSpec};
use sim::Stat;

fn steps(c: &data::Content) -> data::Upgrades {
    data::Upgrades::of_research(c, ["pulse_rate_1", "pulse_rate_2"])
}

#[test]
fn a_voyage_builds_its_fleet_with_the_specs_steps() {
    let c = data::base_content();
    let spec = |upgrades| VoyageSpec { voyage: "yard_approach".into(), seed: 2, design: DesignRef::Ship("rim_skiff".into()), behavior: None, minutes: 5.0, escorts: vec![DesignRef::Ship("rim_skiff".into())], upgrades };
    let rate = |s: &VoyageSpec| {
        let set = VoyageSetup::build(&c, s).unwrap();
        set.designs.iter().map(|(_, d)| d.parts.iter().find(|p| p.id == "pulse_laser").unwrap().base.get(Stat::FireRate)).collect::<Vec<_>>()
    };
    assert_eq!(rate(&spec(data::Upgrades::NONE)), [1.0, 1.0]);
    let up = spec(steps(&c));
    assert!(rate(&up).iter().all(|r| (r - 1.06).abs() < 1e-5));
    // An unusable step is refused, not flown.
    let mut bad = steps(&c);
    bad.0[0].stat = "mass".into();
    assert!(VoyageSetup::build(&c, &spec(bad)).is_err());
}

#[test]
fn a_sides_steps_are_in_its_text_only_when_it_has_some() {
    let c = data::base_content();
    let mut spec = FightSpec::new(1, vec![Entry::ship("rim_skiff")], vec![Entry::ship("rim_skiff")]);
    assert!(!spec.to_ron().contains("upgrades"));
    spec.sides[0].upgrades = steps(&c);
    let text = spec.to_ron();
    assert!(text.contains("upgrades") && text.contains("fire_rate"), "{text}");
    assert_eq!(FightSpec::from_ron(&text).unwrap(), spec);
    // Only the side that has them is built with them.
    let set = arena::FightSetup::build(&c, &spec).unwrap();
    let rate = |side: usize| set.designs[side][0].1.parts.iter().find(|p| p.id == "pulse_laser").unwrap().base.get(Stat::FireRate);
    assert!((rate(0) - 1.06).abs() < 1e-5);
    assert_eq!(rate(1), 1.0);
}
