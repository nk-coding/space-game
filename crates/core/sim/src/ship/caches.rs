//! A piece's cached rows (part of [`Ship`]'s impl): components and the main one, exposure, part stats with bonuses,
//! rebuilding a piece (mass, colliders, power lists, thrusters, guns, heat constants) and checking the caches.

use super::*;

impl Ship {
    /// Centre of mass of a set of parts in the blueprint frame.
    pub(crate) fn com_bp(&self, parts: &[PartIdx]) -> [f32; 2] {
        let d = &self.design;
        combine(
            parts.iter().map(|&p| {
                let s = &d.parts[p as usize];
                (s.mass, s.centroid, s.inertia)
            }),
            [0.0, 0.0],
        )
        .1
    }

    /// Connected components of the alive parts in `parts` (static links filtered by alive). Components are
    /// ordered by their lowest part index; parts inside ascending.
    pub(crate) fn components(&mut self, parts: &[PartIdx]) -> Vec<Vec<PartIdx>> {
        let n = self.parts.len();
        let mark = &mut self.scratch.mark;
        mark.clear();
        mark.resize(n, u32::MAX);
        const IN_SET: u32 = u32::MAX - 1;
        for &p in parts {
            if self.parts[p as usize].alive {
                mark[p as usize] = IN_SET;
            }
        }
        let mut comps: Vec<Vec<PartIdx>> = Vec::new();
        let stack = &mut self.scratch.stack;
        for &p in parts {
            if mark[p as usize] != IN_SET {
                continue;
            }
            let c = comps.len() as u32;
            let mut comp = Vec::new();
            mark[p as usize] = c;
            stack.clear();
            stack.push(p);
            while let Some(q) = stack.pop() {
                comp.push(q);
                for &r in self.design.neighbors.get(q as usize) {
                    if mark[r as usize] == IN_SET {
                        mark[r as usize] = c;
                        stack.push(r);
                    }
                }
            }
            comp.sort_unstable();
            comps.push(comp);
        }
        comps
    }

    /// Index of the main component (`data::design::geometry::main_of`): with a logic core first, then more parts, then the
    /// earlier one.
    pub(crate) fn main_component(&self, comps: &[Vec<PartIdx>]) -> usize {
        let has_core = |c: &Vec<PartIdx>| {
            c.iter()
                .any(|&p| self.design.parts[p as usize].logic && self.parts[p as usize].alive)
        };
        data::design::geometry::main_of(comps.iter().map(|c| (has_core(c), c.len()))).unwrap_or(0)
    }

    /// Recounts the open sides of part `p` from the grid (no allocation). Exposure only changes when a
    /// linked neighbour is destroyed, so `apply_damage` calls this for those neighbours only.
    pub(crate) fn refresh_exposure(&mut self, p: PartIdx) {
        let s = self.design.parts[p as usize].shape;
        let (w, h) = s.dims();
        let mut open = 0u32;
        for y in s.y..s.y + h as i32 {
            for x in s.x..s.x + w as i32 {
                if self.grid.occupant((x, y)) == Some(p as usize) {
                    open += self.grid.exposure((x, y));
                }
            }
        }
        self.parts[p as usize].open_sides = open.min(u16::MAX as u32) as u16;
    }

    /// Recomputes the effective stats (adjacency bonuses) and heat constants of part `t`. Bonuses change
    /// only when a source dies or a source's venting changes, i.e. around destroyed parts. Sources are linked
    /// to `t`, and linked alive parts are always in the same piece.
    pub(crate) fn refresh_stats(&mut self, t: PartIdx) {
        let d = &*self.design;
        let spec = &d.parts[t as usize];
        let mut add = [0.0f32; Stat::COUNT];
        let mut pct = [0.0f32; Stat::COUNT];
        applied_bonuses(d, &self.parts, t, &mut self.scratch.groups, |b| {
            match b.effect {
                Effect::Add(x) => add[b.stat as usize] += x,
                Effect::Percent(x) => pct[b.stat as usize] += x,
            }
        });
        let mut st = spec.base;
        for k in 0..Stat::COUNT {
            if add[k] != 0.0 || pct[k] != 0.0 {
                st.v[k] = (st.v[k] + add[k]) * (1.0 + pct[k] / 100.0);
            }
        }
        self.stats[t as usize] = st;
        let per_side = if spec.is_radiator {
            st.get(Stat::HeatVent)
        } else {
            tuning::HEAT_LOSS_OPEN_SIDE
        };
        let open = self.parts[t as usize].open_sides as f32;
        self.heat_k[t as usize] = HeatK {
            cap: spec.heat_capacity,
            loss: tuning::HEAT_LOSS_BASE * spec.area + open * per_side,
            out: st.get(Stat::HeatOutput),
            limit: st.get(Stat::HeatLimit),
        };
    }

    /// Test helper: recomputes every per-part cache from scratch and compares it with the incrementally
    /// maintained one. `Err` names the first mismatch.
    #[doc(hidden)]
    pub fn verify_caches(&self) -> Result<(), String> {
        let mut fresh = self.clone();
        let alive: Vec<PartIdx> = (0..self.parts.len() as PartIdx)
            .filter(|&p| self.parts[p as usize].alive)
            .collect();
        for &p in &alive {
            fresh.refresh_exposure(p);
        }
        for &p in &alive {
            fresh.refresh_stats(p);
        }
        for &p in &alive {
            let i = p as usize;
            if fresh.parts[i].open_sides != self.parts[i].open_sides {
                return Err(format!(
                    "part {p}: open sides {} vs fresh {}",
                    self.parts[i].open_sides, fresh.parts[i].open_sides
                ));
            }
            if fresh.stats[i] != self.stats[i] || fresh.heat_k[i] != self.heat_k[i] {
                return Err(format!(
                    "part {p}: stats {:?} vs fresh {:?}",
                    self.stats[i], fresh.stats[i]
                ));
            }
        }
        for (k, piece) in self.pieces.iter().enumerate() {
            let mut f = fresh.clone();
            f.rebuild_piece(k);
            let fp = &f.pieces[k];
            if fp.mass != piece.mass
                || fp.colliders != piece.colliders
                || fp.consumers != piece.consumers
                || fp.heat_links != piece.heat_links
            {
                return Err(format!(
                    "piece {:?}: caches differ from a rebuild",
                    piece.id
                ));
            }
        }
        Ok(())
    }

    /// Recomputes the piece-level caches of piece `k`: core flag, lines of fire, mass properties, colliders,
    /// power lists, thruster table and gun mounts. Keeps the piece's frame origin. Parts' `piece` ids, exposure
    /// and stats must already be current.
    pub(crate) fn rebuild_piece(&mut self, k: usize) {
        let design = Arc::clone(&self.design);
        let d = &*design;
        let mut piece = std::mem::replace(
            &mut self.pieces[k],
            Piece::new(PieceId(u32::MAX), Vec::new(), [0.0; 2], Body::default()),
        );
        let pid = piece.id;

        // Core flag, hold and control load.
        piece.has_core = piece.parts.iter().any(|&p| d.parts[p as usize].logic);
        piece.cargo_capacity = piece
            .parts
            .iter()
            .map(|&p| d.parts[p as usize].cargo)
            .sum::<f32>() as u32;
        piece.control.load = piece
            .parts
            .iter()
            .map(|&p| d.parts[p as usize].control_cost)
            .sum();
        piece.control_dirty = true;

        // Lines of fire: only alive parts of the same piece block. Engines are never blocked (GDR-0039).
        for &p in &piece.parts {
            let slot = d.slot_of_part[p as usize];
            let spec = &d.parts[p as usize];
            let clear = slot == NO_SLOT
                || spec.is_engine
                || gun_blockers(d, &self.parts, slot, pid).next().is_none();
            self.parts[p as usize].clear = clear;
        }

        // Mass properties and colliders.
        piece.mass = combine(
            piece.parts.iter().map(|&p| {
                let s = &d.parts[p as usize];
                (s.mass, s.centroid, s.inertia)
            }),
            piece.origin,
        )
        .0;
        build_colliders(
            d,
            &piece.parts,
            piece.origin,
            &mut self.scratch.colliders,
            &mut piece.colliders,
        );
        let mut r2 = 0.0f32;
        for c in &piece.colliders {
            for q in c.points() {
                r2 = r2.max(q.length_squared());
            }
        }
        piece.radius = crate::math::sqrt(r2);

        // Power lists (reactors, consumers in allocation order, heaters).
        self.rebuild_power_lists(&mut piece);

        // Heat links: the design's links between alive parts of this piece, in the design's order.
        piece.heat_links.clear();
        for (i, l) in d.links.iter().enumerate() {
            let (a, b) = (&self.parts[l.a as usize], &self.parts[l.b as usize]);
            if a.alive && b.alive && a.piece == pid && b.piece == pid {
                piece.heat_links.push(i as u32);
            }
        }

        // Torque sources (gyros, logic cores) and cores.
        piece.cores.clear();
        piece.shields.clear();
        piece.torque_sources.clear();
        for &p in &piece.parts {
            let spec = &d.parts[p as usize];
            if spec.logic {
                piece.cores.push(p);
            }
            if spec.is_shield {
                piece.shields.push(p);
            }
            let t = self.stats[p as usize].get(Stat::Torque) * self.torque_scale;
            if t > 0.0 {
                piece.torque_sources.push(TorqueSource { part: p, max: t });
            }
        }

        // Thruster table (engines, then RCS nozzles) and gun mounts.
        piece.thrusters.clear();
        piece.guns.clear();
        piece.rams = false;
        let com = piece.mass.com;
        let row = |slot: u32,
                   part: PartIdx,
                   dir: Vec2,
                   point: Vec2,
                   (max_force, rated): (f32, f32),
                   plume: [f32; 3],
                   gimbal: f32,
                   rcs: bool| {
            let arm = point - com;
            let lat = dir.perp();
            Thruster {
                slot,
                part,
                dir,
                point,
                arm,
                torque_per_force: arm.cross(dir),
                max_force,
                rated,
                plume_length: plume[0],
                plume_damage: plume[1],
                plume_heat: plume[2],
                gimbal,
                lat,
                torque_per_lateral: arm.cross(lat),
                rcs,
            }
        };
        for &p in &piece.parts {
            let slot = d.slot_of_part[p as usize];
            let spec = &d.parts[p as usize];
            let st = &self.stats[p as usize];
            let clear = self.parts[p as usize].clear;
            piece.rams |= spec.ram[0] > 1.0;
            if spec.is_engine && slot != NO_SLOT {
                let e = &d.engines[slot as usize];
                let deg = st.get(Stat::Gimbal).clamp(0.0, 89.0);
                let gimbal = if deg > 0.0 {
                    sin(deg * (core::f32::consts::PI / 180.0))
                } else {
                    0.0
                };
                piece.thrusters.push(row(
                    slot,
                    p,
                    dir_vec(e.facing),
                    to_piece(e.point, piece.origin),
                    (
                        st.get(Stat::Thrust) * self.thrust_scale,
                        st.get(Stat::RatedSpeed) * self.speed_scale,
                    ),
                    [
                        st.get(Stat::PlumeLength),
                        st.get(Stat::PlumeDamage),
                        st.get(Stat::PlumeHeat),
                    ],
                    gimbal,
                    false,
                ));
            } else if spec.weapon.is_some() && slot != NO_SLOT {
                let g = &d.guns[slot as usize];
                let fire_rate = st.get(Stat::FireRate);
                let base_rate = match spec.delivery {
                    Delivery::Beam => 0.0,
                    Delivery::Pulse { rate } | Delivery::Projectile { rate, .. } => rate,
                };
                let damage = st.get(Stat::Damage);
                piece.guns.push(GunMount {
                    slot,
                    part: p,
                    muzzle: to_piece(g.muzzle, piece.origin),
                    dir: dir_vec(g.facing),
                    clear,
                    fire_rate,
                    dps: damage * fire_rate,
                    output: 0.0,
                    family: spec.weapon.unwrap_or(WeaponFamily::Energy),
                    delivery: spec.delivery,
                    range: st.get(Stat::Range),
                    rate: base_rate * fire_rate,
                    shot_damage: if base_rate > 0.0 {
                        damage / base_rate
                    } else {
                        0.0
                    },
                    cut: spec.cut,
                });
            }
        }
        // RCS: one nozzle per straight side whose neighbouring cell is empty or belongs to another piece. It
        // pushes away from its side, at the side's centre.
        for &p in &piece.parts {
            let spec = &d.parts[p as usize];
            if !spec.is_rcs {
                continue;
            }
            let st = &self.stats[p as usize];
            let thrust = (
                st.get(Stat::RcsThrust) * self.thrust_scale,
                st.get(Stat::RcsRatedSpeed) * self.speed_scale,
            );
            for (c, side) in spec.shape.connectable_sides() {
                let (dx, dy) = side.delta();
                let open = match self.grid.occupant((c.0 + dx, c.1 + dy)) {
                    None => true,
                    Some(o) => o != p as usize && self.parts[o].piece != pid,
                };
                if !open {
                    continue;
                }
                let centre = [
                    c.0 as f32 + 0.5 + 0.5 * dx as f32,
                    c.1 as f32 + 0.5 + 0.5 * dy as f32,
                ];
                piece.thrusters.push(row(
                    NO_SLOT,
                    p,
                    dir_vec(side.opposite()),
                    to_piece(centre, piece.origin),
                    thrust,
                    [0.0; 3],
                    0.0,
                    true,
                ));
            }
        }

        piece.alloc = AllocTable::build(&piece.thrusters);
        piece.plume_reach = 0.0;
        piece.plume_blocked = false;
        for t in &piece.thrusters {
            if !t.burns() {
                continue;
            }
            piece.plume_reach = piece.plume_reach.max(t.plume_length);
            let [l0, l1] = d.engines[t.slot as usize].lanes;
            for l in l0..l1 {
                let path = d.plume_path.get(l as usize);
                piece.plume_blocked |=
                    crate::combat::plume::own_burn(path, t.plume_length, &self.parts, pid).is_some();
            }
        }
        piece
            .act
            .resize(piece.thrusters.len(), piece.torque_sources.len());
        let rows = piece.thrusters.len();
        piece.falloff.clear();
        piece.falloff.resize(rows, 1.0);
        piece.output.clear();
        piece.output.resize(rows, 0.0);
        piece.cruise.clear();
        piece.cruise.resize(rows, 0.0);
        piece.active = piece.controlled && piece.has_core && !self.dormant;
        refresh_authority(&mut piece, &self.parts);
        self.pieces[k] = piece;
    }
}
