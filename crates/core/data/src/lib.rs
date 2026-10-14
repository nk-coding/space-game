//! Game content for Space Game: schema, pack loader, validation, stat lines, shared blueprint geometry, the
//! blueprint text format of drydock designs, and the pure models of the drydock ([`design::edit`]), the behavior
//! editor ([`behavior::edit`]) and the player's progression ([`career`]).
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
//! let outline = data::design::geometry::hull_loops(&grid.piece_hull_edges(&pieces, main));
//! ```

pub mod behavior;
pub mod career;
pub mod content;
pub mod design;
pub mod hash;

#[cfg(feature = "fs")]
pub mod fs;

pub use behavior::edit::BehaviorEditor;
pub use career::Career;
pub use content::kind::Kind;
pub use content::load::{Content, Pack, SCHEMA_VERSION, ron_options};
pub use content::model::*;
pub use content::problem::{Problem, Severity};
pub use content::table::Table;
pub use content::text::{Link, parse_links, text_key};
pub use content::validate::{validate, validate_ship};
pub use design::blueprint::{blueprint_to_ron, parse_blueprint};
pub use design::edit::Drydock;
pub use design::geometry::{
    Bounds, Cell, Dir, Footprint, GridPoint, HullLoop, HullSegment, Pieces, PlacedShape, ShipGrid,
};
pub use design::stats::{BonusInfo, ShipTotals, StatLine};

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
