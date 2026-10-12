//! The behavior think pass allocates nothing once warmed up (built-in nodes); Rhai conditions allocate a
//! little inside Rhai. Counted per thread with a counting global allocator.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;

use sim::{Body, ShipCommand, ShipDesign, Sim, TickInputs, Vec2};

struct Counting;

thread_local! {
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.with(|c| c.set(c.get() + 1));
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn allocs() -> u64 {
    ALLOCS.with(|c| c.get())
}

/// Ten rules, built-in nodes only (`examples/behavior_perf.rs` uses the same program).
const TEN_RULES: &str = "target nearest(engines)
when hull < 20% -> flee
when heat > 95% -> kite(150%)
when power < 50% -> hold
when shield_down and target_on_me -> orbit(90%, ccw)
when under_attack and enemies_near(60m) >= 3 -> flee
when target_hull < 15% -> approach(30%)
when target_in_range(130%) and cycle(10s) >= 6s -> orbit(60%)
when target_time > 60s -> orbit(80%, cw)
when not target_in_range(200%) -> approach(67%)
always -> approach(67%)";

fn fleet(program: &str) -> Sim {
    let c = data::base_content();
    let d = |id: &str| Arc::new(ShipDesign::compile(&c, c.ships.get(id).unwrap()).unwrap());
    let (runner, early) = (d("halvex_runner"), d("sample_early"));
    let mut w = Sim::new(3);
    w.set_behaviors(sim::BehaviorLibrary::new(&c).unwrap());
    let id = w.behaviors_mut().add_text("ten", program).unwrap();
    for k in 0..10 {
        let x = (k as f32 - 4.5) * 25.0;
        let a = w.spawn_ship(
            runner.clone(),
            sim::Owner(1),
            Body::at(Vec2::new(x, 0.0), 0.0),
        );
        let b = w.spawn_ship(
            early.clone(),
            sim::Owner(2),
            Body::at(Vec2::new(x, 90.0), core::f32::consts::PI),
        );
        w.command(a, ShipCommand::Behavior(Some(id)));
        w.command(b, ShipCommand::Behavior(Some(id)));
    }
    w
}

#[test]
fn think_pass_allocations() {
    let mut w = fleet(TEN_RULES);
    for _ in 0..30 {
        w.step(&TickInputs::default());
    }
    w.think_all_behaviors_now();
    let before = allocs();
    for _ in 0..100 {
        w.think_all_behaviors_now();
    }
    assert_eq!(
        allocs() - before,
        0,
        "built-in nodes: no allocation per think"
    );

    // With a Rhai condition evaluated at every think: a few small allocations inside Rhai.
    let scripted = TEN_RULES.replace(
        "when hull < 20% -> flee",
        "when closing_in(1s) and hull < 20% -> flee",
    );
    let mut w = fleet(&scripted);
    for _ in 0..30 {
        w.step(&TickInputs::default());
    }
    w.think_all_behaviors_now();
    let (before, evals) = (allocs(), w.script_evaluations());
    for _ in 0..100 {
        w.think_all_behaviors_now();
    }
    let per_eval = (allocs() - before) as f64 / (w.script_evaluations() - evals).max(1) as f64;
    println!("Rhai: {per_eval:.2} allocations per condition evaluation");
    assert!(per_eval <= 4.0, "{per_eval}");
}
