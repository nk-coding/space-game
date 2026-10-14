//! The voyage map's own meshes (atlas meshes in stage px: y down on the stage, flipped into the mesh's y up, origin
//! at the stage's centre), drawn under the map's UI text like home's world under its labels; the galaxy behind them
//! is `galaxy`'s.
//!
//! * **The detail** ([`build_detail`]): the markers of the voyages found so far, no route between them
//!   (GDR-0078): open a ring, cleared a filled dot, a new signal a ring sending out pulses, a guardian a coral
//!   diamond, the shipyard a diamond with a ship, the selection a dotted ring around it. Unknown voyages are not
//!   drawn. Placed through the view (`view::MapView`) but sized in stage px, so they keep their size at any zoom;
//!   they fade out towards the panel's edge. Rebuilt when what it shows ([`Look`], with the pulse's step while a
//!   signal shows) or the view changes: a few hundred vertices.
//! * **The chrome** ([`build_chrome`]): the legend's symbols and the panel's backing, fixed to the stage. Built once.

use bevy::prelude::*;

use super::layout::{Geometry, PANEL_X, STAGE, VoyageState};
use crate::atlas::Region;
use crate::meshbuf::MeshBuf;
use crate::style::{Style, mix, vcol};

/// The legend's rows (stage y) and its columns (stage x of the symbols; the words start [`LEGEND_GAP`] after).
pub const LEGEND_ROWS: [f32; 2] = [676.0, 698.0];
pub const LEGEND_COLUMNS: [f32; 4] = [38.0, 132.0, 226.0, 326.0];
pub const LEGEND_GAP: f32 = 14.0;
/// Marker sizes (stage px): the ring of an open voyage, the selection's dotted ring, how far a signal's pulses run.
const OPEN_R: f32 = 8.5;
const SELECT_R: f32 = 19.0;
const PULSE_R: f32 = 25.0;
/// A signal's pulse: one every this many seconds, drawn in this many steps (the detail is rebuilt per step).
pub const PULSE_PERIOD: f32 = 1.6;
pub const PULSE_STEPS: u32 = 32;
/// How far the panel's backing runs on past the stage (a much wider or taller window than the stage).
const PANEL_RUN: f32 = 6000.0;
/// The detail fades out over this many stage px before the panel's edge.
const EDGE_FADE: (f32, f32) = (8.0, 32.0);

/// What the mesh shows besides the content's layout (the rebuild key).
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub states: Vec<VoyageState>,
    pub guardian: Vec<bool>,
    pub selected: Option<usize>,
    pub hovered: Option<usize>,
    pub glow: bool,
    /// The signals' pulse step (0 to [`PULSE_STEPS`]; always 0 while there is no signal).
    pub pulse: u32,
}

impl Look {
    /// The pulse step at `seconds` while a signal shows, else 0 (so the look, the rebuild key, stays still).
    pub fn pulse_at(states: &[VoyageState], seconds: f32) -> u32 {
        if !states.contains(&VoyageState::Signal) {
            return 0;
        }
        ((seconds / PULSE_PERIOD).fract() * PULSE_STEPS as f32) as u32 % PULSE_STEPS
    }
}

/// The mesh builder in stage coordinates (also `galaxy`'s, in galaxy px).
pub struct Pen {
    m: MeshBuf,
    white: Region,
    soft: Region,
    uv: Vec2,
}

/// A stage point in the mesh (origin at the stage's centre, y up).
fn p(s: Vec2) -> Vec2 {
    Vec2::new(s.x - STAGE.x / 2.0, STAGE.y / 2.0 - s.y)
}

impl Pen {
    pub fn new(white: &Region, soft: &Region) -> Self {
        Self { m: MeshBuf::default(), white: *white, soft: *soft, uv: white.uv_center() }
    }

    pub fn into_mesh(self) -> Mesh {
        self.m.into_mesh()
    }

    pub fn disc(&mut self, c: Vec2, r: f32, col: [f32; 4]) {
        self.disc_n(c, r, (r * 2.5).clamp(8.0, 40.0) as usize, col);
    }

    /// A disc of `n` sides.
    pub fn disc_n(&mut self, c: Vec2, r: f32, n: usize, col: [f32; 4]) {
        let pts: Vec<Vec2> = (0..n).map(|k| p(c + Vec2::from_angle(k as f32 / n as f32 * std::f32::consts::TAU) * r)).collect();
        self.m.polygon(&pts, self.uv, col);
    }

    pub fn ring(&mut self, c: Vec2, r: f32, half: f32, col: [f32; 4]) {
        let n = (r * 2.0).clamp(16.0, 96.0) as usize;
        let pts: Vec<Vec2> = (0..n).map(|k| p(c + Vec2::from_angle(k as f32 / n as f32 * std::f32::consts::TAU) * r)).collect();
        self.m.strip(&pts, true, half, &self.white, col);
    }

    pub fn dotted_ring(&mut self, c: Vec2, r: f32, dots: usize, dot: f32, col: [f32; 4]) {
        for k in 0..dots {
            self.disc(c + Vec2::from_angle(k as f32 / dots as f32 * std::f32::consts::TAU) * r, dot, col);
        }
    }

    pub fn line(&mut self, a: Vec2, b: Vec2, half: f32, col: [f32; 4]) {
        self.m.strip(&[p(a), p(b)], false, half, &self.white, col);
    }

    /// Dots every `step` from `a` to `b`.
    pub fn dotted(&mut self, a: Vec2, b: Vec2, step: f32, dot: f32, col: [f32; 4]) {
        let n = (a.distance(b) / step).floor() as usize;
        for k in 0..=n {
            self.disc(a.lerp(b, k as f32 / n.max(1) as f32), dot, col);
        }
    }

    /// A soft glow (the atlas's soft dot) `size` across.
    pub fn glow(&mut self, c: Vec2, size: f32, col: [f32; 4]) {
        let (a, b) = (p(c) - Vec2::splat(size / 2.0), p(c) + Vec2::splat(size / 2.0));
        self.m.region_box(&self.soft, a, b, 0, false, col);
    }

    pub fn diamond(&mut self, c: Vec2, r: f32, col: [f32; 4]) {
        let pts = [c + Vec2::new(0.0, -r), c + Vec2::new(r, 0.0), c + Vec2::new(0.0, r), c + Vec2::new(-r, 0.0)].map(p);
        self.m.polygon(&pts, self.uv, col);
    }

    pub fn diamond_outline(&mut self, c: Vec2, r: f32, half: f32, col: [f32; 4]) {
        let pts = [c + Vec2::new(0.0, -r), c + Vec2::new(r, 0.0), c + Vec2::new(0.0, r), c + Vec2::new(-r, 0.0)].map(p);
        self.m.strip(&pts, true, half, &self.white, col);
    }

    pub fn rect(&mut self, min: Vec2, max: Vec2, col: [f32; 4]) {
        let pts = [Vec2::new(min.x, max.y), max, Vec2::new(max.x, min.y), min].map(p);
        self.m.polygon(&pts, self.uv, col);
    }

    /// A dotted circle: dots about every `step` px.
    pub fn dotted_circle(&mut self, centre: Vec2, r: f32, step: f32, dot: f32, col: [f32; 4]) {
        self.dotted_ring(centre, r, (std::f32::consts::TAU * r / step).ceil() as usize, dot, col);
    }
}

/// `col` with its alpha times `k`.
fn faded(mut col: [f32; 4], k: f32) -> [f32; 4] {
    col[3] *= k;
    col
}

/// How much of the detail shows at stage x `x`: all of it left of the panel, fading out over [`EDGE_FADE`].
fn edge(x: f32) -> f32 {
    ((PANEL_X - EDGE_FADE.0 - x) / (EDGE_FADE.1 - EDGE_FADE.0)).clamp(0.0, 1.0)
}

/// A new signal's pulses around `at`: two rings running out from an open voyage's ring, fading as they go (`t`
/// the pulse's phase, 0 to 1), over a glow; `f` fades a colour towards the panel.
fn pulses(pen: &mut Pen, at: Vec2, t: f32, glow: bool, tone: Color, f: impl Fn([f32; 4]) -> [f32; 4]) {
    if glow {
        pen.glow(at, 44.0, f(vcol(tone, 0.4)));
    }
    for k in [t, (t + 0.5).fract()] {
        let r = OPEN_R + (PULSE_R - OPEN_R) * k;
        pen.ring(at, r, 1.0, f(vcol(tone, 0.95 * (1.0 - k))));
    }
}

/// The detail's mesh: the known voyages' markers and the shipyard, each galaxy point placed on the stage by `to`.
pub fn build_detail(geo: &Geometry, look: &Look, to: impl Fn(Vec2) -> Vec2, style: &Style, white: &Region, soft: &Region) -> Mesh {
    let ui = &style.ui;
    let mut pen = Pen::new(white, soft);
    let glow = look.glow;

    // The shipyard: a diamond with a ship in it.
    let h = to(geo.home);
    let k = edge(h.x);
    if k > 0.0 {
        if glow {
            pen.glow(h, 40.0, faded(vcol(ui.teal, 0.18), k));
        }
        pen.diamond_outline(h, 13.0, 0.9, faded(vcol(ui.number, 0.9), k));
        pen.m.polygon(&[h + Vec2::new(0.0, -5.5), h + Vec2::new(4.5, 3.5), h + Vec2::new(-4.5, 3.5)].map(p), pen.uv, faded(vcol(ui.teal, 1.0), k));
    }

    // The voyages.
    for (n, node) in geo.nodes.iter().copied().enumerate() {
        let at = to(node);
        let k = edge(at.x);
        if k <= 0.0 {
            continue;
        }
        let f = |col| faded(col, k);
        let state = look.states.get(n).copied().unwrap_or(VoyageState::Unknown);
        if !state.known() {
            continue;
        }
        let guardian = look.guardian.get(n).copied().unwrap_or(false);
        if look.hovered == Some(n) && look.selected != Some(n) {
            pen.ring(at, 16.0, 0.6, f(vcol(ui.number, 0.35)));
        }
        let pulse = look.pulse as f32 / PULSE_STEPS as f32;
        if guardian {
            if state == VoyageState::Signal {
                pulses(&mut pen, at, pulse, glow, ui.coral, f);
            } else if glow {
                pen.glow(at, 40.0, f(vcol(ui.coral, 0.25)));
            }
            pen.diamond_outline(at, 12.5, 1.0, f(vcol(ui.coral, 0.95)));
            let inner = if matches!(state, VoyageState::Cleared { .. }) { ui.teal } else { ui.coral };
            pen.diamond(at, 5.0, f(vcol(inner, 0.95)));
        } else {
            match state {
                VoyageState::Signal => {
                    pulses(&mut pen, at, pulse, glow, ui.teal, f);
                    pen.ring(at, OPEN_R, 1.1, f(vcol(ui.teal, 1.0)));
                    pen.disc(at, 3.4, f(vcol(ui.teal, 1.0)));
                }
                VoyageState::Open => {
                    if glow {
                        pen.glow(at, 36.0, f(vcol(ui.teal, 0.35)));
                    }
                    pen.ring(at, OPEN_R, 1.0, f(vcol(ui.teal, 1.0)));
                    pen.disc(at, 3.4, f(vcol(ui.teal, 1.0)));
                }
                VoyageState::Cleared { .. } => {
                    if glow {
                        pen.glow(at, 32.0, f(vcol(ui.teal, 0.45)));
                    }
                    pen.disc(at, 6.5, f(vcol(mix(ui.teal, ui.number, 0.25), 1.0)));
                }
                VoyageState::Unknown => {}
            }
        }
        if look.selected == Some(n) {
            let tone = if guardian { ui.coral } else { ui.teal };
            pen.dotted_ring(at, SELECT_R, 28, 1.05, f(vcol(tone, 0.85)));
        }
    }
    pen.into_mesh()
}

/// The chrome's mesh: the legend's symbols and the panel's backing.
pub fn build_chrome(style: &Style, white: &Region, soft: &Region) -> Mesh {
    let ui = &style.ui;
    let mut pen = Pen::new(white, soft);
    // The legend's symbols: home, open, cleared, signal; enemies, wrecks, goal site, guardian.
    let (r0, r1) = (LEGEND_ROWS[0], LEGEND_ROWS[1]);
    let c = LEGEND_COLUMNS;
    pen.diamond_outline(Vec2::new(c[0], r0), 5.5, 0.7, vcol(ui.number, 0.85));
    pen.ring(Vec2::new(c[1], r0), 4.5, 0.7, vcol(ui.teal, 1.0));
    pen.disc(Vec2::new(c[2], r0), 4.0, vcol(mix(ui.teal, ui.number, 0.25), 1.0));
    pen.ring(Vec2::new(c[3], r0), 2.6, 0.7, vcol(ui.teal, 1.0));
    pen.ring(Vec2::new(c[3], r0), 5.6, 0.6, vcol(ui.teal, 0.5));
    pen.disc(Vec2::new(c[0], r1), 3.0, vcol(ui.coral, 1.0));
    pen.disc(Vec2::new(c[1], r1), 3.0, vcol(ui.gold, 1.0));
    pen.diamond(Vec2::new(c[2], r1), 4.5, vcol(ui.gold, 1.0));
    pen.diamond_outline(Vec2::new(c[3], r1), 5.5, 0.7, vcol(ui.coral, 1.0));

    // The panel's backing (over the galaxy, which shows through faintly) and its edge; it runs on past the stage
    // for wider windows.
    let back = mix(style.background.outer(), ui.number, 0.02);
    pen.rect(Vec2::new(PANEL_X, -PANEL_RUN), Vec2::new(STAGE.x + PANEL_RUN, STAGE.y + PANEL_RUN), vcol(back, 0.8));
    pen.line(Vec2::new(PANEL_X, 24.0), Vec2::new(PANEL_X, STAGE.y - 24.0), 0.5, vcol(ui.label, 0.22));
    pen.into_mesh()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The detail fades out over the last px before the panel.
    #[test]
    fn the_detail_stops_at_the_panel() {
        assert_eq!((edge(PANEL_X - 100.0), edge(PANEL_X - EDGE_FADE.0)), (1.0, 0.0));
        assert!((edge(PANEL_X - (EDGE_FADE.0 + EDGE_FADE.1) / 2.0) - 0.5).abs() < 1e-5);
    }

    /// The pulse runs through its steps while a signal shows and stands still at 0 otherwise.
    #[test]
    fn the_pulse_runs_only_for_a_signal() {
        let (open, signal) = ([VoyageState::Open, VoyageState::Unknown], [VoyageState::Open, VoyageState::Signal]);
        assert_eq!(Look::pulse_at(&open, 0.7), 0);
        assert_eq!(Look::pulse_at(&signal, 0.0), 0);
        assert_eq!(Look::pulse_at(&signal, PULSE_PERIOD / 2.0), PULSE_STEPS / 2);
        assert!((0..400).all(|k| Look::pulse_at(&signal, k as f32 * 0.037) < PULSE_STEPS));
    }
}
