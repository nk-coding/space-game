//! Launch parameters. On the web they come from the page URL (`?zoom=80&debug=1`), natively from the
//! `GAME_PARAMS` environment variable in the same `key=value&key=value` form. Used for screenshots and
//! measurements (start in a given mode without pressing keys).
//!
//! `scene` picks where the game starts (`mode::AppMode::at_start`): `home` (the default), `sandbox` (the TEST
//! RANGE), `showcase`, `watch`, `alongside`, `drydock`, `behavior`, `research`, `map` (the voyage map). Without a
//! `scene`, the parameters that only mean something in flight ([`FLIGHT_ONLY`]) start the sandbox, as before the game
//! started at home.
//!
//! [`RenderToggles`]: the render switches some of them set (`debug`, `bloom`, `glow`, `hud`, `stats`).

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

/// Parameters that only mean something in flight (the load test, the hash log, wingmen, the sim seed, the flight
/// zoom, the showcase's attackers, the touch widgets).
pub const FLIGHT_ONLY: [&str; 8] = ["load", "hashlog", "hashticks", "fleet", "seed", "zoom", "fire", "touch"];

#[derive(Resource, Default, Debug, Clone)]
pub struct Params(HashMap<String, String>);

impl Params {
    pub fn load() -> Self {
        Self::parse(&raw_query())
    }

    pub fn parse(q: &str) -> Self {
        let map = q
            .trim_start_matches('?')
            .split('&')
            .filter(|kv| !kv.is_empty())
            .map(|kv| match kv.split_once('=') {
                Some((k, v)) => (k.to_string(), v.to_string()),
                None => (kv.to_string(), "1".to_string()),
            })
            .collect();
        Self(map)
    }

    pub fn f32(&self, key: &str) -> Option<f32> {
        self.0.get(key).and_then(|v| v.parse().ok())
    }

    /// A whole number, exact over the full range (seeds, tick counts; an `f32` loses digits above 2^24).
    pub fn u64(&self, key: &str) -> Option<u64> {
        self.0.get(key).and_then(|v| v.parse().ok())
    }

    pub fn text(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    pub fn flag(&self, key: &str) -> Option<bool> {
        self.0.get(key).map(|v| !matches!(v.as_str(), "0" | "false" | "off" | "no"))
    }

    /// Is any [`FLIGHT_ONLY`] parameter given?
    pub fn wants_flight(&self) -> bool {
        FLIGHT_ONLY.iter().any(|k| self.0.contains_key(*k))
    }
}

/// Render switches. The spike measures style 2a vs. debug shapes and pre-blurred glow sprites vs. bloom.
#[derive(Resource, Debug, Clone)]
pub struct RenderToggles {
    /// Plain coloured cells only: no seams, icons, glow, plumes, trails or backdrop.
    pub debug_shapes: bool,
    /// Full-screen bloom (HDR camera).
    pub bloom: bool,
    /// Pre-blurred glow layers (icon glows, hull halo, aura, soft muzzle glows).
    pub glow_sprites: bool,
    pub hud: bool,
    /// Fps / timings overlay.
    pub debug_info: bool,
}

impl RenderToggles {
    pub fn from_params(p: &Params) -> Self {
        Self {
            debug_shapes: p.flag("debug").unwrap_or(false),
            bloom: p.flag("bloom").unwrap_or(false),
            glow_sprites: p.flag("glow").unwrap_or(true),
            hud: p.flag("hud").unwrap_or(true),
            debug_info: p.flag("stats").unwrap_or(false),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn raw_query() -> String {
    web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn raw_query() -> String {
    std::env::var("GAME_PARAMS").unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seeds and tick counts parse exactly (as `f32`, 123456789 became 123456792 and the browser's hash stream
    /// never matched the native one).
    #[test]
    fn whole_numbers_parse_exactly() {
        let p = Params::parse("?seed=123456789&hashticks=0&big=18446744073709551615&neg=-3&half=2.5");
        assert_eq!(p.u64("seed"), Some(123_456_789));
        assert_eq!(p.u64("hashticks"), Some(0));
        assert_eq!(p.u64("big"), Some(u64::MAX));
        assert_eq!((p.u64("neg"), p.u64("half"), p.u64("missing")), (None, None, None));
        assert_eq!(p.f32("half"), Some(2.5));
    }
}
