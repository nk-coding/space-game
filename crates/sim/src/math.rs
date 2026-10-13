//! Deterministic math: a tiny `Vec2`, rotations and the only allowed transcendental functions.
//!
//! # The rule (cross-platform bit-exact results)
//!
//! * `+ − × ÷`, `sqrt`, `abs`, `floor`/`ceil`/`round`/`trunc`, `min`/`max`, comparisons and float↔int casts
//!   are exact IEEE-754 operations (Rust casts saturate): identical on x86_64, aarch64 and wasm32.
//! * Transcendental functions (`sin`, `cos`, `tan`, `atan2`, `exp`, `ln`, `powf`, `hypot`, ...) go through this
//!   module only. They call the `libm` crate (pure Rust, pinned, `arch` feature off), never std's methods,
//!   which call the platform's libm and may differ per OS/CPU. `crates/sim/clippy.toml` bans the std methods.
//! * No `mul_add` (FMA rounds once instead of twice), no `powi` (unspecified rounding sequence), no fast-math.
//!   Rust never fuses `a * b + c` on its own.
//! * Everything is `f32` (the physics engines are `f32` too). Evaluation order is the source order; keep sums in
//!   a fixed iteration order (ordered containers only).
//! * NaN payloads may differ between platforms; the state hash canonicalises NaN, and no NaN should ever be
//!   produced in the first place (debug assertions check the hashed state).
//!
//! # Frames (see `docs/sim.md`)
//!
//! Piece-local and world frames are physics-style: x right, y up, metres, angles counter-clockwise in radians.

use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

/// Sine (libm, pure Rust).
#[inline]
pub fn sin(x: f32) -> f32 {
    libm::sinf(x)
}

/// Cosine (libm, pure Rust).
#[inline]
pub fn cos(x: f32) -> f32 {
    libm::cosf(x)
}

/// `atan2(y, x)` (libm, pure Rust).
#[inline]
pub fn atan2(y: f32, x: f32) -> f32 {
    libm::atan2f(y, x)
}

/// `e^x` (libm, pure Rust).
#[inline]
pub fn exp(x: f32) -> f32 {
    libm::expf(x)
}

/// Square root. IEEE-754 requires it to be correctly rounded, so std's (hardware) version is exact everywhere.
#[inline]
pub fn sqrt(x: f32) -> f32 {
    x.sqrt()
}

/// A 2D vector in metres (or m/s, kN, ... depending on use).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    pub const X: Vec2 = Vec2 { x: 1.0, y: 0.0 };
    pub const Y: Vec2 = Vec2 { x: 0.0, y: 1.0 };

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }

    #[inline]
    pub fn dot(self, o: Vec2) -> f32 {
        self.x * o.x + self.y * o.y
    }

    /// 2D cross product (z of the 3D cross): `self.x * o.y − self.y * o.x`. Torque = `r.cross(f)`.
    #[inline]
    pub fn cross(self, o: Vec2) -> f32 {
        self.x * o.y - self.y * o.x
    }

    /// `ω × r` for a scalar angular velocity: the velocity of point `r` on a body spinning at `w` rad/s.
    #[inline]
    pub fn cross_scalar(w: f32, r: Vec2) -> Vec2 {
        Vec2::new(-w * r.y, w * r.x)
    }

    /// Rotated 90° counter-clockwise.
    #[inline]
    pub fn perp(self) -> Vec2 {
        Vec2::new(-self.y, self.x)
    }

    #[inline]
    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    #[inline]
    pub fn length(self) -> f32 {
        sqrt(self.length_squared())
    }

    #[inline]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// Each component, or 0 where it is NaN or infinite ([`finite_or_zero`]).
    #[inline]
    pub fn finite_or_zero(self) -> Vec2 {
        Vec2::new(finite_or_zero(self.x), finite_or_zero(self.y))
    }
}

/// `x`, or 0 if it is NaN or infinite: external input (tools, packets, replays) is cleaned at the boundary, so no
/// NaN reaches the state or the physics.
#[inline]
pub fn finite_or_zero(x: f32) -> f32 {
    if x.is_finite() { x } else { 0.0 }
}

impl Add for Vec2 {
    type Output = Vec2;
    #[inline]
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    #[inline]
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    #[inline]
    fn mul(self, s: f32) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}

impl Neg for Vec2 {
    type Output = Vec2;
    #[inline]
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}

impl AddAssign for Vec2 {
    #[inline]
    fn add_assign(&mut self, o: Vec2) {
        self.x += o.x;
        self.y += o.y;
    }
}

impl SubAssign for Vec2 {
    #[inline]
    fn sub_assign(&mut self, o: Vec2) {
        self.x -= o.x;
        self.y -= o.y;
    }
}

/// A rotation stored as `(cos, sin)` of a counter-clockwise angle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rot {
    pub cos: f32,
    pub sin: f32,
}

impl Rot {
    pub const IDENTITY: Rot = Rot { cos: 1.0, sin: 0.0 };

    /// From a counter-clockwise angle in radians (libm).
    #[inline]
    pub fn from_angle(angle: f32) -> Rot {
        Rot {
            cos: cos(angle),
            sin: sin(angle),
        }
    }

    /// Rotates `v` counter-clockwise by this rotation (local → world).
    #[inline]
    pub fn apply(self, v: Vec2) -> Vec2 {
        Vec2::new(
            self.cos * v.x - self.sin * v.y,
            self.sin * v.x + self.cos * v.y,
        )
    }

    /// Rotates `v` clockwise by this rotation (world → local).
    #[inline]
    pub fn apply_inverse(self, v: Vec2) -> Vec2 {
        Vec2::new(
            self.cos * v.x + self.sin * v.y,
            -self.sin * v.x + self.cos * v.y,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_is_counter_clockwise() {
        let r = Rot::from_angle(core::f32::consts::FRAC_PI_2);
        let v = r.apply(Vec2::X);
        assert!(v.x.abs() < 1e-6 && (v.y - 1.0).abs() < 1e-6);
        let back = r.apply_inverse(v);
        assert!((back.x - 1.0).abs() < 1e-6 && back.y.abs() < 1e-6);
    }

    #[test]
    fn cross_scalar_matches_3d_cross() {
        // ω = +1 (ccw), r = +x → velocity +y.
        assert_eq!(Vec2::cross_scalar(1.0, Vec2::X), Vec2::Y);
        assert_eq!(Vec2::X.cross(Vec2::Y), 1.0);
    }

    #[test]
    fn libm_values_are_pinned() {
        // Exact bit patterns: if a libm update ever changed these, replays and saved hashes would break.
        assert_eq!(sin(1.0).to_bits(), 0x3f576aa4);
        assert_eq!(cos(1.0).to_bits(), 0x3f0a5140);
        assert_eq!(atan2(1.0, 2.0).to_bits(), 0x3eed6338);
        assert_eq!(exp(1.0).to_bits(), 0x402df854);
    }
}
