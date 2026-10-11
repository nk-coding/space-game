//! Engine-independent, deterministic simulation core for Space Game. See `docs/sim.md` for the full picture.
//!
//! No Bevy, no graphics, no threads, no filesystem, no wall-clock; builds for wasm32. Ships are grids of parts
//! split into pieces (one rigid body each, with mass properties and convex colliders); power, heat, bonuses,
//! damage and breakage run per tick; an always-on flight computer turns pilot input, maneuvers or brains into
//! actuator commands. The physics engine plugs in behind [`PhysicsBackend`]; until it is chosen, the
//! engine-free [`FreeFlight`] backend moves the bodies (`Sim` = `Sim<FreeFlight>`).
//!
//! # Frames and units
//!
//! * **Blueprint frame** (`data` crate): x right, y down, 1 cell = 1 m, forward = −y.
//! * **Piece frame** (one rigid body): x right, **y up**, metres. Origin = a fixed blueprint point
//!   (`Piece::origin`, the piece's centre of mass when it was created): blueprint `(x, y)` → piece
//!   `(x − ox, oy − y)`. The ship's forward is +y.
//! * **World frame**: x right, y up, metres; angles counter-clockwise in radians. With angle 0 a ship's nose
//!   points to world +y. Matches the client's mapping `world = (x, −y)`.
//! * Units: mass "t" and thrust "kN" from content, so kN / t = m/s². Time step [`DT`] = 1/30 s.
//!
//! # Determinism
//!
//! Fixed step; [`rng`] streams per system; ordered containers only; [`math`] for every transcendental
//! function (libm, no FMA); stable ids ([`ids`]); [`Sim::state_hash`] every tick in debug builds.
//!
//! ```
//! use std::sync::Arc;
//! let content = data::base_content();
//! let design = Arc::new(sim::ShipDesign::compile(&content, content.ships.get("halvex_runner").unwrap()).unwrap());
//! let mut world = sim::Sim::new(42);
//! let ship = world.spawn_ship(design, sim::Body::default());
//! world.step(&sim::TickInputs::default());
//! let piece = &world.ship(ship).unwrap().pieces()[0];
//! assert!(piece.mass.mass > 0.0 && piece.colliders.len() > 0);
//! ```

#![forbid(unsafe_code)]

pub mod alloc;
pub mod breakage;
pub mod collider;
pub mod config;
pub mod design;
pub mod events;
pub mod flight;
pub mod hash;
pub mod ids;
pub mod maneuver;
pub mod mass;
pub mod math;
pub mod physics;
pub mod rng;
pub mod scenario;
pub mod ship;
pub mod world;

pub use alloc::{Actuation, TorqueSource};
pub use breakage::{DamageOutcome, DamageTarget};
pub use collider::ConvexPolygon;
pub use config::{FlightTuning, SimConfig};
pub use design::{ConsumerKind, PartIdx, PartStats, ShipDesign, Stat};
pub use events::{PieceSnapshot, SimEvent, SplitEvent};
pub use flight::{FlightGoal, FlightTelemetry, HeadingGoal, HelmMode, PilotInput, Target};
pub use hash::{StateHash, StateHasher};
pub use ids::{PieceId, ShipId};
pub use maneuver::{Maneuver, RaiderBrain};
pub use mass::MassProps;
pub use math::{Rot, Vec2};
pub use physics::{BodyDesc, BodyKey, Contact, FreeFlight, PhysicsBackend, RayHit};
pub use rng::{Pcg32, Stream};
pub use scenario::ScenarioKind;
pub use ship::{
    Body, GunMount, PartState, Piece, PowerState, Ship, ShipControls, ThrustAuthority, Thruster,
    Wrench, inherited_velocity, thruster_wrench,
};
pub use world::{DamageCommand, PartHit, ShipCommand, Sim, TickInputs};

/// Fixed simulation rate (settled: 30 Hz).
pub const TICK_HZ: u32 = 30;
/// Fixed time step in seconds.
pub const DT: f32 = 1.0 / TICK_HZ as f32;
