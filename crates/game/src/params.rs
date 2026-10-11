//! Launch parameters. On the web they come from the page URL (`?zoom=80&debug=1`), natively from the
//! `GAME_PARAMS` environment variable in the same `key=value&key=value` form. Used for screenshots and
//! measurements (start in a given mode without pressing keys).

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

    pub fn text(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    pub fn flag(&self, key: &str) -> Option<bool> {
        self.0.get(key).map(|v| !matches!(v.as_str(), "0" | "false" | "off" | "no"))
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
