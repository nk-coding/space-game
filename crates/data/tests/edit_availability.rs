//! Part availability in the drydock editing model (GDR-0082): with a count set, the design holds at most that
//! many of a part, counting the parts already in it; every tool that puts parts down is refused with
//! `Refusal::NoneLeft` when none is left, a mirror image that would need one more is skipped, fill fills as many
//! cells as are left. The career gives every part unlimited for now.

use data::career::{Availability, Career};
use data::edit::{Drydock, Refusal, Twin};
use data::{Blueprint, Content, Placement};

fn empty(c: &Content) -> Drydock {
    let blank = Blueprint { parts: Vec::new(), ..c.ships.get("halvex_runner").unwrap().clone() };
    Drydock::open(c, &blank, (12, 12)).unwrap()
}

fn block(c: &Content) -> String {
    c.unit_hull_part(data::Shape::Rect).unwrap().id.clone()
}

fn machine(part: &str, x: i32, y: i32) -> Placement {
    Placement { part: part.into(), x, y, rot: 0, flip: false, material: None, power: None }
}

#[test]
fn a_count_limits_placing_counting_the_parts_in_the_design() {
    let c = data::base_content();
    let mut dock = empty(&c);
    assert_eq!(dock.left("radiator"), Availability::Unlimited);
    dock.place(machine("radiator", 1, 1)).unwrap();
    dock.set_available("radiator", Availability::Count(2));
    assert_eq!(dock.left("radiator"), Availability::Count(1));
    let revision = dock.revision();
    dock.place(machine("radiator", 3, 1)).unwrap();
    assert_eq!(dock.left("radiator"), Availability::Count(0));
    let none = Err(Refusal::NoneLeft("radiator".into()));
    assert_eq!(dock.check(&machine("radiator", 5, 1)), none);
    assert_eq!(dock.preview_place(&machine("radiator", 5, 1)).map(|_| ()), none);
    assert_eq!(dock.place(machine("radiator", 5, 1)).map(|_| ()), none);
    assert_eq!(dock.parts().len(), 2);
    // Other parts are not limited; erasing gives one back; setting a count is no edit.
    dock.place(machine("capacitor", 5, 1)).unwrap();
    assert_eq!(dock.revision(), revision + 2);
    dock.erase((1, 1)).unwrap();
    assert_eq!(dock.left("radiator"), Availability::Count(1));
    dock.place(machine("radiator", 7, 1)).unwrap();
    // A count below what the design holds leaves the parts in place: none left, never negative.
    dock.set_available("radiator", Availability::Count(1));
    assert_eq!((dock.left("radiator"), dock.parts().iter().filter(|p| p.part == "radiator").count()), (Availability::Count(0), 2));
    // Undo restores its step whatever the count.
    dock.undo();
    dock.undo();
    assert_eq!(dock.parts().iter().filter(|p| p.part == "radiator").count(), 2);
    dock.set_available("radiator", Availability::Unlimited);
    assert!(dock.place(machine("radiator", 9, 1)).is_ok());
}

#[test]
fn paint_fill_slopes_and_mirror_images_use_up_the_count() {
    let c = data::base_content();
    let block = block(&c);
    let mut dock = empty(&c);
    dock.set_available(&block, Availability::Count(4));
    // Mirror on: two left paint the cell and its mirror image; one left paints the cell and skips the image.
    dock.paint((0, 0), "steel").unwrap();
    dock.mirror = true;
    assert_eq!(dock.paint((1, 0), "steel").unwrap().twin, Twin::Applied);
    assert_eq!(dock.preview_paint((2, 0), "steel").unwrap().twin, Twin::Skipped(Refusal::NoneLeft(block.clone())));
    assert_eq!(dock.paint((2, 0), "steel").unwrap().twin, Twin::Skipped(Refusal::NoneLeft(block.clone())));
    assert_eq!(dock.left(&block), Availability::Count(0));
    assert_eq!(dock.paint((3, 0), "steel").map(|_| ()), Err(Refusal::NoneLeft(block.clone())));
    dock.mirror = false;
    // Repainting a block needs none.
    assert!(dock.paint((0, 0), "composite").unwrap().changed);
    assert_eq!(dock.fill((0, 4), (5, 5), "steel").map(|_| ()), Err(Refusal::NoneLeft(block.clone())));
    // Fill fills as many cells as are left, row by row.
    dock.set_available(&block, Availability::Count(8));
    dock.fill((0, 4), (5, 5), "steel").unwrap();
    assert_eq!(dock.parts().len(), 8);
    assert!(dock.part_at((3, 4)).is_some() && dock.part_at((4, 4)).is_none());
    // A mirrored placement with one left places the part and skips its twin.
    dock.set_available("radiator", Availability::Count(1));
    dock.mirror = true;
    let o = dock.place(machine("radiator", 0, 8)).unwrap();
    assert_eq!(o.twin, Twin::Skipped(Refusal::NoneLeft("radiator".into())));
    // A slope with none left is refused; turning or repainting one in the design is not.
    let slope = c.unit_hull_part(data::Shape::Slope).unwrap().id.clone();
    dock.mirror = false;
    dock.place_slope(&slope, (8, 2), "steel").unwrap();
    dock.set_available(&slope, Availability::Count(1));
    assert_eq!(dock.place_slope(&slope, (10, 2), "steel").map(|_| ()), Err(Refusal::NoneLeft(slope.clone())));
    assert!(dock.place_slope(&slope, (8, 2), "steel").unwrap().changed);
    assert!(dock.place_slope(&slope, (8, 2), "composite").unwrap().changed);
}

#[test]
fn the_career_gives_every_part_unlimited_for_now() {
    let c = data::base_content();
    let career = Career::new(&c, "rim_awakening", 1).unwrap();
    assert!(c.parts.iter().all(|p| career.available(&p.id) == Availability::Unlimited));
}
