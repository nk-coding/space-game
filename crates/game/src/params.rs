//! Launch parameters. On the web they come from the page URL (`?zoom=80&debug=1`), natively from the
//! `GAME_PARAMS` environment variable in the same `key=value&key=value` form. Used for screenshots and
//! measurements (start in a given mode without pressing keys).
//!
//! `scene` picks where the game starts (`mode::AppMode::at_start`): `home` (the default), `sandbox` (the TEST
//! RANGE), `showcase`, `watch`, `alongside`, `drydock`, `behavior`. Without a `scene`, the parameters that only
//! mean something in flight ([`FLIGHT_ONLY`]) start the sandbox, as before the game started at home.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

/// Parameters that only mean something in flight (the load test, the hash log, wingmen, the sim seed, the flight
/// zoom, the showcase's attackers, raiders per wave, the mind capacity, the tuning panel, the touch widgets).
pub const FLIGHT_ONLY: [&str; 11] = ["load", "hashlog", "hashticks", "fleet", "seed", "zoom", "fire", "raiders", "mind", "tuning", "touch"];

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

#[cfg(target_arch = "wasm32")]
fn raw_query() -> String {
    web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn raw_query() -> String {
    std::env::var("GAME_PARAMS").unwrap_or_default()
}
