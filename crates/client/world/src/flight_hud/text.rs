//! The HUD's texts, rebuilt at 8 Hz and laid out again only when they change: hull, hold, speed, the target, the
//! title and subtitle, banners, warnings, the help line.

use super::*;
use crate::settings::Controls;

/// The three clocks the HUD reads (one parameter: a system takes at most sixteen).
type Clocks<'w> = (Res<'w, Time<Real>>, Res<'w, Time<Virtual>>, Res<'w, Time<Fixed>>);

#[allow(clippy::too_many_arguments)]
pub(super) fn update_text(
    (time, virtual_time, fixed): Clocks,
    (controls, menu, layer): (Res<Controls>, Res<FlightMenu>, Res<crate::settings::SettingsLayer>),
    mut state: ResMut<HudState>,
    world: Res<SimWorld>,
    scene: Res<Scene>,
    focus: Res<PilotFocus>,
    input: Res<PlayerInput>,
    watch: Res<BehaviorWatch>,
    toggles: Res<RenderToggles>,
    rig: Res<CameraRig>,
    run: Res<VoyageRun>,
    style: Res<Style>,
    transit: Res<Transit>,
    mut texts: Query<(&HudText, &mut Text, &mut TextColor)>,
    mut root: Query<&mut ScreenFx, With<HudRoot>>,
) {
    // The HUD closes for the hand-over at the end of a voyage or a lost test flight (`warp_fx.rs`).
    for mut fx in &mut root {
        // SETTINGS over the flight takes the screen.
        show_screen(&mut fx, toggles.hud && !transit.hud_out() && !layer.open);
    }
    if !state.timer.tick(time.delta()).just_finished() {
        return;
    }
    let HudState { scratch, labels, labels_for, .. } = &mut *state;
    let alpha = fixed.overstep_fraction();
    let player = focus.0.or(scene.watched()).and_then(|id| world.sim.ship(id));
    let control = player.and_then(|s| s.control_piece().and_then(|c| s.piece(c)));
    // Watch: nobody pilots or locks; the target panel shows what the watched ship's behavior targets.
    let watching = focus.0.is_none() && scene.watched().is_some();
    let target = if watching { player.and_then(|s| s.behavior()).and_then(|b| b.target()).map(Target::Ship) } else { input.lock };

    // Labels per part of the shown ship's design (a design edited in the drydock can keep its part count).
    if let Some(s) = player
        && !labels_for.as_ref().is_some_and(|d| Arc::ptr_eq(d, s.design()))
    {
        *labels_for = Some(s.design().clone());
        labels.clear();
        let mut seen: Vec<(String, u32)> = Vec::new();
        for p in &s.design().parts {
            let name = world.content.parts.get(&p.id).map(|d| d.name.to_uppercase()).unwrap_or_else(|| p.id.to_uppercase());
            let n = match seen.iter_mut().find(|e| e.0 == name) {
                Some(e) => {
                    e.1 += 1;
                    e.1
                }
                None => {
                    seen.push((name.clone(), 1));
                    1
                }
            };
            labels.push(format!("{name} {n}"));
        }
    }

    let ui = style.ui;
    let tick = world.sim.tick();
    let voyage = world.sim.voyage().filter(|_| scene.voyage().is_some());
    let lost = matches!(run.phase, Phase::Lost { .. });
    let warping = matches!(run.phase, Phase::Warping { .. });
    for (kind, mut text, mut color) in &mut texts {
        let s = &mut *scratch;
        s.clear();
        match kind {
            HudText::HullValue => match player {
                Some(ship) => {
                    let _ = write!(s, "{:.0} %", hull_fraction(ship, control.map(|p| p.id)) * 100.0);
                }
                None => s.push('–'),
            },
            HudText::Cargo if voyage.is_some() => s.push_str("SALVAGE"),
            HudText::Cargo => {
                // Salvage in the hold (GDR-0063): cargo units in use / the hold of the controlling piece.
                if let Some(ship) = player {
                    let (used, cap) = (ship.cargo().used(), ship.cargo_capacity());
                    if cap > 0 || used > 0 {
                        let _ = write!(s, "CARGO {used} / {cap}");
                    }
                }
            }
            HudText::SpeedValue => {
                if let Some(p) = control {
                    let _ = write!(s, "{:.1}", v2(p.body.linvel).length());
                } else {
                    s.push('–');
                }
            }
            HudText::SpeedSub => {
                s.push_str("M/S");
                // The top speed of the engines pushing along the best axis (GDR-0041).
                let top = player.map_or(0.0, |ship| ship.telemetry().top_speed);
                if top > 0.0 {
                    let _ = write!(s, "  ·  TOP {top:.0}");
                }
                if input.drift {
                    s.push_str("  ·  DRIFT");
                }
            }
            HudText::TargetName | HudText::TargetValue | HudText::TargetSub => {
                let from = control.map(|p| piece_com(p, alpha)).or(rig.focus);
                let lock_key = (!watching).then(|| controls.0.inputs(Action::LockNext).next().map(|c| c.label())).flatten();
                target_text(*kind, s, &world, target, (!watching).then_some(lock_key.as_deref().unwrap_or("—")), from);
            }
            HudText::Title => match &scene.kind {
                SceneKind::Sandbox => {
                    let name = match scene.test_mode() {
                        Some(TestMode::Watch) => "WATCH",
                        Some(TestMode::Alongside) => "FLY ALONGSIDE",
                        None if scene.test_flight => "TEST FLIGHT",
                        None => "TEST RANGE",
                    };
                    s.push_str(name);
                    if scene.test_mode().is_some() && !watch.name.is_empty() {
                        let _ = write!(s, "  ·  {}", watch.name.to_uppercase());
                    }
                    let _ = write!(s, "  ·  WAVE {}", scene.wave);
                }
                SceneKind::Showcase => s.push_str("SHOWCASE"),
                SceneKind::Load(n) => {
                    let _ = write!(s, "LOAD TEST  ·  {n} SHIPS");
                }
                SceneKind::Voyage(id) => s.push_str(&voyage::title(&world.content, id)),
            },
            HudText::Subtitle => {
                let alive = world.sim.vessels(crate::scene::ENEMY).filter(|v| v.state == sim::VesselState::Running).count();
                match scene.kind {
                    SceneKind::Sandbox => {
                        // The player's vessels are in the fleet bar (`fleet_bar.rs`).
                        let _ = write!(s, "ENEMIES {} / {}  ·  KILLS {}", alive, scene.wave_ships.len(), scene.kills);
                        s.push_str(if scene.waves { "  ·  WAVES ON" } else { "  ·  WAVES OFF" });
                    }
                    SceneKind::Load(_) => {
                        let active = world.sim.ships().iter().filter(|s| s.is_active()).count();
                        let pieces: usize = world.sim.ships().iter().map(|s| s.pieces().len()).sum();
                        let _ = write!(s, "ACTIVE {}  ·  PIECES {}  ·  SHOTS IN FLIGHT {}", active, pieces, world.sim.projectiles().len());
                    }
                    SceneKind::Showcase => s.push_str("DRAG TO PAN  ·  WHEEL TO ZOOM"),
                    SceneKind::Voyage(_) => {
                        if let Some(v) = voyage {
                            let (done, all) = voyage::sites_done(v);
                            let _ = write!(s, "SITES {done} / {all}  ·  {}", voyage::clock(tick));
                        }
                        if run.launched.as_ref().is_some_and(|l| l.practice.is_some()) {
                            s.push_str("  ·  PRACTICE, NOT SAVED");
                        }
                    }
                }
            }
            HudText::Banner => {
                // The flight menu says it itself.
                if virtual_time.is_paused() && !menu.open {
                    s.push_str("PAUSED");
                } else if let Phase::Warping { home_at } = run.phase {
                    let what = if transit.emergency() { "EMERGENCY WARP" } else { "WARPING HOME" };
                    let _ = write!(s, "{what}  ·  {}", voyage::seconds_left(home_at, tick));
                } else if let Some((b, until)) = &scene.banner
                    && world.sim.time_seconds() < *until
                    && !lost
                {
                    s.push_str(b);
                }
                // The emergency recall's warp is amber (GDR-0074, `warp_fx.rs`).
                let tone = match (warping, transit.emergency()) {
                    (true, true) => Tone::Gold,
                    (true, false) => Tone::Teal,
                    _ => scene.banner_tone,
                };
                set_color(&mut color, ui_color(tone.color(&ui), 1.0));
            }
            HudText::BannerSub => {
                if warping {
                    s.push_str("THE HAUL COMES WITH YOU");
                } else if !virtual_time.is_paused()
                    && let Some((_, until)) = &scene.banner
                    && world.sim.time_seconds() < *until
                    && !lost
                {
                    s.push_str(&scene.banner_sub);
                }
            }
            HudText::SalvageValue => {
                if let Some(ship) = player {
                    let _ = write!(s, "{} / {}", ship.cargo().used(), ship.cargo_capacity());
                } else if lost {
                    s.push('–');
                }
                set_color(&mut color, ui_color(ui.gold, if lost { 0.5 } else { 1.0 }));
            }
            HudText::SalvageSub => {
                if lost {
                    s.push_str("LEFT IN THE WRECK");
                    set_color(&mut color, ui_color(ui.coral, 1.0));
                } else {
                    match player {
                        Some(ship) if ship.cargo_capacity() > 0 => s.push_str(&hold_name(&world.content, ship)),
                        Some(_) => s.push_str("NO HOLD"),
                        None => {}
                    }
                    set_color(&mut color, ui_color(ui.dim, 1.0));
                }
            }
            HudText::RecallSub => {
                let (_, line, enabled) = voyage::recall_words(run.phase, run.can_recall, run.emergency, tick);
                s.push_str(&line);
                let tone = if enabled || warping { Tone::Teal } else { Tone::Dim };
                set_color(&mut color, ui_color(tone.color(&ui), 1.0));
            }
            HudText::LostTitle => s.push_str(if matches!(run.phase, Phase::Lost { abandoned: true, .. }) { "VOYAGE ABANDONED" } else { "SHIP LOST" }),
            HudText::LostSub => s.push_str("RECALLED HOME"),
            HudText::LostNote => {
                if let Phase::Lost { left, abandoned, .. } = run.phase {
                    if left > 0 {
                        let place = if abandoned { "BEHIND" } else { "IN THE WRECK" };
                        let _ = write!(s, "THE HAUL STAYS {place}  ·  {left} {} LOST  ·  ", if left == 1 { "PART" } else { "PARTS" });
                    }
                    s.push_str("YOUR SHIPS RE-FORM AT HOME");
                }
            }
            HudText::LostCount | HudText::LostHome => {
                if let Phase::Lost { home_at, .. } = run.phase {
                    let n = voyage::seconds_left(home_at, tick);
                    if *kind == HudText::LostCount {
                        let _ = write!(s, "{n}");
                    } else {
                        let _ = write!(s, "HOME IN {n} S");
                    }
                }
            }
            HudText::Warnings => {
                if let (Some(ship), Some(p)) = (player, control) {
                    warnings(s, ship, p, labels);
                }
            }
            HudText::Help => {
                // The bound inputs (the first of each; the fire button's all), GDR-0090.
                let k = |a: Action| controls.0.inputs(a).next().map_or_else(|| "—".to_string(), |c| c.label());
                let all = |a: Action| controls.0.inputs(a).map(|c| c.label()).collect::<Vec<_>>().join("/");
                let fly = format!(
                    "{}/{} TURN · {}/{} THRUST · {}/{} STRAFE · CLICK LOCK · {} FIRE · {} TARGET · {} DRIFT · {} VESSEL",
                    k(Action::TurnLeft),
                    k(Action::TurnRight),
                    k(Action::ThrustForward),
                    k(Action::ThrustBack),
                    k(Action::StrafeLeft),
                    k(Action::StrafeRight),
                    all(Action::Fire),
                    k(Action::LockNext),
                    k(Action::Drift),
                    k(Action::FocusNext),
                );
                let line = match scene.kind {
                    SceneKind::Sandbox if scene.watched().is_some() => format!(
                        "NOBODY PILOTS · DRAG PAN · {} FOLLOW / FREE · WHEEL ZOOM · {} WAVES · {} RULES · {} BACK · {} HOME · {} HIDE",
                        k(Action::CameraFollow),
                        k(Action::Waves),
                        k(Action::DebugOverlay),
                        k(Action::TestReturn),
                        k(Action::Leave),
                        k(Action::HideHud),
                    ),
                    SceneKind::Sandbox if input.touch_ui => "STICK FLY  ·  TAP SHIP LOCK  ·  FIRE BUTTON  ·  PINCH ZOOM".into(),
                    SceneKind::Sandbox => format!(
                        "{fly} · {} WAVES · {} DRYDOCK · {} HOME · ESC MENU · {} TUNING · {} DEBUG · {} HIDE",
                        k(Action::Waves),
                        k(Action::TestReturn),
                        k(Action::Leave),
                        k(Action::Tuning),
                        k(Action::DebugOverlay),
                        k(Action::HideHud),
                    ),
                    SceneKind::Voyage(_) if input.touch_ui => "STICK FLY  ·  TAP SHIP LOCK  ·  FIRE BUTTON  ·  PINCH ZOOM  ·  RECALL".into(),
                    SceneKind::Voyage(_) => format!(
                        "{fly} · {} RECALL · {} ABANDON · ESC MENU · {} TUNING · {} DEBUG · {} HIDE",
                        k(Action::Recall),
                        k(Action::Leave),
                        k(Action::Tuning),
                        k(Action::DebugOverlay),
                        k(Action::HideHud),
                    ),
                    _ => format!(
                        "DRAG PAN  ·  WHEEL ZOOM  ·  {} FRAME / FREE  ·  {} DEBUG SHAPES  ·  {} GLOW  ·  {} BLOOM  ·  {} STATS  ·  {} HIDE  ·  {} DRYDOCK  ·  {} HOME",
                        k(Action::CameraFollow),
                        k(Action::DebugShapes),
                        k(Action::GlowToggle),
                        k(Action::BloomToggle),
                        k(Action::DebugOverlay),
                        k(Action::HideHud),
                        k(Action::TestReturn),
                        k(Action::Leave),
                    ),
                };
                s.push_str(&line);
            }
        }
        set_text(&mut text, scratch);
    }
}

/// The hold's name under the salvage number: the biggest alive cargo part of the controlling piece (`CARGO HOLD`).
pub(super) fn hold_name(content: &data::Content, ship: &sim::Ship) -> String {
    let control = ship.control_piece();
    let alive = |i: usize| {
        let st = ship.part(i as u32);
        st.alive && Some(st.piece) == control
    };
    let best = ship.design().parts.iter().enumerate().filter(|(i, p)| p.cargo > 0.0 && alive(*i)).max_by(|a, b| a.1.cargo.total_cmp(&b.1.cargo).then(b.0.cmp(&a.0)));
    best.map_or_else(|| "CARGO".into(), |(_, p)| content.parts.get(&p.id).map_or(p.id.to_uppercase(), |d| d.name.to_uppercase()))
}

/// Hull integrity: hit points of the controlling piece over the full design.
pub fn hull_fraction(ship: &sim::Ship, control: Option<sim::PieceId>) -> f32 {
    let mut have = 0.0;
    let mut full = 0.0;
    for (i, p) in ship.design().parts.iter().enumerate() {
        full += p.hp;
        let st = ship.part(i as u32);
        if st.alive && Some(st.piece) == control {
            have += st.hp;
        }
    }
    if full > 0.0 { have / full } else { 0.0 }
}

/// The target panel: the player's lock (with how to lock when there is none), or in watch mode the watched
/// ship's behavior target (`hint`: the lock key; none: nobody can lock).
pub(super) fn target_text(kind: HudText, s: &mut String, world: &SimWorld, target: Option<Target>, hint: Option<&str>, from: Option<Vec2>) {
    let Some(lock) = target else {
        if kind == HudText::TargetName {
            s.push_str("NO TARGET");
        } else if kind == HudText::TargetSub && let Some(key) = hint {
            let _ = write!(s, "CLICK OR {key} TO LOCK");
        }
        return;
    };
    let (ship, piece) = match lock {
        Target::Ship(id) => {
            let ship = world.sim.ship(id);
            (ship, ship.and_then(|s| s.control_piece().or_else(|| s.main_piece())).and_then(|p| ship.and_then(|s| s.piece(p))))
        }
        Target::Piece(id, p) => {
            let ship = world.sim.ship(id);
            (ship, ship.and_then(|s| s.piece(p)))
        }
        Target::Point(_) => (None, None),
    };
    let (Some(ship), Some(piece)) = (ship, piece) else { return };
    match kind {
        HudText::TargetName => {
            let name = &ship.design().name;
            if piece.controlled {

                for c in name.chars() {
                    s.extend(c.to_uppercase());
                }
            } else {
                s.push_str("SALVAGE");
            }
        }
        HudText::TargetValue => {
            let _ = write!(s, "{:.0} %", hull_fraction(ship, Some(piece.id)) * 100.0);
        }
        HudText::TargetSub => {
            let d = from.map(|f| v2(piece.com_world()).distance(f)).unwrap_or(0.0);
            if d >= 1000.0 {
                let _ = write!(s, "{:.1} KM", d / 1000.0);
            } else {
                let _ = write!(s, "{:.0} M", d);
            }
            if !ship.is_active() {
                s.push_str("  ·  DARK");
            }
        }
        _ => {}
    }
}

/// The two hottest parts near their limit, power shortage, shields down, no power.
pub(super) fn warnings(s: &mut String, ship: &sim::Ship, piece: &sim::Piece, labels: &[String]) {
    let mut hot: [(f32, u32); 2] = [(0.0, 0); 2];
    let mut shields_down = 0;
    for &i in &piece.parts {
        let st = ship.part(i);
        let limit = ship.stats(i).get(Stat::HeatLimit);
        if limit.is_finite() && limit > 0.0 {
            let r = if st.overheated { 1.0 } else { st.heat / limit };
            if r > 0.8 {
                if r > hot[0].0 {
                    hot[1] = hot[0];
                    hot[0] = (r, i);
                } else if r > hot[1].0 {
                    hot[1] = (r, i);
                }
            }
        }
        if st.shield_down && ship.design().parts[i as usize].is_shield {
            shields_down += 1;
        }
    }
    let sep = |s: &mut String| {
        if !s.is_empty() {
            s.push_str("   ·   ");
        }
    };
    if !piece.active {
        sep(s);
        s.push_str("NO POWER");
    }
    for (r, i) in hot {
        if r > 0.0 {
            sep(s);
            let name = labels.get(i as usize).map(String::as_str).unwrap_or("PART");
            let _ = write!(s, "{name}  ·  HEAT {:.0} %", (r * 100.0).min(100.0));
        }
    }
    if piece.power.demand > 0.0 && piece.power.served < piece.power.demand * 0.97 {
        sep(s);
        let _ = write!(s, "POWER {:.0} %", piece.power.served / piece.power.demand * 100.0);
    }
    if shields_down > 0 {
        sep(s);
        let _ = write!(s, "SHIELD DOWN {shields_down}");
    }
}
