//! Events the simulation emits for the physics layer and the client. Read them with `Sim::events()` after a
//! step (they are cleared at the start of the next step).

use crate::collider::ConvexPolygon;
use crate::design::PartIdx;
use crate::ids::{PieceId, ShipId};
use crate::mass::MassProps;
use crate::math::Vec2;
use crate::ship::Body;

/// Everything the physics layer needs to create or update the body of one piece.
#[derive(Clone, Debug, PartialEq)]
pub struct PieceSnapshot {
    pub id: PieceId,
    /// Alive parts, ascending.
    pub parts: Vec<PartIdx>,
    /// Origin of the piece frame, as a blueprint point.
    pub origin: [f32; 2],
    /// Origin of this piece's frame in the parent piece's frame (zero for the piece that keeps the parent's
    /// body). For a new piece this is also its centre of mass in the parent's frame (`com_new`).
    pub offset_in_parent: Vec2,
    pub mass: MassProps,
    /// Convex colliders in the piece frame.
    pub colliders: Vec<ConvexPolygon>,
    /// World pose and velocities: for new pieces derived from the parent (same angle and angular velocity,
    /// `linvel = v + ω × (com_new − com_old)`); for the kept piece the parent's pose with `linvel` corrected
    /// for its shifted centre of mass.
    pub body: Body,
    /// Not the ship's controlling piece: drifts as salvage (no hull glow, systems off).
    pub salvage: bool,
}

/// A piece broke into several pieces.
#[derive(Clone, Debug, PartialEq)]
pub struct SplitEvent {
    pub ship: ShipId,
    /// The piece that broke. Its id and body live on in `kept`.
    pub parent: PieceId,
    /// The part of the parent that keeps its id, frame and body (the one with a logic core, else the largest).
    /// Update the body's colliders and mass properties, and set its linear velocity to `kept.body.linvel`.
    pub kept: PieceSnapshot,
    /// New pieces: create a body for each.
    pub spawned: Vec<PieceSnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SimEvent {
    /// A part reached 0 hit points and was removed. `volatile` parts (reactors) will cause chain reactions
    /// later (hook in `Ship::chain_reaction`).
    PartDestroyed {
        ship: ShipId,
        piece: PieceId,
        part: PartIdx,
        volatile: bool,
    },
    /// A piece lost parts but stayed in one piece: replace its colliders and mass properties, set
    /// `piece.body.linvel` (its centre of mass moved).
    PieceChanged { ship: ShipId, piece: PieceSnapshot },
    /// A piece split into several pieces.
    Split(SplitEvent),
    /// A piece lost its last part: remove its body.
    PieceDestroyed { ship: ShipId, piece: PieceId },
    /// The ship's controlling piece lost its last core (or was destroyed): the whole ship is dark for good.
    ControlLost { ship: ShipId },
    /// A ship was spawned: create a body and visuals per piece (`Sim::ship(ship).pieces()`).
    ShipSpawned { ship: ShipId },
    /// A ship was removed with all its pieces.
    ShipRemoved { ship: ShipId },
    /// Two bodies started touching (reported by the physics backend; ramming damage later).
    Impact(crate::physics::Contact),
}
