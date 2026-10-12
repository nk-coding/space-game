//! Game content for Space Game: schema, pack loader, validation, stat lines and shared blueprint geometry.
//!
//! No Bevy, no threads, no filesystem in the core: packs are built from in-memory `(path, contents)` pairs,
//! so the same code runs natively, on wasm32 and later on Android. See `docs/content-model.md`.
//!
//! ```no_run
//! let content = data::base_content();
//! let ship = content.ships.get("halvex_runner").unwrap();
//! let grid = content.ship_grid(ship).unwrap();
//! let pieces = grid.pieces();
//! let main = grid.main_piece(&pieces, |i| content.parts.get(&ship.parts[i].part).unwrap().logic.is_some()).unwrap();
//! let outline = data::geometry::hull_loops(&grid.piece_hull_edges(&pieces, main));
//! ```

pub mod art_keys;
pub mod behavior;
pub mod edit;
pub mod geometry;
pub mod kind;
pub mod load;
pub mod model;
pub mod problem;
pub mod stats;
pub mod svg;
pub mod table;
pub mod text;
pub mod validate;

#[cfg(feature = "fs")]
pub mod fs;

pub use edit::Drydock;
pub use geometry::{
    Bounds, Cell, Dir, Footprint, GridPoint, HullLoop, HullSegment, Pieces, PlacedShape, ShipGrid,
};
pub use kind::Kind;
pub use load::{Content, Pack, SCHEMA_VERSION, ron_options};
pub use model::*;
pub use problem::{Problem, Severity};
pub use stats::{BonusInfo, StatLine};
pub use table::Table;
pub use text::{Link, parse_links, text_key};
pub use validate::validate;

#[cfg(feature = "embed")]
mod embedded {
    include!(concat!(env!("OUT_DIR"), "/base_pack.rs"));
}

/// The files of the embedded base pack (`content/base` without `wiki/`), as `(path, contents)`.
#[cfg(feature = "embed")]
pub fn base_pack_files() -> &'static [(&'static str, &'static str)] {
    embedded::BASE_PACK_FILES
}

/// The embedded base pack, parsed. Combine with mod packs: `Content::load(&[data::base_pack()?, mod_pack])`.
#[cfg(feature = "embed")]
pub fn base_pack() -> Result<Pack, Vec<Problem>> {
    Pack::from_files(embedded::BASE_PACK_FILES.iter().copied())
}

/// The embedded base pack, merged and validated.
#[cfg(feature = "embed")]
pub fn try_base_content() -> Result<Content, Vec<Problem>> {
    Content::load(&[base_pack()?])
}

/// The embedded base content. Panics if the embedded pack is invalid, which the crate's tests rule out.
#[cfg(feature = "embed")]
pub fn base_content() -> Content {
    match try_base_content() {
        Ok(c) => c,
        Err(problems) => {
            let list: Vec<String> = problems.iter().map(|p| p.to_string()).collect();
            panic!("embedded base content is invalid:\n{}", list.join("\n"))
        }
    }
}
