//! Events the simulation emits for the physics layer and the client. Read them with `Sim::events()` after a
//! step (they are cleared at the start of the next step).

use crate::collider::ConvexPolygon;
use crate::design::PartIdx;
use crate::ids::{PieceId, ShipId};
use crate::mass::MassProps;
use crate::math::Vec2;
use crate::ship::Body;
use crate::vessel::Owner;
use crate::weapons::{GoneReason, ProjectileId, ShotKind};
use data::WeaponFamily;

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
    /// Not the ship's controlling piece: drifts as salvage (no hull glow, systems off). A spawned piece that
    /// still has a logic core becomes a vessel of its own right away: `SimEvent::VesselFormed` follows in
    /// the same tick and moves it to a new ship.
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
    /// The ship's controlling piece lost its last core (or was destroyed): the ship is salvage now, dark for
    /// good (`VesselLost` follows).
    ControlLost { ship: ShipId },
    /// A vessel formed (GDR-0013): a ship spawned with a logic core (`from: None`), or a piece that kept a
    /// logic core split off a vessel and became a ship of its own, a vessel of the same owner that inherits
    /// the parent's behavior. `from` = the ship and piece it left: the piece keeps its id, so the client
    /// moves the view of `(from.0, from.1)` to `(ship, from.1)` (the body moved the same way).
    VesselFormed {
        ship: ShipId,
        owner: Owner,
        from: Option<(ShipId, PieceId)>,
    },
    /// A vessel lost its last logic core or was removed: what is left is salvage.
    VesselLost { ship: ShipId, owner: Owner },
    /// The owner runs more logic cores than its mind capacity: this vessel went dormant (dark like
    /// salvage, still owned; GDR-0044).
    VesselDormant { ship: ShipId, owner: Owner },
    /// A dormant vessel runs again (capacity freed up, a wake command, or it became the focus).
    VesselWoken { ship: ShipId, owner: Owner },
    /// The owner's focus (the vessel its player pilots) moved: by `ShipCommand::Focus`, or because the focus
    /// was lost. `to: None`: the owner has no vessel left.
    FocusChanged {
        owner: Owner,
        from: Option<ShipId>,
        to: Option<ShipId>,
    },
    /// A ship was spawned: create a body and visuals per piece (`Sim::ship(ship).pieces()`).
    ShipSpawned { ship: ShipId },
    /// A ship was removed with all its pieces.
    ShipRemoved { ship: ShipId },
    /// Two pieces pushed each other this tick with at least `PhysicsTuning::impact_min_impulse` (sparks,
    /// sound). Above `PhysicsTuning::ram_threshold` it carries ramming damage (none between vessels of one
    /// owner), already dealt after shields (`ShieldHit` for what bubbles took): to `part_a` and `part_b`, the
    /// excess crushing on inward (destroyed parts follow as `PartDestroyed`).
    Impact(crate::physics::Contact),
    /// A pulse or projectile left a gun (beams are continuous: see `Sim::beams()`). `to` is where a pulse
    /// ended (hit, shield or full range); for projectiles the end of their range if nothing stops them.
    ShotFired {
        ship: ShipId,
        piece: PieceId,
        gun: PartIdx,
        kind: ShotKind,
        from: Vec2,
        to: Vec2,
        projectile: Option<ProjectileId>,
    },
    /// Damage reached a part (after shields). One per part hit (beams: one per tick on target).
    Hit {
        by: ShipId,
        ship: ShipId,
        piece: PieceId,
        part: PartIdx,
        point: Vec2,
        /// Surface normal at the hit, world frame.
        normal: Vec2,
        damage: f32,
        /// What shields took off this shot before it hit.
        absorbed: f32,
        family: WeaponFamily,
    },
    ProjectileSpawned {
        id: ProjectileId,
        owner: ShipId,
        pos: Vec2,
        vel: Vec2,
    },
    ProjectileGone {
        id: ProjectileId,
        pos: Vec2,
        reason: GoneReason,
    },
    /// A kinetic shot glanced off a slope face.
    Ricochet {
        id: ProjectileId,
        point: Vec2,
        normal: Vec2,
        /// Velocity after the bounce.
        vel: Vec2,
    },
    /// An engine's exhaust plume burned a part this tick (GDR-0049): one per plume lane and part hit (the lane
    /// stops there). `by` and `engine`: the ship and engine part whose plume it is (the ship's own parts
    /// included: `ship == by`). `damage` and `heat` (heat units, as `heat.output × DT`) after shields;
    /// `absorbed` = what shield bubbles took of the damage (`ShieldHit` for each).
    PlumeHit {
        by: ShipId,
        engine: PartIdx,
        ship: ShipId,
        piece: PieceId,
        part: PartIdx,
        point: Vec2,
        damage: f32,
        heat: f32,
        absorbed: f32,
    },
    /// A shot (or a plume) entered a shield bubble, which took `absorbed` of its damage.
    ShieldHit {
        by: ShipId,
        ship: ShipId,
        piece: PieceId,
        /// The emitter.
        part: PartIdx,
        /// Where the shot entered the bubble.
        point: Vec2,
        absorbed: f32,
        /// Bubble capacity left.
        left: f32,
        /// The bubble collapsed (nothing left).
        depleted: bool,
    },
}
