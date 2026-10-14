//! Content packs: the serde schema of every kind ([`model`]), the pack loader from in-memory files ([`load`]),
//! validation ([`validate`]) with its problems, text keys and links, the art keys, the icon SVG check and the
//! limits on outside input.

pub mod art_keys;
pub mod kind;
pub mod limits;
pub mod load;
pub mod model;
pub mod problem;
pub mod svg;
pub mod table;
pub mod text;
pub mod validate;
