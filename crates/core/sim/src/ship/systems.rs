//! The per-tick systems of a ship (part of [`Ship`]'s impl and the passes it runs): power by tier and weight, heat
//! (production, conduction, radiators, overheating), bonuses, engines and gyros, guns.

use super::*;

impl Ship {
    /// One tick of the ship systems: per piece the control pass (idle parts), power, shield regeneration, the
    /// wrench (engines, RCS, gimbals, gyros, cores) and gun output; then heat (ship-wide); then each piece's
    /// authority for the next plan. Reads this tick's actuator commands (`Piece::act`). `star`: a hot star's heat per
    /// second per side open to space where the ship is (GDR-0102; 0 without one). No allocation once warmed up.
    pub fn update_systems(&mut self, star: f32) {
        let Ship {
            design,
            parts,
            stats,
            heat_k,
            pieces,
            controls,
            scratch,
            dormant,
            scan,
            blink,
            emp_cool,
            overdrive,
            ..
        } = self;
        let d = &**design;
        // Overdrive (GDR-0132): every output times the boost, heat times more, nothing shuts down for heat.
        let overdriven = overdrive.left > 0.0;
        let (boost, heat_mult) = if overdriven {
            (super::overdrive::tuning::BOOST, super::overdrive::tuning::HEAT)
        } else {
            (1.0, 1.0)
        };
        let activity = &mut scratch.activity;
        if activity.len() != parts.len() {
            activity.clear();
            activity.resize(parts.len(), 0.0);
        }

        for piece in pieces.iter_mut() {
            // Control first (which parts idle), then the power supply: a controlled piece is active only
            // when its vessel is not dormant and it has a working core and some power.
            let running = piece.controlled && !*dormant;
            crate::ship::control::update(piece, parts, d, running);
            let supply = reactor_supply(piece, parts) * boost;
            let core_ok = piece.cores.iter().any(|&c| {
                let ps = &parts[c as usize];
                ps.alive && !ps.overheated
            });
            let active = piece.controlled && !*dormant && core_ok && supply > 0.0;
            piece.active = active;

            // What the movers were told to do (engines and RCS pods: summed throttle times the rated-speed
            // falloff, so a thruster held at its rating idles, plus RCS couples; gyros: |use|).
            for c in &piece.consumers {
                activity[c.part as usize] = 0.0;
            }
            if active {
                for (i, t) in piece.thrusters.iter().enumerate() {
                    let throttle = piece.act.throttle.get(i).copied().unwrap_or(0.0);
                    let falloff = piece.falloff.get(i).copied().unwrap_or(1.0);
                    let couple = piece.act.couple.get(i).copied().unwrap_or(0.0);
                    activity[t.part as usize] += throttle * falloff + couple;
                }
                for (j, src) in piece.torque_sources.iter().enumerate() {
                    let u = piece.act.torque.get(j).copied().unwrap_or(0.0).abs();
                    let a = &mut activity[src.part as usize];
                    *a = a.max(u);
                }
            }

            // What each consumer asks for (draw × activity), the allocation by tier and weight, power and work.
            let (demand, served) = power_pass(
                piece,
                parts,
                activity,
                &mut scratch.want,
                &mut scratch.level,
                supply,
                active,
                |slot| controls.gun_fires(slot),
                controls.shields,
                |part| {
                    scan.charging() == Some(part)
                        || crate::flight::blink::blink_charging(blink, d, part)
                        || crate::combat::emp::recharging(emp_cool, d, part)
                },
            );
            if overdriven {
                for c in &piece.consumers {
                    parts[c.part as usize].work *= boost;
                }
                for r in &piece.reactors {
                    parts[r.part as usize].work *= boost;
                }
                for &h in &piece.heaters {
                    parts[h as usize].work *= boost;
                }
            }
            // Shield bubbles recharge while their emitter works.
            for &e in &piece.shields {
                let ps = &mut parts[e as usize];
                if ps.work > 0.0 {
                    let st = &stats[e as usize];
                    let cap = st.get(Stat::ShieldCapacity);
                    ps.shield = (ps.shield + st.get(Stat::ShieldRegen) * ps.work * DT).min(cap);
                    if ps.shield_down && ps.shield >= cap * crate::combat::weapons::tuning::SHIELD_RESTART {
                        ps.shield_down = false;
                    }
                }
            }
            piece.power = PowerState {
                supply,
                demand,
                served,
            };

            // Wrench at the power each actuator actually got and the speed falloff of this tick. `level` is
            // the power level of a thruster that can produce (0 if it cannot: dead, overheated, idle or
            // unpowered, [`producing`]; an idle mover that draws no power has no level of its own to zero).
            let fmax = &mut scratch.fmax;
            fmax.clear();
            let level = &mut scratch.level;
            level.clear();
            for t in &piece.thrusters {
                let ps = &parts[t.part as usize];
                level.push(if active && producing(ps) { ps.power } else { 0.0 });
            }
            fmax.extend(piece.thrusters.iter().enumerate().map(|(i, t)| {
                t.max_force * level[i] * piece.falloff.get(i).copied().unwrap_or(1.0) * boost
            }));
            let full = &mut scratch.full;
            full.clear();
            full.extend(
                piece
                    .thrusters
                    .iter()
                    .zip(level.iter())
                    .map(|(t, &l)| t.max_force * l * boost),
            );
            let tmax = &mut scratch.tmax;
            tmax.clear();
            tmax.extend(piece.torque_sources.iter().map(|src| {
                let ps = &parts[src.part as usize];
                if active && producing(ps) {
                    src.max * ps.power * boost
                } else {
                    0.0
                }
            }));
            // `act` matches the tables: the flight computer sizes it every tick, rebuilds size it too.
            piece.wrench = wrench_of(&piece.thrusters, fmax, full, tmax, &piece.act);
            for (i, t) in piece.thrusters.iter().enumerate() {
                piece.output[i] = if t.max_force > 0.0 {
                    (piece.act.throttle[i] * fmax[i] + piece.act.couple[i] * full[i]) / t.max_force
                } else {
                    0.0
                };
                piece.cruise[i] = piece.act.cruise[i] * level[i];
            }
            for g in &mut piece.guns {
                g.output = g.dps * parts[g.part as usize].work;
            }
        }

        // Heat. Generation and conduction explicit, loss implicit. Links never cross pieces, so the ship can
        // walk its pieces' own parts and links (`Piece::heat_links`): a vessel split off a big blueprint then
        // costs what it owns, not the whole blueprint. A ship that still holds most of its blueprint walks all
        // parts and links in one pass instead (cheaper per part). Both give the same numbers: every part's sums
        // run in the same order. Every energy is written before it is read, so the scratch needs no clearing.
        let sun = star * DT;
        let e = &mut scratch.energy;
        if e.len() != parts.len() {
            e.clear();
            e.resize(parts.len(), 0.0);
        }
        let owned: usize = pieces.iter().map(|p| p.parts.len()).sum();
        if 2 * owned >= parts.len() {
            for ((ps, k), e) in parts.iter().zip(heat_k.iter()).zip(e.iter_mut()) {
                if ps.alive {
                    *e = heat_energy(ps, k, sun, heat_mult);
                }
            }
            for l in &d.links {
                conduct(l, parts, e);
            }
            for ((ps, k), &e) in parts.iter_mut().zip(heat_k.iter()).zip(e.iter()) {
                if ps.alive {
                    settle_heat(ps, k, e, overdriven);
                }
            }
        } else {
            for piece in pieces.iter() {
                for &p in &piece.parts {
                    let ps = &parts[p as usize];
                    if ps.alive {
                        e[p as usize] = heat_energy(ps, &heat_k[p as usize], sun, heat_mult);
                    }
                }
                for &l in &piece.heat_links {
                    conduct(&d.links[l as usize], parts, e);
                }
                for &p in &piece.parts {
                    let ps = &mut parts[p as usize];
                    if ps.alive {
                        settle_heat(ps, &heat_k[p as usize], e[p as usize], overdriven);
                    }
                }
            }
        }

        // What the flight computer may plan with next tick (idle, unpowered and overheated actuators out).
        for piece in pieces.iter_mut() {
            refresh_authority(piece, parts);
            if overdriven {
                piece.authority.scale(boost);
            }
        }
        // Overdriven parts past their heat limit burn (GDR-0132).
        self.overdrive_burn();
    }
}

/// Heat step, generation: a part's heat energy this tick before conduction and loss, with `sun` (a hot star's heat
/// this tick per side open to space; 0 adds exactly nothing) through each of its open sides; its work's heat times
/// `mult` (1 but while overdriven, GDR-0132).
#[inline]
pub(super) fn heat_energy(ps: &PartState, k: &HeatK, sun: f32, mult: f32) -> f32 {
    ps.heat * k.cap + k.out * ps.work * mult * DT + sun * ps.open_sides as f32
}

/// Heat step, conduction over one link between alive parts (explicit, from the heat before this tick's step).
#[inline]
pub(super) fn conduct(l: &crate::ship::design::Link, parts: &[PartState], e: &mut [f32]) {
    let (a, b) = (l.a as usize, l.b as usize);
    if !parts[a].alive || !parts[b].alive {
        return;
    }
    let q = tuning::HEAT_CONDUCTANCE * l.edges as f32 * (parts[a].heat - parts[b].heat) * DT;
    e[a] -= q;
    e[b] += q;
}

/// Heat step, loss to space (implicit) and the overheat hysteresis, from the part's energy `e`. While `overdriven`
/// nothing shuts down for heat (GDR-0132: it burns instead, `overdrive.rs`).
#[inline]
pub(super) fn settle_heat(ps: &mut PartState, k: &HeatK, e: f32, overdriven: bool) {
    let h = e / (k.cap + k.loss * DT);
    ps.heat = if h > 0.0 { h } else { 0.0 };
    if overdriven {
        ps.overheated = false;
    } else if ps.heat > k.limit {
        ps.overheated = true;
    } else if ps.overheated && ps.heat < k.limit * tuning::OVERHEAT_RESUME {
        ps.overheated = false;
    }
}

/// The adjacency bonuses part `t` receives now, in the order they apply (its bonus links, sorted by source):
/// the source is alive and, for a `SourceVenting` bonus, has an open side; at most `max_stacks` per group.
/// `groups` is a scratch buffer (group, count). The one rule for [`Ship`]'s stats and the design report.
pub(crate) fn applied_bonuses<'a>(
    d: &'a ShipDesign,
    parts: &[PartState],
    t: PartIdx,
    groups: &mut Vec<(u32, u32)>,
    mut apply: impl FnMut(&'a BonusLink),
) {
    groups.clear();
    for b in d.bonuses_in.get(t as usize) {
        let src = &parts[b.source as usize];
        if !src.alive || (b.needs_venting && src.open_sides == 0) {
            continue;
        }
        let count = match groups.iter_mut().find(|g| g.0 == b.group) {
            Some(g) => &mut g.1,
            None => {
                groups.push((b.group, 0));
                &mut groups.last_mut().unwrap().1
            }
        };
        if *count >= b.max_stacks {
            continue;
        }
        *count += 1;
        apply(b);
    }
}

/// The parts that block gun `slot`'s line of fire now: the alive parts of `piece` among its static blockers
/// (`ShipDesign::gun_blockers`). A gun is clear when there are none.
pub(crate) fn gun_blockers<'a>(
    d: &'a ShipDesign,
    parts: &'a [PartState],
    slot: u32,
    piece: PieceId,
) -> impl Iterator<Item = PartIdx> + 'a {
    d.gun_blockers
        .get(slot as usize)
        .iter()
        .copied()
        .filter(move |&b| {
            let s = &parts[b as usize];
            s.alive && s.piece == piece
        })
}

/// Output of a piece's working reactors (alive, not overheated, not idle), with bonuses.
pub(crate) fn reactor_supply(piece: &Piece, parts: &[PartState]) -> f32 {
    let mut supply = 0.0f32;
    for r in &piece.reactors {
        let ps = &parts[r.part as usize];
        if ps.alive && !ps.overheated && !ps.idle {
            supply += r.output;
        }
    }
    supply
}

/// How hard a consumer is asked to work, 0..1: nothing while its piece is not active or the part is
/// overheated, idle or a gun without a clear line of fire; else engines, RCS pods and gyros their `mover`
/// activity (throttle × rated-speed falloff, |torque used|), guns `fires`, shield emitters `shields`, scanners
/// `charging`, logic cores and other consumers 1.
#[inline]
pub(crate) fn consumer_activity(
    kind: ConsumerKind,
    ps: &PartState,
    active: bool,
    mover: f32,
    fires: bool,
    shields: bool,
    charging: bool,
) -> f32 {
    if !active || ps.overheated || !ps.clear || ps.idle {
        0.0
    } else {
        match kind {
            ConsumerKind::Engine => mover.clamp(0.0, 1.0),
            ConsumerKind::Weapon => fires as u8 as f32,
            ConsumerKind::Scanner | ConsumerKind::Blink | ConsumerKind::Emp => charging as u8 as f32,
            ConsumerKind::Shield => shields as u8 as f32,
            ConsumerKind::Logic | ConsumerKind::Other => 1.0,
        }
    }
}

/// Reactor work (heat share) for a piece whose consumers got `served` of `supply`: `REACTOR_IDLE_HEAT` at no
/// load rising linearly to 1 at full load; 0 when nothing is served.
#[inline]
pub(crate) fn reactor_work(served: f32, supply: f32) -> f32 {
    if served > 0.0 && supply > 0.0 {
        let load = (served / supply).min(1.0);
        tuning::REACTOR_IDLE_HEAT + (1.0 - tuning::REACTOR_IDLE_HEAT) * load
    } else {
        0.0
    }
}

/// The power pass of one piece for one tick: each consumer's demand (`draw × consumer_activity`), the
/// allocation by tier and weight ([`crate::ship::power::allocate`]), then every part's power level and work:
/// consumers their level (0 while idle) and `activity × level`, reactors full level and [`reactor_work`],
/// heaters (heat without a power role) 1 while working. `activity[part]` holds the movers' activity on entry
/// and each consumer's activity on return; `want` and `level` are scratch (per consumer row on return).
/// `fires(slot)`: does the gun in this slot fire; `charging(part)`: does a scan charge on this scanner. Returns
/// `(demand, served)`. The one power rule for [`Ship::update_systems`] and the design report.
#[allow(clippy::too_many_arguments)]
pub(crate) fn power_pass(
    piece: &Piece,
    parts: &mut [PartState],
    activity: &mut [f32],
    want: &mut Vec<f32>,
    level: &mut Vec<f32>,
    supply: f32,
    active: bool,
    fires: impl Fn(u32) -> bool,
    shields: bool,
    charging: impl Fn(PartIdx) -> bool,
) -> (f32, f32) {
    want.clear();
    for c in &piece.consumers {
        let ps = &parts[c.part as usize];
        let act = consumer_activity(
            c.kind,
            ps,
            active,
            activity[c.part as usize],
            fires(c.slot),
            shields,
            charging(c.part),
        );
        activity[c.part as usize] = act;
        want.push(c.draw * act);
    }
    level.clear();
    level.resize(want.len(), 0.0);
    let fed = if active { supply } else { 0.0 };
    let (demand, served) = crate::ship::power::allocate(&piece.consumers, want, fed, level);
    for (c, &l) in piece.consumers.iter().zip(level.iter()) {
        let ps = &mut parts[c.part as usize];
        ps.power = if ps.idle { 0.0 } else { l };
        ps.work = activity[c.part as usize] * ps.power;
    }
    let reactor_work = reactor_work(served, supply);
    for r in &piece.reactors {
        let ps = &mut parts[r.part as usize];
        ps.power = if ps.idle { 0.0 } else { 1.0 };
        ps.work = if ps.overheated || ps.idle {
            0.0
        } else {
            reactor_work
        };
    }
    for &h in &piece.heaters {
        let ps = &mut parts[h as usize];
        ps.work = if active && !ps.overheated && !ps.idle {
            1.0
        } else {
            0.0
        };
    }
    (demand, served)
}
