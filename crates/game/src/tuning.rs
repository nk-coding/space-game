//! Live tuning panel (egui, `T` to open). For feel tests in the browser; the values are placeholders until
//! Niklas picks them, then they move into content / `SimConfig` defaults.
//!
//! * Live (applied to the running sim with `Sim::set_config`): thrust scale, torque scale (separately or
//!   linked), soft top speed factor, max turn rate.
//! * Next wave: raiders per wave.
//! * Immediate: drift default.
//! * **Reset**: weapon range and damage scales change the content the designs are compiled from, so they
//!   apply with "Apply & reset" (the scene restarts).
//!
//! Every value can also come from the URL (`?thrust=4&torque=2&top=12&turn=3&range=1.5&damage=1&raiders=3&drift=1`),
//! and the panel shows that query string ready to copy.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use data::Content;
use sim::SimConfig;

use crate::controls::PlayerInput;
use crate::params::Params;
use crate::scene::{ResetRequest, Scene, SceneKind};
use crate::sim_bridge::{SimWorld, console_out};

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Tuning {
    pub thrust: f32,
    pub torque: f32,
    pub link: bool,
    pub top_speed_k: f32,
    pub max_turn_rate: f32,
    pub range_scale: f32,
    pub damage_scale: f32,
    pub raiders_per_wave: u32,
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
            top_speed_k: p.f32("top").unwrap_or(d.flight.top_speed_k),
            max_turn_rate: p.f32("turn").unwrap_or(d.flight.max_turn_rate),
            range_scale: range,
            damage_scale: damage,
            raiders_per_wave: p.f32("raiders").map(|r| r as u32).unwrap_or(3),
            drift: p.flag("drift").unwrap_or(false),
            applied: (range, damage),
            open: p.flag("tuning").unwrap_or(false),
        }
    }

    /// The sim config for these values.
    pub fn config(&self) -> SimConfig {
        let mut c = SimConfig::default();
        c.thrust_scale = self.thrust;
        c.torque_scale = self.torque;
        c.flight.top_speed_k = self.top_speed_k;
        c.flight.max_turn_rate = self.max_turn_rate;
        c
    }

    /// Content with weapon range and damage scaled.
    pub fn scaled_content(&self, base: &Content) -> Content {
        let mut c = base.clone();
        if self.range_scale != 1.0 || self.damage_scale != 1.0 {
            for p in c.parts.iter_mut() {
                if let Some(w) = p.weapon.as_mut() {
                    w.range *= self.range_scale;
                    w.damage *= self.damage_scale;
                }
            }
        }
        c
    }

    /// The values as a copyable line and as a URL query.
    pub fn text(&self) -> String {
        format!(
            "thrust_scale={:.2} torque_scale={:.2} top_speed_k={:.1} max_turn_rate={:.2} weapon_range_scale={:.2} weapon_damage_scale={:.2} raiders_per_wave={} drift={}\n?thrust={:.2}&torque={:.2}&top={:.1}&turn={:.2}&range={:.2}&damage={:.2}&raiders={}&drift={}",
            self.thrust,
            self.torque,
            self.top_speed_k,
            self.max_turn_rate,
            self.range_scale,
            self.damage_scale,
            self.raiders_per_wave,
            self.drift,
            self.thrust,
            self.torque,
            self.top_speed_k,
            self.max_turn_rate,
            self.range_scale,
            self.damage_scale,
            self.raiders_per_wave,
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
            .add_systems(Update, toggle_panel)
            .add_systems(EguiPrimaryContextPass, panel);
    }
}

fn toggle_panel(keys: Res<ButtonInput<KeyCode>>, mut t: ResMut<Tuning>, mut contexts: EguiContexts) {
    let typing = contexts.ctx_mut().is_ok_and(|c| c.egui_wants_keyboard_input());
    if keys.just_pressed(KeyCode::KeyT) && !typing {
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
            ui.add(egui::Slider::new(&mut t.top_speed_k, 4.0..=60.0).text("top speed k"));
            ui.add(egui::Slider::new(&mut t.max_turn_rate, 0.5..=8.0).text("max turn rate"));
            ui.checkbox(&mut t.drift, "drift (released stick coasts)");
            ui.separator();
            ui.label(egui::RichText::new("Next wave").strong());
            ui.add(egui::Slider::new(&mut t.raiders_per_wave, 1..=12).text("raiders per wave"));
            ui.separator();
            ui.label(egui::RichText::new("On reset").strong());
            ui.add(egui::Slider::new(&mut t.range_scale, 0.25..=4.0).logarithmic(true).text("weapon range"));
            ui.add(egui::Slider::new(&mut t.damage_scale, 0.25..=4.0).logarithmic(true).text("weapon damage"));
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
        });
    if copy {
        ctx.copy_text(t.text());
    }
    if copy || log {
        console_out(&format!("# tuning {}", t.text().replace('\n', " ")));
    }
    t.open = open;
    if *t != before {
        let cfg = t.config();
        if *world.sim.config() != cfg {
            world.sim.set_config(cfg);
        }
        if t.drift != before.drift {
            input.drift = t.drift;
        }
    }
    Ok(())
}
