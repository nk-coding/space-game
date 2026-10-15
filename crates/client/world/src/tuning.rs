//! Live tuning panel (egui, `U` to open; flight only, T is the drydock). For feel tests in the browser; the
//! values are placeholders until Niklas picks them, then they move into content / `SimConfig` defaults.
//!
//! * Live (applied to the running sim with `Sim::set_config`): thrust scale, torque scale (separately or
//!   linked), speed scale (every engine's and RCS pod's rated speed, so every top speed), max turn rate, key
//!   turn rate (A/D held, GDR-0055), the player's mind capacity (logic cores running at once; fewer than the
//!   fleet has puts the newest vessels to sleep, GDR-0044).
//! * Next wave: raiders per wave. By default a wave follows the flown design (`sim::voyage::waves`, GDR-0074, GDR-0089);
//!   unticking "follow the flown ship" (or `?raiders=N`) sets the size by hand.
//! * Immediate: drift default.
//! * **Reset**: weapon range and damage scales change the content the designs are compiled from, so they
//!   apply with "Apply & reset" (the scene restarts). The range scale applies to every side, the damage scale
//!   to the player's ships only (the raiders keep their content damage).
//!
//! Every value can also come from the URL
//! (`?thrust=4&torque=2&speed=1.5&turn=6&keyturn=1.5&mind=24&range=1.5&damage=1&raiders=3&drift=1`; `raiders` only
//! when set by hand), and the panel shows that query string ready to copy.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use data::Content;
use sim::{Owner, SimConfig};
use kit::params::Params;

use crate::controls::PlayerInput;
use crate::mode::AppMode;
use crate::scene::{ResetRequest, Scene, SceneKind};
use crate::sim_bridge::{SimWorld, console_out};

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Tuning {
    pub thrust: f32,
    pub torque: f32,
    pub link: bool,
    pub speed: f32,
    pub max_turn_rate: f32,
    /// A/D held to the end of the ramp (`FlightTuning::key_turn_rate`, rad/s).
    pub key_turn_rate: f32,
    /// The player's mind capacity (`SimConfig::mind_capacity`).
    pub mind_capacity: u32,
    pub range_scale: f32,
    pub damage_scale: f32,
    /// Raiders per wave set by hand; `None`: each wave follows the flown design (`sim::voyage::waves`, GDR-0074, GDR-0089).
    pub raiders_per_wave: Option<u32>,
    pub drift: bool,
    /// The weapon scales the running scene was built with.
    pub applied: (f32, f32),
    pub open: bool,
}

impl Tuning {
    fn from_params(p: &Params) -> Self {
        let d = SimConfig::default();
        let range = p.f32("range").unwrap_or(1.0);
        let damage = p.f32("damage").unwrap_or(1.0);
        Self {
            thrust: p.f32("thrust").unwrap_or(d.thrust_scale),
            torque: p.f32("torque").unwrap_or(d.torque_scale),
            link: p.flag("link").unwrap_or(false),
            speed: p.f32("speed").unwrap_or(d.speed_scale),
            max_turn_rate: p.f32("turn").unwrap_or(d.flight.max_turn_rate),
            key_turn_rate: p.f32("keyturn").unwrap_or(d.flight.key_turn_rate),
            mind_capacity: p.f32("mind").map(|m| m.max(0.0) as u32).unwrap_or(d.mind_capacity(Owner::PLAYER)),
            range_scale: range,
            damage_scale: damage,
            raiders_per_wave: p.f32("raiders").map(|r| r.max(1.0) as u32),
            drift: p.flag("drift").unwrap_or(false),
            applied: (range, damage),
            open: p.flag("tuning").unwrap_or(false),
        }
    }

    /// The sim config for these values.
    pub fn config(&self) -> SimConfig {
        let mut c = SimConfig { thrust_scale: self.thrust, torque_scale: self.torque, speed_scale: self.speed, ..SimConfig::default() };
        c.flight.max_turn_rate = self.max_turn_rate;
        c.flight.key_turn_rate = self.key_turn_rate;
        c.mind_capacity[Owner::PLAYER.0 as usize] = self.mind_capacity;
        c
    }

    /// Content with the weapon scales applied: the player's ships compile from `player` = true (range and
    /// damage scaled), every other side's from `false` (range only).
    pub fn scaled_content(&self, base: &Content, player: bool) -> Content {
        let damage = if player { self.damage_scale } else { 1.0 };
        let mut c = base.clone();
        if self.range_scale != 1.0 || damage != 1.0 {
            for p in c.parts.iter_mut() {
                if let Some(w) = p.weapon.as_mut() {
                    w.range *= self.range_scale;
                    w.damage *= damage;
                }
            }
        }
        c
    }

    /// The values as a copyable line and as a URL query.
    pub fn text(&self) -> String {
        let (raiders, query) = match self.raiders_per_wave {
            Some(n) => (n.to_string(), format!("&raiders={n}")),
            None => ("ship".to_string(), String::new()),
        };
        format!(
            "thrust_scale={:.2} torque_scale={:.2} speed_scale={:.2} max_turn_rate={:.2} key_turn_rate={:.2} mind_capacity={} weapon_range_scale={:.2} player_weapon_damage_scale={:.2} raiders_per_wave={raiders} drift={}\n?thrust={:.2}&torque={:.2}&speed={:.2}&turn={:.2}&keyturn={:.2}&mind={}&range={:.2}&damage={:.2}{query}&drift={}",
            self.thrust,
            self.torque,
            self.speed,
            self.max_turn_rate,
            self.key_turn_rate,
            self.mind_capacity,
            self.range_scale,
            self.damage_scale,
            self.drift,
            self.thrust,
            self.torque,
            self.speed,
            self.max_turn_rate,
            self.key_turn_rate,
            self.mind_capacity,
            self.range_scale,
            self.damage_scale,
            self.drift as u8,
        )
    }
}

pub struct TuningPlugin;

impl Plugin for TuningPlugin {
    fn build(&self, app: &mut App) {
        let params = app.world().resource::<Params>().clone();
        let t = Tuning::from_params(&params);
        app.insert_resource(t)
            .add_systems(Startup, |t: Res<Tuning>, mut input: ResMut<PlayerInput>| input.drift = t.drift)
            .add_systems(Update, toggle_panel.run_if(in_state(AppMode::Flight)))
            .add_systems(EguiPrimaryContextPass, panel.run_if(in_state(AppMode::Flight)));
    }
}

fn toggle_panel(act: crate::settings::Act, mut t: ResMut<Tuning>, mut contexts: EguiContexts) {
    let typing = contexts.ctx_mut().is_ok_and(|c| c.egui_wants_keyboard_input());
    if act.just_pressed(data::input::Action::Tuning) && !typing {

        t.open = !t.open;
    }
}

fn panel(
    mut contexts: EguiContexts,
    mut t: ResMut<Tuning>,
    mut world: ResMut<SimWorld>,
    mut reset: ResMut<ResetRequest>,
    mut input: ResMut<PlayerInput>,
    scene: Res<Scene>,
) -> Result {
    if !t.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let before = t.clone();
    let mut open = true;
    let mut copy = false;
    let mut log = false;
    egui::Window::new("TUNING")
        .open(&mut open)
        .default_pos(egui::pos2(16.0, 120.0))
        .default_width(330.0)
        .resizable(false)
        .show(ctx, |ui| {
            let t = &mut *t;
            ui.label(egui::RichText::new("Live").strong());
            let old_thrust = t.thrust;
            let old_torque = t.torque;
            ui.add(egui::Slider::new(&mut t.thrust, 0.25..=12.0).logarithmic(true).text("thrust scale"));
            ui.add(egui::Slider::new(&mut t.torque, 0.25..=12.0).logarithmic(true).text("torque scale"));
            ui.checkbox(&mut t.link, "link thrust and torque");
            if t.link {
                if t.thrust != old_thrust {
                    t.torque = (t.torque * t.thrust / old_thrust).clamp(0.25, 12.0);
                } else if t.torque != old_torque {
                    t.thrust = (t.thrust * t.torque / old_torque).clamp(0.25, 12.0);
                }
            }
            ui.add(egui::Slider::new(&mut t.speed, 0.25..=4.0).logarithmic(true).text("speed scale (rated speeds)"));
            ui.add(egui::Slider::new(&mut t.max_turn_rate, 0.5..=12.0).text("max turn rate"));
            ui.add(egui::Slider::new(&mut t.key_turn_rate, 0.25..=6.0).text("key turn rate (A/D)"));
            ui.add(egui::Slider::new(&mut t.mind_capacity, 1..=48).text("mind capacity (cores)"));
            ui.checkbox(&mut t.drift, "drift (keys: pure thrust, no drift cancelling)");
            ui.separator();
            ui.label(egui::RichText::new("Next wave").strong());
            let mut follow = t.raiders_per_wave.is_none();
            if ui.checkbox(&mut follow, "follow the flown ship (GDR-0074)").changed() {
                t.raiders_per_wave = if follow { None } else { Some(3) };
            }
            if let Some(n) = t.raiders_per_wave.as_mut() {
                ui.add(egui::Slider::new(n, 1..=12).text("raiders per wave"));
            }
            ui.separator();
            ui.label(egui::RichText::new("On reset").strong());
            ui.add(egui::Slider::new(&mut t.range_scale, 0.25..=4.0).logarithmic(true).text("weapon range"));
            ui.add(egui::Slider::new(&mut t.damage_scale, 0.25..=4.0).logarithmic(true).text("weapon damage (yours)"));
            let pending = (t.range_scale, t.damage_scale) != t.applied;
            ui.horizontal(|ui| {
                if ui.button(if pending { "Apply & reset" } else { "Reset scene" }).clicked() {
                    reset.0 = true;
                    t.applied = (t.range_scale, t.damage_scale);
                }
                if ui.button("Defaults").clicked() {
                    let open = t.open;
                    *t = Tuning::from_params(&Params::default());
                    t.open = open;
                }
            });
            if pending {
                ui.colored_label(egui::Color32::from_rgb(0xec, 0x8f, 0x7c), "weapon scales apply on reset");
            }
            ui.separator();
            let mut text = t.text();
            ui.add(egui::TextEdit::multiline(&mut text).font(egui::TextStyle::Monospace).desired_rows(3).desired_width(f32::INFINITY));
            ui.horizontal(|ui| {
                copy = ui.button("Copy").clicked();
                log = ui.button("Log to console").clicked();
            });
            if matches!(scene.kind, SceneKind::Load(_)) {
                ui.small("Load test: changing values breaks the hash comparison.");
            }
            if scene.kind == SceneKind::Fight {
                ui.small("A replayed fight plays by its own rules: the panel doesn't change it.");
            }
        });
    if copy {
        ctx.copy_text(t.text());
    }
    if copy || log {
        console_out(&format!("# tuning {}", t.text().replace('\n', " ")));
    }
    t.open = open;
    // A replayed fight keeps the arena's settings, or it would no longer end as recorded.
    if *t != before && scene.kind != SceneKind::Fight {
        // The panel does not tune salvage: the scene's settings stay (the sandbox's loose-item lifetime).
        let cfg = SimConfig { salvage: world.sim.config().salvage, ..t.config() };
        if *world.sim.config() != cfg {
            world.sim.set_config(cfg);
        }
        if t.drift != before.drift {
            input.drift = t.drift;
        }
    }
    Ok(())
}
