//! Headless, deterministic fights between designs on the sim (ADR-0036, GDR-0083; `docs/arena.md`).
//!
//! A [`FightSpec`] names two sides of entries (a design as a content ship id or blueprint text, a behavior as a
//! content id or program text, a count, a formation), the start distance and the time limit; it has a RON text
//! form. [`run_fight`] builds the world (each side its own sim owner, every core running) and steps it until one
//! side has had no running vessel for 5 s or the time limit is reached; [`Fight`] does the same a slice at a
//! time. The result carries the outcome, build points per side at the start and lost, damage dealt, vessels
//! left, the arena score ([`arena_score`]) and the final state hash. [`run_seeds`] and [`Summary`] average over
//! seeds.
//!
//! [`VoyageSpec`], [`run_voyage`] and [`voyage_score`] score a design by the slice's voyages (the headless voyage
//! runner, `sim::voyage_runner`). [`evolve`] breeds designs and behaviors ([`Run`], configured by a [`RunConfig`]).
//!
//! No Bevy, no I/O, no threads, no wall clock; builds for wasm32. The same spec gives the same result and hash
//! on every platform (the sim's determinism rules apply here too: ordered containers, `sim::math` for any
//! transcendental function). The native `arena` binary (`src/bin/arena.rs`) runs batches on every core.
//!
//! ```
//! let content = data::base_content();
//! let mut spec = arena::FightSpec::new(
//!     7,
//!     vec![arena::Entry::ship("halvex_runner")],
//!     vec![arena::Entry::ship("automaton_raider")],
//! );
//! spec.time_limit = 5.0;
//! let r = arena::run_fight(&content, &spec).unwrap();
//! assert!(r.ticks <= 150 && (0.0..=1.0).contains(&r.sides[0].score));
//! ```

#![forbid(unsafe_code)]

mod error;
pub mod evolve;
mod fight;
mod score;
mod spec;
mod voyage;

pub use error::ArenaError;
pub use fight::{
    FORMATION_GAP, Fight, FightResult, JITTER, OUT_AFTER, OWNERS, Outcome, SideResult, run_fight,
};
pub use score::{Summary, arena_score, run_seeds};
pub use spec::{
    BehaviorRef, DEFAULT_START_DISTANCE, DEFAULT_TIME_LIMIT, DesignRef, Entry, FightSpec,
    Formation, MAX_SHIPS_PER_SIDE, MAX_START_DISTANCE, MAX_TIME_LIMIT, Side,
};
pub use evolve::{Genome, Run, RunConfig, RunError};
pub use voyage::{
    DEFAULT_VOYAGE_MINUTES, MAX_VOYAGE_MINUTES, VoyageSpec, run_voyage, start_voyage,
    voyage_fitness, voyage_score,
};
