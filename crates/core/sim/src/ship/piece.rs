//! A piece: one rigid body of a ship (its parts, mass, colliders, power and heat rows), its thrusters and what they
//! can do now ([`ThrustAuthority`]), its gun mounts.

use super::*;

/// One row of a piece's thruster table (for the flight computer): an engine, or one nozzle of an RCS pod.
/// Piece frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thruster {
    /// Engine slot (index into `ShipControls::throttle`); `design::NO_SLOT` for RCS nozzles.
    pub slot: u32,
    pub part: PartIdx,
    /// Unit direction of the force on the ship (towards the engine's facing side; the exhaust goes the other
    /// way).
    pub dir: Vec2,
    /// Application point (the part's centre; the thrust line runs through it).
    pub point: Vec2,
    /// `point − com`.
    pub arm: Vec2,
    /// Torque per kN of thrust: `arm × dir`.
    pub torque_per_force: f32,
    /// Full-throttle thrust with bonuses (kN), before power, heat and speed.
    pub max_force: f32,
    /// Rated speed with bonuses and `SimConfig::speed_scale` (m/s): while the row pushes with the piece's
    /// motion, its thrust fades out as the piece's whole speed approaches it ([`Thruster::falloff`]).
    pub rated: f32,
    /// Engines: how far the exhaust plume reaches behind the engine (m, with bonuses; 0 for RCS nozzles). The
    /// plume leaves through the back side, against `dir`, one lane per back cell (`ShipDesign::plume_lanes`),
    /// and burns the first part in its path (`plume.rs`).
    pub plume_length: f32,
    /// Engines: plume damage and heat per second at full output (with bonuses; shared over the lanes, scaled
    /// by `Piece::output`). 0 for RCS nozzles.
    pub plume_damage: f32,
    pub plume_heat: f32,
    /// `sin` of the max gimbal angle: lateral force up to `gimbal × thrust` (0 = fixed nozzle).
    pub gimbal: f32,
    /// Unit lateral direction of the gimbal force (`dir` turned 90° counter-clockwise).
    pub lat: Vec2,
    /// Torque per kN of lateral force: `arm × lat`.
    pub torque_per_lateral: f32,
    /// An RCS nozzle (else an engine).
    pub rcs: bool,
}

impl Thruster {
    /// Share of the row's force left at piece-frame velocity `v_local` of length `speed` (GDR-0055, GDR-0089): a
    /// row that pushes with the motion (`v_local · dir > 0`, at angle θ to it) fades by the whole speed times
    /// cos²θ ([`rated_falloff`]): straight along the motion by the whole speed, so the ship eases into its rating
    /// as before; the more the push turns away from the motion, the less it fades (full thrust at the top speed
    /// from about 27° off, cos²θ = 0.8), so a push off the motion always turns the velocity towards it, never cut
    /// off at some angle; [`super::govern`] keeps the speed under the top speed meanwhile. Braking (against the
    /// motion) and sideways rows keep full thrust. The caller takes `speed` once per piece (one square root).
    #[inline]
    pub fn falloff(&self, v_local: Vec2, speed: f32) -> f32 {
        let along = v_local.dot(self.dir);
        if along > 0.0 {
            // `speed.max(along)`: a velocity so small that its square underflows has `speed` 0 but `along` > 0.
            rated_falloff(along * (along / speed.max(along)), self.rated)
        } else {
            1.0
        }
    }

    /// An engine whose plume can burn: it reaches past the engine and does damage or heat.
    #[inline]
    pub fn burns(&self) -> bool {
        !(self.slot == NO_SLOT
            || self.plume_length <= 0.0
            || (self.plume_damage <= 0.0 && self.plume_heat <= 0.0))
    }
}

/// What a piece's actuators can do now, per direction (kN) and per turning sense (kN·m): only rows and torque
/// sources that can produce (alive, not overheated, not idle, powered; [`producing`]), each at the power level
/// it was served last (partial power scales the output). All zero while the piece is not active (dark, dormant,
/// salvage). Updated after every systems step and on rebuilds; the flight computer plans with it (`Kin`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ThrustAuthority {
    /// Along +y (the ship's nose).
    pub forward: f32,
    pub backward: f32,
    /// Along +x.
    pub right: f32,
    pub left: f32,
    /// Differential thrust (the last resort, GDR-0055), counter-clockwise: the sum of the rows' positive
    /// torques, less what the RCS couples of that sense already take.
    pub torque_ccw: f32,
    /// The same clockwise, as a positive number.
    pub torque_cw: f32,
    /// Pure torque of gyros and logic cores (either sense), with the global torque scale.
    pub torque_pure: f32,
    /// Torque of the RCS couples (`alloc::couples`: nozzles of opposite groups at equal force, so no net
    /// force), counter-clockwise and clockwise.
    pub couple_ccw: f32,
    pub couple_cw: f32,
    /// Reachable top speed per thrust group (forward +y, backward, right +x, left; m/s): the
    /// highest rated speed among the rows pushing that way, 0 if none (GDR-0041).
    pub speed: [f32; 4],
}

/// Can this actuator push or turn now? Alive, not overheated, not idle (GDR-0044) and powered (its last power
/// level above 0). Rows and torque sources that cannot are left out of the flight computer's authority and
/// allocation.
#[inline]
pub fn producing(ps: &PartState) -> bool {
    ps.alive && !ps.overheated && !ps.idle && ps.power > 0.0
}

/// Can this gun fire now, power aside? A clear line of fire, and its part (`ps`) alive, not overheated and not
/// idle (GDR-0044); never a point-defence gun, which aims itself (GDR-0115). Firepower, reach and the smart lock aim count only these guns (power is the brown-out's
/// business: a gun of a starved tier still counts).
#[inline]
pub fn gun_works(g: &GunMount, ps: &PartState) -> bool {
    g.clear && g.defence.is_none() && ps.alive && !ps.overheated && !ps.idle
}

/// Recomputes `piece.authority` from the actuators that can produce now (see [`ThrustAuthority`]). One pass
/// over the thruster rows and torque sources; no allocation.
pub(crate) fn refresh_authority(piece: &mut Piece, parts: &[PartState]) {
    let mut auth = ThrustAuthority::default();
    if piece.active {
        for t in &piece.thrusters {
            let ps = &parts[t.part as usize];
            if !producing(ps) {
                continue;
            }
            let f = t.max_force * ps.power;
            let g = crate::ship::alloc::group_of(t.dir);
            match g {
                0 => auth.forward += f,
                1 => auth.backward += f,
                2 => auth.right += f,
                _ => auth.left += f,
            }
            if f > 0.0 {
                auth.speed[g] = auth.speed[g].max(t.rated);
            }
            let tq = t.torque_per_force * f;
            if tq > 0.0 {
                auth.torque_ccw += tq;
            } else {
                auth.torque_cw -= tq;
            }
        }
        for src in &piece.torque_sources {
            let ps = &parts[src.part as usize];
            if producing(ps) {
                auth.torque_pure += src.max * ps.power;
            }
        }
        if (auth.forward > 0.0 && auth.backward > 0.0) || (auth.right > 0.0 && auth.left > 0.0) {
            let th = &piece.thrusters;
            let room = |i: usize| {
                let ps = &parts[th[i].part as usize];
                if producing(ps) {
                    th[i].max_force * ps.power
                } else {
                    0.0
                }
            };
            auth.couple_ccw = crate::ship::alloc::couples(th, &piece.alloc, true, &room, None);
            auth.couple_cw = crate::ship::alloc::couples(th, &piece.alloc, false, &room, None);
            auth.torque_ccw = (auth.torque_ccw - auth.couple_ccw).max(0.0);
            auth.torque_cw = (auth.torque_cw - auth.couple_cw).max(0.0);
        }
    }
    piece.authority = auth;
}

/// A gun of a piece. Piece frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunMount {
    pub slot: u32,
    pub part: PartIdx,
    /// Centre of the gun's facing side.
    pub muzzle: Vec2,
    /// Unit firing direction.
    pub dir: Vec2,
    pub clear: bool,
    /// Fire-rate multiplier from bonuses (1 = base).
    pub fire_rate: f32,
    /// Damage per second at full power with bonuses (`damage × fire_rate`).
    pub dps: f32,
    /// Damage per second this tick (0 unless firing, clear, powered and not overheated).
    pub output: f32,
    pub family: WeaponFamily,
    pub delivery: Delivery,
    /// Reach (m).
    pub range: f32,
    /// Shots per second at full power with bonuses (0 for beams).
    pub rate: f32,
    /// Damage per pulse or projectile (`damage / base rate`; per missile `damage / base rate / salvo`; 0 for
    /// beams).
    pub shot_damage: f32,
    /// Point defence (GDR-0115): it aims itself and fires on its own.
    pub defence: Option<Defence>,
    /// Ore cut from rock per second at full power (GDR-0114; 0: not a mining gun).
    pub cut: f32,
    /// Static carried per point of damage that reaches a part (disruption, GDR-0125; 0: none).
    pub disrupt: f32,
    /// A lance (GDR-0127): its shots strike every part on their line through the struck piece.
    pub lance: Option<data::LanceSpec>,
    /// The prisms its beam passes (GDR-0126): `muzzle` and `dir` are then the last prism's front, `range`, `dps`
    /// and `shot_damage` raised by their shares.
    pub relay: Relay,
}

/// The prisms a gun's beam passes on its piece (GDR-0126, [`crate::Ship::beam_path`]), for drawing and reports.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Relay {
    /// The gun's own muzzle (piece frame), where the beam enters the chain.
    pub from: Vec2,
    pub prisms: [PartIdx; data::design::geometry::MAX_PRISMS],
    pub len: u8,
}

impl Relay {
    /// The prisms, in the beam's order (empty: not relayed).
    pub fn prisms(&self) -> &[PartIdx] {
        &self.prisms[..self.len as usize]
    }
}

/// A point-defence gun's targeting, compiled (`data::DefenceSpec`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Defence {
    /// Cosine of its arc: it aims at what lies within the arc of its facing.
    pub arc_cos: f32,
    /// The heaviest vessel (t) it shoots at once no missile is in reach; 0: missiles only.
    pub ship_mass: f32,
}

/// A connected group of parts: one rigid body.
#[derive(Clone, Debug)]
pub struct Piece {
    pub id: PieceId,
    /// Alive parts, ascending.
    pub parts: Vec<PartIdx>,
    /// Origin of the piece frame as a blueprint point (the piece's centre of mass when it was created).
    pub origin: [f32; 2],
    pub mass: MassProps,
    pub colliders: Vec<ConvexPolygon>,
    /// Bounding radius about the frame origin (m): every collider lies within it (cheap proximity tests, e.g.
    /// whether a plume can reach another body).
    pub radius: f32,
    /// Has an alive logic core.
    pub has_core: bool,
    /// Cargo units its alive cargo parts hold (the sum of their `cargo.capacity`, rounded down): a vessel's hold
    /// is its controlling piece's (`salvage.rs`).
    pub cargo_capacity: u32,
    /// The ship's controlling piece: the vessel (it holds the ship's logic cores). Every other piece of the
    /// ship is salvage without a core (cored pieces of a split become vessels of their own, GDR-0013).
    pub controlled: bool,
    /// Controlled, not dormant, with a working, powered core this tick: systems run and the hull glows. A
    /// controlled piece without power goes dark until power returns.
    pub active: bool,
    /// Control load and capacity (GDR-0044, see [`crate::ship::control`]).
    pub control: crate::ship::control::ControlState,
    /// Pose and velocity of the body (synced from the physics backend after each step).
    pub body: Body,
    /// Pose at the end of the previous tick, for render interpolation (`render_pose`).
    pub prev_pos: Vec2,
    pub prev_angle: f32,
    pub power: PowerState,
    pub thrusters: Vec<Thruster>,
    /// Gyros and logic cores with reaction torque.
    pub torque_sources: Vec<TorqueSource>,
    pub authority: ThrustAuthority,
    /// Last-resort turning (GDR-0055): whether gimbals and differential thrust may help, and how many ticks in a
    /// row the first line has been at its limit. Kept by the flight computer.
    pub assist: crate::flight::TurnAssist,
    pub guns: Vec<GunMount>,
    /// Has an alive part that deals more in a ram than it takes (a ram prow): with an engine and power it keeps the
    /// ship armed (GDR-0109).
    pub rams: bool,
    /// This tick's actuator commands (written by the flight computer or from raw controls).
    pub act: Actuation,
    /// Share of each thruster row's force left at this tick's velocity (rated-speed falloff, 0..1; 1 for
    /// pieces the flight computer did not fly this tick).
    pub falloff: Vec<f32>,
    /// Actual output of each thruster row this tick, 0..1 of its max force (throttle × power × falloff).
    /// Plumes.
    pub output: Vec<f32>,
    /// Cruise-flame level of each engine row this tick, 0..1 (GDR-0041): the share of its group's top speed
    /// that the velocity goal asks along the engine's push direction, while the engine is working and powered;
    /// 0 for RCS nozzles. A look only: holding a speed costs no thrust, power or heat (no drag).
    pub cruise: Vec<f32>,
    /// Force (piece frame) and torque this tick from engines, RCS, gimbals, gyros and cores, for the physics
    /// layer.
    pub wrench: Wrench,
    /// Collisions with other pieces of the same ship are off until this tick (after a split; 0 = never off).
    pub ghost_until: u64,
    /// The longest plume of its engines (m; 0 = no plume), and whether some engine's plume lane runs into a part
    /// of this piece within reach (a buried engine). Cached on rebuilds, so the plume pass skips a piece with
    /// nothing in reach after one proximity scan (`plume.rs`).
    pub(crate) plume_reach: f32,
    pub(crate) plume_blocked: bool,
    pub(crate) alloc: AllocTable,
    pub(super) cores: Vec<PartIdx>,
    /// Shield emitters.
    pub(crate) shields: Vec<PartIdx>,
    pub(crate) reactors: Vec<Reactor>,
    /// Consumers in allocation order (tier, weight descending, part).
    pub(crate) consumers: Vec<Consumer>,
    /// Parts that make heat without a power role (e.g. capacitors).
    pub(crate) heaters: Vec<PartIdx>,
    /// The links heat conducts through: indices into `ShipDesign::links` with both ends in this piece, in that
    /// order (links never cross pieces). The heat pass walks these and `parts`, so a vessel split off a big
    /// blueprint costs what its own parts cost, not the whole blueprint.
    pub(crate) heat_links: Vec<u32>,
    pub(crate) dirty: bool,
    /// The idle set must be recomputed (rebuilt piece, changed power setting).
    pub(crate) control_dirty: bool,
}

/// Heat constants of one part, cached when its exposure or bonuses change (compact for the per-tick loop).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct HeatK {
    /// Heat capacity.
    pub cap: f32,
    /// Loss coefficient to space (base + open sides × side loss or radiator vent).
    pub loss: f32,
    /// Heat at full work, with bonuses.
    pub out: f32,
    /// Shutdown level, with bonuses.
    pub limit: f32,
}

impl Piece {
    pub(crate) fn new(id: PieceId, parts: Vec<PartIdx>, origin: [f32; 2], body: Body) -> Piece {
        Piece {
            id,
            parts,
            origin,
            mass: MassProps::default(),
            colliders: Vec::new(),
            radius: 0.0,
            has_core: false,
            cargo_capacity: 0,
            controlled: false,
            active: false,
            control: crate::ship::control::ControlState::default(),
            prev_pos: body.pos,
            prev_angle: body.angle,
            body,
            power: PowerState::default(),
            thrusters: Vec::new(),
            torque_sources: Vec::new(),
            authority: ThrustAuthority::default(),
            assist: crate::flight::TurnAssist::default(),
            guns: Vec::new(),
            rams: false,
            act: Actuation::default(),
            falloff: Vec::new(),
            output: Vec::new(),
            cruise: Vec::new(),
            wrench: Wrench::default(),
            ghost_until: 0,
            plume_reach: 0.0,
            plume_blocked: false,
            alloc: AllocTable::default(),
            cores: Vec::new(),
            shields: Vec::new(),
            reactors: Vec::new(),
            consumers: Vec::new(),
            heaters: Vec::new(),
            heat_links: Vec::new(),
            dirty: false,
            control_dirty: true,
        }
    }

    /// Not the controlling piece: drifting salvage (no hull glow, systems off), even with a core.
    pub fn salvage(&self) -> bool {
        !self.controlled
    }

    /// Pose for rendering between the previous and the current tick: `alpha` 0 = previous, 1 = current.
    /// The angle takes the short way round.
    pub fn render_pose(&self, alpha: f32) -> (Vec2, f32) {
        let pos = self.prev_pos + (self.body.pos - self.prev_pos) * alpha;
        let d = crate::flight::physics::wrap_angle(self.body.angle - self.prev_angle);
        (pos, crate::flight::physics::wrap_angle(self.prev_angle + d * alpha))
    }

    /// Logic cores of this piece (alive when the piece was last rebuilt).
    pub fn cores(&self) -> &[PartIdx] {
        &self.cores
    }

    /// Consumers in the order they get power (tier, weight descending, part).
    pub fn consumers(&self) -> &[Consumer] {
        &self.consumers
    }

    /// Damage per second of its working guns ([`gun_works`]) at full power, with bonuses (the `threat`
    /// sensor's firepower and the design report's).
    pub fn firepower(&self, parts: &[PartState]) -> f32 {
        self.guns
            .iter()
            .filter(|g| gun_works(g, &parts[g.part as usize]))
            .map(|g| g.dps)
            .sum()
    }

    /// Centre of mass in world coordinates.
    pub fn com_world(&self) -> Vec2 {
        self.body.to_world(self.mass.com)
    }
}
