//! Deterministic RNG and math helpers for the scenario generator.
//!
//! Only integer ops and IEEE basic float ops (+ - * / sqrt) are used, so generation is bit-identical on every
//! platform (native, wasm). No `f64::sin` etc. from std.

/// SplitMix64: tiny, fast, good enough for scenario generation. One stream per subsystem.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    /// Independent stream derived from a seed and a stream id.
    pub fn stream(seed: u64, stream: u64) -> Self {
        let mut r = Rng(seed ^ stream.wrapping_mul(0xA24B_AED4_963E_E407));
        r.next_u64();
        r.next_u64();
        r
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform in [lo, hi).
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f64()
    }

    /// Uniform integer in [lo, hi] (inclusive).
    pub fn int(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next_u64() % ((hi - lo + 1) as u64)) as i64
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.f64() < p
    }

    /// Uniform random unit vector (rejection sampling in the unit disk, then normalise with sqrt).
    pub fn unit(&mut self) -> [f64; 2] {
        loop {
            let x = self.range(-1.0, 1.0);
            let y = self.range(-1.0, 1.0);
            let l2 = x * x + y * y;
            if l2 > 1e-4 && l2 <= 1.0 {
                let l = l2.sqrt();
                return [x / l, y / l];
            }
        }
    }
}

/// Deterministic sin/cos (range reduction + Taylor series, basic ops only). Accurate to ~1e-15 for |x| < 1e3.
pub fn sin_cos(x: f64) -> (f64, f64) {
    const HALF_PI: f64 = core::f64::consts::FRAC_PI_2;
    let k = round_half_away(x / HALF_PI);
    let r = x - k * HALF_PI;
    let r2 = r * r;
    // Taylor to degree 17/16 on |r| <= pi/4.
    let mut s = 0.0;
    let mut c = 0.0;
    let mut term_s = r;
    let mut term_c = 1.0;
    for i in 0..9 {
        s += term_s;
        c += term_c;
        let n = (2 * i + 2) as f64;
        term_s *= -r2 / (n * (n + 1.0));
        term_c *= -r2 / ((n - 1.0) * n);
    }
    match (k as i64).rem_euclid(4) {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

fn round_half_away(x: f64) -> f64 {
    // Avoid relying on platform rounding helpers; truncation via `as` is exact and portable.
    if x >= 0.0 {
        ((x + 0.5) as i64) as f64
    } else {
        -(((-x + 0.5) as i64) as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_cos_matches_std() {
        let mut x = -20.0;
        while x < 20.0 {
            let (s, c) = sin_cos(x);
            assert!((s - x.sin()).abs() < 1e-12, "sin {x}");
            assert!((c - x.cos()).abs() < 1e-12, "cos {x}");
            x += 0.0137;
        }
    }
}
