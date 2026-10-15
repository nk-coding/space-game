//! The test range's waves (GDR-0074 sized them, GDR-0089 fills them): how big a wave is, which enemies it sends and
//! where they come in. Shared by the test range (`world::scene`, the sandbox's endless waves) and the arena's
//! TEST-RANGE WAVES gauntlet (GDR-0092), so the lab scores a design against the waves the player tries it on.
//!
//! A wave spends a budget of the flown design's build points ([`WAVE_SHARE`]) on the content's enemy ships (tagged
//! [`ENEMY_TAG`]), the biggest first ([`wave_mix`], [`wave`]); its ships come in on an arc around the ship they are
//! sent at, heading for it ([`wave_bodies`]). Pure and deterministic: the caller rolls the arc, on the sim's
//! `Scenario` stream in the arena, on the client's own generator in the test range.

use data::Content;

use crate::math::{Vec2, atan2, cos, sin};
use crate::ship::Body;
use crate::ship::design::ShipDesign;

/// Content ships with this tag are enemies: the waves draw from all of them.
pub const ENEMY_TAG: &str = "enemy";
/// A wave's budget is this share of the flown design's build points. PLACEHOLDER.
pub const WAVE_SHARE: f32 = 0.5;
/// The most ships in a wave.
pub const WAVE_CAP: u32 = 8;
/// A wave's ships come in this far from the ship they are sent at (m), ...
pub const WAVE_RADIUS: (f32, f32) = (105.0, 125.0);
/// ... this far apart on their arc (rad), heading for it at this speed (m/s). PLACEHOLDERS.
pub const WAVE_SPREAD: f32 = 0.32;
pub const WAVE_SPEED: f32 = 3.0;

/// A wave (PLACEHOLDER rule): a budget of the flown design's build points times [`WAVE_SHARE`], spent on the biggest
/// enemies first (each as often as it fits whole), then one more of the smallest if at least half of it is left; at
/// least one ship, at most [`WAVE_CAP`]. `enemies` holds their build points, smallest first; the result says how many
/// of each. With the content of 15 Oct 2026 the starter `rim_skiff` (59) meets a hunter, the Halvex Runner (132.5)
/// two hunters and a raider, a 300-point design two wardens (Niklas, 15 Oct 2026: a single ship against a swarm of
/// small ones felt wrong).
pub fn wave_mix(design_bp: f32, enemies: &[f32]) -> Vec<u32> {
    let mut n = vec![0u32; enemies.len()];
    let Some(&smallest) = enemies.first().filter(|bp| **bp > 0.0) else {
        return n;
    };
    let mut left = if design_bp > 0.0 { design_bp * WAVE_SHARE } else { 0.0 };
    let mut total = 0;
    for k in (0..enemies.len()).rev() {
        while enemies[k] > 0.0 && enemies[k] <= left && total < WAVE_CAP {
            n[k] += 1;
            total += 1;
            left -= enemies[k];
        }
    }
    if total == 0 || (left >= smallest / 2.0 && total < WAVE_CAP) {
        n[0] += 1;
    }
    n
}

/// A wave's ships in the order they come in, the biggest first: [`wave_mix`] over `enemies` (each with its build
/// points, smallest first).
pub fn wave<T: Clone>(design_bp: f32, enemies: &[(T, f32)]) -> Vec<T> {
    let bp: Vec<f32> = enemies.iter().map(|e| e.1).collect();
    let mix = wave_mix(design_bp, &bp);
    enemies
        .iter()
        .zip(&mix)
        .rev()
        .flat_map(|((e, _), n)| std::iter::repeat_n(e.clone(), *n as usize))
        .collect()
}

/// Where the `n` ships of a wave come in around `centre` (the ship they are sent at): on an arc at a random bearing,
/// [`WAVE_SPREAD`] apart, each at a random distance within [`WAVE_RADIUS`], nose and velocity towards `centre`.
/// `roll(lo, hi)` draws a number in `lo..hi`: the bearing first, then each ship's distance.
pub fn wave_bodies(centre: Vec2, n: usize, mut roll: impl FnMut(f32, f32) -> f32) -> Vec<Body> {
    let base = roll(0.0, core::f32::consts::TAU);
    (0..n)
        .map(|k| {
            let a = base + (k as f32 - (n as f32 - 1.0) / 2.0) * WAVE_SPREAD;
            let r = roll(WAVE_RADIUS.0, WAVE_RADIUS.1);
            let pos = centre + Vec2::new(cos(a), sin(a)) * r;
            let d = centre - pos;
            let len = d.length();
            let to = if len > 0.0 { d * (1.0 / len) } else { Vec2::ZERO };
            Body {
                pos,
                angle: atan2(-to.x, to.y),
                linvel: to * WAVE_SPEED,
                angvel: 0.0,
            }
        })
        .collect()
}

/// A compiled design's build points (as placed: hull parts with their material's factor).
pub fn build_points(design: &ShipDesign) -> f32 {
    design.parts.iter().map(|p| p.build_points).sum()
}

/// The content's enemy ships (tagged [`ENEMY_TAG`], those that compile) with their build points, smallest first:
/// what [`wave`] draws from.
pub fn wave_enemies(content: &Content) -> Vec<(String, f32)> {
    let mut out: Vec<(String, f32)> = content
        .ships
        .iter()
        .filter(|s| s.tags.iter().any(|t| t == ENEMY_TAG))
        .filter_map(|s| Some((s.id.clone(), build_points(&ShipDesign::compile(content, s).ok()?))))
        .collect();
    out.sort_by(|a, b| a.1.total_cmp(&b.1));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GDR-0074, GDR-0089: a wave spends the flown design's budget on the biggest enemies first, at least one ship,
    /// at most the cap.
    #[test]
    fn waves_follow_the_flown_design_biggest_first() {
        let kinds = [15.8, 23.6, 75.0];
        assert_eq!(wave_mix(59.0, &kinds), [0, 1, 0], "the starter meets a hunter");
        assert_eq!(wave_mix(132.5, &kinds), [1, 2, 0]);
        assert_eq!(wave_mix(300.0, &kinds), [0, 0, 2]);
        assert_eq!(wave_mix(40.0, &kinds), [1, 0, 0], "20 points: a raider and a little left over");
        assert_eq!(wave_mix(1.0, &kinds), [1, 0, 0], "at least one ship");
        assert_eq!(wave_mix(0.0, &kinds), [1, 0, 0]);
        assert_eq!(wave_mix(50.0, &[]), Vec::<u32>::new(), "no enemy designs");
        assert_eq!(wave_mix(100_000.0, &kinds).iter().sum::<u32>(), WAVE_CAP);
        // The content's designs, compiled; a wave comes in the biggest first.
        let c = data::base_content();
        let enemies = wave_enemies(&c);
        let ids: Vec<&str> = enemies.iter().map(|e| e.0.as_str()).collect();
        assert_eq!(ids, ["automaton_raider", "automaton_hunter", "automaton_warden"]);
        let bp = |id: &str| build_points(&ShipDesign::compile(&c, c.ships.get(id).unwrap()).unwrap());
        assert_eq!(wave(bp("rim_skiff"), &enemies), ["automaton_hunter"]);
        assert_eq!(wave(bp("halvex_runner"), &enemies), ["automaton_hunter", "automaton_hunter", "automaton_raider"]);
        assert_eq!(wave(300.0, &enemies), ["automaton_warden", "automaton_warden"]);
    }

    /// A wave comes in on an arc around the ship, every ship within the radius and heading for it.
    #[test]
    fn a_wave_comes_in_around_the_ship() {
        let centre = Vec2::new(10.0, -20.0);
        let mut rng = crate::rng::Pcg32::new(7, 3);
        let bodies = wave_bodies(centre, 3, |lo, hi| rng.range_f32(lo, hi));
        assert_eq!(bodies.len(), 3);
        for b in &bodies {
            let d = (b.pos - centre).length();
            assert!((WAVE_RADIUS.0..=WAVE_RADIUS.1).contains(&d), "{d}");
            let to = (centre - b.pos) * (1.0 / d);
            assert!((b.linvel - to * WAVE_SPEED).length() < 1e-4);
            // The nose (+y in the ship's frame) points at the centre.
            let nose = Vec2::new(-sin(b.angle), cos(b.angle));
            assert!((nose - to).length() < 1e-4, "{nose:?} {to:?}");
        }
        assert!(wave_bodies(centre, 0, |lo, _| lo).is_empty());
    }
}
