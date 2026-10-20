//! Events the simulation emits for the physics layer and the client. Read them with `Sim::events()` after a
//! step (they are cleared at the start of the next step).

use crate::ship::collider::ConvexPolygon;
use crate::ship::design::PartIdx;
use crate::ids::{PieceId, ShipId};
use crate::ship::mass::MassProps;
use crate::math::Vec2;
use crate::combat::salvage::{Item, Salvaged};
use crate::ship::Body;
use crate::ship::vessel::Owner;
use crate::combat::weapons::{GoneReason, ProjectileId, ShotKind};
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
    /// A part reached 0 hit points and was removed. `volatile`: it has an `explosion` (GDR-0084), so an
    /// `Explosion` follows right after.
    PartDestroyed {
        ship: ShipId,
        piece: PieceId,
        part: PartIdx,
        volatile: bool,
    },
    /// A heart re-grew a lost part (GDR-0099 A1, `ship/living.rs`): it is alive again on `piece`, with the hit points
    /// it starts with; a `PieceChanged` for the piece follows in the same tick.
    PartRegrown {
        ship: ShipId,
        piece: PieceId,
        part: PartIdx,
    },
    /// A destroyed part exploded (a chain reaction, GDR-0084, GDR-0085): every part within `radius` m of its
    /// centre, on any piece of any ship, took `damage` (times its armour's `blast_factor`, less what armour in
    /// between and other ships' bubbles held back) at once, before the breakage was resolved; parts that died of
    /// it follow as `PartDestroyed` (and explode in turn; on other ships with those ships' events). `at`: the
    /// part's centre (world), as the piece was before the breakage.
    Explosion {
        ship: ShipId,
        piece: PieceId,
        part: PartIdx,
        at: Vec2,
        radius: f32,
        damage: f32,
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
    /// A vessel formed (GDR-0013): a ship spawned with a logic core (`from: None`), or a piece with a logic core
    /// that moved out of a ship into a ship of its own, a vessel of the same owner that inherits the parent's
    /// behavior: split off a vessel, or a further cored piece of a blueprint built in several pieces, at spawn.
    /// `from` = the ship and piece it left: the piece keeps its id, so the client moves the view of
    /// `(from.0, from.1)` to `(ship, from.1)` (the body moved the same way).
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
    /// A hangar launched a drone (GDR-0117): `ship` (spawned this tick, `ShipSpawned` came first; a drone has no
    /// `VesselFormed`) left `carrier` by its hangar `part`.
    DroneLaunched { ship: ShipId, carrier: ShipId, part: PartIdx },
    /// A drone died with its hangar, or lost its core (GDR-0117): it is removed at once, without a wreck
    /// (`ShipRemoved` follows; a drone has no `VesselLost`).
    DroneLost { ship: ShipId, carrier: ShipId, at: Vec2 },
    /// Two pieces pushed each other this tick with at least `PhysicsTuning::impact_min_impulse` (sparks,
    /// sound). Above `PhysicsTuning::ram_threshold` it carries ramming damage (none between vessels of one
    /// owner), already dealt after shields (`ShieldHit` for what bubbles took): to `part_a` and `part_b`, the
    /// excess crushing on inward (destroyed parts follow as `PartDestroyed`).
    Impact(crate::flight::physics::Contact),
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
    /// Missile `id` of ship `by` struck at `at` and its warhead went off (GDR-0115): `damage` to every part
    /// within `radius` m by the chain-reaction rule (the `Hit` events of what it reached come right before it,
    /// its `ProjectileGone` after it).
    Warhead {
        by: ShipId,
        id: ProjectileId,
        at: Vec2,
        radius: f32,
        damage: f32,
    },
    /// A flare launcher (`part`) of `ship` released `decoys` decoys at `at` (GDR-0115, `combat/flare.rs`).
    FlaresReleased {
        ship: ShipId,
        part: crate::ship::design::PartIdx,
        at: Vec2,
        decoys: u32,
    },
    /// A blink drive jumped `ship`'s controlling piece `piece` (GDR-0127, `flight/blink.rs`): its centre of mass from
    /// `from` to `to` (the out and in flashes).
    Blinked {
        ship: ShipId,
        piece: PieceId,
        from: Vec2,
        to: Vec2,
    },
    /// A blink found no clear landing spot towards `to` and fizzled where the ship is (`at`).
    BlinkBlocked {
        ship: ShipId,
        at: Vec2,
        to: Vec2,
    },
    /// A lance (GDR-0127, `combat/lance.rs`) of `by` went through `ship` from `from` to `to` (where it left the piece
    /// or ran out), striking `parts` parts (each its own `Hit`).
    Lanced {
        by: ShipId,
        ship: ShipId,
        from: Vec2,
        to: Vec2,
        parts: u32,
    },
    /// Static from `by`'s disruption guns blinded `ship`'s mind for `seconds` (GDR-0125, `combat/disruption.rs`).
    Blinded {
        by: ShipId,
        ship: ShipId,
        seconds: f32,
    },
    /// A shot struck an asteroid rock (GDR-0104; `Voyage::rocks`) and stopped there: the rock takes nothing (a
    /// mining beam's cut follows as `Mined`, GDR-0114). `by`: the shooter.
    RockHit {
        by: ShipId,
        rock: u32,
        point: Vec2,
        normal: Vec2,
        family: WeaponFamily,
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
    /// A vessel collected salvage (`salvage.rs`): `item` (taking `units` cargo units) went into the hold of
    /// `ship`, from a wreck's part (it left its piece without destruction: no `PartDestroyed`; `PieceChanged`,
    /// `Split` or `PieceDestroyed` for that piece follow in the same tick), a wreck's cargo or a loose fragment.
    /// `at`: where it was (world).
    Collected {
        ship: ShipId,
        from: Salvaged,
        item: Item,
        units: u32,
        at: Vec2,
    },
    /// A vessel's hold shrank below its load (a cargo part destroyed or split off): `count` of `item` left the
    /// hold, the lowest-value items first, and spilled as loose items at the lost hold (`salvage.rs`, GDR-0076;
    /// `Sim::loose`, collected again like fragments).
    CargoLost { ship: ShipId, item: Item, count: u32 },
    /// A mining beam of `ship` cut a unit of ore (`item`, an `Item::Ore`) from rock `rock` at `at` (GDR-0114,
    /// `combat/mining.rs`): into its hold, or, with the hold full, loose beside the rock (`stowed` false). `spent`:
    /// that was the rock's last.
    Mined { ship: ShipId, rock: u32, item: Item, at: Vec2, stowed: bool, spent: bool },
    /// A ship's scan swept (`ship/scan.rs`): from `at` (its controlling piece's centre of mass), out to `range`
    /// m; it wakes sleeping voyage sites within `wake` m.
    Scanned {
        ship: ShipId,
        at: Vec2,
        range: f32,
        wake: f32,
    },
    /// A voyage site shows now (`voyage/mod.rs`): one of the player's vessels has it within its sensors or a scan
    /// swept over it. `site` indexes `Voyage::sites`.
    SiteRevealed { site: u32 },
    /// A scan gave hidden signal site `site` a bearing (`Voyage::bearings`, the newest last).
    SiteBearing { site: u32 },
    /// A voyage site woke (`voyage/mod.rs`): its ships fly their behavior from now on. `site` indexes
    /// `Voyage::sites`.
    SiteWoke { site: u32 },
    /// A site's ship called its group (GDR-0113): the group's site `site` warps in on the cluster's edge
    /// [`crate::voyage::CALL_SECONDS`] later (`Site::arrives`), awake.
    SiteCalled { site: u32 },
    /// A voyage site was cleared (GDR-0068): each of its ships is destroyed or disarmed, none is a vessel that is
    /// still armed (`Ship::is_armed`). `goal`: it was the voyage's goal, so the voyage is cleared too.
    SiteCleared { site: u32, goal: bool },
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
