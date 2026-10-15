//! The controllers: a tick's plan (heading and velocity goals) turned into actuator commands of the controlling
//! piece through the control allocation (`ship::alloc`), the turn rate it aims for, free RCS couples.

use super::*;

/// Desired turn rate towards a heading error `e` (rad).
pub fn turn_rate(e: f32, alpha: f32, tuning: &FlightTuning) -> f32 {
    let mag = e.abs();
    let w = sqrt(2.0 * tuning.turn_margin * alpha * mag)
        .min(tuning.turn_gain * mag)
        .min(tuning.max_turn_rate);
    if e < 0.0 { -w } else { w }
}

/// What the RCS rows can add to the pure torque, counter-clockwise and clockwise (kN·m), while translation that
/// lasts (`alloc::lasting`: a thrust group pushing flat out, e.g. W held or a full strafe) keeps its share of
/// them: for each sense, the torque of translation's fill that turns most that way (`alloc::translate` asked for
/// the most torque of that sense, as the allocation fills when it needs it) plus the couples of that sense in the
/// room it leaves, each row at its last power level as in the authority (never below 0). A nozzle pushing the
/// ship along cannot also turn it in a couple. Without lasting translation, or without couples, the authority's
/// couples: a dead-beat correction is done within the tick and frees its nozzles for the rest of the turn.
pub(super) fn free_couples(
    piece: &Piece,
    parts: &[PartState],
    force: Vec2,
    fmax: &[f32],
    full: &[f32],
    throttle: &mut Vec<f32>,
) -> [f32; 2] {
    let a = &piece.authority;
    if a.couple_ccw <= 0.0 && a.couple_cw <= 0.0 {
        return [a.couple_ccw, a.couple_cw];
    }
    let force = lasting(&piece.alloc, fmax, force);
    if force == Vec2::ZERO {
        return [a.couple_ccw, a.couple_cw];
    }
    let th = &piece.thrusters;
    let mut free = [0.0f32; 2];
    for (k, ccw) in [true, false].into_iter().enumerate() {
        throttle.clear();
        throttle.resize(th.len(), 0.0);
        let most = if ccw { f32::MAX } else { -f32::MAX };
        let made = translate(th, &piece.alloc, fmax, force, most, throttle);
        let room = |i: usize| parts[th[i].part as usize].power * (full[i] - throttle[i] * fmax[i]);
        let couple = couples(th, &piece.alloc, ccw, &room, None);
        free[k] = (if ccw { made } else { -made } + couple).max(0.0);
    }
    free
}

/// Pass 2: turn the plan into actuator commands for the controlling piece; other pieces get nothing.
pub(crate) fn execute(ship: &mut Ship, plan: &Plan, cfg: &SimConfig) {
    let control = ship.control_piece();
    let Ship {
        design,
        pieces,
        parts,
        controls,
        scratch,
        telemetry,
        ..
    } = ship;
    *telemetry = FlightTelemetry::default();
    for piece in pieces.iter_mut() {
        let rows = piece.thrusters.len();
        let sources = piece.torque_sources.len();
        piece.act.resize(rows, sources);
        piece.falloff.clear();
        piece.falloff.resize(rows, 1.0);
        if Some(piece.id) != control || !piece.active {
            piece.assist = TurnAssist::default();
            continue;
        }
        // Rated-speed falloff of every row at this tick's velocity (by the whole speed: one square root per
        // piece), for the allocation below and for the systems (raw throttles too): the same numbers both times.
        let kin = Kin::of(piece, parts);
        let v_local = kin.rot.apply_inverse(kin.vel);
        let speed = v_local.length();
        for (f, t) in piece.falloff.iter_mut().zip(&piece.thrusters) {
            *f = t.falloff(v_local, speed);
        }
        if plan.raw {
            piece.assist = TurnAssist::default();
            for (i, t) in piece.thrusters.iter().enumerate() {
                if t.slot != NO_SLOT {
                    piece.act.throttle[i] = controls
                        .throttle
                        .get(t.slot as usize)
                        .copied()
                        .unwrap_or(0.0)
                        .clamp(0.0, 1.0);
                }
            }
            continue;
        }
        match plan.fire {
            Fire::Hold => {
                controls.fire = false;
                controls.guns.clear();
            }
            // Every group: the trigger alone, no per-gun mask.
            Fire::Groups(FIRE_ALL) => {
                controls.fire = true;
                controls.guns.clear();
            }
            Fire::Groups(mask) => {
                controls.guns.clear();
                controls
                    .guns
                    .extend(design.guns.iter().map(|g| mask & 1 << (g.group - 1) != 0));
                controls.fire = controls.guns.contains(&true);
            }
            Fire::AtWill(ts) => {
                controls.fire = crate::behavior::guns_at_will(
                    piece,
                    &ts,
                    design.guns.len(),
                    &mut controls.guns,
                );
            }
        }
        let Some(goal) = plan.goal else {
            piece.assist = TurnAssist::default();
            continue;
        };
        let tuning = &cfg.flight;

        // Velocity → force in the piece frame.
        let (a_world, dv_len) = match goal.velocity {
            Some(v) => {
                let dv = v - kin.vel;
                (dv * (1.0 / DT), dv.length())
            }
            None => (Vec2::ZERO, 0.0),
        };
        let force = kin.rot.apply_inverse(a_world) * kin.mass;

        // Heading → torque.
        let target = match goal.heading {
            HeadingGoal::Keep | HeadingGoal::Rate(_) => None,
            HeadingGoal::Angle(a) | HeadingGoal::Track { angle: a, .. } => Some(a),
            HeadingGoal::Thrust(d) => (d.length_squared() > 1e-12).then(|| kin.axis_heading(d)),
            HeadingGoal::Auto => {
                if dv_len < AUTO_SETTLED {
                    None
                } else {
                    let u = kin.rot.apply_inverse(a_world) * (1.0 / a_world.length());
                    if kin.cap_along(u) >= tuning.keep_heading_fraction * kin.caps[kin.best] {
                        None
                    } else {
                        Some(kin.axis_heading(a_world))
                    }
                }
            }
        };
        let error = target.map_or(0.0, |t| wrap_angle(t - kin.angle));
        // Turning the thrust towards where it is needed: burn only once roughly aligned (flip, then burn), so
        // slow-turning ships do not chase their own velocity vector.
        let force = match goal.heading {
            HeadingGoal::Thrust(_) | HeadingGoal::Auto if target.is_some() => {
                force * ((cos(error) - ALIGN_NONE) / (ALIGN_FULL - ALIGN_NONE)).clamp(0.0, 1.0)
            }
            _ => force,
        };

        // Availability now, after the speed falloff (`fmax`; `full` without it, for RCS couples): only actuators
        // that can produce (not idle, powered, not overheated, alive: `ship::producing`). Partial power is not
        // scaled in here: it caps what the piece's movers draw together, so the dead-beat request is exact
        // whenever the power suffices, and the systems settle the rest (planning, `Kin`, uses the power levels).
        let full = &mut scratch.full;
        full.clear();
        full.extend(piece.thrusters.iter().map(|t| {
            if producing(&parts[t.part as usize]) {
                t.max_force
            } else {
                0.0
            }
        }));
        let fmax = &mut scratch.fmax;
        fmax.clear();
        fmax.extend(full.iter().zip(&piece.falloff).map(|(&f, &k)| f * k));
        let tmax = &mut scratch.tmax;
        tmax.clear();
        tmax.extend(piece.torque_sources.iter().map(|s| {
            if producing(&parts[s.part as usize]) {
                s.max
            } else {
                0.0
            }
        }));

        // Heading → torque, planned with what the first line can turn with this tick: pure torque and the RCS
        // couples that translation leaves free (a nozzle pushing the ship along cannot also turn it), plus
        // differential thrust while the last resort is on. Keyboard rotation never uses the last resort
        // (GDR-0058): its gate stays closed while a key turns the ship.
        let free = free_couples(piece, parts, force, fmax, full, &mut scratch.translate);
        let key = matches!(goal.heading, HeadingGoal::Rate(_));
        if key {
            piece.assist = TurnAssist::default();
        }
        let alpha = Kin::alpha_with(&piece.authority, free, kin.inertia, piece.assist.on);
        let omega_des = match goal.heading {
            HeadingGoal::Rate(r) => r.clamp(-tuning.max_turn_rate, tuning.max_turn_rate),
            // Feed-forward (GDR-0064): the goal's own turn rate, at most one the ship could stop from within
            // about a quarter turn on what it turns with this tick (as the keys are capped), plus the turn
            // towards the goal, within the max turn rate.
            HeadingGoal::Track { rate, .. } => {
                let lead = sqrt(tuning.turn_margin * alpha * PI);
                (rate.clamp(-lead, lead) + turn_rate(error, alpha, tuning))
                    .clamp(-tuning.max_turn_rate, tuning.max_turn_rate)
            }
            _ => turn_rate(error, alpha, tuning),
        };
        let torque = kin.inertia * (omega_des - kin.omega) * (1.0 / DT);
        // Keyboard rotation (GDR-0055) changes the turn rate only within the turn margin of what the first line
        // gives this tick in that sense (pure torque and the free couples that turn that way), so gyros, cores and
        // RCS couples serve it alone: no sideways push from the keys.
        let torque = if key {
            let couple = if torque > 0.0 { free[0] } else { free[1] };
            let cap = tuning.turn_margin * (piece.authority.torque_pure + couple);
            torque.clamp(-cap, cap)
        } else {
            torque
        };
        let missing = allocate(
            &piece.thrusters,
            &piece.alloc,
            fmax,
            full,
            tmax,
            force,
            torque,
            &mut piece.act,
        );
        // The last resort (GDR-0055): only once the first line has been at its limit for a few ticks, and
        // differential thrust only while steering to a heading (to the end of the turn, so a turn planned with it
        // can brake with it), never to stop a spin without one (`Keep`, a settled `Auto`).
        if !key {
            piece.assist =
                piece
                    .assist
                    .step(missing != 0.0, error.abs() > TURNING, tuning.assist_after);
            if piece.assist.on && missing != 0.0 {
                assist(
                    &piece.thrusters,
                    &piece.alloc,
                    fmax,
                    full,
                    missing,
                    target.is_some(),
                    &mut piece.act,
                );
            }
        }
        let expected = wrench_of(&piece.thrusters, fmax, full, tmax, &piece.act);
        // Cruise flame (GDR-0041): how much of its group's top speed the velocity goal asks along each
        // engine's push direction. Holding the stick at any speed shows it (full stick at the top speed: 1);
        // braking (goal 0) and coasting (no goal) show none.
        if let Some(v) = goal.velocity {
            let v_goal = kin.rot.apply_inverse(v);
            for (c, t) in piece.act.cruise.iter_mut().zip(&piece.thrusters) {
                let top = kin.speeds[group_of(t.dir)];
                if !t.rcs && top > 0.0 {
                    *c = (v_goal.dot(t.dir) / top).clamp(0.0, 1.0);
                }
            }
        }
        *telemetry = FlightTelemetry {
            top_speed: kin.top_speed,
            goal: Some(goal),
            heading_target: target,
            force,
            torque,
            expected,
            assist: piece.assist.on,
        };
    }
}
