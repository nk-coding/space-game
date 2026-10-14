//! What the player owns (ADR-0038: moved out of the drydock, which edits it, so the scene, voyages, the career and
//! every screen read it without the drydock):
//!
//! * [`design`]: [`PlayerDesign`], the blueprint every flight of the player flies, and whether a design can fly.
//! * [`behaviors`]: the player's behaviors on the storage layer ([`behaviors::PlayerBehaviors`]), the behavior a
//!   design names, checks with the player's behaviors known.
//! * [`library`]: the saved designs ([`library::DesignDoc`]), the working design and the autosave, ids and names.

pub mod behaviors;
pub mod design;
pub mod library;

pub use design::PlayerDesign;
