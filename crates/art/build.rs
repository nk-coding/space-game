//! Embeds the style tokens (`art/tokens.json`). Icons are content (`content/base/icons`, read through `data`).

use std::path::Path;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let tokens = Path::new(&manifest).join("../../art/tokens.json").canonicalize().expect("art/tokens.json");
    println!("cargo:rerun-if-changed={}", tokens.display());
    let out = format!("pub static TOKENS_JSON: &str = include_str!({:?});\n", tokens.display().to_string());
    let dest = Path::new(&std::env::var("OUT_DIR").unwrap()).join("embedded.rs");
    std::fs::write(dest, out).expect("write embedded.rs");
}
