//! Launch parameters (ADR-0046). On the web they come from the page URL (`?dev=1`), natively from the `GAME_PARAMS`
//! environment variable in the same `key=value&key=value` form.
//!
//! * **`dev=1`** ([`Params::dev`]): the dev mode: its own save with everything open (`world::career`), the whole
//!   galaxy on the map with each cluster's sites, and the debug keys (`world::settings::Act`). `newgame=1` with it
//!   starts the dev save over.
//! * **Tool flags** for scripts and agents (no player feature, they work without `dev=1`): `scene` (where the game
//!   starts, `mode::AppMode::at_start`: `home` (the default), `sandbox` (the TEST RANGE), `showcase`, `watch`,
//!   `alongside`, `drydock`, `behavior`, `research`, `map`, `fight`), `behavior`, `fight`, `load`, `seed`, `hashlog`,
//!   `hashticks`, `hud` ([`RenderToggles`]) and `slowmo`.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

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

    /// The dev mode (`?dev=1`).
    pub fn dev(&self) -> bool {
        self.flag("dev").unwrap_or(false)
    }
}

/// Render switches: F5–F7 and F8 in the dev mode, glow and bloom from the saved display settings, the HUD (H, and
/// the tool flag `hud=0`).
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
        Self { debug_shapes: false, bloom: false, glow_sprites: true, hud: p.flag("hud").unwrap_or(true), debug_info: false }
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

    /// `dev` alone or `dev=1` is the dev mode; `dev=0` and nothing are not.
    #[test]
    fn the_dev_mode_is_one_flag() {
        assert!(Params::parse("?dev").dev() && Params::parse("?scene=map&dev=1").dev());
        assert!(!Params::parse("?dev=0").dev() && !Params::parse("").dev());
    }
}
