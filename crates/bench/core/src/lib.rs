//! Engine-independent part of the physics benchmark: scenario generator, the `PhysicsEngine` trait every
//! adapter implements, the timing harness and state hashing.
//!
//! No platform APIs in here (no threads, no filesystem, no wall clock): time comes in through `Clock`, so the
//! same code runs natively, in wasm (Node / browser) and later on Android.

pub mod engine;
pub mod harness;
pub mod hash;
pub mod rng;
pub mod scenario;

pub use engine::{BodyState, Clock, EngineConfig, PhysicsEngine, RayStats, velocity_at};
pub use harness::{RunOptions, RunParams, RunResult, Stats, run};
pub use hash::StateHasher;
pub use scenario::{BodyDef, BodyKind, BodyRole, BreakEvent, ColliderMode, Ray, SCENARIOS, ScenarioSpec, Shape};
